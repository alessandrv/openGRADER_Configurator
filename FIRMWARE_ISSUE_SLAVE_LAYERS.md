# Firmware Issue: Slave Layer Data Not Being Read Correctly

## Problem
Slave devices are only returning data for layer 0. When requesting layers 1-7, the firmware returns all zeros.

## Current Behavior
- ✅ Backend correctly iterates through all layers (0-7)
- ✅ Backend sends correct layer parameter in USB payload: `[slave_addr, layer, row, col]`
- ❌ Firmware ignores the layer parameter and always returns layer 0 data
- ❌ All slave layers except layer 0 show as empty in the UI

## Root Cause
The firmware's handler for `ConfigCommand::GetSlaveKeymap` is likely:
1. Not reading the `layer` parameter from the payload, OR
2. Not using the `layer` parameter when reading from the slave's storage, OR
3. Not forwarding the `layer` parameter correctly in the I2C command to the slave

## What Needs to be Fixed in Firmware

### Location: `openGRADER_FW` firmware project

The firmware needs to properly handle the layer parameter in the slave keymap read operation:

```c
// Current (broken) - pseudocode:
case CONFIG_CMD_GET_SLAVE_KEYMAP:
    uint8_t slave_addr = payload[0];
    uint8_t layer = payload[1];      // ← Layer IS received but NOT used!
    uint8_t row = payload[2];
    uint8_t col = payload[3];
    
    // Problem: Always reads from layer 0
    uint16_t keycode = read_slave_keymap(slave_addr, 0, row, col);  // ← Hardcoded 0!
    
    // OR the I2C command doesn't include layer:
    i2c_request_keymap(slave_addr, row, col);  // ← Missing layer parameter!
    
    return keycode;
```

### Required Fix:
```c
// Fixed version:
case CONFIG_CMD_GET_SLAVE_KEYMAP:
    uint8_t slave_addr = payload[0];
    uint8_t layer = payload[1];
    uint8_t row = payload[2];
    uint8_t col = payload[3];
    
    // Use the layer parameter!
    uint16_t keycode = read_slave_keymap(slave_addr, layer, row, col);
    
    // Or if using I2C:
    i2c_request_keymap(slave_addr, layer, row, col);
    
    return keycode;
```

## Testing
Once fixed, test by:
1. Connect master and slave via configurator
2. Select slave device in UI
3. Switch to layer 1 (or any layer > 0)
4. Console should show: `Layer 1: X rows, Y non-zero keys` (where Y > 0)
5. Keys should be visible in the UI

## Related Files

### Backend (Already Correct)
- `src-tauri/src/commands.rs:654` - `get_full_slave_keymap()` - correctly iterates layers
- `src-tauri/src/hid_manager.rs:582` - `get_slave_keymap_entry()` - correctly sends layer in payload

### Frontend (Already Correct)
- `src/routes/+page.svelte:712` - `loadSlaveKeymap()` - receives all layers
- `src/routes/+page.svelte:805` - `getCurrentKeymap()` - correctly accesses layer data

### Firmware (NEEDS FIX)
- `openGRADER_FW/Core/Src/usb_config.c` (or similar) - USB config command handler
- Look for `CONFIG_CMD_GET_SLAVE_KEYMAP` or similar case statement
- Ensure the layer parameter from payload[1] is passed to slave read function

## Expected Behavior After Fix
✅ `get_full_slave_keymap()` should return different data for different layers
✅ UI should show populated keys on all programmed layers
✅ Switching layers in UI should show different keys for each layer
✅ Master and slave layer states remain synchronized

## Status
🔴 **BLOCKED** - Waiting for firmware fix to properly read layer parameter from slaves
