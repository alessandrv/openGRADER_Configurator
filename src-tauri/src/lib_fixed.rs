mod protocol;
mod hid_manager;
mod commands;
mod keycodes;

use commands::*;
use hid_manager::HidManager;
use keycodes::get_keycodes;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::RwLock;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Initialize the HID manager
            let hid_manager = match HidManager::new() {
                Ok(manager) => manager,
                Err(e) => {
                    eprintln!("Failed to initialize HID manager: {}", e);
                    std::process::exit(1);
                }
            };

            // Store the HID manager in app state
            app.manage(Arc::new(RwLock::new(hid_manager)));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Device management
            scan_devices,
            connect_device,
            disconnect_device,
            is_device_connected,
            ping_device,
            check_device_status_and_reconnect,
            auto_connect,
            get_connection_status,
            
            // Device information
            get_device_info,
            load_full_state,
            get_enhanced_connection_status,
            
            // Keymap management
            get_keymap_entry,
            set_keymap_entry,
            get_full_keymap,
            set_full_keymap,
            
            // Encoder management
            get_encoder_entry,
            set_encoder_entry,
            get_all_encoders,
            set_all_encoders,
            
            // Configuration management
            save_config,
            load_config,
            reset_config,
            
            // I2C device management
            get_i2c_devices,
            
            // System commands
            reboot_device,
            
            // Utility commands
            get_keycodes
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}