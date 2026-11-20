use std::sync::Arc;

use arpenteur_core::engine::run_crawl;
use arpenteur_core::model::{CrawlResult, CrawlSettings, SiteSummary};
use arpenteur_core::settings::normalize_start_url;
use arpenteur_core::storage::domain_key;
use chrono::Local;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::events::TauriEvents;
use crate::state::AppState;

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
    let cancel = state
        .try_begin()
        .map_err(|_| "Un crawl est déjà en cours.".to_string())?;

    let domain = domain_key(start.host_str().unwrap_or_default());
    let location = match state.store.create_crawl(&domain, Local::now()) {
        Ok(location) => location,
        Err(e) => {
            state.finish();
            return Err(e.to_string());
        }
    };
    let started = StartedCrawl {
        domain: location.domain.clone(),
        id: location.id.clone(),
    };

    let store = state.store.clone();
    let events = Arc::new(TauriEvents::new(app.clone()));
    let settings = settings.unwrap_or_default();
    tauri::async_runtime::spawn(async move {
        let meta = run_crawl(settings, start, store, location, events, cancel).await;
        app.state::<AppState>().finish();
        let _ = app.emit("crawl-finished", &meta);
    });

    Ok(started)
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
    state.store.load_result(&domain, &id).map_err(|e| e.to_string())
}
