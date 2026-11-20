use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use arpenteur_core::storage::CrawlStore;
use tokio_util::sync::CancellationToken;

/// Un seul crawl à la fois : le jeton du crawl en cours, s'il y en a un.
pub struct AppState {
    pub store: CrawlStore,
    running: Mutex<Option<(u64, CancellationToken)>>,
    next_id: AtomicU64,
    exiting: AtomicBool,
}

/// Crawl accepté par `try_begin` : `id` sert à `finish`, `token` à l'annuler.
#[derive(Debug)]
pub struct RunningCrawl {
    pub id: u64,
    pub token: CancellationToken,
}

#[derive(Debug, PartialEq, Eq)]
pub struct AlreadyRunning;

/// Suite à donner à une demande de fermeture de l'application.
#[derive(Debug, PartialEq, Eq)]
pub enum ExitDecision {
    Allow,
    /// Un arrêt du crawl vient d'être demandé : retenir la sortie.
    WaitForCrawl,
}

impl AppState {
    pub fn new(store: CrawlStore) -> Self {
        Self {
            store,
            running: Mutex::new(None),
            next_id: AtomicU64::new(0),
            exiting: AtomicBool::new(false),
        }
    }

    pub fn try_begin(&self) -> Result<RunningCrawl, AlreadyRunning> {
        let mut running = self.running.lock().unwrap_or_else(|e| e.into_inner());
        if running.is_some() {
            return Err(AlreadyRunning);
        }
        let token = CancellationToken::new();
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        *running = Some((id, token.clone()));
        Ok(RunningCrawl { id, token })
    }

    /// Annule le crawl en cours ; `false` s'il n'y en a pas.
    pub fn stop(&self) -> bool {
        let running = self.running.lock().unwrap_or_else(|e| e.into_inner());
        match running.as_ref() {
            Some((_, token)) => {
                token.cancel();
                true
            }
            None => false,
        }
    }

    pub fn is_running(&self) -> bool {
        self.running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
    }

    /// Premier ExitRequested pendant un crawl : l'arrête et retient la sortie.
    /// Tout autre cas laisse sortir (aucun crawl, sortie déjà en cours).
    pub fn on_exit_requested(&self) -> ExitDecision {
        if self.exiting.load(Ordering::SeqCst) || !self.is_running() {
            return ExitDecision::Allow;
        }
        self.exiting.store(true, Ordering::SeqCst);
        self.stop();
        ExitDecision::WaitForCrawl
    }

    /// Libère l'état seulement si `id` est encore le crawl courant.
    pub fn finish(&self, id: u64) {
        let mut running = self.running.lock().unwrap_or_else(|e| e.into_inner());
        if running.as_ref().is_some_and(|(current, _)| *current == id) {
            *running = None;
        }
    }
}

/// Libère l'état à la sortie de la portée, panique comprise.
pub struct FinishGuard<'a>(pub &'a AppState, pub u64);

impl Drop for FinishGuard<'_> {
    fn drop(&mut self) {
        self.0.finish(self.1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> (AppState, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        (
            AppState::new(CrawlStore::new(dir.path().to_path_buf())),
            dir,
        )
    }

    #[test]
    fn second_begin_is_refused() {
        let (state, _dir) = state();
        assert!(state.try_begin().is_ok());
        assert_eq!(state.try_begin().unwrap_err(), AlreadyRunning);
    }

    #[test]
    fn finish_allows_a_new_crawl() {
        let (state, _dir) = state();
        let crawl = state.try_begin().unwrap();
        state.finish(crawl.id);
        assert!(state.try_begin().is_ok());
    }

    #[test]
    fn stop_cancels_the_running_token() {
        let (state, _dir) = state();
        assert!(!state.stop());
        let crawl = state.try_begin().unwrap();
        assert!(state.stop());
        assert!(crawl.token.is_cancelled());
    }

    #[test]
    fn guard_releases_state_even_on_panic() {
        let (state, _dir) = state();
        let crawl = state.try_begin().unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = FinishGuard(&state, crawl.id);
            panic!("panique simulée dans le crawl");
        }));
        assert!(result.is_err());
        assert!(state.try_begin().is_ok());
    }

    #[test]
    fn late_finish_does_not_clear_a_newer_crawl() {
        let (state, _dir) = state();
        let a = state.try_begin().unwrap();
        state.finish(a.id);
        let b = state.try_begin().unwrap();
        state.finish(a.id);
        assert_eq!(state.try_begin().unwrap_err(), AlreadyRunning);
        assert!(state.stop());
        assert!(b.token.is_cancelled());
    }

    #[test]
    fn is_running_follows_begin_and_finish() {
        let (state, _dir) = state();
        assert!(!state.is_running());
        let crawl = state.try_begin().unwrap();
        assert!(state.is_running());
        state.finish(crawl.id);
        assert!(!state.is_running());
    }

    #[test]
    fn exit_is_allowed_without_crawl() {
        let (state, _dir) = state();
        assert_eq!(state.on_exit_requested(), ExitDecision::Allow);
    }

    #[test]
    fn exit_during_crawl_stops_it_then_second_request_allows() {
        let (state, _dir) = state();
        let crawl = state.try_begin().unwrap();
        assert_eq!(state.on_exit_requested(), ExitDecision::WaitForCrawl);
        assert!(crawl.token.is_cancelled());
        assert_eq!(state.on_exit_requested(), ExitDecision::Allow);
    }
}
