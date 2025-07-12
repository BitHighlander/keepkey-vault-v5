// Chakra UI v3 doesn't export Modal components from main package
// Creating basic modal replacements for now
import React from 'react';
import { 
  Box, 
  Button, 
  CloseButton, 
  useDisclosure as useDisclosureChakra,
  Portal,
  BoxProps,
  ButtonProps
} from '@chakra-ui/react';

// Basic modal implementations
export const Modal = ({ isOpen, onClose, children, ...props }: { isOpen: boolean; onClose: () => void; children: React.ReactNode } & BoxProps) => {
  if (!isOpen) return null;
  
  return (
    <Portal>
      <Box
        position="fixed"
        top={0}
        left={0}
        right={0}
        bottom={0}
        bg="blackAlpha.600"
        display="flex"
        alignItems="center"
        justifyContent="center"
        zIndex={1000}
        onClick={onClose}
        {...props}
      >
        {children}
      </Box>
    </Portal>
  );
};

export const ModalOverlay = ({ children, ...props }: { children?: React.ReactNode } & BoxProps) => (
  <Box {...props}>{children}</Box>
);

export const ModalContent = ({ children, ...props }: { children: React.ReactNode } & BoxProps) => (
  <Box
    bg="white"
    borderRadius="md"
    p={6}
    m={4}
    maxW="md"
    w="full"
    maxH="90vh"
    overflowY="auto"
    onClick={(e) => e.stopPropagation()}
    {...props}
  >
    {children}
  </Box>
);

export const ModalHeader = ({ children, ...props }: { children: React.ReactNode } & BoxProps) => (
  <Box fontSize="lg" fontWeight="bold" mb={4} {...props}>
    {children}
  </Box>
);

export const ModalBody = ({ children, ...props }: { children: React.ReactNode } & BoxProps) => (
  <Box {...props}>
    {children}
  </Box>
);

export const ModalFooter = ({ children, ...props }: { children: React.ReactNode } & BoxProps) => (
  <Box mt={6} display="flex" gap={2} justifyContent="flex-end" {...props}>
    {children}
  </Box>
);

export const ModalCloseButton = ({ onClick, ...props }: { onClick?: () => void } & ButtonProps) => (
  <CloseButton
    position="absolute"
    right={4}
    top={4}
    onClick={onClick}
    {...props}
  />
);

export const useDisclosure = useDisclosureChakra; 