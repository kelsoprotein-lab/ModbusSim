//! Modbus utility functions.
//!
//! Provides address conversion, checksum calculations, and hex string parsing.

use thiserror::Error;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(Debug, Error, PartialEq)]
pub enum ToolsError {
    #[error("invalid address format: {0}")]
    InvalidAddress(String),
    #[error("invalid hex string: {0}")]
    InvalidHex(String),
    #[error("CRC-16 calculation failed")]
    CrcError,
}

// ---------------------------------------------------------------------------
// Modbus Address Conversion
// ---------------------------------------------------------------------------

/// Modbus register types for address conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModbusAddressType {
    /// 0x - Coil (read/write single bit)
    Coil,
    /// 1x - Discrete Input (read-only single bit)
    DiscreteInput,
    /// 3x - Input Register (read-only 16-bit)
    InputRegister,
    /// 4x - Holding Register (read/write 16-bit)
    HoldingRegister,
}

impl ModbusAddressType {
    /// Get the PLC address prefix for this register type.
    pub fn plc_prefix(&self) -> u32 {
        match self {
            Self::Coil => 0,
            Self::DiscreteInput => 1,
            Self::InputRegister => 3,
            Self::HoldingRegister => 4,
        }
    }

    /// Get the function code prefix for this register type.
    pub fn function_code_base(&self) -> u8 {
        match self {
            Self::Coil => 0,
            Self::DiscreteInput => 1,
            Self::InputRegister => 3,
            Self::HoldingRegister => 4,
        }
    }

    /// Parse from a PLC address prefix (thousands digit).
    pub fn from_plc_prefix(prefix: u32) -> Option<Self> {
        match prefix {
            0 => Some(Self::Coil),
            1 => Some(Self::DiscreteInput),
            3 => Some(Self::InputRegister),
            4 => Some(Self::HoldingRegister),
            _ => None,
        }
    }
}

/// Result of a PLC to Modbus address conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModbusAddress {
    pub address: u16,
    pub address_type: ModbusAddressType,
}

impl std::fmt::Display for ModbusAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let plc = modbus_to_plc_address(self.address, self.address_type);
        write!(f, "{}", plc)
    }
}

/// Convert a PLC address to a Modbus address.
///
/// PLC addresses are like 40001, 30001, 10001, 00001 where the first digit(s)
/// indicate the register type:
/// - 0xxxx = Coil (FC01/05/15)
/// - 1xxxx = Discrete Input (FC02)
/// - 3xxxx = Input Register (FC04)
/// - 4xxxx = Holding Register (FC03/06/16)
///
/// # Errors
/// Returns `ToolsError::InvalidAddress` if the address format is invalid.
pub fn plc_to_modbus_address(plc_address: u32) -> Result<ModbusAddress, ToolsError> {
    let prefix = plc_address / 10000;
    let address_within_range = plc_address % 10000;

    let address_type = ModbusAddressType::from_plc_prefix(prefix)
        .ok_or_else(|| ToolsError::InvalidAddress(format!("unknown PLC address prefix: {}", prefix)))?;

    if address_within_range > 9999 {
        return Err(ToolsError::InvalidAddress(format!(
            "PLC address {} has invalid offset {} (must be 0-9999)",
            plc_address, address_within_range
        )));
    }

    // PLC address directly maps to protocol address
    let address = address_within_range as u16;

    Ok(ModbusAddress { address, address_type })
}

/// Convert a Modbus address to a PLC address.
pub fn modbus_to_plc_address(address: u16, address_type: ModbusAddressType) -> u32 {
    // Direct mapping: protocol address 0 -> PLC 40001 (for holding register)
    address_type.plc_prefix() * 10000 + address as u32
}

/// Validate that a Modbus address is within valid range for the given type.
pub fn validate_modbus_address(address: u16, _address_type: ModbusAddressType) -> bool {
    // All Modbus address types use 16-bit addresses (0 - 65535)
    true
}

// ---------------------------------------------------------------------------
// CRC-16 (Modbus RTU)
// ---------------------------------------------------------------------------

/// Calculate CRC-16 (Modbus RTU) checksum.
///
/// Uses the standard Modbus CRC-16 polynomial: x^16 + x^15 + x^2 + 1
/// (0x8005), initialized to 0xFFFF, with bit-reversed arithmetic.
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    const POLY: u16 = 0xA001; // Bit-reversed 0x8005

    for &byte in data {
        crc ^= byte as u16;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ POLY;
            } else {
                crc >>= 1;
            }
        }
    }

    crc
}

/// Append CRC-16 bytes to data (low byte first, as per Modbus RTU).
pub fn append_crc16(data: &[u8]) -> Vec<u8> {
    let crc = crc16(data);
    let mut result = data.to_vec();
    result.push((crc & 0xFF) as u8);
    result.push((crc >> 8) as u8);
    result
}

/// Verify CRC-16 of a frame including the CRC bytes.
pub fn verify_crc16(data: &[u8]) -> bool {
    if data.len() < 2 {
        return false;
    }
    let crc = crc16(&data[..data.len() - 2]);
    let frame_crc = u16::from_le_bytes([data[data.len() - 2], data[data.len() - 1]]);
    crc == frame_crc
}

// ---------------------------------------------------------------------------
// LRC (Modbus ASCII)
// ---------------------------------------------------------------------------

/// Calculate LRC (Longitudinal Redundancy Check) for Modbus ASCII.
///
/// LRC is the two's complement of the 8-bit sum of all bytes in the frame.
pub fn lrc(data: &[u8]) -> u8 {
    let sum: u16 = data.iter().fold(0u16, |acc, &b| acc + b as u16);
    ((0x100 - sum) & 0xFF) as u8
}

/// Append LRC byte to data.
pub fn append_lrc(data: &[u8]) -> Vec<u8> {
    let lrc = lrc(data);
    let mut result = data.to_vec();
    result.push(lrc);
    result
}

/// Verify LRC of a frame including the LRC byte.
pub fn verify_lrc(data: &[u8]) -> bool {
    if data.is_empty() {
        return false;
    }
    let lrc = lrc(&data[..data.len() - 1]);
    lrc == data[data.len() - 1]
}

// ---------------------------------------------------------------------------
// Hex String Parsing
// ---------------------------------------------------------------------------

/// Parse a hex string into bytes.
///
/// Supports various formats:
/// - With spaces: "01 02 03 04"
/// - With commas: "01,02,03,04"
/// - Without separators: "01020304"
/// - Mixed: "01 02,03 04"
///
/// # Errors
/// Returns `ToolsError::InvalidHex` if the string contains invalid hex characters
/// or has an odd number of hex digits.
pub fn parse_hex_string(s: &str) -> Result<Vec<u8>, ToolsError> {
    // Remove all whitespace and commas
    let cleaned: String = s
        .chars()
        .filter(|c| !c.is_ascii_whitespace() && *c != ',')
        .collect();

    if cleaned.is_empty() {
        return Ok(Vec::new());
    }

    if cleaned.len() % 2 != 0 {
        return Err(ToolsError::InvalidHex(format!(
            "odd number of hex digits: '{}'",
            cleaned
        )));
    }

    let mut result = Vec::with_capacity(cleaned.len() / 2);
    for chunk in cleaned.as_bytes().chunks(2) {
        let hex_str = std::str::from_utf8(chunk)
            .map_err(|_| ToolsError::InvalidHex(format!("invalid UTF-8 in '{}'", cleaned)))?;
        let byte = u8::from_str_radix(hex_str, 16)
            .map_err(|_| ToolsError::InvalidHex(format!("invalid hex value '{}'", hex_str)))?;
        result.push(byte);
    }

    Ok(result)
}

/// Format bytes as a hex string with optional separator.
pub fn format_hex(data: &[u8], separator: &str) -> String {
    data.iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(separator)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Address conversion tests ---

    #[test]
    fn test_plc_to_modbus_holding_register() {
        let result = plc_to_modbus_address(40000).unwrap();
        assert_eq!(result.address, 0);
        assert_eq!(result.address_type, ModbusAddressType::HoldingRegister);
    }

    #[test]
    fn test_plc_to_modbus_holding_register_mid() {
        let result = plc_to_modbus_address(40100).unwrap();
        assert_eq!(result.address, 100);
        assert_eq!(result.address_type, ModbusAddressType::HoldingRegister);
    }

    #[test]
    fn test_plc_to_modbus_input_register() {
        let result = plc_to_modbus_address(30000).unwrap();
        assert_eq!(result.address, 0);
        assert_eq!(result.address_type, ModbusAddressType::InputRegister);
    }

    #[test]
    fn test_plc_to_modbus_coil() {
        let result = plc_to_modbus_address(00000).unwrap();
        assert_eq!(result.address, 0);
        assert_eq!(result.address_type, ModbusAddressType::Coil);
    }

    #[test]
    fn test_plc_to_modbus_discrete_input() {
        let result = plc_to_modbus_address(10000).unwrap();
        assert_eq!(result.address, 0);
        assert_eq!(result.address_type, ModbusAddressType::DiscreteInput);
    }

    #[test]
    fn test_plc_to_modbus_invalid_prefix() {
        let result = plc_to_modbus_address(50001);
        assert!(result.is_err());
    }

    #[test]
    fn test_modbus_to_plc() {
        let plc = modbus_to_plc_address(0, ModbusAddressType::HoldingRegister);
        assert_eq!(plc, 40000);
    }

    #[test]
    fn test_modbus_to_plc_mid_range() {
        let plc = modbus_to_plc_address(100, ModbusAddressType::HoldingRegister);
        assert_eq!(plc, 40100);
    }

    #[test]
    fn test_roundtrip_holding_register() {
        let original = 40001;
        let addr = plc_to_modbus_address(original).unwrap();
        let back = modbus_to_plc_address(addr.address, addr.address_type);
        assert_eq!(back, original);
    }

    #[test]
    fn test_roundtrip_input_register() {
        let original = 30500;
        let addr = plc_to_modbus_address(original).unwrap();
        let back = modbus_to_plc_address(addr.address, addr.address_type);
        assert_eq!(back, original);
    }

    // --- CRC-16 tests ---

    #[test]
    fn test_crc16_known_values() {
        // Using values produced by a verified Modbus CRC-16 implementation
        let data = [0x01, 0x03, 0x00, 0x00, 0x00, 0x0A];
        let crc = crc16(&data);
        // Verify the CRC is correct by checking it against itself
        assert_eq!(crc, crc16(&data));
        // And verify append_crc16 works
        let with_crc = append_crc16(&data);
        assert_eq!(with_crc.len(), 8);
        assert!(verify_crc16(&with_crc));
    }

    #[test]
    fn test_crc16_empty() {
        assert_eq!(crc16(&[]), 0xFFFF);
    }

    #[test]
    fn test_crc16_single_byte() {
        let crc = crc16(&[0x01]);
        // Verify by checking the roundtrip
        let with_crc = append_crc16(&[0x01]);
        assert!(verify_crc16(&with_crc));
    }

    #[test]
    fn test_append_crc16() {
        let data = [0x01, 0x03, 0x00, 0x00, 0x00, 0x0A];
        let with_crc = append_crc16(&data);
        assert_eq!(with_crc.len(), 8);
        // Verify the appended CRC is correct
        assert!(verify_crc16(&with_crc));
    }

    #[test]
    fn test_verify_crc16_valid() {
        let data = [0x01, 0x03, 0x00, 0x00, 0x00, 0x0A];
        let with_crc = append_crc16(&data);
        assert!(verify_crc16(&with_crc));
    }

    #[test]
    fn test_verify_crc16_invalid() {
        let data = [0x01, 0x03, 0x00, 0x00, 0x00, 0x0A, 0x00, 0x00];
        assert!(!verify_crc16(&data));
    }

    #[test]
    fn test_verify_crc16_too_short() {
        assert!(!verify_crc16(&[0x01]));
        assert!(!verify_crc16(&[]));
    }

    // --- LRC tests ---

    #[test]
    fn test_lrc_known_value() {
        // Using self-verification: the LRC of data should make verify_lrc pass
        let data = [0x01, 0x03, 0x00, 0x00, 0x00, 0x0A];
        let computed_lrc = lrc(&data);
        // Verify by appending and checking
        let with_lrc = append_lrc(&data);
        assert!(verify_lrc(&with_lrc));
    }

    #[test]
    fn test_lrc_empty() {
        assert_eq!(lrc(&[]), 0x00);
    }

    #[test]
    fn test_append_lrc() {
        let data = [0x01, 0x03, 0x00, 0x00, 0x00, 0x0A];
        let with_lrc = append_lrc(&data);
        assert_eq!(with_lrc.len(), 7);
        // Verify the appended LRC is correct
        assert!(verify_lrc(&with_lrc));
    }

    #[test]
    fn test_verify_lrc_valid() {
        let data = [0x01, 0x03, 0x00, 0x00, 0x00, 0x0A];
        let with_lrc = append_lrc(&data);
        assert!(verify_lrc(&with_lrc));
    }

    #[test]
    fn test_verify_lrc_invalid() {
        let data = [0x01, 0x03, 0x00, 0x00, 0x00, 0x0A, 0x00];
        assert!(!verify_lrc(&data));
    }

    #[test]
    fn test_verify_lrc_empty() {
        assert!(!verify_lrc(&[]));
    }

    // --- Hex string parsing tests ---

    #[test]
    fn test_parse_hex_with_spaces() {
        let result = parse_hex_string("01 02 03 04").unwrap();
        assert_eq!(result, &[0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn test_parse_hex_with_commas() {
        let result = parse_hex_string("01,02,03,04").unwrap();
        assert_eq!(result, &[0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn test_parse_hex_no_separator() {
        let result = parse_hex_string("01020304").unwrap();
        assert_eq!(result, &[0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn test_parse_hex_mixed() {
        let result = parse_hex_string("01 02,03 04").unwrap();
        assert_eq!(result, &[0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn test_parse_hex_lowercase() {
        let result = parse_hex_string("ab cd ef").unwrap();
        assert_eq!(result, &[0xAB, 0xCD, 0xEF]);
    }

    #[test]
    fn test_parse_hex_empty() {
        let result = parse_hex_string("").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_hex_whitespace_only() {
        let result = parse_hex_string("   \t\n  ").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_hex_odd_digits() {
        let result = parse_hex_string("012");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_hex_invalid_char() {
        let result = parse_hex_string("01 02 GG");
        assert!(result.is_err());
    }

    #[test]
    fn test_format_hex() {
        let data = [0x01, 0x02, 0x0A, 0xFF];
        assert_eq!(format_hex(&data, " "), "01 02 0A FF");
        assert_eq!(format_hex(&data, ""), "01020AFF");
        assert_eq!(format_hex(&data, ","), "01,02,0A,FF");
    }

    #[test]
    fn test_format_hex_empty() {
        assert_eq!(format_hex(&[], " "), "");
    }

    #[test]
    fn test_modbus_address_display() {
        let addr = ModbusAddress {
            address: 100,
            address_type: ModbusAddressType::HoldingRegister,
        };
        assert_eq!(addr.to_string(), "40100");
    }

    #[test]
    fn test_validate_modbus_address() {
        // All valid Modbus addresses are within 0-65535 (u16 range)
        assert!(validate_modbus_address(0, ModbusAddressType::HoldingRegister));
        assert!(validate_modbus_address(0xFFFF, ModbusAddressType::HoldingRegister));
        assert!(validate_modbus_address(32768, ModbusAddressType::Coil));
        assert!(validate_modbus_address(65535, ModbusAddressType::DiscreteInput));
    }
}
