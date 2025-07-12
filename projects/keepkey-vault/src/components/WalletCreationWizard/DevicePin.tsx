import {
  Button,
  Text,
  VStack,
  Box,
  HStack,
  SimpleGrid,
  Input,
  Icon,
  Heading,
  Image,
} from "@chakra-ui/react";
import { useState, useCallback, useRef, useEffect } from "react";
import { FaCircle, FaChevronDown, FaChevronRight, FaInfoCircle } from "react-icons/fa";
import cipherImage from "../../assets/onboarding/cipher.png";
import { PinService } from "../../services/pinService";
import { PinCreationSession, PinStep, PinPosition, PIN_MATRIX_LAYOUT } from "../../types/pin";

interface DevicePinProps {
  deviceId: string;
  deviceLabel?: string;
  mode: 'create' | 'confirm';
  onComplete: (session: PinCreationSession) => void;
  onBack?: () => void;
  isLoading?: boolean;
  error?: string | null;
}

export function DevicePin({ deviceId, deviceLabel, mode, onComplete, onBack, isLoading = false, error }: DevicePinProps) {
  const [positions, setPositions] = useState<PinPosition[]>([]);
  const [showMoreInfo, setShowMoreInfo] = useState(false);
  const [session, setSession] = useState<PinCreationSession | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [stepError, setStepError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Dynamic title and description based on session state
  const getTitle = () => {
    if (!session) return 'Initializing PIN Setup...';
    switch (session.current_step) {
      case PinStep.AwaitingFirst:
        return 'Create Your PIN';
      case PinStep.AwaitingSecond:
        return 'Confirm Your PIN';
      default:
        return 'PIN Setup';
    }
  };

  const getDescription = () => {
    if (!session) return 'Starting PIN creation session with your KeepKey device...';
    switch (session.current_step) {
      case PinStep.AwaitingFirst:
        return 'Use PIN layout shown on your device to find the location to press on the PIN pad.';
      case PinStep.AwaitingSecond:
        return 'Re-enter your PIN to confirm it matches.';
      default:
        return 'Processing PIN setup...';
    }
  };

  // Initialize PIN creation session
  useEffect(() => {
    const initializeSession = async () => {
      if (!session && mode === 'create') {
        try {
          const newSession = await PinService.startPinCreation(deviceId, deviceLabel);
          setSession(newSession);
          setStepError(null); // Clear any previous errors
        } catch (error) {
          console.error("PIN creation initialization error:", error);
          setStepError(`Failed to start PIN creation: ${error}`);
        }
      }
    };

    initializeSession();
  }, [deviceId, mode, session]);

  useEffect(() => {
    // Focus the hidden input on mount
    if (inputRef.current) {
      inputRef.current.focus();
    }
  }, []);

  const handlePinPress = useCallback((position: PinPosition) => {
    if (positions.length < 9 && !isLoading && !isSubmitting) {  // Cap at 9 digits
      setPositions(prev => [...prev, position]);
    }
  }, [positions.length, isLoading, isSubmitting]);

  const handleBackspace = useCallback(() => {
    if (!isLoading && !isSubmitting) {
      setPositions(prev => prev.slice(0, -1));
    }
  }, [isLoading, isSubmitting]);

  const handleSubmit = useCallback(async () => {
    if (positions.length > 0 && !isLoading && !isSubmitting) {
      if (!session) {
        setStepError("PIN session not initialized. Please try again.");
        return;
      }

      setIsSubmitting(true);
      setStepError(null);
      
      try {
        const validation = PinService.validatePositions(positions);
        if (!validation.valid) {
          setStepError(validation.error!);
          setIsSubmitting(false);
          return;
        }

        const result = await PinService.sendPinResponse(session.session_id, positions);
        
        if (result.success) {
          if (result.next_step === 'confirm') {
            // Move to confirmation step
            setPositions([]);
            const updatedSession = await PinService.getSessionStatus(session.session_id);
            if (updatedSession) {
              setSession(updatedSession);
            }
          } else if (result.next_step === 'complete') {
            // PIN creation completed
            const finalSession = await PinService.getSessionStatus(session.session_id);
            if (finalSession) {
              onComplete(finalSession);
            }
          }
        } else {
          setStepError(result.error || 'PIN setup failed');
        }
      } catch (error) {
        setStepError(`PIN setup error: ${error}`);
      } finally {
        setIsSubmitting(false);
      }
    }
  }, [positions, isLoading, isSubmitting, session, onComplete]);

  const handleKeyPress = useCallback((e: React.KeyboardEvent) => {
    e.preventDefault();
    
    if (isLoading || isSubmitting) return;

    if (e.key === 'Backspace') {
      handleBackspace();
    } else if (e.key === 'Enter') {
      handleSubmit();
    } else if (PIN_MATRIX_LAYOUT.includes(Number(e.key) as any)) {
      handlePinPress(Number(e.key) as PinPosition);
    }
  }, [handleBackspace, handleSubmit, handlePinPress, isLoading, isSubmitting]);

  // Generate PIN dots for display (show 4 by default, expand as needed)
  const maxDotsToShow = Math.max(4, positions.length + (positions.length < 8 ? 1 : 0));
  const pinDots = Array.from({ length: Math.min(maxDotsToShow, 8) }, (_, i) => (
    <Box
      key={i}
      w="12px"
      h="12px"
      borderRadius="full"
      bg={i < positions.length ? "green.400" : i < 4 ? "gray.500" : "gray.600"}
      opacity={i < positions.length ? 1 : i < 4 ? 0.7 : 0.4}
      transition="all 0.2s"
      transform={i === 3 && positions.length === 4 ? "scale(1.1)" : "scale(1)"}
    />
  ));

  return (
    <HStack 
      w="100%" 
      maxW={showMoreInfo ? "1200px" : "500px"}
      gap={6}
      align="flex-start"
      transition="all 0.3s ease-in-out"
    >
      {/* Main PIN Entry Panel */}
      <Box
        w={showMoreInfo ? "500px" : "100%"}
        bg="gray.800"
        borderRadius="xl"
        boxShadow="xl"
        borderWidth="1px"
        borderColor="gray.700"
        overflow="hidden"
        flexShrink={0}
      >
        <Box bg="gray.850" p={6}>
          <Heading fontSize="2xl" fontWeight="bold" color="white" textAlign="center">
            {getTitle()}
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
              {getDescription()}
            </Text>

            {/* More Info Toggle Button */}
            <Box w="full">
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setShowMoreInfo(!showMoreInfo)}
                color="blue.400"
                _hover={{ color: "blue.300", bg: "gray.700" }}
              >
                <HStack gap={2}>
                  <Icon as={FaInfoCircle} />
                  <Text>More Info: How PIN Security Works</Text>
                  <Icon as={showMoreInfo ? FaChevronDown : FaChevronRight} />
                </HStack>
              </Button>
            </Box>

            {/* PIN Dots Display with 4-digit recommendation */}
            <VStack gap={2}>
              <Box
                p={4}
                bg="gray.700"
                borderRadius="lg"
                borderWidth="2px"
                borderColor={positions.length === 4 ? "green.500" : "gray.600"}
                transition="all 0.2s"
              >
                <HStack gap={2} justify="center">
                  {pinDots}
                </HStack>
              </Box>
              
              {/* PIN Length Hints */}
              {positions.length === 0 && (
                <Text fontSize="xs" color="blue.400" textAlign="center">
                  💡 We recommend using 4 digits for optimal security
                </Text>
              )}
              {positions.length > 0 && positions.length < 4 && (
                <Text fontSize="xs" color="yellow.400" textAlign="center">
                  {4 - positions.length} more digit{4 - positions.length !== 1 ? 's' : ''} recommended
                </Text>
              )}
              {positions.length === 4 && (
                <Text fontSize="xs" color="green.400" textAlign="center">
                  ✅ Perfect! 4 digits provides great security
                </Text>
              )}
              {positions.length > 4 && positions.length < 8 && (
                <Text fontSize="xs" color="orange.400" textAlign="center">
                  {positions.length} digits • {8 - positions.length} more allowed
                </Text>
              )}
              {positions.length === 8 && (
                <Text fontSize="xs" color="red.400" textAlign="center">
                  Maximum length reached (8 digits)
                </Text>
              )}
            </VStack>

            {/* Scrambled PIN Grid */}
            <SimpleGrid
              columns={3}
              gap={2}
              w="200px"
            >
              {PIN_MATRIX_LAYOUT.map((position) => (
                <Button
                  key={position}
                  onClick={() => handlePinPress(position as PinPosition)}
                  size="md"
                  h="50px"
                  bg="gray.700"
                  borderColor="gray.600"
                  borderWidth="1px"
                  color="gray.300"
                  fontSize="lg"
                  fontWeight="bold"
                  _hover={{
                    bg: "gray.600",
                    borderColor: "green.500",
                    transform: "scale(1.05)",
                  }}
                  _active={{
                    bg: "gray.500",
                    transform: "scale(0.95)",
                  }}
                  transition="all 0.15s"
                  disabled={isLoading || isSubmitting || positions.length >= 9}
                >
                  <Icon as={FaCircle} boxSize={3} />
                </Button>
              ))}
            </SimpleGrid>

            {/* Hidden input for keyboard support */}
            <Input
              ref={inputRef}
              type="password"
              value={positions.join('')}
              onChange={() => {}} // Controlled by keyboard handler
              onKeyDown={handleKeyPress}
              style={{
                position: 'absolute',
                left: '-9999px',
                opacity: 0,
                pointerEvents: 'none',
              }}
              autoComplete="off"
            />

            {(error || stepError) && (
              <Text color="red.400" fontSize="sm" textAlign="center">
                {error || stepError}
              </Text>
            )}

            {/* Action Buttons */}
            <HStack gap={4} w="full">
              {onBack && (
                <Button
                  onClick={onBack}
                  variant="outline"
                  size="lg"
                  flex={1}
                  borderColor="gray.600"
                  color="gray.300"
                  fontSize="md"
                  fontWeight="semibold"
                  _hover={{
                    bg: "gray.700",
                    borderColor: "gray.500",
                  }}
                  disabled={isLoading || isSubmitting}
                >
                  Back
                </Button>
              )}
              
              <Button
                onClick={handleBackspace}
                variant="outline"
                size="lg"
                flex={1}
                borderColor="gray.600"
                color="gray.300"
                fontSize="md"
                fontWeight="semibold"
                _hover={{
                  bg: "gray.700",
                  borderColor: "gray.500",
                }}
                disabled={isLoading || isSubmitting || positions.length === 0}
              >
                Clear
              </Button>
              
              <Button
                onClick={handleSubmit}
                colorScheme="green"
                size="lg"
                flex={1}
                fontSize="md"
                fontWeight="semibold"
                _hover={{
                  transform: "translateY(-1px)",
                  boxShadow: "lg",
                }}
                transition="all 0.2s"
                disabled={isLoading || isSubmitting || positions.length === 0}
                loading={isSubmitting}
                loadingText="Processing..."
              >
                {session?.current_step === PinStep.AwaitingFirst ? 'Set PIN' : 'Confirm PIN'}
              </Button>
            </HStack>

            <Text fontSize="xs" color="gray.500" textAlign="center">
              PIN layout is scrambled for security. Use the positions shown on your KeepKey device.
            </Text>
          </VStack>
        </Box>
      </Box>

      {/* Side Info Panel */}
      {showMoreInfo && (
        <Box
          w="600px"
          bg="gray.800"
          borderRadius="xl"
          boxShadow="xl"
          borderWidth="1px"
          borderColor="gray.700"
          overflow="hidden"
          flexShrink={0}
        >
          <Box bg="gray.850" p={6}>
            <Heading fontSize="xl" fontWeight="bold" color="white" textAlign="center">
              KeepKey's PIN Security
            </Heading>
          </Box>
          
          <Box p={6}>
            <VStack gap={4}>
              <Image 
                src={cipherImage} 
                alt="KeepKey PIN Cipher Explanation"
                maxW="100%"
                borderRadius="md"
                border="2px solid"
                borderColor="gray.600"
              />
              
              <VStack gap={3} align="start">
                <Text color="gray.300" fontSize="sm" lineHeight="1.5">
                  <strong>🛡️ Zero-Knowledge Security:</strong> Your KeepKey displays numbers 1-9 in 
                  scrambled positions. You click positions on this screen, but only the device knows 
                  which actual numbers you're entering.
                </Text>
                
                <Text color="gray.300" fontSize="sm" lineHeight="1.5">
                  <strong>🔒 Anti-Keylogging Protection:</strong> Even if malware is monitoring your 
                  computer, it only sees position clicks (like "5, 2, 8") - not your real PIN digits.
                </Text>
                
                <Text color="gray.300" fontSize="sm" lineHeight="1.5">
                  <strong>👀 Anti-Shoulder Surfing:</strong> The scrambled layout changes each session, 
                  so someone watching can't learn your PIN pattern.
                </Text>
                
                <Text color="gray.300" fontSize="sm" lineHeight="1.5">
                  <strong>⚡ Power Analysis Protection:</strong> The display uses constant power 
                  consumption to prevent side-channel attacks that monitor electrical usage.
                </Text>
                
                <Box p={3} bg="blue.900" borderRadius="md" borderWidth="1px" borderColor="blue.600">
                  <Text color="blue.200" fontSize="xs" textAlign="center">
                    <Icon as={FaInfoCircle} mr={2} />
                    Your PIN can be 1-9 digits long. We recommend at least 4 digits for security.
                  </Text>
                </Box>
              </VStack>
            </VStack>
          </Box>
        </Box>
      )}
    </HStack>
  );
} 