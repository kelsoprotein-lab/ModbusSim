use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Direction of the Modbus communication frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Received (inbound)
    Rx,
    /// Sent (outbound)
    Tx,
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Direction::Rx => write!(f, "RX"),
            Direction::Tx => write!(f, "TX"),
        }
    }
}

/// Modbus function codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FunctionCode {
    ReadCoils = 0x01,
    ReadDiscreteInputs = 0x02,
    ReadHoldingRegisters = 0x03,
    ReadInputRegisters = 0x04,
    WriteSingleCoil = 0x05,
    WriteSingleRegister = 0x06,
    WriteMultipleCoils = 0x0F,
    WriteMultipleRegisters = 0x10,
}

impl FunctionCode {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::ReadCoils),
            0x02 => Some(Self::ReadDiscreteInputs),
            0x03 => Some(Self::ReadHoldingRegisters),
            0x04 => Some(Self::ReadInputRegisters),
            0x05 => Some(Self::WriteSingleCoil),
            0x06 => Some(Self::WriteSingleRegister),
            0x0F => Some(Self::WriteMultipleCoils),
            0x10 => Some(Self::WriteMultipleRegisters),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::ReadCoils => "FC01",
            Self::ReadDiscreteInputs => "FC02",
            Self::ReadHoldingRegisters => "FC03",
            Self::ReadInputRegisters => "FC04",
            Self::WriteSingleCoil => "FC05",
            Self::WriteSingleRegister => "FC06",
            Self::WriteMultipleCoils => "FC15",
            Self::WriteMultipleRegisters => "FC16",
        }
    }
}

/// A single entry in the communication log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Timestamp when the frame was captured.
    pub timestamp: DateTime<Utc>,
    /// Direction: received or sent.
    pub direction: Direction,
    /// Modbus function code.
    pub function_code: FunctionCode,
    /// Human-readable detail description.
    pub detail: String,
    /// Raw bytes of the frame (optional, for advanced debugging).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_bytes: Option<Vec<u8>>,
}

impl LogEntry {
    /// Create a new log entry with the current timestamp.
    pub fn new(
        direction: Direction,
        function_code: FunctionCode,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            direction,
            function_code,
            detail: detail.into(),
            raw_bytes: None,
        }
    }

    /// Create a new log entry with raw bytes included.
    pub fn with_raw_bytes(
        direction: Direction,
        function_code: FunctionCode,
        detail: impl Into<String>,
        raw_bytes: Vec<u8>,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            direction,
            function_code,
            detail: detail.into(),
            raw_bytes: Some(raw_bytes),
        }
    }

    /// Format for CSV export.
    pub fn to_csv_row(&self) -> String {
        let timestamp = self.timestamp.format("%Y-%m-%d %H:%M:%S%.3f");
        let direction = self.direction.to_string();
        let fc = self.function_code.name();
        let raw = self
            .raw_bytes
            .as_ref()
            .map(|b| {
                b.iter()
                    .map(|v| format!("{:02X}", v))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();
        format!(
            "\"{}\",{},{},\"{}\",\"{}\"",
            timestamp, direction, fc, self.detail, raw
        )
    }

    /// CSV header row.
    pub fn csv_header() -> &'static str {
        "Timestamp,Direction,Function,Detail,RawBytes"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_entry_creation() {
        let entry = LogEntry::new(Direction::Rx, FunctionCode::ReadHoldingRegisters, "R 0 x10");
        assert_eq!(entry.direction, Direction::Rx);
        assert_eq!(entry.function_code, FunctionCode::ReadHoldingRegisters);
        assert_eq!(entry.detail, "R 0 x10");
        assert!(entry.raw_bytes.is_none());
    }

    #[test]
    fn test_log_entry_with_raw_bytes() {
        let entry = LogEntry::with_raw_bytes(
            Direction::Tx,
            FunctionCode::WriteSingleRegister,
            "W 10 = 42",
            vec![0x01, 0x06, 0x00, 0x0A, 0x00, 0x2A],
        );
        assert!(entry.raw_bytes.is_some());
        assert_eq!(entry.raw_bytes.as_ref().unwrap().len(), 6);
    }

    #[test]
    fn test_function_code_from_u8() {
        assert_eq!(FunctionCode::from_u8(0x01), Some(FunctionCode::ReadCoils));
        assert_eq!(
            FunctionCode::from_u8(0x03),
            Some(FunctionCode::ReadHoldingRegisters)
        );
        assert_eq!(
            FunctionCode::from_u8(0x10),
            Some(FunctionCode::WriteMultipleRegisters)
        );
        assert_eq!(FunctionCode::from_u8(0xFF), None);
    }

    #[test]
    fn test_function_code_name() {
        assert_eq!(FunctionCode::ReadHoldingRegisters.name(), "FC03");
        assert_eq!(FunctionCode::WriteMultipleCoils.name(), "FC15");
    }

    #[test]
    fn test_csv_export() {
        let entry = LogEntry::new(Direction::Rx, FunctionCode::ReadHoldingRegisters, "R 0 x2");
        let row = entry.to_csv_row();
        assert!(row.contains("RX"));
        assert!(row.contains("FC03"));
        assert!(row.contains("R 0 x2"));
    }
}
