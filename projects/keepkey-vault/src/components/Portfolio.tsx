import React from 'react';
import {
  Box,
  Flex,
  Text,
  HStack,
  Button,
  VStack,
  Spinner
} from '@chakra-ui/react';
import { FaPaperPlane, FaDownload } from 'react-icons/fa';
import { SiBitcoin } from 'react-icons/si';
import { useWallet } from '../contexts/WalletContext';

interface PortfolioProps {
  onNavigate?: (action: 'send' | 'receive') => void;
}

export const Portfolio: React.FC<PortfolioProps> = ({ onNavigate }) => {
  const { portfolio, loading, error, refreshPortfolio } = useWallet();
  const [showStartButton, setShowStartButton] = React.useState(false);
  const [syncingTime, setSyncingTime] = React.useState(0);

  console.log('portfolio: ', portfolio);

  // Timer to show the start button after 15 seconds
  React.useEffect(() => {
    let timer: NodeJS.Timeout;
    let intervalTimer: NodeJS.Timeout;
    
    if (!portfolio && !loading && !error) {
      // Start counting sync time
      intervalTimer = setInterval(() => {
        setSyncingTime(prev => prev + 1);
      }, 1000);
      
      // Show start button after 15 seconds
      timer = setTimeout(() => {
        setShowStartButton(true);
      }, 15000);
    } else {
      // Reset when portfolio loads or error occurs
      setShowStartButton(false);
      setSyncingTime(0);
    }
    
    return () => {
      clearTimeout(timer);
      clearInterval(intervalTimer);
    };
  }, [portfolio, loading, error]);

  // Format USD value
  const formatUsd = (value: number | string) => {
    const numValue = typeof value === 'string' ? parseFloat(value) : value;
    if (isNaN(numValue)) return '0.00';
    return numValue.toLocaleString('en-US', {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    });
  };

  // Debug function to inspect USD values
  const debugUsdValues = () => {
    console.group('🔍 DEBUG: USD Values Analysis');
    
    console.log('📊 Portfolio object:', portfolio);
    
    if (portfolio) {
      console.log('💰 Portfolio total_value_usd:', portfolio.total_value_usd, typeof portfolio.total_value_usd);
      
      if (portfolio.assets && portfolio.assets.length > 0) {
        console.log('💵 Individual asset USD values:');
        portfolio.assets.forEach((asset, index) => {
          console.log(`  [${index}] ${asset.caip}: balance="${asset.balance}", value_usd=${asset.value_usd} (${typeof asset.value_usd})`);
        });
      }
    }
    
    console.groupEnd();
  };

  if (loading) {
    return (
      <Box height="100%" display="flex" alignItems="center" justifyContent="center" bg="transparent">
        <VStack 
          gap={4}
          bg="rgba(26, 32, 44, 0.9)" 
          p={8} 
          borderRadius="xl" 
          backdropFilter="blur(20px)"
          border="1px solid rgba(255, 255, 255, 0.1)"
        >
          <Spinner size="xl" color="blue.400" />
          <Text color="gray.300" fontSize="lg">Loading Portfolio...</Text>
        </VStack>
      </Box>
    );
  }

  if (error) {
    return (
      <Box height="100%" display="flex" alignItems="center" justifyContent="center" bg="transparent">
        <VStack 
          gap={4}
          bg="rgba(26, 32, 44, 0.9)" 
          p={8} 
          borderRadius="xl" 
          backdropFilter="blur(20px)"
          border="1px solid rgba(255, 255, 255, 0.1)"
        >
          <Text color="red.400" textAlign="center">{error}</Text>
        </VStack>
      </Box>
    );
  }

  if (!portfolio) {
    return (
      <Box height="100%" display="flex" alignItems="center" justifyContent="center" bg="transparent">
        <VStack 
          gap={6}
          bg="rgba(26, 32, 44, 0.9)" 
          p={10} 
          borderRadius="xl" 
          backdropFilter="blur(20px)"
          border="1px solid rgba(255, 255, 255, 0.1)"
          minW="340px"
          textAlign="center"
        >
          {!showStartButton ? (
            <>
              <Spinner size="xl" color="blue.400" />
              <VStack gap={2}>
                <Text color="white" fontSize="xl" fontWeight="semibold">
                  Syncing with your KeepKey
                </Text>
                <Text color="gray.400" fontSize="md">
                  Please make sure your device is unlocked
                </Text>
                {syncingTime > 5 && (
                  <Text color="gray.500" fontSize="sm" mt={2}>
                    This is taking longer than usual...
                  </Text>
                )}
              </VStack>
            </>
          ) : (
            <>
              <Box color="orange.400" fontSize="4xl">
                <SiBitcoin />
              </Box>
              <VStack gap={3}>
                <Text color="white" fontSize="xl" fontWeight="semibold">
                  Ready to get started?
                </Text>
                <Text color="gray.400" fontSize="md" maxW="280px">
                  Make sure your KeepKey is connected and unlocked
                </Text>
              </VStack>
              <Button 
                colorScheme="blue" 
                size="lg" 
                onClick={refreshPortfolio}
                _hover={{ transform: 'scale(1.05)' }}
                transition="all 0.2s"
                fontWeight="bold"
                px={8}
              >
                Press here to start! 🚀
              </Button>
            </>
          )}
        </VStack>
      </Box>
    );
  }

  // Find BTC balance (sum all BTC balances if multiple, or 0 if none)
  const btcAssets = portfolio.assets
    .filter(asset => asset.caip === 'bip122:000000000019d6689c085ae165831e93/slip44:0');
  const btcTotal = btcAssets.reduce((sum, asset) => sum + parseFloat(asset.balance), 0);

  // Debug logging to help troubleshoot - compare with Send component
  console.log('🔍 Portfolio component balance debug:', {
    portfolio: portfolio ? 'loaded' : 'null',
    totalAssets: portfolio?.assets.length || 0,
    allAssetCAIPs: portfolio?.assets.map(a => a.caip) || [],
    btcAssets: btcAssets.length,
    btcTotal,
    btcAssetsDetails: btcAssets.map(a => ({ caip: a.caip, balance: a.balance, symbol: a.symbol })),
    expectedCAIP: 'bip122:000000000019d6689c085ae165831e93/slip44:0'
  });

  return (
    <Flex align="center" justify="center" h="100%" bg="transparent">
      <VStack 
        gap={6} 
        align="center" 
        bg="rgba(26, 32, 44, 0.9)" 
        p={10} 
        borderRadius="xl" 
        boxShadow="2xl" 
        minW="340px"
        backdropFilter="blur(20px)"
        border="1px solid rgba(255, 255, 255, 0.1)"
      >
        {/* Bitcoin Logo */}
        <Box color="orange.400" fontSize="4xl" onClick={debugUsdValues}>
          <SiBitcoin />
        </Box>
        
        {/* USD Balance */}
        <Box textAlign="center">
          <Text fontSize="3xl" fontWeight="bold" color="white">
            ${formatUsd(portfolio.total_value_usd)}
          </Text>
          <Text fontSize="md" color="gray.400" mt={1}>
            Total USD Value
          </Text>
        </Box>
        
        {/* BTC Balance */}
        <Box textAlign="center">
          <Text fontSize="2xl" fontWeight="semibold" color="orange.300">
            {btcTotal.toFixed(8)} BTC
          </Text>
          <Text fontSize="md" color="gray.400" mt={1}>
            Bitcoin Balance
          </Text>
        </Box>
        
        {/* Send/Receive Buttons */}
        <HStack gap={8} mt={4}>
          <Button 
            colorScheme="blue" 
            size="lg" 
            fontWeight="bold"
            onClick={() => onNavigate?.('send')}
            minW="120px"
          >
            <HStack gap={2}>
              <FaPaperPlane />
              <Text>Send</Text>
            </HStack>
          </Button>
          <Button 
            colorScheme="green" 
            size="lg" 
            fontWeight="bold"
            onClick={() => onNavigate?.('receive')}
            minW="120px"
          >
            <HStack gap={2}>
              <FaDownload />
              <Text>Receive</Text>
            </HStack>
          </Button>
        </HStack>

      </VStack>
    </Flex>
  );
};