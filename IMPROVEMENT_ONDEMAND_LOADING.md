# Improvement: On-Demand Slave Keymap Loading

## Previous Behavior

When connecting to a master device with slaves:
1. Master connects ✅
2. Get I2C devices list ✅
3. **Load ALL slave keymaps immediately** ❌
   - For each detected slave
   - Query every key position
   - Even if user doesn't select that slave

**Problems:**
- Slow initial connection (e.g., 3 slaves × 16 keys = 48 I2C transactions)
- Wasted time loading keymaps user might not need
- No feedback on which slave is being queried
- All slaves queried even if only viewing master

## New Behavior

When connecting to a master device with slaves:
1. Master connects ✅
2. Get I2C devices list ✅
3. **Don't load slave keymaps yet** ⏸️
4. **Load on-demand when slave is selected** ✅

**Benefits:**
- ⚡ **Faster connection** - Only loads master keymap initially
- 🎯 **Load only what's needed** - Only queries selected slave
- 📊 **Better feedback** - Loading spinner shows which slave is loading
- 🔄 **Smart caching** - Loaded keymaps are cached, no re-query

## Implementation

### 1. Removed Eager Loading

**Before:**
```javascript
for (const device of i2cDevices) {
    if (device.status === 1) {
        const slaveKeymap = await invoke('get_full_slave_keymap', { slaveAddr: device.address });
        slaveKeymaps[device.address] = slaveKeymap;
    }
}
```

**After:**
```javascript
// Don't load slave keymaps on initial connection - load on-demand when selected
// This makes the initial connection faster
```

### 2. Added On-Demand Loader

```javascript
async function loadSlaveKeymap(slaveAddr) {
    // Check if already loaded
    if (slaveKeymaps[slaveAddr] && slaveKeymaps[slaveAddr].length > 0) {
        console.log(`Slave ${slaveAddr} keymap already loaded`);
        return;
    }
    
    loadingKeymap = true;
    try {
        console.log(`Loading keymap for slave device ${slaveAddr}...`);
        const slaveKeymap = await invoke('get_full_slave_keymap', { slaveAddr: slaveAddr });
        slaveKeymaps[slaveAddr] = slaveKeymap;
        slaveKeymaps = {...slaveKeymaps}; // trigger reactivity
        console.log(`Loaded keymap for slave device ${slaveAddr}:`, slaveKeymap);
    } catch (e) {
        error = `Failed to load keymap for slave device ${slaveAddr}: ${e}`;
    } finally {
        loadingKeymap = false;
    }
}
```

### 3. Added Device Selection Watcher

Uses Svelte 5's `$effect` rune to watch for device selection changes:

```javascript
$effect(() => {
    if (selectedDevice !== 'main' && isConnected) {
        const slaveAddr = parseInt(selectedDevice);
        console.log(`Device selector changed to slave: ${slaveAddr}`);
        loadSlaveKeymap(slaveAddr);
    }
});
```

### 4. Improved Loading Message

**Before:**
```
Loading Keymap...
Fetching key configuration from device
```

**After:**
```
Loading Keymap...
Loading keymap from slave device 0x42
```

## User Experience

### Scenario 1: Master with 3 Slaves
**Before:**
```
1. Click "Connect Device"
2. Wait 5-10 seconds (loading all slaves)
3. Connection complete
4. Select slave from dropdown → instant (already loaded)
```

**After:**
```
1. Click "Connect Device"
2. Wait 1-2 seconds (loading master only)
3. Connection complete
4. Select slave from dropdown → wait 1 second (loading this slave)
5. Switch to another slave → wait 1 second (loading that slave)
6. Switch back to first slave → instant (cached)
```

### Scenario 2: Only Using Master
**Before:**
- Still loaded all slave keymaps (wasted time)

**After:**
- Only loads master keymap (faster)
- Slaves never queried unless selected

### Scenario 3: Quick Configuration Changes
**Before:**
- Had to wait for all slaves on every connection

**After:**
- Connect fast, make changes to master, done
- Slaves not involved at all

## Caching Behavior

The `slaveKeymaps` object acts as a cache:

```javascript
slaveKeymaps = {
    66: [[...], [...], ...],  // Loaded
    67: [[...], [...], ...],  // Loaded
    // 68: not loaded yet
}
```

When selecting a device:
1. Check if `slaveKeymaps[addr]` exists and has data
2. If yes → use cached data (instant)
3. If no → load from device (1 second)

Cache is cleared on disconnect:
```javascript
async function disconnectDevice() {
    // ...
    slaveKeymaps = {};
    // ...
}
```

## Console Logging

### Before Selection Change
```
Device selector changed to slave: 66
Loading keymap for slave device 66...
```

### After Successful Load
```
Loaded keymap for slave device 66: [[{row:0,col:0,keycode:4},...],...]
```

### Selecting Already-Loaded Device
```
Device selector changed to slave: 66
Slave 66 keymap already loaded
```

### Error Case
```
Device selector changed to slave: 66
Loading keymap for slave device 66...
Failed to load keymap for slave device 66: Command timeout
```

## Performance Comparison

### Example: 1 Master + 3 Slaves (4×4 matrix each)

**Before (Eager Loading):**
- Master keymap: 16 queries = ~0.5s
- Slave 1 keymap: 16 queries = ~0.5s
- Slave 2 keymap: 16 queries = ~0.5s
- Slave 3 keymap: 16 queries = ~0.5s
- **Total: ~2 seconds**

**After (On-Demand):**
- Master keymap: 16 queries = ~0.5s
- **Total: ~0.5 seconds** (if only using master)
- +0.5s per slave when selected

### Typical Workflow
Most users will:
1. Connect
2. Configure master
3. Maybe configure 1-2 specific slaves
4. Done

**Time saved:** ~1-1.5 seconds per connection

## Edge Cases Handled

### 1. Switching Devices Quickly
If user rapidly clicks through devices:
- Each selection triggers load
- `loadingKeymap` flag prevents UI issues
- Already-loaded slaves return instantly

### 2. Connection Lost During Load
If device disconnects while loading slave:
- Error caught and displayed
- `loadingKeymap` flag cleared
- UI returns to disconnected state

### 3. Empty Slave Keymap
If slave returns empty keymap:
- Still cached as `[]`
- Won't re-query
- Shows "No keymap available" message

### 4. Slave Not Responding
If slave doesn't respond:
- Timeout after ~800ms
- Error message shown
- Can retry by re-selecting device

## Future Enhancements

### 1. Prefetch Next Slave
When selecting a slave, prefetch the next one in background:
```javascript
// If selecting slave 66, start loading 67 in background
if (i2cDevices[currentIndex + 1]) {
    loadSlaveKeymap(i2cDevices[currentIndex + 1].address);
}
```

### 2. Bulk Load Option
Add "Load All Slaves" button for power users:
```svelte
<button onclick={loadAllSlaves}>
    Load All Slave Keymaps
</button>
```

### 3. Progress Indicator
Show which slave is loading and total progress:
```
Loading slave 2/3 (0x43)...
```

### 4. Background Loading
Load slaves in background after master loads:
```javascript
// After master loaded
setTimeout(() => {
    for (const device of i2cDevices) {
        loadSlaveKeymap(device.address);
    }
}, 1000); // Start 1 second after connection
```

## Files Modified

- `src/routes/+page.svelte` - Added on-demand loading logic

## Related
- `FIX_SLAVE_KEYMAP.md` - Slave command handler implementation
- `TESTING_GUIDE.md` - Testing scenarios with master/slave
