import { useDialog } from '../contexts/DialogContext';
import { useCallback } from 'react';
import React from 'react';
import { OnboardingWizard } from '../components/OnboardingWizard/OnboardingWizard';
import { WalletCreationWizard } from '../components/WalletCreationWizard/WalletCreationWizard';

// Import dialog components dynamically
const dialogComponents = {
  onboarding: () => import('../components/OnboardingWizard/OnboardingWizard').then(m => ({ default: m.OnboardingWizard })),
  settings: () => import('../components/SettingsDialog').then(m => ({ default: m.SettingsDialog })),
  walletCreation: () => import('../components/WalletCreationWizard/WalletCreationWizard').then(m => ({ default: m.WalletCreationWizard })),
  // Add more dialog imports as needed
};

export function useCommonDialogs() {
  const { show, hide, hideAll, requestAppFocus, releaseAppFocus } = useDialog();
  
  const showOnboarding = useCallback((props?: any) => {
    show({
      id: 'onboarding',
      component: OnboardingWizard, // Direct component instead of lazy loading for testing
      props,
      priority: 'high',
      persistent: true,
      onOpen: () => {
        requestAppFocus();
      },
      onClose: () => {
        releaseAppFocus();
      },
    });
  }, [show, requestAppFocus, releaseAppFocus]);
  
  const showSettings = useCallback((props?: any) => {
    show({
      id: 'settings',
      component: React.lazy(dialogComponents.settings),
      props,
      priority: 'normal',
    });
  }, [show]);

  const showWalletCreation = useCallback((props?: {
    deviceId?: string;
    onWizardComplete?: () => void;
    onWizardClose?: () => void;
  }) => {
    console.log(`🏦 [useCommonDialogs] showWalletCreation called with props:`, props);
    console.trace('🏦 [useCommonDialogs] Call stack trace:');
    
    show({
      id: 'wallet-creation',
      component: WalletCreationWizard, // Direct component instead of lazy loading for better UX
      props: {
        ...props,
        onComplete: () => {
          console.log(`🏦 [useCommonDialogs] showWalletCreation onComplete called`);
          if (props?.onWizardComplete) props.onWizardComplete();
          hide('wallet-creation');
        },
        onClose: () => {
          console.log(`🏦 [useCommonDialogs] showWalletCreation onClose called`);
          if (props?.onWizardClose) props.onWizardClose();
          hide('wallet-creation');
        }
      },
      priority: 'critical', // High priority since it's device setup
      persistent: true, // User must complete flow
      onOpen: () => {
        requestAppFocus();
      },
      onClose: () => {
        releaseAppFocus();
      },
    });
  }, [show, hide, requestAppFocus, releaseAppFocus]);
  
  // TODO: Implement these dialogs
  const showError = useCallback((title: string, message: string) => {
    console.error('Error dialog not implemented yet:', title, message);
  }, []);
  
  const showConfirm = useCallback((title: string, message: string, onConfirm: () => void) => {
    console.warn('Confirm dialog not implemented yet:', title, message);
    // For now, just call the confirm callback
    onConfirm();
  }, []);
  
  return {
    showOnboarding,
    showSettings,
    showWalletCreation,
    showError,
    showConfirm,
    hideDialog: hide,
    hideAllDialogs: hideAll,
  };
} 