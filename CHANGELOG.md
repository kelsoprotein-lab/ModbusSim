# Changelog

All notable changes to ModbusSim are documented in this file.

本文档记录 ModbusSim 所有显著变更,中英对照。

格式遵循 [Keep a Changelog](https://keepachangelog.com/),版本号遵循 [Semantic Versioning](https://semver.org/)。

---

## [0.15.0] - 2026-05-02

Minor 版本:前端大型重构 + 后端推送式事件架构。两端统一抽出共享 `LogPanelShell` / `useFcLabel` / `formatAddress`;`useDialog` 去掉 provide/inject 中转层并接入 i18n;Slave / Master Toolbar 各拆 3 个 modal 子组件;Slave RegisterTable 拆出 `useRegisterValues` + `useRegisterFormat` composables。后端新增 `RegisterChangeCallback` 与 `LogAppendCallback`,核心写路径成功后 emit `register-value-changed` / `log-appended`,前端 `setInterval` 2s 轮询全量替换为 `listen()`。`LogCollector` 内部从 `Vec` 切到 `VecDeque`,日志满 buffer 时 `pop_front()` 取代 O(N) 的 `remove(0)`。无破坏性变更(仍是单工程 git tag 发版,Cargo.toml/tauri.conf.json 不动)。

Minor release: large frontend refactor plus event-driven push architecture on the backend. Both apps now share `LogPanelShell` / `useFcLabel` / `formatAddress`; `useDialog` drops its provide/inject middleman and gains i18n titles; the Slave and Master Toolbars each split into three modal subcomponents; the Slave RegisterTable factors out `useRegisterValues` + `useRegisterFormat` composables. The backend introduces `RegisterChangeCallback` / `LogAppendCallback`; successful core writes emit `register-value-changed` / `log-appended`, and the frontend replaces every 2-second `setInterval` polling loop with `listen()`. `LogCollector` switches its internal storage from `Vec` to `VecDeque` so the ring-buffer eviction is O(1) instead of O(N). No breaking changes (release versioning still tag-only; Cargo.toml/tauri.conf.json unchanged).

### Highlights / 亮点

- ⚡ **2s 轮询 → 事件推送** — 子站寄存器值与通信日志改为后端 emit Tauri 事件,前端 `listen()` 接收;一次 `WriteMultipleRegisters(values=[100])` 从 200 个独立事件压缩为 1 个 batched event,UI 响应即时。/ 2-second polling replaced by Tauri event push for both register values and communication logs; an FC16 write of 100 registers now sends 1 batched event instead of 200 individual ones.
- 🧹 **共享层一次到位** — 新增 `shared-frontend/components/LogPanelShell.vue` + `composables/useFcLabel`、`useAddressFormat`,主 / 从 `LogPanel.vue` 各从 ~240 行收敛到 ~45 行。/ New `LogPanelShell.vue` + `useFcLabel` / `useAddressFormat` cut both `LogPanel.vue` files from ~240 lines to ~45.
- 🎛️ **Toolbar / RegisterTable 大瘦身** — Slave Toolbar 844→194 行 + 3 个独立 dialog;Master Toolbar 876→197 行 + 3 个独立 dialog;Slave RegisterTable 1050→874 行 + 2 个 composable。/ Slave Toolbar 844→194 lines + 3 dialog components; Master Toolbar 876→197 lines + 3 dialogs; Slave RegisterTable 1050→874 lines + 2 composables.
- 🗨️ **Dialog 接入 i18n + 单例兜底** — `useDialog` 标题走 `t('dialog.alertTitle/...')`,旧未关 Promise 在新 dialog 打开时被 cancel,11 处 `inject(dialogKey)` 样板全部移除直接 `import { showAlert }`。/ `useDialog` titles now use i18n; any unresolved previous Promise is cancelled when a new dialog opens; 11 `inject(dialogKey)` boilerplate sites replaced with direct imports.
- 🪣 **LogCollector 改 `VecDeque`** — 满 buffer (10000 条) 时 `pop_front()` O(1) 取代原 `Vec::remove(0)` O(N);未安装 callback 时不再 clone entry。/ `LogCollector` now uses `VecDeque`: full-buffer eviction is O(1), and unobserved appends skip the clone.

### Added 新增

- 后端推送事件:`crates/modbussim-core::slave::RegisterChangeCallback` (`Arc<dyn Fn(&[RegisterChange])>`) + `SlaveConnection::set_change_callback`;TCP / RTU / ASCII / RTU-over-TCP / TLS 五条写入路径成功后通过 callback 发出。`crates/modbussim-core::log_collector::LogAppendCallback` 在 `add` / `add_blocking` / `try_add` 三条路径触发。/ Backend push events: a `RegisterChangeCallback` taking `&[RegisterChange]` is invoked from all five slave transport write paths after a successful write; `LogAppendCallback` fires from all three `add*` paths.
- Tauri commands 新增 emit:slave `start_slave_connection` 注入两个 callback,分别 emit `register-value-changed` (batched) 与 `log-appended`;master `connect_master` 注入 log-append callback emit `log-appended`;`crates/modbussim-app::commands::RegisterChangePayload` 与 `crates/modbusmaster-app::state::LogAppendedEvent` 新 DTO。/ Tauri commands now wire app-handle clones into both callbacks and emit `register-value-changed` / `log-appended`; new DTOs `RegisterChangePayload`, `LogAppendedEvent` exposed for the frontend.
- Shared-frontend 新增:`components/LogPanelShell.vue`(连接列表 + i18n + filter + listen + 合流)、`composables/useFcLabel.ts`(FC / 寄存器类型 i18n 标签)、`composables/useAddressFormat.ts`(`formatAddress(addr, mode)`),并新增 i18n keys `dialog.alertTitle/confirmTitle/promptTitle`、`formats.*`、`fc.*`。/ Shared-frontend gains `LogPanelShell`, `useFcLabel`, `useAddressFormat` composables, plus new i18n keys for dialog titles, value formats, FC labels.
- Slave 端新组件:`MutationControl.vue` / `NewConnectionDialog.vue` / `NewSlaveDialog.vue` 从 Toolbar 拆出。Composables `useRegisterFormat.ts`(`formatU16` / `formatTypedValue` / `formatFloatPair` / `encodeTypedValue`)与 `useRegisterValues.ts`(load / refresh / `register-value-changed` listen / `loadDirtyKeys` race guard)从 RegisterTable 抽出。/ Slave gets `MutationControl`, `NewConnectionDialog`, `NewSlaveDialog` carved out of Toolbar, plus `useRegisterFormat` and `useRegisterValues` composables out of RegisterTable.
- Master 端新组件:`NewConnectionDialog.vue` / `NewScanGroupDialog.vue` / `WriteDialog.vue` 从 Toolbar 拆出。/ Master gets `NewConnectionDialog`, `NewScanGroupDialog`, `WriteDialog` carved out of Toolbar.
- `crates/modbussim-core::parse::register_type_to_str` 反向函数,与已有 `parse_register_type` 配对。/ `parse::register_type_to_str` companion to `parse_register_type`.

### Changed 改进

- `LogCollector` 内部存储 `Vec<LogEntry>` → `VecDeque<LogEntry>`;`add` / `add_blocking` / `try_add` 重写为先 snapshot callback Arc(单次锁),无 callback 时跳过 clone。/ `LogCollector` storage switched to `VecDeque`; `add*` methods snapshot the callback once and skip cloning the entry when no callback is installed.
- `useRegisterValues::loadRegisters` 增加 `loadSeq` race guard 与 `loadDirtyKeys` 集合 — load 期间到达的 push event 被记录,load 完成时不被快照覆盖。/ `useRegisterValues::loadRegisters` now snapshots `loadSeq` and tracks `loadDirtyKeys`, so push events arriving during a load are not clobbered by the snapshot.
- `RegisterTable::commitEdit` 抽 `applyWrites(register_type, [[addr, value], ...])` helper,3 路 try/catch + cache write + emitSelection 重复结构合并;新增 `isBitType(rt)` 替代多处 `rt === 'coil' || rt === 'discrete_input'`。/ `RegisterTable::commitEdit` extracts `applyWrites` and `isBitType` helpers, collapsing three duplicated try/catch + cache + emit blocks.
- `LogPanelShell::scheduleReload` 合流逻辑由 `pendingReload` + do/while 简化为单一 `reloadInFlight` guard(每次 fetch 已为全量,二次 fetch 无意义)。/ `LogPanelShell::scheduleReload` collapses the pending+do/while coalescing pattern into a single `reloadInFlight` guard.
- 所有 11 处 `inject<{ showAlert: ... }>(dialogKey)!` 样板替换为 `import { showAlert } from 'shared-frontend'`;`App.vue` 中的 `provide(dialogKey, ...)` 一并删除。/ All 11 `inject(dialogKey)` boilerplate sites replaced with direct imports of `showAlert`/`showConfirm`/`showPrompt`; the `provide(dialogKey, ...)` calls in both `App.vue`s removed.
- `useDialog::open` 对未 resolve 的旧 Promise 调 cancel 路径,避免悬挂;`title` 改走 i18n,`AppDialog` 按钮文案接 `t('common.cancel')` / `t('common.ok')`。/ `useDialog::open` cancels any unresolved previous Promise before opening a new dialog; titles now go through i18n, and `AppDialog`'s buttons localise via `t('common.cancel')` / `t('common.ok')`.
- `useLogPanel` 解耦 Tauri 命令名:接受 `LogPanelDataSource = { fetchLogs, clearLogs, exportCsv }` 注入,不再硬编码 `get_communication_logs` / `clear_communication_logs` / `export_logs_csv`。/ `useLogPanel` now takes a `LogPanelDataSource` injection instead of hardcoding Tauri command names.
- `RegisterTable` `formatRegType` 与 `formatOptions` 字符串硬编码改走 i18n(`fc.*`、`formats.*`)。/ `RegisterTable`'s register-type and value-format dropdown labels routed through i18n.

### Removed 移除

- 删除 2-second `setInterval` 轮询:`frontend/src/components/RegisterTable.vue` 与 `LogPanel.vue`、`master-frontend/src/components/LogPanel.vue` 三处。改为 listen 事件 + 必要时 `refreshKey` 触发 bulk refresh。/ Removed three `setInterval(..., 2000)` polling loops; replaced by event listeners + on-demand `refreshKey`-driven bulk refresh.
- 删除 `frontend/src/composables/useDialog.ts` 与 `master-frontend/src/composables/useDialog.ts` 两个无意义 re-export 转发壳;`shared-frontend::useDialog` 中未使用的 `dialogKey` 导出删除。/ Removed both `useDialog.ts` re-export shells and the unused `dialogKey` export.
- 删除未引用资源:`frontend/src/components/HelloWorld.vue`、`ToolsView.vue`,以及孤立的 `frontend/src/assets/{hero.png, vite.svg, vue.svg}`。/ Deleted unused `HelloWorld.vue`, `ToolsView.vue`, and orphaned hero/Vite/Vue logo assets.

### Fixed 修复

- master `LogPanel.vue` 自动刷新里硬编码的 `'zh-CN'` locale 改走 `useI18n().locale`;之前写到 `error` ref 后从不显示的问题在 `LogPanelShell` 中以右上角 `!` 角标 + tooltip 修复。/ Hardcoded `'zh-CN'` locale in master `LogPanel.vue` replaced with `useI18n().locale`; the previously dropped `error` ref now surfaces as a `!` badge with tooltip in `LogPanelShell`.
- master `NewConnectionDialog.vue` 删除从未触发的 `(e: 'request-scan'): void` 与多余的 `connectionId?: string` emit 类型声明。/ Removed never-emitted `request-scan` and unused `connectionId` parameter from master `NewConnectionDialog.vue`'s emit declarations.
- Slave Toolbar `random_mutate_registers` 残留 `console.debug` 日志移除。/ Removed leftover `console.debug` from slave Toolbar mutation handler.

### Internal 内部

- 重构覆盖 47 个文件、净减约 2000 行;两端 `vue-tsc` 类型检查 + `vite build` 全绿;`shared-frontend` `vitest` 16/16 通过;`cargo test --workspace` 276/276 通过。/ Refactor touches 47 files with a net ~2000-line reduction; both frontends pass `vue-tsc` + `vite build`; `shared-frontend` vitest 16/16, `cargo test --workspace` 276/276.
- `changes_from_tokio_request` / `changes_from_modbus_request` 多写入变体预分配 `Vec::with_capacity(2 * values.len())`,避免重复 reallocation。/ Multi-write variants of the change-extraction helpers now pre-size the result vector.
- `start_slave_connection` 中 callback 捕获改用 `Arc<str>` 而非反复 `String::clone()`,降低每事件分配。/ Callbacks in `start_slave_connection` now capture connection ids as `Arc<str>` instead of cloning a `String` per event.

---

## [0.14.1] - 2026-05-01

补丁版本:把 v0.14.0 hotfix 引入的 `SlaveDevice::apply_random_mutation_thread` 在 Tauri 子站 `random_mutate_registers` 命令中真正用上,删除 75 行重复实现;新增 6 个单元测试钉住四类寄存器变异行为。无破坏性变更。

Patch release: Tauri slave's `random_mutate_registers` command now actually calls the core `SlaveDevice::apply_random_mutation_thread` API introduced as a v0.14.0 hotfix, removing 75 lines of duplicated logic; six new unit tests pin the mutation behaviour for all four register types. No breaking changes.

### Highlights / 亮点

- ♻️ **Tauri slave 复用 core 变异 API** — `commands.rs::random_mutate_registers` 由 75 行就地实现改为单行 `device.apply_random_mutation_thread(&types)`,行为与 egui 子站完全一致。/ Tauri slave's mutation command shrinks from 75 lines to a single core call, matching egui slave behaviour exactly.
- 🧪 **变异行为单元测试覆盖** — 新增 `tests/random_mutation.rs`:6 个用例钉住 Coil / DiscreteInput / HoldingRegister / InputRegister 都会真正变化,empty defs 返回 0 不 panic;workspace 测试达 280 / 0 失败。/ New `tests/random_mutation.rs` with 6 cases proving all four register types actually mutate (the original "FC03/FC04 不变化" report) and empty defs return 0 without panicking; workspace test count reaches 280, 0 failures.
- 🩺 **后端诊断日志** — `random_mutate` 命令现在打印每类型 addr 计数 + 实际变异数,前端 invoke 也打 `console.debug`,排查"变异请求来了但 UI 没刷"类问题不再靠猜。/ Both backend (`log::debug!`) and frontend (`console.debug`) now record per-type address counts and actual mutation counts, removing the guesswork when diagnosing silent mutation requests.
- 📜 **项目级 CLAUDE.md 入库** — `.claude/CLAUDE.md` 加入仓库:Think Before Coding / Simplicity First / Surgical Changes / Goal-Driven Execution 四条 LLM 协作准则,所有协作者共享同一基线。/ Project-level `.claude/CLAUDE.md` is now checked in, sharing the four LLM-collaboration guidelines with every contributor.

### Changed 改进

- `crates/modbussim-app/src/commands.rs::random_mutate_registers` 删除 ~75 行就地变异逻辑(coils / discrete inputs / holding / input 各自手写 RNG + delta clamp),改调 core `apply_random_mutation_thread`,行为可被 `random_mutation.rs` 测试覆盖。/ `commands.rs::random_mutate_registers` drops ~75 lines of in-place mutation logic in favour of the core helper; behaviour is now testable via `random_mutation.rs`.
- `frontend/src/components/Toolbar.vue::scheduleMutation` invoke 增加 `<number>` 类型标注 + `console.debug` 日志(types + 实际变异数)。/ `scheduleMutation` now types the invoke as `<number>` and logs the mutation count to dev console.

### Tests 测试

- 新增 `crates/modbussim-core/tests/random_mutation.rs`(6 cases):`coil_actually_flips` / `discrete_input_actually_flips` / `holding_register_actually_changes_after_iterations` / `input_register_actually_changes_after_iterations` / `mixed_types_all_change` / `empty_defs_returns_zero_no_panic`。/ Six unit tests in the new `random_mutation.rs` file cover every register type plus the empty-defs no-op edge case.
- 全量 `cargo test --workspace`:**280 通过 / 0 失败**(此前 274 + 本版本 6)。/ Workspace test count is now **280 passing, 0 failing** (previous 274 + 6 new).

### Internal 内部

- `.claude/CLAUDE.md` 入库为项目级 LLM 行为指引(`settings.local.json` / `commands/` / `skills/` 仍为个人本地配置,继续不入库)。/ `.claude/CLAUDE.md` is now version-controlled while local skill / command / settings files remain untracked.

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

[0.14.1]: https://github.com/kelsoprotein-lab/ModbusSim/releases/tag/v0.14.1
[0.14.0]: https://github.com/kelsoprotein-lab/ModbusSim/releases/tag/v0.14.0
[0.13.0]: https://github.com/kelsoprotein-lab/ModbusSim/releases/tag/v0.13.0
