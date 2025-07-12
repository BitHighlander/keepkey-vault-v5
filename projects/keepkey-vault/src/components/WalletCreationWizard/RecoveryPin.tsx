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
} from "@chakra-ui/react";
import { useState, useCallback, useRef, useEffect } from "react";
import { FaCircle } from "react-icons/fa";
import { PinService } from "../../services/pinService";
import { PinCreationSession, PinStep, PinPosition, PIN_MATRIX_LAYOUT } from "../../types/pin";

interface RecoveryPinProps {
  deviceId: string;
  deviceLabel?: string;
  onComplete: (session: PinCreationSession) => void;
  onBack?: () => void;
  isLoading?: boolean;
  error?: string | null;
}

export function RecoveryPin({ 
  deviceId, 
  deviceLabel, 
  onComplete, 
  onBack, 
  isLoading = false, 
  error 
}: RecoveryPinProps) {
  const [positions, setPositions] = useState<PinPosition[]>([]);
  const [session, setSession] = useState<PinCreationSession | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [stepError, setStepError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  console.log("🔐 RecoveryPin component rendered");

  // Dynamic title and description based on session state
  const getTitle = () => {
    if (!session) return 'Setup PIN for Recovery';
    switch (session.current_step) {
      case PinStep.AwaitingFirst:
        return 'Create Recovery PIN';
      case PinStep.AwaitingSecond:
        return 'Confirm Recovery PIN';
      default:
        return 'Recovery PIN Setup';
    }
  };

  const getDescription = () => {
    if (!session) return 'Creating PIN for your wallet recovery...';
    switch (session.current_step) {
      case PinStep.AwaitingFirst:
        return 'Create a PIN to secure your recovered wallet. Use the layout shown on your device.';
      case PinStep.AwaitingSecond:
        return 'Re-enter your PIN to confirm it matches.';
      default:
        return 'Processing recovery PIN setup...';
    }
  };

  // Initialize PIN creation session for recovery
  useEffect(() => {
    const initializeSession = async () => {
      if (!session) {
        try {
          console.log("🔐 Starting PIN creation for recovery");
          const newSession = await PinService.startPinCreation(deviceId, deviceLabel);
          console.log("🔐 Recovery PIN session created:", newSession);
          setSession(newSession);
          setStepError(null);
        } catch (error) {
          console.error("🔐 Recovery PIN creation initialization error:", error);
          setStepError(`Failed to start recovery PIN creation: ${error}`);
        }
      }
    };

    initializeSession();
  }, [deviceId, deviceLabel, session]);

  useEffect(() => {
    // Focus the hidden input on mount
    if (inputRef.current) {
      inputRef.current.focus();
    }
  }, []);

  const handlePinPress = useCallback((position: PinPosition) => {
    if (positions.length < 8 && !isLoading && !isSubmitting) {
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
        setStepError("Recovery PIN session not initialized. Please try again.");
        return;
      }

      console.log("🔐 Submitting recovery PIN");
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
        console.log("🔐 Recovery PIN response result:", result);
        
        if (result.success) {
          if (result.next_step === 'confirm') {
            console.log("🔐 Moving to PIN confirmation step");
            // Move to confirmation step
            setPositions([]);
            const updatedSession = await PinService.getSessionStatus(session.session_id);
            if (updatedSession) {
              setSession(updatedSession);
            }
          } else if (result.next_step === 'complete') {
            console.log("🔐 Recovery PIN creation completed - calling onComplete");
            // PIN creation completed - call our specific recovery completion handler
            const finalSession = await PinService.getSessionStatus(session.session_id);
            if (finalSession) {
              console.log("🔐 Calling recovery-specific onComplete handler");
              onComplete(finalSession);
            }
          }
        } else {
          setStepError(result.error || 'Recovery PIN setup failed');
        }
      } catch (error) {
        console.error("🔐 Recovery PIN setup error:", error);
        setStepError(`Recovery PIN setup error: ${error}`);
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

  // Generate PIN dots for display
  const maxDotsToShow = Math.max(4, positions.length + (positions.length < 8 ? 1 : 0));
  const pinDots = Array.from({ length: Math.min(maxDotsToShow, 8) }, (_, i) => (
    <Box
      key={i}
      w="12px"
      h="12px"
      borderRadius="full"
      bg={i < positions.length ? "blue.400" : i < 4 ? "gray.500" : "gray.600"}
      opacity={i < positions.length ? 1 : i < 4 ? 0.7 : 0.4}
      transition="all 0.2s"
      transform={i === 3 && positions.length === 4 ? "scale(1.1)" : "scale(1)"}
    />
  ));

  return (
    <div style={{ 
      position: 'fixed', 
      top: 0, 
      left: 0, 
      right: 0, 
      bottom: 0, 
      backgroundColor: 'rgba(0, 0, 0, 0.8)', 
      display: 'flex', 
      alignItems: 'center', 
      justifyContent: 'center', 
      zIndex: 9999 
    }}>
      <Box
        maxW="500px"
        bg="gray.800"
        borderRadius="xl"
        boxShadow="xl"
        borderWidth="1px"
        borderColor="gray.700"
        overflow="hidden"
        w="90%"
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

            {/* PIN Dots Display */}
            <VStack gap={2}>
              <Box
                p={4}
                bg="gray.700"
                borderRadius="lg"
                borderWidth="2px"
                borderColor={positions.length === 4 ? "blue.500" : "gray.600"}
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
                <Text fontSize="xs" color="blue.400" textAlign="center">
                  ✅ Perfect! 4 digits provides great security
                </Text>
              )}
            </VStack>

            {/* Scrambled PIN Grid */}
            <SimpleGrid
              columns={3}
              gap={4}
              w="250px"
            >
              {PIN_MATRIX_LAYOUT.map((position) => (
                <Button
                  key={position}
                  onClick={() => handlePinPress(position as PinPosition)}
                  size="lg"
                  h="60px"
                  bg="gray.700"
                  borderColor="gray.600"
                  borderWidth="1px"
                  color="gray.300"
                  _hover={{
                    bg: "gray.600",
                    borderColor: "blue.500",
                    transform: "scale(1.05)",
                  }}
                  _active={{
                    bg: "gray.500",
                    transform: "scale(0.95)",
                  }}
                  transition="all 0.2s"
                  disabled={isLoading || isSubmitting || positions.length >= 8}
                >
                  <Icon as={FaCircle} boxSize={4} />
                </Button>
              ))}
            </SimpleGrid>

            {/* Hidden input for keyboard support */}
            <Input
              ref={inputRef}
              type="password"
              value={positions.join('')}
              onChange={() => {}}
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
                colorScheme="blue"
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
              >
                {isSubmitting ? 'Processing...' : 
                 session?.current_step === PinStep.AwaitingFirst ? 'Set PIN' : 'Confirm PIN'}
              </Button>
            </HStack>

            <Text fontSize="xs" color="gray.500" textAlign="center">
              PIN layout is scrambled for security. Use the positions shown on your KeepKey device.
            </Text>
          </VStack>
        </Box>
      </Box>
    </div>
  );
} 