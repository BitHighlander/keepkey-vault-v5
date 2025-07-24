import { useState, useEffect, useCallback } from "react";
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import "./App.css";
import { Box, Text, Flex, Link, Stack, Button, Spinner } from "@chakra-ui/react";
import { FaCopy, FaCheck } from "react-icons/fa";

import { Logo } from './components/logo/logo';
import splashBg from './assets/splash-bg.png'
import { EllipsisDots } from "./components/EllipsisSpinner";
import { SettingsDialog, SettingsButton } from './components/SettingsDialog';
import { useCommonDialogs } from './hooks/useCommonDialogs';
import { DeviceUpdateManager } from './components/DeviceUpdateManager';
import { useOnboardingState } from './hooks/useOnboardingState';
import { VaultInterface } from './components/VaultInterface';
import { DialogProvider, useDialog } from './contexts/DialogContext'
import { useTroubleshootingWizard } from './contexts/DialogContext'

// Define the expected structure of DeviceFeatures from Rust
interface DeviceFeatures {
    label: string | null;
    vendor: string | null;
    model: string | null;
    firmware_variant: string | null;
    device_id: string | null;
    language: string | null;
    bootloader_mode: boolean;
    version: string;
    firmware_hash: string | null;
    bootloader_hash: string | null;
    initialized: boolean;
    imported: boolean | null;
    no_backup: boolean;
    pin_protection: boolean;
    pin_cached: boolean;
    passphrase_protection: boolean;
    passphrase_cached: boolean;
    wipe_code_protection: boolean;
    auto_lock_delay_ms: number | null;
    policies: string[];
}

interface DeviceInfoState {
    features: DeviceFeatures | null;
    error: string | null;
}

interface ApplicationState {
    status: string;
    connected: boolean;
    features: DeviceFeatures | null;
}

function App() {
    const [loadingStatus, setLoadingStatus] = useState<string>('Starting...')
    const [deviceConnected, setDeviceConnected] = useState<boolean>(false)
    const [deviceInfo, setDeviceInfo] = useState<any>(null)
    const [deviceUpdateComplete, setDeviceUpdateComplete] = useState<boolean>(false)
    const [serverReady, setServerReady] = useState<boolean>(false)
    const [serverError, setServerError] = useState<string | null>(null)
    const [isRestarting, setIsRestarting] = useState<boolean>(false)
    const [frontendReadySignalSent, setFrontendReadySignalSent] = useState<boolean>(false)
    const [eventListenersSetup, setEventListenersSetup] = useState<boolean>(false)
    
    const troubleshootingWizard = useTroubleshootingWizard()

    const reinitialize = () => {
        console.log("🔄 Reinitializing app state...")
        setLoadingStatus('Starting...')
        setDeviceConnected(false)
        setDeviceInfo(null)
        setDeviceUpdateComplete(false)
        setServerReady(false)
        setServerError(null)
        setFrontendReadySignalSent(false) // Reset frontend ready signal state
        setEventListenersSetup(false) // Reset event listeners state
    }

    // AppContent is an inner component with access to DialogContext
    const AppContent = () => {
        // We're tracking application state from backend events
        const [loadingStatus, setLoadingStatus] = useState<string>('Starting...');
        const [deviceConnected, setDeviceConnected] = useState<boolean>(false);
        const [, setDeviceInfo] = useState<DeviceInfoState | null>(null);
        const [isSettingsOpen, setIsSettingsOpen] = useState(false);
        const [isRestarting, setIsRestarting] = useState(false);
        const [deviceUpdateComplete, setDeviceUpdateComplete] = useState(false);
        const [onboardingActive, setOnboardingActive] = useState(false);
        const { showOnboarding, showError } = useCommonDialogs();
        const { shouldShowOnboarding, loading: onboardingLoading, clearCache } = useOnboardingState();
        const { hideAll, hide, activeDialog, getQueue } = useDialog();
        
        // Debug log active dialogs
        useEffect(() => {
            const queue = getQueue();
            if (activeDialog || queue.length > 0) {
                console.log('📱 [App] Active dialog:', activeDialog?.id);
                console.log('📱 [App] Dialog queue:', queue.map(d => d.id));
            }
        }, [activeDialog, getQueue]);
        
        // Clear any stuck dialogs when showing VaultInterface (but not onboarding-related dialogs)
        useEffect(() => {
            console.log('📱 [App] Dialog cleanup effect triggered with:', {
                loadingStatus,
                deviceConnected,
                deviceUpdateComplete,
                shouldShowOnboarding,
                onboardingActive,
                expectedLoadingStatus: "Device ready",
                statusMatches: loadingStatus === "Device ready",
                allConditionsMet: loadingStatus === "Device ready" && deviceConnected && deviceUpdateComplete
            });
            
            if (loadingStatus === "Device ready" && deviceConnected && deviceUpdateComplete && !shouldShowOnboarding && !onboardingActive) {
                const queue = getQueue();
                console.log('📱 [App] All conditions met and no onboarding needed! Dialog queue length:', queue.length);
                if (queue.length > 0) {
                    // Only clear non-onboarding dialogs
                    const nonOnboardingDialogs = queue.filter(d => !d.id.includes('onboarding'));
                                         if (nonOnboardingDialogs.length > 0) {
                         console.warn('📱 [App] Clearing stuck non-onboarding dialogs:', nonOnboardingDialogs.map(d => d.id));
                         nonOnboardingDialogs.forEach(d => hide(d.id));
                     } else {
                         console.log('📱 [App] Only onboarding dialogs in queue, not clearing');
                     }
                } else {
                    console.log('📱 [App] No stuck dialogs to clear');
                }
            }
        }, [loadingStatus, deviceConnected, deviceUpdateComplete, shouldShowOnboarding, onboardingActive, getQueue, hide]);
        
        // Function to show device access error dialog
        const showDeviceAccessError = (errorMessage: string) => {
            showError("KeepKey Device Access Error", errorMessage);
        };
        
        // Function to restart backend startup process
        const handleLogoClick = async () => {
            if (isRestarting) return; // Prevent multiple clicks
            
            setIsRestarting(true);
            try {
                console.log("Logo clicked - restarting backend startup process");
                
                // Reset all frontend state to initial values
                console.log("Resetting frontend state...");
                setLoadingStatus('Starting...');
                setDeviceConnected(false);
                setDeviceInfo(null);
                setDeviceUpdateComplete(false);
                setFrontendReadySignalSent(false); // Reset frontend ready signal state
                
                // Restart backend
                await invoke('restart_backend_startup');
                console.log("Backend restart initiated successfully");
                
                // Re-run wallet initialization to resubscribe device state after backend restart
                reinitialize();
                
                // Signal backend that frontend is ready again
                setTimeout(async () => {
                    try {
                        console.log('🎯 Re-signaling backend that frontend is ready after restart...');
                        await invoke('frontend_ready');
                        console.log('✅ Frontend ready signal sent successfully after restart');
                    } catch (error) {
                        console.log('frontend_ready command failed after restart:', error);
                    }
                }, 500);
            } catch (error) {
                console.error("Failed to restart backend startup:", error);
            } finally {
                // Reset the restarting flag after a delay
                setTimeout(() => setIsRestarting(false), 2000);
            }
        };

        // Onboarding is now handled via backend events (device:onboarding-required)
        // Just track the state for UI logic
        useEffect(() => {
            if (onboardingLoading) {
                console.log("App.tsx: Onboarding state still loading...");
                return;
            }

            console.log(`App.tsx: Onboarding check - should show: ${shouldShowOnboarding}`);
            
            if (shouldShowOnboarding) {
                console.log("App.tsx: Onboarding will be handled by backend events when device is ready");
                setOnboardingActive(true);
            } else {
                console.log("App.tsx: Onboarding not needed, user is already onboarded");
                setOnboardingActive(false);
            }
        }, [shouldShowOnboarding, onboardingLoading, clearCache]);

        useEffect(() => {
            let unlistenStatusUpdate: (() => void) | undefined;
            let unlistenDeviceReady: (() => void) | undefined;
            let unlistenServerReady: (() => void) | undefined;
            let unlistenServerError: (() => void) | undefined;
            let unlistenOnboardingRequired: (() => void) | undefined;

            const setupEventListeners = async () => {
                try {
                    // Prevent duplicate event listener setup
                    if (eventListenersSetup) {
                        console.log('🎯 Event listeners already set up, skipping...');
                        return;
                    }
                    
                    console.log('🎯 Setting up event listeners...');
                    
                    // Signal backend that frontend is ready to receive events FIRST (only once)
                    if (!frontendReadySignalSent) {
                        try {
                            console.log('🎯 Signaling backend that frontend is ready...');
                            await invoke('frontend_ready');
                            console.log('✅ Frontend ready signal sent successfully');
                            setFrontendReadySignalSent(true);
                        } catch (error) {
                            console.log('DeviceUpdateManager: frontend_ready command failed:', error);
                        }
                    } else {
                        console.log('🎯 Frontend ready signal already sent, skipping...');
                    }
                    
                    // TEMPORARY: Check if servers are running and manually set server ready
                    setTimeout(async () => {
                        try {
                            console.log('🔍 Manually checking if servers are running...');
                            const response = await fetch('http://127.0.0.1:1646/api/health');
                            if (response.ok) {
                                console.log('✅ API server is running, manually setting serverReady = true');
                                setServerReady(true);
                                setServerError(null);
                            }
                        } catch (error) {
                            console.log('❌ API server check failed:', error);
                            setServerError('API server not responding');
                            setServerReady(false);
                        }
                    }, 2000); // Check after 2 seconds
                    
                    // Listen for status updates from backend
                    console.log('🎯 Setting up status:update listener...');
                    unlistenStatusUpdate = await listen('status:update', (event) => {
                        // eslint-disable-next-line @typescript-eslint/no-unsafe-assignment
                        const payload = event.payload as any;
                        console.log('📱 [App] Frontend received status update:', payload);
                        
                        if (payload.status) {
                            console.log('📱 [App] Setting loading status from', loadingStatus, 'to:', payload.status);
                            setLoadingStatus(payload.status);
                            
                            // Special check for "Device ready" status
                            if (payload.status === "Device ready") {
                                console.log('📱 [App] Received "Device ready" status! Current state:', {
                                    deviceConnected,
                                    deviceUpdateComplete
                                });
                            }
                        } else {
                            console.log('❌ No status field in payload:', payload);
                        }
                    });

                    // Listen for device ready events (device with features loaded and fully ready)
                    unlistenDeviceReady = await listen('device:ready', (event) => {
                        // eslint-disable-next-line @typescript-eslint/no-unsafe-assignment
                        const payload = event.payload as any;
                        console.log('📱 [App] Device ready event received:', payload);
                        
                        if (payload.device && payload.features) {
                            console.log('📱 [App] Setting deviceConnected to true from device:ready event');
                            setDeviceConnected(true);
                            setDeviceInfo({ features: payload.features, error: null });
                            console.log('📱 [App] Setting deviceUpdateComplete to true from device:ready event');
                            setDeviceUpdateComplete(true);
                            console.log('📱 [App] Setting loadingStatus to "Device ready" from device:ready event');
                            setLoadingStatus('Device ready');
                            console.log(`✅ Device ready: ${payload.features.label || 'Unlabeled'} v${payload.features.version}`);
                        }
                    });

                    // Listen for device features-updated events (includes status evaluation for DeviceUpdateManager)
                    const unlistenFeaturesUpdated = await listen('device:features-updated', (event) => {
                        // eslint-disable-next-line @typescript-eslint/no-unsafe-assignment
                        const payload = event.payload as any;
                        console.log('Device features-updated event received:', payload);
                        
                        if (payload.features) {
                            setDeviceConnected(true);
                            setDeviceInfo({ features: payload.features, error: null });
                            // Don't reset deviceUpdateComplete here - let DeviceUpdateManager handle it
                            // Only reset on actual device disconnection, not feature updates
                        }
                    });

                    // Listen for device access errors from backend
                    const unlistenAccessError = await listen('device:access-error', (event) => {
                        // eslint-disable-next-line @typescript-eslint/no-unsafe-assignment
                        const errorData = event.payload as any;
                        console.log('Device access error event received:', errorData);
                        
                        showDeviceAccessError(errorData.error);
                        setLoadingStatus('Device busy');
                    });

                    // Listen for device disconnection events
                    const unlistenDeviceDisconnected = await listen('device:disconnected', (event) => {
                        console.log('Device disconnected event received:', event.payload);
                        setDeviceConnected(false);
                        setDeviceInfo(null);
                        setDeviceUpdateComplete(false);
                    });

                    // Listen for server ready events
                    console.log('🎯 Setting up server:ready listener...');
                    unlistenServerReady = await listen('server:ready', (event) => {
                        console.log('📡 Received server:ready event:', event.payload);
                        setServerReady(true);
                        setServerError(null);
                        console.log('✅ REST API server is ready');
                    });

                    // Listen for server error events
                    console.log('🎯 Setting up server:error listener...');
                    unlistenServerError = await listen('server:error', (event) => {
                        console.log('📡 Received server:error event:', event.payload);
                        const payload = event.payload as any;
                        setServerError(payload.error || 'Server failed to start');
                        setServerReady(false);
                        console.error('❌ REST API server failed:', payload.error);
                        
                        // If server error is critical, keep loading status as error
                        if (payload.critical) {
                            setLoadingStatus('Server startup failed');
                        }
                    });

                    // Listen for onboarding required events
                    console.log('🎯 Setting up device:onboarding-required listener...');
                    unlistenOnboardingRequired = await listen('device:onboarding-required', (event) => {
                        console.log('📱 [App] Received device:onboarding-required event:', event.payload);
                        setOnboardingActive(true);
                        showOnboarding({
                            onComplete: () => {
                                console.log("App.tsx: Onboarding completed callback");
                                clearCache(); // Clear the cache after completion
                                setOnboardingActive(false);
                            }
                        });
                    });

                    console.log('✅ All event listeners set up successfully');
                    setEventListenersSetup(true);
                    
                    // Return cleanup function that removes all listeners
                    return () => {
                        console.log('🧹 Cleaning up event listeners...');
                        if (unlistenStatusUpdate) unlistenStatusUpdate();
                        if (unlistenDeviceReady) unlistenDeviceReady();
                        if (unlistenFeaturesUpdated) unlistenFeaturesUpdated();
                        if (unlistenAccessError) unlistenAccessError();
                        if (unlistenDeviceDisconnected) unlistenDeviceDisconnected();
                        if (unlistenServerReady) unlistenServerReady();
                        if (unlistenServerError) unlistenServerError();
                        if (unlistenOnboardingRequired) unlistenOnboardingRequired();
                        setEventListenersSetup(false); // Reset so listeners can be set up again if needed
                    };
                    
                } catch (error) {
                    console.error("Failed to set up event listeners:", error);
                }
            };

            setupEventListeners();

            return () => {
                if (unlistenStatusUpdate) unlistenStatusUpdate();
                if (unlistenDeviceReady) unlistenDeviceReady();
                if (unlistenServerReady) unlistenServerReady();
                if (unlistenServerError) unlistenServerError();
                if (unlistenOnboardingRequired) unlistenOnboardingRequired();
            };
        }, []); // Empty dependency array ensures this runs once on mount and cleans up on unmount

        const mcpUrl = "http://127.0.0.1:1646/mcp";
        const [hasCopied, setHasCopied] = useState(false);
        
        const handleCopy = () => {
          navigator.clipboard.writeText(mcpUrl);
          setHasCopied(true);
          setTimeout(() => setHasCopied(false), 2000);
        };

        // Move onComplete callback BEFORE any early returns to fix React Hooks error
        const handleDeviceUpdateComplete = useCallback(() => {
          console.log('📱 [App] DeviceUpdateManager onComplete callback triggered');
          console.log('📱 [App] Current state before updates:', {
            deviceUpdateComplete,
            loadingStatus,
            deviceConnected
          });
          console.log('📱 [App] Setting deviceUpdateComplete to true');
          setDeviceUpdateComplete(true);
          console.log('📱 [App] Setting loadingStatus to "Device ready"');
          setLoadingStatus('Device ready');
          // Also ensure deviceConnected is true if not already
          if (!deviceConnected) {
            console.log('📱 [App] Also setting deviceConnected to true from onComplete');
            setDeviceConnected(true);
          }
        }, [deviceUpdateComplete, loadingStatus, deviceConnected]);

        // CRITICAL: Only show VaultInterface when BOTH device is ready AND onboarding is complete
        // Onboarding has absolute priority over everything else
        console.log('📱 [App] Checking if should show VaultInterface:', {
            loadingStatus,
            deviceConnected,
            deviceUpdateComplete,
            serverReady,
            serverError,
            shouldShowOnboarding,
            onboardingActive,
            deviceReady: loadingStatus === "Device ready" && deviceConnected && deviceUpdateComplete,
            onboardingComplete: !shouldShowOnboarding && !onboardingActive
        });
        
        // Show VaultInterface when device is ready, even if onboarding is needed (dialog will overlay)
        if (loadingStatus === "Device ready" && deviceConnected && deviceUpdateComplete) {
            console.log('📱 [App] ✅ Device ready - showing VaultInterface! (onboarding dialog will overlay if needed)');
            return <VaultInterface />;
        } else if (shouldShowOnboarding || onboardingActive) {
            console.log('📱 [App] ⏳ Onboarding needed but device not ready yet - showing loading UI');
            // Continue with normal loading UI until device is ready
        }

        // Show splash screen while connecting
        return (
          <Box
            height="100vh"
            width="100vw"
            position="relative"
            backgroundImage={`url(${splashBg})`}
            backgroundSize="cover"
            backgroundPosition="center"
          >
            <Flex 
              height="100%"
              width="100%"
              direction="column"
              alignItems="center"
              justifyContent="center"
            >
              {/* Clickable Logo in the center - Show during loading and connecting */}
              {(loadingStatus !== "Device ready" || !onboardingActive) && (
                <Logo 
                width="100px" 
                onClick={handleLogoClick}
                style={{
                  filter: isRestarting ? 'brightness(1.3)' : 'none',
                  transition: 'filter 0.2s ease'
                  }}
                />
              )}
              
              
              {/* Clickable hint */}
              <Text 
                fontSize="xs" 
                color="gray.400" 
                mt={2} 
                textAlign="center"
                opacity={isRestarting ? 0.5 : 0.7}
                transition="opacity 0.2s ease"
              >
                {isRestarting ? "Restarting..." : ""}
              </Text>
              
              {/* Loading text at the bottom */}
                <Box
                    position="absolute"
                    bottom="40px"
                    textAlign="center"
                    width="auto"
                    px={3}
                    py={1}
                    borderRadius="md"
                    bg="rgba(0, 0, 0, 0.5)"
                >
                    <Flex gap="2" justifyContent="center" alignItems="center">
                        <Spinner size="xs" color={deviceConnected ? "green.400" : "gray.400"} />
                        <Text fontSize="xs" color="gray.300">
                            {loadingStatus}
                        </Text>
                        <EllipsisDots interval={300} /> {/* ⟵ no layout shift */}
                    </Flex>
                </Box>

              {/* Settings button in bottom left */}
              <SettingsButton onClick={() => setIsSettingsOpen(true)} />

              {/* Settings dialog */}
              <SettingsDialog 
                isOpen={isSettingsOpen} 
                onClose={() => setIsSettingsOpen(false)} 
              />

              {/* Device update manager - handles bootloader/firmware updates and wallet creation */}
              <DeviceUpdateManager 
                onComplete={handleDeviceUpdateComplete}
              />

              {/* REST and MCP links in bottom right corner */}
              {/*<Box*/}
              {/*  position="absolute"*/}
              {/*  bottom="20px"*/}
              {/*  right="20px"*/}
              {/*  background="rgba(0, 0, 0, 0.7)"*/}
              {/*  borderRadius="md"*/}
              {/*  boxShadow="md"*/}
              {/*  padding={2}*/}
              {/*  border="1px solid rgba(100, 255, 100, 0.3)"*/}
              {/*>*/}
              {/*  <Flex direction="column" gap={1}>*/}
              {/*    <Stack direction="row" gap={2} align="center">*/}
              {/*      <Text fontSize="2xs" color="gray.300">REST:</Text>*/}
              {/*      <Link */}
              {/*        href="http://127.0.0.1:1646/docs" */}
              {/*        target="_blank" */}
              {/*        fontSize="xs" */}
              {/*        color="blue.300"*/}
              {/*        _hover={{ color: "blue.200", textDecoration: "underline" }}*/}
              {/*      >*/}
              {/*        http://127.0.0.1:1646/docs*/}
              {/*      </Link>*/}
              {/*    </Stack>*/}
              {/*    <Stack direction="row" gap={2} align="center">*/}
              {/*      <Text fontSize="2xs" color="gray.300">MCP:</Text>*/}
              {/*      <Link */}
              {/*        href="http://127.0.0.1:1646/mcp" */}
              {/*        target="_blank" */}
              {/*        fontSize="xs" */}
              {/*        color="blue.300"*/}
              {/*        _hover={{ color: "blue.200", textDecoration: "underline" }}*/}
              {/*      >*/}
              {/*        http://127.0.0.1:1646/mcp*/}
              {/*      </Link>*/}
              {/*      <Box position="relative">*/}
              {/*        <Button*/}
              {/*          size="xs"*/}
              {/*          variant="ghost"*/}
              {/*          colorScheme={hasCopied ? "green" : "blue"}*/}
              {/*          aria-label="Copy MCP URL to clipboard"*/}
              {/*          title={hasCopied ? "Copied!" : "Copy to clipboard"}*/}
              {/*          onClick={handleCopy}*/}
              {/*          p={1}*/}
              {/*          minW={0}*/}
              {/*          height="16px"*/}
              {/*        >*/}
              {/*          {hasCopied ? <FaCheck size="10px" /> : <FaCopy size="10px" />}*/}
              {/*        </Button>*/}
              {/*      </Box>*/}
              {/*    </Stack>*/}
              {/*  </Flex>*/}
              {/*</Box>*/}
            </Flex>
          </Box>
        );
    };

    return (
        <DialogProvider>
            <AppContent />
        </DialogProvider>
    );
}

export default App;
