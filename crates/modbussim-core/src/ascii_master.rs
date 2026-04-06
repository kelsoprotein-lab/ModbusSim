//! ASCII master transport over serial port.
//!
//! Provides a `request()` method that sends an ASCII-framed request PDU and
//! returns the response PDU. A `Mutex` ensures half-duplex serialization.

use crate::frame;
use crate::transport::{Parity, SerialConfig};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tokio_serial::SerialPortBuilderExt;

pub struct AsciiMasterTransport {
    port: Arc<Mutex<tokio_serial::SerialStream>>,
}

impl AsciiMasterTransport {
    /// Open a serial port and create an ASCII master transport.
    pub async fn connect(config: &SerialConfig) -> Result<Self, String> {
        let parity = convert_parity(&config.parity);
        let data_bits = convert_data_bits(config.data_bits);
        let stop_bits = convert_stop_bits(config.stop_bits);

        let port = tokio_serial::new(&config.port, config.baud_rate)
            .parity(parity)
            .data_bits(data_bits)
            .stop_bits(stop_bits)
            .open_native_async()
            .map_err(|e| format!("Failed to open serial port {}: {}", config.port, e))?;

        Ok(Self {
            port: Arc::new(Mutex::new(port)),
        })
    }

    /// Send an ASCII request and read the response. Serialized via Mutex.
    pub async fn request(
        &self,
        slave_id: u8,
        request_pdu: &[u8],
        timeout: Duration,
    ) -> Result<Vec<u8>, String> {
        let mut port = self.port.lock().await;

        // 1. Encode ASCII frame and write.
        let req_frame = frame::encode_ascii(slave_id, request_pdu);
        port.write_all(&req_frame)
            .await
            .map_err(|e| format!("write error: {}", e))?;

        // 2. Read byte-by-byte until '\r\n' is found, with deadline timeout.
        let deadline = tokio::time::Instant::now() + timeout;
        let mut response = Vec::with_capacity(512);
        let mut single = [0u8; 1];

        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                if response.is_empty() {
                    return Err("response timeout".to_string());
                }
                break;
            }

            match tokio::time::timeout(remaining, port.read(&mut single)).await {
                Ok(Ok(1)) => {
                    response.push(single[0]);
                    // Check if last 2 bytes are '\r\n'.
                    if response.len() >= 2
                        && response[response.len() - 2] == b'\r'
                        && response[response.len() - 1] == b'\n'
                    {
                        break;
                    }
                }
                Ok(Ok(_)) => continue,
                Ok(Err(e)) => return Err(format!("read error: {}", e)),
                Err(_) => {
                    if response.is_empty() {
                        return Err("response timeout".to_string());
                    }
                    break;
                }
            }
        }

        // 3. Decode ASCII frame and verify slave_id.
        let ascii_frame = frame::decode_ascii(&response)?;
        if ascii_frame.slave_id != slave_id {
            return Err(format!(
                "slave_id mismatch: expected {}, got {}",
                slave_id, ascii_frame.slave_id
            ));
        }
        Ok(ascii_frame.pdu)
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
