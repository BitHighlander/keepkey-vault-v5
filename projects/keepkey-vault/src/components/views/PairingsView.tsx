import { useState } from 'react';
import { 
  Box, 
  Text, 
  Button, 
  Flex, 
  Stack,
  Badge
} from '@chakra-ui/react';
import { FaLink, FaTrash, FaPlus, FaCheck, FaClock } from 'react-icons/fa';

interface Pairing {
  id: string;
  name: string;
  domain: string;
  status: 'connected' | 'pending' | 'disconnected';
  lastConnected: Date;
  description?: string;
}

const mockPairings: Pairing[] = [
  {
    id: '1',
    name: 'ShapeShift',
    domain: 'app.shapeshift.com',
    status: 'connected',
    lastConnected: new Date(),
    description: 'Decentralized trading platform',
  },
  {
    id: '2',
    name: 'Uniswap',
    domain: 'app.uniswap.org',
    status: 'pending',
    lastConnected: new Date(Date.now() - 1000 * 60 * 30), // 30 minutes ago
    description: 'Decentralized exchange protocol',
  },
  {
    id: '3',
    name: 'MetaMask',
    domain: 'metamask.io',
    status: 'disconnected',
    lastConnected: new Date(Date.now() - 1000 * 60 * 60 * 24), // 1 day ago
    description: 'Ethereum wallet',
  },
];

export const PairingsView = () => {
  const [pairings, setPairings] = useState<Pairing[]>(mockPairings);

  const handleRemovePairing = (id: string) => {
    setPairings(prev => prev.filter(pairing => pairing.id !== id));
  };

  const handleAddPairing = () => {
    // TODO: Implement add pairing functionality
    console.log('Add pairing clicked');
  };

  const getStatusColor = (status: Pairing['status']) => {
    switch (status) {
      case 'connected':
        return 'green';
      case 'pending':
        return 'yellow';
      case 'disconnected':
        return 'gray';
      default:
        return 'gray';
    }
  };

  const getStatusIcon = (status: Pairing['status']) => {
    switch (status) {
      case 'connected':
        return <FaCheck />;
      case 'pending':
        return <FaClock />;
      case 'disconnected':
        return <FaLink />;
      default:
        return <FaLink />;
    }
  };

  const formatLastConnected = (date: Date) => {
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / (1000 * 60));
    const diffHours = Math.floor(diffMs / (1000 * 60 * 60));
    const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

    if (diffMins < 1) return 'Just now';
    if (diffMins < 60) return `${diffMins}m ago`;
    if (diffHours < 24) return `${diffHours}h ago`;
    return `${diffDays}d ago`;
  };

  return (
    <Box p={6} height="100%" bg="rgba(0, 0, 0, 0.4)">
      <Stack direction="column" gap={6} height="100%">
        {/* Header */}
        <Flex justify="space-between" align="center">
          <Stack direction="column" gap={1}>
            <Text fontSize="2xl" fontWeight="bold" color="white">
              Pairings
            </Text>
            <Text fontSize="md" color="gray.400">
              Manage your device connections to dApps and services
            </Text>
          </Stack>
          <Button
            colorScheme="blue"
            variant="outline"
            size="sm"
            onClick={handleAddPairing}
          >
            <FaPlus style={{ marginRight: '8px' }} />
            Add Pairing
          </Button>
        </Flex>

        {/* Pairings List */}
        <Box flex="1" overflowY="auto">
          {pairings.length === 0 ? (
            <Flex
              height="100%"
              alignItems="center"
              justifyContent="center"
              direction="column"
              gap={4}
            >
              <Text fontSize="lg" color="gray.500">
                No pairings found
              </Text>
              <Text fontSize="sm" color="gray.600" textAlign="center">
                Connect your KeepKey to dApps to see pairings here
              </Text>
              <Button colorScheme="blue" onClick={handleAddPairing}>
                <FaPlus style={{ marginRight: '8px' }} />
                Add First Pairing
              </Button>
            </Flex>
          ) : (
            <Stack direction="column" gap={4}>
              {pairings.map((pairing) => (
                <Box 
                  key={pairing.id} 
                  bg="gray.800" 
                  border="1px solid" 
                  borderColor="gray.700"
                  borderRadius="md"
                  p={4}
                >
                  <Flex justify="space-between" align="center">
                    <Stack direction="column" gap={2} flex="1">
                      <Flex align="center" gap={3}>
                        <Text fontSize="lg" fontWeight="semibold" color="white">
                          {pairing.name}
                        </Text>
                        <Badge
                          colorScheme={getStatusColor(pairing.status)}
                          variant="solid"
                          display="flex"
                          alignItems="center"
                          gap={1}
                        >
                          {getStatusIcon(pairing.status)}
                          {pairing.status}
                        </Badge>
                      </Flex>
                      
                      <Text fontSize="sm" color="gray.400">
                        {pairing.domain}
                      </Text>
                      
                      {pairing.description && (
                        <Text fontSize="xs" color="gray.500">
                          {pairing.description}
                        </Text>
                      )}
                      
                      <Text fontSize="xs" color="gray.600">
                        Last connected: {formatLastConnected(pairing.lastConnected)}
                      </Text>
                    </Stack>
                    
                    <Button
                      size="sm"
                      variant="outline"
                      colorScheme="red"
                      onClick={() => handleRemovePairing(pairing.id)}
                    >
                      <FaTrash />
                    </Button>
                  </Flex>
                </Box>
              ))}
            </Stack>
          )}
        </Box>

        {/* Footer Info */}
        <Box pt={4} borderTop="1px solid" borderColor="gray.700">
          <Text fontSize="xs" color="gray.600" textAlign="center">
            Pairings allow dApps to connect to your KeepKey device for secure transactions
          </Text>
        </Box>
      </Stack>
    </Box>
  );
}; 