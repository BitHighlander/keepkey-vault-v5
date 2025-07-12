import {
  Button,
  Text,
  VStack,
  Icon,
  Box,
  Heading,
} from "@chakra-ui/react";
import { FaPlus, FaUndo } from "react-icons/fa";

interface FactoryStateProps {
  onCreateWallet: () => void;
  onRecoverWallet: () => void;
  onClose?: () => void;
}

export function FactoryState({ onCreateWallet, onRecoverWallet, onClose }: FactoryStateProps) {
  return (
    <Box
      w="100%"
      maxW="500px"
      bg="gray.800"
      borderRadius="xl"
      boxShadow="xl"
      borderWidth="1px"
      borderColor="gray.700"
      overflow="hidden"
    >
        <Box bg="gray.850" p={6}>
          <Heading fontSize="2xl" fontWeight="bold" color="white" textAlign="center">
            Get Started with KeepKey
          </Heading>
        </Box>
        
        <Box p={6}>
          <VStack gap={6}>
            <Text 
              color="gray.400" 
              textAlign="center"
              fontSize="md"
              lineHeight="1.6"
            >
              Your KeepKey is set to factory default settings. You can create a new wallet, 
              or restore a wallet using a Recovery Sentence.
            </Text>
            
            <VStack gap={4} w="full">
              <Button
                onClick={onCreateWallet}
                colorScheme="green"
                size="lg"
                width="full"
                fontSize="md"
                fontWeight="semibold"
                py={8}
                _hover={{
                  transform: "translateY(-1px)",
                  boxShadow: "lg",
                }}
                transition="all 0.2s"
              >
                <Icon as={FaPlus} mr={2} />
                Create a New Wallet
              </Button>
              
              <Button
                onClick={onRecoverWallet}
                variant="outline"
                size="lg"
                width="full"
                borderColor="gray.600"
                color="gray.300"
                fontSize="md"
                fontWeight="semibold"
                py={8}
                _hover={{
                  bg: "gray.700",
                  borderColor: "gray.500",
                  transform: "translateY(-1px)",
                  boxShadow: "lg",
                }}
                transition="all 0.2s"
              >
                <Icon as={FaUndo} mr={2} />
                Recover Wallet
              </Button>
            </VStack>
            
            <Text 
              fontSize="xs" 
              color="gray.500" 
              textAlign="center"
              mt={4}
            >
              Recovery requires your existing 12-word recovery phrase
            </Text>
          </VStack>
        </Box>
      </Box>
  );
} 