use crate::hid_manager::{HidManager, DeviceDescriptor};
use crate::protocol::{DeviceInfo, KeymapEntry, EncoderEntry, I2CDeviceInfo, SlaveKeymapEntry, SlaveEncoderEntry, BoardLayoutInfo, LayerState, LayoutCellType, LayoutCell, SliderConfig};
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
pub async fn get_board_layout(state: State<'_, AppState>) -> Result<BoardLayoutInfo, String> {
    let manager = state.read().await;
    manager.get_board_layout().await
}

#[tauri::command]
pub async fn get_layout_cell_type(row: u8, col: u8, state: State<'_, AppState>) -> Result<u8, String> {
    let manager = state.read().await;
    manager.get_layout_cell_type(row, col).await
}

#[tauri::command]
pub async fn get_layout_cell_component_id(row: u8, col: u8, state: State<'_, AppState>) -> Result<u8, String> {
    let manager = state.read().await;
    manager.get_layout_cell_component_id(row, col).await
}

#[tauri::command]
pub async fn get_slider_value(slider_id: u8, state: State<'_, AppState>) -> Result<u8, String> {
    let manager = state.read().await;
    manager.get_slider_value(slider_id).await
}

#[tauri::command]
pub async fn get_slider_config(layer: u8, slider_id: u8, state: State<'_, AppState>) -> Result<SliderConfig, String> {
    let manager = state.read().await;
    manager.get_slider_config(layer, slider_id).await
}

#[tauri::command]
pub async fn set_slider_config(config: SliderConfig, state: State<'_, AppState>) -> Result<(), String> {
    let manager = state.read().await;
    manager.set_slider_config(&config).await
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
    
    // Step 3: Fetch layout metadata (optional but preferred)
    println!("simple_connect: fetching board layout metadata...");
    let layout = {
        let manager = state.read().await;
        match manager.get_board_layout().await {
            Ok(info) => Some(info),
            Err(e) => {
                println!("simple_connect: warning - failed to fetch board layout: {}", e);
                None
            }
        }
    };

    let full_state = build_full_state(&state, device_info, layout).await?;
    println!("simple_connect: all data loaded successfully");
    Ok(full_state)
}

// Simple disconnect command
#[tauri::command]
pub async fn simple_disconnect(state: State<'_, AppState>) -> Result<(), String> {
    let manager_w = state.write().await;
    manager_w.disconnect();
    println!("simple_disconnect: device disconnected");
    Ok(())
}

async fn build_full_state(
    state: &State<'_, AppState>,
    device_info: DeviceInfo,
    layout: Option<BoardLayoutInfo>,
) -> Result<FullState, String> {
    println!(
        "build_full_state: snapshotting '{}' (layers={}, rows={} cols={} encoders={})",
        device_info.device_name,
        device_info.layer_count,
        device_info.matrix_rows,
        device_info.matrix_cols,
        device_info.encoder_count
    );

    let layer_state = {
        let manager = state.read().await;
        match manager.get_layer_state().await {
            Ok(state) => Some(state),
            Err(e) => {
                println!("build_full_state: layer state unavailable ({})", e);
                None
            }
        }
    };

    let layer_count = device_info.layer_count.max(1);
    let matrix_rows = device_info.matrix_rows;
    let matrix_cols = device_info.matrix_cols;
    let encoder_count = device_info.encoder_count;

    println!(
        "build_full_state: loading keymap (layers={}, rows={}, cols={})",
        layer_count,
        matrix_rows,
        matrix_cols
    );
    let keymap = {
        let manager = state.read().await;
        let mut all_layers = Vec::with_capacity(layer_count as usize);
        let mut total_entries = 0usize;
        let mut failed_entries = 0usize;

        for layer_idx in 0..layer_count {
            let mut layer_rows = Vec::with_capacity(matrix_rows as usize);
            for row_idx in 0..matrix_rows {
                let mut row_entries = Vec::with_capacity(matrix_cols as usize);
                for col_idx in 0..matrix_cols {
                    total_entries += 1;
                    match manager.get_keymap_entry(layer_idx, row_idx, col_idx).await {
                        Ok(entry) => row_entries.push(entry),
                        Err(e) => {
                            failed_entries += 1;
                            println!(
                                "build_full_state: keymap read failed at L{} R{} C{} -> {}",
                                layer_idx, row_idx, col_idx, e
                            );
                            row_entries.push(KeymapEntry {
                                layer: layer_idx,
                                row: row_idx,
                                col: col_idx,
                                keycode: 0,
                            });
                        }
                    }
                }
                layer_rows.push(row_entries);
            }
            all_layers.push(layer_rows);
        }

        println!(
            "build_full_state: keymap complete ({} entries, {} failures)",
            total_entries,
            failed_entries
        );
        all_layers
    };

    println!(
        "build_full_state: loading encoders (layers={}, count={})",
        layer_count,
        encoder_count
    );
    let encoders = {
        let manager = state.read().await;
        let mut all_layers = Vec::with_capacity(layer_count as usize);
        let mut failed_encoders = 0usize;

        for layer_idx in 0..layer_count {
            let mut layer_encoders = Vec::with_capacity(encoder_count as usize);
            for encoder_idx in 0..encoder_count {
                match manager.get_encoder_entry(layer_idx, encoder_idx).await {
                    Ok(entry) => layer_encoders.push(entry),
                    Err(e) => {
                        failed_encoders += 1;
                        println!(
                            "build_full_state: encoder read failed at L{} #{} -> {}",
                            layer_idx, encoder_idx, e
                        );
                        layer_encoders.push(EncoderEntry {
                            layer: layer_idx,
                            encoder_id: encoder_idx,
                            ccw_keycode: 0,
                            cw_keycode: 0,
                            reserved: 0,
                        });
                    }
                }
            }
            all_layers.push(layer_encoders);
        }

        println!(
            "build_full_state: encoders complete ({} failures)",
            failed_encoders
        );
        all_layers
    };

    Ok(FullState {
        device_info,
        keymap,
        encoders,
        layout,
        layer_state,
    })
}

// Batched full-state loader to avoid multiple interleaved invokes
#[derive(serde::Serialize)]
pub struct FullState {
    pub device_info: DeviceInfo,
    pub keymap: Vec<Vec<Vec<KeymapEntry>>>,
    pub encoders: Vec<Vec<EncoderEntry>>,
    pub layout: Option<BoardLayoutInfo>,
    pub layer_state: Option<LayerState>,
}

#[tauri::command]
pub async fn load_full_state(state: State<'_, AppState>) -> Result<FullState, String> {
    let device_info = {
        let manager = state.read().await;
        manager.get_device_info().await?
    };

    let layout = {
        let manager = state.read().await;
        match manager.get_board_layout().await {
            Ok(info) => Some(info),
            Err(e) => {
                println!("load_full_state: warning - no layout info available: {}", e);
                None
            }
        }
    };

    build_full_state(&state, device_info, layout).await
}

// Enhanced connection status that includes all data in one call
#[derive(Serialize)]
pub struct EnhancedConnectionStatus {
    pub connected: bool,
    pub device_info: Option<DeviceInfo>,
    pub keymap: Option<Vec<Vec<Vec<KeymapEntry>>>>,
    pub encoders: Option<Vec<Vec<EncoderEntry>>>,
    pub layout: Option<BoardLayoutInfo>,
    pub layer_state: Option<LayerState>,
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
            layout: None,
            layer_state: None,
            error: None,
        });
    }

    println!("Getting device info...");
    let device_info = match manager.get_device_info().await {
        Ok(info) => {
            println!("Device info retrieved: {}", info.device_name);
            info
        }
        Err(e) => {
            println!("Failed to get device info: {}", e);
            return Ok(EnhancedConnectionStatus {
                connected: false,
                device_info: None,
                keymap: None,
                encoders: None,
                layout: None,
                layer_state: None,
                error: Some(format!("Failed to get device info: {}", e)),
            });
        }
    };
    drop(manager);

    let layout = {
        let manager = state.read().await;
        match manager.get_board_layout().await {
            Ok(info) => Some(info),
            Err(e) => {
                println!("Enhanced connection status: layout unavailable: {}", e);
                None
            }
        }
    };

    let snapshot = build_full_state(&state, device_info, layout).await?;
    let FullState {
        device_info,
        keymap,
        encoders,
        layout,
        layer_state,
    } = snapshot;

    println!("Enhanced connection status complete: connected=true");
    Ok(EnhancedConnectionStatus {
        connected: true,
        device_info: Some(device_info),
        keymap: Some(keymap),
        encoders: Some(encoders),
        layout,
        layer_state,
        error: None,
    })
}

// Keymap management commands

#[tauri::command]
pub async fn get_keymap_entry(
    layer: u8,
    row: u8,
    col: u8,
    state: State<'_, AppState>,
) -> Result<KeymapEntry, String> {
    let manager = state.read().await;
    manager.get_keymap_entry(layer, row, col).await
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
pub async fn get_full_keymap(state: State<'_, AppState>) -> Result<Vec<Vec<Vec<KeymapEntry>>>, String> {
    let device_info = {
        let manager = state.read().await;
        manager.get_device_info().await?
    };

    let snapshot = build_full_state(&state, device_info, None).await?;
    Ok(snapshot.keymap)
}

#[tauri::command]
pub async fn set_full_keymap(
    keymap: Vec<Vec<Vec<KeymapEntry>>>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let manager = state.read().await;
    
    for layer_entries in keymap {
        for row_entries in layer_entries {
            for entry in row_entries {
                manager.set_keymap_entry(&entry).await?;
            }
        }
    }
    
    Ok(())
}

// Encoder management commands

#[tauri::command]
pub async fn get_encoder_entry(
    layer: u8,
    encoder_id: u8,
    state: State<'_, AppState>,
) -> Result<EncoderEntry, String> {
    let manager = state.read().await;
    manager.get_encoder_entry(layer, encoder_id).await
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
pub async fn get_all_encoders(state: State<'_, AppState>) -> Result<Vec<Vec<EncoderEntry>>, String> {
    let device_info = {
        let manager = state.read().await;
        manager.get_device_info().await?
    };

    let snapshot = build_full_state(&state, device_info, None).await?;
    Ok(snapshot.encoders)
}

#[tauri::command]
pub async fn set_all_encoders(
    encoders: Vec<Vec<EncoderEntry>>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let manager = state.read().await;
    
    for layer_entries in encoders {
        for entry in layer_entries {
            manager.set_encoder_entry(&entry).await?;
        }
    }
    
    Ok(())
}

// Configuration management commands

#[tauri::command]
pub async fn get_layer_state(state: State<'_, AppState>) -> Result<LayerState, String> {
    let manager = state.read().await;
    manager.get_layer_state().await
}

#[tauri::command]
pub async fn set_layer_state(
    layer_state: LayerState,
    state: State<'_, AppState>,
) -> Result<LayerState, String> {
    let manager = state.read().await;
    manager.set_layer_state(&layer_state).await
}

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
    layer: u8,
    row: u8,
    col: u8,
    state: State<'_, AppState>,
) -> Result<SlaveKeymapEntry, String> {
    let manager = state.read().await;
    manager.get_slave_keymap_entry(slave_addr, layer, row, col).await
}

#[tauri::command]
pub async fn set_slave_keymap_entry(
    entry: SlaveKeymapEntry,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let manager = state.read().await;
    manager.set_slave_keymap_entry(&entry).await
}

// Slave device encoder commands

#[tauri::command]
pub async fn get_slave_encoder_entry(
    slave_addr: u8,
    layer: u8,
    encoder_id: u8,
    state: State<'_, AppState>,
) -> Result<SlaveEncoderEntry, String> {
    let manager = state.read().await;
    manager.get_slave_encoder_entry(slave_addr, layer, encoder_id).await
}

#[tauri::command]
pub async fn set_slave_encoder_entry(
    entry: SlaveEncoderEntry,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let manager = state.read().await;
    manager.set_slave_encoder_entry(&entry).await
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
) -> Result<Vec<Vec<Vec<SlaveKeymapEntry>>>, String> {
    let manager = state.read().await;
    
    // Get slave device info first to know matrix dimensions
    let device_info = manager.get_slave_info(slave_addr).await?;
    let layer_count = device_info.layer_count.max(1);
    
    let mut keymap = Vec::with_capacity(layer_count as usize);

    for layer in 0..layer_count {
        let mut layer_rows = Vec::with_capacity(device_info.matrix_rows as usize);
        for row in 0..device_info.matrix_rows {
            let mut row_entries = Vec::with_capacity(device_info.matrix_cols as usize);
            for col in 0..device_info.matrix_cols {
                match manager.get_slave_keymap_entry(slave_addr, layer, row, col).await {
                    Ok(entry) => row_entries.push(entry),
                    Err(_e) => {
                        row_entries.push(SlaveKeymapEntry {
                            slave_addr,
                            layer,
                            row,
                            col,
                            keycode: 0,
                        });
                    }
                }
            }
            layer_rows.push(row_entries);
        }
        keymap.push(layer_rows);
    }
    
    Ok(keymap)
}

#[tauri::command]
pub async fn get_full_slave_encoders(
    slave_addr: u8,
    state: State<'_, AppState>,
) -> Result<Vec<Vec<SlaveEncoderEntry>>, String> {
    let manager = state.read().await;

    let device_info = manager.get_slave_info(slave_addr).await?;
    let layer_count = device_info.layer_count.max(1);
    let mut encoders = Vec::with_capacity(layer_count as usize);

    for layer in 0..layer_count {
        let mut layer_encoders = Vec::with_capacity(device_info.encoder_count as usize);
        for encoder_id in 0..device_info.encoder_count {
            match manager.get_slave_encoder_entry(slave_addr, layer, encoder_id).await {
                Ok(entry) => layer_encoders.push(entry),
                Err(_) => layer_encoders.push(SlaveEncoderEntry {
                    slave_addr,
                    layer,
                    encoder_id,
                    ccw_keycode: 0,
                    cw_keycode: 0,
                    reserved: 0,
                }),
            }
        }
        encoders.push(layer_encoders);
    }

    Ok(encoders)
}

#[tauri::command]
pub async fn set_full_slave_keymap(
    keymap: Vec<Vec<Vec<SlaveKeymapEntry>>>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let manager = state.read().await;
    
    for layer_rows in keymap {
        for row_entries in layer_rows {
            for entry in row_entries {
                manager.set_slave_keymap_entry(&entry).await?;
            }
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

#[tauri::command]
pub async fn set_full_slave_encoders(
    encoders: Vec<Vec<SlaveEncoderEntry>>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let manager = state.read().await;

    for layer_entries in encoders {
        for entry in layer_entries {
            manager.set_slave_encoder_entry(&entry).await?;
        }
    }

    Ok(())
}