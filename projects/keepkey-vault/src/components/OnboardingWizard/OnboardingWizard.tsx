import {
  Box,
  Button,
  HStack,
  VStack,
  Text,
  Flex,
  Icon,
} from "@chakra-ui/react";
import { useState } from "react";
import { FaCheckCircle } from "react-icons/fa";
import { invoke } from "@tauri-apps/api/core";
import { useDialog } from "../../contexts/DialogContext";

// Import individual steps
import { Step0Language } from "./steps/Step0Language";
import { Step1AppSettings } from "./steps/Step1AppSettings";
import { Step2Pin } from "./steps/Step2Pin";
import { Step3Mnemonics } from "./steps/Step3Mnemonics";
import { Step4Complete } from "./steps/Step4Complete";

interface OnboardingWizardProps {
  onClose?: () => void;
  onComplete?: () => void;
}

interface Step {
  id: string;
  label: string;
  description: string;
  component: React.ComponentType;
}

const STEPS: Step[] = [
  {
    id: "language",
    label: "Language",
    description: "Select your preferred language",
    component: Step0Language,
  },
  {
    id: "app-settings",
    label: "App Settings",
    description: "Configure your application preferences",
    component: Step1AppSettings,
  },
  {
    id: "pin",
    label: "Security",
    description: "Set up your PIN and security settings",
    component: Step2Pin,
  },
  {
    id: "mnemonics",
    label: "Recovery",
    description: "Understand recovery phrases",
    component: Step3Mnemonics,
  },
  {
    id: "complete",
    label: "Complete",
    description: "You're all set!",
    component: Step4Complete,
  },
];

export function OnboardingWizard({ onClose, onComplete }: OnboardingWizardProps) {
  const [currentStep, setCurrentStep] = useState(0);
  const highlightColor = "green.500";
  const { hide } = useDialog();

  const handleNext = () => {
    if (currentStep < STEPS.length - 1) {
      setCurrentStep(currentStep + 1);
    } else {
      handleComplete();
    }
  };

  const handlePrevious = () => {
    if (currentStep > 0) {
      setCurrentStep(currentStep - 1);
    }
  };

  const handleComplete = async () => {
    console.log("=== Starting onboarding completion ===");
    try {
      // Mark onboarding as completed in the database
      console.log("Calling set_onboarding_completed...");
      await invoke("set_onboarding_completed");
      console.log("set_onboarding_completed completed successfully");

      // Call the completion callback if provided
      if (onComplete) {
        console.log("Calling onComplete callback");
        onComplete();
      }

      // Use multiple methods to ensure the dialog closes
      if (onClose) {
        console.log("Calling onClose callback");
        onClose();
      }

      // Use the dialog context directly to force close after a short delay
      setTimeout(() => {
        hide('onboarding');
        console.log('Forced onboarding dialog closure via DialogContext');
      }, 100);
    } catch (error) {
      console.error("Failed to mark onboarding as completed:", error);
      // Try to get debug info on failure
      try {
        const debugInfo = await invoke<string>('debug_onboarding_state');
        console.log("Debug info after error:", debugInfo);
      } catch (debugError) {
        console.warn("Could not get debug info:", debugError);
      }
    }
  };

  const StepComponent = STEPS[currentStep].component;
  const progress = ((currentStep + 1) / STEPS.length) * 100;

  return (
    <Box
      w="100%"
      maxW="1200px"
      bg="gray.800"
      borderRadius="xl"
      boxShadow="xl"
      borderWidth="1px"
      borderColor="gray.700"
      overflow="hidden"
    >
        {/* Header */}
        <Box 
          p={6} 
          borderBottomWidth="1px" 
          borderColor="gray.700"
          bg="gray.850"
        >
          <VStack gap={4}>
            <Text fontSize="2xl" fontWeight="bold" color={highlightColor}>
              KeepKey Desktop Setup
            </Text>
            <Text fontSize="md" color="gray.400">
              {STEPS[currentStep].description}
            </Text>
          </VStack>
        </Box>

        {/* Progress */}
        <Box px={6} py={2}>
          <Box 
            h="4px" 
            bg="gray.700" 
            borderRadius="full"
            overflow="hidden"
          >
            <Box 
              h="100%" 
              bg="green.500" 
              borderRadius="full"
              transition="width 0.3s"
              w={`${progress}%`}
            />
          </Box>
        </Box>

        {/* Step indicators */}
        <HStack gap={4} justify="center" p={4}>
          {STEPS.map((step, index) => (
            <Flex key={step.id} align="center">
              <Box
                w={8}
                h={8}
                borderRadius="full"
                bg={index <= currentStep ? highlightColor : "gray.600"}
                display="flex"
                alignItems="center"
                justifyContent="center"
                transition="all 0.3s"
              >
                {index < currentStep ? (
                  <Icon as={FaCheckCircle} color="white" boxSize={4} />
                ) : (
                  <Text color="white" fontSize="sm" fontWeight="bold">
                    {index + 1}
                  </Text>
                )}
              </Box>
              <Text
                ml={2}
                fontSize="sm"
                fontWeight={index === currentStep ? "bold" : "normal"}
                color={index <= currentStep ? highlightColor : "gray.400"}
              >
                {step.label}
              </Text>
              {index < STEPS.length - 1 && (
                <Box
                  w={8}
                  h={0.5}
                  bg={index < currentStep ? highlightColor : "gray.600"}
                  ml={2}
                />
              )}
            </Flex>
          ))}
        </HStack>

        {/* Content */}
        <Box
          p={4}
          maxH="60vh"
          overflowY="auto"
          bg="gray.800"
          display="flex"
          alignItems="center"
          justifyContent="center"
        >
          <StepComponent />
        </Box>

        {/* Footer */}
        <Box p={6} borderTopWidth="1px" borderColor="gray.700" bg="gray.850">
          <HStack justify="space-between">
            <Text fontSize="sm" color="gray.400">
              Step {currentStep + 1} of {STEPS.length}
            </Text>
            <HStack gap={4}>
              <Button
                variant="outline"
                onClick={handlePrevious}
                disabled={currentStep === 0}
                borderColor="gray.600"
                color="gray.300"
                _hover={{ bg: "gray.700" }}
              >
                Previous
              </Button>
              <Button
                colorScheme="green"
                onClick={handleNext}
                size="lg"
              >
                {currentStep === STEPS.length - 1 ? "Complete Setup" : "Next"}
              </Button>
            </HStack>
          </HStack>
        </Box>
      </Box>
  );
} 