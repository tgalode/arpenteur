mod commands;
mod events;
mod state;

use arpenteur_core::storage::CrawlStore;
use tauri::Manager;

use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let root = app.path().app_data_dir()?;
            std::fs::create_dir_all(&root)?;
            app.manage(AppState::new(CrawlStore::new(root)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_crawl,
            commands::stop_crawl,
            commands::list_sites,
            commands::load_crawl,
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de l'application Arpenteur");
}
