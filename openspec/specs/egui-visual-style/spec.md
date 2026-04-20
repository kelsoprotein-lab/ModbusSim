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
- `secondary_button(ui, flavor, text)` — transparent fill，hover `bg_hover`，text 跟随主题文本色
- `danger_button(ui, flavor, text)` — fill `danger`，white text
- `icon_button(ui, flavor, icon)` — 24×24 透明默认，hover `bg_hover`

所有按钮 rounding 必须 ≤ 3 px；padding 垂直方向 3 px、水平方向 10 px。

#### 场景:Slave 批量添加按钮视觉

- **当** 用户在 Slave `Selection::RegisterGroup` 看到"批量添加"按钮
- **那么** 该按钮必须使用 `primary_button` 渲染
- **那么** 背景必须为 `accent` 主橙 `#cc7832`，不得为 `#0e639c` 或任何蓝色

#### 场景:次级按钮无默认边框

- **当** 用户看到 Slave 中任意次级按钮（"停止"、"删除"等）
- **那么** 按钮在 Normal 状态下不得可见描边
- **那么** 鼠标 hover 时必须显示 `bg_hover` 背景反馈

### 需求:Bool 寄存器切换按钮形变

`Selection::RegisterGroup` 中 FC01 线圈 / FC02 离散输入的 Bool 列当前为 `[0]` / `[1]` 填色按钮，必须改为**圆点 + 文字**的轻量形态：

- `false` 状态：`○ OFF`，圆点色 `#8b8f97`（次文本色），文字 `#8b8f97`
- `true` 状态：`● ON`，圆点色 `success` `#6a8759`，文字 `#d4d7db`
- 整个单元格点击即可切换（hit-box 为整行高度 × 列宽）
- 不得再显示填色方块按钮

#### 场景:FC02 离散输入行切换

- **当** 用户打开 FC02 视图并点击 addr=5 的 Bool 单元格
- **那么** 该单元格视觉从 `○ OFF` 变为 `● ON`
- **那么** 圆点颜色必须为 `success` 绿 `#6a8759`
- **那么** 行其他列内容保持不变

#### 场景:大量 Bool 连续查看

- **当** 用户滚动 FC01 线圈视图查看 20000 行
- **那么** 所有 OFF 行必须统一显示 `○ OFF` 灰字
- **那么** 所有 ON 行必须统一显示 `● ON` 白字 + 绿点
