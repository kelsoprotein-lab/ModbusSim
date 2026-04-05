# Phase 2: Protocol & Transport Extension Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add RTU, ASCII, and RTU-over-TCP transport support to both master and slave apps, alongside the existing TCP transport, with serial port hardware I/O and a frontend transport selection UI.

**Architecture:** Create a `frame.rs` module for RTU/ASCII frame encode/decode (reusing existing CRC-16/LRC from `tools.rs`). Create a `transport.rs` module defining the `Transport` enum and serial config types. Refactor `SlaveConnection` and `MasterConnection` to accept a `Transport` parameter and dispatch to transport-specific I/O loops. The existing `tokio_modbus` TCP path is preserved unchanged; RTU/ASCII/RtuOverTcp paths use custom frame-level I/O over `tokio-serial` or raw TCP streams.

**Tech Stack:** Rust (`tokio-serial` for async serial I/O, `serialport` for port enumeration), Vue 3 + TypeScript (transport selection UI)

---

## File Structure

### New Files

| File | Responsibility |
|------|---------------|
| `crates/modbussim-core/src/frame.rs` | RTU and ASCII frame encode/decode functions |
| `crates/modbussim-core/src/transport.rs` | Transport enum, SerialConfig, serial port enumeration |
| `crates/modbussim-core/src/rtu_slave.rs` | RTU serial slave server loop |
| `crates/modbussim-core/src/ascii_slave.rs` | ASCII serial slave server loop |
| `crates/modbussim-core/src/rtu_tcp_slave.rs` | RTU-over-TCP slave server loop |
| `crates/modbussim-core/src/rtu_master.rs` | RTU serial master client |
| `crates/modbussim-core/src/ascii_master.rs` | ASCII serial master client |
| `crates/modbussim-core/src/rtu_tcp_master.rs` | RTU-over-TCP master client |
| `crates/modbussim-core/src/pdu.rs` | PDU (Protocol Data Unit) parsing and building — shared between all transports |

### Modified Files

| File | Changes |
|------|---------|
| `crates/modbussim-core/src/lib.rs` | Add new module declarations |
| `crates/modbussim-core/Cargo.toml` | Add `tokio-serial`, `serialport` dependencies |
| `crates/modbussim-core/src/slave.rs` | Refactor `start()` to dispatch on Transport; extract PDU processing |
| `crates/modbussim-core/src/master.rs` | Refactor `connect()` to dispatch on Transport |
| `crates/modbussim-core/src/project.rs` | Extend `TransportConfig` enum with Rtu/Ascii/RtuOverTcp |
| `crates/modbussim-app/src/commands.rs` | Accept transport param in `create_slave_connection` |
| `crates/modbusmaster-app/src/commands.rs` | Accept transport param in `create_master_connection` |
| `frontend/src/components/Toolbar.vue` | Transport type selector + serial config UI |
| `master-frontend/src/components/Toolbar.vue` | Transport type selector + serial config UI |

---

## Task 1: RTU and ASCII Frame Encode/Decode

**Files:**
- Create: `crates/modbussim-core/src/frame.rs`
- Modify: `crates/modbussim-core/src/lib.rs`

- [ ] **Step 1: Write failing tests for RTU frame functions**

Create `crates/modbussim-core/src/frame.rs`:

```rust
use crate::tools;

/// An RTU frame: slave_id + PDU + CRC16.
#[derive(Debug, Clone, PartialEq)]
pub struct RtuFrame {
    pub slave_id: u8,
    pub pdu: Vec<u8>,
}

/// An ASCII frame: slave_id + PDU (binary), encoded as hex with ':' prefix and CRLF suffix + LRC.
#[derive(Debug, Clone, PartialEq)]
pub struct AsciiFrame {
    pub slave_id: u8,
    pub pdu: Vec<u8>,
}

/// Encode an RTU frame: [slave_id | PDU | CRC16_lo | CRC16_hi]
pub fn encode_rtu(slave_id: u8, pdu: &[u8]) -> Vec<u8> {
    todo!()
}

/// Decode an RTU frame from raw bytes (including CRC).
/// Returns error if frame is too short or CRC fails.
pub fn decode_rtu(data: &[u8]) -> Result<RtuFrame, String> {
    todo!()
}

/// Encode an ASCII frame: ":" + hex(slave_id) + hex(PDU) + hex(LRC) + "\r\n"
pub fn encode_ascii(slave_id: u8, pdu: &[u8]) -> Vec<u8> {
    todo!()
}

/// Decode an ASCII frame from raw bytes (including ':' prefix, hex data, LRC, CRLF).
pub fn decode_ascii(data: &[u8]) -> Result<AsciiFrame, String> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    // FC03 Read Holding Registers: addr=0x0000, qty=0x000A
    const TEST_PDU: &[u8] = &[0x03, 0x00, 0x00, 0x00, 0x0A];

    #[test]
    fn test_encode_rtu_structure() {
        let frame = encode_rtu(1, TEST_PDU);
        assert_eq!(frame[0], 1); // slave_id
        assert_eq!(&frame[1..6], TEST_PDU); // PDU
        assert_eq!(frame.len(), 1 + 5 + 2); // slave_id + pdu + crc
    }

    #[test]
    fn test_encode_rtu_crc_valid() {
        let frame = encode_rtu(1, TEST_PDU);
        assert!(tools::verify_crc16(&frame));
    }

    #[test]
    fn test_decode_rtu_roundtrip() {
        let frame = encode_rtu(1, TEST_PDU);
        let decoded = decode_rtu(&frame).unwrap();
        assert_eq!(decoded.slave_id, 1);
        assert_eq!(decoded.pdu, TEST_PDU);
    }

    #[test]
    fn test_decode_rtu_too_short() {
        assert!(decode_rtu(&[0x01, 0x03]).is_err());
    }

    #[test]
    fn test_decode_rtu_bad_crc() {
        let mut frame = encode_rtu(1, TEST_PDU);
        *frame.last_mut().unwrap() ^= 0xFF; // corrupt CRC
        assert!(decode_rtu(&frame).is_err());
    }

    #[test]
    fn test_encode_ascii_structure() {
        let frame = encode_ascii(1, TEST_PDU);
        assert_eq!(frame[0], b':');
        assert_eq!(frame[frame.len() - 2], b'\r');
        assert_eq!(frame[frame.len() - 1], b'\n');
    }

    #[test]
    fn test_encode_ascii_hex_content() {
        let frame = encode_ascii(1, &[0x03, 0x00, 0x00, 0x00, 0x0A]);
        let s = String::from_utf8(frame.clone()).unwrap();
        // Should be: ":0103000000XX\r\n" where XX is LRC hex
        assert!(s.starts_with(":0103000000"));
        assert!(s.ends_with("\r\n"));
    }

    #[test]
    fn test_decode_ascii_roundtrip() {
        let frame = encode_ascii(1, TEST_PDU);
        let decoded = decode_ascii(&frame).unwrap();
        assert_eq!(decoded.slave_id, 1);
        assert_eq!(decoded.pdu, TEST_PDU);
    }

    #[test]
    fn test_decode_ascii_bad_lrc() {
        let mut frame = encode_ascii(1, TEST_PDU);
        // Corrupt the LRC hex chars (2 chars before \r\n)
        let len = frame.len();
        frame[len - 4] = b'F';
        frame[len - 3] = b'F';
        assert!(decode_ascii(&frame).is_err());
    }

    #[test]
    fn test_decode_ascii_missing_prefix() {
        assert!(decode_ascii(b"0103000000XX\r\n").is_err());
    }
}
```

- [ ] **Step 2: Register module in lib.rs**

Add to `crates/modbussim-core/src/lib.rs`:
```rust
pub mod frame;
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p modbussim-core frame`
Expected: FAIL with "not yet implemented"

- [ ] **Step 4: Implement encode/decode functions**

Replace the `todo!()` bodies:

```rust
use crate::tools;

#[derive(Debug, Clone, PartialEq)]
pub struct RtuFrame {
    pub slave_id: u8,
    pub pdu: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AsciiFrame {
    pub slave_id: u8,
    pub pdu: Vec<u8>,
}

/// Encode an RTU frame: [slave_id | PDU | CRC16_lo | CRC16_hi]
pub fn encode_rtu(slave_id: u8, pdu: &[u8]) -> Vec<u8> {
    let mut data = Vec::with_capacity(1 + pdu.len() + 2);
    data.push(slave_id);
    data.extend_from_slice(pdu);
    tools::append_crc16(&data)
}

/// Decode an RTU frame from raw bytes (including CRC).
pub fn decode_rtu(data: &[u8]) -> Result<RtuFrame, String> {
    if data.len() < 4 {
        return Err(format!("RTU frame too short: {} bytes (min 4)", data.len()));
    }
    if !tools::verify_crc16(data) {
        return Err("RTU frame CRC mismatch".to_string());
    }
    Ok(RtuFrame {
        slave_id: data[0],
        pdu: data[1..data.len() - 2].to_vec(),
    })
}

/// Encode an ASCII frame: ":" + hex(slave_id + PDU + LRC) + "\r\n"
pub fn encode_ascii(slave_id: u8, pdu: &[u8]) -> Vec<u8> {
    let mut binary = Vec::with_capacity(1 + pdu.len());
    binary.push(slave_id);
    binary.extend_from_slice(pdu);
    let lrc = tools::lrc(&binary);
    binary.push(lrc);

    let mut frame = Vec::new();
    frame.push(b':');
    for &b in &binary {
        frame.push(hex_char_upper(b >> 4));
        frame.push(hex_char_upper(b & 0x0F));
    }
    frame.push(b'\r');
    frame.push(b'\n');
    frame
}

/// Decode an ASCII frame from raw bytes.
pub fn decode_ascii(data: &[u8]) -> Result<AsciiFrame, String> {
    if data.len() < 9 {
        return Err(format!("ASCII frame too short: {} bytes", data.len()));
    }
    if data[0] != b':' {
        return Err("ASCII frame missing ':' prefix".to_string());
    }
    if data[data.len() - 2] != b'\r' || data[data.len() - 1] != b'\n' {
        return Err("ASCII frame missing CRLF suffix".to_string());
    }

    let hex_data = &data[1..data.len() - 2];
    if hex_data.len() % 2 != 0 {
        return Err("ASCII frame hex data has odd length".to_string());
    }

    let binary: Vec<u8> = hex_data
        .chunks(2)
        .map(|pair| {
            let hi = hex_val(pair[0]).ok_or_else(|| format!("invalid hex char: {}", pair[0]))?;
            let lo = hex_val(pair[1]).ok_or_else(|| format!("invalid hex char: {}", pair[1]))?;
            Ok((hi << 4) | lo)
        })
        .collect::<Result<Vec<u8>, String>>()?;

    if binary.len() < 3 {
        return Err("ASCII frame decoded data too short".to_string());
    }

    if !tools::verify_lrc(&binary) {
        return Err("ASCII frame LRC mismatch".to_string());
    }

    Ok(AsciiFrame {
        slave_id: binary[0],
        pdu: binary[1..binary.len() - 1].to_vec(),
    })
}

fn hex_char_upper(nibble: u8) -> u8 {
    match nibble {
        0..=9 => b'0' + nibble,
        10..=15 => b'A' + nibble - 10,
        _ => b'?',
    }
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'A'..=b'F' => Some(c - b'A' + 10),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p modbussim-core frame`
Expected: All 9 tests pass

- [ ] **Step 6: Commit**

```bash
git add crates/modbussim-core/src/frame.rs crates/modbussim-core/src/lib.rs
git commit -m "feat(core): add RTU and ASCII frame encode/decode module"
```

---

## Task 2: Transport Types and Serial Port Enumeration

**Files:**
- Create: `crates/modbussim-core/src/transport.rs`
- Modify: `crates/modbussim-core/src/lib.rs`
- Modify: `crates/modbussim-core/Cargo.toml`

- [ ] **Step 1: Add serialport dependency**

Add to `crates/modbussim-core/Cargo.toml` under `[dependencies]`:

```toml
serialport = "4"
```

- [ ] **Step 2: Create transport.rs with types and serial enumeration**

```rust
use serde::{Deserialize, Serialize};

/// Serial port parity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Parity {
    None,
    Odd,
    Even,
}

/// Serial port configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialConfig {
    pub port: String,
    pub baud_rate: u32,
    pub data_bits: u8,
    pub stop_bits: u8,
    pub parity: Parity,
}

impl Default for SerialConfig {
    fn default() -> Self {
        Self {
            port: String::new(),
            baud_rate: 9600,
            data_bits: 8,
            stop_bits: 1,
            parity: Parity::None,
        }
    }
}

/// Transport configuration for a Modbus connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Transport {
    Tcp { host: String, port: u16 },
    Rtu(SerialConfig),
    Ascii(SerialConfig),
    RtuOverTcp { host: String, port: u16 },
}

/// Information about an available serial port.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialPortInfo {
    pub name: String,
    pub description: String,
    pub manufacturer: String,
}

/// List available serial ports on the system.
pub fn list_serial_ports() -> Result<Vec<SerialPortInfo>, String> {
    let ports = serialport::available_ports()
        .map_err(|e| format!("failed to enumerate serial ports: {}", e))?;

    Ok(ports
        .into_iter()
        .map(|p| {
            let (description, manufacturer) = match &p.port_type {
                serialport::SerialPortType::UsbPort(info) => (
                    info.product.clone().unwrap_or_default(),
                    info.manufacturer.clone().unwrap_or_default(),
                ),
                serialport::SerialPortType::PciPort => {
                    ("PCI Serial Port".to_string(), String::new())
                }
                serialport::SerialPortType::BluetoothPort => {
                    ("Bluetooth Serial".to_string(), String::new())
                }
                serialport::SerialPortType::Unknown => (String::new(), String::new()),
            };
            SerialPortInfo {
                name: p.port_name,
                description,
                manufacturer,
            }
        })
        .collect())
}

/// Calculate the inter-frame silence duration for RTU mode.
/// Per Modbus spec: 3.5 character times at the given baud rate.
/// For baud >= 19200, fixed at 1750 microseconds.
pub fn rtu_interframe_delay_us(baud_rate: u32) -> u64 {
    if baud_rate >= 19200 {
        1750
    } else {
        // 11 bits per character (1 start + 8 data + 1 parity + 1 stop)
        // 3.5 characters = 38.5 bits
        let bits_per_char = 11u64;
        let silence_bits = bits_per_char * 7 / 2; // 3.5 chars = 38.5 bits, round to 38
        (silence_bits * 1_000_000) / baud_rate as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serial_config_default() {
        let cfg = SerialConfig::default();
        assert_eq!(cfg.baud_rate, 9600);
        assert_eq!(cfg.data_bits, 8);
        assert_eq!(cfg.stop_bits, 1);
        assert_eq!(cfg.parity, Parity::None);
    }

    #[test]
    fn test_transport_tcp_serde() {
        let t = Transport::Tcp { host: "0.0.0.0".into(), port: 502 };
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains("\"type\":\"tcp\""));
        let t2: Transport = serde_json::from_str(&json).unwrap();
        match t2 {
            Transport::Tcp { host, port } => {
                assert_eq!(host, "0.0.0.0");
                assert_eq!(port, 502);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_transport_rtu_serde() {
        let t = Transport::Rtu(SerialConfig {
            port: "/dev/ttyUSB0".into(),
            baud_rate: 19200,
            ..Default::default()
        });
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains("\"type\":\"rtu\""));
        assert!(json.contains("ttyUSB0"));
    }

    #[test]
    fn test_transport_rtu_over_tcp_serde() {
        let t = Transport::RtuOverTcp { host: "192.168.1.1".into(), port: 502 };
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains("\"type\":\"rtu_over_tcp\""));
    }

    #[test]
    fn test_list_serial_ports_does_not_panic() {
        // Just verify the function runs without panicking.
        // On CI or systems without serial ports, it should return an empty vec.
        let result = list_serial_ports();
        assert!(result.is_ok());
    }

    #[test]
    fn test_rtu_interframe_delay_high_baud() {
        assert_eq!(rtu_interframe_delay_us(19200), 1750);
        assert_eq!(rtu_interframe_delay_us(115200), 1750);
    }

    #[test]
    fn test_rtu_interframe_delay_low_baud() {
        let delay = rtu_interframe_delay_us(9600);
        // 38.5 bits / 9600 baud ≈ 4010 us
        assert!(delay > 3500 && delay < 4500, "delay was {}", delay);
    }
}
```

- [ ] **Step 3: Register in lib.rs**

Add `pub mod transport;` to `crates/modbussim-core/src/lib.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p modbussim-core transport`
Expected: All 7 tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/modbussim-core/src/transport.rs crates/modbussim-core/src/lib.rs crates/modbussim-core/Cargo.toml
git commit -m "feat(core): add transport types, serial config, and port enumeration"
```

---

## Task 3: PDU Parsing Module

**Files:**
- Create: `crates/modbussim-core/src/pdu.rs`
- Modify: `crates/modbussim-core/src/lib.rs`

This module extracts the Modbus PDU parsing logic that will be shared between TCP, RTU, ASCII, and RTU-over-TCP transports on the slave side. It converts raw PDU bytes into structured requests and structured responses back into PDU bytes.

- [ ] **Step 1: Create pdu.rs with request/response parsing and tests**

```rust
use tokio_modbus::prelude::*;

/// A parsed Modbus request from a PDU.
#[derive(Debug, Clone)]
pub enum ModbusRequest {
    ReadCoils { address: u16, quantity: u16 },
    ReadDiscreteInputs { address: u16, quantity: u16 },
    ReadHoldingRegisters { address: u16, quantity: u16 },
    ReadInputRegisters { address: u16, quantity: u16 },
    WriteSingleCoil { address: u16, value: bool },
    WriteSingleRegister { address: u16, value: u16 },
    WriteMultipleCoils { address: u16, values: Vec<bool> },
    WriteMultipleRegisters { address: u16, values: Vec<u16> },
}

/// Parse a PDU byte slice into a ModbusRequest.
/// PDU format: [function_code | data...]
pub fn parse_request_pdu(pdu: &[u8]) -> Result<ModbusRequest, String> {
    if pdu.is_empty() {
        return Err("empty PDU".to_string());
    }
    let fc = pdu[0];
    let data = &pdu[1..];

    match fc {
        0x01 => {
            if data.len() < 4 { return Err("FC01 PDU too short".into()); }
            Ok(ModbusRequest::ReadCoils {
                address: u16::from_be_bytes([data[0], data[1]]),
                quantity: u16::from_be_bytes([data[2], data[3]]),
            })
        }
        0x02 => {
            if data.len() < 4 { return Err("FC02 PDU too short".into()); }
            Ok(ModbusRequest::ReadDiscreteInputs {
                address: u16::from_be_bytes([data[0], data[1]]),
                quantity: u16::from_be_bytes([data[2], data[3]]),
            })
        }
        0x03 => {
            if data.len() < 4 { return Err("FC03 PDU too short".into()); }
            Ok(ModbusRequest::ReadHoldingRegisters {
                address: u16::from_be_bytes([data[0], data[1]]),
                quantity: u16::from_be_bytes([data[2], data[3]]),
            })
        }
        0x04 => {
            if data.len() < 4 { return Err("FC04 PDU too short".into()); }
            Ok(ModbusRequest::ReadInputRegisters {
                address: u16::from_be_bytes([data[0], data[1]]),
                quantity: u16::from_be_bytes([data[2], data[3]]),
            })
        }
        0x05 => {
            if data.len() < 4 { return Err("FC05 PDU too short".into()); }
            let addr = u16::from_be_bytes([data[0], data[1]]);
            let raw = u16::from_be_bytes([data[2], data[3]]);
            Ok(ModbusRequest::WriteSingleCoil { address: addr, value: raw == 0xFF00 })
        }
        0x06 => {
            if data.len() < 4 { return Err("FC06 PDU too short".into()); }
            Ok(ModbusRequest::WriteSingleRegister {
                address: u16::from_be_bytes([data[0], data[1]]),
                value: u16::from_be_bytes([data[2], data[3]]),
            })
        }
        0x0F => {
            if data.len() < 5 { return Err("FC15 PDU too short".into()); }
            let addr = u16::from_be_bytes([data[0], data[1]]);
            let qty = u16::from_be_bytes([data[2], data[3]]);
            let byte_count = data[4] as usize;
            if data.len() < 5 + byte_count { return Err("FC15 PDU data truncated".into()); }
            let mut values = Vec::with_capacity(qty as usize);
            for i in 0..qty as usize {
                let byte_idx = i / 8;
                let bit_idx = i % 8;
                values.push((data[5 + byte_idx] >> bit_idx) & 1 == 1);
            }
            Ok(ModbusRequest::WriteMultipleCoils { address: addr, values })
        }
        0x10 => {
            if data.len() < 5 { return Err("FC16 PDU too short".into()); }
            let addr = u16::from_be_bytes([data[0], data[1]]);
            let qty = u16::from_be_bytes([data[2], data[3]]);
            let byte_count = data[4] as usize;
            if data.len() < 5 + byte_count { return Err("FC16 PDU data truncated".into()); }
            let mut values = Vec::with_capacity(qty as usize);
            for i in 0..qty as usize {
                let hi = data[5 + i * 2];
                let lo = data[5 + i * 2 + 1];
                values.push(u16::from_be_bytes([hi, lo]));
            }
            Ok(ModbusRequest::WriteMultipleRegisters { address: addr, values })
        }
        _ => Err(format!("unsupported function code: 0x{:02X}", fc)),
    }
}

/// Build a response PDU from function code and response data.
/// Returns the complete PDU bytes.
pub fn build_response_pdu(fc: u8, data: &ResponseData) -> Vec<u8> {
    match data {
        ResponseData::ReadBits(bits) => {
            let byte_count = (bits.len() + 7) / 8;
            let mut pdu = Vec::with_capacity(2 + byte_count);
            pdu.push(fc);
            pdu.push(byte_count as u8);
            let mut packed = vec![0u8; byte_count];
            for (i, &bit) in bits.iter().enumerate() {
                if bit {
                    packed[i / 8] |= 1 << (i % 8);
                }
            }
            pdu.extend_from_slice(&packed);
            pdu
        }
        ResponseData::ReadRegisters(regs) => {
            let byte_count = regs.len() * 2;
            let mut pdu = Vec::with_capacity(2 + byte_count);
            pdu.push(fc);
            pdu.push(byte_count as u8);
            for &reg in regs {
                pdu.extend_from_slice(&reg.to_be_bytes());
            }
            pdu
        }
        ResponseData::WriteSingleCoil { address, value } => {
            let mut pdu = vec![fc];
            pdu.extend_from_slice(&address.to_be_bytes());
            pdu.extend_from_slice(&(if *value { 0xFF00u16 } else { 0x0000u16 }).to_be_bytes());
            pdu
        }
        ResponseData::WriteSingleRegister { address, value } => {
            let mut pdu = vec![fc];
            pdu.extend_from_slice(&address.to_be_bytes());
            pdu.extend_from_slice(&value.to_be_bytes());
            pdu
        }
        ResponseData::WriteMultiple { address, quantity } => {
            let mut pdu = vec![fc];
            pdu.extend_from_slice(&address.to_be_bytes());
            pdu.extend_from_slice(&quantity.to_be_bytes());
            pdu
        }
    }
}

/// Build an exception response PDU.
pub fn build_exception_pdu(fc: u8, exception_code: u8) -> Vec<u8> {
    vec![fc | 0x80, exception_code]
}

/// Response data variants for building response PDUs.
#[derive(Debug, Clone)]
pub enum ResponseData {
    ReadBits(Vec<bool>),
    ReadRegisters(Vec<u16>),
    WriteSingleCoil { address: u16, value: bool },
    WriteSingleRegister { address: u16, value: u16 },
    WriteMultiple { address: u16, quantity: u16 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_fc03_read_holding_registers() {
        // FC03, addr=0x006B, qty=0x0003
        let pdu = &[0x03, 0x00, 0x6B, 0x00, 0x03];
        let req = parse_request_pdu(pdu).unwrap();
        match req {
            ModbusRequest::ReadHoldingRegisters { address, quantity } => {
                assert_eq!(address, 0x006B);
                assert_eq!(quantity, 3);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_parse_fc05_write_single_coil_on() {
        let pdu = &[0x05, 0x00, 0x0A, 0xFF, 0x00];
        let req = parse_request_pdu(pdu).unwrap();
        match req {
            ModbusRequest::WriteSingleCoil { address, value } => {
                assert_eq!(address, 10);
                assert!(value);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_parse_fc05_write_single_coil_off() {
        let pdu = &[0x05, 0x00, 0x0A, 0x00, 0x00];
        let req = parse_request_pdu(pdu).unwrap();
        match req {
            ModbusRequest::WriteSingleCoil { address, value } => {
                assert_eq!(address, 10);
                assert!(!value);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_parse_fc10_write_multiple_registers() {
        // FC16, addr=0x0001, qty=0x0002, byte_count=4, values=[0x000A, 0x0102]
        let pdu = &[0x10, 0x00, 0x01, 0x00, 0x02, 0x04, 0x00, 0x0A, 0x01, 0x02];
        let req = parse_request_pdu(pdu).unwrap();
        match req {
            ModbusRequest::WriteMultipleRegisters { address, values } => {
                assert_eq!(address, 1);
                assert_eq!(values, vec![0x000A, 0x0102]);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_parse_empty_pdu() {
        assert!(parse_request_pdu(&[]).is_err());
    }

    #[test]
    fn test_parse_unsupported_fc() {
        assert!(parse_request_pdu(&[0x2B, 0x00]).is_err());
    }

    #[test]
    fn test_build_response_read_registers() {
        let data = ResponseData::ReadRegisters(vec![0x0001, 0x0002, 0x0003]);
        let pdu = build_response_pdu(0x03, &data);
        assert_eq!(pdu, &[0x03, 0x06, 0x00, 0x01, 0x00, 0x02, 0x00, 0x03]);
    }

    #[test]
    fn test_build_response_read_bits() {
        let data = ResponseData::ReadBits(vec![true, false, true, true, false, false, false, false, true]);
        let pdu = build_response_pdu(0x01, &data);
        // byte_count=2, byte0=0b00001101=0x0D, byte1=0b00000001=0x01
        assert_eq!(pdu[0], 0x01);
        assert_eq!(pdu[1], 2); // byte count
        assert_eq!(pdu[2], 0x0D);
        assert_eq!(pdu[3], 0x01);
    }

    #[test]
    fn test_build_exception_pdu() {
        let pdu = build_exception_pdu(0x03, 0x02);
        assert_eq!(pdu, &[0x83, 0x02]);
    }

    #[test]
    fn test_build_response_write_single_coil() {
        let data = ResponseData::WriteSingleCoil { address: 10, value: true };
        let pdu = build_response_pdu(0x05, &data);
        assert_eq!(pdu, &[0x05, 0x00, 0x0A, 0xFF, 0x00]);
    }

    #[test]
    fn test_build_response_write_multiple() {
        let data = ResponseData::WriteMultiple { address: 1, quantity: 10 };
        let pdu = build_response_pdu(0x10, &data);
        assert_eq!(pdu, &[0x10, 0x00, 0x01, 0x00, 0x0A]);
    }
}
```

- [ ] **Step 2: Register in lib.rs**

Add `pub mod pdu;` to lib.rs.

- [ ] **Step 3: Run tests**

Run: `cargo test -p modbussim-core pdu`
Expected: All 11 tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/modbussim-core/src/pdu.rs crates/modbussim-core/src/lib.rs
git commit -m "feat(core): add PDU request/response parsing module"
```

---

## Task 4: Extend Project TransportConfig

**Files:**
- Modify: `crates/modbussim-core/src/project.rs`

- [ ] **Step 1: Update TransportConfig enum**

In `crates/modbussim-core/src/project.rs`, replace the current `TransportConfig`:

```rust
// Old:
pub enum TransportConfig {
    Tcp { host: String, port: u16 },
}

// New:
pub enum TransportConfig {
    Tcp { host: String, port: u16 },
    Rtu {
        port: String,
        baud_rate: u32,
        data_bits: u8,
        stop_bits: u8,
        parity: String,
    },
    Ascii {
        port: String,
        baud_rate: u32,
        data_bits: u8,
        stop_bits: u8,
        parity: String,
    },
    RtuOverTcp { host: String, port: u16 },
}
```

- [ ] **Step 2: Add test for new variants**

Add to the existing tests module:

```rust
#[test]
fn test_transport_rtu_serde() {
    let proj = ProjectFile {
        version: 1,
        project_type: ProjectType::Slave,
        connections: vec![ConnectionConfig {
            id: "c1".to_string(),
            name: "serial".to_string(),
            transport: TransportConfig::Rtu {
                port: "/dev/ttyUSB0".to_string(),
                baud_rate: 9600,
                data_bits: 8,
                stop_bits: 1,
                parity: "none".to_string(),
            },
            devices: vec![],
            scan_groups: vec![],
        }],
    };
    let json = serde_json::to_string(&proj).unwrap();
    assert!(json.contains("\"type\":\"rtu\""));
    assert!(json.contains("ttyUSB0"));
    let loaded: ProjectFile = serde_json::from_str(&json).unwrap();
    match &loaded.connections[0].transport {
        TransportConfig::Rtu { port, baud_rate, .. } => {
            assert_eq!(port, "/dev/ttyUSB0");
            assert_eq!(*baud_rate, 9600);
        }
        _ => panic!("wrong transport variant"),
    }
}

#[test]
fn test_transport_rtu_over_tcp_serde() {
    let config = TransportConfig::RtuOverTcp { host: "10.0.0.1".to_string(), port: 502 };
    let json = serde_json::to_string(&config).unwrap();
    assert!(json.contains("\"type\":\"rtu_over_tcp\""));
    let loaded: TransportConfig = serde_json::from_str(&json).unwrap();
    match loaded {
        TransportConfig::RtuOverTcp { host, port } => {
            assert_eq!(host, "10.0.0.1");
            assert_eq!(port, 502);
        }
        _ => panic!("wrong variant"),
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p modbussim-core project`
Expected: All tests pass (existing + 2 new)

- [ ] **Step 4: Commit**

```bash
git add crates/modbussim-core/src/project.rs
git commit -m "feat(core): extend TransportConfig with Rtu, Ascii, RtuOverTcp variants"
```

---

## Task 5: RTU Slave Server

**Files:**
- Create: `crates/modbussim-core/src/rtu_slave.rs`
- Modify: `crates/modbussim-core/src/lib.rs`
- Modify: `crates/modbussim-core/Cargo.toml`

- [ ] **Step 1: Add tokio-serial dependency**

Add to `crates/modbussim-core/Cargo.toml`:
```toml
tokio-serial = "5"
```

- [ ] **Step 2: Create rtu_slave.rs**

```rust
use crate::frame;
use crate::log_collector::LogCollector;
use crate::log_entry::{Direction, FunctionCode, LogEntry};
use crate::pdu::{self, ModbusRequest, ResponseData};
use crate::register::RegisterMap;
use crate::slave::{handle_read, handle_write, SharedDevices};
use crate::transport::SerialConfig;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::oneshot;
use tokio_serial::SerialPortBuilderExt;

/// Run an RTU slave server on a serial port.
/// Blocks until shutdown signal is received.
pub async fn run_rtu_slave(
    config: SerialConfig,
    devices: SharedDevices,
    log_collector: Option<Arc<LogCollector>>,
    mut shutdown_rx: oneshot::Receiver<()>,
) -> Result<(), String> {
    let parity = match config.parity {
        crate::transport::Parity::None => tokio_serial::Parity::None,
        crate::transport::Parity::Odd => tokio_serial::Parity::Odd,
        crate::transport::Parity::Even => tokio_serial::Parity::Even,
    };

    let stop_bits = match config.stop_bits {
        2 => tokio_serial::StopBits::Two,
        _ => tokio_serial::StopBits::One,
    };

    let data_bits = match config.data_bits {
        7 => tokio_serial::DataBits::Seven,
        _ => tokio_serial::DataBits::Eight,
    };

    let mut port = tokio_serial::new(&config.port, config.baud_rate)
        .parity(parity)
        .stop_bits(stop_bits)
        .data_bits(data_bits)
        .timeout(Duration::from_millis(100))
        .open_native_async()
        .map_err(|e| format!("failed to open serial port {}: {}", config.port, e))?;

    let interframe_delay = Duration::from_micros(
        crate::transport::rtu_interframe_delay_us(config.baud_rate),
    );

    let mut buf = vec![0u8; 512];

    loop {
        // Check for shutdown
        if shutdown_rx.try_recv().is_ok() {
            break;
        }

        // Read with timeout — accumulate bytes until interframe silence
        let frame_data = match read_rtu_frame(&mut port, &mut buf, interframe_delay).await {
            Ok(data) if data.is_empty() => continue,
            Ok(data) => data,
            Err(_) => continue,
        };

        // Decode RTU frame
        let rtu_frame = match frame::decode_rtu(&frame_data) {
            Ok(f) => f,
            Err(_) => continue, // Bad frame, ignore
        };

        // Log RX
        if let Some(ref collector) = log_collector {
            if let Ok(req) = pdu::parse_request_pdu(&rtu_frame.pdu) {
                let fc = request_to_fc(&req);
                let detail = format_request(&req);
                collector.try_add(LogEntry::new(Direction::Rx, fc, &detail));
            }
        }

        // Process request
        let response_pdu = process_request(rtu_frame.slave_id, &rtu_frame.pdu, &devices).await;

        if let Some(resp_pdu) = response_pdu {
            // Log TX
            if let Some(ref collector) = log_collector {
                if let Some(fc) = rtu_frame.pdu.first().and_then(|&b| FunctionCode::from_u8(b)) {
                    let detail = if resp_pdu[0] & 0x80 != 0 {
                        format!("ERR: exception 0x{:02X}", resp_pdu.get(1).copied().unwrap_or(0))
                    } else {
                        "OK".to_string()
                    };
                    collector.try_add(LogEntry::new(Direction::Tx, fc, &detail));
                }
            }

            // Encode and send response
            let response_frame = frame::encode_rtu(rtu_frame.slave_id, &resp_pdu);
            let _ = port.write_all(&response_frame).await;
        }
    }

    Ok(())
}

/// Read an RTU frame by accumulating bytes until an interframe silence gap.
async fn read_rtu_frame(
    port: &mut tokio_serial::SerialStream,
    buf: &mut [u8],
    interframe_delay: Duration,
) -> Result<Vec<u8>, String> {
    let mut frame = Vec::new();

    loop {
        match tokio::time::timeout(interframe_delay, port.read(buf)).await {
            Ok(Ok(n)) if n > 0 => {
                frame.extend_from_slice(&buf[..n]);
            }
            Ok(Ok(_)) => break, // 0 bytes = EOF
            Ok(Err(_)) => break,
            Err(_) => break, // Timeout = interframe silence detected
        }

        if frame.is_empty() {
            // No data yet, keep waiting with longer timeout
            match tokio::time::timeout(Duration::from_secs(1), port.read(buf)).await {
                Ok(Ok(n)) if n > 0 => {
                    frame.extend_from_slice(&buf[..n]);
                }
                _ => return Ok(Vec::new()),
            }
        }
    }

    Ok(frame)
}

/// Process a Modbus request PDU against the device registry.
/// Returns the response PDU bytes, or None if the slave_id is not found.
pub(crate) async fn process_request(
    slave_id: u8,
    request_pdu: &[u8],
    devices: &SharedDevices,
) -> Option<Vec<u8>> {
    let parsed = match pdu::parse_request_pdu(request_pdu) {
        Ok(r) => r,
        Err(_) => {
            return Some(pdu::build_exception_pdu(
                request_pdu.first().copied().unwrap_or(0),
                0x01, // Illegal Function
            ));
        }
    };

    let fc = request_pdu[0];
    let is_write = matches!(
        parsed,
        ModbusRequest::WriteSingleCoil { .. }
            | ModbusRequest::WriteSingleRegister { .. }
            | ModbusRequest::WriteMultipleCoils { .. }
            | ModbusRequest::WriteMultipleRegisters { .. }
    );

    if is_write {
        let mut devices = devices.write().await;
        let device = devices.get_mut(&slave_id)?;
        match execute_write(&mut device.register_map, &parsed) {
            Ok(data) => Some(pdu::build_response_pdu(fc, &data)),
            Err(exc) => Some(pdu::build_exception_pdu(fc, exc)),
        }
    } else {
        let devices = devices.read().await;
        let device = devices.get(&slave_id)?;
        match execute_read(&device.register_map, &parsed) {
            Ok(data) => Some(pdu::build_response_pdu(fc, &data)),
            Err(exc) => Some(pdu::build_exception_pdu(fc, exc)),
        }
    }
}

fn execute_read(register_map: &RegisterMap, req: &ModbusRequest) -> Result<ResponseData, u8> {
    match req {
        ModbusRequest::ReadCoils { address, quantity } => {
            let mut bits = Vec::with_capacity(*quantity as usize);
            for i in 0..*quantity {
                bits.push(*register_map.coils.get(&(address + i)).unwrap_or(&false));
            }
            Ok(ResponseData::ReadBits(bits))
        }
        ModbusRequest::ReadDiscreteInputs { address, quantity } => {
            let mut bits = Vec::with_capacity(*quantity as usize);
            for i in 0..*quantity {
                bits.push(*register_map.discrete_inputs.get(&(address + i)).unwrap_or(&false));
            }
            Ok(ResponseData::ReadBits(bits))
        }
        ModbusRequest::ReadHoldingRegisters { address, quantity } => {
            let mut regs = Vec::with_capacity(*quantity as usize);
            for i in 0..*quantity {
                regs.push(*register_map.holding_registers.get(&(address + i)).unwrap_or(&0));
            }
            Ok(ResponseData::ReadRegisters(regs))
        }
        ModbusRequest::ReadInputRegisters { address, quantity } => {
            let mut regs = Vec::with_capacity(*quantity as usize);
            for i in 0..*quantity {
                regs.push(*register_map.input_registers.get(&(address + i)).unwrap_or(&0));
            }
            Ok(ResponseData::ReadRegisters(regs))
        }
        _ => Err(0x01), // Illegal Function
    }
}

fn execute_write(register_map: &mut RegisterMap, req: &ModbusRequest) -> Result<ResponseData, u8> {
    match req {
        ModbusRequest::WriteSingleCoil { address, value } => {
            register_map.coils.insert(*address, *value);
            register_map.discrete_inputs.insert(*address, *value);
            Ok(ResponseData::WriteSingleCoil { address: *address, value: *value })
        }
        ModbusRequest::WriteSingleRegister { address, value } => {
            register_map.holding_registers.insert(*address, *value);
            register_map.input_registers.insert(*address, *value);
            Ok(ResponseData::WriteSingleRegister { address: *address, value: *value })
        }
        ModbusRequest::WriteMultipleCoils { address, values } => {
            for (i, &val) in values.iter().enumerate() {
                register_map.coils.insert(address + i as u16, val);
                register_map.discrete_inputs.insert(address + i as u16, val);
            }
            Ok(ResponseData::WriteMultiple { address: *address, quantity: values.len() as u16 })
        }
        ModbusRequest::WriteMultipleRegisters { address, values } => {
            for (i, &val) in values.iter().enumerate() {
                register_map.holding_registers.insert(address + i as u16, val);
                register_map.input_registers.insert(address + i as u16, val);
            }
            Ok(ResponseData::WriteMultiple { address: *address, quantity: values.len() as u16 })
        }
        _ => Err(0x01),
    }
}

fn request_to_fc(req: &ModbusRequest) -> FunctionCode {
    match req {
        ModbusRequest::ReadCoils { .. } => FunctionCode::ReadCoils,
        ModbusRequest::ReadDiscreteInputs { .. } => FunctionCode::ReadDiscreteInputs,
        ModbusRequest::ReadHoldingRegisters { .. } => FunctionCode::ReadHoldingRegisters,
        ModbusRequest::ReadInputRegisters { .. } => FunctionCode::ReadInputRegisters,
        ModbusRequest::WriteSingleCoil { .. } => FunctionCode::WriteSingleCoil,
        ModbusRequest::WriteSingleRegister { .. } => FunctionCode::WriteSingleRegister,
        ModbusRequest::WriteMultipleCoils { .. } => FunctionCode::WriteMultipleCoils,
        ModbusRequest::WriteMultipleRegisters { .. } => FunctionCode::WriteMultipleRegisters,
    }
}

fn format_request(req: &ModbusRequest) -> String {
    match req {
        ModbusRequest::ReadCoils { address, quantity }
        | ModbusRequest::ReadDiscreteInputs { address, quantity }
        | ModbusRequest::ReadHoldingRegisters { address, quantity }
        | ModbusRequest::ReadInputRegisters { address, quantity } => {
            format!("R {} x{}", address, quantity)
        }
        ModbusRequest::WriteSingleCoil { address, value } => format!("W {} = {}", address, value),
        ModbusRequest::WriteSingleRegister { address, value } => format!("W {} = 0x{:04X}", address, value),
        ModbusRequest::WriteMultipleCoils { address, values } => format!("W {} x{}", address, values.len()),
        ModbusRequest::WriteMultipleRegisters { address, values } => format!("W {} x{}", address, values.len()),
    }
}
```

- [ ] **Step 3: Register in lib.rs**

Add `pub mod rtu_slave;` to lib.rs.

- [ ] **Step 4: Build to verify compilation**

Run: `cargo build -p modbussim-core`
Expected: Clean compilation

- [ ] **Step 5: Commit**

```bash
git add crates/modbussim-core/src/rtu_slave.rs crates/modbussim-core/src/lib.rs crates/modbussim-core/Cargo.toml
git commit -m "feat(core): add RTU slave server over serial port"
```

---

## Task 6: ASCII Slave Server and RTU-over-TCP Slave Server

**Files:**
- Create: `crates/modbussim-core/src/ascii_slave.rs`
- Create: `crates/modbussim-core/src/rtu_tcp_slave.rs`
- Modify: `crates/modbussim-core/src/lib.rs`

- [ ] **Step 1: Create ascii_slave.rs**

The ASCII slave reads frames delimited by ':' ... '\r\n' instead of RTU interframe silence.

```rust
use crate::frame;
use crate::log_collector::LogCollector;
use crate::log_entry::{Direction, FunctionCode, LogEntry};
use crate::pdu;
use crate::rtu_slave::{process_request, format_request, request_to_fc};
use crate::slave::SharedDevices;
use crate::transport::SerialConfig;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::oneshot;
use tokio_serial::SerialPortBuilderExt;

/// Run an ASCII slave server on a serial port.
pub async fn run_ascii_slave(
    config: SerialConfig,
    devices: SharedDevices,
    log_collector: Option<Arc<LogCollector>>,
    mut shutdown_rx: oneshot::Receiver<()>,
) -> Result<(), String> {
    let parity = match config.parity {
        crate::transport::Parity::None => tokio_serial::Parity::None,
        crate::transport::Parity::Odd => tokio_serial::Parity::Odd,
        crate::transport::Parity::Even => tokio_serial::Parity::Even,
    };
    let stop_bits = match config.stop_bits {
        2 => tokio_serial::StopBits::Two,
        _ => tokio_serial::StopBits::One,
    };
    let data_bits = match config.data_bits {
        7 => tokio_serial::DataBits::Seven,
        _ => tokio_serial::DataBits::Eight,
    };

    let port = tokio_serial::new(&config.port, config.baud_rate)
        .parity(parity)
        .stop_bits(stop_bits)
        .data_bits(data_bits)
        .timeout(Duration::from_millis(100))
        .open_native_async()
        .map_err(|e| format!("failed to open serial port {}: {}", config.port, e))?;

    let (reader, mut writer) = tokio::io::split(port);
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        if shutdown_rx.try_recv().is_ok() {
            break;
        }

        line.clear();
        match tokio::time::timeout(Duration::from_secs(1), buf_reader.read_line(&mut line)).await {
            Ok(Ok(0)) => continue,
            Ok(Ok(_)) => {}
            Ok(Err(_)) => continue,
            Err(_) => continue,
        }

        // ASCII frames start with ':' and end with '\r\n'
        let trimmed = line.trim();
        if !trimmed.starts_with(':') {
            continue;
        }

        let frame_bytes = line.as_bytes();
        let ascii_frame = match frame::decode_ascii(frame_bytes) {
            Ok(f) => f,
            Err(_) => continue,
        };

        // Log RX
        if let Some(ref collector) = log_collector {
            if let Ok(req) = pdu::parse_request_pdu(&ascii_frame.pdu) {
                let fc = request_to_fc(&req);
                let detail = format_request(&req);
                collector.try_add(LogEntry::new(Direction::Rx, fc, &detail));
            }
        }

        // Process request
        let response_pdu = process_request(ascii_frame.slave_id, &ascii_frame.pdu, &devices).await;

        if let Some(resp_pdu) = response_pdu {
            // Log TX
            if let Some(ref collector) = log_collector {
                if let Some(fc) = ascii_frame.pdu.first().and_then(|&b| FunctionCode::from_u8(b)) {
                    let detail = if resp_pdu[0] & 0x80 != 0 { "ERR".to_string() } else { "OK".to_string() };
                    collector.try_add(LogEntry::new(Direction::Tx, fc, &detail));
                }
            }

            let response_frame = frame::encode_ascii(ascii_frame.slave_id, &resp_pdu);
            let _ = writer.write_all(&response_frame).await;
        }
    }

    Ok(())
}
```

- [ ] **Step 2: Create rtu_tcp_slave.rs**

RTU-over-TCP: TCP listener but frames use RTU format (no MBAP header).

```rust
use crate::frame;
use crate::log_collector::LogCollector;
use crate::log_entry::{Direction, FunctionCode, LogEntry};
use crate::pdu;
use crate::rtu_slave::{process_request, format_request, request_to_fc};
use crate::slave::SharedDevices;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

/// Run an RTU-over-TCP slave server.
pub async fn run_rtu_tcp_slave(
    host: String,
    port: u16,
    devices: SharedDevices,
    log_collector: Option<Arc<LogCollector>>,
    mut shutdown_rx: oneshot::Receiver<()>,
) -> Result<(), String> {
    let addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .map_err(|e| format!("invalid address: {}", e))?;

    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| format!("failed to bind {}: {}", addr, e))?;

    loop {
        tokio::select! {
            _ = &mut shutdown_rx => break,
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, _peer)) => {
                        let devices = devices.clone();
                        let log_collector = log_collector.clone();
                        tokio::spawn(async move {
                            handle_rtu_tcp_client(stream, devices, log_collector).await;
                        });
                    }
                    Err(_) => continue,
                }
            }
        }
    }

    Ok(())
}

async fn handle_rtu_tcp_client(
    mut stream: tokio::net::TcpStream,
    devices: SharedDevices,
    log_collector: Option<Arc<LogCollector>>,
) {
    let mut buf = vec![0u8; 512];

    loop {
        let n = match tokio::time::timeout(Duration::from_secs(60), stream.read(&mut buf)).await {
            Ok(Ok(0)) => break,
            Ok(Ok(n)) => n,
            Ok(Err(_)) => break,
            Err(_) => continue,
        };

        let data = &buf[..n];
        let rtu_frame = match frame::decode_rtu(data) {
            Ok(f) => f,
            Err(_) => continue,
        };

        if let Some(ref collector) = log_collector {
            if let Ok(req) = pdu::parse_request_pdu(&rtu_frame.pdu) {
                let fc = request_to_fc(&req);
                let detail = format_request(&req);
                collector.try_add(LogEntry::new(Direction::Rx, fc, &detail));
            }
        }

        let response_pdu = process_request(rtu_frame.slave_id, &rtu_frame.pdu, &devices).await;

        if let Some(resp_pdu) = response_pdu {
            if let Some(ref collector) = log_collector {
                if let Some(fc) = rtu_frame.pdu.first().and_then(|&b| FunctionCode::from_u8(b)) {
                    let detail = if resp_pdu[0] & 0x80 != 0 { "ERR".to_string() } else { "OK".to_string() };
                    collector.try_add(LogEntry::new(Direction::Tx, fc, &detail));
                }
            }

            let response_frame = frame::encode_rtu(rtu_frame.slave_id, &resp_pdu);
            if stream.write_all(&response_frame).await.is_err() {
                break;
            }
        }
    }
}
```

- [ ] **Step 3: Register modules and make rtu_slave helpers pub(crate)**

Add to lib.rs:
```rust
pub mod ascii_slave;
pub mod rtu_tcp_slave;
```

In `rtu_slave.rs`, make `process_request`, `request_to_fc`, and `format_request` `pub(crate)` so they can be used by `ascii_slave` and `rtu_tcp_slave`.

- [ ] **Step 4: Build**

Run: `cargo build -p modbussim-core`
Expected: Clean compilation

- [ ] **Step 5: Commit**

```bash
git add crates/modbussim-core/src/ascii_slave.rs crates/modbussim-core/src/rtu_tcp_slave.rs \
        crates/modbussim-core/src/rtu_slave.rs crates/modbussim-core/src/lib.rs
git commit -m "feat(core): add ASCII slave and RTU-over-TCP slave servers"
```

---

## Task 7: Refactor SlaveConnection to Support Transport

**Files:**
- Modify: `crates/modbussim-core/src/slave.rs`

- [ ] **Step 1: Add Transport to SlaveConnection**

Replace `TransportConfig` in slave.rs with the new `Transport` enum from `transport.rs`:

```rust
use crate::transport::Transport;

pub struct SlaveConnection {
    pub transport: Transport,  // was: TransportConfig
    pub devices: SharedDevices,
    pub log_collector: SharedLogCollector,
    state: ConnectionState,
    shutdown_tx: Option<oneshot::Sender<()>>,
    server_handle: Option<tokio::task::JoinHandle<()>>,
}
```

Update `SlaveConnection::new()` to accept `Transport`:
```rust
pub fn new(transport: Transport) -> Self {
    Self {
        transport,
        devices: Arc::new(RwLock::new(HashMap::new())),
        log_collector: None,
        state: ConnectionState::Stopped,
        shutdown_tx: None,
        server_handle: None,
    }
}
```

- [ ] **Step 2: Refactor start() to dispatch on transport type**

```rust
pub async fn start(&mut self) -> Result<(), SlaveError> {
    if self.state == ConnectionState::Running {
        return Err(SlaveError::AlreadyRunning);
    }

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let devices = self.devices.clone();
    let log_collector = self.log_collector.clone();

    let handle = match &self.transport {
        Transport::Tcp { host, port } => {
            let addr: SocketAddr = format!("{}:{}", host, port)
                .parse()
                .map_err(|e| SlaveError::BindError(format!("Invalid address: {e}")))?;
            // Existing TCP server logic using tokio_modbus
            self.start_tcp(addr, devices, log_collector, shutdown_rx).await?
        }
        Transport::Rtu(config) => {
            let config = config.clone();
            tokio::spawn(async move {
                let _ = crate::rtu_slave::run_rtu_slave(config, devices, log_collector, shutdown_rx).await;
            })
        }
        Transport::Ascii(config) => {
            let config = config.clone();
            tokio::spawn(async move {
                let _ = crate::ascii_slave::run_ascii_slave(config, devices, log_collector, shutdown_rx).await;
            })
        }
        Transport::RtuOverTcp { host, port } => {
            let host = host.clone();
            let port = *port;
            tokio::spawn(async move {
                let _ = crate::rtu_tcp_slave::run_rtu_tcp_slave(host, port, devices, log_collector, shutdown_rx).await;
            })
        }
    };

    self.shutdown_tx = Some(shutdown_tx);
    self.server_handle = Some(handle);
    self.state = ConnectionState::Running;
    Ok(())
}
```

Extract the existing TCP server startup into a private `start_tcp()` method to keep start() clean.

- [ ] **Step 3: Remove old TransportConfig struct**

Delete the old `TransportConfig` struct (lines ~166-173) since we now use `transport::Transport`. Update any remaining references.

- [ ] **Step 4: Build and test**

Run: `cargo build -p modbussim-core && cargo test -p modbussim-core`
Expected: Build succeeds, existing tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/modbussim-core/src/slave.rs
git commit -m "refactor(core): SlaveConnection now accepts Transport enum, dispatches to TCP/RTU/ASCII/RtuOverTcp"
```

---

## Task 8: RTU/ASCII/RtuOverTcp Master Clients

**Files:**
- Create: `crates/modbussim-core/src/rtu_master.rs`
- Create: `crates/modbussim-core/src/ascii_master.rs`
- Create: `crates/modbussim-core/src/rtu_tcp_master.rs`
- Modify: `crates/modbussim-core/src/lib.rs`

- [ ] **Step 1: Create rtu_master.rs**

The RTU master opens a serial port, sends request frames, and reads responses. All requests are serialized (half-duplex).

```rust
use crate::frame;
use crate::master::ReadResult;
use crate::pdu::{self, ResponseData};
use crate::transport::SerialConfig;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tokio_serial::SerialPortBuilderExt;

/// A serial RTU master connection.
pub struct RtuMasterTransport {
    port: Arc<Mutex<tokio_serial::SerialStream>>,
    interframe_delay: Duration,
}

impl RtuMasterTransport {
    pub async fn connect(config: &SerialConfig) -> Result<Self, String> {
        let parity = match config.parity {
            crate::transport::Parity::None => tokio_serial::Parity::None,
            crate::transport::Parity::Odd => tokio_serial::Parity::Odd,
            crate::transport::Parity::Even => tokio_serial::Parity::Even,
        };
        let stop_bits = match config.stop_bits {
            2 => tokio_serial::StopBits::Two,
            _ => tokio_serial::StopBits::One,
        };
        let data_bits = match config.data_bits {
            7 => tokio_serial::DataBits::Seven,
            _ => tokio_serial::DataBits::Eight,
        };

        let port = tokio_serial::new(&config.port, config.baud_rate)
            .parity(parity)
            .stop_bits(stop_bits)
            .data_bits(data_bits)
            .timeout(Duration::from_millis(100))
            .open_native_async()
            .map_err(|e| format!("failed to open serial port: {}", e))?;

        let interframe_delay = Duration::from_micros(
            crate::transport::rtu_interframe_delay_us(config.baud_rate),
        );

        Ok(Self {
            port: Arc::new(Mutex::new(port)),
            interframe_delay,
        })
    }

    /// Send an RTU request and read the response.
    /// All calls are serialized through the Mutex.
    pub async fn request(
        &self,
        slave_id: u8,
        request_pdu: &[u8],
        timeout: Duration,
    ) -> Result<Vec<u8>, String> {
        let mut port = self.port.lock().await;

        // Encode and send
        let frame = frame::encode_rtu(slave_id, request_pdu);
        port.write_all(&frame)
            .await
            .map_err(|e| format!("write error: {}", e))?;

        // Wait for interframe delay then read response
        tokio::time::sleep(self.interframe_delay).await;

        let mut buf = vec![0u8; 512];
        let mut response = Vec::new();

        loop {
            match tokio::time::timeout(timeout, port.read(&mut buf)).await {
                Ok(Ok(n)) if n > 0 => {
                    response.extend_from_slice(&buf[..n]);
                    // Check if we have a complete frame
                    if response.len() >= 4 {
                        // Try to decode — if CRC checks out, we're done
                        if frame::decode_rtu(&response).is_ok() {
                            break;
                        }
                    }
                }
                Ok(Ok(_)) => break,
                Ok(Err(e)) => return Err(format!("read error: {}", e)),
                Err(_) => {
                    if response.is_empty() {
                        return Err("response timeout".to_string());
                    }
                    break;
                }
            }
        }

        let rtu_frame = frame::decode_rtu(&response)
            .map_err(|e| format!("invalid response frame: {}", e))?;

        if rtu_frame.slave_id != slave_id {
            return Err(format!("slave_id mismatch: expected {}, got {}", slave_id, rtu_frame.slave_id));
        }

        Ok(rtu_frame.pdu)
    }
}
```

- [ ] **Step 2: Create ascii_master.rs**

Similar structure but uses ASCII framing:

```rust
use crate::frame;
use crate::transport::SerialConfig;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;
use tokio_serial::SerialPortBuilderExt;

/// An ASCII serial master connection.
pub struct AsciiMasterTransport {
    port: Arc<Mutex<tokio_serial::SerialStream>>,
}

impl AsciiMasterTransport {
    pub async fn connect(config: &SerialConfig) -> Result<Self, String> {
        let parity = match config.parity {
            crate::transport::Parity::None => tokio_serial::Parity::None,
            crate::transport::Parity::Odd => tokio_serial::Parity::Odd,
            crate::transport::Parity::Even => tokio_serial::Parity::Even,
        };
        let stop_bits = match config.stop_bits {
            2 => tokio_serial::StopBits::Two,
            _ => tokio_serial::StopBits::One,
        };
        let data_bits = match config.data_bits {
            7 => tokio_serial::DataBits::Seven,
            _ => tokio_serial::DataBits::Eight,
        };

        let port = tokio_serial::new(&config.port, config.baud_rate)
            .parity(parity)
            .stop_bits(stop_bits)
            .data_bits(data_bits)
            .timeout(Duration::from_millis(100))
            .open_native_async()
            .map_err(|e| format!("failed to open serial port: {}", e))?;

        Ok(Self {
            port: Arc::new(Mutex::new(port)),
        })
    }

    pub async fn request(
        &self,
        slave_id: u8,
        request_pdu: &[u8],
        timeout: Duration,
    ) -> Result<Vec<u8>, String> {
        let mut port = self.port.lock().await;

        let frame = frame::encode_ascii(slave_id, request_pdu);
        port.write_all(&frame)
            .await
            .map_err(|e| format!("write error: {}", e))?;

        // Read until CRLF
        let mut line = Vec::new();
        let mut byte = [0u8; 1];
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err("response timeout".to_string());
            }
            match tokio::time::timeout(remaining, port.read(&mut byte)).await {
                Ok(Ok(1)) => {
                    line.push(byte[0]);
                    if line.len() >= 2 && line[line.len() - 2] == b'\r' && line[line.len() - 1] == b'\n' {
                        break;
                    }
                }
                Ok(Ok(_)) => return Err("EOF on serial port".to_string()),
                Ok(Err(e)) => return Err(format!("read error: {}", e)),
                Err(_) => return Err("response timeout".to_string()),
            }
        }

        let ascii_frame = frame::decode_ascii(&line)
            .map_err(|e| format!("invalid response frame: {}", e))?;

        if ascii_frame.slave_id != slave_id {
            return Err(format!("slave_id mismatch: expected {}, got {}", slave_id, ascii_frame.slave_id));
        }

        Ok(ascii_frame.pdu)
    }
}
```

- [ ] **Step 3: Create rtu_tcp_master.rs**

RTU framing over a TCP connection:

```rust
use crate::frame;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

/// An RTU-over-TCP master connection.
pub struct RtuTcpMasterTransport {
    stream: Arc<Mutex<TcpStream>>,
}

impl RtuTcpMasterTransport {
    pub async fn connect(host: &str, port: u16, timeout: Duration) -> Result<Self, String> {
        let addr = format!("{}:{}", host, port);
        let stream = tokio::time::timeout(timeout, TcpStream::connect(&addr))
            .await
            .map_err(|_| format!("connection timeout: {}", addr))?
            .map_err(|e| format!("connection failed: {}", e))?;

        Ok(Self {
            stream: Arc::new(Mutex::new(stream)),
        })
    }

    pub async fn request(
        &self,
        slave_id: u8,
        request_pdu: &[u8],
        timeout: Duration,
    ) -> Result<Vec<u8>, String> {
        let mut stream = self.stream.lock().await;

        let frame = frame::encode_rtu(slave_id, request_pdu);
        stream.write_all(&frame)
            .await
            .map_err(|e| format!("write error: {}", e))?;

        let mut buf = vec![0u8; 512];
        let mut response = Vec::new();

        loop {
            match tokio::time::timeout(timeout, stream.read(&mut buf)).await {
                Ok(Ok(n)) if n > 0 => {
                    response.extend_from_slice(&buf[..n]);
                    if response.len() >= 4 && frame::decode_rtu(&response).is_ok() {
                        break;
                    }
                }
                Ok(Ok(_)) => break,
                Ok(Err(e)) => return Err(format!("read error: {}", e)),
                Err(_) => {
                    if response.is_empty() {
                        return Err("response timeout".to_string());
                    }
                    break;
                }
            }
        }

        let rtu_frame = frame::decode_rtu(&response)
            .map_err(|e| format!("invalid response: {}", e))?;

        if rtu_frame.slave_id != slave_id {
            return Err(format!("slave_id mismatch: expected {}, got {}", slave_id, rtu_frame.slave_id));
        }

        Ok(rtu_frame.pdu)
    }
}
```

- [ ] **Step 4: Register modules**

Add to lib.rs:
```rust
pub mod rtu_master;
pub mod ascii_master;
pub mod rtu_tcp_master;
```

- [ ] **Step 5: Build**

Run: `cargo build -p modbussim-core`
Expected: Clean compilation

- [ ] **Step 6: Commit**

```bash
git add crates/modbussim-core/src/rtu_master.rs crates/modbussim-core/src/ascii_master.rs \
        crates/modbussim-core/src/rtu_tcp_master.rs crates/modbussim-core/src/lib.rs
git commit -m "feat(core): add RTU, ASCII, and RTU-over-TCP master transports"
```

---

## Task 9: Refactor MasterConnection to Support Transport

**Files:**
- Modify: `crates/modbussim-core/src/master.rs`

- [ ] **Step 1: Add transport-specific connection handling**

Add a `TransportCtx` enum to hold the active transport:

```rust
use crate::transport::Transport;
use crate::rtu_master::RtuMasterTransport;
use crate::ascii_master::AsciiMasterTransport;
use crate::rtu_tcp_master::RtuTcpMasterTransport;

enum TransportCtx {
    Tcp(Arc<Mutex<client::Context>>),
    Rtu(RtuMasterTransport),
    Ascii(AsciiMasterTransport),
    RtuTcp(RtuTcpMasterTransport),
}
```

Replace `ctx: Option<Arc<Mutex<client::Context>>>` with `transport_ctx: Option<TransportCtx>`.

- [ ] **Step 2: Refactor connect()**

```rust
pub async fn connect(&mut self) -> Result<(), MasterError> {
    if self.state == MasterState::Connected {
        return Err(MasterError::AlreadyConnected);
    }

    let timeout = self.timeout_duration();

    let ctx = match &self.transport {
        Transport::Tcp { host, port } => {
            // Existing TCP connection logic using tokio_modbus
            let addr: SocketAddr = format!("{}:{}", host, port).parse()
                .map_err(|e| MasterError::ConnectionFailed(format!("{}", e)))?;
            let ctx = tokio::time::timeout(timeout, tcp::connect_slave(addr, Slave(self.config.slave_id)))
                .await
                .map_err(|_| MasterError::Timeout("connection timeout".into()))?
                .map_err(|e| MasterError::ConnectionFailed(format!("{}", e)))?;
            TransportCtx::Tcp(Arc::new(Mutex::new(ctx)))
        }
        Transport::Rtu(serial_config) => {
            let transport = RtuMasterTransport::connect(serial_config)
                .await
                .map_err(|e| MasterError::ConnectionFailed(e))?;
            TransportCtx::Rtu(transport)
        }
        Transport::Ascii(serial_config) => {
            let transport = AsciiMasterTransport::connect(serial_config)
                .await
                .map_err(|e| MasterError::ConnectionFailed(e))?;
            TransportCtx::Ascii(transport)
        }
        Transport::RtuOverTcp { host, port } => {
            let transport = RtuTcpMasterTransport::connect(host, *port, timeout)
                .await
                .map_err(|e| MasterError::ConnectionFailed(e))?;
            TransportCtx::RtuTcp(transport)
        }
    };

    self.transport_ctx = Some(ctx);
    self.state = MasterState::Connected;
    Ok(())
}
```

- [ ] **Step 3: Add a unified read method that dispatches on transport**

For RTU/ASCII/RtuOverTcp, build the request PDU and use the transport's `request()` method:

```rust
async fn execute_read_via_transport(
    transport: &TransportCtx,
    slave_id: u8,
    function: ReadFunction,
    start_address: u16,
    quantity: u16,
    timeout: Duration,
) -> Result<ReadResult, MasterError> {
    let fc: u8 = match function {
        ReadFunction::ReadCoils => 0x01,
        ReadFunction::ReadDiscreteInputs => 0x02,
        ReadFunction::ReadHoldingRegisters => 0x03,
        ReadFunction::ReadInputRegisters => 0x04,
    };

    let mut pdu = vec![fc];
    pdu.extend_from_slice(&start_address.to_be_bytes());
    pdu.extend_from_slice(&quantity.to_be_bytes());

    let response_pdu = match transport {
        TransportCtx::Tcp(ctx) => {
            // Use existing tokio_modbus path
            let mut ctx = ctx.lock().await;
            ctx.set_slave(Slave(slave_id));
            return execute_read(&mut ctx, function, start_address, quantity, timeout).await;
        }
        TransportCtx::Rtu(t) => t.request(slave_id, &pdu, timeout).await,
        TransportCtx::Ascii(t) => t.request(slave_id, &pdu, timeout).await,
        TransportCtx::RtuTcp(t) => t.request(slave_id, &pdu, timeout).await,
    }.map_err(|e| MasterError::Transport(e))?;

    // Parse response PDU
    parse_read_response(function, &response_pdu, quantity)
}
```

- [ ] **Step 4: Build and test**

Run: `cargo build -p modbussim-core && cargo test -p modbussim-core`
Expected: Build succeeds, existing tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/modbussim-core/src/master.rs
git commit -m "refactor(core): MasterConnection dispatches on Transport for connect/read/write"
```

---

## Task 10: Update Tauri Commands for Transport Support

**Files:**
- Modify: `crates/modbussim-app/src/commands.rs`
- Modify: `crates/modbusmaster-app/src/commands.rs`

- [ ] **Step 1: Update slave create_slave_connection command**

Change the request struct to accept a transport configuration:

```rust
#[derive(Debug, Deserialize)]
pub struct CreateSlaveConnectionRequest {
    pub transport: TransportRequest,
    pub init_mode: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransportRequest {
    Tcp { port: u16 },
    Rtu { serial_port: String, baud_rate: u32, data_bits: u8, stop_bits: u8, parity: String },
    Ascii { serial_port: String, baud_rate: u32, data_bits: u8, stop_bits: u8, parity: String },
    RtuOverTcp { host: String, port: u16 },
}
```

Convert `TransportRequest` to `Transport` in the command handler:

```rust
fn to_transport(req: &TransportRequest) -> Transport {
    match req {
        TransportRequest::Tcp { port } => Transport::Tcp { host: "0.0.0.0".into(), port: *port },
        TransportRequest::Rtu { serial_port, baud_rate, data_bits, stop_bits, parity } => {
            Transport::Rtu(SerialConfig {
                port: serial_port.clone(),
                baud_rate: *baud_rate,
                data_bits: *data_bits,
                stop_bits: *stop_bits,
                parity: parse_parity(parity),
            })
        }
        // ... similar for Ascii, RtuOverTcp
    }
}
```

- [ ] **Step 2: Add serial port enumeration command to both apps**

```rust
#[tauri::command]
pub fn list_serial_ports() -> Result<Vec<transport::SerialPortInfo>, String> {
    transport::list_serial_ports()
}
```

Register in both apps' invoke_handler.

- [ ] **Step 3: Update master create_master_connection similarly**

Add transport parameter to the master connection creation request.

- [ ] **Step 4: Build both apps**

Run: `cargo build --workspace`
Expected: Clean compilation

- [ ] **Step 5: Commit**

```bash
git add crates/modbussim-app/src/commands.rs crates/modbussim-app/src/lib.rs \
        crates/modbusmaster-app/src/commands.rs crates/modbusmaster-app/src/lib.rs
git commit -m "feat(apps): update Tauri commands to accept Transport config and list serial ports"
```

---

## Task 11: Update Frontend UIs with Transport Selection

**Files:**
- Modify: `frontend/src/components/Toolbar.vue`
- Modify: `master-frontend/src/components/Toolbar.vue`

- [ ] **Step 1: Add transport type selector to slave Toolbar**

In the new connection modal, add a transport type dropdown before the existing port field:

```html
<div class="modal-field">
  <label>传输类型</label>
  <select v-model="newConnTransport">
    <option value="tcp">TCP</option>
    <option value="rtu">RTU (Serial)</option>
    <option value="ascii">ASCII (Serial)</option>
    <option value="rtu_over_tcp">RTU over TCP</option>
  </select>
</div>
```

Conditionally show TCP or serial config fields:

```html
<!-- TCP / RTU over TCP -->
<template v-if="newConnTransport === 'tcp' || newConnTransport === 'rtu_over_tcp'">
  <div class="modal-field">
    <label>端口号</label>
    <input v-model.number="newConnPort" type="number" min="1" max="65535" />
  </div>
</template>

<!-- RTU / ASCII (Serial) -->
<template v-if="newConnTransport === 'rtu' || newConnTransport === 'ascii'">
  <div class="modal-field">
    <label>串口</label>
    <select v-model="serialPort">
      <option v-for="p in serialPorts" :key="p.name" :value="p.name">
        {{ p.name }} {{ p.description ? `(${p.description})` : '' }}
      </option>
    </select>
    <button class="btn-sm" @click="refreshSerialPorts">刷新</button>
  </div>
  <div class="modal-field">
    <label>波特率</label>
    <select v-model.number="baudRate">
      <option :value="9600">9600</option>
      <option :value="19200">19200</option>
      <option :value="38400">38400</option>
      <option :value="57600">57600</option>
      <option :value="115200">115200</option>
    </select>
  </div>
  <div class="modal-field">
    <label>数据位</label>
    <select v-model.number="dataBits">
      <option :value="7">7</option>
      <option :value="8">8</option>
    </select>
  </div>
  <div class="modal-field">
    <label>停止位</label>
    <select v-model.number="stopBits">
      <option :value="1">1</option>
      <option :value="2">2</option>
    </select>
  </div>
  <div class="modal-field">
    <label>校验</label>
    <select v-model="parityMode">
      <option value="none">None</option>
      <option value="odd">Odd</option>
      <option value="even">Even</option>
    </select>
  </div>
</template>
```

Add the data refs and serial port refresh function:

```typescript
const newConnTransport = ref('tcp')
const serialPort = ref('')
const baudRate = ref(9600)
const dataBits = ref(8)
const stopBits = ref(1)
const parityMode = ref('none')
const serialPorts = ref<{ name: string; description: string }[]>([])

async function refreshSerialPorts() {
  try {
    serialPorts.value = await invoke('list_serial_ports')
  } catch (e) {
    await showAlert(String(e))
  }
}
```

Update the create connection invoke call to send the transport object:

```typescript
async function createConnection() {
  let transport: Record<string, unknown>
  if (newConnTransport.value === 'tcp') {
    transport = { type: 'tcp', port: newConnPort.value }
  } else if (newConnTransport.value === 'rtu' || newConnTransport.value === 'ascii') {
    transport = {
      type: newConnTransport.value,
      serial_port: serialPort.value,
      baud_rate: baudRate.value,
      data_bits: dataBits.value,
      stop_bits: stopBits.value,
      parity: parityMode.value,
    }
  } else {
    transport = { type: 'rtu_over_tcp', host: '0.0.0.0', port: newConnPort.value }
  }

  await invoke('create_slave_connection', {
    request: { transport, init_mode: newConnInitMode.value }
  })
}
```

- [ ] **Step 2: Apply same pattern to master Toolbar**

Add the same transport type selector and serial config fields to the master Toolbar's connection modal. The master additionally shows host/IP for TCP and RTU-over-TCP modes.

- [ ] **Step 3: Build both frontends**

```bash
cd frontend && npm run build && cd ../master-frontend && npm run build
```

Expected: Both build successfully

- [ ] **Step 4: Commit**

```bash
git add frontend/src/components/Toolbar.vue master-frontend/src/components/Toolbar.vue
git commit -m "feat(frontend): add transport type selector and serial port config to both toolbars"
```

---

## Task 12: Integration Verification

- [ ] **Step 1: Run full Rust test suite**

Run: `cargo test --workspace`
Expected: All new tests pass; only pre-existing failures

- [ ] **Step 2: Build everything**

```bash
cargo build --workspace
cd frontend && npm run build
cd ../master-frontend && npm run build
```

Expected: All builds succeed

- [ ] **Step 3: Verify serial port enumeration compiles and runs**

Run: `cargo test -p modbussim-core transport::tests::test_list_serial_ports_does_not_panic`
Expected: Pass (returns empty list on systems without serial ports)

- [ ] **Step 4: Run git log to verify commits**

```bash
git log --oneline feature/phase1-architecture-foundation..HEAD
```

- [ ] **Step 5: Commit any final cleanup**

```bash
git status
# If needed:
git add -A && git commit -m "chore: Phase 2 final cleanup"
```
