import React, { useState, useEffect } from 'react';
import { 
  VStack, 
  Text, 
  Button, 
  SimpleGrid, 
  Box, 
  HStack, 
  Icon,
  Progress,
  Badge
} from '@chakra-ui/react';
import { FaKeyboard, FaBackspace, FaCheck, FaArrowRight, FaTrash } from 'react-icons/fa';
import { invoke } from '@tauri-apps/api/core';

interface VerificationPhraseEntryProps {
  session: {
    sessionId: string;
    deviceId: string;
    wordCount: number;
    currentWord: number;
    currentCharacter: number;
  };
  deviceLabel: string;
  onComplete: (success: boolean, errorMessage?: string) => void;
  onCancel: () => void;
}

interface RecoveryProgress {
  word_pos: number;
  character_pos: number;
  auto_completed: boolean;
  is_complete: boolean;
  error?: string;
}

type RecoveryAction = 'Space' | 'Done' | 'Delete';

const VerificationPhraseEntry: React.FC<VerificationPhraseEntryProps> = ({
  session,
  deviceLabel,
  onComplete,
  onCancel
}) => {
  const [currentWordPos, setCurrentWordPos] = useState(session.currentWord);
  const [currentCharacterPos, setCurrentCharacterPos] = useState(session.currentCharacter);
  const [enteredWords, setEnteredWords] = useState<string[]>([]);
  const [currentWordBuffer, setCurrentWordBuffer] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const alphabet = 'abcdefghijklmnopqrstuvwxyz'.split('');

  useEffect(() => {
    setCurrentWordPos(session.currentWord);
    setCurrentCharacterPos(session.currentCharacter);
  }, [session]);

  const handleCharacterClick = async (character: string) => {
    setIsSubmitting(true);
    setError(null);

    try {
      const progress = await invoke<RecoveryProgress>('send_verification_character', {
        sessionId: session.sessionId,
        character,
        action: null
      });

      if (progress.is_complete) {
        if (progress.error) {
          // Verification failed
          onComplete(false, progress.error);
        } else {
          // Verification successful
          onComplete(true);
        }
        return;
      }

      // Update current position
      setCurrentWordPos(progress.word_pos);
      setCurrentCharacterPos(progress.character_pos);
      setCurrentWordBuffer(prev => prev + character);

    } catch (err) {
      console.error('Failed to send character:', err);
      setError(err as string);
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleAction = async (action: RecoveryAction) => {
    setIsSubmitting(true);
    setError(null);

    try {
      const progress = await invoke<RecoveryProgress>('send_verification_character', {
        sessionId: session.sessionId,
        character: null,
        action
      });

      if (progress.is_complete) {
        if (progress.error) {
          // Verification failed
          onComplete(false, progress.error);
        } else {
          // Verification successful
          onComplete(true);
        }
        return;
      }

      // Handle different actions
      if (action === 'Space') {
        // Word completed, move to next
        setEnteredWords(prev => [...prev, currentWordBuffer]);
        setCurrentWordBuffer('');
      } else if (action === 'Delete') {
        // Remove last character
        setCurrentWordBuffer(prev => prev.slice(0, -1));
      }

      // Update current position
      setCurrentWordPos(progress.word_pos);
      setCurrentCharacterPos(progress.character_pos);

    } catch (err) {
      console.error('Failed to send action:', err);
      setError(err as string);
    } finally {
      setIsSubmitting(false);
    }
  };

  const renderAlphabetGrid = () => {
    return (
      <SimpleGrid columns={6} gap={2} w="full" maxW="400px" mx="auto">
        {alphabet.map((char) => (
          <Button
            key={char}
            size="sm"
            variant="outline"
            height="40px"
            fontSize="lg"
            fontWeight="bold"
            onClick={() => handleCharacterClick(char)}
            disabled={isSubmitting}
            _hover={{ bg: 'blue.50', borderColor: 'blue.300' }}
            _active={{ bg: 'blue.100' }}
          >
            {char.toUpperCase()}
          </Button>
        ))}
      </SimpleGrid>
    );
  };

  const renderProgress = () => {
    const progressPercentage = ((currentWordPos) / session.wordCount) * 100;
    
    return (
      <VStack gap={2} w="full">
        <HStack justify="space-between" w="full">
          <Text fontSize="sm" color="gray.500">
            Word {currentWordPos + 1} of {session.wordCount}
          </Text>
          <Text fontSize="sm" color="gray.500">
            Character {currentCharacterPos + 1}
          </Text>
        </HStack>
        <Progress.Root 
          value={progressPercentage} 
          size="sm" 
          colorScheme="blue"
          w="full"
        >
          <Progress.Track borderRadius="full">
            <Progress.Range />
          </Progress.Track>
        </Progress.Root>
      </VStack>
    );
  };

  const renderWordHistory = () => {
    return (
      <Box w="full" p={3} bg="gray.50" borderRadius="md" borderWidth={1} borderColor="gray.200">
        <Text fontSize="sm" fontWeight="medium" mb={2} color="gray.700">
          Entered Words:
        </Text>
        <HStack gap={2} wrap="wrap">
          {enteredWords.map((word, index) => (
            <Badge key={index} colorScheme="green" fontSize="xs">
              {index + 1}: {word}
            </Badge>
          ))}
          {currentWordBuffer && (
            <Badge colorScheme="blue" fontSize="xs">
              {enteredWords.length + 1}: {currentWordBuffer}...
            </Badge>
          )}
        </HStack>
      </Box>
    );
  };

  return (
    <VStack gap={6} align="stretch">
      <VStack gap={3}>
        <Icon as={FaKeyboard} boxSize={8} color="blue.500" />
        <Text fontSize="lg" fontWeight="medium" textAlign="center">
          Enter Your Recovery Phrase
        </Text>
        <Text textAlign="center" color="gray.500" fontSize="sm">
          Use the scrambled keyboard on your {deviceLabel} to enter your recovery phrase.
          Click the letters below as they appear on your device screen.
        </Text>
      </VStack>

      {/* Progress */}
      {renderProgress()}

      {/* Word History */}
      {renderWordHistory()}

      {/* Alphabet Grid */}
      {renderAlphabetGrid()}

      {/* Action Buttons */}
      <VStack gap={3}>
        <HStack gap={3} w="full">
          <Button
            variant="outline"
            onClick={() => handleAction('Delete')}
            disabled={currentCharacterPos === 0 || isSubmitting}
            flex="1"
          >
            <HStack gap={2}>
              <FaBackspace />
              <Text>Delete</Text>
            </HStack>
          </Button>
          
          <Button
            variant="outline"
            onClick={() => handleAction('Space')}
            disabled={currentWordBuffer.length === 0 || isSubmitting}
            flex="1"
          >
            <HStack gap={2}>
              <FaArrowRight />
              <Text>Next Word</Text>
            </HStack>
          </Button>
        </HStack>

        <Button
          colorScheme="green"
          onClick={() => handleAction('Done')}
          disabled={currentWordPos < session.wordCount - 1 || isSubmitting}
          loading={isSubmitting}
          size="lg"
          w="full"
        >
          <HStack gap={2}>
            <FaCheck />
            <Text>Complete Verification</Text>
          </HStack>
        </Button>
      </VStack>

      {/* Error Display */}
      {error && (
        <Box p={3} bg="red.50" borderRadius="md" borderWidth={1} borderColor="red.200">
          <Text fontSize="sm" color="red.700">
            {error}
          </Text>
        </Box>
      )}

      {/* Instructions */}
      <Box p={3} bg="blue.50" borderRadius="md" borderWidth={1} borderColor="blue.200">
        <Text fontSize="sm" color="blue.700">
          <strong>Important:</strong> Look at your device screen to see the scrambled alphabet. 
          Click the letters below that correspond to the letters shown on your device for each word 
          of your recovery phrase.
        </Text>
      </Box>
    </VStack>
  );
};

export default VerificationPhraseEntry; 