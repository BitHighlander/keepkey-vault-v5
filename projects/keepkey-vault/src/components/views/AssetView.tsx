import { Box } from '@chakra-ui/react';
import { BrowserView } from './BrowserView';

export const AssetView: React.FC = () => {
  // AssetView now simply shows the webview that handles all asset functionality
  return (
    <Box height="100%" width="100%">
      <BrowserView />
    </Box>
  );
}; 