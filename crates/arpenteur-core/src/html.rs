use std::sync::LazyLock;

use ego_tree::NodeRef;
use regex::Regex;
use scraper::{ElementRef, Html, Node, Selector};
use sha2::{Digest, Sha256};
use url::Url;

use crate::model::{Image, JsonLd, Link, PageFacts};
use crate::urls::normalize_url;

const HIDDEN_TEXT: [&str; 4] = ["script", "style", "noscript", "template"];

fn sel(css: &str) -> Selector {
    Selector::parse(css).expect("sélecteur statique valide")
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn element_text(el: ElementRef) -> String {
    collapse(&el.text().collect::<String>())
}

fn has_rel(el: &ElementRef, token: &str) -> bool {
    el.value().attr("rel").is_some_and(|rel| {
        rel.split_whitespace()
            .any(|t| t.eq_ignore_ascii_case(token))
    })
}

fn resolve(base: &Url, href: &str) -> Option<String> {
    let url = base.join(href.trim()).ok()?;
    matches!(url.scheme(), "http" | "https").then_some(())?;
    normalize_url(url.as_str())
}

fn collect_visible(root: NodeRef<Node>, out: &mut Vec<String>) {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        match node.value() {
            Node::Text(text) => out.extend(text.split_whitespace().map(str::to_lowercase)),
            Node::Element(el) if HIDDEN_TEXT.contains(&el.name()) => {}
            _ => stack.extend(node.children().rev()),
        }
    }
}

/// Texte visible d'un sous-arbre, un séparateur `|` entre nœuds texte pour
/// qu'un numéro ne puisse pas se recoller à travers deux éléments.
fn collect_visible_text(root: NodeRef<Node>) -> String {
    let mut out = String::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        match node.value() {
            Node::Text(text) => {
                out.push_str(&collapse(text));
                out.push('|');
            }
            Node::Element(el) if HIDDEN_TEXT.contains(&el.name()) => {}
            _ => stack.extend(node.children().rev()),
        }
    }
    out
}

static PHONE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:\+33\s?(?:\(0\)\s?)?|0)[1-9](?:[\s.\-]?\d{2}){4}")
        .expect("regex statique valide")
});

fn is_phone_sep(c: char) -> bool {
    c.is_whitespace() || c == '.' || c == '-'
}

/// Vrai si le numéro est collé à d'autres chiffres (SIRET, prix, suite de
/// nombres) avant ou après, séparateur éventuel compris.
fn touches_digits(text: &str, start: usize, end: usize) -> bool {
    let mut before = text[..start].chars().rev();
    match before.next() {
        Some(c) if c.is_ascii_digit() => return true,
        Some(c) if is_phone_sep(c) && before.next().is_some_and(|d| d.is_ascii_digit()) => {
            return true;
        }
        _ => {}
    }
    let mut after = text[end..].chars();
    match after.next() {
        Some(c) if c.is_ascii_digit() => true,
        Some(c) if is_phone_sep(c) => after.next().is_some_and(|d| d.is_ascii_digit()),
        _ => false,
    }
}

/// Chiffres d'un numéro, `+33`/`0033` ramené à `0`.
fn normalize_phone(raw: &str) -> String {
    let mut cleaned = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            chars.next();
            chars.next();
        } else {
            cleaned.push(c);
        }
    }
    let international = cleaned.trim_start().starts_with('+');
    let mut digits: String = cleaned.chars().filter(char::is_ascii_digit).collect();
    if let Some(rest) = digits.strip_prefix("0033") {
        digits = format!("0{}", rest.strip_prefix('0').unwrap_or(rest));
    } else if international && let Some(rest) = digits.strip_prefix("33") {
        digits = format!("0{}", rest.strip_prefix('0').unwrap_or(rest));
    }
    digits
}

fn unlinked_phones(text: &str, linked: &[String]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for m in PHONE_RE.find_iter(text) {
        if touches_digits(text, m.start(), m.end()) {
            continue;
        }
        let digits = normalize_phone(m.as_str());
        if linked.contains(&digits) || seen.contains(&digits) {
            continue;
        }
        seen.push(digits);
        out.push(m.as_str().to_owned());
    }
    out
}

fn json_ld_types(value: &serde_json::Value, types: &mut Vec<String>) {
    let push = |v: Option<&serde_json::Value>, types: &mut Vec<String>| match v {
        Some(serde_json::Value::String(s)) => types.push(s.clone()),
        Some(serde_json::Value::Array(items)) => {
            types.extend(items.iter().filter_map(|i| i.as_str().map(str::to_owned)))
        }
        _ => {}
    };
    push(value.get("@type"), types);
    if let Some(graph) = value.get("@graph").and_then(|g| g.as_array()) {
        for item in graph {
            push(item.get("@type"), types);
        }
    }
}

fn parse_json_ld(raw: &str) -> JsonLd {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return JsonLd {
            valid: false,
            types: Vec::new(),
        };
    };
    let mut types = Vec::new();
    match &value {
        serde_json::Value::Array(items) => items.iter().for_each(|i| json_ld_types(i, &mut types)),
        other => json_ld_types(other, &mut types),
    }
    JsonLd { valid: true, types }
}

/// Extrait les faits SEO d'une page HTML. Les URL sont résolues contre
/// `<base href>` s'il existe, sinon contre `page_url`.
pub fn extract_page(html: &str, page_url: &Url) -> PageFacts {
    let doc = Html::parse_document(html);

    let base = doc
        .select(&sel("base[href]"))
        .next()
        .and_then(|b| b.value().attr("href"))
        .and_then(|href| page_url.join(href.trim()).ok())
        .unwrap_or_else(|| page_url.clone());

    let title = doc.select(&sel("title")).next().map(element_text);

    let meta_content = |name: &str| {
        doc.select(&sel("meta[name][content]"))
            .find(|m| {
                m.value()
                    .attr("name")
                    .is_some_and(|n| n.eq_ignore_ascii_case(name))
            })
            .and_then(|m| m.value().attr("content"))
            .map(collapse)
    };

    let h1 = doc.select(&sel("h1")).map(element_text).collect();

    let canonical = doc
        .select(&sel("link[rel][href]"))
        .find(|l| has_rel(l, "canonical"))
        .and_then(|l| resolve(&base, l.value().attr("href")?));

    let links = doc
        .select(&sel("a[href]"))
        .filter_map(|a| {
            let href = a.value().attr("href")?;
            if href.trim_start().starts_with('#') {
                return None;
            }
            Some(Link {
                href: resolve(&base, href)?,
                anchor: element_text(a),
                nofollow: has_rel(&a, "nofollow"),
            })
        })
        .collect();

    let images = doc
        .select(&sel("img[src]"))
        .filter_map(|img| {
            Some(Image {
                src: resolve(&base, img.value().attr("src")?)?,
                alt: img.value().attr("alt").map(str::to_owned),
            })
        })
        .collect();

    let json_ld = doc
        .select(&sel("script[type]"))
        .filter(|s| {
            s.value()
                .attr("type")
                .is_some_and(|t| t.trim().eq_ignore_ascii_case("application/ld+json"))
        })
        .map(|s| parse_json_ld(&s.text().collect::<String>()))
        .collect();

    let lang = doc
        .select(&sel("html[lang]"))
        .next()
        .and_then(|h| h.value().attr("lang"))
        .map(|l| l.trim().to_owned())
        .filter(|l| !l.is_empty());

    let headings = doc
        .select(&sel("h1, h2, h3, h4, h5, h6"))
        .filter_map(|h| h.value().name().strip_prefix('h')?.parse::<u8>().ok())
        .collect();

    let linked_phones: Vec<String> = doc
        .select(&sel("a[href]"))
        .filter_map(|a| {
            let href = a.value().attr("href")?.trim();
            let scheme = href.get(..4)?;
            scheme
                .eq_ignore_ascii_case("tel:")
                .then(|| normalize_phone(&href[4..]))
        })
        .collect();

    let mut words = Vec::new();
    let mut visible_text = String::new();
    if let Some(body) = doc.select(&sel("body")).next() {
        collect_visible(*body, &mut words);
        visible_text = collect_visible_text(*body);
    }
    let unlinked_phones = unlinked_phones(&visible_text, &linked_phones);
    let text_hash = Sha256::digest(words.join(" ").as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();

    PageFacts {
        title,
        meta_description: meta_content("description"),
        h1,
        canonical,
        meta_robots: meta_content("robots"),
        links,
        images,
        json_ld,
        word_count: words.len() as u32,
        text_hash,
        lang,
        headings,
        unlinked_phones,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    fn page() -> Url {
        Url::parse("https://x.fr/dir/page").unwrap()
    }

    fn facts(html: &str) -> crate::model::PageFacts {
        extract_page(html, &page())
    }

    #[test]
    fn extracts_title_meta_h1_and_robots() {
        let f = facts(
            r#"<html><head><title> Mon  titre </title>
            <meta name="description" content="Une description">
            <meta name="robots" content="noindex, follow"></head>
            <body><h1>Un</h1><h1>Deux</h1></body></html>"#,
        );
        assert_eq!(f.title.as_deref(), Some("Mon titre"));
        assert_eq!(f.meta_description.as_deref(), Some("Une description"));
        assert_eq!(f.h1, vec!["Un", "Deux"]);
        assert_eq!(f.meta_robots.as_deref(), Some("noindex, follow"));
    }

    #[test]
    fn first_title_wins_when_duplicated() {
        let f = facts("<head><title>A</title><title>B</title></head>");
        assert_eq!(f.title.as_deref(), Some("A"));
    }

    #[test]
    fn resolves_relative_canonical() {
        let f = facts(r#"<head><link rel="canonical" href="../a"></head>"#);
        assert_eq!(f.canonical.as_deref(), Some("https://x.fr/a"));
    }

    #[test]
    fn respects_base_href() {
        let f =
            facts(r#"<head><base href="https://cdn.x.fr/"></head><body><a href="a">L</a></body>"#);
        assert_eq!(f.links[0].href, "https://cdn.x.fr/a");
    }

    #[test]
    fn rel_canonical_is_case_insensitive() {
        let f = facts(r#"<head><link REL="Canonical" href="/c"></head>"#);
        assert_eq!(f.canonical.as_deref(), Some("https://x.fr/c"));
        let g = facts(r#"<head><link rel="alternate" href="/c"></head>"#);
        assert_eq!(g.canonical, None);
    }

    #[test]
    fn no_canonical_gives_none() {
        assert_eq!(facts("<head></head>").canonical, None);
    }

    #[test]
    fn json_ld_valid_with_types() {
        let f = facts(
            r#"<script type="application/ld+json">{"@type":"Organization"}</script>
            <script type="application/ld+json">{"@graph":[{"@type":"A"},{"@type":["B","C"]}]}</script>"#,
        );
        assert_eq!(f.json_ld.len(), 2);
        assert!(f.json_ld[0].valid);
        assert_eq!(f.json_ld[0].types, vec!["Organization"]);
        assert!(f.json_ld[1].valid);
        assert_eq!(f.json_ld[1].types, vec!["A", "B", "C"]);
    }

    #[test]
    fn json_ld_invalid() {
        let f = facts(r#"<script type="application/ld+json">{</script>"#);
        assert!(!f.json_ld[0].valid);
        assert!(f.json_ld[0].types.is_empty());
    }

    #[test]
    fn links_resolved_with_anchor_and_nofollow() {
        let f = facts(r#"<a href="/b#x" rel="nofollow">Voir B</a>"#);
        assert_eq!(f.links.len(), 1);
        assert_eq!(f.links[0].href, "https://x.fr/b");
        assert_eq!(f.links[0].anchor, "Voir B");
        assert!(f.links[0].nofollow);
    }

    #[test]
    fn ignores_non_http_links() {
        let f = facts(
            r##"<a href="mailto:a@b.fr">m</a><a href="tel:123">t</a>
            <a href="javascript:void(0)">j</a><a href="#ancre">a</a>"##,
        );
        assert!(f.links.is_empty());
    }

    #[test]
    fn image_alt_none_versus_empty() {
        let f = facts(r#"<img src="a.jpg"><img src="b.jpg" alt="">"#);
        assert_eq!(f.images[0].src, "https://x.fr/dir/a.jpg");
        assert_eq!(f.images[0].alt, None);
        assert_eq!(f.images[1].alt.as_deref(), Some(""));
    }

    #[test]
    fn word_count_ignores_script_style_noscript() {
        let f = facts(
            "<body>un <b>deux</b> trois<script>var a = 1 2 3;</script>\
             <style>p { color: red }</style><noscript>pas vu</noscript></body>",
        );
        assert_eq!(f.word_count, 3);
    }

    #[test]
    fn text_hash_ignores_markup_and_whitespace() {
        let a = facts("<body><p>Un  Deux</p></body>");
        let b = facts("<body><div>un</div>\n\n<span>deux</span></body>");
        let c = facts("<body><p>Un Trois</p></body>");
        assert_eq!(a.text_hash, b.text_hash);
        assert_ne!(a.text_hash, c.text_hash);
    }

    #[test]
    fn deeply_nested_page_does_not_overflow() {
        let depth = 20000;
        let html = format!(
            "<body>{}un deux{}</body>",
            "<div>".repeat(depth),
            "</div>".repeat(depth)
        );
        let handle = std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(move || facts(&html).word_count)
            .unwrap();
        assert_eq!(handle.join().unwrap(), 2);
    }

    #[test]
    fn latin1_page_does_not_panic() {
        let bytes = b"<html><head><title>caf\xe9</title></head></html>";
        let f = facts(&String::from_utf8_lossy(bytes));
        assert!(f.title.is_some());
    }

    #[test]
    fn extracts_html_lang() {
        let f = facts(r#"<html lang="fr-FR"><body>x</body></html>"#);
        assert_eq!(f.lang.as_deref(), Some("fr-FR"));
        assert_eq!(facts("<html><body>x</body></html>").lang, None);
    }

    #[test]
    fn extracts_heading_levels_in_order() {
        let f = facts("<body><h1>a</h1><h2>b</h2><h3>c</h3><h2>d</h2></body>");
        assert_eq!(f.headings, vec![1, 2, 3, 2]);
    }

    #[test]
    fn detects_unlinked_french_phone() {
        let f = facts("<body><p>Appelez le 02 99 12 34 56</p></body>");
        assert_eq!(f.unlinked_phones, vec!["02 99 12 34 56"]);
    }

    #[test]
    fn linked_phone_is_not_reported() {
        let f = facts(
            r#"<body><a href="tel:+33299123456">02 99 12 34 56</a> ou 02.99.12.34.56</body>"#,
        );
        assert!(f.unlinked_phones.is_empty());
    }

    #[test]
    fn siret_and_prices_are_not_phones() {
        let f = facts(
            "<body><p>SIRET 123 456 789 00012</p><p>Prix 1 290,00 \u{20ac}</p>\
             <p>12 02 99 12 34 56</p><p>02 99 12 34 56 78</p></body>",
        );
        assert!(f.unlinked_phones.is_empty(), "{:?}", f.unlinked_phones);
    }

    #[test]
    fn international_phone_with_zero_in_parens_is_detected_once() {
        let f = facts("<body>+33 (0)2 99 12 34 56 puis 0299123456</body>");
        assert_eq!(f.unlinked_phones.len(), 1, "{:?}", f.unlinked_phones);
    }

    #[test]
    fn old_facts_without_new_fields_deserialize() {
        let json = r#"{"title":null,"metaDescription":null,"h1":[],"canonical":null,
            "metaRobots":null,"links":[],"images":[],"jsonLd":[],"wordCount":0,"textHash":""}"#;
        let f: crate::model::PageFacts = serde_json::from_str(json).unwrap();
        assert_eq!(f.lang, None);
        assert!(f.headings.is_empty());
        assert!(f.unlinked_phones.is_empty());
    }
}
