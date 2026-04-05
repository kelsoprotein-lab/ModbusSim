# ModbusSim 主从站全面改进设计

## 概述

对 ModbusSim 项目进行三阶段渐进式重构，解决代码重复、功能缺失、错误处理不完整、UI/UX 体验不佳和协议局限五大问题。

**策略：** 先重构架构基础（消除重复），再扩展协议支持（RTU/ASCII），最后增强用户体验。每阶段交付可用版本，主分支始终保持可用。

**关键决策：**
- 主站和从站保持独立应用，通过共享库消除重复代码
- 配置采用项目文件模式（`.modbusproj`），支持多场景切换
- 支持真实串口通信 + RTU over TCP
- 自动重连采用通知式（自动重试 + UI 状态可见 + 用户可取消）

---

## Phase 1：架构基础

### 1.1 后端共享 crate 重构

**目标：** 将 `modbussim-app` 和 `modbusmaster-app` 中的重复代码下沉到 `modbussim-core`。

**新增模块：**

#### `modbussim-core::parse`

统一解析函数，取代两个 app 中各自实现的版本：

```rust
// crates/modbussim-core/src/parse.rs
pub fn parse_register_type(s: &str) -> Result<RegisterType, ModbusError>;
pub fn parse_endian(s: &str) -> Result<Endian, ModbusError>;
pub fn parse_data_type(s: &str) -> Result<DataType, ModbusError>;
```

#### `modbussim-core::log_commands`

通用日志操作逻辑（纯函数，非 Tauri command）：

```rust
// crates/modbussim-core/src/log_commands.rs
pub fn query_logs(collector: &LogCollector, offset: usize, limit: usize) -> Vec<LogEntry>;
pub fn export_csv(collector: &LogCollector) -> String;
pub fn export_text(collector: &LogCollector) -> String;
pub fn clear_logs(collector: &LogCollector);
```

两个 app 的 Tauri command 瘦身为薄调用层：解包 Tauri State → 调用 core 函数 → 返回结果。

#### 统一错误类型基础

在 core 中定义 `ModbusError` 枚举，Phase 1 仅包含当前已有的错误场景（`DuplicateSlaveId`、`ConnectionFailed`、`Io` 等），确保两个 app 基于同一类型。Phase 3 再扩展为完整的细粒度错误体系。这样 Phase 1 的改动范围可控，不需要一次性修改所有错误路径。

### 1.2 前端共享组件库

**新增 `shared-frontend/` 目录**，作为 npm workspace package。

目录结构：

```
shared-frontend/
├── package.json
├── src/
│   ├── components/
│   │   └── LogPanel.vue          # 通用日志面板
│   ├── composables/
│   │   ├── useValueFormat.ts     # 值格式化（U16/I16/HEX/BIN/Float32）
│   │   ├── useDataTypeConversion.ts  # 数据类型转换逻辑
│   │   └── useModbusApi.ts       # Tauri invoke 封装（错误处理、loading）
│   └── types/
│       └── modbus.ts             # 共享 TypeScript 类型定义
└── tsconfig.json
```

**抽取策略：**
- `LogPanel.vue`：两端几乎相同，直接抽取
- `ValuePanel`：抽取共通逻辑为 composables，各 app 保留自己的 ValuePanel 组件消费 composables
- 类型定义：`RegisterType`、`Endian`、`DataType`、`LogEntry` 等统一到 `types/modbus.ts`

**不动的内容：**
- 从站独有：`RegisterModal.vue`、`BatchAddModal.vue`、`RegisterTable.vue`
- 主站独有：`ScanDialog.vue`、`DataTable.vue`

根目录 `package.json` 配置 npm workspaces：

```json
{
  "workspaces": ["frontend", "master-frontend", "shared-frontend"]
}
```

### 1.3 项目文件持久化

#### 文件格式

`.modbusproj` 文件，JSON 格式，带版本号：

```jsonc
{
  "version": 1,
  "type": "slave",
  "connections": [
    {
      "id": "uuid-string",
      "name": "测试连接1",
      "transport": {
        "type": "tcp",
        "host": "0.0.0.0",
        "port": 502
      },
      "devices": [
        {
          "slave_id": 1,
          "registers": {
            "coils": [
              { "address": 0, "count": 16, "values": [true, false, ...] }
            ],
            "discrete_inputs": [],
            "holding": [
              {
                "address": 0,
                "count": 10,
                "data_type": "uint16",
                "endian": "big",
                "values": [100, 200, 300, 0, 0, 0, 0, 0, 0, 0],
                "names": { "0": "温度", "1": "湿度" }
              }
            ],
            "input": []
          }
        }
      ]
    }
  ]
}
```

Master 类型额外包含扫描组：

```jsonc
{
  "version": 1,
  "type": "master",
  "connections": [
    {
      "id": "uuid-string",
      "name": "主站连接1",
      "transport": { "type": "tcp", "host": "192.168.1.100", "port": 502 },
      "scan_groups": [
        {
          "name": "温度采集",
          "slave_id": 1,
          "function_code": 3,
          "start_address": 0,
          "count": 10,
          "interval_ms": 1000
        }
      ]
    }
  ]
}
```

#### 核心实现

在 `modbussim-core` 中新增 `project` 模块：

```rust
// crates/modbussim-core/src/project.rs
pub struct ProjectFile {
    pub version: u32,
    pub project_type: ProjectType,  // Slave | Master
    pub connections: Vec<ConnectionConfig>,
}

pub fn save_project(project: &ProjectFile, path: &Path) -> Result<(), ModbusError>;
pub fn load_project(path: &Path) -> Result<ProjectFile, ModbusError>;
pub fn migrate_project(data: &str) -> Result<ProjectFile, ModbusError>;  // 版本迁移
```

#### 前端功能

- 工具栏增加：新建项目 / 打开项目 / 保存 (Ctrl+S) / 另存为
- 窗口标题显示当前项目文件名
- 自动保存：可在设置中开启，默认关闭，间隔 30 秒
- 最近项目列表：存储在 Tauri app data 目录，记录最近 10 个项目路径
- 退出时未保存提示：检测到修改未保存时弹出确认对话框
- 启动行为：无参数启动 = 空白状态；双击 `.modbusproj` 文件 = 加载项目

---

## Phase 2：协议与传输层扩展

### 2.1 传输层抽象

在 `modbussim-core` 中定义传输配置：

```rust
// crates/modbussim-core/src/transport.rs

pub enum Parity {
    None,
    Odd,
    Even,
}

pub struct SerialConfig {
    pub port: String,          // "/dev/ttyUSB0" 或 "COM3"
    pub baud_rate: u32,        // 9600, 19200, 38400, 57600, 115200
    pub data_bits: u8,         // 7 或 8
    pub stop_bits: u8,         // 1 或 2
    pub parity: Parity,
}

pub enum Transport {
    Tcp { host: String, port: u16 },
    Rtu(SerialConfig),
    Ascii(SerialConfig),
    RtuOverTcp { host: String, port: u16 },
}
```

#### 对 Slave 的改动

`SlaveService::start()` 根据 `Transport` 类型选择监听方式：

- **TCP：** 保持现有 `tokio-modbus` TCP server 逻辑不变
- **RTU：** 使用 `tokio-serial` 打开串口，手动解析 RTU 帧（slave_id + FC + data + CRC16），复用现有的寄存器处理逻辑
- **ASCII：** 类似 RTU，但帧以 `:` 开头、`\r\n` 结尾，数据为 ASCII hex 编码，使用 LRC 校验
- **RTU over TCP：** TCP 监听，但帧格式为 RTU（无 MBAP header，带 CRC）

#### 对 Master 的改动

`MasterConnection::connect()` 根据 `Transport` 类型选择连接方式：

- **TCP：** 保持现有逻辑
- **RTU/ASCII：** 打开串口，发送请求帧，等待响应（半双工，需要串行化请求）
- **RTU over TCP：** TCP 连接，但帧编解码走 RTU 路径

**串口通信的特殊处理：**
- 半双工约束：所有扫描组轮询必须串行执行（排队），不能并发
- 帧间隔检测：RTU 模式需要 3.5 字符时间的静默间隔（基于波特率计算）
- 响应超时：基于帧长度和波特率计算合理超时值，而非固定值

### 2.2 RTU/ASCII 帧编解码

在 core 中新增 `frame` 模块：

```rust
// crates/modbussim-core/src/frame.rs

pub struct RtuFrame {
    pub slave_id: u8,
    pub pdu: Vec<u8>,       // FC + data
    pub crc: u16,
}

pub struct AsciiFrame {
    pub slave_id: u8,
    pub pdu: Vec<u8>,
    pub lrc: u8,
}

pub fn encode_rtu(slave_id: u8, pdu: &[u8]) -> Vec<u8>;
pub fn decode_rtu(data: &[u8]) -> Result<RtuFrame, ModbusError>;
pub fn encode_ascii(slave_id: u8, pdu: &[u8]) -> Vec<u8>;
pub fn decode_ascii(data: &[u8]) -> Result<AsciiFrame, ModbusError>;
```

CRC-16 和 LRC 计算复用 `tools.rs` 中已有的实现。

### 2.3 串口枚举

新增系统串口扫描功能：

```rust
// crates/modbussim-core/src/transport.rs
pub fn list_serial_ports() -> Result<Vec<SerialPortInfo>, ModbusError>;
```

使用 `serialport` crate 枚举系统可用串口，返回端口名、描述、厂商信息。

### 2.4 前端适配

连接配置 UI 改动：

- 新增 "传输类型" 下拉选择：TCP / RTU / ASCII / RTU over TCP
- 选择 TCP 或 RTU over TCP 时：显示 Host + Port 输入框（现有 UI）
- 选择 RTU 或 ASCII 时：显示串口选择下拉（自动枚举）+ 波特率/数据位/停止位/校验配置
- 连接树节点显示传输类型标签（如 `[TCP]`、`[RTU]`）
- 项目文件中 `transport` 字段对应更新

---

## Phase 3：体验增强

### 3.1 错误处理细化

#### 统一错误枚举

扩展 `ModbusError` 为完整的错误体系：

```rust
pub enum ModbusError {
    // 连接层
    ConnectionRefused { addr: String },
    ConnectionTimeout { addr: String, timeout_ms: u64 },
    ConnectionLost { reason: String },
    SerialPortBusy { port: String },
    SerialPortNotFound { port: String },
    SerialPortPermissionDenied { port: String },

    // 协议层
    IllegalFunction { fc: u8 },
    IllegalDataAddress { addr: u16, count: u16 },
    IllegalDataValue { detail: String },
    SlaveDeviceFailure { slave_id: u8 },
    ResponseTimeout { slave_id: u8, fc: u8 },
    CrcMismatch { expected: u16, actual: u16 },
    LrcMismatch { expected: u8, actual: u8 },
    FrameError { detail: String },

    // 应用层
    SlaveIdConflict { id: u8 },
    RegisterOverlap { addr: u16, count: u16 },
    ProjectFileCorrupt { path: String },
    ProjectVersionUnsupported { version: u32 },
    
    // 通用
    Io(String),
    Internal(String),
}
```

所有错误实现 `serde::Serialize`，前端可直接解析结构化错误。

#### 前端错误处理

在 `shared-frontend` 中新增 `useErrorHandler` composable：

```typescript
// shared-frontend/src/composables/useErrorHandler.ts
export function useErrorHandler() {
  function handleError(error: ModbusError) {
    // 连接错误 → 红色持久 toast + 状态栏变化
    // 协议错误 → 橙色 3 秒 toast
    // 应用错误 → 蓝色 toast，附带操作建议
  }
}
```

### 3.2 自动重连

#### 重连状态机

```
Connected → (连接断开) → Reconnecting → (成功) → Connected
                              ↓ (用户取消/超过上限)
                          Disconnected
```

#### 实现细节

在 `MasterConnection` 中新增：

```rust
pub struct ReconnectPolicy {
    pub enabled: bool,
    pub initial_delay_ms: u64,    // 1000
    pub max_delay_ms: u64,        // 30000
    pub backoff_factor: f64,      // 2.0
    pub max_attempts: Option<u32>, // None = 无限重试
}
```

行为：
- 连接断开后自动进入重连，指数退避：1s → 2s → 4s → 8s → ... → 最大 30s
- 重连期间所有扫描组轮询暂停
- 重连成功后自动恢复所有之前活跃的扫描组
- 每次状态变化通过 Tauri 事件通知前端（`master-connection-state`）
- 用户可随时点击 "取消重连" 切换到 Disconnected 状态
- 串口模式特殊处理：检测到设备拔出 → 标记 "设备断开"，设备重新插入 → 自动重连

#### 前端 UI

- 连接树节点图标反映状态：绿色 = 已连接，黄色闪烁 = 重连中，灰色 = 已断开
- 重连中时工具栏显示 "正在重连... (第 3 次尝试)" + 取消按钮
- 重连成功/失败写入日志

### 3.3 RegisterTable 虚拟滚动

**方案：** 使用 `@tanstack/vue-virtual`

改动点：
- `RegisterTable.vue` 和 `DataTable.vue` 引入虚拟滚动
- 固定行高 32px，只渲染可见行 + 上下各 5 行缓冲
- 保持现有功能不变：搜索过滤、多选、内联编辑、Tab 切换显示格式
- 滚动容器使用 CSS `contain: strict` 优化渲染性能
- 目标：10,000+ 寄存器流畅滚动

### 3.4 日志面板增强

#### 搜索与过滤

- 搜索框：文本匹配日志内容（实时过滤，防抖 300ms）
- 过滤下拉菜单：
  - 方向：全部 / TX / RX
  - 功能码：全部 / FC01 / FC02 / FC03 / FC04 / FC05 / FC06 / FC15 / FC16
  - 从站 ID：全部 / 具体 ID（从当前日志中提取可选值）
- 多个过滤器可组合（AND 关系）
- 标题栏显示当前筛选条件摘要

#### 交互改进

- 默认高度从 32px 改为 150px
- 拖拽调整面板高度（保存到本地偏好）
- 新日志自动滚动到底部（用户手动上滚时暂停自动滚动，点击 "跳到最新" 恢复）
- 右键菜单：复制单条日志、清除所有、从此处导出

### 3.5 动态数据生成（从站）

#### 数据源定义

在 core 中新增 `data_source` 模块：

```rust
// crates/modbussim-core/src/data_source.rs

pub enum DataSource {
    Fixed { value: u16 },
    Random { min: u16, max: u16 },
    Sine { amplitude: f64, frequency: f64, offset: f64, phase: f64 },
    Sawtooth { min: u16, max: u16, period_ms: u64 },
    Triangle { min: u16, max: u16, period_ms: u64 },
    Counter { start: u16, step: i16, wrap: bool },
    CsvPlayback { values: Vec<u16>, loop_playback: bool },
}

pub struct DataSourceConfig {
    pub source: DataSource,
    pub update_interval_ms: u64,  // 数据刷新间隔
}
```

#### 运行时行为

- 每个寄存器可绑定一个 `DataSourceConfig`（可选，默认无 = 静态值）
- 后台定时器按 `update_interval_ms` 刷新绑定了数据源的寄存器值
- 数据源配置保存在项目文件中（扩展寄存器定义）

#### 前端 UI

- 寄存器编辑对话框（`RegisterModal`）增加 "数据源" Tab
- 选择数据源类型后，显示对应参数配置表单
- CSV 导入：文件选择器 + 预览前 10 行
- 数据源激活状态在寄存器表格中显示标识（如波浪线图标）

---

## 跨阶段关注点

### 测试策略

- **Phase 1：** 后端 core 模块新增单元测试覆盖 parse/log_commands/project 模块
- **Phase 2：** RTU/ASCII 帧编解码的单元测试；传输层集成测试（TCP ↔ RTU loopback）
- **Phase 3：** 错误处理路径测试；重连状态机测试；数据源输出正确性测试

### 兼容性

- 项目文件版本迁移：`migrate_project()` 函数处理旧版本文件升级
- 前端共享库不引入破坏性 API 变更，两个 app 可逐步迁移

### 依赖新增

- `tokio-serial`：串口异步 I/O
- `serialport`：串口枚举
- `@tanstack/vue-virtual`：虚拟滚动
