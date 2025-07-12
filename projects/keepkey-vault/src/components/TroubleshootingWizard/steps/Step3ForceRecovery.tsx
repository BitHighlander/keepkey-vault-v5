import React, { useState } from 'react';
import { VStack, Text, Button, Box, HStack, Icon, Link, Textarea } from '@chakra-ui/react';
import { FaExclamationTriangle, FaLifeRing, FaDownload, FaEnvelope } from 'react-icons/fa';
import type { StepProps } from '../TroubleshootingWizard';

export const Step3ForceRecovery: React.FC<StepProps> = ({
  deviceId,
  errorDetails,
  onNext,
  onPrevious,
  onContactSupport,
}) => {
  const [diagnosticInfo, setDiagnosticInfo] = useState('');
  const [isGeneratingDiagnostics, setIsGeneratingDiagnostics] = useState(false);

  const generateDiagnostics = () => {
    setIsGeneratingDiagnostics(true);
    
    const diagnostics = {
      deviceId,
      errorDetails,
      timestamp: new Date().toISOString(),
      platform: navigator.platform,
      userAgent: navigator.userAgent,
      steps_attempted: [
        'Basic cable and port troubleshooting',
        'Advanced recovery methods',
        'Emergency recovery attempts'
      ],
      device_info: {
        detected: true,
        communication_failed: true,
        last_error: errorDetails
      }
    };

    const diagnosticText = JSON.stringify(diagnostics, null, 2);
    setDiagnosticInfo(diagnosticText);
    setIsGeneratingDiagnostics(false);
  };

  const handleContactSupport = () => {
    if (onContactSupport) {
      onContactSupport({
        deviceId,
        errorDetails,
        diagnosticInfo: diagnosticInfo || 'No diagnostics generated'
      });
    }
  };

  return (
    <VStack align="start" gap={6}>
      {/* Emergency Notice */}
      <Box bg="red.50" p={4} borderRadius="md" w="full" borderLeft="4px solid" borderLeftColor="red.400">
        <HStack mb={2}>
          <Icon as={FaExclamationTriangle} color="red.500" boxSize={5} />
          <Text fontWeight="bold" color="red.700">Emergency Recovery</Text>
        </HStack>
        <Text fontSize="sm" color="red.600">
          If you've reached this step, your KeepKey requires additional assistance. The options below will help you get back up and running.
        </Text>
      </Box>

      {/* Recovery Options */}
      <VStack w="full" gap={4}>
        
        {/* Factory Reset Option */}
        <Box w="full" p={4} bg="orange.50" borderRadius="md" border="1px solid" borderColor="orange.200">
          <VStack align="start" gap={3}>
            <HStack>
              <Icon as={FaLifeRing} color="orange.500" boxSize={5} />
              <Text fontWeight="medium" color="orange.700">Factory Reset (Last Resort)</Text>
            </HStack>
            
            <Text fontSize="sm" color="orange.600">
              A factory reset will restore your device to its original state. <strong>Your wallet and funds will be safe</strong> as long as you have your recovery phrase.
            </Text>
            
            <Box bg="yellow.50" p={3} borderRadius="md" w="full">
              <Text fontSize="sm" fontWeight="medium" color="yellow.700" mb={2}>
                ⚠️ Before proceeding, ensure you have:
              </Text>
              <VStack align="start" gap={1}>
                <Text fontSize="sm" color="yellow.600">• Your 12-word recovery phrase written down</Text>
                <Text fontSize="sm" color="yellow.600">• Any passphrases you may have used</Text>
                <Text fontSize="sm" color="yellow.600">• Knowledge of your wallet setup</Text>
              </VStack>
            </Box>
            
            <Text fontSize="sm" color="orange.600">
              If you need to perform a factory reset, please contact support for guided assistance.
            </Text>
          </VStack>
        </Box>

        {/* Generate Diagnostics */}
        <Box w="full" p={4} bg="blue.50" borderRadius="md" border="1px solid" borderColor="blue.200">
          <VStack align="start" gap={3}>
            <HStack>
              <Icon as={FaDownload} color="blue.500" boxSize={5} />
              <Text fontWeight="medium" color="blue.700">Generate Diagnostic Information</Text>
            </HStack>
            
            <Text fontSize="sm" color="blue.600">
              Create a diagnostic report to help support understand your specific issue.
            </Text>
            
            <Button 
              colorScheme="blue" 
              size="sm"
              onClick={generateDiagnostics}
              loading={isGeneratingDiagnostics}
              loadingText="Generating..."
            >
              Generate Diagnostics
            </Button>

            {diagnosticInfo && (
              <Box w="full">
                <Text fontSize="sm" fontWeight="medium" color="blue.700" mb={2}>
                  Diagnostic Information:
                </Text>
                <Textarea 
                  value={diagnosticInfo}
                  readOnly
                  size="sm"
                  bg="white"
                  rows={8}
                  fontFamily="mono"
                  fontSize="xs"
                />
                <Text fontSize="xs" color="blue.500" mt={1}>
                  Copy this information when contacting support
                </Text>
              </Box>
            )}
          </VStack>
        </Box>

        {/* Contact Support */}
        <Box w="full" p={4} bg="green.50" borderRadius="md" border="1px solid" borderColor="green.200">
          <VStack align="start" gap={3}>
            <HStack>
              <Icon as={FaEnvelope} color="green.500" boxSize={5} />
              <Text fontWeight="medium" color="green.700">Contact KeepKey Support</Text>
            </HStack>
            
            <Text fontSize="sm" color="green.600">
              Our support team can provide personalized assistance for your specific issue.
            </Text>
            
            <VStack align="start" gap={2}>
              <HStack>
                <Text fontSize="sm" color="green.600">Email:</Text>
                <Link href="mailto:support@keepkey.com" color="green.700" fontWeight="medium">
                  support@keepkey.com
                </Link>
              </HStack>
              <HStack>
                <Text fontSize="sm" color="green.600">Website:</Text>
                <Link href="https://support.keepkey.com" target="_blank" color="green.700" fontWeight="medium">
                  support.keepkey.com
                </Link>
              </HStack>
            </VStack>

            <HStack gap={3}>
              <Button 
                colorScheme="green" 
                size="sm"
                onClick={handleContactSupport}
              >
                Contact Support Now
              </Button>
              
              {diagnosticInfo && (
                <Button 
                  variant="outline" 
                  colorScheme="green" 
                  size="sm"
                  onClick={() => navigator.clipboard?.writeText(diagnosticInfo)}
                >
                  Copy Diagnostics
                </Button>
              )}
            </HStack>
          </VStack>
        </Box>
      </VStack>

      {/* Navigation */}
      <HStack justify="space-between" w="full" pt={4}>
        <Button variant="outline" onClick={onPrevious}>
          Back to Advanced Steps
        </Button>
        
        <Button colorScheme="gray" onClick={onNext}>
          Continue to Summary
        </Button>
      </HStack>
    </VStack>
  );
}; 