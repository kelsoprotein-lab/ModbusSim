//! ASCII slave server over serial port.
//!
//! Opens a serial port, reads ASCII frames delimited by ':' prefix and "\r\n"
//! suffix, processes Modbus requests against the shared device registry, and
//! sends ASCII responses.

use crate::frame;
use crate::log_entry::{Direction, FunctionCode};
use crate::pdu::parse_request_pdu;
use crate::rtu_slave::{format_request, log_if_enabled, process_request};
use crate::slave::{SharedChangeCallback, SharedDevices, SharedLogCollector};
use crate::transport::{Parity, SerialConfig};

use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::oneshot;
use tokio_serial::SerialPortBuilderExt;

/// Run an ASCII slave server on the given serial port.
///
/// Loops reading ASCII frames (`:` … `\r\n`), processing requests, and writing
/// responses until the `shutdown_rx` signal fires or an unrecoverable I/O error
/// occurs.
pub async fn run_ascii_slave(
    config: SerialConfig,
    devices: SharedDevices,
    log_collector: SharedLogCollector,
    change_callback: SharedChangeCallback,
    shutdown_rx: oneshot::Receiver<()>,
) -> Result<(), String> {
    let parity = convert_parity(&config.parity);
    let data_bits = convert_data_bits(config.data_bits);
    let stop_bits = convert_stop_bits(config.stop_bits);

    let port = tokio_serial::new(&config.port, config.baud_rate)
        .parity(parity)
        .data_bits(data_bits)
        .stop_bits(stop_bits)
        .open_native_async()
        .map_err(|e| format!("Failed to open serial port {}: {}", config.port, e))?;

    let (reader, mut writer) = tokio::io::split(port);
    let mut reader = BufReader::new(reader);

    tokio::pin!(shutdown_rx);

    let mut line_buf = String::with_capacity(512);

    loop {
        line_buf.clear();

        // Read a line (ASCII frames end with \r\n).
        let read_result = tokio::select! {
            _ = &mut shutdown_rx => {
                log::info!("ASCII slave shutting down");
                return Ok(());
            }
            result = tokio::time::timeout(Duration::from_secs(1), reader.read_line(&mut line_buf)) => {
                result
            }
        };

        // Handle timeout — just loop again so we can check shutdown.
        let read_result = match read_result {
            Ok(r) => r,
            Err(_timeout) => continue,
        };

        match read_result {
            Ok(0) => continue, // No data yet.
            Ok(_) => {}
            Err(e) => {
                return Err(format!("Serial port read error: {e}"));
            }
        }

        // Decode the ASCII frame.
        let ascii_frame = match frame::decode_ascii(line_buf.as_bytes()) {
            Ok(f) => f,
            Err(e) => {
                log::warn!("ASCII decode error: {e}");
                continue;
            }
        };

        let slave_id = ascii_frame.slave_id;
        let request_pdu = &ascii_frame.pdu;

        // Log inbound frame.
        if let Some(fc_val) = request_pdu.first() {
            if let Some(fc) = FunctionCode::from_u8(*fc_val) {
                if let Ok(req) = parse_request_pdu(request_pdu) {
                    log_if_enabled(&log_collector, Direction::Rx, fc, &format_request(&req));
                }
            }
        }

        // Process the request against the device registry.
        if let Some(response_pdu) =
            process_request(slave_id, request_pdu, &devices, &change_callback).await
        {
            // Log outbound response.
            if let Some(fc_val) = request_pdu.first() {
                if let Some(fc) = FunctionCode::from_u8(*fc_val) {
                    let detail = if response_pdu.first().map_or(false, |b| b & 0x80 != 0) {
                        format!(
                            "ERR: exception 0x{:02X}",
                            response_pdu.get(1).copied().unwrap_or(0)
                        )
                    } else {
                        "OK".to_string()
                    };
                    log_if_enabled(&log_collector, Direction::Tx, fc, &detail);
                }
            }

            let response_frame = frame::encode_ascii(slave_id, &response_pdu);
            if let Err(e) = writer.write_all(&response_frame).await {
                log::error!("Serial port write error: {e}");
                return Err(format!("Serial port write error: {e}"));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Internal utilities (same conversions as rtu_slave)
// ---------------------------------------------------------------------------

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
