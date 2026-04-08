//! RTU slave server over serial port.
//!
//! Opens a serial port, reads RTU frames delimited by interframe silence,
//! processes Modbus requests against the shared device registry, and sends
//! RTU responses.

use crate::frame;
use crate::log_entry::{Direction, FunctionCode, LogEntry};
use crate::pdu::{
    build_exception_pdu, build_response_pdu, parse_request_pdu, ModbusRequest, ResponseData,
};
use crate::register::RegisterMap;
use crate::slave::{SharedDevices, SharedLogCollector};
use crate::transport::{self, Parity, SerialConfig};

use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::oneshot;
use tokio_serial::SerialPortBuilderExt;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run an RTU slave server on the given serial port.
///
/// Loops reading RTU frames, processing requests, and writing responses until
/// the `shutdown_rx` signal fires or an unrecoverable I/O error occurs.
pub async fn run_rtu_slave(
    config: SerialConfig,
    devices: SharedDevices,
    log_collector: SharedLogCollector,
    shutdown_rx: oneshot::Receiver<()>,
) -> Result<(), String> {
    let parity = convert_parity(&config.parity);
    let data_bits = convert_data_bits(config.data_bits);
    let stop_bits = convert_stop_bits(config.stop_bits);

    let mut port = tokio_serial::new(&config.port, config.baud_rate)
        .parity(parity)
        .data_bits(data_bits)
        .stop_bits(stop_bits)
        .open_native_async()
        .map_err(|e| format!("Failed to open serial port {}: {}", config.port, e))?;

    let interframe_us = transport::rtu_interframe_delay_us(config.baud_rate);
    let interframe = Duration::from_micros(interframe_us);

    // Pin the shutdown receiver so we can select on it.
    tokio::pin!(shutdown_rx);

    let mut buf = vec![0u8; 256]; // RTU frames are at most 256 bytes
    let mut frame_buf: Vec<u8> = Vec::with_capacity(256);

    loop {
        frame_buf.clear();

        // Read the first byte (or detect shutdown).
        let first_byte = tokio::select! {
            _ = &mut shutdown_rx => {
                log::info!("RTU slave shutting down");
                return Ok(());
            }
            result = port.read(&mut buf) => {
                match result {
                    Ok(0) => continue,
                    Ok(n) => {
                        frame_buf.extend_from_slice(&buf[..n]);
                        true
                    }
                    Err(e) => {
                        return Err(format!("Serial port read error: {e}"));
                    }
                }
            }
        };

        if !first_byte {
            continue;
        }

        // Keep reading until interframe silence indicates end-of-frame.
        loop {
            match tokio::time::timeout(interframe, port.read(&mut buf)).await {
                Ok(Ok(0)) => continue,
                Ok(Ok(n)) => {
                    frame_buf.extend_from_slice(&buf[..n]);
                }
                Ok(Err(e)) => {
                    return Err(format!("Serial port read error: {e}"));
                }
                Err(_timeout) => {
                    // Interframe silence elapsed -- frame is complete.
                    break;
                }
            }
        }

        // Decode the RTU frame.
        let rtu_frame = match frame::decode_rtu(&frame_buf) {
            Ok(f) => f,
            Err(e) => {
                log::warn!("RTU decode error: {e}");
                continue;
            }
        };

        let slave_id = rtu_frame.slave_id;
        let request_pdu = &rtu_frame.pdu;

        // Log inbound frame.
        if let Some(fc_val) = request_pdu.first() {
            if let Some(fc) = FunctionCode::from_u8(*fc_val) {
                if let Ok(req) = parse_request_pdu(request_pdu) {
                    log_if_enabled(&log_collector, Direction::Rx, fc, &format_request(&req));
                }
            }
        }

        // Process the request against the device registry.
        if let Some(response_pdu) = process_request(slave_id, request_pdu, &devices).await {
            // Log outbound response.
            if let Some(fc_val) = request_pdu.first() {
                if let Some(fc) = FunctionCode::from_u8(*fc_val) {
                    let detail = if response_pdu.first().map_or(false, |b| b & 0x80 != 0) {
                        format!("ERR: exception 0x{:02X}", response_pdu.get(1).copied().unwrap_or(0))
                    } else {
                        "OK".to_string()
                    };
                    log_if_enabled(&log_collector, Direction::Tx, fc, &detail);
                }
            }

            let response_frame = frame::encode_rtu(slave_id, &response_pdu);
            if let Err(e) = port.write_all(&response_frame).await {
                log::error!("Serial port write error: {e}");
                return Err(format!("Serial port write error: {e}"));
            }
        }
        // If process_request returns None, the slave_id was not found -- silently ignore.
    }
}

// ---------------------------------------------------------------------------
// Request processing (pub(crate) for reuse by ascii_slave / rtu_tcp_slave)
// ---------------------------------------------------------------------------

/// Process a Modbus request PDU for the given slave.
///
/// Returns `Some(response_pdu)` if the slave_id exists in the device map,
/// or `None` if the slave is unknown (the request is silently dropped).
pub(crate) async fn process_request(
    slave_id: u8,
    request_pdu: &[u8],
    devices: &SharedDevices,
) -> Option<Vec<u8>> {
    let req = match parse_request_pdu(request_pdu) {
        Ok(r) => r,
        Err(_) => {
            // Unsupported or malformed function code -- respond with Illegal Function.
            let fc = request_pdu.first().copied().unwrap_or(0);
            return Some(build_exception_pdu(fc, 0x01));
        }
    };

    let fc = request_pdu[0];

    let is_write = matches!(
        req,
        ModbusRequest::WriteSingleCoil { .. }
            | ModbusRequest::WriteSingleRegister { .. }
            | ModbusRequest::WriteMultipleCoils { .. }
            | ModbusRequest::WriteMultipleRegisters { .. }
    );

    if is_write {
        let mut devices = devices.write().await;
        let device = devices.get_mut(&slave_id)?;
        match execute_write(&mut device.register_map, &req) {
            Ok(data) => Some(build_response_pdu(fc, &data)),
            Err(exception) => Some(build_exception_pdu(fc, exception)),
        }
    } else {
        let devices = devices.read().await;
        let device = devices.get(&slave_id)?;
        match execute_read(&device.register_map, &req) {
            Ok(data) => Some(build_response_pdu(fc, &data)),
            Err(exception) => Some(build_exception_pdu(fc, exception)),
        }
    }
}

// ---------------------------------------------------------------------------
// Read / Write helpers
// ---------------------------------------------------------------------------

pub(crate) fn execute_read(register_map: &RegisterMap, req: &ModbusRequest) -> Result<ResponseData, u8> {
    match req {
        ModbusRequest::ReadCoils { address, quantity } => {
            validate_quantity(*address, *quantity, 2000)?;
            let bits = register_map.read_coils(*address, *quantity);
            Ok(ResponseData::ReadBits(bits))
        }
        ModbusRequest::ReadDiscreteInputs { address, quantity } => {
            validate_quantity(*address, *quantity, 2000)?;
            let bits = register_map.read_discrete_inputs(*address, *quantity);
            Ok(ResponseData::ReadBits(bits))
        }
        ModbusRequest::ReadHoldingRegisters { address, quantity } => {
            validate_quantity(*address, *quantity, 125)?;
            let regs = register_map.read_holding_registers(*address, *quantity);
            Ok(ResponseData::ReadRegisters(regs))
        }
        ModbusRequest::ReadInputRegisters { address, quantity } => {
            validate_quantity(*address, *quantity, 125)?;
            let regs = register_map.read_input_registers(*address, *quantity);
            Ok(ResponseData::ReadRegisters(regs))
        }
        _ => Err(0x01), // Illegal Function
    }
}

pub(crate) fn execute_write(
    register_map: &mut RegisterMap,
    req: &ModbusRequest,
) -> Result<ResponseData, u8> {
    match req {
        ModbusRequest::WriteSingleCoil { address, value } => {
            register_map.write_coil(*address, *value);
            register_map.discrete_inputs.insert(*address, *value);
            Ok(ResponseData::WriteSingleCoil {
                address: *address,
                value: *value,
            })
        }
        ModbusRequest::WriteSingleRegister { address, value } => {
            register_map.write_holding_register(*address, *value);
            register_map.input_registers.insert(*address, *value);
            Ok(ResponseData::WriteSingleRegister {
                address: *address,
                value: *value,
            })
        }
        ModbusRequest::WriteMultipleCoils { address, values } => {
            let quantity = values.len() as u16;
            validate_quantity(*address, quantity, 1968)?;
            register_map.write_coils(*address, values);
            for (i, &val) in values.iter().enumerate() {
                register_map.discrete_inputs.insert(*address + i as u16, val);
            }
            Ok(ResponseData::WriteMultiple {
                address: *address,
                quantity,
            })
        }
        ModbusRequest::WriteMultipleRegisters { address, values } => {
            let quantity = values.len() as u16;
            validate_quantity(*address, quantity, 123)?;
            register_map.write_holding_registers(*address, values);
            for (i, &val) in values.iter().enumerate() {
                register_map.input_registers.insert(*address + i as u16, val);
            }
            Ok(ResponseData::WriteMultiple {
                address: *address,
                quantity,
            })
        }
        _ => Err(0x01), // Illegal Function
    }
}

/// Validate quantity > 0, quantity <= max, and address + quantity <= 65536.
/// Returns exception code 0x03 (Illegal Data Value) or 0x02 (Illegal Data Address).
pub(crate) fn validate_quantity(addr: u16, quantity: u16, max_quantity: u16) -> Result<(), u8> {
    if quantity == 0 || quantity > max_quantity {
        return Err(0x03); // Illegal Data Value
    }
    if (addr as u32) + (quantity as u32) > 65536 {
        return Err(0x02); // Illegal Data Address
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// pub(crate) helpers (reused by ascii_slave, rtu_tcp_slave)
// ---------------------------------------------------------------------------

/// Map a `ModbusRequest` variant to its `FunctionCode`.
pub(crate) fn request_to_fc(req: &ModbusRequest) -> FunctionCode {
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

/// Format a `ModbusRequest` into a short human-readable string for logging.
pub(crate) fn format_request(req: &ModbusRequest) -> String {
    match req {
        ModbusRequest::ReadCoils { address, quantity } => format!("R {} x{}", address, quantity),
        ModbusRequest::ReadDiscreteInputs { address, quantity } => {
            format!("R {} x{}", address, quantity)
        }
        ModbusRequest::ReadHoldingRegisters { address, quantity } => {
            format!("R {} x{}", address, quantity)
        }
        ModbusRequest::ReadInputRegisters { address, quantity } => {
            format!("R {} x{}", address, quantity)
        }
        ModbusRequest::WriteSingleCoil { address, value } => {
            format!("W {} = {}", address, value)
        }
        ModbusRequest::WriteSingleRegister { address, value } => {
            format!("W {} = {:#06x}", address, value)
        }
        ModbusRequest::WriteMultipleCoils { address, values } => {
            format!("W {} x{}", address, values.len())
        }
        ModbusRequest::WriteMultipleRegisters { address, values } => {
            format!("W {} x{}", address, values.len())
        }
    }
}

// ---------------------------------------------------------------------------
// Internal utilities
// ---------------------------------------------------------------------------

pub(crate) fn log_if_enabled(
    log_collector: &SharedLogCollector,
    direction: Direction,
    fc: FunctionCode,
    detail: &str,
) {
    if let Some(collector) = log_collector {
        let entry = LogEntry::new(direction, fc, detail);
        collector.try_add(entry);
    }
}

fn convert_parity(p: &Parity) -> tokio_serial::Parity {
    match p {
        Parity::None => tokio_serial::Parity::None,
        Parity::Odd => tokio_serial::Parity::Odd,
        Parity::Even => tokio_serial::Parity::Even,
    }
}

fn convert_data_bits(bits: u8) -> tokio_serial::DataBits {
    match bits {
        5 => tokio_serial::DataBits::Five,
        6 => tokio_serial::DataBits::Six,
        7 => tokio_serial::DataBits::Seven,
        _ => tokio_serial::DataBits::Eight,
    }
}

fn convert_stop_bits(bits: u8) -> tokio_serial::StopBits {
    match bits {
        2 => tokio_serial::StopBits::Two,
        _ => tokio_serial::StopBits::One,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slave::SlaveDevice;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    fn make_devices(slave_id: u8) -> SharedDevices {
        let mut map = HashMap::new();
        let device = SlaveDevice::with_default_registers(slave_id, "test", 100);
        map.insert(slave_id, device);
        Arc::new(RwLock::new(map))
    }

    #[tokio::test]
    async fn test_process_request_read_holding() {
        let devices = make_devices(1);
        // Write some values first.
        {
            let mut devs = devices.write().await;
            let dev = devs.get_mut(&1).unwrap();
            dev.register_map.write_holding_register(0, 0x1234);
            dev.register_map.write_holding_register(1, 0x5678);
        }
        // FC03 Read Holding Registers: addr=0, qty=2
        let pdu = [0x03, 0x00, 0x00, 0x00, 0x02];
        let resp = process_request(1, &pdu, &devices).await.unwrap();
        // Response: FC=0x03, byte_count=4, 0x12 0x34 0x56 0x78
        assert_eq!(resp[0], 0x03);
        assert_eq!(resp[1], 0x04); // 2 regs * 2 bytes
        assert_eq!(resp[2], 0x12);
        assert_eq!(resp[3], 0x34);
        assert_eq!(resp[4], 0x56);
        assert_eq!(resp[5], 0x78);
    }

    #[tokio::test]
    async fn test_process_request_write_single_register() {
        let devices = make_devices(1);
        // FC06 Write Single Register: addr=10, value=0x00FF
        let pdu = [0x06, 0x00, 0x0A, 0x00, 0xFF];
        let resp = process_request(1, &pdu, &devices).await.unwrap();
        assert_eq!(resp, vec![0x06, 0x00, 0x0A, 0x00, 0xFF]);

        // Verify the value was written.
        let devs = devices.read().await;
        let dev = devs.get(&1).unwrap();
        assert_eq!(dev.register_map.holding_registers.get(&10), Some(&0x00FF));
        // Mirror to input_registers.
        assert_eq!(dev.register_map.input_registers.get(&10), Some(&0x00FF));
    }

    #[tokio::test]
    async fn test_process_request_unknown_slave() {
        let devices = make_devices(1);
        let pdu = [0x03, 0x00, 0x00, 0x00, 0x01];
        // Slave 99 does not exist.
        let resp = process_request(99, &pdu, &devices).await;
        assert!(resp.is_none());
    }

    #[tokio::test]
    async fn test_process_request_invalid_pdu() {
        let devices = make_devices(1);
        // Unsupported FC 0x2B.
        let pdu = [0x2B, 0x00];
        let resp = process_request(1, &pdu, &devices).await.unwrap();
        // Should get exception response: FC | 0x80, exception code 0x01.
        assert_eq!(resp[0], 0x2B | 0x80);
        assert_eq!(resp[1], 0x01);
    }

    #[tokio::test]
    async fn test_process_request_write_single_coil() {
        let devices = make_devices(1);
        // FC05 Write Single Coil: addr=5, value=ON (0xFF00)
        let pdu = [0x05, 0x00, 0x05, 0xFF, 0x00];
        let resp = process_request(1, &pdu, &devices).await.unwrap();
        assert_eq!(resp, vec![0x05, 0x00, 0x05, 0xFF, 0x00]);

        let devs = devices.read().await;
        let dev = devs.get(&1).unwrap();
        assert_eq!(dev.register_map.coils.get(&5), Some(&true));
        assert_eq!(dev.register_map.discrete_inputs.get(&5), Some(&true));
    }

    #[tokio::test]
    async fn test_process_request_read_coils() {
        let devices = make_devices(1);
        {
            let mut devs = devices.write().await;
            let dev = devs.get_mut(&1).unwrap();
            dev.register_map.write_coil(0, true);
            dev.register_map.write_coil(1, false);
            dev.register_map.write_coil(2, true);
        }
        // FC01 Read Coils: addr=0, qty=3
        let pdu = [0x01, 0x00, 0x00, 0x00, 0x03];
        let resp = process_request(1, &pdu, &devices).await.unwrap();
        assert_eq!(resp[0], 0x01);
        assert_eq!(resp[1], 0x01); // 1 byte for 3 bits
        assert_eq!(resp[2], 0b00000101); // bits: T, F, T => 0x05
    }

    #[tokio::test]
    async fn test_process_request_write_multiple_registers() {
        let devices = make_devices(1);
        // FC10 Write Multiple Registers: addr=0, qty=2, byte_count=4, data=[0x000A, 0x000B]
        let pdu = [0x10, 0x00, 0x00, 0x00, 0x02, 0x04, 0x00, 0x0A, 0x00, 0x0B];
        let resp = process_request(1, &pdu, &devices).await.unwrap();
        assert_eq!(resp, vec![0x10, 0x00, 0x00, 0x00, 0x02]);

        let devs = devices.read().await;
        let dev = devs.get(&1).unwrap();
        assert_eq!(dev.register_map.holding_registers.get(&0), Some(&0x000A));
        assert_eq!(dev.register_map.holding_registers.get(&1), Some(&0x000B));
        assert_eq!(dev.register_map.input_registers.get(&0), Some(&0x000A));
        assert_eq!(dev.register_map.input_registers.get(&1), Some(&0x000B));
    }

    #[test]
    fn test_request_to_fc() {
        let req = ModbusRequest::ReadHoldingRegisters {
            address: 0,
            quantity: 1,
        };
        assert_eq!(request_to_fc(&req), FunctionCode::ReadHoldingRegisters);

        let req = ModbusRequest::WriteSingleCoil {
            address: 0,
            value: true,
        };
        assert_eq!(request_to_fc(&req), FunctionCode::WriteSingleCoil);
    }

    #[test]
    fn test_format_request() {
        let req = ModbusRequest::ReadHoldingRegisters {
            address: 100,
            quantity: 10,
        };
        assert_eq!(format_request(&req), "R 100 x10");

        let req = ModbusRequest::WriteSingleRegister {
            address: 5,
            value: 0x1234,
        };
        assert_eq!(format_request(&req), "W 5 = 0x1234");
    }

    #[test]
    fn test_validate_quantity_ok() {
        assert!(validate_quantity(0, 10, 125).is_ok());
        assert!(validate_quantity(0, 125, 125).is_ok());
    }

    #[test]
    fn test_validate_quantity_zero() {
        assert_eq!(validate_quantity(0, 0, 125), Err(0x03));
    }

    #[test]
    fn test_validate_quantity_overflow() {
        assert_eq!(validate_quantity(65535, 2, 125), Err(0x02));
    }

    #[test]
    fn test_validate_quantity_exceeds_max() {
        assert_eq!(validate_quantity(0, 200, 125), Err(0x03));
    }

    #[test]
    fn test_convert_parity() {
        assert_eq!(convert_parity(&Parity::None), tokio_serial::Parity::None);
        assert_eq!(convert_parity(&Parity::Odd), tokio_serial::Parity::Odd);
        assert_eq!(convert_parity(&Parity::Even), tokio_serial::Parity::Even);
    }
}
