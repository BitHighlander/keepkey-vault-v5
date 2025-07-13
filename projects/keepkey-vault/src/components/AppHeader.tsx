import { useState, useEffect } from 'react';
import { Box, HStack, Text, IconButton } from '@chakra-ui/react';
import { FaCode } from 'react-icons/fa';
import { invoke } from '@tauri-apps/api/core';

export const AppHeader = () => {
  const [version, setVersion] = useState<string>('');
  const [devToolsOpen, setDevToolsOpen] = useState(false);

  // Get app version on mount
  useEffect(() => {
    const getVersion = async () => {
      try {
        const appVersion = await invoke<string>('get_app_version');
        setVersion(appVersion);
      } catch (error) {
        console.error('Failed to get app version:', error);
        setVersion('0.1.1'); // fallback
      }
    };
    getVersion();
  }, []);

  // Handle dev tools toggle
  const handleDevToolsToggle = async () => {
    try {
      await invoke('toggle_dev_tools');
      setDevToolsOpen(!devToolsOpen);
      console.log(devToolsOpen ? 'Dev Tools Closed' : 'Dev Tools Opened');
    } catch (error) {
      console.error('Failed to toggle dev tools:', error);
    }
  };

  // Handle keyboard shortcut
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      // Cmd+Option+I (Mac) or Ctrl+Shift+I (Windows/Linux)
      if (
        (event.metaKey && event.altKey && event.key === 'i') ||
        (event.ctrlKey && event.shiftKey && event.key === 'I')
      ) {
        event.preventDefault();
        handleDevToolsToggle();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [devToolsOpen]);

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
            KeepKey Vault
          </Text>
          <Text fontSize="xs" color="gray.400" fontFamily="mono">
            v{version}
          </Text>
        </HStack>
        
        <HStack gap={2}>
          <IconButton
            aria-label="Toggle dev tools"
            size="xs"
            variant="ghost"
            colorScheme={devToolsOpen ? 'blue' : 'gray'}
            onClick={handleDevToolsToggle}
            title={`${devToolsOpen ? 'Close' : 'Open'} Dev Tools (${
              navigator.platform.includes('Mac') ? 'Cmd+Opt+I' : 'Ctrl+Shift+I'
            })`}
          >
            <FaCode />
          </IconButton>
        </HStack>
      </HStack>
    </Box>
  );
}; 