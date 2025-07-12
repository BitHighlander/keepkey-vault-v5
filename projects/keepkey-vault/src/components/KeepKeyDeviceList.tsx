import { VStack, HStack, Box, Text, Button, Badge, Icon, Spinner, IconButton, Flex, Alert } from '@chakra-ui/react'
import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { FaUsb, FaDownload, FaWallet, FaShieldAlt, FaExclamationTriangle, FaTools, FaTrash, FaCheckCircle } from 'react-icons/fa'
import type { DeviceFeatures, DeviceStatus } from '../types/device'
import { useTroubleshootingWizard } from '../contexts/DialogContext'
const TAG = " | KeepKeyDeviceList | "
interface Device {
  id: string
  name: string
  features?: DeviceFeatures
  status?: DeviceStatus
}

interface KeepKeyDeviceListProps {
  onBootloaderUpdate: (deviceId: string) => void
  onFirmwareUpdate: (deviceId: string) => void
  onCreateWallet: (deviceId: string) => void
  onVerifySeed: (deviceId: string, deviceLabel?: string) => void
}

export const KeepKeyDeviceList = ({ 
  onBootloaderUpdate, 
  onFirmwareUpdate, 
  onCreateWallet,
  onVerifySeed 
}: KeepKeyDeviceListProps) => {
  const [devices, setDevices] = useState<Device[]>([])
  const [loading, setLoading] = useState(true)
  const [wipingDevice, setWipingDevice] = useState<string | null>(null)
  const [connectStatus, setConnectStatus] = useState<string | null>(null) // NEW: connection attempt status
  const troubleshootingWizard = useTroubleshootingWizard()

  // Listen for feature fetch retrying events from backend
  useEffect(() => {
    const unlisten = listen('feature:retrying', (event: any) => {
      // event.payload: { device_id, attempt, max }
      const { attempt, max } = event.payload || {}
      if (attempt && max) {
        if (attempt < max) {
          setConnectStatus(`Attempting to connect (${attempt}/${max})...`)
        } else {
          setConnectStatus(`Giving up after ${max} attempts – please reconnect KeepKey.`)
        }
      }
    })
    return () => {
      // @ts-ignore
      if (typeof unlisten.then === 'function') unlisten.then((fn: any) => fn())
    }
  }, [])

  // Helper function to determine if device communication is working
  const isDeviceCommunicating = (device: Device): boolean => {
    if (!device.features) {
      return false
    }
    
    // Check for indicators of failed communication
    if (device.features.version === "Unknown" || device.features.version === "") {
      return false
    }
    
    // If device info shows bootloader mode but no proper firmware version, it's likely a communication issue
    if (device.features.bootloaderMode && device.features.version === "Unknown") {
      return false
    }
    
    return true
  }

  const handleTroubleshoot = (device: Device) => {
    console.log('🔧 [KeepKeyDeviceList] Launching troubleshooting wizard for device:', device.id)
    
    const errorDetails = device.features 
      ? `Device detected but communication failed. Version: ${device.features.version || 'Unknown'}`
      : 'Device detected but no features available'
    
    troubleshootingWizard.show({
      deviceId: device.id,
      errorDetails,
      onResolved: () => {
        console.log('🔧 [KeepKeyDeviceList] Device communication restored, reloading devices')
        loadDevices() // Refresh device list
      },
      onContactSupport: (diagnostics) => {
        console.log('🔧 [KeepKeyDeviceList] User contacted support:', diagnostics)
      }
    })
  }

  const handleWipeDevice = async (deviceId: string) => {
    console.log('🗑️ [KeepKeyDeviceList] Wipe device button clicked for device:', deviceId)
    
    // TEMPORARY: Skip confirmation dialog to test wipe functionality
    // TODO: Implement proper React-based confirmation dialog
    console.log('⚠️ [KeepKeyDeviceList] TESTING: Skipping confirmation dialog - native confirm() not working in Tauri')
    console.log('🗑️ [KeepKeyDeviceList] Proceeding with wipe...')
    setWipingDevice(deviceId)
    
    try {
      console.log('🗑️ [KeepKeyDeviceList] Invoking wipe_device command...')
      await invoke('wipe_device', { deviceId })
      console.log('✅ [KeepKeyDeviceList] Device wiped successfully!')
      
      // Refresh device list to reflect changes
      console.log('🗑️ [KeepKeyDeviceList] Refreshing device list...')
      await loadDevices()
      console.log('✅ [KeepKeyDeviceList] Device list refreshed')
    } catch (error) {
      console.error('❌ [KeepKeyDeviceList] Failed to wipe device:', error)
      alert(`Failed to wipe device: ${error}`)
    } finally {
      setWipingDevice(null)
      console.log('🗑️ [KeepKeyDeviceList] Wipe operation completed')
    }
  }

  const handleVerifySeed = (device: Device) => {
    console.log('🔍 [KeepKeyDeviceList] Verify seed button clicked for device:', device.id)
    
    // Call the parent handler with device info
    onVerifySeed(device.id, device.features?.label || device.name)
  }

  useEffect(() => {
    // Initial load of devices
    loadDevices()

    // Listen for device updates
    const unsubscribeConnected = listen('device:connected', () => {
      loadDevices()
    })

    const unsubscribeDisconnected = listen('device:disconnected', () => {
      loadDevices()
    })

    const unsubscribeFeatures = listen<{
      deviceId: string
      features: DeviceFeatures
      status: DeviceStatus
    }>('device:features-updated', (event) => {
      const { deviceId, features, status } = event.payload
      setDevices(prev => prev.map(device => 
        device.id === deviceId 
          ? { ...device, features, status }
          : device
      ))
    })

    return () => {
      unsubscribeConnected.then(fn => fn())
      unsubscribeDisconnected.then(fn => fn())
      unsubscribeFeatures.then(fn => fn())
    }
  }, [])

  const loadDevices = async () => {
    let tag = TAG + " | loadDevices | "
    try {
      setLoading(true)
      // Use the enhanced command that fetches features through the device queue
      const connectedDevices = await invoke<any[]>('get_connected_devices_with_features')
      console.log(tag, connectedDevices)
      // Map the devices to our format and fetch status for each
      const mappedDevices: Device[] = await Promise.all(
        connectedDevices
          .filter(entry => entry.device.is_keepkey)
          .map(async (entry) => {
            console.log('connectedDevices: ',connectedDevices)
            const device: Device = {
              id: entry.device.unique_id,
              name: entry.device.name || 'KeepKey Device',
              features: entry.features,
              status: undefined
            }
            
            // Get device status if features are available (indicating communication works)
            if (entry.features) {
              try {
                const status = await invoke<DeviceStatus | null>('get_device_status', { 
                  deviceId: entry.device.unique_id 
                })
                if (status) {
                  device.status = status
                }
              } catch (error) {
                console.error('Failed to get device status:', error)
              }
            } else {
              // If no features, device is not communicating - log this
              console.warn(`Device ${entry.device.unique_id} detected but no features available - communication failed`)
            }
            
            return device
          })
      )
      
      setDevices(mappedDevices)
    } catch (error) {
      console.error('Failed to load devices:', error)
    } finally {
      setLoading(false)
    }
  }

  if (loading) {
    return (
      <Box textAlign="center" py={8}>
        <Spinner size="lg" color="blue.500" />
        <Text mt={4} color="gray.400">Loading devices...</Text>
      </Box>
    )
  }

  if (devices.length === 0) {
    return (
      <Box 
        textAlign="center" 
        py={12} 
        px={6}
        bg="gray.800"
        borderRadius="lg"
        borderWidth="1px"
        borderColor="gray.700"
      >
        <Icon as={FaUsb} boxSize={12} color="gray.600" mb={4} />
        <Text fontSize="lg" fontWeight="medium" color="gray.300">
          No KeepKey devices connected
        </Text>
        <Text fontSize="sm" color="gray.500" mt={2}>
          Connect your KeepKey device via USB to get started
        </Text>
      </Box>
    )
  }

  return (
    <VStack align="stretch" gap={4}>
      {/* Header with refresh button */}
      <HStack justify="space-between" mb={2}>
        <HStack gap={2}>
          <Icon as={FaUsb} color="green.400" />
          <Text fontSize="lg" fontWeight="medium" color="gray.300">
            Connected Devices
          </Text>
        </HStack>
        <IconButton
          aria-label="Refresh devices"
          size="sm"
          variant="ghost"
          colorScheme="blue"
          onClick={loadDevices}
        />
      </HStack>
      
      {devices.map((device) => (
        <Box
          key={device.id}
          borderWidth="1px"
          borderRadius="md"
          p={4}
          bg="gray.800"
          boxShadow="md"
        >
          <VStack align="stretch" gap={3}>
            {/* Device Header */}
            <HStack justify="space-between">
              <HStack gap={3}>
                <Icon as={FaUsb} color="green.400" />
                <Box>
                  <Text fontWeight="medium" color="white">
                    {device.features?.label || device.name}
                  </Text>
                  <Text fontSize="sm" color="gray.400">
                    {device.features?.deviceId 
                      ? `ID: ${device.features.deviceId.slice(0, 8)}...`
                      : device.id
                    }
                  </Text>
                </Box>
              </HStack>
              
              {/* Status Badges */}
              <HStack gap={2}>
                {device.features?.bootloaderMode && (
                  <Badge colorScheme="orange">Bootloader Mode</Badge>
                )}
                {device.features?.initialized ? (
                  <Badge colorScheme="green" bg="green.500" color="white">Initialized</Badge>
                ) : (
                  <Badge colorScheme="yellow">Not Initialized</Badge>
                )}
              </HStack>
            </HStack>

            {/* Device Info */}
            {device.features && (
              <Box 
                p={3} 
                bg="gray.750" 
                borderRadius="md"
                fontSize="sm"
              >
                <HStack justify="space-between">
                  <Text color="gray.400">Firmware Version:</Text>
                  <Text color="white" fontFamily="mono">
                    {device.features.version}
                  </Text>
                </HStack>
                
                {device.features.bootloaderVersion && (
                  <HStack justify="space-between" mt={1}>
                    <Text color="gray.400">Bootloader Version:</Text>
                    <Text color="white" fontFamily="mono">
                      {device.features.bootloaderVersion}
                    </Text>
                  </HStack>
                )}
                
                {device.features.vendor && (
                  <HStack justify="space-between" mt={1}>
                    <Text color="gray.400">Vendor:</Text>
                    <Text color="white">{device.features.vendor}</Text>
                  </HStack>
                )}
                
                {device.features.model && (
                  <HStack justify="space-between" mt={1}>
                    <Text color="gray.400">Model:</Text>
                    <Text color="white">{device.features.model}</Text>
                  </HStack>
                )}
              </Box>
            )}

            {/* Action Buttons - Conditional based on device communication */}
            {!isDeviceCommunicating(device) ? (
              /* Device communication failed - show warning and troubleshoot button */
              <VStack gap={3} pt={2}>
                <Box 
                  w="full"
                  p={3} 
                  bg="yellow.900" 
                  borderRadius="md"
                  borderWidth="1px"
                  borderColor="yellow.600"
                >
                  <HStack gap={2} mb={2}>
                    <Icon as={FaExclamationTriangle} color="yellow.400" />
                    <Text fontSize="sm" fontWeight="medium" color="yellow.100">
                      Device detected but not communicating
                    </Text>
                  </HStack>
                  <Text fontSize="xs" color="yellow.200">
                    Your KeepKey was detected but we can't establish communication. 
                    This is usually fixable with basic troubleshooting steps.
                  </Text>
                </Box>
                
                <Button
                  size="sm"
                  colorScheme="yellow"
                  onClick={() => handleTroubleshoot(device)}
                  w="full"
                >
                  <HStack gap={2}>
                    <FaTools />
                    <Text fontSize="sm">Launch Troubleshooter</Text>
                  </HStack>
                </Button>
              </VStack>
            ) : (
              /* Device communication working - show normal action buttons */
              <Flex gap={2} pt={2} wrap="wrap">
                <Button
                  size="sm"
                  colorScheme="orange"
                  onClick={() => onBootloaderUpdate(device.id)}
                  disabled={device.features?.bootloaderVersion === "2.1.4"}
                  flex="1"
                  minW="140px"
                >
                  <HStack gap={1}>
                    <FaShieldAlt />
                    <Text fontSize="xs">Update Bootloader</Text>
                  </HStack>
                </Button>
                
                <Button
                  size="sm"
                  colorScheme="blue"
                  onClick={() => onFirmwareUpdate(device.id)}
                  disabled={device.features?.bootloaderMode}
                  flex="1"
                  minW="140px"
                >
                  <HStack gap={1}>
                    <FaDownload />
                    <Text fontSize="xs">Update Firmware</Text>
                  </HStack>
                </Button>
                
                <Button
                  size="sm"
                  colorScheme="green"
                  onClick={() => onCreateWallet(device.id)}
                  disabled={device.features?.initialized}
                  flex="1"
                  minW="120px"
                >
                  <HStack gap={1}>
                    <FaWallet />
                    <Text fontSize="xs">Create Wallet</Text>
                  </HStack>
                </Button>
                
                {/* Verify Seed button - only show for initialized devices */}
                {device.features?.initialized && (
                  <Button
                    size="sm"
                    colorScheme="blue"
                    variant="outline"
                    onClick={() => handleVerifySeed(device)}
                    disabled={device.features?.bootloaderMode}
                    flex="1"
                    minW="120px"
                  >
                    <HStack gap={1}>
                      <FaCheckCircle />
                      <Text fontSize="xs">Verify Seed</Text>
                    </HStack>
                  </Button>
                )}
              </Flex>
            )}

            {/* Update Status */}
            {device.status && (
              <Box 
                p={3} 
                bg="blue.900" 
                borderRadius="md"
                borderWidth="1px"
                borderColor="blue.700"
              >
                <Text fontSize="sm" color="blue.300" fontWeight="medium">
                  Update Status:
                </Text>
                
                {device.status.needsBootloaderUpdate && (
                  <Text fontSize="sm" color="orange.300" mt={1}>
                    • Bootloader update available (v{device.status.bootloaderCheck?.latestVersion})
                  </Text>
                )}
                
                {device.status.needsFirmwareUpdate && (
                  <Text fontSize="sm" color="blue.300" mt={1}>
                    • Firmware update available (v{device.status.firmwareCheck?.latestVersion})
                  </Text>
                )}
                
                {device.status.needsInitialization && (
                  <Text fontSize="sm" color="yellow.300" mt={1}>
                    • Device needs to be initialized
                  </Text>
                )}
                
                {!device.status.needsBootloaderUpdate && 
                 !device.status.needsFirmwareUpdate && 
                 !device.status.needsInitialization && (
                  <Text fontSize="sm" color="green.300" mt={1}>
                    • Device is up to date and ready
                  </Text>
                )}
              </Box>
            )}

            {/* Device Management Footer - Only show wipe for initialized devices */}
            {device.features?.initialized && isDeviceCommunicating(device) && (
              <Box 
                p={2} 
                bg="gray.900" 
                borderRadius="md"
                borderWidth="1px"
                borderColor="red.800"
                borderTop="none"
                mt={-2}
              >
                <HStack justify="space-between" align="center">
                  <Text fontSize="xs" color="gray.400">
                    Danger Zone:
                  </Text>
                  <Button
                    size="xs"
                    bg="red.600"
                    color="white"
                    onClick={() => {
                      console.log('🗑️ Button clicked for device:', device.id)
                      handleWipeDevice(device.id)
                    }}
                    disabled={wipingDevice === device.id}
                    _hover={{ bg: "red.700" }}
                    _active={{ bg: "red.800" }}
                  >
                    <HStack gap={1}>
                      <FaTrash />
                      <Text fontSize="xs">
                        {wipingDevice === device.id ? 'Wiping...' : 'Wipe Device'}
                      </Text>
                    </HStack>
                  </Button>
                </HStack>
              </Box>
            )}
          </VStack>
        </Box>
      ))}
    </VStack>
  )
}