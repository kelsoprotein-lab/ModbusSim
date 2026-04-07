# Modbus TCP TLS Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Modbus TCP over TLS support for both Slave and Master, using `native-tls`, with self-implemented MBAP framing for TLS mode while preserving existing `tokio-modbus` for plain TCP.

**Architecture:** TLS mode uses independent modules (`tls_slave.rs`, `tls_master.rs`) that implement Modbus TCP protocol directly on `native_tls::TlsStream`. A shared `mbap.rs` handles MBAP frame encoding/decoding. Existing plain TCP code via `tokio-modbus` remains untouched. TLS config structs live in `transport.rs`, separate from the `Transport` enum.

**Tech Stack:** Rust, native-tls 0.2, tokio-native-tls 0.3, rcgen (dev), Tauri 2, Vue 3

---

## File Structure

| File | Responsibility |
|------|---------------|
| `crates/modbussim-core/src/mbap.rs` | MBAP header encode/decode, frame read/write helpers |
| `crates/modbussim-core/src/tls_slave.rs` | TLS slave server: accept, handshake, MBAP loop |
| `crates/modbussim-core/src/tls_master.rs` | TLS master client: connect, handshake, read/write operations |
| `crates/modbussim-core/src/transport.rs` | Extended with `TcpTls` variant, `TlsConfig`, `SlaveTlsConfig` |
| `crates/modbussim-core/src/slave.rs` | Add `SlaveTlsConfig` field, `TcpTls` branch in `start()` |
| `crates/modbussim-core/src/master.rs` | Add `TlsConfig` field to `MasterConfig`, `TcpTls` branch in `connect()` |
| `crates/modbussim-core/src/error.rs` | Add TLS error variants |
| `crates/modbussim-core/Cargo.toml` | Add dependencies |
| `crates/modbussim-core/src/lib.rs` | Export new modules |
| `crates/modbussim-app/src/commands.rs` | Slave Tauri commands: TLS params |
| `crates/modbusmaster-app/src/commands.rs` | Master Tauri commands: TLS params |
| `frontend/src/components/Toolbar.vue` | Slave UI: TLS toggle + cert fields |
| `master-frontend/src/components/Toolbar.vue` | Master UI: TLS toggle + cert fields |
| `crates/modbussim-core/tests/tls_e2e.rs` | E2E tests with rcgen-generated certs |

---

### Task 1: Add Dependencies and Scaffold Modules

**Files:**
- Modify: `crates/modbussim-core/Cargo.toml`
- Modify: `crates/modbussim-core/src/lib.rs`

- [ ] **Step 1: Add native-tls and tokio-native-tls dependencies**

In `crates/modbussim-core/Cargo.toml`, add to `[dependencies]`:

```toml
native-tls = "0.2"
tokio-native-tls = "0.3"
```

And add to `[dev-dependencies]`:

```toml
rcgen = "0.13"
openssl = "0.10"
```

- [ ] **Step 2: Add module declarations to lib.rs**

In `crates/modbussim-core/src/lib.rs`, add these lines:

```rust
pub mod mbap;
pub mod tls_slave;
pub mod tls_master;
```

- [ ] **Step 3: Create empty module files**

Create three empty files with minimal module-level doc comments:

`crates/modbussim-core/src/mbap.rs`:
```rust
//! MBAP (Modbus Application Protocol) frame encoding/decoding for TLS mode.
```

`crates/modbussim-core/src/tls_slave.rs`:
```rust
//! TLS-enabled Modbus TCP slave server.
```

`crates/modbussim-core/src/tls_master.rs`:
```rust
//! TLS-enabled Modbus TCP master client.
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p modbussim-core`
Expected: Compiles successfully with no errors.

- [ ] **Step 5: Commit**

```bash
git add crates/modbussim-core/Cargo.toml crates/modbussim-core/src/lib.rs \
  crates/modbussim-core/src/mbap.rs crates/modbussim-core/src/tls_slave.rs \
  crates/modbussim-core/src/tls_master.rs
git commit -m "feat: add TLS dependencies and scaffold modules"
```

---

### Task 2: Extend Transport and Error Types

**Files:**
- Modify: `crates/modbussim-core/src/transport.rs`
- Modify: `crates/modbussim-core/src/error.rs`

- [ ] **Step 1: Write tests for new Transport variant and TLS config structs**

Add to the `#[cfg(test)] mod tests` block in `crates/modbussim-core/src/transport.rs`:

```rust
#[test]
fn test_transport_tcp_tls_serde() {
    let t = Transport::TcpTls {
        host: "127.0.0.1".to_string(),
        port: 802,
    };
    let json = serde_json::to_string(&t).unwrap();
    let t2: Transport = serde_json::from_str(&json).unwrap();
    assert_eq!(t, t2);
}

#[test]
fn test_tls_config_default() {
    let cfg = TlsConfig::default();
    assert!(!cfg.enabled);
    assert!(cfg.ca_file.is_empty());
    assert!(!cfg.accept_invalid_certs);
}

#[test]
fn test_slave_tls_config_default() {
    let cfg = SlaveTlsConfig::default();
    assert!(!cfg.enabled);
    assert!(!cfg.require_client_cert);
    assert!(cfg.cert_file.is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p modbussim-core transport::tests::test_transport_tcp_tls_serde -- --nocapture`
Expected: FAIL — `TcpTls` variant and TLS config types don't exist yet.

- [ ] **Step 3: Add TcpTls variant and TLS config structs**

In `crates/modbussim-core/src/transport.rs`, add the `TcpTls` variant to the `Transport` enum:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Transport {
    Tcp { host: String, port: u16 },
    TcpTls { host: String, port: u16 },
    Rtu(SerialConfig),
    Ascii(SerialConfig),
    RtuOverTcp { host: String, port: u16 },
}
```

Add these structs after the `Transport` enum:

```rust
/// Master-side TLS configuration.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TlsConfig {
    pub enabled: bool,
    pub ca_file: String,
    pub cert_file: String,
    pub key_file: String,
    pub pkcs12_file: String,
    pub pkcs12_password: String,
    pub accept_invalid_certs: bool,
}

/// Slave-side TLS configuration.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SlaveTlsConfig {
    pub enabled: bool,
    pub cert_file: String,
    pub key_file: String,
    pub ca_file: String,
    pub require_client_cert: bool,
    pub pkcs12_file: String,
    pub pkcs12_password: String,
}
```

- [ ] **Step 4: Add TLS error variants to error.rs**

In `crates/modbussim-core/src/error.rs`, add two new variants to `ModbusError` (inside the `// Connection layer` section):

```rust
#[error("TLS error: {message}")]
TlsError { message: String },

#[error("certificate error: {message}")]
CertError { message: String },
```

And add them to the `category()` method match in the `"connection"` arm:

```rust
| ModbusError::TlsError { .. }
| ModbusError::CertError { .. } => "connection",
```

- [ ] **Step 5: Fix all compile errors from new Transport variant**

The new `TcpTls` variant will cause `match` exhaustiveness errors in `slave.rs` and `master.rs`. Add temporary placeholder arms:

In `crates/modbussim-core/src/slave.rs`, inside `start()` method's match block (after the `Transport::RtuOverTcp` arm), add:

```rust
Transport::TcpTls { .. } => {
    return Err(SlaveError::BindError("TLS not yet implemented".to_string()));
}
```

In `crates/modbussim-core/src/master.rs`, inside `connect()` method's match block (after the `Transport::RtuOverTcp` arm), add:

```rust
Transport::TcpTls { .. } => {
    return Err(MasterError::ConnectionFailed("TLS not yet implemented".to_string()));
}
```

Also add `TcpTls` to any other match statements in the Tauri command files that match on `Transport`. Check both `crates/modbussim-app/src/commands.rs` and `crates/modbusmaster-app/src/commands.rs` — their `to_transport()` functions and any match on `Transport` in the commands. Add a `TcpTls` variant to the `TransportRequest` enum in both command files:

In `crates/modbussim-app/src/commands.rs`, add to `TransportRequest`:
```rust
TcpTls { port: u16 },
```

And in `to_transport()`:
```rust
TransportRequest::TcpTls { port } => Transport::TcpTls { host: "0.0.0.0".into(), port: *port },
```

And add `Transport::TcpTls { host, port }` to the match in `create_slave_connection` that extracts `(bind_address, port)`:
```rust
Transport::Tcp { host, port } | Transport::TcpTls { host, port } | Transport::RtuOverTcp { host, port } => {
    (host.clone(), *port)
}
```

In `crates/modbusmaster-app/src/commands.rs`, add to `TransportRequest`:
```rust
TcpTls { host: String, port: u16 },
```

And in `to_transport()`:
```rust
TransportRequest::TcpTls { host, port } => Transport::TcpTls { host: host.clone(), port: *port },
```

And add `Transport::TcpTls { host, port }` alongside `Transport::Tcp` in the match that extracts `(target_address, port)`.

- [ ] **Step 6: Run all tests**

Run: `cargo test -p modbussim-core`
Expected: All tests pass including new transport and TLS config tests.

- [ ] **Step 7: Commit**

```bash
git add crates/modbussim-core/src/transport.rs crates/modbussim-core/src/error.rs \
  crates/modbussim-core/src/slave.rs crates/modbussim-core/src/master.rs \
  crates/modbussim-app/src/commands.rs crates/modbusmaster-app/src/commands.rs
git commit -m "feat: add TcpTls transport variant and TLS config structs"
```

---

### Task 3: Implement MBAP Frame Encoding/Decoding

**Files:**
- Create: `crates/modbussim-core/src/mbap.rs`

- [ ] **Step 1: Write MBAP tests**

Replace the contents of `crates/modbussim-core/src/mbap.rs` with:

```rust
//! MBAP (Modbus Application Protocol) frame encoding/decoding for TLS mode.

use std::io::{self, Read, Write};

/// MBAP header: 7 bytes total.
/// - transaction_id: 2 bytes (big-endian)
/// - protocol_id: 2 bytes (always 0x0000 for Modbus)
/// - length: 2 bytes (big-endian, = unit_id byte + PDU length)
/// - unit_id: 1 byte
#[derive(Debug, Clone, PartialEq)]
pub struct MbapHeader {
    pub transaction_id: u16,
    pub protocol_id: u16,
    pub length: u16,
    pub unit_id: u8,
}

pub const MBAP_HEADER_LEN: usize = 7;

impl MbapHeader {
    pub fn new(transaction_id: u16, unit_id: u8, pdu_len: usize) -> Self {
        Self {
            transaction_id,
            protocol_id: 0,
            length: (pdu_len + 1) as u16, // +1 for unit_id
            unit_id,
        }
    }

    pub fn encode(&self) -> [u8; MBAP_HEADER_LEN] {
        let mut buf = [0u8; MBAP_HEADER_LEN];
        buf[0..2].copy_from_slice(&self.transaction_id.to_be_bytes());
        buf[2..4].copy_from_slice(&self.protocol_id.to_be_bytes());
        buf[4..6].copy_from_slice(&self.length.to_be_bytes());
        buf[6] = self.unit_id;
        buf
    }

    pub fn decode(buf: &[u8; MBAP_HEADER_LEN]) -> Self {
        Self {
            transaction_id: u16::from_be_bytes([buf[0], buf[1]]),
            protocol_id: u16::from_be_bytes([buf[2], buf[3]]),
            length: u16::from_be_bytes([buf[4], buf[5]]),
            unit_id: buf[6],
        }
    }

    /// PDU length = length field - 1 (subtract unit_id byte).
    pub fn pdu_len(&self) -> usize {
        if self.length > 0 {
            (self.length - 1) as usize
        } else {
            0
        }
    }
}

/// Read one complete MBAP frame from a stream.
/// Returns (header, pdu_bytes).
pub fn read_frame(reader: &mut impl Read) -> io::Result<(MbapHeader, Vec<u8>)> {
    let mut hdr_buf = [0u8; MBAP_HEADER_LEN];
    reader.read_exact(&mut hdr_buf)?;
    let header = MbapHeader::decode(&hdr_buf);

    let pdu_len = header.pdu_len();
    if pdu_len == 0 || pdu_len > 253 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid MBAP PDU length: {}", pdu_len),
        ));
    }

    let mut pdu = vec![0u8; pdu_len];
    reader.read_exact(&mut pdu)?;
    Ok((header, pdu))
}

/// Write one complete MBAP frame to a stream.
pub fn write_frame(
    writer: &mut impl Write,
    transaction_id: u16,
    unit_id: u8,
    pdu: &[u8],
) -> io::Result<()> {
    let header = MbapHeader::new(transaction_id, unit_id, pdu.len());
    writer.write_all(&header.encode())?;
    writer.write_all(pdu)?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_header_encode_decode_roundtrip() {
        let header = MbapHeader::new(0x0001, 1, 5);
        let encoded = header.encode();
        let decoded = MbapHeader::decode(&encoded);
        assert_eq!(header, decoded);
    }

    #[test]
    fn test_header_fields() {
        let header = MbapHeader::new(0x1234, 7, 5);
        assert_eq!(header.transaction_id, 0x1234);
        assert_eq!(header.protocol_id, 0);
        assert_eq!(header.length, 6); // 5 + 1 for unit_id
        assert_eq!(header.unit_id, 7);
        assert_eq!(header.pdu_len(), 5);
    }

    #[test]
    fn test_header_encode_bytes() {
        let header = MbapHeader {
            transaction_id: 0x0001,
            protocol_id: 0x0000,
            length: 0x0006,
            unit_id: 0x01,
        };
        let bytes = header.encode();
        assert_eq!(bytes, [0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x01]);
    }

    #[test]
    fn test_read_write_frame_roundtrip() {
        let pdu = vec![0x03, 0x00, 0x00, 0x00, 0x0A]; // FC03 read 10 regs from addr 0
        let mut buf = Vec::new();
        write_frame(&mut buf, 1, 1, &pdu).unwrap();

        let mut cursor = Cursor::new(buf);
        let (header, read_pdu) = read_frame(&mut cursor).unwrap();
        assert_eq!(header.transaction_id, 1);
        assert_eq!(header.unit_id, 1);
        assert_eq!(read_pdu, pdu);
    }

    #[test]
    fn test_read_frame_invalid_length() {
        // Header with length=1 means pdu_len=0 which is invalid
        let buf = vec![0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x01];
        let mut cursor = Cursor::new(buf);
        let result = read_frame(&mut cursor);
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p modbussim-core mbap::tests -- --nocapture`
Expected: All 5 tests PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/modbussim-core/src/mbap.rs
git commit -m "feat: implement MBAP frame encoding/decoding"
```

---

### Task 4: Implement TLS Slave Server

**Files:**
- Create: `crates/modbussim-core/src/tls_slave.rs`
- Modify: `crates/modbussim-core/src/slave.rs`

- [ ] **Step 1: Implement tls_slave.rs**

Replace the contents of `crates/modbussim-core/src/tls_slave.rs` with:

```rust
//! TLS-enabled Modbus TCP slave server.

use crate::log_collector::LogCollector;
use crate::log_entry::{Direction, FunctionCode, LogEntry};
use crate::mbap;
use crate::pdu::{build_exception_pdu, build_response_pdu, parse_request_pdu, ModbusRequest, ResponseData};
use crate::register::RegisterMap;
use crate::slave::{SharedDevices, SharedLogCollector};
use crate::transport::SlaveTlsConfig;
use native_tls::{Identity, Protocol, TlsAcceptor};
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

fn build_tls_acceptor(config: &SlaveTlsConfig) -> Result<TlsAcceptor, String> {
    let identity = if !config.pkcs12_file.is_empty() {
        let p12_bytes = std::fs::read(&config.pkcs12_file)
            .map_err(|e| format!("failed to read PKCS#12 file: {e}"))?;
        Identity::from_pkcs12(&p12_bytes, &config.pkcs12_password)
            .map_err(|e| format!("invalid PKCS#12: {e}"))?
    } else if !config.cert_file.is_empty() && !config.key_file.is_empty() {
        let cert_pem = std::fs::read(&config.cert_file)
            .map_err(|e| format!("failed to read cert file: {e}"))?;
        let key_pem = std::fs::read(&config.key_file)
            .map_err(|e| format!("failed to read key file: {e}"))?;
        Identity::from_pkcs8(&cert_pem, &key_pem)
            .map_err(|e| format!("invalid PEM cert/key: {e}"))?
    } else {
        return Err("TLS enabled but no certificate configured".to_string());
    };

    let mut builder = TlsAcceptor::builder(identity);
    builder.min_protocol_version(Some(Protocol::Tlsv12));

    builder.build().map_err(|e| format!("failed to build TLS acceptor: {e}"))
}

fn log_tls_event(log_collector: &SharedLogCollector, direction: Direction, detail: &str) {
    if let Some(collector) = log_collector {
        // Use FC03 as a placeholder for TLS events — the detail field carries the info
        collector.try_add(LogEntry::new(direction, FunctionCode::ReadHoldingRegisters, detail));
    }
}

fn handle_client(
    mut stream: native_tls::TlsStream<std::net::TcpStream>,
    peer_addr: SocketAddr,
    devices: SharedDevices,
    log_collector: SharedLogCollector,
    shutdown: Arc<std::sync::Mutex<bool>>,
) {
    log_tls_event(&log_collector, Direction::Rx, &format!("TLS client connected: {}", peer_addr));

    // Set read timeout so we can periodically check shutdown flag
    if let Ok(inner) = stream.get_ref().try_clone() {
        let _ = inner.set_read_timeout(Some(Duration::from_millis(200)));
    }

    loop {
        if *shutdown.lock().unwrap() {
            break;
        }

        let (header, pdu) = match mbap::read_frame(&mut stream) {
            Ok(frame) => frame,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue; // timeout, check shutdown and retry
            }
            Err(_) => {
                break; // connection closed or error
            }
        };

        let unit_id = header.unit_id;
        let transaction_id = header.transaction_id;

        // Parse request
        let request = match parse_request_pdu(&pdu) {
            Ok(req) => req,
            Err(_) => {
                // Send exception: illegal function
                let fc = if pdu.is_empty() { 0 } else { pdu[0] };
                let exc_pdu = build_exception_pdu(fc, 0x01);
                let _ = mbap::write_frame(&mut stream, transaction_id, unit_id, &exc_pdu);
                continue;
            }
        };

        // Log RX
        let fc_byte = pdu[0];
        let fc = FunctionCode::from_u8(fc_byte);
        if let Some(fc) = fc {
            if let Some(collector) = &log_collector {
                collector.try_add(LogEntry::new(Direction::Rx, fc, &format_request_detail(&request)));
            }
        }

        // Process request using shared devices
        let response_pdu = process_request(unit_id, fc_byte, &request, &devices);

        // Log TX
        if let Some(fc) = fc {
            if let Some(collector) = &log_collector {
                let detail = if response_pdu.len() >= 2 && (response_pdu[0] & 0x80) != 0 {
                    format!("ERR: exception 0x{:02X}", response_pdu[1])
                } else {
                    "OK".to_string()
                };
                collector.try_add(LogEntry::new(Direction::Tx, fc, &detail));
            }
        }

        if mbap::write_frame(&mut stream, transaction_id, unit_id, &response_pdu).is_err() {
            break;
        }
    }

    log_tls_event(&log_collector, Direction::Tx, &format!("TLS client disconnected: {}", peer_addr));
}

fn format_request_detail(request: &ModbusRequest) -> String {
    match request {
        ModbusRequest::ReadCoils { address, quantity } => format!("R {} x{}", address, quantity),
        ModbusRequest::ReadDiscreteInputs { address, quantity } => format!("R {} x{}", address, quantity),
        ModbusRequest::ReadHoldingRegisters { address, quantity } => format!("R {} x{}", address, quantity),
        ModbusRequest::ReadInputRegisters { address, quantity } => format!("R {} x{}", address, quantity),
        ModbusRequest::WriteSingleCoil { address, value } => format!("W {} = {}", address, value),
        ModbusRequest::WriteSingleRegister { address, value } => format!("W {} = {:#06x}", address, value),
        ModbusRequest::WriteMultipleCoils { address, values } => format!("W {} x{}", address, values.len()),
        ModbusRequest::WriteMultipleRegisters { address, values } => format!("W {} x{}", address, values.len()),
    }
}

fn process_request(
    unit_id: u8,
    fc_byte: u8,
    request: &ModbusRequest,
    devices: &SharedDevices,
) -> Vec<u8> {
    let is_write = matches!(
        request,
        ModbusRequest::WriteSingleCoil { .. }
            | ModbusRequest::WriteSingleRegister { .. }
            | ModbusRequest::WriteMultipleCoils { .. }
            | ModbusRequest::WriteMultipleRegisters { .. }
    );

    if is_write {
        match devices.try_write() {
            Ok(mut devs) => match devs.get_mut(&unit_id) {
                Some(device) => execute_write(&mut device.register_map, fc_byte, request),
                None => build_exception_pdu(fc_byte, 0x0B), // Gateway target device failed
            },
            Err(_) => build_exception_pdu(fc_byte, 0x06), // Server device busy
        }
    } else {
        match devices.try_read() {
            Ok(devs) => match devs.get(&unit_id) {
                Some(device) => execute_read(&device.register_map, fc_byte, request),
                None => build_exception_pdu(fc_byte, 0x0B),
            },
            Err(_) => build_exception_pdu(fc_byte, 0x06),
        }
    }
}

fn execute_read(map: &RegisterMap, fc: u8, request: &ModbusRequest) -> Vec<u8> {
    match request {
        ModbusRequest::ReadCoils { address, quantity } => {
            if !map.has_all_coils(*address, *quantity) {
                return build_exception_pdu(fc, 0x02);
            }
            build_response_pdu(fc, &ResponseData::ReadBits(map.read_coils(*address, *quantity)))
        }
        ModbusRequest::ReadDiscreteInputs { address, quantity } => {
            if !map.has_all_discrete_inputs(*address, *quantity) {
                return build_exception_pdu(fc, 0x02);
            }
            build_response_pdu(fc, &ResponseData::ReadBits(map.read_discrete_inputs(*address, *quantity)))
        }
        ModbusRequest::ReadHoldingRegisters { address, quantity } => {
            if !map.has_all_holding_registers(*address, *quantity) {
                return build_exception_pdu(fc, 0x02);
            }
            build_response_pdu(fc, &ResponseData::ReadRegisters(map.read_holding_registers(*address, *quantity)))
        }
        ModbusRequest::ReadInputRegisters { address, quantity } => {
            if !map.has_all_input_registers(*address, *quantity) {
                return build_exception_pdu(fc, 0x02);
            }
            build_response_pdu(fc, &ResponseData::ReadRegisters(map.read_input_registers(*address, *quantity)))
        }
        _ => build_exception_pdu(fc, 0x01),
    }
}

fn execute_write(map: &mut RegisterMap, fc: u8, request: &ModbusRequest) -> Vec<u8> {
    match request {
        ModbusRequest::WriteSingleCoil { address, value } => {
            if !map.has_coil(*address) {
                return build_exception_pdu(fc, 0x02);
            }
            map.write_coil(*address, *value);
            map.discrete_inputs.insert(*address, *value);
            build_response_pdu(fc, &ResponseData::WriteSingleCoil { address: *address, value: *value })
        }
        ModbusRequest::WriteSingleRegister { address, value } => {
            if !map.has_holding_register(*address) {
                return build_exception_pdu(fc, 0x02);
            }
            map.write_holding_register(*address, *value);
            map.input_registers.insert(*address, *value);
            build_response_pdu(fc, &ResponseData::WriteSingleRegister { address: *address, value: *value })
        }
        ModbusRequest::WriteMultipleCoils { address, values } => {
            let qty = values.len() as u16;
            if !map.has_all_coils(*address, qty) {
                return build_exception_pdu(fc, 0x02);
            }
            map.write_coils(*address, values);
            for (i, &v) in values.iter().enumerate() {
                map.discrete_inputs.insert(*address + i as u16, v);
            }
            build_response_pdu(fc, &ResponseData::WriteMultiple { address: *address, quantity: qty })
        }
        ModbusRequest::WriteMultipleRegisters { address, values } => {
            let qty = values.len() as u16;
            if !map.has_all_holding_registers(*address, qty) {
                return build_exception_pdu(fc, 0x02);
            }
            map.write_holding_registers(*address, values);
            for (i, &v) in values.iter().enumerate() {
                map.input_registers.insert(*address + i as u16, v);
            }
            build_response_pdu(fc, &ResponseData::WriteMultiple { address: *address, quantity: qty })
        }
        _ => build_exception_pdu(fc, 0x01),
    }
}

/// Run TLS slave server. Called from `SlaveConnection::start()` for `Transport::TcpTls`.
pub async fn run_tls_slave(
    addr: SocketAddr,
    tls_config: SlaveTlsConfig,
    devices: SharedDevices,
    log_collector: SharedLogCollector,
    shutdown_rx: oneshot::Receiver<()>,
) -> Result<(), String> {
    let acceptor = Arc::new(build_tls_acceptor(&tls_config)?);

    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| format!("failed to bind {addr}: {e}"))?;

    log_tls_event(&log_collector, Direction::Tx, &format!("TLS slave listening on {}", addr));

    let shutdown_flag = Arc::new(std::sync::Mutex::new(false));
    let mut shutdown_rx = shutdown_rx;

    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((tcp_stream, peer_addr)) => {
                        let acceptor = acceptor.clone();
                        let devices = devices.clone();
                        let log_collector = log_collector.clone();
                        let shutdown_flag = shutdown_flag.clone();

                        // Convert async TcpStream to std TcpStream for native-tls
                        let std_stream = tcp_stream.into_std()
                            .map_err(|e| format!("failed to convert stream: {e}"))?;

                        tokio::task::spawn_blocking(move || {
                            // TLS handshake
                            match acceptor.accept(std_stream) {
                                Ok(tls_stream) => {
                                    log_tls_event(&log_collector, Direction::Rx,
                                        &format!("TLS handshake OK: {}", peer_addr));
                                    handle_client(tls_stream, peer_addr, devices, log_collector, shutdown_flag);
                                }
                                Err(e) => {
                                    log_tls_event(&log_collector, Direction::Rx,
                                        &format!("TLS handshake failed from {}: {}", peer_addr, e));
                                }
                            }
                        });
                    }
                    Err(e) => {
                        log::error!("TLS slave accept error: {}", e);
                    }
                }
            }
            _ = &mut shutdown_rx => {
                *shutdown_flag.lock().unwrap() = true;
                log_tls_event(&log_collector, Direction::Tx, "TLS slave shutting down");
                break;
            }
        }
    }

    Ok(())
}
```

- [ ] **Step 2: Add SlaveTlsConfig field and TcpTls branch to SlaveConnection**

In `crates/modbussim-core/src/slave.rs`:

Add the import at the top:
```rust
use crate::transport::SlaveTlsConfig;
```

Add a field to `SlaveConnection`:
```rust
pub struct SlaveConnection {
    pub transport: Transport,
    pub tls_config: SlaveTlsConfig,
    pub devices: SharedDevices,
    // ... rest unchanged
}
```

Update the `new()` constructor:
```rust
pub fn new(transport: Transport) -> Self {
    Self {
        transport,
        tls_config: SlaveTlsConfig::default(),
        devices: Arc::new(RwLock::new(HashMap::new())),
        log_collector: None,
        state: ConnectionState::Stopped,
        shutdown_tx: None,
        server_handle: None,
    }
}
```

Add a builder method after `with_log_collector`:
```rust
pub fn with_tls_config(mut self, config: SlaveTlsConfig) -> Self {
    self.tls_config = config;
    self
}
```

Replace the `Transport::TcpTls` placeholder in `start()` with:

```rust
Transport::TcpTls { host, port } => {
    let addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .map_err(|e| SlaveError::BindError(format!("Invalid address: {e}")))?;
    let tls_config = self.tls_config.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::tls_slave::run_tls_slave(
            addr, tls_config, devices, log_collector, shutdown_rx,
        ).await {
            log::error!("TLS slave error: {}", e);
        }
    })
}
```

Also add `TlsError` and `CertError` variants to `SlaveError`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum SlaveError {
    // ... existing variants ...
    #[error("TLS error: {0}")]
    TlsError(String),
    #[error("certificate error: {0}")]
    CertError(String),
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p modbussim-core`
Expected: Compiles successfully.

- [ ] **Step 4: Run existing tests to confirm no regression**

Run: `cargo test -p modbussim-core`
Expected: All existing tests still pass.

- [ ] **Step 5: Commit**

```bash
git add crates/modbussim-core/src/tls_slave.rs crates/modbussim-core/src/slave.rs
git commit -m "feat: implement TLS slave server with MBAP framing"
```

---

### Task 5: Implement TLS Master Client

**Files:**
- Create: `crates/modbussim-core/src/tls_master.rs`
- Modify: `crates/modbussim-core/src/master.rs`

- [ ] **Step 1: Implement tls_master.rs**

Replace the contents of `crates/modbussim-core/src/tls_master.rs` with:

```rust
//! TLS-enabled Modbus TCP master client.

use crate::log_collector::LogCollector;
use crate::log_entry::{Direction, FunctionCode, LogEntry};
use crate::master::{MasterError, ReadFunction, ReadResult};
use crate::mbap;
use crate::pdu::{parse_request_pdu, ModbusRequest};
use crate::transport::TlsConfig;
use native_tls::{Certificate, Identity, Protocol, TlsConnector};
use std::io::{Read, Write};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// A TLS master connection wrapping a native_tls TlsStream.
pub struct TlsMasterConnection {
    stream: Arc<Mutex<native_tls::TlsStream<std::net::TcpStream>>>,
    next_transaction_id: Arc<Mutex<u16>>,
}

impl TlsMasterConnection {
    pub fn new(stream: native_tls::TlsStream<std::net::TcpStream>) -> Self {
        Self {
            stream: Arc::new(Mutex::new(stream)),
            next_transaction_id: Arc::new(Mutex::new(1)),
        }
    }

    async fn next_tid(&self) -> u16 {
        let mut tid = self.next_transaction_id.lock().await;
        let current = *tid;
        *tid = tid.wrapping_add(1);
        if *tid == 0 { *tid = 1; }
        current
    }

    pub async fn read(
        &self,
        slave_id: u8,
        function: ReadFunction,
        start_address: u16,
        quantity: u16,
        timeout: Duration,
    ) -> Result<ReadResult, MasterError> {
        let fc_byte = match function {
            ReadFunction::ReadCoils => 0x01,
            ReadFunction::ReadDiscreteInputs => 0x02,
            ReadFunction::ReadHoldingRegisters => 0x03,
            ReadFunction::ReadInputRegisters => 0x04,
        };

        let mut pdu = vec![fc_byte];
        pdu.extend_from_slice(&start_address.to_be_bytes());
        pdu.extend_from_slice(&quantity.to_be_bytes());

        let tid = self.next_tid().await;
        let resp_pdu = self.send_receive(tid, slave_id, &pdu, timeout).await?;

        // Check for exception
        if !resp_pdu.is_empty() && (resp_pdu[0] & 0x80) != 0 {
            let exc = if resp_pdu.len() > 1 { resp_pdu[1] } else { 0 };
            return Err(MasterError::Transport(format!("exception 0x{:02X}", exc)));
        }

        parse_read_response(function, &resp_pdu, quantity)
    }

    pub async fn write_single_coil(
        &self,
        slave_id: u8,
        address: u16,
        value: bool,
        timeout: Duration,
    ) -> Result<(), MasterError> {
        let coil_val: u16 = if value { 0xFF00 } else { 0x0000 };
        let mut pdu = vec![0x05];
        pdu.extend_from_slice(&address.to_be_bytes());
        pdu.extend_from_slice(&coil_val.to_be_bytes());

        let tid = self.next_tid().await;
        let resp = self.send_receive(tid, slave_id, &pdu, timeout).await?;
        check_exception(&resp)?;
        Ok(())
    }

    pub async fn write_single_register(
        &self,
        slave_id: u8,
        address: u16,
        value: u16,
        timeout: Duration,
    ) -> Result<(), MasterError> {
        let mut pdu = vec![0x06];
        pdu.extend_from_slice(&address.to_be_bytes());
        pdu.extend_from_slice(&value.to_be_bytes());

        let tid = self.next_tid().await;
        let resp = self.send_receive(tid, slave_id, &pdu, timeout).await?;
        check_exception(&resp)?;
        Ok(())
    }

    pub async fn write_multiple_coils(
        &self,
        slave_id: u8,
        address: u16,
        values: &[bool],
        timeout: Duration,
    ) -> Result<(), MasterError> {
        let quantity = values.len() as u16;
        let byte_count = (values.len() + 7) / 8;
        let mut coil_bytes = vec![0u8; byte_count];
        for (i, &v) in values.iter().enumerate() {
            if v { coil_bytes[i / 8] |= 1 << (i % 8); }
        }
        let mut pdu = vec![0x0F];
        pdu.extend_from_slice(&address.to_be_bytes());
        pdu.extend_from_slice(&quantity.to_be_bytes());
        pdu.push(byte_count as u8);
        pdu.extend_from_slice(&coil_bytes);

        let tid = self.next_tid().await;
        let resp = self.send_receive(tid, slave_id, &pdu, timeout).await?;
        check_exception(&resp)?;
        Ok(())
    }

    pub async fn write_multiple_registers(
        &self,
        slave_id: u8,
        address: u16,
        values: &[u16],
        timeout: Duration,
    ) -> Result<(), MasterError> {
        let quantity = values.len() as u16;
        let byte_count = (values.len() * 2) as u8;
        let mut pdu = vec![0x10];
        pdu.extend_from_slice(&address.to_be_bytes());
        pdu.extend_from_slice(&quantity.to_be_bytes());
        pdu.push(byte_count);
        for v in values {
            pdu.extend_from_slice(&v.to_be_bytes());
        }

        let tid = self.next_tid().await;
        let resp = self.send_receive(tid, slave_id, &pdu, timeout).await?;
        check_exception(&resp)?;
        Ok(())
    }

    async fn send_receive(
        &self,
        transaction_id: u16,
        unit_id: u8,
        pdu: &[u8],
        timeout: Duration,
    ) -> Result<Vec<u8>, MasterError> {
        let stream = self.stream.clone();
        let pdu = pdu.to_vec();

        tokio::task::spawn_blocking(move || {
            let mut stream = stream.blocking_lock();
            // Set timeouts
            if let Ok(inner) = stream.get_ref().try_clone() {
                let _ = inner.set_write_timeout(Some(timeout));
                let _ = inner.set_read_timeout(Some(timeout));
            }
            mbap::write_frame(&mut *stream, transaction_id, unit_id, &pdu)
                .map_err(|e| MasterError::Transport(format!("write failed: {e}")))?;
            let (_header, resp_pdu) = mbap::read_frame(&mut *stream)
                .map_err(|e| MasterError::Transport(format!("read failed: {e}")))?;
            Ok(resp_pdu)
        })
        .await
        .map_err(|e| MasterError::Transport(format!("task join error: {e}")))?
    }
}

fn check_exception(pdu: &[u8]) -> Result<(), MasterError> {
    if !pdu.is_empty() && (pdu[0] & 0x80) != 0 {
        let exc = if pdu.len() > 1 { pdu[1] } else { 0 };
        return Err(MasterError::Transport(format!("exception 0x{:02X}", exc)));
    }
    Ok(())
}

fn parse_read_response(function: ReadFunction, pdu: &[u8], quantity: u16) -> Result<ReadResult, MasterError> {
    if pdu.len() < 2 {
        return Err(MasterError::Transport("response too short".into()));
    }
    let byte_count = pdu[1] as usize;
    let data = &pdu[2..];
    if data.len() < byte_count {
        return Err(MasterError::Transport("response data too short".into()));
    }

    match function {
        ReadFunction::ReadCoils => {
            let mut bits = Vec::with_capacity(quantity as usize);
            for i in 0..quantity as usize {
                let byte_idx = i / 8;
                let bit_idx = i % 8;
                bits.push(if byte_idx < data.len() { (data[byte_idx] >> bit_idx) & 1 == 1 } else { false });
            }
            Ok(ReadResult::Coils(bits))
        }
        ReadFunction::ReadDiscreteInputs => {
            let mut bits = Vec::with_capacity(quantity as usize);
            for i in 0..quantity as usize {
                let byte_idx = i / 8;
                let bit_idx = i % 8;
                bits.push(if byte_idx < data.len() { (data[byte_idx] >> bit_idx) & 1 == 1 } else { false });
            }
            Ok(ReadResult::DiscreteInputs(bits))
        }
        ReadFunction::ReadHoldingRegisters => {
            let mut regs = Vec::with_capacity(quantity as usize);
            for i in 0..quantity as usize {
                if i * 2 + 1 < data.len() {
                    regs.push(u16::from_be_bytes([data[i * 2], data[i * 2 + 1]]));
                }
            }
            Ok(ReadResult::HoldingRegisters(regs))
        }
        ReadFunction::ReadInputRegisters => {
            let mut regs = Vec::with_capacity(quantity as usize);
            for i in 0..quantity as usize {
                if i * 2 + 1 < data.len() {
                    regs.push(u16::from_be_bytes([data[i * 2], data[i * 2 + 1]]));
                }
            }
            Ok(ReadResult::InputRegisters(regs))
        }
    }
}

pub fn build_tls_connector(config: &TlsConfig) -> Result<TlsConnector, String> {
    let mut builder = TlsConnector::builder();
    builder.min_protocol_version(Some(Protocol::Tlsv12));

    if !config.ca_file.is_empty() {
        let ca_pem = std::fs::read(&config.ca_file)
            .map_err(|e| format!("failed to read CA file: {e}"))?;
        let ca_cert = Certificate::from_pem(&ca_pem)
            .map_err(|e| format!("invalid CA cert: {e}"))?;
        builder.add_root_certificate(ca_cert);
    }

    if !config.pkcs12_file.is_empty() {
        let p12 = std::fs::read(&config.pkcs12_file)
            .map_err(|e| format!("failed to read PKCS#12: {e}"))?;
        let identity = Identity::from_pkcs12(&p12, &config.pkcs12_password)
            .map_err(|e| format!("invalid PKCS#12: {e}"))?;
        builder.identity(identity);
    } else if !config.cert_file.is_empty() && !config.key_file.is_empty() {
        let cert = std::fs::read(&config.cert_file)
            .map_err(|e| format!("failed to read cert: {e}"))?;
        let key = std::fs::read(&config.key_file)
            .map_err(|e| format!("failed to read key: {e}"))?;
        let identity = Identity::from_pkcs8(&cert, &key)
            .map_err(|e| format!("invalid PEM identity: {e}"))?;
        builder.identity(identity);
    }

    if config.accept_invalid_certs {
        builder.danger_accept_invalid_certs(true);
        builder.danger_accept_invalid_hostnames(true);
    }

    builder.build().map_err(|e| format!("failed to build TLS connector: {e}"))
}

/// Connect to a TLS-enabled Modbus slave. Returns a TlsMasterConnection.
pub async fn connect_tls(
    addr: &str,
    port: u16,
    tls_config: &TlsConfig,
    timeout: Duration,
) -> Result<TlsMasterConnection, MasterError> {
    let connector = build_tls_connector(tls_config)
        .map_err(|e| MasterError::ConnectionFailed(e))?;

    let socket_addr: std::net::SocketAddr = format!("{}:{}", addr, port)
        .parse()
        .map_err(|e| MasterError::ConnectionFailed(format!("invalid address: {e}")))?;

    let tcp_stream = tokio::time::timeout(
        timeout,
        tokio::net::TcpStream::connect(socket_addr),
    )
    .await
    .map_err(|_| MasterError::Timeout("TLS connection timed out".into()))?
    .map_err(|e| MasterError::ConnectionFailed(format!("{e}")))?;

    let std_stream = tcp_stream.into_std()
        .map_err(|e| MasterError::ConnectionFailed(format!("stream conversion: {e}")))?;

    let domain = addr.to_string();
    let tls_stream = tokio::task::spawn_blocking(move || {
        connector.connect(&domain, std_stream)
            .map_err(|e| MasterError::ConnectionFailed(format!("TLS handshake: {e}")))
    })
    .await
    .map_err(|e| MasterError::ConnectionFailed(format!("task join: {e}")))?
    ?;

    Ok(TlsMasterConnection::new(tls_stream))
}
```

- [ ] **Step 2: Integrate TLS into MasterConnection**

In `crates/modbussim-core/src/master.rs`:

Add import at the top:
```rust
use crate::tls_master::TlsMasterConnection;
use crate::transport::TlsConfig;
```

Add `TlsConfig` field to `MasterConfig`:
```rust
pub struct MasterConfig {
    pub target_address: String,
    pub port: u16,
    pub slave_id: u8,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub tls: TlsConfig,
}
```

Update the `Default` impl:
```rust
impl Default for MasterConfig {
    fn default() -> Self {
        Self {
            target_address: "127.0.0.1".to_string(),
            port: 502,
            slave_id: 1,
            timeout_ms: default_timeout_ms(),
            tls: TlsConfig::default(),
        }
    }
}
```

Add `TcpTls` variant to `TransportCtx`:
```rust
enum TransportCtx {
    Tcp(Arc<Mutex<client::Context>>),
    Rtu(Arc<RtuMasterTransport>),
    Ascii(Arc<AsciiMasterTransport>),
    RtuTcp(Arc<RtuTcpMasterTransport>),
    TcpTls(Arc<TlsMasterConnection>),
}
```

Replace the `Transport::TcpTls` placeholder in `connect()` with:
```rust
Transport::TcpTls { host, port } => {
    let tls_conn = crate::tls_master::connect_tls(
        host, *port, &self.config.tls, timeout,
    ).await?;
    TransportCtx::TcpTls(Arc::new(tls_conn))
}
```

In the `read()` method, add a `TransportCtx::TcpTls` arm (before the result log):
Find the `execute_read_any` call and add TLS handling. The simplest approach is to handle TLS before the generic call:

After `let transport_ctx = self.get_transport_ctx()?;`, add:
```rust
if let TransportCtx::TcpTls(ref tls) = transport_ctx {
    let result = tls.read(self.config.slave_id, function, start_address, quantity, timeout).await?;
    let detail = match &result {
        ReadResult::Coils(vals) => format!("{:?}", vals),
        ReadResult::DiscreteInputs(vals) => format!("{:?}", vals),
        ReadResult::HoldingRegisters(vals) => format!("{:?}", vals),
        ReadResult::InputRegisters(vals) => format!("{:?}", vals),
    };
    self.log_rx(fc, &detail).await;
    return Ok(result);
}
```

Similarly, in each write method (`write_single_coil`, `write_single_register`, `write_multiple_coils`, `write_multiple_registers`), add a `TransportCtx::TcpTls` match arm alongside the existing `TransportCtx::Tcp` arm. For example in `write_single_coil`:

```rust
TransportCtx::TcpTls(tls) => {
    tls.write_single_coil(self.config.slave_id, address, value, timeout).await
        .map_err(|e| MasterError::Transport(format!("{e}")))?;
}
```

And similarly for the other write methods, using `tls.write_single_register(...)`, `tls.write_multiple_coils(...)`, `tls.write_multiple_registers(...)`.

In `disconnect()`, add handling for TLS (it just drops):
```rust
if let TransportCtx::Tcp(tcp_ctx) = ctx {
    let mut ctx = tcp_ctx.lock().await;
    let _ = ctx.disconnect().await;
}
// TLS, RTU, ASCII, RtuTcp: transport is dropped, closing the connection.
```

**Critical:** Update `execute_read_any()` (around line 768) to handle TLS. Add a `TransportCtx::TcpTls` arm:

```rust
async fn execute_read_any(
    ctx: &TransportCtx,
    slave_id: u8,
    function: ReadFunction,
    start_address: u16,
    quantity: u16,
    timeout: Duration,
) -> Result<ReadResult, MasterError> {
    match ctx {
        TransportCtx::Tcp(tcp_ctx) => {
            execute_read_tcp(tcp_ctx, function, start_address, quantity, timeout).await
        }
        TransportCtx::TcpTls(tls) => {
            tls.read(slave_id, function, start_address, quantity, timeout).await
        }
        other => {
            let pdu = build_read_pdu(function, start_address, quantity);
            let resp = send_pdu_via_transport(other, slave_id, &pdu, timeout).await?;
            parse_read_response_pdu(function, &resp)
        }
    }
}
```

Also update `send_pdu_via_transport()` (around line 666) to handle the TLS variant so it doesn't panic if called:

```rust
TransportCtx::TcpTls(_) => Err(MasterError::Transport(
    "send_pdu_via_transport called for TLS".into(),
)),
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p modbussim-core`
Expected: Compiles successfully.

- [ ] **Step 4: Run existing tests**

Run: `cargo test -p modbussim-core`
Expected: All existing tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/modbussim-core/src/tls_master.rs crates/modbussim-core/src/master.rs
git commit -m "feat: implement TLS master client with MBAP framing"
```

---

### Task 6: TLS E2E Integration Tests

**Files:**
- Create: `crates/modbussim-core/tests/tls_e2e.rs`

- [ ] **Step 1: Write TLS E2E tests**

Create `crates/modbussim-core/tests/tls_e2e.rs`:

```rust
//! End-to-end TLS tests using dynamically generated certificates.

use modbussim_core::master::{MasterConfig, MasterConnection, ReadFunction};
use modbussim_core::slave::{SlaveConnection, SlaveDevice};
use modbussim_core::transport::{SlaveTlsConfig, TlsConfig, Transport};
use rcgen::{BasicConstraints, CertificateParams, IsCa, KeyPair};
use std::io::Write;
use tempfile::NamedTempFile;

/// Generate a self-signed CA, server cert, and client cert.
/// Returns (ca_cert_pem, server_p12_bytes, client_p12_bytes).
fn generate_test_certs() -> (String, Vec<u8>, Vec<u8>) {
    // CA
    let ca_key = KeyPair::generate().unwrap();
    let mut ca_params = CertificateParams::new(vec!["Test CA".to_string()]).unwrap();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let ca_cert = ca_params.self_signed(&ca_key).unwrap();
    let ca_pem = ca_cert.pem();

    // Server cert signed by CA
    let server_key = KeyPair::generate().unwrap();
    let server_params = CertificateParams::new(vec!["localhost".to_string()]).unwrap();
    let server_cert = server_params.signed_by(&server_key, &ca_cert, &ca_key).unwrap();

    // Client cert signed by CA
    let client_key = KeyPair::generate().unwrap();
    let client_params = CertificateParams::new(vec!["test-client".to_string()]).unwrap();
    let client_cert = client_params.signed_by(&client_key, &ca_cert, &ca_key).unwrap();

    // Create PKCS#12 bundles using openssl crate
    let server_p12 = make_pkcs12(
        &server_cert.pem(),
        &server_key.serialize_pem(),
        &ca_pem,
        "server",
    );
    let client_p12 = make_pkcs12(
        &client_cert.pem(),
        &client_key.serialize_pem(),
        &ca_pem,
        "client",
    );

    (ca_pem, server_p12, client_p12)
}

fn make_pkcs12(cert_pem: &str, key_pem: &str, ca_pem: &str, password: &str) -> Vec<u8> {
    let cert = openssl::x509::X509::from_pem(cert_pem.as_bytes()).unwrap();
    let key = openssl::pkey::PKey::private_key_from_pem(key_pem.as_bytes()).unwrap();
    let ca = openssl::x509::X509::from_pem(ca_pem.as_bytes()).unwrap();
    let mut ca_stack = openssl::stack::Stack::new().unwrap();
    ca_stack.push(ca).unwrap();
    let p12 = openssl::pkcs12::Pkcs12::builder()
        .name("test")
        .pkey(&key)
        .cert(&cert)
        .ca(ca_stack)
        .build2(password)
        .unwrap();
    p12.to_der().unwrap()
}

fn write_temp_file(data: &[u8]) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(data).unwrap();
    f.flush().unwrap();
    f
}

#[tokio::test]
async fn test_tls_read_holding_registers() {
    let (ca_pem, server_p12, _client_p12) = generate_test_certs();

    let ca_file = write_temp_file(ca_pem.as_bytes());
    let server_p12_file = write_temp_file(&server_p12);

    let port = 15802u16; // Use high port to avoid conflicts

    // Start TLS slave
    let mut slave = SlaveConnection::new(Transport::TcpTls {
        host: "127.0.0.1".to_string(),
        port,
    });
    slave.tls_config = SlaveTlsConfig {
        enabled: true,
        pkcs12_file: server_p12_file.path().to_str().unwrap().to_string(),
        pkcs12_password: "server".to_string(),
        require_client_cert: false,
        ..Default::default()
    };

    let device = SlaveDevice::with_default_registers(1, "TLS Test", 10);
    slave.add_device(device).await.unwrap();
    slave.start().await.unwrap();

    // Give server time to bind
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Connect TLS master
    let config = MasterConfig {
        target_address: "127.0.0.1".to_string(),
        port,
        slave_id: 1,
        timeout_ms: 5000,
        tls: TlsConfig {
            enabled: true,
            ca_file: ca_file.path().to_str().unwrap().to_string(),
            accept_invalid_certs: false,
            ..Default::default()
        },
    };

    let mut master = MasterConnection::new(
        config,
        Transport::TcpTls {
            host: "127.0.0.1".to_string(),
            port,
        },
    );
    master.connect().await.unwrap();

    // Read holding registers
    let result = master
        .read(ReadFunction::ReadHoldingRegisters, 0, 5)
        .await
        .unwrap();
    match result {
        modbussim_core::master::ReadResult::HoldingRegisters(vals) => {
            assert_eq!(vals.len(), 5);
            assert_eq!(vals, vec![0, 0, 0, 0, 0]);
        }
        _ => panic!("expected HoldingRegisters"),
    }

    // Write and read back
    master.write_single_register(0, 42).await.unwrap();
    let result = master
        .read(ReadFunction::ReadHoldingRegisters, 0, 1)
        .await
        .unwrap();
    match result {
        modbussim_core::master::ReadResult::HoldingRegisters(vals) => {
            assert_eq!(vals, vec![42]);
        }
        _ => panic!("expected HoldingRegisters"),
    }

    master.disconnect().await.unwrap();
    slave.stop().await.unwrap();
}

#[tokio::test]
async fn test_tls_accept_invalid_certs() {
    let (_ca_pem, server_p12, _client_p12) = generate_test_certs();
    let server_p12_file = write_temp_file(&server_p12);

    let port = 15803u16;

    let mut slave = SlaveConnection::new(Transport::TcpTls {
        host: "127.0.0.1".to_string(),
        port,
    });
    slave.tls_config = SlaveTlsConfig {
        enabled: true,
        pkcs12_file: server_p12_file.path().to_str().unwrap().to_string(),
        pkcs12_password: "server".to_string(),
        ..Default::default()
    };

    let device = SlaveDevice::with_default_registers(1, "Test", 10);
    slave.add_device(device).await.unwrap();
    slave.start().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Connect without CA but with accept_invalid_certs
    let config = MasterConfig {
        target_address: "127.0.0.1".to_string(),
        port,
        slave_id: 1,
        timeout_ms: 5000,
        tls: TlsConfig {
            enabled: true,
            accept_invalid_certs: true,
            ..Default::default()
        },
    };

    let mut master = MasterConnection::new(
        config,
        Transport::TcpTls { host: "127.0.0.1".to_string(), port },
    );
    master.connect().await.unwrap();

    let result = master.read(ReadFunction::ReadHoldingRegisters, 0, 1).await.unwrap();
    match result {
        modbussim_core::master::ReadResult::HoldingRegisters(vals) => {
            assert_eq!(vals.len(), 1);
        }
        _ => panic!("expected HoldingRegisters"),
    }

    master.disconnect().await.unwrap();
    slave.stop().await.unwrap();
}
```

- [ ] **Step 2: Run E2E tests**

Run: `cargo test -p modbussim-core --test tls_e2e -- --nocapture`
Expected: Both tests PASS. (Note: on CI, OpenSSL dev headers must be available on Linux.)

- [ ] **Step 3: Commit**

```bash
git add crates/modbussim-core/tests/tls_e2e.rs
git commit -m "test: add TLS E2E integration tests"
```

---

### Task 7: Update Tauri Commands for TLS

**Files:**
- Modify: `crates/modbussim-app/src/commands.rs`
- Modify: `crates/modbusmaster-app/src/commands.rs`

- [ ] **Step 1: Update Slave Tauri commands**

In `crates/modbussim-app/src/commands.rs`:

Add import:
```rust
use modbussim_core::transport::SlaveTlsConfig;
```

Add TLS fields to `CreateSlaveRequest`:
```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateSlaveRequest {
    pub transport: TransportRequest,
    pub init_mode: Option<String>,
    pub use_tls: Option<bool>,
    pub cert_file: Option<String>,
    pub key_file: Option<String>,
    pub ca_file: Option<String>,
    pub require_client_cert: Option<bool>,
    pub pkcs12_file: Option<String>,
    pub pkcs12_password: Option<String>,
}
```

In `create_slave_connection()`, after `let connection = connection.with_log_collector(...)`, add TLS config:

```rust
let connection = if request.use_tls.unwrap_or(false) {
    connection.with_tls_config(SlaveTlsConfig {
        enabled: true,
        cert_file: request.cert_file.unwrap_or_default(),
        key_file: request.key_file.unwrap_or_default(),
        ca_file: request.ca_file.unwrap_or_default(),
        require_client_cert: request.require_client_cert.unwrap_or(false),
        pkcs12_file: request.pkcs12_file.unwrap_or_default(),
        pkcs12_password: request.pkcs12_password.unwrap_or_default(),
    })
} else {
    connection
};
```

- [ ] **Step 2: Update Master Tauri commands**

In `crates/modbusmaster-app/src/commands.rs`:

Add import:
```rust
use modbussim_core::transport::TlsConfig;
```

Add TLS fields to `CreateMasterRequest`:
```rust
#[derive(Debug, Deserialize)]
pub struct CreateMasterRequest {
    pub transport: TransportRequest,
    pub slave_id: u8,
    pub timeout_ms: Option<u64>,
    pub use_tls: Option<bool>,
    pub ca_file: Option<String>,
    pub cert_file: Option<String>,
    pub key_file: Option<String>,
    pub pkcs12_file: Option<String>,
    pub pkcs12_password: Option<String>,
    pub accept_invalid_certs: Option<bool>,
}
```

In `create_master_connection()`, update the `MasterConfig` construction to include TLS:

```rust
let config = MasterConfig {
    target_address: target_address.clone(),
    port,
    slave_id: request.slave_id,
    timeout_ms: request.timeout_ms.unwrap_or(3000),
    tls: TlsConfig {
        enabled: request.use_tls.unwrap_or(false),
        ca_file: request.ca_file.unwrap_or_default(),
        cert_file: request.cert_file.unwrap_or_default(),
        key_file: request.key_file.unwrap_or_default(),
        pkcs12_file: request.pkcs12_file.unwrap_or_default(),
        pkcs12_password: request.pkcs12_password.unwrap_or_default(),
        accept_invalid_certs: request.accept_invalid_certs.unwrap_or(false),
    },
};
```

- [ ] **Step 3: Verify compilation of both apps**

Run: `cargo check -p modbussim-app && cargo check -p modbusmaster-app`
Expected: Both compile successfully.

- [ ] **Step 4: Commit**

```bash
git add crates/modbussim-app/src/commands.rs crates/modbusmaster-app/src/commands.rs
git commit -m "feat: add TLS parameters to Tauri commands"
```

---

### Task 8: Slave Frontend TLS UI

**Files:**
- Modify: `frontend/src/components/Toolbar.vue`

- [ ] **Step 1: Add TLS state variables**

In the `<script setup>` section of `frontend/src/components/Toolbar.vue`, after the existing `parityMode` ref declarations (around line 67), add:

```typescript
const useTls = ref(false)
const tlsCertFile = ref('')
const tlsKeyFile = ref('')
const tlsCaFile = ref('')
const tlsRequireClientCert = ref(false)
const tlsPkcs12File = ref('')
const tlsPkcs12Password = ref('')
```

- [ ] **Step 2: Reset TLS state in openNewConnModal**

In the `openNewConnModal()` function, add resets:

```typescript
useTls.value = false
tlsCertFile.value = ''
tlsKeyFile.value = ''
tlsCaFile.value = ''
tlsRequireClientCert.value = false
tlsPkcs12File.value = ''
tlsPkcs12Password.value = ''
```

- [ ] **Step 3: Pass TLS params in submitNewConnection**

In `submitNewConnection()`, when `useTls.value` is true, change the transport type to `tcp_tls` and include TLS fields in the request:

Replace the existing `invoke('create_slave_connection', ...)` call with:

```typescript
if (useTls.value && newConnTransport.value === 'tcp') {
    transport = { type: 'tcp_tls', port }
}

await invoke('create_slave_connection', {
    request: {
        transport,
        init_mode: newConnInitMode.value,
        ...(useTls.value ? {
            use_tls: true,
            cert_file: tlsCertFile.value || undefined,
            key_file: tlsKeyFile.value || undefined,
            ca_file: tlsCaFile.value || undefined,
            require_client_cert: tlsRequireClientCert.value || undefined,
            pkcs12_file: tlsPkcs12File.value || undefined,
            pkcs12_password: tlsPkcs12Password.value || undefined,
        } : {}),
    }
})
```

- [ ] **Step 4: Add TLS UI elements to the New Connection modal template**

In the `<template>` section, inside the New Connection modal, after the TCP port field block (`<template v-if="newConnTransport === 'tcp' || newConnTransport === 'rtu_over_tcp'">`), add:

```html
<template v-if="newConnTransport === 'tcp'">
    <div class="modal-field">
        <label>
            <input type="checkbox" v-model="useTls" /> 启用 TLS
        </label>
    </div>
    <template v-if="useTls">
        <div class="modal-field">
            <label>服务器证书 (PEM)</label>
            <div style="display: flex; gap: 4px;">
                <input v-model="tlsCertFile" type="text" placeholder="证书文件路径" style="flex: 1;" />
                <button class="tool-btn" @click="pickFile('cert')" style="padding: 4px 8px;">...</button>
            </div>
        </div>
        <div class="modal-field">
            <label>服务器私钥 (PEM)</label>
            <div style="display: flex; gap: 4px;">
                <input v-model="tlsKeyFile" type="text" placeholder="私钥文件路径" style="flex: 1;" />
                <button class="tool-btn" @click="pickFile('key')" style="padding: 4px 8px;">...</button>
            </div>
        </div>
        <div class="modal-field">
            <label>PKCS#12 文件</label>
            <div style="display: flex; gap: 4px;">
                <input v-model="tlsPkcs12File" type="text" placeholder="可选，优先于 PEM" style="flex: 1;" />
                <button class="tool-btn" @click="pickFile('pkcs12')" style="padding: 4px 8px;">...</button>
            </div>
        </div>
        <div class="modal-field" v-if="tlsPkcs12File">
            <label>PKCS#12 密码</label>
            <input v-model="tlsPkcs12Password" type="password" placeholder="密码" />
        </div>
        <div class="modal-field">
            <label>
                <input type="checkbox" v-model="tlsRequireClientCert" /> 要求客户端证书 (mTLS)
            </label>
        </div>
        <div class="modal-field" v-if="tlsRequireClientCert">
            <label>CA 证书 (验证客户端)</label>
            <div style="display: flex; gap: 4px;">
                <input v-model="tlsCaFile" type="text" placeholder="CA 证书路径" style="flex: 1;" />
                <button class="tool-btn" @click="pickFile('ca')" style="padding: 4px 8px;">...</button>
            </div>
        </div>
    </template>
</template>
```

- [ ] **Step 5: Add file picker helper function**

In the `<script setup>` section, add the file picker function:

```typescript
async function pickFile(target: 'cert' | 'key' | 'ca' | 'pkcs12') {
    try {
        const path = await open({
            filters: target === 'pkcs12'
                ? [{ name: 'PKCS#12', extensions: ['p12', 'pfx'] }]
                : [{ name: 'PEM Certificate', extensions: ['pem', 'crt', 'key'] }],
        })
        if (!path) return
        const p = path as string
        if (target === 'cert') tlsCertFile.value = p
        else if (target === 'key') tlsKeyFile.value = p
        else if (target === 'ca') tlsCaFile.value = p
        else if (target === 'pkcs12') tlsPkcs12File.value = p
    } catch (e) {
        await showAlert(String(e))
    }
}
```

- [ ] **Step 6: Verify frontend builds**

Run: `cd frontend && npm run build`
Expected: Build succeeds with no errors.

- [ ] **Step 7: Commit**

```bash
git add frontend/src/components/Toolbar.vue
git commit -m "feat: add TLS configuration UI to slave frontend"
```

---

### Task 9: Master Frontend TLS UI

**Files:**
- Modify: `master-frontend/src/components/Toolbar.vue`

- [ ] **Step 1: Add TLS state variables**

In the `<script setup>` section of `master-frontend/src/components/Toolbar.vue`, after the existing `serialPorts` ref (around line 70), add:

```typescript
const useTls = ref(false)
const tlsCaFile = ref('')
const tlsCertFile = ref('')
const tlsKeyFile = ref('')
const tlsPkcs12File = ref('')
const tlsPkcs12Password = ref('')
const tlsAcceptInvalidCerts = ref(false)
```

- [ ] **Step 2: Pass TLS params in createConnection**

In `createConnection()`, when building the transport and invoking the command:

If `useTls` is true and transport is TCP, change transport type to `tcp_tls`:

```typescript
if (useTls.value && newConnForm.value.transport === 'tcp') {
    transport = { type: 'tcp_tls', host: newConnForm.value.target_address, port: newConnForm.value.port }
}
```

Update the `invoke` call:
```typescript
await invoke('create_master_connection', {
    request: {
        transport,
        slave_id: newConnForm.value.slave_id,
        timeout_ms: newConnForm.value.timeout_ms,
        ...(useTls.value ? {
            use_tls: true,
            ca_file: tlsCaFile.value || undefined,
            cert_file: tlsCertFile.value || undefined,
            key_file: tlsKeyFile.value || undefined,
            pkcs12_file: tlsPkcs12File.value || undefined,
            pkcs12_password: tlsPkcs12Password.value || undefined,
            accept_invalid_certs: tlsAcceptInvalidCerts.value || undefined,
        } : {}),
    }
})
```

- [ ] **Step 3: Add TLS UI elements to the New Connection modal template**

In the `<template>` section, inside the New Connection modal, after the TCP port field (`<input v-model.number="newConnForm.port" ...>`'s parent label), add:

```html
<template v-if="newConnForm.transport === 'tcp'">
    <label class="form-label">
        <input type="checkbox" v-model="useTls" /> 启用 TLS
    </label>
    <template v-if="useTls">
        <label class="form-label">
            CA 证书 (验证服务器)
            <div style="display: flex; gap: 4px;">
                <input v-model="tlsCaFile" class="form-input" type="text" placeholder="CA 证书路径" style="flex: 1;" />
                <button class="tool-btn" @click="pickFile('ca')" style="padding: 4px 8px;">...</button>
            </div>
        </label>
        <label class="form-label">
            客户端证书 (PEM)
            <div style="display: flex; gap: 4px;">
                <input v-model="tlsCertFile" class="form-input" type="text" placeholder="可选，用于 mTLS" style="flex: 1;" />
                <button class="tool-btn" @click="pickFile('cert')" style="padding: 4px 8px;">...</button>
            </div>
        </label>
        <label class="form-label">
            客户端私钥 (PEM)
            <div style="display: flex; gap: 4px;">
                <input v-model="tlsKeyFile" class="form-input" type="text" placeholder="可选，用于 mTLS" style="flex: 1;" />
                <button class="tool-btn" @click="pickFile('key')" style="padding: 4px 8px;">...</button>
            </div>
        </label>
        <label class="form-label">
            PKCS#12 文件
            <div style="display: flex; gap: 4px;">
                <input v-model="tlsPkcs12File" class="form-input" type="text" placeholder="可选，优先于 PEM" style="flex: 1;" />
                <button class="tool-btn" @click="pickFile('pkcs12')" style="padding: 4px 8px;">...</button>
            </div>
        </label>
        <label class="form-label" v-if="tlsPkcs12File">
            PKCS#12 密码
            <input v-model="tlsPkcs12Password" class="form-input" type="password" placeholder="密码" />
        </label>
        <label class="form-label">
            <input type="checkbox" v-model="tlsAcceptInvalidCerts" /> 接受自签名证书 (测试用)
        </label>
    </template>
</template>
```

- [ ] **Step 4: Add file picker helper function**

In the `<script setup>` section, add:

```typescript
async function pickFile(target: 'cert' | 'key' | 'ca' | 'pkcs12') {
    try {
        const path = await open({
            filters: target === 'pkcs12'
                ? [{ name: 'PKCS#12', extensions: ['p12', 'pfx'] }]
                : [{ name: 'PEM Certificate', extensions: ['pem', 'crt', 'key'] }],
        })
        if (!path) return
        const p = path as string
        if (target === 'cert') tlsCertFile.value = p
        else if (target === 'key') tlsKeyFile.value = p
        else if (target === 'ca') tlsCaFile.value = p
        else if (target === 'pkcs12') tlsPkcs12File.value = p
    } catch (e) {
        await showAlert(String(e))
    }
}
```

- [ ] **Step 5: Verify frontend builds**

Run: `cd master-frontend && npm run build`
Expected: Build succeeds.

- [ ] **Step 6: Commit**

```bash
git add master-frontend/src/components/Toolbar.vue
git commit -m "feat: add TLS configuration UI to master frontend"
```

---

### Task 10: Final Verification

- [ ] **Step 1: Run all Rust tests**

Run: `cargo test --workspace`
Expected: All tests pass.

- [ ] **Step 2: Build both Tauri apps**

Run: `cargo build -p modbussim-app && cargo build -p modbusmaster-app`
Expected: Both compile successfully.

- [ ] **Step 3: Build both frontends**

Run: `cd frontend && npm run build && cd ../master-frontend && npm run build`
Expected: Both build successfully.

- [ ] **Step 4: Commit any remaining fixes**

If any fixes were needed, commit them:
```bash
git add -A
git commit -m "fix: resolve final compilation issues for TLS support"
```
