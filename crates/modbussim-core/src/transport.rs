use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Parity {
    None,
    Odd,
    Even,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SerialConfig {
    pub port: String,
    pub baud_rate: u32,
    pub data_bits: u8,
    pub stop_bits: u8,
    pub parity: Parity,
}

impl Default for SerialConfig {
    fn default() -> Self {
        Self {
            port: String::new(),
            baud_rate: 9600,
            data_bits: 8,
            stop_bits: 1,
            parity: Parity::None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Transport {
    Tcp { host: String, port: u16 },
    Rtu(SerialConfig),
    Ascii(SerialConfig),
    RtuOverTcp { host: String, port: u16 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SerialPortInfo {
    pub name: String,
    pub description: String,
    pub manufacturer: String,
}

pub fn list_serial_ports() -> Vec<SerialPortInfo> {
    match serialport::available_ports() {
        Ok(ports) => ports
            .into_iter()
            .map(|p| {
                let (description, manufacturer) = match &p.port_type {
                    serialport::SerialPortType::UsbPort(info) => (
                        info.product.clone().unwrap_or_default(),
                        info.manufacturer.clone().unwrap_or_default(),
                    ),
                    serialport::SerialPortType::BluetoothPort => {
                        ("Bluetooth".to_string(), String::new())
                    }
                    serialport::SerialPortType::PciPort => ("PCI".to_string(), String::new()),
                    serialport::SerialPortType::Unknown => (String::new(), String::new()),
                };
                SerialPortInfo {
                    name: p.port_name,
                    description,
                    manufacturer,
                }
            })
            .collect(),
        Err(_) => vec![],
    }
}

/// Returns the RTU interframe delay in microseconds.
/// For baud rates >= 19200, a fixed 1750 µs is used per the Modbus spec.
/// For lower baud rates, 3.5 character times are used (11 bits per character).
pub fn rtu_interframe_delay_us(baud_rate: u32) -> u64 {
    if baud_rate >= 19200 {
        1750
    } else {
        // 3.5 chars * 11 bits/char = 38.5 bits; time in µs = 38.5 / baud * 1_000_000
        (38_500_000u64 + baud_rate as u64 - 1) / baud_rate as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serial_config_default() {
        let cfg = SerialConfig::default();
        assert_eq!(cfg.baud_rate, 9600);
        assert_eq!(cfg.data_bits, 8);
        assert_eq!(cfg.stop_bits, 1);
        assert_eq!(cfg.parity, Parity::None);
    }

    #[test]
    fn test_transport_tcp_serde() {
        let t = Transport::Tcp {
            host: "127.0.0.1".to_string(),
            port: 502,
        };
        let json = serde_json::to_string(&t).unwrap();
        let t2: Transport = serde_json::from_str(&json).unwrap();
        assert_eq!(t, t2);
    }

    #[test]
    fn test_transport_rtu_serde() {
        let t = Transport::Rtu(SerialConfig::default());
        let json = serde_json::to_string(&t).unwrap();
        let t2: Transport = serde_json::from_str(&json).unwrap();
        assert_eq!(t, t2);
    }

    #[test]
    fn test_transport_rtu_over_tcp_serde() {
        let t = Transport::RtuOverTcp {
            host: "192.168.1.1".to_string(),
            port: 503,
        };
        let json = serde_json::to_string(&t).unwrap();
        let t2: Transport = serde_json::from_str(&json).unwrap();
        assert_eq!(t, t2);
    }

    #[test]
    fn test_list_serial_ports_does_not_panic() {
        let _ = list_serial_ports();
    }

    #[test]
    fn test_rtu_interframe_delay_high_baud() {
        assert_eq!(rtu_interframe_delay_us(19200), 1750);
        assert_eq!(rtu_interframe_delay_us(115200), 1750);
    }

    #[test]
    fn test_rtu_interframe_delay_low_baud() {
        let delay = rtu_interframe_delay_us(9600);
        assert!(delay >= 3500 && delay <= 4500, "delay={delay}");
    }
}
