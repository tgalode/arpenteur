/// Détecte un site qui limite ou bloque le crawl : 429/503 consécutifs, ou
/// 403 consécutifs une fois qu'au moins un 2xx a été vu.
pub struct LimitGuard {
    threshold: u32,
    limited_streak: u32,
    forbidden_streak: u32,
    seen_success: bool,
}

impl LimitGuard {
    pub fn new(threshold: u32) -> LimitGuard {
        LimitGuard {
            threshold,
            limited_streak: 0,
            forbidden_streak: 0,
            seen_success: false,
        }
    }

    /// Renvoie `true` tant que le seuil est atteint.
    pub fn observe(&mut self, status: i32) -> bool {
        match status {
            200..=299 => {
                self.seen_success = true;
                self.limited_streak = 0;
                self.forbidden_streak = 0;
            }
            429 | 503 => self.limited_streak += 1,
            403 if self.seen_success => self.forbidden_streak += 1,
            _ => {}
        }
        self.limited_streak >= self.threshold || self.forbidden_streak >= self.threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observe_all(guard: &mut LimitGuard, statuses: &[i32]) -> Vec<bool> {
        statuses
            .iter()
            .map(|&status| guard.observe(status))
            .collect()
    }

    #[test]
    fn trips_after_five_consecutive_429() {
        let mut guard = LimitGuard::new(5);
        let results = observe_all(&mut guard, &[200, 429, 429, 429, 429, 429]);
        assert_eq!(results, vec![false, false, false, false, false, true]);
    }

    #[test]
    fn status_503_counts_like_429() {
        let mut guard = LimitGuard::new(5);
        let results = observe_all(&mut guard, &[503, 429, 503, 429, 503]);
        assert_eq!(results, vec![false, false, false, false, true]);
    }

    #[test]
    fn success_resets_the_count() {
        let mut guard = LimitGuard::new(5);
        let results = observe_all(&mut guard, &[429, 429, 429, 429, 200, 429, 429, 429, 429]);
        assert!(results.iter().all(|&tripped| !tripped));
    }

    #[test]
    fn wave_of_403_after_200_trips() {
        let mut guard = LimitGuard::new(5);
        let results = observe_all(&mut guard, &[200, 403, 403, 403, 403, 403]);
        assert_eq!(results, vec![false, false, false, false, false, true]);
    }

    #[test]
    fn forbidden_without_prior_success_does_not_trip() {
        let mut guard = LimitGuard::new(5);
        let results = observe_all(&mut guard, &[403; 10]);
        assert!(results.iter().all(|&tripped| !tripped));
    }
}
