import { VStack, Text, Button, Box, Icon, HStack } from '@chakra-ui/react'
import { 
  DialogRoot,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogBody,
  DialogFooter
} from './ui/dialog'
import { FaShieldAlt, FaCheckCircle } from 'react-icons/fa'
import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import type { BootloaderCheck } from '../types/device'

interface BootloaderUpdateDialogProps {
  isOpen: boolean
  bootloaderCheck: BootloaderCheck
  deviceId: string
  onUpdateComplete?: () => void
}

export const BootloaderUpdateDialog = ({ 
  isOpen, 
  bootloaderCheck,
  deviceId,
  onUpdateComplete
}: BootloaderUpdateDialogProps) => {
  const [isUpdating, setIsUpdating] = useState(false)
  const [error, setError] = useState<string | null>(null)
  
  const handleUpdate = async () => {
    setIsUpdating(true)
    setError(null)
    
    try {
      // Call the Tauri command to update the bootloader
      const success = await invoke('update_device_bootloader', {
        deviceId,
        targetVersion: bootloaderCheck.latestVersion
      })
      
      if (success) {
        // Update completed successfully
        onUpdateComplete?.()
      }
    } catch (err) {
      console.error('Bootloader update failed:', err)
      setError(err?.toString() || 'Update failed')
      setIsUpdating(false)
    }
  }
  
  return (
    <DialogRoot 
      open={isOpen} 
      onOpenChange={() => {
        // Dialog cannot be closed during update
        if (!isUpdating) {
          // You might want to handle close here if appropriate
        }
      }}
    >
      <DialogContent 
        maxW="md" 
        bg="gray.900" 
        color="white"
        borderColor="blue.500"
        borderWidth="2px"
      >
        <DialogHeader borderBottomWidth="1px" borderColor="gray.700" pb={4}>
          <DialogTitle color="white" display="flex" alignItems="center" gap={2}>
            <Icon as={FaShieldAlt} color="blue.400" />
            Bootloader Update Available
          </DialogTitle>
        </DialogHeader>
        
        <DialogBody py={6}>
          <VStack align="stretch" gap={6}>
            <Box 
              bg="blue.900"
              borderRadius="md"
              borderWidth="1px"
              borderColor="blue.600"
              p={4}
            >
              <HStack gap={3} align="start">
                <Icon as={FaCheckCircle} color="blue.400" mt={1} />
                <Box>
                  <Text fontWeight="bold">Security Enhancement Available</Text>
                  <Text fontSize="sm" mt={1}>
                    A new bootloader version is available to enhance your device security and performance.
                  </Text>
                </Box>
              </HStack>
            </Box>
            
            <VStack align="stretch" gap={3}>
              <Box>
                <Text fontSize="sm" color="gray.400">Current Version</Text>
                <Text fontSize="lg" fontWeight="semibold">{bootloaderCheck.currentVersion}</Text>
              </Box>
              
              <Box>
                <Text fontSize="sm" color="gray.400">New Version</Text>
                <Text fontSize="lg" fontWeight="semibold" color="green.400">
                  {bootloaderCheck.latestVersion}
                </Text>
              </Box>
              
              <Box>
                <Text fontSize="sm" color="gray.400">Estimated Time</Text>
                <Text>2-3 minutes</Text>
              </Box>
            </VStack>
            
            <VStack align="stretch" gap={2} pt={2}>
              <Text fontSize="sm" fontWeight="bold" color="blue.400">
                Update Information:
              </Text>
              <HStack align="start">
                <Icon as={FaCheckCircle} color="green.400" boxSize={3} mt={1} />
                <Text fontSize="sm" color="gray.300">
                  Your device will remain connected during the update
                </Text>
              </HStack>
              <HStack align="start">
                <Icon as={FaCheckCircle} color="green.400" boxSize={3} mt={1} />
                <Text fontSize="sm" color="gray.300">
                  The device will restart automatically when complete
                </Text>
              </HStack>
              <HStack align="start">
                <Icon as={FaCheckCircle} color="green.400" boxSize={3} mt={1} />
                <Text fontSize="sm" color="gray.300">
                  All your keys and settings will be preserved
                </Text>
              </HStack>
            </VStack>
            
            {error && (
              <Box 
                bg="red.900"
                borderRadius="md"
                borderWidth="1px"
                borderColor="red.600"
                p={3}
              >
                <Text fontSize="sm" color="red.200">{error}</Text>
              </Box>
            )}
          </VStack>
        </DialogBody>
        
        <DialogFooter borderTopWidth="1px" borderColor="gray.700" pt={4}>
          <Button
            colorScheme="blue"
            onClick={handleUpdate}
            loading={isUpdating}
            loadingText="Updating Bootloader..."
            width="full"
            size="lg"
            disabled={isUpdating}
          >
            Update Bootloader Now
          </Button>
        </DialogFooter>
      </DialogContent>
    </DialogRoot>
  )
} 