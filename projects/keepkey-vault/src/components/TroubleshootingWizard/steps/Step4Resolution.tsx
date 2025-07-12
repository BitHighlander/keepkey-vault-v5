import React from 'react';
import { VStack, Text, Button, Box, HStack, Icon } from '@chakra-ui/react';
import { FaCheckCircle, FaExclamationTriangle, FaRedo, FaEnvelope } from 'react-icons/fa';
import type { StepProps } from '../TroubleshootingWizard';

export const Step4Resolution: React.FC<StepProps> = ({
  deviceId,
  errorDetails,
  onNext,
  onPrevious,
  onContactSupport,
  errorInfo,
}) => {
  const isResolved = !errorInfo; // If no error info, we assume it was resolved

  return (
    <VStack align="start" gap={6}>
      {isResolved ? (
        // Success State
        <>
          <Box bg="green.50" p={6} borderRadius="md" w="full" borderLeft="4px solid" borderLeftColor="green.400">
            <VStack align="center" gap={4}>
              <Icon as={FaCheckCircle} color="green.500" boxSize={12} />
              <Text fontSize="xl" fontWeight="bold" color="green.700" textAlign="center">
                Communication Restored!
              </Text>
              <Text fontSize="sm" color="green.600" textAlign="center">
                Your KeepKey is now communicating properly with the application. You can use your device normally.
              </Text>
            </VStack>
          </Box>

          <Box bg="blue.50" p={4} borderRadius="md" w="full">
            <Text fontSize="sm" fontWeight="medium" color="blue.700" mb={2}>
              What was fixed:
            </Text>
            <VStack align="start" gap={1}>
              <Text fontSize="sm" color="blue.600">• Device communication has been restored</Text>
              <Text fontSize="sm" color="blue.600">• Your KeepKey should now appear in the devices list</Text>
              <Text fontSize="sm" color="blue.600">• All normal functions should be available</Text>
            </VStack>
          </Box>

          <Box bg="yellow.50" p={4} borderRadius="md" w="full">
            <Text fontSize="sm" fontWeight="medium" color="yellow.700" mb={2}>
              To prevent future issues:
            </Text>
            <VStack align="start" gap={1}>
              <Text fontSize="sm" color="yellow.600">• Use high-quality USB data cables</Text>
              <Text fontSize="sm" color="yellow.600">• Connect directly to computer ports when possible</Text>
              <Text fontSize="sm" color="yellow.600">• Avoid USB hubs unless necessary</Text>
              <Text fontSize="sm" color="yellow.600">• Keep your KeepKey firmware updated</Text>
            </VStack>
          </Box>
        </>
      ) : (
        // Failure State
        <>
          <Box bg="red.50" p={6} borderRadius="md" w="full" borderLeft="4px solid" borderLeftColor="red.400">
            <VStack align="center" gap={4}>
              <Icon as={FaExclamationTriangle} color="red.500" boxSize={12} />
              <Text fontSize="xl" fontWeight="bold" color="red.700" textAlign="center">
                Communication Still Not Working
              </Text>
              <Text fontSize="sm" color="red.600" textAlign="center">
                We've tried all the standard troubleshooting steps, but your KeepKey still isn't communicating properly.
              </Text>
            </VStack>
          </Box>

          {errorInfo && (
            <Box bg="gray.50" p={4} borderRadius="md" w="full">
              <Text fontSize="sm" fontWeight="medium" color="gray.700" mb={2}>
                Current Issue:
              </Text>
              <Text fontSize="sm" color="gray.600">{errorInfo.message}</Text>
              {errorInfo.advice && (
                <Text fontSize="sm" color="gray.500" mt={2} fontStyle="italic">
                  {errorInfo.advice}
                </Text>
              )}
            </Box>
          )}

          <Box bg="orange.50" p={4} borderRadius="md" w="full">
            <Text fontSize="sm" fontWeight="medium" color="orange.700" mb={3}>
              Next Steps:
            </Text>
            <VStack align="start" gap={2}>
              <HStack align="start">
                <Text fontSize="sm" color="orange.500" minW="20px">1.</Text>
                <Text fontSize="sm" color="orange.600">
                  Contact KeepKey Support with your diagnostic information
                </Text>
              </HStack>
              <HStack align="start">
                <Text fontSize="sm" color="orange.500" minW="20px">2.</Text>
                <Text fontSize="sm" color="orange.600">
                  Consider trying on a different computer to isolate the issue
                </Text>
              </HStack>
              <HStack align="start">
                <Text fontSize="sm" color="orange.500" minW="20px">3.</Text>
                <Text fontSize="sm" color="orange.600">
                  If possible, test with different USB cables and ports
                </Text>
              </HStack>
            </VStack>
          </Box>

          <Box bg="blue.50" p={4} borderRadius="md" w="full">
            <Text fontSize="sm" fontWeight="medium" color="blue.700" mb={3}>
              Important Reminders:
            </Text>
            <VStack align="start" gap={1}>
              <Text fontSize="sm" color="blue.600">• Your funds are safe - this is only a communication issue</Text>
              <Text fontSize="sm" color="blue.600">• Your recovery phrase gives you access to your funds</Text>
              <Text fontSize="sm" color="blue.600">• Support can help resolve device communication problems</Text>
            </VStack>
          </Box>
        </>
      )}

      {/* Device Information */}
      <Box bg="gray.100" p={4} borderRadius="md" w="full">
        <Text fontSize="sm" fontWeight="medium" color="gray.700" mb={2}>
          Device Information:
        </Text>
        <VStack align="start" gap={1}>
          <HStack>
            <Text fontSize="sm" color="gray.600" minW="80px">Device ID:</Text>
            <Text fontSize="sm" color="gray.800" fontFamily="mono">{deviceId}</Text>
          </HStack>
          <HStack align="start">
            <Text fontSize="sm" color="gray.600" minW="80px">Status:</Text>
            <Text fontSize="sm" color={isResolved ? "green.600" : "red.600"}>
              {isResolved ? "Communication Restored" : "Communication Failed"}
            </Text>
          </HStack>
        </VStack>
      </Box>

      {/* Actions */}
      <HStack justify="space-between" w="full" pt={4}>
        {!isResolved && (
          <Button 
            variant="outline" 
            onClick={onPrevious}
          >
            Try Again
          </Button>
        )}
        
        <HStack gap={3}>
          {!isResolved && onContactSupport && (
            <Button 
              colorScheme="blue"
              onClick={() => onContactSupport({
                deviceId,
                errorDetails,
                status: 'communication_failed',
                steps_completed: 'all_troubleshooting_steps'
              })}
            >
              Contact Support
            </Button>
          )}
          
          <Button 
            colorScheme={isResolved ? "green" : "gray"}
            onClick={onNext}
          >
            {isResolved ? "Complete" : "Close"}
          </Button>
        </HStack>
      </HStack>
    </VStack>
  );
}; 