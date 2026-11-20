use std::sync::Mutex;

use arpenteur_core::storage::CrawlStore;
use tokio_util::sync::CancellationToken;

/// Un seul crawl à la fois : le jeton du crawl en cours, s'il y en a un.
pub struct AppState {
    pub store: CrawlStore,
    running: Mutex<Option<CancellationToken>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct AlreadyRunning;

impl AppState {
    pub fn new(store: CrawlStore) -> Self {
        Self {
            store,
            running: Mutex::new(None),
        }
    }

    pub fn try_begin(&self) -> Result<CancellationToken, AlreadyRunning> {
        let mut running = self.running.lock().unwrap_or_else(|e| e.into_inner());
        if running.is_some() {
            return Err(AlreadyRunning);
        }
        let token = CancellationToken::new();
        *running = Some(token.clone());
        Ok(token)
    }

    /// Annule le crawl en cours ; `false` s'il n'y en a pas.
    pub fn stop(&self) -> bool {
        let running = self.running.lock().unwrap_or_else(|e| e.into_inner());
        match running.as_ref() {
            Some(token) => {
                token.cancel();
                true
            }
            None => false,
        }
    }

    pub fn finish(&self) {
        *self.running.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

/// Libère l'état à la sortie de la portée, panique comprise.
pub struct FinishGuard<'a>(pub &'a AppState);

impl Drop for FinishGuard<'_> {
    fn drop(&mut self) {
        self.0.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> (AppState, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        (AppState::new(CrawlStore::new(dir.path().to_path_buf())), dir)
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
        state.try_begin().unwrap();
        state.finish();
        assert!(state.try_begin().is_ok());
    }

    #[test]
    fn stop_cancels_the_running_token() {
        let (state, _dir) = state();
        assert!(!state.stop());
        let token = state.try_begin().unwrap();
        assert!(state.stop());
        assert!(token.is_cancelled());
    }

    #[test]
    fn guard_releases_state_even_on_panic() {
        let (state, _dir) = state();
        state.try_begin().unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = FinishGuard(&state);
            panic!("panique simulée dans le crawl");
        }));
        assert!(result.is_err());
        assert!(state.try_begin().is_ok());
    }
}
