import { Box } from '@chakra-ui/react';
import { BrowserView } from './BrowserView';

export const VaultView = () => {
  // VaultView now simply shows the webview that handles all wallet functionality
  return (
    <Box height="100%" width="100%">
      <BrowserView />
    </Box>
  );
}; 