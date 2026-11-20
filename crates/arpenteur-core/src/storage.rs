use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::checks::assemble;
use crate::model::{
    CollectedData, CrawlLocation, CrawlMeta, CrawlResult, CrawlStatus, SiteSummary,
};

const META_FILE: &str = "crawl.json";
const DATA_FILE: &str = "arpenteur.json";
const INTERRUPTED_MESSAGE: &str = "Crawl interrompu par la fermeture de l'application.";

#[derive(Clone, Debug)]
pub struct CrawlStore {
    root: PathBuf,
}

/// Nom de dossier sûr (Windows compris) pour un hôte.
pub fn domain_key(host: &str) -> String {
    let cleaned: String = host
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    cleaned.trim_end_matches(['.', ' ']).to_string()
}

impl CrawlStore {
    pub fn new(root: PathBuf) -> CrawlStore {
        CrawlStore { root }
    }

    fn sites_dir(&self) -> PathBuf {
        self.root.join("sites")
    }

    pub fn create_crawl(
        &self,
        domain: &str,
        started_at: DateTime<Local>,
    ) -> io::Result<CrawlLocation> {
        let domain = domain_key(domain);
        let site_dir = self.sites_dir().join(&domain);
        fs::create_dir_all(&site_dir)?;
        let base = started_at.format("%Y-%m-%d_%H-%M-%S").to_string();
        let mut n = 1u32;
        loop {
            let id = if n == 1 {
                base.clone()
            } else {
                format!("{base}-{n}")
            };
            let dir = site_dir.join(&id);
            match fs::create_dir(&dir) {
                Ok(()) => return Ok(CrawlLocation { domain, id, dir }),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => n += 1,
                Err(e) => return Err(e),
            }
        }
    }

    pub fn write_meta(&self, loc: &CrawlLocation, meta: &CrawlMeta) -> io::Result<()> {
        write_json_atomic(&loc.dir.join(META_FILE), meta)
    }

    pub fn write_data(&self, loc: &CrawlLocation, data: &CollectedData) -> io::Result<()> {
        write_json_atomic(&loc.dir.join(DATA_FILE), data)
    }

    pub fn list_sites(&self) -> io::Result<Vec<SiteSummary>> {
        let entries = match fs::read_dir(self.sites_dir()) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut sites = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let domain = entry.file_name().to_string_lossy().into_owned();
            let mut crawls: Vec<CrawlMeta> = match fs::read_dir(&path) {
                Ok(rd) => rd
                    .flatten()
                    .filter(|c| c.path().is_dir())
                    .filter_map(|c| read_json::<CrawlMeta>(&c.path().join(META_FILE)).ok())
                    .collect(),
                Err(_) => continue,
            };
            if crawls.is_empty() {
                continue;
            }
            crawls.sort_by(|a, b| b.id.cmp(&a.id));
            sites.push(SiteSummary { domain, crawls });
        }
        sites.sort_by(|a, b| a.domain.cmp(&b.domain));
        Ok(sites)
    }

    pub fn read_crawl(&self, domain: &str, id: &str) -> io::Result<(CrawlMeta, CollectedData)> {
        check_segment(domain)?;
        check_segment(id)?;
        let dir = self.sites_dir().join(domain).join(id);
        let meta = read_json::<CrawlMeta>(&dir.join(META_FILE))?;
        let data = match read_json::<CollectedData>(&dir.join(DATA_FILE)) {
            Ok(d) => d,
            Err(e) if e.kind() == io::ErrorKind::NotFound => CollectedData::default(),
            Err(e) => return Err(e),
        };
        Ok((meta, data))
    }

    /// À appeler au démarrage, quand aucun crawl ne tourne : un crawl encore
    /// « en cours » a été interrompu par la fermeture de l'application et passe
    /// en échec. Renvoie le nombre de crawls corrigés ; un fichier illisible est ignoré.
    pub fn recover_interrupted(&self) -> io::Result<u32> {
        let sites = match fs::read_dir(self.sites_dir()) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(e),
        };
        let mut fixed = 0;
        for site in sites.flatten() {
            let Ok(crawls) = fs::read_dir(site.path()) else {
                continue;
            };
            for crawl in crawls.flatten() {
                let path = crawl.path().join(META_FILE);
                let Ok(mut meta) = read_json::<CrawlMeta>(&path) else {
                    continue;
                };
                if meta.status != CrawlStatus::Running {
                    continue;
                }
                meta.status = CrawlStatus::Failed {
                    message: INTERRUPTED_MESSAGE.to_string(),
                };
                meta.finished_at
                    .get_or_insert_with(|| Local::now().to_rfc3339());
                write_json_atomic(&path, &meta)?;
                fixed += 1;
            }
        }
        Ok(fixed)
    }

    pub fn load_result(&self, domain: &str, id: &str) -> io::Result<CrawlResult> {
        let (meta, data) = self.read_crawl(domain, id)?;
        Ok(assemble(meta, data))
    }
}

/// Refuse tout ce qui n'est pas un nom de dossier simple (entrée non fiable).
fn check_segment(segment: &str) -> io::Result<()> {
    let bad = segment.is_empty()
        || segment == "."
        || segment == ".."
        || segment.contains(['/', '\\', '\0'])
        || segment.contains(':');
    if bad {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid path segment: {segment:?}"),
        ));
    }
    Ok(())
}

fn read_json<T: DeserializeOwned>(path: &Path) -> io::Result<T> {
    let bytes = fs::read(path)?;
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::CrawlSettings;
    use chrono::{NaiveDate, TimeZone};

    fn fixed() -> DateTime<Local> {
        let naive = NaiveDate::from_ymd_opt(2026, 11, 20)
            .unwrap()
            .and_hms_opt(14, 3, 7)
            .unwrap();
        Local.from_local_datetime(&naive).single().unwrap()
    }

    fn meta(id: &str, domain: &str) -> CrawlMeta {
        CrawlMeta {
            id: id.to_string(),
            domain: domain.to_string(),
            start_url: format!("https://{domain}/"),
            started_at: "2026-11-20T14:03:07+01:00".to_string(),
            finished_at: None,
            status: CrawlStatus::Completed,
            settings: CrawlSettings::default(),
            pages: 3,
        }
    }

    #[test]
    fn creates_dated_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let store = CrawlStore::new(tmp.path().to_path_buf());
        let loc = store.create_crawl("exemple.fr", fixed()).unwrap();
        assert_eq!(loc.id, "2026-11-20_14-03-07");
        assert_eq!(loc.domain, "exemple.fr");
        assert_eq!(
            loc.dir,
            tmp.path()
                .join("sites")
                .join("exemple.fr")
                .join("2026-11-20_14-03-07")
        );
        assert!(loc.dir.is_dir());
    }

    #[test]
    fn same_second_gets_suffix() {
        let tmp = tempfile::tempdir().unwrap();
        let store = CrawlStore::new(tmp.path().to_path_buf());
        let first = store.create_crawl("exemple.fr", fixed()).unwrap();
        let second = store.create_crawl("exemple.fr", fixed()).unwrap();
        assert_eq!(first.id, "2026-11-20_14-03-07");
        assert_eq!(second.id, "2026-11-20_14-03-07-2");
        assert!(second.dir.is_dir());
    }

    #[test]
    fn domain_key_is_sanitized() {
        assert_eq!(domain_key("Exemple.FR"), "exemple.fr");
        assert_eq!(domain_key("localhost:8080"), "localhost_8080");
        assert_eq!(domain_key("a<b>c\"d/e\\f|g?h*i"), "a_b_c_d_e_f_g_h_i");
        assert_eq!(domain_key("exemple.fr. "), "exemple.fr");
    }

    #[test]
    fn list_sites_sorted() {
        let tmp = tempfile::tempdir().unwrap();
        let store = CrawlStore::new(tmp.path().to_path_buf());
        for (domain, id) in [
            ("zeta.fr", "2026-01-01_10-00-00"),
            ("alpha.fr", "2026-01-01_10-00-00"),
            ("alpha.fr", "2026-03-01_10-00-00"),
        ] {
            let loc = CrawlLocation {
                domain: domain.to_string(),
                id: id.to_string(),
                dir: tmp.path().join("sites").join(domain).join(id),
            };
            fs::create_dir_all(&loc.dir).unwrap();
            store.write_meta(&loc, &meta(id, domain)).unwrap();
        }
        // dossier sans crawl.json, crawl corrompu, site sans crawl valide
        fs::create_dir_all(tmp.path().join("sites/alpha.fr/vide")).unwrap();
        fs::create_dir_all(tmp.path().join("sites/alpha.fr/casse")).unwrap();
        fs::write(tmp.path().join("sites/alpha.fr/casse/crawl.json"), "{").unwrap();
        fs::create_dir_all(tmp.path().join("sites/orphelin.fr/vide")).unwrap();

        let sites = store.list_sites().unwrap();
        let domains: Vec<_> = sites.iter().map(|s| s.domain.as_str()).collect();
        assert_eq!(domains, ["alpha.fr", "zeta.fr"]);
        let ids: Vec<_> = sites[0].crawls.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["2026-03-01_10-00-00", "2026-01-01_10-00-00"]);
    }

    #[test]
    fn list_sites_without_root_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let store = CrawlStore::new(tmp.path().join("absent"));
        assert!(store.list_sites().unwrap().is_empty());
    }

    #[test]
    fn roundtrip_meta_and_data() {
        let tmp = tempfile::tempdir().unwrap();
        let store = CrawlStore::new(tmp.path().to_path_buf());
        let loc = store.create_crawl("exemple.fr", fixed()).unwrap();
        let m = meta(&loc.id, "exemple.fr");
        store.write_meta(&loc, &m).unwrap();

        // arpenteur.json absent : données vides
        let (read_meta, data) = store.read_crawl("exemple.fr", &loc.id).unwrap();
        assert_eq!(read_meta.id, loc.id);
        assert_eq!(read_meta.pages, 3);
        assert!(data.pages.is_empty());
        assert!(!data.sitemap.found);

        let mut collected = CollectedData::default();
        collected.sitemap.found = true;
        collected.sitemap.urls = vec!["https://exemple.fr/a".to_string()];
        store.write_data(&loc, &collected).unwrap();
        let (_, data) = store.read_crawl("exemple.fr", &loc.id).unwrap();
        assert!(data.sitemap.found);
        assert_eq!(data.sitemap.urls, ["https://exemple.fr/a"]);

        assert!(store.read_crawl("exemple.fr", "inconnu").is_err());
    }

    #[test]
    fn read_crawl_rejects_path_traversal() {
        let tmp = tempfile::tempdir().unwrap();
        let store = CrawlStore::new(tmp.path().to_path_buf());
        let loc = store.create_crawl("exemple.fr", fixed()).unwrap();
        store.write_meta(&loc, &meta(&loc.id, "exemple.fr")).unwrap();
        for bad in ["", ".", "..", "../x", "a/b", "a\\b", "/etc", "C:\\x", "C:x"] {
            let e = store.read_crawl(bad, &loc.id).unwrap_err();
            assert_eq!(e.kind(), io::ErrorKind::InvalidInput, "domain {bad:?}");
            let e = store.read_crawl("exemple.fr", bad).unwrap_err();
            assert_eq!(e.kind(), io::ErrorKind::InvalidInput, "id {bad:?}");
        }
        assert!(store.read_crawl("exemple.fr", &loc.id).is_ok());
    }

    #[test]
    fn recover_interrupted_fails_running_crawls() {
        let tmp = tempfile::tempdir().unwrap();
        let store = CrawlStore::new(tmp.path().to_path_buf());
        let running = |domain: &str, finished_at: Option<&str>| {
            let loc = store.create_crawl(domain, fixed()).unwrap();
            let mut m = meta(&loc.id, domain);
            m.status = CrawlStatus::Running;
            m.finished_at = finished_at.map(str::to_string);
            store.write_meta(&loc, &m).unwrap();
            loc
        };
        let open = running("exemple.fr", None);
        let dated = running("exemple.fr", Some("2026-11-20T14:05:00+01:00"));
        let done = store.create_crawl("autre.fr", fixed()).unwrap();
        store.write_meta(&done, &meta(&done.id, "autre.fr")).unwrap();
        let broken = tmp.path().join("sites/exemple.fr/casse");
        fs::create_dir_all(&broken).unwrap();
        fs::write(broken.join("crawl.json"), "{").unwrap();

        assert_eq!(store.recover_interrupted().unwrap(), 2);

        let interrupted = CrawlStatus::Failed {
            message: "Crawl interrompu par la fermeture de l'application.".to_string(),
        };
        let (m, _) = store.read_crawl("exemple.fr", &open.id).unwrap();
        assert_eq!(m.status, interrupted);
        assert!(m.finished_at.is_some());
        let (m, _) = store.read_crawl("exemple.fr", &dated.id).unwrap();
        assert_eq!(m.status, interrupted);
        assert_eq!(m.finished_at.as_deref(), Some("2026-11-20T14:05:00+01:00"));
        let (m, _) = store.read_crawl("autre.fr", &done.id).unwrap();
        assert_eq!(m.status, CrawlStatus::Completed);
        assert_eq!(fs::read_to_string(broken.join("crawl.json")).unwrap(), "{");

        assert_eq!(store.recover_interrupted().unwrap(), 0);
    }

    #[test]
    fn recover_interrupted_without_root_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let store = CrawlStore::new(tmp.path().join("absent"));
        assert_eq!(store.recover_interrupted().unwrap(), 0);
    }

    #[test]
    fn writes_are_atomic() {
        let tmp = tempfile::tempdir().unwrap();
        let store = CrawlStore::new(tmp.path().to_path_buf());
        let loc = store.create_crawl("exemple.fr", fixed()).unwrap();
        let m = meta(&loc.id, "exemple.fr");
        store.write_meta(&loc, &m).unwrap();
        store.write_meta(&loc, &m).unwrap();
        store.write_data(&loc, &CollectedData::default()).unwrap();
        let mut names: Vec<_> = fs::read_dir(&loc.dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["arpenteur.json", "crawl.json"]);
    }
}
