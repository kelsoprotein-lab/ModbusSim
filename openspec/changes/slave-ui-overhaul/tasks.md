## 1. 后端：自动创建默认从站

- [x] 1.1 修改 `create_slave_connection` 命令，在创建连接后自动创建 slave_id=1 的 SlaveDevice（名称"从站 1"）
- [x] 1.2 在自动创建的从站中预填 404 个 RegisterDef（FC1/FC2/FC3/FC4 各 101 个，地址 0~100）
- [x] 1.3 初始化 RegisterMap 中 coils/discrete_inputs/holding_registers/input_registers 地址 0~100 的默认值
- [x] 1.4 更新 `create_slave_connection` 返回的 SlaveConnectionInfo，device_count 改为 1
- [x] 1.5 添加后端单元测试：验证创建连接后 list_slave_devices 返回 1 个设备，list_registers 返回 404 个定义

## 2. 前端：布局骨架重构

- [x] 2.1 重写 `App.vue` 布局，从 Sidebar+Tab 改为 CSS Grid 四区域：顶部工具栏、左侧树、中间内容、右侧面板、底部日志
- [x] 2.2 删除 `Sidebar.vue` 组件
- [x] 2.3 创建 `Toolbar.vue` 骨架组件，包含按钮占位（新建连接、新建从站、启动、停止、关闭、工具）
- [x] 2.4 创建 `ConnectionTree.vue` 骨架组件，包含空的树容器
- [x] 2.5 创建 `ValuePanel.vue` 骨架组件，包含空状态提示 "选择一个寄存器查看详情"
- [x] 2.6 改造 `LogPanel.vue` 为底部嵌入式面板，默认折叠，支持展开/折叠切换

## 3. 前端：树形导航实现

- [x] 3.1 实现 `ConnectionTree.vue` 数据加载，调用 list_slave_connections 和 list_slave_devices 构建树数据
- [x] 3.2 实现三层树节点渲染：连接节点（图标+地址:端口+状态）、从站节点（从站ID+名称）、寄存器分组节点（0x/1x/3x/4x）
- [x] 3.3 实现节点展开/折叠功能，新创建的连接默认展开
- [x] 3.4 实现节点点击事件：点击连接节点选中连接，点击从站节点加载全部寄存器，点击分组节点过滤寄存器类型
- [x] 3.5 实现右键菜单：连接节点（启动/停止/删除）、从站节点（删除）
- [x] 3.6 实现树数据的自动刷新（创建/删除连接或从站后更新树）

## 4. 前端：寄存器表格重构

- [x] 4.1 创建 `RegisterTable.vue` 替代 SlaveView 中的寄存器表格部分
- [x] 4.2 实现表格列：ADDRESS（十六进制显示）、ALIAS、VALUE、COMMENT
- [x] 4.3 实现双击 VALUE 单元格进入编辑模式，回车或失焦时提交写入
- [x] 4.4 实现表格行选中高亮，选中行时触发事件通知 ValuePanel
- [x] 4.5 实现按寄存器类型过滤显示（响应树节点点击的分组过滤）
- [x] 4.6 保留右键菜单功能（编辑寄存器定义、删除寄存器）

## 5. 前端：值解析面板实现

- [x] 5.1 实现 `ValuePanel.vue` 接收选中寄存器信息（类型、地址、值）
- [x] 5.2 实现 16 位值解析显示：Signed、Unsigned、Hex、Binary
- [x] 5.3 实现 32 位组合值解析：读取相邻寄存器，计算 Long AB CD、Long CD AB、Float AB CD、Float CD AB
- [x] 5.4 实现 64 位组合值解析：读取连续 4 个寄存器，计算 Double
- [x] 5.5 实现 Bool 类型寄存器的适配显示（Coil/DI 类型）
- [x] 5.6 实现面板标题显示："4x Holding Register @ 0x0000" 格式

## 6. 前端：工具栏功能实现

- [x] 6.1 实现"新建连接"按钮：弹出端口输入对话框，确认后调用 create_slave_connection 并刷新树
- [x] 6.2 实现"新建从站"按钮：弹出从站ID输入对话框，确认后调用 add_slave_device 并刷新树；未选中连接时禁用
- [x] 6.3 实现"启动连接"/"停止连接"按钮：根据选中连接状态启用/禁用，调用对应命令并更新树节点状态
- [x] 6.4 实现"关闭连接"按钮：调用 delete_slave_connection 删除选中连接，清理树和表格状态
- [x] 6.5 实现"工具"按钮：弹出工具面板（复用现有 ToolsView 的 CRC/LRC/地址转换功能）

## 7. 前端：底部日志面板

- [x] 7.1 实现日志面板折叠/展开切换按钮
- [x] 7.2 实现日志面板跟随当前选中连接自动切换数据源
- [x] 7.3 保留清空日志和导出 CSV 功能按钮

## 8. 清理和收尾

- [x] 8.1 删除旧的 `SlaveView.vue`（功能已拆分到 RegisterTable + ConnectionTree + Toolbar）
- [x] 8.2 处理 `MasterView.vue` 的入口：在工具栏或菜单中提供 Master 模式访问方式
- [x] 8.3 统一所有 UI 文案为中文
- [x] 8.4 调整深色主题配色，保持与 Redisant MSE 的专业工具风格一致
- [ ] 8.5 端到端手动测试：创建连接 → 树展示正确 → 点击分组 → 表格过滤 → 选中行 → 值面板解析 → 双击编辑 → 日志面板查看通信记录
