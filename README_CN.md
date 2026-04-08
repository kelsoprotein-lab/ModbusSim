# ModbusSim

跨平台 Modbus 仿真套件 — 包含 **ModbusSlave**（从站模拟器）和 **ModbusMaster**（主站工具），基于 Tauri 2、Rust 和 Vue 3 构建。支持 **TCP、TCP+TLS、RTU、ASCII、RTU-over-TCP** 五种传输模式。

[English](README.md)

## 下载

**[最新版本下载](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest)**

| 平台 | ModbusSlave | ModbusMaster |
|------|------------|--------------|
| macOS (Apple Silicon) | [.dmg](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusSlave_0.1.0_aarch64.dmg) | [.dmg](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusMaster_0.1.0_aarch64.dmg) |
| macOS (Intel) | [.dmg](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusSlave_0.1.0_x64.dmg) | [.dmg](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusMaster_0.1.0_x64.dmg) |
| Windows | [.exe](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusSlave_0.1.0_x64-setup.exe) / [.msi](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusSlave_0.1.0_x64_en-US.msi) | [.exe](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusMaster_0.1.0_x64-setup.exe) / [.msi](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusMaster_0.1.0_x64_en-US.msi) |
| Linux | [.deb](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusSlave_0.1.0_amd64.deb) / [.AppImage](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusSlave_0.1.0_amd64.AppImage) / [.rpm](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusSlave-0.1.0-1.x86_64.rpm) | [.deb](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusMaster_0.1.0_amd64.deb) / [.AppImage](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusMaster_0.1.0_amd64.AppImage) / [.rpm](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusMaster-0.1.0-1.x86_64.rpm) |

## 功能

### ModbusSlave — 从站模拟器

- **多传输模式** — TCP、TCP+TLS、RTU（串口）、ASCII（串口）、RTU-over-TCP
- **Modbus TCP over TLS** — TLS 1.2+ 加密传输，支持 PEM 和 PKCS#12 证书格式，可选 mTLS 双向认证（验证客户端证书）
- **多设备支持** — 在任意端口创建连接，每个连接支持多个从站设备
- **四种寄存器类型** — 线圈 (FC01)、离散输入 (FC02)、保持寄存器 (FC03)、输入寄存器 (FC04)
- **完整协议支持** — 读取 (FC01-04)、单点写入 (FC05/06)、多点写入 (FC15/16)，支持 Modbus 异常码
- **寄存器表格** — 地址搜索/过滤、行内值编辑、Ctrl/Shift 多选、虚拟滚动（支持 10,000+ 寄存器）
- **值解析面板** — 多格式显示：有符号/无符号/十六进制/二进制 (16 位)、Long/Float (32 位)、Double (64 位)，四种字节序 (AB CD / CD AB / BA DC / DC BA)
- **动态数据源** — 模拟寄存器值变化：固定值、随机、正弦波、锯齿波、三角波、计数器、CSV 回放
- **通信日志** — 实时 TX/RX 日志记录，支持搜索、方向/功能码过滤、CSV 导出
- **项目文件** — 保存/加载完整配置为 `.modbusproj` 文件，方便多场景切换
- **串口支持** — 自动检测系统串口，可配置波特率、数据位、停止位、校验

### ModbusMaster — 主站工具

- **多传输模式** — TCP、TCP+TLS、RTU（串口）、ASCII（串口）、RTU-over-TCP
- **Modbus TCP over TLS** — TLS 1.2+ 加密传输，支持 PEM 和 PKCS#12 证书格式，支持自签名证书测试模式
- **扫描组** — 按寄存器组配置周期性轮询，自定义轮询间隔，支持独立从站 ID 覆盖
- **设备发现** — 从站 ID 扫描 (1-247)、寄存器地址扫描、发现设备后自动添加到扫描组
- **多格式数据视图** — 无符号、有符号、十六进制、二进制、Float32 (AB CD / CD AB)，虚拟滚动
- **写入操作** — 支持写入单/多个线圈和寄存器 (FC05/06/15/16)
- **通信日志** — TX/RX 日志，支持搜索/过滤（方向、功能码、文本）
- **自动重连** — 可配置的指数退避重连策略（1s → 2s → 4s → ... → 最大 30s）
- **项目文件** — 保存/加载连接和扫描组配置
- **连接即扫描** — 连接成功后自动提示扫描从站设备

### 共享架构

- **统一错误系统** — 结构化 `ModbusError`，分类错误类型（连接/协议/应用），序列化为 JSON 供前端解析
- **共享前端组件** — 通用 composables 和类型定义通过 `shared-frontend` npm workspace 在两个应用间共享

## 支持的功能码

| 功能码 | 功能 | 从站（服务器） | 主站（客户端） |
|--------|------|:-:|:-:|
| FC01 | 读线圈 | 读取 | 读取/轮询 |
| FC02 | 读离散输入 | 读取 | 读取/轮询 |
| FC03 | 读保持寄存器 | 读取 | 读取/轮询 |
| FC04 | 读输入寄存器 | 读取 | 读取/轮询 |
| FC05 | 写单个线圈 | 写入 | 写入 |
| FC06 | 写单个寄存器 | 写入 | 写入 |
| FC15 | 写多个线圈 | 写入 | 写入 |
| FC16 | 写多个寄存器 | 写入 | 写入 |

## 传输模式

| 模式 | 传输层 | 帧格式 | 使用场景 |
|------|--------|--------|----------|
| TCP | TCP/IP 套接字 | MBAP 头 | 标准 Modbus TCP |
| TCP+TLS | TLS over TCP | MBAP 头 | 安全 Modbus TCP（TLS 1.2+） |
| RTU | 串口 | 从站 ID + CRC-16 | RS-485/RS-232 设备 |
| ASCII | 串口 | `:` + 十六进制 + LRC + CRLF | 传统串口设备 |
| RTU-over-TCP | TCP/IP 套接字 | 从站 ID + CRC-16 | 工业网关 |

## 技术栈

- **后端**: Rust + [tokio-modbus](https://github.com/slowtec/tokio-modbus) + [tokio-serial](https://github.com/berkowski/tokio-serial)
- **TLS**: [native-tls](https://crates.io/crates/native-tls)（系统 TLS：macOS Security.framework、Linux OpenSSL、Windows SChannel）
- **前端**: Vue 3 + TypeScript + [@tanstack/vue-virtual](https://tanstack.com/virtual)
- **框架**: [Tauri 2](https://tauri.app/)
- **串口**: [serialport](https://crates.io/crates/serialport) 端口枚举

## 项目结构

```
ModbusSim/
├── crates/
│   ├── modbussim-core/        # 核心库：协议、传输、寄存器、日志
│   │   ├── src/
│   │   │   ├── slave.rs       # 从站连接（TCP/RTU/ASCII/RtuOverTcp 派发）
│   │   │   ├── master.rs      # 主站连接，多传输模式支持
│   │   │   ├── frame.rs       # RTU/ASCII 帧编解码
│   │   │   ├── pdu.rs         # Modbus PDU 请求/响应解析
│   │   │   ├── transport.rs   # Transport 枚举、串口配置、TLS 配置、端口枚举
│   │   │   ├── mbap.rs        # MBAP 帧编解码（TLS 模式使用）
│   │   │   ├── tls_slave.rs   # TLS 加密 Modbus TCP 从站服务器
│   │   │   ├── tls_master.rs  # TLS 加密 Modbus TCP 主站客户端
│   │   │   ├── register.rs    # 寄存器类型、编码/解码
│   │   │   ├── data_source.rs # 动态数据源（正弦波、计数器等）
│   │   │   ├── reconnect.rs   # 重连策略（指数退避）
│   │   │   ├── error.rs       # 统一 ModbusError 枚举
│   │   │   ├── project.rs     # .modbusproj 文件保存/加载/迁移
│   │   │   ├── log_collector.rs # 线程安全日志环形缓冲区
│   │   │   └── ...
│   ├── modbussim-app/         # Tauri 应用 — ModbusSlave
│   └── modbusmaster-app/      # Tauri 应用 — ModbusMaster
├── shared-frontend/           # 共享 Vue composables 和组件
│   └── src/
│       ├── composables/       # useDialog, useValueFormat, useLogPanel, useLogFilter, useErrorHandler
│       ├── components/        # AppDialog
│       └── types/             # 共享 TypeScript 类型
├── frontend/                  # Vue 3 前端 — ModbusSlave
└── master-frontend/           # Vue 3 前端 — ModbusMaster
```

## 开发

### 前置要求

- [Rust](https://rustup.rs/)（stable）
- [Node.js](https://nodejs.org/)（v18+）
- [Tauri CLI](https://tauri.app/start/prerequisites/)

### 启动开发

```bash
# 安装前端依赖（npm workspaces）
npm install

# ModbusSlave（开发模式）
cd crates/modbussim-app && cargo tauri dev

# ModbusMaster（开发模式）
cd crates/modbusmaster-app && cargo tauri dev
```

### 构建

```bash
cd crates/modbussim-app && cargo tauri build
cd crates/modbusmaster-app && cargo tauri build
```

### 运行测试

```bash
cargo test --workspace
```

## 许可证

MIT

## 作者

[kelsoprotein-lab](https://github.com/kelsoprotein-lab)
