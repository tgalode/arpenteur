use std::path::Path;
use std::sync::Arc;

use arpenteur_core::defaults::validate_settings;
use arpenteur_core::engine::{CrawlTarget, run_crawl};
use arpenteur_core::model::{
    CrawlMeta, CrawlMode, CrawlResult, CrawlSettings, CrawlStatus, SiteSummary,
};
use arpenteur_core::settings::normalize_start_url;
use arpenteur_core::storage::domain_key;
use arpenteur_core::url_list::{check_list_size, parse_url_list};
use chrono::Local;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::events::TauriEvents;
use crate::state::{AppState, FinishGuard};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StartedCrawl {
    pub domain: String,
    pub id: String,
}

#[tauri::command]
pub fn start_crawl(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
    settings: Option<CrawlSettings>,
) -> Result<StartedCrawl, String> {
    let start = normalize_start_url(&url).map_err(|e| e.to_string())?;
    let settings = settings.unwrap_or_else(|| state.store.read_default_settings());
    validate_settings(&settings).map_err(|errors| errors.join(" "))?;
    begin_crawl(app, &state, settings, CrawlTarget::Site(start))
}

/// Les messages de réglages, de lignes fautives puis de taille sont renvoyés avant de prendre la place du crawl.
#[tauri::command]
pub fn start_list_crawl(
    app: AppHandle,
    state: State<'_, AppState>,
    urls: String,
    settings: Option<CrawlSettings>,
) -> Result<StartedCrawl, String> {
    let settings = settings.unwrap_or_else(|| state.store.read_default_settings());
    validate_settings(&settings).map_err(|errors| errors.join("\n"))?;
    let list = parse_url_list(&urls).map_err(|errors| errors.join("\n"))?;
    check_list_size(&list, &settings)?;
    begin_crawl(app, &state, settings, CrawlTarget::List(list))
}

/// Le crawl est rangé sous le site de la première adresse, même si robots.txt l'écarte ensuite.
fn target_site_key(target: &CrawlTarget) -> String {
    let first = match target {
        CrawlTarget::Site(url) => Some(url),
        CrawlTarget::List(urls) => urls.first(),
    };
    domain_key(first.and_then(|u| u.host_str()).unwrap_or_default())
}

fn begin_crawl(
    app: AppHandle,
    state: &State<'_, AppState>,
    settings: CrawlSettings,
    target: CrawlTarget,
) -> Result<StartedCrawl, String> {
    let running = state
        .try_begin()
        .map_err(|_| "Un crawl est déjà en cours.".to_string())?;

    let domain = target_site_key(&target);
    let location = match state.store.create_crawl(&domain, Local::now()) {
        Ok(location) => location,
        Err(e) => {
            state.finish(running.id);
            return Err(e.to_string());
        }
    };
    let started = StartedCrawl {
        domain: location.domain.clone(),
        id: location.id.clone(),
    };

    let (mode, start_url, list_urls) = match &target {
        CrawlTarget::Site(url) => (CrawlMode::Site, url.as_str().to_string(), Vec::new()),
        CrawlTarget::List(urls) => (
            CrawlMode::List,
            urls.first().map(|u| u.as_str().to_string()).unwrap_or_default(),
            urls.iter().map(|u| u.as_str().to_string()).collect(),
        ),
    };
    let store = state.store.clone();
    let events = Arc::new(TauriEvents::new(app.clone()));
    let run_id = running.id;
    let cancel = running.token;
    let started_at = Local::now().to_rfc3339();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let _guard = FinishGuard(&state, run_id);
        let inner = tauri::async_runtime::spawn(run_crawl(
            settings.clone(),
            target,
            store.clone(),
            location.clone(),
            events,
            cancel,
        ));
        let meta = match inner.await {
            Ok(meta) => meta,
            Err(_) => {
                let meta = CrawlMeta {
                    id: location.id.clone(),
                    domain: location.domain.clone(),
                    start_url,
                    started_at,
                    finished_at: Some(Local::now().to_rfc3339()),
                    status: CrawlStatus::Failed {
                        message: "Le crawl s'est arrêté sur une erreur interne.".to_string(),
                    },
                    settings,
                    pages: 0,
                    mode,
                    list_urls,
                };
                let _ = store.write_meta(&location, &meta);
                meta
            }
        };
        state.finish(run_id);
        let _ = app.emit("crawl-finished", &meta);
    });

    Ok(started)
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> CrawlSettings {
    state.store.read_default_settings()
}

/// Refuse sans rien écrire si un réglage est invalide ; renvoie alors tous les messages.
#[tauri::command]
pub fn save_settings(
    state: State<'_, AppState>,
    settings: CrawlSettings,
) -> Result<(), Vec<String>> {
    validate_settings(&settings)?;
    state
        .store
        .write_default_settings(&settings)
        .map_err(|e| vec![format!("Enregistrement impossible : {e}")])
}

#[tauri::command]
pub fn stop_crawl(state: State<'_, AppState>) -> bool {
    state.stop()
}

#[tauri::command]
pub fn list_sites(state: State<'_, AppState>) -> Result<Vec<SiteSummary>, String> {
    state.store.list_sites().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn load_crawl(
    state: State<'_, AppState>,
    domain: String,
    id: String,
) -> Result<CrawlResult, String> {
    state
        .store
        .load_result(&domain, &id)
        .map_err(|e| e.to_string())
}

fn has_csv_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("csv"))
}

/// Écrit exactement `contents` (BOM compris) dans un fichier `.csv`.
#[tauri::command]
pub fn export_file(path: String, contents: String) -> Result<(), String> {
    let path = Path::new(&path);
    if !has_csv_extension(path) {
        return Err("seuls les fichiers .csv peuvent être exportés".to_string());
    }
    std::fs::write(path, contents.as_bytes())
        .map_err(|e| format!("impossible d’écrire le fichier : {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::Url;

    #[test]
    fn list_crawl_is_filed_under_the_first_url_site() {
        let urls = ["https://www.exemple.fr/tarifs", "https://www.exemple.fr/"]
            .map(|u| Url::parse(u).unwrap())
            .to_vec();
        let site = CrawlTarget::Site(Url::parse("https://www.exemple.fr/").unwrap());
        assert_eq!(target_site_key(&CrawlTarget::List(urls)), target_site_key(&site));
        assert_eq!(target_site_key(&site), domain_key("www.exemple.fr"));
    }

    #[test]
    fn export_file_rejects_other_extensions() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["a.txt", "a", "a.csv.exe", "a.csvx"] {
            let path = dir.path().join(name);
            let err = export_file(path.to_string_lossy().into_owned(), "x".into()).unwrap_err();
            assert!(err.contains(".csv"), "{err}");
            assert!(!path.exists());
        }
    }

    #[test]
    fn export_file_writes_exact_bytes_for_csv_in_any_case() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.CSV");
        let contents = "\u{FEFF}a;b\r\n1;2";
        export_file(path.to_string_lossy().into_owned(), contents.into()).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), contents.as_bytes());
    }

    #[test]
    fn export_file_reports_write_failure_in_french() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("absent").join("a.csv");
        let err = export_file(path.to_string_lossy().into_owned(), "x".into()).unwrap_err();
        assert!(err.starts_with("impossible d’écrire"), "{err}");
    }
}
