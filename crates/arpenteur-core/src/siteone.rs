//! Constats de site tirés du rapport JSON de SiteOne (`siteone.json`).

use serde_json::Value;

use crate::model::{Finding, FindingCode};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SecurityIssue {
    pub header: String,
    pub recommendation: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SiteOneSummary {
    pub certificate_errors: Vec<String>,
    pub certificate_expired: bool,
    pub certificate_issuer: Option<String>,
    pub security_issues: Vec<SecurityIssue>,
}

fn rows<'a>(root: &'a Value, table: &str) -> &'a [Value] {
    root.pointer(&format!("/tables/{table}/rows"))
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn text<'a>(row: &'a Value, key: &str) -> &'a str {
    row.get(key).and_then(Value::as_str).unwrap_or("").trim()
}

/// Les colonnes de la table `security` sont des compteurs d'URL, en texte.
fn count(row: &Value, key: &str) -> u64 {
    text(row, key).parse().unwrap_or(0)
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

pub fn read_siteone(json: &str) -> Option<SiteOneSummary> {
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

    // SiteOne compte, par en-tête, les URL en avertissement ou en critique.
    for row in rows(&root, "security") {
        if count(row, "warning") + count(row, "critical") > 0 {
            summary.security_issues.push(SecurityIssue {
                header: text(row, "header").to_string(),
                recommendation: text(row, "recommendation").to_string(),
            });
        }
    }
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
    if !summary.security_issues.is_empty() {
        let recommendations: Vec<&str> = summary
            .security_issues
            .iter()
            .map(|i| i.recommendation.as_str())
            .filter(|r| !r.is_empty())
            .collect();
        out.push(Finding {
            code: FindingCode::SecurityHeadersMissing,
            url: start_url.to_string(),
            related: summary
                .security_issues
                .iter()
                .map(|i| i.header.clone())
                .collect(),
            detail: (!recommendations.is_empty()).then(|| recommendations.join(" ")),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: &str = "https://x.fr/";

    fn report(cert_rows: &str, security_rows: &str) -> String {
        format!(
            r#"{{"tables":{{"certificate-info":{{"rows":[{cert_rows}]}},"security":{{"rows":[{security_rows}]}}}}}}"#
        )
    }

    const ISSUER: &str = r#"{"info":"Issuer","value":"C = US, O = Zscaler Inc., CN = Zscaler Intermediate Root CA (zscaler.net) (t) "}"#;
    const VALID_TO_OK: &str =
        r#"{"info":"Valid to","value":"Nov 28 02:06:20 2026 GMT (VALID still for 7.4 day(s))"}"#;
    const VALID_TO_EXPIRED: &str =
        r#"{"info":"Valid to","value":"Nov 17 02:06:20 2026 GMT (EXPIRED 3 day(s) ago)"}"#;
    const ERRORS_ROW: &str = r#"{"info":"Errors","value":"TLS handshake failed"}"#;

    const CSP_WARNING: &str = r#"{"critical":"0","header":"Content-Security-Policy","highestSeverity":"3","notice":"0","ok":"0","recommendation":"Content-Security-Policy is set but weakened by 'unsafe-inline'.","warning":"11"}"#;
    const HSTS_CRITICAL: &str = r#"{"critical":"11","header":"Strict-Transport-Security","highestSeverity":"4","notice":"0","ok":"0","recommendation":"Strict-Transport-Security is not set.","warning":"0"}"#;
    const SERVER_NOTICE: &str = r#"{"critical":"0","header":"Server","highestSeverity":"2","notice":"11","ok":"0","recommendation":"Server header is set to 'railway-hikari'.","warning":"0"}"#;
    const NOSNIFF_OK: &str = r#"{"critical":"0","header":"X-Content-Type-Options","highestSeverity":"1","notice":"0","ok":"11","recommendation":"","warning":"0"}"#;

    fn findings(json: &str) -> Vec<Finding> {
        site_findings(&read_siteone(json).unwrap(), START)
    }

    #[test]
    fn valid_certificate_gives_no_finding() {
        let json = report(&[ISSUER, VALID_TO_OK].join(","), NOSNIFF_OK);
        let summary = read_siteone(&json).unwrap();
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
        let expired = findings(&report(&[ISSUER, VALID_TO_EXPIRED].join(","), ""));
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].code, FindingCode::HttpsInvalid);
        assert_eq!(expired[0].url, START);
        assert!(expired[0].related.is_empty());
        let detail = expired[0].detail.as_deref().unwrap();
        assert!(detail.contains("EXPIRED 3 day(s) ago"), "{detail}");
        assert!(detail.contains("Zscaler Intermediate Root CA"), "{detail}");

        let errored = findings(&report(&[ISSUER, VALID_TO_OK, ERRORS_ROW].join(","), ""));
        assert_eq!(errored.len(), 1);
        assert_eq!(errored[0].code, FindingCode::HttpsInvalid);
        let detail = errored[0].detail.as_deref().unwrap();
        assert!(detail.contains("TLS handshake failed"), "{detail}");
        assert!(detail.contains("Zscaler"), "{detail}");

        let no_issuer = findings(&report(ERRORS_ROW, ""));
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
            let json = report(&row, "");
            let summary = read_siteone(&json).unwrap();
            assert!(summary.certificate_errors.is_empty(), "{message}");
            assert!(findings(&json).is_empty(), "{message}");
        }
        let json = report(&[ISSUER, CONNECT_FAILURE].join(","), "");
        assert!(findings(&json).is_empty());
        let json = report(&[ISSUER, VALID_TO_OK, CONNECT_FAILURE].join(","), "");
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
            let f = findings(&report(&row, ""));
            assert_eq!(f.len(), 1, "{message}");
            assert_eq!(f[0].detail.as_deref(), Some(message));
        }
    }

    #[test]
    fn expiry_is_said_once_in_the_real_row_order() {
        // SiteOne écrit « Valid to » (EXPIRED) puis « Errors » avec le même fait.
        let json = report(&[ISSUER, VALID_TO_EXPIRED, EXPIRED_ERROR].join(","), "");
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
        let json = report(&[ISSUER, EXPIRED_ERROR, VALID_TO_EXPIRED].join(","), "");
        let detail = findings(&json)[0].detail.clone().unwrap();
        assert_eq!(
            detail.to_lowercase().matches("expired").count(),
            1,
            "{detail}"
        );

        // Expiré et rien d'autre : la mention de « Valid to » reste.
        let json = report(&[ISSUER, VALID_TO_EXPIRED].join(","), "");
        let detail = findings(&json)[0].detail.clone().unwrap();
        assert!(detail.contains("EXPIRED 3 day(s) ago"), "{detail}");

        // Une erreur de connexion à côté d'une expiration : l'expiration reste seule.
        let json = report(&[ISSUER, VALID_TO_EXPIRED, CONNECT_FAILURE].join(","), "");
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
    fn security_detail_is_none_when_no_recommendation_is_given() {
        let blank = CSP_WARNING.replace(
            "Content-Security-Policy is set but weakened by 'unsafe-inline'.",
            "",
        );
        let f = findings(&report("", &blank));
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].code, FindingCode::SecurityHeadersMissing);
        assert_eq!(f[0].detail, None);

        let f = findings(&report("", CSP_WARNING));
        assert!(f[0].detail.is_some());
    }

    #[test]
    fn security_warnings_give_one_site_finding() {
        let rows = [CSP_WARNING, HSTS_CRITICAL, SERVER_NOTICE, NOSNIFF_OK].join(",");
        let f = findings(&report("", &rows));
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].code, FindingCode::SecurityHeadersMissing);
        assert_eq!(f[0].url, START);
        assert_eq!(
            f[0].related,
            ["Content-Security-Policy", "Strict-Transport-Security"]
        );
        let detail = f[0].detail.as_deref().unwrap();
        assert!(detail.contains("weakened by 'unsafe-inline'"), "{detail}");
        assert!(
            detail.contains("Strict-Transport-Security is not set"),
            "{detail}"
        );
        assert!(!detail.contains("railway-hikari"), "{detail}");
    }

    #[test]
    fn notices_and_ok_headers_give_no_finding() {
        let rows = [SERVER_NOTICE, NOSNIFF_OK].join(",");
        assert!(findings(&report("", &rows)).is_empty());
    }

    #[test]
    fn missing_or_garbled_json_is_none() {
        assert!(read_siteone("").is_none());
        assert!(read_siteone("pas du json").is_none());
        assert!(read_siteone("[1,2]").is_none());
    }

    #[test]
    fn absent_tables_are_tolerated() {
        let summary = read_siteone(r#"{"tables":{}}"#).unwrap();
        assert_eq!(summary, SiteOneSummary::default());
        assert!(site_findings(&summary, START).is_empty());
        assert!(read_siteone("{}").is_some());
    }
}
