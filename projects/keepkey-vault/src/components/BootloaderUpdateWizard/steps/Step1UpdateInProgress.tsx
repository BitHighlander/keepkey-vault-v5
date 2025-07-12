import React, { useEffect, useState } from 'react';
import { VStack, Text, Spinner, Box, Icon } from '@chakra-ui/react';
import { FaCog } from 'react-icons/fa';
import { invoke } from '@tauri-apps/api/core';
import type { StepProps } from '../BootloaderUpdateWizard';

export const Step1UpdateInProgress: React.FC<StepProps> = ({ 
  deviceId,
  onNext, 
  onError,
  onSetProgress,
  clearError
}) => {
  const [statusMessage, setStatusMessage] = useState('Initializing update...');

  useEffect(() => {
    clearError(); // Clear previous errors when entering this step
    const performUpdate = async () => {
      try {
        if (onSetProgress) onSetProgress({ value: 10, message: 'Preparing device...' });
        setStatusMessage('Preparing device for bootloader update...');
        
        // TODO: Replace with actual Tauri command to start bootloader update
        // This command should ideally emit progress events or return status updates.
        // For now, we simulate a multi-stage process.

        await new Promise(resolve => setTimeout(resolve, 2000)); // Simulate prep time
        if (onSetProgress) onSetProgress({ value: 30, message: 'Entering bootloader mode...' });
        setStatusMessage('Device entering bootloader mode...');
        // Example: await invoke('enter_bootloader_mode', { deviceId });

        await new Promise(resolve => setTimeout(resolve, 3000)); // Simulate mode switch
        if (onSetProgress) onSetProgress({ value: 50, message: 'Sending update payload...' });
        setStatusMessage('Sending update payload to device...');
        // Example: await invoke('send_bootloader_firmware', { deviceId });

        await new Promise(resolve => setTimeout(resolve, 5000)); // Simulate flashing
        if (onSetProgress) onSetProgress({ value: 80, message: 'Verifying update...' });
        setStatusMessage('Verifying update integrity...');
        // Example: await invoke('verify_bootloader_update', { deviceId });

        await new Promise(resolve => setTimeout(resolve, 2000)); // Simulate verification
        if (onSetProgress) onSetProgress({ value: 100, message: 'Update successful! Rebooting...' });
        setStatusMessage('Bootloader update successful! Device is rebooting.');
        
        // Example: await invoke('reboot_device_after_update', { deviceId });
        await new Promise(resolve => setTimeout(resolve, 3000)); // Simulate reboot

        onNext(); // Move to completion step
      } catch (err: any) {
        console.error('Bootloader update failed:', err);
        const errorMessage = err.message || 'An unknown error occurred during the update.';
        const errorAdvice = 'Please ensure your device remained connected. You may need to unplug and replug your device, then try the update again. If the problem persists, contact support.';
        if (onSetProgress) onSetProgress({ value: 100, message: `Error: ${errorMessage}`}); // Show error in progress too
        onError(errorMessage, errorAdvice);
      }
    };

    performUpdate();
  }, [deviceId, onNext, onError, onSetProgress, clearError]);

  return (
    <VStack align="center" justify="center" gap={6} h="100%">
      <Spinner size="xl" color="orange.500" />
      <VStack align="center">
        <Text fontSize="lg" fontWeight="semibold">Updating Bootloader...</Text>
        <Text fontSize="sm" color="gray.400">{statusMessage}</Text>
      </VStack>
      <Box 
        borderWidth="1px" 
        borderColor="gray.700" 
        borderRadius="md" 
        p={4} 
        w="full" 
        bg="gray.750"
      >
        <Text fontSize="xs" color="yellow.400" fontWeight="bold">
          <Icon as={FaCog} mr={2} animation="spin 2s linear infinite" />
          DO NOT DISCONNECT YOUR DEVICE.
        </Text>
        <Text fontSize="xs" color="gray.300" mt={1}>
          The update process is critical. Disconnecting your KeepKey now may render it unusable.
        </Text>
      </Box>
    </VStack>
  );
};
