## 为什么

经过四轮 `visual-flat-layered-v2` → `bool-view-and-button-polish` 的自研视觉迭代，用户连续反馈"按钮还是丑、所有页面按键都不好看"——说明问题不是某个细节调整能解决的，而是 **aesthetic ceiling**：自研实在难以做出主流成熟 design system 的质感。

市场调研发现 `egui-shadcn`（对 shadcn/ui 的 egui 实现）完整提供了经过业界验证的按钮体系（default / outline / ghost / destructive 变体、sm / md / lg 尺寸、focus ring、hover transition），但该 crate 要求 egui 0.33，我们当前锁在 0.29。

同时生态审计发现两件"意外礼物"：
1. `egui-modal` 和 `egui-toast` 在 workspace Cargo.toml 里声明但 **代码中完全没用** —— 是两个零成本的清除项
2. `catppuccin-egui` 5.7 已支持 egui33 feature，`egui-phosphor` 在 0.11 有 0.33 兼容版

一次升级打掉"视觉天花板 + 死依赖清理 + 生态版本落后"三个问题，并为以后接 egui 生态（如 `egui_dock` / `egui-material3` 等主流 0.33+ crate）铺路。

## 变更内容

- **升级 egui 生态** 到 0.33.3：`egui` / `eframe` / `egui_extras` 三件；`catppuccin-egui` 5.0(egui29) → 5.7(egui33 feature)；`egui-phosphor` 0.7 → 0.11（0.12 已绑 0.34 跳过）
- **删除死依赖** `egui-modal` + `egui-toast`（workspace + 三个 crate 的 Cargo.toml），代码无实际引用
- **新增 `egui-shadcn` 0.3+** 到 workspace
- **按钮体系切换**：`modbussim-ui-shared::ui` 的 `primary_button` / `secondary_button` / `danger_button` / `icon_button` 内部改用 egui-shadcn 的 `button` 变体（primary=default / secondary=outline / danger=destructive / icon=ghost+icon），保持对外 API 签名不变（上层调用点零改动）
- **Toggle 开关切换**：`toggle_switch` helper 从当前手绘椭圆改为 `egui-shadcn` 的 `Switch`，尺寸 / 动画 / focus ring 由 shadcn 的 Radix 实现接管
- **API 迁移**：egui 0.29 → 0.33 的 breaking changes（`screen_rect` → `viewport_rect`/`content_rect` 无影响；font backend 换 `skrifa`，`install_cjk_fonts` 需微调；`TextEdit::load_state` / `CCursorRange` 签名 T6 已用处需验证）
- **不做**：升级到 egui 0.34（有 phosphor 0.12 可用但 egui-shadcn 还不支持）；集成 egui-material3（目前 shadcn 已够）；换掉 `egui_extras::TableBuilder`（沿用）

## 功能 (Capabilities)

### 新增功能
- `shadcn-components`: 定义通过 egui-shadcn 提供的 UI 控件（Button 变体 / Switch / 未来 Popover / Select），以及对 modbussim-ui-shared 的 wrapper 规范

### 修改功能
- `egui-visual-style`: 按钮样式规范里的 "fill / hover / stroke" 语义被 shadcn 的 Radix variants 接管；`toggle_switch` 实现改为 shadcn Switch

## 影响

**受影响的 Cargo.toml**：workspace 根 + `modbussim-ui-shared` + `modbussim-egui` + `modbusmaster-egui`。

**受影响的 crate**：
- `modbussim-ui-shared` — `theme.rs` 微调 egui API 迁移、`ui.rs` 按钮 / toggle 改用 shadcn、`fonts.rs` 适配 skrifa backend
- `modbussim-egui` + `modbusmaster-egui` — 因 ui.rs API 签名不变无需修改；但 egui API 微调（如 `TextEdit` state 访问）可能触达 2-3 处

**受影响的 UX**：
- 视觉一次性"升级感"：按钮圆角 / focus ring / hover transition 由 shadcn 接管，立刻比自研精致
- `toggle_switch` 的 40×18 尺寸 + hover 放大行为保留（shadcn Switch 可配置），对外行为不变

**受影响的发布**：CI 矩阵需重新验证 3 平台 release build。

**风险**：
- **egui 0.33 ecosystem 未齐**：phosphor 0.11 未经独立验证，shadcn 可能 API 不稳。若 phosphor 0.11 实际不支持 0.33，降级 0.10 或自己 fork
- **shadcn 维护风险**：egui-shadcn v0.3.1 最新，维护者 FerrisMind 活跃度中等。未来如 stall 需要 fork
- **工作量**：预估 3-5 天编码 + 1 天 CI 调整 + 1 天 smoke。是本项目 egui 分支最大的一次基础设施变更

**回退路径**：保留 refactor/egui-skeleton 当前 HEAD 作 tag，升级失败可 revert。
