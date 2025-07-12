import React from 'react';
import { VStack, Text, Button, Box, Icon, HStack } from '@chakra-ui/react';
import { FaExclamationTriangle, FaShieldAlt } from 'react-icons/fa';
import type { StepProps } from '../BootloaderUpdateWizard';

export const Step0Warning: React.FC<StepProps> = ({ 
  deviceId, 
  currentVersion, 
  requiredVersion, 
  onNext,
  clearError
}) => {
  React.useEffect(() => {
    clearError(); // Clear any errors from previous wizard attempts when this step loads
  }, [clearError]);

  return (
    <VStack align="stretch" gap={5}>
      <Box 
        bg="orange.900"
        borderRadius="md"
        borderWidth="1px"
        borderColor="orange.700" // Darker border
        p={4}
      >
        <HStack gap={3} align="center">
          <Icon as={FaExclamationTriangle} color="orange.400" boxSize={6} />
          <Box>
            <Text fontWeight="bold" fontSize="md">Mandatory Security Update</Text>
            <Text fontSize="xs" color="gray.300" mt={1}>
              Your KeepKey's bootloader (v{currentVersion}) is outdated. Update to v{requiredVersion} for critical security and performance improvements.
            </Text>
          </Box>
        </HStack>
      </Box>
      
      <VStack align="stretch" gap={2} bg="gray.750" p={3} borderRadius="md">
        <HStack justify="space-between">
          <Text fontSize="xs" color="gray.400">Device ID:</Text>
          <Text fontSize="xs" fontWeight="semibold" fontFamily="monospace">{deviceId}</Text>
        </HStack>
        <HStack justify="space-between">
          <Text fontSize="xs" color="gray.400">Current Bootloader:</Text>
          <Text fontSize="xs" fontWeight="semibold" color="red.400">v{currentVersion}</Text>
        </HStack>
        <HStack justify="space-between">
          <Text fontSize="xs" color="gray.400">Required Bootloader:</Text>
          <Text fontSize="xs" fontWeight="semibold" color="green.400">v{requiredVersion}</Text>
        </HStack>
      </VStack>
      
      <VStack align="stretch" gap={1} pt={1}>
        <Text fontSize="sm" fontWeight="medium" color="orange.300">
          Important Notes:
        </Text>
        <Text fontSize="xs" color="gray.300">• This update is critical for device security.</Text>
        <Text fontSize="xs" color="gray.300">• The process will take approximately 1-2 minutes.</Text>
        <Text fontSize="xs" color="gray.300">• Ensure your device remains connected throughout the update.</Text>
        <Text fontSize="xs" color="gray.300">• Your KeepKey will restart automatically after completion.</Text>
      </VStack>

      <Button
        colorScheme="orange"
        onClick={onNext}
        width="full"
        size="md"
        mt={3}
        // leftIcon={<FaShieldAlt />} // Chakra UI v3 - use children with icon instead
      >
        Start Bootloader Update
      </Button>
    </VStack>
  );
};
