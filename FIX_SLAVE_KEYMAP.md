# Fix: Slave Keymap Configuration Support

## Problem

When connecting to a master device that detected slaves via I2C, the configurator could see the slave devices in the dropdown, but couldn't load their keymaps. The keymap would appear empty.

## Root Cause

The slave firmware had **two different I2C modes** that were incompatible:

### 1. Event Reporting Mode (Working ✅)
- Used for slaves to send key press/release events to master
- Uses FIFO queue system
- Works via `i2c_manager_poll_slaves()`
- **Purpose:** Real-time key event streaming

### 2. Configuration Mode (Broken ❌)
- Used by master to query/set slave keymap configuration
- Master sends commands like `CMD_GET_KEYMAP` with row/col
- Slave should respond with the keycode
- **Problem:** Slave had NO handler for these commands!

The slave would:
- ✅ Receive the I2C data in `i2c_slave_rx_buffer`
- ❌ But never process it or respond
- ❌ Master would timeout waiting for response

## Solution

Added a **configuration command handler** in the slave's I2C callback:

### Changes Made

**File:** `Core/Src/i2c_manager.c`

1. **Added config response buffer:**
   ```c
   static uint8_t i2c_slave_config_response[4] = {0};
   static volatile uint8_t i2c_slave_has_config_response = 0;
   ```

2. **Updated `i2c_manager_slave_rx_complete_callback()`:**
   - Parses received commands (`CMD_GET_KEYMAP`, `CMD_SET_KEYMAP`)
   - Gets/sets keycode from local keymap
   - Prepares response in config buffer
   - Sets response-ready flag

3. **Updated `i2c_manager_addr_callback()`:**
   - Checks for pending config response before FIFO events
   - Sends config response with priority
   - Falls back to event FIFO for key events

4. **Added includes:**
   ```c
   #include "input/keymap.h"
   #include "config_protocol.h"
   ```

### Protocol Details

#### GET_KEYMAP Command
**Master → Slave:**
```
[CMD_GET_KEYMAP][row][col][padding]
```

**Slave → Master:**
```
[CMD_GET_KEYMAP][keycode_low][keycode_high][STATUS_OK]
```

#### SET_KEYMAP Command
**Master → Slave:**
```
[CMD_SET_KEYMAP][row][col][keycode_low][keycode_high][padding]
```

**Slave → Master:**
```
[CMD_SET_KEYMAP][STATUS_OK]
```

## Testing

### Before Fix
- ❌ Slave detected but keymap empty
- ❌ Console showed I2C timeouts
- ❌ Couldn't edit slave keys

### After Fix
- ✅ Slave detected with full keymap
- ✅ Can view each key's current assignment
- ✅ Can click keys to edit them
- ✅ Can save changes to slave EEPROM
- ✅ No I2C timeouts

### Test Steps

1. **Build and Flash:**
   ```bash
   # Build firmware
   cd openGRADER_FW
   cmake --build build --target clean
   cmake --build build
   
   # Flash master device
   STM32_Programmer_CLI -c port=usb1 -d build/Debug/TINYUSBTEST.elf -rst
   
   # Flash slave device (with FORCE_SLAVE_MODE=1)
   # Connect via STLINK and flash
   ```

2. **Wire Devices:**
   - Master SDA ↔ Slave SDA
   - Master SCL ↔ Slave SCL  
   - Master GND ↔ Slave GND

3. **Test Configuration:**
   - Connect master to computer via USB
   - Open configurator
   - Should see "Main Device" and "Slave: Slave 42" in dropdown
   - Select slave from dropdown
   - Should see slave's keymap (e.g., 4x4 matrix)
   - Click any key to edit
   - Change keycode and apply
   - Save to EEPROM

4. **Verify Logs:**

   **Firmware (CDC):**
   ```
   I2C Slave: Received keymap query [0,0], will respond with 0x0004
   I2C: Sending config response
   ```

   **Configurator (Console):**
   ```
   Loaded keymap for slave device 66
   Selected device changed to: 66
   ```

## Known Limitations

1. **Encoder Support:** 
   - Slave encoder configuration not yet implemented
   - Returns `STATUS_NOT_SUPPORTED` for `CMD_GET_ENCODER_MAP`
   - Can be added in future if slaves have encoders

2. **Device Info Query:**
   - `handle_get_slave_info()` returns hardcoded info
   - Assumes slave has same matrix size as master
   - Should be updated to query actual slave info

3. **Response Priority:**
   - Config responses take priority over event FIFO
   - If master queries config during heavy key activity, events may queue up
   - FIFO size is 16 events, should be sufficient

## Future Improvements

### 1. Full Device Info Query
Currently hardcoded:
```c
info->matrix_rows = MATRIX_ROWS; // Assumes same as master
info->matrix_cols = MATRIX_COLS;
```

Should query slave:
- Send `CMD_GET_INFO` via I2C
- Slave responds with actual matrix size
- Use real dimensions for keymap queries

### 2. Slave Encoder Support
Add commands:
- `CMD_GET_ENCODER_MAP` - Get encoder CCW/CW keycodes
- `CMD_SET_ENCODER_MAP` - Set encoder configuration
- Similar to keymap commands

### 3. Bulk Transfer Optimization
Current: One I2C transaction per key (slow for 4x4 = 16 transactions)

Possible improvement:
- `CMD_GET_FULL_KEYMAP` - Returns all keys in one packet
- Use larger buffer or chunked transfer
- Much faster for initial load

### 4. EEPROM Save on Slave
Currently:
- Master can set slave keycodes in RAM
- Changes lost on slave reboot

Should add:
- `CMD_SAVE_CONFIG` forwarded to slave
- Slave saves to its own EEPROM
- Persistent configuration

## Files Modified

- `Core/Src/i2c_manager.c` - Added config command handler
- `src/routes/+page.svelte` - Fixed device_type check for slave query

## Related Documentation

- `TESTING_GUIDE.md` - Comprehensive testing scenarios
- `Core/Inc/config_protocol.h` - Protocol command definitions
- `Core/Inc/i2c_protocol.h` - I2C message structures
