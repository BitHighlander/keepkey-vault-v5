import { Box, Button, Card, HStack, Text, VStack, Icon } from "@chakra-ui/react";
import { FaCog, FaPaintBrush, FaBell } from "react-icons/fa";

// Step components no longer need props - navigation handled by main wizard

export function Step1AppSettings() {
  return (
    <Box width="full" maxWidth="lg">
      <Card.Root bg="gray.900" borderColor="gray.700">
        <Card.Header bg="gray.850">
          <HStack justify="center" gap={3}>
            <Icon asChild color="green.500">
              <FaCog />
            </Icon>
            <Text fontSize="xl" fontWeight="bold" color="white">
              App Settings
            </Text>
          </HStack>
        </Card.Header>
        <Card.Body>
          <VStack gap={6}>
            <Text color="gray.400" textAlign="center">
              Configure your application preferences. You can change these settings later in the app.
            </Text>
            
            <HStack gap={6} width="full" alignItems="stretch">
              <Box flex="1" p={4} bg="gray.800" borderRadius="md" borderWidth="2px" borderColor="blue.600">
                <HStack gap={3} mb={3}>
                  <Icon asChild color="blue.400">
                    <FaPaintBrush />
                  </Icon>
                  <Text color="white" fontWeight="medium">Theme</Text>
                </HStack>
                <Text color="gray.400" fontSize="sm" mb={2}>
                  Dark theme provides better visibility and reduces eye strain during extended use.
                </Text>
                <Text color="blue.400" fontSize="sm" fontWeight="medium">
                  ✓ Dark theme active
                </Text>
              </Box>
              
              <Box flex="1" p={4} bg="gray.800" borderRadius="md" borderWidth="2px" borderColor="yellow.600">
                <HStack gap={3} mb={3}>
                  <Icon asChild color="yellow.400">
                    <FaBell />
                  </Icon>
                  <Text color="white" fontWeight="medium">Notifications</Text>
                </HStack>
                <Text color="gray.400" fontSize="sm" mb={2}>
                  Stay informed about important security updates and device status changes.
                </Text>
                <Text color="yellow.400" fontSize="sm" fontWeight="medium">
                  ✓ Notifications enabled
                </Text>
              </Box>
            </HStack>
            
            <Box textAlign="center" p={4} bg="green.900" borderRadius="md" borderWidth="1px" borderColor="green.600">
              <Text color="green.400" fontSize="sm">
                Default settings are optimized for security and usability
              </Text>
            </Box>
          </VStack>
        </Card.Body>
      </Card.Root>
    </Box>
  );
} 