mod commands;
mod events;
mod state;

use arpenteur_core::storage::CrawlStore;
use tauri::Manager;

use crate::state::{AppState, ExitDecision};

const EXIT_WAIT: std::time::Duration = std::time::Duration::from_secs(5);
const EXIT_POLL: std::time::Duration = std::time::Duration::from_millis(100);

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
            commands::get_settings,
            commands::save_settings,
            commands::list_sites,
            commands::load_crawl,
            commands::export_file,
        ])
        .build(tauri::generate_context!())
        .expect("erreur au lancement de l'application Arpenteur")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                let state = app.state::<AppState>();
                if state.on_exit_requested() == ExitDecision::WaitForCrawl {
                    api.prevent_exit();
                    let app = app.clone();
                    // Thread dédié : le runtime reste libre pour finir d'écrire le crawl.
                    tauri::async_runtime::spawn_blocking(move || {
                        let deadline = std::time::Instant::now() + EXIT_WAIT;
                        let state = app.state::<AppState>();
                        while state.is_running() && std::time::Instant::now() < deadline {
                            std::thread::sleep(EXIT_POLL);
                        }
                        app.exit(0);
                    });
                }
            }
        });
}
