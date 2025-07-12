import { VStack, Text, Button, Box, Icon, HStack, Badge } from '@chakra-ui/react'
import { 
  DialogRoot,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogBody,
  DialogFooter,
  DialogCloseTrigger
} from './ui/dialog'
import { FaDownload, FaShieldAlt } from 'react-icons/fa'
import { useState } from 'react'
import type { FirmwareCheck } from '../types/device'

interface FirmwareUpdateDialogProps {
  isOpen: boolean
  firmwareCheck: FirmwareCheck
  onUpdateStart: () => void
  onSkip: () => void
  onRemindLater: () => void
  onClose: () => void
  isLoading?: boolean
}

export const FirmwareUpdateDialog = ({ 
  isOpen, 
  firmwareCheck,
  onUpdateStart,
  onSkip,
  onRemindLater,
  onClose,
  isLoading = false
}: FirmwareUpdateDialogProps) => {
  const [isUpdating, setIsUpdating] = useState(false)
  
  const handleUpdate = () => {
    setIsUpdating(true)
    onUpdateStart()
  }
  
  return (
    <DialogRoot open={isOpen} onOpenChange={({ open }) => !open && onClose()}>
      <DialogContent 
        maxW="lg" 
        bg="gray.900" 
        color="white"
        borderColor="blue.600"
      >
        <DialogHeader borderBottomWidth="1px" borderColor="gray.700" pb={4}>
          <DialogTitle color="white" display="flex" alignItems="center" gap={2}>
            <Icon as={FaDownload} color="blue.400" />
            Firmware Update Available
            {firmwareCheck.needsUpdate && (
              <Badge colorScheme="orange" ml={2}>Update Available</Badge>
            )}
          </DialogTitle>
          <DialogCloseTrigger color="gray.400" _hover={{ color: "white" }} />
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
                <Icon as={FaShieldAlt} color="blue.400" mt={1} />
                <Box>
                  <Text fontWeight="bold">New Firmware Available</Text>
                  <Text fontSize="sm" mt={1}>
                    Update your KeepKey to get the latest features and security improvements.
                  </Text>
                </Box>
              </HStack>
            </Box>
            
            <VStack align="stretch" gap={3}>
              <HStack justify="space-between">
                <Box>
                  <Text fontSize="sm" color="gray.400">Current Version</Text>
                  <Text fontSize="lg" fontWeight="semibold">{firmwareCheck.currentVersion}</Text>
                </Box>
                <Icon as={FaDownload} color="gray.600" boxSize={6} />
                <Box>
                  <Text fontSize="sm" color="gray.400">New Version</Text>
                  <Text fontSize="lg" fontWeight="semibold" color="green.400">
                    {firmwareCheck.latestVersion}
                  </Text>
                </Box>
              </HStack>
              
              <Box>
                <Text fontSize="sm" color="gray.400">Estimated Time</Text>
                <Text>3-5 minutes</Text>
              </Box>
            </VStack>
            
            <VStack align="stretch" gap={2} pt={2}>
              <Text fontSize="sm" fontWeight="bold" color="yellow.400">
                Important:
              </Text>
              <Text fontSize="sm" color="gray.300">
                • Do not disconnect your device during the update
              </Text>
              <Text fontSize="sm" color="gray.300">
                • You may need to re-enter your PIN after the update
              </Text>
              <Text fontSize="sm" color="gray.300">
                • Your funds and settings will remain safe
              </Text>
            </VStack>
          </VStack>
        </DialogBody>
        
        <DialogFooter borderTopWidth="1px" borderColor="gray.700" pt={4}>
          <HStack width="full" gap={3}>
            <Button
              variant="outline"
              colorScheme="gray"
              onClick={onSkip}
              flex={1}
              disabled={isUpdating || isLoading}
            >
              Skip
            </Button>
            <Button
              variant="outline"
              colorScheme="blue"
              onClick={onRemindLater}
              flex={1}
              disabled={isUpdating || isLoading}
            >
              Remind Later
            </Button>
            <Button
              colorScheme="blue"
              onClick={handleUpdate}
              loading={isUpdating || isLoading}
              loadingText="Updating..."
              flex={2}
            >
              Update Now
            </Button>
          </HStack>
        </DialogFooter>
      </DialogContent>
    </DialogRoot>
  )
} 