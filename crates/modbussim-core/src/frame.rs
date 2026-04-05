//! Modbus RTU and ASCII frame encode/decode.

use crate::tools;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Convert a nibble (0–15) to its uppercase hex ASCII character.
fn hex_char_upper(nibble: u8) -> u8 {
    match nibble {
        0..=9 => b'0' + nibble,
        _ => b'A' + (nibble - 10),
    }
}

/// Convert an ASCII hex character to its numeric value (0–15).
/// Accepts both upper and lower case.
fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'A'..=b'F' => Some(c - b'A' + 10),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// RTU
// ---------------------------------------------------------------------------

/// Encode a Modbus RTU frame: [slave_id | PDU | CRC16_lo | CRC16_hi].
pub fn encode_rtu(slave_id: u8, pdu: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(1 + pdu.len());
    raw.push(slave_id);
    raw.extend_from_slice(pdu);
    tools::append_crc16(&raw)
}

/// Decode a Modbus RTU frame.
///
/// Validates minimum length (≥ 4 bytes) and CRC, then extracts slave_id and PDU.
pub fn decode_rtu(data: &[u8]) -> Result<RtuFrame, String> {
    if data.len() < 4 {
        return Err(format!(
            "RTU frame too short: {} bytes (minimum 4)",
            data.len()
        ));
    }
    if !tools::verify_crc16(data) {
        return Err("RTU frame CRC check failed".to_string());
    }
    let slave_id = data[0];
    let pdu = data[1..data.len() - 2].to_vec();
    Ok(RtuFrame { slave_id, pdu })
}

// ---------------------------------------------------------------------------
// ASCII
// ---------------------------------------------------------------------------

/// Encode a Modbus ASCII frame: ':' + HEX(slave_id + PDU + LRC) + "\r\n".
pub fn encode_ascii(slave_id: u8, pdu: &[u8]) -> Vec<u8> {
    // Build raw bytes: slave_id | PDU, then append LRC.
    let mut raw = Vec::with_capacity(1 + pdu.len());
    raw.push(slave_id);
    raw.extend_from_slice(pdu);
    let with_lrc = tools::append_lrc(&raw);

    // ':' + 2 hex chars per byte + "\r\n"
    let mut frame = Vec::with_capacity(1 + with_lrc.len() * 2 + 2);
    frame.push(b':');
    for byte in &with_lrc {
        frame.push(hex_char_upper(byte >> 4));
        frame.push(hex_char_upper(byte & 0x0F));
    }
    frame.push(b'\r');
    frame.push(b'\n');
    frame
}

/// Decode a Modbus ASCII frame.
///
/// Validates ':' prefix and CRLF suffix, hex-decodes the body, verifies LRC,
/// then extracts slave_id and PDU.
pub fn decode_ascii(data: &[u8]) -> Result<AsciiFrame, String> {
    // Validate framing characters.
    if data.first() != Some(&b':') {
        return Err("ASCII frame missing ':' prefix".to_string());
    }
    if data.len() < 2 || data[data.len() - 2] != b'\r' || data[data.len() - 1] != b'\n' {
        return Err("ASCII frame missing CRLF suffix".to_string());
    }

    // Hex content sits between ':' and "\r\n".
    let hex_bytes = &data[1..data.len() - 2];

    if hex_bytes.len() % 2 != 0 {
        return Err("ASCII frame has odd number of hex characters".to_string());
    }
    if hex_bytes.len() < 6 {
        // Minimum: 1 byte slave_id + 1 byte PDU + 1 byte LRC = 3 bytes → 6 hex chars
        return Err("ASCII frame hex content too short".to_string());
    }

    // Decode hex pairs.
    let mut decoded = Vec::with_capacity(hex_bytes.len() / 2);
    for chunk in hex_bytes.chunks(2) {
        let hi = hex_val(chunk[0])
            .ok_or_else(|| format!("invalid hex character: {}", chunk[0] as char))?;
        let lo = hex_val(chunk[1])
            .ok_or_else(|| format!("invalid hex character: {}", chunk[1] as char))?;
        decoded.push((hi << 4) | lo);
    }

    // decoded = [slave_id | PDU... | LRC]
    if !tools::verify_lrc(&decoded) {
        return Err("ASCII frame LRC check failed".to_string());
    }

    let slave_id = decoded[0];
    let pdu = decoded[1..decoded.len() - 1].to_vec();
    Ok(AsciiFrame { slave_id, pdu })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools;

    const TEST_PDU: &[u8] = &[0x03, 0x00, 0x00, 0x00, 0x0A];

    // 1. RTU frame structure
    #[test]
    fn test_encode_rtu_structure() {
        let slave_id = 0x01;
        let frame = encode_rtu(slave_id, TEST_PDU);
        assert_eq!(frame[0], slave_id);
        assert_eq!(&frame[1..6], TEST_PDU);
        assert_eq!(frame.len(), 1 + 5 + 2);
    }

    // 2. RTU CRC valid after encode
    #[test]
    fn test_encode_rtu_crc_valid() {
        let frame = encode_rtu(0x01, TEST_PDU);
        assert!(tools::verify_crc16(&frame));
    }

    // 3. RTU roundtrip
    #[test]
    fn test_decode_rtu_roundtrip() {
        let slave_id = 0x01;
        let frame = encode_rtu(slave_id, TEST_PDU);
        let decoded = decode_rtu(&frame).expect("decode should succeed");
        assert_eq!(decoded.slave_id, slave_id);
        assert_eq!(decoded.pdu, TEST_PDU);
    }

    // 4. RTU too short
    #[test]
    fn test_decode_rtu_too_short() {
        let result = decode_rtu(&[0x01, 0x03]);
        assert!(result.is_err());
    }

    // 5. RTU bad CRC
    #[test]
    fn test_decode_rtu_bad_crc() {
        let mut frame = encode_rtu(0x01, TEST_PDU);
        *frame.last_mut().unwrap() ^= 0xFF; // corrupt last byte
        let result = decode_rtu(&frame);
        assert!(result.is_err());
    }

    // 6. ASCII frame structure
    #[test]
    fn test_encode_ascii_structure() {
        let frame = encode_ascii(0x01, TEST_PDU);
        assert_eq!(frame[0], b':');
        assert_eq!(frame[frame.len() - 2], b'\r');
        assert_eq!(frame[frame.len() - 1], b'\n');
    }

    // 7. ASCII hex content correctness
    #[test]
    fn test_encode_ascii_hex_content() {
        // slave_id=0x01, PDU=[0x03,0x00,0x00,0x00,0x0A]
        // raw = [0x01,0x03,0x00,0x00,0x00,0x0A]
        // raw = [0x01,0x03,0x00,0x00,0x00,0x0A] (6 bytes)
        // LRC = (0x100 - (0x01+0x03+0x00+0x00+0x00+0x0A)) & 0xFF
        //      = (0x100 - 0x0E) & 0xFF = 0xF2
        // Expected hex body: "01030000000AF2" (12 hex for data + 2 for LRC = 14)
        let frame = encode_ascii(0x01, TEST_PDU);
        let body = std::str::from_utf8(&frame[1..frame.len() - 2]).unwrap();
        assert_eq!(body, "01030000000AF2");
    }

    // 8. ASCII roundtrip
    #[test]
    fn test_decode_ascii_roundtrip() {
        let slave_id = 0x01;
        let frame = encode_ascii(slave_id, TEST_PDU);
        let decoded = decode_ascii(&frame).expect("decode should succeed");
        assert_eq!(decoded.slave_id, slave_id);
        assert_eq!(decoded.pdu, TEST_PDU);
    }

    // 9. ASCII bad LRC
    #[test]
    fn test_decode_ascii_bad_lrc() {
        let mut frame = encode_ascii(0x01, TEST_PDU);
        // Corrupt the last two hex chars (LRC) before CRLF.
        let len = frame.len();
        frame[len - 4] = b'0';
        frame[len - 3] = b'0';
        let result = decode_ascii(&frame);
        assert!(result.is_err());
    }
}
