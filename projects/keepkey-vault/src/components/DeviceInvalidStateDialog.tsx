import { 
  DialogRoot,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogBody,
  DialogFooter
} from "./ui/dialog";
import { Button, VStack, Text, Icon, Box } from '@chakra-ui/react';
import { FaExclamationTriangle, FaPlug } from 'react-icons/fa';
import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';

interface DeviceInvalidStateDialogProps {
  deviceId: string;
  error?: string;
  onClose: () => void;
}

export const DeviceInvalidStateDialog = ({
  deviceId,
  error,
  onClose
}: DeviceInvalidStateDialogProps) => {
  
  // Auto-close when this specific device is disconnected
  useEffect(() => {
    console.log(`🔌 Setting up disconnect listener for device: ${deviceId}`);
    
    const setupListener = async () => {
      const unlisten = await listen<string>('device:disconnected', (event) => {
        const disconnectedDeviceId = event.payload;
        console.log(`🔌 Device disconnected: ${disconnectedDeviceId}, our device: ${deviceId}`);
        
        // Check if it's our device that was disconnected
        if (disconnectedDeviceId === deviceId) {
          console.log('🔌 Our device was disconnected, auto-closing dialog');
          onClose();
        }
      });
      
      return unlisten;
    };
    
    const unlistenPromise = setupListener();
    
    // Cleanup listener on unmount
    return () => {
      unlistenPromise.then(unlisten => unlisten());
    };
  }, [deviceId, onClose]);
  
  return (
    <DialogRoot 
      open={true} 
      onOpenChange={({ open }) => !open && onClose()}
      placement="center"
      modal
    >
      <DialogContent
        bg="gray.800"
        borderColor="orange.600"
        borderWidth="2px"
        boxShadow="0 0 30px rgba(255, 152, 0, 0.3)"
        maxW="450px"
        borderRadius="lg"
      >
        <DialogHeader borderBottomWidth="1px" borderColor="gray.700" pb={3}>
          <DialogTitle color="white" fontSize="lg" display="flex" alignItems="center" gap={2}>
            <Icon as={FaExclamationTriangle} color="orange.400" />
            Device in Invalid State
          </DialogTitle>
        </DialogHeader>

        <DialogBody py={6}>
          <VStack gap={4} align="stretch">
            {/* Main message */}
            <Box bg="gray.750" p={4} borderRadius="md" borderWidth="1px" borderColor="gray.600">
              <VStack gap={3} align="start">
                <Text color="gray.100" fontSize="md" fontWeight="semibold">
                  Your KeepKey is not responding properly.
                </Text>
                <Text color="gray.300" fontSize="sm">
                  This can happen when the device is in an unexpected state or if there was a communication error.
                </Text>
              </VStack>
            </Box>

            {/* Instructions */}
            <Box bg="blue.900/30" p={4} borderRadius="md" borderWidth="1px" borderColor="blue.600/50">
              <VStack gap={3} align="start">
                <Text color="blue.200" fontSize="sm" fontWeight="semibold" display="flex" alignItems="center" gap={2}>
                  <Icon as={FaPlug} />
                  Please follow these steps:
                </Text>
                <VStack gap={2} align="start" pl={4}>
                  <Text color="gray.300" fontSize="sm">1. Unplug your KeepKey from the USB port</Text>
                  <Text color="gray.300" fontSize="sm">2. Wait 2-3 seconds</Text>
                  <Text color="gray.300" fontSize="sm">3. Reconnect your KeepKey normally</Text>
                  <Text color="orange.300" fontSize="sm" fontWeight="semibold">
                    ⚠️ Do NOT hold any buttons when reconnecting
                  </Text>
                </VStack>
              </VStack>
            </Box>

            {/* Error details if provided */}
            {error && (
              <Box bg="red.900/20" p={3} borderRadius="md" borderWidth="1px" borderColor="red.600/30">
                <Text color="red.300" fontSize="xs" fontFamily="mono">
                  Error: {error}
                </Text>
              </Box>
            )}

            {/* Additional help */}
            <Text color="gray.400" fontSize="xs" textAlign="center" mt={2}>
              If the problem persists after reconnecting, please close any other wallet applications 
              that might be using your KeepKey.
            </Text>
          </VStack>
        </DialogBody>

        <DialogFooter borderTopWidth="1px" borderColor="gray.700" pt={3}>
          <Button 
            colorScheme="blue" 
            onClick={onClose}
            size="md"
            w="full"
          >
            I've Reconnected My Device
          </Button>
        </DialogFooter>
      </DialogContent>
    </DialogRoot>
  );
}; 