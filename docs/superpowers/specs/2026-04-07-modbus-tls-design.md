# Modbus TCP TLS Support Design

## Overview

为 ModbusSim 的 Slave 和 Master 两端添加 Modbus TCP over TLS 支持，参照 IEC 60870-5-104 Simulator 项目的 TLS 实现模式。

## Requirements

- Slave 和 Master 两端均支持 TLS
- 支持单向 TLS（客户端验证服务器）和双向 mTLS（服务器也验证客户端），用户可选
- 证书格式支持 PEM 和 PKCS#12
- TLS 模式使用自实现的 Modbus TCP 协议层；非 TLS 模式保留现有 `tokio-modbus` 代码
- Master 端提供 `accept_invalid_certs` 选项用于自签名证书测试
- 前端 UI 与 104 项目保持一致风格

## Dependencies

- `native-tls` v0.2 — TLS 核心库（使用系统 TLS: macOS Security.framework, Linux OpenSSL）
- `tokio-native-tls` v0.3 — tokio 异步包装
- `rcgen` (dev) — 测试中动态生成证书

## Data Structures

### Transport 枚举扩展

`transport.rs` 新增 `TcpTls` 变体：

```rust
pub enum Transport {
    Tcp { host: String, port: u16 },
    TcpTls { host: String, port: u16 },  // 新增
    Rtu(SerialConfig),
    Ascii(SerialConfig),
    RtuOverTcp { host: String, port: u16 },
}
```

`Transport::TcpTls` 只标识传输类型。TLS 证书配置作为独立字段存在于 `SlaveConnection` 和 `MasterConfig` 中，避免枚举臃肿。

### TLS 配置结构体

```rust
/// Master 端 TLS 配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TlsConfig {
    pub enabled: bool,
    pub ca_file: String,              // CA 证书 (PEM)，验证服务器
    pub cert_file: String,            // 客户端证书 (PEM)
    pub key_file: String,             // 客户端私钥 (PEM)
    pub pkcs12_file: String,          // PKCS#12 身份包（优先于 PEM）
    pub pkcs12_password: String,      // PKCS#12 密码
    pub accept_invalid_certs: bool,   // 接受自签名证书（测试用）
}

/// Slave 端 TLS 配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SlaveTlsConfig {
    pub enabled: bool,
    pub cert_file: String,            // 服务器证书 (PEM)
    pub key_file: String,             // 服务器私钥 (PEM)
    pub ca_file: String,              // CA 证书 (PEM)，验证客户端
    pub require_client_cert: bool,    // 是否要求客户端证书 (mTLS)
    pub pkcs12_file: String,          // PKCS#12 身份包（优先于 PEM）
    pub pkcs12_password: String,      // PKCS#12 密码
}
```

## Slave TLS Implementation

### 新模块 `tls_slave.rs`

独立模块，不修改现有 `slave.rs` 中的 `tokio-modbus` 逻辑。

### 连接处理流程

```
TcpListener::accept()
  -> spawn_blocking { tls_acceptor.accept(tcp_stream) }
    -> 握手成功: 进入 MBAP 帧处理循环
    -> 握手失败: 记录日志，关闭连接
```

### TLS Acceptor 构建

```rust
fn build_tls_acceptor(config: &SlaveTlsConfig) -> Result<native_tls::TlsAcceptor> {
    // 1. 加载服务器身份（PKCS#12 优先，否则 PEM）
    let identity = if !config.pkcs12_file.is_empty() {
        Identity::from_pkcs12(&bytes, &config.pkcs12_password)?
    } else {
        Identity::from_pkcs8(&cert_pem, &key_pem)?
    };

    // 2. 构建 acceptor
    let mut builder = native_tls::TlsAcceptor::builder(identity);
    builder.min_protocol_version(Some(Protocol::Tlsv12));

    // 3. mTLS: 通过加载 CA 证书启用客户端验证
    // (require_client_cert + ca_file)

    builder.build()
}
```

### MBAP 帧处理循环

在同步线程 (`spawn_blocking`) 中运行：

```
loop {
    1. 读取 7 字节 MBAP Header (transaction_id, protocol_id, length, unit_id)
    2. 读取 PDU (length - 1 字节)
    3. 解析 Request
    4. 调用现有的 handle_read / handle_write 函数处理请求
    5. 构建 Response MBAP Header + Response PDU，写回
    6. 通过 LogCollector 记录日志
}
```

关键点：
- 复用 `SharedDevices` 和现有的 `handle_read`/`handle_write` 函数
- TLS 流使用 `set_read_timeout` 避免阻塞线程无法响应 shutdown 信号
- 每个客户端连接一个 `spawn_blocking` 线程

### SlaveConnection::start() 分支

```rust
match &self.transport {
    Transport::Tcp { .. }       => { /* 现有 tokio-modbus 代码不动 */ }
    Transport::TcpTls { .. }    => { /* 调用 tls_slave::run_tls_slave(...) */ }
    Transport::Rtu(..)          => { /* 不动 */ }
    Transport::Ascii(..)        => { /* 不动 */ }
    Transport::RtuOverTcp { .. } => { /* 不动 */ }
}
```

## Master TLS Implementation

### 新模块 `tls_master.rs`

独立模块，不修改现有 `master.rs` 中的 `tokio-modbus` 逻辑。

### 连接流程

```
TcpStream::connect(addr)
  -> spawn_blocking { tls_connector.connect(domain, tcp_stream) }
    -> 握手成功: 返回 TlsStream, 进入读写循环
    -> 握手失败: 返回错误，触发重连策略
```

### TLS Connector 构建

```rust
fn build_tls_connector(config: &TlsConfig) -> Result<native_tls::TlsConnector> {
    let mut builder = native_tls::TlsConnector::builder();
    builder.min_protocol_version(Some(Protocol::Tlsv12));

    // CA 证书
    if !config.ca_file.is_empty() {
        let ca_pem = std::fs::read(&config.ca_file)?;
        let ca_cert = Certificate::from_pem(&ca_pem)?;
        builder.add_root_certificate(ca_cert);
    }

    // 客户端身份（PKCS#12 优先）
    if !config.pkcs12_file.is_empty() {
        let p12 = std::fs::read(&config.pkcs12_file)?;
        let identity = Identity::from_pkcs12(&p12, &config.pkcs12_password)?;
        builder.identity(identity);
    } else if !config.cert_file.is_empty() && !config.key_file.is_empty() {
        let cert = std::fs::read(&config.cert_file)?;
        let key = std::fs::read(&config.key_file)?;
        let identity = Identity::from_pkcs8(&cert, &key)?;
        builder.identity(identity);
    }

    // 测试模式
    if config.accept_invalid_certs {
        builder.danger_accept_invalid_certs(true);
        builder.danger_accept_invalid_hostnames(true);
    }

    builder.build()
}
```

### MBAP 帧读写

与 Slave 端对称：

```
发送: 构建 MBAP Header + Request PDU -> write_all
接收: 读取 7 字节 Header -> 读取 Response PDU -> 解析 Response
```

### 与现有 Master 的关系

- `MasterConfig` 增加 `tls: TlsConfig` 字段
- 连接时根据 `transport` 类型分流:
  - `Transport::Tcp` -> 现有 `tokio-modbus` 路径
  - `Transport::TcpTls` -> `tls_master::connect_tls(...)`
- 重连策略 (`reconnect.rs`) 对 TLS 连接同样适用，复用现有的指数退避逻辑

## Tauri Commands

### Slave 端

`CreateServerRequest` 增加字段：

```rust
pub struct CreateServerRequest {
    // 现有字段...
    pub use_tls: Option<bool>,
    pub cert_file: Option<String>,
    pub key_file: Option<String>,
    pub ca_file: Option<String>,
    pub require_client_cert: Option<bool>,
    pub pkcs12_file: Option<String>,
    pub pkcs12_password: Option<String>,
}
```

当 `use_tls == true` 时，`transport` 构建为 `Transport::TcpTls`，并填充 `SlaveTlsConfig`。

### Master 端

`CreateConnectionRequest` 增加字段：

```rust
pub struct CreateConnectionRequest {
    // 现有字段...
    pub use_tls: Option<bool>,
    pub ca_file: Option<String>,
    pub cert_file: Option<String>,
    pub key_file: Option<String>,
    pub pkcs12_file: Option<String>,
    pub pkcs12_password: Option<String>,
    pub accept_invalid_certs: Option<bool>,
}
```

## Frontend UI

### Slave (Toolbar.vue)

TCP 模式下新增 "TLS" 开关。开启后展开：
- 服务器证书路径（文件选择器）
- 服务器私钥路径（文件选择器）
- CA 证书路径（文件选择器，用于 mTLS）
- PKCS#12 文件路径 + 密码
- `require_client_cert` 开关

### Master (连接配置)

新增 "TLS" 开关。开启后展开：
- CA 证书路径（文件选择器）
- 客户端证书路径（文件选择器）
- 客户端私钥路径（文件选择器）
- PKCS#12 文件路径 + 密码
- `accept_invalid_certs` 开关

文件路径输入使用 Tauri 的 `dialog.open` API。

## Logging

TLS 相关事件记入现有的 `LogCollector`：
- TLS 握手开始 / 成功 / 失败（含错误原因）
- 证书加载成功 / 失败
- 连接断开（区分正常关闭和 TLS 错误）

## Error Handling

`SlaveError` 和 Master 错误类型各增加 TLS 变体：

```rust
pub enum SlaveError {
    // 现有变体...
    #[error("TLS error: {0}")]
    TlsError(String),
    #[error("certificate error: {0}")]
    CertError(String),
}
```

## Testing

1. **单元测试** — MBAP 帧编码/解码函数（不依赖 TLS）
2. **集成测试** `tests/tls_e2e.rs`:
   - 用 `rcgen` 动态生成 CA + 服务器证书 + 客户端证书
   - 单向 TLS: Slave TLS 服务器 + Master TLS 客户端，读写寄存器
   - 双向 mTLS: Slave 要求客户端证书
   - 证书验证失败场景: 错误 CA、无客户端证书等

## Out of Scope

- 不修改现有的 `tokio-modbus` TCP 路径
- 不加 TLS 版本选择（固定最低 TLS 1.2）
- 不加证书热更新（改证书重启服务即可）
- 不加 RTU/ASCII over TLS

## New Files

| File | Purpose |
|------|---------|
| `crates/modbussim-core/src/tls_slave.rs` | Slave TLS 服务器实现 |
| `crates/modbussim-core/src/tls_master.rs` | Master TLS 客户端实现 |
| `crates/modbussim-core/src/mbap.rs` | MBAP 帧编解码（TLS 模式共用） |
| `crates/modbussim-core/tests/tls_e2e.rs` | TLS 端到端测试 |

## Modified Files

| File | Change |
|------|--------|
| `crates/modbussim-core/Cargo.toml` | 添加 `native-tls`, `tokio-native-tls` 依赖 |
| `crates/modbussim-core/src/lib.rs` | 导出新模块 |
| `crates/modbussim-core/src/transport.rs` | `Transport` 枚举增加 `TcpTls` 变体，增加 TLS 配置结构体 |
| `crates/modbussim-core/src/slave.rs` | `SlaveConnection` 增加 `SlaveTlsConfig` 字段，`start()` 增加 `TcpTls` 分支 |
| `crates/modbussim-core/src/master.rs` | `MasterConfig` 增加 `TlsConfig` 字段，连接逻辑增加 TLS 分支 |
| `crates/modbussim-core/src/error.rs` | 增加 TLS 错误变体 |
| `crates/modbussim-app/src/commands.rs` | Slave Tauri commands 增加 TLS 参数 |
| `crates/modbusmaster-app/src/commands.rs` | Master Tauri commands 增加 TLS 参数 |
| `frontend/src/components/Toolbar.vue` | Slave UI 增加 TLS 配置区域 |
| `master-frontend/src/components/Toolbar.vue` | Master UI 增加 TLS 配置区域 |
