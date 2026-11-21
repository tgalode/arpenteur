use arpenteur_core::checks::assemble;
use arpenteur_core::model::{
    CollectedData, CrawlMeta, CrawlSettings, CrawlStatus, FACTS_VERSION, FindingCode, Indexability,
};

const PLAN_1_DATA: &str = include_str!("fixtures/arpenteur-plan1.json");

fn meta() -> CrawlMeta {
    CrawlMeta {
        id: "2026-01-01_00-00-00".into(),
        domain: "www.exemple.fr".into(),
        start_url: "https://www.exemple.fr/".into(),
        started_at: String::new(),
        finished_at: None,
        status: CrawlStatus::Completed,
        settings: CrawlSettings::default(),
        pages: 4,
    }
}

fn has(findings: &[arpenteur_core::model::Finding], code: FindingCode, path: &str) -> bool {
    let url = format!("https://www.exemple.fr{path}");
    findings.iter().any(|f| f.code == code && f.url == url)
}

#[test]
fn plan_1_crawl_opens_without_a_false_missing_lang() {
    let data: CollectedData = serde_json::from_str(PLAN_1_DATA).unwrap();
    assert_eq!(data.facts_version, 0);
    let result = assemble(meta(), data, None);

    assert!(
        !result
            .findings
            .iter()
            .any(|f| f.code == FindingCode::HtmlLangMissing)
    );
    // Les contrôles qui ne dépendent pas des nouveaux faits tournent toujours.
    assert!(has(&result.findings, FindingCode::TitleTooShort, "/"));
    assert!(has(
        &result.findings,
        FindingCode::MetaDescriptionMissing,
        "/"
    ));
    assert!(has(
        &result.findings,
        FindingCode::BrokenInternalLink,
        "/absente"
    ));
    assert!(has(&result.findings, FindingCode::TitleDuplicate, "/a"));
}

#[test]
fn new_crawl_without_lang_reports_it() {
    let mut data: CollectedData = serde_json::from_str(PLAN_1_DATA).unwrap();
    data.facts_version = FACTS_VERSION;
    let result = assemble(meta(), data, None);

    assert!(has(&result.findings, FindingCode::HtmlLangMissing, "/"));
    assert!(has(&result.findings, FindingCode::HtmlLangMissing, "/a"));
}

#[test]
fn crawl_without_probed_field_loads() {
    let data: CollectedData = serde_json::from_str(PLAN_1_DATA).unwrap();
    assert!(data.pages.iter().all(|p| !p.probed));
    let result = assemble(meta(), data, None);

    assert!(
        result
            .rows
            .iter()
            .all(|r| r.indexable != Indexability::NotAnalyzed)
    );
    let codes: Vec<String> = result
        .findings
        .iter()
        .map(|f| format!("{:?}", f.code))
        .collect();
    assert_eq!(
        codes,
        [
            "BrokenInternalLink",
            "SitemapNotFound",
            "DuplicateContent",
            "DuplicateContent",
            "TitleTooShort",
            "TitleTooShort",
            "MetaDescriptionMissing",
            "TitleDuplicate",
            "TitleDuplicate",
            "SecurityHeadersMissing",
        ]
    );
}

#[test]
fn crawl_without_external_links_loads() {
    let data: CollectedData = serde_json::from_str(PLAN_1_DATA).unwrap();
    assert!(data.external_links.is_empty());
    assert_eq!(data.external_unchecked, 0);
    let result = assemble(meta(), data, None);
    let codes: Vec<String> = result
        .findings
        .iter()
        .map(|f| format!("{:?}", f.code))
        .collect();
    assert_eq!(
        codes,
        [
            "BrokenInternalLink",
            "SitemapNotFound",
            "DuplicateContent",
            "DuplicateContent",
            "TitleTooShort",
            "TitleTooShort",
            "MetaDescriptionMissing",
            "TitleDuplicate",
            "TitleDuplicate",
            "SecurityHeadersMissing",
        ]
    );
}
