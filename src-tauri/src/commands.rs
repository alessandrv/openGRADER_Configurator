use crate::hid_manager::{HidManager, DeviceDescriptor};
use crate::protocol::{DeviceInfo, KeymapEntry, EncoderEntry, I2CDeviceInfo, SlaveKeymapEntry};
use std::sync::Arc;
use tauri::{AppHandle, State, Emitter};
use tokio::sync::RwLock;
use serde::Serialize;

pub type AppState = Arc<RwLock<HidManager>>;

// Device management commands

#[tauri::command]
pub async fn scan_devices(state: State<'_, AppState>) -> Result<Vec<DeviceDescriptor>, String> {
    let manager = state.read().await;
    manager.scan_devices().await
}

#[tauri::command]
pub async fn connect_device(path: String, state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    let manager = state.read().await;
    let res = manager.connect(&path);
    if res.is_ok() {
        let _ = app.emit("og:connected", ());
    }
    res
}

#[tauri::command]
pub async fn disconnect_device(state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    let manager = state.read().await;
    manager.disconnect();
    let _ = app.emit("og:disconnected", ());
    Ok(())
}

#[tauri::command]
pub async fn is_device_connected(state: State<'_, AppState>) -> Result<bool, String> {
    let manager = state.read().await;
    Ok(manager.is_connected())
}

#[derive(serde::Serialize)]
pub struct ConnectionStatus {
    pub connected: bool,
    pub device_info: Option<DeviceInfo>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn get_connection_status(state: State<'_, AppState>) -> Result<ConnectionStatus, String> {
    let manager = state.read().await;
    let connected = manager.is_connected();
    
    let mut status = ConnectionStatus {
        connected,
        device_info: None,
        error: None,
    };
    
    if connected {
        match manager.get_device_info().await {
            Ok(info) => {
                status.device_info = Some(info);
            }
            Err(e) => {
                status.error = Some(e);
                status.connected = false; // If we can't get info, consider it disconnected
            }
        }
    }
    
    Ok(status)
}

#[tauri::command]
pub async fn ping_device(state: State<'_, AppState>) -> Result<bool, String> {
    let manager = state.read().await;
    Ok(manager.is_connected())
}

#[tauri::command]
pub async fn check_device_status_and_reconnect(state: State<'_, AppState>) -> Result<bool, String> {
    let manager = state.read().await;
    let connected = manager.is_connected();
    println!("Device status check: connected={}", connected);
    Ok(connected)
}

// Auto-connect command exposed to frontend
#[tauri::command]
pub async fn auto_connect(state: State<'_, AppState>, app: AppHandle) -> Result<bool, String> {
    let manager = state.read().await;
    if manager.is_connected() {
        println!("auto_connect: already connected, skipping");
        return Ok(true);
    }
    drop(manager);
    
    println!("auto_connect: attempting to connect...");
    let manager_w = state.write().await;
    let ok = manager_w.auto_connect();
    drop(manager_w);

    if let Ok(true) = ok {
        println!("auto_connect: connection successful, verifying device...");
        // Verify device responds to GetInfo before emitting event
        for attempt in 0..3 {
            println!("auto_connect: verification attempt {} of 3", attempt + 1);
            let mgr = state.read().await;
            let res = mgr.get_device_info().await;
            drop(mgr);
            match res {
                Ok(info) => {
                    println!("auto_connect: device verification successful, device_name={}", info.device_name);
                    println!("auto_connect: emitting og:connected event");
                    let emit_result = app.emit("og:connected", ());
                    println!("auto_connect: og:connected emit result: {:?}", emit_result);
                    return Ok(true);
                }
                Err(e) if attempt < 2 => {
                    println!("auto_connect: device verification failed (attempt {}): {}", attempt + 1, e);
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    continue;
                }
                Err(e) => {
                    println!("auto_connect: all device verification attempts failed: {}", e);
                    // Disconnect to allow next poll to retry cleanly
                    let mgrw = state.write().await;
                    mgrw.disconnect();
                    return Ok(false);
                }
            }
        }
    } else {
        println!("auto_connect: initial connection failed: {:?}", ok);
    }
    Ok(false)
}

// Simple connect command that connects and loads all data
#[tauri::command]
pub async fn simple_connect(state: State<'_, AppState>) -> Result<FullState, String> {
    // Step 1: Connect to device
    {
        let manager_w = state.write().await;
        
        // First disconnect if already connected
        if manager_w.is_connected() {
            manager_w.disconnect();
            println!("simple_connect: disconnected existing connection");
            // Small delay after disconnect
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        
        // Try to connect
        println!("simple_connect: attempting to connect...");
        match manager_w.auto_connect() {
            Ok(true) => {
                println!("simple_connect: connection successful");
            }
            Ok(false) => {
                return Err("No device found".to_string());
            }
            Err(e) => {
                return Err(format!("Connection failed: {}", e));
            }
        }
    } // Release write lock
    
    // Small delay to ensure connection is stable
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    
    // Step 2: Get device info
    println!("simple_connect: getting device info...");
    let device_info = {
        let manager = state.read().await;
        manager.get_device_info().await?
    };
    println!("simple_connect: device connected: {} ({}x{} matrix, {} encoders)", 
             device_info.device_name, device_info.matrix_rows, device_info.matrix_cols, device_info.encoder_count);
    
    // Step 3: Load keymap
    println!("simple_connect: loading keymap...");
    let keymap = {
        let manager = state.read().await;
        let mut keymap = Vec::new();
        let mut total_keys = 0;
        let mut failed_keys = 0;
        
        for row in 0..device_info.matrix_rows {
            let mut row_entries = Vec::new();
            for col in 0..device_info.matrix_cols {
                total_keys += 1;
                match manager.get_keymap_entry(row, col).await {
                    Ok(entry) => {
                        row_entries.push(entry);
                        if total_keys % 10 == 0 {
                            println!("simple_connect: loaded keymap entries: {}/{}", total_keys, device_info.matrix_rows * device_info.matrix_cols);
                        }
                    },
                    Err(e) => {
                        failed_keys += 1;
                        println!("Warning: failed to read keymap entry at {},{}: {}", row, col, e);
                        row_entries.push(KeymapEntry { row, col, keycode: 0 });
                    }
                }
            }
            keymap.push(row_entries);
        }
        println!("simple_connect: keymap loaded: {} rows, {} total keys, {} failed", keymap.len(), total_keys, failed_keys);
        keymap
    };
    
    // Step 4: Load encoders
    println!("simple_connect: loading encoders...");
    let encoders = {
        let manager = state.read().await;
        let mut encoders = Vec::new();
        let mut failed_encoders = 0;
        
        for encoder_id in 0..device_info.encoder_count {
            match manager.get_encoder_entry(encoder_id).await {
                Ok(entry) => {
                    println!("simple_connect: loaded encoder {}: CCW={}, CW={}", encoder_id, entry.ccw_keycode, entry.cw_keycode);
                    encoders.push(entry);
                },
                Err(e) => {
                    failed_encoders += 1;
                    println!("Warning: failed to read encoder entry {}: {}", encoder_id, e);
                    encoders.push(EncoderEntry { encoder_id, ccw_keycode: 0, cw_keycode: 0, reserved: 0 });
                }
            }
        }
        println!("simple_connect: encoders loaded: {} encoders, {} failed", encoders.len(), failed_encoders);
        encoders
    };
    
    println!("simple_connect: all data loaded successfully");
    Ok(FullState { device_info, keymap, encoders })
}

// Simple disconnect command
#[tauri::command]
pub async fn simple_disconnect(state: State<'_, AppState>) -> Result<(), String> {
    let manager_w = state.write().await;
    manager_w.disconnect();
    println!("simple_disconnect: device disconnected");
    Ok(())
}

// Batched full-state loader to avoid multiple interleaved invokes
#[derive(serde::Serialize)]
pub struct FullState {
    pub device_info: DeviceInfo,
    pub keymap: Vec<Vec<KeymapEntry>>,
    pub encoders: Vec<EncoderEntry>,
}

#[tauri::command]
pub async fn load_full_state(state: State<'_, AppState>) -> Result<FullState, String> {
    let manager = state.read().await;
    let device_info = manager.get_device_info().await?;

    // Build keymap
    let mut keymap = Vec::new();
    for row in 0..device_info.matrix_rows {
        let mut row_entries = Vec::new();
        for col in 0..device_info.matrix_cols {
            match manager.get_keymap_entry(row, col).await {
                Ok(entry) => row_entries.push(entry),
                Err(_) => row_entries.push(KeymapEntry { row, col, keycode: 0 }),
            }
        }
        keymap.push(row_entries);
    }

    // Build encoders
    let mut encoders = Vec::new();
    for encoder_id in 0..device_info.encoder_count {
        match manager.get_encoder_entry(encoder_id).await {
            Ok(entry) => encoders.push(entry),
            Err(_) => encoders.push(EncoderEntry { encoder_id, ccw_keycode: 0, cw_keycode: 0, reserved: 0 }),
        }
    }

    Ok(FullState { device_info, keymap, encoders })
}

// Enhanced connection status that includes all data in one call
#[derive(Serialize)]
pub struct EnhancedConnectionStatus {
    pub connected: bool,
    pub device_info: Option<DeviceInfo>,
    pub keymap: Option<Vec<Vec<KeymapEntry>>>,
    pub encoders: Option<Vec<EncoderEntry>>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn get_enhanced_connection_status(state: State<'_, AppState>) -> Result<EnhancedConnectionStatus, String> {
    let manager = state.read().await;
    
    println!("Enhanced connection status: is_connected = {}", manager.is_connected());
    
    if !manager.is_connected() {
        return Ok(EnhancedConnectionStatus {
            connected: false,
            device_info: None,
            keymap: None,
            encoders: None,
            error: None,
        });
    }

    // Get device info
    println!("Getting device info...");
    let device_info = match manager.get_device_info().await {
        Ok(info) => {
            println!("Device info retrieved: {}", info.device_name);
            Some(info)
        },
        Err(e) => {
            println!("Failed to get device info: {}", e);
            return Ok(EnhancedConnectionStatus {
                connected: false,
                device_info: None,
                keymap: None,
                encoders: None,
                error: Some(format!("Failed to get device info: {}", e)),
            });
        },
    };

    let device_info_ref = device_info.as_ref().unwrap();

    // Build keymap
    println!("Building keymap: {}x{}", device_info_ref.matrix_rows, device_info_ref.matrix_cols);
    let mut keymap = Vec::new();
    for row in 0..device_info_ref.matrix_rows {
        let mut row_entries = Vec::new();
        for col in 0..device_info_ref.matrix_cols {
            match manager.get_keymap_entry(row, col).await {
                Ok(entry) => row_entries.push(entry),
                Err(_) => row_entries.push(KeymapEntry { row, col, keycode: 0 }),
            }
        }
        keymap.push(row_entries);
    }
    println!("Keymap built: {} rows", keymap.len());

    // Build encoders
    println!("Building encoders: {} encoders", device_info_ref.encoder_count);
    let mut encoders = Vec::new();
    for encoder_id in 0..device_info_ref.encoder_count {
        match manager.get_encoder_entry(encoder_id).await {
            Ok(entry) => encoders.push(entry),
            Err(_) => encoders.push(EncoderEntry { encoder_id, ccw_keycode: 0, cw_keycode: 0, reserved: 0 }),
        }
    }
    println!("Encoders built: {} encoders", encoders.len());

    println!("Enhanced connection status complete: connected=true");
    Ok(EnhancedConnectionStatus {
        connected: true,
        device_info,
        keymap: Some(keymap),
        encoders: Some(encoders),
        error: None,
    })
}

// Keymap management commands

#[tauri::command]
pub async fn get_keymap_entry(
    row: u8,
    col: u8,
    state: State<'_, AppState>,
) -> Result<KeymapEntry, String> {
    let manager = state.read().await;
    manager.get_keymap_entry(row, col).await
}

#[tauri::command]
pub async fn set_keymap_entry(
    entry: KeymapEntry,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let manager = state.read().await;
    manager.set_keymap_entry(&entry).await
}

#[tauri::command]
pub async fn get_full_keymap(state: State<'_, AppState>) -> Result<Vec<Vec<KeymapEntry>>, String> {
    let manager = state.read().await;
    
    // Get device info first to know matrix dimensions
    let device_info = manager.get_device_info().await?;
    
    let mut keymap = Vec::new();
    
    for row in 0..device_info.matrix_rows {
        let mut row_entries = Vec::new();
        
        for col in 0..device_info.matrix_cols {
            match manager.get_keymap_entry(row, col).await {
                Ok(entry) => row_entries.push(entry),
                Err(_e) => {
                    // If we can't read a key, create a placeholder
                    row_entries.push(KeymapEntry {
                        row,
                        col,
                        keycode: 0, // KC_NO
                    });
                }
            }
        }
        
        keymap.push(row_entries);
    }
    
    Ok(keymap)
}

#[tauri::command]
pub async fn set_full_keymap(
    keymap: Vec<Vec<KeymapEntry>>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let manager = state.read().await;
    
    for row_entries in keymap {
        for entry in row_entries {
            manager.set_keymap_entry(&entry).await?;
        }
    }
    
    Ok(())
}

// Encoder management commands

#[tauri::command]
pub async fn get_encoder_entry(
    encoder_id: u8,
    state: State<'_, AppState>,
) -> Result<EncoderEntry, String> {
    let manager = state.read().await;
    manager.get_encoder_entry(encoder_id).await
}

#[tauri::command]
pub async fn set_encoder_entry(
    entry: EncoderEntry,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let manager = state.read().await;
    manager.set_encoder_entry(&entry).await
}

#[tauri::command]
pub async fn get_all_encoders(state: State<'_, AppState>) -> Result<Vec<EncoderEntry>, String> {
    let manager = state.read().await;
    
    // Get device info first to know encoder count
    let device_info = manager.get_device_info().await?;
    
    let mut encoders = Vec::new();
    
    for encoder_id in 0..device_info.encoder_count {
        match manager.get_encoder_entry(encoder_id).await {
            Ok(entry) => encoders.push(entry),
            Err(_) => {
                // If we can't read an encoder, create a placeholder
                encoders.push(EncoderEntry {
                    encoder_id,
                    ccw_keycode: 0,
                    cw_keycode: 0,
                    reserved: 0,
                });
            }
        }
    }
    
    Ok(encoders)
}

#[tauri::command]
pub async fn set_all_encoders(
    encoders: Vec<EncoderEntry>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let manager = state.read().await;
    
    for entry in encoders {
        manager.set_encoder_entry(&entry).await?;
    }
    
    Ok(())
}

// Configuration management commands

#[tauri::command]
pub async fn save_config(state: State<'_, AppState>) -> Result<(), String> {
    let manager = state.read().await;
    manager.save_config().await
}

#[tauri::command]
pub async fn load_config(state: State<'_, AppState>) -> Result<(), String> {
    let manager = state.read().await;
    manager.load_config().await
}

#[tauri::command]
pub async fn reset_config(state: State<'_, AppState>) -> Result<(), String> {
    let manager = state.read().await;
    manager.reset_config().await
}

// Slave device keymap commands

#[tauri::command]
pub async fn get_slave_keymap_entry(
    slave_addr: u8,
    row: u8,
    col: u8,
    state: State<'_, AppState>,
) -> Result<SlaveKeymapEntry, String> {
    let manager = state.read().await;
    manager.get_slave_keymap_entry(slave_addr, row, col).await
}

#[tauri::command]
pub async fn set_slave_keymap_entry(
    entry: SlaveKeymapEntry,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let manager = state.read().await;
    manager.set_slave_keymap_entry(&entry).await
}

#[tauri::command]
pub async fn get_slave_info(
    slave_addr: u8,
    state: State<'_, AppState>,
) -> Result<DeviceInfo, String> {
    let manager = state.read().await;
    manager.get_slave_info(slave_addr).await
}

#[tauri::command]
pub async fn get_i2c_devices(
    state: State<'_, AppState>,
) -> Result<Vec<I2CDeviceInfo>, String> {
    let manager = state.read().await;
    manager.get_i2c_devices().await
}

#[tauri::command]
pub async fn get_full_slave_keymap(
    slave_addr: u8,
    state: State<'_, AppState>,
) -> Result<Vec<Vec<SlaveKeymapEntry>>, String> {
    let manager = state.read().await;
    
    // Get slave device info first to know matrix dimensions
    let device_info = manager.get_slave_info(slave_addr).await?;
    
    let mut keymap = Vec::new();
    
    for row in 0..device_info.matrix_rows {
        let mut row_entries = Vec::new();
        
        for col in 0..device_info.matrix_cols {
            match manager.get_slave_keymap_entry(slave_addr, row, col).await {
                Ok(entry) => row_entries.push(entry),
                Err(_e) => {
                    // If we can't read a key, create a placeholder
                    row_entries.push(SlaveKeymapEntry {
                        slave_addr,
                        row,
                        col,
                        keycode: 0, // KC_NO
                    });
                }
            }
        }
        
        keymap.push(row_entries);
    }
    
    Ok(keymap)
}

#[tauri::command]
pub async fn set_full_slave_keymap(
    keymap: Vec<Vec<SlaveKeymapEntry>>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let manager = state.read().await;
    
    for row_entries in keymap {
        for entry in row_entries {
            manager.set_slave_keymap_entry(&entry).await?;
        }
    }
    
    Ok(())
}

// I2C device management commands

// System commands

#[tauri::command]
pub async fn reboot_device(state: State<'_, AppState>) -> Result<(), String> {
    let manager = state.read().await;
    manager.reboot_device().await
}

// Utility commands

#[tauri::command]
pub fn get_keycodes() -> Result<Vec<crate::keycodes::Keycode>, String> {
    let keymap = crate::keycodes::get_keycodes();
    Ok(keymap.into_values().collect())
}