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

pub fn apply_tick(map: &mut RegisterMap, cfg: &JitterConfig, rng: &mut impl Rng) {
    if !cfg.enabled {
        return;
    }
    let rate = cfg.mutation_rate.min(100) as u32;
    let delta_pct = cfg.delta_percent.min(100) as i32;

    if cfg.affect_coils {
        flip_bools(&mut map.coils, rate, rng);
    }
    if cfg.affect_discrete {
        flip_bools(&mut map.discrete_inputs, rate, rng);
    }
    if cfg.affect_holding {
        perturb_u16(&mut map.holding_registers, rate, delta_pct, rng);
    }
    if cfg.affect_input {
        perturb_u16(&mut map.input_registers, rate, delta_pct, rng);
    }
}

fn flip_bools(store: &mut std::collections::HashMap<u16, bool>, rate: u32, rng: &mut impl Rng) {
    for v in store.values_mut() {
        if rng.gen_range(0..100) < rate {
            *v = !*v;
        }
    }
}

fn perturb_u16(
    store: &mut std::collections::HashMap<u16, u16>,
    rate: u32,
    delta_pct: i32,
    rng: &mut impl Rng,
) {
    for v in store.values_mut() {
        if rng.gen_range(0..100) >= rate {
            continue;
        }
        if delta_pct == 0 {
            continue;
        }
        // Use the current value (min 1 so drift works on zero-seeded registers).
        let base = (*v as i32).max(1);
        let pct = rng.gen_range(-delta_pct..=delta_pct);
        let mut delta = base * pct / 100;
        // 保底：当 base 较小（如 0/1）时整数除法 base*pct/100 会被截断为 0，
        // 导致零值寄存器永远不动。pct≠0 但 delta=0 时强制给 ±1（符号随 pct）。
        if delta == 0 && pct != 0 {
            delta = pct.signum();
        }
        *v = (*v).wrapping_add(delta as u16);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn fixture_map() -> RegisterMap {
        let mut m = RegisterMap::new();
        for addr in 0..10u16 {
            m.coils.insert(addr, false);
            m.discrete_inputs.insert(addr, false);
            m.holding_registers.insert(addr, 1000);
            m.input_registers.insert(addr, 2000);
        }
        m
    }

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

    #[test]
    fn mutation_rate_zero_leaves_map_unchanged() {
        let mut map = fixture_map();
        let expected = map.clone();
        let cfg = JitterConfig {
            enabled: true,
            interval_ms: 100,
            mutation_rate: 0,
            delta_percent: 50,
            affect_coils: true,
            affect_discrete: true,
            affect_holding: true,
            affect_input: true,
        };
        let mut rng = StdRng::seed_from_u64(42);
        apply_tick(&mut map, &cfg, &mut rng);
        assert_eq!(map.coils, expected.coils);
        assert_eq!(map.discrete_inputs, expected.discrete_inputs);
        assert_eq!(map.holding_registers, expected.holding_registers);
        assert_eq!(map.input_registers, expected.input_registers);
    }

    #[test]
    fn full_rate_flips_all_bools_and_perturbs_all_u16() {
        let mut map = fixture_map();
        let cfg = JitterConfig {
            enabled: true,
            interval_ms: 100,
            mutation_rate: 100,
            delta_percent: 50,
            affect_coils: true,
            affect_discrete: true,
            affect_holding: true,
            affect_input: true,
        };
        let mut rng = StdRng::seed_from_u64(42);
        apply_tick(&mut map, &cfg, &mut rng);
        // All bool registers started at false; with mutation_rate=100 each must flip to true.
        assert!(map.coils.values().all(|&v| v));
        assert!(map.discrete_inputs.values().all(|&v| v));
        // All u16 started at 1000; with delta_percent=50 each result must land in [500, 1500]
        // because the drift is computed as value * rand(-50..=50) / 100 then wrapping_add to value.
        for &v in map.holding_registers.values() {
            assert!(
                (500..=1500).contains(&v),
                "holding out of drift range: {}",
                v
            );
        }
        for &v in map.input_registers.values() {
            assert!(
                (1000..=3000).contains(&v),
                "input out of drift range: {}",
                v
            );
        }
    }

    #[test]
    fn type_selection_filters_which_registers_mutate() {
        let mut map = fixture_map();
        let baseline = map.clone();
        let cfg = JitterConfig {
            enabled: true,
            interval_ms: 100,
            mutation_rate: 100,
            delta_percent: 50,
            affect_coils: false,
            affect_discrete: false,
            affect_holding: true,
            affect_input: false,
        };
        let mut rng = StdRng::seed_from_u64(42);
        apply_tick(&mut map, &cfg, &mut rng);
        // Coils / discrete / input should be untouched.
        assert_eq!(map.coils, baseline.coils);
        assert_eq!(map.discrete_inputs, baseline.discrete_inputs);
        assert_eq!(map.input_registers, baseline.input_registers);
        // Holding should have shifted for every address.
        for &v in map.holding_registers.values() {
            assert!((500..=1500).contains(&v));
        }
    }

    /// 回归测试：之前 perturb_u16 用 `base * pct / 100` 整数除法，零值寄存器
    /// base=1 时 delta 永远被截断为 0，导致 FC03/FC04 在初值全 0 的常见场景下
    /// 不动。修复后保底 ±1，确保至少能起步漂移。
    #[test]
    fn zero_seeded_holding_registers_actually_drift() {
        let mut map = RegisterMap::new();
        for addr in 0..16u16 {
            map.holding_registers.insert(addr, 0);
            map.input_registers.insert(addr, 0);
        }
        let cfg = JitterConfig {
            enabled: true,
            interval_ms: 100,
            mutation_rate: 100,
            delta_percent: 10,
            affect_coils: false,
            affect_discrete: false,
            affect_holding: true,
            affect_input: true,
        };
        let mut rng = StdRng::seed_from_u64(7);
        apply_tick(&mut map, &cfg, &mut rng);
        let any_holding_moved = map.holding_registers.values().any(|&v| v != 0);
        let any_input_moved = map.input_registers.values().any(|&v| v != 0);
        assert!(
            any_holding_moved,
            "零值 holding registers 在 100% mutation rate 下应至少有一个动"
        );
        assert!(
            any_input_moved,
            "零值 input registers 在 100% mutation rate 下应至少有一个动"
        );
    }
}
