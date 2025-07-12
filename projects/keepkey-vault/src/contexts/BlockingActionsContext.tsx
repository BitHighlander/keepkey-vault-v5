import React, { createContext, useContext, useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useBootloaderUpdateWizard, useFirmwareUpdateWizard, useTroubleshootingWizard } from './DialogContext';

// Types that mirror the Rust backend types
export enum BlockingActionType {
  MandatoryBootloaderUpdate = "mandatory_bootloader_update",
  FirmwareUpdate = "firmware_update",
  DeviceInitialization = "device_initialization",
  DeviceCommunicationFailure = "device_communication_failure"
  // Add more types here as they're added in the Rust backend
}

export interface BlockingAction {
  device_id: string;
  action_type: BlockingActionType;
  message: string;
  priority: number;
  current_version?: string;
  required_version?: string;
}

interface BlockingActionsContextType {
  actions: BlockingAction[];
  resolveAction: (deviceId: string, actionType: BlockingActionType) => Promise<boolean>;
  pendingActionsCount: number;
  bootloaderUpdateCount: number;
  firmwareUpdateCount: number;
  deviceInitializationCount: number;
  highestPriorityAction: BlockingAction | null;
}

const BlockingActionsContext = createContext<BlockingActionsContextType | undefined>(undefined);

export const BlockingActionsProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [actions, setActions] = useState<BlockingAction[]>([]);
  const [pendingActionsCount, setPendingActionsCount] = useState(0);
  const [bootloaderUpdateCount, setBootloaderUpdateCount] = useState(0);
  const [firmwareUpdateCount, setFirmwareUpdateCount] = useState(0);
  const [deviceInitializationCount, setDeviceInitializationCount] = useState(0);
  const [highestPriorityAction, setHighestPriorityAction] = useState<BlockingAction | null>(null);
  
  // Get dialog hooks
  const bootloaderUpdateWizard = useBootloaderUpdateWizard();
  const firmwareUpdateWizard = useFirmwareUpdateWizard();
  const troubleshootingWizard = useTroubleshootingWizard();

  // Function to fetch actions from backend
  const fetchActions = async () => {
    try {
      const allActions = await invoke<BlockingAction[]>('get_blocking_actions', { deviceId: null });
      console.log('Fetched blocking actions:', allActions);
      setActions(allActions);
      setPendingActionsCount(allActions.length);
    } catch (error) {
      console.error('Failed to fetch blocking actions:', error);
    }
  };

  // Fetch initial actions on mount
  useEffect(() => {
    fetchActions();
  }, []);

  // Listen for new blocking actions
  useEffect(() => {
    const unlistenPromise = listen<number>('blocking:actions_updated', (event) => {
      const count = event.payload;
      console.log(`Received blocking:actions_updated with count: ${count}`);
      setPendingActionsCount(count);
      // We'll fetch the actual actions to get their details
      fetchActions();
    });

    return () => {
      unlistenPromise.then(unlisten => unlisten());
    };
  }, []);
  
  // Calculate action counts by type whenever actions change
  useEffect(() => {
    // Count actions by type
    const bootloaderCount = actions.filter(
      a => a.action_type === BlockingActionType.MandatoryBootloaderUpdate
    ).length;
    
    const firmwareCount = actions.filter(
      a => a.action_type === BlockingActionType.FirmwareUpdate
    ).length;
    
    const initCount = actions.filter(
      a => a.action_type === BlockingActionType.DeviceInitialization
    ).length;
    
    setBootloaderUpdateCount(bootloaderCount);
    setFirmwareUpdateCount(firmwareCount);
    setDeviceInitializationCount(initCount);
    
    // Find highest priority action
    const highest = actions.length > 0 
      ? actions.reduce((prev, current) => 
          (current.priority > (prev?.priority || 0)) ? current : prev, null as BlockingAction | null)
      : null;
    
    setHighestPriorityAction(highest);
    
    console.log(`Actions updated: bootloader=${bootloaderCount}, firmware=${firmwareCount}, init=${initCount}, highest=${highest?.action_type || 'none'}`);
  }, [actions]);

  // Function to resolve an action
  const resolveAction = async (deviceId: string, actionType: BlockingActionType): Promise<boolean> => {
    try {
      const wasResolved = await invoke<boolean>('resolve_blocking_action', {
        deviceId,
        actionType
      });

      if (wasResolved) {
        // Remove the resolved action from local state
        setActions(prev => prev.filter(a => {
          console.debug('[BlockingActionsContext] Checking action with deviceId:', deviceId, 'against', a.device_id, a.action_type);
          return !(a.device_id === deviceId && a.action_type === actionType);
        }));

        // Update the count
        setPendingActionsCount(prev => prev - 1);
      }

      return wasResolved;
    } catch (error) {
      console.error(`Failed to resolve action ${actionType} for device ${deviceId}:`, error);
      return false;
    }
  };

  // Auto-show appropriate wizard based on highest priority action
  useEffect(() => {
    // Only process if we have a highest priority action
    if (highestPriorityAction) {
      console.log(`Processing highest priority action: ${highestPriorityAction.action_type}`);
      
      switch (highestPriorityAction.action_type) {
        case BlockingActionType.DeviceCommunicationFailure:
          // Check if wizard is already shown to prevent duplicate dialogs
          const isTroubleshootingWizardShown = troubleshootingWizard.isShowing(highestPriorityAction.device_id);
          
          if (!isTroubleshootingWizardShown) {
            console.log('Showing troubleshooting wizard for communication failure');
            troubleshootingWizard.show({
              deviceId: highestPriorityAction.device_id,
              errorDetails: highestPriorityAction.message,
              onResolved: async () => {
                // When communication is restored, resolve the action
                await resolveAction(highestPriorityAction.device_id, BlockingActionType.DeviceCommunicationFailure);
                // Re-fetch actions to get the next highest priority
                fetchActions();
              },
              onContactSupport: (diagnostics) => {
                console.log('User contacted support for communication issue:', diagnostics);
                // Handle support contact - could open email client, send to support system, etc.
              }
            });
          }
          break;
          
        case BlockingActionType.MandatoryBootloaderUpdate:
          // Check if wizard is already shown to prevent duplicate dialogs
          const isBootloaderWizardShown = bootloaderUpdateWizard.isShowing(highestPriorityAction.device_id);
          
          // Only show if we have both version information and wizard is not already shown
          if (!isBootloaderWizardShown && highestPriorityAction.current_version && highestPriorityAction.required_version) {
            console.log('Showing bootloader update wizard for blocking action');
            bootloaderUpdateWizard.show({
              deviceId: highestPriorityAction.device_id,
              currentVersion: highestPriorityAction.current_version,
              requiredVersion: highestPriorityAction.required_version,
              onWizardComplete: async (success: boolean, deviceId: string) => {
                // When the bootloader update is complete, resolve the action
                if (success) {
                  await resolveAction(deviceId, BlockingActionType.MandatoryBootloaderUpdate);
                }
                // Re-fetch actions to get the next highest priority
                fetchActions();
              }
            });
          }
          break;
          
        case BlockingActionType.FirmwareUpdate:
          // Don't show firmware update wizard if there's also a bootloader update needed
          const hasBootloaderAction = actions.some(
            a => a.device_id === highestPriorityAction.device_id && 
                 a.action_type === BlockingActionType.MandatoryBootloaderUpdate
          );
          
          // Check if wizard is already shown to prevent duplicate dialogs
          const isWizardShown = firmwareUpdateWizard.isShowing(highestPriorityAction.device_id);
          
          if (!hasBootloaderAction && !isWizardShown && highestPriorityAction.current_version && highestPriorityAction.required_version) {
            console.log('Showing firmware update wizard for blocking action');
            firmwareUpdateWizard.show({
              deviceId: highestPriorityAction.device_id,
              currentVersion: highestPriorityAction.current_version,
              targetVersion: highestPriorityAction.required_version,
              onComplete: async (success: boolean, deviceId: string) => {
                // When the firmware update is complete, resolve the action
                if (success) {
                  await resolveAction(deviceId, BlockingActionType.FirmwareUpdate);
                }
                // Re-fetch actions to get the next highest priority
                fetchActions();
              }
            });
          }
          break;
          
        // Handle other action types as needed
      }
    }
  }, [highestPriorityAction, actions, bootloaderUpdateWizard, firmwareUpdateWizard, troubleshootingWizard, fetchActions]);

  return (
    <BlockingActionsContext.Provider value={{
      actions,
      resolveAction,
      pendingActionsCount,
      bootloaderUpdateCount,
      firmwareUpdateCount,
      deviceInitializationCount,
      highestPriorityAction
    }}>
      {children}
    </BlockingActionsContext.Provider>
  );
};

// Hook for consuming the context
export const useBlockingActions = (): BlockingActionsContextType => {
  const context = useContext(BlockingActionsContext);
  if (context === undefined) {
    throw new Error('useBlockingActions must be used within a BlockingActionsProvider');
  }
  return context;
};
