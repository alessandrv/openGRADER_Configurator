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
    
    // Keymap and encoder data
    let keymap = $state([]);
    let encoders = $state([]);
    let keycodes = {};
    
    // UI state
    let selectedTab = $state('keymap');
    let selectedKey = $state(null);
    let selectedEncoder = null;
    let originalKeymap = [];
    let originalEncoders = [];
    let hasChanges = false;
    let showSavePopup = false;
    let connectionStatus = $state('disconnected'); // 'disconnected', 'connected', 'reconnecting'

    onMount(async () => {
        console.log('=== FRONTEND: App started ===');
        
        // Load keycodes
        await loadKeycodes();
        
        console.log('=== FRONTEND: App ready ===');
        
        // No event listeners, no polling - just manual connect/disconnect
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
    }

    function checkForChanges() {
        const keymapChanged = JSON.stringify(keymap) !== JSON.stringify(originalKeymap);
        const encodersChanged = JSON.stringify(encoders) !== JSON.stringify(originalEncoders);
        hasChanges = keymapChanged || encodersChanged;
    }

    async function connectDevice() {
        loading = true;
        error = null;
        
        try {
            console.log('Connecting to device...');
            const result = await invoke('simple_connect');
            
            // Update state with all the data we received
            isConnected = true;
            connectionStatus = 'connected';
            deviceInfo = result.device_info;
            keymap = result.keymap || [];
            encoders = result.encoders || [];
            dataLoaded = true;
            
            // Store original data for change tracking
            storeOriginalData();
            
            console.log('Connected successfully:', result);
        } catch (e) {
            error = `Failed to connect: ${e}`;
            console.error('Connection failed:', e);
            
            // Reset state on failure
            isConnected = false;
            connectionStatus = 'disconnected';
            deviceInfo = null;
            keymap = [];
            encoders = [];
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
            
            // Reset all state
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
            await invoke('set_keymap_entry', {
                entry: { row, col, keycode }
            });
            
            // Update local keymap
            keymap[row][col].keycode = keycode;
            keymap = keymap; // Trigger reactivity
            checkForChanges();
        } catch (e) {
            error = `Failed to update keymap: ${e}`;
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
            
            // Update local encoder data
            const encoder = encoders.find(e => e.encoder_id === encoderId);
            if (encoder) {
                encoder.ccw_keycode = ccwKeyCode;
                encoder.cw_keycode = cwKeyCode;
                encoders = encoders; // Trigger reactivity
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
            storeOriginalData(); // Reset change tracking after save
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
            // Reload data after loading config
            if (isConnected) {
                await loadDeviceData(true);
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
            // Reload data after reset
            if (isConnected) {
                await loadDeviceData(true);
            }
        } catch (e) {
            error = `Failed to reset config: ${e}`;
        }
        
        loading = false;
    }

    // Utility functions
    // Describe OP MIDI codes (same logic as firmware and backend) so frontend can show friendly labels
    function describeOpMidi(code) {
        const base = 0x7E10;
        if (code < base) return null;
        const delta = (code - base) & 0xFFFF;
        const channel = ((delta >> 11) & 0x0F) + 1;
        const controller = (delta >> 4) & 0x7F;
        const index = delta & 0x0F;
        if (index === 0x0F) {
            // Note message: controller bits hold note value
            const note = controller & 0x7F;
            return `MIDI Note ch${channel} note${note}`;
        }
        const values = [0,1,7,15,31,43,45,63,64,79,95,111,120,127,50,100];
        const val = values[index] !== undefined ? values[index] : 127;
        return `MIDI CC ch${channel} ctrl${controller} val${val}`;
    }

    // Cache of display names (prepopulated from static keycodes, and used for MIDI decoding)
    let keycodeNamesCache = {};

    // Prepopulate cache from loaded keycodes
    function populateKeycodeCache() {
        for (const [k, v] of Object.entries(keycodes)) {
            keycodeNamesCache[Number(k)] = v.display_name;
        }
        // Trigger Svelte reactivity
        keycodeNamesCache = { ...keycodeNamesCache };
    }

    function getKeycodeName(code) {
        return keycodes[code]?.display_name || `0x${code.toString(16).toUpperCase().padStart(4, '0')}`;
    }

    // Return compact label lines for display (array of short strings)
    function formatKeyLabel(code) {
        const OP_MIDI_CC_BASE = 0x7E10;
        if (typeof code !== 'number') return [''];
        if (code >= OP_MIDI_CC_BASE) {
            const delta = (code - OP_MIDI_CC_BASE) & 0xFFFF;
            const ch = ((delta >> 11) & 0x0F) + 1;
            const ctrl_or_note = (delta >> 4) & 0x7F;
            const idx = delta & 0x0F;
            const values = [0,1,7,15,31,43,45,63,64,79,95,111,120,127,50,100];
            if (idx === 0x0F) {
                // Note
                return [`Ch ${ch}`, `Note ${ctrl_or_note}`];
            }
            const val = values[idx] !== undefined ? values[idx] : 127;
            return [`Ch ${ch}`, `CC ${ctrl_or_note}`, `${val}`];
        }
        // Non-MIDI: try friendly name from map
        const name = keycodes[code]?.display_name;
        if (name) {
            // split long names into up to two lines if they contain spaces
            const parts = name.split(' ');
            if (parts.length <= 2) return [name];
            return [parts.slice(0,2).join(' '), parts.slice(2).join(' ')];
        }
        return [`0x${code.toString(16).toUpperCase().padStart(4, '0')}`];
    }

    function getKeycodeOptions() {
        return Object.entries(keycodes).map(([code, keycode]) => ({
            value: parseInt(code),
            label: `${keycode.display_name} (${keycode.name})`,
            category: keycode.category
        }));
    }

    function handleKeycodeChange(event) {
        if (selectedKey) {
            selectedKey.keycode = parseInt(event.target.value);
        }
    }

    function handleEncoderCcwChange(encoder, event) {
        encoder.ccw_keycode = parseInt(event.target.value);
    }

    function handleEncoderCwChange(encoder, event) {
        encoder.cw_keycode = parseInt(event.target.value);
    }

    // MIDI encoding helpers (match firmware OP_MIDI encoding in op_keycodes.h)
    const OP_MIDI_CC_BASE = 0x7E10;
    function midiValueIndex(value) {
        const values = [0, 1, 7, 15, 31, 43, 45, 63, 64, 79, 95, 111, 120, 127, 50, 100];
        const idx = values.indexOf(Number(value));
        return idx >= 0 ? idx : 12; // default index 12 -> 127
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

    // MIDI form state for key editor
    let midiType = 'cc'; // 'cc' or 'note'
    let midiChannel = 1;
    let midiController = 1;
    let midiValue = 43;
    let midiNote = 60;

    function applyMidiToSelectedKey() {
        if (!selectedKey) return;
        let code = 0;
        if (midiType === 'cc') {
            code = encodeMidiCC(midiChannel, midiController, midiValue);
        } else {
            code = encodeMidiNote(midiChannel, midiNote);
        }
        selectedKey.keycode = code;
    }

    // Apply MIDI to encoder object
    function applyMidiToEncoder(encoder, which) {
        let code = 0;
        if (midiType === 'cc') {
            code = encodeMidiCC(midiChannel, midiController, midiValue);
        } else {
            code = encodeMidiNote(midiChannel, midiNote);
        }
        if (which === 'ccw') encoder.ccw_keycode = code;
        else encoder.cw_keycode = code;
    }
</script>

<main class="app">
    <div class="app-background"></div>
    
    <div class="container">
        <!-- Header -->
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

        <!-- Error Display -->
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

        <!-- Connection Section -->
        <div class="glass-card connection-card">
            <div class="card-header">
                <h2>Device Connection</h2>
                <p>Connect your keyboard to begin configuration</p>
            </div>
            <div class="card-content">
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
        </div>

        <!-- Device Info -->
        {#if deviceInfo}
            <div class="glass-card device-info-card">
                <div class="card-header">
                    <h2>Device Information</h2>
                </div>
                <div class="device-info-grid">
                    <div class="info-item">
                        <span class="info-label">Device Name</span>
                        <span class="info-value">{deviceInfo.device_name}</span>
                    </div>
                    <div class="info-item">
                        <span class="info-label">Firmware Version</span>
                        <span class="info-value">{deviceInfo.firmware_version_major}.{deviceInfo.firmware_version_minor}.{deviceInfo.firmware_version_patch}</span>
                    </div>
                    <div class="info-item">
                        <span class="info-label">Protocol Version</span>
                        <span class="info-value">{deviceInfo.protocol_version}</span>
                    </div>
                    <div class="info-item">
                        <span class="info-label">Device Type</span>
                        <span class="info-value">{deviceInfo.device_type === 1 ? 'Master' : 'Slave'}</span>
                    </div>
                    <div class="info-item">
                        <span class="info-label">Matrix Size</span>
                        <span class="info-value">{deviceInfo.matrix_rows}×{deviceInfo.matrix_cols}</span>
                    </div>
                    <div class="info-item">
                        <span class="info-label">Encoders</span>
                        <span class="info-value">{deviceInfo.encoder_count}</span>
                    </div>
                </div>
            </div>
        {/if}

        <!-- Tab Navigation -->
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
            <button 
                class="tab-button"
                class:active={selectedTab === 'config'}
                onclick={() => selectedTab = 'config'}
                disabled={!isConnected}
                aria-label="Configuration Tab"
            >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                    <circle cx="12" cy="12" r="3" stroke="currentColor" stroke-width="2"/>
                    <path d="M19.4 15A1.65 1.65 0 0 0 21 13.09A1.65 1.65 0 0 0 19.4 9A1.65 1.65 0 0 0 21 6.91A1.65 1.65 0 0 0 19.4 3" stroke="currentColor" stroke-width="2"/>
                    <path d="M4.6 9A1.65 1.65 0 0 0 3 10.91A1.65 1.65 0 0 0 4.6 15A1.65 1.65 0 0 0 3 17.09A1.65 1.65 0 0 0 4.6 21" stroke="currentColor" stroke-width="2"/>
                </svg>
                <span>Configuration</span>
            </button>
        </nav>

        <!-- Tab Content -->
        <div class="tab-content">
            <!-- Keymap Tab -->
            {#if selectedTab === 'keymap'}
                <div class="glass-card keymap-card">
                    <div class="card-header">
                        <h2>Keymap Configuration</h2>
                        <p>Click on any key to customize its function</p>
                    </div>
                    
                    {#if loadingKeymap}
                        <div class="loading-state">
                            <div class="spinner-large"></div>
                            <h3>Loading Keymap...</h3>
                            <p>Fetching key configuration from device</p>
                        </div>
                    {:else if keymap.length > 0}
                        <div class="keymap-container">
                            <div class="keymap">
                                {#each keymap as row, rowIndex}
                                    <div class="keymap-row">
                                        {#each row as key, colIndex}
                                            <button 
                                                class="key"
                                                class:selected={selectedKey && selectedKey.row === rowIndex && selectedKey.col === colIndex}
                                                onclick={() => selectedKey = {row: rowIndex, col: colIndex, keycode: key.keycode}}
                                                aria-label={`Key R${rowIndex}C${colIndex}`}>
                                                {#each formatKeyLabel(key.keycode) as line}
                                                   <div class="key-label">{line}</div>
                                                {/each}
                                            </button>
                                        {/each}
                                    </div>
                                {/each}
                            </div>

                            {#if selectedKey}
                                <div class="key-editor">
                                    <div class="editor-header">
                                        <h3>Edit Key R{selectedKey.row}C{selectedKey.col}</h3>
                                    </div>
                                    <div class="editor-content">
                                        <div class="form-group">
                                            <label for="keycode-select">Keycode</label>
                                            <!-- Updated to use onchange instead of bind:value -->
                                            <select id="keycode-select" value={selectedKey.keycode} onchange={handleKeycodeChange}>
                                                {#each getKeycodeOptions() as option}
                                                    <option value={option.value}>{option.label}</option>
                                                {/each}
                                            </select>
                                        </div>

                                        <div class="form-group midi-group">
                                            <label>MIDI Assignment</label>
                                            <div class="midi-row">
                                                <select bind:value={midiType} aria-label="MIDI type">
                                                    <option value="cc">Control Change (CC)</option>
                                                    <option value="note">Note</option>
                                                </select>
                                                <input type="number" min="1" max="16" bind:value={midiChannel} aria-label="MIDI channel" />
                                                {#if midiType === 'cc'}
                                                    <input type="number" min="0" max="127" bind:value={midiController} aria-label="MIDI controller" />
                                                    <input type="number" min="0" max="127" bind:value={midiValue} aria-label="MIDI value" />
                                                {:else}
                                                    <input type="number" min="0" max="127" bind:value={midiNote} aria-label="MIDI note" />
                                                {/if}
                                                <button class="secondary-button" onclick={applyMidiToSelectedKey}>Apply MIDI</button>
                                            </div>
                                            <p class="muted">After applying, press Update Key to save to the device.</p>
                                        </div>

                                        <button class="primary-button" onclick={() => updateKeymap(selectedKey.row, selectedKey.col, selectedKey.keycode)}>
                                            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                                                <path d="M20 6L9 17L4 12" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
                                            </svg>
                                            Update Key
                                        </button>
                                    </div>
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

            <!-- Encoders Tab -->
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
                        <div class="encoders-grid">
                            {#each encoders as encoder}
                                <div class="encoder-item">
                                    <div class="encoder-header">
                                        <h4>Encoder {encoder.encoder_id}</h4>
                                    </div>
                                    <div class="encoder-controls">
                                        <div class="form-group">
                                            <label for={`ccw-keycode-${encoder.encoder_id}`}>Counter-clockwise</label>
                                            <!-- Updated to use onchange instead of bind:value -->
                                            <select id={`ccw-keycode-${encoder.encoder_id}`} value={encoder.ccw_keycode} onchange={(e) => handleEncoderCcwChange(encoder, e)}>
                                                {#each getKeycodeOptions() as option}
                                                    <option value={option.value}>{option.label}</option>
                                                {/each}
                                            </select>
                                            <div class="midi-inline">
                                                <button class="secondary-button" onclick={() => applyMidiToEncoder(encoder, 'ccw')}>Apply MIDI</button>
                                            </div>
                                        </div>
                                        <div class="form-group">
                                            <label for={`cw-keycode-${encoder.encoder_id}`}>Clockwise</label>
                                            <!-- Updated to use onchange instead of bind:value -->
                                            <select id={`cw-keycode-${encoder.encoder_id}`} value={encoder.cw_keycode} onchange={(e) => handleEncoderCwChange(encoder, e)}>
                                                {#each getKeycodeOptions() as option}
                                                    <option value={option.value}>{option.label}</option>
                                                {/each}
                                            </select>
                                            <div class="midi-inline">
                                                <button class="secondary-button" onclick={() => applyMidiToEncoder(encoder, 'cw')}>Apply MIDI</button>
                                            </div>
                                        </div>
                                        <!-- MIDI Controls -->
                                        <div class="midi-inline-control">
                                            <select bind:value={midiType} aria-label="MIDI Type">
                                                <option value="cc">CC</option>
                                                <option value="note">Note</option>
                                            </select>
                                            <input type="number" bind:value={midiChannel} min="1" max="16" aria-label="MIDI Channel"/>
                                            {#if midiType === 'cc'}
                                                <input type="number" bind:value={midiController} min="0" max="127" aria-label="MIDI Controller"/>
                                                <input type="number" bind:value={midiValue} min="0" max="127" aria-label="MIDI Value"/>
                                            {:else}
                                                <input type="number" bind:value={midiNote} min="0" max="127" aria-label="MIDI Note"/>
                                            {/if}
                                            <button class="primary-button" onclick={() => applyMidiToEncoder(encoder, 'ccw')}>
                                                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                                                    <path d="M20 6L9 17L4 12" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
                                                </svg>
                                                Apply to CCW
                                            </button>
                                            <button class="primary-button" onclick={() => applyMidiToEncoder(encoder, 'cw')}>
                                                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                                                    <path d="M20 6L9 17L4 12" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
                                                </svg>
                                                Apply to CW
                                            </button>
                                        </div>
                                        <button class="primary-button" onclick={() => updateEncoder(encoder.encoder_id, encoder.ccw_keycode, encoder.cw_keycode)}>
                                            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                                                <path d="M20 6L9 17L4 12" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
                                            </svg>
                                            Update
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

            <!-- Configuration Tab -->
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
                    
                    <div class="config-info">
                        <div class="info-card">
                            <h4>Save to EEPROM</h4>
                            <p>Permanently stores your current configuration to the keyboard's memory</p>
                        </div>
                        <div class="info-card">
                            <h4>Load from EEPROM</h4>
                            <p>Restores the previously saved configuration from the keyboard's memory</p>
                        </div>
                        <div class="info-card">
                            <h4>Reset to Defaults</h4>
                            <p>Resets all settings to factory defaults - this action cannot be undone</p>
                        </div>
                    </div>
                </div>
            {/if}
        </div>
    </div>
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
        display: flex;
        justify-content: space-between;
        align-items: flex-start;
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

    /* Device Info Grid */
    .device-info-grid {
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
        gap: 16px;
    }

    .info-item {
        display: flex;
        flex-direction: column;
        gap: 4px;
        padding: 16px;
        background: rgba(255, 255, 255, 0.05);
        border-radius: 8px;
        border: 1px solid rgba(255, 255, 255, 0.1);
    }

    .info-label {
        font-size: 12px;
        color: #a0a0a0;
        text-transform: uppercase;
        letter-spacing: 0.5px;
        font-weight: 500;
    }

    .info-value {
        font-size: 14px;
        color: #ffffff;
        font-weight: 500;
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
        gap: 24px;
        align-items: flex-start;
    }

    .keymap {
        display: flex;
        flex-direction: column;
        gap: 4px;
        flex: 1;
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
        transform: translateY(-1px);
        box-shadow: 0 4px 12px rgba(102, 126, 234, 0.2);
    }

    .key.selected {
        border-color: #667eea;
        background: rgba(102, 126, 234, 0.1);
        box-shadow: 0 0 0 2px rgba(102, 126, 234, 0.3);
    }

    .key-label {
        font-size: 12px;
        line-height: 1.0;
        text-align: center;
        margin: 0;
    }

    .key-coords {
        font-size: 9px;
        color: #a0a0a0;
        position: relative;
        z-index: 1;
    }

    /* Key Editor */
    .key-editor {
        min-width: 300px;
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 12px;
        padding: 20px;
        backdrop-filter: blur(10px);
    }

    .editor-header h3 {
        margin: 0 0 16px 0;
        font-size: 16px;
        font-weight: 600;
        color: #ffffff;
    }

    .form-group {
        margin-bottom: 16px;
    }

    .form-group label {
        display: block;
        margin-bottom: 8px;
        font-size: 14px;
        font-weight: 500;
        color: #ffffff;
    }

    .form-group select {
        width: 100%;
        padding: 12px;
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.2);
        border-radius: 8px;
        color: #ffffff;
        font-size: 14px;
        backdrop-filter: blur(10px);
    }

    .form-group select:focus {
        outline: none;
        border-color: #667eea;
        box-shadow: 0 0 0 2px rgba(102, 126, 234, 0.2);
    }

    /* Encoders */
    .encoders-grid {
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(350px, 1fr));
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

    .encoder-controls {
        display: flex;
        flex-direction: column;
        gap: 16px;
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

    /* Connection Status */
    .device-header-left {
        display: flex;
        flex-direction: column;
        gap: 8px;
    }

    .device-header-left h2 {
        margin: 0;
    }

    .connection-status {
        display: flex;
        align-items: center;
        gap: 8px;
        font-size: 13px;
        font-weight: 500;
    }

    .status-indicator {
        width: 8px;
        height: 8px;
        border-radius: 50%;
        animation: pulse 2s infinite;
    }

    .connection-status.connected .status-indicator {
        background-color: #22c55e;
        animation: none;
    }

    .connection-status.reconnecting .status-indicator {
        background-color: #eab308;
        animation: pulse 1.5s infinite;
    }

    .connection-status.disconnected .status-indicator {
        background-color: #ef4444;
        animation: none;
    }

    .connection-status.connected .status-text {
        color: #22c55e;
    }

    .connection-status.reconnecting .status-text {
        color: #eab308;
    }

    .connection-status.disconnected .status-text {
        color: #ef4444;
    }

    @keyframes pulse {
        0%, 100% {
            opacity: 1;
        }
        50% {
            opacity: 0.5;
        }
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

        .keymap-container {
            flex-direction: column;
        }

        .config-actions {
            flex-direction: column;
        }

        .device-info-grid {
            grid-template-columns: 1fr;
        }
    }
</style>
