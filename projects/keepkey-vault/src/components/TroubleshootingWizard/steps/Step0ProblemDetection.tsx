import React from 'react';
import { VStack, Text, Button, Box, HStack, Icon, Code } from '@chakra-ui/react';
import { FaExclamationTriangle, FaUsb, FaInfoCircle } from 'react-icons/fa';
import type { StepProps } from '../TroubleshootingWizard';

export const Step0ProblemDetection: React.FC<StepProps> = ({
  deviceId,
  errorDetails,
  onNext,
}) => {
  return (
    <VStack align="start" gap={6}>
      {/* Problem Description */}
      <Box bg="yellow.100" p={4} borderRadius="md" borderLeft="4px solid" borderColor="yellow.500" w="full">
        <HStack mb={2}>
          <Icon as={FaExclamationTriangle} color="yellow.600" boxSize={5} />
          <Text fontWeight="bold" color="yellow.800">Communication Issue Detected</Text>
        </HStack>
        <Text fontSize="sm" color="yellow.700">
          Your KeepKey has been detected but we're having trouble communicating with it. Don't worry - this is usually fixable!
        </Text>
      </Box>

      {/* Device Information */}
      <Box bg="gray.100" p={4} borderRadius="md" w="full">
        <HStack mb={3}>
          <Icon as={FaUsb} color="gray.600" boxSize={4} />
          <Text fontWeight="medium" color="gray.700">Device Information</Text>
        </HStack>
        <VStack align="start" gap={2}>
          <HStack>
            <Text fontSize="sm" color="gray.600" minW="80px">Device ID:</Text>
            <Code fontSize="xs" bg="gray.200" color="gray.800">{deviceId}</Code>
          </HStack>
          <HStack align="start">
            <Text fontSize="sm" color="gray.600" minW="80px">Error:</Text>
            <Text fontSize="sm" color="gray.800">{errorDetails}</Text>
          </HStack>
        </VStack>
      </Box>

      {/* What This Wizard Will Do */}
      <Box bg="blue.50" p={4} borderRadius="md" w="full">
        <HStack mb={3}>
          <Icon as={FaInfoCircle} color="blue.500" boxSize={4} />
          <Text fontWeight="medium" color="blue.700">What This Wizard Will Do</Text>
        </HStack>
        <VStack align="start" gap={2}>
          <Text fontSize="sm" color="blue.700">
            • Guide you through basic troubleshooting steps
          </Text>
          <Text fontSize="sm" color="blue.700">
            • Help you check cables, ports, and connections
          </Text>
          <Text fontSize="sm" color="blue.700">
            • Provide advanced recovery options if needed
          </Text>
          <Text fontSize="sm" color="blue.700">
            • Connect you with support if the issue persists
          </Text>
        </VStack>
      </Box>

      {/* Next Steps */}
      <Box w="full" pt={4}>
        <Text fontSize="sm" color="gray.600" mb={4}>
          Most communication issues are resolved within the first few steps. Let's get your KeepKey working again!
        </Text>
        
        <HStack justify="flex-end">
          <Button colorScheme="yellow" onClick={onNext} size="lg">
            Start Troubleshooting
          </Button>
        </HStack>
      </Box>
    </VStack>
  );
}; 