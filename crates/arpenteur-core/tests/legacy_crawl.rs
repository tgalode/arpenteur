use arpenteur_core::checks::assemble;
use arpenteur_core::model::{
    CollectedData, CrawlMeta, CrawlMode, CrawlSettings, CrawlStatus, FACTS_VERSION, FindingCode,
    Indexability,
};
use arpenteur_core::storage::CrawlStore;

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
        mode: CrawlMode::Site,
        list_urls: Vec::new(),
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

/// `crawl.json` tel que l'écrivaient les plans 1 et 2 : ni `mode`, ni `listUrls`.
const OLD_META: &str = r#"{
  "id": "2026-01-01_00-00-00",
  "domain": "www.exemple.fr",
  "startUrl": "https://www.exemple.fr/",
  "startedAt": "2026-01-01T00:00:00+01:00",
  "finishedAt": "2026-01-01T00:01:00+01:00",
  "status": { "kind": "completed" },
  "settings": {},
  "pages": 4
}"#;

#[test]
fn crawl_without_mode_is_a_site_crawl() {
    let root = tempfile::tempdir().unwrap();
    let dir = root
        .path()
        .join("sites")
        .join("www.exemple.fr")
        .join("2026-01-01_00-00-00");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("crawl.json"), OLD_META).unwrap();
    std::fs::write(dir.join("arpenteur.json"), PLAN_1_DATA).unwrap();

    let store = CrawlStore::new(root.path().to_path_buf());
    let result = store
        .load_result("www.exemple.fr", "2026-01-01_00-00-00")
        .unwrap();

    assert_eq!(result.meta.mode, CrawlMode::Site);
    assert!(result.meta.list_urls.is_empty());
    assert!(result.list_skipped.is_empty());
    let codes: Vec<String> = result
        .findings
        .iter()
        .map(|f| format!("{:?}", f.code))
        .collect();
    assert_eq!(codes, PLAN_1_CODES);

    let root_row = result
        .rows
        .iter()
        .find(|r| r.url == "https://www.exemple.fr/")
        .unwrap();
    assert_eq!((root_row.depth, root_row.inlinks), (Some(0), Some(1)));
    assert!(result.rows.iter().all(|r| !r.off_list));
    assert!(result.rows.iter().all(|r| r.inlinks.is_some()));
    assert!(result.rows.iter().any(|r| r.depth == Some(1)));
}

const PLAN_1_CODES: [&str; 10] = [
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
];

/// Faits du plan 3 (version 2) : pas de `canonicals`, donc jamais `canonicalMultiple`.
#[test]
fn plan_3_crawl_gains_no_canonical_multiple() {
    let mut data: CollectedData = serde_json::from_str(PLAN_1_DATA).unwrap();
    data.facts_version = 2;
    assert!(
        data.pages
            .iter()
            .filter_map(|p| p.facts.as_ref())
            .all(|f| f.canonicals.is_empty())
    );
    let result = assemble(meta(), data, None);

    let codes: Vec<String> = result
        .findings
        .iter()
        .map(|f| format!("{:?}", f.code))
        .collect();
    assert!(!codes.iter().any(|c| c == "CanonicalMultiple"));
    assert!(!codes.iter().any(|c| c == "ListedUrlError"));
    assert!(!codes.iter().any(|c| c == "H1Duplicate" || c == "H1TooLong"));
    // La version 2 relève la langue : seul `HtmlLangMissing` s'ajoute au plan 1.
    let others: Vec<&String> = codes.iter().filter(|c| *c != "HtmlLangMissing").collect();
    assert_eq!(others, PLAN_1_CODES);
}

/// Un crawl du plan 3 n'a ni sonde d'adresse inexistante ni statut de robots.txt :
/// ni `soft404`, ni `soft404Site`, ni `robotsTxtMissing`.
#[test]
fn plan_3_crawl_gains_no_soft_404_nor_robots_finding() {
    let mut data: CollectedData = serde_json::from_str(PLAN_1_DATA).unwrap();
    data.facts_version = 2;
    assert!(data.not_found_probe.is_none());
    assert_eq!(data.sitemap.robots_status, None);
    let result = assemble(meta(), data, None);

    let codes: Vec<String> = result
        .findings
        .iter()
        .map(|f| format!("{:?}", f.code))
        .collect();
    for absent in ["Soft404", "Soft404Site", "RobotsTxtMissing"] {
        assert!(!codes.iter().any(|c| c == absent), "{absent}");
    }
}
