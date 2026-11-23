//! Constats de site tirés du rapport JSON de SiteOne (`siteone.json`).

use serde_json::Value;

use url::Url;

use crate::model::{BlockedUrl, Finding, FindingCode};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SiteOneSummary {
    pub certificate_errors: Vec<String>,
    pub certificate_expired: bool,
    pub certificate_issuer: Option<String>,
    pub robots_blocked: Vec<BlockedUrl>,
}

fn rows<'a>(root: &'a Value, table: &str) -> &'a [Value] {
    root.pointer(&format!("/tables/{table}/rows"))
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn text<'a>(row: &'a Value, key: &str) -> &'a str {
    row.get(key).and_then(Value::as_str).unwrap_or("").trim()
}

/// Début des messages d'échec de connexion de l'analyse TLS de SiteOne : elle ouvre
/// sa propre connexion, directe, donc un échec ne dit rien du certificat du site.
const CONNECTION_FAILURES: [&str; 3] = [
    "Unable to connect to ",
    "Unable to create TLS connection to ",
    "Invalid hostname ",
];

fn is_connection_failure(message: &str) -> bool {
    CONNECTION_FAILURES.iter().any(|p| message.starts_with(p))
}

/// Raison notée par SiteOne pour une adresse écartée par robots.txt.
const ROBOTS_REASON: &str = "Robots.txt";

/// SiteOne écrit sans schéma ni hôte les adresses de l'hôte de départ (`/prive?x=1`) :
/// on les rend absolues avec l'origine du crawl. Tout ce qui n'est ni de ce genre ni
/// une adresse http(s) complète est ignoré.
fn absolute(value: &str, origin: &Url) -> Option<String> {
    if value.starts_with('/') && !value.starts_with("//") {
        return origin.join(value).ok().map(String::from);
    }
    let url = Url::parse(value).ok()?;
    matches!(url.scheme(), "http" | "https").then(|| value.to_string())
}

fn robots_blocked(root: &Value, start_url: &str) -> Vec<BlockedUrl> {
    let Ok(origin) = Url::parse(start_url) else {
        return Vec::new();
    };
    rows(root, "skipped")
        .iter()
        .filter(|row| text(row, "reason") == ROBOTS_REASON)
        .filter_map(|row| {
            Some(BlockedUrl {
                url: absolute(text(row, "url"), &origin)?,
                found_on: absolute(text(row, "sourceUqId"), &origin),
            })
        })
        .collect()
}

pub fn read_siteone(json: &str, start_url: &str) -> Option<SiteOneSummary> {
    let root: Value = serde_json::from_str(json).ok()?;
    if !root.is_object() {
        return None;
    }
    let mut summary = SiteOneSummary::default();

    let mut expiry_note = None;
    for row in rows(&root, "certificate-info") {
        let value = text(row, "value");
        match text(row, "info") {
            "Issuer" if !value.is_empty() => summary.certificate_issuer = Some(value.to_string()),
            "Errors" if !value.is_empty() && !is_connection_failure(value) => {
                summary.certificate_errors.push(value.to_string())
            }
            "Valid to" if value.contains("EXPIRED") => {
                summary.certificate_expired = true;
                expiry_note = Some(value.to_string());
            }
            _ => {}
        }
    }
    // « Valid to » et « Errors » disent la même expiration : une seule mention,
    // quel que soit l'ordre des lignes.
    if let Some(note) = expiry_note {
        let already_said = summary
            .certificate_errors
            .iter()
            .any(|e| e.to_lowercase().contains("expired"));
        if !already_said {
            summary.certificate_errors.push(note);
        }
    }

    summary.robots_blocked = robots_blocked(&root, start_url);

    Some(summary)
}

pub fn site_findings(summary: &SiteOneSummary, start_url: &str) -> Vec<Finding> {
    let mut out = Vec::new();
    if !summary.certificate_errors.is_empty() || summary.certificate_expired {
        let mut detail = summary.certificate_errors.join(" ; ");
        if let Some(issuer) = &summary.certificate_issuer {
            if !detail.is_empty() {
                detail.push_str(" | ");
            }
            detail.push_str(&format!("Émetteur : {issuer}"));
        }
        out.push(Finding {
            code: FindingCode::HttpsInvalid,
            url: start_url.to_string(),
            related: vec![],
            detail: Some(detail),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: &str = "https://x.fr/";

    fn report(cert_rows: &str) -> String {
        format!(r#"{{"tables":{{"certificate-info":{{"rows":[{cert_rows}]}}}}}}"#)
    }

    const ISSUER: &str = r#"{"info":"Issuer","value":"C = US, O = Zscaler Inc., CN = Zscaler Intermediate Root CA (zscaler.net) (t) "}"#;
    const VALID_TO_OK: &str =
        r#"{"info":"Valid to","value":"Nov 28 02:06:20 2026 GMT (VALID still for 7.4 day(s))"}"#;
    const VALID_TO_EXPIRED: &str =
        r#"{"info":"Valid to","value":"Nov 17 02:06:20 2026 GMT (EXPIRED 3 day(s) ago)"}"#;
    const ERRORS_ROW: &str = r#"{"info":"Errors","value":"TLS handshake failed"}"#;

    fn findings(json: &str) -> Vec<Finding> {
        site_findings(&read_siteone(json, START).unwrap(), START)
    }

    #[test]
    fn valid_certificate_gives_no_finding() {
        let json = report(&[ISSUER, VALID_TO_OK].join(","));
        let summary = read_siteone(&json, START).unwrap();
        assert!(summary.certificate_errors.is_empty());
        assert!(!summary.certificate_expired);
        assert_eq!(
            summary.certificate_issuer.as_deref(),
            Some("C = US, O = Zscaler Inc., CN = Zscaler Intermediate Root CA (zscaler.net) (t)")
        );
        assert!(findings(&json).is_empty());
    }

    #[test]
    fn expired_or_errored_certificate() {
        let expired = findings(&report(&[ISSUER, VALID_TO_EXPIRED].join(",")));
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].code, FindingCode::HttpsInvalid);
        assert_eq!(expired[0].url, START);
        assert!(expired[0].related.is_empty());
        let detail = expired[0].detail.as_deref().unwrap();
        assert!(detail.contains("EXPIRED 3 day(s) ago"), "{detail}");
        assert!(detail.contains("Zscaler Intermediate Root CA"), "{detail}");

        let errored = findings(&report(&[ISSUER, VALID_TO_OK, ERRORS_ROW].join(",")));
        assert_eq!(errored.len(), 1);
        assert_eq!(errored[0].code, FindingCode::HttpsInvalid);
        let detail = errored[0].detail.as_deref().unwrap();
        assert!(detail.contains("TLS handshake failed"), "{detail}");
        assert!(detail.contains("Zscaler"), "{detail}");

        let no_issuer = findings(&report(ERRORS_ROW));
        assert_eq!(no_issuer.len(), 1);
        assert_eq!(no_issuer[0].detail.as_deref(), Some("TLS handshake failed"));
    }

    const CONNECT_FAILURE: &str = r#"{"info":"Errors","value":"Unable to connect to www.exemple.fr:443: Connection refused (os error 61)"}"#;
    const EXPIRED_ERROR: &str =
        r#"{"info":"Errors","value":"SSL/TLS certificate expired 3 day(s) ago."}"#;

    #[test]
    fn connection_failures_of_the_tls_analysis_are_not_findings() {
        for message in [
            "Unable to connect to www.exemple.fr:443: timed out",
            "Unable to create TLS connection to www.exemple.fr:443: oups",
            "Invalid hostname 'exemple_fr': invalid dns name",
        ] {
            let row = format!(r#"{{"info":"Errors","value":"{message}"}}"#);
            let json = report(&row);
            let summary = read_siteone(&json, START).unwrap();
            assert!(summary.certificate_errors.is_empty(), "{message}");
            assert!(findings(&json).is_empty(), "{message}");
        }
        let json = report(&[ISSUER, CONNECT_FAILURE].join(","));
        assert!(findings(&json).is_empty());
        let json = report(&[ISSUER, VALID_TO_OK, CONNECT_FAILURE].join(","));
        assert!(findings(&json).is_empty());
    }

    #[test]
    fn real_failures_still_give_a_finding() {
        for message in [
            "TLS handshake failed (produced no certificate)",
            "Unable to parse certificate: bad der",
            "SSL/TLS certificate is not valid yet, valid from Nov 30 2026.",
        ] {
            let row = format!(r#"{{"info":"Errors","value":"{message}"}}"#);
            let f = findings(&report(&row));
            assert_eq!(f.len(), 1, "{message}");
            assert_eq!(f[0].detail.as_deref(), Some(message));
        }
    }

    #[test]
    fn expiry_is_said_once_in_the_real_row_order() {
        // SiteOne écrit « Valid to » (EXPIRED) puis « Errors » avec le même fait.
        let json = report(&[ISSUER, VALID_TO_EXPIRED, EXPIRED_ERROR].join(","));
        let f = findings(&json);
        assert_eq!(f.len(), 1);
        let detail = f[0].detail.as_deref().unwrap();
        assert_eq!(
            detail.to_lowercase().matches("expired").count(),
            1,
            "{detail}"
        );
        assert!(detail.contains("Zscaler"), "{detail}");

        // Ordre inverse : même résultat.
        let json = report(&[ISSUER, EXPIRED_ERROR, VALID_TO_EXPIRED].join(","));
        let detail = findings(&json)[0].detail.clone().unwrap();
        assert_eq!(
            detail.to_lowercase().matches("expired").count(),
            1,
            "{detail}"
        );

        // Expiré et rien d'autre : la mention de « Valid to » reste.
        let json = report(&[ISSUER, VALID_TO_EXPIRED].join(","));
        let detail = findings(&json)[0].detail.clone().unwrap();
        assert!(detail.contains("EXPIRED 3 day(s) ago"), "{detail}");

        // Une erreur de connexion à côté d'une expiration : l'expiration reste seule.
        let json = report(&[ISSUER, VALID_TO_EXPIRED, CONNECT_FAILURE].join(","));
        let f = findings(&json);
        assert_eq!(f.len(), 1);
        assert!(
            !f[0]
                .detail
                .as_deref()
                .unwrap()
                .contains("Unable to connect")
        );
    }

    #[test]
    fn missing_or_garbled_json_is_none() {
        assert!(read_siteone("", START).is_none());
        assert!(read_siteone("pas du json", START).is_none());
        assert!(read_siteone("[1,2]", START).is_none());
    }

    #[test]
    fn absent_tables_are_tolerated() {
        let summary = read_siteone(r#"{"tables":{}}"#, START).unwrap();
        assert_eq!(summary, SiteOneSummary::default());
        assert!(site_findings(&summary, START).is_empty());
        assert!(read_siteone("{}", START).is_some());
    }

    fn skipped_report(rows: &str) -> String {
        format!(r#"{{"tables":{{"skipped":{{"rows":[{rows}]}}}}}}"#)
    }

    const EXEMPLE: &str = "https://exemple.fr/";

    fn blocked(json: &str) -> Vec<BlockedUrl> {
        read_siteone(json, EXEMPLE).unwrap().robots_blocked
    }

    #[test]
    fn reads_robots_blocked_urls_from_the_skipped_table() {
        let json = skipped_report(
            r#"{"reason":"Robots.txt","url":"/prive?x=1","sourceAttr":"a","sourceUqId":"/"}"#,
        );
        assert_eq!(
            blocked(&json),
            vec![BlockedUrl {
                url: "https://exemple.fr/prive?x=1".into(),
                found_on: Some("https://exemple.fr/".into()),
            }]
        );
    }

    #[test]
    fn ignores_other_reasons_and_tolerates_odd_rows() {
        let json = skipped_report(
            r#"{"reason":"Not allowed host","url":"https://autre.fr/","sourceUqId":"/"},
               {"reason":"Max depth","url":"/profond","sourceUqId":"/"},
               {"reason":"Robots.txt","url":"https://autre.fr/x","sourceUqId":"/a"},
               {"reason":"Robots.txt","sourceUqId":"/a"},
               {"reason":"Robots.txt","url":"","sourceUqId":"/a"},
               {"reason":"Robots.txt","url":"//autre.fr/y","sourceUqId":"/a"},
               {"reason":"Robots.txt","url":"prive","sourceUqId":"/a"},
               {"reason":"Robots.txt","url":"/sans-source"},
               {"reason":"Robots.txt","url":12,"sourceUqId":"/a"}"#,
        );
        assert_eq!(
            blocked(&json),
            vec![
                BlockedUrl {
                    url: "https://autre.fr/x".into(),
                    found_on: Some("https://exemple.fr/a".into()),
                },
                BlockedUrl {
                    url: "https://exemple.fr/sans-source".into(),
                    found_on: None,
                },
            ]
        );
        assert!(blocked(r#"{"tables":{}}"#).is_empty());
        assert!(blocked(&skipped_report("")).is_empty());
        assert!(
            read_siteone(
                &skipped_report(r#"{"reason":"Robots.txt","url":"/x"}"#),
                "pas une adresse"
            )
            .unwrap()
            .robots_blocked
            .is_empty()
        );
    }
}
