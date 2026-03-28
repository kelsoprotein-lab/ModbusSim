## 为什么

当前从站模式的交互流程需要用户手动执行多步操作（创建连接 → 添加从站 → 逐个添加寄存器），才能得到一个可用的 Modbus 从站。UI 采用扁平的 Tab+列表布局，信息层级不清晰，与专业 Modbus 工具（如 Redisant MSE）相比差距明显。用户希望创建端口后立即可用，同时获得更专业、信息密度更高的操作界面。

## 变更内容

- **新增**：创建从站连接时自动生成默认从站设备（slave_id=1），并预填 FC1/2/3/4 四类寄存器地址 0~100
- **新增**：左侧树形导航组件，层级为：连接 > 从站 > 寄存器分组（0x Coils / 1x DI / 3x IR / 4x HR）
- **新增**：右侧值解析面板，选中寄存器时显示多格式解读（Signed/Unsigned/Hex/Binary/Float/Double/各字节序）
- **新增**：顶部工具栏，替代原有 Sidebar 导航，提供新建连接、新建从站、启动/停止、工具等操作入口
- **修改**：中间寄存器表格重构，支持双击编辑值、地址十六进制显示、ALIAS/COMMENT 列
- **修改**：通信日志从独立 Tab 改为底部常驻可折叠面板
- **移除**：左侧 Sidebar Tab 导航（Slave/Master/Log/Tools 四个 Tab 页）
- **BREAKING**：整体布局从 Sidebar+Tab 切换式重构为 工具栏+三栏+底部日志 的专业工具布局

## 功能 (Capabilities)

### 新增功能

- `connection-tree`: 树形导航组件 — 以树形结构展示连接、从站、寄存器分组的层级关系，支持右键菜单操作
- `value-panel`: 值解析面板 — 选中寄存器时在右侧显示多种数据格式解读（Signed/Unsigned/Hex/Binary/Long/Float/Double，含多种字节序）
- `toolbar`: 顶部工具栏 — 集中提供新建连接、新建从站、启动/停止连接、工具入口等操作按钮
- `auto-default-slave`: 自动创建默认从站 — 创建连接时自动生成 slave_id=1 的从站设备，预填 FC1/2/3/4 地址 0~100 的寄存器

### 修改功能

- `slave-mode`: 后端 create_slave_connection 命令变更为自动创建默认从站和寄存器，不再返回空连接

## 影响

- **前端**：`App.vue` 布局重写；`Sidebar.vue` 删除；`SlaveView.vue` 拆分为 `RegisterTable.vue` + `ValuePanel.vue`；新增 `ConnectionTree.vue`、`Toolbar.vue`；`LogPanel.vue` 改为底部嵌入式面板
- **后端**：`commands.rs` 中 `create_slave_connection` 增加自动创建从站和寄存器逻辑；`register.rs` 中 `RegisterMap` 需支持批量初始化
- **API**：`create_slave_connection` 返回值增加默认从站信息
- **依赖**：无新增外部依赖
