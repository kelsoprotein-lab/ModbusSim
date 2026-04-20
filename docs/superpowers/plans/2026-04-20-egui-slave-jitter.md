# egui Slave 随机抖动 + DataSource UI 扩展 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** egui Slave 每个 Device 可独立开启"寄存器抖动"（周期性按概率按百分比漂移已定义寄存器），并补齐 DataSource 快添加 ComboBox 到 7 种。

**Architecture:** 纯函数 `modbussim_core::jitter::apply_tick` 负责单 tick 变位；`SlaveDevice` 加 `jitter: JitterConfig` 字段（`#[serde(default)]` 保兼容）。调度复用已有 egui `SlaveApp` 里的 50 ms 后台 runner 模式——新增第二个独立 tokio 任务扫 `connections → devices → jitter`，到期调用 `apply_tick`。UI 侧在 `Selection::Device` 分支插入一个抖动卡片，改字段直写 `devices.write().await` 的 `SlaveDevice.jitter`（不引入 `watch` 通道）。

**Tech Stack:** Rust 2021 · tokio 1.x · egui 0.29 / eframe · rand 0.8（已经是 core 依赖）· serde JSON

**Branch:** `refactor/egui-skeleton`（继续使用，不新建）

**Spec:** `docs/superpowers/specs/2026-04-20-egui-slave-jitter-design.md`

## File Structure

| 文件 | 动作 | 责任 |
|---|---|---|
| `crates/modbussim-core/src/jitter.rs` | 新建 | `JitterConfig` 结构 + `Default` impl + `apply_tick` 纯函数 + 单测 |
| `crates/modbussim-core/src/lib.rs` | 改 | `pub mod jitter;` + `pub use jitter::JitterConfig;` |
| `crates/modbussim-core/src/slave.rs` | 改 | `SlaveDevice` 加 `#[serde(default)] pub jitter: JitterConfig` |
| `crates/modbussim-egui/src/app.rs` | 改 | `DsKind` 扩 3 个变体；`Selection::Device` 加抖动卡片；新 jitter tokio runner |

---

## Task 1: core — JitterConfig 结构 + 序列化测试

**Files:**
- Create: `crates/modbussim-core/src/jitter.rs`
- Modify: `crates/modbussim-core/src/lib.rs` (新增 `pub mod jitter;` 一行)

- [ ] **Step 1: 写失败测试**

创建 `crates/modbussim-core/src/jitter.rs`，暂时只有测试骨架：

```rust
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
```

在 `crates/modbussim-core/src/lib.rs` 找到 `pub mod data_source;` 这类 `pub mod` 声明段，追加一行：

```rust
pub mod jitter;
```

- [ ] **Step 2: 运行测试验证失败（尚未注册模块即报错）**

Run: `cargo test -p modbussim-core --lib jitter::tests --no-run`
Expected: 第一次改动后应编译通过（因为 `apply_tick` 的 `unimplemented!()` 只在调用时 panic，测试不调它）。测试应全 PASS：

```
running 2 tests
test jitter::tests::default_is_disabled_with_sensible_values ... ok
test jitter::tests::json_roundtrip ... ok
```

如果编译失败，是因为忘了 `pub mod jitter;` 或 `serde_json` 不在 core 的 [dev-dependencies]。

确认 `crates/modbussim-core/Cargo.toml` 的 `[dev-dependencies]` 包含 `serde_json`；已包含就不用加。若没有，则加：

```toml
[dev-dependencies]
serde_json = "1"
```

- [ ] **Step 3: 提交**

```bash
git add crates/modbussim-core/src/jitter.rs crates/modbussim-core/src/lib.rs crates/modbussim-core/Cargo.toml
git commit -m "feat(core): JitterConfig 结构 + 序列化测试（apply_tick 待实现）"
```

---

## Task 2: core — apply_tick 实现 + 4 个行为测试

**Files:**
- Modify: `crates/modbussim-core/src/jitter.rs`

- [ ] **Step 1: 扩充失败测试（4 个新测试）**

把 `tests` 模块整段替换为：

```rust
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
            assert!((500..=1500).contains(&v), "holding out of drift range: {}", v);
        }
        for &v in map.input_registers.values() {
            assert!((1000..=3000).contains(&v), "input out of drift range: {}", v);
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
}
```

- [ ] **Step 2: 运行测试验证 4 个行为测试 FAIL（apply_tick 还是 unimplemented!）**

Run: `cargo test -p modbussim-core --lib jitter::tests 2>&1 | tail -20`
Expected: 2 个结构测试 PASS，3 个行为测试因为 `unimplemented!()` 触发 panic 而 FAIL。

- [ ] **Step 3: 实现 apply_tick**

把 `pub fn apply_tick` 整段替换为：

```rust
pub fn apply_tick(
    map: &mut RegisterMap,
    cfg: &JitterConfig,
    rng: &mut impl Rng,
) {
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

fn flip_bools(
    store: &mut std::collections::HashMap<u16, bool>,
    rate: u32,
    rng: &mut impl Rng,
) {
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
        let delta = base * pct / 100;
        *v = (*v).wrapping_add(delta as u16);
    }
}
```

> 注意：`mutation_rate=100 delta_percent=50` 下 `full_rate_flips_all_bools_and_perturbs_all_u16` 里 `input_registers` 原值 2000，漂移后范围 `[1000, 3000]`。测试已写对。

- [ ] **Step 4: 运行测试验证全部 PASS**

Run: `cargo test -p modbussim-core --lib jitter::tests 2>&1 | tail -10`
Expected:

```
running 5 tests
test jitter::tests::default_is_disabled_with_sensible_values ... ok
test jitter::tests::json_roundtrip ... ok
test jitter::tests::mutation_rate_zero_leaves_map_unchanged ... ok
test jitter::tests::full_rate_flips_all_bools_and_perturbs_all_u16 ... ok
test jitter::tests::type_selection_filters_which_registers_mutate ... ok

test result: ok. 5 passed
```

- [ ] **Step 5: 提交**

```bash
git add crates/modbussim-core/src/jitter.rs
git commit -m "feat(core): jitter::apply_tick 纯函数（bool 翻转 / u16 百分比漂移）"
```

---

## Task 3: core — SlaveDevice 加 jitter 字段（兼容老工程）

**Files:**
- Modify: `crates/modbussim-core/src/slave.rs:18-24` (SlaveDevice 结构 + new)

- [ ] **Step 1: 写失败测试**

在 `crates/modbussim-core/src/slave.rs` 文件末尾的 `#[cfg(test)] mod tests` 里（找到已有的 `mod tests` block，追加进去）加两个新测试：

```rust
    #[test]
    fn slave_device_has_default_jitter() {
        let d = SlaveDevice::new(1, "s1");
        assert!(!d.jitter.enabled);
        assert_eq!(d.jitter.interval_ms, 1000);
    }

    #[test]
    fn slave_device_deserializes_legacy_json_without_jitter() {
        // Older .modbusproj files wrote SlaveDevice without a `jitter` field.
        let legacy = r#"{
            "slave_id": 1,
            "name": "legacy",
            "register_map": {
                "coils": {},
                "discrete_inputs": {},
                "holding_registers": {},
                "input_registers": {}
            },
            "register_defs": []
        }"#;
        let d: SlaveDevice = serde_json::from_str(legacy).expect("legacy parse");
        assert_eq!(d.slave_id, 1);
        assert_eq!(d.name, "legacy");
        assert!(!d.jitter.enabled); // default
    }
```

- [ ] **Step 2: 运行测试验证 FAIL（jitter 字段还不存在）**

Run: `cargo test -p modbussim-core --lib slave::tests::slave_device_ 2>&1 | tail -15`
Expected: 编译错误 `no field jitter on type SlaveDevice`.

- [ ] **Step 3: 加 jitter 字段到 SlaveDevice + 更新 constructor**

找到 `crates/modbussim-core/src/slave.rs` 的 `pub struct SlaveDevice`（行 18 附近）：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaveDevice {
    pub slave_id: u8,
    pub name: String,
    pub register_map: RegisterMap,
    pub register_defs: Vec<RegisterDef>,
}
```

改为：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaveDevice {
    pub slave_id: u8,
    pub name: String,
    pub register_map: RegisterMap,
    pub register_defs: Vec<RegisterDef>,
    #[serde(default)]
    pub jitter: crate::jitter::JitterConfig,
}
```

并在紧邻的 `pub fn new` 里加 `jitter: Default::default(),` 到 struct literal。原 `new` 是：

```rust
    pub fn new(slave_id: u8, name: impl Into<String>) -> Self {
        Self {
            slave_id,
            name: name.into(),
            register_map: RegisterMap::new(),
            register_defs: Vec::new(),
        }
    }
```

改为：

```rust
    pub fn new(slave_id: u8, name: impl Into<String>) -> Self {
        Self {
            slave_id,
            name: name.into(),
            register_map: RegisterMap::new(),
            register_defs: Vec::new(),
            jitter: crate::jitter::JitterConfig::default(),
        }
    }
```

同文件若有其他 `SlaveDevice { ... }` literal 构造（比如 `with_default_registers`），因为它调用 `Self::new` 再 mutate，不用改。grep 一下确认。

- [ ] **Step 4: 运行测试验证全部 PASS**

Run: `cargo test -p modbussim-core --lib slave::tests 2>&1 | tail -5`
Expected: 全部测试 PASS（包括前面修过的 4 个 test_handle_write_* 和 2 个新的 slave_device_*）。

附加校验：workspace 级别回归：

Run: `cargo test --workspace --exclude modbussim-app --exclude modbusmaster-app 2>&1 | tail -3`
Expected: `test result: ok.` for every test executable.

- [ ] **Step 5: 提交**

```bash
git add crates/modbussim-core/src/slave.rs
git commit -m "feat(core): SlaveDevice 加 jitter 字段（#[serde(default)] 保兼容老工程）"
```

---

## Task 4: egui — DsKind 扩 3 个变体 + ComboBox

**Files:**
- Modify: `crates/modbussim-egui/src/app.rs:56-94` (DsKind enum + impl)
- Modify: `crates/modbussim-egui/src/app.rs:1904-1915` (ComboBox 下拉列表)

- [ ] **Step 1: 扩展 DsKind enum + label + default_source**

找到 `crates/modbussim-egui/src/app.rs` 行 56–94 的 `pub enum DsKind` 整段：

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DsKind {
    Counter,
    Sine,
    Random,
    Fixed,
}

impl DsKind {
    pub fn label(&self) -> &'static str {
        match self {
            DsKind::Counter => "计数器 (+1)",
            DsKind::Sine => "正弦波",
            DsKind::Random => "随机 U16",
            DsKind::Fixed => "固定值",
        }
    }

    pub fn default_source(&self) -> DataSource {
        match self {
            DsKind::Counter => DataSource::Counter {
                start: 0,
                step: 1,
                wrap: true,
            },
            DsKind::Sine => DataSource::Sine {
                amplitude: 10000.0,
                frequency: 0.5,
                offset: 32768.0,
                phase: 0.0,
            },
            DsKind::Random => DataSource::Random {
                min: 0,
                max: 65535,
            },
            DsKind::Fixed => DataSource::Fixed { value: 42 },
        }
    }
}
```

替换为：

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DsKind {
    Counter,
    Sine,
    Sawtooth,
    Triangle,
    Random,
    Fixed,
    CsvPlayback,
}

impl DsKind {
    pub fn label(&self) -> &'static str {
        match self {
            DsKind::Counter => "计数器 (+1)",
            DsKind::Sine => "正弦波",
            DsKind::Sawtooth => "锯齿波",
            DsKind::Triangle => "三角波",
            DsKind::Random => "随机 U16",
            DsKind::Fixed => "固定值",
            DsKind::CsvPlayback => "CSV 序列",
        }
    }

    pub fn default_source(&self) -> DataSource {
        match self {
            DsKind::Counter => DataSource::Counter {
                start: 0,
                step: 1,
                wrap: true,
            },
            DsKind::Sine => DataSource::Sine {
                amplitude: 10000.0,
                frequency: 0.5,
                offset: 32768.0,
                phase: 0.0,
            },
            DsKind::Sawtooth => DataSource::Sawtooth {
                min: 0,
                max: 1000,
                period_ms: 5000,
            },
            DsKind::Triangle => DataSource::Triangle {
                min: 0,
                max: 1000,
                period_ms: 5000,
            },
            DsKind::Random => DataSource::Random {
                min: 0,
                max: 65535,
            },
            DsKind::Fixed => DataSource::Fixed { value: 42 },
            DsKind::CsvPlayback => DataSource::CsvPlayback {
                values: vec![0, 100, 200, 300, 400],
                loop_playback: true,
            },
        }
    }
}
```

- [ ] **Step 2: 在 ComboBox 下拉里加入 3 个新变体**

找到 `crates/modbussim-egui/src/app.rs` 行 1904–1915 的 `egui::ComboBox::from_id_salt("ds_kind")`：

```rust
                            egui::ComboBox::from_id_salt("ds_kind")
                                .selected_text(self.ds_add_kind.label())
                                .show_ui(ui, |ui| {
                                    for k in [
                                        DsKind::Counter,
                                        DsKind::Sine,
                                        DsKind::Random,
                                        DsKind::Fixed,
                                    ] {
                                        ui.selectable_value(&mut self.ds_add_kind, k, k.label());
                                    }
                                });
```

改为：

```rust
                            egui::ComboBox::from_id_salt("ds_kind")
                                .selected_text(self.ds_add_kind.label())
                                .show_ui(ui, |ui| {
                                    for k in [
                                        DsKind::Counter,
                                        DsKind::Sine,
                                        DsKind::Sawtooth,
                                        DsKind::Triangle,
                                        DsKind::Random,
                                        DsKind::Fixed,
                                        DsKind::CsvPlayback,
                                    ] {
                                        ui.selectable_value(&mut self.ds_add_kind, k, k.label());
                                    }
                                });
```

- [ ] **Step 3: 编译通过**

Run: `cargo build -p modbussim-egui 2>&1 | tail -5`
Expected: `Finished \`dev\` profile` 无错误（几个 `unused_variable` warning 可忽略）。

- [ ] **Step 4: 单元冒烟（启动目视选项）**

Run `pkill -f modbussim-egui 2>/dev/null; ./target/debug/modbussim-egui --auto-tcp 127.0.0.1:5502 &`；在 UI 里选一个 Device，点"数据源"下拉，应看到 7 种：`计数器 / 正弦波 / 锯齿波 / 三角波 / 随机 U16 / 固定值 / CSV 序列`。

（实施本 task 的 agent 可以跳过手测，用 `cargo build` 编译通过作为最小验收；下一个 task 完成后会统一 smoke。）

- [ ] **Step 5: 提交**

```bash
git add crates/modbussim-egui/src/app.rs
git commit -m "feat(egui): DataSource 快添加补齐 Sawtooth / Triangle / CsvPlayback"
```

---

## Task 5: egui — Selection::Device 加抖动卡片 UI（直写 devices）

**Files:**
- Modify: `crates/modbussim-egui/src/app.rs:1807-1835` (Selection::Device 设备卡片之后、数据源 section 之前)

- [ ] **Step 1: 在 Selection::Device 分支插入 Jitter 卡片**

找到 `crates/modbussim-egui/src/app.rs:1807`（`ui.end_row();` 后面 `ui.separator();` 前面的区域）。下面这段代码的插入位置紧跟行 1833 的 `});`（即`批量添加 / 删除从站`按钮行结束）之后、`ui.separator();`（开启"数据源"section）之前：

```rust
                        // ----- Jitter card (run-time register mutation for master-stress) -----
                        ui.separator();
                        // Pass empty icon: phosphor font isn't actually embedded, so any
                        // phosphor glyph renders as a placeholder box (same as "✕" did).
                        uikit::section_heading(ui, "", "寄存器抖动（压测）");

                        // Read current jitter config from the shared state (blocking,
                        // OK because we hold no other locks here).
                        let cur_jitter: modbussim_core::jitter::JitterConfig = {
                            let conns = self.connections.blocking_read();
                            let entry = conns.iter().find(|e| e.id == *conn_id);
                            let jitter_opt = entry.and_then(|e| {
                                let conn = e.connection.blocking_read();
                                let devs = conn.devices.blocking_read();
                                devs.get(slave_id).map(|d| d.jitter.clone())
                            });
                            jitter_opt.unwrap_or_default()
                        };
                        let mut new_jitter = cur_jitter.clone();
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut new_jitter.enabled, "启用");
                        });
                        let mut interval = new_jitter.interval_ms as i32;
                        let mut rate = new_jitter.mutation_rate as i32;
                        let mut delta = new_jitter.delta_percent as i32;
                        egui::Grid::new("jitter_grid")
                            .num_columns(2)
                            .spacing([12.0, 6.0])
                            .show(ui, |ui| {
                                ui.label("周期");
                                ui.add(
                                    egui::Slider::new(&mut interval, 100..=5000)
                                        .suffix(" ms"),
                                );
                                ui.end_row();
                                ui.label("变位率");
                                ui.add(
                                    egui::Slider::new(&mut rate, 0..=100)
                                        .suffix(" %"),
                                );
                                ui.end_row();
                                ui.label("漂移幅度");
                                ui.add(
                                    egui::Slider::new(&mut delta, 0..=100)
                                        .suffix(" %"),
                                );
                                ui.end_row();
                            });
                        new_jitter.interval_ms = interval as u64;
                        new_jitter.mutation_rate = rate as u8;
                        new_jitter.delta_percent = delta as u8;
                        ui.horizontal(|ui| {
                            ui.label("影响范围");
                            ui.checkbox(&mut new_jitter.affect_coils, "线圈");
                            ui.checkbox(&mut new_jitter.affect_discrete, "离散");
                            ui.checkbox(&mut new_jitter.affect_holding, "保持");
                            ui.checkbox(&mut new_jitter.affect_input, "输入");
                        });

                        if new_jitter != cur_jitter {
                            // Write new config back to the shared state.
                            self.set_device_jitter(conn_id.clone(), *slave_id, new_jitter);
                        }
```

> **关于 `uikit::section_heading` 的 icon 常量**：本仓库 `crates/modbussim-ui-shared/src/icons.rs` 里是否已有 `WAVE_SINE`？如果没有，用 `icons::GEAR`（已在其他地方使用过）作为占位即可。

- [ ] **Step 2: 加 set_device_jitter 方法**

在 `crates/modbussim-egui/src/app.rs` 的 `impl SlaveApp` 里紧跟 `fn remove_data_source` 之后（行 568 附近）加：

```rust
    fn set_device_jitter(
        &self,
        conn_id: String,
        slave_id: u8,
        cfg: modbussim_core::jitter::JitterConfig,
    ) {
        let connections = self.connections.clone();
        self.rt.spawn(async move {
            let conns = connections.read().await;
            let Some(entry) = conns.iter().find(|e| e.id == conn_id) else { return };
            let conn = entry.connection.read().await;
            let mut devs = conn.devices.write().await;
            if let Some(dev) = devs.get_mut(&slave_id) {
                dev.jitter = cfg;
            }
        });
    }
```

- [ ] **Step 3: 编译通过**

Run: `cargo build -p modbussim-egui 2>&1 | tail -5`
Expected: 编译成功。如果 `icons::WAVE_SINE` 不存在就改成 `icons::GEAR`；如果 `self.connections.blocking_read()` 提示 `RwLock` 不支持 blocking，看是否 app.rs 已经 `use tokio::sync::RwLock;`（应是），那么 `.blocking_read()` 需要使用 `tokio::sync::RwLock::blocking_read` — 这个 API 存在。若 `modbussim_core::jitter` 路径拼错就改为 `modbussim_core::JitterConfig`（Task 1 Step 1 里 `lib.rs` 把 `pub use jitter::JitterConfig` 也 re-export 了）。

- [ ] **Step 4: 冒烟（目视）**

Run: `pkill -f modbussim-egui 2>/dev/null; sleep 1; ./target/debug/modbussim-egui --auto-tcp 127.0.0.1:5502 &`
在 UI 里：连接 → 从站 → 选中"从站 1" → 应看到"寄存器抖动（压测）"卡片，启用 checkbox、3 个 slider、4 个 type checkbox。此时拖动 slider 不应导致崩溃；但**寄存器不会动**（因为后台 jitter runner 还没实现）。

- [ ] **Step 5: 提交**

```bash
git add crates/modbussim-egui/src/app.rs
git commit -m "feat(egui): Selection::Device 视图加抖动卡片（UI 端，write 到 SlaveDevice.jitter）"
```

---

## Task 6: egui — 后台 jitter runner（订阅 SlaveDevice.jitter）

**Files:**
- Modify: `crates/modbussim-egui/src/app.rs:305-355` (DataSource runner 所在的 `impl SlaveApp` → `fn new` block) — 在 DataSource runner 之后再 spawn 一个 jitter runner

- [ ] **Step 1: 在 SlaveApp::new 里追加 jitter runner**

找到 `crates/modbussim-egui/src/app.rs` 的 DataSource runner block（行 308-355 的 `{` → `}`），在它的闭合 `}` 后（仍在 `pub fn new` 内部，在 `Self { ... }` 之前）加：

```rust
        // Background jitter runner: every 100 ms, iterate connections → devices,
        // apply `jitter::apply_tick` to any device whose jitter is enabled and
        // whose interval_ms has elapsed since its last tick.
        {
            let connections = connections.clone();
            rt.spawn(async move {
                use rand::SeedableRng;
                let mut rng = rand::rngs::StdRng::from_entropy();
                let mut last_tick: std::collections::HashMap<(String, u8), Instant> =
                    std::collections::HashMap::new();
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    let now = Instant::now();
                    let conns = connections.read().await;
                    for entry in conns.iter() {
                        let conn_id = entry.id.clone();
                        let conn = entry.connection.read().await;
                        let mut devs = conn.devices.write().await;
                        for (slave_id, dev) in devs.iter_mut() {
                            if !dev.jitter.enabled {
                                last_tick.remove(&(conn_id.clone(), *slave_id));
                                continue;
                            }
                            let interval = std::time::Duration::from_millis(
                                dev.jitter.interval_ms.clamp(100, 5000),
                            );
                            let key = (conn_id.clone(), *slave_id);
                            let due = match last_tick.get(&key) {
                                Some(t) => now.duration_since(*t) >= interval,
                                None => true,
                            };
                            if !due {
                                continue;
                            }
                            modbussim_core::jitter::apply_tick(
                                &mut dev.register_map,
                                &dev.jitter,
                                &mut rng,
                            );
                            last_tick.insert(key, now);
                        }
                    }
                }
            });
        }
```

> 注意：这个 runner 握 `connections → entry.connection → devices` 三层锁，每 100 ms 扫一遍。压测场景下 SlaveConnection 少（通常 1–2 个）+ Device 少（通常 1–10 个），开销可接受。如果未来加多连接，可以改成按连接拆 task。

- [ ] **Step 2: 编译通过**

Run: `cargo build -p modbussim-egui 2>&1 | tail -5`
Expected: 编译成功。

- [ ] **Step 3: 冒烟**

```bash
pkill -f modbussim-egui 2>/dev/null; sleep 1
./target/debug/modbussim-egui --auto-tcp 127.0.0.1:5502 &
sleep 2
mbpoll -m tcp -a 1 -r 0 -c 10 127.0.0.1 -p 5502
```

在 UI 里：选中从站 → 点"批量添加" → 在 FC03 添加 addr 0–9 默认值 100 → 返回 Device 视图 → 启用"寄存器抖动"，周期 500 ms / 变位率 100 % / 漂移 50 % / 只勾选"保持"。

然后：

```bash
mbpoll -m tcp -a 1 -r 0 -c 10 127.0.0.1 -p 5502  # 第 1 次
sleep 2
mbpoll -m tcp -a 1 -r 0 -c 10 127.0.0.1 -p 5502  # 第 2 次，应与第 1 次数值不同
```

两次值应有差异（大部分地址值不同）。如果两次完全一样：检查 jitter.enabled 是否真正被 set_device_jitter 写入（加日志 `eprintln!("jitter tick for {} {}", conn_id, slave_id);`debug 一下）。

- [ ] **Step 4: 提交**

```bash
git add crates/modbussim-egui/src/app.rs
git commit -m "feat(egui): jitter 后台 runner（每 100 ms 扫 SlaveDevice.jitter 调 apply_tick）"
```

---

## Task 7: 回归 + CI + push

- [ ] **Step 1: 全 workspace 测试**

Run: `cargo test --workspace --exclude modbussim-app --exclude modbusmaster-app 2>&1 | tail -5`
Expected: 全 PASS。

- [ ] **Step 2: egui 双端 release build**

Run: `cargo build --release -p modbussim-egui -p modbusmaster-egui 2>&1 | tail -3`
Expected: `Finished \`release\` profile` 无错误。

- [ ] **Step 3: 手工冒烟**

同 Task 6 Step 3 的 mbpoll 测试再跑一次；此外：
1. 关闭 jitter → 后续 mbpoll 两次读数值应完全一致
2. 只勾选"线圈" → 读 holding register 两次应一致（FC01 bool 在变）
3. 重启应用 → jitter 字段应丢回 Default（未序列化进工程 / 或序列化进了也是合理）—— 确认取决于 .modbusproj 保存时机

- [ ] **Step 4: push + 观察 CI**

```bash
git push origin refactor/egui-skeleton
gh run list --branch refactor/egui-skeleton --limit 2
```

CI (`ci-egui.yml`) 应该在三平台全绿。

- [ ] **Step 5: 合并 commit 或留独立 commits**

不用额外 commit，前 6 个 task 已经分别 commit。

---

## 不在本 plan 范围（后续 sprint）

- Jitter 作用地址范围筛选（start_addr / end_addr）
- Jitter 模式切换（drift / flip / walk 混合）
- DataSource 参数编辑对话框（让用户自定义 Sawtooth period 等）
- Tauri 版同等功能迁移
- Jitter 值 clamp 到用户自定义 min/max（目前用 `wrapping_add`）
