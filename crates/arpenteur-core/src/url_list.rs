//! Liste d'URL collée par l'utilisateur pour le mode liste.

use std::collections::HashSet;

use url::Url;

use crate::model::CrawlSettings;
use crate::urls::is_same_site;

/// Une URL par ligne. Ignore lignes vides, espaces, BOM et doublons. `Err` : un message par ligne fautive.
pub fn parse_url_list(input: &str) -> Result<Vec<Url>, Vec<String>> {
    let mut urls: Vec<Url> = Vec::new();
    let mut errors = Vec::new();
    let mut first: Option<Url> = None;
    let mut seen: HashSet<String> = HashSet::new();
    for (index, line) in input.lines().enumerate() {
        // Deux colonnes copiées d'un tableur : seule la première est l'adresse.
        let first_field = line.split('\t').next().unwrap_or_default();
        let line = first_field.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
        if line.is_empty() {
            continue;
        }
        let number = index + 1;
        if line.contains(char::is_whitespace) {
            errors.push(format!(
                "ligne {number} : « {line} » contient une espace : une seule adresse par ligne"
            ));
            continue;
        }
        let parsed = Url::parse(line)
            .ok()
            .filter(|u| matches!(u.scheme(), "http" | "https") && u.host_str().is_some());
        let Some(mut url) = parsed else {
            errors.push(format!(
                "ligne {number} : « {line} » n'est pas une adresse http ou https"
            ));
            continue;
        };
        if first.as_ref().is_some_and(|f| !is_same_site(f, &url)) {
            errors.push(format!(
                "ligne {number} : « {line} » n'est pas sur le même site que la première adresse"
            ));
            continue;
        }
        url.set_fragment(None);
        first.get_or_insert_with(|| url.clone());
        if seen.insert(url.to_string()) {
            urls.push(url);
        }
    }
    match (errors.is_empty(), urls.is_empty()) {
        (false, _) => Err(errors),
        (true, true) => Err(vec!["La liste est vide.".to_string()]),
        (true, false) => Ok(urls),
    }
}

/// `Err` si la liste dépasse `settings.max_requests`.
pub fn check_list_size(urls: &[Url], settings: &CrawlSettings) -> Result<(), String> {
    if urls.len() > settings.max_requests as usize {
        return Err(format!(
            "La liste compte {} adresses, la limite de requêtes est de {}.",
            grouped(urls.len() as u64),
            grouped(u64::from(settings.max_requests)),
        ));
    }
    Ok(())
}

/// Milliers séparés par une espace : 12 000.
fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(urls: &[Url]) -> Vec<&str> {
        urls.iter().map(Url::as_str).collect()
    }

    #[test]
    fn parses_a_pasted_list() {
        let pasted = "\u{feff}https://exemple.fr/\r\n\r\n  https://exemple.fr/tarifs  \r\nhttps://exemple.fr/\r\n";
        let urls = parse_url_list(pasted).unwrap();
        assert_eq!(
            strings(&urls),
            ["https://exemple.fr/", "https://exemple.fr/tarifs"]
        );
    }

    #[test]
    fn fragments_do_not_make_a_new_address() {
        let urls = parse_url_list("https://exemple.fr/a#haut\nhttps://exemple.fr/a\n").unwrap();
        assert_eq!(strings(&urls), ["https://exemple.fr/a"]);
    }

    #[test]
    fn www_and_apex_are_the_same_site() {
        let urls = parse_url_list("https://exemple.fr/\nhttps://www.exemple.fr/a\n").unwrap();
        assert_eq!(urls.len(), 2);
    }

    #[test]
    fn reports_faulty_lines_with_their_number() {
        let input = "https://exemple.fr/\nftp://x\n\nhttps://autre.fr/\nhttps://exemple.fr/ok\n";
        let errors = parse_url_list(input).unwrap_err();
        assert_eq!(
            errors,
            [
                "ligne 2 : « ftp://x » n'est pas une adresse http ou https",
                "ligne 4 : « https://autre.fr/ » n'est pas sur le même site que la première adresse",
            ]
        );
    }

    #[test]
    fn a_line_without_scheme_is_not_an_address() {
        let errors = parse_url_list("exemple.fr/a\n").unwrap_err();
        assert_eq!(
            errors,
            ["ligne 1 : « exemple.fr/a » n'est pas une adresse http ou https"]
        );
    }

    #[test]
    fn spreadsheet_columns_keep_the_first_field() {
        let urls = parse_url_list(
            "https://exemple.fr/tarifs\tTarifs\nhttps://exemple.fr/a\t\thttps://exemple.fr/b\n",
        )
        .unwrap();
        assert_eq!(
            strings(&urls),
            ["https://exemple.fr/tarifs", "https://exemple.fr/a"]
        );
    }

    #[test]
    fn empty_first_column_is_an_empty_line() {
        let urls = parse_url_list("https://exemple.fr/\n\thttps://exemple.fr/x\n").unwrap();
        assert_eq!(strings(&urls), ["https://exemple.fr/"]);
        assert_eq!(
            parse_url_list("\thttps://exemple.fr/x\n").unwrap_err(),
            ["La liste est vide."]
        );
    }

    #[test]
    fn inner_space_is_refused_with_its_line_number() {
        let errors = parse_url_list(
            "https://exemple.fr/\n\nhttps://exemple.fr/a b\nhttps://exemple.fr/ok\n",
        )
        .unwrap_err();
        assert_eq!(
            errors,
            [
                "ligne 3 : « https://exemple.fr/a b » contient une espace : une seule adresse par ligne"
            ]
        );
    }

    #[test]
    fn empty_list_is_refused() {
        for input in ["", "\n\r\n  \n", "\u{feff}"] {
            assert_eq!(
                parse_url_list(input).unwrap_err(),
                ["La liste est vide."],
                "{input:?}"
            );
        }
    }

    #[test]
    fn list_larger_than_the_request_limit_is_refused() {
        let settings = CrawlSettings {
            max_requests: 2,
            ..CrawlSettings::default()
        };
        let urls = |n: usize| -> Vec<Url> {
            (0..n)
                .map(|i| Url::parse(&format!("https://exemple.fr/{i}")).unwrap())
                .collect()
        };
        assert_eq!(
            check_list_size(&urls(3), &settings).unwrap_err(),
            "La liste compte 3 adresses, la limite de requêtes est de 2."
        );
        assert!(check_list_size(&urls(2), &settings).is_ok());
    }

    #[test]
    fn list_size_message_groups_thousands() {
        let urls: Vec<Url> = (0..12_000)
            .map(|i| Url::parse(&format!("https://exemple.fr/{i}")).unwrap())
            .collect();
        assert_eq!(
            check_list_size(&urls, &CrawlSettings::default()).unwrap_err(),
            "La liste compte 12 000 adresses, la limite de requêtes est de 10 000."
        );
    }
}
