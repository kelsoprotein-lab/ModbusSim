# egui Slave 随机抖动 + DataSource UI 扩展

**Date**: 2026-04-20
**Branch**: `refactor/egui-skeleton`
**Scope**: 仅 `modbussim-egui` 与 `modbussim-core`；Tauri 版 `modbussim-app` 不动

## 背景

egui 版 Slave 相对 Tauri 版缺失运行时"随机变位"（压测 Master 用），且 DataSource 快添加 ComboBox 只暴露 4 种（Fixed / Random / Counter / Sine），core 已有的 Sawtooth / Triangle / CsvPlayback 没有 UI 入口。

随机初始化已有（`DeviceInitMode::Random`，等价 Tauri 的 `with_random_registers`），不在本 spec 范围。

## 目标

1. egui Slave 每个 Device 可独立开启"寄存器抖动"：周期性、按概率、按百分比漂移已定义的寄存器值，模拟压测场景下 Master 端需要持续响应变化。
2. DataSource 快添加菜单补齐到 7 种，Sawtooth / Triangle / CsvPlayback 用默认参数即可添加。
3. 老 `.modbusproj` 文件仍能加载（`#[serde(default)]`）。

## 非目标

- DataSource 参数编辑对话框（留给后续 sprint）
- Jitter 地址范围筛选（起始/结束）
- Jitter 模式切换（drift / flip / walk 混合）
- Tauri 版同等功能迁移

## 数据模型

### `modbussim-core::jitter` （新模块）

```rust
/// Per-device jitter configuration. Serialized into .modbusproj.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct JitterConfig {
    pub enabled: bool,
    pub interval_ms: u64,          // 100..=5000, 默认 1000
    pub mutation_rate: u8,         // 0..=100 (百分比), 默认 30
    pub delta_percent: u8,         // 0..=100, 当前值的 ±百分比漂移, 默认 10
    pub affect_coils: bool,        // 默认 true
    pub affect_discrete: bool,     // 默认 true
    pub affect_holding: bool,      // 默认 true
    pub affect_input: bool,        // 默认 true
}

impl Default for JitterConfig {
    fn default() -> Self { /* 上面的默认值 */ }
}
```

### `Device` 变更

在 `modbussim-core` 的 `Device` struct（位置 `device.rs` 或 `slave.rs`，以探索为准）加字段：

```rust
#[serde(default)]
pub jitter: JitterConfig,
```

老工程文件缺字段时 serde 填 `Default`，全部 `affect_*=true + enabled=false`，即不启用且不破兼容。

## 调度

### 纯函数：单 tick 变位

```rust
// modbussim-core::jitter
pub fn apply_tick(
    map: &mut RegisterMap,
    cfg: &JitterConfig,
    rng: &mut impl rand::RngCore,
);
```

职责：遍历 4 类寄存器（按 `affect_*` 开关），对已定义的 key，以 `mutation_rate/100` 概率命中：
- coil / discrete：`value = !value`
- holding / input：`delta = value.max(1) as i32 * (rand_range(-delta_percent..=delta_percent) as i32) / 100; value = value.wrapping_add(delta as u16)`

> 用 `.max(1)` 避免 0 × 任何百分比 = 0 永远不动；对 0 值按 `delta_percent` 取基准 `1` 保证能起步。

单元测试：
- `mutation_rate=0` → map 不变
- `mutation_rate=100, delta_percent=0` → map 不变（u16 类）但 bool 类会翻
- `mutation_rate=100, delta_percent=50` → 每个 u16 在 [value*0.5, value*1.5] 区间
- 用固定 seed `StdRng::seed_from_u64(42)` 确保可重现

### Runner 任务

每个 Device 独立起一个 tokio task：

```rust
// egui 侧在 app.rs（或提到 modbussim-core）
async fn run_jitter(
    map: Arc<RwLock<RegisterMap>>,
    mut cfg_rx: watch::Receiver<JitterConfig>,
    mut stop_rx: oneshot::Receiver<()>,
);
```

循环逻辑：
1. 每次外层 loop 读当前 cfg；`!cfg.enabled` → `cfg_rx.changed().await` 阻塞到被改启用
2. 起 `interval(Duration::from_millis(cfg.interval_ms.clamp(100, 5000)))`
3. 内层 select：`stop_rx / cfg_rx.changed() / interval.tick()`
   - 收到 stop → return
   - 收到 cfg 变化 → break 内层，重新读 cfg
   - tick → `map.write().await` + `apply_tick(&mut *m, &cfg, &mut rng)`

**并发一致性**：沿用现有 `SlaveConnection` 里的 `Arc<RwLock<RegisterMap>>`。Jitter 与 DataSource Runner 以及 Modbus 请求处理共用同一把 `write` 锁，MVP 阶段不考虑读写分离。压测场景下 interval ≥ 100ms 不会有锁竞争瓶颈。

## UI

### Selection::Device 视图

在 `app.rs` 中 `Selection::Device` 分支主区加入新卡片，放在"设备名 / 初始化模式"之下：

```
┌ 寄存器抖动（压测）──────────────────────┐
│ [✓] 启用                                  │
│ 周期      [====|=====] 1000 ms            │
│ 变位率    [===|======]   30 %             │
│ 漂移幅度  [==|=======]   10 %             │
│ 影响范围  [✓]线圈 [✓]离散 [✓]保持 [✓]输入│
└──────────────────────────────────────────┘
```

实现细节：
- 3 个 `egui::Slider::new` 水平占满宽度，数字显示在右端
- 类型复选按 `ui.horizontal` 横排
- 任意字段改动 → `app.dispatch(UiCommand::SetJitter { slave_id, device_id, cfg })`
- 已启用且收到 cfg 变更时，后台 task 通过 `watch::Sender` 推送新 cfg，无需 respawn
- Device 删除时（现有 `DeleteDevice` 命令）发 stop

### DataSource ComboBox

`ds_add_kind` 的 `egui::ComboBox` 加入枚举 `Sawtooth / Triangle / CsvPlayback`，选项 label 保持 core 现有命名一致。添加时 spawn 使用 core 已有 `DataSourceConfig::default_for_kind(kind)` 辅助函数；如不存在，在本 spec 里新增此函数返回每种 kind 的默认参数：

```rust
// modbussim-core::data_source
pub fn default_for_kind(kind: DataSourceKind) -> DataSourceConfig { ... }
```

默认参数：
- `Sawtooth { min: 0, max: 1000, period_ms: 5000 }`
- `Triangle { min: 0, max: 1000, period_ms: 5000 }`
- `CsvPlayback { values: vec![0, 100, 200, 300, 400], loop_playback: true }`

## 持久化

`.modbusproj` schema 保持 v2，仅 Device 增一个可选字段。roundtrip 测试新增：老文件（无 `jitter`）→ 加载 → 保存 → diff 与原文件只差一个 `jitter: { ... default ... }` 段（或不写出，取决于 serde skip_if_default；为简化就正常写出）。

## 验证

### 单测（modbussim-core）

- `jitter::apply_tick` 系列如上
- `DeviceConfig` serde roundtrip 包含 jitter 默认值
- 老工程 JSON（缺 `jitter`）解析成功且 jitter = Default

### 冒烟（本地）

```bash
./target/debug/modbussim-egui --auto-tcp 127.0.0.1:5502 &
# UI：选中 Device → 打开抖动 → 周期 500ms / 变位率 50% / 漂移 20%
mbpoll -m tcp -a 1 -r 0 -c 10 127.0.0.1 -p 5502   # 第 1 次读
sleep 2
mbpoll -m tcp -a 1 -r 0 -c 10 127.0.0.1 -p 5502   # 第 2 次读，应有差异
```

### CI

workspace 测试不回归；`cargo build --release -p modbussim-egui` 三平台 pass。

## 关键文件

新建：
- `crates/modbussim-core/src/jitter.rs`

修改：
- `crates/modbussim-core/src/lib.rs` — `pub mod jitter;`
- `crates/modbussim-core/src/slave.rs`（或 device.rs）— `Device` 加 `jitter` 字段
- `crates/modbussim-core/src/data_source.rs` — 补 `default_for_kind`（若缺）
- `crates/modbussim-egui/src/app.rs` — 抖动卡片 UI + task 调度 + DataSource ComboBox 扩展

## 依赖

- `rand = "0.8"`（测试需要可 seed 的 `StdRng`；不用 `fastrand`）。若 workspace 未声明则加到 `[workspace.dependencies]`
- `tokio::sync::watch` / `oneshot`（tokio 已加 feature full）

## 阶段划分

单 plan 实施，无子 sprint。

## 风险

1. **`apply_tick` 性能**：20K 寄存器 × 4 类 × mutation_rate=100 每 100ms 一次 → 每秒 80 万次 rand + map 写入。M1 基线粗估 <5ms，可接受。若测出 >50ms 需改成"先采样索引再写"。
2. **tokio watch 与 egui ctx 重绘**：jitter 写入后需调用 `ctx.request_repaint()` 否则表格不刷。这已经是现有 DataSource 的做法，沿用即可。
3. **u16 wrapping 溢出**：按百分比漂移后 u16 wrap 用户可能不预期。MVP 不 clamp，记入 doc。
