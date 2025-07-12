import React from 'react';
import { VStack, Text, Button, HStack, Icon } from '@chakra-ui/react';
import { FaKey } from 'react-icons/fa';

interface VerificationWordCountProps {
  onSelectWordCount: (wordCount: number) => void;
  isLoading: boolean;
}

const VerificationWordCount: React.FC<VerificationWordCountProps> = ({
  onSelectWordCount,
  isLoading
}) => {
  const wordCountOptions = [
    { count: 12, description: "Standard for KeepKey devices" }
  ];

  return (
    <VStack gap={6} align="stretch">
      <VStack gap={3}>
        <Icon as={FaKey} boxSize={8} color="blue.500" />
        <Text textAlign="center" color="gray.500">
          KeepKey uses 12-word recovery phrases. Click below to begin verification.
        </Text>
      </VStack>

      <VStack gap={3}>
        {wordCountOptions.map(({ count, description }) => (
          <Button
            key={count}
            size="lg"
            variant="outline"
            width="full"
            height="auto"
            p={4}
            onClick={() => onSelectWordCount(count)}
            loading={isLoading}
            _hover={{ bg: 'gray.50', borderColor: 'blue.300' }}
          >
            <VStack gap={1}>
              <Text fontSize="xl" fontWeight="bold">
                {count} Words
              </Text>
              <Text fontSize="sm" color="gray.500">
                {description}
              </Text>
            </VStack>
          </Button>
        ))}
      </VStack>

      <Text fontSize="xs" textAlign="center" color="gray.400">
        Your KeepKey uses a 12-word recovery phrase that was generated when you first initialized your device.
      </Text>
    </VStack>
  );
};

export default VerificationWordCount; 