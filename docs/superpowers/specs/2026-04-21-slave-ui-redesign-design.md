# 子站 UI 重设计 · 工业 HMI 中文版（Spec）

## Context

当前子站界面（`crates/modbussim-egui`）用户反馈"布局/字体/颜色都奇怪"。经过代码侦察定位根因：

- 字号梯度太平（15/13/12.5 相邻仅 <3px 差），层级不清
- 三层背景 `#1e1f22 / #2b2d30 / #313338` 仅 RGB 差 6 unit，"一片灰"
- 大量二级操作用纯文本链接（停止/删除/清空/关闭/新建 TCP 连接），无视觉重量
- 橙色 accent 仅出现在"批量添加"按钮一处，孤悬突兀
- "值解析"侧栏常驻但长期空白，浪费右侧 ~20% 宽度
- `item_spacing = (8,4)` 与行高 22 不匹配，上下留白不匀

目标：重设计视觉系统与信息架构，使其成为一个专业工业调试工具（SCADA/HMI 风）但保留中文界面友好性。主使用场景锁定为"调试寄存器值"，因此表格是 C 位。

风格方向已与用户确认：**B · 工业 HMI 中文版**（高对比黑底 + 蓝 accent + 绿数值）。值解析位置：**右侧抽屉、默认收起**。

## 现状入口（关键文件）

| 文件 | 行 | 作用 |
|---|---|---|
| `crates/modbussim-egui/src/app.rs` | 2750-2870 | 主布局（TopPanel/SidePanel/CentralPanel/BottomPanel） |
| `crates/modbussim-egui/src/app.rs` | 2315-2330 / 2457-2462 | 寄存器 TableBuilder |
| `crates/modbussim-ui-shared/src/theme.rs` | 56-84 | `Layer::L0/L1/L2` 配色 |
| `crates/modbussim-ui-shared/src/theme.rs` | 181-338 | Visuals 应用、语义颜色、字号 |
| `crates/modbussim-ui-shared/src/ui.rs` | 95-129 | shadcn palette 覆盖 |
| `crates/modbussim-ui-shared/src/log_panel.rs` | 69-141 | 通信日志面板 |
| `crates/modbussim-ui-shared/src/fonts.rs` | 47-91 | CJK 字体加载 |

## 设计决策总览

### 信息架构

```
TopPanel   菜单栏（文件 / 视图 / 帮助）
SidePanel  左侧连接树（240px，原 320）
           - 头部: "连接" tiny_caps + 右上 "+ 新建" 链接
           - 树: 3 级，激活节点左 2px 蓝竖线
           - 底 footer: [停止] [删除连接]
Central    主区
  A. 主区头 (两行: 标题 + 面包屑 | 搜索 + 主操作)
  B. 工具栏 (格式 pill | 已选 N 行 | 次要操作)
  C. 表格 + 值解析抽屉（抽屉默认收起）
  D. 通信日志 (可折叠)
BottomPanel 状态栏（22px）
```

### 配色 Token（深色模式）

```
bg.chrome    #010409   菜单/侧栏/状态栏
bg.surface   #0d1117   主区
bg.raised    #161b22   hover / 抽屉卡片
border.subtle #21262d  分割线
border.strong #30363d  输入框/按钮边框
accent.primary #1f6feb  选中/表头下划线/focus
accent.primary.fg #58a6ff 表头文字/链接
accent.success #3fb950  数值 / RX / 就绪
accent.warn    #f0883e  HEX 列
accent.danger  #f85149  删除 hover / 错误
text.primary  #e6edf3   标题
text.body     #c9d1d9   正文
text.muted    #6e7681   地址/元信息
alias         #d2a8ff   别名列
```

主 accent 从橙 `#cc7832` 换成蓝 `#1f6feb`；绿色承担"数值"语义；橙色退居 HEX 列。浅色模式保留双主题，accent 同步换蓝。

### 字号 Token

```
Heading   15.0   主区标题
Body      12.5   正文
Button    12.0   按钮
Monospace 12.5   表格数值/地址
Small     10.5   表头 / 面包屑 / 状态栏

tiny_caps 10.5  大写、accent_fg 蓝、强调字重 → 表头、分组标题
crumb     11.0  muted 色 → 面包屑
```

### 间距 Token

```
spacing.item_spacing   (10, 6)   原 (8, 4)
spacing.button_padding (12, 4)
spacing.interact_size.y 24       原 22
panel.inner_margin     14 / 12   (L/R / T/B)
表格行高               22
表头行高               26
日志行高               18
```

## 组件级改动

### 左侧连接面板（app.rs SidePanel 块, ~2798）
- 宽 `320 → 240`
- 头部替换为 `连接` tiny_caps 标签 + 右上 `+ 新建` 链接（去掉当前"新建 TCP 连接"整行）
- 树节点 3 级样式统一，激活节点：左 2px 蓝竖线 + `#1f6feb @ 15% alpha` 背景 + 文字 `#58a6ff` 粗体
- 节点右 badge 显示行数（等宽、muted）
- 底部 footer：`[停止] [删除连接]`（删除 hover 变 `#f85149`），与树之间 1px 顶分割线

### 主区头部（新）
- `egui::Frame` 两行结构
- 上行：`Heading` 主标题 + `crumb` 面包屑
- 下行：`TextEdit` 搜索框（宽 200，1px 灰边，focus 蓝） + 绿色实心 `+ 批量添加`

### 工具栏（新，`CentralPanel` 内顶条）
- 格式 pill：`#161b22` 底 + 1px 灰边 + 12px 圆角 + 蓝字
- 选中计数：`已选 N 行` muted
- 右侧：`导出` / `清零` 透明次要按钮

### 寄存器表格（app.rs, ~2315）
- 列宽：地址 80 / 别名 120 / 值 100 右对齐 / HEX 80 / 二进制 remainder
- 表头：`tiny_caps` 样式、字色蓝、下划线 2px 蓝
- 行：
  - 地址：右对齐、muted
  - 别名：紫 `#d2a8ff`；空值显示 `—`
  - 值：绿色粗体、右对齐；编辑态用 `DragValue` 右对齐
  - HEX：橙 `#f0883e`
  - 二进制：muted 小号
- 选中行：`#1f6feb @ 15% alpha`；hover 行：`bg.raised`
- **移除 `striped(true)`**（与选中/hover 背景冲突）

### 值解析抽屉（右侧可关闭，替换当前常驻列）
- 默认不渲染；工具栏右侧加切换按钮 `◧ 值解析` 或快捷键 `V`
- 打开：从右侧渲染 240px `egui::SidePanel::right`，`show_animated`
- 内容：U16 / I16 / HEX / BIN / U32 / F32 纵向网格
- 多行选中 (2–4 行) 时底部追加组合解析（U32/F32/ASCII 串）
- 未选中时 empty state：`选中 1–4 行寄存器以查看`

### 通信日志（log_panel.rs）
- 单行头部：`▼ 通信日志 · slave_1 · N 条 | ☑RX ☑TX [过滤…] [清空] [导出 CSV] [关闭]`
- 点 `▼` 折叠到仅头部
- 列宽：时间 150 / 方向 28 / FC 60 / 详情 remainder
- 方向：`←` 绿 / `→` 蓝；FC 橙；详情 body 色
- 去掉行 striped

### 状态栏（新增 `TopBottomPanel::bottom("statusbar")`）
- 高度 22，字号 11，`bg.chrome` 底
- 左：`● 就绪` (绿) / `N 连接 · M 从站`
- 右：版本号 `env!("CARGO_PKG_VERSION")`

### 顶部菜单栏
- 保留 `文件 / 视图 / 帮助`
- `视图` 菜单新增：显示值解析 (V) / 显示通信日志 / 浅色深色切换

### 字体加载（fonts.rs, ~47）
- 主字体保持系统 CJK 加载顺序
- nice-to-have：Monospace 回退链 `SF Mono / Menlo / Consolas / JetBrains Mono`

### 快捷键（新）
- `V` 切值解析
- `L` 切日志折叠
- `/` 聚焦搜索
- `Esc` 清除选中

## 主要修改文件清单

- `crates/modbussim-ui-shared/src/theme.rs` — 重写 Layer 色值、Visuals、TextStyle、新增 tiny_caps/crumb helper、accent 换蓝
- `crates/modbussim-ui-shared/src/ui.rs` — shadcn palette 同步、card 改 token、新增 panel_header/link_action
- `crates/modbussim-ui-shared/src/log_panel.rs` — 单行 header + 折叠 + RX/TX 箭头
- `crates/modbussim-egui/src/app.rs` — SidePanel/主区/表格/值解析抽屉/状态栏/快捷键
