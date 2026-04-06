//! RTU master transport over serial port.
//!
//! Provides a `request()` method that sends an RTU request PDU and returns the
//! response PDU. A `Mutex` ensures half-duplex serialization for the serial port.

use crate::frame;
use crate::transport::{self, Parity, SerialConfig};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tokio_serial::SerialPortBuilderExt;

pub struct RtuMasterTransport {
    port: Arc<Mutex<tokio_serial::SerialStream>>,
    interframe_delay: Duration,
}

impl RtuMasterTransport {
    /// Open a serial port and create an RTU master transport.
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

        let interframe_us = transport::rtu_interframe_delay_us(config.baud_rate);
        let interframe_delay = Duration::from_micros(interframe_us);

        Ok(Self {
            port: Arc::new(Mutex::new(port)),
            interframe_delay,
        })
    }

    /// Send an RTU request and read the response. Serialized via Mutex.
    pub async fn request(
        &self,
        slave_id: u8,
        request_pdu: &[u8],
        timeout: Duration,
    ) -> Result<Vec<u8>, String> {
        let mut port = self.port.lock().await;

        // 1. Encode RTU frame and write.
        let req_frame = frame::encode_rtu(slave_id, request_pdu);
        port.write_all(&req_frame)
            .await
            .map_err(|e| format!("write error: {}", e))?;

        // 2. Wait interframe delay.
        tokio::time::sleep(self.interframe_delay).await;

        // 3. Read response bytes with timeout, accumulate until CRC validates.
        let mut buf = vec![0u8; 512];
        let mut response = Vec::new();
        loop {
            match tokio::time::timeout(timeout, port.read(&mut buf)).await {
                Ok(Ok(n)) if n > 0 => {
                    response.extend_from_slice(&buf[..n]);
                    if response.len() >= 4 && frame::decode_rtu(&response).is_ok() {
                        break;
                    }
                }
                Ok(Ok(_)) => break,
                Ok(Err(e)) => return Err(format!("read error: {}", e)),
                Err(_) => {
                    if response.is_empty() {
                        return Err("response timeout".to_string());
                    }
                    break;
                }
            }
        }

        // 4. Decode and verify slave_id.
        let rtu_frame = frame::decode_rtu(&response)?;
        if rtu_frame.slave_id != slave_id {
            return Err(format!(
                "slave_id mismatch: expected {}, got {}",
                slave_id, rtu_frame.slave_id
            ));
        }
        Ok(rtu_frame.pdu)
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
