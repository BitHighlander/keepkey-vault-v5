import { useState, useEffect } from 'react';
import { Box, HStack, Text, IconButton } from '@chakra-ui/react';
import { FaCode } from 'react-icons/fa';
import { invoke } from '@tauri-apps/api/core';

export const AppHeader = () => {
  const [version, setVersion] = useState<string>('');

  // Get app version on mount
  useEffect(() => {
    const getVersion = async () => {
      try {
        const appVersion = await invoke<string>('get_app_version');
        setVersion(appVersion);
      } catch (error) {
        console.error('Failed to get app version:', error);
        setVersion('0.1.2'); // fallback to current version
      }
    };
    getVersion();
  }, []);

  return (
    <Box
      position="fixed"
      top={0}
      left={0}
      right={0}
      zIndex={1000}
      bg="rgba(26, 32, 44, 0.95)"
      backdropFilter="blur(20px)"
      borderBottom="1px solid"
      borderColor="rgba(255, 255, 255, 0.1)"
      px={4}
      py={2}
      height="40px"
    >
      <HStack justify="space-between" align="center" height="100%">
        <HStack gap={3}>
          <Text fontSize="sm" fontWeight="bold" color="white">
            KeepKey Vault v{version}
          </Text>
        </HStack>
        
        <HStack gap={2}>
          {/* Dev tools functionality will be handled through config */}
          <Text fontSize="xs" color="gray.500">
            Press F12 or Cmd+Opt+I for DevTools
          </Text>
        </HStack>
      </HStack>
    </Box>
  );
}; 