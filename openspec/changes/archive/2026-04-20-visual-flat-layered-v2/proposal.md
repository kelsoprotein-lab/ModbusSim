## 为什么

当前 egui 双端（`modbussim-egui` + `modbusmaster-egui`）视觉上"硬边界"感严重：每个功能区都包 `uikit::card` / `accent_card`，表格、值解析、日志面板全部各自 1 px stroke，叠加 `SidePanel` 默认边界线、`ui.separator()` 横线，呈现给用户的是一堆"强行划线分割"的盒子。同时深色主题下所有区域共用同一块 `#3c3f41` 填色，区与区之间只靠线区分，没有"呼吸感"。

另一个阻塞日常使用的问题：**寄存器视图 20000+ 行却无任何检索/跳转功能**。用户要修改地址 12345 只能手动滚动，Tauri 版早已有的 Vue 搜索框（`RegisterTable.vue:544-549`）尚未移植。

现在趁视觉和交互两个痛点一起做，避免后续重复返工。

## 变更内容

- **视觉色板**：引入三级背景分层（chrome `#1e1f22` / 主区 `#2b2d30` / 数据容器 `#313338`），取代当前所有区域共用 `#3c3f41` 的扁平做法。活动 hover `#3c3f45`，选中行 `#214283`（30% alpha）。
- **容器哲学**：放弃"每区一个 stroked card"。改用"背景色差 + inner padding"区分区域，只在**功能性边界**（表格容器、输入框）保留 1 px 细线。新增 `ui.rs::region(ui, layer, margin, add)` helper，取代多数 `uikit::card` 调用位点。
- **按钮规范**：去 stroke、改 `fill + hover bg` 驱动。Primary 主橙 `#cc7832` 填 + white text；Secondary transparent + hover `#3c3f45`；Danger 红填；Icon-only 24×24 + 透明默认 / hover bg。
- **Bool 切换**：`Selection::RegisterGroup` 里 FC01/FC02 行的 0/1 按钮改为 `○ OFF` / `● ON` 的圆点 + 文字形态（ON 绿 `#6a8759`，OFF 灰 `#8b8f97`）。
- **always-visible 寄存器搜索框**：`Selection::RegisterGroup` 的 heading 行右端固定 `TextEdit`。输入纯数字 → 解析地址 → scroll_to_row + 行高亮 2 秒；输入含字母 → 过滤 name/comment 行。Cmd+F / Ctrl+F 聚焦该输入框。
- **覆盖范围**：Slave 全部四个 Selection 分支 + Master 三个 Tab + 共享 `log_panel.rs`，统一一次改完，避免风格割裂。
- **移除**：所有多余的 `ui.separator()` 横线；`accent_card` 的 2 px 顶色条（改用背景 + margin 代替）；`uikit::card` 在大多数调用点被 `region` 取代，但保留 `card` 入口供未来极少数需要 stroked 强调的场景。

## 功能 (Capabilities)

### 新增功能
- `egui-visual-style`: 定义 egui 双端共用的视觉规范 — 色板分层、容器哲学、按钮样式、bool 切换形态
- `egui-register-search`: `Selection::RegisterGroup` 视图内的寄存器检索交互 — always-visible 搜索框、地址跳转、Cmd+F 聚焦、名称/注释模糊过滤

### 修改功能
（无 — 项目此前无已定义 capability，全新建立）

## 影响

**受影响的 crate**：
- `modbussim-ui-shared` — `theme.rs` 色板扩展、`ui.rs` 新 `region` helper + 按钮重写、新文件 `search_box.rs`、`log_panel.rs` 去 card
- `modbussim-egui` — `app.rs` 4 个 Selection 分支重写、Bool 行改自绘圆点、RegisterGroup 加搜索框 + scroll_to_row
- `modbusmaster-egui` — `app.rs` 3 个 Tab 去 card / 按钮改色

**受影响的 UX**：
- 旧工程 `.modbusproj` 文件**无需迁移**（本次只改视觉与 UI 交互，不动序列化）
- Cmd+F 在 macOS 可能被 egui 之外的全局绑定干扰，需在应用内 `consume_key` 前确认
- `egui_extras::TableBuilder` 0.29 不原生支持 sticky header，表头需手动贴底 rect_filled 来模拟（design.md 细述）

**受影响的依赖**：无新 crate。沿用现有 eframe 0.29 / egui_extras / egui-phosphor。

**风险**：布局重构一次涉及 6 处视图，commit 粒度要足够细（每视图一个 commit），否则回退困难。
