import React, { useState, useEffect } from 'react';
import {
  DialogRoot,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogBody,
  DialogFooter,
  DialogCloseTrigger
} from "../ui/dialog";
import { 
  Box,
  Text,
  Button,
  VStack,
  HStack,
  Icon,
  Progress
} from '@chakra-ui/react';
import { FaShieldAlt, FaCheckCircle, FaTimesCircle } from 'react-icons/fa';
import { invoke } from '@tauri-apps/api/core';
import VerificationPin from './VerificationPin';

interface SeedVerificationWizardProps {
  isOpen: boolean;
  onClose: () => void;
  deviceId: string;
  deviceLabel?: string;
}

type VerificationStep = 'word-count' | 'pin' | 'phrase-entry' | 'result';

interface VerificationSession {
  session_id: string;
  device_id: string;
  word_count: number;
  current_word: number;
  current_character: number;
  is_active: boolean;
  pin_verified: boolean;
}

const SeedVerificationWizard: React.FC<SeedVerificationWizardProps> = ({
  isOpen,
  onClose,
  deviceId,
  deviceLabel = 'KeepKey'
}) => {
  const [currentStep, setCurrentStep] = useState<VerificationStep>('word-count');
  const [selectedWordCount, setSelectedWordCount] = useState<number>(12);
  const [session, setSession] = useState<VerificationSession | null>(null);
  const [verificationResult, setVerificationResult] = useState<boolean | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Cleanup session on unmount or close
  useEffect(() => {
    return () => {
      if (session?.session_id) {
        invoke('cancel_seed_verification', { sessionId: session.session_id }).catch(() => {});
      }
    };
  }, [session]);

  const handleStartVerification = async (wordCount: number) => {
    setIsLoading(true);
    setError(null);
    setSelectedWordCount(wordCount);

    try {
      const verificationSession = await invoke<VerificationSession>('start_seed_verification', {
        deviceId,
        wordCount
      });

      setSession(verificationSession);
      
      // Move to PIN step unless PIN is already verified
      if (verificationSession.pin_verified) {
        setCurrentStep('phrase-entry');
      } else {
        setCurrentStep('pin');
      }
    } catch (err) {
      console.error('Failed to start seed verification:', err);
      const errorMsg = err as string;
      
      // Handle "Device is already in recovery flow" error
      if (errorMsg.includes('Device is already in recovery flow')) {
        console.log('Device already in recovery flow, cleaning up and retrying...');
        setError('Device was already in verification mode. Cleaning up and retrying...');
        
        try {
          // Force cleanup any existing verification sessions for this device
          console.log('Forcing cleanup of existing verification sessions...');
          await invoke('force_cleanup_seed_verification', { deviceId: deviceId });
          
          // Small delay to let device reset
          await new Promise(resolve => setTimeout(resolve, 1000));
          
          // Retry the verification
          console.log('Retrying seed verification...');
          const retrySession = await invoke<VerificationSession>('start_seed_verification', {
            deviceId,
            wordCount
          });

          setSession(retrySession);
          setError(null); // Clear error on successful retry
          
          // Move to PIN step unless PIN is already verified
          if (retrySession.pin_verified) {
            setCurrentStep('phrase-entry');
          } else {
            setCurrentStep('pin');
          }
        } catch (retryErr) {
          console.error('Retry also failed:', retryErr);
          setError(`Still unable to start verification after cleanup: ${retryErr}`);
        }
      } else {
        setError(errorMsg);
      }
    } finally {
      setIsLoading(false);
    }
  };

  const handlePinComplete = async () => {
    // After PIN is verified, move to phrase entry
    setCurrentStep('phrase-entry');
  };

  const handleVerificationComplete = (success: boolean, errorMessage?: string) => {
    setVerificationResult(success);
    setCurrentStep('result');
    
    if (!success && errorMessage) {
      setError(errorMessage);
    }
  };

  const handleClose = () => {
    // Cleanup session if active
    if (session?.session_id) {
      invoke('cancel_seed_verification', { sessionId: session.session_id }).catch(() => {});
    }
    
    // Reset state
    setCurrentStep('word-count');
    setSession(null);
    setVerificationResult(null);
    setError(null);
    
    onClose();
  };

  const getStepNumber = () => {
    switch (currentStep) {
      case 'word-count': return 1;
      case 'pin': return 2;
      case 'phrase-entry': return 3;
      case 'result': return 4;
      default: return 1;
    }
  };

  const getStepTitle = () => {
    switch (currentStep) {
      case 'word-count': return 'Select Recovery Phrase Length';
      case 'pin': return 'Enter Your PIN';
      case 'phrase-entry': return 'Enter Recovery Phrase';
      case 'result': return verificationResult ? 'Verification Successful' : 'Verification Failed';
      default: return 'Verify Seed Phrase';
    }
  };

  return (
    <DialogRoot open={isOpen} onOpenChange={(details: any) => !details.open && handleClose()}>
      <DialogContent 
        maxW="500px"
        bg="gray.800"
        borderColor="gray.600"
        color="white"
      >
        <DialogHeader borderBottomWidth={1} borderColor="gray.600" pb={4}>
          <DialogTitle>
            <HStack gap={3}>
              <Icon as={FaShieldAlt} boxSize={5} color="blue.400" />
              <Text color="white">Verify Recovery Phrase</Text>
            </HStack>
          </DialogTitle>
          <DialogCloseTrigger color="gray.400" _hover={{ color: "white" }} />
        </DialogHeader>

        <DialogBody py={6}>
          <VStack gap={6} align="stretch">
            {/* Progress indicator */}
            {currentStep !== 'result' && (
              <Box>
                <HStack justify="space-between" mb={2}>
                  <Text fontSize="sm" color="gray.400">
                    Step {getStepNumber()} of 3
                  </Text>
                  <Text fontSize="sm" color="gray.400">
                    {deviceLabel}
                  </Text>
                </HStack>
                <Progress.Root 
                  value={(getStepNumber() / 3) * 100} 
                  size="sm"
                  bg="gray.700"
                >
                  <Progress.Track borderRadius="full" bg="gray.700">
                    <Progress.Range bg="blue.400" />
                  </Progress.Track>
                </Progress.Root>
              </Box>
            )}

            {/* Step title */}
            <Text fontSize="lg" fontWeight="medium" textAlign="center" color="white">
              {getStepTitle()}
            </Text>

            {/* Step content */}
            <Box 
              p={6} 
              borderWidth={1} 
              borderColor="gray.600"
              borderRadius="lg"
              bg="gray.700"
            >
              {currentStep === 'word-count' && (
                <VStack gap={4}>
                  <Text textAlign="center" color="gray.300">
                    KeepKey uses 12-word recovery phrases. Click below to begin verification:
                  </Text>
                  <VStack gap={2}>
                    <Button
                      size="lg"
                      variant="outline"
                      width="full"
                      onClick={() => handleStartVerification(12)}
                      loading={isLoading}
                      borderColor="gray.500"
                      color="white"
                      _hover={{ 
                        borderColor: "blue.400", 
                        bg: "gray.600" 
                      }}
                      _active={{ bg: "gray.500" }}
                    >
                      12 Words
                    </Button>
                  </VStack>
                </VStack>
              )}

              {currentStep === 'pin' && session && (
                <VerificationPin
                  sessionId={session.session_id}
                  deviceLabel={deviceLabel}
                  onComplete={handlePinComplete}
                  onCancel={handleClose}
                />
              )}

              {currentStep === 'phrase-entry' && (
                <VStack gap={4}>
                  <Text textAlign="center" color="gray.300">
                    Phrase entry component would go here
                  </Text>
                  <HStack gap={2}>
                    <Button 
                      onClick={() => handleVerificationComplete(true)}
                      colorScheme="green"
                    >
                      Mock Success
                    </Button>
                    <Button 
                      onClick={() => handleVerificationComplete(false, "Test failure")}
                      colorScheme="red"
                    >
                      Mock Failure
                    </Button>
                  </HStack>
                </VStack>
              )}

              {currentStep === 'result' && (
                <VStack gap={6} py={4}>
                  <Icon
                    as={verificationResult ? FaCheckCircle : FaTimesCircle}
                    boxSize={16}
                    color={verificationResult ? "green.400" : "red.400"}
                  />
                  
                  <VStack gap={3}>
                    <Text fontSize="xl" fontWeight="bold" color="white">
                      {verificationResult 
                        ? 'Verification Successful!' 
                        : 'Verification Failed'
                      }
                    </Text>
                    
                    <Text textAlign="center" color="gray.300">
                      {verificationResult
                        ? 'Your recovery phrase matches the one stored on your device.'
                        : 'The recovery phrase you entered does not match the one stored on your device.'
                      }
                    </Text>

                    {!verificationResult && error && (
                      <Box p={3} bg="red.900" borderRadius="md" borderWidth={1} borderColor="red.600">
                        <Text fontSize="sm" color="red.200">
                          {error}
                        </Text>
                      </Box>
                    )}
                  </VStack>

                  <Button
                    colorScheme={verificationResult ? "green" : "red"}
                    onClick={handleClose}
                    size="lg"
                    width="full"
                  >
                    {verificationResult ? 'Done' : 'Try Again'}
                  </Button>
                </VStack>
              )}
            </Box>

            {/* Error display */}
            {error && currentStep !== 'result' && (
              <Box p={3} bg="red.900" borderRadius="md" borderWidth={1} borderColor="red.600">
                <Text fontSize="sm" color="red.200">
                  {error}
                </Text>
              </Box>
            )}
          </VStack>
        </DialogBody>

        <DialogFooter borderTopWidth={1} borderColor="gray.600" pt={4}>
          {currentStep !== 'result' && (
            <Button 
              variant="outline" 
              onClick={handleClose}
              borderColor="gray.500"
              color="white"
              _hover={{ borderColor: "gray.400", bg: "gray.600" }}
            >
              Cancel
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </DialogRoot>
  );
};

export default SeedVerificationWizard; 