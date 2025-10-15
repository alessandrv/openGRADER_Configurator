use serde::{Deserialize, Serialize};

/// Configuration Protocol Version
pub const CONFIG_PROTOCOL_VERSION: u8 = 1;

/// Packet header and sizes
pub const CONFIG_PACKET_HEADER: u16 = 0x4F47; // "OG" - will send as [0x47, 0x4F] in little-endian
pub const CONFIG_MAX_PAYLOAD_SIZE: usize = 56; // 64 - 8 (header + command + status + sequence + payload_length + reserved bytes)
pub const CONFIG_PACKET_SIZE: usize = 64;

/// Command types
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ConfigCommand {
    GetInfo = 0x01,
    GetKeymap = 0x02,
    SetKeymap = 0x03,
    GetEncoderMap = 0x04,
    SetEncoderMap = 0x05,
    SaveConfig = 0x06,
    LoadConfig = 0x07,
    ResetConfig = 0x08,
    GetI2CDevices = 0x09,
    SetI2CConfig = 0x0A,
    GetDeviceStatus = 0x0B,
    Reboot = 0x0C,
    MidiSendRaw = 0x0D,
    MidiNoteOn = 0x0E,
    MidiNoteOff = 0x0F,
    MidiControlChange = 0x10,
    // Slave device commands
    GetSlaveKeymap = 0x11,
    SetSlaveKeymap = 0x12,
    GetSlaveInfo = 0x13,
    GetSlaveEncoder = 0x14,
    SetSlaveEncoder = 0x15,
    GetLayoutInfo = 0x16,
    SetLayerState = 0x17,
    GetLayerState = 0x18,
}
                         
impl From<u8> for ConfigCommand {
    fn from(value: u8) -> Self {
        match value {
            0x01 => ConfigCommand::GetInfo,
            0x02 => ConfigCommand::GetKeymap,
            0x03 => ConfigCommand::SetKeymap,
            0x04 => ConfigCommand::GetEncoderMap,
            0x05 => ConfigCommand::SetEncoderMap,
            0x06 => ConfigCommand::SaveConfig,
            0x07 => ConfigCommand::LoadConfig,
            0x08 => ConfigCommand::ResetConfig,
            0x09 => ConfigCommand::GetI2CDevices,
            0x0A => ConfigCommand::SetI2CConfig,
            0x0B => ConfigCommand::GetDeviceStatus,
            0x0C => ConfigCommand::Reboot,
            0x0D => ConfigCommand::MidiSendRaw,
            0x0E => ConfigCommand::MidiNoteOn,
            0x0F => ConfigCommand::MidiNoteOff,
            0x10 => ConfigCommand::MidiControlChange,
            0x11 => ConfigCommand::GetSlaveKeymap,
            0x12 => ConfigCommand::SetSlaveKeymap,
            0x13 => ConfigCommand::GetSlaveInfo,
            0x14 => ConfigCommand::GetSlaveEncoder,
            0x15 => ConfigCommand::SetSlaveEncoder,
            0x16 => ConfigCommand::GetLayoutInfo,
            0x17 => ConfigCommand::SetLayerState,
            0x18 => ConfigCommand::GetLayerState,
            _ => ConfigCommand::GetInfo, // Default fallback
        }
    }
}

/// Status codes
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum StatusCode {
    Ok = 0x00,
    Error = 0x01,          // STATUS_ERROR - generic error
    InvalidCmd = 0x02,     // STATUS_INVALID_CMD - command not recognized
    InvalidParam = 0x03,   // STATUS_INVALID_PARAM - invalid parameter
    Busy = 0x04,           // STATUS_BUSY - device is busy
    NotSupported = 0x05,   // STATUS_NOT_SUPPORTED - feature not supported
}

impl From<u8> for StatusCode {
    fn from(value: u8) -> Self {
        match value {
            0x00 => StatusCode::Ok,
            0x01 => StatusCode::Error,
            0x02 => StatusCode::InvalidCmd,
            0x03 => StatusCode::InvalidParam,
            0x04 => StatusCode::Busy,
            0x05 => StatusCode::NotSupported,
            _ => StatusCode::Error, // Default fallback
        }
    }
}

/// Configuration packet structure (matches firmware)
#[repr(C, packed)]
#[derive(Debug, Clone)]
pub struct ConfigPacket {
    pub header: u16,
    pub command: u8,
    pub status: u8,
    pub sequence: u8,
    pub payload_length: u8,
    pub reserved: [u8; 2],
    pub payload: [u8; CONFIG_MAX_PAYLOAD_SIZE],
}

impl ConfigPacket {
    pub fn new(command: ConfigCommand, sequence: u8, payload: &[u8]) -> Self {
        let mut packet = ConfigPacket {
            header: CONFIG_PACKET_HEADER,
            command: command as u8,
            sequence,
            status: StatusCode::Ok as u8,
            payload_length: payload.len().min(CONFIG_MAX_PAYLOAD_SIZE) as u8,
            reserved: [0; 2],
            payload: [0; CONFIG_MAX_PAYLOAD_SIZE],
        };
        
        let len = payload.len().min(CONFIG_MAX_PAYLOAD_SIZE);
        packet.payload[..len].copy_from_slice(&payload[..len]);
        
        packet
    }

    pub fn to_bytes(&self) -> [u8; CONFIG_PACKET_SIZE] {
        // Since we have a packed C struct, we can safely transmute it to bytes
        // This ensures the exact same memory layout as the C struct
        unsafe {
            let ptr = self as *const ConfigPacket as *const u8;
            let mut bytes = [0u8; CONFIG_PACKET_SIZE];
            std::ptr::copy_nonoverlapping(ptr, bytes.as_mut_ptr(), CONFIG_PACKET_SIZE);
            bytes
        }
    }
    
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < CONFIG_PACKET_SIZE {
            return Err("Packet too short".to_string());
        }

        let header = u16::from_le_bytes([bytes[0], bytes[1]]);
        if header != CONFIG_PACKET_HEADER {
            return Err(format!("Invalid header: 0x{:04X}", header));
        }

        let mut packet = ConfigPacket {
            header,
            command: bytes[2],
            sequence: bytes[4],      // sequence at position 4 in firmware
            status: bytes[3],        // status at position 3 in firmware
            payload_length: bytes[5],
            reserved: [bytes[6], bytes[7]],
            payload: [0; CONFIG_MAX_PAYLOAD_SIZE],
        };

        packet.payload.copy_from_slice(&bytes[8..8 + CONFIG_MAX_PAYLOAD_SIZE]);

        Ok(packet)
    }
}

/// Device information structure (matches firmware)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub protocol_version: u8,
    pub firmware_version_major: u8,
    pub firmware_version_minor: u8,
    pub firmware_version_patch: u8,
    pub device_type: u8, // 0=Slave, 1=Master
    pub matrix_rows: u8,
    pub matrix_cols: u8,
    pub encoder_count: u8,
    pub layer_count: u8,
    pub i2c_devices: u8,
    pub device_name: String,
}

impl DeviceInfo {
    pub fn from_payload(payload: &[u8]) -> Result<Self, String> {
        if payload.len() < 56 {
            return Err("Device info payload too short".to_string());
        }

        // Extract device name (null-terminated string)
        let name_bytes = &payload[10..42];
        let name_end = name_bytes.iter().position(|&b| b == 0).unwrap_or(32);
        let device_name = String::from_utf8_lossy(&name_bytes[..name_end]).to_string();

        Ok(DeviceInfo {
            protocol_version: payload[0],
            firmware_version_major: payload[1],
            firmware_version_minor: payload[2],
            firmware_version_patch: payload[3],
            device_type: payload[4],
            matrix_rows: payload[5],
            matrix_cols: payload[6],
            encoder_count: payload[7],
            layer_count: payload[8],
            i2c_devices: payload[9],
            device_name,
        })
    }
}

/// Keymap entry structure (matches firmware)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeymapEntry {
    pub layer: u8,
    pub row: u8,
    pub col: u8,
    pub keycode: u16,
}

/// Slave keymap entry structure (matches firmware)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaveKeymapEntry {
    pub slave_addr: u8,
    pub layer: u8,
    pub row: u8,
    pub col: u8,
    pub keycode: u16,
}

/// Slave encoder entry structure (matches firmware)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaveEncoderEntry {
    pub slave_addr: u8,
    pub layer: u8,
    pub encoder_id: u8,
    pub ccw_keycode: u16,
    pub cw_keycode: u16,
    pub reserved: u8,
}

impl KeymapEntry {
    pub fn from_payload(payload: &[u8]) -> Result<Self, String> {
        if payload.len() < 5 {
            return Err("Keymap entry payload too short".to_string());
        }

        Ok(KeymapEntry {
            layer: payload[0],
            row: payload[1],
            col: payload[2],
            keycode: u16::from_le_bytes([payload[3], payload[4]]),
        })
    }
}

impl SlaveKeymapEntry {
    pub fn from_payload(slave_addr: u8, payload: &[u8]) -> Result<Self, String> {
        if payload.len() < 5 {
            return Err("Slave keymap entry payload too short".to_string());
        }

        Ok(SlaveKeymapEntry {
            slave_addr,
            layer: payload[0],
            row: payload[1],
            col: payload[2],
            keycode: u16::from_le_bytes([payload[3], payload[4]]),
        })
    }
    
    pub fn to_payload(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.push(self.slave_addr);
        payload.push(self.layer);
        payload.push(self.row);
        payload.push(self.col);
        payload.extend_from_slice(&self.keycode.to_le_bytes());
        payload
    }
}

impl KeymapEntry {
    pub fn to_payload(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.push(self.layer);
        payload.push(self.row);
        payload.push(self.col);
        payload.extend_from_slice(&self.keycode.to_le_bytes());
        payload
    }
}

/// Encoder entry structure (matches firmware)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncoderEntry {
    pub layer: u8,
    pub encoder_id: u8,
    pub ccw_keycode: u16,
    pub cw_keycode: u16,
    pub reserved: u8,
}

impl EncoderEntry {
    pub fn from_payload(payload: &[u8]) -> Result<Self, String> {
        if payload.len() < 7 {
            return Err("Encoder entry payload too short".to_string());
        }

        Ok(EncoderEntry {
            layer: payload[0],
            encoder_id: payload[1],
            ccw_keycode: u16::from_le_bytes([payload[2], payload[3]]),
            cw_keycode: u16::from_le_bytes([payload[4], payload[5]]),
            reserved: payload[6],
        })
    }

    pub fn to_payload(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.push(self.layer);
        payload.push(self.encoder_id);
        payload.extend_from_slice(&self.ccw_keycode.to_le_bytes());
        payload.extend_from_slice(&self.cw_keycode.to_le_bytes());
        payload.push(self.reserved);
        payload
    }
}

/// I2C device info structure (matches firmware)  
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct I2CDeviceInfo {
    pub address: u8,
    pub device_type: u8,
    pub status: u8,
    pub firmware_version_major: u8,
    pub firmware_version_minor: u8,
    pub firmware_version_patch: u8,
    pub name: String,
}

impl I2CDeviceInfo {
    pub fn from_payload_at_index(payload: &[u8], index: usize) -> Result<Self, String> {
        // Each I2C device entry is 28 bytes (matches firmware i2c_device_info_t)
        // Firmware structure: address(1) + device_type(1) + status(1) + fw_ver(3) + name(22) = 28 bytes
        let offset = index * 28;
        if payload.len() < offset + 28 {
            return Err("I2C device info payload too short".to_string());
        }

        // Extract device name (null-terminated string, 22 bytes in firmware)
        let name_bytes = &payload[offset + 6..offset + 28]; // name starts at offset 6 (after address, device_type, status, and 3 fw_ver bytes)
        let name_end = name_bytes.iter().position(|&b| b == 0).unwrap_or(22);
        let name = String::from_utf8_lossy(&name_bytes[..name_end]).to_string();

        Ok(I2CDeviceInfo {
            address: payload[offset],
            device_type: payload[offset + 1],
            status: payload[offset + 2], // status is at offset 2 (after address and device_type)
            firmware_version_major: payload[offset + 3],
            firmware_version_minor: payload[offset + 4],
            firmware_version_patch: payload[offset + 5],
            name,
        })
    }
    
    pub fn from_payload(payload: &[u8]) -> Result<Self, String> {
        if payload.len() < 28 {
            return Err("I2C device info payload too short".to_string());
        }

        // Extract device name (null-terminated string)
        let name_bytes = &payload[6..28];
        let name_end = name_bytes.iter().position(|&b| b == 0).unwrap_or(22);
        let name = String::from_utf8_lossy(&name_bytes[..name_end]).to_string();

        Ok(I2CDeviceInfo {
            address: payload[0],
            device_type: payload[1],
            status: payload[2],
            firmware_version_major: payload[3],
            firmware_version_minor: payload[4],
            firmware_version_patch: payload[5],
            name,
        })
    }
}

impl SlaveEncoderEntry {
    pub fn from_payload(slave_addr: u8, payload: &[u8]) -> Result<Self, String> {
        if payload.len() < 7 {
            return Err("Slave encoder entry payload too short".to_string());
        }

        Ok(SlaveEncoderEntry {
            slave_addr,
            layer: payload[0],
            encoder_id: payload[1],
            ccw_keycode: u16::from_le_bytes([payload[2], payload[3]]),
            cw_keycode: u16::from_le_bytes([payload[4], payload[5]]),
            reserved: payload[6],
        })
    }

    pub fn to_payload(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.push(self.slave_addr);
        payload.push(self.layer);
        payload.push(self.encoder_id);
        payload.extend_from_slice(&self.ccw_keycode.to_le_bytes());
        payload.extend_from_slice(&self.cw_keycode.to_le_bytes());
        payload.push(self.reserved);
        payload
    }
}

/// Layer state payload (active layer mask + default layer index)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerState {
    pub active_mask: u8,
    pub default_layer: u8,
}

impl LayerState {
    pub fn from_payload(payload: &[u8]) -> Result<Self, String> {
        if payload.len() < 2 {
            return Err("Layer state payload too short".to_string());
        }

        Ok(LayerState {
            active_mask: payload[0],
            default_layer: payload[1],
        })
    }

    pub fn to_payload(&self) -> [u8; 2] {
        [self.active_mask, self.default_layer]
    }
}

/// Board layout metadata structure (matches firmware board_layout_info_t)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardLayoutInfo {
    pub version: u8,
    pub matrix_rows: u8,
    pub matrix_cols: u8,
    pub encoder_count: u8,
    pub first_encoder_column: u8,
    pub encoders_per_row: u8,
    pub bitmap_length: u8,
    pub encoder_bitmap: Vec<u8>,
}

impl BoardLayoutInfo {
    pub fn from_payload(payload: &[u8]) -> Result<Self, String> {
        if payload.len() < 7 {
            return Err("Board layout payload too short".to_string());
        }

        let bitmap_length = payload[6] as usize;
        let expected_len = 8 + bitmap_length;
        if payload.len() < expected_len {
            return Err(format!(
                "Board layout payload truncated: expected {} bytes, got {}",
                expected_len,
                payload.len()
            ));
        }

        let mut bitmap = Vec::with_capacity(bitmap_length);
        bitmap.extend_from_slice(&payload[8..8 + bitmap_length]);

        Ok(BoardLayoutInfo {
            version: payload[0],
            matrix_rows: payload[1],
            matrix_cols: payload[2],
            encoder_count: payload[3],
            first_encoder_column: payload[4],
            encoders_per_row: payload[5],
            bitmap_length: payload[6],
            encoder_bitmap: bitmap,
        })
    }

    pub fn cell_index(&self, row: u8, col: u8) -> Option<usize> {
        if row >= self.matrix_rows || col >= self.matrix_cols {
            return None;
        }
        Some((row as usize) * self.matrix_cols as usize + col as usize)
    }

    pub fn is_encoder_cell(&self, row: u8, col: u8) -> bool {
        if let Some(index) = self.cell_index(row, col) {
            let byte_index = index / 8;
            let bit_index = index % 8;
            if byte_index >= self.encoder_bitmap.len() {
                return false;
            }
            (self.encoder_bitmap[byte_index] & (1 << bit_index)) != 0
        } else {
            false
        }
    }

    pub fn encoder_id_for_cell(&self, row: u8, col: u8) -> Option<u8> {
        if !self.is_encoder_cell(row, col) {
            return None;
        }

        if col < self.first_encoder_column {
            return None;
        }

        let col_offset = col - self.first_encoder_column;
        let per_row = self.encoders_per_row;
        if per_row == 0 {
            return None;
        }

        let id = (row as u16) * (per_row as u16) + (col_offset as u16);
        if id < self.encoder_count as u16 {
            Some(id as u8)
        } else {
            None
        }
    }
}