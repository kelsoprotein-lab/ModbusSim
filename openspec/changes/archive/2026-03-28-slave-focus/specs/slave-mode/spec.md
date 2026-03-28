## 修改需求

### 需求:程序运行模式

程序必须仅包含从站模式，禁止包含任何主站相关的 UI 或后端命令。启动后直接进入从站三栏布局，无需模式切换。

#### 场景:程序启动
- **当** 用户启动程序
- **那么** 必须直接显示从站三栏布局（左侧树形导航 + 中间寄存器表格 + 右侧值解析面板），禁止显示模式切换按钮

#### 场景:工具栏内容
- **当** 程序显示工具栏
- **那么** 工具栏禁止包含"从站模式/主站模式"切换按钮
- **那么** 工具栏必须保留新建连接、新建从站、启动、停止、关闭、工具等从站相关按钮

#### 场景:后端命令
- **当** 程序启动并注册 Tauri 命令
- **那么** 禁止注册任何 `master_*` 相关命令（`create_master_connection`、`connect_master`、`disconnect_master`、`delete_master_connection`、`list_master_connections`、`master_read`、`master_write_single_coil`、`master_write_single_register`）

### 需求:通信日志命令

通信日志命令（`get_communication_logs`、`clear_communication_logs`、`export_logs_csv`）必须仅服务从站连接，禁止包含 `is_master` 参数。

#### 场景:获取从站通信日志
- **当** 前端调用 `get_communication_logs`，传入 `connection_id`
- **那么** 系统必须返回该从站连接的通信日志，命令签名中禁止包含 `is_master` 参数

#### 场景:清空从站通信日志
- **当** 前端调用 `clear_communication_logs`，传入 `connection_id`
- **那么** 系统必须清空该从站连接的通信日志

#### 场景:导出从站通信日志
- **当** 前端调用 `export_logs_csv`，传入 `connection_id`
- **那么** 系统必须导出该从站连接的 CSV 格式通信日志
