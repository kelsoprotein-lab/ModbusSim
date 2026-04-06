mod commands;
mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            // Connection commands
            commands::create_master_connection,
            commands::connect_master,
            commands::disconnect_master,
            commands::delete_master_connection,
            commands::list_master_connections,
            // Scan group commands
            commands::add_scan_group,
            commands::update_scan_group,
            commands::remove_scan_group,
            commands::list_scan_groups,
            // Polling commands
            commands::start_polling,
            commands::stop_polling,
            commands::start_all_polling,
            commands::stop_all_polling,
            commands::get_poll_data,
            // Read/Write commands
            commands::read_once,
            commands::write_single_register,
            commands::write_single_coil,
            commands::write_multiple_registers,
            commands::write_multiple_coils,
            // Log commands
            commands::get_communication_logs,
            commands::clear_communication_logs,
            commands::export_logs_csv,
            // Scan commands
            commands::start_slave_id_scan,
            commands::start_register_scan,
            commands::cancel_scan,
            // Tool commands
            commands::convert_plc_to_modbus,
            commands::convert_modbus_to_plc,
            commands::calculate_crc16,
            commands::calculate_lrc,
            commands::parse_hex,
            // Project file commands
            commands::save_project_file,
            commands::load_project_file,
            // Serial port commands
            commands::list_serial_ports,
        ])
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
