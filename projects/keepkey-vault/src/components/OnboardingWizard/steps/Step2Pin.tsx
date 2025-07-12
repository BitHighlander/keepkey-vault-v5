import { Box, Card, HStack, Text, VStack, Icon } from "@chakra-ui/react";
import { FaShieldAlt, FaLock, FaKey } from "react-icons/fa";

// Step components no longer need props - navigation handled by main wizard

export function Step2Pin() {
  return (
    <Box width="full" maxWidth="lg">
      <Card.Root bg="gray.900" borderColor="gray.700">
        <Card.Header bg="gray.850">
          <HStack justify="center" gap={3}>
            <Icon asChild color="green.500">
              <FaShieldAlt />
            </Icon>
            <Text fontSize="xl" fontWeight="bold" color="white">
              Security Setup
            </Text>
          </HStack>
        </Card.Header>
        <Card.Body>
          <VStack gap={3}>
            <Text color="gray.400" textAlign="center">
              Learn about PIN protection and security best practices for your KeepKey device.
            </Text>
            
            <HStack gap={6} width="full" alignItems="stretch">
              <Box flex="1" p={3} bg="gray.800" borderRadius="md" borderWidth="2px" borderColor="green.600">
                <HStack gap={3} mb={3}>
                  <Icon asChild color="green.400">
                    <FaLock />
                  </Icon>
                  <Text color="white" fontWeight="medium">PIN Protection</Text>
                </HStack>
                <Text color="gray.400" fontSize="sm" mb={2}>
                  Your PIN protects your device from unauthorized access. Choose a PIN that's easy for you to remember but hard for others to guess.
                </Text>
                <Text color="green.400" fontSize="sm" fontWeight="medium">
                  4-9 digits recommended
                </Text>
              </Box>
              
              <Box flex="1" p={3} bg="gray.800" borderRadius="md" borderWidth="2px" borderColor="orange.600">
                <HStack gap={3} mb={3}>
                  <Icon asChild color="orange.400">
                    <FaKey />
                  </Icon>
                  <Text color="white" fontWeight="medium">Recovery Phrase</Text>
                </HStack>
                <Text color="gray.400" fontSize="sm" mb={2}>
                  Your recovery phrase is the master backup of your wallet. Keep it safe and never share it with anyone.
                </Text>
                <Text color="orange.400" fontSize="sm" fontWeight="medium">
                  Write it down securely
                </Text>
              </Box>
            </HStack>
            
            <Box textAlign="center" p={3} bg="blue.900" borderRadius="md" borderWidth="1px" borderColor="blue.600">
              <Text color="blue.400" fontSize="sm">
                💡 You'll set up your PIN directly on your KeepKey device when you connect it
              </Text>
            </Box>
          </VStack>
        </Card.Body>
      </Card.Root>
    </Box>
  );
} 