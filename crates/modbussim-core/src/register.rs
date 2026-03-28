use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Modbus register types (four areas)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegisterType {
    /// 0x - read/write single bit
    Coil,
    /// 1x - read-only single bit
    DiscreteInput,
    /// 3x - read-only 16-bit
    InputRegister,
    /// 4x - read/write 16-bit
    HoldingRegister,
}

/// Data types for interpreting register values
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataType {
    Bool,
    UInt16,
    Int16,
    UInt32,
    Int32,
    Float32,
}

impl DataType {
    /// Number of 16-bit registers this data type occupies
    pub fn register_count(&self) -> u16 {
        match self {
            DataType::Bool | DataType::UInt16 | DataType::Int16 => 1,
            DataType::UInt32 | DataType::Int32 | DataType::Float32 => 2,
        }
    }
}

/// Byte order for multi-register data types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Endian {
    /// AB CD (most common)
    Big,
    /// CD AB
    Little,
    /// BA DC
    MidBig,
    /// DC BA
    MidLittle,
}

impl Default for Endian {
    fn default() -> Self {
        Endian::Big
    }
}

/// Metadata definition for a register (used for UI display and config export)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterDef {
    pub address: u16,
    pub register_type: RegisterType,
    pub data_type: DataType,
    #[serde(default)]
    pub endian: Endian,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub comment: String,
}

#[derive(Debug, Error)]
pub enum RegisterError {
    #[error("value {0} out of range for {1:?}")]
    ValueOutOfRange(f64, DataType),
    #[error("address {0} not found")]
    AddressNotFound(u16),
    #[error("invalid data for conversion")]
    InvalidData,
}

/// Storage for the four Modbus register areas
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RegisterMap {
    pub coils: HashMap<u16, bool>,
    pub discrete_inputs: HashMap<u16, bool>,
    pub holding_registers: HashMap<u16, u16>,
    pub input_registers: HashMap<u16, u16>,
}

impl RegisterMap {
    pub fn new() -> Self {
        Self::default()
    }

    // --- Coil operations ---

    pub fn read_coils(&self, start: u16, count: u16) -> Vec<bool> {
        (start..start + count)
            .map(|addr| self.coils.get(&addr).copied().unwrap_or(false))
            .collect()
    }

    pub fn write_coil(&mut self, addr: u16, value: bool) {
        self.coils.insert(addr, value);
    }

    pub fn write_coils(&mut self, start: u16, values: &[bool]) {
        for (i, &val) in values.iter().enumerate() {
            self.coils.insert(start + i as u16, val);
        }
    }

    // --- Discrete input operations ---

    pub fn read_discrete_inputs(&self, start: u16, count: u16) -> Vec<bool> {
        (start..start + count)
            .map(|addr| self.discrete_inputs.get(&addr).copied().unwrap_or(false))
            .collect()
    }

    // --- Holding register operations ---

    pub fn read_holding_registers(&self, start: u16, count: u16) -> Vec<u16> {
        (start..start + count)
            .map(|addr| self.holding_registers.get(&addr).copied().unwrap_or(0))
            .collect()
    }

    pub fn write_holding_register(&mut self, addr: u16, value: u16) {
        self.holding_registers.insert(addr, value);
    }

    pub fn write_holding_registers(&mut self, start: u16, values: &[u16]) {
        for (i, &val) in values.iter().enumerate() {
            self.holding_registers.insert(start + i as u16, val);
        }
    }

    // --- Input register operations ---

    pub fn read_input_registers(&self, start: u16, count: u16) -> Vec<u16> {
        (start..start + count)
            .map(|addr| self.input_registers.get(&addr).copied().unwrap_or(0))
            .collect()
    }
}

// --- Data type encoding/decoding with endian support ---

/// Encode a typed value into one or two raw u16 registers
pub fn encode_value(value: f64, data_type: DataType, endian: Endian) -> Result<Vec<u16>, RegisterError> {
    validate_range(value, data_type)?;
    match data_type {
        DataType::Bool => {
            Ok(vec![if value != 0.0 { 1 } else { 0 }])
        }
        DataType::UInt16 => {
            Ok(vec![value as u16])
        }
        DataType::Int16 => {
            Ok(vec![(value as i16) as u16])
        }
        DataType::UInt32 => {
            let raw = (value as u32).to_be_bytes();
            Ok(apply_endian_encode(raw, endian))
        }
        DataType::Int32 => {
            let raw = (value as i32).to_be_bytes();
            Ok(apply_endian_encode(raw, endian))
        }
        DataType::Float32 => {
            let raw = (value as f32).to_be_bytes();
            Ok(apply_endian_encode(raw, endian))
        }
    }
}

/// Decode one or two raw u16 registers into a typed value
pub fn decode_value(registers: &[u16], data_type: DataType, endian: Endian) -> Result<f64, RegisterError> {
    match data_type {
        DataType::Bool => {
            let v = registers.first().ok_or(RegisterError::InvalidData)?;
            Ok(if *v != 0 { 1.0 } else { 0.0 })
        }
        DataType::UInt16 => {
            let v = registers.first().ok_or(RegisterError::InvalidData)?;
            Ok(*v as f64)
        }
        DataType::Int16 => {
            let v = registers.first().ok_or(RegisterError::InvalidData)?;
            Ok((*v as i16) as f64)
        }
        DataType::UInt32 => {
            if registers.len() < 2 {
                return Err(RegisterError::InvalidData);
            }
            let bytes = apply_endian_decode(registers[0], registers[1], endian);
            Ok(u32::from_be_bytes(bytes) as f64)
        }
        DataType::Int32 => {
            if registers.len() < 2 {
                return Err(RegisterError::InvalidData);
            }
            let bytes = apply_endian_decode(registers[0], registers[1], endian);
            Ok(i32::from_be_bytes(bytes) as f64)
        }
        DataType::Float32 => {
            if registers.len() < 2 {
                return Err(RegisterError::InvalidData);
            }
            let bytes = apply_endian_decode(registers[0], registers[1], endian);
            Ok(f32::from_be_bytes(bytes) as f64)
        }
    }
}

/// Validate that a value is within the range of the given data type
pub fn validate_range(value: f64, data_type: DataType) -> Result<(), RegisterError> {
    let valid = match data_type {
        DataType::Bool => value == 0.0 || value == 1.0,
        DataType::UInt16 => value >= 0.0 && value <= u16::MAX as f64 && value.fract() == 0.0,
        DataType::Int16 => value >= i16::MIN as f64 && value <= i16::MAX as f64 && value.fract() == 0.0,
        DataType::UInt32 => value >= 0.0 && value <= u32::MAX as f64 && value.fract() == 0.0,
        DataType::Int32 => value >= i32::MIN as f64 && value <= i32::MAX as f64 && value.fract() == 0.0,
        DataType::Float32 => true, // any f64 that can be cast to f32
    };
    if valid {
        Ok(())
    } else {
        Err(RegisterError::ValueOutOfRange(value, data_type))
    }
}

// Encode 4 bytes (big-endian) into two u16 registers with endian transformation
fn apply_endian_encode(be_bytes: [u8; 4], endian: Endian) -> Vec<u16> {
    let [a, b, c, d] = be_bytes;
    match endian {
        Endian::Big => {
            // AB CD
            vec![u16::from_be_bytes([a, b]), u16::from_be_bytes([c, d])]
        }
        Endian::Little => {
            // CD AB
            vec![u16::from_be_bytes([c, d]), u16::from_be_bytes([a, b])]
        }
        Endian::MidBig => {
            // BA DC
            vec![u16::from_be_bytes([b, a]), u16::from_be_bytes([d, c])]
        }
        Endian::MidLittle => {
            // DC BA
            vec![u16::from_be_bytes([d, c]), u16::from_be_bytes([b, a])]
        }
    }
}

// Decode two u16 registers into 4 big-endian bytes with endian transformation
fn apply_endian_decode(reg0: u16, reg1: u16, endian: Endian) -> [u8; 4] {
    let r0 = reg0.to_be_bytes();
    let r1 = reg1.to_be_bytes();
    match endian {
        Endian::Big => {
            // reg0=AB, reg1=CD → ABCD
            [r0[0], r0[1], r1[0], r1[1]]
        }
        Endian::Little => {
            // reg0=CD, reg1=AB → ABCD
            [r1[0], r1[1], r0[0], r0[1]]
        }
        Endian::MidBig => {
            // reg0=BA, reg1=DC → ABCD
            [r0[1], r0[0], r1[1], r1[0]]
        }
        Endian::MidLittle => {
            // reg0=DC, reg1=BA → ABCD
            [r1[1], r1[0], r0[1], r0[0]]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_type_serde() {
        let json = serde_json::to_string(&RegisterType::HoldingRegister).unwrap();
        assert_eq!(json, "\"holding_register\"");
    }

    #[test]
    fn test_data_type_register_count() {
        assert_eq!(DataType::Bool.register_count(), 1);
        assert_eq!(DataType::UInt16.register_count(), 1);
        assert_eq!(DataType::Float32.register_count(), 2);
        assert_eq!(DataType::UInt32.register_count(), 2);
    }

    // --- Encode/Decode round-trip tests ---

    #[test]
    fn test_uint16_round_trip() {
        let regs = encode_value(1234.0, DataType::UInt16, Endian::Big).unwrap();
        assert_eq!(regs, vec![1234]);
        let val = decode_value(&regs, DataType::UInt16, Endian::Big).unwrap();
        assert_eq!(val, 1234.0);
    }

    #[test]
    fn test_int16_negative() {
        let regs = encode_value(-2.0, DataType::Int16, Endian::Big).unwrap();
        assert_eq!(regs, vec![0xFFFE]);
        let val = decode_value(&regs, DataType::Int16, Endian::Big).unwrap();
        assert_eq!(val, -2.0);
    }

    #[test]
    fn test_float32_big_endian() {
        // 25.0 in IEEE 754 = 0x41C80000
        let regs = encode_value(25.0, DataType::Float32, Endian::Big).unwrap();
        assert_eq!(regs, vec![0x41C8, 0x0000]);
        let val = decode_value(&regs, DataType::Float32, Endian::Big).unwrap();
        assert!((val - 25.0).abs() < 0.001);
    }

    #[test]
    fn test_float32_little_endian() {
        // 25.0 = 0x41C80000, Little Endian swaps register order: CD AB
        let regs = encode_value(25.0, DataType::Float32, Endian::Little).unwrap();
        assert_eq!(regs, vec![0x0000, 0x41C8]);
        let val = decode_value(&regs, DataType::Float32, Endian::Little).unwrap();
        assert!((val - 25.0).abs() < 0.001);
    }

    #[test]
    fn test_float32_mid_big_endian() {
        // 25.0 = 0x41C80000 → bytes A=0x41 B=0xC8 C=0x00 D=0x00
        // MidBig = BA DC → reg0=0xC841 reg1=0x0000
        let regs = encode_value(25.0, DataType::Float32, Endian::MidBig).unwrap();
        assert_eq!(regs, vec![0xC841, 0x0000]);
        let val = decode_value(&regs, DataType::Float32, Endian::MidBig).unwrap();
        assert!((val - 25.0).abs() < 0.001);
    }

    #[test]
    fn test_uint32_round_trip() {
        let regs = encode_value(70000.0, DataType::UInt32, Endian::Big).unwrap();
        let val = decode_value(&regs, DataType::UInt32, Endian::Big).unwrap();
        assert_eq!(val, 70000.0);
    }

    #[test]
    fn test_int32_negative() {
        let regs = encode_value(-100000.0, DataType::Int32, Endian::Big).unwrap();
        let val = decode_value(&regs, DataType::Int32, Endian::Big).unwrap();
        assert_eq!(val, -100000.0);
    }

    // --- Validation tests ---

    #[test]
    fn test_uint16_out_of_range() {
        assert!(validate_range(70000.0, DataType::UInt16).is_err());
        assert!(validate_range(-1.0, DataType::UInt16).is_err());
        assert!(validate_range(100.5, DataType::UInt16).is_err());
    }

    #[test]
    fn test_int16_out_of_range() {
        assert!(validate_range(40000.0, DataType::Int16).is_err());
        assert!(validate_range(-40000.0, DataType::Int16).is_err());
    }

    #[test]
    fn test_bool_validation() {
        assert!(validate_range(0.0, DataType::Bool).is_ok());
        assert!(validate_range(1.0, DataType::Bool).is_ok());
        assert!(validate_range(2.0, DataType::Bool).is_err());
    }

    // --- RegisterMap tests ---

    #[test]
    fn test_holding_register_read_write() {
        let mut map = RegisterMap::new();
        map.write_holding_register(0, 1234);
        map.write_holding_register(1, 5678);
        let vals = map.read_holding_registers(0, 3);
        assert_eq!(vals, vec![1234, 5678, 0]); // addr 2 defaults to 0
    }

    #[test]
    fn test_coil_read_write() {
        let mut map = RegisterMap::new();
        map.write_coil(0, true);
        map.write_coil(1, false);
        map.write_coil(2, true);
        assert_eq!(map.read_coils(0, 3), vec![true, false, true]);
    }

    #[test]
    fn test_write_multiple_holding_registers() {
        let mut map = RegisterMap::new();
        map.write_holding_registers(10, &[100, 200, 300]);
        assert_eq!(map.read_holding_registers(10, 3), vec![100, 200, 300]);
    }

    #[test]
    fn test_write_multiple_coils() {
        let mut map = RegisterMap::new();
        map.write_coils(0, &[true, false, true, true]);
        assert_eq!(map.read_coils(0, 4), vec![true, false, true, true]);
    }
}
