import React, { useState, useCallback } from 'react';
import { 
  VStack, 
  Text, 
  Button, 
  SimpleGrid, 
  Box, 
  HStack, 
  Heading,
  Icon
} from '@chakra-ui/react';
import { FaCircle } from 'react-icons/fa';
import { invoke } from '@tauri-apps/api/core';
import { PIN_MATRIX_LAYOUT, PinPosition } from '../../types/pin';

interface VerificationPinProps {
  sessionId: string;
  deviceLabel: string;
  onComplete: () => void;
  onCancel: () => void;
}

const VerificationPin: React.FC<VerificationPinProps> = ({
  sessionId,
  deviceLabel,
  onComplete,
  onCancel
}) => {
  const [selectedPositions, setSelectedPositions] = useState<number[]>([]);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleNumberClick = useCallback((position: number) => {
    if (selectedPositions.length < 9) {
      setSelectedPositions(prev => [...prev, position]);
    }
  }, [selectedPositions.length]);

  const handleBackspace = useCallback(() => {
    setSelectedPositions(prev => prev.slice(0, -1));
  }, []);

  const handleClear = useCallback(() => {
    setSelectedPositions([]);
  }, []);

  const handleSubmit = useCallback(async () => {
    if (selectedPositions.length === 0) {
      setError("Please enter your PIN");
      return;
    }

    setIsSubmitting(true);
    setError(null);

    try {
      console.log('Sending PIN verification with sessionId:', sessionId, 'positions:', selectedPositions);
      
      const success = await invoke<boolean>('send_verification_pin', {
        sessionId: sessionId,
        positions: selectedPositions
      });

      if (success) {
        console.log('PIN verified successfully');
        onComplete();
      } else {
        setError("Invalid PIN. Please try again.");
        setSelectedPositions([]);
      }
    } catch (err) {
      console.error('Failed to verify PIN:', err);
      const errorMsg = err as string;
      
      // Check if it's a device failure with specific message
      if (errorMsg.includes('Invalid PIN')) {
        setError("Incorrect PIN. Please check your device screen and try again.");
      } else if (errorMsg.includes('PIN')) {
        setError(`PIN verification failed: ${errorMsg}`);
      } else {
        setError(`Verification failed: ${errorMsg}`);
      }
      
      setSelectedPositions([]);
    } finally {
      setIsSubmitting(false);
    }
  }, [sessionId, selectedPositions, onComplete]);

  const renderPinDisplay = () => {
    return (
      <HStack gap={2} justify="center">
        {Array.from({ length: 9 }, (_, i) => (
          <Box
            key={i}
            w="12px"
            h="12px"
            borderRadius="full"
            bg={i < selectedPositions.length ? "blue.400" : "gray.600"}
            transition="background-color 0.2s"
          />
        ))}
      </HStack>
    );
  };

  const renderPinGrid = () => {
    return (
      <SimpleGrid columns={3} gap={3} w="full" maxW="300px" mx="auto">
        {PIN_MATRIX_LAYOUT.map((position) => (
          <Button
            key={position}
            size="lg"
            variant="outline"
            height="60px"
            onClick={() => handleNumberClick(position as PinPosition)}
            disabled={isSubmitting}
            borderColor="gray.500"
            color="gray.300"
            _hover={{ bg: 'gray.600', borderColor: 'blue.400' }}
            _active={{ bg: 'gray.500' }}
          >
            <Icon as={FaCircle} boxSize={4} />
          </Button>
        ))}
      </SimpleGrid>
    );
  };

  return (
    <VStack gap={6} align="center" w="full">
      <VStack gap={2} align="center">
        <Heading size="md" color="white" textAlign="center">
          Enter your PIN
        </Heading>
        <Text color="gray.400" textAlign="center" fontSize="sm">
          Use the keypad below. The numbers are scrambled on your device for security.
        </Text>
      </VStack>

      {/* PIN dots display */}
      <VStack gap={4} align="center">
        <Text color="gray.300" fontSize="sm">
          PIN Length: {selectedPositions.length}/9
        </Text>
        {renderPinDisplay()}
      </VStack>

      {/* PIN keypad with correct layout */}
      <VStack gap={4} align="center">
        <Text color="gray.400" fontSize="xs" textAlign="center">
          Click the positions that match the scrambled numbers on your device
        </Text>
        {renderPinGrid()}
      </VStack>

      {error && (
        <Text color="red.400" fontSize="sm" textAlign="center">
          {error}
        </Text>
      )}

      {/* Action buttons */}
      <HStack gap={4} w="full" maxW="400px">
        <Button
          variant="outline"
          onClick={handleBackspace}
          disabled={isSubmitting || selectedPositions.length === 0}
          size="md"
          flex={1}
        >
          Backspace
        </Button>
        
        <Button
          variant="outline"
          onClick={handleClear}
          disabled={isSubmitting || selectedPositions.length === 0}
          size="md"
          flex={1}
        >
          Clear
        </Button>
        
        <Button
          colorScheme="blue"
          onClick={handleSubmit}
          disabled={isSubmitting || selectedPositions.length === 0}
          size="md"
          flex={2}
        >
          {isSubmitting ? 'Verifying...' : 'Enter PIN'}
        </Button>
      </HStack>

      {/* Instructions */}
      <Box p={4} bg="blue.900" borderRadius="md" borderWidth={1} borderColor="blue.600" maxW="500px">
        <Text fontSize="sm" color="blue.200" fontWeight="bold" mb={2}>
          How to enter your PIN:
        </Text>
        <Text fontSize="sm" color="blue.200" mb={1}>
          1. Look at your {deviceLabel} screen - it shows numbers 1-9 in scrambled positions
        </Text>
        <Text fontSize="sm" color="blue.200" mb={1}>
          2. For each digit of your PIN, click the circle above that matches the position on your device
        </Text>
        <Text fontSize="sm" color="blue.200" mb={2}>
          3. The 3×3 grid layout matches your device screen exactly
        </Text>
        <Text fontSize="sm" color="blue.200" fontWeight="bold">
          Example: If your PIN digit appears in the top-right on your device, click the top-right circle above.
        </Text>
      </Box>
    </VStack>
  );
};

export default VerificationPin; 