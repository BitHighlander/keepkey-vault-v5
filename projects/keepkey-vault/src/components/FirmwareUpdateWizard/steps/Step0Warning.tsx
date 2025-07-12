import React from 'react';
import { Box, Button, Text, VStack, HStack, Icon } from '@chakra-ui/react';
import { FaInfoCircle, FaCheckCircle } from 'react-icons/fa';
import { StepProps } from '../FirmwareUpdateWizard';

export const Step0Warning: React.FC<StepProps> = ({
  currentVersion,
  targetVersion,
  onNext,
}) => {
  return (
    <VStack gap={6} align="stretch" w="full">
      <Box bg="blue.900" p={4} borderRadius="md" border="1px solid" borderColor="blue.600">
        <HStack align="flex-start">
          <Icon as={FaInfoCircle} color="blue.400" boxSize={5} mt={0.5} />
          <Text fontWeight="medium" color="blue.100">
            Your device firmware can be updated from v{currentVersion} to v{targetVersion}
          </Text>
        </HStack>
      </Box>
      
      <Box>
        <Text fontWeight="medium" mb={4} color="gray.200">Before you begin:</Text>
        <VStack gap={3} align="stretch">
          <HStack align="flex-start">
            <Icon as={FaCheckCircle} color="green.400" mt={0.5} />
            <Text color="gray.300">Make sure your device is connected via USB and not being used by other applications</Text>
          </HStack>
          <HStack align="flex-start">
            <Icon as={FaCheckCircle} color="green.400" mt={0.5} />
            <Text color="gray.300">Ensure your device has sufficient battery if using a wireless connection</Text>
          </HStack>
          <HStack align="flex-start">
            <Icon as={FaCheckCircle} color="green.400" mt={0.5} />
            <Text color="gray.300">The update process will take approximately 2-3 minutes</Text>
          </HStack>
          <HStack align="flex-start">
            <Icon as={FaCheckCircle} color="green.400" mt={0.5} />
            <Text color="gray.300">Your device settings and keys will be preserved during the update</Text>
          </HStack>
        </VStack>
      </Box>
      
      <Box>
        <Text fontSize="sm" color="gray.400">
          This firmware update includes security improvements and new features.
          Updating is recommended to ensure optimal performance and security of your KeepKey.
        </Text>
      </Box>
      
      <HStack justifyContent="center" pt={4}>
        <Button colorScheme="blue" size="lg" onClick={onNext}>
          Begin Update
        </Button>
      </HStack>
    </VStack>
  );
};

export default Step0Warning;
