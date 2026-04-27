# ModbusSim

跨平台 Modbus 仿真套件 — 包含 **ModbusSlave**（从站模拟器）和 **ModbusMaster**（主站工具），提供两套 UI：**Tauri 2 + Vue 3** 桌面版，以及 **Rust + egui** 原生版。支持 **TCP、TCP+TLS、RTU、ASCII、RTU-over-TCP** 五种传输模式。

[English](README.md)

## 下载

**→ [最新版本 (v0.12.0)](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest)**

每次 GitHub Release 会附带 Tauri 版预编译安装包：

| 平台 | 安装包格式 |
|------|-----------|
| macOS (Apple Silicon / Intel) | `.dmg` |
| Windows (x64) | `.exe` (NSIS) / `.msi` |
| Linux (x64) | `.deb` / `.AppImage` / `.rpm` |

egui 版暂未打包 release 产物，本地运行见下方 [开发](#开发) 章节：`cargo run -p modbussim-egui` / `cargo run -p modbusmaster-egui`。

## 开发版更新（main 分支，未发布）

- **egui 中英文切换** — 子站 `modbussim-egui` 菜单栏新增「语言 / Language」，支持中 / 英即时切换；选择写入 eframe storage (`lang_v1`)，下次启动自动恢复。翻译表位于新增的 `modbussim-ui-shared::i18n` 模块（`Lang { Zh, En }` + `tr/tr1/tr2`），未命中键直接回落为 key 本身，便于开发期定位缺译项。已覆盖菜单、欢迎屏、侧边栏、TLS 表单、状态栏、寄存器表工具栏与列头、数据源 / 抖动面板、新增从站 / 批量添加对话框等用户可见字符串；运行时错误消息暂保留中文。

## v0.12.0 更新要点

- **egui 版** — ModbusSlave / ModbusMaster 新增纯 Rust 原生 egui 桌面版（`modbussim-egui`、`modbusmaster-egui`），共享 `modbussim-ui-shared` crate（主题 / 字体 / 日志面板 / ValuePanel / 项目读写）。
- **从站寄存器抖动** — 每个从站独立 `JitterConfig`，后台 runner 每 100 ms 驱动一次（bool 翻转概率、u16 百分比漂移）；序列化进 `.modbusproj`，老工程 `#[serde(default)]` 兼容。
- **寄存器表 UX** — 搜索 + 地址跳转（`Cmd/Ctrl+F`）、bool 改 `○ / ●` 圆点切换、改 4 列布局（地址 / 值 / 名称 / 注释）、新增 `RegViewCache` 缓存名称与注释。
- **数据源扩展** — egui 从站快添加菜单补齐 Sawtooth / Triangle / CsvPlayback。
- **视觉重构** — 扁平分层「redisant 工业风」浅色主题、暖灰 + 橙色 accent 的 Darcula 深色面板、L0/L1/L2 三级 region、iOS 风格 toggle 开关。
- **CI** — 新增 `ci-egui.yml`，在 macOS / Linux / Windows 三平台验证 egui 二进制可构建。

## 功能

### ModbusSlave — 从站模拟器

- **多传输模式** — TCP、TCP+TLS、RTU（串口）、ASCII（串口）、RTU-over-TCP
- **Modbus TCP over TLS** — TLS 1.2+ 加密传输，支持 PEM 和 PKCS#12 证书格式，可选 mTLS 双向认证（验证客户端证书）
- **多设备支持** — 在任意端口创建连接，每个连接支持多个从站设备
- **四种寄存器类型** — 线圈 (FC01)、离散输入 (FC02)、保持寄存器 (FC03)、输入寄存器 (FC04)
- **完整协议支持** — 读取 (FC01-04)、单点写入 (FC05/06)、多点写入 (FC15/16)，支持 Modbus 异常码
- **寄存器表格** — 地址搜索/过滤、行内值编辑、Ctrl/Shift 多选、虚拟滚动（支持 20,000+ 寄存器），多格式显示（Auto / U16 / I16 / Hex / Bin / Float32 四种字节序）
- **默认初始化** — 新建从站默认铺满地址 0~20,000（四种寄存器类型），批量添加单次最多 50,000 条
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
- **共享 Vue 组件** — 通用 composables 和类型定义通过 `shared-frontend` npm workspace 在两个 Tauri 应用间共享
- **共享 egui 组件** — `modbussim-ui-shared` crate：扁平分层主题、CJK 字体注入、日志面板、ValuePanel、寄存器搜索、`.modbusproj` 项目读写

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

- **核心（Rust）**: [tokio-modbus](https://github.com/slowtec/tokio-modbus) + [tokio-serial](https://github.com/berkowski/tokio-serial)
- **TLS**: [native-tls](https://crates.io/crates/native-tls)（系统 TLS：macOS Security.framework、Linux OpenSSL、Windows SChannel）
- **Tauri UI**: Vue 3 + TypeScript + [@tanstack/vue-virtual](https://tanstack.com/virtual) + [Tauri 2](https://tauri.app/)
- **egui UI**: [eframe](https://crates.io/crates/eframe) + [egui](https://github.com/emilk/egui) + egui_extras / egui-modal / egui-toast
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
│   ├── modbusmaster-app/      # Tauri 应用 — ModbusMaster
│   ├── modbussim-egui/        # egui 原生应用 — ModbusSlave
│   ├── modbusmaster-egui/     # egui 原生应用 — ModbusMaster
│   └── modbussim-ui-shared/   # egui 共享组件：主题 / 字体 / log_panel / value_panel / project
├── shared-frontend/           # 共享 Vue composables 和组件（Tauri UI 用）
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

### 启动开发 — Tauri 版

```bash
# 安装前端依赖（npm workspaces）
npm install

# ModbusSlave（开发模式）
cd crates/modbussim-app && cargo tauri dev

# ModbusMaster（开发模式）
cd crates/modbusmaster-app && cargo tauri dev
```

### 构建 — Tauri 版

```bash
cd crates/modbussim-app && cargo tauri build
cd crates/modbusmaster-app && cargo tauri build
```

### 运行 — egui 版

```bash
# ModbusSlave（原生 egui）
cargo run -p modbussim-egui --release

# ModbusMaster（原生 egui）
cargo run -p modbusmaster-egui --release
```

### 运行测试

```bash
cargo test --workspace
```

## 更新日志

完整版本历史见 [`CHANGELOG.md`](CHANGELOG.md);二进制下载(Tauri + egui 双版本)见
[Releases 页面](https://github.com/kelsoprotein-lab/ModbusSim/releases)。

## 许可证

MIT

## 作者

[kelsoprotein-lab](https://github.com/kelsoprotein-lab)
