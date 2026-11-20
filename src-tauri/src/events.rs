use std::sync::Mutex;
use std::time::{Duration, Instant};

use arpenteur_core::collector::CrawlEvents;
use arpenteur_core::model::Progress;
use tauri::{AppHandle, Emitter};

const MIN_INTERVAL: Duration = Duration::from_millis(100);

/// Relaie la progression au front, au plus une fois toutes les 100 ms.
pub struct TauriEvents {
    app: AppHandle,
    last_emit: Mutex<Option<Instant>>,
}

impl TauriEvents {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            last_emit: Mutex::new(None),
        }
    }
}

impl CrawlEvents for TauriEvents {
    fn progress(&self, p: &Progress) {
        // Appelée sous les verrous de SiteOne : jamais bloquante.
        let Ok(mut last) = self.last_emit.try_lock() else {
            return;
        };
        let now = Instant::now();
        if last.is_some_and(|t| now.duration_since(t) < MIN_INTERVAL) {
            return;
        }
        *last = Some(now);
        let _ = self.app.emit("crawl-progress", p);
    }
}
