import React from 'react';
import { VStack, Text, Button, Box, Icon, HStack } from '@chakra-ui/react';
import { FaCheckCircle, FaTimesCircle, FaRedo } from 'react-icons/fa';
import type { StepProps } from '../BootloaderUpdateWizard';

export const Step2Completion: React.FC<StepProps> = ({ 
  deviceId, 
  onNext, // This will effectively be 'close' or 'finish'
  onPrevious, // To go back and retry the update step
  errorInfo
}) => {
  const isSuccess = !errorInfo;

  const handleRetry = () => {
    // This should ideally reset the state of the update process
    // and navigate back to the 'in-progress' step.
    if (onPrevious) {
      onPrevious(); 
    }
  };

  return (
    <VStack align="center" justify="center" gap={5} h="100%">
      {isSuccess ? (
        <Icon as={FaCheckCircle} boxSize={16} color="green.400" />
      ) : (
        <Icon as={FaTimesCircle} boxSize={16} color="red.400" />
      )}
      
      <VStack align="center">
        <Text fontSize="xl" fontWeight="semibold">
          {isSuccess ? 'Bootloader Update Successful!' : 'Bootloader Update Failed'}
        </Text>
        {isSuccess ? (
          <Text fontSize="sm" color="gray.300" textAlign="center">
            Your KeepKey's bootloader has been updated. The device will now restart.
            Once restarted, it will be ready for use with the latest security features.
          </Text>
        ) : (
          <Box textAlign="center" bg="red.900" p={3} borderRadius="md" borderWidth="1px" borderColor="red.700">
            <Text fontSize="sm" color="red.200" fontWeight="bold">
              Error: {errorInfo?.message || 'An unknown error occurred.'}
            </Text>
            {errorInfo?.advice && (
              <Text fontSize="xs" color="red.300" mt={1}>
                {errorInfo.advice}
              </Text>
            )}
          </Box>
        )}
      </VStack>

      {isSuccess ? (
        <Button colorScheme="green" onClick={onNext} size="lg" width="full">
          Finish
        </Button>
      ) : (
        <HStack width="full" gap={4} mt={4}>
          <Button 
            colorScheme="orange" 
            onClick={handleRetry} 
            // leftIcon={<FaRedo />} // Chakra UI v3 - use children with icon instead 
            variant="outline"
            flex={1}
          >
            Try Again
          </Button>
          <Button colorScheme="gray" onClick={onNext} flex={1}> {/* onNext here means close wizard */}
            Close
          </Button>
        </HStack>
      )}
      
      <Text fontSize="xs" color="gray.500" mt={2}>
        Device ID: {deviceId}
      </Text>
    </VStack>
  );
};
