use modbussim_core::log_collector::LogCollector;
use modbussim_core::slave::{SlaveConnection, SlaveDevice};
use modbussim_core::transport::Transport;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let transport = Transport::Tcp {
        host: "0.0.0.0".to_string(),
        port: 5020,
    };
    let log = Arc::new(LogCollector::new());
    let mut slave = SlaveConnection::new(transport).with_log_collector(log);
    slave
        .add_device(SlaveDevice::with_random_registers(1, "Test Slave", 100))
        .await
        .unwrap();
    slave.start().await.unwrap();
    println!("Slave running on 0.0.0.0:5020 with LogCollector");
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    }
}
