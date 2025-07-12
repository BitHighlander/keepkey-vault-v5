import { 
  DialogRoot, 
  DialogContent, 
  DialogHeader, 
  DialogTitle, 
  DialogCloseTrigger,
  DialogBody
} from '@chakra-ui/react'
import { VStack, Text, Box, Button, HStack } from '@chakra-ui/react'
import { FaTimes, FaExclamationTriangle } from 'react-icons/fa'
import type { BootloaderCheck } from '../types/device'

// Import the SVG image for the hold-and-connect instructions
import holdAndConnectSvg from '../assets/svg/hold-and-connect.svg'

interface EnterBootloaderModeDialogProps {
  isOpen: boolean
  bootloaderCheck: BootloaderCheck
  deviceId: string  
  onClose: () => void
}

export const EnterBootloaderModeDialog = ({ 
  isOpen, 
  bootloaderCheck, 
  deviceId,
  onClose 
}: EnterBootloaderModeDialogProps) => {
  return (
    <DialogRoot open={isOpen} onOpenChange={({ open }) => !open && onClose()}>
      <DialogContent 
        maxW="sm" 
        bg="gray.900" 
        color="white" 
        borderColor="yellow.500" 
        borderWidth="2px"
        borderRadius="md"
      >
        <DialogHeader borderBottomWidth="1px" borderColor="gray.700" pb={3}>
          <HStack gap={2}>
            <FaExclamationTriangle color="yellow" />
            <DialogTitle color="white" fontSize="lg">Enter Bootloader Mode</DialogTitle>
          </HStack>
          <DialogCloseTrigger color="gray.400" _hover={{ color: "white" }}>
            <FaTimes />
          </DialogCloseTrigger>
        </DialogHeader>
        
        <DialogBody py={4}>
          <VStack align="stretch" gap={3}>
            <Text fontSize="sm" color="gray.300" textAlign="center">
              Bootloader update available: v{bootloaderCheck.currentVersion} → v{bootloaderCheck.latestVersion}
            </Text>
            
            <Text fontSize="sm" color="gray.300" textAlign="center">
              To update, your device must be in <Text as="span" fontWeight="bold" color="yellow.300">Bootloader Mode</Text>
            </Text>

            <Box display="flex" justifyContent="center" py={4}>
              <img
                  src={holdAndConnectSvg}
                  alt="Hold button while connecting device"
                  style={{ maxWidth: '240px', height: 'auto' }}
              />
            </Box>

            <VStack align="stretch" gap={1} bg="gray.800" p={3} borderRadius="md" fontSize="sm">
              <Text fontWeight="semibold" color="yellow.300">Quick Steps:</Text>
              <Text>1. Unplug device</Text>
              <Text>2. Hold button + plug in</Text>
              <Text>3. Keep holding until "BOOTLOADER MODE" appears</Text>
            </VStack>
            
            <Text fontSize="xs" color="blue.300" textAlign="center">
              Update dialog will appear automatically once in bootloader mode
            </Text>
          </VStack>
        </DialogBody>
        
        <Box borderTopWidth="1px" borderColor="gray.700" pt={3}>
          <Button 
            colorScheme="yellow" 
            onClick={onClose} 
            width="full"
            size="sm"
          >
            Got It
          </Button>
        </Box>
      </DialogContent>
    </DialogRoot>
  )
} 