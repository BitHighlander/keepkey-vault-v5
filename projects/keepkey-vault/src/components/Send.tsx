import React, { useState, useEffect } from 'react';
import { 
  Box, 
  Heading, 
  Text, 
  VStack, 
  HStack, 
  Button, 
  Input, 
  Flex,
  IconButton,
  Spinner,
  Badge,
  Code
} from '@chakra-ui/react';
import { FaArrowLeft, FaQrcode, FaPaperPlane, FaEye, FaSignature, FaCheck, FaChevronUp, FaChevronDown } from 'react-icons/fa';
import { SiBitcoin } from 'react-icons/si';
import { useWallet } from '../contexts/WalletContext';
import { PioneerAPI, DeviceQueueAPI } from '../lib/api';
import { createUnsignedUxtoTx } from '../lib/createUnsignedUxtoTx';

interface SendPageProps {
  onBack: () => void;
}

interface FeeRates {
  slow: number;
  medium: number;
  fast: number;
}

type SendStep = 'compose' | 'review' | 'sign' | 'complete';

interface TransactionReview {
  to: string;
  amount: string;
  amountInSats: number;
  fee: number;
  feeRate: number;
  total: number;
  inputs: any[];
  outputs: any[];
  unsignedTx: any;
}

const Send: React.FC<SendPageProps> = ({ onBack }) => {
  const { portfolio, loading: walletLoading, error: walletError, selectAsset, selectedAsset, signTransaction, fetchedXpubs } = useWallet();
  
  // Step management
  const [currentStep, setCurrentStep] = useState<SendStep>('compose');
  const [transactionReview, setTransactionReview] = useState<TransactionReview | null>(null);
  const [signedTransaction, setSignedTransaction] = useState<string | null>(null);
  
  // Compose step state
  const [recipientAddress, setRecipientAddress] = useState('');
  const [amount, setAmount] = useState('');
  const [feeRate, setFeeRate] = useState<'slow' | 'medium' | 'fast'>('medium');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [addressValidation, setAddressValidation] = useState<{ valid: boolean; error?: string }>({ valid: false });
  const [amountCurrency, setAmountCurrency] = useState<'BTC' | 'USD'>('BTC');
  const [btcPrice, setBtcPrice] = useState<number>(43000);
  const [isMaxSend, setIsMaxSend] = useState(false);
  const [isShowingHex, setIsShowingHex] = useState(false); // Hex collapsed by default
  
  // Broadcast state
  const [isBroadcasting, setIsBroadcasting] = useState(false);
  const [broadcastSuccess, setBroadcastSuccess] = useState(false);
  const [broadcastError, setBroadcastError] = useState<string | null>(null);
  const [txid, setTxid] = useState<string | null>(null);
  
  // Fee-related state
  const [feeRates, setFeeRates] = useState<FeeRates>({ slow: 1, medium: 5, fast: 10 }); // Fallback rates
  const [loadingFees, setLoadingFees] = useState(true);
  const [feeError, setFeeError] = useState<string | null>(null);

  // Get BTC balance from portfolio (sum all BTC assets like Portfolio component does)
  const btcAssets = portfolio?.assets.filter(asset => 
    asset.caip === 'bip122:000000000019d6689c085ae165831e93/slip44:0'
  ) || [];
  const availableBalance = btcAssets.reduce((sum, asset) => sum + parseFloat(asset.balance), 0);
  
  // Debug logging to help troubleshoot balance issues
  console.log('🔍 Send component balance debug:', {
    walletLoading,
    portfolio: portfolio ? 'loaded' : 'null',
    totalAssets: portfolio?.assets.length || 0,
    allAssetCAIPs: portfolio?.assets.map(a => a.caip) || [],
    btcAssets: btcAssets.length,
    availableBalance,
    btcAssetsDetails: btcAssets.map(a => ({ caip: a.caip, balance: a.balance, symbol: a.symbol })),
    expectedCAIP: 'bip122:000000000019d6689c085ae165831e93/slip44:0'
  });
  
  // Get first BTC asset for price info (they should all have same price)
  const btcAsset = btcAssets[0] || null;

  // Load fee rates from Pioneer API
  useEffect(() => {
    const loadFeeRates = async () => {
      if (btcAssets.length === 0) return;
      
      try {
        setLoadingFees(true);
        setFeeError(null);
        
        console.log('💰 Loading real fee rates from Pioneer for:', btcAssets[0].caip);
        const feeData = await PioneerAPI.getFeeRates(btcAssets[0].caip);
        
        const realFeeRates: FeeRates = {
          slow: feeData.average || 1,
          medium: feeData.fast || 5,
          fast: feeData.fastest || 10
        };
        
        setFeeRates(realFeeRates);
        console.log('✅ Real fee rates loaded:', realFeeRates);
        
      } catch (error) {
        console.warn('⚠️ Failed to load real fee rates, using fallback:', error);
        setFeeError('Unable to load current fee rates, using estimates');
        // Keep fallback rates
      } finally {
        setLoadingFees(false);
      }
    };

    loadFeeRates();
  }, [btcAssets.length]);

  // Load initial data
  useEffect(() => {
    // Set BTC price from portfolio if available
    if (btcAsset && btcAsset.price_usd) {
      console.log('🔍 Setting BTC price from portfolio:', btcAsset.price_usd);
      setBtcPrice(btcAsset.price_usd);
    } else {
      console.log('🔍 BTC price not available, using default:', btcPrice);
    }
    
    // Ensure BTC asset is selected for sending
    if (btcAssets.length > 0 && btcAsset) {
      selectAsset(btcAsset);
    }
  }, [btcAsset, btcAssets.length, selectAsset]);

  // Validate address when it changes
  useEffect(() => {
    if (recipientAddress) {
      // Simple Bitcoin address validation (basic check)
      const isValidAddress = /^[13][a-km-zA-HJ-NP-Z1-9]{25,34}$|^bc1[a-z0-9]{39,59}$/.test(recipientAddress);
      setAddressValidation({ 
        valid: isValidAddress,
        error: isValidAddress ? undefined : 'Invalid Bitcoin address format'
      });
    } else {
      setAddressValidation({ valid: false });
    }
  }, [recipientAddress]);

  const handleMaxAmount = () => {
    if (availableBalance > 0) {
      // For max send, coinSelectSplit will handle the calculation automatically
      // We still show an estimated amount in the UI for user feedback
      const selectedFeeRate = feeRates[feeRate];
      const estimatedFeeInBtc = (250 * selectedFeeRate) / 100000000; // Assume ~250 vBytes for typical tx
      const maxAmountInBtc = Math.max(0, availableBalance - estimatedFeeInBtc);
      
      // Convert to display currency
      if (amountCurrency === 'USD') {
        const maxAmountInUsd = convertBtcToUsd(maxAmountInBtc);
        setAmount(maxAmountInUsd.toFixed(2));
      } else {
        setAmount(maxAmountInBtc.toFixed(8));
      }
      
      // Mark this as a max send
      setIsMaxSend(true);
      console.log('💰 Max button clicked - will use coinSelectSplit for max send');
    }
  };

  // Step navigation helpers
  const getStepNumber = (step: SendStep): number => {
    switch (step) {
      case 'compose': return 1;
      case 'review': return 2;
      case 'sign': return 3;
      case 'complete': return 4;
      default: return 1;
    }
  };

  const getStepTitle = (step: SendStep): string => {
    switch (step) {
      case 'compose': return 'Compose Transaction';
      case 'review': return 'Review Details';
      case 'sign': return 'Sign Transaction';
      case 'complete': return 'Transaction Complete';
      default: return 'Compose Transaction';
    }
  };

  const canGoBack = (): boolean => {
    return currentStep !== 'compose' && currentStep !== 'complete';
  };

  const handleStepBack = () => {
    if (currentStep === 'review') {
      setCurrentStep('compose');
      setError(null);
    } else if (currentStep === 'sign') {
      setCurrentStep('review');
      setError(null);
    }
  };

  const handleBuildTransaction = async () => {
    if (!recipientAddress || !amount || !addressValidation.valid) {
      setError('Please enter a valid recipient address and amount');
      return;
    }

    const inputAmount = parseFloat(amount);
    if (inputAmount <= 0) {
      setError('Amount must be greater than 0');
      return;
    }

    // Convert to BTC for validation if in USD mode
    const sendAmountInBtc = amountCurrency === 'USD' ? convertUsdToBtc(inputAmount) : inputAmount;
    
    console.log('🔍 Send validation debug:', {
      inputAmount,
      amountCurrency,
      sendAmountInBtc,
      availableBalance,
      btcPrice,
      wouldPass: sendAmountInBtc <= availableBalance
    });
    
    if (sendAmountInBtc > availableBalance) {
      const availableInDisplayCurrency = amountCurrency === 'USD' 
        ? convertBtcToUsd(availableBalance) 
        : availableBalance;
      setError(`Insufficient balance. Available: ${availableInDisplayCurrency.toFixed(amountCurrency === 'USD' ? 2 : 8)} ${amountCurrency}`);
      return;
    }

    try {
      setLoading(true);
      setError(null);
      setSuccess(null);

      console.log('🔄 Building transaction...');

      const selectedFeeRateValue = feeRates[feeRate];
      const estimatedFee = (250 * selectedFeeRateValue) / 100000000; // ~250 vBytes typical tx
      const amountInSats = Math.round(sendAmountInBtc * 100000000);
      const feeInSats = Math.round(estimatedFee * 100000000);
      
      const review: TransactionReview = {
        to: recipientAddress,
        amount: `${sendAmountInBtc.toFixed(8)} BTC`,
        amountInSats,
        fee: estimatedFee,
        feeRate: selectedFeeRateValue,
        total: sendAmountInBtc + estimatedFee,
        inputs: [], // Will be populated by createUnsignedUxtoTx
        outputs: [
          { address: recipientAddress, amount: amountInSats }
        ],
        unsignedTx: null // Will be populated by createUnsignedUxtoTx
      };

      setTransactionReview(review);
      setCurrentStep('review');
      setError(null);

    } catch (error) {
      console.error('Error building transaction:', error);
      setError(error instanceof Error ? error.message : 'Failed to build transaction');
    } finally {
      setLoading(false);
    }
  };

  const handleSignTransaction = async () => {
    if (!transactionReview) {
      setError('No transaction to sign');
      return;
    }

    try {
      setLoading(true);
      setError(null);
      
      setCurrentStep('sign');
      
            console.log('🔐 Building and signing real Bitcoin transaction with proper UTXO selection...');
      
      // Get the connected device
      const connectedDevices = await DeviceQueueAPI.getConnectedDevices();
      if (!connectedDevices || connectedDevices.length === 0) {
        throw new Error('No KeepKey device connected');
      }
      
      const device = connectedDevices[0].device || connectedDevices[0];
      const deviceId = device.unique_id;
console.debug('[Send] deviceId from device.unique_id:', deviceId);
      
      console.log('🔑 Using device:', deviceId);
      
      // Get ALL Bitcoin xpubs (legacy, segwit, native segwit)
      const btcNetworkId = "bip122:000000000019d6689c085ae165831e93";
      const btcXpubs = fetchedXpubs.filter(x => 
        x.caip && x.caip.includes(btcNetworkId)
      );
      
      if (!btcXpubs || btcXpubs.length === 0) {
        throw new Error('No Bitcoin xpubs found. Please sync your wallet first.');
      }
      
      console.log('💰 Using ALL Bitcoin xpubs:', btcXpubs.length, 'found');
      btcXpubs.forEach((xpub, i) => {
        const prefix = xpub.xpub.substring(0, 4);
        const scriptType = prefix === 'xpub' ? 'p2pkh' : prefix === 'ypub' ? 'p2sh-p2wpkh' : prefix === 'zpub' ? 'p2wpkh' : 'unknown';
        console.log(`   ${i+1}. ${prefix}... (${scriptType}) - CAIP: ${xpub.caip}`);
      });
      
      // Prepare pubkeys array for createUnsignedUxtoTx with ALL Bitcoin xpubs
      const pubkeys = btcXpubs.map(btcXpub => {
        const prefix = btcXpub.xpub.substring(0, 4);
        let scriptType = 'p2pkh'; // Default to legacy
        
        if (prefix === 'ypub') {
          scriptType = 'p2sh-p2wpkh'; // P2WPKH nested in P2SH
        } else if (prefix === 'zpub') {
          scriptType = 'p2wpkh'; // Native SegWit
        }
        
        return {
          pubkey: btcXpub.xpub,
          xpub: btcXpub.xpub,
          networks: ['bip122:000000000019d6689c085ae165831e93'],
          scriptType: scriptType
        };
      });
      
      // Create Pioneer API client that uses our already-loaded fee rates
      const pioneer = {
        async ListUnspent({ network, xpub }: { network: string; xpub: string }) {
          const data = await PioneerAPI.listUnspent('Bitcoin', xpub);
          return { data };
        },
        async GetChangeAddress({ network, xpub }: { network: string; xpub: string }) {
          const data = await PioneerAPI.getChangeAddress('Bitcoin', xpub);
          return { data };
        },
        async GetFeeRate({ networkId }: { networkId: string }) {
          // Use our already-loaded fee rates instead of fetching again
          console.log('💰 Using pre-loaded fee rates instead of fetching again');
          const selectedFeeRateValue = feeRates[feeRate];
          return { 
            data: {
              slow: feeRates.slow,
              average: feeRates.medium, 
              fastest: feeRates.fast
            }
          };
        }
      };
      
                    // Use the proper transaction builder with real input selection
       console.log('⚙️ Building transaction with proper coinselect algorithm...');
        const sendAmountBtc = amountCurrency === 'USD' ? convertUsdToBtc(parseFloat(amount)) : parseFloat(amount); // Amount in BTC (not satoshis)
      
      console.log(`💰 Transaction type: ${isMaxSend ? 'MAX SEND (coinSelectSplit)' : 'REGULAR SEND (coinSelect)'}`);
      console.log(`💰 Send amount: ${sendAmountBtc} BTC (${isMaxSend ? 'will be calculated by coinSelectSplit' : 'fixed amount'})`);
      
      
      const unsignedTx = await createUnsignedUxtoTx(
        btcXpubs[0].caip, // Use first Bitcoin CAIP (any will work since they're all Bitcoin)
        recipientAddress,
        sendAmountBtc,
        '', // memo (empty for now)
        pubkeys,
        pioneer,
        null, // keepKeySdk (not needed for signing)
        isMaxSend // Use tracked max send state instead of hardcoded false
      );
      
      console.log('📊 Transaction built by coinselect:');
      console.log(`   Inputs: ${unsignedTx.inputs.length} UTXOs`);
      console.log(`   Outputs: ${unsignedTx.outputs.length} outputs`);
      console.log(`   Total input value: ${unsignedTx.inputs.reduce((sum: number, input: any) => sum + parseInt(input.amount), 0)} sats`);
      console.log(`   Total output value: ${unsignedTx.outputs.reduce((sum: number, output: any) => sum + parseInt(output.amount), 0)} sats`);
      
      // Transform to format expected by our device signing API
      const realInputs = unsignedTx.inputs.map((input: any) => ({
        address_n_list: input.addressNList,
        script_type: input.scriptType,
        amount: input.amount,
        vout: input.vout,
        txid: input.txid,
        prev_tx_hex: input.hex
      }));
      
      const realOutputs = unsignedTx.outputs.map((output: any) => ({
        address: output.address,
        amount: parseInt(output.amount),
        address_type: output.addressType === 'change' ? 'change' : 'spend',
        script_type: output.scriptType || 'p2pkh',
        address_n_list: output.addressNList
      }));
      
      // Sign the transaction using real device with properly selected UTXOs
      console.log('🔐 Calling signTransaction with device queue...');
      console.log('🔐 Request ID will be:', `sign_tx_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`);
      
      let signedTxHex: string;
      try {
        signedTxHex = await signTransaction(
          deviceId,
          unsignedTx.coin,
          realInputs,
          realOutputs,
          unsignedTx.version,
          unsignedTx.locktime
        );
        
        console.log('✅ SEND COMPONENT: signTransaction promise resolved successfully!');
        console.log('🔐 SEND COMPONENT: Signed transaction hex received:');
        console.log(`   Type: ${typeof signedTxHex}`);
        console.log(`   Length: ${signedTxHex ? signedTxHex.length : 'null/undefined'} characters`);
        console.log(`   Full hex: "${signedTxHex}"`);
        
        if (!signedTxHex || signedTxHex.length === 0) {
          throw new Error('Received empty or null signed transaction hex');
        }
        
      } catch (promiseError) {
        console.error('❌ SEND COMPONENT: signTransaction promise rejected:', promiseError);
        throw promiseError;
      }
      
      setSignedTransaction(signedTxHex);
      setCurrentStep('complete');
      setSuccess('Transaction signed successfully!');

    } catch (error) {
      console.error('Error signing transaction:', error);
      setError(error instanceof Error ? error.message : 'Failed to sign transaction');
      setCurrentStep('review'); // Go back to review on error
    } finally {
      setLoading(false);
    }
  };

  const handleNewTransaction = () => {
    // Reset all state for new transaction
    setCurrentStep('compose');
    setTransactionReview(null);
    setSignedTransaction(null);
    setRecipientAddress('');
    setAmount('');
    setError(null);
    setSuccess(null);
    setIsMaxSend(false);
    // Reset broadcast state
    setIsBroadcasting(false);
    setBroadcastSuccess(false);
    setBroadcastError(null);
    setTxid(null);
  };

  const handleBroadcastTransaction = async () => {
    if (!signedTransaction) {
      setBroadcastError('No signed transaction to broadcast');
      return;
    }

    try {
      setIsBroadcasting(true);
      setBroadcastError(null);
      
      console.log('📡 Broadcasting signed transaction to Bitcoin network...');
      
      // Use the Bitcoin network ID for Pioneer API
      const networkId = 'bip122:000000000019d6689c085ae165831e93';
      
      const result = await PioneerAPI.broadcastTransaction(networkId, signedTransaction);
      
      console.log('✅ Transaction broadcast successful!');
      console.log('🆔 Transaction ID:', result.txid);
      
      setTxid(result.txid);
      setBroadcastSuccess(true);
      setSuccess('Transaction broadcast successfully! ✅');
      
    } catch (error) {
      console.error('❌ Transaction broadcast failed:', error);
      const errorMessage = error instanceof Error ? error.message : 'Unknown broadcast error';
      setBroadcastError(errorMessage);
      setError(`Broadcast failed: ${errorMessage}`);
    } finally {
      setIsBroadcasting(false);
    }
  };

  const copyTxidToClipboard = async () => {
    if (txid) {
      try {
        await navigator.clipboard.writeText(txid);
        console.log('📋 TXID copied to clipboard:', txid);
        // Temporary success feedback
        const originalSuccess = success;
        setSuccess('TXID copied to clipboard! 📋');
        setTimeout(() => setSuccess(originalSuccess), 2000);
      } catch (error) {
        console.error('Failed to copy TXID to clipboard:', error);
        setError('Failed to copy TXID to clipboard');
      }
    }
  };

  // Currency conversion helpers
  const convertBtcToUsd = (btc: number): number => btc * btcPrice;
  const convertUsdToBtc = (usd: number): number => usd / btcPrice;
  
  const formatCurrency = (value: number, currency: 'BTC' | 'USD'): string => {
    if (currency === 'BTC') {
      return value.toFixed(8);
    } else {
      return value.toFixed(2);
    }
  };

  const toggleCurrency = () => {
    const currentAmountNum = parseFloat(amount) || 0;
    let convertedAmount: number;
    
    if (amountCurrency === 'BTC') {
      // Converting from BTC to USD
      convertedAmount = convertBtcToUsd(currentAmountNum);
      setAmountCurrency('USD');
    } else {
      // Converting from USD to BTC
      convertedAmount = convertUsdToBtc(currentAmountNum);
      setAmountCurrency('BTC');
    }
    
    setAmount(formatCurrency(convertedAmount, amountCurrency === 'BTC' ? 'USD' : 'BTC'));
  };

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

  const renderStepContent = () => {
    switch (currentStep) {
      case 'compose':
        return renderComposeStep();
      case 'review':
        return renderReviewStep();
      case 'sign':
        return renderSignStep();
      case 'complete':
        return renderCompleteStep();
      default:
        return renderComposeStep();
    }
  };

  const renderComposeStep = () => {
    return (
      <VStack gap={4}>
        {/* Compact Balance Display */}
        <Box bg="gray.800" p={3} borderRadius="md" textAlign="center">
          <Text color="gray.400" fontSize="xs">Available</Text>
          <Text color="white" fontSize="lg" fontWeight="bold">
            {availableBalance.toFixed(8)} BTC
          </Text>
          <Text color="gray.400" fontSize="xs">
            ≈ ${convertBtcToUsd(availableBalance).toFixed(2)} USD
          </Text>
          {/* Show balance warning if zero */}
          {availableBalance === 0 && (
            <Text color="yellow.400" fontSize="xs" mt={1}>
              ⚠️ No balance available. Please ensure your wallet is synced.
            </Text>
          )}
        </Box>

        {/* Compact Send Form */}
        <VStack gap={3} bg="gray.800" p={4} borderRadius="md">
          {/* Recipient Address */}
          <Box w="100%">
            <Text color="gray.300" mb={2} fontSize="sm" fontWeight="medium">
              Recipient Address
            </Text>
            <HStack>
              <Input
                placeholder="Enter Bitcoin address"
                value={recipientAddress}
                onChange={(e) => setRecipientAddress(e.target.value)}
                color="white"
                bg="gray.700"
                border="1px solid"
                borderColor={addressValidation.valid ? "green.400" : "gray.600"}
                _focus={{ borderColor: addressValidation.valid ? "green.400" : "blue.400" }}
                flex="1"
              />
              <IconButton
                aria-label="Scan QR code"
                size="md"
                onClick={() => {
                  alert('QR code scanning will be implemented later');
                }}
              >
                <FaQrcode />
              </IconButton>
            </HStack>
            {!addressValidation.valid && recipientAddress && (
              <Text color="red.400" fontSize="xs" mt={1}>
                {addressValidation.error || 'Invalid Bitcoin address'}
              </Text>
            )}
            {addressValidation.valid && (
              <Text color="green.400" fontSize="xs" mt={1}>
                ✓ Valid Bitcoin address
              </Text>
            )}
          </Box>

          {/* Amount with BTC/USD Toggle */}
          <Box w="100%">
            <HStack justify="space-between" mb={2}>
              <HStack gap={2}>
                <Text color="gray.300" fontSize="sm" fontWeight="medium">
                  Amount
                </Text>
                <Button
                  size="xs"
                  variant="solid"
                  colorScheme={amountCurrency === 'BTC' ? 'orange' : 'green'}
                  onClick={toggleCurrency}
                  fontWeight="bold"
                  _hover={{ opacity: 0.8 }}
                >
                  {amountCurrency}
                </Button>
              </HStack>
              <Button
                size="xs"
                variant="outline"
                colorScheme="blue"
                onClick={handleMaxAmount}
                disabled={availableBalance === 0}
              >
                Max
              </Button>
            </HStack>
            <Input
              placeholder={amountCurrency === 'BTC' ? "0.00000000" : "0.00"}
              value={amount}
              onChange={(e) => {
                setAmount(e.target.value);
                // Clear max send flag if user manually types amount
                if (isMaxSend) {
                  setIsMaxSend(false);
                  console.log('💰 Manual amount entered - clearing max send flag');
                }
              }}
              color="white"
              bg="gray.700"
              border="1px solid"
              borderColor="gray.600"
              _focus={{ borderColor: "blue.400" }}
              type="number"
              step={amountCurrency === 'BTC' ? "0.00000001" : "0.01"}
              min="0"
            />
            <Text fontSize="xs" color="gray.500" mt={1}>
              {isMaxSend && (
                <Text color="orange.400" fontSize="xs" mb={1}>
                  💰 MAX SEND: Will send all available funds minus network fees
                </Text>
              )}
              {amount && parseFloat(amount) > 0 && (
                <>
                  ≈ {amountCurrency === 'BTC' 
                    ? `$${convertBtcToUsd(parseFloat(amount)).toFixed(2)} USD` 
                    : `${convertUsdToBtc(parseFloat(amount)).toFixed(8)} BTC`
                  }
                </>
              )}
            </Text>
          </Box>

          {/* Compact Fee Selection */}
          <Box w="100%">
            <HStack justify="space-between" align="center" mb={1}>
              <Text color="gray.300" fontSize="xs" fontWeight="medium">
                Fee
              </Text>
              {!loadingFees && !feeError && (
                <Text fontSize="2xs" color="green.400">✓ Live</Text>
              )}
            </HStack>
            <HStack gap={1}>
              {(['slow', 'medium', 'fast'] as const).map((preset) => {
                const presetFeeRate = feeRates[preset];
                
                return (
                  <Button
                    key={preset}
                    size="xs"
                    variant={feeRate === preset ? "solid" : "outline"}
                    colorScheme={feeRate === preset ? "blue" : "gray"}
                    onClick={() => setFeeRate(preset)}
                    flex="1"
                    fontSize="2xs"
                    h="6"
                    disabled={loadingFees}
                  >
                    {preset} ({loadingFees ? '...' : Math.round(presetFeeRate)})
                  </Button>
                );
              })}
            </HStack>
          </Box>

          {/* Error Display */}
          {(error || walletError) && (
            <Box bg="red.900" p={3} borderRadius="md" border="1px solid" borderColor="red.600">
              <Text color="red.200" fontSize="sm">⚠️ {error || walletError}</Text>
            </Box>
          )}

          {/* Success Display */}
          {success && (
            <Box bg="green.900" p={3} borderRadius="md" border="1px solid" borderColor="green.600">
              <Text color="green.200" fontSize="sm">✅ {success}</Text>
            </Box>
          )}
        </VStack>

        {/* Single Action Button */}
        <Button
          colorScheme="blue"
          size="md"
          width="100%"
          onClick={handleBuildTransaction}
          disabled={!addressValidation.valid || !amount || loading || btcAssets.length === 0 || availableBalance === 0}
        >
          <HStack gap={2}>
            {loading ? <Spinner size="sm" /> : <FaEye />}
            <Text>{loading ? 'Building...' : 'Review Transaction'}</Text>
          </HStack>
        </Button>
      </VStack>
    );
  };

  const renderReviewStep = () => {
    if (!transactionReview) return null;
    
    return (
      <VStack gap={6}>
        <Box bg="gray.800" p={6} borderRadius="lg" w="100%">
          <Heading size="md" color="white" mb={4}>Transaction Details</Heading>
          <VStack gap={4} align="stretch">
            <HStack justify="space-between">
              <Text color="gray.400">To:</Text>
              <Code fontSize="sm" colorScheme="blue">{transactionReview.to}</Code>
            </HStack>
            <HStack justify="space-between">
              <Text color="gray.400">Amount:</Text>
              <Text color="white" fontWeight="bold">{transactionReview.amount}</Text>
            </HStack>
            <HStack justify="space-between">
              <Text color="gray.400">Fee Rate:</Text>
              <Text color="white">{transactionReview.feeRate} sat/vB</Text>
            </HStack>
            <HStack justify="space-between">
              <Text color="gray.400">Est. Fee:</Text>
              <Text color="white">{transactionReview.fee.toFixed(8)} BTC</Text>
            </HStack>
            <HStack justify="space-between" borderTop="1px solid" borderColor="gray.600" pt={2}>
              <Text color="gray.400" fontWeight="bold">Total:</Text>
              <Text color="orange.300" fontWeight="bold">{transactionReview.total.toFixed(8)} BTC</Text>
            </HStack>
          </VStack>
        </Box>

        <VStack gap={3} w="100%">
          <Button
            colorScheme="blue"
            size="lg"
            width="100%"
            onClick={handleSignTransaction}
            disabled={loading}
          >
            <HStack gap={2}>
              {loading ? <Spinner size="sm" /> : <FaSignature />}
              <Text>{loading ? 'Preparing...' : 'Sign Transaction'}</Text>
            </HStack>
          </Button>
          <Button
            variant="ghost"
            color="gray.400"
            onClick={handleStepBack}
          >
            Back to Edit
          </Button>
        </VStack>

        {error && (
          <Box bg="red.900" p={3} borderRadius="md" border="1px solid" borderColor="red.600" w="100%">
            <Text color="red.200" fontSize="sm">⚠️ {error}</Text>
          </Box>
        )}
      </VStack>
    );
  };

  const renderSignStep = () => {
    return (
      <VStack gap={6} align="center">
        <Box bg="gray.800" p={8} borderRadius="lg" textAlign="center">
          <VStack gap={4}>
            <Box color="blue.400" fontSize="4xl">
              <FaSignature />
            </Box>
            <Heading size="md" color="white">Signing Transaction</Heading>
            <Text color="gray.400" textAlign="center">
              Please confirm the transaction on your KeepKey device
            </Text>
            <Spinner size="lg" color="blue.400" />
          </VStack>
        </Box>

        {error && (
          <Box bg="red.900" p={3} borderRadius="md" border="1px solid" borderColor="red.600" w="100%">
            <Text color="red.200" fontSize="sm">⚠️ {error}</Text>
          </Box>
        )}
      </VStack>
    );
  };

  const renderCompleteStep = () => {
    return (
      <VStack gap={4} align="center">
        <Box bg="gray.800" p={6} borderRadius="lg" textAlign="center" w="100%">
          <VStack gap={3}>
            <Box color="green.400" fontSize="3xl">
              <FaCheck />
            </Box>
            <Heading size="md" color="white">Transaction Signed!</Heading>
            <Text color="gray.400" textAlign="center" fontSize="sm">
              Your transaction has been successfully signed
            </Text>

            {signedTransaction && (
              <Box w="100%" mt={3}>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => setIsShowingHex(!isShowingHex)}
                  color="gray.400"
                  _hover={{ color: "white" }}
                >
                  <HStack gap={2}>
                    <Text>{isShowingHex ? 'Hide' : 'Show'} Signed Transaction</Text>
                    {isShowingHex ? <FaChevronUp /> : <FaChevronDown />}
                  </HStack>
                </Button>
                
                {isShowingHex && (
                  <Box bg="gray.900" p={3} borderRadius="md" border="1px solid" borderColor="gray.600" mt={2}>
                    <Code fontSize="2xs" wordBreak="break-all" color="gray.300" bg="transparent">
                      {signedTransaction}
                    </Code>
                  </Box>
                )}
              </Box>
            )}
          </VStack>
        </Box>

        {/* Broadcast Status */}
        {broadcastSuccess && txid && (
          <Box bg="green.900" p={4} borderRadius="lg" border="1px solid" borderColor="green.600" w="100%">
            <VStack gap={3}>
              <Text color="green.200" fontSize="sm" fontWeight="bold">
                🎉 Transaction Broadcast Successfully!
              </Text>
              <VStack gap={2} w="100%">
                <Text color="gray.300" fontSize="xs">Transaction ID:</Text>
                <Box bg="gray.800" p={2} borderRadius="md" w="100%">
                  <Code fontSize="xs" wordBreak="break-all" color="green.300" bg="transparent">
                    {txid}
                  </Code>
                </Box>
                <HStack gap={2} w="100%">
                  <Button
                    size="sm"
                    colorScheme="green"
                    variant="outline"
                    onClick={copyTxidToClipboard}
                    flex="1"
                  >
                    📋 Copy TXID
                  </Button>
                  <Button
                    size="sm"
                    colorScheme="blue"
                    variant="outline"
                    onClick={async () => {
                      try {
                        const { invoke } = await import('@tauri-apps/api/core');
                        await invoke('open_url', { url: `https://blockstream.info/tx/${txid}` });
                      } catch (error) {
                        console.error('Failed to open URL:', error);
                        // Fallback to window.open if Tauri command fails
                        window.open(`https://blockstream.info/tx/${txid}`, '_blank');
                      }
                    }}
                    flex="1"
                  >
                    🔗 View on Blockstream
                  </Button>
                </HStack>
              </VStack>
            </VStack>
          </Box>
        )}

        {/* Broadcast Error */}
        {broadcastError && (
          <Box bg="red.900" p={4} borderRadius="lg" border="1px solid" borderColor="red.600" w="100%">
            <VStack gap={2}>
              <Text color="red.200" fontSize="sm" fontWeight="bold">
                ❌ Broadcast Failed
              </Text>
              <Text color="red.300" fontSize="xs" textAlign="center">
                {broadcastError}
              </Text>
            </VStack>
          </Box>
        )}

        {/* Main action button */}
        {!broadcastSuccess && (
          <Button
            colorScheme="orange"
            size="lg"
            width="100%"
            onClick={handleBroadcastTransaction}
            disabled={isBroadcasting || !signedTransaction}
            loading={isBroadcasting}
            loadingText="Broadcasting..."
          >
            <HStack gap={2}>
              <FaPaperPlane />
              <Text>Broadcast Transaction</Text>
            </HStack>
          </Button>
        )}

        {/* New Transaction Button (only after successful broadcast) */}
        {broadcastSuccess && (
          <Button
            colorScheme="blue"
            size="lg"
            width="100%"
            onClick={handleNewTransaction}
          >
            <HStack gap={2}>
              <FaPaperPlane />
              <Text>Send Another Transaction</Text>
            </HStack>
          </Button>
        )}

        {/* General success/error messages */}
        {success && !broadcastSuccess && (
          <Box bg="green.900" p={2} borderRadius="md" border="1px solid" borderColor="green.600" w="100%">
            <Text color="green.200" fontSize="sm" textAlign="center">✅ {success}</Text>
          </Box>
        )}
      </VStack>
    );
  };

  return (
    <Box height="100%" bg="transparent" display="flex" alignItems="center" justifyContent="center" p={6}>
      <VStack 
        align="stretch" 
        gap={6} 
        maxW="600px" 
        w="100%"
        bg="rgba(26, 32, 44, 0.95)" 
        p={6} 
        borderRadius="xl" 
        backdropFilter="blur(20px)"
        border="1px solid rgba(255, 255, 255, 0.1)"
        boxShadow="2xl"
      >
        {/* Header with back button and step progress */}
        <VStack gap={4}>
          <HStack w="100%">
            <IconButton
              aria-label="Go back"
              onClick={canGoBack() ? handleStepBack : onBack}
              size="sm"
            >
              <FaArrowLeft />
            </IconButton>
            <Flex align="center" justify="center" flex="1" gap={2}>
              <Box color="orange.400" fontSize="xl">
                <SiBitcoin />
              </Box>
              <Heading size="lg" color="white">
                Send Bitcoin
              </Heading>
            </Flex>
            <Box w="40px" /> {/* Spacer for centering */}
          </HStack>

          {/* Step Progress */}
          <VStack w="100%" gap={2}>
            <HStack justify="space-between" w="100%" px={4}>
              {(['compose', 'review', 'sign', 'complete'] as SendStep[]).map((step, index) => (
                <VStack key={step} align="center" gap={1}>
                  <Box
                    w={8}
                    h={8}
                    borderRadius="full"
                    bg={getStepNumber(currentStep) > index + 1 ? 'green.500' : 
                        getStepNumber(currentStep) === index + 1 ? 'blue.500' : 'gray.600'}
                    display="flex"
                    alignItems="center"
                    justifyContent="center"
                    color="white"
                    fontWeight="bold"
                    fontSize="sm"
                  >
                    {getStepNumber(currentStep) > index + 1 ? <FaCheck /> : index + 1}
                  </Box>
                  <Text fontSize="xs" color="gray.400" textAlign="center">
                    {step === 'compose' && 'Compose'}
                    {step === 'review' && 'Review'}
                    {step === 'sign' && 'Sign'}
                    {step === 'complete' && 'Complete'}
                  </Text>
                </VStack>
              ))}
            </HStack>
            <Box w="100%" h="2" bg="gray.600" borderRadius="full" overflow="hidden">
              <Box 
                h="100%" 
                bg="blue.500" 
                w={`${(getStepNumber(currentStep) - 1) * 33.33}%`}
                transition="width 0.3s ease"
                borderRadius="full"
              />
            </Box>
          </VStack>
          

        </VStack>

        {/* Step Content */}
        {renderStepContent()}
      </VStack>
    </Box>
  );
};

export default Send; 