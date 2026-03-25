## 为什么

团队在日常工业自动化开发和调试中，需要一个跨平台（macOS / Windows）的 Modbus 模拟器来模拟从站设备和调试主站通信。现有工具要么仅支持 Windows，要么功能单一，无法满足同时模拟多个从站、灵活配置寄存器映射、团队间共享设备配置等需求。构建一个自研的、轻量级的 Modbus 模拟器，可以统一团队的调试工具链，提升开发效率。

## 变更内容

从零构建一个基于 Rust + Tauri 的桌面应用 **ModbusSim**，包含：

- **新增** Slave（子站）模式：模拟一个或多个 Modbus 从站设备，响应主站的读写请求
- **新增** Master（主站）模式：主动连接从站设备，执行寄存器读写操作和轮询
- **新增** 多从站支持：一个连接（TCP 端口 / 串口）下可挂载多个从站 ID，各自拥有独立的寄存器映射
- **新增** 寄存器管理：支持四种寄存器类型（Coil / Discrete Input / Holding Register / Input Register），可配置名称、数据类型、字节序、注释
- **新增** 多数据类型解读：bool / uint16 / int16 / uint32 / int32 / float32，支持 Big/Little/Mid-Big/Mid-Little 字节序
- **新增** 通信日志：实时显示收发报文，支持清除和导出
- **新增** 配置文件：设备寄存器映射以 JSON 格式导入/导出，方便团队共享
- **新增** 实用工具：Modbus 地址与 PLC 地址互转、CRC/LRC 校验计算

## 功能 (Capabilities)

### 新增功能

- `slave-mode`: 子站模拟引擎 — TCP 监听、多从站 ID 管理、寄存器存储与请求响应
- `master-mode`: 主站调试引擎 — TCP 连接、读写操作、单次请求与轮询调度
- `register-model`: 寄存器数据模型 — 四种寄存器类型、多数据类型解读、字节序处理、元数据（名称/注释）
- `comm-log`: 通信日志 — 报文实时捕获、解析展示、导出功能
- `config-management`: 配置管理 — 设备寄存器映射的 JSON 导入/导出、连接参数持久化
- `modbus-tools`: 实用工具集 — 地址转换、CRC/LRC 校验计算

### 修改功能

（无 — 全新项目）

## 影响

- **技术栈引入**：Rust（后端核心）、Tauri v2（桌面框架）、Web 前端框架（Svelte/Vue/React）
- **依赖库**：tokio-modbus（Modbus 协议）、tokio（异步运行时）、serde（序列化）、tokio-serial（串口，P1 阶段）
- **构建产物**：macOS .dmg / .app、Windows .msi / .exe
- **团队影响**：统一调试工具，需要团队成员了解基本的 Modbus 协议概念
