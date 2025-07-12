import React, { useState, useCallback } from 'react';
import { Box, Button, HStack, VStack, Text, Flex, Icon, Progress } from '@chakra-ui/react';
import { FaCheckCircle, FaExclamationTriangle } from 'react-icons/fa';
import { useDialog } from '../../contexts/DialogContext';
import { Step0Warning } from './steps/Step0Warning';
import { Step1UpdateInProgress } from './steps/Step1UpdateInProgress';
import { Step2Completion } from './steps/Step2Completion';

export interface FirmwareUpdateWizardProps {
  deviceId: string;
  currentVersion: string;
  targetVersion: string;
  onClose?: () => void; // Called when wizard is closed by any means
  onComplete?: (success: boolean, deviceId: string) => void; // Called on successful completion
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
  targetVersion: string;
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
    label: 'Firmware Update',
    description: 'A firmware update is available for your KeepKey.',
    component: Step0Warning,
  },
  {
    id: 'in-progress',
    label: 'Updating',
    description: 'Firmware update in progress. Do not disconnect.',
    component: Step1UpdateInProgress,
  },
  {
    id: 'completion',
    label: 'Complete',
    description: 'Firmware update complete.',
    component: Step2Completion,
  },
];

export const FirmwareUpdateWizard: React.FC<FirmwareUpdateWizardProps> = ({
  deviceId,
  currentVersion,
  targetVersion,
  onClose,
  onComplete,
}) => {
  const [currentStepIndex, setCurrentStepIndex] = useState(0);
  const [errorInfo, setErrorInfo] = useState<{ message: string; advice?: string } | null>(null);
  const [updateProgress, setUpdateProgress] = useState<{ value: number; message: string } | null>(null);
  
  const { hide } = useDialog();

  // Safety check to prevent undefined currentStep
  const currentStep = STEPS[currentStepIndex] || STEPS[0];
  
  // Additional safety check and logging
  if (!STEPS[currentStepIndex]) {
    console.error(`🚨 [FirmwareUpdateWizard] Invalid currentStepIndex: ${currentStepIndex}, STEPS length: ${STEPS.length}`);
    console.error(`🚨 [FirmwareUpdateWizard] Resetting to step 0`);
    setCurrentStepIndex(0);
  }
  
  const handleNext = useCallback(() => {
    console.log(`🎯 [FirmwareUpdateWizard] handleNext called - currentStepIndex: ${currentStepIndex}, STEPS.length: ${STEPS.length}`);
    
    // Strict bounds checking to prevent step index overflow
    if (currentStepIndex < 0 || currentStepIndex >= STEPS.length) {
      console.error(`🚨 [FirmwareUpdateWizard] Invalid step index ${currentStepIndex}, resetting to 0`);
      setCurrentStepIndex(0);
      return;
    }
    
    if (currentStepIndex < STEPS.length - 1) {
      const nextIndex = currentStepIndex + 1;
      console.log(`🎯 [FirmwareUpdateWizard] Moving to step ${nextIndex}`);
      setCurrentStepIndex(nextIndex);
    } else {
      // Final step complete - only execute once
      console.log(`🎯 [FirmwareUpdateWizard] Final step complete, closing wizard`);
      
      if (onComplete) {
        onComplete(!errorInfo, deviceId);
      }
      if (onClose) {
        onClose();
      }
      hide(`firmware-update-wizard-${deviceId}`);
    }
  }, [currentStepIndex, hide, onComplete, onClose, errorInfo, deviceId]);
  
  const handlePrevious = useCallback(() => {
    if (currentStepIndex > 0) {
      setCurrentStepIndex(prev => prev - 1);
    }
  }, [currentStepIndex]);
  
  const handleError = useCallback((message: string, advice?: string) => {
    setErrorInfo({ message, advice });
  }, []);
  
  const clearError = useCallback(() => {
    setErrorInfo(null);
  }, []);
  
  const handleSetProgress = useCallback((progress: { value: number; message: string }) => {
    setUpdateProgress(progress);
  }, []);

  const StepComponent = currentStep?.component;

  // Safety check - if no valid component, show error
  if (!StepComponent || !currentStep) {
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
            Firmware Update Wizard Error
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
      bg="gray.850"
      borderRadius="xl"
      boxShadow="2xl"
      borderWidth="1px"
      borderColor={errorInfo ? "red.500" : "blue.500"}
      overflow="hidden"
    >
      <VStack gap={4} p={6}>
      <HStack w="full" gap={2} mb={4}>
        {STEPS.map((step, idx) => (
          <React.Fragment key={step.id}>
            <Flex 
              direction="column" 
              alignItems="center" 
              flex={1}
              opacity={idx === currentStepIndex ? 1 : 0.5}
            >
              <Box 
                w={8} 
                h={8} 
                borderRadius="full" 
                bg={idx < currentStepIndex ? "green.500" : idx === currentStepIndex ? "blue.500" : "gray.200"}
                display="flex"
                alignItems="center"
                justifyContent="center"
                color="white"
                fontWeight="bold"
              >
                {idx < currentStepIndex ? (
                  <Icon as={FaCheckCircle} />
                ) : (
                  idx + 1
                )}
              </Box>
              <Text fontSize="sm" mt={1} textAlign="center" color="gray.200">{step.label}</Text>
            </Flex>
            
            {idx < STEPS.length - 1 && (
              <Box flex={0.5} h={1} bg={idx < currentStepIndex ? "green.500" : "gray.200"} />
            )}
          </React.Fragment>
        ))}
      </HStack>
      
      <Box w="full" p={6} bg="gray.800" borderRadius="md" boxShadow="md" border="1px solid" borderColor="gray.600">
        <Text fontSize="xl" fontWeight="bold" mb={4} color="gray.100">{currentStep.description}</Text>
        
        <StepComponent
          deviceId={deviceId}
          currentVersion={currentVersion}
          targetVersion={targetVersion}
          onNext={handleNext}
          onPrevious={handlePrevious}
          onError={handleError}
          onSetProgress={handleSetProgress}
          clearError={clearError}
          errorInfo={errorInfo}
        />
      </Box>
      
      {updateProgress && (
        <Box w="full">
          <Text fontSize="sm" mb={1} color="gray.300">{updateProgress.message}</Text>
          <Progress.Root value={updateProgress.value} size="sm" colorScheme="blue">
            <Progress.Track borderRadius="md">
              <Progress.Range />
            </Progress.Track>
          </Progress.Root>
        </Box>
      )}
      
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
      </VStack>
    </Box>
  );
};

export default FirmwareUpdateWizard;
