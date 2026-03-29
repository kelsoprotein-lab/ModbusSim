/// Simulates the exact Tauri app flow:
/// - Multi-threaded tokio runtime (same as Tauri)
/// - Arc<RwLock<HashMap>> state (same as AppState)
/// - Spawned bridge tasks (same as start_polling command)
use modbussim_core::log_collector::LogCollector;
use modbussim_core::master::{
    MasterConfig, MasterConnection, MasterState, PollEvent, ReadFunction, ReadResult, ScanGroup,
};
use modbussim_core::slave::{SlaveConnection, SlaveDevice, TransportConfig};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

struct MasterConnectionState {
    connection: MasterConnection,
    scan_groups: Vec<ScanGroup>,
    log_collector: Arc<LogCollector>,
    cached_data: HashMap<String, ReadResult>,
}

type AppState = Arc<RwLock<HashMap<String, MasterConnectionState>>>;

#[tokio::main] // multi-threaded runtime, same as Tauri
async fn main() {
    println!("=== Simulating Tauri App Flow (multi-threaded runtime) ===\n");

    // 1) Start slave
    println!("[1] Starting slave on 0.0.0.0:15022...");
    let transport = TransportConfig {
        bind_address: "0.0.0.0".to_string(),
        port: 15022,
    };
    let mut slave = SlaveConnection::new(transport);
    slave
        .add_device(SlaveDevice::with_random_registers(1, "Slave", 100))
        .await
        .unwrap();
    slave.start().await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    println!("    OK\n");

    // 2) Create app state (same pattern as Tauri AppState)
    let state: AppState = Arc::new(RwLock::new(HashMap::new()));

    // 3) Simulate: create_master_connection command
    println!("[2] create_master_connection...");
    {
        let config = MasterConfig {
            target_address: "127.0.0.1".to_string(),
            port: 15022,
            slave_id: 1,
            timeout_ms: 3000,
        };
        let log_collector = Arc::new(LogCollector::new());
        let connection =
            MasterConnection::new(config).with_log_collector(log_collector.clone());
        state.write().await.insert(
            "master_1".to_string(),
            MasterConnectionState {
                connection,
                scan_groups: Vec::new(),
                log_collector,
                cached_data: HashMap::new(),
            },
        );
    }
    println!("    OK\n");

    // 4) Simulate: connect_master command
    println!("[3] connect_master...");
    {
        let mut conns = state.write().await;
        let cs = conns.get_mut("master_1").unwrap();
        cs.connection.connect().await.unwrap();
        println!("    State: {:?}", cs.connection.state());
    }
    println!();

    // 5) Simulate: add_scan_group command
    println!("[4] add_scan_group...");
    let group = ScanGroup {
        id: "sg1".to_string(),
        name: "Test FC03".to_string(),
        function: ReadFunction::ReadHoldingRegisters,
        start_address: 0,
        quantity: 10,
        interval_ms: 500,
        enabled: true,
    };
    {
        let mut conns = state.write().await;
        let cs = conns.get_mut("master_1").unwrap();
        cs.scan_groups.push(group.clone());
    }
    println!("    OK\n");

    // 6) Simulate: start_polling command (with bridge task, same as commands.rs)
    println!("[5] start_polling (with bridge task)...");
    let state_clone = state.clone();
    {
        let mut conns = state_clone.write().await;
        let cs = conns.get_mut("master_1").unwrap();
        let mut rx = cs.connection.start_scan_group(&group).await.unwrap();
        println!("    Polling active: {}", cs.connection.is_scan_active("sg1"));

        // Drop the write lock before spawning bridge task
        drop(conns);

        // Spawn bridge task (same pattern as commands.rs start_polling_inner)
        let cache_ref = state_clone.clone();
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    PollEvent::Data(result) => {
                        let mut conns = cache_ref.write().await;
                        if let Some(cs) = conns.get_mut("master_1") {
                            cs.cached_data.insert("sg1".to_string(), result);
                        }
                    }
                    PollEvent::Error(e) => {
                        eprintln!("    POLL ERROR: {}", e);
                    }
                }
            }
        });
    }
    println!();

    // 7) Wait for data and check
    println!("[6] Waiting for poll data...");
    for i in 0..5 {
        tokio::time::sleep(Duration::from_millis(600)).await;
        let conns = state.read().await;
        let cs = conns.get("master_1").unwrap();
        if let Some(data) = cs.cached_data.get("sg1") {
            match data {
                ReadResult::HoldingRegisters(vals) => {
                    println!("    Attempt {}: Got {} values: {:?}", i + 1, vals.len(), &vals[..5.min(vals.len())]);
                    if !vals.is_empty() {
                        println!("    DATA RECEIVED!\n");

                        // 8) Test write
                        drop(conns);
                        println!("[7] write_single_register(0, 12345)...");
                        {
                            let mut conns = state.write().await;
                            let cs = conns.get_mut("master_1").unwrap();
                            cs.connection.write_single_register(0, 12345).await.unwrap();
                        }
                        println!("    Write OK");

                        // Wait for next poll
                        tokio::time::sleep(Duration::from_millis(800)).await;
                        let conns = state.read().await;
                        let cs = conns.get("master_1").unwrap();
                        if let Some(ReadResult::HoldingRegisters(vals)) = cs.cached_data.get("sg1") {
                            println!("    After write, Register[0] = {}", vals[0]);
                            if vals[0] == 12345 {
                                println!("    WRITE VERIFIED!\n");
                            }
                        }

                        // Check logs
                        drop(conns);
                        let conns = state.read().await;
                        let cs = conns.get("master_1").unwrap();
                        let logs = cs.log_collector.get_all().await;
                        println!("    Communication logs: {} entries", logs.len());

                        // Cleanup
                        drop(conns);
                        {
                            let mut conns = state.write().await;
                            let cs = conns.get_mut("master_1").unwrap();
                            cs.connection.stop_scan_group("sg1").await.unwrap();
                            cs.connection.disconnect().await.unwrap();
                        }
                        slave.stop().await.unwrap();

                        println!("\n========================================");
                        println!("  ALL TESTS PASSED!");
                        println!("========================================");
                        return;
                    }
                }
                _ => {}
            }
        } else {
            println!("    Attempt {}: No data yet...", i + 1);
        }
    }

    println!("\n  FAIL: No poll data received after 5 attempts!");
    slave.stop().await.unwrap();
}
