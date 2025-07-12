import React, { createContext, useContext, useState, useCallback, useRef, useEffect, Suspense, startTransition } from 'react';

export type DialogPriority = 'low' | 'normal' | 'high' | 'critical';

export interface DialogConfig {
  id: string;
  component: React.ComponentType<any>;
  props?: any;
  priority?: DialogPriority;
  persistent?: boolean; // If true, dialog can't be closed by clicking outside
  onClose?: () => void;
  onOpen?: () => void;
}

interface DialogState {
  queue: DialogConfig[];
  active: DialogConfig | null;
  history: string[];
}

interface DialogContextType {
  // Core dialog management
  show: (config: DialogConfig) => void;
  hide: (id: string) => void;
  hideAll: () => void;
  hideAllExcept: (id: string) => void;
  
  // Queue management
  getQueue: () => DialogConfig[];
  isShowing: (id: string) => boolean;
  
  // Current dialog
  activeDialog: DialogConfig | null;
  
  // App focus management
  requestAppFocus: () => void;
  releaseAppFocus: () => void;
}

const DialogContext = createContext<DialogContextType | null>(null);

const PRIORITY_ORDER: Record<DialogPriority, number> = {
  low: 0,
  normal: 1,
  high: 2,
  critical: 3,
};

export function DialogProvider({ children }: { children: React.ReactNode }) {
  const [state, setState] = useState<DialogState>({
    queue: [],
    active: null,
    history: [],
  });
  
  const focusCount = useRef(0);

  // Process queue to determine which dialog should be active
  const processQueue = useCallback((queue: DialogConfig[]) => {
    if (queue.length === 0) {
      return null;
    }
    
    // First check if there's a PIN dialog - it always has highest priority
    const pinDialog = queue.find(d => d.id.includes('pin-unlock'));
    if (pinDialog) {
      return pinDialog;
    }
    
    // Sort by priority (highest first)
    const sorted = [...queue].sort((a, b) => {
      const priorityA = PRIORITY_ORDER[a.priority || 'normal'];
      const priorityB = PRIORITY_ORDER[b.priority || 'normal'];
      return priorityB - priorityA;
    });
    
    return sorted[0];
  }, []);

  // Show a dialog
  const show = useCallback((config: DialogConfig) => {
    console.log(`🎯 [DialogContext] show() called for dialog:`, config.id, config.priority);
    console.log(`🎯 [DialogContext] Current queue before:`, state.queue.map(d => d.id));
    console.log(`🎯 [DialogContext] Current active before:`, state.active?.id);
    
    // Special handling for PIN dialog - it should always take priority when device is ready
    const isPinDialog = config.id.includes('pin-unlock');
    
    // Use startTransition to avoid synchronous suspense issues with lazy components
    startTransition(() => {
      setState((prevState) => {
        // Check if dialog already exists in queue
        const exists = prevState.queue.some(d => d.id === config.id);
        if (exists) {
          console.warn(`🎯 [DialogContext] Dialog with id "${config.id}" already exists in queue`);
          return prevState;
        }
        
        // If this is a critical dialog or PIN dialog, remove lower priority dialogs
        let newQueue = [...prevState.queue];
        if (config.priority === 'critical' || isPinDialog) {
          // Remove non-critical dialogs except for PIN dialogs
          newQueue = newQueue.filter(d => {
            const dialogPriority = PRIORITY_ORDER[d.priority || 'normal'];
            const configPriority = PRIORITY_ORDER[config.priority || 'normal'];
            const isDialogPin = d.id.includes('pin-unlock');
            
            // Keep dialog if:
            // 1. It's a PIN dialog (PIN dialogs are always kept)
            // 2. It has equal or higher priority than the new dialog
            return isDialogPin || dialogPriority >= configPriority;
          });
          
          // Call onClose for removed dialogs
          prevState.queue.forEach(dialog => {
            if (!newQueue.some(d => d.id === dialog.id) && dialog.onClose) {
              console.log(`🎯 [DialogContext] Closing lower priority dialog:`, dialog.id);
              dialog.onClose();
            }
          });
        }
        
        // Add the new dialog
        newQueue.push(config);
        
        // Process queue with special PIN handling
        let newActive = processQueue(newQueue);
        
        // If a PIN dialog is in the queue and device is ready, it should always be active
        const pinDialog = newQueue.find(d => d.id.includes('pin-unlock'));
        if (pinDialog && (isPinDialog || !prevState.active?.id.includes('pin-unlock'))) {
          newActive = pinDialog;
        }
        
        console.log(`🎯 [DialogContext] New queue:`, newQueue.map(d => d.id));
        console.log(`🎯 [DialogContext] New active:`, newActive?.id);
        
        // Call onOpen if this dialog becomes active
        if (newActive?.id === config.id && config.onOpen) {
          console.log(`🎯 [DialogContext] Calling onOpen for:`, config.id);
          config.onOpen();
        }
        
        return {
          ...prevState,
          queue: newQueue,
          active: newActive,
          history: [...prevState.history, config.id],
        };
      });
    });
  }, [processQueue]);

  // Hide a specific dialog
  const hide = useCallback((id: string) => {
    setState((prevState) => {
      const dialog = prevState.queue.find(d => d.id === id);
      const newQueue = prevState.queue.filter(d => d.id !== id);
      const wasActive = prevState.active?.id === id;
      const newActive = processQueue(newQueue);
      
      // Call onClose if dialog was found
      if (dialog?.onClose) {
        dialog.onClose();
      }
      
      // If active dialog changed, call onOpen for new active
      if (wasActive && newActive && newActive.onOpen) {
        newActive.onOpen();
      }
      
      return {
        ...prevState,
        queue: newQueue,
        active: newActive,
      };
    });
  }, [processQueue]);

  // Hide all dialogs
  const hideAll = useCallback(() => {
    setState((prevState) => {
      // Call onClose for all dialogs
      prevState.queue.forEach(dialog => {
        if (dialog.onClose) {
          dialog.onClose();
        }
      });
      
      return {
        ...prevState,
        queue: [],
        active: null,
      };
    });
  }, []);

  // Hide all dialogs except a specific one
  const hideAllExcept = useCallback((id: string) => {
    setState((prevState) => {
      const dialogToKeep = prevState.queue.find(d => d.id === id);
      if (!dialogToKeep) {
        return prevState;
      }
      
      // Call onClose for all other dialogs
      prevState.queue.forEach(dialog => {
        if (dialog.id !== id && dialog.onClose) {
          dialog.onClose();
        }
      });
      
      return {
        ...prevState,
        queue: [dialogToKeep],
        active: dialogToKeep,
      };
    });
  }, []);

  // Get current queue
  const getQueue = useCallback(() => {
    return state.queue;
  }, [state.queue]);

  // Check if a dialog is showing
  const isShowing = useCallback((id: string) => {
    return state.active?.id === id;
  }, [state.active]);

  // Request app focus (for critical dialogs)
  const requestAppFocus = useCallback(async () => {
    focusCount.current += 1;
    
    if (focusCount.current === 1) {
      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const appWindow = getCurrentWindow();
        await appWindow.setFocus();
        await appWindow.setAlwaysOnTop(true);
      } catch (error) {
        console.error('Failed to request app focus:', error);
      }
    }
  }, []);

  // Release app focus
  const releaseAppFocus = useCallback(async () => {
    focusCount.current = Math.max(0, focusCount.current - 1);
    
    if (focusCount.current === 0) {
      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const appWindow = getCurrentWindow();
        await appWindow.setAlwaysOnTop(false);
      } catch (error) {
        console.error('Failed to release app focus:', error);
      }
    }
  }, []);

  const value: DialogContextType = {
    show,
    hide,
    hideAll,
    hideAllExcept,
    getQueue,
    isShowing,
    activeDialog: state.active,
    requestAppFocus,
    releaseAppFocus,
  };

  return (
    <DialogContext.Provider value={value}>
      {children}
      {/* Render active dialog */}
      {state.active && (() => {
        console.log('🎭 [DialogContext] Rendering active dialog:', state.active.id, 'Component:', state.active.component);
        return (
          <div 
            style={{ 
              position: 'fixed', 
              top: 0, 
              left: 0, 
              right: 0, 
              bottom: 0, 
              zIndex: state.active.id.includes('pin-unlock') ? 99999 : 9999,
              backgroundColor: 'rgba(0, 0, 0, 0.5)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              pointerEvents: 'auto'
            }}
          >
            <DialogRenderer
              dialog={state.active}
              onClose={() => hide(state.active!.id)}
            />
          </div>
        );
      })()}
    </DialogContext.Provider>
  );
}

// Error Boundary class component for handling dialog errors
class DialogErrorBoundary extends React.Component<
  { children: React.ReactNode; onClose: () => void; dialogId: string },
  { hasError: boolean; error?: Error }
> {
  constructor(props: { children: React.ReactNode; onClose: () => void; dialogId: string }) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(error: Error) {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error('Dialog Error Boundary caught an error:', error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div style={{ 
          padding: '20px', 
          color: 'white',
          backgroundColor: 'rgba(139, 0, 0, 0.9)',
          borderRadius: '8px',
          maxWidth: '500px'
        }}>
          <h2 style={{ margin: '0 0 10px 0', color: '#ffcccb' }}>Dialog Error</h2>
          <p style={{ margin: '0 0 10px 0', fontSize: '14px' }}>
            Failed to load dialog: {this.props.dialogId}
          </p>
          <pre style={{ 
            fontSize: '12px', 
            backgroundColor: 'rgba(0, 0, 0, 0.3)', 
            padding: '10px', 
            borderRadius: '4px',
            overflow: 'auto',
            maxHeight: '200px'
          }}>
            {this.state.error?.message || 'Unknown error'}
          </pre>
          <button 
            onClick={this.props.onClose}
            style={{
              marginTop: '15px',
              padding: '8px 16px',
              backgroundColor: '#dc3545',
              color: 'white',
              border: 'none',
              borderRadius: '4px',
              cursor: 'pointer'
            }}
          >
            Close
          </button>
        </div>
      );
    }

    return this.props.children;
  }
}

// Dialog renderer component with proper Suspense and error boundary handling
function DialogRenderer({ dialog, onClose }: { dialog: DialogConfig; onClose: () => void }) {
  const Component = dialog.component;
  
  const handleClose = useCallback(() => {
    if (!dialog.persistent) {
      onClose();
    }
  }, [dialog.persistent, onClose]);
  
  // Loading fallback for lazy components
  const LoadingFallback = () => (
    <div style={{ 
      padding: '40px', 
      textAlign: 'center', 
      color: 'white',
      backgroundColor: 'rgba(0, 0, 0, 0.8)',
      borderRadius: '8px',
      minWidth: '300px'
    }}>
      <div style={{ marginBottom: '16px', fontSize: '18px' }}>Loading...</div>
      <div style={{ fontSize: '14px', opacity: 0.7 }}>{dialog.id}</div>
    </div>
  );
  
  return (
    <DialogErrorBoundary onClose={onClose} dialogId={dialog.id}>
      <Suspense fallback={<LoadingFallback />}>
        <Component {...dialog.props} onClose={handleClose} />
      </Suspense>
    </DialogErrorBoundary>
  );
}

// Hook to use dialog context
export function useDialog() {
  const context = useContext(DialogContext);
  if (!context) {
    throw new Error('useDialog must be used within a DialogProvider');
  }
  return context;
}

// Pre-configured dialog types
// Pre-configured dialog for Bootloader Update Wizard
export function useBootloaderUpdateWizard() {
  const { show, hide, isShowing } = useDialog();
  return {
    show: (props: Omit<import('../components/BootloaderUpdateWizard/BootloaderUpdateWizard').BootloaderUpdateWizardProps, 'onClose' | 'onComplete'> & { 
      onWizardClose?: () => void; 
      onWizardComplete?: (success: boolean, deviceId: string) => void; 
    }) => {
      const dialogId = `bootloader-update-wizard-${props.deviceId}`;
      show({
        id: dialogId,
        component: React.lazy(() => import('../components/BootloaderUpdateWizard/BootloaderUpdateWizard').then(m => ({ default: m.BootloaderUpdateWizard }))),
        props: {
          ...props,
          onClose: () => {
            if (props.onWizardClose) props.onWizardClose();
            hide(dialogId); // Ensure default hide is called
          },
          onComplete: (success: boolean, deviceId: string) => {
            if (props.onWizardComplete) props.onWizardComplete(success, deviceId);
            hide(dialogId); // Ensure default hide is called
          }
        },
        priority: 'critical',
        persistent: true, // User cannot click away, must complete or explicitly close via wizard actions
      });
    },
    hide: (deviceId: string) => hide(`bootloader-update-wizard-${deviceId}`),
    isShowing: (deviceId: string) => isShowing(`bootloader-update-wizard-${deviceId}`),
  };
}

// Pre-configured dialog for Firmware Update Wizard
export function useFirmwareUpdateWizard() {
  const { show, hide, isShowing } = useDialog();
  return {
    show: (props: {
      deviceId: string;
      currentVersion: string;
      targetVersion: string;
      onWizardClose?: () => void;
      onComplete?: (success: boolean, deviceId: string) => void;
    }) => {
      const dialogId = `firmware-update-wizard-${props.deviceId}`;
      show({
        id: dialogId,
        component: React.lazy(() => import('../components/FirmwareUpdateWizard/FirmwareUpdateWizard').then(m => ({ default: m.FirmwareUpdateWizard }))),
        props: {
          ...props,
          onClose: () => {
            if (props.onWizardClose) props.onWizardClose();
            hide(dialogId);
          },
          onComplete: (success: boolean, deviceId: string) => {
            if (props.onComplete) props.onComplete(success, deviceId);
            hide(dialogId);
          }
        },
        priority: 'high', // High but not as high as bootloader updates
        persistent: true,
      });
    },
    hide: (deviceId: string) => hide(`firmware-update-wizard-${deviceId}`),
    isShowing: (deviceId: string) => isShowing(`firmware-update-wizard-${deviceId}`),
  };
}

// Pre-configured dialog for Troubleshooting Wizard
export function useTroubleshootingWizard() {
  const { show, hide, isShowing } = useDialog();
  return {
    show: (props: {
      deviceId: string;
      errorDetails: string;
      onResolved?: () => void;
      onContactSupport?: (diagnostics: any) => void;
    }) => {
      const dialogId = `troubleshooting-wizard-${props.deviceId}`;
      show({
        id: dialogId,
        component: React.lazy(() => import('../components/TroubleshootingWizard/TroubleshootingWizard').then(m => ({ default: m.TroubleshootingWizard }))),
        props: {
          ...props,
          onClose: () => {
            hide(dialogId);
          },
          onResolved: () => {
            if (props.onResolved) props.onResolved();
            hide(dialogId);
          },
          onContactSupport: (diagnostics: any) => {
            if (props.onContactSupport) props.onContactSupport(diagnostics);
            // Don't auto-hide here - let user choose
          }
        },
        priority: 'critical', // Highest priority - communication issues block everything
        persistent: true,
      });
    },
    hide: (deviceId: string) => hide(`troubleshooting-wizard-${deviceId}`),
    isShowing: (deviceId: string) => isShowing(`troubleshooting-wizard-${deviceId}`),
  };
}

export function useOnboardingDialog() {
  const { show, hide } = useDialog();
  
  return {
    show: (props?: any) => {
      console.log(`📚 [OnboardingDialog] show() called with props:`, props);
      console.trace('📚 [OnboardingDialog] Call stack trace:');
      
      show({
        id: 'onboarding',
        component: React.lazy(() => import('../components/OnboardingWizard/OnboardingWizard').then(m => ({ default: m.OnboardingWizard }))),
        props,
        priority: 'high',
        persistent: true,
      });
    },
    hide: () => hide('onboarding'),
  };
}

// Pre-configured dialog for Wallet Creation Wizard
export function useWalletCreationWizard() {
  const { show, hide, isShowing } = useDialog();
  return {
    show: (props: {
      deviceId: string;
      onWizardClose?: () => void;
      onWizardComplete?: () => void;
    }) => {
      const dialogId = `wallet-creation-wizard-${props.deviceId}`;
      console.log(`💰 [WalletCreationWizard] show() called for device:`, props.deviceId);
      console.log(`💰 [WalletCreationWizard] Dialog ID:`, dialogId);
      
      show({
        id: dialogId,
        component: React.lazy(() => import('../components/WalletCreationWizard/WalletCreationWizard').then(m => ({ default: m.WalletCreationWizard }))),
        props: {
          ...props,
          onClose: () => {
            console.log(`💰 [WalletCreationWizard] onClose called for:`, dialogId);
            if (props.onWizardClose) props.onWizardClose();
            hide(dialogId);
          },
          onComplete: () => {
            console.log(`💰 [WalletCreationWizard] onComplete called for:`, dialogId);
            if (props.onWizardComplete) props.onWizardComplete();
            hide(dialogId);
          }
        },
        priority: 'critical', // High priority since it's device setup
        persistent: true, // User must complete flow
      });
      
      console.log(`💰 [WalletCreationWizard] show() config sent to DialogContext`);
    },
    hide: (deviceId: string) => hide(`wallet-creation-wizard-${deviceId}`),
    isShowing: (deviceId: string) => isShowing(`wallet-creation-wizard-${deviceId}`),
  };
}

// Pre-configured dialog for Device Invalid State
export function useDeviceInvalidStateDialog() {
  const { show, hide, isShowing } = useDialog();
  return {
    show: (props: {
      deviceId: string;
      error?: string;
      onDialogClose?: () => void;
    }) => {
      const dialogId = `device-invalid-state-${props.deviceId}`;
      
      // Always hide any existing troubleshooting wizards first
      hide(`troubleshooting-wizard-${props.deviceId}`);
      
      show({
        id: dialogId,
        component: React.lazy(() => import('../components/DeviceInvalidStateDialog').then(m => ({ default: m.DeviceInvalidStateDialog }))),
        props: {
          ...props,
          onClose: () => {
            if (props.onDialogClose) props.onDialogClose();
            hide(dialogId);
          }
        },
        priority: 'high', // High priority to ensure it shows above other dialogs
        persistent: true, // User must acknowledge before closing
      });
    },
    hide: (deviceId: string) => hide(`device-invalid-state-${deviceId}`),
    isShowing: (deviceId: string) => isShowing(`device-invalid-state-${deviceId}`),
  };
}