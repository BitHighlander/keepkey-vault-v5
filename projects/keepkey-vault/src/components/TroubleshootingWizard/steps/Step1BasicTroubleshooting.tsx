import React, { useState } from 'react';
import { VStack, Text, Button, Box, HStack, Icon, Checkbox, Progress } from '@chakra-ui/react';
import { FaUsb, FaSync, FaCheck, FaExclamationTriangle } from 'react-icons/fa';
import { invoke } from '@tauri-apps/api/core';
import type { StepProps } from '../TroubleshootingWizard';

interface TroubleshootingStep {
  id: string;
  title: string;
  description: string;
  icon: any;
  completed: boolean;
}

export const Step1BasicTroubleshooting: React.FC<StepProps> = ({
  deviceId,
  onNext,
  onPrevious,
  onSuccess,
  onSetProgress,
}) => {
  const [currentStep, setCurrentStep] = useState(0);
  const [isTesting, setIsTesting] = useState(false);
  const [steps, setSteps] = useState<TroubleshootingStep[]>([
    {
      id: 'cable-check',
      title: 'Check USB Cable',
      description: 'Ensure you\'re using a data cable, not a charge-only cable. Try a different USB cable if available.',
      icon: FaUsb,
      completed: false,
    },
    {
      id: 'port-check',
      title: 'Try Different USB Port',
      description: 'Use a different USB port, preferably USB 2.0. Avoid USB hubs and use direct computer ports.',
      icon: FaUsb,
      completed: false,
    },
    {
      id: 'direct-connection',
      title: 'Remove USB Hubs/Extensions',
      description: 'Connect your KeepKey directly to your computer without any USB hubs or extension cables.',
      icon: FaUsb,
      completed: false,
    },
    {
      id: 'device-restart',
      title: 'Restart Device Connection',
      description: 'Unplug your KeepKey for 10 seconds, then reconnect it to refresh the connection.',
      icon: FaSync,
      completed: false,
    },
  ]);

  const handleStepComplete = (stepIndex: number) => {
    const newSteps = [...steps];
    newSteps[stepIndex].completed = true;
    setSteps(newSteps);
    setCurrentStep(stepIndex + 1);
  };

  const testConnection = async () => {
    setIsTesting(true);
    if (onSetProgress) {
      onSetProgress({ value: 10, message: 'Testing device connection...' });
    }

    try {
      // Try to get device info to test connection
      const deviceInfo = await invoke('get_device_info_by_id', { deviceId });
      
      if (onSetProgress) {
        onSetProgress({ value: 100, message: 'Connection test successful!' });
      }
      
      // Success! Device is communicating
      onSuccess('Device communication restored!');
    } catch (error) {
      if (onSetProgress) {
        onSetProgress({ value: 0, message: 'Connection test failed - let\'s try more steps' });
      }
      
      // Connection still not working
      setTimeout(() => {
        setIsTesting(false);
      }, 2000);
    }
  };

  const allStepsCompleted = steps.every(step => step.completed);

  return (
    <VStack align="start" gap={6}>
      {/* Introduction */}
      <Box bg="blue.50" p={4} borderRadius="md" w="full">
        <Text fontSize="sm" color="blue.700" fontWeight="medium" mb={2}>
          Let's start with the most common solutions
        </Text>
        <Text fontSize="sm" color="blue.600">
          Please follow these steps in order. After each step, we'll test if the connection is restored.
        </Text>
      </Box>

      {/* Troubleshooting Steps */}
      <VStack w="full" gap={4}>
        {steps.map((step, index) => (
          <Box 
            key={step.id}
            w="full" 
            p={4} 
            bg={step.completed ? "green.50" : index === currentStep ? "yellow.50" : "gray.50"}
            borderRadius="md" 
            border="1px solid" 
            borderColor={step.completed ? "green.200" : index === currentStep ? "yellow.200" : "gray.200"}
          >
            <HStack justify="space-between" align="start">
              <HStack align="start" flex={1}>
                <Icon 
                  as={step.completed ? FaCheck : step.icon} 
                  color={step.completed ? "green.500" : index === currentStep ? "yellow.500" : "gray.400"} 
                  boxSize={5} 
                  mt={1}
                />
                <VStack align="start" gap={2} flex={1}>
                  <Text fontWeight="medium" color={step.completed ? "green.700" : index === currentStep ? "yellow.700" : "gray.600"}>
                    Step {index + 1}: {step.title}
                  </Text>
                  <Text fontSize="sm" color={step.completed ? "green.600" : index === currentStep ? "yellow.600" : "gray.500"}>
                    {step.description}
                  </Text>
                </VStack>
              </HStack>
              
              {index === currentStep && !step.completed && (
                <Button 
                  size="sm" 
                  colorScheme="yellow"
                  onClick={() => handleStepComplete(index)}
                >
                  Done
                </Button>
              )}
              
              {step.completed && (
                <Icon as={FaCheck} color="green.500" boxSize={6} />
              )}
            </HStack>
          </Box>
        ))}
      </VStack>

      {/* Test Connection */}
      {allStepsCompleted && (
        <Box w="full" bg="blue.50" p={4} borderRadius="md">
          <Text fontWeight="medium" color="blue.700" mb={3}>
            Great! Now let's test if the connection is working:
          </Text>
          <Button 
            colorScheme="blue" 
            onClick={testConnection}
            loading={isTesting}
            loadingText="Testing..."
          >
            Test Device Connection
          </Button>
        </Box>
      )}

      {/* Navigation */}
      <HStack justify="space-between" w="full" pt={4}>
        <Button variant="outline" onClick={onPrevious}>
          Back
        </Button>
        
        {!allStepsCompleted ? (
          <Button 
            colorScheme="yellow" 
            onClick={() => setCurrentStep(steps.length)}
            variant="outline"
          >
            Skip to Advanced Steps
          </Button>
        ) : (
          <Button colorScheme="yellow" onClick={onNext}>
            Continue to Advanced Steps
          </Button>
        )}
      </HStack>
    </VStack>
  );
}; 