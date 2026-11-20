use regex::Regex;
use siteone_crawler::utils::extract_pcre_regex_pattern;

use crate::model::CrawlSettings;

pub const MAX_WORKERS: u32 = 8;
pub const MIN_RATE: f64 = 0.1;
pub const MAX_RATE: f64 = 20.0;
pub const MAX_REQUESTS: u32 = 10_000;
pub const MAX_LIMIT_THRESHOLD: u32 = 50;

/// Renvoie tous les problèmes d'un coup, un message par réglage invalide.
pub fn validate_settings(s: &CrawlSettings) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    if !(1..=MAX_WORKERS).contains(&s.workers) {
        errors.push(format!(
            "Connexions simultanées : entre 1 et {MAX_WORKERS} (reçu {}).",
            s.workers
        ));
    }
    if !(MIN_RATE..=MAX_RATE).contains(&s.max_reqs_per_sec) {
        errors.push(format!(
            "Débit : entre 0,1 et 20 requêtes par seconde (reçu {}).",
            s.max_reqs_per_sec
        ));
    }
    if !(1..=MAX_REQUESTS).contains(&s.max_requests) {
        errors.push(format!(
            "Limite de requêtes : entre 1 et 10 000 (reçu {}).",
            s.max_requests
        ));
    }
    if !(1..=MAX_LIMIT_THRESHOLD).contains(&s.limit_threshold) {
        errors.push(format!(
            "Seuil de limitation : entre 1 et {MAX_LIMIT_THRESHOLD} (reçu {}).",
            s.limit_threshold
        ));
    }
    check_patterns("Motif à inclure", &s.include_regex, &mut errors);
    check_patterns("Motif à ignorer", &s.ignore_regex, &mut errors);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Même compilation que SiteOne : un motif qui échoue ici serait ignoré en silence au crawl.
fn check_patterns(label: &str, patterns: &[String], errors: &mut Vec<String>) {
    for p in patterns {
        if p.contains(',') {
            errors.push(format!(
                "{label} invalide : « {p} » contient une virgule, que SiteOne lit comme un séparateur. \
                 Écrivez \\x2C pour une virgule littérale ; une répétition {{n,m}} n'est pas prise en charge \
                 (écrivez par exemple \\d\\d?\\d? ou deux motifs)."
            ));
        } else if Regex::new(&extract_pcre_regex_pattern(p)).is_err() {
            errors.push(format!("{label} invalide : « {p} » ne compile pas."));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::CrawlStore;

    #[test]
    fn rejects_out_of_range_values() {
        assert!(validate_settings(&CrawlSettings::default()).is_ok());
        let bad = CrawlSettings {
            workers: 9,
            max_reqs_per_sec: 0.0,
            max_requests: 0,
            limit_threshold: 0,
            ..CrawlSettings::default()
        };
        let errors = validate_settings(&bad).unwrap_err();
        assert_eq!(errors.len(), 4);
        assert!(errors.iter().any(|e| e.starts_with("Connexions")));
        assert!(errors.iter().any(|e| e.starts_with("Débit")));
        assert!(errors.iter().any(|e| e.starts_with("Limite")));
        assert!(errors.iter().any(|e| e.starts_with("Seuil")));
    }

    #[test]
    fn accepts_bounds_and_refuses_just_beyond() {
        let ok = |f: fn(&mut CrawlSettings)| {
            let mut s = CrawlSettings::default();
            f(&mut s);
            validate_settings(&s).is_ok()
        };
        assert!(ok(|s| s.workers = 1) && ok(|s| s.workers = 8) && !ok(|s| s.workers = 0));
        assert!(ok(|s| s.max_reqs_per_sec = 0.1) && ok(|s| s.max_reqs_per_sec = 20.0));
        assert!(!ok(|s| s.max_reqs_per_sec = 20.1) && !ok(|s| s.max_reqs_per_sec = f64::NAN));
        assert!(ok(|s| s.max_requests = 10_000) && !ok(|s| s.max_requests = 10_001));
        assert!(ok(|s| s.limit_threshold = 50) && !ok(|s| s.limit_threshold = 51));
    }

    #[test]
    fn rejects_invalid_regex() {
        let bad = CrawlSettings {
            include_regex: vec!["(".to_string(), "^/blog".to_string()],
            ignore_regex: vec!["[a-".to_string()],
            ..CrawlSettings::default()
        };
        let errors = validate_settings(&bad).unwrap_err();
        assert_eq!(errors.len(), 2);
        assert!(errors[0].contains("« ( »"));
        assert!(errors[1].contains("[a-"));
    }

    #[test]
    fn rejects_regex_with_comma_because_siteone_splits_it() {
        let with = |r: &str| CrawlSettings {
            include_regex: vec![r.to_string()],
            ignore_regex: vec![r.to_string()],
            ..CrawlSettings::default()
        };
        let errors = validate_settings(&with("a{1,3}")).unwrap_err();
        assert_eq!(errors.len(), 2);
        assert!(errors[0].contains("« a{1,3} »") && errors[0].contains("\\x2C"));
        assert!(validate_settings(&with("a,b")).is_err());
        assert!(validate_settings(&with("a{1}")).is_ok());
        assert!(validate_settings(&with("a\\x2Cb")).is_ok());
    }

    #[test]
    fn default_settings_roundtrip_and_fallback() {
        let tmp = tempfile::tempdir().unwrap();
        let store = CrawlStore::new(tmp.path().to_path_buf());
        assert_eq!(store.read_default_settings().workers, 1);

        let saved = CrawlSettings {
            workers: 4,
            max_reqs_per_sec: 0.5,
            include_regex: vec!["^/blog".to_string()],
            ..CrawlSettings::default()
        };
        store.write_default_settings(&saved).unwrap();
        let read = store.read_default_settings();
        assert_eq!((read.workers, read.max_reqs_per_sec), (4, 0.5));
        assert_eq!(read.include_regex, ["^/blog"]);

        std::fs::write(tmp.path().join("settings.json"), r#"{"workers": 3}"#).unwrap();
        let partial = store.read_default_settings();
        assert_eq!((partial.workers, partial.limit_threshold), (3, 5));

        std::fs::write(tmp.path().join("settings.json"), "pas du json").unwrap();
        assert_eq!(store.read_default_settings().workers, 1);

        store.write_default_settings(&saved).unwrap();
        assert!(!tmp.path().join("settings.json.tmp").exists());
    }
}
