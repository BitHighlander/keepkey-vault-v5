import React, { useState } from 'react';
import { VStack, Text, Button, Box, HStack, Icon, Badge } from '@chakra-ui/react';
import { FaTools, FaSync, FaCheck, FaExclamationTriangle } from 'react-icons/fa';
import { invoke } from '@tauri-apps/api/core';
import type { StepProps } from '../TroubleshootingWizard';

interface AdvancedStep {
  id: string;
  title: string;
  description: string;
  instructions: string[];
  warning?: string;
  completed: boolean;
}

export const Step2AdvancedRecovery: React.FC<StepProps> = ({
  deviceId,
  onNext,
  onPrevious,
  onSuccess,
  onSetProgress,
}) => {
  const [currentStep, setCurrentStep] = useState(0);
  const [isTesting, setIsTesting] = useState(false);
  const [steps, setSteps] = useState<AdvancedStep[]>([
    {
      id: 'bootloader-mode',
      title: 'Try Bootloader Mode',
      description: 'Connect your device in bootloader mode for better compatibility',
      instructions: [
        'Unplug your KeepKey completely',
        'Hold down the button on your KeepKey',
        'While holding the button, plug the USB cable back in',
        'Keep holding for 3 seconds, then release',
        'Your device should now be in bootloader mode'
      ],
      warning: 'Only use bootloader mode if normal connection fails',
      completed: false,
    },
    {
      id: 'device-reset',
      title: 'Hardware Reset',
      description: 'Perform a hardware reset to clear any stuck states',
      instructions: [
        'Ensure your KeepKey is connected',
        'Hold down the button for 12 seconds',
        'Release the button',
        'Wait for the device to restart',
        'The screen should show the KeepKey logo'
      ],
      warning: 'This will not affect your wallet or funds, only device state',
      completed: false,
    },
    {
      id: 'driver-check',
      title: 'Check System Drivers',
      description: 'Verify that your system can detect the device',
      instructions: [
        'Check if the device appears in system device manager',
        'Look for any warning indicators or unknown devices',
        'Ensure USB drivers are properly installed',
        'Try installing the latest drivers if needed'
      ],
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
      onSetProgress({ value: 10, message: 'Testing advanced connection methods...' });
    }

    try {
      // Try to get device info to test connection
      const deviceInfo = await invoke('get_device_info_by_id', { deviceId });
      
      if (onSetProgress) {
        onSetProgress({ value: 100, message: 'Advanced recovery successful!' });
      }
      
      // Success! Device is communicating
      onSuccess('Device communication restored using advanced methods!');
    } catch (error) {
      if (onSetProgress) {
        onSetProgress({ value: 0, message: 'Advanced methods unsuccessful' });
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
      <Box bg="orange.50" p={4} borderRadius="md" w="full">
        <Text fontSize="sm" color="orange.700" fontWeight="medium" mb={2}>
          Advanced Recovery Methods
        </Text>
        <Text fontSize="sm" color="orange.600">
          These steps involve more technical solutions. Please follow the instructions carefully.
        </Text>
      </Box>

      {/* Advanced Steps */}
      <VStack w="full" gap={4}>
        {steps.map((step, index) => (
          <Box 
            key={step.id}
            w="full" 
            p={4} 
            bg={step.completed ? "green.50" : index === currentStep ? "orange.50" : "gray.50"}
            borderRadius="md" 
            border="1px solid" 
            borderColor={step.completed ? "green.200" : index === currentStep ? "orange.200" : "gray.200"}
          >
            <VStack align="start" gap={3}>
              <HStack justify="space-between" w="full">
                <HStack>
                  <Icon 
                    as={step.completed ? FaCheck : FaTools} 
                    color={step.completed ? "green.500" : index === currentStep ? "orange.500" : "gray.400"} 
                    boxSize={5}
                  />
                  <Text fontWeight="medium" color={step.completed ? "green.700" : index === currentStep ? "orange.700" : "gray.600"}>
                    {step.title}
                  </Text>
                  {index === currentStep && !step.completed && (
                    <Badge colorScheme="orange" variant="solid">Current</Badge>
                  )}
                </HStack>
                
                {step.completed && (
                  <Icon as={FaCheck} color="green.500" boxSize={6} />
                )}
              </HStack>

              <Text fontSize="sm" color={step.completed ? "green.600" : index === currentStep ? "orange.600" : "gray.500"}>
                {step.description}
              </Text>

              {(index === currentStep || step.completed) && (
                <Box w="full">
                  {step.warning && (
                    <Box bg="yellow.50" p={3} borderRadius="md" mb={3} borderLeft="3px solid" borderLeftColor="yellow.400">
                      <HStack>
                        <Icon as={FaExclamationTriangle} color="yellow.500" boxSize={4} />
                        <Text fontSize="sm" color="yellow.700" fontWeight="medium">
                          {step.warning}
                        </Text>
                      </HStack>
                    </Box>
                  )}
                  
                  <VStack align="start" gap={2}>
                    <Text fontSize="sm" fontWeight="medium" color="gray.700">Instructions:</Text>
                    {step.instructions.map((instruction, i) => (
                      <HStack key={i} align="start">
                        <Text fontSize="sm" color="gray.500" minW="20px">{i + 1}.</Text>
                        <Text fontSize="sm" color="gray.600">{instruction}</Text>
                      </HStack>
                    ))}
                  </VStack>

                  {index === currentStep && !step.completed && (
                    <HStack mt={3}>
                      <Button 
                        size="sm" 
                        colorScheme="orange"
                        onClick={() => handleStepComplete(index)}
                      >
                        I've Completed This Step
                      </Button>
                    </HStack>
                  )}
                </Box>
              )}
            </VStack>
          </Box>
        ))}
      </VStack>

      {/* Test Connection */}
      {allStepsCompleted && (
        <Box w="full" bg="blue.50" p={4} borderRadius="md">
          <Text fontWeight="medium" color="blue.700" mb={3}>
            Now let's test if any of these advanced methods worked:
          </Text>
          <Button 
            colorScheme="blue" 
            onClick={testConnection}
            loading={isTesting}
            loadingText="Testing..."
          >
            Test Advanced Recovery
          </Button>
        </Box>
      )}

      {/* Navigation */}
      <HStack justify="space-between" w="full" pt={4}>
        <Button variant="outline" onClick={onPrevious}>
          Back to Basic Steps
        </Button>
        
        {!allStepsCompleted ? (
          <Button 
            colorScheme="orange" 
            onClick={() => setCurrentStep(steps.length)}
            variant="outline"
          >
            Skip to Emergency Recovery
          </Button>
        ) : (
          <Button colorScheme="orange" onClick={onNext}>
            Continue to Emergency Recovery
          </Button>
        )}
      </HStack>
    </VStack>
  );
}; 