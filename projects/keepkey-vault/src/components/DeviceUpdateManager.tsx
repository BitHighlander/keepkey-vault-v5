import { useEffect, useState } from 'react'
import { BootloaderUpdateDialog } from './BootloaderUpdateDialog'
import { FirmwareUpdateDialog } from './FirmwareUpdateDialog'
import { WalletCreationWizard } from './WalletCreationWizard/WalletCreationWizard'
import { EnterBootloaderModeDialog } from './EnterBootloaderModeDialog'
import { PinUnlockDialog } from './PinUnlockDialog'
import type { DeviceStatus, DeviceFeatures } from '../types/device'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { useDeviceInvalidStateDialog } from '../contexts/DialogContext'

interface DeviceUpdateManagerProps {
  // Optional callback when all updates/setup is complete
  onComplete?: () => void
}

export const DeviceUpdateManager: React.FC<DeviceUpdateManagerProps> = ({ onComplete }) => {
  const [deviceStatus, setDeviceStatus] = useState<DeviceStatus | null>(null)
  const [connectedDeviceId, setConnectedDeviceId] = useState<string | null>(null)
  const [showBootloaderUpdate, setShowBootloaderUpdate] = useState(false)
  const [showFirmwareUpdate, setShowFirmwareUpdate] = useState(false)
  const [showWalletCreation, setShowWalletCreation] = useState(false)
  const [showEnterBootloaderMode, setShowEnterBootloaderMode] = useState(false)
  const [showPinUnlock, setShowPinUnlock] = useState(false)
  const [isProcessing, setIsProcessing] = useState(false)
  const [retryCount, setRetryCount] = useState(0)
  const [hasCompletedOnce, setHasCompletedOnce] = useState(false)
  
  // Track processed events to prevent loops
  const [processedEvents, setProcessedEvents] = useState<Set<string>>(new Set())
  const [lastProcessedStatus, setLastProcessedStatus] = useState<string | null>(null)
  
  // Get device invalid state dialog hook
  const deviceInvalidStateDialog = useDeviceInvalidStateDialog()

  // Function to try getting device status via command when events fail
  const tryGetDeviceStatus = async (deviceId: string, attempt = 1) => {
    const maxAttempts = 3
    console.log(`Attempting to get device status for ${deviceId} (attempt ${attempt}/${maxAttempts})`)
    
    try {
      const status = await invoke<DeviceStatus | null>('get_device_status', { deviceId })
      if (status) {
        console.log('Successfully got device status via command:', status)
        setDeviceStatus(status)
        handleDeviceStatus(status)
        return true
      } else {
        console.log('No device status returned')
        return false
      }
    } catch (error) {
      console.error(`Failed to get device status (attempt ${attempt}):`, error)
      
      if (attempt < maxAttempts) {
        const delay = Math.min(1000 * Math.pow(2, attempt - 1), 5000) // Exponential backoff, max 5s
        console.log(`Retrying in ${delay}ms...`)
        setTimeout(() => {
          tryGetDeviceStatus(deviceId, attempt + 1)
        }, delay)
      } else {
        console.error('Max attempts reached, giving up on getting device status')
      }
      return false
    }
  }

  const handleDeviceStatus = (status: DeviceStatus) => {
    console.log('🔧 DeviceUpdateManager: Handling device status:', status)
    
    // Create a unique key for this status to prevent duplicate processing (without timestamp)
    const statusKey = `${status.deviceId}-${status.needsInitialization}-${status.needsFirmwareUpdate}-${status.needsBootloaderUpdate}-${status.needsPinUnlock}`
    
    // Check if we've already processed this exact status recently
    if (lastProcessedStatus === statusKey) {
      console.log('🔧 DeviceUpdateManager: Skipping duplicate status processing')
      return
    }
    
    setLastProcessedStatus(statusKey)
    
    console.log('🔧 DeviceUpdateManager: Status needsInitialization:', status.needsInitialization)
    console.log('🔧 DeviceUpdateManager: Status needsFirmwareUpdate:', status.needsFirmwareUpdate)
    console.log('🔧 DeviceUpdateManager: Status needsBootloaderUpdate:', status.needsBootloaderUpdate)
    console.log('🔧 DeviceUpdateManager: Status needsPinUnlock:', status.needsPinUnlock)

    // Check bootloader mode specifically
    const bootloaderModeCheck = {
      bootloader_mode: status.features?.bootloader_mode,
      needsBootloaderUpdate: status.needsBootloaderUpdate
    }
    console.log('🔧 Bootloader mode check:', bootloaderModeCheck)

    // Clear all dialogs first
    setShowBootloaderUpdate(false)
    setShowFirmwareUpdate(false) 
    setShowWalletCreation(false)
    setShowEnterBootloaderMode(false)
    setShowPinUnlock(false)

    // Handle bootloader update first (highest priority)
    if (status.needsBootloaderUpdate) {
      console.log('🔧 DeviceUpdateManager: Device needs bootloader update')
      setShowBootloaderUpdate(true)
      return
    }
    
    // Handle firmware update
    if (status.needsFirmwareUpdate) {
      console.log('🔧 DeviceUpdateManager: Device needs firmware update')
      setShowFirmwareUpdate(true)
      return
    }
    
    // Handle PIN unlock
    if (status.needsPinUnlock) {
      console.log('🔧 DeviceUpdateManager: Device needs PIN unlock')
      setShowPinUnlock(true)
      return
    }
    
    // Handle wallet creation/initialization
    if (status.needsInitialization) {
      console.log('🔧 DeviceUpdateManager: Device needs initialization')
      setShowWalletCreation(true)
      return
    }
    
    // All checks passed - device is ready
    console.log('🔧 DeviceUpdateManager: Device is ready, no updates needed')
    
    // Prevent multiple onComplete calls for the same device
    if (!hasCompletedOnce) {
      console.log('🔧 DeviceUpdateManager: Calling onComplete() - this will show VaultInterface')
      setHasCompletedOnce(true)
      // Use optional chaining to safely call onComplete
      onComplete?.()
    } else {
      console.log('🔧 DeviceUpdateManager: Device ready but onComplete() already called, skipping')
    }
  }

  // Reset completion state when device changes
  useEffect(() => {
    if (connectedDeviceId) {
      // Only reset if we have a different device ID
      setProcessedEvents(new Set())
      setLastProcessedStatus(null)
      // Don't automatically reset hasCompletedOnce - let it be based on actual device state
    }
  }, [connectedDeviceId])

  useEffect(() => {
    let featuresUnsubscribe: Promise<() => void> | null = null
    let connectedUnsubscribe: Promise<() => void> | null = null
    let timeoutId: NodeJS.Timeout | null = null

    const setupListeners = async () => {
      console.log('DeviceUpdateManager: Setting up event listeners...')
      
      // Listen for device features updates which include status (primary method)
      featuresUnsubscribe = listen<{
        deviceId: string
        features: DeviceFeatures
        status: DeviceStatus
      }>('device:features-updated', (event) => {
        console.log('🔧 DeviceUpdateManager: Device features updated event received:', event.payload)
        const { status } = event.payload
        console.log('🔧 DeviceUpdateManager: Extracted status from event:', status)
        
        // Create event key for deduplication
        const eventKey = `${status.deviceId}-${JSON.stringify(status)}`
        
        // Check if we've already processed this exact event
        if (processedEvents.has(eventKey)) {
          console.log('🔧 DeviceUpdateManager: Skipping duplicate event')
          return
        }
        
        // Mark event as processed
        setProcessedEvents(prev => new Set([...prev, eventKey]))
        
        // Check if recovery is in progress - if so, be very careful about state changes
        if ((window as any).KEEPKEY_RECOVERY_IN_PROGRESS) {
          console.log('🛡️ DeviceUpdateManager: Recovery in progress - handling features event carefully')
          // Still update device status (for recovery to work) but don't trigger UI changes
          setDeviceStatus(status)
          setConnectedDeviceId(status.deviceId)
          setRetryCount(0)
          // DO NOT call handleDeviceStatus during recovery to prevent UI conflicts
          return;
        }
        
        setDeviceStatus(status)
        setConnectedDeviceId(status.deviceId)
        setRetryCount(0) // Reset retry count on successful event
        handleDeviceStatus(status)
      })

      // Listen for basic device connected events as fallback
      connectedUnsubscribe = listen<{
        unique_id: string
        name: string
        vid: number
        pid: number
        manufacturer?: string
        product?: string
        serial_number?: string
        is_keepkey: boolean
      }>('device:connected', (event) => {
        const device = event.payload
        console.log('Device connected event received (fallback):', device)
        
        if (device.is_keepkey) {
          setConnectedDeviceId(device.unique_id)
          
          // Set a timeout to try getting device status if features event doesn't come
          if (timeoutId) clearTimeout(timeoutId)
          timeoutId = setTimeout(() => {
            console.log('Features event timeout, trying direct device status call...')
            tryGetDeviceStatus(device.unique_id)
          }, 3000) // Wait 3 seconds for features event before trying fallback
        }
      })

      // Listen for device access errors
      const accessErrorUnsubscribe = listen<{
        deviceId: string
        error: string
        errorType: string
        status: string
      }>('device:access-error', (event) => {
        console.log('Device access error received:', event.payload)
        // Clear any pending dialogs when there's an access error
        setShowBootloaderUpdate(false)
        setShowFirmwareUpdate(false)
        setShowWalletCreation(false)
        setShowEnterBootloaderMode(false)
        setShowPinUnlock(false)
        setDeviceStatus(null)
        setConnectedDeviceId(null)
      })

      // Listen for device invalid state (timeout) errors
      const invalidStateUnsubscribe = listen<{
        deviceId: string
        error: string
        errorType: string
        status: string
      }>('device:invalid-state', (event) => {
        console.log('⏱️ Device invalid state detected:', event.payload)
        
        // Check if this is a transient error that should be handled gracefully
        const isTransient = event.payload.error.includes('Device operation timed out') ||
                          event.payload.error.includes('temporarily unavailable') ||
                          event.payload.error.includes('Device not found') ||
                          event.payload.error.includes('Communication Timeout')
        
        if (isTransient) {
          console.log('📋 Treating as transient error - applying grace period')
          // setTemporarilyDisconnected(true) // This state was removed, so this line is removed
          
          // Clear any existing timeout
          // if (disconnectionTimeout) { // This state was removed, so this line is removed
          //   clearTimeout(disconnectionTimeout)
          // }
          
          // Set a timeout to show dialog if not reconnected within grace period
          // const timeout = setTimeout(() => { // This state was removed, so this line is removed
          //   if (temporarilyDisconnected) { // This state was removed, so this line is removed
          //     console.log('⏰ Grace period expired - showing invalid state dialog') // This state was removed, so this line is removed
          //     showInvalidStateDialog(event.payload) // This state was removed, so this line is removed
          //   } // This state was removed, so this line is removed
          // }, 10000) // 10 second grace period // This state was removed, so this line is removed
          
          // setDisconnectionTimeout(timeout) // This state was removed, so this line is removed
          return
        }
        
        // Non-transient error - show dialog immediately
        // showInvalidStateDialog(event.payload) // This state was removed, so this line is removed
      })
      
      // const showInvalidStateDialog = (payload: any) => { // This state was removed, so this line is removed
      //   // CRITICAL: Clear ALL existing dialogs first // This state was removed, so this line is removed
      //   setShowBootloaderUpdate(false) // This state was removed, so this line is removed
      //   setShowFirmwareUpdate(false) // This state was removed, so this line is removed
      //   setShowWalletCreation(false) // This state was removed, so this line is removed
      //   setShowEnterBootloaderMode(false) // This state was removed, so this line is removed
      //   setShowPinUnlock(false)  // This is crucial to prevent overlapping // This state was removed, so this line is removed
        
      //   // Clear device status to prevent any further state updates // This state was removed, so this line is removed
      //   setDeviceStatus(null) // This state was removed, so this line is removed
        
      //   // Show the simple invalid state dialog // This state was removed, so this line is removed
      //   deviceInvalidStateDialog.show({ // This state was removed, so this line is removed
      //     deviceId: payload.deviceId, // This state was removed, so this line is removed
      //     error: payload.error, // This state was removed, so this line is removed
      //     onDialogClose: () => { // This state was removed, so this line is removed
      //       console.log('Invalid state dialog closed - user should reconnect device') // This state was removed, so this line is removed
      //       // Device status will be updated when device reconnects // This state was removed, so this line is removed
      //     } // This state was removed, so this line is removed
      //   }) // This state was removed, so this line is removed
      // } // This state was removed, so this line is removed

      // Listen for PIN unlock needed events
      const pinUnlockUnsubscribe = listen<{
        deviceId: string
        features: DeviceFeatures
        status: DeviceStatus
        needsPinUnlock: boolean
      }>('device:pin-unlock-needed', async (event) => {
        console.log('🔒 DeviceUpdateManager: PIN unlock needed event received:', event.payload)
        const { status } = event.payload
        
        // CRITICAL: Hide any invalid state dialogs first - PIN has priority
        if (deviceInvalidStateDialog.isShowing(status.deviceId)) {
          console.log('🔒 Hiding invalid state dialog to show PIN dialog')
          deviceInvalidStateDialog.hide(status.deviceId)
        }
        
        // Verify device is actually ready for PIN operations before showing dialog
        try {
          const isPinReady = await invoke('check_device_pin_ready', { deviceId: status.deviceId })
          
          if (isPinReady) {
            // Show PIN unlock dialog
            console.log('🔒 DeviceUpdateManager: Device confirmed ready for PIN, showing unlock dialog')
            setDeviceStatus(status)
            setConnectedDeviceId(status.deviceId)
            setShowBootloaderUpdate(false)
            setShowFirmwareUpdate(false)
            setShowWalletCreation(false)
            setShowEnterBootloaderMode(false)
            setShowPinUnlock(true)
          } else {
            console.log('🔒 DeviceUpdateManager: Device not ready for PIN unlock, waiting...')
            // Device may not be ready yet, wait for next status update
          }
        } catch (error) {
          console.error('🔒 DeviceUpdateManager: Failed to check PIN readiness:', error)
          // Fallback to showing the dialog anyway
          setDeviceStatus(status)
          setConnectedDeviceId(status.deviceId)
          setShowBootloaderUpdate(false)
          setShowFirmwareUpdate(false)
          setShowWalletCreation(false)
          setShowEnterBootloaderMode(false)
          setShowPinUnlock(true)
        }
      })

      // Listen for device reconnection
      const reconnectedUnsubscribe = listen<{
        deviceId: string
        wasTemporary: boolean
      }>('device:reconnected', (event) => {
        console.log('🔄 Device reconnected:', event.payload)
        
        // if (event.payload.wasTemporary) { // This state was removed, so this line is removed
        //   console.log('✅ Temporary disconnection resolved') // This state was removed, so this line is removed
        //   setTemporarilyDisconnected(false) // This state was removed, so this line is removed
          
        //   // Clear the grace period timeout // This state was removed, so this line is removed
        //   if (disconnectionTimeout) { // This state was removed, so this line is removed
        //     clearTimeout(disconnectionTimeout) // This state was removed, so this line is removed
        //     setDisconnectionTimeout(null) // This state was removed, so this line is removed
        //   } // This state was removed, so this line is removed
          
        //   // If invalid state dialog is showing for this device, hide it // This state was removed, so this line is removed
        //   if (deviceInvalidStateDialog.isShowing(event.payload.deviceId)) { // This state was removed, so this line is removed
        //     console.log('🔄 Hiding invalid state dialog due to reconnection') // This state was removed, so this line is removed
        //     deviceInvalidStateDialog.hide(event.payload.deviceId) // This state was removed, so this line is removed
        //   } // This state was removed, so this line is removed
        // } // This state was removed, so this line is removed
      })

      // Listen for device disconnection
      const disconnectedUnsubscribe = listen<string>('device:disconnected', (event) => {
        const disconnectedDeviceId = event.payload;
        console.log('Device disconnected:', disconnectedDeviceId)
        
        // Check if recovery is in progress - if so, ignore disconnection events
        if ((window as any).KEEPKEY_RECOVERY_IN_PROGRESS) {
          console.log('🛡️ DeviceUpdateManager: Recovery in progress - IGNORING disconnection event')
          console.log('🛡️ DeviceUpdateManager: Keeping current state to protect recovery')
          return; // Don't change state during recovery
        }
        
        // Clear all state when device disconnects (only if not in recovery)
        setDeviceStatus(null)
        setConnectedDeviceId(null)
        setShowBootloaderUpdate(false)
        setShowFirmwareUpdate(false)
        setShowWalletCreation(false)
        setShowEnterBootloaderMode(false)
        setShowPinUnlock(false)
        setRetryCount(0)
        if (timeoutId) clearTimeout(timeoutId)
        
        // Also hide the invalid state dialog if it's showing for this device
        if (deviceInvalidStateDialog.isShowing(disconnectedDeviceId)) {
          console.log('🔌 Hiding invalid state dialog for disconnected device')
          deviceInvalidStateDialog.hide(disconnectedDeviceId)
        }
      })

      // Frontend ready signal is now sent by App.tsx during initial setup

      return async () => {
        if (featuresUnsubscribe) (await featuresUnsubscribe)()
        if (connectedUnsubscribe) (await connectedUnsubscribe)()
        ;(await accessErrorUnsubscribe)()
        ;(await invalidStateUnsubscribe)()
        ;(await pinUnlockUnsubscribe)()
        ;(await reconnectedUnsubscribe)()
        ;(await disconnectedUnsubscribe)()
        if (timeoutId) clearTimeout(timeoutId)
        // if (disconnectionTimeout) clearTimeout(disconnectionTimeout) // This state was removed, so this line is removed
      }
    }

    setupListeners()

    return () => {
      // Cleanup function will be called automatically
      if (timeoutId) clearTimeout(timeoutId)
    }
  }, []) // Remove onComplete from dependencies to prevent infinite loop

  const handleFirmwareUpdate = async () => {
    setIsProcessing(true)
    try {
      // Update firmware using our implemented Tauri command
      await invoke('update_device_firmware', { 
        deviceId: deviceStatus?.deviceId,
        targetVersion: deviceStatus?.firmwareCheck?.latestVersion || ''
      })
      
      // After successful update, check if initialization is needed
      setShowFirmwareUpdate(false)
    } catch (error) {
      console.error('Firmware update failed:', error)
      // TODO: Show error dialog
    } finally {
      setIsProcessing(false)
    }
  }

  const handleFirmwareSkip = () => {
    setShowFirmwareUpdate(false)
    
    // Check if we need to show wallet creation
    if (deviceStatus?.needsInitialization) {
      setShowWalletCreation(true)
    } else {
      onComplete?.()
    }
  }

  const handleFirmwareRemindLater = () => {
    // TODO: Store reminder preference
    setShowFirmwareUpdate(false)
    
    // Continue to next step
    if (deviceStatus?.needsInitialization) {
      setShowWalletCreation(true)
    } else {
      onComplete?.()
    }
  }

  const handleWalletCreationComplete = () => {
    setShowWalletCreation(false)
    onComplete?.()
  }

  const handleEnterBootloaderModeClose = () => {
    setShowEnterBootloaderMode(false)
    // Don't call onComplete here - wait for user to actually enter bootloader mode
  }

  const handlePinUnlocked = async () => {
    console.log('🔒 PIN unlock successful, device is now unlocked')
    console.log('🔒 Current dialog states:', {
      showPinUnlock,
      showBootloaderUpdate,
      showFirmwareUpdate,
      showWalletCreation,
      showEnterBootloaderMode
    })
    setShowPinUnlock(false)
    
    // Device is now ready - webview will handle wallet functionality
    console.log('✅ Device ready after PIN unlock - webview will handle wallet operations')
    
    // Device should now be ready to use
    console.log('🔒 Calling onComplete after PIN unlock')
    onComplete?.()
  }

  const handlePinUnlockClose = () => {
    setShowPinUnlock(false)
    // Don't call onComplete - user cancelled PIN entry
  }

  if (!deviceStatus) {
    console.log('🔧 DeviceUpdateManager: No deviceStatus, returning null')
    return null
  }

  console.log('🔧 DeviceUpdateManager: Rendering with state:', {
    showWalletCreation,
    showFirmwareUpdate,
    showBootloaderUpdate,
    showEnterBootloaderMode,
    showPinUnlock,
    deviceStatus: deviceStatus?.needsInitialization
  })

  return (
    <>
      {showEnterBootloaderMode && deviceStatus.bootloaderCheck && deviceStatus.deviceId && (
        <EnterBootloaderModeDialog
          isOpen={showEnterBootloaderMode}
          bootloaderCheck={deviceStatus.bootloaderCheck}
          deviceId={deviceStatus.deviceId}
          onClose={handleEnterBootloaderModeClose}
        />
      )}

      {showBootloaderUpdate && deviceStatus.bootloaderCheck && deviceStatus.deviceId && (
        <BootloaderUpdateDialog
          isOpen={showBootloaderUpdate}
          bootloaderCheck={deviceStatus.bootloaderCheck}
          deviceId={deviceStatus.deviceId}
          onUpdateComplete={() => {
            setShowBootloaderUpdate(false)
            // The device will restart and emit new features
          }}
        />
      )}

      {showFirmwareUpdate && deviceStatus.firmwareCheck && (
        <FirmwareUpdateDialog
          isOpen={showFirmwareUpdate}
          firmwareCheck={deviceStatus.firmwareCheck}
          onUpdateStart={handleFirmwareUpdate}
          onSkip={handleFirmwareSkip}
          onRemindLater={handleFirmwareRemindLater}
          onClose={() => setShowFirmwareUpdate(false)}
          isLoading={isProcessing}
        />
      )}

      {showWalletCreation && deviceStatus.deviceId && (
        <WalletCreationWizard
          deviceId={deviceStatus.deviceId}
          onComplete={handleWalletCreationComplete}
          onClose={() => setShowWalletCreation(false)}
        />
      )}

      {showPinUnlock && deviceStatus.deviceId && (
        <PinUnlockDialog
          isOpen={showPinUnlock}
          deviceId={deviceStatus.deviceId}
          onUnlocked={handlePinUnlocked}
          onClose={handlePinUnlockClose}
        />
      )}
    </>
  )
} 