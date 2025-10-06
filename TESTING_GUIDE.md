# OpenGrader Configurator Testing Guide

## Device Types

Your firmware supports two modes:

1. **Master Mode** (`device_type = 1`)
   - Connected to USB host (computer)
   - Can query and configure connected I2C slave devices
   - Shows main device keymap + all slave keymaps
   - Has encoders

2. **Slave Mode** (`device_type = 0`) 
   - Not connected to USB host OR `FORCE_SLAVE_MODE = 1`
   - Communicates with master via I2C
   - Cannot query other slaves
   - Only shows its own keymap and encoders

## Current Test Setup

Looking at your `main.c`:
```c
#ifndef FORCE_SLAVE_MODE
#define FORCE_SLAVE_MODE 1  // ← Currently forcing slave mode
#endif
```

**You are currently testing with a SLAVE device**, which means:
- ✅ The device responds to configuration commands
- ✅ You can view/edit the slave's own keymap
- ✅ You can view/edit the slave's own encoders
- ❌ It won't detect any I2C slave devices (because it IS a slave)
- ❌ The device selector won't show any slaves

## Testing Scenarios

### Scenario 1: Single Slave Device (Current Setup)
**Firmware:**
```c
#define FORCE_SLAVE_MODE 1
```

**Expected Behavior:**
- Configurator connects successfully
- Shows device as "Slave" mode
- Displays the slave's keymap (e.g., 4x4 matrix)
- Displays the slave's encoders (if any)
- Device selector only shows "Main Device"
- No slave devices listed

**Test:**
1. Build and flash firmware with `FORCE_SLAVE_MODE = 1`
2. Connect via USB to configurator
3. Should see the slave's keymap and encoders
4. Can edit keys and encoders
5. Can save configuration to EEPROM

### Scenario 2: Master Device with No Slaves
**Firmware:**
```c
#define FORCE_SLAVE_MODE 0
```

**Expected Behavior:**
- Configurator connects successfully
- Shows device as "Master" mode
- Displays the master's keymap
- Displays the master's encoders
- Scans I2C bus but finds no slaves
- Device selector only shows "Main Device"

**Test:**
1. Build and flash firmware with `FORCE_SLAVE_MODE = 0`
2. Connect via USB to configurator
3. Should see the master's keymap and encoders
4. Device selector shows only main device
5. Can edit and save configuration

### Scenario 3: Master + Slave System (Full Test)
**Setup:**
- Device A: Master (`FORCE_SLAVE_MODE = 0`, connected to USB)
- Device B: Slave (`FORCE_SLAVE_MODE = 1`, I2C address 0x42)
- Connect Device B's I2C pins (SDA/SCL) to Device A

**Expected Behavior:**
- Configurator connects to Master
- Master scans and finds Slave at 0x42
- Device selector shows:
  - "Main Device (Master)"
  - "Slave: Slave 42"
- Can view/edit Master's keymap and encoders
- Can view/edit Slave's keymap and encoders
- Changes saved to respective devices

**Test:**
1. Flash Device A as master
2. Flash Device B as slave  
3. Wire I2C: A.SDA ↔ B.SDA, A.SCL ↔ B.SCL, GND ↔ GND
4. Connect Device A to computer
5. Open configurator
6. Should see both devices in selector
7. Test editing keys on both devices

## Encoder Support

Encoders work the same way for both master and slave:

- **Master Mode:** Encoders send HID events directly
- **Slave Mode:** Encoders send events to master via I2C

### Configuring Encoders:
1. Go to "Encoders" tab
2. Click CCW or CW button for encoder
3. Select keycode (standard key or MIDI)
4. Apply changes
5. Save to EEPROM

### Encoder Commands:
- `CMD_GET_ENCODER_MAP` - Get encoder configuration
- `CMD_SET_ENCODER_MAP` - Set encoder configuration
- Both work for master's local encoders
- Slave encoders are configured when connected to slave via I2C

## Common Issues

### "No keymap available"
- Check that device is connected
- Verify keymap data is loaded (check console logs)
- For slave: Make sure it's responding to commands

### "No slaves detected" (on Master)
- Check I2C wiring (SDA, SCL, GND)
- Verify slave device is powered and running
- Check slave I2C address (default: 0x42)
- Look at CDC debug logs for "Found slave at 0xXX"

### "Incomplete read" errors
- ✅ Fixed in latest version
- If still occurring, check USB connection
- Try reconnecting the device

## Debug Logging

### Firmware CDC Output:
```
I2C: i2c_manager_scan_slaves called
I2C: Starting bus scan...
I2C: Found slave at 0x42
I2C: Scan complete - found 1 slaves
I2C: handle_get_i2c_devices called
I2C: Detected 1 slaves
I2C: Adding device 0: addr=0x42, name=Slave 42
```

### Configurator Console:
```
DEBUG: Device count from firmware: 1
DEBUG: Parsed device 0: addr=0x42, name=Slave 42
I2C devices loaded: [{address: 66, name: "Slave 42", ...}]
Loaded keymap for slave device 66
```

## Next Steps

1. **Test Current Slave Setup:**
   - Verify you can see and edit the slave's keymap ✓
   - Verify you can see and edit the slave's encoders ✓
   - Test saving configuration ✓

2. **Test Master Mode:**
   - Change `FORCE_SLAVE_MODE` to 0
   - Rebuild and flash
   - Verify master behavior

3. **Test Master+Slave:**
   - Flash two devices (one master, one slave)
   - Wire I2C connections
   - Test full system

## Code Locations

- Firmware I2C: `Core/Src/i2c_manager.c`
- Firmware Protocol: `Core/Src/config_protocol.c`
- Configurator Protocol: `src-tauri/src/protocol.rs`
- Configurator HID: `src-tauri/src/hid_manager.rs`
- Frontend UI: `src/routes/+page.svelte`
