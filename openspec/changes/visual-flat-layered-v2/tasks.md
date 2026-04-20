## 1. 视觉基建（theme + ui 新原语）

- [x] 1.1 在 `modbussim-ui-shared/src/theme.rs` 新增 `enum Layer { L0, L1, L2 }` 并实现 `bg_of(flavor: Flavor, layer: Layer) -> Color32`（深色三级色 `#1e1f22 / #2b2d30 / #313338`；浅色可暂回退为原 `#ffffff / #f5f5f5 / #f0f0f0`）
- [x] 1.2 在 `modbussim-ui-shared/src/theme.rs` 新增 pub fn `bg_hover(flavor)` / `bg_selected_row(flavor)` 常量（`#3c3f45` / `#214283` 30% alpha）
- [x] 1.3 `theme::apply()` 里的 `visuals.panel_fill` 改为 `bg_of(flavor, L1)`；`extreme_bg_color` 改为 `bg_of(flavor, L2)`；确保 `selection.bg_fill` 使用 `bg_selected_row`
- [x] 1.4 在 `modbussim-ui-shared/src/ui.rs` 新增 `pub fn region<R>(ui, flavor, layer, margin, add) -> R`（Frame::none().fill().inner_margin().show()，**不**设 stroke）
- [x] 1.5 在 `modbussim-ui-shared/src/ui.rs` 重写 `primary_button / secondary_button / danger_button`：去掉 default `stroke`，改 hover 用 `bg_hover`，primary fill 使用 `theme::accent(flavor)`（橙色）
- [x] 1.6 新增 `pub fn icon_button(ui, flavor, icon: &str) -> Response` — 24×24 透明默认、hover `bg_hover` 填
- [x] 1.7 `cargo test -p modbussim-ui-shared` 全绿
- [x] 1.8 commit：`feat(ui-shared): 三级背景色板 + region helper + 按钮去 stroke`

## 2. 日志面板（log_panel）改造

- [x] 2.1 `modbussim-ui-shared/src/log_panel.rs` 外层 `uikit::card` 调用替换为 `ui::region(ui, flavor, Layer::L0, Margin::symmetric(14, 10), ...)`
- [x] 2.2 日志表格列标题行 bg 用 `bg_of(flavor, L1)`，body 用 `bg_of(flavor, L2)` — 靠 Frame 包裹
- [x] 2.3 删除日志面板内部任何 `ui.separator()` 调用（bg 色差已经分区）
- [x] 2.4 "清空 / 导出 CSV / 关闭" 三个按钮改用 `secondary_button`，配色跟新规范
- [x] 2.5 `cargo build -p modbussim-egui -p modbusmaster-egui` 通过
- [x] 2.6 commit：`style(ui-shared): log_panel 去 card，三级背景分层`

## 3. Master 三个 Tab 改造

- [x] 3.1 `modbusmaster-egui/src/app.rs` 顶部连接栏去 `uikit::card`，改 `region(L0)`
- [x] 3.2 Read Tab 内容 `uikit::card` → `region(L1)`；按钮（"读取"等）改 primary/secondary
- [x] 3.3 Write Tab 同样改造
- [x] 3.4 Poll Tab 的 ScanGroup 左侧列表用 `region(L1)`；选中组详情右侧用 `region(L2)`；ScanGroup 列表项 selected 态用 `bg_selected_row` 整行填色
- [x] 3.5 结果表 wrapper 改 `region(L2)`；删除结果表附近的 `ui.separator()`
- [x] 3.6 `cargo build -p modbusmaster-egui` 通过；启动目视 3 Tab 视觉一致
- [x] 3.7 commit：`style(master): Tab 内容去 card，按钮换橙色 accent`

## 4. Slave 非 RegisterGroup 三个分支改造（None / Connection / Device）

- [x] 4.1 `modbussim-egui/src/app.rs` `Selection::None` 分支的 `uikit::card` / `accent_card` 全部替换为 `region`；heading 左侧 padding 统一 16 px
- [x] 4.2 `Selection::Connection` 分支同样改造；"停止 / 删除"按钮改 secondary / danger
- [x] 4.3 `Selection::Device` 分支改造：去 `accent_card` 顶色条；"批量添加 / 删除从站"改 primary / danger；数据源列表 `ds_list` 外层改 `region(L2)`；寄存器抖动卡片（Task 5 已加）外层改 `region(L2)`
- [x] 4.4 删除 Slave 左侧 SidePanel 自带的 1 px 粗竖线（改 `SidePanel::left("connections").resizable(true).show_separator_line(false)`）
- [x] 4.5 SidePanel 内部 bg 改 `bg_of(flavor, L0)`（手动 `Frame::none().fill(L0)` 覆盖）
- [x] 4.6 `cargo build -p modbussim-egui` 通过；启动目视三分支一致
- [x] 4.7 commit：`style(slave): None / Connection / Device 视图去 card，SidePanel 去粗竖线`

## 5. Slave RegisterGroup 视图改造（视觉 + Bool 圆点）

- [ ] 5.1 `Selection::RegisterGroup` 的 `accent_card` / `uikit::card` 两层嵌套改 `region(L1)` 外层 + `region(L2)` 表格区
- [ ] 5.2 heading 行调整：左 icon + 名称 + 地址范围 + 右端留位给搜索框（本 task 先不加搜索框，Task 6 加）+ "批量添加" primary_button
- [ ] 5.3 ValuePanel 侧栏改 `region(L1)` 外层，内部 Grid 不动
- [ ] 5.4 Bool 列（FC01 / FC02）自绘：`ui.allocate_exact_size(vec2(full_col_width, row_h), Sense::click())`；`painter.circle_filled(center, 4.0, success_or_subtext)` + `painter.text(..., "ON"|"OFF", font, color)`；命中 `resp.clicked()` 翻转并写回
- [ ] 5.5 ValuePanel 内部背景用 `bg_of(flavor, L1)`（与外层略同色，用内 padding 区分即可）；删除内部 `ui.separator()`
- [ ] 5.6 启动目视 FC01 / FC02 圆点切换工作；FC03 / FC04 表格层级正确
- [ ] 5.7 commit：`style(slave): RegisterGroup 去 card + Bool 列自绘圆点 ON/OFF`

## 6. Slave RegisterGroup 搜索功能

- [ ] 6.1 `SlaveApp` 新增 `search_buf: HashMap<(String, u8, RegisterType), String>` 字段，`Default::default()` 初始化
- [ ] 6.2 `SlaveApp` 新增 `highlight: Option<(String, u8, RegisterType, u16, Instant)>` 字段（tracking 跳转目标 + 起点时刻）
- [ ] 6.3 `SlaveApp` 新增 `want_focus_search: bool`
- [ ] 6.4 在 `update()` 顶部绑定 Cmd+F / Ctrl+F：`ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::F)))` → 若 selection 是 RegisterGroup 则置 `want_focus_search = true`
- [ ] 6.5 `Selection::RegisterGroup` 分支 heading 行右端前插入 `ui.add(TextEdit::singleline(&mut *buf).hint_text("地址 / 名称…").desired_width(160.0))`；`want_focus_search` 时调 `resp.request_focus()` 并全选
- [ ] 6.6 搜索逻辑 helper：`fn parse_search(input: &str) -> SearchMode { Jump(u16) | Filter(String) }`（十进制 / 0x 十六进制 / 超范围或含字母则走 Filter）
- [ ] 6.7 `Jump(addr)` 分支：在 TableBuilder body_rows 渲染期间 `if self.highlight needs scroll { ui.scroll_to_rect(row_rect, Some(Align::Center)); }`；同时记录 highlight 起点以驱动淡出
- [ ] 6.8 `Filter(pat)` 分支：在渲染 body 前先构造过滤后的 addr 列表；TableBuilder 改用 `body.rows(row_h, filtered.len(), |mut row| { let addr = filtered[row.index()]; ... })`
- [ ] 6.9 高亮淡出：body_rows 回调内，若 row 的 addr == highlight.addr 且 elapsed < 2.0s，`painter.rect_filled(row_rect, 0.0, accent.linear_multiply(0.6 * (1.0 - elapsed/2.0)))`；存在 highlight 时 `ctx.request_repaint()` 驱动动画
- [ ] 6.10 空过滤结果处理：`if filtered.is_empty() { ui.centered(|ui| ui.label("无匹配寄存器")) }`
- [ ] 6.11 启动 Slave，手测：
  - 输入 `1234` → 滚动到 addr=1234 + 高亮 2s 淡出
  - 输入 `0xFF` → 滚动到 addr=255
  - 输入 `Temp` → 过滤 name/comment 含 temp 的行
  - 输入 `XYZ` → 显示 "无匹配寄存器"
  - Cmd+F → TextEdit 获得焦点且内容全选
  - 切到 FC04 → 搜索框清空；切回 FC03 → 文本还在
- [ ] 6.12 commit：`feat(slave): RegisterGroup 加搜索框 + 地址跳转 + Cmd+F 聚焦 + 名称过滤`

## 7. 回归 + CI + push

- [ ] 7.1 `cargo test --workspace --exclude modbussim-app --exclude modbusmaster-app` 全绿
- [ ] 7.2 `cargo build --release -p modbussim-egui -p modbusmaster-egui` 通过
- [ ] 7.3 Slave + Master 双端启动目视：三层背景差肉眼可辨；无多余硬分割线；Bool 圆点正常；搜索功能完整
- [ ] 7.4 `git push origin refactor/egui-skeleton`；CI（`ci-egui.yml`）三平台绿
- [ ] 7.5 `openspec-cn validate visual-flat-layered-v2` 通过
