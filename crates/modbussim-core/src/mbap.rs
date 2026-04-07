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
            length: (pdu_len + 1) as u16,
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

    pub fn pdu_len(&self) -> usize {
        if self.length > 0 {
            (self.length - 1) as usize
        } else {
            0
        }
    }
}

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
        assert_eq!(header.length, 6);
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
        let pdu = vec![0x03, 0x00, 0x00, 0x00, 0x0A];
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
        let buf = vec![0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x01];
        let mut cursor = Cursor::new(buf);
        let result = read_frame(&mut cursor);
        assert!(result.is_err());
    }
}
