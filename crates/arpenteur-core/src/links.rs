use std::collections::{BTreeMap, BTreeSet, VecDeque};

use url::Url;

use crate::model::{InLink, LinkReport, PageRecord};
use crate::urls::{is_same_site, normalize_url};

pub fn analyze_links(pages: &[PageRecord], start_url: &str) -> LinkReport {
    let start = normalize_url(start_url);
    let start_parsed = start.as_deref().and_then(|s| Url::parse(s).ok());

    let mut sorted: Vec<(String, &PageRecord)> = pages
        .iter()
        .filter_map(|p| normalize_url(&p.url).map(|u| (u, p)))
        .collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    let is_internal = |target: &str| match (&start_parsed, Url::parse(target)) {
        (Some(s), Ok(t)) => is_same_site(s, &t),
        _ => false,
    };

    let mut inlinks: BTreeMap<String, Vec<InLink>> = BTreeMap::new();
    let mut link_edges: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut redirects: BTreeMap<String, String> = BTreeMap::new();

    for (url, page) in &sorted {
        if (300..400).contains(&page.status) {
            if let Some(target) = redirect_target(url, page) {
                redirects.insert(url.clone(), target);
            }
            continue;
        }
        if !page.is_html() {
            continue;
        }
        let Some(facts) = &page.facts else { continue };
        let mut seen_targets = BTreeSet::new();
        for link in &facts.links {
            let Some(target) = normalize_url(&link.href) else { continue };
            if &target == url || !is_internal(&target) {
                continue;
            }
            if seen_targets.insert(target.clone()) {
                inlinks.entry(target.clone()).or_default().push(InLink {
                    from: url.clone(),
                    anchor: link.anchor.clone(),
                    nofollow: link.nofollow,
                });
                link_edges.entry(url.clone()).or_default().push(target);
            }
        }
    }

    LinkReport {
        inlinks,
        depth: compute_depth(start, &link_edges, &redirects),
        redirect_chains: compute_chains(&redirects),
    }
}

fn redirect_target(url: &str, page: &PageRecord) -> Option<String> {
    let location = page.location()?.trim();
    if location.is_empty() {
        return None;
    }
    let resolved = Url::parse(url).ok()?.join(location).ok()?;
    normalize_url(resolved.as_str())
}

fn compute_depth(
    start: Option<String>,
    link_edges: &BTreeMap<String, Vec<String>>,
    redirects: &BTreeMap<String, String>,
) -> BTreeMap<String, u32> {
    let mut depth = BTreeMap::new();
    let Some(start) = start else { return depth };
    let mut queue = VecDeque::new();
    depth.insert(start.clone(), 0u32);
    queue.push_back((start, 0u32));

    // 0-1 BFS: redirects cost 0 (front), links cost 1 (back).
    while let Some((url, d)) = queue.pop_front() {
        if depth.get(&url).is_some_and(|&best| best < d) {
            continue;
        }
        if let Some(target) = redirects.get(&url) {
            if depth.get(target).is_none_or(|&best| d < best) {
                depth.insert(target.clone(), d);
                queue.push_front((target.clone(), d));
            }
        }
        for target in link_edges.get(&url).into_iter().flatten() {
            if depth.get(target).is_none_or(|&best| d + 1 < best) {
                depth.insert(target.clone(), d + 1);
                queue.push_back((target.clone(), d + 1));
            }
        }
    }
    depth
}

fn compute_chains(redirects: &BTreeMap<String, String>) -> BTreeMap<String, Vec<String>> {
    let mut chains = BTreeMap::new();
    for first in redirects.keys() {
        let mut hops = Vec::new();
        let mut seen = BTreeSet::from([first.clone()]);
        let mut current = first;
        while let Some(next) = redirects.get(current) {
            hops.push(next.clone());
            if !seen.insert(next.clone()) {
                break;
            }
            current = next;
        }
        if hops.len() >= 2 {
            chains.insert(first.clone(), hops);
        }
    }
    chains
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Link, PageFacts};

    const BASE: &str = "https://x.fr";

    struct PageBuilder(PageRecord);

    fn page(path: &str, status: i32) -> PageBuilder {
        let url = if path.starts_with("http") { path.to_string() } else { format!("{BASE}{path}") };
        PageBuilder(PageRecord {
            url,
            status,
            content_type: if (200..300).contains(&status) { "html".into() } else { String::new() },
            size: None,
            elapsed_ms: 0,
            headers: BTreeMap::new(),
            facts: None,
        })
    }

    impl PageBuilder {
        fn html_links(mut self, links: &[(&str, &str)]) -> PageRecord {
            self.0.facts = Some(PageFacts {
                title: None,
                meta_description: None,
                h1: vec![],
                canonical: None,
                meta_robots: None,
                links: links
                    .iter()
                    .map(|(href, anchor)| Link {
                        href: if href.starts_with("http") { href.to_string() } else { format!("{BASE}{href}") },
                        anchor: anchor.to_string(),
                        nofollow: false,
                    })
                    .collect(),
                images: vec![],
                json_ld: vec![],
                word_count: 0,
                text_hash: String::new(),
            });
            self.0
        }

        fn location(mut self, target: &str) -> PageRecord {
            self.0.headers.insert("location".into(), target.into());
            self.0
        }

        fn done(self) -> PageRecord {
            self.0
        }
    }

    fn u(path: &str) -> String {
        format!("{BASE}{path}")
    }

    #[test]
    fn counts_inlinks_from_distinct_pages_excluding_self() {
        let pages = vec![
            page("/", 200).html_links(&[("/a", "A"), ("/a", "A bis"), ("/", "self")]),
            page("/b", 200).html_links(&[("/a", "depuis b"), ("https://autre.com/z", "ext")]),
            page("/a", 200).html_links(&[("/a", "self")]),
        ];
        let report = analyze_links(&pages, "https://x.fr/");
        let ins = &report.inlinks[&u("/a")];
        assert_eq!(ins.len(), 2);
        assert_eq!(ins[0].from, u("/"));
        assert_eq!(ins[0].anchor, "A");
        assert_eq!(ins[1].from, u("/b"));
        assert!(!report.inlinks.contains_key(&u("/")));
        assert!(!report.inlinks.contains_key("https://autre.com/z"));
    }

    #[test]
    fn depth_from_start_with_transparent_redirects() {
        let pages = vec![
            page("/", 200).html_links(&[("/old", "old")]),
            page("/old", 301).location("/a"),
            page("/a", 200).html_links(&[("/b", "b")]),
            page("/b", 200).html_links(&[]),
        ];
        let report = analyze_links(&pages, "https://x.fr/");
        assert_eq!(report.depth[&u("/")], 0);
        assert_eq!(report.depth[&u("/old")], 1);
        assert_eq!(report.depth[&u("/a")], 1);
        assert_eq!(report.depth[&u("/b")], 2);
    }

    #[test]
    fn unreached_page_has_no_depth() {
        let pages = vec![
            page("/", 200).html_links(&[]),
            page("/orphan", 200).html_links(&[]),
        ];
        let report = analyze_links(&pages, "https://x.fr/");
        assert!(!report.depth.contains_key(&u("/orphan")));
    }

    #[test]
    fn detects_chain_of_two_redirects() {
        let pages = vec![
            page("/r1", 301).location("/r2"),
            page("/r2", 302).location("https://x.fr/final"),
            page("/final", 200).html_links(&[]),
        ];
        let report = analyze_links(&pages, "https://x.fr/");
        assert_eq!(report.redirect_chains[&u("/r1")], vec![u("/r2"), u("/final")]);
        assert_eq!(report.redirect_chains.len(), 1);
    }

    #[test]
    fn single_redirect_is_not_a_chain() {
        let pages = vec![page("/r1", 301).location("/final"), page("/final", 200).done()];
        let report = analyze_links(&pages, "https://x.fr/");
        assert!(report.redirect_chains.is_empty());
    }

    #[test]
    fn redirect_loop_terminates() {
        let pages = vec![page("/x", 301).location("/y"), page("/y", 301).location("/x")];
        let report = analyze_links(&pages, "https://x.fr/");
        assert_eq!(report.redirect_chains[&u("/x")], vec![u("/y"), u("/x")]);
        assert_eq!(report.redirect_chains[&u("/y")], vec![u("/x"), u("/y")]);
    }

    #[test]
    fn www_and_apex_links_are_internal() {
        let pages = vec![page("/", 200).html_links(&[("https://www.x.fr/a", "A")])];
        let report = analyze_links(&pages, "https://x.fr/");
        assert_eq!(report.inlinks["https://www.x.fr/a"].len(), 1);
        assert_eq!(report.depth["https://www.x.fr/a"], 1);
    }

    #[test]
    fn start_redirect_target_gets_depth_zero() {
        let pages = vec![
            page("/", 301).location("https://www.x.fr/"),
            page("https://www.x.fr/", 200).html_links(&[("https://www.x.fr/a", "A")]),
        ];
        let report = analyze_links(&pages, "https://x.fr/");
        assert_eq!(report.depth["https://www.x.fr/"], 0);
        assert_eq!(report.depth["https://www.x.fr/a"], 1);
    }
}
