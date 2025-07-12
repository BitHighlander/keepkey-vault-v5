import {
  Button,
  Text,
  VStack,
  Input,
  Box,
  HStack,
  Heading,
} from "@chakra-ui/react";
import { useState, useCallback } from "react";

interface DeviceLabelProps {
  onComplete: (label: string) => void;
  onSkip: () => void;
  isLoading?: boolean;
}

// Sanitize label: max 12 chars, ASCII printable only
const sanitizeLabel = (input: string): string => {
  return input
    .replace(/[^\x20-\x7E]+/g, '') // Remove non-ASCII printable chars
    .substring(0, 12); // Limit to 12 characters
};

export function DeviceLabel({ onComplete, onSkip, isLoading = false }: DeviceLabelProps) {
  const [label, setLabel] = useState('');
  const [error, setError] = useState<string | null>(null);

  const handleLabelChange = useCallback((value: string) => {
    const sanitized = sanitizeLabel(value);
    setLabel(sanitized);
    setError(null);
  }, []);

  const handleSubmit = useCallback(async () => {
    // Require at least 2 characters for label
    if (label.trim().length < 2) {
      setError('Label must be at least 2 characters');
      return;
    }

    // Additional validation
    if (label.length > 12) {
      setError('Label must be 12 characters or less');
      return;
    }

    onComplete(label.trim());
  }, [label, onComplete]);

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      handleSubmit();
    }
  };

  return (
    <Box
      w="100%"
      maxW="500px"
      bg="gray.800"
      borderRadius="xl"
      boxShadow="xl"
      borderWidth="1px"
      borderColor="gray.700"
      overflow="hidden"
    >
        <Box bg="gray.850" p={6}>
          <Heading fontSize="2xl" fontWeight="bold" color="white" textAlign="center">
            Label Your KeepKey (Optional)
          </Heading>
        </Box>
        
        <Box p={6}>
          <VStack gap={6}>
            <Text 
              color="gray.400" 
              textAlign="center"
              fontSize="md"
              lineHeight="1.6"
            >
              Labelling each of your devices allows you to easily distinguish one from the other.
            </Text>
            
            <VStack gap={2} w="full">
              <Text color="gray.300" fontSize="sm" fontWeight="semibold" alignSelf="start">
                Device Label
              </Text>
              <Input
                value={label}
                onChange={(e) => handleLabelChange(e.target.value)}
                onKeyPress={handleKeyPress}
                placeholder="Enter device label"
                size="lg"
                bg="gray.700"
                borderColor="gray.600"
                color="white"
                _placeholder={{ color: "gray.400" }}
                _focus={{
                  borderColor: "green.500",
                  boxShadow: "0 0 0 1px var(--chakra-colors-green-500)",
                }}
                disabled={isLoading}
                autoFocus
              />
              <Text color="gray.500" fontSize="xs" alignSelf="start">
                Minimum 2 characters, maximum 12. ASCII only. Special characters will be removed.
              </Text>
              {error && (
                <Text color="red.400" fontSize="sm" alignSelf="start">
                  {error}
                </Text>
              )}
            </VStack>
            
            <HStack gap={4} w="full">
              <Button
                onClick={onSkip}
                variant="outline"
                size="lg"
                flex={1}
                borderColor="gray.600"
                color="gray.300"
                fontSize="md"
                fontWeight="semibold"
                _hover={{
                  bg: "gray.700",
                  borderColor: "gray.500",
                }}
                disabled={isLoading}
              >
                Skip
              </Button>
              
              <Button
                onClick={handleSubmit}
                colorScheme="green"
                size="lg"
                flex={1}
                fontSize="md"
                fontWeight="semibold"
                _hover={{
                  transform: "translateY(-1px)",
                  boxShadow: "lg",
                }}
                transition="all 0.2s"
                disabled={isLoading || label.trim().length < 2}
                loading={isLoading}
                loadingText="Setting Label..."
              >
                Set Label
              </Button>
            </HStack>
          </VStack>
        </Box>
      </Box>
  );
} 