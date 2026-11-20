mod commands;
mod events;
mod state;

use arpenteur_core::storage::CrawlStore;
use tauri::Manager;

use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let root = app.path().app_data_dir()?;
            std::fs::create_dir_all(&root)?;
            let store = CrawlStore::new(root);
            // Aucun crawl ne tourne encore : un crawl « en cours » a été interrompu.
            let _ = store.recover_interrupted();
            app.manage(AppState::new(store));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_crawl,
            commands::stop_crawl,
            commands::list_sites,
            commands::load_crawl,
            commands::export_file,
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de l'application Arpenteur");
}
