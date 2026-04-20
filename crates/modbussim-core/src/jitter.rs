//! Per-device jitter: periodic, randomized mutation of register values
//! driven by a pure `apply_tick` function (for testability) and scheduled
//! from the egui app.

use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::register::RegisterMap;

/// Per-device jitter configuration. Persisted inside `SlaveDevice`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct JitterConfig {
    pub enabled: bool,
    pub interval_ms: u64,
    pub mutation_rate: u8,
    pub delta_percent: u8,
    pub affect_coils: bool,
    pub affect_discrete: bool,
    pub affect_holding: bool,
    pub affect_input: bool,
}

impl Default for JitterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_ms: 1000,
            mutation_rate: 30,
            delta_percent: 10,
            affect_coils: true,
            affect_discrete: true,
            affect_holding: true,
            affect_input: true,
        }
    }
}

pub fn apply_tick(
    _map: &mut RegisterMap,
    _cfg: &JitterConfig,
    _rng: &mut impl Rng,
) {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_disabled_with_sensible_values() {
        let d = JitterConfig::default();
        assert!(!d.enabled);
        assert_eq!(d.interval_ms, 1000);
        assert_eq!(d.mutation_rate, 30);
        assert_eq!(d.delta_percent, 10);
        assert!(d.affect_coils && d.affect_discrete && d.affect_holding && d.affect_input);
    }

    #[test]
    fn json_roundtrip() {
        let original = JitterConfig {
            enabled: true,
            interval_ms: 500,
            mutation_rate: 42,
            delta_percent: 7,
            affect_coils: false,
            affect_discrete: true,
            affect_holding: true,
            affect_input: false,
        };
        let s = serde_json::to_string(&original).expect("serialize");
        let back: JitterConfig = serde_json::from_str(&s).expect("deserialize");
        assert_eq!(original, back);
    }
}
