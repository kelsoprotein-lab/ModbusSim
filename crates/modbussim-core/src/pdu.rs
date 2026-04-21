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

#[derive(Debug, Clone)]
pub enum ResponseData {
    ReadBits(Vec<bool>),
    ReadRegisters(Vec<u16>),
    WriteSingleCoil { address: u16, value: bool },
    WriteSingleRegister { address: u16, value: u16 },
    WriteMultiple { address: u16, quantity: u16 },
}

pub fn parse_request_pdu(pdu: &[u8]) -> Result<ModbusRequest, String> {
    if pdu.is_empty() {
        return Err("PDU is empty".to_string());
    }
    let fc = pdu[0];
    let data = &pdu[1..];
    match fc {
        0x01 => {
            if data.len() < 4 {
                return Err(format!("FC01: expected 4 bytes, got {}", data.len()));
            }
            Ok(ModbusRequest::ReadCoils {
                address: u16::from_be_bytes([data[0], data[1]]),
                quantity: u16::from_be_bytes([data[2], data[3]]),
            })
        }
        0x02 => {
            if data.len() < 4 {
                return Err(format!("FC02: expected 4 bytes, got {}", data.len()));
            }
            Ok(ModbusRequest::ReadDiscreteInputs {
                address: u16::from_be_bytes([data[0], data[1]]),
                quantity: u16::from_be_bytes([data[2], data[3]]),
            })
        }
        0x03 => {
            if data.len() < 4 {
                return Err(format!("FC03: expected 4 bytes, got {}", data.len()));
            }
            Ok(ModbusRequest::ReadHoldingRegisters {
                address: u16::from_be_bytes([data[0], data[1]]),
                quantity: u16::from_be_bytes([data[2], data[3]]),
            })
        }
        0x04 => {
            if data.len() < 4 {
                return Err(format!("FC04: expected 4 bytes, got {}", data.len()));
            }
            Ok(ModbusRequest::ReadInputRegisters {
                address: u16::from_be_bytes([data[0], data[1]]),
                quantity: u16::from_be_bytes([data[2], data[3]]),
            })
        }
        0x05 => {
            if data.len() < 4 {
                return Err(format!("FC05: expected 4 bytes, got {}", data.len()));
            }
            let address = u16::from_be_bytes([data[0], data[1]]);
            let raw = u16::from_be_bytes([data[2], data[3]]);
            let value = match raw {
                0xFF00 => true,
                0x0000 => false,
                _ => return Err(format!("FC05: invalid coil value 0x{:04X}", raw)),
            };
            Ok(ModbusRequest::WriteSingleCoil { address, value })
        }
        0x06 => {
            if data.len() < 4 {
                return Err(format!("FC06: expected 4 bytes, got {}", data.len()));
            }
            Ok(ModbusRequest::WriteSingleRegister {
                address: u16::from_be_bytes([data[0], data[1]]),
                value: u16::from_be_bytes([data[2], data[3]]),
            })
        }
        0x0F => {
            // address(2) + quantity(2) + byte_count(1) + bytes
            if data.len() < 5 {
                return Err(format!("FC0F: too short, got {}", data.len()));
            }
            let address = u16::from_be_bytes([data[0], data[1]]);
            let quantity = u16::from_be_bytes([data[2], data[3]]) as usize;
            let byte_count = data[4] as usize;
            if data.len() < 5 + byte_count {
                return Err(format!("FC0F: byte_count mismatch"));
            }
            let coil_bytes = &data[5..5 + byte_count];
            let mut values = Vec::with_capacity(quantity);
            for i in 0..quantity {
                let byte_idx = i / 8;
                let bit_idx = i % 8;
                let bit = if byte_idx < coil_bytes.len() {
                    (coil_bytes[byte_idx] >> bit_idx) & 1 == 1
                } else {
                    false
                };
                values.push(bit);
            }
            Ok(ModbusRequest::WriteMultipleCoils { address, values })
        }
        0x10 => {
            // address(2) + quantity(2) + byte_count(1) + words
            if data.len() < 5 {
                return Err(format!("FC10: too short, got {}", data.len()));
            }
            let address = u16::from_be_bytes([data[0], data[1]]);
            let quantity = u16::from_be_bytes([data[2], data[3]]) as usize;
            let byte_count = data[4] as usize;
            if data.len() < 5 + byte_count {
                return Err(format!("FC10: byte_count mismatch"));
            }
            if byte_count != quantity * 2 {
                return Err(format!(
                    "FC10: byte_count {} != quantity*2 {}",
                    byte_count,
                    quantity * 2
                ));
            }
            let reg_bytes = &data[5..5 + byte_count];
            let mut values = Vec::with_capacity(quantity);
            for i in 0..quantity {
                values.push(u16::from_be_bytes([reg_bytes[i * 2], reg_bytes[i * 2 + 1]]));
            }
            Ok(ModbusRequest::WriteMultipleRegisters { address, values })
        }
        _ => Err(format!("Unsupported function code: 0x{:02X}", fc)),
    }
}

pub fn build_response_pdu(fc: u8, data: &ResponseData) -> Vec<u8> {
    match data {
        ResponseData::ReadBits(bits) => {
            let byte_count = (bits.len() + 7) / 8;
            let mut packed = vec![0u8; byte_count];
            for (i, &bit) in bits.iter().enumerate() {
                if bit {
                    packed[i / 8] |= 1 << (i % 8);
                }
            }
            let mut pdu = Vec::with_capacity(2 + byte_count);
            pdu.push(fc);
            pdu.push(byte_count as u8);
            pdu.extend_from_slice(&packed);
            pdu
        }
        ResponseData::ReadRegisters(regs) => {
            let byte_count = regs.len() * 2;
            let mut pdu = Vec::with_capacity(2 + byte_count);
            pdu.push(fc);
            pdu.push(byte_count as u8);
            for &r in regs {
                pdu.extend_from_slice(&r.to_be_bytes());
            }
            pdu
        }
        ResponseData::WriteSingleCoil { address, value } => {
            let val_hi = if *value { 0xFF } else { 0x00 };
            vec![
                fc,
                (address >> 8) as u8,
                (address & 0xFF) as u8,
                val_hi,
                0x00,
            ]
        }
        ResponseData::WriteSingleRegister { address, value } => {
            vec![
                fc,
                (address >> 8) as u8,
                (address & 0xFF) as u8,
                (value >> 8) as u8,
                (value & 0xFF) as u8,
            ]
        }
        ResponseData::WriteMultiple { address, quantity } => {
            vec![
                fc,
                (address >> 8) as u8,
                (address & 0xFF) as u8,
                (quantity >> 8) as u8,
                (quantity & 0xFF) as u8,
            ]
        }
    }
}

pub fn build_exception_pdu(fc: u8, exception_code: u8) -> Vec<u8> {
    vec![fc | 0x80, exception_code]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_fc03_read_holding_registers() {
        let pdu = [0x03, 0x00, 0x6B, 0x00, 0x03];
        let req = parse_request_pdu(&pdu).unwrap();
        match req {
            ModbusRequest::ReadHoldingRegisters { address, quantity } => {
                assert_eq!(address, 0x6B);
                assert_eq!(quantity, 3);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_parse_fc05_write_single_coil_on() {
        let pdu = [0x05, 0x00, 0x0A, 0xFF, 0x00];
        let req = parse_request_pdu(&pdu).unwrap();
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
        let pdu = [0x05, 0x00, 0x0A, 0x00, 0x00];
        let req = parse_request_pdu(&pdu).unwrap();
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
        let pdu = [0x10, 0x00, 0x01, 0x00, 0x02, 0x04, 0x00, 0x0A, 0x01, 0x02];
        let req = parse_request_pdu(&pdu).unwrap();
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
        let result = parse_request_pdu(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_unsupported_fc() {
        let result = parse_request_pdu(&[0x2B, 0x00]);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_response_read_registers() {
        let data = ResponseData::ReadRegisters(vec![1, 2, 3]);
        let pdu = build_response_pdu(0x03, &data);
        assert_eq!(pdu, vec![0x03, 0x06, 0x00, 0x01, 0x00, 0x02, 0x00, 0x03]);
    }

    #[test]
    fn test_build_response_read_bits() {
        // bits: T,F,T,T,F,F,F,F,T — 9 bits → 2 bytes
        // byte0: bit0=T(1), bit1=F(0), bit2=T(1), bit3=T(1), bit4=F, bit5=F, bit6=F, bit7=F → 0b00001101 = 0x0D
        // byte1: bit0=T(1) → 0b00000001 = 0x01
        let bits = vec![true, false, true, true, false, false, false, false, true];
        let data = ResponseData::ReadBits(bits);
        let pdu = build_response_pdu(0x01, &data);
        assert_eq!(pdu, vec![0x01, 2, 0x0D, 0x01]);
    }

    #[test]
    fn test_build_exception_pdu() {
        let pdu = build_exception_pdu(0x03, 0x02);
        assert_eq!(pdu, vec![0x83, 0x02]);
    }

    #[test]
    fn test_build_response_write_single_coil() {
        let data = ResponseData::WriteSingleCoil {
            address: 10,
            value: true,
        };
        let pdu = build_response_pdu(0x05, &data);
        assert_eq!(pdu, vec![0x05, 0x00, 0x0A, 0xFF, 0x00]);
    }

    #[test]
    fn test_build_response_write_multiple() {
        let data = ResponseData::WriteMultiple {
            address: 1,
            quantity: 10,
        };
        let pdu = build_response_pdu(0x10, &data);
        assert_eq!(pdu, vec![0x10, 0x00, 0x01, 0x00, 0x0A]);
    }
}
