import { useState, useEffect } from 'react';
import { 
  Box, 
  Input, 
  Button, 
  Flex, 
  Text, 
  Stack,
  IconButton,
  Spinner
} from '@chakra-ui/react';
import { FaArrowLeft, FaArrowRight, FaRedo, FaHome, FaSearch } from 'react-icons/fa';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

export const BrowserView = () => {
  const [url, setUrl] = useState('https://vault.keepkey.com');
  const [inputUrl, setInputUrl] = useState('https://vault.keepkey.com');
  const [isLoading, setIsLoading] = useState(true);
  const [canGoBack, setCanGoBack] = useState(false);
  const [canGoForward, setCanGoForward] = useState(false);
  const [showControls, setShowControls] = useState(false);
  const [hoverTimer, setHoverTimer] = useState<NodeJS.Timeout | null>(null);

  const handleNavigate = async () => {
    if (!inputUrl) return;
    
    // Simple URL validation and formatting
    let formattedUrl = inputUrl;
    if (!inputUrl.startsWith('http://') && !inputUrl.startsWith('https://')) {
      formattedUrl = `https://${inputUrl}`;
    }
    
    // Notify backend of URL change
    try {
      await invoke('browser_navigate', { url: formattedUrl });
    } catch (error) {
      console.error('Failed to notify backend of navigation:', error);
      // Still navigate even if backend call fails
      setUrl(formattedUrl);
      setInputUrl(formattedUrl);
      setIsLoading(true);
    }
  };

  const handleHome = () => {
    const homeUrl = 'https://vault.keepkey.com';
    setInputUrl(homeUrl);
    setUrl(homeUrl);
    setIsLoading(true);
  };

  const handleRefresh = () => {
    setIsLoading(true);
    // Force iframe reload by changing src
    const iframe = document.getElementById('browser-iframe') as HTMLIFrameElement;
    if (iframe) {
      iframe.src = iframe.src;
    }
  };

  const handleBack = () => {
    // Note: iframe history navigation is limited due to security restrictions
    window.history.back();
  };

  const handleForward = () => {
    // Note: iframe history navigation is limited due to security restrictions
    window.history.forward();
  };

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      handleNavigate();
    }
  };

  const handleIframeLoad = () => {
    setIsLoading(false);
    // Try to get the actual URL from iframe (limited by CORS)
    const iframe = document.getElementById('browser-iframe') as HTMLIFrameElement;
    if (iframe) {
      try {
        setInputUrl(iframe.contentWindow?.location.href || url);
      } catch (e) {
        // Cross-origin restrictions prevent accessing iframe URL
        setInputUrl(url);
      }
    }
  };

  const handleMouseEnter = () => {
    // Clear any existing timer
    if (hoverTimer) {
      clearTimeout(hoverTimer);
    }
    
    // Set a 3-second timer to show controls
    const timer = setTimeout(() => {
      setShowControls(true);
    }, 3000);
    
    setHoverTimer(timer);
  };

  const handleMouseLeave = () => {
    // Clear the timer if mouse leaves before 3 seconds
    if (hoverTimer) {
      clearTimeout(hoverTimer);
      setHoverTimer(null);
    }
    
    // Hide controls immediately when mouse leaves
    setShowControls(false);
  };

  // Listen for backend navigation commands
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupEventListeners = async () => {
      try {
        // Listen for backend navigation commands
        unlisten = await listen('browser:navigate', (event) => {
          const { url: newUrl } = event.payload as { url: string };
          console.log('Received backend navigation command:', newUrl);
          setUrl(newUrl);
          setInputUrl(newUrl);
          setIsLoading(true);
        });
      } catch (error) {
        console.error('Failed to set up browser event listeners:', error);
      }
    };

    setupEventListeners();

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

      return (
      <Box height="100%" bg="rgba(0, 0, 0, 0.4)" display="flex" flexDirection="column">
      {/* Browser Toolbar - Hidden by default, shown after 3 second hover */}
      {showControls && (
        <Box 
          bg="gray.800" 
          borderBottom="1px solid" 
          borderColor="gray.700" 
          p={3}
          transition="all 0.3s ease"
          onMouseEnter={handleMouseEnter}
          onMouseLeave={handleMouseLeave}
        >
          <Stack direction="column" gap={3}>
            {/* Navigation Controls */}
            <Flex gap={2} align="center">
              <IconButton
                aria-label="Back"
                size="sm"
                variant="outline"
                colorScheme="gray"
                onClick={handleBack}
                disabled={!canGoBack}
              >
                <FaArrowLeft />
              </IconButton>
              <IconButton
                aria-label="Forward"
                size="sm"
                variant="outline"
                colorScheme="gray"
                onClick={handleForward}
                disabled={!canGoForward}
              >
                <FaArrowRight />
              </IconButton>
              <IconButton
                aria-label="Refresh"
                size="sm"
                variant="outline"
                colorScheme="gray"
                onClick={handleRefresh}
              >
                <FaRedo />
              </IconButton>
              <IconButton
                aria-label="Home"
                size="sm"
                variant="outline"
                colorScheme="blue"
                onClick={handleHome}
              >
                <FaHome />
              </IconButton>
            </Flex>

            {/* Address Bar */}
            <Flex gap={2} align="center">
              <Input
                placeholder="Enter URL..."
                value={inputUrl}
                onChange={(e) => setInputUrl(e.target.value)}
                onKeyPress={handleKeyPress}
                bg="gray.700"
                border="1px solid"
                borderColor="gray.600"
                color="white"
                _placeholder={{ color: 'gray.400' }}
                _focus={{
                  borderColor: 'blue.500',
                  boxShadow: '0 0 0 1px var(--chakra-colors-blue-500)',
                }}
              />
              <Button
                colorScheme="blue"
                size="md"
                onClick={handleNavigate}
                disabled={isLoading}
              >
                <FaSearch />
              </Button>
            </Flex>
          </Stack>
        </Box>
      )}

      {/* Invisible hover area at top for triggering controls */}
      <Box
        position="absolute"
        top="0"
        left="0"
        right="0"
        height="20px"
        zIndex={5}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
      />

      {/* Browser Content */}
      <Box flex="1" position="relative" overflow="hidden">
        {isLoading && (
          <Flex
            position="absolute"
            top="20px"
            left="50%"
            transform="translateX(-50%)"
            align="center"
            gap={2}
            bg="gray.800"
            px={4}
            py={2}
            borderRadius="md"
            zIndex={10}
          >
            <Spinner size="sm" color="blue.400" />
            <Text color="white" fontSize="sm">Loading...</Text>
          </Flex>
        )}
        
        {/* Embedded Website */}
        <iframe
          id="browser-iframe"
          src={url}
          style={{
            width: '100%',
            height: '100%',
            border: 'none',
          }}
          onLoad={handleIframeLoad}
          sandbox="allow-same-origin allow-scripts allow-popups allow-forms allow-top-navigation"
          title="KeepKey Browser"
        />
      </Box>
    </Box>
  );
}; 