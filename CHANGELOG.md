# Changelog

All notable changes to ModbusSim are documented in this file.

本文档记录 ModbusSim 所有显著变更,中英对照。

格式遵循 [Keep a Changelog](https://keepachangelog.com/),版本号遵循 [Semantic Versioning](https://semver.org/)。

---

## [0.14.0] - 2026-04-28

自 `v0.13.0` 起的大版本更新:Slave UI 整体冷蓝重构、TLS 支持、egui 双端全面 i18n、空状态 Hero 三色弦动画、端到端联动测试覆盖扩展到 10 场景。无破坏性变更。

A large feature drop since `v0.13.0`: full slave-UI cool-blue redesign, TLS support, end-to-end i18n across both egui apps, an empty-state Hero animation, and master/slave E2E coverage extended to 10 scenarios. No breaking changes.

### Highlights / 亮点

- 🎨 **Slave UI 整体重构** — 冷蓝 palette、shadcn 迁移、SidePanel 三段式 240px、状态栏脉动 ●、寄存器表格色彩语义化、按钮层级平衡。/ Slave UI redesigned end-to-end: cool-blue palette, shadcn migration, three-section 240px sidebar, pulsing status dot, semantic register table colours.
- 🔒 **Slave TLS 支持** — 新建对话框新增 TLS 选项,`Transport::TcpTls` 全链路打通,配置可持久化。/ Slave gains TLS in the new-connection dialog (`Transport::TcpTls`), with persisted config.
- 🌐 **Master + Slave egui 全面接入 i18n** — 菜单、侧栏、状态栏、运行时错误串均通过 `tr/tr1`,中英文即时切换并随 eframe 持久化。/ Both egui apps now run through `tr/tr1` end-to-end (menu, sidebar, status bar, runtime error strings); language switch is live and persisted.
- 🎭 **Slave 空状态 Hero 动画** — 三色弦 `paint_dancing_strings` + 心跳采样 `show_welcome_hero`,首次启动不再是空白页。/ New empty-state Hero: three-string dancing animation with heartbeat sampling — no more blank welcome screen.
- 🧪 **端到端覆盖扩展到 10 场景** — `e2e_flow.rs` 新增 7 个非交互式 `#[tokio::test]`(异常码全谱、连接生命周期、多设备路由、多 ScanGroup 并行、主站扫描器、随机变异传播、并发读写),共 274 测试 / 0 失败。/ `e2e_flow.rs` grows from 3 to 10 non-interactive scenarios covering exception codes, lifecycle, multi-device routing, parallel scan groups, scanners, mutation propagation and concurrent r/w. Workspace now at 274 tests, 0 failures.
- ♻️ **master-egui `update()` 抽方法重构** — 460 行 → 190 行,拆出 menu / sidebar / status / 3-tab 共 6 个 render 方法。/ `master-egui::update()` shrinks from ~460 to ~190 lines via per-section render methods.

### Added 新增

- `master-egui` 新增中英文菜单项 + Lang 持久化 (`lang_v1`),i18n 键扩展 ~70 条覆盖 conn / read / write / poll / result 各模块。/ `master-egui` gains a Language menu and `lang_v1` persistence; ~70 new i18n keys across conn/read/write/poll/result modules.
- `slave-egui` 新增中英文切换,完整菜单 + 侧栏 + 错误串国际化。/ `slave-egui` adds language switching with fully translated menu, sidebar and error toasts.
- `modbussim-ui-shared::hero_anim::show_welcome_hero` — 三色弦欢迎屏 helper,Slave / Master 共享。/ Shared `show_welcome_hero` helper drives the new empty-state animation.
- `modbussim-core::LogCollector::try_count_within` — 非阻塞时间窗计数,用于状态栏脉动 ● 强度采样。/ Non-blocking time-window counter on `LogCollector`, feeding the pulse dot intensity.
- `slave-app` 新建对话框 TLS 选项 + `Transport::TcpTls` 持久化字段。/ TLS option in slave's new-connection dialog plus `Transport::TcpTls` persistence.
- `slave-app::danger_button_sm` helper + `pending_delete` 二次确认字段(footer 删除连接 3 秒确认)。/ `danger_button_sm` helper and a 3-second confirmation pattern for connection deletion.
- `crates/modbussim-core/tests/e2e_flow.rs` 新增 7 个非交互式 E2E 测试,自带带时间戳的 `step!` 日志宏。/ Seven new non-interactive `#[tokio::test]` cases in `e2e_flow.rs`, with a timestamped `step!` logging macro.

### Changed 改进

- `master-egui::update()` 函数体抽 6 个方法:`render_menu_bar` / `render_sidebar` / `render_status_bar` / `render_read_tab` / `render_write_tab` / `render_poll_tab`。/ `master-egui::update()` decomposed into six render methods.
- `master-egui` 模块化拆分:新建 `events.rs` (`UiEvent`)、`scan_group.rs` (`ScanGroupUi`)、`result_table.rs`(读结果表渲染)。/ `master-egui` modularised into sibling files for events, scan groups and result tables.
- 子站运行时错误改走 i18n — 引入 `UiEvent::ErrorKey { key, arg } / InfoKey`,`drain_events` 阶段调 `tr1(self.lang, key, arg)`,避免在 async spawn 里捕获 `lang`。/ Slave runtime errors now flow through `UiEvent::ErrorKey/InfoKey`; `tr1` is invoked at drain time, eliminating `lang` capture in async spawns.
- 整套主题切换为冷蓝 palette,字号梯度拉开,新增语义色 token,`shadcn-egui` 替换原有 button/frame 样式。/ Theme switched to a cool-blue palette with widened font scale, new semantic colour tokens, and `shadcn-egui` replaces the old button/frame styling.
- `slave-app` SidePanel 重构为 240px 三段式(头/树/footer),树节点按钮简化为单按钮 + 状态色 icon,删除连接按钮挪到 footer。/ Slave sidebar rebuilt as a three-section 240px panel; tree nodes simplified to a single coloured-icon button; connection deletion moved to the footer.
- 状态栏从静态文字改为脉动 ● + 三态文案(运行中 / 已停止 / 未连接),色彩与 LogCollector 速率联动。/ Status bar evolves from static text to a pulsing dot with three-state labels, intensity driven by the LogCollector rate.
- 寄存器表格:`fmt-pill`、按类型语义化背景色、表头 `tiny_caps`、关闭 `striped`,值解析可隐藏(V/L/Esc 快捷键)。/ Register table now uses `fmt-pill`, semantic per-type background, `tiny_caps` headers, no zebra-striping, and a hideable value-parse pane (V / L / Esc).
- Log panel 改为单行 header 可折叠,RX/TX 改用箭头符号。/ Log panel header collapsed to a single line; RX/TX rendered with arrow glyphs.

### Fixed 修复

- `master-app` 补齐 `TcpSpec { tls: None }` 字段,修复 CI 上的 `E0063` 编译错误。/ `master-app` now sets `TcpSpec { tls: None }`, fixing the CI `E0063` compile failure.
- `slave-app` data source runner 补齐 `Coil` / `DiscreteInput` 写入分支(原来只覆盖 HR/IR)。/ Slave data-source runner now handles `Coil` / `DiscreteInput` writes (previously HR/IR only).
- Jitter:零值 holding/input 寄存器不再被推动,整数除法时保底 ±1。/ Jitter no longer perturbs zero-valued holding/input registers; integer division has a ±1 floor.

### Tests 测试

- `e2e_flow.rs` 从 3 场景扩展到 10 场景,719 行,覆盖完整 FC01-04 / FC05/06/15/16 + 异常码 + 生命周期 + 多设备 / 多 ScanGroup + 主站扫描器 + 变异传播 + 并发读写。/ `e2e_flow.rs` grows from 3 to 10 scenarios (719 lines) covering all FCs, exception codes, lifecycle, multi-device, parallel scan groups, scanners, mutation propagation and concurrent r/w.
- `slave-app::amp_from_counts` 归一化边界单测。/ Boundary tests for `slave-app::amp_from_counts` normalisation.
- 工作区测试总数 274 / 0 失败。/ Workspace test total: 274 / 0 failures.

### Internal 内部

- `Frame::none` → `Frame::new`(egui 0.33 deprecated 迁移)。/ Migrated `Frame::none` → `Frame::new` (egui 0.33 deprecation).
- 多次 `cargo fmt --all` 整体规整。/ Repeated `cargo fmt --all` housekeeping.
- 新增 `docs/superpowers/specs` 与 `openspec` 流程产物归档。/ Spec/plan artefacts archived under `docs/superpowers/specs` and `openspec`.

---

## [0.13.0] - 2026-04-20

详见 git tag 与提交历史。/ See git tag and commit history.

[0.14.0]: https://github.com/kelsoprotein-lab/ModbusSim/releases/tag/v0.14.0
[0.13.0]: https://github.com/kelsoprotein-lab/ModbusSim/releases/tag/v0.13.0
