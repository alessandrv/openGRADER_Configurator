use crate::protocol::*;
use hidapi::{HidApi, HidDevice};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::timeout;
use tokio::sync::Semaphore;

pub const OPENGRADER_VID: u16 = 0xCAFE; // Matches firmware USB_VID in usb_descriptors.c
pub const OPENGRADER_PID: u16 = 0x4011; // Matches firmware USB_PID in usb_descriptors.c

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceDescriptor {
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial_number: Option<String>,
    pub product_string: Option<String>,
    pub path: String,
    pub interface_number: i32,
    pub usage_page: u16,
}

pub struct HidManager {
    api: Arc<Mutex<HidApi>>,
    device: Arc<Mutex<Option<HidDevice>>>,
    connected_path: Arc<Mutex<Option<String>>>,
    sequence_counter: Arc<Mutex<u8>>,
    is_mock_device: Arc<Mutex<bool>>,
    cmd_sem: Arc<Semaphore>,
}

impl HidManager {
    pub fn new() -> Result<Self, String> {
        let api = HidApi::new().map_err(|e| format!("Failed to initialize HID API: {}", e))?;
        
        Ok(HidManager {
            api: Arc::new(Mutex::new(api)),
            device: Arc::new(Mutex::new(None)),
            connected_path: Arc::new(Mutex::new(None)),
            sequence_counter: Arc::new(Mutex::new(0)),
            is_mock_device: Arc::new(Mutex::new(false)),
            cmd_sem: Arc::new(Semaphore::new(1)),
        })
    }

    /// Scan for OpenGrader devices
    pub async fn scan_devices(&self) -> Result<Vec<DeviceDescriptor>, String> {
        let mut api = self.api.lock().unwrap();
        api.refresh_devices().map_err(|e| format!("Failed to refresh devices: {}", e))?;
        
        let mut devices = Vec::new();
        
        for device_info in api.device_list() {
            // Look for devices with our VID/PID or check for OpenGrader in product string
            let is_opengrader = device_info.vendor_id() == OPENGRADER_VID && 
                               device_info.product_id() == OPENGRADER_PID;
            
            let has_opengrader_name = device_info.product_string()
                .map(|s| s.to_lowercase().contains("opengrader"))
                .unwrap_or(false);

            // Also check if this is a HID device with usage page 0xFF00 (vendor-defined)
            // This helps identify our custom HID interface
            let is_custom_hid = device_info.usage_page() == 0xFF00 || 
                               device_info.interface_number() == 2; // Our config interface is #2

            if is_opengrader || has_opengrader_name || is_custom_hid {
                println!(
                    "HID dev: path={} vid={:04X} pid={:04X} if={} usage={:04X} name={}",
                    device_info.path().to_string_lossy(),
                    device_info.vendor_id(),
                    device_info.product_id(),
                    device_info.interface_number(),
                    device_info.usage_page(),
                    device_info.product_string().unwrap_or("")
                );
                devices.push(DeviceDescriptor {
                    vendor_id: device_info.vendor_id(),
                    product_id: device_info.product_id(),
                    serial_number: device_info.serial_number().map(|s| s.to_string()),
                    product_string: device_info.product_string().map(|s| s.to_string()),
                    path: device_info.path().to_string_lossy().to_string(),
                    interface_number: device_info.interface_number(),
                    usage_page: device_info.usage_page(),
                });
            }
        }
        
        // If no devices found, add a mock device for testing
        if devices.is_empty() {
            devices.push(DeviceDescriptor {
                vendor_id: OPENGRADER_VID,
                product_id: OPENGRADER_PID,
                serial_number: Some("MOCK001".to_string()),
                product_string: Some("OpenGrader Mock Device".to_string()),
                path: "MOCK_DEVICE_PATH".to_string(),
                interface_number: 2,
                usage_page: 0xFF00,
            });
        }
        
        Ok(devices)
    }

    /// Connect to a specific device
    pub fn connect(&self, device_path: &str) -> Result<(), String> {
        // Handle mock device
        if device_path == "MOCK_DEVICE_PATH" {
            // For mock device, we don't actually create a HID connection
            // but we set the device as connected
            *self.device.lock().unwrap() = None; // Mock device doesn't have real HID device
            *self.is_mock_device.lock().unwrap() = true;
            return Ok(());
        }
        
        let mut api = self.api.lock().unwrap();
        
        println!("Attempting to open HID path: {}", device_path);
        let mut actual_path = device_path.to_string();
        let c_path = std::ffi::CString::new(device_path)
            .map_err(|e| format!("Invalid device path: {}", e))?;
        let device = match api.open_path(&c_path) {
            Ok(d) => d,
            Err(e) => {
                // Fallback: iterate devices and try a best candidate (prefer 'Configuration' interface)
                let _ = api.refresh_devices();
                let mut alt_path: Option<String> = None;
                for di in api.device_list() {
                    let name = di.product_string().unwrap_or("");
                    println!(
                        "HID dev candidate: path={} vid={:04X} pid={:04X} if={} usage={:04X} name={}",
                        di.path().to_string_lossy(),
                        di.vendor_id(),
                        di.product_id(),
                        di.interface_number(),
                        di.usage_page(),
                        name
                    );
                    // Only consider our device VID/PID
                    if di.vendor_id() == OPENGRADER_VID && di.product_id() == OPENGRADER_PID
                        && (name.to_lowercase().contains("configuration") || di.usage_page() == 0xFF00)
                    {
                        alt_path = Some(di.path().to_string_lossy().to_string());
                        break;
                    }
                }
                if let Some(p) = alt_path {
                    println!("Primary open_path failed: {}. Trying alt path: {}", e, p);
                    let c_alt = std::ffi::CString::new(p.clone())
                        .map_err(|e2| format!("Invalid fallback path: {}", e2))?;
                    match api.open_path(&c_alt) {
                        Ok(d2) => { actual_path = p; d2 },
                        Err(e2) => {
                            return Err(format!("Failed to open HID device. primary='{}' fallback='{}'", e, e2));
                        }
                    }
                } else {
                    return Err(format!("Failed to open HID device at path {}: {}", device_path, e));
                }
            }
        };
        
        // Set non-blocking mode
        device.set_blocking_mode(false)
            .map_err(|e| format!("Failed to set non-blocking mode: {}", e))?;
        
        *self.device.lock().unwrap() = Some(device);
        *self.connected_path.lock().unwrap() = Some(actual_path.clone());
        println!("Connected to HID path: {}", actual_path);
        
        Ok(())
    }

    /// Disconnect from the current device
    pub fn disconnect(&self) {
        *self.device.lock().unwrap() = None;
        *self.is_mock_device.lock().unwrap() = false;
        *self.connected_path.lock().unwrap() = None;
    }

    /// Check if we're connected to a device
    pub fn is_connected(&self) -> bool {
        // If mock, treat as connected
        if *self.is_mock_device.lock().unwrap() { return true; }

        // Verify current device handle and that OS still reports the same path
        let has_handle = self.device.lock().unwrap().is_some();
        if !has_handle { return false; }

        // Refresh device list to check if device is still present
        let current_path = self.connected_path.lock().unwrap().clone();
        
        let device_found = {
            let mut api = self.api.lock().unwrap();
            let _ = api.refresh_devices();
            if let Some(path) = &current_path {
                api.device_list().any(|di| di.path().to_string_lossy() == *path)
            } else {
                false
            }
        };
        
        if device_found {
            // Device still present, but let's also verify we can still communicate
            return self.verify_communication();
        }
        
        // Device not present anymore, clean up
        self.disconnect();
        false
    }

    /// Verify we can still communicate with the device
    fn verify_communication(&self) -> bool {
        // Try a simple read to see if device is responsive
        let device_guard = self.device.lock().unwrap();
        if let Some(device) = device_guard.as_ref() {
            let mut buffer = [0u8; 1];
            // Non-blocking read with very short timeout
            match device.read_timeout(&mut buffer, 1) {
                Ok(_) => true,  // Got data or no data available (both OK)
                Err(hidapi::HidError::HidApiError { message }) if message.contains("timeout") => true, // Timeout is OK
                Err(_) => {
                    // Communication error, device likely disconnected
                    drop(device_guard);
                    self.disconnect();
                    false
                }
            }
        } else {
            false
        }
    }

    /// Attempt to auto-connect to an OpenGrader device by VID/PID/name/interface
    pub fn auto_connect(&self) -> Result<bool, String> {
        // Check if already connected
        if self.is_connected() {
            println!("auto_connect: already connected, skipping scan");
            return Ok(true);
        }
        
        let mut api = self.api.lock().unwrap();
        api.refresh_devices().map_err(|e| format!("Failed to refresh devices: {}", e))?;

        // Determine ranked candidate paths while holding the lock (prefer 'Configuration' interface)
        let mut candidates: Vec<(i32, String)> = Vec::new();
        for di in api.device_list() {
            // Only consider our device VID/PID
            let vidpid = di.vendor_id() == OPENGRADER_VID && di.product_id() == OPENGRADER_PID;
            let name_lc = di.product_string().unwrap_or("").to_lowercase();
            let name_cfg = name_lc.contains("configuration");
            let name_opg = name_lc.contains("opengrader");
            let is_cfg_if = di.interface_number() == 2;
            let usage_ff00 = di.usage_page() == 0xFF00;

            let score = if !vidpid { 0 }
                        else if usage_ff00 && name_cfg { 140 }
                        else if usage_ff00 { 130 }
                        else if vidpid && name_cfg { 120 }
                        else if name_cfg { 100 }
                        else if vidpid && is_cfg_if { 90 }
                        else if vidpid { 80 }
                        else if is_cfg_if && name_opg { 70 }
                        else if name_opg { 60 }
                        else { 0 };

            if score > 0 {
                candidates.push((score, di.path().to_string_lossy().to_string()));
            }
        }
        // Sort by score desc and try each until one connects
        candidates.sort_by(|a, b| b.0.cmp(&a.0));
        println!("auto_connect: candidates={:?}", candidates);
        drop(api);

        for (_score, path) in candidates {
            if self.connect(&path).is_ok() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Get next sequence number
    fn get_next_sequence(&self) -> u8 {
        let mut seq = self.sequence_counter.lock().unwrap();
        *seq = seq.wrapping_add(1);
        *seq
    }

    pub async fn send_command(
        &self,
        command: ConfigCommand,
        payload: &[u8],
    ) -> Result<ConfigPacket, String> {
        // Serialize all HID traffic to avoid interleaving (e.g., ping vs keymap fetch)
        let _permit = self
            .cmd_sem
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| "Command semaphore closed".to_string())?;
        let sequence = self.get_next_sequence();
        let packet = ConfigPacket::new(command, sequence, payload);
        let packet_bytes = packet.to_bytes();

        // Debug: Log the first 8 bytes being sent
        println!("Sending bytes: {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X}",
                 packet_bytes[0], packet_bytes[1], packet_bytes[2], packet_bytes[3],
                 packet_bytes[4], packet_bytes[5], packet_bytes[6], packet_bytes[7]);

        // Retry loop: a couple of attempts with reasonable timeouts
        const MAX_RETRIES: u8 = 2;
        let mut attempt: u8 = 0;
        let response = loop {
            attempt = attempt.wrapping_add(1);

            // Send the packet - do this in a separate scope to release the lock
            {
                let device_guard = self.device.lock().unwrap();
                let device = device_guard.as_ref()
                    .ok_or("No device connected")?;

                // On Windows, HIDAPI expects the first byte to be the report ID (0x00 if none)
                // and will NOT send that byte to the device. If we don't provide it, the first
                // byte of our payload becomes the report ID and is dropped, shifting our packet.
                #[cfg(target_os = "windows")]
                {
                    let mut write_buf = [0u8; CONFIG_PACKET_SIZE + 1];
                    write_buf[0] = 0x00; // report ID 0
                    write_buf[1..].copy_from_slice(&packet_bytes);
                    device
                        .write(&write_buf)
                        .map_err(|e| format!("Failed to write packet: {}", e))?;
                }

                #[cfg(not(target_os = "windows"))]
                {
                    device
                        .write(&packet_bytes)
                        .map_err(|e| format!("Failed to write packet: {}", e))?;
                }
            }

            // Per-attempt response wait with a reasonable deadline
            let attempt_result = timeout(Duration::from_millis(800), async {
                let mut read_attempts = 0;
                let max_read_attempts = 8; // 8 * 100ms = 800ms max
                
                loop {
                    read_attempts += 1;
                    if read_attempts > max_read_attempts {
                        return Err("Too many read attempts without valid response".to_string());
                    }
                    
                    // On Windows, reads include the report ID byte (total 65 bytes)
                    #[cfg(target_os = "windows")]
                    let mut buffer65 = [0u8; CONFIG_PACKET_SIZE + 1];

                    #[cfg(not(target_os = "windows"))]
                    let mut buffer64 = [0u8; CONFIG_PACKET_SIZE];

                    // Read response - do this in a separate scope to release the lock
                    let read_result = {
                        let device_guard = self.device.lock().unwrap();
                        let device = device_guard.as_ref()
                            .ok_or("Device disconnected during operation")?;

                        #[cfg(target_os = "windows")]
                        {
                            device.read_timeout(&mut buffer65, 100)
                        }

                        #[cfg(not(target_os = "windows"))]
                        {
                            device.read_timeout(&mut buffer64, 100)
                        }
                    };

                    match read_result {
                        // Windows: 65 bytes -> drop report ID; 64 bytes -> use as-is
                        #[cfg(target_os = "windows")]
                        Ok(bytes_read) if bytes_read == CONFIG_PACKET_SIZE + 1 || bytes_read == CONFIG_PACKET_SIZE => {
                            let data: &[u8] = if bytes_read == CONFIG_PACKET_SIZE + 1 { &buffer65[1..] } else { &buffer65[..CONFIG_PACKET_SIZE] };
                            println!(
                                "Received bytes (win): {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X}",
                                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7]
                            );
                            match ConfigPacket::from_bytes(data) {
                                Ok(response) if response.sequence == sequence => {
                                    return Ok(response);
                                }
                                Ok(response) => {
                                    println!("Wrong sequence received: got {}, expected {}", response.sequence, sequence);
                                    continue; // Wrong sequence, keep waiting
                                }
                                Err(e) => return Err(format!("Invalid response packet: {}", e)),
                            }
                        }

                        // Non-Windows: 64 bytes as-is
                        #[cfg(not(target_os = "windows"))]
                        Ok(bytes_read) if bytes_read == CONFIG_PACKET_SIZE => {
                            println!(
                                "Received bytes: {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X}",
                                buffer64[0], buffer64[1], buffer64[2], buffer64[3], buffer64[4], buffer64[5], buffer64[6], buffer64[7]
                            );
                            match ConfigPacket::from_bytes(&buffer64) {
                                Ok(response) if response.sequence == sequence => {
                                    return Ok(response);
                                }
                                Ok(response) => {
                                    println!("Wrong sequence received: got {}, expected {}", response.sequence, sequence);
                                    continue; // Wrong sequence, keep waiting
                                }
                                Err(e) => return Err(format!("Invalid response packet: {}", e)),
                            }
                        }
                        Ok(_) => {
                            println!("Incomplete read, attempt {}/{}", read_attempts, max_read_attempts);
                            continue; // Incomplete read, keep trying
                        }
                        Err(hidapi::HidError::HidApiError { message }) if message.contains("timeout") => {
                            println!("Read timeout, attempt {}/{}", read_attempts, max_read_attempts);
                            tokio::time::sleep(Duration::from_millis(5)).await;
                            continue;
                        }
                        Err(e) => return Err(format!("Failed to read response: {}", e)),
                    }
                }
            })
        .await;

            match attempt_result {
                Ok(Ok(resp)) => break resp,
                Ok(Err(e)) => return Err(e),
                Err(_elapsed) => {
                    println!("send_command timeout: cmd={:?} seq={} after 800ms", command, sequence);
                    if attempt < MAX_RETRIES {
                        // quick retry with same sequence
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        continue;
                    } else {
                        return Err("Command timeout".to_string());
                    }
                }
            }
        };

        Ok(response)
    }

    /// Get device information
    pub async fn get_device_info(&self) -> Result<DeviceInfo, String> {
        // Handle mock device
        if *self.is_mock_device.lock().unwrap() {
            return Ok(DeviceInfo {
                device_name: "Mock OpenGrader".to_string(),
                protocol_version: 1,
                firmware_version_major: 1,
                firmware_version_minor: 0,
                firmware_version_patch: 0,
                device_type: 1, // Master
                matrix_rows: 4,
                matrix_cols: 4,
                encoder_count: 2,
                i2c_devices: 0,
            });
        }
        
        let response = self.send_command(ConfigCommand::GetInfo, &[]).await?;
        
        println!("DEBUG get_device_info: status_byte=0x{:02X}, sequence={}, payload_length={}", 
                 response.status, response.sequence, response.payload_length);
        
        let status = StatusCode::from(response.status);
        if !matches!(status, StatusCode::Ok) {
            println!("DEBUG get_device_info: status check failed, got {:?} instead of Ok", status);
            return Err(format!("Device returned error: {:?}", status));
        }

        DeviceInfo::from_payload(&response.payload[..response.payload_length as usize])
    }

    /// Get keymap entry for specific row/col
    pub async fn get_keymap_entry(&self, row: u8, col: u8) -> Result<KeymapEntry, String> {
        // Handle mock device
        if *self.is_mock_device.lock().unwrap() {
            // Return mock keymap data - simple layout with letters
            let keycode = match (row, col) {
                (0, 0) => 0x04, // KC_A
                (0, 1) => 0x05, // KC_B
                (0, 2) => 0x06, // KC_C  
                (0, 3) => 0x07, // KC_D
                (1, 0) => 0x08, // KC_E
                (1, 1) => 0x09, // KC_F
                (1, 2) => 0x0A, // KC_G
                (1, 3) => 0x0B, // KC_H
                (2, 0) => 0x0C, // KC_I
                (2, 1) => 0x0D, // KC_J
                (2, 2) => 0x0E, // KC_K
                (2, 3) => 0x0F, // KC_L
                (3, 0) => 0x10, // KC_M
                (3, 1) => 0x11, // KC_N
                (3, 2) => 0x12, // KC_O
                (3, 3) => 0x13, // KC_P
                _ => 0x00, // KC_NO
            };
            
            return Ok(KeymapEntry {
                row,
                col,
                keycode,
            });
        }
        
        let payload = [row, col];
        let response = self.send_command(ConfigCommand::GetKeymap, &payload).await?;
        
        println!("DEBUG get_keymap_entry: status_byte=0x{:02X}, sequence={}, payload_length={}", 
                 response.status, response.sequence, response.payload_length);
        
        let status = StatusCode::from(response.status);
        if !matches!(status, StatusCode::Ok) {
            println!("DEBUG get_keymap_entry: status check failed, got {:?} instead of Ok", status);
            return Err(format!("Device returned error: {:?}", status));
        }

        KeymapEntry::from_payload(&response.payload[..response.payload_length as usize])
    }

    /// Set keymap entry for specific row/col
    pub async fn set_keymap_entry(&self, entry: &KeymapEntry) -> Result<(), String> {
        // Handle mock device
        if *self.is_mock_device.lock().unwrap() {
            // For mock device, just pretend to set the keymap
            return Ok(());
        }
        
        let payload = entry.to_payload();
        let response = self.send_command(ConfigCommand::SetKeymap, &payload).await?;
        
        let status = StatusCode::from(response.status);
        if !matches!(status, StatusCode::Ok) {
            return Err(format!("Device returned error: {:?}", status));
        }

        Ok(())
    }

    /// Get encoder mapping
    pub async fn get_encoder_entry(&self, encoder_id: u8) -> Result<EncoderEntry, String> {
        // Handle mock device
        if *self.is_mock_device.lock().unwrap() {
            let (ccw_keycode, cw_keycode) = match encoder_id {
                0 => (0x52, 0x51), // Up/Down arrows for encoder 0
                1 => (0x50, 0x4F), // Left/Right arrows for encoder 1
                _ => (0x00, 0x00), // KC_NO
            };
            
            return Ok(EncoderEntry {
                encoder_id,
                ccw_keycode,
                cw_keycode,
                reserved: 0,
            });
        }
        
        let payload = [encoder_id];
        let response = self.send_command(ConfigCommand::GetEncoderMap, &payload).await?;
        
        println!("DEBUG get_encoder_entry: status_byte=0x{:02X}, sequence={}, payload_length={}", 
                 response.status, response.sequence, response.payload_length);
        
        let status = StatusCode::from(response.status);
        if !matches!(status, StatusCode::Ok) {
            println!("DEBUG get_encoder_entry: status check failed, got {:?} instead of Ok", status);
            return Err(format!("Device returned error: {:?}", status));
        }

        EncoderEntry::from_payload(&response.payload[..response.payload_length as usize])
    }

    /// Set encoder mapping
    pub async fn set_encoder_entry(&self, entry: &EncoderEntry) -> Result<(), String> {
        // Handle mock device
        if *self.is_mock_device.lock().unwrap() {
            // For mock device, just pretend to set the encoder
            return Ok(());
        }
        
        let payload = entry.to_payload();
        let response = self.send_command(ConfigCommand::SetEncoderMap, &payload).await?;
        
        let status = StatusCode::from(response.status);
        if !matches!(status, StatusCode::Ok) {
            return Err(format!("Device returned error: {:?}", status));
        }

        Ok(())
    }

    /// Save configuration to EEPROM
    pub async fn save_config(&self) -> Result<(), String> {
        let response = self.send_command(ConfigCommand::SaveConfig, &[]).await?;
        
        let status = StatusCode::from(response.status);
        match status {
            StatusCode::Ok => Ok(()),
            StatusCode::NotSupported => Err("Save config not yet implemented in firmware".to_string()),
            _ => Err(format!("Device returned error: {:?}", status)),
        }
    }

    /// Load configuration from EEPROM
    pub async fn load_config(&self) -> Result<(), String> {
        let response = self.send_command(ConfigCommand::LoadConfig, &[]).await?;
        
        let status = StatusCode::from(response.status);
        match status {
            StatusCode::Ok => Ok(()),
            StatusCode::NotSupported => Err("Load config not yet implemented in firmware".to_string()),
            _ => Err(format!("Device returned error: {:?}", status)),
        }
    }

    /// Reset configuration to defaults
    pub async fn reset_config(&self) -> Result<(), String> {
        let response = self.send_command(ConfigCommand::ResetConfig, &[]).await?;
        
        let status = StatusCode::from(response.status);
        match status {
            StatusCode::Ok => Ok(()),
            StatusCode::NotSupported => Err("Reset config not yet implemented in firmware".to_string()),
            _ => Err(format!("Device returned error: {:?}", status)),
        }
    }

    /// Get I2C devices
    pub async fn get_i2c_devices(&self) -> Result<Vec<I2CDeviceInfo>, String> {
        let response = self.send_command(ConfigCommand::GetI2CDevices, &[]).await?;
        
        let status = StatusCode::from(response.status);
        if !matches!(status, StatusCode::Ok) {
            return Err(format!("Device returned error: {:?}", status));
        }

        // Parse multiple I2C devices from payload (not yet implemented in firmware)
        Ok(vec![]) // Placeholder
    }

    /// Reboot device
    pub async fn reboot_device(&self) -> Result<(), String> {
        let response = self.send_command(ConfigCommand::Reboot, &[]).await?;
        
        let status = StatusCode::from(response.status);
        if !matches!(status, StatusCode::Ok) {
            return Err(format!("Device returned error: {:?}", status));
        }

        // Device will disconnect after reboot
        self.disconnect();
        Ok(())
    }
}

impl Default for HidManager {
    fn default() -> Self {
        Self::new().expect("Failed to create HidManager")
    }
}