import React, { useState, useEffect } from 'react';
import { 
  Box, 
  Heading, 
  Text, 
  VStack, 
  HStack, 
  Button, 
  IconButton,
  Flex,
  Spinner, // Only if used in JSX
  Code,
  Badge
} from '@chakra-ui/react';
import { FaArrowLeft, FaCopy, FaCheck, FaEye, FaChevronDown, FaChevronUp } from 'react-icons/fa';
import { SiBitcoin } from 'react-icons/si'; // Only if used in JSX
import QRCode from 'react-qr-code';
import { useWallet } from '../contexts/WalletContext';

interface ReceiveProps {
  onBack?: () => void;
}

const Receive: React.FC<ReceiveProps> = ({ onBack }) => {
  const { portfolio, getReceiveAddress, selectAsset, loading: walletLoading, selectedAsset, fetchedXpubs } = useWallet();
  
  const [address, setAddress] = useState<string>('');
  const [hasCopied, setHasCopied] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [shouldGenerate, setShouldGenerate] = useState(false);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [addressType, setAddressType] = useState<'legacy' | 'segwit' | 'native-segwit'>('native-segwit');
  const [derivationPath, setDerivationPath] = useState<string>("m/84'/0'/0'/0/0");

  // Component lifecycle debugging
  useEffect(() => {
    console.log('🚀 Receive component mounted');
    return () => {
      console.log('🛬 Receive component unmounting');
    };
  }, []);

  // Get BTC asset from portfolio
  const btcAsset = portfolio?.assets.find(asset => asset.caip === 'bip122:000000000019d6689c085ae165831e93/slip44:0');

  // Debug portfolio and BTC asset
  useEffect(() => {
    console.log('🔍 Receive component - Portfolio:', portfolio);
    console.log('🔍 Receive component - All assets:', portfolio?.assets);
    console.log('🔍 Receive component - BTC Asset:', btcAsset);
    if (portfolio?.assets) {
      console.log('🔍 Receive component - Asset CAIPs:', portfolio.assets.map(a => ({ symbol: a.symbol, caip: a.caip })));
    }
  }, [portfolio, btcAsset]);

  // Generate receive address
  const generateAddress = () => {
    if (!btcAsset) {
      setError('Bitcoin asset not found in portfolio. Please sync your device or check your wallet setup.');
      return;
    }
    setLoading(true);
    setError(null);
    setAddress(''); // Clear any previous address before new generation
    selectAsset(btcAsset);
    
    // Set address type and derivation path based on selected type
    // For now, defaulting to native segwit but this could be made configurable
    setAddressType('native-segwit');
    setDerivationPath("m/84'/0'/0'/0/0");
    
    setShouldGenerate(true); // Triggers useEffect below
  };

  useEffect(() => {
    if (shouldGenerate && btcAsset) {
      (async () => {
        try {
          console.log('🎯 Starting address generation...');
          const addr = await getReceiveAddress();
          console.log('📬 Received address from getReceiveAddress:', addr);
          if (addr) {
            setAddress(addr);
            setError(null);
            console.log('✅ Address set in state:', addr);
          } else {
            console.error('❌ No address returned from getReceiveAddress');
            setError('Failed to generate address - no response from device');
          }
        } catch (e) {
          console.error('❌ Error in address generation:', e);
          setError(
            e instanceof Error ? e.message : 'Failed to generate receive address. Ensure your device is connected and unlocked.'
          );
        } finally {
          setLoading(false);
          setShouldGenerate(false);
        }
      })();
    }
  }, [shouldGenerate, btcAsset, getReceiveAddress]);

  // Copy address to clipboard
  const onCopy = () => {
    if (address) {
      navigator.clipboard.writeText(address);
      setHasCopied(true);
      setTimeout(() => setHasCopied(false), 2000);
    }
  };

  // Monitor address state changes
  useEffect(() => {
    console.log('📍 Address state changed:', address);
  }, [address]);

  // Log render branch
  useEffect(() => {
    if (loading) {
      console.log('[RENDER] Receive: loading spinner branch');
    } else if (!address) {
      console.log('[RENDER] Receive: no address branch');
    } else {
      console.log('[RENDER] Receive: address display branch, address:', address);
    }
  }, [loading, address]);

  // Get relevant xpub for the current address type
  const getRelevantXpub = () => {
    if (!fetchedXpubs || fetchedXpubs.length === 0) return null;
    
    // Map address type to derivation path prefix
    const pathPrefix = addressType === 'legacy' ? "m/44'/0'/0'" :
                      addressType === 'segwit' ? "m/49'/0'/0'" :
                      "m/84'/0'/0'"; // native-segwit
    
    return fetchedXpubs.find(xpub => xpub.path === pathPrefix);
  };

  const relevantXpub = getRelevantXpub();

  if (walletLoading) {
    return (
      <Box height="100%" display="flex" alignItems="center" justifyContent="center" bg="transparent">
        <VStack gap={4}>
          <Spinner size="xl" color="blue.400" />
          <Text color="gray.300" fontSize="lg">Loading Wallet...</Text>
        </VStack>
      </Box>
    );
  }

  return (
    <Box height="100%" bg="transparent" display="flex" alignItems="center" justifyContent="center" p={6}>
      <Box 
        maxW="500px" 
        w="100%"
        bg="rgba(26, 32, 44, 0.95)" 
        p={6} 
        borderRadius="xl" 
        backdropFilter="blur(20px)"
        border="1px solid rgba(255, 255, 255, 0.1)"
        boxShadow="2xl"
      >
        <VStack align="stretch" gap={6}>
          {/* Header */}
          <HStack>
            <IconButton
              aria-label="Go back"
              onClick={onBack}
              size="sm"
            >
              <FaArrowLeft />
            </IconButton>
            <Flex align="center" justify="center" flex="1" gap={2}>
              <Box color="orange.400" fontSize="xl">
                <SiBitcoin />
              </Box>
              <Heading size="lg" color="white">
                Receive Bitcoin
              </Heading>
            </Flex>
            <Box w="40px" /> {/* Spacer for centering */}
          </HStack>

          {/* Main Content */}
          <VStack gap={6} bg="gray.800" p={6} borderRadius="lg" minH="400px" justify="center">
            {loading ? (
              /* Show spinner and message while loading, regardless of address state */
              <VStack gap={4} textAlign="center">
                <Spinner size="xl" color="blue.400" />
                <Text color="gray.300" fontSize="lg">Generating Address...</Text>
                {error && (
                  <Box bg="red.900" p={3} borderRadius="md" border="1px solid" borderColor="red.600">
                    <Text color="red.200" fontSize="sm">⚠️ {error}</Text>
                  </Box>
                )}
              </VStack>
            ) : !address ? (
              /* Generate Address View */
              <VStack gap={4} textAlign="center">
                <Box color="orange.400" fontSize="4xl">
                  <SiBitcoin />
                </Box>
                <Text color="gray.300" fontSize="lg" fontWeight="medium">
                  Generate Receive Address
                </Text>
                <Text color="gray.400" fontSize="sm" textAlign="center" maxW="300px">
                  Generate a Bitcoin address to receive payments. This address will be linked to your KeepKey device.
                </Text>
                {error && (
                  <Box bg="red.900" p={3} borderRadius="md" border="1px solid" borderColor="red.600">
                    <Text color="red.200" fontSize="sm">⚠️ {error}</Text>
                  </Box>
                )}
                <Button
                  colorScheme="blue"
                  size="lg"
                  onClick={generateAddress}
                  disabled={loading || !btcAsset}
                  minW="200px"
                >
                  <HStack gap={2}>
                    <FaEye />
                    <Text>Generate Address</Text>
                  </HStack>
                </Button>
                {!btcAsset && (
                  <Text color="yellow.400" fontSize="xs" textAlign="center">
                    No Bitcoin asset found in portfolio
                  </Text>
                )}
              </VStack>
            ) : (
              /* Address Display View */
              <VStack gap={4} w="100%">
                {/* Success Header */}
                <VStack gap={2} textAlign="center">
                  <Text color="green.300" fontSize="lg" fontWeight="bold">
                    ✅ Receive Address Generated
                  </Text>
                  <Text color="gray.400" fontSize="sm">
                    Use this address to receive Bitcoin payments
                  </Text>
                </VStack>

                {/* QR Code */}
                <Box
                  w="200px"
                  h="200px"
                  bg="white"
                  borderRadius="md"
                  display="flex"
                  alignItems="center"
                  justifyContent="center"
                  mx="auto"
                  p={2}
                >
                  <QRCode
                    value={address}
                    size={184}
                    style={{ height: "auto", maxWidth: "100%", width: "100%" }}
                    viewBox={`0 0 184 184`}
                  />
                </Box>

                {/* Address String */}
                <VStack w="100%" gap={2}>
                  <Text color="gray.300" fontSize="sm" fontWeight="medium">
                    Bitcoin Address
                  </Text>
                  <Box
                    w="100%"
                    bg="gray.700"
                    p={3}
                    borderRadius="md"
                    border="1px solid"
                    borderColor="gray.600"
                  >
                    <Text
                      color="white"
                      fontSize="sm"
                      fontFamily="mono"
                      wordBreak="break-all"
                      textAlign="center"
                    >
                      {address}
                    </Text>
                  </Box>
                </VStack>

                {/* Advanced View Toggle */}
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => setShowAdvanced(!showAdvanced)}
                  color="gray.400"
                  _hover={{ color: "gray.200" }}
                  mt={2}
                >
                  <HStack gap={1}>
                    <Text fontSize="xs">Advanced View</Text>
                    {showAdvanced ? <FaChevronUp /> : <FaChevronDown />}
                  </HStack>
                </Button>

                {/* Advanced View Content */}
                {showAdvanced && (
                  <VStack
                    w="100%"
                    gap={4}
                    bg="gray.900"
                    p={4}
                    borderRadius="md"
                    border="1px solid"
                    borderColor="gray.700"
                    mt={3}
                  >
                    <VStack align="stretch" w="100%" gap={3}>
                      {/* Address Type */}
                      <HStack justify="space-between">
                        <Text color="gray.400" fontSize="xs">Address Type:</Text>
                        <Badge colorScheme="blue" fontSize="xs">
                          {addressType === 'legacy' ? 'Legacy (P2PKH)' :
                           addressType === 'segwit' ? 'SegWit (P2SH-P2WPKH)' :
                           'Native SegWit (P2WPKH)'}
                        </Badge>
                      </HStack>

                      <Box w="100%" h="1px" bg="gray.700" />

                      {/* Derivation Path */}
                      <VStack align="stretch" gap={1}>
                        <Text color="gray.400" fontSize="xs">Derivation Path:</Text>
                        <Code
                          bg="gray.800"
                          color="blue.300"
                          p={2}
                          borderRadius="md"
                          fontSize="xs"
                        >
                          {derivationPath}
                        </Code>
                      </VStack>

                      <Box w="100%" h="1px" bg="gray.700" />

                      {/* Extended Public Key (if available) */}
                      {relevantXpub && (
                        <VStack align="stretch" gap={1}>
                          <Text color="gray.400" fontSize="xs">Account Extended Public Key:</Text>
                          <Code
                            bg="gray.800"
                            color="gray.300"
                            p={2}
                            borderRadius="md"
                            fontSize="xs"
                            wordBreak="break-all"
                          >
                            {relevantXpub.xpub.substring(0, 50)}...
                          </Code>
                          <Text color="gray.500" fontSize="xs" mt={1}>
                            Path: {relevantXpub.path}
                          </Text>
                        </VStack>
                      )}

                      <Box w="100%" h="1px" bg="gray.700" />

                      {/* Script Type Info */}
                      <VStack align="stretch" gap={1}>
                        <Text color="gray.400" fontSize="xs">Script Type:</Text>
                        <Text color="gray.300" fontSize="xs">
                          {addressType === 'legacy' ? 'Pay-to-Public-Key-Hash (P2PKH)' :
                           addressType === 'segwit' ? 'Pay-to-Script-Hash wrapped Witness (P2SH-P2WPKH)' :
                           'Pay-to-Witness-Public-Key-Hash (P2WPKH)'}
                        </Text>
                      </VStack>

                      <Box w="100%" h="1px" bg="gray.700" />

                      {/* Address Format Info */}
                      <HStack justify="space-between">
                        <Text color="gray.400" fontSize="xs">Address Format:</Text>
                        <Text color="gray.300" fontSize="xs">
                          {addressType === 'legacy' ? 'Starts with 1' :
                           addressType === 'segwit' ? 'Starts with 3' :
                           'Starts with bc1'}
                        </Text>
                      </HStack>

                      {/* Device Info */}
                      <Box w="100%" h="1px" bg="gray.700" />
                      <VStack align="stretch" gap={1}>
                        <Text color="gray.400" fontSize="xs">Generated By:</Text>
                        <HStack>
                          <Badge colorScheme="green" fontSize="xs">KeepKey Device</Badge>
                          <Text color="gray.500" fontSize="xs">Hardware Wallet</Text>
                        </HStack>
                      </VStack>
                    </VStack>

                    {/* Info Note */}
                    <Box bg="blue.900" p={3} borderRadius="md" border="1px solid" borderColor="blue.700" mt={2}>
                      <Text color="blue.200" fontSize="xs">
                        💡 This address is derived from your hardware wallet's seed phrase using BIP44/49/84 standards. 
                        Each address is unique and can only be spent by your KeepKey device.
                      </Text>
                    </Box>
                  </VStack>
                )}

                {/* Action Buttons */}
                <HStack gap={3} w="100%">
                  <Button
                    colorScheme="blue"
                    size="lg"
                    onClick={onCopy}
                    flex="1"
                    bg={hasCopied ? "green.600" : "blue.600"}
                    _hover={{ bg: hasCopied ? "green.500" : "blue.500" }}
                  >
                    <HStack gap={2}>
                      {hasCopied ? <FaCheck /> : <FaCopy />}
                      <Text>{hasCopied ? 'Copied!' : 'Copy Address'}</Text>
                    </HStack>
                  </Button>
                  
                  <Button
                    variant="outline"
                    colorScheme="gray"
                    size="lg"
                    onClick={() => {
                      setAddress('');
                      setError(null);
                    }}
                    flex="1"
                  >
                    Generate New
                  </Button>
                </HStack>

                {/* Security Notice */}
                <Box bg="blue.900" p={3} borderRadius="md" border="1px solid" borderColor="blue.600">
                  <Text color="blue.200" fontSize="xs" textAlign="center">
                    🔒 This address was generated by your KeepKey device and is safe to use for receiving Bitcoin.
                  </Text>
                </Box>
              </VStack>
            )}
          </VStack>

          {/* Back Button */}
          <Button
            variant="ghost"
            color="gray.400"
            onClick={onBack}
            alignSelf="center"
          >
            Back to Vault
          </Button>
        </VStack>
      </Box>
    </Box>
  );
};

export default Receive; 