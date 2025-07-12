import React, { useState, useCallback } from 'react';
import { Box, Button, HStack, VStack, Text, Flex, Icon, Progress } from '@chakra-ui/react';
import { FaCheckCircle, FaExclamationTriangle } from 'react-icons/fa';
import { useDialog } from '../../contexts/DialogContext';
import { Step0Warning } from './steps/Step0Warning';
import { Step1UpdateInProgress } from './steps/Step1UpdateInProgress';
import { Step2Completion } from './steps/Step2Completion';

export interface BootloaderUpdateWizardProps {
  deviceId: string;
  currentVersion: string;
  requiredVersion: string;
  onClose?: () => void; // Called when wizard is closed by any means
  onComplete?: (deviceId: string) => void; // Called on successful completion
}

interface Step {
  id: string;
  label: string;
  description: string;
  component: React.ComponentType<StepProps>;
}

export interface StepProps {
  deviceId: string;
  currentVersion: string;
  requiredVersion: string;
  onNext: () => void;
  onPrevious: () => void; 
  onError: (error: string, advice?: string) => void; 
  onSetProgress?: (progress: { value: number; message: string }) => void; // For in-progress step
  clearError: () => void;
  errorInfo?: { message: string; advice?: string } | null; // For completion step
}

const STEPS: Step[] = [
  {
    id: 'warning',
    label: 'Update Required',
    description: 'A critical bootloader update is required for your KeepKey.',
    component: Step0Warning,
  },
  {
    id: 'in-progress',
    label: 'Updating',
    description: 'Bootloader update in progress. Do not disconnect.',
    component: Step1UpdateInProgress,
  },
  {
    id: 'completion',
    label: 'Complete',
    description: 'Bootloader update process finished.',
    component: Step2Completion,
  },
];

export function BootloaderUpdateWizard({ 
  deviceId,
  currentVersion,
  requiredVersion,
  onClose,
  onComplete 
}: BootloaderUpdateWizardProps) {
  const [currentStepIndex, setCurrentStepIndex] = useState(0);
  const [errorInfo, setErrorInfo] = useState<{ message: string; advice?: string } | null>(null);
  const [progressInfo, setProgressInfo] = useState<{ value: number; message: string } | null>(null);
  const { hide } = useDialog();
  const highlightColor = 'orange.500';

  // Safety check to prevent undefined activeStep
  const activeStep = STEPS[currentStepIndex] || STEPS[0];
  
  // Additional safety check and logging
  if (!STEPS[currentStepIndex]) {
    console.error(`🚨 [BootloaderUpdateWizard] Invalid currentStepIndex: ${currentStepIndex}, STEPS length: ${STEPS.length}`);
    console.error(`🚨 [BootloaderUpdateWizard] Resetting to step 0`);
    setCurrentStepIndex(0);
  }

  const handleNext = useCallback(() => {
    setErrorInfo(null);
    setProgressInfo(null);
    if (currentStepIndex < STEPS.length - 1) {
      setCurrentStepIndex(currentStepIndex + 1);
    } else {
      if (onComplete) onComplete(deviceId);
      if (onClose) onClose();
      hide(`bootloader-update-wizard-${deviceId}`);
    }
  }, [currentStepIndex, deviceId, onComplete, onClose, hide]);

  const handlePrevious = useCallback(() => {
    if (currentStepIndex > 0) {
      setCurrentStepIndex(currentStepIndex - 1);
      setErrorInfo(null);
      setProgressInfo(null);
    }
  }, [currentStepIndex]);
  
  const handleError = useCallback((message: string, advice?: string) => {
    setErrorInfo({ message, advice });
    // If an error occurs during 'in-progress', move to 'completion' step to show it.
    if (activeStep.id === 'in-progress') {
      setCurrentStepIndex(STEPS.findIndex(s => s.id === 'completion'));
    }
  }, [activeStep.id]);

  const clearError = useCallback(() => {
    setErrorInfo(null);
  }, []);

  const handleSetProgress = useCallback((progress: { value: number; message: string }) => {
    setProgressInfo(progress);
  }, []);

  const CurrentStepComponent = activeStep?.component;
  const overallProgress = ((currentStepIndex + 1) / STEPS.length) * 100;

  // Safety check - if no valid component, show error
  if (!CurrentStepComponent || !activeStep) {
    return (
      <Box
        w="100%"
        maxW="600px"
        bg="red.900" 
        borderRadius="xl"
        boxShadow="2xl"
        borderWidth="1px"
        borderColor="red.500"
        overflow="hidden"
        p={6}
      >
        <VStack gap={4}>
          <Text fontSize="xl" fontWeight="bold" color="red.200">
            Bootloader Update Wizard Error
          </Text>
          <Text color="red.300">
            Failed to load wizard step. Current step index: {currentStepIndex}
          </Text>
          <Button colorScheme="red" onClick={onClose}>
            Close
          </Button>
        </VStack>
      </Box>
    );
  }

  return (
    <Box
      w="100%"
      maxW="600px"
      bg="gray.850" // Slightly darker bg
      borderRadius="xl"
      boxShadow="2xl" // More pronounced shadow
      borderWidth="1px"
      borderColor={errorInfo ? "red.500" : highlightColor}
      overflow="hidden"
    >
        <Box p={5} borderBottomWidth="1px" borderColor="gray.700" bg="gray.900">
          <HStack justifyContent="space-between">
            <Text fontSize="lg" fontWeight="bold" color={highlightColor}>
              KeepKey Bootloader Update
            </Text>
            <Text fontSize="xs" color="gray.500">
              Step {currentStepIndex + 1} of {STEPS.length}: {activeStep.label}
            </Text>
          </HStack>
          <Text fontSize="sm" color="gray.400" mt={1}>
            {activeStep.description}
          </Text>
        </Box>

        {/* Progress Bar */} 
        <Box px={5} py={2} bg="gray.900">
            <Progress.Root 
              value={progressInfo ? progressInfo.value : overallProgress} 
              size="xs" 
              colorScheme={errorInfo ? "red" : "orange"}
            >
              <Progress.Track borderRadius="sm">
                <Progress.Range />
              </Progress.Track>
            </Progress.Root>
            {activeStep.id === 'in-progress' && progressInfo && (
                 <Text fontSize="xs" color="gray.400" mt={1} textAlign="center">{progressInfo.message}</Text>
            )}
        </Box>

        {errorInfo && activeStep.id !== 'completion' && (
          <Box p={4} bg="red.900" borderBottomWidth="1px" borderColor="red.700">
            <HStack>
              <Icon as={FaExclamationTriangle} color="red.400" />
              <Text color="red.300" fontSize="sm" fontWeight="bold">Error: {errorInfo.message}</Text>
            </HStack>
            {errorInfo.advice && <Text color="red.400" fontSize="xs" mt={1}>{errorInfo.advice}</Text>}
          </Box>
        )}

        <Box p={6} minH="300px" bg="gray.800"> {/* Content area bg */} 
          <CurrentStepComponent 
            deviceId={deviceId}
            currentVersion={currentVersion}
            requiredVersion={requiredVersion}
            onNext={handleNext} 
            onPrevious={handlePrevious} 
            onError={handleError}
            onSetProgress={handleSetProgress}
            clearError={clearError}
            errorInfo={errorInfo}
          />
        </Box>

        {/* Footer is usually handled by steps themselves for conditional buttons */}
      </Box>
  );
}
