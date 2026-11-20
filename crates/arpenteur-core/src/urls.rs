use url::Url;

/// URL normalisée : sans fragment, hôte en minuscules, port par défaut retiré,
/// chemin vide devenu `/`. `None` si l'URL est invalide.
pub fn normalize_url(raw: &str) -> Option<String> {
    let mut url = Url::parse(raw.trim()).ok()?;
    url.set_fragment(None);
    Some(url.to_string())
}

/// Hôtes égaux une fois le `www.` de tête retiré.
pub fn is_same_site(a: &Url, b: &Url) -> bool {
    match (a.host_str(), b.host_str()) {
        (Some(x), Some(y)) => bare(x) == bare(y),
        _ => false,
    }
}

fn bare(host: &str) -> String {
    let host = host.to_ascii_lowercase();
    host.strip_prefix("www.").map(str::to_owned).unwrap_or(host)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_drops_fragment_and_default_port() {
        assert_eq!(
            normalize_url("HTTPS://X.fr:443/a#top").as_deref(),
            Some("https://x.fr/a")
        );
        assert_eq!(normalize_url("https://x.fr").as_deref(), Some("https://x.fr/"));
    }

    #[test]
    fn normalize_rejects_invalid() {
        assert_eq!(normalize_url("pas une url"), None);
    }

    #[test]
    fn www_and_apex_are_same_site() {
        let u = |s: &str| Url::parse(s).unwrap();
        assert!(is_same_site(&u("https://x.fr/"), &u("https://www.x.fr/a")));
        assert!(!is_same_site(&u("https://x.fr/"), &u("https://y.fr/")));
        assert!(!is_same_site(&u("https://sub.x.fr/"), &u("https://x.fr/")));
    }
}
