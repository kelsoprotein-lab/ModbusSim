## 上下文

`visual-flat-layered-v2` 已归档，奠定了三级背景分层 + 无 stroke 容器 + 橙 accent 按钮 + ○/● Bool 切换的基础视觉语言。用户实际使用反馈暴露两个具体问题：

1. FC01/FC02 Bool 视图沿用 holding 表的 5 列骨架，右侧 3 列对 bool 无用（渲染为 `—`），空间浪费且信息密度低。
2. `○/●` 圆点+文字仅是 Label + Sense::click，完全不像按钮。次级按钮 `transparent fill + stroke::NONE` 静止态近乎隐形——用户抱怨"所有页面按钮都不好看"。

这次是对 visual-flat-layered-v2 的精修，不是推倒重来。

## 目标 / 非目标

**目标：**
- 让 Bool 视图的空间分配贴合工业软件惯例（地址 / 值 / 名称 / 注释）
- 让 Bool 值列具备"现代 HMI" 的可交互感（iOS toggle 最典型）
- 让次级按钮静止态"看得见",同时不喧宾夺主

**非目标：**
- 16-bit 网格 bit-map 布局（方案 A1，抛弃"每地址一命名"模型，YAGNI）
- 多列并排布局（方案 A2，TableBuilder 改造成本大且注释列无法放下）
- Phosphor icon 嵌入字体（和之前的 `✕` 显示空框同因；本次不做）
- 浅色 Flavor (Latte) 专项调整

## 决策

### D1 · `toggle_switch` 自绘，不用 egui 原生 checkbox

egui 的 `ui.checkbox(&mut bool, "")` 和 `ui.toggle_value` 都是方块勾选样式，和现代 HMI 的椭圆开关大不相同。工业 SCADA 产品（Ignition / WinCC / redisant MSE）基本都自绘椭圆 toggle。

实现轮廓（`modbussim-ui-shared/src/ui.rs`）：

```rust
pub fn toggle_switch(ui: &mut egui::Ui, flavor: Flavor, value: &mut bool) -> egui::Response {
    let desired = egui::vec2(40.0, 18.0);
    let (rect, mut resp) = ui.allocate_exact_size(desired, egui::Sense::click());
    if resp.clicked() {
        *value = !*value;
        resp.mark_changed();
    }
    let track_color = if *value { theme::success(flavor) } else { theme::bg_hover(flavor) };
    let knob_r = if resp.hovered() { 8.0 } else { 7.0 };
    ui.painter().rect_filled(rect, 9.0, track_color);
    let cx = if *value { rect.right() - 9.0 } else { rect.left() + 9.0 };
    let center = egui::pos2(cx, rect.center().y);
    ui.painter().circle_filled(center, knob_r, egui::Color32::from_rgb(235, 235, 235));
    resp
}
```

**Reject**：`egui_toggle_switch` 等第三方 crate。引入依赖只为一个 40×18 元件，不值。

### D2 · Bool 分支的 row.col 层次

当前 `is_bool` 分支里是：

```rust
row.col(|ui| { /* 布尔自绘 */ });
row.col(|_| {});  // Hex
row.col(|_| {});  // Binary
row.col(|_| {});  // 空
```

改成：

```rust
row.col(|ui| { toggle_switch(ui, flavor, &mut tmp); });
row.col(|ui| { ui.monospace(&reg_def_name); });   // 可能为空串
row.col(|ui| { ui.monospace(&reg_def_comment); }); // 可能为空串
```

只有 3 个 col，因为新布局是 4 列（地址/值/名称/注释）——地址列还在前面。这意味着 TableBuilder 的 `.column()` 调用也得改：bool 分支前面要 if 判断走不同 column 配置。简单做法：在 `TableBuilder` 初始化时按 `is_bool` 分叉两套 column 布局。

### D3 · register_defs 数据流

`RegViewCache` 当前只有 `u16_map` 和 `bool_map`,**没有** `register_defs` 信息。要让名称/注释列可读，需要把 defs 也缓存到 cache。

方案 A: 在 `refresh_reg_view` 里把 `register_defs` 里的 name/comment 按 addr 拍成 `HashMap<u16, (String, String)>` 存 cache。
方案 B: 在渲染时临时 blocking_read `self.connections` 拉 defs——同帧多次读锁,性能差。

选 A。`RegViewCache` 新字段 `defs: Arc<HashMap<u16, (String, String)>>`——Arc 复用 refresh 时的一次 clone,和 `u16_map`/`bool_map` 同模式。

### D4 · secondary_button fill

当前代码：

```rust
let btn = egui::Button::new(...)
    .fill(Color32::TRANSPARENT)
    .stroke(egui::Stroke::NONE)
    .rounding(2.0);
```

改成：

```rust
let btn = egui::Button::new(...)
    .fill(theme::bg_of(flavor, Layer::L2))  // #313338 深色
    .stroke(egui::Stroke::NONE)
    .rounding(2.0);
```

**为什么用 L2 而不是自定义色**:
- L2 已是"数据容器"层（TextEdit、表格），复用语义"值区"
- 深色 #313338 vs 主区 L1 #2b2d30 只差 RGB 6 单位——肉眼能辨"这是按钮"但不喧宾夺主
- 浅色下 L2 = #ffffff vs L1 = #f5f5f5 也有对比

**Reject**: 给 secondary 加 1 px stroke。我们整个风格主张"无框",stroke 会破坏一致性。fill 差异是更优解。

### D5 · primary / danger 不动

primary 橙 `#cc7832` fill + white text，danger 红 `#bc3f3c` fill + white text 本身就有强 fill，affordance 充足。不改动。

### D6 · 兼容性

- spec 层：`egui-visual-style` 里"Bool 圆点+文字"要求被 MODIFIED 为"iOS toggle"；"按钮样式规范"的 secondary 描述被更新；新增"Bool 表格列布局"需求
- 代码层：`○/●` 自绘代码被整段删除；`toggle_switch` 是新函数
- 无 breaking API（`secondary_button` 签名不变，只是内部 fill 变了）

## 风险 / 权衡

| 风险 | 影响 | 缓解 |
|---|---|---|
| toggle_switch hover 放大每帧触发 repaint | 20K 行同时 hover 时帧率降 | hover 只影响当前行；egui body.rows 懒渲染只画可视行 |
| register_defs 在 RegViewCache 里用 Arc<HashMap> clone | refresh 时内存开销 | 20K addr × 2 个 String (多数为空) ≈ 几 MB；可接受 |
| `secondary_button` 换 L2 fill 在表格内会和 L2 表格 bg 合并（"按钮融进表格"） | 日志面板等表格里按钮不可见 | 对于表格内按钮必要时局部强制 primary；日志"清空/导出/关闭"在 L0 日志面板 bg 上，L2 差值 18 单位够看 |
| 浅色 Flavor secondary fill = L2 = 纯白 = 和 CentralPanel 浅灰 #f5f5f5 对比差 | 浅色下按钮不明显 | 非目标；后续浅色专项变更时再调 |

## 迁移计划

纯 UI 改动，无数据迁移。回退 = `git revert`。

实施顺序（对应 tasks.md 3 个阶段）：

1. **T1** `ui.rs` 加 `toggle_switch` + 改 `secondary_button` fill
2. **T2** `RegViewCache` 加 `defs` 字段，`refresh_reg_view` 填充
3. **T3** `Selection::RegisterGroup` 的 `is_bool` 分支：按 is_bool 分叉 TableBuilder column 配置 + body 渲染 toggle_switch / 名称 / 注释

冒烟：FC01 点 addr=10 翻转应看到轨道变绿滑块右移；hover 滑块放大；FC03 表不受影响；"清空" 按钮静止态肉眼能看见。

## 未决问题

1. 名称/注释列空字符串时是否保留列头文字"名称"/"注释"? 倾向保留（用户未来知道这里可填）。
2. 是否需要给 toggle_switch 加 press 反馈（按下时滑块变小）? 倾向不做（hover 放大已足）。
3. secondary_button 在浅色主题下 fill 的具体值？暂不处理，留作浅色变更。
