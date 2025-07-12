import React from 'react';
import { Box, Button, Text, VStack, HStack, Icon, Flex } from '@chakra-ui/react';
import { FaCheckCircle, FaExclamationTriangle } from 'react-icons/fa';
import { StepProps } from '../FirmwareUpdateWizard';

export const Step2Completion: React.FC<StepProps> = ({
  currentVersion,
  targetVersion,
  onNext,
  errorInfo,
}) => {
  const isSuccess = !errorInfo;

  return (
    <VStack>
      {isSuccess ? (
        // Success state
        <>
          <Flex justifyContent="center" mb={2}>
            <Icon as={FaCheckCircle} boxSize={12} color="green.500" />
          </Flex>
          
          <Text fontSize="xl" fontWeight="bold" textAlign="center">
            Firmware Update Successful
          </Text>
          
          <Box bg="green.50" p={4} borderRadius="md">
            <Text>
              Your device firmware has been successfully updated from version {currentVersion} to {targetVersion}.
            </Text>
          </Box>
          
          <Box>
            <Text fontSize="sm">
              Your KeepKey is ready to use with all the latest features and security improvements.
              The device will reconnect automatically within a few seconds.
            </Text>
          </Box>
        </>
      ) : (
        // Error state
        <>
          <Flex justifyContent="center" mb={2}>
            <Icon as={FaExclamationTriangle} boxSize={12} color="red.500" />
          </Flex>
          
          <Text fontSize="xl" fontWeight="bold" textAlign="center">
            Firmware Update Failed
          </Text>
          
          <Box bg="red.50" p={4} borderRadius="md" borderLeft="4px solid" borderColor="red.500">
            <Text fontWeight="medium" color="red.700">{errorInfo?.message || 'An unknown error occurred'}</Text>
            {errorInfo?.advice && <Text fontSize="sm" color="red.600" mt={1}>{errorInfo.advice}</Text>}
          </Box>
          
          <Box>
            <Text fontSize="sm">
              Please try reconnecting your device and attempt the update again. If the problem persists,
              contact support for assistance.
            </Text>
          </Box>
        </>
      )}
      
      <HStack justifyContent="flex-end" pt={4}>
        <Button colorScheme={isSuccess ? "green" : "blue"} onClick={onNext}>
          {isSuccess ? "Finish" : "Close"}
        </Button>
      </HStack>
    </VStack>
  );
};

export default Step2Completion;
