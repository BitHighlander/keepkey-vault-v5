import React, { useState } from 'react';
import {
  Box,
  VStack,
  HStack,
  Text,
  Button,
  Grid,
  GridItem,
  Badge,
  Flex,
  Spinner,
  Input,
  Icon,
} from '@chakra-ui/react';
import { FaSearch, FaArrowUp, FaArrowDown, FaExchangeAlt } from 'react-icons/fa';
import { useWallet } from '../../contexts/WalletContext';

// Theme colors
const theme = {
  bg: 'gray.900',
  cardBg: 'gray.800',
  gold: '#FFD700',
  goldHover: '#FFE135',
  border: 'gray.700',
};

export const AssetView: React.FC = () => {
  const { portfolio, loading, error, selectAsset, selectedAsset } = useWallet();
  const [searchQuery, setSearchQuery] = useState('');

  // Filter assets based on search query
  const filteredAssets = portfolio?.assets.filter(asset =>
    asset.symbol.toLowerCase().includes(searchQuery.toLowerCase()) ||
    asset.name.toLowerCase().includes(searchQuery.toLowerCase())
  ) || [];

  const handleAssetSelect = (asset: any) => {
    selectAsset(asset);
  };

  if (loading) {
    return (
      <Box height="100%" bg={theme.bg} display="flex" alignItems="center" justifyContent="center">
        <VStack gap={4}>
          <Spinner size="xl" color="blue.400" />
          <Text color="gray.300" fontSize="lg">Loading Assets...</Text>
        </VStack>
      </Box>
    );
  }

  if (error) {
    return (
      <Box height="100%" bg={theme.bg} p={6}>
        <VStack justify="center" align="center" height="100%">
          <Text color="red.400" fontSize="lg">Error loading assets</Text>
          <Text color="gray.400" fontSize="sm">{error}</Text>
        </VStack>
      </Box>
    );
  }

  return (
    <Box height="100%" bg={theme.bg} overflowY="auto">
      <VStack gap={6} align="stretch" p={6}>
        {/* Header */}
        <VStack gap={4} align="stretch">
          <HStack justify="space-between">
            <Text fontSize="2xl" fontWeight="bold" color="white">
              Your Assets
            </Text>
            <Badge colorScheme="blue" variant="subtle">
              {filteredAssets.length} assets
            </Badge>
          </HStack>
          
          {/* Search Bar */}
          <Box maxW="400px">
            <Input
              placeholder="Search assets..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              bg={theme.cardBg}
              border="1px solid"
              borderColor={theme.border}
              color="white"
              _placeholder={{ color: 'gray.400' }}
              _focus={{
                borderColor: theme.gold,
                boxShadow: `0 0 0 1px ${theme.gold}`,
              }}
            />
          </Box>
        </VStack>

        {/* Assets Grid */}
        {filteredAssets.length > 0 ? (
          <Grid templateColumns="repeat(auto-fill, minmax(320px, 1fr))" gap={4}>
            {filteredAssets.map((asset, index) => (
              <GridItem key={index}>
                <Box
                  bg={theme.cardBg}
                  borderRadius="xl"
                  border="1px solid"
                  borderColor={selectedAsset?.symbol === asset.symbol ? theme.gold : theme.border}
                  _hover={{
                    borderColor: theme.gold,
                    transform: "translateY(-2px)",
                    boxShadow: `0 4px 20px ${theme.gold}30`,
                  }}
                  transition="all 0.2s"
                  cursor="pointer"
                  onClick={() => handleAssetSelect(asset)}
                  p={6}
                >
                  <VStack align="stretch" gap={4}>
                    {/* Asset Header */}
                    <HStack justify="space-between">
                      <HStack gap={3}>
                        <Box
                          w="40px"
                          h="40px"
                          borderRadius="full"
                          bg={asset.symbol === 'BTC' ? '#f7931a' : 
                              asset.symbol === 'ETH' ? '#627eea' : 
                              theme.gold}
                          display="flex"
                          alignItems="center"
                          justifyContent="center"
                          color="white"
                          fontWeight="bold"
                          fontSize="sm"
                        >
                          {asset.symbol.slice(0, 2)}
                        </Box>
                        <VStack align="start" gap={0}>
                          <Text fontWeight="bold" color="white" fontSize="lg">
                            {asset.symbol}
                          </Text>
                          <Text fontSize="sm" color="gray.400">
                            {asset.name}
                          </Text>
                        </VStack>
                      </HStack>
                      
                      <VStack align="end" gap={0}>
                        <Text fontWeight="bold" color="white" fontSize="lg">
                          {parseFloat(asset.balance).toFixed(6)}
                        </Text>
                        <Text fontSize="sm" color="gray.400">
                          ${asset.value_usd.toFixed(2)}
                        </Text>
                      </VStack>
                    </HStack>

                    {/* Network Info */}
                    <HStack justify="space-between">
                      <HStack gap={2}>
                        <Text fontSize="xs" color="gray.500">Network:</Text>
                        <Badge size="sm" colorScheme="purple" variant="subtle">
                          {asset.network_id}
                        </Badge>
                      </HStack>
                      
                      {asset.change_24h !== undefined && (
                        <HStack gap={1}>
                          <Text fontSize="xs" color="gray.500">24h:</Text>
                          <Text 
                            fontSize="xs" 
                            color={asset.change_24h >= 0 ? "green.400" : "red.400"}
                            fontWeight="medium"
                          >
                            {asset.change_24h >= 0 ? '+' : ''}{asset.change_24h.toFixed(2)}%
                          </Text>
                        </HStack>
                      )}
                    </HStack>

                    {/* Quick Actions */}
                    {selectedAsset?.symbol === asset.symbol && (
                      <HStack gap={2} pt={2} borderTop="1px solid" borderColor={theme.border}>
                        <Button
                          size="sm"
                          colorScheme="blue"
                          variant="outline"
                          flex="1"
                        >
                          <Icon as={FaArrowUp} mr={2} />
                          Send
                        </Button>
                        <Button
                          size="sm"
                          colorScheme="green"
                          variant="outline"
                          flex="1"
                        >
                          <Icon as={FaArrowDown} mr={2} />
                          Receive
                        </Button>
                        <Button
                          size="sm"
                          colorScheme="purple"
                          variant="outline"
                          flex="1"
                        >
                          <Icon as={FaExchangeAlt} mr={2} />
                          Swap
                        </Button>
                      </HStack>
                    )}
                  </VStack>
                </Box>
              </GridItem>
            ))}
          </Grid>
        ) : (
          <Box
            p={8}
            textAlign="center"
            bg={theme.cardBg}
            borderRadius="xl"
            border="1px dashed"
            borderColor={theme.border}
          >
            <VStack gap={3}>
              <Text color="gray.400" fontSize="lg">
                {searchQuery ? 'No assets found' : 'No assets available'}
              </Text>
              <Text fontSize="sm" color="gray.500">
                {searchQuery 
                  ? `No assets match "${searchQuery}"`
                  : 'Connect your KeepKey device to view your assets'
                }
              </Text>
              {searchQuery && (
                <Button
                  size="sm"
                  variant="ghost"
                  color={theme.gold}
                  onClick={() => setSearchQuery('')}
                >
                  Clear search
                </Button>
              )}
            </VStack>
          </Box>
        )}

        {/* Portfolio Summary at Bottom */}
        {portfolio && (
          <Box
            p={6}
            bg={theme.cardBg}
            borderRadius="xl"
            border="1px solid"
            borderColor={theme.border}
            mt={4}
          >
            <HStack justify="space-between">
              <VStack align="start" gap={1}>
                <Text fontSize="sm" color="gray.400">Total Portfolio Value</Text>
                <Text fontSize="2xl" fontWeight="bold" color={theme.gold}>
                  ${parseFloat(portfolio.total_value_usd).toFixed(2)}
                </Text>
              </VStack>
              
              <VStack align="end" gap={1}>
                <Text fontSize="sm" color="gray.400">Assets</Text>
                <Text fontSize="lg" fontWeight="semibold" color="white">
                  {portfolio.assets.length}
                </Text>
              </VStack>
            </HStack>
          </Box>
        )}
      </VStack>
    </Box>
  );
}; 