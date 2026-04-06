//! RTU-over-TCP master transport.
//!
//! Uses RTU framing (slave_id + PDU + CRC) over a TCP connection.
//! A `Mutex` serializes access to the TCP stream.

use crate::frame;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

pub struct RtuTcpMasterTransport {
    stream: Arc<Mutex<tokio::net::TcpStream>>,
}

impl RtuTcpMasterTransport {
    /// Connect to a remote RTU-over-TCP server.
    pub async fn connect(host: &str, port: u16, timeout: Duration) -> Result<Self, String> {
        let addr = format!("{}:{}", host, port);
        let stream = tokio::time::timeout(timeout, tokio::net::TcpStream::connect(&addr))
            .await
            .map_err(|_| format!("connection timeout: {}", addr))?
            .map_err(|e| format!("connection failed: {}", e))?;
        Ok(Self {
            stream: Arc::new(Mutex::new(stream)),
        })
    }

    /// Send an RTU request over TCP and read the response. Serialized via Mutex.
    pub async fn request(
        &self,
        slave_id: u8,
        request_pdu: &[u8],
        timeout: Duration,
    ) -> Result<Vec<u8>, String> {
        let mut stream = self.stream.lock().await;

        // 1. Encode RTU frame and write.
        let req_frame = frame::encode_rtu(slave_id, request_pdu);
        stream
            .write_all(&req_frame)
            .await
            .map_err(|e| format!("write error: {}", e))?;

        // 2. Read response bytes with timeout, accumulate until CRC validates.
        let mut buf = vec![0u8; 512];
        let mut response = Vec::new();
        loop {
            match tokio::time::timeout(timeout, stream.read(&mut buf)).await {
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

        // 3. Decode and verify slave_id.
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
