import { useState, useEffect, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Button, Text, HStack, Icon, VStack, Box, Spinner, SimpleGrid, Heading } from '@chakra-ui/react'
import { FaCircle, FaExclamationTriangle, FaTimes, FaCheckCircle, FaSync, FaBackspace } from 'react-icons/fa'

interface PinUnlockDialogProps {
  isOpen: boolean
  deviceId: string
  onUnlocked: () => void
  onClose: () => void
}

// The KeepKey device shows this scrambled layout on its screen:
// 7 8 9
// 4 5 6
// 1 2 3
// We need to send these exact numbers when the user clicks each position
const PIN_MATRIX_LAYOUT = [7, 8, 9, 4, 5, 6, 1, 2, 3] as const

export const PinUnlockDialog = ({ isOpen, deviceId, onUnlocked, onClose }: PinUnlockDialogProps) => {
  const [pinPositions, setPinPositions] = useState<number[]>([])
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [step, setStep] = useState<'verifying' | 'trigger' | 'enter' | 'submitting' | 'success'>('verifying')
  const [retryCount, setRetryCount] = useState(0)
  const [deviceReadyStatus, setDeviceReadyStatus] = useState<string>('Checking device...')

  // Debug component lifecycle
  useEffect(() => {
    console.log('🔒 PinUnlockDialog mounted, isOpen:', isOpen)
    return () => {
      console.log('🔒 PinUnlockDialog unmounting')
    }
  }, [])

  // Reset state when dialog opens/closes
  useEffect(() => {
    if (isOpen) {
      setPinPositions([])
      setError(null)
      setStep('verifying')
      setRetryCount(0)
      setDeviceReadyStatus('Checking device...')
      verifyDeviceReadiness()
    }
  }, [isOpen, deviceId])

  const verifyDeviceReadiness = async () => {
    try {
      setIsLoading(true)
      setError(null)
      setDeviceReadyStatus('Verifying device is ready for PIN...')
      console.log('🔍 Verifying device readiness for PIN unlock:', deviceId)
      
      // Use the dedicated device PIN readiness check
      const isPinReady = await invoke('check_device_pin_ready', { deviceId })
      console.log('📊 Device PIN ready status:', isPinReady)
      
      if (!isPinReady) {
        // Device is not ready or no longer needs PIN unlock
        console.log('✅ Device no longer needs PIN unlock or is not ready, closing dialog')
        onUnlocked()
        return
      }
      
      // Device is ready for PIN unlock attempt
      setDeviceReadyStatus('Device ready - requesting PIN matrix...')
      await triggerPinRequest()
      
    } catch (err: any) {
      console.error('❌ Device readiness verification failed:', err)
      setError(`Device not ready: ${err}`)
      setStep('trigger')
      setDeviceReadyStatus('Device not ready')
    } finally {
      setIsLoading(false)
    }
  }

  const triggerPinRequest = async () => {
    try {
      setIsLoading(true)
      setError(null)
      setDeviceReadyStatus('Requesting PIN matrix from device...')
      console.log('🔐 Triggering PIN request for device:', deviceId)
      
      const result = await invoke('trigger_pin_request', { deviceId })
      
      if (result === true) {
        // PIN request was successfully triggered - device should now be showing PIN matrix
        console.log('✅ PIN trigger successful, device should be showing PIN matrix')
        setStep('enter')
        setDeviceReadyStatus('PIN matrix ready')
        setError(null) // Clear any previous errors
      } else {
        throw new Error('PIN trigger returned unexpected result')
      }
      
    } catch (err: any) {
      console.error('❌ PIN trigger failed:', err)
      
      const errorStr = String(err).toLowerCase()
      
      // Check if device is already showing PIN matrix (expected "failure")
      if (errorStr.includes('unknown message') || errorStr.includes('failure: unknown message')) {
        console.log('🔐 Device is already in PIN mode (expected behavior), proceeding to PIN entry')
        setStep('enter')
        setDeviceReadyStatus('PIN matrix ready')
        setError(null)
        return
      }
      
      // Check if this is a device communication issue
      if (errorStr.includes('device not found') || errorStr.includes('not connected')) {
        setError('Device disconnected. Please reconnect your KeepKey and try again.')
        setStep('trigger')
      } else if (errorStr.includes('device already in use') || errorStr.includes('claimed')) {
        setError('Device is being used by another application. Please close other wallet software and try again.')
        setStep('trigger')
      } else if (errorStr.includes('timeout')) {
        setError('Device communication timeout. Please check your connection and try again.')
        setStep('trigger')
      } else {
        // For other errors, allow retry but show the specific error
        const userFriendlyError = errorStr.includes('failed to trigger pin request') 
          ? 'Unable to request PIN from device. Please check your device screen and try again.'
          : `Communication error: ${err}`
        
        setError(userFriendlyError)
        setStep('trigger')
      }
      
      setDeviceReadyStatus('PIN request failed')
    } finally {
      setIsLoading(false)
    }
  }

  const handlePinButtonClick = useCallback((position: number) => {
    if (pinPositions.length >= 9) return // Max 9 digits
    
    console.log(`Button clicked: position ${position}`)
    setPinPositions(prev => [...prev, position])
  }, [pinPositions.length])

  const handleBackspace = useCallback(() => {
    setPinPositions(prev => prev.slice(0, -1))
  }, [])

  const handleSubmitPin = async () => {
    if (pinPositions.length === 0) {
      setError('Please enter your PIN')
      return
    }

    try {
      setStep('submitting')
      setError(null)
      console.log('🔐 Submitting PIN with positions:', pinPositions)
      
      const result = await invoke('send_pin_matrix_ack', { 
        deviceId, 
        positions: pinPositions 
      })
      
      if (result === true) {
        // PIN submitted successfully - show success feedback briefly before closing
        console.log('✅ PIN submitted successfully')
        setStep('success')
        
        // Auto-close after brief success display
        setTimeout(() => {
          console.log('🔒 PIN dialog auto-closing after success')
          onUnlocked()
        }, 1000)
      } else {
        throw new Error('PIN verification failed')
      }
      
    } catch (err: any) {
      console.error('❌ PIN submission failed:', err)
      
      // This is a real PIN validation error - show it clearly
      const errorStr = String(err)
      if (errorStr.toLowerCase().includes('incorrect') || errorStr.toLowerCase().includes('invalid') || errorStr.toLowerCase().includes('wrong')) {
        setError('Incorrect PIN. Please check your device screen and try again.')
        setRetryCount(prev => prev + 1)
        
        // If too many failed attempts, warn user about device lockout
        if (retryCount >= 2) {
          setError('Incorrect PIN. Warning: Too many failed attempts may temporarily lock your device!')
        }
      } else if (errorStr.toLowerCase().includes('device not found')) {
        setError('Device disconnected during PIN entry. Please reconnect and try again.')
      } else if (errorStr.toLowerCase().includes('locked') || errorStr.toLowerCase().includes('too many')) {
        setError('Device is temporarily locked due to too many failed PIN attempts. Please wait and try again later.')
        setStep('trigger')
        return
      } else {
        setError(`PIN verification failed: ${err}`)
      }
      
      // Reset to PIN entry step and clear entered PIN
      setPinPositions([])
      setStep('enter')
    }
  }

  const handleClearPin = () => {
    setPinPositions([])
  }

  const handleRetry = () => {
    setError(null)
    setRetryCount(prev => prev + 1)
    
    if (retryCount >= 2) {
      // After multiple retries, go back to device verification
      setStep('verifying')
      verifyDeviceReadiness()
    } else {
      // Simple retry of PIN trigger
      setStep('trigger')
      triggerPinRequest()
    }
  }

  // Generate PIN dots for display
  const pinDots = Array.from({ length: Math.max(4, pinPositions.length) }, (_, i) => (
    <Box
      key={i}
      w="10px"
      h="10px"
      borderRadius="full"
      bg={i < pinPositions.length ? "green.400" : "gray.600"}
      opacity={i < pinPositions.length ? 1 : 0.5}
      transition="all 0.2s"
    />
  ))

  if (!isOpen) return null

  return (
    <Box
      position="fixed"
      top={0}
      left={0}
      right={0}
      bottom={0}
      bg="blackAlpha.800"
      zIndex={99999}
      display="flex"
      alignItems="center"
      justifyContent="center"
    >
      <Box
        bg="gray.800"
        color="white"
        borderRadius="xl"
        boxShadow="2xl"
        borderWidth="1px"
        borderColor="gray.700"
        overflow="hidden"
        maxW="400px"
        w="90%"
      >
        {/* Header */}
        <Box bg="gray.850" p={4} position="relative">
          <Heading fontSize="xl" fontWeight="bold" color="white" textAlign="center">
            Unlock Device
          </Heading>
          <Button
            position="absolute"
            right={2}
            top="50%"
            transform="translateY(-50%)"
            size="sm"
            variant="ghost"
            onClick={onClose}
            color="gray.400"
            _hover={{ color: "white", bg: "gray.700" }}
            borderRadius="md"
          >
            <Icon as={FaTimes} />
          </Button>
        </Box>

        <Box p={5}>
          <VStack gap={4}>
            
            {step === 'verifying' && (
              <VStack gap={3} py={4}>
                <Spinner size="lg" color="blue.400" />
                <Text fontSize="md" fontWeight="semibold">
                  Preparing Device
                </Text>
                <Text fontSize="sm" color="gray.400">
                  {deviceReadyStatus}
                </Text>
              </VStack>
            )}

            {step === 'trigger' && (
              <VStack gap={3} py={4}>
                <Text color="gray.300" fontSize="sm">
                  Your device will display a PIN matrix when ready
                </Text>
                {isLoading ? (
                  <Spinner size="md" color="blue.400" />
                ) : (
                  <Button onClick={triggerPinRequest} colorScheme="blue" size="md">
                    Request PIN Matrix
                  </Button>
                )}
              </VStack>
            )}

            {step === 'enter' && (
              <VStack gap={3} w="full">
                <VStack gap={1}>
                  <Text color="gray.300" fontSize="sm" textAlign="center">
                    Look at your device screen for the scrambled numbers
                  </Text>
                  <Text color="gray.400" fontSize="xs" textAlign="center">
                    Click the positions where your PIN digits appear
                  </Text>
                </VStack>

                {/* PIN dots display */}
                <Box
                  p={3}
                  bg="gray.750"
                  borderRadius="lg"
                  borderWidth="1px"
                  borderColor="gray.600"
                >
                  <HStack gap={1.5} justify="center">
                    {pinDots}
                  </HStack>
                </Box>

                {/* PIN Grid - matches device layout */}
                <Box>
                  <SimpleGrid
                    columns={3}
                    gap={2}
                    w="200px"
                    mx="auto"
                  >
                    {PIN_MATRIX_LAYOUT.map((number, index) => (
                      <Button
                        key={index}
                        onClick={() => handlePinButtonClick(number)}
                        size="md"
                        h="50px"
                        bg="gray.700"
                        borderColor="gray.600"
                        borderWidth="1px"
                        color="gray.300"
                        fontSize="lg"
                        fontWeight="bold"
                        position="relative"
                        _hover={{
                          bg: "gray.600",
                          borderColor: "green.500",
                          transform: "scale(1.05)",
                        }}
                        _active={{
                          bg: "gray.500",
                          transform: "scale(0.95)",
                        }}
                        transition="all 0.15s"
                        disabled={pinPositions.length >= 9}
                      >
                        <Icon as={FaCircle} boxSize={3} />
                      </Button>
                    ))}
                  </SimpleGrid>
                  <Text fontSize="10px" color="gray.600" textAlign="center" mt={1}>
                    Numbers shown match your device display
                  </Text>
                </Box>

                {/* Action buttons */}
                <HStack gap={2} w="full" pt={2}>
                  <Button 
                    onClick={handleBackspace}
                    variant="outline"
                    size="sm"
                    borderColor="gray.600"
                    color="gray.300"
                    _hover={{
                      bg: "gray.700",
                      borderColor: "gray.500",
                    }}
                    disabled={pinPositions.length === 0}
                    flex={1}
                  >
                    <Icon as={FaBackspace} mr={1} />
                    Back
                  </Button>
                  <Button 
                    onClick={handleClearPin}
                    variant="outline"
                    size="sm"
                    borderColor="gray.600"
                    color="gray.300"
                    _hover={{
                      bg: "gray.700",
                      borderColor: "gray.500",
                    }}
                    disabled={pinPositions.length === 0}
                    flex={1}
                  >
                    Clear
                  </Button>
                  <Button
                    onClick={handleSubmitPin}
                    colorScheme="green"
                    size="sm"
                    _hover={{
                      transform: "translateY(-1px)",
                      boxShadow: "lg",
                    }}
                    transition="all 0.2s"
                    disabled={pinPositions.length === 0}
                    flex={2}
                  >
                    Unlock
                  </Button>
                </HStack>

                <Text fontSize="xs" color="gray.500" textAlign="center">
                  Use the scrambled layout shown on your device
                </Text>
              </VStack>
            )}

            {step === 'submitting' && (
              <VStack gap={3} py={4}>
                <Spinner size="lg" color="blue.400" />
                <Text fontSize="md" fontWeight="semibold">
                  Verifying PIN...
                </Text>
                <Text fontSize="sm" color="gray.400">
                  Please wait
                </Text>
              </VStack>
            )}

            {step === 'success' && (
              <VStack gap={3} py={4}>
                <Icon as={FaCheckCircle} color="green.400" boxSize={10} />
                <Text fontSize="md" fontWeight="semibold" color="green.400">
                  Device Unlocked!
                </Text>
              </VStack>
            )}

            {/* Error display */}
            {error && step !== 'success' && (
              <Box bg="red.900" borderColor="red.700" border="1px solid" p={3} borderRadius="md" w="full">
                <HStack gap={2} align="start">
                  <Icon as={FaExclamationTriangle} color="red.400" mt={0.5} boxSize={4} />
                  <VStack align="start" gap={1} flex={1}>
                    <Text fontSize="sm" color="red.100">{error}</Text>
                    <Button 
                      size="xs" 
                      colorScheme="red" 
                      variant="outline" 
                      onClick={handleRetry}
                    >
                      <Icon as={FaSync} mr={1} />
                      {retryCount >= 2 ? 'Full Retry' : 'Try Again'}
                    </Button>
                  </VStack>
                </HStack>
              </Box>
            )}

          </VStack>
        </Box>
      </Box>
    </Box>
  )
} 