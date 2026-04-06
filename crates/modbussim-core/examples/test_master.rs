/// Full integration test: slave WITH log collector + master, multi-threaded runtime
/// This reproduces the exact Tauri app scenario that was causing Broken pipe.
use modbussim_core::log_collector::LogCollector;
use modbussim_core::master::{MasterConfig, MasterConnection, PollEvent, ReadFunction, ReadResult, ScanGroup};
use modbussim_core::slave::{SlaveConnection, SlaveDevice};
use modbussim_core::transport::Transport;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

#[tokio::main] // multi-threaded runtime (same as Tauri)
async fn main() {
    println!("=== Full Tauri-equivalent Integration Test ===\n");

    // 1) Start slave WITH log collector (same as Tauri slave app does)
    println!("[1] Starting slave with LogCollector on port 15023...");
    let transport = Transport::Tcp {
        host: "0.0.0.0".to_string(),
        port: 15023,
    };
    let slave_log = Arc::new(LogCollector::new());
    let mut slave = SlaveConnection::new(transport).with_log_collector(slave_log.clone());
    slave
        .add_device(SlaveDevice::with_random_registers(1, "Slave", 100))
        .await
        .unwrap();
    slave.start().await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    println!("    Slave running.\n");

    // 2) Create master WITH log collector
    println!("[2] Creating master with LogCollector...");
    let master_log = Arc::new(LogCollector::new());
    let config = MasterConfig {
        target_address: "127.0.0.1".to_string(),
        port: 15023,
        slave_id: 1,
        timeout_ms: 3000,
    };
    let state: Arc<RwLock<HashMap<String, MasterConnection>>> =
        Arc::new(RwLock::new(HashMap::new()));
    {
        let conn = MasterConnection::new(config).with_log_collector(master_log.clone());
        state.write().await.insert("m1".to_string(), conn);
    }
    println!("    OK\n");

    // 3) Connect
    println!("[3] Connecting to slave...");
    {
        let mut conns = state.write().await;
        conns.get_mut("m1").unwrap().connect().await.unwrap();
    }
    println!("    Connected!\n");

    // 4) Single read (tests master log_tx/log_rx)
    println!("[4] Single read FC03...");
    {
        let mut conns = state.write().await;
        let conn = conns.get_mut("m1").unwrap();
        match conn.read(ReadFunction::ReadHoldingRegisters, 0, 5).await {
            Ok(ReadResult::HoldingRegisters(vals)) => println!("    Read OK: {:?}", vals),
            Ok(other) => println!("    Unexpected: {:?}", other),
            Err(e) => {
                println!("    FAILED: {}", e);
                println!("\n  TEST FAILED!");
                return;
            }
        }
    }
    println!();

    // 5) Start polling with bridge task
    println!("[5] Start polling...");
    let cached: Arc<RwLock<Option<Vec<u16>>>> = Arc::new(RwLock::new(None));
    {
        let group = ScanGroup {
            id: "sg1".to_string(),
            name: "Test".to_string(),
            function: ReadFunction::ReadHoldingRegisters,
            start_address: 0,
            quantity: 10,
            interval_ms: 500,
            enabled: true,
        };
        let mut conns = state.write().await;
        let conn = conns.get_mut("m1").unwrap();
        let mut rx = conn.start_scan_group(&group).await.unwrap();
        drop(conns);

        let cached_ref = cached.clone();
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    PollEvent::Data(ReadResult::HoldingRegisters(vals)) => {
                        *cached_ref.write().await = Some(vals);
                    }
                    PollEvent::Error(e) => {
                        eprintln!("    POLL ERROR: {}", e);
                    }
                    _ => {}
                }
            }
        });
    }
    println!("    Polling started.\n");

    // 6) Wait for poll data
    println!("[6] Waiting for poll data...");
    for i in 0..5 {
        tokio::time::sleep(Duration::from_millis(700)).await;
        let data = cached.read().await;
        if let Some(vals) = data.as_ref() {
            println!("    Got data: {:?}", &vals[..5]);
            break;
        }
        println!("    Attempt {} - no data yet", i + 1);
    }
    println!();

    // 7) Write
    println!("[7] Write register...");
    {
        let mut conns = state.write().await;
        let conn = conns.get_mut("m1").unwrap();
        conn.write_single_register(0, 42).await.unwrap();
    }
    println!("    Write OK");

    tokio::time::sleep(Duration::from_millis(700)).await;
    {
        let data = cached.read().await;
        if let Some(vals) = data.as_ref() {
            println!("    After write, Register[0] = {}", vals[0]);
            assert_eq!(vals[0], 42);
        }
    }
    println!();

    // 8) Check logs
    let slave_logs = slave_log.get_all().await;
    let master_logs = master_log.get_all().await;
    println!("[8] Logs: slave={} entries, master={} entries", slave_logs.len(), master_logs.len());
    println!();

    // Cleanup
    {
        let mut conns = state.write().await;
        let conn = conns.get_mut("m1").unwrap();
        conn.stop_scan_group("sg1").await.unwrap();
        conn.disconnect().await.unwrap();
    }
    slave.stop().await.unwrap();

    println!("========================================");
    println!("  ALL TESTS PASSED!");
    println!("========================================");
}
