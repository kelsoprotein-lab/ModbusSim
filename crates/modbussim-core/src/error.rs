use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "category", rename_all = "snake_case")]
pub enum ModbusError {
    // Connection layer
    #[error("connection refused to {addr}")]
    ConnectionRefused { addr: String },

    #[error("connection to {addr} timed out after {timeout_ms}ms")]
    ConnectionTimeout { addr: String, timeout_ms: u64 },

    #[error("connection lost: {reason}")]
    ConnectionLost { reason: String },

    #[error("serial port {port} is busy")]
    SerialPortBusy { port: String },

    #[error("serial port {port} not found")]
    SerialPortNotFound { port: String },

    #[error("permission denied for serial port {port}")]
    SerialPortPermissionDenied { port: String },

    #[error("TLS error: {message}")]
    TlsError { message: String },

    #[error("certificate error: {message}")]
    CertError { message: String },

    // Protocol layer
    #[error("illegal function code: {fc}")]
    IllegalFunction { fc: u8 },

    #[error("illegal data address: addr={addr}, count={count}")]
    IllegalDataAddress { addr: u16, count: u16 },

    #[error("illegal data value: {detail}")]
    IllegalDataValue { detail: String },

    #[error("slave device failure on slave {slave_id}")]
    SlaveDeviceFailure { slave_id: u8 },

    #[error("response timeout from slave {slave_id} for function code {fc}")]
    ResponseTimeout { slave_id: u8, fc: u8 },

    #[error("CRC mismatch: expected={expected:#06x}, actual={actual:#06x}")]
    CrcMismatch { expected: u16, actual: u16 },

    #[error("LRC mismatch: expected={expected:#04x}, actual={actual:#04x}")]
    LrcMismatch { expected: u8, actual: u8 },

    #[error("frame error: {detail}")]
    FrameError { detail: String },

    // Application layer
    #[error("slave ID conflict: {id}")]
    SlaveIdConflict { id: u8 },

    #[error("project file corrupt: {path}")]
    ProjectFileCorrupt { path: String },

    #[error("unsupported project version: {version}")]
    ProjectVersionUnsupported { version: String },

    // Generic
    #[error("I/O error: {message}")]
    Io { message: String },

    #[error("internal error: {message}")]
    Internal { message: String },
}

impl ModbusError {
    pub fn category(&self) -> &'static str {
        match self {
            ModbusError::ConnectionRefused { .. }
            | ModbusError::ConnectionTimeout { .. }
            | ModbusError::ConnectionLost { .. }
            | ModbusError::SerialPortBusy { .. }
            | ModbusError::SerialPortNotFound { .. }
            | ModbusError::SerialPortPermissionDenied { .. }
            | ModbusError::TlsError { .. }
            | ModbusError::CertError { .. } => "connection",

            ModbusError::IllegalFunction { .. }
            | ModbusError::IllegalDataAddress { .. }
            | ModbusError::IllegalDataValue { .. }
            | ModbusError::SlaveDeviceFailure { .. }
            | ModbusError::ResponseTimeout { .. }
            | ModbusError::CrcMismatch { .. }
            | ModbusError::LrcMismatch { .. }
            | ModbusError::FrameError { .. } => "protocol",

            ModbusError::SlaveIdConflict { .. }
            | ModbusError::ProjectFileCorrupt { .. }
            | ModbusError::ProjectVersionUnsupported { .. } => "application",

            ModbusError::Io { .. } | ModbusError::Internal { .. } => "generic",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = ModbusError::ConnectionTimeout {
            addr: "192.168.1.1:502".to_string(),
            timeout_ms: 3000,
        };
        let msg = err.to_string();
        assert!(msg.contains("192.168.1.1:502"), "should contain addr");
        assert!(msg.contains("3000"), "should contain timeout_ms");
    }

    #[test]
    fn test_error_category() {
        assert_eq!(
            ModbusError::ConnectionRefused { addr: "x".into() }.category(),
            "connection"
        );
        assert_eq!(
            ModbusError::ConnectionTimeout { addr: "x".into(), timeout_ms: 1 }.category(),
            "connection"
        );
        assert_eq!(
            ModbusError::IllegalFunction { fc: 1 }.category(),
            "protocol"
        );
        assert_eq!(
            ModbusError::CrcMismatch { expected: 0, actual: 1 }.category(),
            "protocol"
        );
        assert_eq!(
            ModbusError::SlaveIdConflict { id: 1 }.category(),
            "application"
        );
        assert_eq!(
            ModbusError::ProjectFileCorrupt { path: "p".into() }.category(),
            "application"
        );
        assert_eq!(ModbusError::Io { message: "err".into() }.category(), "generic");
        assert_eq!(ModbusError::Internal { message: "err".into() }.category(), "generic");
    }

    #[test]
    fn test_error_serialize_json() {
        let err = ModbusError::ConnectionTimeout {
            addr: "10.0.0.1:502".to_string(),
            timeout_ms: 5000,
        };
        let json = serde_json::to_string(&err).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["category"], "connection_timeout");
        assert_eq!(v["addr"], "10.0.0.1:502");
        assert_eq!(v["timeout_ms"], 5000);
    }

    #[test]
    fn test_error_serialize_protocol() {
        let err = ModbusError::CrcMismatch {
            expected: 0x1234,
            actual: 0x5678,
        };
        let json = serde_json::to_string(&err).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["category"], "crc_mismatch");
        assert_eq!(err.category(), "protocol");
    }

    #[test]
    fn test_error_serialize_io() {
        let err = ModbusError::Io { message: "disk full".to_string() };
        let json = serde_json::to_string(&err).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["category"], "io");
        assert_eq!(v["message"], "disk full");
    }
}
