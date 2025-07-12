import { Box, Card, HStack, Text, VStack, Icon } from "@chakra-ui/react";
import { FaCheckCircle, FaRocket } from "react-icons/fa";

// Step components no longer need props - navigation handled by main wizard

export function Step4Complete() {
  return (
    <Box width="full" maxWidth="lg">
      <Card.Root bg="gray.900" borderColor="gray.700">
        <Card.Body>
          <VStack gap={3} align="center" textAlign="center">
            <Icon asChild color="green.500" boxSize={12}>
              <FaCheckCircle />
            </Icon>
            
            <VStack gap={2}>
              <Text fontSize="2xl" fontWeight="bold" color="green.400">
                Setup Complete!
              </Text>
              <Text color="gray.400" fontSize="lg">
                You're all set to start using KeepKey Desktop.
              </Text>
              <Text color="gray.300">
                Your preferences have been saved and you can now connect your KeepKey device.
              </Text>
            </VStack>
            
            <Box w="full" mt={3} p={4} bg="gradient-to-r from-blue.900 to-green.900" borderRadius="lg" borderWidth="2px" borderColor="green.600">
              <HStack gap={3} justify="center" mb={3}>
                <Icon asChild color="yellow.400" boxSize={6}>
                  <FaRocket />
                </Icon>
                <Text color="white" fontWeight="bold" fontSize="lg">Next Steps</Text>
              </HStack>
              <VStack gap={2}>
                <Text color="green.200" fontSize="sm" fontWeight="medium">
                  1. Connect your KeepKey device via USB
                </Text>
                <Text color="green.200" fontSize="sm" fontWeight="medium">
                  2. Follow the device setup prompts
                </Text>
                <Text color="green.200" fontSize="sm" fontWeight="medium">
                  3. Start managing your crypto assets securely
                </Text>
              </VStack>
            </Box>
            
            <Box textAlign="center" p={3} bg="gray.800" borderRadius="md" borderWidth="1px" borderColor="gray.600">
              <Text color="gray.400" fontSize="sm">
                💡 Need help? Visit our support center or check the built-in tutorials
              </Text>
            </Box>
          </VStack>
        </Card.Body>
      </Card.Root>
    </Box>
  );
} 