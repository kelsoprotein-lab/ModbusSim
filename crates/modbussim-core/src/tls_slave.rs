//! TLS-enabled Modbus TCP slave server.
//!
//! Binds a TCP listener (tokio), accepts connections, performs a TLS handshake
//! via `native_tls`, and then handles MBAP-framed Modbus requests on each
//! connection in a blocking thread.

use crate::log_entry::{Direction, FunctionCode, LogEntry};
use crate::mbap;
use crate::pdu::{
    build_exception_pdu, build_response_pdu, parse_request_pdu, ModbusRequest, ResponseData,
};
use crate::register::RegisterMap;
use crate::slave::{SharedDevices, SharedLogCollector};
use crate::transport::SlaveTlsConfig;
use native_tls::{Identity, Protocol, TlsAcceptor};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

// ---------------------------------------------------------------------------
// TLS acceptor construction
// ---------------------------------------------------------------------------

/// Build a `native_tls::TlsAcceptor` from a `SlaveTlsConfig`.
///
/// Loads identity from PKCS#12 (priority) or PEM cert+key, sets min TLS
/// version to 1.2, and returns the acceptor.
pub fn build_tls_acceptor(config: &SlaveTlsConfig) -> Result<TlsAcceptor, String> {
    let identity = if !config.pkcs12_file.is_empty() {
        // Load PKCS#12 identity.
        let pkcs12_bytes = std::fs::read(&config.pkcs12_file)
            .map_err(|e| format!("Failed to read PKCS#12 file '{}': {e}", config.pkcs12_file))?;
        Identity::from_pkcs12(&pkcs12_bytes, &config.pkcs12_password)
            .map_err(|e| format!("Failed to parse PKCS#12: {e}"))?
    } else if !config.cert_file.is_empty() && !config.key_file.is_empty() {
        // Load PEM cert + key.
        let cert_bytes = std::fs::read(&config.cert_file)
            .map_err(|e| format!("Failed to read cert file '{}': {e}", config.cert_file))?;
        let key_bytes = std::fs::read(&config.key_file)
            .map_err(|e| format!("Failed to read key file '{}': {e}", config.key_file))?;
        Identity::from_pkcs8(&cert_bytes, &key_bytes)
            .map_err(|e| format!("Failed to create identity from PEM: {e}"))?
    } else {
        return Err("No certificate configured: set pkcs12_file or cert_file+key_file".into());
    };

    let acceptor = TlsAcceptor::builder(identity)
        .min_protocol_version(Some(Protocol::Tlsv12))
        .build()
        .map_err(|e| format!("Failed to build TLS acceptor: {e}"))?;

    Ok(acceptor)
}

// ---------------------------------------------------------------------------
// Server entry point
// ---------------------------------------------------------------------------

/// Run a TLS Modbus TCP slave on `addr`.
///
/// Accepts TCP connections, performs TLS handshake via `spawn_blocking`, then
/// hands off to `handle_client` which runs in a blocking thread.
pub async fn run_tls_slave(
    addr: SocketAddr,
    tls_config: SlaveTlsConfig,
    devices: SharedDevices,
    log_collector: SharedLogCollector,
    shutdown_rx: oneshot::Receiver<()>,
) -> Result<(), String> {
    let acceptor = build_tls_acceptor(&tls_config)?;
    let acceptor = Arc::new(acceptor);

    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| format!("Failed to bind {addr}: {e}"))?;
    log::info!("TLS Modbus slave listening on {addr}");

    // Shared shutdown flag that blocking client threads can check.
    let shutdown_flag = Arc::new(std::sync::Mutex::new(false));

    tokio::pin!(shutdown_rx);

    loop {
        tokio::select! {
            _ = &mut shutdown_rx => {
                log::info!("TLS slave shutting down");
                *shutdown_flag.lock().unwrap() = true;
                return Ok(());
            }
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, peer)) => {
                        log::info!("TLS client connected: {peer}");
                        if let Some(ref collector) = log_collector {
                            collector.try_add(LogEntry::new(
                                Direction::Rx,
                                FunctionCode::ReadHoldingRegisters,
                                format!("TLS client connected: {peer}"),
                            ));
                        }

                        // Convert tokio TcpStream to std TcpStream for blocking TLS handshake.
                        let std_stream = stream.into_std()
                            .map_err(|e| format!("Failed to convert stream: {e}"))?;

                        let acceptor = acceptor.clone();
                        let devices = devices.clone();
                        let log_collector = log_collector.clone();
                        let shutdown_flag = shutdown_flag.clone();

                        tokio::task::spawn_blocking(move || {
                            // Perform TLS handshake.
                            let tls_stream = match acceptor.accept(std_stream) {
                                Ok(s) => s,
                                Err(e) => {
                                    log::warn!("TLS handshake failed for {peer}: {e}");
                                    return;
                                }
                            };

                            if let Err(e) = handle_client(
                                tls_stream,
                                peer,
                                devices,
                                log_collector,
                                shutdown_flag,
                            ) {
                                log::warn!("TLS client {peer} error: {e}");
                            }
                            log::info!("TLS client {peer} disconnected");
                        });
                    }
                    Err(e) => {
                        log::error!("TLS accept error: {e}");
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Client handler (blocking)
// ---------------------------------------------------------------------------

/// Handle a single TLS client connection.
///
/// Sets a 200ms read timeout so the thread can periodically check the shutdown
/// flag. Reads MBAP frames, processes requests, and writes responses.
fn handle_client(
    mut tls_stream: native_tls::TlsStream<std::net::TcpStream>,
    peer_addr: SocketAddr,
    devices: SharedDevices,
    log_collector: SharedLogCollector,
    shutdown: Arc<std::sync::Mutex<bool>>,
) -> Result<(), String> {
    // Set read timeout so we can check the shutdown flag periodically.
    tls_stream
        .get_ref()
        .set_read_timeout(Some(Duration::from_millis(200)))
        .map_err(|e| format!("Failed to set read timeout: {e}"))?;

    loop {
        // Check shutdown flag.
        if *shutdown.lock().unwrap() {
            return Ok(());
        }

        // Try to read an MBAP frame.
        let (header, pdu) = match mbap::read_frame(&mut tls_stream) {
            Ok(frame) => frame,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut
                {
                    // Read timeout — loop back to check shutdown flag.
                    continue;
                }
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    // Client disconnected.
                    return Ok(());
                }
                return Err(format!("Read error from {peer_addr}: {e}"));
            }
        };

        let unit_id = header.unit_id;
        let transaction_id = header.transaction_id;
        let fc_byte = pdu.first().copied().unwrap_or(0);

        // Log inbound request.
        if let Some(fc) = FunctionCode::from_u8(fc_byte) {
            if let Ok(req) = parse_request_pdu(&pdu) {
                log_if_enabled(
                    &log_collector,
                    Direction::Rx,
                    fc,
                    &format_request(&req),
                );
            }
        }

        // Process the request.
        let response_pdu = process_request(unit_id, fc_byte, &pdu, &devices);

        if let Some(ref resp) = response_pdu {
            // Log outbound response.
            if let Some(fc) = FunctionCode::from_u8(fc_byte) {
                let detail = if resp.first().map_or(false, |b| b & 0x80 != 0) {
                    format!(
                        "ERR: exception 0x{:02X}",
                        resp.get(1).copied().unwrap_or(0)
                    )
                } else {
                    "OK".to_string()
                };
                log_if_enabled(&log_collector, Direction::Tx, fc, &detail);
            }

            // Write the response MBAP frame.
            mbap::write_frame(&mut tls_stream, transaction_id, unit_id, resp)
                .map_err(|e| format!("Write error to {peer_addr}: {e}"))?;
        }
        // If process_request returns None, slave_id not found — silently drop.
    }
}

// ---------------------------------------------------------------------------
// Request processing (blocking, uses try_read / try_write)
// ---------------------------------------------------------------------------

/// Process a Modbus request PDU for the given unit/slave.
///
/// Uses `try_read` / `try_write` on the shared device map because this runs
/// in a blocking thread (cannot `.await`).
fn process_request(
    unit_id: u8,
    fc_byte: u8,
    pdu: &[u8],
    devices: &SharedDevices,
) -> Option<Vec<u8>> {
    let req = match parse_request_pdu(pdu) {
        Ok(r) => r,
        Err(_) => {
            // Unsupported or malformed function code — Illegal Function.
            return Some(build_exception_pdu(fc_byte, 0x01));
        }
    };

    let is_write = matches!(
        req,
        ModbusRequest::WriteSingleCoil { .. }
            | ModbusRequest::WriteSingleRegister { .. }
            | ModbusRequest::WriteMultipleCoils { .. }
            | ModbusRequest::WriteMultipleRegisters { .. }
    );

    if is_write {
        match devices.try_write() {
            Ok(mut devices) => {
                let device = devices.get_mut(&unit_id)?;
                match execute_write(&mut device.register_map, &req) {
                    Ok(data) => Some(build_response_pdu(fc_byte, &data)),
                    Err(exception) => Some(build_exception_pdu(fc_byte, exception)),
                }
            }
            Err(_) => Some(build_exception_pdu(fc_byte, 0x06)), // Server Device Busy
        }
    } else {
        match devices.try_read() {
            Ok(devices) => {
                let device = devices.get(&unit_id)?;
                match execute_read(&device.register_map, &req) {
                    Ok(data) => Some(build_response_pdu(fc_byte, &data)),
                    Err(exception) => Some(build_exception_pdu(fc_byte, exception)),
                }
            }
            Err(_) => Some(build_exception_pdu(fc_byte, 0x06)), // Server Device Busy
        }
    }
}

// ---------------------------------------------------------------------------
// Read / Write helpers
// ---------------------------------------------------------------------------

fn execute_read(register_map: &RegisterMap, req: &ModbusRequest) -> Result<ResponseData, u8> {
    match req {
        ModbusRequest::ReadCoils { address, quantity } => {
            validate_quantity(*address, *quantity, 2000)?;
            if !register_map.has_all_coils(*address, *quantity) {
                return Err(0x02); // Illegal Data Address
            }
            Ok(ResponseData::ReadBits(
                register_map.read_coils(*address, *quantity),
            ))
        }
        ModbusRequest::ReadDiscreteInputs { address, quantity } => {
            validate_quantity(*address, *quantity, 2000)?;
            if !register_map.has_all_discrete_inputs(*address, *quantity) {
                return Err(0x02);
            }
            Ok(ResponseData::ReadBits(
                register_map.read_discrete_inputs(*address, *quantity),
            ))
        }
        ModbusRequest::ReadHoldingRegisters { address, quantity } => {
            validate_quantity(*address, *quantity, 125)?;
            if !register_map.has_all_holding_registers(*address, *quantity) {
                return Err(0x02);
            }
            Ok(ResponseData::ReadRegisters(
                register_map.read_holding_registers(*address, *quantity),
            ))
        }
        ModbusRequest::ReadInputRegisters { address, quantity } => {
            validate_quantity(*address, *quantity, 125)?;
            if !register_map.has_all_input_registers(*address, *quantity) {
                return Err(0x02);
            }
            Ok(ResponseData::ReadRegisters(
                register_map.read_input_registers(*address, *quantity),
            ))
        }
        _ => Err(0x01), // Illegal Function
    }
}

fn execute_write(
    register_map: &mut RegisterMap,
    req: &ModbusRequest,
) -> Result<ResponseData, u8> {
    match req {
        ModbusRequest::WriteSingleCoil { address, value } => {
            if !register_map.has_coil(*address) {
                return Err(0x02);
            }
            register_map.write_coil(*address, *value);
            // Mirror to discrete_inputs.
            register_map.discrete_inputs.insert(*address, *value);
            Ok(ResponseData::WriteSingleCoil {
                address: *address,
                value: *value,
            })
        }
        ModbusRequest::WriteSingleRegister { address, value } => {
            if !register_map.has_holding_register(*address) {
                return Err(0x02);
            }
            register_map.write_holding_register(*address, *value);
            // Mirror to input_registers.
            register_map.input_registers.insert(*address, *value);
            Ok(ResponseData::WriteSingleRegister {
                address: *address,
                value: *value,
            })
        }
        ModbusRequest::WriteMultipleCoils { address, values } => {
            let quantity = values.len() as u16;
            validate_quantity(*address, quantity, 1968)?;
            if !register_map.has_all_coils(*address, quantity) {
                return Err(0x02);
            }
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
            if !register_map.has_all_holding_registers(*address, quantity) {
                return Err(0x02);
            }
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

// ---------------------------------------------------------------------------
// Validation / logging helpers
// ---------------------------------------------------------------------------

fn validate_quantity(addr: u16, quantity: u16, max_quantity: u16) -> Result<(), u8> {
    if quantity == 0 || quantity > max_quantity {
        return Err(0x03); // Illegal Data Value
    }
    if (addr as u32) + (quantity as u32) > 65536 {
        return Err(0x02); // Illegal Data Address
    }
    Ok(())
}

fn log_if_enabled(
    log_collector: &SharedLogCollector,
    direction: Direction,
    fc: FunctionCode,
    detail: &str,
) {
    if let Some(collector) = log_collector {
        collector.try_add(LogEntry::new(direction, fc, detail));
    }
}

fn format_request(req: &ModbusRequest) -> String {
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slave::SlaveDevice;
    use std::collections::HashMap;
    use tokio::sync::RwLock;

    fn make_devices(slave_id: u8) -> SharedDevices {
        let mut map = HashMap::new();
        let device = SlaveDevice::with_default_registers(slave_id, "test", 100);
        map.insert(slave_id, device);
        Arc::new(RwLock::new(map))
    }

    #[test]
    fn test_process_request_read_holding() {
        let devices = make_devices(1);
        // Write some values.
        {
            let mut devs = devices.try_write().unwrap();
            let dev = devs.get_mut(&1).unwrap();
            dev.register_map.write_holding_register(0, 0x1234);
            dev.register_map.write_holding_register(1, 0x5678);
        }
        // FC03 Read Holding Registers: addr=0, qty=2
        let pdu = [0x03, 0x00, 0x00, 0x00, 0x02];
        let resp = process_request(1, 0x03, &pdu, &devices).unwrap();
        assert_eq!(resp[0], 0x03);
        assert_eq!(resp[1], 0x04);
        assert_eq!(resp[2], 0x12);
        assert_eq!(resp[3], 0x34);
        assert_eq!(resp[4], 0x56);
        assert_eq!(resp[5], 0x78);
    }

    #[test]
    fn test_process_request_write_single_register() {
        let devices = make_devices(1);
        let pdu = [0x06, 0x00, 0x0A, 0x00, 0xFF];
        let resp = process_request(1, 0x06, &pdu, &devices).unwrap();
        assert_eq!(resp, vec![0x06, 0x00, 0x0A, 0x00, 0xFF]);

        let devs = devices.try_read().unwrap();
        let dev = devs.get(&1).unwrap();
        assert_eq!(dev.register_map.holding_registers.get(&10), Some(&0x00FF));
        assert_eq!(dev.register_map.input_registers.get(&10), Some(&0x00FF));
    }

    #[test]
    fn test_process_request_write_single_coil() {
        let devices = make_devices(1);
        let pdu = [0x05, 0x00, 0x05, 0xFF, 0x00];
        let resp = process_request(1, 0x05, &pdu, &devices).unwrap();
        assert_eq!(resp, vec![0x05, 0x00, 0x05, 0xFF, 0x00]);

        let devs = devices.try_read().unwrap();
        let dev = devs.get(&1).unwrap();
        assert_eq!(dev.register_map.coils.get(&5), Some(&true));
        assert_eq!(dev.register_map.discrete_inputs.get(&5), Some(&true));
    }

    #[test]
    fn test_process_request_unknown_slave() {
        let devices = make_devices(1);
        let pdu = [0x03, 0x00, 0x00, 0x00, 0x01];
        let resp = process_request(99, 0x03, &pdu, &devices);
        assert!(resp.is_none());
    }

    #[test]
    fn test_process_request_unsupported_fc() {
        let devices = make_devices(1);
        let pdu = [0x2B, 0x00];
        let resp = process_request(1, 0x2B, &pdu, &devices).unwrap();
        assert_eq!(resp[0], 0x2B | 0x80);
        assert_eq!(resp[1], 0x01);
    }

    #[test]
    fn test_process_request_write_multiple_registers() {
        let devices = make_devices(1);
        let pdu = [0x10, 0x00, 0x00, 0x00, 0x02, 0x04, 0x00, 0x0A, 0x00, 0x0B];
        let resp = process_request(1, 0x10, &pdu, &devices).unwrap();
        assert_eq!(resp, vec![0x10, 0x00, 0x00, 0x00, 0x02]);

        let devs = devices.try_read().unwrap();
        let dev = devs.get(&1).unwrap();
        assert_eq!(dev.register_map.holding_registers.get(&0), Some(&0x000A));
        assert_eq!(dev.register_map.holding_registers.get(&1), Some(&0x000B));
        assert_eq!(dev.register_map.input_registers.get(&0), Some(&0x000A));
        assert_eq!(dev.register_map.input_registers.get(&1), Some(&0x000B));
    }

    #[test]
    fn test_process_request_write_multiple_coils() {
        let devices = make_devices(1);
        // FC0F Write Multiple Coils: addr=0, qty=3, byte_count=1, data=0b00000101 (T,F,T)
        let pdu = [0x0F, 0x00, 0x00, 0x00, 0x03, 0x01, 0x05];
        let resp = process_request(1, 0x0F, &pdu, &devices).unwrap();
        assert_eq!(resp, vec![0x0F, 0x00, 0x00, 0x00, 0x03]);

        let devs = devices.try_read().unwrap();
        let dev = devs.get(&1).unwrap();
        assert_eq!(dev.register_map.coils.get(&0), Some(&true));
        assert_eq!(dev.register_map.coils.get(&1), Some(&false));
        assert_eq!(dev.register_map.coils.get(&2), Some(&true));
        // Mirror check
        assert_eq!(dev.register_map.discrete_inputs.get(&0), Some(&true));
        assert_eq!(dev.register_map.discrete_inputs.get(&1), Some(&false));
        assert_eq!(dev.register_map.discrete_inputs.get(&2), Some(&true));
    }

    #[test]
    fn test_process_request_read_coils() {
        let devices = make_devices(1);
        {
            let mut devs = devices.try_write().unwrap();
            let dev = devs.get_mut(&1).unwrap();
            dev.register_map.write_coil(0, true);
            dev.register_map.write_coil(1, false);
            dev.register_map.write_coil(2, true);
        }
        let pdu = [0x01, 0x00, 0x00, 0x00, 0x03];
        let resp = process_request(1, 0x01, &pdu, &devices).unwrap();
        assert_eq!(resp[0], 0x01);
        assert_eq!(resp[1], 0x01);
        assert_eq!(resp[2], 0b00000101);
    }

    #[test]
    fn test_validate_quantity_ok() {
        assert!(validate_quantity(0, 10, 125).is_ok());
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
    fn test_build_tls_acceptor_no_cert() {
        let result = build_tls_acceptor(&SlaveTlsConfig::default());
        let err = result.err().expect("should fail");
        assert!(err.contains("No certificate configured"), "got: {err}");
    }

    #[test]
    fn test_build_tls_acceptor_missing_pkcs12() {
        let config = SlaveTlsConfig {
            pkcs12_file: "/nonexistent/cert.p12".to_string(),
            pkcs12_password: "test".to_string(),
            ..Default::default()
        };
        let err = build_tls_acceptor(&config).err().expect("should fail");
        assert!(err.contains("Failed to read PKCS#12"), "got: {err}");
    }

    #[test]
    fn test_build_tls_acceptor_missing_pem() {
        let config = SlaveTlsConfig {
            cert_file: "/nonexistent/cert.pem".to_string(),
            key_file: "/nonexistent/key.pem".to_string(),
            ..Default::default()
        };
        let err = build_tls_acceptor(&config).err().expect("should fail");
        assert!(err.contains("Failed to read cert file"), "got: {err}");
    }
}
