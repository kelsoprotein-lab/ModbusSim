mod commands;
mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            // Slave connection commands
            commands::create_slave_connection,
            commands::start_slave_connection,
            commands::stop_slave_connection,
            commands::delete_slave_connection,
            commands::list_slave_connections,
            // Slave device commands
            commands::add_slave_device,
            commands::remove_slave_device,
            commands::list_slave_devices,
            // Register commands
            commands::add_register,
            commands::remove_register,
            commands::read_register,
            commands::write_register,
            commands::list_registers,
            commands::export_registers,
            commands::import_registers,
            // Log commands
            commands::get_communication_logs,
            commands::clear_communication_logs,
            commands::export_logs_csv,
            // Tool commands
            commands::convert_plc_to_modbus,
            commands::convert_modbus_to_plc,
            commands::calculate_crc16,
            commands::calculate_lrc,
            commands::parse_hex,
            // State persistence commands
            commands::export_app_state,
            commands::import_app_state,
            commands::clear_app_state,
            // Simulation commands
            commands::random_mutate_registers,
            // Project file commands
            commands::save_project_file,
            commands::load_project_file,
            // Serial port commands
            commands::list_serial_ports,
            // Data source commands
            commands::set_data_source,
            commands::remove_data_source,
            commands::list_data_sources,
            commands::start_data_source_runner,
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
