### 需求:三级背景分层色板

系统必须为 egui 双端（Slave / Master）提供统一的三级深色背景色板，用于区分"外壳/主区/数据容器"三个视觉层级。相邻两层的 RGB 分量差异必须 ≥ 6 个单位以保证肉眼可辨，但不得形成强对比。

主色板必须定义如下常量（在 `modbussim-ui-shared::theme` 模块内）：
- `bg_layer_0` = `#1e1f22` — 外壳层（SidePanel、底部日志面板）
- `bg_layer_1` = `#2b2d30` — 主内容层（CentralPanel）
- `bg_layer_2` = `#313338` — 数据容器层（表格、TextEdit、Slider 轨道）
- `bg_hover` = `#3c3f45` — 交互 hover 填色
- `bg_selected` = `#214283` 半透明 30% — 选中行填色
- `accent` = `#cc7832` — Darcula 橙，保留作为主要强调色
- `success` = `#6a8759` — ON/成功状态
- `danger` = `#bc3f3c` — 错误/断开

浅色 Flavor（`Latte`）不在本次变更范围内；仅要求浅色下功能可用，色板由后续变更处理。

#### 场景:深色模式下三层可辨

- **当** 用户在深色主题下打开 Slave 应用并选中一个 Device 的 RegisterGroup
- **那么** SidePanel 背景必须可辨地比 CentralPanel 更深（`#1e1f22` vs `#2b2d30`）
- **那么** RegisterGroup 内的表格容器必须可辨地比主区更浅（`#313338` vs `#2b2d30`）

#### 场景:色值 API 稳定

- **当** 上层 UI 代码调用 `theme::bg_layer_1(flavor)`
- **那么** 函数必须返回与规范一致的 `Color32::from_rgb(43, 45, 48)`（对应 `#2b2d30`）
- **那么** 调用点不得硬编码任何 RGB 数值

### 需求:零框容器 (region) 取代大部分 card 使用

系统必须提供 `ui::region(ui, layer, margin, add)` helper（在 `modbussim-ui-shared::ui` 模块），用于在**不画 stroke** 的情况下按层背景色承载功能块。该 helper 内部使用 `egui::Frame::none().fill(bg).inner_margin(margin)`，**禁止**设置 `stroke`。

`uikit::card` 与 `uikit::accent_card` 接口保留（作兼容），但在 Slave / Master / log_panel 的主要渲染路径上必须被 `region` 替代。改造后，**全部**视图中不得有嵌套的 stroke 组合（即不得出现"一个 stroked 容器包另一个 stroked 容器"）。

#### 场景:Slave 主区去 card

- **当** 用户选中 Slave 的 `Selection::RegisterGroup`
- **那么** 渲染路径必须使用 `region(ui, Layer::L1, Margin::symmetric(16.0, 12.0), ...)` 包裹主区内容
- **那么** 主区内的表格区必须使用 `region(ui, Layer::L2, ...)` 包裹
- **那么** 目视上**不得出现** `accent_card` 的 2 px 顶部色条

#### 场景:日志面板去 stroke

- **当** 共享 `log_panel::render` 被任一端调用
- **那么** 其外层容器必须为 `Layer::L0` 背景的 `region`，而不是 `uikit::card`
- **那么** 不得在日志外层画 1 px stroke

### 需求:按钮样式规范

所有按钮必须按"fill + hover bg"模式渲染，**不得**设置默认 stroke（只有禁用态 / 选中态可以临时加 1 px 弱描边）。

四种按钮样式必须在 `modbussim-ui-shared::ui` 模块提供：
- `primary_button(ui, flavor, text)` — fill 主色 `accent`，white text，hover 亮 10%，press 暗 10%
- `secondary_button(ui, flavor, text)` — fill 淡色 `bg_of(flavor, Layer::L2)`（深色 `#313338`），hover `bg_hover`，text 跟随主题文本色。**必须不是 transparent**——透明 fill 会让静止态按钮失去视觉 affordance
- `danger_button(ui, flavor, text)` — fill `danger`，white text
- `icon_button(ui, flavor, icon)` — 24×24 透明默认，hover `bg_hover`

所有按钮 rounding 必须 ≤ 3 px；padding 垂直方向 3 px、水平方向 10 px。

#### 场景:Slave 批量添加按钮视觉

- **当** 用户在 Slave `Selection::RegisterGroup` 看到"批量添加"按钮
- **那么** 该按钮必须使用 `primary_button` 渲染
- **那么** 背景必须为 `accent` 主橙 `#cc7832`，不得为 `#0e639c` 或任何蓝色

#### 场景:次级按钮静止态可见

- **当** 用户看到 Slave 中任意次级按钮（"停止"、"删除"、"清空"、"导出 CSV"、"关闭"、"创建"等）
- **那么** 按钮在 Normal 状态下必须显示淡色 fill `bg_of(flavor, Layer::L2)`，肉眼可辨
- **那么** 按钮不得为完全 transparent
- **那么** 鼠标 hover 时必须显示 `bg_hover` 背景反馈
- **那么** 按钮不得可见描边

### 需求:Bool 寄存器切换按钮形变

`Selection::RegisterGroup` 中 FC01 线圈 / FC02 离散输入的 Bool 列必须使用**iOS 风格 toggle 开关**控件，不再使用之前圆点+文字的轻量标签形态。开关控件的视觉特征：

- 轨道（track）: 40 px 宽 × 18 px 高椭圆，`rounding = 9`
  - `on` 态轨道填色: `theme::success(flavor)` (深色 `#6a8759`)
  - `off` 态轨道填色: `theme::bg_hover(flavor)` (深色 `#3c3f45`)
- 滑块（knob）: 14 px 直径的白色圆（`Color32::from_rgb(235, 235, 235)`），垂直居中
  - `off` 位置: 轨道左端 + 2 px 内边距
  - `on` 位置: 轨道右端 − 2 px 内边距
- **hover 反馈**: 滑块在鼠标悬停时直径放大到 16 px（这是"可交互"的关键视觉 cue）
- **点击命中区域**: 必须为整个 40×18 的轨道矩形，而非仅滑块——20K 行的列表里滑块过小无法精准点击
- 控件必须通过新的 `ui::toggle_switch(ui, flavor, value: &mut bool) -> Response` helper 渲染（位于 `modbussim-ui-shared/src/ui.rs`），调用方从 `response.clicked()` 判断翻转

不再显示 `○/●` 圆点或 `ON/OFF` 文字——开关自身的位置 + 轨道色是足够的状态指示。

#### 场景:FC02 离散输入行切换

- **当** 用户打开 FC02 视图并点击 addr=5 的 Bool 单元格
- **那么** 该单元格视觉从"灰色轨道 + 左端滑块"变为"绿色轨道 + 右端滑块"
- **那么** 绿色必须为 `theme::success(flavor)` (深色下 `#6a8759`)
- **那么** 行其他列（名称、注释）内容保持不变

#### 场景:大量 Bool 行 hover 反馈

- **当** 用户把鼠标悬停在 FC01 视图的某一行 toggle 上
- **那么** 该行 toggle 的滑块直径必须从 14 px 放大到 16 px
- **那么** 其他行的 toggle 必须保持 14 px 滑块
- **那么** 移开鼠标后滑块必须恢复到 14 px

#### 场景:全 40×18 命中区

- **当** 用户点击 toggle 轨道的任意位置（不只是滑块圆点）
- **那么** 开关必须触发翻转
- **那么** `response.clicked()` 必须返回 true

### 需求:Bool 寄存器表格列布局

FC01 线圈 / FC02 离散输入视图的表格必须使用"地址 / 值 / 名称 / 注释"4 列布局，不再沿用 holding/input 寄存器的"地址 / 值 / Hex / Binary / 空"5 列布局。列宽分配：

- 列 1 **地址**: `Column::exact(80.0)`
- 列 2 **值**: `Column::exact(170.0)` — 容纳 40×18 toggle + 居中空间
- 列 3 **名称**: `Column::exact(200.0)` — 从 `register_defs[addr].name` 取值，空串则列空
- 列 4 **注释**: `Column::remainder()` — 从 `register_defs[addr].comment` 取值，空串则列空

具体 header 文案：`地址 / 值 / 名称 / 注释`。**不得**再显示"Hex"、"Binary"或两个 `—` 占位符。

对 FC03 保持寄存器 / FC04 输入寄存器的五列布局（地址 / 值 / Hex / Binary / 空）保持不变，本需求仅影响 bool 分支。

#### 场景:FC01 表格头

- **当** 用户打开 FC01 线圈视图
- **那么** 表格 header 必须显示 `地址`、`值`、`名称`、`注释` 四列
- **那么** 必须不出现 `Hex` 或 `Binary` 列
- **那么** 表格可用宽度必须被四列铺满，无大片空白

#### 场景:空名称注释降级

- **当** register_defs 里 addr=1234 的 `name` 和 `comment` 都是空字符串
- **那么** 该行第 3 / 第 4 列必须渲染为空（不渲染 `—` 或占位符）
- **那么** 第 2 列 toggle 仍必须正常渲染和响应点击
