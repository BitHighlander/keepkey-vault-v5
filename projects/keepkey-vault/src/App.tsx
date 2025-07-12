import { useState, useEffect } from "react";
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
import { useWallet } from './contexts/WalletContext';
import { DialogProvider, useDialog } from './contexts/DialogContext'

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
        const { hideAll, activeDialog, getQueue } = useDialog();
        const { fetchedXpubs, portfolio, isSync, reinitialize } = useWallet();
        
        // Check wallet context state and sync with local state
        useEffect(() => {
            console.log('📱 [App] Wallet context state check:', {
                portfolioPresent: !!portfolio,
                fetchedXpubsLength: fetchedXpubs.length,
                isSync,
                currentLocalState: {
                    loadingStatus,
                    deviceConnected,
                    deviceUpdateComplete
                }
            });
            
            // If wallet has xpubs and portfolio, ensure we show the vault
            if (fetchedXpubs.length > 0 && portfolio) {
                console.log('📱 [App] Wallet is ready with xpubs and portfolio, syncing local state');
                
                // Only update if not already set to avoid infinite loops
                if (!deviceConnected) {
                    console.log('📱 [App] Setting deviceConnected to true from wallet context');
                    setDeviceConnected(true);
                }
                if (!deviceUpdateComplete) {
                    console.log('📱 [App] Setting deviceUpdateComplete to true from wallet context');
                    setDeviceUpdateComplete(true);
                }
                if (loadingStatus !== 'Device ready') {
                    console.log('📱 [App] Setting loadingStatus to "Device ready" from wallet context');
                    setLoadingStatus('Device ready');
                }
            }
        }, [fetchedXpubs, portfolio, isSync, deviceConnected, deviceUpdateComplete, loadingStatus]);
        
        // Debug log active dialogs
        useEffect(() => {
            const queue = getQueue();
            if (activeDialog || queue.length > 0) {
                console.log('📱 [App] Active dialog:', activeDialog?.id);
                console.log('📱 [App] Dialog queue:', queue.map(d => d.id));
            }
        }, [activeDialog, getQueue]);
        
        // Clear any stuck dialogs when showing VaultInterface
        useEffect(() => {
            console.log('📱 [App] Dialog cleanup effect triggered with:', {
                loadingStatus,
                deviceConnected,
                deviceUpdateComplete,
                expectedLoadingStatus: "Device ready",
                statusMatches: loadingStatus === "Device ready",
                allConditionsMet: loadingStatus === "Device ready" && deviceConnected && deviceUpdateComplete
            });
            
            if (loadingStatus === "Device ready" && deviceConnected && deviceUpdateComplete) {
                const queue = getQueue();
                console.log('📱 [App] All conditions met! Dialog queue length:', queue.length);
                if (queue.length > 0) {
                    console.warn('📱 [App] Clearing stuck dialogs before showing VaultInterface:', queue.map(d => d.id));
                    hideAll();
                } else {
                    console.log('📱 [App] No stuck dialogs to clear');
                }
            }
        }, [loadingStatus, deviceConnected, deviceUpdateComplete, getQueue, hideAll]);
        
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

        // Check onboarding status on startup
        useEffect(() => {
            if (onboardingLoading) {
                console.log("App.tsx: Onboarding state still loading...");
                return;
            }

            console.log(`App.tsx: Should show onboarding: ${shouldShowOnboarding}`);
            
            if (shouldShowOnboarding) {
                console.log("App.tsx: Showing onboarding wizard");
                setOnboardingActive(true);
                // Add a small delay to ensure the dialog system is ready
                setTimeout(() => {
                    showOnboarding({
                        onComplete: () => {
                            console.log("App.tsx: Onboarding completed callback");
                            clearCache(); // Clear the cache after completion
                            setOnboardingActive(false);
                        }
                    });
                }, 1000);
            } else {
                console.log("App.tsx: Onboarding not needed, user is already onboarded");
            }
        }, [shouldShowOnboarding, onboardingLoading, showOnboarding, clearCache]);

        useEffect(() => {
            let unlistenStatusUpdate: (() => void) | undefined;
            let unlistenDeviceReady: (() => void) | undefined;

            const setupEventListeners = async () => {
                try {
                    console.log('🎯 Setting up event listeners...');
                    
                    // Signal backend that frontend is ready to receive events FIRST
                    try {
                        console.log('🎯 Signaling backend that frontend is ready...');
                        await invoke('frontend_ready');
                        console.log('✅ Frontend ready signal sent successfully');
                    } catch (error) {
                        console.log('DeviceUpdateManager: frontend_ready command failed:', error);
                    }
                    
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
                            // Reset update completion state for new device connections
                            setDeviceUpdateComplete(false);
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

                    console.log('✅ All event listeners set up successfully');
                    
                    // Return cleanup function that removes all listeners
                    return () => {
                        console.log('🧹 Cleaning up event listeners...');
                        if (unlistenStatusUpdate) unlistenStatusUpdate();
                        if (unlistenDeviceReady) unlistenDeviceReady();
                        if (unlistenFeaturesUpdated) unlistenFeaturesUpdated();
                        if (unlistenAccessError) unlistenAccessError();
                        if (unlistenDeviceDisconnected) unlistenDeviceDisconnected();
                    };
                    
                } catch (error) {
                    console.error("Failed to set up event listeners:", error);
                }
            };

            setupEventListeners();

            return () => {
                if (unlistenStatusUpdate) unlistenStatusUpdate();
                if (unlistenDeviceReady) unlistenDeviceReady();
            };
        }, []); // Empty dependency array ensures this runs once on mount and cleans up on unmount

        const mcpUrl = "http://127.0.0.1:1646/mcp";
        const [hasCopied, setHasCopied] = useState(false);
        
        const handleCopy = () => {
          navigator.clipboard.writeText(mcpUrl);
          setHasCopied(true);
          setTimeout(() => setHasCopied(false), 2000);
        };

        // Show the main vault interface ONLY when device is ready AND updates are complete
        console.log('📱 [App] Checking if should show VaultInterface:', {
            loadingStatus,
            deviceConnected,
            deviceUpdateComplete,
            shouldShow: loadingStatus === "Device ready" && deviceConnected && deviceUpdateComplete
        });
        
        if (loadingStatus === "Device ready" && deviceConnected && deviceUpdateComplete) {
            console.log('📱 [App] ✅ All conditions met - showing VaultInterface!');
            return <VaultInterface />;
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
              {/* Clickable Logo in the center */}
              {!onboardingActive && (
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
                onComplete={() => {
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
                }}
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
