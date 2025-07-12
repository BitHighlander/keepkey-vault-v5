import React, { useEffect, useState } from 'react';
import { Box, Button, Text, Flex } from '@chakra-ui/react';
import { CacheAPI, CacheStatus as CacheStatusType } from '../lib/cache';

interface CacheStatusProps {
  deviceId: string;
}

export const CacheStatus: React.FC<CacheStatusProps> = ({ deviceId }) => {
  const [cacheStatus, setCacheStatus] = useState<CacheStatusType | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [isFrontloading, setIsFrontloading] = useState(false);
  const [frontloadProgress, setFrontloadProgress] = useState(0);

  useEffect(() => {
    loadCacheStatus();
  }, [deviceId]);

  const loadCacheStatus = async () => {
    try {
      setIsLoading(true);
      setError(null);
      const status = await CacheAPI.getCacheStatus(deviceId);
      setCacheStatus(status);
      setIsFrontloading(status.frontload_status === 'in_progress');
      setFrontloadProgress(status.frontload_progress);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load cache status');
    } finally {
      setIsLoading(false);
    }
  };

  const handleFrontload = async () => {
    try {
      setError(null);
      setIsFrontloading(true);
      await CacheAPI.triggerFrontload(deviceId);
      
      // Monitor progress
      await CacheAPI.monitorFrontload(deviceId, (progress) => {
        setFrontloadProgress(progress);
      });
      
      // Reload status when complete
      await loadCacheStatus();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Frontload failed');
    } finally {
      setIsFrontloading(false);
    }
  };

  const handleClearCache = async () => {
    try {
      setError(null);
      await CacheAPI.clearCache(deviceId);
      await loadCacheStatus();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to clear cache');
    }
  };

  if (isLoading) {
    return <Box>Loading cache status...</Box>;
  }

  return (
    <Box p={4} borderWidth="1px" borderRadius="lg">
      <Text fontSize="lg" fontWeight="bold" mb={4}>Cache Status</Text>
      
      {error && (
        <Box p={3} mb={4} bg="red.50" borderRadius="md">
          <Text color="red.600">{error}</Text>
        </Box>
      )}
      
      {cacheStatus && (
        <Box mb={4}>
          <Flex gap={4} mb={3}>
            <Box>
              <Text fontSize="sm" color="gray.600">Cached Entries</Text>
              <Text fontSize="2xl" fontWeight="bold">{cacheStatus.total_cached}</Text>
            </Box>
            
            <Box>
              <Text fontSize="sm" color="gray.600">Hit Rate</Text>
              <Text fontSize="2xl" fontWeight="bold">{(cacheStatus.hit_rate * 100).toFixed(1)}%</Text>
              <Text fontSize="xs" color="gray.500">
                {cacheStatus.cache_hits} hits / {cacheStatus.cache_misses} misses
              </Text>
            </Box>
            
            <Box>
              <Text fontSize="sm" color="gray.600">Status</Text>
              <Text fontSize="lg" fontWeight="medium">{cacheStatus.frontload_status}</Text>
              <Text fontSize="xs" color="gray.500">
                {cacheStatus.last_frontload 
                  ? `Last: ${new Date(cacheStatus.last_frontload * 1000).toLocaleString()}`
                  : 'Never run'}
              </Text>
            </Box>
          </Flex>
        </Box>
      )}
      
      {isFrontloading && (
        <Box mb={4}>
          <Text mb={2}>Frontloading addresses... {frontloadProgress}%</Text>
          <Box bg="gray.200" h="4px" borderRadius="full">
            <Box 
              bg="blue.500" 
              h="100%" 
              borderRadius="full" 
              width={`${frontloadProgress}%`}
              transition="width 0.3s"
            />
          </Box>
        </Box>
      )}
      
      <Flex gap={3}>
        <Button
          colorScheme="blue"
          onClick={handleFrontload}
          loading={isFrontloading}
          loadingText="Frontloading..."
          disabled={isFrontloading}
        >
          Start Frontload
        </Button>
        
        <Button
          variant="outline"
          colorScheme="red"
          onClick={handleClearCache}
          disabled={isFrontloading}
        >
          Clear Cache
        </Button>
        
        <Button
          variant="outline"
          onClick={loadCacheStatus}
          disabled={isFrontloading}
        >
          Refresh
        </Button>
      </Flex>
    </Box>
  );
};