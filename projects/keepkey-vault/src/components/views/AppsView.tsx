import React, { useState, useEffect } from 'react';
import { 
  Box, 
  Grid, 
  GridItem, 
  Text, 
  Button, 
  Stack, 
  Icon, 
  Flex, 
  Image, 
  Spinner,
  Badge
} from '@chakra-ui/react';
import { FaPlus, FaExternalLinkAlt, FaGlobe } from 'react-icons/fa';
import { invoke } from '@tauri-apps/api/core';
import axios from 'axios';

interface PioneerApp {
  _id: string;
  name: string;
  app: string; // URL
  tags: string[];
  image: string;
  developer: string;
  description: string;
  homepage: string;
  id: string;
  isSpotlight: boolean;
  whitelist: boolean;
  created: number;
  trust: number;
  transparency: number;
  innovation: number;
  popularity: number;
  score: number;
}

const PIONEER_APPS_API = 'https://pioneers.dev/api/v1/appsbyVersion/0.0.1/10000/0';

export const AppsView = () => {
  const [apps, setApps] = useState<PioneerApp[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Fetch apps from Pioneer API
  const fetchApps = async () => {
    try {
      setLoading(true);
      setError(null);
      
      console.log('🌐 Fetching apps from Pioneer API...');
      const response = await axios.get(PIONEER_APPS_API, {
        headers: { 'accept': 'application/json' },
        timeout: 10000
      });
      
      console.log('✅ Apps fetched successfully:', response.data.length, 'apps');
      
      // Sort by score (highest first) and take top apps
      const sortedApps = response.data
        .filter((app: PioneerApp) => app.whitelist && app.app && app.name)
        .sort((a: PioneerApp, b: PioneerApp) => (b.score || 0) - (a.score || 0))
        .slice(0, 50); // Limit to top 50 apps
      
      setApps(sortedApps);
    } catch (err) {
      console.error('❌ Failed to fetch apps:', err);
      setError('Failed to load apps. Please try again.');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchApps();
  }, []);

  const getCategoryFromTags = (tags: string[]): string => {
    const tagStr = tags.join(' ').toLowerCase();
    if (tagStr.includes('defi') || tagStr.includes('dex') || tagStr.includes('swap')) return 'DeFi';
    if (tagStr.includes('wallet') || tagStr.includes('custody')) return 'Wallet';
    if (tagStr.includes('nft') || tagStr.includes('art') || tagStr.includes('collectible')) return 'NFT';
    if (tagStr.includes('game') || tagStr.includes('gaming')) return 'Gaming';
    if (tagStr.includes('social') || tagStr.includes('media')) return 'Social';
    if (tagStr.includes('ethereum')) return 'Ethereum';
    if (tagStr.includes('bitcoin')) return 'Bitcoin';
    return 'Web3';
  };

  const getCategoryColor = (category: string): string => {
    switch (category) {
      case 'DeFi': return 'green';
      case 'Wallet': return 'blue';
      case 'NFT': return 'purple';
      case 'Gaming': return 'orange';
      case 'Social': return 'pink';
      case 'Ethereum': return 'blue';
      case 'Bitcoin': return 'orange';
      default: return 'gray';
    }
  };
  const handleAppClick = async (app: PioneerApp) => {
    if (app.app) {
      try {
        // Tell backend to switch to browser and navigate to app URL
        await invoke('vault_open_app', { 
          appId: app.id, 
          appName: app.name, 
          url: app.app 
        });
      } catch (error) {
        console.error('Failed to open app via backend:', error);
        // Fallback to direct open
        window.open(app.app, '_blank');
      }
    }
  };

  const handleAddApp = () => {
    // TODO: Implement add custom app functionality
    console.log('Add custom app clicked');
  };

  const handleRefresh = () => {
    fetchApps();
  };

  return (
    <Box 
      p={6} 
      height="100%" 
      bg="rgba(26, 32, 44, 0.8)"
      backdropFilter="blur(20px)"
    >
      <Stack direction="column" gap={6} align="stretch" height="100%">
        {/* Header */}
        <Flex justify="space-between" align="center">
          <Stack direction="column" gap={1} align="start">
            <Text fontSize="2xl" fontWeight="bold" color="white">
              Apps
            </Text>
            <Text fontSize="md" color="gray.400">
              Connect to your favorite dApps and services
            </Text>
            {!loading && apps.length > 0 && (
              <Text fontSize="sm" color="gray.500">
                {apps.length} apps available
              </Text>
            )}
          </Stack>
          <Stack direction="row" gap={2}>
            <Button
              colorScheme="gray"
              variant="outline"
              size="sm"
              onClick={handleRefresh}
              disabled={loading}
            >
              <Icon as={FaGlobe} mr={2} />
              Refresh
            </Button>
            <Button
              colorScheme="blue"
              variant="outline"
              size="sm"
              onClick={handleAddApp}
            >
              <Icon as={FaPlus} mr={2} />
              Add App
            </Button>
          </Stack>
        </Flex>

        {/* Content */}
        <Box flex="1" overflowY="auto">
          {loading ? (
            <Flex justify="center" align="center" height="200px">
              <Stack align="center" gap={4}>
                <Spinner size="lg" color="blue.400" />
                <Text color="gray.400">Loading apps from Pioneer...</Text>
              </Stack>
            </Flex>
          ) : error ? (
            <Flex justify="center" align="center" height="200px">
              <Stack align="center" gap={4}>
                <Text color="red.400" fontSize="lg">⚠️ {error}</Text>
                <Button colorScheme="blue" size="sm" onClick={handleRefresh}>
                  Try Again
                </Button>
              </Stack>
            </Flex>
          ) : apps.length === 0 ? (
            <Flex justify="center" align="center" height="200px">
              <Text color="gray.400">No apps found.</Text>
            </Flex>
          ) : (
            <Grid templateColumns="repeat(auto-fill, minmax(240px, 1fr))" gap={4}>
              {apps.map((app) => {
                const category = getCategoryFromTags(app.tags);
                const categoryColor = getCategoryColor(category);
                
                return (
                  <GridItem key={app._id}>
                    <Button
                      height="140px"
                      width="100%"
                      bg="gray.800"
                      border="1px solid"
                      borderColor="gray.700"
                      _hover={{
                        bg: "gray.700",
                        borderColor: `${categoryColor}.500`,
                        transform: "translateY(-2px)",
                        boxShadow: `0 4px 12px rgba(0,0,0,0.3)`
                      }}
                      _active={{
                        bg: "gray.600",
                      }}
                      borderRadius="xl"
                      onClick={() => handleAppClick(app)}
                      transition="all 0.2s"
                      position="relative"
                      overflow="hidden"
                      title={`${app.description} • Score: ${app.score ? app.score.toFixed(1) : 'N/A'} • Trust: ${app.trust || 'N/A'}`}
                    >
                      <Stack direction="column" gap={3} align="center" p={2}>
                        {/* App Icon */}
                        <Box position="relative">
                          {app.image ? (
                            <Image
                              src={app.image}
                              alt={app.name}
                              width="48px"
                              height="48px"
                              borderRadius="lg"
                              onError={(e) => {
                                (e.target as HTMLImageElement).style.display = 'none';
                              }}
                            />
                          ) : null}
                          
                          {/* Fallback icon */}
                          <Box
                            width="48px"
                            height="48px"
                            bg={`${categoryColor}.500`}
                            borderRadius="lg"
                            display="flex"
                            alignItems="center"
                            justifyContent="center"
                            color="white"
                            fontSize="xl"
                            position={app.image ? "absolute" : "static"}
                            top={app.image ? "0" : undefined}
                            left={app.image ? "0" : undefined}
                          >
                            <Icon as={FaGlobe} />
                          </Box>
                          
                          {/* External Link Icon */}
                          <Box
                            position="absolute"
                            top="-2px"
                            right="-2px"
                            bg="blue.500"
                            borderRadius="full"
                            p={1}
                            fontSize="xs"
                          >
                            <Icon as={FaExternalLinkAlt} color="white" />
                          </Box>
                        </Box>
                        
                        {/* App Info */}
                        <Stack direction="column" gap={1} align="center" flex="1">
                          <Text 
                            fontWeight="bold" 
                            color="white" 
                            fontSize="sm"
                            textAlign="center"
                            overflow="hidden"
                            textOverflow="ellipsis"
                            whiteSpace="nowrap"
                            maxW="100%"
                          >
                            {app.name}
                          </Text>
                          <Text 
                            fontSize="xs" 
                            color="gray.400" 
                            textAlign="center"
                            lineHeight="1.2"
                            overflow="hidden"
                            textOverflow="ellipsis"
                            whiteSpace="nowrap"
                            maxW="100%"
                          >
                            {app.description || 'Web3 Application'}
                          </Text>
                          
                          {/* Category Badge */}
                          <Badge
                            colorScheme={categoryColor}
                            size="sm"
                            borderRadius="md"
                            fontSize="xs"
                            mt={1}
                          >
                            {category}
                          </Badge>
                        </Stack>
                      </Stack>
                    </Button>
                  </GridItem>
                );
              })}
            </Grid>
          )}
        </Box>
      </Stack>
    </Box>
  );
}; 