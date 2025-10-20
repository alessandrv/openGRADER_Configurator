<script>
    // @ts-nocheck
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
    let loadingLayout = $state(false);
    
    // I2C slave devices
    let i2cDevices = $state([]);
    let loadingI2CDevices = $state(false);
    let i2cRefreshInFlight = false;
    let selectedDevice = $state('main'); // 'main' or slave address
    
    // Keymap and encoder data
    let keymap = $state([]); // [layer][row][col]
    let slaveKeymaps = $state({}); // Map of slave address -> [layer][row][col]
    let slaveEncoders = $state({}); // Map of slave address -> [layer][encoder]
    let encoders = $state([]); // [layer][encoder]
    let selectedLayer = $state(0);
    let layerCount = $state(1);
    let layerState = $state(null);
    let layerStateBusy = $state(false);
    let momentaryLayer = $state(null);
    let lastStableLayer = $state(0);
    let baselineLayerMask = 0;
    let hardwareActiveLayer = $state(0);
    let manualLayerOverride = false;
    let boardLayout = $state(null);
    let activeEncoderMenu = $state(null);
    let activeSliderMenu = $state(null); // For slider configuration menu
    
    // Slider state
    let sliderValues = $state({}); // Map of slider ID -> current value (0-127)
    let sliderConfigs = $state({}); // Map of slider ID -> configuration
    
    // Magnetic switch state
    let magneticSwitchValues = $state({}); // Map of switch ID -> current value (0-100%)
    let magneticSwitchConfigs = $state({}); // Map of switch ID -> configuration
    let calibrationMode = $state({}); // Map of switch ID -> calibration state
    let activeMagneticSwitchMenu = $state(null); // For magnetic switch configuration menu
    
    let keycodes = {};
    let keycodeByName = {};
    
    // UI state
    let selectedTab = $state('keymap');
    let selectedKey = $state(null);
    let selectedEncoder = null;
    let originalKeymap = [];
    let originalEncoders = [];
    let originalSlaveKeymaps = {};
    let originalSlaveEncoders = {};
    let originalLayerState = null;
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
    let layerStatePollInterval = null;
    let sliderPollInterval = null; // For real-time slider value updates
    let magneticSwitchPollInterval = null; // For real-time magnetic switch value updates

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
            stopLayerStatePolling();
            stopSliderPolling();
            stopMagneticSwitchPolling();
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

    async function refreshI2CDevices({ quiet = false } = {}) {
        if (!isConnected) {
            return;
        }

        if ((deviceInfo?.device_type ?? 0) !== 1) {
            if (!quiet) {
                console.log('Connected to slave device - no I2C slaves to query');
            }
            if (i2cDevices.length) {
                i2cDevices = [];
            }
            if (selectedDevice !== 'main') {
                selectedDevice = 'main';
            }
            return;
        }

        if (i2cRefreshInFlight) {
            return;
        }

        i2cRefreshInFlight = true;

        if (!quiet) {
            loadingI2CDevices = true;
        }

        try {
            const response = await invoke('get_i2c_devices');
            const devices = Array.isArray(response) ? response : [];
            const nextSnapshot = JSON.stringify(devices);
            const currentSnapshot = JSON.stringify(i2cDevices);

            if (nextSnapshot !== currentSnapshot) {
                i2cDevices = devices;
                if (selectedDevice !== 'main') {
                    const stillPresent = devices.some((device) => String(device.address) === selectedDevice);
                    if (!stillPresent) {
                        selectedDevice = 'main';
                    }
                }
                if (!quiet) {
                    console.log('I2C devices updated:', devices);
                }
            }
        } catch (e) {
            console.error('Failed to load I2C devices:', e);
        } finally {
            if (!quiet) {
                loadingI2CDevices = false;
            }
            i2cRefreshInFlight = false;
        }
    }
    
    function stopConnectionCheck() {
        if (connectionCheckInterval) {
            clearInterval(connectionCheckInterval);
            connectionCheckInterval = null;
        }
    }

    function startLayerStatePolling() {
        if (layerStatePollInterval) return;

        layerStatePollInterval = setInterval(() => {
            if (isConnected) {
                refreshLayerStateFromDevice();
            }
        }, 500);
    }

    function stopLayerStatePolling() {
        if (layerStatePollInterval) {
            clearInterval(layerStatePollInterval);
            layerStatePollInterval = null;
        }
    }
    
    function startSliderPolling() {
        if (sliderPollInterval) return;

        sliderPollInterval = setInterval(() => {
            if (isConnected) {
                pollSliderValues();
            }
        }, 50); // Poll every 50ms for very responsive slider feedback
    }

    function stopSliderPolling() {
        if (sliderPollInterval) {
            clearInterval(sliderPollInterval);
            sliderPollInterval = null;
        }
    }
    
    function startMagneticSwitchPolling() {
        if (magneticSwitchPollInterval) return;

        magneticSwitchPollInterval = setInterval(() => {
            if (isConnected) {
                pollMagneticSwitchValues();
            }
        }, 50); // Poll every 50ms for responsive magnetic switch feedback
    }

    function stopMagneticSwitchPolling() {
        if (magneticSwitchPollInterval) {
            clearInterval(magneticSwitchPollInterval);
            magneticSwitchPollInterval = null;
        }
    }
    
    async function tryAutoConnect() {
        try {
            console.log('Attempting auto-connect...');
            const result = await invoke('simple_connect');
            
            isConnected = true;
            connectionStatus = 'connected';
            deviceInfo = result.device_info;
            keymap = Array.isArray(result.keymap) ? result.keymap : [];
            encoders = Array.isArray(result.encoders) ? result.encoders : [];
            layerCount = Math.max(deviceInfo?.layer_count ?? keymap.length ?? 1, 1);
            const maskLayers = Math.min(layerCount, 8);
            const defaultMask = maskLayers >= 8 ? 0xff : ((1 << maskLayers) - 1) & 0xff;
            const initialLayerState = result.layer_state ?? {
                active_mask: defaultMask === 0 ? 1 : defaultMask,
                default_layer: 0,
            };
            updateLayerCountForDevice('main');
            updateLayerStateLocal(initialLayerState, { fromDevice: true });
            manualLayerOverride = false;

            await refreshLayerStateFromDevice();
            boardLayout = result.layout ?? null;
            // Normalize layout field names for the frontend: some code expects
            // matrix_rows/matrix_cols while other parts use rows/cols. Provide
            // fallback aliases to keep both styles working.
            if (boardLayout) {
                boardLayout.rows = boardLayout.rows ?? boardLayout.matrix_rows;
                boardLayout.cols = boardLayout.cols ?? boardLayout.matrix_cols;
            }
            dataLoaded = true;

            if (!boardLayout) {
                try {
                    boardLayout = await invoke('get_board_layout');
                    if (boardLayout) {
                        boardLayout.rows = boardLayout.rows ?? boardLayout.matrix_rows;
                        boardLayout.cols = boardLayout.cols ?? boardLayout.matrix_cols;
                    }
                } catch (layoutError) {
                    console.warn('Failed to fetch board layout metadata:', layoutError);
                }
            }
            
            // Initialize default slider data
            initializeSliderData();
            
            // Initialize default magnetic switch data
            initializeMagneticSwitchData();
            
            // Load real slider config from device and get initial values for all sliders
            await loadAllSliderConfigs();
            await pollSliderValues(); // Get initial values immediately
            startSliderPolling();
            
            // Load magnetic switch configs and start polling
            await loadAllMagneticSwitchConfigs();
            await pollMagneticSwitchValues();
            startMagneticSwitchPolling();
            
            activeEncoderMenu = null;

            console.log('Auto-connected successfully:', result);
            await refreshI2CDevices();
            
            slaveKeymaps = {};
            slaveEncoders = {};
            storeOriginalData();
            
            // Start monitoring connection
            startConnectionCheck();
            startLayerStatePolling();
            
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
            await refreshLayerStateFromDevice();
            await refreshI2CDevices({ quiet: true });
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
    selectedLayer = 0;
    layerCount = 1;
    layerStateBusy = false;
    updateLayerStateLocal(null);
        boardLayout = null;
        layoutMatrix = [];
        layoutMatrixReady = false;
        selectedKey = null;
        selectedEncoder = null;
        activeEncoderMenu = null;
        originalKeymap = [];
        originalEncoders = [];
        originalSlaveKeymaps = {};
        originalSlaveEncoders = {};
    originalLayerState = null;
        dataLoaded = false;
        hasChanges = false;
        showSavePopup = false;
        i2cDevices = [];
        
        // Clear layout cache when disconnecting
        clearLayoutCache();
        
        stopConnectionCheck();
        stopLayerStatePolling();
        stopSliderPolling();
        stopMagneticSwitchPolling();
        
        console.log('Device disconnected, will attempt to reconnect...');
    }

    // Load keycodes from backend
    async function loadKeycodes() {
        try {
            const keycodeList = await invoke('get_keycodes');
            const sorted = Array.isArray(keycodeList)
                ? [...keycodeList].sort((a, b) => Number(a.code) - Number(b.code))
                : [];

            const mapped = {};
            for (const entry of sorted) {
                if (entry && typeof entry.code === 'number') {
                    mapped[entry.code] = entry;
                }
            }

            keycodes = mapped;
            keycodeByName = {};
            for (const entry of Object.values(mapped)) {
                if (entry?.name) {
                    keycodeByName[entry.name] = entry.code;
                }
            }
        } catch (e) {
            console.error('Failed to load keycodes:', e);
        }
    }

    function updateLayerCountForDevice(deviceId) {
        // Always use the master's layer count - all devices are synced to the same layers
        layerCount = Math.max(deviceInfo?.layer_count ?? keymap.length ?? 1, 1);
        
        const clampedSelected = clampLayerIndex(selectedLayer);
        if (clampedSelected !== selectedLayer) {
            selectedLayer = clampedSelected;
        }

        const clampedActive = clampLayerIndex(hardwareActiveLayer);
        if (clampedActive !== hardwareActiveLayer) {
            hardwareActiveLayer = clampedActive;
        }

        if (selectedLayer === hardwareActiveLayer) {
            manualLayerOverride = false;
        }
    }

    function setSelectedDevice(deviceId) {
        if (selectedDevice !== deviceId) {
            selectedDevice = deviceId;
            // Don't reset layer when switching devices - keep the current layer
            // because master and slaves are always in sync
            updateLayerCountForDevice(deviceId);
        }
    }

    function layerIndices() {
        return Array.from({ length: layerCount }, (_, i) => i);
    }

    function clampLayerIndex(index) {
        if (!Number.isFinite(index)) {
            return 0;
        }
        if (layerCount <= 0) {
            return 0;
        }
        return Math.max(0, Math.min(Math.floor(index), layerCount - 1));
    }

    function bitForLayer(index) {
        if (!Number.isFinite(index)) {
            return 0;
        }
        if (index < 0 || index >= 8) {
            return 0;
        }
        return 1 << index;
    }

    function highestActiveLayer(maskValue) {
        if (!Number.isFinite(maskValue)) {
            return 0;
        }
        const limit = Math.min(layerCount, 8);
        for (let i = limit - 1; i >= 0; i--) {
            if ((maskValue & bitForLayer(i)) !== 0) {
                return i;
            }
        }
        return 0;
    }

    function validLayerMask(mask) {
        if (!Number.isFinite(mask)) {
            return 0;
        }
        const availableLayers = Math.min(layerCount, 8);
        if (availableLayers <= 0) {
            return 0;
        }
        const allowedMask = availableLayers >= 8 ? 0xff : ((1 << availableLayers) - 1) & 0xff;
        return (mask & allowedMask) & 0xff;
    }

    function extractActiveLayers(mask) {
        const active = [];
        const filtered = validLayerMask(mask);
        for (let i = 0; i < layerCount && i < 8; i++) {
            if ((filtered & bitForLayer(i)) !== 0) {
                active.push(i);
            }
        }
        return active;
    }

    function stableLayerStateSnapshot(state) {
        if (!state) {
            return null;
        }

        const normalizedDefault = clampLayerIndex(state.default_layer ?? 0);
        const stableMask = bitForLayer(normalizedDefault) || 1;

        return {
            active_mask: stableMask,
            default_layer: normalizedDefault,
        };
    }

    function updateLayerStateLocal(state, { fromDevice = false } = {}) {
        if (!state) {
            layerState = null;
            momentaryLayer = null;
            lastStableLayer = 0;
            baselineLayerMask = 0;
            hardwareActiveLayer = 0;
            manualLayerOverride = false;
            if (selectedLayer !== 0) {
                selectedLayer = 0;
            }
            return;
        }

        const normalizedDefault = clampLayerIndex(state.default_layer ?? 0);
        let mask = validLayerMask(state.active_mask ?? 0);
        if (mask === 0) {
            mask = bitForLayer(normalizedDefault) || 1;
        }

        const previousMask = layerState?.active_mask ?? null;
        const previousDefault = layerState?.default_layer ?? null;
        const previousHardware = hardwareActiveLayer;

        let baseMask = baselineLayerMask;

        if (baseMask === 0 && mask !== 0) {
            baseMask = mask;
        }

        const removedBits = baselineLayerMask & ~mask;
        if (removedBits) {
            baseMask = mask;
        }

        let addedBits = mask & ~baseMask;
        let newMomentaryLayer = null;

        if (addedBits) {
            newMomentaryLayer = highestActiveLayer(addedBits);
        } else {
            baseMask = mask;
        }

        baselineLayerMask = baseMask;

        if (newMomentaryLayer !== null && !Number.isFinite(newMomentaryLayer)) {
            newMomentaryLayer = null;
        }

        if (newMomentaryLayer !== null && (newMomentaryLayer < 0 || newMomentaryLayer >= layerCount)) {
            newMomentaryLayer = null;
        }

        momentaryLayer = newMomentaryLayer;

        const baseLayer = baseMask ? highestActiveLayer(baseMask) : normalizedDefault;
        const activeLayer = momentaryLayer !== null ? momentaryLayer : baseLayer;

        if (momentaryLayer === null) {
            lastStableLayer = baseLayer;
        }

        hardwareActiveLayer = activeLayer;

        const clampedSelection = clampLayerIndex(selectedLayer);
        if (clampedSelection !== selectedLayer) {
            selectedLayer = clampedSelection;
        }

        const maskChanged = previousMask !== mask || previousDefault !== normalizedDefault || previousHardware !== hardwareActiveLayer;

        if (fromDevice) {
            if (maskChanged) {
                manualLayerOverride = false;
            }
            if (!manualLayerOverride || maskChanged) {
                if (selectedLayer !== hardwareActiveLayer) {
                    selectedLayer = hardwareActiveLayer;
                }
            }
        } else if (!manualLayerOverride) {
            if (selectedLayer !== hardwareActiveLayer) {
                selectedLayer = hardwareActiveLayer;
            }
        }

        if (selectedLayer === hardwareActiveLayer) {
            manualLayerOverride = false;
        }

        layerState = {
            active_mask: mask,
            default_layer: normalizedDefault,
        };
    }

    function isLayerActive(index) {
        if (!layerState) {
            return index === 0;
        }
        const bit = bitForLayer(index);
        if (bit === 0) {
            return index === 0;
        }
        return (layerState.active_mask & bit) !== 0;
    }

    function isSoleActiveLayer(index) {
        if (!layerState) {
            return index === 0;
        }
        const targetMask = bitForLayer(index) || 1;
        const filtered = validLayerMask(layerState.active_mask ?? 0);
        return filtered === targetMask;
    }

    function isDefaultLayer(index) {
        return layerState?.default_layer === index;
    }

    function selectLayer(index) {
        const clamped = clampLayerIndex(index);
        if (selectedLayer !== clamped) {
            selectedLayer = clamped;
        }
        manualLayerOverride = clamped !== hardwareActiveLayer;
    }

    async function applyLayerStateUpdate(newState, options = {}) {
        const { fromDevice = false } = options;

        if (selectedDevice !== 'main') {
            updateLayerStateLocal(newState, { fromDevice });
            checkForChanges();
            return;
        }

        if (layerStateBusy) {
            return;
        }

        layerStateBusy = true;
        try {
            const updated = await invoke('set_layer_state', { layerState: newState });
            updateLayerStateLocal(updated ?? newState, { fromDevice: true });
        } catch (e) {
            error = `Failed to update layer state: ${e}`;
            console.error('Failed to update layer state:', e);
        } finally {
            layerStateBusy = false;
            checkForChanges();
        }
    }

    async function refreshLayerStateFromDevice() {
        // Layer state is global and managed by the master, so always poll it
        // even when viewing a slave device
        if (!isConnected) {
            return;
        }

        try {
            const refreshedLayerState = await invoke('get_layer_state');
            if (refreshedLayerState) {
                updateLayerStateLocal(refreshedLayerState, { fromDevice: true });
                checkForChanges();
            }
        } catch (e) {
            console.warn('Failed to refresh layer state:', e);
        }
    }

    async function setDefaultLayer(index) {
        if (!layerState) {
            return;
        }
        if (layerStateBusy && selectedDevice === 'main') {
            return;
        }
        const target = clampLayerIndex(index);
        const currentDefault = clampLayerIndex(layerState.default_layer ?? 0);
        const targetMask = bitForLayer(target) || 1;

        if (currentDefault === target && layerState.active_mask === targetMask) {
            if (selectedLayer !== target) {
                selectedLayer = target;
            }
            return;
        }

        const newState = {
            active_mask: targetMask,
            default_layer: target,
        };
        await applyLayerStateUpdate(newState);
        manualLayerOverride = false;
        if (selectedLayer !== target) {
            selectedLayer = target;
        }
    }

    async function setActiveLayer(index) {
        if (!layerState) {
            return;
        }
        if (layerStateBusy && selectedDevice === 'main') {
            return;
        }
        const target = clampLayerIndex(index);
        const targetMask = bitForLayer(target) || 1;
        const currentMask = layerState.active_mask ?? 0;

        if (currentMask === targetMask) {
            if (selectedLayer !== target) {
                selectedLayer = target;
            }
            return;
        }

        const newState = {
            active_mask: targetMask,
            default_layer: clampLayerIndex(layerState.default_layer ?? 0),
        };
        await applyLayerStateUpdate(newState);
        manualLayerOverride = false;
        if (selectedLayer !== target) {
            selectedLayer = target;
        }
    }

    function storeOriginalData() {
        originalKeymap = JSON.parse(JSON.stringify(keymap));
        originalEncoders = JSON.parse(JSON.stringify(encoders));
        originalSlaveKeymaps = JSON.parse(JSON.stringify(slaveKeymaps));
        originalSlaveEncoders = JSON.parse(JSON.stringify(slaveEncoders));
        originalLayerState = stableLayerStateSnapshot(layerState);
        hasChanges = false;
        showSavePopup = false;
    }

    function checkForChanges() {
        const keymapChanged = JSON.stringify(keymap) !== JSON.stringify(originalKeymap);
        const encodersChanged = JSON.stringify(encoders) !== JSON.stringify(originalEncoders);
        const slaveKeymapsChanged = JSON.stringify(slaveKeymaps) !== JSON.stringify(originalSlaveKeymaps);
    const slaveEncodersChanged = JSON.stringify(slaveEncoders) !== JSON.stringify(originalSlaveEncoders);
    const currentLayerSnapshot = stableLayerStateSnapshot(layerState);
    const layerStateChanged = JSON.stringify(currentLayerSnapshot) !== JSON.stringify(originalLayerState);
        hasChanges = keymapChanged || encodersChanged || slaveKeymapsChanged || slaveEncodersChanged || layerStateChanged;
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
    async function updateKeymap(row, col, keycode, layerOverride = selectedLayer) {
        const layer = layerOverride;
        try {
            if (selectedDevice === 'main') {
                await invoke('set_keymap_entry', {
                    entry: { layer, row, col, keycode }
                });
                const layers = [...keymap];
                const layerEntries = [...(layers[layer] ?? [])];
                const rowEntries = [...(layerEntries[row] ?? [])];
                if (rowEntries[col]) {
                    rowEntries[col] = {
                        ...rowEntries[col],
                        layer,
                        row,
                        col,
                        keycode,
                    };
                    layerEntries[row] = rowEntries;
                    layers[layer] = layerEntries;
                    keymap = layers;
                }
            } else {
                // Update slave device keymap
                const slaveAddr = parseInt(selectedDevice, 10);
                await invoke('set_slave_keymap_entry', {
                    entry: { slave_addr: slaveAddr, layer, row, col, keycode }
                });
                const existingLayers = [...(slaveKeymaps[slaveAddr] ?? [])];
                const layerRows = [...(existingLayers[layer] ?? [])];
                const rowEntries = [...(layerRows[row] ?? [])];
                rowEntries[col] = {
                    slave_addr: slaveAddr,
                    layer,
                    row,
                    col,
                    keycode,
                };
                layerRows[row] = rowEntries;
                existingLayers[layer] = layerRows;
                slaveKeymaps = {
                    ...slaveKeymaps,
                    [slaveAddr]: existingLayers,
                }; // trigger reactivity
            }
            
            // Update original data since changes are now saved to EEPROM
            storeOriginalData();
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
            console.log(`[loadSlaveKeymap] Received keymap with ${slaveKeymap.length} layers for slave ${slaveAddr}`);
            
            // Log details about each layer
            slaveKeymap.forEach((layer, layerIdx) => {
                const keyCount = layer.flat().filter(k => k.keycode !== 0).length;
                console.log(`[loadSlaveKeymap] Layer ${layerIdx}: ${layer.length} rows, ${keyCount} non-zero keys`);
            });
            
            slaveKeymaps[slaveAddr] = slaveKeymap;
            slaveKeymaps = {...slaveKeymaps}; // trigger reactivity
            if (!originalSlaveKeymaps[String(slaveAddr)]) {
                originalSlaveKeymaps[String(slaveAddr)] = JSON.parse(JSON.stringify(slaveKeymap));
            }
            if (selectedDevice === String(slaveAddr)) {
                updateLayerCountForDevice(selectedDevice);
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
            if (selectedDevice === String(slaveAddr)) {
                updateLayerCountForDevice(selectedDevice);
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
            return keymap?.[selectedLayer] ?? [];
        } else {
            const slaveAddr = parseInt(selectedDevice, 10);
            const layers = slaveKeymaps[slaveAddr];
            const currentLayerKeymap = layers?.[selectedLayer] ?? [];
            console.log(`[getCurrentKeymap] Slave ${slaveAddr}, Layer ${selectedLayer}: ${currentLayerKeymap.length} rows, total layers: ${layers?.length ?? 0}`);
            return currentLayerKeymap;
        }
    }

    function getCurrentEncoders() {
        if (selectedDevice === 'main') {
            return encoders?.[selectedLayer] ?? [];
        } else {
            const slaveAddr = parseInt(selectedDevice, 10);
            const layers = slaveEncoders[slaveAddr];
            const currentLayerEncoders = layers?.[selectedLayer] ?? [];
            console.log(`[getCurrentEncoders] Slave ${slaveAddr}, Layer ${selectedLayer}: ${currentLayerEncoders.length} encoders, total layers: ${layers?.length ?? 0}`);
            return currentLayerEncoders;
        }
    }

    function getCurrentLayout() {
        return boardLayout;
    }
    
    // Debug helper to inspect slave layer data
    function debugSlaveLayerData(slaveAddr) {
        const layers = slaveKeymaps[slaveAddr];
        if (!layers) {
            console.log(`[DEBUG] No keymap loaded for slave ${slaveAddr}`);
            return;
        }
        console.log(`[DEBUG] Slave ${slaveAddr} has ${layers.length} layers:`);
        layers.forEach((layer, idx) => {
            const allKeys = layer.flat();
            const nonZeroKeys = allKeys.filter(k => k && k.keycode !== 0);
            console.log(`  Layer ${idx}: ${layer.length} rows × ${layer[0]?.length ?? 0} cols, ${nonZeroKeys.length} programmed keys`);
            if (nonZeroKeys.length > 0 && nonZeroKeys.length <= 10) {
                nonZeroKeys.forEach(k => console.log(`    R${k.row}C${k.col} = 0x${k.keycode.toString(16)}`));
            }
        });
    }

    // TODO: Fix firmware to properly return slave layer data for all layers
    // Currently the firmware is only returning layer 0 data for slaves
    // The backend get_full_slave_keymap() is correctly iterating through all layers,
    // but the firmware's get_slave_keymap_entry() is probably not reading the correct layer
    
    async function DISABLED_copyMasterToSlave(slaveAddr) {
        if (!keymap || keymap.length === 0) {
            error = 'No master keymap to copy';
            return;
        }

        if (!confirm(`This will overwrite ALL layers of slave 0x${slaveAddr.toString(16).toUpperCase()} with the master's keymap. Continue?`)) {
            return;
        }

        copyingToSlave = true;
        error = null;

        try {
            console.log(`[copyMasterToSlave] Starting copy to slave ${slaveAddr}...`);
            
            // Get slave info to know its dimensions
            const slaveInfo = await invoke('get_slave_info', { slaveAddr });
            const slaveRows = slaveInfo.matrix_rows;
            const slaveCols = slaveInfo.matrix_cols;
            
            let entriesWritten = 0;
            let entriesSkipped = 0;

            // Copy each layer from master to slave
            for (let layer = 0; layer < keymap.length && layer < 8; layer++) {
                const masterLayer = keymap[layer];
                if (!masterLayer) continue;

                for (let row = 0; row < Math.min(masterLayer.length, slaveRows); row++) {
                    const masterRow = masterLayer[row];
                    if (!masterRow) continue;

                    for (let col = 0; col < Math.min(masterRow.length, slaveCols); col++) {
                        const masterKey = masterRow[col];
                        if (!masterKey) {
                            entriesSkipped++;
                            continue;
                        }

                        // Write to slave
                        await invoke('set_slave_keymap_entry', {
                            entry: {
                                slave_addr: slaveAddr,
                                layer,
                                row,
                                col,
                                keycode: masterKey.keycode
                            }
                        });
                        entriesWritten++;
                    }
                }
                console.log(`[copyMasterToSlave] Copied layer ${layer} (${entriesWritten} entries so far)`);
            }

            console.log(`[copyMasterToSlave] Completed! Wrote ${entriesWritten} entries, skipped ${entriesSkipped}`);
            
            // Reload the slave keymap to show the changes
            await loadSlaveKeymap(slaveAddr);
            
            error = null;
        } catch (e) {
            error = `Failed to copy keymap to slave: ${e}`;
            console.error('[copyMasterToSlave] Error:', e);
        } finally {
            copyingToSlave = false;
        }
    }

    // Copy master encoders to slave
    async function DISABLED_copyMasterEncodersToSlave(slaveAddr) {
        if (!encoders || encoders.length === 0) {
            error = 'No master encoders to copy';
            return;
        }

        if (!confirm(`This will overwrite ALL encoder layers of slave 0x${slaveAddr.toString(16).toUpperCase()} with the master's encoders. Continue?`)) {
            return;
        }

        copyingToSlave = true;
        error = null;

        try {
            console.log(`[copyMasterEncodersToSlave] Starting copy to slave ${slaveAddr}...`);
            
            // Get slave info to know how many encoders it has
            const slaveInfo = await invoke('get_slave_info', { slaveAddr });
            const slaveEncoderCount = slaveInfo.encoder_count;
            
            let entriesWritten = 0;

            // Copy each layer's encoders from master to slave
            for (let layer = 0; layer < encoders.length && layer < 8; layer++) {
                const masterLayerEncoders = encoders[layer];
                if (!masterLayerEncoders) continue;

                for (let encoderId = 0; encoderId < Math.min(masterLayerEncoders.length, slaveEncoderCount); encoderId++) {
                    const masterEncoder = masterLayerEncoders[encoderId];
                    if (!masterEncoder) continue;

                    // Write to slave
                    await invoke('set_slave_encoder_entry', {
                        entry: {
                            slave_addr: slaveAddr,
                            layer,
                            encoder_id: encoderId,
                            ccw_keycode: masterEncoder.ccw_keycode,
                            cw_keycode: masterEncoder.cw_keycode,
                            reserved: 0
                        }
                    });
                    entriesWritten++;
                }
                console.log(`[copyMasterEncodersToSlave] Copied layer ${layer} encoders (${entriesWritten} entries so far)`);
            }

            console.log(`[copyMasterEncodersToSlave] Completed! Wrote ${entriesWritten} encoder entries`);
            
            // Reload the slave encoders to show the changes
            await loadSlaveEncoders(slaveAddr);
            
            error = null;
        } catch (e) {
            error = `Failed to copy encoders to slave: ${e}`;
            console.error('[copyMasterEncodersToSlave] Error:', e);
        } finally {
            copyingToSlave = false;
        }
    }

    // Layout cell type cache for performance
    let layoutCellCache = new Map();
    let layoutMatrix = $state([]); // 2D array to store layout cell types for template use
    let layoutMatrixReady = $state(false); // Flag to indicate when layout matrix is fully populated
    
    // Initialize layout matrix when board layout info is available
    async function initializeLayoutMatrix() {
        if (!isConnected || !boardLayout || loadingLayout || layoutMatrixReady) return;
        
        console.log('Starting layout matrix initialization...');
        loadingLayout = true;
        layoutMatrixReady = false;
        
        const rows = boardLayout.matrix_rows || 0;
        const cols = boardLayout.matrix_cols || 0;
        
        console.log(`Initializing layout matrix: ${rows}x${cols}`);
        
        try {
            const matrix = [];
            for (let row = 0; row < rows; row++) {
                const rowCells = [];
                for (let col = 0; col < cols; col++) {
                    try {
                        const cellType = await getCachedLayoutCellType(row, col);
                        const componentId = await invoke('get_layout_cell_component_id', { row, col });
                        
                        rowCells.push({
                            type: cellType,
                            componentId: componentId,
                            isSwitch: cellType === 1,
                            isEncoder: cellType === 2,
                            isSlider: cellType === 3,
                            isPotentiometer: cellType === 4,
                            isMagneticSwitch: cellType === 5,
                            isEmpty: cellType === 0
                        });
                        
                        // Log layout for debugging
                        let typeStr = 'EMPTY';
                        if (cellType === 1) typeStr = 'SW';
                        else if (cellType === 2) typeStr = 'ENC';
                        else if (cellType === 3) typeStr = 'SLIDER';
                        else if (cellType === 4) typeStr = 'POT';
                        else if (cellType === 5) typeStr = 'MAG_SW';
                        console.log(`Layout (${row},${col}): ${typeStr} ID=${componentId}`);
                    } catch (e) {
                        console.error(`Failed to get layout for (${row},${col}):`, e);
                        rowCells.push({
                            type: 0,
                            componentId: 0,
                            isSwitch: false,
                            isEncoder: false,
                            isSlider: false,
                            isPotentiometer: false,
                            isMagneticSwitch: false,
                            isEmpty: true
                        });
                    }
                }
                matrix.push(rowCells);
            }
            
            layoutMatrix = matrix;
            layoutMatrixReady = true;
            console.log('Layout matrix initialized:', matrix);
        } catch (e) {
            console.error('Failed to initialize layout matrix:', e);
        } finally {
            loadingLayout = false;
        }
    }
    
    // Synchronous helper functions for template use
    function getLayoutCell(row, col) {
        if (!layoutMatrix || row >= layoutMatrix.length || col >= (layoutMatrix[row]?.length || 0)) {
            return { type: 0, componentId: 0, isSwitch: false, isEncoder: false, isSlider: false, isPotentiometer: false, isMagneticSwitch: false, isEmpty: true };
        }
        return layoutMatrix[row][col];
    }
    
    function isEncoderCellSync(row, col) {
        if (!layoutMatrixReady) return false; // Don't show encoders until layout is fully loaded
        return getLayoutCell(row, col).isEncoder;
    }
    
    function isSwitchCellSync(row, col) {
        if (!layoutMatrixReady) return true; // Show all cells as switches until layout is loaded
        return getLayoutCell(row, col).isSwitch;
    }
    
    function isSliderCellSync(row, col) {
        return getLayoutCell(row, col).isSlider;
    }
    
    function getComponentId(row, col) {
        return getLayoutCell(row, col).componentId;
    }
    
    // Watch for board layout changes and initialize layout matrix
    $effect(() => {
        console.log('Effect triggered:', { 
            boardLayout: !!boardLayout, 
            isConnected, 
            loadingLayout, 
            layoutMatrixReady 
        });
        if (boardLayout && isConnected && !loadingLayout && !layoutMatrixReady) {
            console.log('Board layout changed, initializing layout matrix...');
            initializeLayoutMatrix();
        }
    });

    // Watch for layer changes and reload slider configurations
    $effect(() => {
        if (isConnected && selectedLayer !== undefined && sliderConfigs) {
            console.log(`Layer changed to ${selectedLayer}, reloading slider configurations...`);
            // Reload slider configs for the new layer
            loadAllSliderConfigs(); // Reload for all sliders
        }
    });
    
    // Test the new layout API when device connects
    async function testLayoutAPI() {
        if (!isConnected) return;
        
        console.log('=== Testing Layout API ===');
        
        try {
            const layout = await invoke('get_board_layout');
            console.log('Board layout:', layout);
            
            // Test a few cells
            for (let row = 0; row < Math.min(3, layout.matrix_rows || 0); row++) {
                for (let col = 0; col < Math.min(5, layout.matrix_cols || 0); col++) {
                    const cellType = await getCachedLayoutCellType(row, col);
                    const componentId = await invoke('get_layout_cell_component_id', { row, col });
                    
                    let typeStr = 'EMPTY';
                    if (cellType === 1) typeStr = 'SW';
                    else if (cellType === 2) typeStr = 'ENC';
                    else if (cellType === 3) typeStr = 'SLIDER';
                    
                    console.log(`Cell (${row},${col}): ${typeStr} ID=${componentId}`);
                }
            }
        } catch (e) {
            console.error('Layout API test failed:', e);
        }
        
        console.log('=== Layout API Test Complete ===');
    }
    async function getCachedLayoutCellType(row, col) {
        const key = `${row},${col}`;
        if (layoutCellCache.has(key)) {
            return layoutCellCache.get(key);
        }
        
        try {
            const cellType = await invoke('get_layout_cell_type', { row, col });
            layoutCellCache.set(key, cellType);
            return cellType;
        } catch (e) {
            console.error('Failed to get layout cell type:', e);
            return 0; // LAYOUT_EMPTY
        }
    }
    
    // Clear layout cache when device changes
    function clearLayoutCache() {
        layoutCellCache.clear();
        layoutMatrix = [];
    }
    
    // Async versions for programmatic use
    async function isEncoderCellAsync(row, col) {
        if (!isConnected) return false;
        const cellType = await getCachedLayoutCellType(row, col);
        return cellType === 2; // LAYOUT_ENCODER
    }
    
    async function isSwitchCellAsync(row, col) {
        if (!isConnected) return false;
        const cellType = await getCachedLayoutCellType(row, col);
        return cellType === 1; // LAYOUT_SWITCH
    }
    
    async function isSliderCellAsync(row, col) {
        if (!isConnected) return false;
        const cellType = await getCachedLayoutCellType(row, col);
        return cellType === 3; // LAYOUT_SLIDER
    }

    async function encoderIdForCellAsync(row, col) {
        if (!await isEncoderCellAsync(row, col)) return null;
        
        // Use the synchronous version for better performance once matrix is loaded
        if (layoutMatrix && layoutMatrix[row] && layoutMatrix[row][col]) {
            return layoutMatrix[row][col].isEncoder ? layoutMatrix[row][col].componentId : null;
        }
        
        try {
            const componentId = await invoke('get_layout_cell_component_id', { row, col });
            return componentId;
        } catch (e) {
            console.error('Failed to get encoder component ID:', e);
            return null;
        }
    }
    
    // Synchronous version for template use
    function encoderIdForCellSync(row, col) {
        if (!isEncoderCellSync(row, col)) return null;
        return getComponentId(row, col);
    }
    
    // Template-compatible synchronous functions that use the layout matrix
    function isEncoderCell(row, col) {
        return isEncoderCellSync(row, col);
    }
    
    function isSliderCell(row, col) {
        return isSliderCellSync(row, col);
    }
    
    function isPotentiometerCell(row, col) {
        return getLayoutCell(row, col).isPotentiometer;
    }
    
    function isMagneticSwitchCell(row, col) {
        return getLayoutCell(row, col).isMagneticSwitch;
    }
    
    function isEmptyCell(row, col) {
        return getLayoutCell(row, col).isEmpty;
    }
    
    function encoderIdForCell(row, col) {
        return encoderIdForCellSync(row, col);
    }

    function getEncoderEntryById(encoderId) {
        return getCurrentEncoders().find((encoder) => encoder.encoder_id === encoderId) || null;
    }

    // Slider functions
    function sliderIdForCell(row, col) {
        return getComponentId(row, col);
    }
    
    function getSliderValue(sliderId) {
        // Return actual value if available, otherwise 0 (which means no data yet)
        return sliderValues[sliderId] !== undefined ? sliderValues[sliderId] : 0;
    }
    
    function getSliderConfig(sliderId) {
        return sliderConfigs[sliderId] || {
            midi_channel: 0,
            midi_cc: sliderId + 1, // Use sliderId+1 as default CC so pot 0 = CC1, pot 1 = CC2
            min_midi_value: 0,
            max_midi_value: 127
        };
    }
    
    function getSliderPosition(sliderId) {
        const value = getSliderValue(sliderId);
        const config = getSliderConfig(sliderId);
        const percentage = ((value - config.min_midi_value) / (config.max_midi_value - config.min_midi_value)) * 100;
        
        // Account for thumb height (12px) relative to track height (240px)
        // This prevents the thumb from extending beyond the track boundaries
        const thumbHeightPercent = (12 / 240) * 100; // ~5%
        const maxPosition = 100 - thumbHeightPercent;
        
        return Math.max(0, Math.min(maxPosition, percentage));
    }
    
    function getPotentiometerAngle(sliderId) {
        const value = getSliderValue(sliderId);  // Use slider value function
        const config = getSliderConfig(sliderId);  // Use slider config function
        const percentage = ((value - config.min_midi_value) / (config.max_midi_value - config.min_midi_value));
        
        // Convert to angle: 0% = -135°, 100% = +135° (270° total range)
        const minAngle = -135;
        const maxAngle = 135;
        const angle = minAngle + (percentage * (maxAngle - minAngle));
        
        return Math.max(minAngle, Math.min(maxAngle, angle));
    }
    
    // Magnetic switch functions
    // We need to map layout component IDs (component_id) to hardware indices (0..N-1)
    // `magneticComponentToIndex` maps component_id -> hardware index used by firmware
    let magneticComponentToIndex = {};

    function magneticSwitchIdForCell(row, col) {
        const compId = getComponentId(row, col);
        if (compId === null || compId === undefined) return null;
        // Return the hardware index (fallback to compId if mapping missing)
        return magneticComponentToIndex.hasOwnProperty(compId) ? magneticComponentToIndex[compId] : null;
    }
    
    function getMagneticSwitchValue(switchId) {
        // Return actual value if available, otherwise 0 (which means no data yet)
        return magneticSwitchValues[switchId] !== undefined ? magneticSwitchValues[switchId] : 0;
    }
    
    function getMagneticSwitchConfig(switchId) {
        return magneticSwitchConfigs[switchId] || {
            layer: selectedLayer,
            switch_id: switchId,
            unpressed_value: 0,
            pressed_value: 4095,
            sensitivity: 50,
            keycode: 0x04, // KC_A as default
            is_calibrated: false
        };
    }
    
    function getMagneticSwitchPercentage(switchId) {
        const value = getMagneticSwitchValue(switchId);
        return Math.max(0, Math.min(100, value));
    }
    
    function isMagneticSwitchPressed(switchId) {
        const percentage = getMagneticSwitchPercentage(switchId);
        const config = getMagneticSwitchConfig(switchId);
        return percentage >= config.sensitivity;
    }
    
    function initializeSliderData() {
        // Initialize slider configurations and values based on board layout
        const sliderIds = [];
        
        // Get all slider IDs from the board layout
        if (boardLayout && boardLayout.layout) {
            for (let row = 0; row < (boardLayout.rows || boardLayout.matrix_rows); row++) {
                for (let col = 0; col < (boardLayout.cols || boardLayout.matrix_cols); col++) {
                    const cellIndex = row * (boardLayout.cols || boardLayout.matrix_cols) + col;
                    const cell = boardLayout.layout[cellIndex];
                    if (!cell) continue;

                    // Support different serializers/shapes coming from backend:
                    // - cell.type (string like 'SLIDER' or 'Slider')
                    // - cell.cell_type (string or numeric)
                    // - cell.component_id vs componentId
                    const rawType = cell.type ?? cell.cell_type ?? cell.cellType ?? null;
                    let isSlider = false;
                    if (rawType !== null && rawType !== undefined) {
                        if (typeof rawType === 'number') {
                            isSlider = rawType === 3 || rawType === 4; // Include both sliders (3) and potentiometers (4)
                        } else if (typeof rawType === 'string') {
                            isSlider = rawType.toLowerCase() === 'slider' || rawType.toLowerCase() === 'sliders' || rawType.toLowerCase() === 'slider' || rawType.toLowerCase() === 'potentiometer';
                        }
                    }

                    const compId = cell.component_id ?? cell.componentId ?? cell.componentid ?? cell.component ?? 0;
                    if (isSlider) {
                        sliderIds.push(compId);
                    }
                }
            }
        }
        
        // If no sliders found in layout, fallback to slider 0 for backward compatibility
        if (sliderIds.length === 0) {
            sliderIds.push(0);
        }
        
        // Initialize configurations and values for all found sliders
        sliderConfigs = {};
        sliderValues = {};
        
        sliderIds.forEach((sliderId, index) => {
            sliderConfigs[sliderId] = {
                midi_channel: 0,
                midi_cc: index + 1, // Default to CC 1, 2, 3, etc.
                min_midi_value: 0,
                max_midi_value: 127
            };
            sliderValues[sliderId] = 64; // Start at middle position
        });
        
        console.log('Initialized slider data:', { sliderConfigs, sliderValues, foundSliders: sliderIds });
    }
    
    function initializeMagneticSwitchData() {
        // Initialize magnetic switch configurations and values based on board layout
        const magneticSwitchIds = [];
        
        // Get all magnetic switch IDs from the board layout
        if (boardLayout && boardLayout.layout) {
            for (let row = 0; row < (boardLayout.rows || boardLayout.matrix_rows); row++) {
                for (let col = 0; col < (boardLayout.cols || boardLayout.matrix_cols); col++) {
                    const cellIndex = row * (boardLayout.cols || boardLayout.matrix_cols) + col;
                    const cell = boardLayout.layout[cellIndex];
                    if (!cell) continue;

                    const rawType = cell.type ?? cell.cell_type ?? cell.cellType ?? null;
                    let isMagneticSwitch = false;
                    if (rawType !== null && rawType !== undefined) {
                        if (typeof rawType === 'number') {
                            isMagneticSwitch = rawType === 5; // Magnetic switch type
                        } else if (typeof rawType === 'string') {
                            isMagneticSwitch = rawType.toLowerCase() === 'magneticswitch' || rawType.toLowerCase() === 'magnetic_switch';
                        }
                    }

                    const compId = cell.component_id ?? cell.componentId ?? cell.componentid ?? cell.component ?? 0;
                    if (isMagneticSwitch) {
                        magneticSwitchIds.push(compId);
                    }
                }
            }
        }
        
        // If no magnetic switches found in layout, fallback to switch 0 for testing
        if (magneticSwitchIds.length === 0) {
            magneticSwitchIds.push(0);
        }
        
        // Initialize configurations and values for all found magnetic switches
        // Map component IDs to hardware indices (0..N-1). Firmware expects switch indices, not layout component IDs.
        magneticSwitchConfigs = {};
        magneticSwitchValues = {};
        calibrationMode = {};
        magneticComponentToIndex = {};

        magneticSwitchIds.forEach((compId, index) => {
            // hardware index is `index`
            magneticComponentToIndex[compId] = index;
            magneticSwitchConfigs[index] = {
                layer: selectedLayer,
                switch_id: index,
                unpressed_value: 0,
                pressed_value: 4095,
                sensitivity: 50,
                keycode: 0x04 + index, // KC_A, KC_B, KC_C, etc.
                is_calibrated: false
            };
            magneticSwitchValues[index] = 0; // Start at unpressed
            calibrationMode[index] = null;
        });
        
        console.log('Initialized magnetic switch data:', { magneticSwitchConfigs, magneticSwitchValues, foundSwitches: magneticSwitchIds, mapping: magneticComponentToIndex });
    }
    
    // Real-time slider value polling
    async function pollSliderValues() {
        if (!isConnected) return;
        
        try {
            // For potentiometer keyboard, directly poll sliders 0 and 1
            // since we know they exist from the layout initialization
            const sliderIds = [0, 1];
            console.log(`DEBUG: Directly polling slider IDs: [${sliderIds.join(', ')}]`);
            
            // Poll each slider's current raw value
            for (const sliderId of sliderIds) {
                try {
                    const value = await invoke('get_slider_value', { sliderId });
                    console.log(`DEBUG: Got slider ${sliderId} value = ${value}`);
                    sliderValues[sliderId] = value;
                } catch (e) {
                    console.warn(`Failed to poll slider ${sliderId} value:`, e);
                }
            }
            
            // Force reactivity update
            sliderValues = { ...sliderValues };
        } catch (e) {
            console.warn('Failed to poll slider values:', e);
        }
    }
    
    // Load slider configuration from device
    async function loadSliderConfig(sliderId) {
        try {
            console.log(`Loading slider ${sliderId} config for layer ${selectedLayer}...`);
            const config = await invoke('get_slider_config', { layer: selectedLayer, sliderId });
            sliderConfigs[sliderId] = config;
            console.log(`Loaded slider ${sliderId} config for layer ${selectedLayer}:`, config);
        } catch (e) {
            console.warn(`Failed to load slider ${sliderId} config:`, e);
            // Keep default config on error
        }
    }

    // Load configurations for all sliders found in the layout
    async function loadAllSliderConfigs() {
        if (!boardLayout) return;
        
        // Get all slider and potentiometer IDs from the board layout (potentiometers reuse slider backend)
        const sliderIds = [];
        if (boardLayout.layout) {
            for (let row = 0; row < (boardLayout.rows || boardLayout.matrix_rows); row++) {
                for (let col = 0; col < (boardLayout.cols || boardLayout.matrix_cols); col++) {
                    const cellIndex = row * (boardLayout.cols || boardLayout.matrix_cols) + col;
                    const cell = boardLayout.layout[cellIndex];
                    if (!cell) continue;

                    const rawType = cell.type ?? cell.cell_type ?? cell.cellType ?? null;
                    let isSlider = false;
                    if (rawType !== null && rawType !== undefined) {
                        if (typeof rawType === 'number') {
                            isSlider = rawType === 3 || rawType === 4; // Include both sliders (3) and potentiometers (4)
                        } else if (typeof rawType === 'string') {
                            isSlider = rawType.toLowerCase() === 'slider' || rawType.toLowerCase() === 'potentiometer';
                        }
                    }

                    const compId = cell.component_id ?? cell.componentId ?? cell.component ?? 0;
                    if (isSlider) {
                        sliderIds.push(compId);
                    }
                }
            }
        }
        
        // If no sliders/potentiometers found in layout, fallback to slider 0
        if (sliderIds.length === 0) {
            sliderIds.push(0);
        }
        
        // Load configuration for each slider/potentiometer
        for (const sliderId of sliderIds) {
            await loadSliderConfig(sliderId);
        }
        
        console.log('Loaded all slider configs:', sliderConfigs);
    }

    // Load configurations for all magnetic switches found in the layout
    async function loadAllMagneticSwitchConfigs() {
        if (!boardLayout) return;
        
        // Get all magnetic switch IDs from the board layout
        const magneticSwitchIds = [];
        if (boardLayout.layout) {
            for (let row = 0; row < (boardLayout.rows || boardLayout.matrix_rows); row++) {
                for (let col = 0; col < (boardLayout.cols || boardLayout.matrix_cols); col++) {
                    const cellIndex = row * (boardLayout.cols || boardLayout.matrix_cols) + col;
                    const cell = boardLayout.layout[cellIndex];
                    if (!cell) continue;

                    const rawType = cell.type ?? cell.cell_type ?? cell.cellType ?? null;
                    let isMagneticSwitch = false;
                    if (rawType !== null && rawType !== undefined) {
                        if (typeof rawType === 'number') {
                            isMagneticSwitch = rawType === 5; // Magnetic switch type
                        } else if (typeof rawType === 'string') {
                            isMagneticSwitch = rawType.toLowerCase() === 'magneticswitch' || rawType.toLowerCase() === 'magnetic_switch';
                        }
                    }

                    const compId = cell.component_id ?? cell.componentId ?? cell.component ?? 0;
                    if (isMagneticSwitch) {
                        magneticSwitchIds.push(compId);
                    }
                }
            }
        }
        
        // If no magnetic switches found in layout, fallback to switch 0 for testing
        if (magneticSwitchIds.length === 0) {
            magneticSwitchIds.push(0);
        }
        
        // Load configuration for each magnetic switch (use hardware index mapping)
        for (const compId of magneticSwitchIds) {
            const hwIndex = magneticComponentToIndex[compId];
            if (hwIndex !== undefined && hwIndex !== null) {
                await loadMagneticSwitchConfig(hwIndex);
            }
        }
        
        console.log('Loaded all magnetic switch configs:', magneticSwitchConfigs);
    }

    // Load configuration for a specific magnetic switch
    async function loadMagneticSwitchConfig(switchId) {
        try {
            const config = await invoke('get_magnetic_switch_config', { 
                layer: selectedLayer, 
                switchId 
            });
            magneticSwitchConfigs[switchId] = config;
        } catch (e) {
            console.error(`Failed to load magnetic switch ${switchId} config:`, e);
            // Keep default config if loading fails
        }
    }

    // Poll all magnetic switch values
    async function pollMagneticSwitchValues() {
        if (!isConnected) return;
        
        try {
            // Get all magnetic switch IDs from configs
            const switchIds = Object.keys(magneticSwitchConfigs).map(id => parseInt(id));
            
            // Poll each magnetic switch's current value
            for (const switchId of switchIds) {
                try {
                    const value = await invoke('get_magnetic_switch_value', { switchId });
                    magneticSwitchValues[switchId] = value;
                } catch (e) {
                    console.error(`Failed to get magnetic switch ${switchId} value:`, e);
                }
            }
        } catch (e) {
            console.error('Failed to poll magnetic switch values:', e);
        }
    }

    function closeEncoderMenu() {
        activeEncoderMenu = null;
    }
    
    function closeSliderMenu() {
        activeSliderMenu = null;
    }
    
    function closeMagneticSwitchMenu() {
        activeMagneticSwitchMenu = null;
    }
    
    function toggleSliderMenu(row, col, event) {
        event.stopPropagation();
        if (loadingEncoders || loadingKeymap || loadingLayout) {
            return;
        }
        
        const sliderId = sliderIdForCell(row, col);
        if (sliderId === null || sliderId === undefined) return;

        if (activeSliderMenu && activeSliderMenu.row === row && activeSliderMenu.col === col) {
            activeSliderMenu = null;
        } else {
            activeSliderMenu = { row, col, sliderId };
        }
        
        // Close encoder menu if open
        activeEncoderMenu = null;
    }
    
    function toggleMagneticSwitchMenu(row, col, event) {
        event.stopPropagation();
        if (loadingEncoders || loadingKeymap || loadingLayout) {
            return;
        }
        
        const switchId = magneticSwitchIdForCell(row, col);
        if (switchId === null || switchId === undefined) return;

        if (activeMagneticSwitchMenu && activeMagneticSwitchMenu.row === row && activeMagneticSwitchMenu.col === col) {
            activeMagneticSwitchMenu = null;
        } else {
            activeMagneticSwitchMenu = { row, col, switchId };
        }
        
        // Close other menus if open
        activeEncoderMenu = null;
        activeSliderMenu = null;
    }

    // Magnetic switch calibration functions
    async function startMagneticSwitchCalibration(switchId) {
        try {
            await invoke('calibrate_magnetic_switch', { switchId, step: 0 }); // Start calibration
            calibrationMode[switchId] = 'ready';
            console.log(`Started calibration for magnetic switch ${switchId}`);
        } catch (e) {
            console.error(`Failed to start calibration for magnetic switch ${switchId}:`, e);
        }
    }

    async function setMagneticSwitchUnpressed(switchId) {
        try {
            await invoke('calibrate_magnetic_switch', { switchId, step: 1 }); // Set unpressed value
            calibrationMode[switchId] = 'unpressed';
            console.log(`Set unpressed value for magnetic switch ${switchId}`);
        } catch (e) {
            console.error(`Failed to set unpressed value for magnetic switch ${switchId}:`, e);
        }
    }

    async function setMagneticSwitchPressed(switchId) {
        try {
            await invoke('calibrate_magnetic_switch', { switchId, step: 2 }); // Set pressed value
            calibrationMode[switchId] = 'pressed';
            console.log(`Set pressed value for magnetic switch ${switchId}`);
        } catch (e) {
            console.error(`Failed to set pressed value for magnetic switch ${switchId}:`, e);
        }
    }

    async function completeMagneticSwitchCalibration(switchId) {
        try {
            await invoke('calibrate_magnetic_switch', { switchId, step: 3 }); // Complete calibration
            calibrationMode[switchId] = 'complete';
            await loadMagneticSwitchConfig(switchId); // Reload config to get updated values
            console.log(`Completed calibration for magnetic switch ${switchId}`);
        } catch (e) {
            console.error(`Failed to complete calibration for magnetic switch ${switchId}:`, e);
        }
    }

    async function setMagneticSwitchSensitivity(switchId, sensitivity) {
        try {
            await invoke('set_magnetic_switch_sensitivity', { switchId, sensitivity });
            await loadMagneticSwitchConfig(switchId); // Reload config to get updated sensitivity
            console.log(`Set sensitivity for magnetic switch ${switchId} to ${sensitivity}%`);
        } catch (e) {
            console.error(`Failed to set sensitivity for magnetic switch ${switchId}:`, e);
        }
    }
    
    function updateSliderValue(sliderId, newValue) {
        sliderValues[sliderId] = Math.max(0, Math.min(127, newValue));
        console.log(`Slider ${sliderId} value updated to:`, sliderValues[sliderId]);
    }
    
    async function updateSliderConfig(sliderId, config) {
        // Update local state
        sliderConfigs[sliderId] = { ...sliderConfigs[sliderId], ...config };
        console.log(`Slider ${sliderId} config updated:`, sliderConfigs[sliderId]);
        
        // Apply changes immediately to device and save to EEPROM
        try {
            const deviceConfig = {
                ...sliderConfigs[sliderId],
                layer: selectedLayer,
                slider_id: sliderId
            };
            await invoke('set_slider_config', { config: deviceConfig });
            console.log(`Saved slider ${sliderId} config to device EEPROM immediately`);
            
            // Update original data since we just saved to EEPROM
            storeOriginalData();
        } catch (e) {
            console.error(`Failed to save slider ${sliderId} config to device:`, e);
        }
    }

    /** @param {PointerEvent} event */
    function handleGlobalPointerDown(event) {
        if (!activeEncoderMenu && !activeSliderMenu) return;
        const target = event.target;
        if (!(target instanceof Element)) {
            return;
        }

        // Check if click is inside encoder menu or encoder key
        if (activeEncoderMenu && (target.closest('.encoder-menu') || target.closest('.encoder-key'))) {
            return;
        }
        
        // Check if click is inside slider menu or slider container
        if (activeSliderMenu && (target.closest('.slider-menu') || target.closest('.slider-container'))) {
            return;
        }

        // Close menus if clicked outside
        if (activeEncoderMenu) closeEncoderMenu();
        if (activeSliderMenu) closeSliderMenu();
        if (activeMagneticSwitchMenu) closeMagneticSwitchMenu();
    }

    /** @param {KeyboardEvent} event */
    function handleGlobalKeydown(event) {
        if (!activeEncoderMenu && !activeSliderMenu && !activeMagneticSwitchMenu) return;
        if (event.key === 'Escape') {
            if (activeEncoderMenu) closeEncoderMenu();
            if (activeSliderMenu) closeSliderMenu();
            if (activeMagneticSwitchMenu) closeMagneticSwitchMenu();
        }
    }

    function toggleEncoderMenu(row, col, event) {
        if (event) {
            event.stopPropagation();
        }
        if (loadingEncoders || loadingKeymap || loadingLayout) {
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
            layer: selectedLayer,
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

    async function updateEncoder(encoderId, ccwKeycode, cwKeycode, layerOverride = selectedLayer) {
        const layer = layerOverride;
        try {
            if (selectedDevice === 'main') {
                await invoke('set_encoder_entry', {
                    entry: {
                        layer,
                        encoder_id: encoderId,
                        ccw_keycode: ccwKeycode,
                        cw_keycode: cwKeycode,
                        reserved: 0
                    }
                });

                const layers = [...encoders];
                const layerEncoders = [...(layers[layer] ?? [])];
                const encoderIndex = layerEncoders.findIndex(e => e.encoder_id === encoderId);
                const reserved = encoderIndex >= 0 ? layerEncoders[encoderIndex].reserved ?? 0 : 0;
                const updatedEntry = {
                    layer,
                    encoder_id: encoderId,
                    ccw_keycode: ccwKeycode,
                    cw_keycode: cwKeycode,
                    reserved,
                };
                if (encoderIndex >= 0) {
                    layerEncoders[encoderIndex] = {
                        ...layerEncoders[encoderIndex],
                        ...updatedEntry,
                    };
                } else {
                    layerEncoders.push(updatedEntry);
                }
                layers[layer] = layerEncoders;
                encoders = layers;
            } else {
                const slaveAddr = parseInt(selectedDevice, 10);
                if (!slaveEncoders[slaveAddr]) {
                    slaveEncoders[slaveAddr] = [];
                }
                if (!slaveEncoders[slaveAddr][layer]) {
                    slaveEncoders[slaveAddr][layer] = [];
                }
                const existingEncoders = [...(slaveEncoders[slaveAddr][layer] ?? [])];
                const entryIndex = existingEncoders.findIndex(e => e.encoder_id === encoderId);
                const reserved = entryIndex >= 0 ? existingEncoders[entryIndex].reserved ?? 0 : 0;

                await invoke('set_slave_encoder_entry', {
                    entry: {
                        slave_addr: slaveAddr,
                        layer,
                        encoder_id: encoderId,
                        ccw_keycode: ccwKeycode,
                        cw_keycode: cwKeycode,
                        reserved
                    }
                });

                if (entryIndex >= 0) {
                    existingEncoders[entryIndex] = {
                        ...existingEncoders[entryIndex],
                        layer,
                        ccw_keycode: ccwKeycode,
                        cw_keycode: cwKeycode,
                        reserved,
                    };
                } else {
                    existingEncoders.push({
                        slave_addr: slaveAddr,
                        layer,
                        encoder_id: encoderId,
                        ccw_keycode: ccwKeycode,
                        cw_keycode: cwKeycode,
                        reserved
                    });
                }

                const slaveLayers = [...(slaveEncoders[slaveAddr] ?? [])];
                slaveLayers[layer] = existingEncoders;
                slaveEncoders = {
                    ...slaveEncoders,
                    [slaveAddr]: slaveLayers
                };
            }
            
            // Update original data since changes are now saved to EEPROM
            storeOriginalData();
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

    const OP_LAYER_ID_MASK = 0x1f;
    const OP_TO_BASE = 0x5200;
    const OP_TO_MAX = 0x521F;
    const OP_MOMENTARY_BASE = 0x5220;
    const OP_MOMENTARY_MAX = 0x523F;

    // MIDI encoding helpers
    const OP_MIDI_CC_BASE = 0x7E10;

    const standardKeyCategories = new Set([
        'Letters',
        'Numbers',
        'Punctuation',
        'Function',
        'Navigation',
        'Modifiers',
        'Keypad'
    ]);

    const STANDARD_KEY_LAYOUT = [
        {
            offset: 0,
            keys: [
                { code: 'KC_ESCAPE', legend: ['Esc'] },
                { spacer: 1 },
                { code: 'KC_F1', legend: ['F1'] },
                { code: 'KC_F2', legend: ['F2'] },
                { code: 'KC_F3', legend: ['F3'] },
                { code: 'KC_F4', legend: ['F4'] },
                { spacer: 0.5 },
                { code: 'KC_F5', legend: ['F5'] },
                { code: 'KC_F6', legend: ['F6'] },
                { code: 'KC_F7', legend: ['F7'] },
                { code: 'KC_F8', legend: ['F8'] },
                { spacer: 0.5 },
                { code: 'KC_F9', legend: ['F9'] },
                { code: 'KC_F10', legend: ['F10'] },
                { code: 'KC_F11', legend: ['F11'] },
                { code: 'KC_F12', legend: ['F12'] },
                { spacer: 0.25 },
                { code: 'KC_PSCR', legend: ['PrtSc'] },
                { code: 'KC_SCRL', legend: ['Scroll', 'Lock'] },
                { code: 'KC_PAUS', legend: ['Pause', 'Break'] }
            ]
        },
        {
            offset: 0.5,
            keys: [
                { code: 'KC_GRAVE', legend: ['~', '`'] },
                { code: 'KC_1', legend: ['!', '1'] },
                { code: 'KC_2', legend: ['@', '2'] },
                { code: 'KC_3', legend: ['#', '3'] },
                { code: 'KC_4', legend: ['$', '4'] },
                { code: 'KC_5', legend: ['%', '5'] },
                { code: 'KC_6', legend: ['^', '6'] },
                { code: 'KC_7', legend: ['&', '7'] },
                { code: 'KC_8', legend: ['*', '8'] },
                { code: 'KC_9', legend: ['(', '9'] },
                { code: 'KC_0', legend: [')', '0'] },
                { code: 'KC_MINUS', legend: ['_', '-'] },
                { code: 'KC_EQUAL', legend: ['+', '='] },
                { code: 'KC_BACKSPACE', legend: ['Backspace'], width: 2 },
                { spacer: 0.25 },
                { code: 'KC_INSERT', legend: ['Insert'] },
                { code: 'KC_HOME', legend: ['Home'] },
                { code: 'KC_PGUP', legend: ['PgUp'] },
                { spacer: 0.25 },
                { code: 'KC_NUMLOCK', legend: ['Num', 'Lock'] },
                { code: 'KC_KP_SLASH', legend: ['/'] },
                { code: 'KC_KP_ASTERISK', legend: ['*'] },
                { code: 'KC_KP_MINUS', legend: ['-'] }
            ]
        },
        {
            offset: 0,
            keys: [
                { code: 'KC_TAB', legend: ['Tab'], width: 1.5 },
                { code: 'KC_Q', legend: ['Q'] },
                { code: 'KC_W', legend: ['W'] },
                { code: 'KC_E', legend: ['E'] },
                { code: 'KC_R', legend: ['R'] },
                { code: 'KC_T', legend: ['T'] },
                { code: 'KC_Y', legend: ['Y'] },
                { code: 'KC_U', legend: ['U'] },
                { code: 'KC_I', legend: ['I'] },
                { code: 'KC_O', legend: ['O'] },
                { code: 'KC_P', legend: ['P'] },
                { code: 'KC_LEFT_BRACKET', legend: ['{', '['] },
                { code: 'KC_RIGHT_BRACKET', legend: ['}', ']'] },
                { code: 'KC_BACKSLASH', legend: ['|', '\\'], width: 1.5 },
                { spacer: 0.25 },
                { code: 'KC_DELETE', legend: ['Delete'] },
                { code: 'KC_END', legend: ['End'] },
                { code: 'KC_PGDN', legend: ['PgDn'] },
                { spacer: 0.25 },
                { code: 'KC_KP_7', legend: ['7', 'Home'] },
                { code: 'KC_KP_8', legend: ['8', '↑'] },
                { code: 'KC_KP_9', legend: ['9', 'PgUp'] },
                { code: 'KC_KP_PLUS', legend: ['+'], height: 2 }
            ]
        },
        {
            offset: 0,
            keys: [
                { code: 'KC_CAPSLOCK', legend: ['Caps', 'Lock'], width: 1.75 },
                { code: 'KC_A', legend: ['A'] },
                { code: 'KC_S', legend: ['S'] },
                { code: 'KC_D', legend: ['D'] },
                { code: 'KC_F', legend: ['F'] },
                { code: 'KC_G', legend: ['G'] },
                { code: 'KC_H', legend: ['H'] },
                { code: 'KC_J', legend: ['J'] },
                { code: 'KC_K', legend: ['K'] },
                { code: 'KC_L', legend: ['L'] },
                { code: 'KC_SEMICOLON', legend: [':', ';'] },
                { code: 'KC_QUOTE', legend: ["\"", "'"] },
                { code: 'KC_ENTER', legend: ['Enter'], width: 2.25 },
                { spacer: 3.5 },
                { code: 'KC_KP_4', legend: ['4', '←'] },
                { code: 'KC_KP_5', legend: ['5'] },
                { code: 'KC_KP_6', legend: ['6', '→'] }
            ]
        },
        {
            offset: 0,
            keys: [
                { code: 'KC_LEFT_SHIFT', legend: ['Shift'], width: 2.25 },
                { code: 'KC_Z', legend: ['Z'] },
                { code: 'KC_X', legend: ['X'] },
                { code: 'KC_C', legend: ['C'] },
                { code: 'KC_V', legend: ['V'] },
                { code: 'KC_B', legend: ['B'] },
                { code: 'KC_N', legend: ['N'] },
                { code: 'KC_M', legend: ['M'] },
                { code: 'KC_COMMA', legend: ['<', ','] },
                { code: 'KC_DOT', legend: ['>', '.'] },
                { code: 'KC_SLASH', legend: ['?', '/'] },
                { code: 'KC_RIGHT_SHIFT', legend: ['Shift'], width: 2.75 },
                { spacer: 1.25 },
                { code: 'KC_UP', legend: ['↑'] },
                { spacer: 1.25 },
                { code: 'KC_KP_1', legend: ['1', 'End'] },
                { code: 'KC_KP_2', legend: ['2', '↓'] },
                { code: 'KC_KP_3', legend: ['3', 'PgDn'] },
                { code: 'KC_KP_ENTER', legend: ['Enter'], height: 2 }
            ]
        },
        {
            offset: 0,
            keys: [
                { code: 'KC_LEFT_CTRL', legend: ['Ctrl'], width: 1.25 },
                { code: 'KC_LEFT_GUI', legend: ['Win'], width: 1.25 },
                { code: 'KC_LEFT_ALT', legend: ['Alt'], width: 1.25 },
                { code: 'KC_SPACE', legend: ['Space'], width: 6.25 },
                { code: 'KC_RIGHT_ALT', legend: ['Alt'], width: 1.25 },
                { code: 'KC_RIGHT_GUI', legend: ['Win'], width: 1.25 },
                { code: 'KC_APP', legend: ['Menu'], width: 1.25 },
                { code: 'KC_RIGHT_CTRL', legend: ['Ctrl'], width: 1.25 },
                { spacer: 0.25 },
                { code: 'KC_LEFT', legend: ['←'] },
                { code: 'KC_DOWN', legend: ['↓'] },
                { code: 'KC_RIGHT', legend: ['→'] },
                { spacer: 0.25 },
                { code: 'KC_KP_0', legend: ['0', 'Ins'], width: 2 },
                { code: 'KC_KP_DOT', legend: ['.', 'Del'] }
            ]
        }
    ];

    function lookupKeycodeByName(name) {
        if (!name) {
            return null;
        }
        return keycodeByName?.[name] ?? null;
    }

    function resolvedStandardKeyLayout() {
        return STANDARD_KEY_LAYOUT.map((row) => ({
            offset: row.offset ?? 0,
            keys: row.keys.map((item) => {
                if (item.spacer !== undefined) {
                    return item;
                }
                const codeValue = item.code ? lookupKeycodeByName(item.code) ?? null : null;
                return {
                    ...item,
                    codeValue
                };
            })
        }));
    }

    
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

    function layerKeycodeInfo(code) {
        if (typeof code !== 'number') return null;
        const layer = code & OP_LAYER_ID_MASK;
        if (code >= OP_TO_BASE && code <= OP_TO_MAX) {
            return {
                layer,
                label: `Change layer (${layer})`,
                name: `KC_TO(${layer})`,
                category: 'Layers',
                kind: 'persistent'
            };
        }
        if (code >= OP_MOMENTARY_BASE && code <= OP_MOMENTARY_MAX) {
            return {
                layer,
                label: `Momentary layer (${layer})`,
                name: `KC_MO(${layer})`,
                category: 'Layers',
                kind: 'momentary'
            };
        }
        return null;
    }

    function formatKeyLabel(code) {
        if (typeof code !== 'number') return [''];
        const layerInfo = layerKeycodeInfo(code);
        if (layerInfo) {
            return [layerInfo.label];
        }
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
        const baseOptions = Object.entries(keycodes).map(([code, keycode]) => ({
            value: parseInt(code),
            label: `${keycode.display_name}`,
            name: keycode.name,
            category: keycode.category
        }));

        const merged = new Map();
        for (const option of baseOptions) {
            if (!merged.has(option.value)) {
                merged.set(option.value, option);
            }
        }

        return Array.from(merged.values());
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
        modalKey = { row, col, keycode, layer: selectedLayer };
        const info = layerKeycodeInfo(keycode);
        const limit = layerKeyLimit();

        if (info) {
            layerKeyType = info.kind === 'momentary' ? 'momentary' : 'persistent';
            let target = Number(info.layer ?? 0);
            if (!Number.isFinite(target)) {
                target = 0;
            }
            target = Math.max(0, Math.min(target, limit - 1));
            layerKeyLayer = target;
            keyModalTab = 'layer';
        } else {
            layerKeyType = 'persistent';
            let defaultLayer = clampLayerIndex(selectedLayer);
            defaultLayer = Math.max(0, Math.min(defaultLayer, limit - 1));
            layerKeyLayer = defaultLayer;
            keyModalTab = 'standard';
        }
        showKeyModal = true;
        activeEncoderMenu = null;
    }

    function closeKeyModal() {
        showKeyModal = false;
        modalKey = null;
    }

    function applyKeyChange() {
        if (modalKey) {
            updateKeymap(modalKey.row, modalKey.col, modalKey.keycode, modalKey.layer ?? selectedLayer);
            closeKeyModal();
        }
    }

    function openEncoderModal(encoder, direction) {
        modalEncoder = { ...encoder };
        if (modalEncoder.layer === undefined) {
            modalEncoder.layer = selectedLayer;
        }
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
                modalEncoder.cw_keycode,
                modalEncoder.layer ?? selectedLayer
            );
            closeEncoderModal();
        }
    }

    // Layer switch form state
    let layerKeyType = $state('persistent');
    let layerKeyLayer = $state(0);

    function layerKeyLimit() {
        const total = Number.isFinite(layerCount) ? layerCount : 1;
        const bounded = Math.min(Math.max(total, 1), OP_LAYER_ID_MASK + 1);
        return bounded;
    }

    function layerKeySelectableIndices() {
        const limit = layerKeyLimit();
        return Array.from({ length: limit }, (_, i) => i);
    }

    function updateLayerKeySelection() {
        const limit = layerKeyLimit();
        let chosen = Number(layerKeyLayer);
        if (!Number.isFinite(chosen)) {
            chosen = 0;
        }
        chosen = Math.max(0, Math.min(chosen, limit - 1));
        layerKeyLayer = chosen;

        const base = layerKeyType === 'momentary' ? OP_MOMENTARY_BASE : OP_TO_BASE;
        const code = base + chosen;
        if (modalKey) {
            modalKey.keycode = code;
            modalKey = modalKey; // trigger reactivity
        }
    }

    function initLayerTab() {
        const info = modalKey ? layerKeycodeInfo(modalKey.keycode) : null;
        const limit = layerKeyLimit();

        if (info) {
            layerKeyType = info.kind === 'momentary' ? 'momentary' : 'persistent';
            let target = Number(info.layer ?? 0);
            if (!Number.isFinite(target)) {
                target = 0;
            }
            target = Math.max(0, Math.min(target, limit - 1));
            layerKeyLayer = target;
            return;
        }

        layerKeyType = 'persistent';
        let defaultLayer = clampLayerIndex(selectedLayer);
        defaultLayer = Math.max(0, Math.min(defaultLayer, limit - 1));
        layerKeyLayer = defaultLayer;
        updateLayerKeySelection();
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
        const stateToRestore = originalLayerState ? { ...originalLayerState } : null;
        updateLayerStateLocal(stateToRestore, { fromDevice: true });
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
                        
                    </div>
                    <h1 class="title">openGRADER Configurator</h1>
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

                    <div class="layer-toolbar" aria-label="Layer controls">
                        <div class="layer-buttons" role="tablist" aria-label="Layer selection">
                            {#each layerIndices() as layerIndex}
                                <button
                                    type="button"
                                    class="layer-button"
                                    class:current={selectedLayer === layerIndex}
                                    class:active-hardware={hardwareActiveLayer === layerIndex}
                                    onclick={() => selectLayer(layerIndex)}
                                >
                                    <span class="layer-label">{layerIndex}</span>
                                    {#if isDefaultLayer(layerIndex)}
                                        <span class="layer-pill">D</span>
                                    {/if}
                                    {#if hardwareActiveLayer === layerIndex}
                                        <span class="layer-pill active">A</span>
                                    {/if}
                                    {#if momentaryLayer === layerIndex}
                                        <span class="layer-pill momentary">M</span>
                                    {/if}
                                    
                                </button>
                            {/each}
                        </div>

                        {#if selectedDevice === 'main'}
                            <div class="layer-state-controls">
                                <button
                                    type="button"
                                    class="secondary-button"
                                    onclick={() => setActiveLayer(selectedLayer)}
                                    disabled={layerStateBusy || isSoleActiveLayer(selectedLayer)}
                                >
                                    Set Active
                                </button>
                                <button
                                    type="button"
                                    class="secondary-button"
                                    onclick={() => setDefaultLayer(selectedLayer)}
                                    disabled={layerStateBusy || isDefaultLayer(selectedLayer)}
                                >
                                    Set as Default
                                </button>
                               
                               
                            </div>
                        {/if}
                    </div>
                    
                    {#if loadingKeymap || loadingLayout || (getCurrentKeymap().length > 0 && !layoutMatrixReady)}
                        <div class="loading-state">
                            <div class="spinner-large"></div>
                            <h3>{loadingLayout || !layoutMatrixReady ? 'Loading Layout...' : 'Loading Keymap...'}</h3>
                            <p>{loadingLayout || !layoutMatrixReady ? 'Detecting switch and encoder positions' : (selectedDevice === 'main' ? 'Fetching key configuration from device' : `Loading keymap from slave device 0x${parseInt(selectedDevice, 10).toString(16).toUpperCase()}`)}</p>
                        </div>
                    {:else if getCurrentKeymap().length > 0 && layoutMatrixReady}
                        <!-- keys open modal -->
                        <div class="keymap-container">
                            <div
                                class="keymap"
                                class:loading={loadingKeymap || loadingEncoders || loadingLayout}
                                aria-busy={loadingKeymap || loadingEncoders || loadingLayout}
                                style="--board-cols: {boardLayout?.cols || boardLayout?.matrix_cols || 3};"
                            >
                                {#each getCurrentKeymap() as row, rowIndex}
                                    <div class="keymap-row">
                                        {#each row as key, colIndex}
                                            {#if !isEmptyCell(rowIndex, colIndex)}
                                                <div class={`keymap-cell ${isEncoderCell(rowIndex, colIndex) ? 'encoder-cell' : ''} ${isSliderCell(rowIndex, colIndex) ? 'slider-cell' : ''} ${isPotentiometerCell(rowIndex, colIndex) ? 'potentiometer-cell' : ''} ${isMagneticSwitchCell(rowIndex, colIndex) ? 'magnetic-switch-cell' : ''} ${activeEncoderMenu && activeEncoderMenu.row === rowIndex && activeEncoderMenu.col === colIndex ? 'active-encoder' : ''}`}>
                                                {#if isSliderCell(rowIndex, colIndex)}
                                                    <!-- Slider component -->
                                                    {@const sliderId = sliderIdForCell(rowIndex, colIndex)}
                                                    {@const sliderValue = getSliderValue(sliderId)}
                                                    {@const sliderConfig = getSliderConfig(sliderId)}
                                                    {@const sliderPosition = getSliderPosition(sliderId)}
                                                    <button
                                                        class="slider-container"
                                                        onclick={(event) => toggleSliderMenu(rowIndex, colIndex, event)}
                                                        aria-label={`Slider ${sliderId} at R${rowIndex}C${colIndex}, value ${sliderValue}`}
                                                    >
                                                        <div class="slider-track">
                                                            <div class="slider-thumb" style="bottom: {sliderPosition}%"></div>
                                                        </div>
                                                        <div class="slider-info">
                                                            <div class="slider-value">{sliderValue}</div>
                                                            <div class="slider-range">{sliderConfig.min_midi_value}-{sliderConfig.max_midi_value}</div>
                                                            <div class="slider-cc">CC{sliderConfig.midi_cc}</div>
                                                            <div class="slider-channel">CH{sliderConfig.midi_channel + 1}</div>
                                                        </div>
                                                    </button>
                                                    
                                                    <!-- Slider Configuration Menu -->
                                                    {#if activeSliderMenu && activeSliderMenu.row === rowIndex && activeSliderMenu.col === colIndex}
                                                        <div class="slider-menu active">
                                                            <div class="slider-config-panel">
                                                                <h3>Slider Configuration</h3>
                                                                <div class="slider-config-section">
                                                                    <label for="midi-channel-{sliderId}">MIDI Channel</label>
                                                                    <input 
                                                                        id="midi-channel-{sliderId}"
                                                                        type="number" 
                                                                        min="1" 
                                                                        max="16" 
                                                                        value={sliderConfig.midi_channel + 1}
                                                                        onchange={(e) => {
                                                                            let ch = parseInt(e.target.value) - 1;
                                                                            ch = Math.max(0, Math.min(15, ch));
                                                                            updateSliderConfig(sliderId, { midi_channel: ch });
                                                                        }}
                                                                    />
                                                                </div>
                                                                <div class="slider-config-section">
                                                                    <label for="midi-cc-{sliderId}">CC Number</label>
                                                                    <input 
                                                                        id="midi-cc-{sliderId}"
                                                                        type="number" 
                                                                        min="0" 
                                                                        max="127" 
                                                                        bind:value={sliderConfig.midi_cc}
                                                                        onchange={(e) => {
                                                                            let cc = parseInt(e.target.value);
                                                                            cc = Math.max(0, Math.min(127, cc));
                                                                            updateSliderConfig(sliderId, { midi_cc: cc });
                                                                        }}
                                                                    />
                                                                </div>
                                                                <div class="slider-config-section">
                                                                    <label for="min-value-{sliderId}">Min Value</label>
                                                                    <input 
                                                                        id="min-value-{sliderId}"
                                                                        type="number" 
                                                                        min="0" 
                                                                        max="127" 
                                                                        bind:value={sliderConfig.min_midi_value}
                                                                        onchange={(e) => {
                                                                            let min = parseInt(e.target.value);
                                                                            min = Math.max(0, Math.min(127, min));
                                                                            // Ensure min <= max
                                                                            if (min > sliderConfig.max_midi_value) {
                                                                                min = sliderConfig.max_midi_value;
                                                                            }
                                                                            updateSliderConfig(sliderId, { min_midi_value: min });
                                                                        }}
                                                                    />
                                                                </div>
                                                                <div class="slider-config-section">
                                                                    <label for="max-value-{sliderId}">Max Value</label>
                                                                    <input 
                                                                        id="max-value-{sliderId}"
                                                                        type="number" 
                                                                        min="0" 
                                                                        max="127" 
                                                                        bind:value={sliderConfig.max_midi_value}
                                                                        onchange={(e) => {
                                                                            let max = parseInt(e.target.value);
                                                                            max = Math.max(0, Math.min(127, max));
                                                                            // Ensure max >= min
                                                                            if (max < sliderConfig.min_midi_value) {
                                                                                max = sliderConfig.min_midi_value;
                                                                            }
                                                                            updateSliderConfig(sliderId, { max_midi_value: max });
                                                                        }}
                                                                    />
                                                                </div>
                                                            </div>
                                                        </div>
                                                    {/if}
                                                {:else if isPotentiometerCell(rowIndex, colIndex)}
                                                    <!-- Potentiometer component - uses same backend as sliders -->
                                                    {@const sliderId = sliderIdForCell(rowIndex, colIndex)}
                                                    {@const sliderValue = getSliderValue(sliderId)}
                                                    {@const sliderConfig = getSliderConfig(sliderId)}
                                                    {@const potAngle = getPotentiometerAngle(sliderId)}
                                                    <button
                                                        class="potentiometer-container"
                                                        onclick={(event) => toggleSliderMenu(rowIndex, colIndex, event)}
                                                        aria-label={`Potentiometer ${sliderId} at R${rowIndex}C${colIndex}, value ${sliderValue}`}
                                                    >
                                                        <div class="potentiometer-progress" style="--progress: {((sliderValue - sliderConfig.min_midi_value) / (sliderConfig.max_midi_value - sliderConfig.min_midi_value)) * 270}deg;"></div>
                                                        <div class="potentiometer-indicator" style="transform: translateX(-50%) rotate({potAngle}deg);"></div>
                                                        <div class="potentiometer-text">
                                                            <div>CH{sliderConfig.midi_channel + 1}</div>
                                                            <div>CC{sliderConfig.midi_cc}</div>
                                                            <div>{sliderValue}</div>
                                                        </div>
                                                    </button>
                                                    
                                                    <!-- Potentiometer Configuration Menu (reuses slider menu) -->
                                                    {#if activeSliderMenu && activeSliderMenu.row === rowIndex && activeSliderMenu.col === colIndex}
                                                        <div class="slider-menu active">
                                                            <div class="slider-config-panel">
                                                                <h3>Potentiometer Configuration</h3>
                                                                <div class="slider-config-section">
                                                                    <label for="midi-channel-{sliderId}">MIDI Channel</label>
                                                                    <input 
                                                                        id="midi-channel-{sliderId}"
                                                                        type="number" 
                                                                        min="1" 
                                                                        max="16" 
                                                                        value={sliderConfig.midi_channel + 1}
                                                                        onchange={(e) => {
                                                                            let ch = parseInt(e.target.value) - 1;
                                                                            ch = Math.max(0, Math.min(15, ch));
                                                                            updateSliderConfig(sliderId, { midi_channel: ch });
                                                                        }}
                                                                    />
                                                                </div>
                                                                <div class="slider-config-section">
                                                                    <label for="midi-cc-{sliderId}">CC Number</label>
                                                                    <input 
                                                                        id="midi-cc-{sliderId}"
                                                                        type="number" 
                                                                        min="0" 
                                                                        max="127" 
                                                                        bind:value={sliderConfig.midi_cc}
                                                                        onchange={(e) => {
                                                                            let cc = parseInt(e.target.value);
                                                                            cc = Math.max(0, Math.min(127, cc));
                                                                            updateSliderConfig(sliderId, { midi_cc: cc });
                                                                        }}
                                                                    />
                                                                </div>
                                                                <div class="slider-config-section">
                                                                    <label for="min-value-{sliderId}">Min Value</label>
                                                                    <input 
                                                                        id="min-value-{sliderId}"
                                                                        type="number" 
                                                                        min="0" 
                                                                        max="127" 
                                                                        bind:value={sliderConfig.min_midi_value}
                                                                        onchange={(e) => {
                                                                            let min = parseInt(e.target.value);
                                                                            min = Math.max(0, Math.min(127, min));
                                                                            if (min > sliderConfig.max_midi_value) {
                                                                                min = sliderConfig.max_midi_value;
                                                                            }
                                                                            updateSliderConfig(sliderId, { min_midi_value: min });
                                                                        }}
                                                                    />
                                                                </div>
                                                                <div class="slider-config-section">
                                                                    <label for="max-value-{sliderId}">Max Value</label>
                                                                    <input 
                                                                        id="max-value-{sliderId}"
                                                                        type="number" 
                                                                        min="0" 
                                                                        max="127" 
                                                                        bind:value={sliderConfig.max_midi_value}
                                                                        onchange={(e) => {
                                                                            let max = parseInt(e.target.value);
                                                                            max = Math.max(0, Math.min(127, max));
                                                                            if (max < sliderConfig.min_midi_value) {
                                                                                max = sliderConfig.min_midi_value;
                                                                            }
                                                                            updateSliderConfig(sliderId, { max_midi_value: max });
                                                                        }}
                                                                    />
                                                                </div>
                                                            </div>
                                                        </div>
                                                    {/if}
                                                {:else if isMagneticSwitchCell(rowIndex, colIndex)}
                                                    <!-- Magnetic Switch component -->
                                                    {@const switchId = magneticSwitchIdForCell(rowIndex, colIndex)}
                                                    {@const switchValue = getMagneticSwitchValue(switchId)}
                                                    {@const switchConfig = getMagneticSwitchConfig(switchId)}
                                                    {@const switchPercentage = getMagneticSwitchPercentage(switchId)}
                                                    {@const isPressed = isMagneticSwitchPressed(switchId)}
                                                    <button
                                                        class="magnetic-switch-container"
                                                        class:pressed={isPressed}
                                                        onclick={(event) => toggleMagneticSwitchMenu(rowIndex, colIndex, event)}
                                                        aria-label={`Magnetic Switch ${switchId} at R${rowIndex}C${colIndex}, value ${switchPercentage}%`}
                                                    >
                                                        <div class="magnetic-switch-visual">
                                                            <div class="magnetic-switch-bar">
                                                                <div class="magnetic-switch-fill" style="height: {switchPercentage}%;"></div>
                                                            </div>
                                                            <div class="magnetic-switch-threshold" style="bottom: {switchConfig.sensitivity}%;"></div>
                                                        </div>
                                                        <div class="magnetic-switch-text">
                                                            <div>MAG</div>
                                                            <div>{switchPercentage}%</div>
                                                            <div>{switchConfig.is_calibrated ? 'CAL' : 'RAW'}</div>
                                                        </div>
                                                    </button>
                                                    
                                                    <!-- Magnetic Switch Configuration Menu -->
                                                    {#if activeMagneticSwitchMenu && activeMagneticSwitchMenu.row === rowIndex && activeMagneticSwitchMenu.col === colIndex}
                                                        <div class="magnetic-switch-menu active">
                                                            <div class="magnetic-switch-config-panel">
                                                                <h3>Magnetic Switch Configuration</h3>
                                                                
                                                                <!-- Calibration Section -->
                                                                <div class="magnetic-switch-config-section">
                                                                    <span>Calibration</span>
                                                                    {#if !switchConfig.is_calibrated}
                                                                        <button onclick={() => startMagneticSwitchCalibration(switchId)}>
                                                                            Start Calibration
                                                                        </button>
                                                                        <p class="calibration-help">Calibration required for accurate trigger detection</p>
                                                                    {:else}
                                                                        <div class="calibration-status">
                                                                            <span class="calibrated">✓ Calibrated</span>
                                                                            <button onclick={() => startMagneticSwitchCalibration(switchId)}>
                                                                                Recalibrate
                                                                            </button>
                                                                        </div>
                                                                    {/if}
                                                                    
                                                                    {#if calibrationMode[switchId]}
                                                                        <div class="calibration-steps">
                                                                            {#if calibrationMode[switchId] === 'ready'}
                                                                                <p>Step 1: Release the switch, then click below</p>
                                                                                <button onclick={() => setMagneticSwitchUnpressed(switchId)}>
                                                                                    Set Unpressed Value
                                                                                </button>
                                                                            {:else if calibrationMode[switchId] === 'unpressed'}
                                                                                <p>Step 2: Fully press the switch, then click below</p>
                                                                                <button onclick={() => setMagneticSwitchPressed(switchId)}>
                                                                                    Set Pressed Value
                                                                                </button>
                                                                            {:else if calibrationMode[switchId] === 'pressed'}
                                                                                <p>Step 3: Calibration complete</p>
                                                                                <button onclick={() => completeMagneticSwitchCalibration(switchId)}>
                                                                                    Finish Calibration
                                                                                </button>
                                                                            {/if}
                                                                        </div>
                                                                    {/if}
                                                                </div>
                                                                
                                                                <!-- Sensitivity Section -->
                                                                {#if switchConfig.is_calibrated}
                                                                    <div class="magnetic-switch-config-section">
                                                                        <label for="sensitivity-{switchId}">Sensitivity: {switchConfig.sensitivity}%</label>
                                                                        <input 
                                                                            id="sensitivity-{switchId}"
                                                                            type="range" 
                                                                            min="1" 
                                                                            max="100" 
                                                                            value={switchConfig.sensitivity}
                                                                            onchange={(e) => {
                                                                                let sensitivity = parseInt(e.target.value);
                                                                                setMagneticSwitchSensitivity(switchId, sensitivity);
                                                                            }}
                                                                        />
                                                                        <p class="sensitivity-help">Lower = more sensitive</p>
                                                                    </div>
                                                                {/if}
                                                                
                                                                <!-- Key Assignment Section -->
                                                                <div class="magnetic-switch-config-section">
                                                                    <span>Assigned Key</span>
                                                                    <button 
                                                                        class="key-assignment-button"
                                                                        onclick={() => openKeyModal(rowIndex, colIndex)}
                                                                    >
                                                                        {formatKeyLabel(switchConfig.keycode).join(' ')}
                                                                    </button>
                                                                </div>
                                                            </div>
                                                        </div>
                                                    {/if}
                                                {:else if isEncoderCell(rowIndex, colIndex)}
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
                                                            <div class="encoder-config-panel">
                                                                <h3>Encoder Configuration</h3>
                                                                <div class="encoder-config-section">
                                                                    <label>Clockwise (CW)</label>
                                                                    <button class="encoder-action-button" onclick={(event) => handleEncoderAction(rowIndex, colIndex, 'cw', event)}>
                                                                        <span class="encoder-action-icon">⟳</span>
                                                                        <span class="encoder-action-key">Up Arrow</span>
                                                                    </button>
                                                                </div>
                                                                <div class="encoder-config-section">
                                                                    <label>Counter-Clockwise (CCW)</label>
                                                                    <button class="encoder-action-button" onclick={(event) => handleEncoderAction(rowIndex, colIndex, 'ccw', event)}>
                                                                        <span class="encoder-action-icon">⟲</span>
                                                                        <span class="encoder-action-key">Down Arrow</span>
                                                                    </button>
                                                                </div>
                                                            </div>
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
                                            {/if}
                                        {/each}
                                    </div>
                                {/each}
                            </div>
                            {#if loadingKeymap || loadingEncoders || loadingLayout}
                                <div class="keymap-overlay" aria-live="polite">
                                    <div class="spinner spinner-small"></div>
                                    <span>{loadingLayout ? 'Detecting layout…' : 'Loading layout…'}</span>
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
                    <h3 id="key-modal-title">Edit Key L{modalKey.layer ?? selectedLayer} · R{modalKey.row}C{modalKey.col}</h3>
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
                        class:active={keyModalTab === 'layer'}
                        onclick={() => {
                            keyModalTab = 'layer';
                            initLayerTab();
                        }}
                    >
                        Layer Switch
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
                        <div
                            class="standard-key-layout"
                            style="--key-unit: clamp(34px, (min(100vw, 1180px) - 220px) / 18.5, 48px);"
                        >
                            {#each resolvedStandardKeyLayout() as row}
                                <div
                                    class="standard-key-row"
                                    style={`margin-left: calc(var(--key-unit) * ${(row.offset ?? 0)})`}
                                >
                                    {#each row.keys as item}
                                        {#if item.spacer !== undefined}
                                            <div
                                                class="standard-key-spacer"
                                                style={`width: calc(var(--key-unit) * ${item.spacer}); height: calc(var(--key-unit) * ${(item.height ?? 1)})`}
                                            ></div>
                                        {:else}
                                            <button
                                                type="button"
                                                class="standard-key-button"
                                                class:selected={modalKey.keycode === item.codeValue}
                                                class:unavailable={!item.codeValue}
                                                disabled={!item.codeValue}
                                                style={`width: calc(var(--key-unit) * ${(item.width ?? 1)}); height: calc(var(--key-unit) * ${(item.height ?? 1)})`}
                                                onclick={() => {
                                                    if (!item.codeValue) {
                                                        return;
                                                    }
                                                    modalKey.keycode = item.codeValue;
                                                    modalKey = modalKey;
                                                }}
                                            >
                                                {#each item.legend as line}
                                                    <span>{line}</span>
                                                {/each}
                                            </button>
                                        {/if}
                                    {/each}
                                </div>
                            {/each}
                        </div>
                        <details class="other-key-options">
                            <summary>Other keys</summary>
                            <div class="keycode-selector">
                                {#each Object.entries(getKeycodesByCategory()) as [category, options]}
                                    {#if !standardKeyCategories.has(category)}
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
                                    {/if}
                                {/each}
                            </div>
                        </details>
                    {:else if keyModalTab === 'layer'}
                        <div class="midi-config">
                            <div class="form-group">
                                <label for="layerKeyAction">Layer Action</label>
                                <select
                                    id="layerKeyAction"
                                    value={layerKeyType}
                                    onchange={(event) => {
                                        layerKeyType = event.target.value;
                                        updateLayerKeySelection();
                                    }}
                                >
                                    <option value="persistent">Persistent (TO)</option>
                                    <option value="momentary">Momentary (MO)</option>
                                </select>
                            </div>
                            <div class="form-group">
                                <label for="layerKeyLayer">Layer</label>
                                <select
                                    id="layerKeyLayer"
                                    value={layerKeyLayer}
                                    onchange={(event) => {
                                        layerKeyLayer = Number(event.target.value);
                                        updateLayerKeySelection();
                                    }}
                                >
                                    {#each layerKeySelectableIndices() as layerOption}
                                        <option value={layerOption}>Layer {layerOption}</option>
                                    {/each}
                                </select>
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
                    <h3 id="encoder-modal-title">Edit Encoder L{modalEncoder.layer ?? selectedLayer} · {modalEncoder.encoder_id} - {encoderModalDirection === 'ccw' ? 'Counter-Clockwise' : 'Clockwise'}</h3>
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

    .layer-toolbar {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        justify-content: space-between;
        gap: 16px;
        margin-bottom: 16px;
    }

    .layer-buttons {
        display: flex;
        flex-wrap: wrap;
        gap: 8px;
    }

    .layer-button {
        display: inline-flex;
        align-items: center;
        gap: 8px;
        padding: 10px 14px;
        border-radius: 999px;
        border: 1px solid rgba(0, 0, 0, 0.08);
        background: #ffffff;
        color: #1d1d1f;
        font-size: 13px;
        font-weight: 500;
        cursor: pointer;
        transition: all 0.2s ease;
        box-shadow: 0 1px 3px rgba(0, 0, 0, 0.04);
    }

    .layer-button:hover {
        border-color: rgba(0, 122, 255, 0.4);
        box-shadow: 0 4px 12px rgba(0, 122, 255, 0.12);
    }

    .layer-button.current {
        background: rgba(0, 122, 255, 0.12);
        border-color: rgba(0, 122, 255, 0.5);
        color: #0051d5;
        box-shadow: 0 4px 12px rgba(0, 122, 255, 0.18);
    }

    .layer-button.active-hardware:not(.current) {
        border-color: rgba(52, 199, 89, 0.35);
        box-shadow: 0 4px 10px rgba(52, 199, 89, 0.16);
    }

    .layer-label {
        letter-spacing: -0.2px;
    }

    .layer-pill {
        padding: 2px 8px;
        border-radius: 999px;
        background: rgba(0, 122, 255, 0.12);
        color: #0051d5;
        font-size: 11px;
        font-weight: 600;
        letter-spacing: 0.3px;
        text-transform: uppercase;
    }

    .layer-pill.inactive {
        background: rgba(0, 0, 0, 0.06);
        color: #6c6c70;
    }

    .layer-pill.active {
        background: rgba(52, 199, 89, 0.18);
        color: #22863a;
    }

    .layer-pill.momentary {
        background: rgba(255, 159, 10, 0.18);
        color: #a05a00;
    }

    .layer-state-controls {
        display: flex;
        align-items: center;
        gap: 12px;
        flex-wrap: wrap;
    }

    .layer-state-summary {
        display: inline-flex;
        align-items: center;
        gap: 12px;
        font-size: 13px;
        color: #1d1d1f;
        font-weight: 500;
        padding: 6px 12px;
        border-radius: 999px;
        background: rgba(0, 0, 0, 0.04);
    }

    .momentary-indicator {
        font-size: 13px;
        color: #0051d5;
        font-weight: 500;
        background: rgba(0, 122, 255, 0.12);
        padding: 6px 10px;
        border-radius: 999px;
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
        display: grid;
        grid-template-columns: repeat(var(--board-cols, 3), 64px); /* Dynamic columns based on board layout */
        gap: 4px;
        transition: opacity 0.2s ease;
        justify-content: center;
    }

    .keymap.loading {
        opacity: 0.35;
    }

    .keymap-row {
        display: contents; /* Allow grid items to be placed directly */
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
        top: 50%;
        transform: translateY(-50%);
        left: 100%;
        margin-left: 12px;
        z-index: 200;
        opacity: 0;
        pointer-events: none;
        transition: opacity 0.2s ease;
    }

    .encoder-menu.active {
        opacity: 1;
        pointer-events: auto;
    }

    .encoder-config-panel {
        width: 280px;
        padding: 20px;
        background: #ffffff;
        border: 1px solid #d1d1d6;
        border-radius: 16px;
        box-shadow: 
            0 8px 32px rgba(0, 0, 0, 0.12),
            0 0 0 1px rgba(0, 0, 0, 0.04);
    }

    .encoder-config-panel h3 {
        margin: 0 0 16px 0;
        font-size: 16px;
        font-weight: 600;
        color: #1d1d1f;
    }

    .encoder-config-section {
        margin-bottom: 16px;
    }

    .encoder-config-section:last-child {
        margin-bottom: 0;
    }

    .encoder-config-section label {
        display: block;
        margin-bottom: 8px;
        font-size: 14px;
        font-weight: 500;
        color: #424245;
    }

    .encoder-action-button {
        display: flex;
        align-items: center;
        gap: 8px;
        width: 100%;
        padding: 12px;
        background: #f2f2f7;
        border: 1px solid #d1d1d6;
        border-radius: 8px;
        cursor: pointer;
        transition: all 0.2s ease;
    }

    .encoder-action-button:hover {
        background: #e5e5ea;
        border-color: #007aff;
        transform: translateY(-1px);
        box-shadow: 0 2px 8px rgba(0, 122, 255, 0.15);
    }

    .encoder-action-button:active {
        transform: translateY(0);
        box-shadow: 0 1px 4px rgba(0, 122, 255, 0.2);
    }

    .encoder-action-icon {
        font-size: 18px;
        color: #007aff;
    }

    .encoder-action-key {
        font-size: 14px;
        font-weight: 500;
        color: #1d1d1f;
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

    /* Slider Styles */
    .slider-cell {
        /* Override grid cell sizing for sliders */
        width: 64px;
        height: 336px; /* 5 rows × 64px + 4 gaps × 4px = 320px + 16px = 336px */
        grid-row: span 5; /* Make it span 5 rows in the grid */
    }

    .slider-container {
        width: 100%;
        height: 100%;
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 8px;
        padding: 8px;
        background: #ffffff;
        border: 1px solid #d1d1d6;
        border-radius: 12px;
        position: relative;
        cursor: pointer;
        box-shadow: 
            0 1px 3px rgba(0, 0, 0, 0.04),
            0 0 0 1px rgba(0, 0, 0, 0.02);
    }

    .slider-track {
        width: 16px;
        height: 280px; /* Taller track to better fill the 5-row space */
        background: #f2f2f7;
        border-radius: 16px;
        position: relative;
        border: 1px solid #d1d1d6;
    }

    .slider-thumb {
        width: 32px;
        height: 12px;
        background: #ffffff;
        border: 2px solid #007aff;
        border-radius: 6px;
        position: absolute;
        left: 50%;
        transform: translateX(-50%);
        transition: bottom 0.15s ease;
        box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
    }

    .slider-info {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 2px;
        font-size: 10px;
        color: #1d1d1f;
    }

    .slider-value {
        font-weight: 600;
        font-size: 12px;
        color: #007aff;
    }

    .slider-range {
        color: #86868b;
        font-size: 9px;
    }

    .unsaved-indicator {
        color: #ff9500;
        font-size: 8px;
        line-height: 1;
        margin-top: 1px;
    }

    .slider-cc {
        font-weight: 500;
        color: #000000;
        padding: 2px 6px;
        border-radius: 4px;
        font-size: 9px;
    }

    .slider-channel {
        font-weight: 500;
        color: #000000;
        padding: 2px 6px;
        border-radius: 4px;
        font-size: 9px;
    }

    .slider-container:hover {
        border-color: rgba(0, 122, 255, 0.5);
        transform: translateY(-1px);
        box-shadow: 
            0 4px 12px rgba(0, 122, 255, 0.12),
            0 0 0 1px rgba(0, 122, 255, 0.2);
    }

    /* Potentiometer Styles */
    .potentiometer-cell {
        width: 64px;
        height: 64px;
    }

    .potentiometer-container {
        width: 100%;
        height: 100%;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        background: #ffffff;
        border: 1px solid #d1d1d6;
        border-radius: 50%;
        position: relative;
        cursor: pointer;
        box-shadow: 
            0 1px 3px rgba(0, 0, 0, 0.04),
            0 0 0 1px rgba(0, 0, 0, 0.02);
    }

    .potentiometer-progress {
        position: absolute;
        top: -12px;
        left: -12px;
        width: calc(100% + 24px);
        height: calc(100% + 24px);
        border-radius: 50%;
        background: conic-gradient(
            from -135deg,
            #007aff 0deg,
            #007aff var(--progress, 0deg),
            transparent var(--progress, 0deg)
        );
        mask: radial-gradient(circle, transparent calc(50% - 3px), white calc(50% - 3px), white calc(50% + 1px), transparent calc(50% + 1px));
        -webkit-mask: radial-gradient(circle, transparent calc(50% - 3px), white calc(50% - 3px), white calc(50% + 1px), transparent calc(50% + 1px));
        z-index: 0;
    }

    .potentiometer-text {
        position: absolute;
        top: 50%;
        left: 50%;
        transform: translate(-50%, -50%);
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 1px;
        font-size: 9px;
        color: #1d1d1f;
        font-weight: 500;
        z-index: 5;
        pointer-events: none;
    }

    .potentiometer-text > div {
        color: #1d1d1f;
        padding: 1px 3px;
        border-radius: 3px;
        font-size: 8px;
        line-height: 1;
    }

    .potentiometer-indicator {
        width: 5px;
        height: 15px;
        background: #007aff;
        position: absolute;
        top: -4px;
        left: 50%;
        transform-origin: center 36px; /* Almost at the very edge of 64px button */
        border-radius: 5px;
        z-index: 10;
    }

    .potentiometer-knob {
        display: none; /* Remove the inner knob completely */
    }

    .potentiometer-info {
        position: relative;
        z-index: 5;
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 1px;
        font-size: 8px;
        color: #1d1d1f;
        background: rgba(255, 255, 255, 0.95);
        padding: 4px 6px;
        border-radius: 8px;
        box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
        border: 1px solid rgba(209, 209, 214, 0.5);
    }

    .potentiometer-value {
        font-weight: 600;
        font-size: 9px;
        color: #007aff;
    }

    .potentiometer-range {
        color: #86868b;
        font-size: 7px;
    }

    .potentiometer-cc {
        font-weight: 500;
        color: #000000;
        background: #f2f2f7;
        padding: 1px 3px;
        border-radius: 3px;
        font-size: 7px;
    }

    .potentiometer-channel {
        font-weight: 500;
        color: #000000;
        background: #e1f5fe;
        padding: 1px 3px;
        border-radius: 3px;
        font-size: 7px;
    }

    .potentiometer-container:hover {
        border-color: rgba(0, 122, 255, 0.5);
        transform: translateY(-1px);
        box-shadow: 
            0 4px 12px rgba(0, 122, 255, 0.12),
            0 0 0 1px rgba(0, 122, 255, 0.2);
    }

    .potentiometer-container:hover .potentiometer-info {
        opacity: 1;
        visibility: visible;
    }

    /* Slider Menu */
    .slider-menu {
        position: absolute;
        top: 50%;
        transform: translateY(-50%);
        left: 100%;
        margin-left: 12px;
        z-index: 200;
        opacity: 0;
        pointer-events: none;
        transition: opacity 0.2s ease;
    }

    .slider-menu.active {
        opacity: 1;
        pointer-events: auto;
    }

    .slider-config-panel {
        width: 280px;
        padding: 20px;
        background: #ffffff;
        border: 1px solid #d1d1d6;
        border-radius: 16px;
        box-shadow: 
            0 8px 32px rgba(0, 0, 0, 0.12),
            0 0 0 1px rgba(0, 0, 0, 0.04);
    }

    /* Magnetic Switch Styles */
    .magnetic-switch-cell {
        position: relative;
    }

    .magnetic-switch-container {
        width: 100%;
        height: 100%;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        background: #ffffff;
        border: 1px solid #d1d1d6;
        border-radius: 8px;
        position: relative;
        cursor: pointer;
        box-shadow: 
            0 1px 3px rgba(0, 0, 0, 0.04),
            0 0 0 1px rgba(0, 0, 0, 0.02);
        transition: all 0.2s ease;
        padding: 8px;
    }

    .magnetic-switch-container:hover {
        border-color: rgba(0, 122, 255, 0.5);
        transform: translateY(-1px);
        box-shadow: 
            0 4px 12px rgba(0, 122, 255, 0.12),
            0 0 0 1px rgba(0, 122, 255, 0.2);
    }

    .magnetic-switch-container.pressed {
        background: rgba(255, 59, 48, 0.1);
        border-color: #ff3b30;
    }

    .magnetic-switch-visual {
        position: relative;
        width: 20px;
        height: 40px;
        margin-bottom: 8px;
    }

    .magnetic-switch-bar {
        width: 100%;
        height: 100%;
        background: #f2f2f7;
        border-radius: 4px;
        position: relative;
        overflow: hidden;
    }

    .magnetic-switch-fill {
        position: absolute;
        bottom: 0;
        left: 0;
        width: 100%;
        background: linear-gradient(to top, #007aff, #5ac8fa);
        border-radius: 4px;
        transition: height 0.1s ease;
    }

    .magnetic-switch-threshold {
        position: absolute;
        left: -2px;
        right: -2px;
        height: 2px;
        background: #ff3b30;
        border-radius: 1px;
    }

    .magnetic-switch-text {
        display: flex;
        flex-direction: column;
        align-items: center;
        font-size: 10px;
        font-weight: 600;
        color: #1d1d1f;
        line-height: 1.2;
    }

    .magnetic-switch-menu {
        position: absolute;
        top: 50%;
        transform: translateY(-50%);
        left: 100%;
        margin-left: 12px;
        z-index: 200;
        opacity: 0;
        pointer-events: none;
        transition: opacity 0.2s ease;
    }

    .magnetic-switch-menu.active {
        opacity: 1;
        pointer-events: auto;
    }

    .magnetic-switch-config-panel {
        width: 320px;
        padding: 20px;
        background: #ffffff;
        border: 1px solid #d1d1d6;
        border-radius: 16px;
        box-shadow: 
            0 8px 32px rgba(0, 0, 0, 0.12),
            0 0 0 1px rgba(0, 0, 0, 0.04);
    }

    .magnetic-switch-config-panel h3 {
        margin: 0 0 16px 0;
        font-size: 16px;
        font-weight: 600;
        color: #1d1d1f;
    }

    .magnetic-switch-config-section {
        margin-bottom: 16px;
    }

    .magnetic-switch-config-section span,
    .magnetic-switch-config-section label {
        display: block;
        font-size: 13px;
        font-weight: 600;
        color: #1d1d1f;
        margin-bottom: 8px;
    }

    .calibration-status {
        display: flex;
        align-items: center;
        gap: 8px;
        margin-bottom: 8px;
    }

    .calibrated {
        color: #30d158;
        font-weight: 600;
        font-size: 12px;
    }

    .calibration-steps {
        margin-top: 12px;
        padding: 12px;
        background: #f2f2f7;
        border-radius: 8px;
    }

    .calibration-steps p {
        margin: 0 0 8px 0;
        font-size: 12px;
        color: #1d1d1f;
    }

    .calibration-help,
    .sensitivity-help {
        margin: 4px 0 0 0;
        font-size: 11px;
        color: #8e8e93;
    }

    .key-assignment-button {
        width: 100%;
        padding: 8px 12px;
        background: #f2f2f7;
        border: 1px solid #d1d1d6;
        border-radius: 6px;
        font-size: 12px;
        font-weight: 500;
        color: #1d1d1f;
        cursor: pointer;
        transition: all 0.2s ease;
    }

    .key-assignment-button:hover {
        background: #e8e8ed;
        border-color: #b8b8be;
    }

    .save-button {
        width: 100%;
        padding: 12px 16px;
        border: none;
        border-radius: 8px;
        font-size: 14px;
        font-weight: 600;
        cursor: pointer;
        transition: all 0.2s ease;
        margin-top: 8px;
    }

    .save-button:disabled {
        background: #f5f5f7;
        color: #8e8e93;
        cursor: not-allowed;
    }

    .save-button.has-changes {
        background: #007aff;
        color: white;
    }

    .save-button.has-changes:hover {
        background: #0056cc;
    }

    .slider-config-panel h3 {
        margin: 0 0 16px 0;
        font-size: 16px;
        font-weight: 600;
        color: #1d1d1f;
    }

    .slider-config-section {
        margin-bottom: 16px;
    }

    .slider-config-section label {
        display: block;
        font-size: 12px;
        font-weight: 500;
        color: #86868b;
        margin-bottom: 6px;
        text-transform: uppercase;
        letter-spacing: 0.05em;
    }

    .slider-config-section input[type="number"] {
        width: 100%;
        padding: 8px 12px;
        border: 1px solid #d1d1d6;
        border-radius: 8px;
        font-size: 14px;
        background: #ffffff;
        color: #1d1d1f;
    }



    .slider-config-section span {
        font-weight: 600;
        color: #007aff;
        min-width: 40px;
        display: inline-block;
        text-align: right;
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
        width: min(1120px, calc(100vw - 48px));
        max-width: 1120px;
        max-height: 90vh;
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

    .standard-key-layout {
        display: flex;
        flex-direction: column;
        gap: 12px;
        padding: 18px 16px;
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 16px;
        width: fit-content;
        margin: 0 auto;
    }

    .standard-key-row {
        display: flex;
        align-items: flex-start;
        gap: 6px;
    }

    .standard-key-spacer {
        height: calc(var(--key-unit));
    }

    .standard-key-button {
        display: flex;
        flex-direction: column;
        justify-content: center;
        align-items: center;
        gap: 2px;
        padding: 10px 6px;
        border-radius: 10px;
        border: 1px solid rgba(255, 255, 255, 0.08);
        background: linear-gradient(180deg, rgba(40, 44, 56, 0.92), rgba(26, 29, 38, 0.94));
        color: #f5f7ff;
        font-size: 13px;
        font-weight: 500;
        cursor: pointer;
        box-shadow: 0 8px 18px rgba(12, 19, 32, 0.35);
        transition: transform 0.15s ease, box-shadow 0.2s ease, border-color 0.2s ease, color 0.2s ease;
    }

    .standard-key-button span {
        display: block;
        line-height: 1.1;
    }

    .standard-key-button span:not(:last-child) {
        font-size: 11px;
        opacity: 0.72;
    }

    .standard-key-button span:last-child {
        font-size: 13px;
    }

    .standard-key-button:hover:not(.unavailable) {
        transform: translateY(-1px);
        box-shadow: 0 12px 22px rgba(12, 19, 32, 0.45);
        border-color: rgba(94, 189, 255, 0.4);
    }

    .standard-key-button.selected {
        border-color: rgba(94, 189, 255, 0.7);
        box-shadow: 0 0 0 3px rgba(94, 189, 255, 0.25);
    }

    .standard-key-button.unavailable {
        cursor: not-allowed;
        opacity: 0.4;
        box-shadow: none;
    }

    .other-key-options {
        margin-top: 24px;
        background: rgba(15, 23, 42, 0.25);
        border: 1px solid rgba(148, 163, 184, 0.18);
        border-radius: 14px;
        padding: 0 18px 12px;
        color: #e1e4f5;
    }

    .other-key-options summary {
        cursor: pointer;
        font-size: 14px;
        font-weight: 600;
        list-style: none;
        padding: 14px 0;
        display: flex;
        align-items: center;
        gap: 8px;
    }

    .other-key-options summary::-webkit-details-marker {
        display: none;
    }

    .other-key-options summary::before {
        content: '';
        width: 12px;
        height: 12px;
        border-radius: 50%;
        background: rgba(94, 189, 255, 0.8);
        box-shadow: 0 0 12px rgba(94, 189, 255, 0.5);
    }

    .other-key-options[open] {
        box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.08);
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

    .form-group select {
        appearance: none;
        -webkit-appearance: none;
        padding: 12px 40px 12px 14px;
        border-radius: 12px;
        border: 1px solid rgba(15, 23, 42, 0.08);
        background: linear-gradient(135deg, rgba(255, 255, 255, 0.92), rgba(240, 243, 248, 0.94));
        box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.65), 0 8px 20px rgba(15, 23, 42, 0.08);
        color: #0f172a;
        font-size: 14px;
        font-weight: 500;
        cursor: pointer;
        transition: border-color 0.2s ease, box-shadow 0.2s ease, transform 0.15s ease;
        background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='14' height='8' viewBox='0 0 14 8'%3E%3Cpath fill='%2357636f' d='M1 1l6 6 6-6'/%3E%3C/svg%3E");
        background-repeat: no-repeat;
        background-position: calc(100% - 14px) 50%;
        background-size: 12px;
    }

    .form-group select:hover {
        box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.7), 0 10px 24px rgba(15, 23, 42, 0.12);
        transform: translateY(-1px);
    }

    .form-group select:focus {
        outline: none;
        border-color: rgba(59, 130, 246, 0.6);
        box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.2), inset 0 1px 0 rgba(255, 255, 255, 0.7);
    }

    .form-group input {
        padding: 12px;
        border-radius: 8px;
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.2);
        color: #ffffff;
        font-size: 14px;
    }

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
