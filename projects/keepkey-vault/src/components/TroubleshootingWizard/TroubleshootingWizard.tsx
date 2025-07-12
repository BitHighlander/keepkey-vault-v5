import React, { useState, useCallback } from 'react';
import { Box, Button, HStack, VStack, Text, Flex, Icon, Progress } from '@chakra-ui/react';
import { FaCheckCircle, FaExclamationTriangle } from 'react-icons/fa';
import { useDialog } from '../../contexts/DialogContext';
import { Step0ProblemDetection } from './steps/Step0ProblemDetection';
import { Step1BasicTroubleshooting } from './steps/Step1BasicTroubleshooting';
import { Step2AdvancedRecovery } from './steps/Step2AdvancedRecovery';
import { Step3ForceRecovery } from './steps/Step3ForceRecovery';
import { Step4Resolution } from './steps/Step4Resolution';

export interface TroubleshootingWizardProps {
  deviceId: string;
  errorDetails: string;
  onClose?: () => void; // Called when wizard is closed by any means
  onResolved?: () => void; // Called when communication is restored
  onContactSupport?: (diagnostics: any) => void; // Called when user needs support
}

interface Step {
  id: string;
  label: string;
  description: string;
  component: React.ComponentType<StepProps>;
}

export interface StepProps {
  deviceId: string;
  errorDetails: string;
  onNext: () => void;
  onPrevious: () => void; 
  onError: (error: string, advice?: string) => void; 
  onSuccess: (message: string) => void;
  onSetProgress?: (progress: { value: number; message: string }) => void;
  clearError: () => void;
  errorInfo?: { message: string; advice?: string } | null;
  onContactSupport?: (diagnostics: any) => void;
}

const STEPS: Step[] = [
  {
    id: 'problem-detection',
    label: 'Issue Detected',
    description: 'Device Communication Issue Detected',
    component: Step0ProblemDetection,
  },
  {
    id: 'basic-troubleshooting',
    label: 'Basic Solutions',
    description: 'Let\'s Try Basic Solutions',
    component: Step1BasicTroubleshooting,
  },
  {
    id: 'advanced-recovery',
    label: 'Advanced Recovery',
    description: 'Advanced Recovery Steps',
    component: Step2AdvancedRecovery,
  },
  {
    id: 'force-recovery',
    label: 'Emergency Recovery',
    description: 'Emergency Recovery Options',
    component: Step3ForceRecovery,
  },
  {
    id: 'resolution',
    label: 'Resolution',
    description: 'Communication Status',
    component: Step4Resolution,
  },
];

export const TroubleshootingWizard: React.FC<TroubleshootingWizardProps> = ({
  deviceId,
  errorDetails,
  onClose,
  onResolved,
  onContactSupport,
}) => {
  const [currentStepIndex, setCurrentStepIndex] = useState(0);
  const [errorInfo, setErrorInfo] = useState<{ message: string; advice?: string } | null>(null);
  const [updateProgress, setUpdateProgress] = useState<{ value: number; message: string } | null>(null);
  const [isResolved, setIsResolved] = useState(false);
  
  const { hide } = useDialog();

  const currentStep = STEPS[currentStepIndex];
  
  const handleNext = useCallback(() => {
    if (currentStepIndex < STEPS.length - 1) {
      setCurrentStepIndex(prev => prev + 1);
    } else {
      // Final step complete
      if (isResolved && onResolved) {
        onResolved();
      }
      if (onClose) {
        onClose();
      }
      hide(`troubleshooting-wizard-${deviceId}`);
    }
  }, [currentStepIndex, hide, onResolved, onClose, isResolved, deviceId]);
  
  const handlePrevious = useCallback(() => {
    if (currentStepIndex > 0) {
      setCurrentStepIndex(prev => prev - 1);
    }
  }, [currentStepIndex]);
  
  const handleError = useCallback((message: string, advice?: string) => {
    setErrorInfo({ message, advice });
  }, []);
  
  const handleSuccess = useCallback((message: string) => {
    setIsResolved(true);
    setErrorInfo(null);
    // Jump to resolution step
    setCurrentStepIndex(STEPS.findIndex(s => s.id === 'resolution'));
  }, []);
  
  const clearError = useCallback(() => {
    setErrorInfo(null);
  }, []);
  
  const handleSetProgress = useCallback((progress: { value: number; message: string }) => {
    setUpdateProgress(progress);
  }, []);

  const handleContactSupport = useCallback((diagnostics: any) => {
    if (onContactSupport) {
      onContactSupport(diagnostics);
    }
  }, [onContactSupport]);

  const StepComponent = currentStep.component;

  return (
    <VStack 
      maxWidth="600px" 
      width="100%" 
      maxHeight="90vh" 
      gap={6} 
      align="stretch"
      bg="gray.800" 
      borderRadius="lg" 
      boxShadow="2xl" 
      border="1px solid" 
      borderColor="gray.600"
      p={6}
      overflow="auto"
    >
        
        {/* Progress Indicator */}
        <Box w="full">
          <HStack justify="space-between" mb={2}>
            <Text fontSize="sm" color="gray.400">Step {currentStepIndex + 1} of {STEPS.length}</Text>
            <Text fontSize="sm" color="gray.400">{currentStep.label}</Text>
          </HStack>
          <Progress.Root value={(currentStepIndex / (STEPS.length - 1)) * 100} size="sm" colorScheme="yellow">
            <Progress.Track borderRadius="md">
              <Progress.Range />
            </Progress.Track>
          </Progress.Root>
        </Box>
        
        {/* Step Content */}
        <Box w="full" p={6} bg="gray.800" borderRadius="md" boxShadow="md" border="1px solid" borderColor="gray.600">
          <Text fontSize="xl" fontWeight="bold" mb={4} color="gray.100">{currentStep.description}</Text>
          
          <StepComponent
            deviceId={deviceId}
            errorDetails={errorDetails}
            onNext={handleNext}
            onPrevious={handlePrevious}
            onError={handleError}
            onSuccess={handleSuccess}
            onSetProgress={handleSetProgress}
            clearError={clearError}
            errorInfo={errorInfo}
            onContactSupport={handleContactSupport}
          />
        </Box>
        
        {/* Progress Display */}
        {updateProgress && (
          <Box w="full">
            <Text fontSize="sm" mb={1} color="gray.300">{updateProgress.message}</Text>
            <Progress.Root value={updateProgress.value} size="sm" colorScheme="yellow">
              <Progress.Track borderRadius="md">
                <Progress.Range />
              </Progress.Track>
            </Progress.Root>
          </Box>
        )}
        
        {/* Error Display */}
        {errorInfo && (
          <Box w="full" p={4} bg="red.900" borderRadius="md" borderLeft="4px solid" borderColor="red.500">
            <Flex alignItems="center">
              <Icon as={FaExclamationTriangle} color="red.400" boxSize={5} mr={2} />
              <Box>
                <Text fontWeight="medium" color="red.200">{errorInfo.message}</Text>
                {errorInfo.advice && <Text fontSize="sm" color="red.300" mt={1}>{errorInfo.advice}</Text>}
              </Box>
            </Flex>
          </Box>
        )}
        
        {/* Success Display */}
        {isResolved && (
          <Box w="full" p={4} bg="green.900" borderRadius="md" borderLeft="4px solid" borderColor="green.500">
            <Flex alignItems="center">
              <Icon as={FaCheckCircle} color="green.400" boxSize={5} mr={2} />
              <Text fontWeight="medium" color="green.200">Communication restored! Your KeepKey is now working properly.</Text>
            </Flex>
          </Box>
        )}
      </VStack>
  );
}; 