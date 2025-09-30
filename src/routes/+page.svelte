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
    let encoders = $state([]);
    let keycodes = {};
    
    // UI state
    let selectedTab = $state('keymap');
    let selectedKey = $state(null);
    let selectedEncoder = null;
    let originalKeymap = [];
    let originalEncoders = [];
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

    onMount(async () => {
        console.log('=== FRONTEND: App started ===');
        await loadKeycodes();
        console.log('=== FRONTEND: App ready ===');
    });

    // Load keycodes from backend
    async function loadKeycodes() {
        try {
            keycodes = await invoke('get_keycodes');
        } catch (e) {
            console.error('Failed to load keycodes:', e);
        }
    }

    function storeOriginalData() {
        originalKeymap = JSON.parse(JSON.stringify(keymap));
        originalEncoders = JSON.parse(JSON.stringify(encoders));
        hasChanges = false;
        showSavePopup = false;
    }

    function checkForChanges() {
        const keymapChanged = JSON.stringify(keymap) !== JSON.stringify(originalKeymap);
        const encodersChanged = JSON.stringify(encoders) !== JSON.stringify(originalEncoders);
        hasChanges = keymapChanged || encodersChanged;
        showSavePopup = hasChanges;
    }

    async function connectDevice() {
        loading = true;
        error = null;
        
        try {
            console.log('Connecting to device...');
            const result = await invoke('simple_connect');
            
            isConnected = true;
            connectionStatus = 'connected';
            deviceInfo = result.device_info;
            keymap = result.keymap || [];
            encoders = result.encoders || [];
            dataLoaded = true;
            
            // Load I2C devices and their keymaps
            loadingI2CDevices = true;
            try {
                i2cDevices = await invoke('get_i2c_devices');
                console.log('I2C devices loaded:', i2cDevices);
                
                // Load slave keymaps for connected devices
                for (const device of i2cDevices) {
                    if (device.status === 1) { // Connected
                        try {
                            const slaveKeymap = await invoke('get_full_slave_keymap', { slaveAddr: device.address });
                            slaveKeymaps[device.address] = slaveKeymap;
                            console.log(`Loaded keymap for slave device ${device.address}`);
                        } catch (e) {
                            console.error(`Failed to load keymap for slave device ${device.address}:`, e);
                        }
                    }
                }
            } catch (e) {
                console.error('Failed to load I2C devices:', e);
            } finally {
                loadingI2CDevices = false;
            }
            
            storeOriginalData();
            
            console.log('Connected successfully:', result);
        } catch (e) {
            error = `Failed to connect: ${e}`;
            console.error('Connection failed:', e);
            
            isConnected = false;
            connectionStatus = 'disconnected';
            deviceInfo = null;
            keymap = [];
            encoders = [];
            i2cDevices = [];
            slaveKeymaps = {};
            dataLoaded = false;
        }
        
        loading = false;
    }

    async function disconnectDevice() {
        loading = true;
        error = null;
        
        try {
            console.log('Disconnecting from device...');
            await invoke('simple_disconnect');
            
            isConnected = false;
            connectionStatus = 'disconnected';
            deviceInfo = null;
            keymap = [];
            encoders = [];
            selectedKey = null;
            selectedEncoder = null;
            originalKeymap = [];
            originalEncoders = [];
            dataLoaded = false;
            hasChanges = false;
            showSavePopup = false;
            
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
                const slaveAddr = parseInt(selectedDevice);
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
    
    // Get the current active keymap based on selected device
    function getCurrentKeymap() {
        if (selectedDevice === 'main') {
            return keymap;
        } else {
            const slaveAddr = parseInt(selectedDevice);
            return slaveKeymaps[slaveAddr] || [];
        }
    }

    async function updateEncoder(encoderId, ccwKeycode, cwKeycode) {
        try {
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
                checkForChanges();
            }
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
                    <h1 class="title">Keyboard Configurator</h1>
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

    <div class="glass-card connection-card">
            <div class="connection-layout">
                <div class="connection-action">
                    {#if !isConnected}
                        <button class="primary-button" onclick={connectDevice} disabled={loading}>
                            {#if loading}
                                <div class="spinner"></div>
                                <span>Connecting...</span>
                            {:else}
                                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                                    <path d="M13 2L3 14H12L11 22L21 10H12L13 2Z" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
                                </svg>
                                <span>Connect Device</span>
                            {/if}
                        </button>
                    {:else}
                        <button class="secondary-button" onclick={disconnectDevice} disabled={loading}>
                            {#if loading}
                                <div class="spinner"></div>
                                <span>Disconnecting...</span>
                            {:else}
                                <span>Disconnect Device</span>
                            {/if}
                        </button>
                    {/if}
                </div>

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

        <nav class="tab-nav">
            <button 
                class="tab-button"
                class:active={selectedTab === 'keymap'}
                onclick={() => selectedTab = 'keymap'}
                disabled={!isConnected}
                aria-label="Keymap Tab"
            >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                    <rect x="2" y="6" width="20" height="12" rx="2" stroke="currentColor" stroke-width="2"/>
                    <path d="M6 10H8" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
                    <path d="M10 10H12" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
                    <path d="M14 10H16" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
                    <path d="M6 14H10" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
                    <path d="M12 14H16" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
                </svg>
                <span>Keymap</span>
            </button>
            <button 
                class="tab-button"
                class:active={selectedTab === 'encoders'}
                onclick={() => selectedTab = 'encoders'}
                disabled={!isConnected}
                aria-label="Encoders Tab"
            >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                    <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="2"/>
                    <path d="M8 12L12 8L16 12L12 16L8 12Z" stroke="currentColor" stroke-width="2"/>
                </svg>
                <span>Encoders</span>
            </button>
           
        </nav>

        {#if isConnected}
            <div class="device-selector-container">
                <label for="device-selector">Select Device:</label>
                <select 
                    id="device-selector" 
                    bind:value={selectedDevice}
                    class="device-selector"
                >
                    <option value="main">Main Device ({deviceInfo?.device_name || 'Unknown'})</option>
                    {#each i2cDevices as device}
                        {#if device.status === 1} <!-- Only show connected devices -->
                            <option value={device.address}>
                                Slave: {device.name || `Device at 0x${device.address.toString(16)}`}
                            </option>
                        {/if}
                    {/each}
                </select>
            </div>
        {/if}

        <div class="tab-content">
            {#if selectedTab === 'keymap'}
                <div class="glass-card keymap-card">
                    <div class="card-header">
                        <h2>Keymap Configuration</h2>
                        <p>Click on any key to customize its function</p>
                    </div>
                    
                    {#if loadingKeymap || (selectedDevice !== 'main' && loadingI2CDevices)}
                        <div class="loading-state">
                            <div class="spinner-large"></div>
                            <h3>Loading Keymap...</h3>
                            <p>Fetching key configuration from device</p>
                        </div>
                    {:else if (selectedDevice === 'main' && keymap.length > 0) || (selectedDevice !== 'main' && slaveKeymaps[selectedDevice] && slaveKeymaps[selectedDevice].length > 0)}
                        <!-- keys open modal -->
                        <div class="keymap-container">
                            <div class="keymap">
                                {#each getCurrentKeymap() as row, rowIndex}
                                    <div class="keymap-row">
                                        {#each row as key, colIndex}
                                            <button 
                                                class="key"
                                                onclick={() => openKeyModal(rowIndex, colIndex, key.keycode)}
                                                aria-label={`Key R${rowIndex}C${colIndex}`}>
                                                {#each formatKeyLabel(key.keycode) as line}
                                                   <div class="key-label">{line}</div>
                                                {/each}
                                            </button>
                                        {/each}
                                    </div>
                                {/each}
                            </div>
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

            {#if selectedTab === 'encoders'}
                <div class="glass-card encoders-card">
                    <div class="card-header">
                        <h2>Encoder Configuration</h2>
                        <p>Configure rotary encoder actions for clockwise and counter-clockwise rotation</p>
                    </div>
                    
                    {#if loadingEncoders}
                        <div class="loading-state">
                            <div class="spinner-large"></div>
                            <h3>Loading Encoders...</h3>
                            <p>Fetching encoder configuration from device</p>
                        </div>
                    {:else if encoders.length > 0}
                         <!-- encoder list (open modal to edit) -->
                        <div class="encoders-grid">
                            {#each encoders as encoder}
                                <div class="encoder-item">
                                    <div class="encoder-header">
                                        <h4>Encoder {encoder.encoder_id}</h4>
                                    </div>
                                    <div class="encoder-actions">
                                        <button 
                                            class="encoder-direction-btn ccw"
                                            onclick={() => openEncoderModal(encoder, 'ccw')}
                                        >
                                            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                                                <path d="M15 18L9 12L15 6" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
                                            </svg>
                                            <div class="direction-info">
                                                <span class="direction-label">Counter-Clockwise</span>
                                                <div class="direction-value">
                                                    {#each formatKeyLabel(encoder.ccw_keycode) as line}
                                                        <div class="value-line">{line}</div>
                                                    {/each}
                                                </div>
                                            </div>
                                        </button>
                                        <button 
                                            class="encoder-direction-btn cw"
                                            onclick={() => openEncoderModal(encoder, 'cw')}
                                        >
                                            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                                                <path d="M9 18L15 12L9 6" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
                                            </svg>
                                            <div class="direction-info">
                                                <span class="direction-label">Clockwise</span>
                                                <div class="direction-value">
                                                    {#each formatKeyLabel(encoder.cw_keycode) as line}
                                                        <div class="value-line">{line}</div>
                                                    {/each}
                                                </div>
                                            </div>
                                        </button>
                                    </div>
                                </div>
                            {/each}
                        </div>
                    {:else}
                        <div class="empty-state">
                            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                                <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="2"/>
                                <path d="M8 12L12 8L16 12L12 16L8 12Z" stroke="currentColor" stroke-width="2"/>
                            </svg>
                            <h3>No Encoders Available</h3>
                            <p>Connect to a device to view and configure encoders</p>
                        </div>
                    {/if}
                </div>
            {/if}

            {#if selectedTab === 'config'}
                <div class="glass-card config-card">
                    <div class="card-header">
                        <h2>Configuration Management</h2>
                        <p>Save, load, or reset your keyboard configuration</p>
                    </div>
                    
                    <div class="config-actions">
                        <button class="primary-button" onclick={saveConfig} disabled={loading}>
                            {#if loading}
                                <div class="spinner"></div>
                            {:else}
                                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                                    <path d="M19 21H5A2 2 0 0 1 3 19V5A2 2 0 0 1 5 3H16L21 8V19A2 2 0 0 1 19 21Z" stroke="currentColor" stroke-width="2"/>
                                    <polyline points="17,21 17,13 7,13 7,21" stroke="currentColor" stroke-width="2"/>
                                    <polyline points="7,3 7,8 15,8" stroke="currentColor" stroke-width="2"/>
                                </svg>
                            {/if}
                            <span>{loading ? 'Saving...' : 'Save to EEPROM'}</span>
                        </button>
                        
                        <button class="secondary-button" onclick={loadConfig} disabled={loading}>
                            {#if loading}
                                <div class="spinner"></div>
                            {:else}
                                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                                    <path d="M14 2H6A2 2 0 0 0 4 4V20A2 2 0 0 0 6 22H18A2 2 0 0 0 20 20V8L14 2Z" stroke="currentColor" stroke-width="2"/>
                                    <polyline points="14,2 14,8 20,8" stroke="currentColor" stroke-width="2"/>
                                    <line x1="16" y1="13" x2="8" y2="13" stroke="currentColor" stroke-width="2"/>
                                    <line x1="16" y1="17" x2="8" y2="17" stroke="currentColor" stroke-width="2"/>
                                    <polyline points="10,9 9,9 8,9" stroke="currentColor" stroke-width="2"/>
                                </svg>
                            {/if}
                            <span>{loading ? 'Loading...' : 'Load from EEPROM'}</span>
                        </button>
                        
                        <button class="danger-button" onclick={resetConfig} disabled={loading}>
                            {#if loading}
                                <div class="spinner"></div>
                            {:else}
                                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                                    <polyline points="1,4 1,10 7,10" stroke="currentColor" stroke-width="2"/>
                                    <path d="M3.51 15A9 9 0 1 0 6 5.64L1 10" stroke="currentColor" stroke-width="2"/>
                                </svg>
                            {/if}
                            <span>{loading ? 'Resetting...' : 'Reset to Defaults'}</span>
                        </button>
                    </div>
                    
                 
                </div>
            {/if}
        </div>
    </div>

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
        font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
        background: #0a0a0a;
        color: #ffffff;
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
            radial-gradient(circle at 20% 80%, rgba(120, 119, 198, 0.3) 0%, transparent 50%),
            radial-gradient(circle at 80% 20%, rgba(255, 119, 198, 0.15) 0%, transparent 50%),
            radial-gradient(circle at 40% 40%, rgba(120, 219, 255, 0.1) 0%, transparent 50%),
            linear-gradient(135deg, #0a0a0a 0%, #111111 100%);
        z-index: -1;
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
        font-weight: 700;
        margin: 0;
        background: linear-gradient(135deg, #ffffff 0%, #a0a0a0 100%);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
        background-clip: text;
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
        backdrop-filter: blur(10px);
        border: 1px solid rgba(255, 255, 255, 0.1);
    }

    .status-indicator.connected {
        background: rgba(34, 197, 94, 0.1);
        color: #22c55e;
        border-color: rgba(34, 197, 94, 0.2);
    }

    .status-indicator.disconnected {
        background: rgba(239, 68, 68, 0.1);
        color: #ef4444;
        border-color: rgba(239, 68, 68, 0.2);
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
        background: rgba(255, 255, 255, 0.05);
        backdrop-filter: blur(20px);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 16px;
        padding: 24px;
        margin-bottom: 24px;
        box-shadow: 
            0 8px 32px rgba(0, 0, 0, 0.3),
            inset 0 1px 0 rgba(255, 255, 255, 0.1);
    }

    .card-header {
        margin-bottom: 24px;
    }

    .card-header h2 {
        font-size: 20px;
        font-weight: 600;
        margin: 0 0 4px 0;
        color: #ffffff;
    }

    .card-header p {
        margin: 0;
        color: #a0a0a0;
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
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 8px;
        font-size: 13px;
    }

    .info-chip .info-label {
        color: #a0a0a0;
        font-weight: 500;
    }

    .info-chip .info-value {
        color: #ffffff;
        font-weight: 600;
    }

    /* Error Banner */
    .error-banner {
        background: rgba(239, 68, 68, 0.1);
        border: 1px solid rgba(239, 68, 68, 0.2);
        border-radius: 12px;
        padding: 16px;
        margin-bottom: 24px;
        display: flex;
        justify-content: space-between;
        align-items: center;
        backdrop-filter: blur(10px);
    }

    .error-content {
        display: flex;
        align-items: center;
        gap: 12px;
        color: #ef4444;
        font-size: 14px;
    }

    .error-close {
        background: none;
        border: none;
        color: #ef4444;
        cursor: pointer;
        padding: 4px;
        border-radius: 4px;
        transition: background-color 0.2s;
    }

    .error-close:hover {
        background: rgba(239, 68, 68, 0.1);
    }

    /* Buttons */
    .primary-button {
        background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
        color: white;
        border: none;
        border-radius: 8px;
        padding: 12px 20px;
        font-size: 14px;
        font-weight: 500;
        cursor: pointer;
        display: flex;
        align-items: center;
        gap: 8px;
        transition: all 0.2s;
        box-shadow: 0 4px 12px rgba(102, 126, 234, 0.3);
    }

    .primary-button:hover:not(:disabled) {
        transform: translateY(-1px);
        box-shadow: 0 6px 20px rgba(102, 126, 234, 0.4);
    }

    .primary-button:disabled {
        opacity: 0.5;
        cursor: not-allowed;
        transform: none;
    }

    .secondary-button {
        background: rgba(255, 255, 255, 0.1);
        color: #ffffff;
        border: 1px solid rgba(255, 255, 255, 0.2);
        border-radius: 8px;
        padding: 12px 20px;
        font-size: 14px;
        font-weight: 500;
        cursor: pointer;
        transition: all 0.2s;
        backdrop-filter: blur(10px);
    }

    .secondary-button:hover:not(:disabled) {
        background: rgba(255, 255, 255, 0.15);
        border-color: rgba(255, 255, 255, 0.3);
    }

    .danger-button {
        background: linear-gradient(135deg, #ef4444 0%, #dc2626 100%);
        color: white;
        border: none;
        border-radius: 8px;
        padding: 12px 20px;
        font-size: 14px;
        font-weight: 500;
        cursor: pointer;
        display: flex;
        align-items: center;
        gap: 8px;
        transition: all 0.2s;
        box-shadow: 0 4px 12px rgba(239, 68, 68, 0.3);
    }

    .danger-button:hover:not(:disabled) {
        transform: translateY(-1px);
        box-shadow: 0 6px 20px rgba(239, 68, 68, 0.4);
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
        border: 3px solid rgba(255, 255, 255, 0.2);
        border-top: 3px solid #667eea;
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
        color: #ffffff;
    }

    .loading-state p {
        margin: 0;
        font-size: 14px;
        color: #a0a0a0;
    }

    /* Tab Navigation */
    .tab-nav {
        display: flex;
        gap: 4px;
        margin-bottom: 24px;
        background: rgba(255, 255, 255, 0.05);
        border-radius: 12px;
        padding: 4px;
        backdrop-filter: blur(10px);
        border: 1px solid rgba(255, 255, 255, 0.1);
    }

    .tab-button {
        background: none;
        border: none;
        padding: 12px 20px;
        border-radius: 8px;
        cursor: pointer;
        display: flex;
        align-items: center;
        gap: 8px;
        font-size: 14px;
        font-weight: 500;
        color: #a0a0a0;
        transition: all 0.2s;
        flex: 1;
        justify-content: center;
    }

    .tab-button:hover:not(:disabled) {
        color: #ffffff;
        background: rgba(255, 255, 255, 0.05);
    }

    .tab-button.active {
        background: rgba(255, 255, 255, 0.1);
        color: #ffffff;
        box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
    }

    .tab-button:disabled {
        opacity: 0.3;
        cursor: not-allowed;
    }

    /* Keymap */
    .keymap-container {
        display: flex;
        justify-content: center;
    }

    .keymap {
        display: flex;
        flex-direction: column;
        gap: 4px;
    }

    .keymap-row {
        display: flex;
        gap: 4px;
    }

    .key {
        width: 64px;
        height: 64px;
        padding: 8px;
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 8px;
        cursor: pointer;
        transition: all 0.2s;
        display: flex;
        flex-direction: column;
        justify-content: center;
        align-items: center;
        backdrop-filter: blur(10px);
        position: relative;
        overflow: hidden;
    }

    .key::before {
        content: '';
        position: absolute;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        background: linear-gradient(135deg, rgba(102, 126, 234, 0.1) 0%, rgba(118, 75, 162, 0.1) 100%);
        opacity: 0;
        transition: opacity 0.2s;
    }

    .key:hover::before {
        opacity: 1;
    }

    .key:hover {
        border-color: rgba(102, 126, 234, 0.5);
        transform: translateY(-2px);
        box-shadow: 0 4px 12px rgba(102, 126, 234, 0.2);
    }

    .key-label {
        font-size: 11px;
        line-height: 1.2;
        text-align: center;
        position: relative;
        z-index: 1;
    }

    /* Keep key cells square and multiline labels compact */
    .key { display:flex; align-items:center; justify-content:center; text-align:center; }
    .preview-compact { display:flex; flex-direction:column; gap:2px; align-items:center; }
    .preview-line { font-size:11px; line-height:1; }
    .muted { font-size:12px; color:#a0a0a0; margin-top:6px; }

    /* Encoder value lines fit in one row when possible */
    .value-line { display:inline-block; font-size:13px; }

    /* Encoders */
    .encoders-grid {
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
        gap: 20px;
    }

    .encoder-item {
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 12px;
        padding: 20px;
        backdrop-filter: blur(10px);
    }

    .encoder-header h4 {
        margin: 0 0 16px 0;
        font-size: 16px;
        font-weight: 600;
        color: #ffffff;
    }

    .encoder-actions {
        display: flex;
        flex-direction: column;
        gap: 12px;
    }

    .encoder-direction-btn {
        display: flex;
        align-items: center;
        gap: 12px;
        padding: 16px;
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 8px;
        cursor: pointer;
        transition: all 0.2s;
        color: #ffffff;
    }

    .encoder-direction-btn:hover {
        background: rgba(255, 255, 255, 0.1);
        border-color: rgba(102, 126, 234, 0.5);
        transform: translateX(4px);
    }

    .direction-info {
        flex: 1;
        text-align: left;
    }

    .direction-label {
        display: block;
        font-size: 12px;
        color: #a0a0a0;
        margin-bottom: 4px;
    }

    .direction-value {
        font-size: 14px;
        font-weight: 500;
        color: #ffffff;
    }

    .value-line {
        line-height: 1.3;
    }

    /* Configuration */
    .config-actions {
        display: flex;
        gap: 16px;
        margin-bottom: 32px;
        flex-wrap: wrap;
    }

    .config-info {
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
        gap: 16px;
    }

    .info-card {
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 8px;
        padding: 16px;
        backdrop-filter: blur(10px);
    }

    .info-card h4 {
        margin: 0 0 8px 0;
        font-size: 14px;
        font-weight: 600;
        color: #ffffff;
    }

    .info-card p {
        margin: 0;
        font-size: 13px;
        color: #a0a0a0;
        line-height: 1.4;
    }

    /* Empty State */
    .empty-state {
        text-align: center;
        padding: 48px 24px;
        color: #a0a0a0;
    }

    .empty-state svg {
        margin-bottom: 16px;
        opacity: 0.5;
    }

    .empty-state h3 {
        margin: 0 0 8px 0;
        font-size: 18px;
        font-weight: 600;
        color: #ffffff;
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
        background: rgba(0, 0, 0, 0.7);
        backdrop-filter: blur(4px);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 1000;
        padding: 24px;
    }

    .modal-content {
        background: rgba(20, 20, 20, 0.95);
        backdrop-filter: blur(20px);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 16px;
        width: 100%;
        max-width: 700px;
        max-height: 80vh;
        display: flex;
        flex-direction: column;
        box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
    }

    .modal-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 24px;
        border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    }

    .modal-header h3 {
        margin: 0;
        font-size: 18px;
        font-weight: 600;
        color: #ffffff;
    }

    .modal-close {
        background: none;
        border: none;
        color: #a0a0a0;
        cursor: pointer;
        padding: 4px;
        border-radius: 4px;
        transition: all 0.2s;
    }

    .modal-close:hover {
        background: rgba(255, 255, 255, 0.1);
        color: #ffffff;
    }

    .modal-tabs {
        display: flex;
        gap: 4px;
        padding: 16px 24px 0;
        border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    }

    .modal-tab {
        background: none;
        border: none;
        padding: 12px 20px;
        border-radius: 8px 8px 0 0;
        cursor: pointer;
        font-size: 14px;
        font-weight: 500;
        color: #a0a0a0;
        transition: all 0.2s;
    }

    .modal-tab:hover {
        color: #ffffff;
        background: rgba(255, 255, 255, 0.05);
    }

    .modal-tab.active {
        background: rgba(255, 255, 255, 0.1);
        color: #ffffff;
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
        color: #a0a0a0;
        text-transform: uppercase;
        letter-spacing: 0.5px;
        margin: 0;
    }

    .keycode-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
        gap: 8px;
    }

    .keycode-option {
        padding: 12px;
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 8px;
        cursor: pointer;
        font-size: 13px;
        color: #ffffff;
        transition: all 0.2s;
        text-align: center;
    }

    .keycode-option:hover {
        background: rgba(255, 255, 255, 0.1);
        border-color: rgba(102, 126, 234, 0.5);
    }

    .keycode-option.selected {
        background: rgba(102, 126, 234, 0.2);
        border-color: #667eea;
        box-shadow: 0 0 0 2px rgba(102, 126, 234, 0.3);
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

    .preview-label {
        font-size: 13px;
        color: #a0a0a0;
        font-weight: 500;
    }

    .preview-value {
        font-size: 14px;
        color: #ffffff;
        font-weight: 600;
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
