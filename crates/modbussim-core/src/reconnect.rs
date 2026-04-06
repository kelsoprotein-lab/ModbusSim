use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Policy that controls automatic reconnection behaviour.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconnectPolicy {
    /// Whether automatic reconnection is enabled.
    pub enabled: bool,
    /// Delay before the first reconnect attempt (milliseconds).
    #[serde(default = "default_initial_delay_ms")]
    pub initial_delay_ms: u64,
    /// Maximum delay between reconnect attempts (milliseconds).
    #[serde(default = "default_max_delay_ms")]
    pub max_delay_ms: u64,
    /// Multiplicative factor applied each attempt.
    #[serde(default = "default_backoff_factor")]
    pub backoff_factor: f64,
    /// Maximum number of attempts. `None` means unlimited.
    #[serde(default)]
    pub max_attempts: Option<u32>,
}

fn default_initial_delay_ms() -> u64 {
    1000
}

fn default_max_delay_ms() -> u64 {
    30_000
}

fn default_backoff_factor() -> f64 {
    2.0
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            initial_delay_ms: default_initial_delay_ms(),
            max_delay_ms: default_max_delay_ms(),
            backoff_factor: default_backoff_factor(),
            max_attempts: None,
        }
    }
}

impl ReconnectPolicy {
    /// Compute the delay before `attempt` (0-based).
    ///
    /// delay = initial_delay_ms * backoff_factor^attempt, clamped to max_delay_ms.
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let delay_ms =
            self.initial_delay_ms as f64 * self.backoff_factor.powi(attempt as i32);
        let clamped = delay_ms.min(self.max_delay_ms as f64) as u64;
        Duration::from_millis(clamped)
    }

    /// Returns `true` when another retry should be made.
    ///
    /// - Always `false` when `enabled` is `false`.
    /// - When `max_attempts` is `Some(n)`, `false` once `attempt >= n`.
    pub fn should_retry(&self, attempt: u32) -> bool {
        if !self.enabled {
            return false;
        }
        match self.max_attempts {
            Some(max) => attempt < max,
            None => true,
        }
    }
}

/// Tracks the current reconnection state of a `MasterConnection`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ReconnectState {
    Idle,
    Reconnecting { attempt: u32 },
    GaveUp { attempts: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy() {
        let p = ReconnectPolicy::default();
        assert!(p.enabled);
        assert_eq!(p.initial_delay_ms, 1000);
        assert_eq!(p.max_delay_ms, 30_000);
        assert_eq!(p.backoff_factor, 2.0);
        assert!(p.max_attempts.is_none());
    }

    #[test]
    fn test_delay_exponential_backoff() {
        let p = ReconnectPolicy::default();
        assert_eq!(p.delay_for_attempt(0), Duration::from_millis(1000));
        assert_eq!(p.delay_for_attempt(1), Duration::from_millis(2000));
        assert_eq!(p.delay_for_attempt(2), Duration::from_millis(4000));
        assert_eq!(p.delay_for_attempt(3), Duration::from_millis(8000));
    }

    #[test]
    fn test_delay_clamped_to_max() {
        let p = ReconnectPolicy::default();
        // attempt 5: 1000 * 2^5 = 32000 > 30000 => clamped to 30000
        assert_eq!(p.delay_for_attempt(5), Duration::from_millis(30_000));
    }

    #[test]
    fn test_should_retry_unlimited() {
        let p = ReconnectPolicy::default();
        for attempt in [0, 1, 100, 1_000_000] {
            assert!(p.should_retry(attempt));
        }
    }

    #[test]
    fn test_should_retry_limited() {
        let p = ReconnectPolicy {
            max_attempts: Some(3),
            ..ReconnectPolicy::default()
        };
        assert!(p.should_retry(0));
        assert!(p.should_retry(1));
        assert!(p.should_retry(2));
        assert!(!p.should_retry(3));
    }

    #[test]
    fn test_should_retry_disabled() {
        let p = ReconnectPolicy {
            enabled: false,
            ..ReconnectPolicy::default()
        };
        for attempt in [0, 1, 100] {
            assert!(!p.should_retry(attempt));
        }
    }

    #[test]
    fn test_reconnect_state_serde() {
        let state = ReconnectState::Reconnecting { attempt: 3 };
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"reconnecting\"") || json.contains("reconnecting"));
        assert!(json.contains("3"));
    }
}
