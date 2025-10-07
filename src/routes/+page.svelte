<script>
    import { onMount, onDestroy } from 'svelte';
    import { invoke } from '@tauri-apps/api/core';

    // State variables
    let devices = [];
    let connectedDevice = null;
    let deviceInfo = $state(null);
    let isConnected = $state(false);
    let error = $state(null);
    let loading = $state(false);
    let dataLoaded = false;
    let loadingKeymap = $state(false);
    let loadingEncoders = $state(false);
    
    // I2C slave devices
    let i2cDevices = $state([]);
    let loadingI2CDevices = $state(false);
    let selectedDevice = $state('main'); // 'main' or slave address
    
    // Keymap and encoder data
    let keymap = $state([]);
    let slaveKeymaps = $state({}); // Map of slave address -> keymap
    let slaveEncoders = $state({}); // Map of slave address -> encoders
    let encoders = $state([]);
    let boardLayout = $state(null);
    let activeEncoderMenu = $state(null);
    let keycodes = {};
    
    // UI state
    let selectedTab = $state('keymap');
    let selectedKey = $state(null);
    let selectedEncoder = null;
    let originalKeymap = [];
    let originalEncoders = [];
    let originalSlaveKeymaps = {};
    let originalSlaveEncoders = {};
    let hasChanges = $state(false);
    let showSavePopup = $state(false);
    let connectionStatus = $state('disconnected');
    
    // Modal state
    let showKeyModal = $state(false);
    let showEncoderModal = $state(false);
    let modalKey = $state(null);
    let modalEncoder = $state(null);
    let keyModalTab = $state('standard'); // 'standard' or 'midi'
    let encoderModalTab = $state('standard');
    let encoderModalDirection = $state('ccw'); // 'ccw' or 'cw'
    
    // Autoconnect state
    let autoConnectInterval = null;
    let connectionCheckInterval = null;

    onMount(() => {
        console.log('=== FRONTEND: App started ===');
        loadKeycodes().finally(() => {
            console.log('=== FRONTEND: App ready ===');
        });

        document.addEventListener('pointerdown', handleGlobalPointerDown);
        document.addEventListener('keydown', handleGlobalKeydown);
        
        // Start autoconnect interval
        startAutoConnect();

        return () => {
            document.removeEventListener('pointerdown', handleGlobalPointerDown);
            document.removeEventListener('keydown', handleGlobalKeydown);
            stopAutoConnect();
            stopConnectionCheck();
        };
    });
    
    function startAutoConnect() {
        if (autoConnectInterval) return;
        
        // Try to connect immediately
        tryAutoConnect();
        
        // Then try every 1 second
        autoConnectInterval = setInterval(() => {
            if (!isConnected && !loading) {
                tryAutoConnect();
            }
        }, 1000);
    }
    
    function stopAutoConnect() {
        if (autoConnectInterval) {
            clearInterval(autoConnectInterval);
            autoConnectInterval = null;
        }
    }
    
    function startConnectionCheck() {
        if (connectionCheckInterval) return;
        
        // Check connection every 2 seconds
        connectionCheckInterval = setInterval(() => {
            if (isConnected) {
                checkConnection();
            }
        }, 2000);
    }
    
    function stopConnectionCheck() {
        if (connectionCheckInterval) {
            clearInterval(connectionCheckInterval);
            connectionCheckInterval = null;
        }
    }
    
    async function tryAutoConnect() {
        try {
            console.log('Attempting auto-connect...');
            const result = await invoke('simple_connect');
            
            isConnected = true;
            connectionStatus = 'connected';
            deviceInfo = result.device_info;
            keymap = result.keymap || [];
            encoders = result.encoders || [];
            boardLayout = result.layout ?? null;
            dataLoaded = true;

            if (!boardLayout) {
                try {
                    boardLayout = await invoke('get_board_layout');
                } catch (layoutError) {
                    console.warn('Failed to fetch board layout metadata:', layoutError);
                }
            }
            activeEncoderMenu = null;

            console.log('Auto-connected successfully:', result);
            
            // Only load I2C devices if this is a Master device (device_type = 1)
            if (deviceInfo.device_type === 1) {
                loadingI2CDevices = true;
                try {
                    i2cDevices = await invoke('get_i2c_devices');
                    console.log('I2C devices loaded:', i2cDevices);
                } catch (e) {
                    console.error('Failed to load I2C devices:', e);
                } finally {
                    loadingI2CDevices = false;
                }
            } else {
                console.log('Connected to slave device - no I2C slaves to query');
                i2cDevices = [];
            }
            
            slaveKeymaps = {};
            slaveEncoders = {};
            storeOriginalData();
            
            // Start monitoring connection
            startConnectionCheck();
            
            // Clear any previous errors
            error = null;
        } catch (e) {
            // Silent fail for autoconnect - don't show errors
            console.log('Auto-connect attempt failed (this is normal if no device is connected)');
        }
    }
    
    async function checkConnection() {
        try {
            // Try a lightweight operation to check if device is still connected
            await invoke('get_board_layout');
        } catch (e) {
            console.log('Device disconnected, cleaning up...');
            handleDisconnection();
        }
    }
    
    function handleDisconnection() {
        isConnected = false;
        connectionStatus = 'disconnected';
        deviceInfo = null;
        keymap = [];
        encoders = [];
        slaveKeymaps = {};
        slaveEncoders = {};
        boardLayout = null;
        selectedKey = null;
        selectedEncoder = null;
        activeEncoderMenu = null;
        originalKeymap = [];
        originalEncoders = [];
        originalSlaveKeymaps = {};
        originalSlaveEncoders = {};
        dataLoaded = false;
        hasChanges = false;
        showSavePopup = false;
        i2cDevices = [];
        
        stopConnectionCheck();
        
        console.log('Device disconnected, will attempt to reconnect...');
    }

    // Load keycodes from backend
    async function loadKeycodes() {
        try {
            keycodes = await invoke('get_keycodes');
        } catch (e) {
            console.error('Failed to load keycodes:', e);
        }
    }

    function setSelectedDevice(deviceId) {
        if (selectedDevice !== deviceId) {
            selectedDevice = deviceId;
        }
    }

    function storeOriginalData() {
        originalKeymap = JSON.parse(JSON.stringify(keymap));
        originalEncoders = JSON.parse(JSON.stringify(encoders));
        originalSlaveKeymaps = JSON.parse(JSON.stringify(slaveKeymaps));
        originalSlaveEncoders = JSON.parse(JSON.stringify(slaveEncoders));
        hasChanges = false;
        showSavePopup = false;
    }

    function checkForChanges() {
        const keymapChanged = JSON.stringify(keymap) !== JSON.stringify(originalKeymap);
        const encodersChanged = JSON.stringify(encoders) !== JSON.stringify(originalEncoders);
        const slaveKeymapsChanged = JSON.stringify(slaveKeymaps) !== JSON.stringify(originalSlaveKeymaps);
        const slaveEncodersChanged = JSON.stringify(slaveEncoders) !== JSON.stringify(originalSlaveEncoders);
        hasChanges = keymapChanged || encodersChanged || slaveKeymapsChanged || slaveEncodersChanged;
        showSavePopup = hasChanges;
    }

    async function connectDevice() {
        // This function is now just for manual reconnect if needed
        // The autoconnect will handle most connections
        await tryAutoConnect();
    }

    async function disconnectDevice() {
        loading = true;
        error = null;
        
        try {
            console.log('Manually disconnecting from device...');
            await invoke('simple_disconnect');
            handleDisconnection();
            console.log('Disconnected successfully');
        } catch (e) {
            error = `Failed to disconnect: ${e}`;
            console.error('Disconnect failed:', e);
        }
        
        loading = false;
    }

    // Keymap functions
    async function updateKeymap(row, col, keycode) {
        try {
            if (selectedDevice === 'main') {
                await invoke('set_keymap_entry', {
                    entry: { row, col, keycode }
                });
                
                keymap[row][col].keycode = keycode;
                keymap = keymap;
            } else {
                // Update slave device keymap
                    const slaveAddr = parseInt(selectedDevice, 10);
                await invoke('set_slave_keymap_entry', {
                    entry: { slave_addr: slaveAddr, row, col, keycode }
                });
                
                if (!slaveKeymaps[slaveAddr]) {
                    slaveKeymaps[slaveAddr] = [];
                }
                
                if (!slaveKeymaps[slaveAddr][row]) {
                    slaveKeymaps[slaveAddr][row] = [];
                }
                
                slaveKeymaps[slaveAddr][row][col] = { row, col, keycode };
                slaveKeymaps = {...slaveKeymaps}; // trigger reactivity
            }
            
            checkForChanges();
        } catch (e) {
            error = `Failed to update keymap: ${e}`;
            console.error('Failed to update keymap:', e);
        }
    }
    
    // Load slave keymap on-demand
    async function loadSlaveKeymap(slaveAddr) {
        console.log(`[loadSlaveKeymap] Called with slaveAddr: ${slaveAddr}`);
        
        // Always reload for now (during debugging)
        // TODO: Re-enable cache check after fixing the issue
        // if (slaveKeymaps[slaveAddr] && slaveKeymaps[slaveAddr].length > 0) {
        //     console.log(`[loadSlaveKeymap] Slave ${slaveAddr} keymap already loaded`);
        //     return;
        // }
        
        console.log(`[loadSlaveKeymap] Starting load for slave ${slaveAddr}...`);
        loadingKeymap = true;
        try {
            console.log(`[loadSlaveKeymap] Invoking get_full_slave_keymap for slave 0x${slaveAddr.toString(16)}...`);
            const slaveKeymap = await invoke('get_full_slave_keymap', { slaveAddr: slaveAddr });
            console.log(`[loadSlaveKeymap] Received ${slaveKeymap.length} keymap entries for slave ${slaveAddr}`);
            slaveKeymaps[slaveAddr] = slaveKeymap;
            slaveKeymaps = {...slaveKeymaps}; // trigger reactivity
            if (!originalSlaveKeymaps[String(slaveAddr)]) {
                originalSlaveKeymaps[String(slaveAddr)] = JSON.parse(JSON.stringify(slaveKeymap));
            }
            checkForChanges();
            console.log(`[loadSlaveKeymap] Successfully loaded keymap for slave device ${slaveAddr}`);
        } catch (e) {
            error = `Failed to load keymap for slave device ${slaveAddr}: ${e}`;
            console.error(`[loadSlaveKeymap] Failed to load keymap for slave device ${slaveAddr}:`, e);
        } finally {
            loadingKeymap = false;
            console.log(`[loadSlaveKeymap] Finished loading for slave ${slaveAddr}`);
        }
    }

    async function loadSlaveEncoders(slaveAddr) {
        console.log(`[loadSlaveEncoders] Called with slaveAddr: ${slaveAddr}`);
        loadingEncoders = true;
        try {
            console.log(`[loadSlaveEncoders] Invoking get_full_slave_encoders for slave 0x${slaveAddr.toString(16)}...`);
            const slaveEncoderList = await invoke('get_full_slave_encoders', { slaveAddr });
            console.log(`[loadSlaveEncoders] Received ${slaveEncoderList.length} encoders for slave ${slaveAddr}`);
            slaveEncoders[slaveAddr] = slaveEncoderList;
            slaveEncoders = { ...slaveEncoders };
            if (!originalSlaveEncoders[String(slaveAddr)]) {
                originalSlaveEncoders[String(slaveAddr)] = JSON.parse(JSON.stringify(slaveEncoderList));
            }
            checkForChanges();
        } catch (e) {
            error = `Failed to load encoders for slave device ${slaveAddr}: ${e}`;
            console.error(`[loadSlaveEncoders] Failed to load encoders for slave device ${slaveAddr}:`, e);
        } finally {
            loadingEncoders = false;
            console.log(`[loadSlaveEncoders] Finished loading for slave ${slaveAddr}`);
        }
    }
    
    // Watch for device selection changes
    $effect(() => {
        console.log(`[EFFECT] selectedDevice changed to: ${selectedDevice}, isConnected: ${isConnected}`);
        activeEncoderMenu = null;
        if (selectedDevice !== 'main' && isConnected) {
            const slaveAddr = parseInt(selectedDevice, 10);
            console.log(`[EFFECT] Loading keymap for slave: ${slaveAddr}`);
            loadSlaveKeymap(slaveAddr);
            loadSlaveEncoders(slaveAddr);
        } else {
            console.log(`[EFFECT] Not loading slave keymap (selectedDevice=${selectedDevice}, isConnected=${isConnected})`);
        }
    });

    // Ensure encoder layouts refresh when board metadata changes
    $effect(() => {
        if (!isConnected) {
            activeEncoderMenu = null;
        }
    });
    
    // Get the current active keymap based on selected device
    function getCurrentKeymap() {
        if (selectedDevice === 'main') {
            return keymap;
        } else {
            const slaveAddr = parseInt(selectedDevice, 10);
            return slaveKeymaps[slaveAddr] || [];
        }
    }

    function getCurrentEncoders() {
        if (selectedDevice === 'main') {
            return encoders;
        } else {
            const slaveAddr = parseInt(selectedDevice, 10);
            return slaveEncoders[slaveAddr] || [];
        }
    }

    function getCurrentLayout() {
        return boardLayout;
    }

    function isEncoderCell(row, col) {
        const layout = getCurrentLayout();
        if (!layout) return false;
        const cols = layout.matrix_cols ?? 0;
        if (col >= cols) return false;
        const index = row * cols + col;
        const byteIndex = Math.floor(index / 8);
        const bitIndex = index % 8;
        const bitmap = layout.encoder_bitmap || [];
        const byte = bitmap[byteIndex] ?? 0;
        return (byte & (1 << bitIndex)) !== 0;
    }

    function encoderIdForCell(row, col) {
        const layout = getCurrentLayout();
        if (!layout) return null;
        if (!isEncoderCell(row, col)) return null;

        const firstEncoderColumn = layout.first_encoder_column ?? 0;
        if (col < firstEncoderColumn) return null;

        const perRow = layout.encoders_per_row ?? 0;
        if (!perRow) return null;

        const offsetCol = col - firstEncoderColumn;
        const id = row * perRow + offsetCol;
        return id < (layout.encoder_count ?? 0) ? id : null;
    }

    function getEncoderEntryById(encoderId) {
        return getCurrentEncoders().find((encoder) => encoder.encoder_id === encoderId) || null;
    }

    function closeEncoderMenu() {
        activeEncoderMenu = null;
    }

    /** @param {PointerEvent} event */
    function handleGlobalPointerDown(event) {
        if (!activeEncoderMenu) return;
        const target = event.target;
        if (!(target instanceof Element)) {
            return;
        }

        if (target.closest('.encoder-menu') || target.closest('.encoder-key')) {
            return;
        }

        closeEncoderMenu();
    }

    /** @param {KeyboardEvent} event */
    function handleGlobalKeydown(event) {
        if (!activeEncoderMenu) return;
        if (event.key === 'Escape') {
            closeEncoderMenu();
        }
    }

    function toggleEncoderMenu(row, col, event) {
        if (event) {
            event.stopPropagation();
        }
        if (loadingEncoders || loadingKeymap) {
            return;
        }
        const encoderId = encoderIdForCell(row, col);
        if (encoderId === null) return;

        if (activeEncoderMenu && activeEncoderMenu.row === row && activeEncoderMenu.col === col) {
            activeEncoderMenu = null;
        } else {
            activeEncoderMenu = { row, col, encoderId };
        }
    }

    function handleEncoderAction(row, col, action, event) {
        if (event) {
            event.stopPropagation();
        }

        const encoderId = encoderIdForCell(row, col);
        if (encoderId === null) {
            return;
        }

        const encoderEntry = getEncoderEntryById(encoderId) || {
            encoder_id: encoderId,
            ccw_keycode: 0,
            cw_keycode: 0,
            reserved: 0
        };

        if (action === 'press') {
            const currentKeymap = getCurrentKeymap();
            const key = currentKeymap?.[row]?.[col];
            const keycode = key?.keycode ?? 0;
            openKeyModal(row, col, keycode);
        } else if (action === 'ccw' || action === 'cw') {
            openEncoderModal(encoderEntry, action);
        }

        activeEncoderMenu = null;
    }

    async function updateEncoder(encoderId, ccwKeycode, cwKeycode) {
        try {
            if (selectedDevice === 'main') {
                await invoke('set_encoder_entry', {
                    entry: {
                        encoder_id: encoderId,
                        ccw_keycode: ccwKeycode,
                        cw_keycode: cwKeycode,
                        reserved: 0
                    }
                });

                const encoder = encoders.find(e => e.encoder_id === encoderId);
                if (encoder) {
                    encoder.ccw_keycode = ccwKeycode;
                    encoder.cw_keycode = cwKeycode;
                    encoders = encoders;
                }
            } else {
                const slaveAddr = parseInt(selectedDevice, 10);
                const existingEncoders = slaveEncoders[slaveAddr] ? [...slaveEncoders[slaveAddr]] : [];
                const entryIndex = existingEncoders.findIndex(e => e.encoder_id === encoderId);
                const reserved = entryIndex >= 0 ? existingEncoders[entryIndex].reserved ?? 0 : 0;

                await invoke('set_slave_encoder_entry', {
                    entry: {
                        slave_addr: slaveAddr,
                        encoder_id: encoderId,
                        ccw_keycode: ccwKeycode,
                        cw_keycode: cwKeycode,
                        reserved
                    }
                });

                if (entryIndex >= 0) {
                    existingEncoders[entryIndex] = {
                        ...existingEncoders[entryIndex],
                        ccw_keycode: ccwKeycode,
                        cw_keycode: cwKeycode
                    };
                } else {
                    existingEncoders.push({
                        slave_addr: slaveAddr,
                        encoder_id: encoderId,
                        ccw_keycode: ccwKeycode,
                        cw_keycode: cwKeycode,
                        reserved
                    });
                }

                slaveEncoders = {
                    ...slaveEncoders,
                    [slaveAddr]: existingEncoders
                };
            }
            checkForChanges();
        } catch (e) {
            error = `Failed to update encoder: ${e}`;
        }
    }

    // Configuration functions
    async function saveConfig() {
        loading = true;
        error = null;
        
        try {
            await invoke('save_config');
            storeOriginalData();
        } catch (e) {
            error = `Failed to save config: ${e}`;
        }
        
        loading = false;
    }

    async function loadConfig() {
        loading = true;
        error = null;
        
        try {
            await invoke('load_config');
            if (isConnected) {
                // Fix: loadDeviceData was undeclared. Assuming it's a typo and should be connectDevice or a similar function.
                // For now, we'll assume it's meant to reload device data after loading config.
                // If loadDeviceData is a separate function, it needs to be defined.
                // As a placeholder, we'll call connectDevice to refresh data.
                await connectDevice(); 
            }
        } catch (e) {
            error = `Failed to load config: ${e}`;
        }
        
        loading = false;
    }

    async function resetConfig() {
        if (!confirm('Are you sure you want to reset the configuration to defaults?')) {
            return;
        }
        
        loading = true;
        error = null;
        
        try {
            await invoke('reset_config');
            if (isConnected) {
                // Fix: loadDeviceData was undeclared. Assuming it's a typo and should be connectDevice or a similar function.
                // For now, we'll assume it's meant to reload device data after resetting config.
                // If loadDeviceData is a separate function, it needs to be defined.
                // As a placeholder, we'll call connectDevice to refresh data.
                await connectDevice();
            }
        } catch (e) {
            error = `Failed to reset config: ${e}`;
        }
        
        loading = false;
    }

    // MIDI encoding helpers
    const OP_MIDI_CC_BASE = 0x7E10;
    
    function midiValueIndex(value) {
        const values = [0, 1, 7, 15, 31, 43, 45, 63, 64, 79, 95, 111, 120, 127, 50, 100];
        const idx = values.indexOf(Number(value));
        return idx >= 0 ? idx : 12;
    }
    
    function encodeMidiCC(channel, controller, value) {
        const ch = (Number(channel) - 1) & 0x0F;
        const ctrl = Number(controller) & 0x7F;
        const idx = midiValueIndex(Number(value));
        return (OP_MIDI_CC_BASE + (ch << 11) + (ctrl << 4) + (idx & 0x0F)) & 0xFFFF;
    }
    
    function encodeMidiNote(channel, note) {
        const ch = (Number(channel) - 1) & 0x0F;
        const n = Number(note) & 0x7F;
        return (OP_MIDI_CC_BASE + (ch << 11) + (n << 4) + 0x0F) & 0xFFFF;
    }

    function formatKeyLabel(code) {
        if (typeof code !== 'number') return [''];
        if (code >= OP_MIDI_CC_BASE) {
            const delta = (code - OP_MIDI_CC_BASE) & 0xFFFF;
            const ch = ((delta >> 11) & 0x0F) + 1;
            const ctrl_or_note = (delta >> 4) & 0x7F;
            const idx = delta & 0x0F;
            const values = [0,1,7,15,31,43,45,63,64,79,95,111,120,127,50,100];
            if (idx === 0x0F) {
                return [`Ch ${ch}`, `Note ${ctrl_or_note}`];
            }
            const val = values[idx] !== undefined ? values[idx] : 127;
            return [`Ch ${ch}`, `CC ${ctrl_or_note}`, `${val}`];
        }
        const name = keycodes[code]?.display_name;
        if (name) {
            const parts = name.split(' ');
            if (parts.length <= 2) return [name];
            return [parts.slice(0,2).join(' '), parts.slice(2).join(' ')];
        }
        return [`0x${code.toString(16).toUpperCase().padStart(4, '0')}`];
    }

    function getKeycodeOptions() {
        return Object.entries(keycodes).map(([code, keycode]) => ({
            value: parseInt(code),
            label: `${keycode.display_name}`,
            name: keycode.name,
            category: keycode.category
        }));
    }

    function getKeycodesByCategory() {
        const options = getKeycodeOptions();
        const categories = {};
        options.forEach(opt => {
            if (!categories[opt.category]) {
                categories[opt.category] = [];
            }
            categories[opt.category].push(opt);
        });
        return categories;
    }

    // Modal handlers
    function openKeyModal(row, col, keycode) {
        modalKey = { row, col, keycode };
        keyModalTab = 'standard';
        showKeyModal = true;
        activeEncoderMenu = null;
    }

    function closeKeyModal() {
        showKeyModal = false;
        modalKey = null;
    }

    function applyKeyChange() {
        if (modalKey) {
            updateKeymap(modalKey.row, modalKey.col, modalKey.keycode);
            closeKeyModal();
        }
    }

    function openEncoderModal(encoder, direction) {
        modalEncoder = { ...encoder };
        encoderModalDirection = direction;
        encoderModalTab = 'standard';
        showEncoderModal = true;
        activeEncoderMenu = null;
    }

    function closeEncoderModal() {
        showEncoderModal = false;
        modalEncoder = null;
    }

    function applyEncoderChange() {
        if (modalEncoder) {
            updateEncoder(
                modalEncoder.encoder_id,
                modalEncoder.ccw_keycode,
                modalEncoder.cw_keycode
            );
            closeEncoderModal();
        }
    }

    // MIDI form state
    let midiType = $state('cc');
    let midiChannel = $state(1);
    let midiController = $state(1);
    let midiValue = $state(43);
    let midiNote = $state(60);

    function applyMidiToKey() {
        if (!modalKey) return;
        let code = 0;
        if (midiType === 'cc') {
            code = encodeMidiCC(midiChannel, midiController, midiValue);
        } else {
            code = encodeMidiNote(midiChannel, midiNote);
        }
        modalKey.keycode = code;
        modalKey = modalKey; // trigger reactivity
    }

    function applyMidiToEncoder() {
        if (!modalEncoder) return;
        let code = 0;
        if (midiType === 'cc') {
            code = encodeMidiCC(midiChannel, midiController, midiValue);
        } else {
            code = encodeMidiNote(midiChannel, midiNote);
        }
        if (encoderModalDirection === 'ccw') {
            modalEncoder.ccw_keycode = code;
        } else {
            modalEncoder.cw_keycode = code;
        }
        modalEncoder = modalEncoder; // trigger reactivity
    }

    function discardChanges() {
        keymap = JSON.parse(JSON.stringify(originalKeymap));
        encoders = JSON.parse(JSON.stringify(originalEncoders));
        slaveKeymaps = JSON.parse(JSON.stringify(originalSlaveKeymaps));
        slaveEncoders = JSON.parse(JSON.stringify(originalSlaveEncoders));
        hasChanges = false;
        showSavePopup = false;
    }

    function handleOverlayKeydown(event, closeHandler) {
        if (event.key === 'Enter' || event.key === ' ' || event.key === 'Escape') {
            event.preventDefault();
            closeHandler();
        }
    }
</script>

<main class="app">
    <div class="app-background"></div>
    
    {#if !isConnected}
        <div class="fullscreen-loading">
            <div class="loading-content">
                <div class="logo-large">
                    <svg width="64" height="64" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                        <path d="M3 7H21L19 2H5L3 7Z" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
                        <path d="M3 7L5 22H19L21 7" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
                        <path d="M9 12H15" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
                    </svg>
                </div>
                <div class="spinner-large"></div>
                <h2>Searching for device...</h2>
                <p>Please connect your keyboard</p>
            </div>
        </div>
    {:else}
    <div class="container">
    <header class="header">
            <div class="header-content">
                <div class="logo-section">
                    <div class="logo">
                        <svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                            <path d="M3 7H21L19 2H5L3 7Z" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
                            <path d="M3 7L5 22H19L21 7" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
                            <path d="M9 12H15" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
                        </svg>
                    </div>
                    <h1 class="title">openGRADER Configurator</h1>
                </div>
                
                <div class="connection-status">
                    {#if isConnected}
                        <div class="status-indicator connected">
                            <div class="status-dot"></div>
                            <span>Connected</span>
                        </div>
                    {:else}
                        <div class="status-indicator disconnected">
                            <div class="status-dot"></div>
                            <span>Disconnected</span>
                        </div>
                    {/if}
                </div>
            </div>
        </header>

    {#if error}
            <div class="error-banner">
                <div class="error-content">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                        <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="2"/>
                        <line x1="15" y1="9" x2="9" y2="15" stroke="currentColor" stroke-width="2"/>
                        <line x1="9" y1="9" x2="15" y2="15" stroke="currentColor" stroke-width="2"/>
                    </svg>
                    <span>{error}</span>
                </div>
                <button class="error-close" onclick={() => error = null} aria-label="Close error message">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                        <line x1="18" y1="6" x2="6" y2="18" stroke="currentColor" stroke-width="2"/>
                        <line x1="6" y1="6" x2="18" y2="18" stroke="currentColor" stroke-width="2"/>
                    </svg>
                </button>
            </div>
        {/if}
{#if isConnected}
            <div class="device-card-grid" role="tablist" aria-label="Device selection">
                <button
                    class="device-card"
                    class:active={selectedDevice === 'main'}
                    type="button"
                    aria-pressed={selectedDevice === 'main'}
                    onclick={() => setSelectedDevice('main')}
                >
                    <div class="device-card-image master-placeholder" aria-hidden="true"></div>
                    <div class="device-card-content">
                        <span class="device-card-title">{deviceInfo?.device_name || 'Main Device'}</span>
                        <span class="device-card-subtitle">Master module</span>
                    </div>
                </button>

                {#each i2cDevices as device}
                    {#if device.status === 1}
                        {#key device.address}
                            <button
                                class="device-card"
                                class:active={selectedDevice === String(device.address)}
                                type="button"
                                aria-pressed={selectedDevice === String(device.address)}
                                onclick={() => setSelectedDevice(String(device.address))}
                            >
                                <div class="device-card-image module-placeholder" aria-hidden="true"></div>
                                <div class="device-card-content">
                                    <span class="device-card-title">{device.name || `Slave 0x${device.address.toString(16).toUpperCase()}`}</span>
                                    <span class="device-card-subtitle">Module connected</span>
                                </div>
                            </button>
                        {/key}
                    {/if}
                {/each}
            </div>
        {/if}
    <!--
    <div class="glass-card connection-card">
            <div class="connection-layout">
                {#if !isConnected}
                    <div class="autoconnect-status">
                        <div class="spinner-small"></div>
                        <span>Searching for device...</span>
                    </div>
                {/if}

                {#if deviceInfo}
                    <div class="device-info-compact">
                        <div class="info-chip">
                            <span class="info-label">Device:</span>
                            <span class="info-value">{deviceInfo.device_name}</span>
                        </div>
                        <div class="info-chip">
                            <span class="info-label">FW:</span>
                            <span class="info-value">{deviceInfo.firmware_version_major}.{deviceInfo.firmware_version_minor}.{deviceInfo.firmware_version_patch}</span>
                        </div>
                        <div class="info-chip">
                            <span class="info-label">Matrix:</span>
                            <span class="info-value">{deviceInfo.matrix_rows}×{deviceInfo.matrix_cols}</span>
                        </div>
                        <div class="info-chip">
                            <span class="info-label">Encoders:</span>
                            <span class="info-value">{deviceInfo.encoder_count}</span>
                        </div>
                    </div>
                {/if}
            </div>
        </div>
    -->

        

        {#if isConnected}
                <div class="glass-card keymap-card" class:has-active-menu={activeEncoderMenu !== null}>
                    <div class="card-header">
                        <h2>Keymap Configuration</h2>
                        <p>Click on any key to customize its function</p>
                    </div>
                    
                    {#if loadingKeymap}
                        <div class="loading-state">
                            <div class="spinner-large"></div>
                            <h3>Loading Keymap...</h3>000
                            <p>{selectedDevice === 'main' ? 'Fetching key configuration from device' : `Loading keymap from slave device 0x${parseInt(selectedDevice, 10).toString(16).toUpperCase()}`}</p>
                        </div>
                    {:else if (selectedDevice === 'main' && keymap.length > 0) || (selectedDevice !== 'main' && slaveKeymaps[selectedDevice] && slaveKeymaps[selectedDevice].length > 0)}
                        <!-- keys open modal -->
                        <div class="keymap-container">
                            <div
                                class="keymap"
                                class:loading={loadingKeymap || loadingEncoders}
                                aria-busy={loadingKeymap || loadingEncoders}
                            >
                                {#each getCurrentKeymap() as row, rowIndex}
                                    <div class="keymap-row">
                                        {#each row as key, colIndex}
                                            <div class={`keymap-cell ${isEncoderCell(rowIndex, colIndex) ? 'encoder-cell' : ''} ${activeEncoderMenu && activeEncoderMenu.row === rowIndex && activeEncoderMenu.col === colIndex ? 'active-encoder' : ''}`}>
                                                {#if isEncoderCell(rowIndex, colIndex)}
                                                    <button
                                                        class="key encoder-key"
                                                        class:menu-open={activeEncoderMenu && activeEncoderMenu.row === rowIndex && activeEncoderMenu.col === colIndex}
                                                        onclick={(event) => toggleEncoderMenu(rowIndex, colIndex, event)}
                                                        aria-label={`Encoder ${encoderIdForCell(rowIndex, colIndex) ?? ''} at R${rowIndex}C${colIndex}`}
                                                    >
                                                        {#if key}
                                                            {#each formatKeyLabel(key.keycode) as line}
                                                                <div class="key-label">{line}</div>
                                                            {/each}
                                                        {/if}
                                                    </button>
                                                    <div class={`encoder-menu ${activeEncoderMenu && activeEncoderMenu.row === rowIndex && activeEncoderMenu.col === colIndex ? 'active' : ''}`}>
                                                        {#if activeEncoderMenu && activeEncoderMenu.row === rowIndex && activeEncoderMenu.col === colIndex}
                                                            {@const encoderId = encoderIdForCell(rowIndex, colIndex)}
                                                            {@const encoderEntry = getEncoderEntryById(encoderId)}
                                                            <button class="encoder-action ccw" onclick={(event) => handleEncoderAction(rowIndex, colIndex, 'ccw', event)}>
                                                                <div class="encoder-action-header">
                                                                    <span class="encoder-action-icon">⟲</span>
                                                                    <span class="encoder-action-label">CCW</span>
                                                                </div>
                                                                <div class="encoder-action-preview">
                                                                    {#each formatKeyLabel(encoderEntry?.ccw_keycode ?? 0) as line}
                                                                        <div class="preview-line">{line}</div>
                                                                    {/each}
                                                                </div>
                                                            </button>
                                                            <button class="encoder-action press" onclick={(event) => handleEncoderAction(rowIndex, colIndex, 'press', event)}>
                                                                <div class="encoder-action-header">
                                                                    <svg class="encoder-action-icon" width="20" height="20" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                                                                        <path d="M9 3V6H4V9L12 17L20 9V6H15V3H9Z" fill="currentColor"/>
                                                                        <path d="M12 17L6 20V22H18V20L12 17Z" fill="currentColor" opacity="0.6"/>
                                                                    </svg>
                                                                    <span class="encoder-action-label">Press</span>
                                                                </div>
                                                                <div class="encoder-action-preview">
                                                                    {#each formatKeyLabel(key?.keycode ?? 0) as line}
                                                                        <div class="preview-line">{line}</div>
                                                                    {/each}
                                                                </div>
                                                            </button>
                                                            <button class="encoder-action cw" onclick={(event) => handleEncoderAction(rowIndex, colIndex, 'cw', event)}>
                                                                <div class="encoder-action-header">
                                                                    <span class="encoder-action-icon">⟳</span>
                                                                    <span class="encoder-action-label">CW</span>
                                                                </div>
                                                                <div class="encoder-action-preview">
                                                                    {#each formatKeyLabel(encoderEntry?.cw_keycode ?? 0) as line}
                                                                        <div class="preview-line">{line}</div>
                                                                    {/each}
                                                                </div>
                                                            </button>
                                                        {/if}
                                                    </div>
                                                {:else}
                                                    <button
                                                        class="key"
                                                        onclick={() => openKeyModal(rowIndex, colIndex, key.keycode)}
                                                        aria-label={`Key R${rowIndex}C${colIndex}`}
                                                    >
                                                        {#each formatKeyLabel(key.keycode) as line}
                                                            <div class="key-label">{line}</div>
                                                        {/each}
                                                    </button>
                                                {/if}
                                            </div>
                                        {/each}
                                    </div>
                                {/each}
                            </div>
                            {#if loadingKeymap || loadingEncoders}
                                <div class="keymap-overlay" aria-live="polite">
                                    <div class="spinner spinner-small"></div>
                                    <span>Loading layout…</span>
                                </div>
                            {/if}
                        </div>
                    {:else}
                        <div class="empty-state">
                            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                                <rect x="2" y="6" width="20" height="12" rx="2" stroke="currentColor" stroke-width="2"/>
                                <path d="M6 10H8" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
                                <path d="M10 10H12" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
                                <path d="M14 10H16" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
                            </svg>
                            <h3>No Keymap Available</h3>
                            <p>Connect to a device to view and configure the keymap</p>
                        </div>
                    {/if}
                </div>
            {/if}
    </div>
    {/if}

    {#if showKeyModal && modalKey}
        <div 
            class="modal-overlay" 
            onclick={closeKeyModal}
            onkeydown={(e) => handleOverlayKeydown(e, closeKeyModal)}
            role="button"
            tabindex="0"
            aria-label="Close modal"
        >
            <div 
                class="modal-content" 
                onclick={(e) => e.stopPropagation()}
                onkeydown={(e) => e.stopPropagation()}
                role="dialog"
                tabindex="-1"
                aria-modal="true"
                aria-labelledby="key-modal-title"
            >
                <div class="modal-header">
                    <h3 id="key-modal-title">Edit Key R{modalKey.row}C{modalKey.col}</h3>
                    <button class="modal-close" onclick={closeKeyModal} aria-label="Close modal">
                        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                            <line x1="18" y1="6" x2="6" y2="18" stroke="currentColor" stroke-width="2"/>
                            <line x1="6" y1="6" x2="18" y2="18" stroke="currentColor" stroke-width="2"/>
                        </svg>
                    </button>
                </div>

                <div class="modal-tabs">
                    <button 
                        class="modal-tab"
                        class:active={keyModalTab === 'standard'}
                        onclick={() => keyModalTab = 'standard'}
                    >
                        Standard Keys
                    </button>
                    <button 
                        class="modal-tab"
                        class:active={keyModalTab === 'midi'}
                        onclick={() => keyModalTab = 'midi'}
                    >
                        MIDI
                    </button>
                </div>

                <div class="modal-body">
                    {#if keyModalTab === 'standard'}
                        <div class="keycode-selector">
                            {#each Object.entries(getKeycodesByCategory()) as [category, options]}
                                <div class="keycode-category">
                                    <h4 class="category-title">{category}</h4>
                                    <div class="keycode-grid">
                                        {#each options as option}
                                            <button
                                                class="keycode-option"
                                                class:selected={modalKey.keycode === option.value}
                                                onclick={() => modalKey.keycode = option.value}
                                            >
                                                {option.label}
                                            </button>
                                        {/each}
                                    </div>
                                </div>
                            {/each}
                        </div>
                    {:else}
                        <div class="midi-config">
                            <div class="form-group">
                                <label for="midiType">MIDI Type</label>
                                <select id="midiType" bind:value={midiType}>
                                    <option value="cc">Control Change (CC)</option>
                                    <option value="note">Note</option>
                                </select>
                            </div>
                            <div class="form-row">
                                <div class="form-group">
                                    <label for="midiChannel">Channel</label>
                                    <input id="midiChannel" type="number" min="1" max="16" bind:value={midiChannel} />
                                </div>
                                {#if midiType === 'cc'}
                                    <div class="form-group">
                                        <label for="midiController">Controller</label>
                                        <input id="midiController" type="number" min="0" max="127" bind:value={midiController} />
                                    </div>
                                    <div class="form-group">
                                        <label for="midiValue">Value</label>
                                        <input id="midiValue" type="number" min="0" max="127" bind:value={midiValue} />
                                    </div>
                                {:else}
                                    <div class="form-group">
                                        <label for="midiNote">Note</label>
                                        <input id="midiNote" type="number" min="0" max="127" bind:value={midiNote} />
                                    </div>
                                {/if}
                            </div>
                            <div class="midi-preview">
                                <div class="preview-compact">
                                    {#each formatKeyLabel(modalKey.keycode) as line}
                                        <div class="preview-line">{line}</div>
                                    {/each}
                                </div>
                                <div class="muted">Use the main Apply button to save changes.</div>
                            </div>
                        </div>
                    {/if}
                </div>

                <div class="modal-footer">
                    <button class="secondary-button" onclick={closeKeyModal}>Cancel</button>
                    <button class="primary-button" onclick={applyKeyChange}>Apply</button>
                </div>
            </div>
        </div>
    {/if}

    {#if showEncoderModal && modalEncoder}
        <div 
            class="modal-overlay" 
            onclick={closeEncoderModal}
            onkeydown={(e) => handleOverlayKeydown(e, closeEncoderModal)}
            role="button"
            tabindex="0"
            aria-label="Close modal"
        >
            <div 
                class="modal-content" 
                onclick={(e) => e.stopPropagation()}
                onkeydown={(e) => e.stopPropagation()}
                role="dialog"
                tabindex="-1"
                aria-modal="true"
                aria-labelledby="encoder-modal-title"
            >
                <div class="modal-header">
                    <h3 id="encoder-modal-title">Edit Encoder {modalEncoder.encoder_id} - {encoderModalDirection === 'ccw' ? 'Counter-Clockwise' : 'Clockwise'}</h3>
                    <button class="modal-close" onclick={closeEncoderModal} aria-label="Close modal">
                        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                            <line x1="18" y1="6" x2="6" y2="18" stroke="currentColor" stroke-width="2"/>
                            <line x1="6" y1="6" x2="18" y2="18" stroke="currentColor" stroke-width="2"/>
                        </svg>
                    </button>
                </div>

                <div class="modal-tabs">
                    <button 
                        class="modal-tab"
                        class:active={encoderModalTab === 'standard'}
                        onclick={() => encoderModalTab = 'standard'}
                    >
                        Standard Keys
                    </button>
                    <button 
                        class="modal-tab"
                        class:active={encoderModalTab === 'midi'}
                        onclick={() => encoderModalTab = 'midi'}
                    >
                        MIDI
                    </button>
                </div>

                <div class="modal-body">
                    {#if encoderModalTab === 'standard'}
                        <div class="keycode-selector">
                            {#each Object.entries(getKeycodesByCategory()) as [category, options]}
                                <div class="keycode-category">
                                    <h4 class="category-title">{category}</h4>
                                    <div class="keycode-grid">
                                        {#each options as option}
                                            <button
                                                class="keycode-option"
                                                class:selected={
                                                    (encoderModalDirection === 'ccw' && modalEncoder.ccw_keycode === option.value) ||
                                                    (encoderModalDirection === 'cw' && modalEncoder.cw_keycode === option.value)
                                                }
                                                onclick={() => {
                                                    if (encoderModalDirection === 'ccw') {
                                                        modalEncoder.ccw_keycode = option.value;
                                                    } else {
                                                        modalEncoder.cw_keycode = option.value;
                                                    }
                                                    modalEncoder = modalEncoder;
                                                }}
                                            >
                                                {option.label}
                                            </button>
                                        {/each}
                                    </div>
                                </div>
                            {/each}
                        </div>
                    {:else}
                        <div class="midi-config">
                            <div class="form-group">
                                <label for="encoderMidiType">MIDI Type</label>
                                <select id="encoderMidiType" bind:value={midiType}>
                                    <option value="cc">Control Change (CC)</option>
                                    <option value="note">Note</option>
                                </select>
                            </div>
                            <div class="form-row">
                                <div class="form-group">
                                    <label for="encoderMidiChannel">Channel</label>
                                    <input id="encoderMidiChannel" type="number" min="1" max="16" bind:value={midiChannel} />
                                </div>
                                {#if midiType === 'cc'}
                                    <div class="form-group">
                                        <label for="encoderMidiController">Controller</label>
                                        <input id="encoderMidiController" type="number" min="0" max="127" bind:value={midiController} />
                                    </div>
                                    <div class="form-group">
                                        <label for="encoderMidiValue">Value</label>
                                        <input id="encoderMidiValue" type="number" min="0" max="127" bind:value={midiValue} />
                                    </div>
                                {:else}
                                    <div class="form-group">
                                        <label for="encoderMidiNote">Note</label>
                                        <input id="encoderMidiNote" type="number" min="0" max="127" bind:value={midiNote} />
                                    </div>
                                {/if}
                            </div>
                            <div class="midi-preview">
                                <div class="preview-compact">
                                    {#each formatKeyLabel(encoderModalDirection === 'ccw' ? modalEncoder.ccw_keycode : modalEncoder.cw_keycode) as line}
                                        <div class="preview-line">{line}</div>
                                    {/each}
                                </div>
                                <div class="muted">Use the main Apply button to save changes.</div>
                            </div>
                        </div>
                    {/if}
                </div>

                <div class="modal-footer">
                    <button class="secondary-button" onclick={closeEncoderModal}>Cancel</button>
                    <button class="primary-button" onclick={applyEncoderChange}>Apply</button>
                </div>
            </div>
        </div>
    {/if}

    {#if showSavePopup}
        <div class="save-popup">
            <div class="save-popup-content">
                <div class="save-popup-text">
                    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                        <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="2"/>
                        <path d="M12 8V12" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
                        <circle cx="12" cy="16" r="1" fill="currentColor"/>
                    </svg>
                    <span>You have unsaved changes</span>
                </div>
                <div class="save-popup-actions">
                    <button class="popup-button discard" onclick={discardChanges}>
                        Discard
                    </button>
                    <button class="popup-button save" onclick={saveConfig}>
                        Save to EEPROM
                    </button>
                </div>
            </div>
        </div>
    {/if}
</main>

<style>
    :global(*) {
        box-sizing: border-box;
    }

    :global(body) {
        margin: 0;
        padding: 0;
        font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Display', 'SF Pro Text', 'Segoe UI', Roboto, sans-serif;
        background: #f5f5f7;
        color: #1d1d1f;
        overflow-x: hidden;
    }

    .app {
        min-height: 100vh;
        position: relative;
        padding-bottom: 100px;
    }

    .app-background {
        position: fixed;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        background: 
            radial-gradient(circle at 20% 80%, rgba(120, 119, 198, 0.04) 0%, transparent 50%),
            radial-gradient(circle at 80% 20%, rgba(255, 119, 198, 0.03) 0%, transparent 50%),
            radial-gradient(circle at 40% 40%, rgba(120, 219, 255, 0.02) 0%, transparent 50%),
            linear-gradient(135deg, #f5f5f7 0%, #fafafa 100%);
        z-index: -1;
    }
    
    /* Fullscreen Loading Overlay */
    .fullscreen-loading {
        position: fixed;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        width: 100vw;
        height: 100vh;
        display: flex;
        align-items: center;
        justify-content: center;
        background: #f5f5f7;
        z-index: 9999;
    }
    
    .loading-content {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 24px;
        text-align: center;
    }
    
    .logo-large {
        width: 80px;
        height: 80px;
        background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
        border-radius: 20px;
        display: flex;
        align-items: center;
        justify-content: center;
        color: white;
        box-shadow: 0 8px 32px rgba(102, 126, 234, 0.3);
    }
    
    .loading-content h2 {
        font-size: 24px;
        font-weight: 600;
        color: #1d1d1f;
        margin: 0;
        letter-spacing: -0.5px;
    }
    
    .loading-content p {
        font-size: 16px;
        color: #86868b;
        margin: 0;
    }

    .container {
        max-width: 1400px;
        margin: 0 auto;
        padding: 24px;
        position: relative;
        z-index: 1;
    }

    /* Header */
    .header {
        margin-bottom: 32px;
    }

    .header-content {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 20px 0;
    }

    .logo-section {
        display: flex;
        align-items: center;
        gap: 16px;
    }

    .logo {
        width: 40px;
        height: 40px;
        background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
        border-radius: 12px;
        display: flex;
        align-items: center;
        justify-content: center;
        color: white;
    }

    .title {
        font-size: 28px;
        font-weight: 600;
        margin: 0;
        color: #1d1d1f;
        letter-spacing: -0.5px;
    }

    .connection-status {
        display: flex;
        align-items: center;
    }

    .status-indicator {
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 8px 16px;
        border-radius: 20px;
        font-size: 14px;
        font-weight: 500;
        backdrop-filter: blur(20px);
        border: 1px solid rgba(0, 0, 0, 0.08);
    }

    .status-indicator.connected {
        background: rgba(52, 199, 89, 0.12);
        color: #28a745;
        border-color: rgba(52, 199, 89, 0.2);
    }

    .status-indicator.disconnected {
        background: rgba(255, 59, 48, 0.12);
        color: #ff3b30;
        border-color: rgba(255, 59, 48, 0.2);
    }

    .status-dot {
        width: 8px;
        height: 8px;
        border-radius: 50%;
        background: currentColor;
        animation: pulse 2s infinite;
    }

    @keyframes pulse {
        0%, 100% { opacity: 1; }
        50% { opacity: 0.5; }
    }

    /* Glass Cards */
    .glass-card {
        background: #fbfbff;
        backdrop-filter: blur(30px) saturate(180%);
        border: 1px solid rgba(0, 0, 0, 0.08);
        border-radius: 20px;
        padding: 24px;
        margin-bottom: 24px;
        box-shadow: 
            0 4px 16px rgba(0, 0, 0, 0.04),
            0 1px 3px rgba(0, 0, 0, 0.08);
    }

    .card-header {
        margin-bottom: 24px;
    }

    .card-header h2 {
        font-size: 20px;
        font-weight: 600;
        margin: 0 0 4px 0;
        color: #1d1d1f;
        letter-spacing: -0.3px;
    }

    .card-header p {
        margin: 0;
        color: #86868b;
        font-size: 14px;
    }

    /* Connection Layout */
    .connection-layout {
        display: flex;
        align-items: center;
        gap: 24px;
        flex-wrap: wrap;
    }

    .connection-action {
        flex-shrink: 0;
    }
    
    .autoconnect-status {
        display: flex;
        align-items: center;
        gap: 12px;
        padding: 8px 16px;
        background: rgba(0, 0, 0, 0.03);
        border: 1px solid rgba(0, 0, 0, 0.08);
        border-radius: 12px;
        font-size: 14px;
        color: #86868b;
        font-weight: 500;
    }
    
    .spinner-small {
        width: 16px;
        height: 16px;
        border: 2px solid rgba(0, 0, 0, 0.1);
        border-top: 2px solid #007aff;
        border-radius: 50%;
        animation: spin 1s linear infinite;
    }

    /* Compact Device Info */
    .device-info-compact {
        display: flex;
        gap: 12px;
        flex-wrap: wrap;
        flex: 1;
    }

    .info-chip {
        display: flex;
        align-items: center;
        gap: 6px;
        padding: 8px 12px;
        background: rgba(0, 0, 0, 0.03);
        border: 1px solid rgba(0, 0, 0, 0.08);
        border-radius: 10px;
        font-size: 13px;
    }

    .info-chip .info-label {
        color: #86868b;
        font-weight: 500;
    }

    .info-chip .info-value {
        color: #1d1d1f;
        font-weight: 600;
    }

    /* Error Banner */
    .error-banner {
        background: rgba(255, 59, 48, 0.08);
        border: 1px solid rgba(255, 59, 48, 0.15);
        border-radius: 14px;
        padding: 16px;
        margin-bottom: 24px;
        display: flex;
        justify-content: space-between;
        align-items: center;
        backdrop-filter: blur(20px);
    }

    .error-content {
        display: flex;
        align-items: center;
        gap: 12px;
        color: #ff3b30;
        font-size: 14px;
    }

    .error-close {
        background: none;
        border: none;
        color: #ff3b30;
        cursor: pointer;
        padding: 4px;
        border-radius: 8px;
        transition: background-color 0.2s;
    }

    .error-close:hover {
        background: rgba(255, 59, 48, 0.1);
    }

    /* Buttons */
    .primary-button {
        background: #007aff;
        color: white;
        border: none;
        border-radius: 12px;
        padding: 12px 20px;
        font-size: 15px;
        font-weight: 600;
        cursor: pointer;
        display: flex;
        align-items: center;
        gap: 8px;
        transition: all 0.2s;
        box-shadow: 0 2px 8px rgba(0, 122, 255, 0.15);
        letter-spacing: -0.2px;
    }

    .primary-button:hover:not(:disabled) {
        background: #0051d5;
        box-shadow: 0 4px 12px rgba(0, 122, 255, 0.25);
    }

    .primary-button:disabled {
        opacity: 0.4;
        cursor: not-allowed;
        transform: none;
    }

    .secondary-button {
        background: rgba(0, 0, 0, 0.04);
        color: #007aff;
        border: 1px solid rgba(0, 0, 0, 0.08);
        border-radius: 12px;
        padding: 12px 20px;
        font-size: 15px;
        font-weight: 600;
        cursor: pointer;
        transition: all 0.2s;
        backdrop-filter: blur(20px);
        letter-spacing: -0.2px;
    }

    .secondary-button:hover:not(:disabled) {
        background: rgba(0, 0, 0, 0.08);
        border-color: rgba(0, 0, 0, 0.12);
    }

    .danger-button {
        background: #ff3b30;
        color: white;
        border: none;
        border-radius: 12px;
        padding: 12px 20px;
        font-size: 15px;
        font-weight: 600;
        cursor: pointer;
        display: flex;
        align-items: center;
        gap: 8px;
        transition: all 0.2s;
        box-shadow: 0 2px 8px rgba(255, 59, 48, 0.15);
        letter-spacing: -0.2px;
    }

    .danger-button:hover:not(:disabled) {
        background: #d32f2f;
        box-shadow: 0 4px 12px rgba(255, 59, 48, 0.25);
    }

    /* Spinner */
    .spinner {
        width: 16px;
        height: 16px;
        border: 2px solid rgba(255, 255, 255, 0.3);
        border-top: 2px solid currentColor;
        border-radius: 50%;
        animation: spin 1s linear infinite;
    }

    .spinner-large {
        width: 48px;
        height: 48px;
        border: 3px solid rgba(0, 0, 0, 0.1);
        border-top: 3px solid #007aff;
        border-radius: 50%;
        animation: spin 1s linear infinite;
        margin: 0 auto 16px;
    }

    @keyframes spin {
        to { transform: rotate(360deg); }
    }

    /* Loading State */
    .loading-state {
        text-align: center;
        padding: 64px 24px;
        color: #ffffff;
    }

    .loading-state h3 {
        margin: 0 0 8px 0;
        font-size: 18px;
        font-weight: 600;
        color: #1d1d1f;
        letter-spacing: -0.3px;
    }

    .loading-state p {
        margin: 0;
        font-size: 14px;
        color: #86868b;
    }



    /* Device selection cards */
    .device-card-grid {
        display: flex;
        gap: 16px;
        flex-wrap: wrap;
        margin-bottom: 24px;
    }

    .device-card {
        background: #ffffff;
        border: 1px solid rgba(0, 0, 0, 0.08);
        border-radius: 16px;
        padding: 16px;
        width: 200px;
        display: flex;
        flex-direction: column;
        gap: 12px;
        align-items: flex-start;
        color: #1d1d1f;
        cursor: pointer;
        transition: transform 0.2s ease, box-shadow 0.2s ease, border-color 0.2s ease;
        background-origin: border-box;
        outline: none;
        box-shadow: 0 2px 8px rgba(0, 0, 0, 0.04);
    }

    .device-card:hover {
        transform: translateY(-2px);
        box-shadow: 0 8px 20px rgba(0, 0, 0, 0.08);
    }

    .device-card.active {
        border-color: rgba(0, 122, 255, 0.6);
        box-shadow: 0 8px 24px rgba(0, 122, 255, 0.15);
        background: rgba(0, 122, 255, 0.04);
    }

    .device-card:focus-visible {
        border-color: #007aff;
        box-shadow: 0 0 0 4px rgba(0, 122, 255, 0.2);
    }

    .device-card-image {
        width: 100%;
        height: 96px;
        border-radius: 12px;
        background: rgba(0, 0, 0, 0.04);
        display: flex;
        align-items: center;
        justify-content: center;
        color: #86868b;
        font-size: 11px;
        text-transform: uppercase;
        letter-spacing: 1.2px;
        font-weight: 600;
    }

    .master-placeholder {
        background: linear-gradient(135deg, rgba(0, 122, 255, 0.12), rgba(88, 86, 214, 0.12));
        position: relative;
        color: #007aff;
    }

    .master-placeholder::after {
        content: 'MASTER';
    }

    .module-placeholder {
        background: linear-gradient(135deg, rgba(52, 199, 89, 0.12), rgba(48, 209, 88, 0.12));
        position: relative;
        color: #34c759;
    }

    .module-placeholder::after {
        content: 'MODULE';
    }

    .device-card-content {
        display: flex;
        flex-direction: column;
        gap: 4px;
        align-items: flex-start;
    }

    .device-card-title {
        font-weight: 600;
        font-size: 16px;
    }

    .device-card-subtitle {
        font-size: 12px;
        color: #86868b;
    }

    /* Keymap */
    .keymap-container {
        position: relative;
        display: flex;
        justify-content: center;
        padding: 8px;
    }

    .keymap {
        position: relative;
        display: flex;
        flex-direction: column;
        gap: 4px;
        transition: opacity 0.2s ease;
    }

    .keymap.loading {
        opacity: 0.35;
    }

    .keymap-row {
        display: flex;
        gap: 4px;
    }

    .keymap-cell {
        position: relative;
        width: 64px;
        height: 64px;
        display: flex;
        align-items: center;
        justify-content: center;
    }

    .keymap-card {
        position: relative;
        isolation: isolate;
    }

    .keymap-card.has-active-menu::before {
        content: '';
        position: absolute;
        inset: 0;
        backdrop-filter: blur(3px);
        background: rgba(251, 251, 255, 0.3);
        border-radius: 20px;
        z-index: 50;
        pointer-events: none;
    }

    .key {
        width: 100%;
        height: 100%;
        padding: 8px;
        background: #ffffff;
        border: 1px solid #d1d1d6;
        border-radius: 12px;
        cursor: pointer;
        transition: all 0.2s;
        display: flex;
        flex-direction: column;
        justify-content: center;
        align-items: center;
        position: relative;
        overflow: hidden;
        box-shadow: 
            0 1px 3px rgba(0, 0, 0, 0.04),
            0 0 0 1px rgba(0, 0, 0, 0.02);
        color: #000000;
    }

    .key::before {
        content: '';
        position: absolute;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        background: linear-gradient(135deg, rgba(0, 122, 255, 0.08) 0%, rgba(88, 86, 214, 0.08) 100%);
        opacity: 0;
        transition: opacity 0.2s;
    }

    .key:hover::before {
        opacity: 1;
    }

    .key:hover {
        border-color: rgba(0, 122, 255, 0.5);
        transform: translateY(-1px);
        box-shadow: 
            0 4px 12px rgba(0, 122, 255, 0.12),
            0 0 0 1px rgba(0, 122, 255, 0.2);
    }

    .key-label {
        font-size: 11px;
        line-height: 1.2;
        text-align: center;
        position: relative;
        z-index: 1;
        color: #000000;
        font-weight: 500;
    }

    .encoder-key {
        border-radius: 999px;
        width: 56px;
        height: 56px;
        padding: 12px;
        border: 1px solid #d1d1d6;
        background: #ffffff;
        box-shadow: 
            0 2px 8px rgba(0, 0, 0, 0.04),
            0 0 0 1px rgba(0, 0, 0, 0.02);
        gap: 6px;
    }

    .encoder-key:hover {
        border-color: rgba(0, 122, 255, 0.5);
        box-shadow: 
            0 4px 12px rgba(0, 122, 255, 0.12),
            0 0 0 1px rgba(0, 122, 255, 0.2);
    }

    .encoder-key.menu-open {
        border-color: rgba(0, 122, 255, 0.6);
        box-shadow: 
            0 6px 16px rgba(0, 122, 255, 0.18),
            0 0 0 1px rgba(0, 122, 255, 0.3);
    }

    .encoder-key .key-label {
        font-size: 10px;
        color: #000000;
        font-weight: 500;
    }

    .keymap-overlay {
        position: absolute;
        inset: 8px;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 10px;
        background: rgba(255, 255, 255, 0.9);
        border-radius: 20px;
        border: 1px solid rgba(0, 0, 0, 0.08);
        color: #1d1d1f;
        font-size: 13px;
        font-weight: 500;
        z-index: 5;
        pointer-events: none;
        backdrop-filter: blur(20px);
    }

    .encoder-menu {
        position: absolute;
        inset: 0;
        display: flex;
        align-items: center;
        justify-content: center;
        opacity: 0;
        pointer-events: none;
        transition: opacity 0.18s ease;
        z-index: 100;
    }

    .encoder-menu.active {
        opacity: 1;
        pointer-events: auto;
    }

    .encoder-action {
        position: absolute;
        top: 50%;
        left: 50%;
        width: 80px;
        height: 80px;
        padding: 12px;
        border-radius: 16px;
        border: 1px solid #d1d1d6;
        background: #ffffff;
        color: #000000;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 6px;
        transform: translate(-50%, -50%) scale(0.45);
        opacity: 0;
        transition: transform 0.28s cubic-bezier(0.24, 0.8, 0.25, 1), opacity 0.22s ease;
        box-shadow: 
            0 4px 16px rgba(0, 0, 0, 0.1),
            0 2px 4px rgba(0, 0, 0, 0.06);
        pointer-events: none;
    }

    .encoder-menu.active .encoder-action {
        opacity: 1;
        pointer-events: auto;
        transform: translate(calc(-50% + var(--dx, 0px)), calc(-50% + var(--dy, -48px))) scale(1);
    }

    .encoder-action.ccw {
        --dx: -95px;
        --dy: -70px;
    }

    .encoder-action.press {
        --dx: 0px;
        --dy: -100px;
        background: #ffffff;
        color: #000000;
        border-color: #d1d1d6;
    }

    .encoder-action.cw {
        --dx: 95px;
        --dy: -70px;
    }

    .encoder-menu.active .encoder-action.ccw { transition-delay: 0.02s; }
    .encoder-menu.active .encoder-action.press { transition-delay: 0.08s; }
    .encoder-menu.active .encoder-action.cw { transition-delay: 0.14s; }

    .encoder-action:hover {
        transform: translate(calc(-50% + var(--dx, 0px)), calc(-50% + var(--dy, -48px))) scale(1.05);
        box-shadow: 
            0 8px 24px rgba(0, 0, 0, 0.15),
            0 4px 8px rgba(0, 0, 0, 0.08);
        border-color: rgba(0, 122, 255, 0.5);
    }

    .encoder-action-header {
        display: flex;
        align-items: center;
        gap: 6px;
        margin-bottom: 2px;
    }

    .encoder-action-icon {
        font-size: 20px;
        line-height: 1;
        color: #007aff;
    }

    .encoder-action-label {
        font-size: 10px;
        font-weight: 600;
        letter-spacing: 0.08em;
        text-transform: uppercase;
        color: #86868b;
        white-space: nowrap;
    }

    .encoder-action-preview {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 2px;
        font-size: 11px;
        line-height: 1.2;
        color: #000000;
        font-weight: 600;
        text-align: center;
        min-height: 22px;
    }

    .encoder-action-preview .preview-line {
        white-space: nowrap;
    }

    .keymap-container .spinner-small {
        color: #9ca3af;
    }

    .spinner-small {
        width: 20px;
        height: 20px;
        border-width: 2px;
    }

    /* Keep key cells square and multiline labels compact */
    .key { display:flex; align-items:center; justify-content:center; text-align:center; }
    .preview-compact { display:flex; flex-direction:column; gap:2px; align-items:center; }
    .preview-line { font-size:11px; line-height:1; }
    .muted { font-size:12px; color:#a0a0a0; margin-top:6px; }

    /* Encoder value lines fit in one row when possible */
    .value-line { display:inline-block; font-size:13px; line-height:1.3; }

    /* Configuration */
    .config-actions {
        display: flex;
        gap: 16px;
        margin-bottom: 32px;
        flex-wrap: wrap;
    }


    /* Empty State */
    .empty-state {
        text-align: center;
        padding: 48px 24px;
        color: #86868b;
    }

    .empty-state svg {
        margin-bottom: 16px;
        opacity: 0.3;
    }

    .empty-state h3 {
        margin: 0 0 8px 0;
        font-size: 18px;
        font-weight: 600;
        color: #1d1d1f;
        letter-spacing: -0.3px;
    }

    .empty-state p {
        margin: 0;
        font-size: 14px;
    }

    /* Modal */
    .modal-overlay {
        position: fixed;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        background: rgba(0, 0, 0, 0.4);
        backdrop-filter: blur(10px);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 1000;
        padding: 24px;
    }

    .modal-content {
        background: rgba(255, 255, 255, 0.95);
        backdrop-filter: blur(30px) saturate(180%);
        border: 1px solid rgba(0, 0, 0, 0.08);
        border-radius: 20px;
        width: 100%;
        max-width: 700px;
        max-height: 80vh;
        display: flex;
        flex-direction: column;
        box-shadow: 
            0 20px 60px rgba(0, 0, 0, 0.15),
            0 8px 24px rgba(0, 0, 0, 0.08);
    }

    .modal-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 24px;
        border-bottom: 1px solid rgba(0, 0, 0, 0.08);
    }

    .modal-header h3 {
        margin: 0;
        font-size: 18px;
        font-weight: 600;
        color: #1d1d1f;
        letter-spacing: -0.3px;
    }

    .modal-close {
        background: none;
        border: none;
        color: #86868b;
        cursor: pointer;
        padding: 4px;
        border-radius: 8px;
        transition: all 0.2s;
    }

    .modal-close:hover {
        background: rgba(0, 0, 0, 0.05);
        color: #1d1d1f;
    }

    .modal-tabs {
        display: flex;
        gap: 8px;
        padding: 16px 24px 0;
        border-bottom: 1px solid rgba(0, 0, 0, 0.08);
    }

    .modal-tab {
        background: none;
        border: none;
        padding: 10px 20px;
        border-radius: 10px 10px 0 0;
        cursor: pointer;
        font-size: 14px;
        font-weight: 500;
        color: #86868b;
        transition: all 0.2s;
    }

    .modal-tab:hover {
        color: #1d1d1f;
        background: rgba(0, 0, 0, 0.03);
    }

    .modal-tab.active {
        background: rgba(0, 122, 255, 0.08);
        color: #007aff;
    }

    .modal-body {
        padding: 24px;
        overflow-y: auto;
        flex: 1;
    }

    .keycode-selector {
        display: flex;
        flex-direction: column;
        gap: 24px;
    }

    .keycode-category {
        display: flex;
        flex-direction: column;
        gap: 12px;
    }

    .category-title {
        font-size: 13px;
        font-weight: 600;
        color: #86868b;
        text-transform: uppercase;
        letter-spacing: 0.8px;
        margin: 0;
    }

    .keycode-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
        gap: 8px;
    }

    .keycode-option {
        padding: 12px;
        background: rgba(0, 0, 0, 0.03);
        border: 1px solid rgba(0, 0, 0, 0.08);
        border-radius: 10px;
        cursor: pointer;
        font-size: 13px;
        color: #1d1d1f;
        transition: all 0.2s;
        text-align: center;
        font-weight: 500;
    }

    .keycode-option:hover {
        background: rgba(0, 0, 0, 0.05);
        border-color: rgba(0, 122, 255, 0.3);
    }

    .keycode-option.selected {
        background: rgba(0, 122, 255, 0.12);
        border-color: #007aff;
        color: #007aff;
        box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.15);
    }

    .midi-config {
        display: flex;
        flex-direction: column;
        gap: 20px;
    }

    .form-group {
        display: flex;
        flex-direction: column;
        gap: 8px;
    }

    .form-group label {
        font-size: 13px;
        font-weight: 500;
        color: #a0a0a0;
    }

    .form-group select,
    .form-group input {
        padding: 12px;
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.2);
        border-radius: 8px;
        color: #ffffff;
        font-size: 14px;
    }

    .form-group select:focus,
    .form-group input:focus {
        outline: none;
        border-color: #667eea;
        box-shadow: 0 0 0 2px rgba(102, 126, 234, 0.2);
    }

    .form-row {
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
        gap: 12px;
    }

    .midi-preview {
        padding: 16px;
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 8px;
        display: flex;
        align-items: center;
        gap: 12px;
    }


    .modal-footer {
        display: flex;
        justify-content: flex-end;
        gap: 12px;
        padding: 24px;
        border-top: 1px solid rgba(255, 255, 255, 0.1);
    }

    /* Save Popup */
    .save-popup {
        position: fixed;
        bottom: 24px;
        left: 50%;
        transform: translateX(-50%);
        z-index: 999;
        animation: slideUp 0.3s ease-out;
    }

    @keyframes slideUp {
        from {
            transform: translateX(-50%) translateY(100px);
            opacity: 0;
        }
        to {
            transform: translateX(-50%) translateY(0);
            opacity: 1;
        }
    }

    .save-popup-content {
        background: rgba(20, 20, 20, 0.95);
        backdrop-filter: blur(20px);
        border: 1px solid rgba(255, 255, 255, 0.2);
        border-radius: 16px;
        padding: 16px 24px;
        display: flex;
        align-items: center;
        gap: 24px;
        box-shadow: 0 12px 40px rgba(0, 0, 0, 0.5);
    }

    .save-popup-text {
        display: flex;
        align-items: center;
        gap: 12px;
        color: #ffffff;
        font-size: 14px;
        font-weight: 500;
    }

    .save-popup-text svg {
        color: #eab308;
    }

    .save-popup-actions {
        display: flex;
        gap: 12px;
    }

    .popup-button {
        padding: 10px 20px;
        border-radius: 8px;
        font-size: 14px;
        font-weight: 500;
        cursor: pointer;
        transition: all 0.2s;
        border: none;
    }

    .popup-button.discard {
        background: rgba(255, 255, 255, 0.1);
        color: #ffffff;
        border: 1px solid rgba(255, 255, 255, 0.2);
    }

    .popup-button.discard:hover {
        background: rgba(255, 255, 255, 0.15);
    }

    .popup-button.save {
        background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
        color: white;
        box-shadow: 0 4px 12px rgba(102, 126, 234, 0.3);
    }

    .popup-button.save:hover {
        transform: translateY(-1px);
        box-shadow: 0 6px 20px rgba(102, 126, 234, 0.4);
    }

    /* Responsive */
    @media (max-width: 768px) {
        .container {
            padding: 16px;
        }

        .header-content {
            flex-direction: column;
            gap: 16px;
            align-items: flex-start;
        }

        .tab-nav {
            flex-direction: column;
        }

        .config-actions {
            flex-direction: column;
        }

        .modal-content {
            max-width: 100%;
        }

        .keycode-grid {
            grid-template-columns: repeat(auto-fill, minmax(100px, 1fr));
        }

        .save-popup-content {
            flex-direction: column;
            gap: 16px;
        }
    }
</style>
