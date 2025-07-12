import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FactoryState } from "./FactoryState";
import { DeviceLabel } from "./DeviceLabel";
import { DevicePin } from "./DevicePin";
import { RecoverySettings, RecoverySettings as RecoverySettingsType } from "./RecoverySettings";
import { RecoveryFlow } from "./RecoveryFlow";
import { WalletFlowStep } from "../../constants/WalletFlowRoutes";
import { PinCreationSession } from "../../types/pin";

interface WalletCreationWizardProps {
  deviceId?: string;
  onComplete?: () => void;
  onClose?: () => void;
}

interface RecoverySession {
  session_id: string;
  device_id: string;
  word_count: number;
  current_word: number;
  current_character: number;
  is_active: boolean;
}

interface FlowState {
  step: WalletFlowStep | 'recovery-settings' | 'recovery-flow' | 'recovery-complete';
  deviceLabel: string;
  pinSession: PinCreationSession | null;
  recoverySettings: RecoverySettingsType | null;
  recoverySession: RecoverySession | null;
  isLoading: boolean;
  error: string | null;
  flowType: 'create' | 'recovery' | null;
}

export function WalletCreationWizard({ 
  deviceId = "mock-device-id", 
  onComplete, 
  onClose 
}: WalletCreationWizardProps) {
  console.debug('[WalletCreationWizard] deviceId prop on mount:', deviceId);
  const [state, setState] = useState<FlowState>({
    step: 'factory-state',
    deviceLabel: '',
    pinSession: null,
    recoverySettings: null,
    recoverySession: null,
    isLoading: false,
    error: null,
    flowType: null,
  });

  const updateState = useCallback((updates: Partial<FlowState>) => {
    if (updates.step) {
      console.log(`🔄 STEP CHANGE: ${state.step} → ${updates.step} (flowType: ${state.flowType})`);
    }
    setState(prev => ({ ...prev, ...updates }));
  }, [state.step, state.flowType]);

  const setLoading = useCallback((loading: boolean, error: string | null = null) => {
    updateState({ isLoading: loading, error });
  }, [updateState]);

  // Handle create wallet selection from factory state
  const handleCreateWallet = useCallback(() => {
    console.log("Starting wallet creation flow");
    updateState({ step: 'label', flowType: 'create' });
  }, [updateState]);

  // Handle recover wallet selection from factory state
  const handleRecoverWallet = useCallback(() => {
    console.log("Starting wallet recovery flow");
    updateState({ step: 'recovery-settings', flowType: 'recovery' });
  }, [updateState]);

  // Handle recovery settings completion
  const handleRecoverySettingsComplete = useCallback(async (settings: RecoverySettingsType) => {
    console.log("🔄 Recovery settings completed:", settings);
    
    // Store settings and move to recovery flow
    updateState({
      recoverySettings: settings,
      step: 'recovery-flow',
      flowType: 'recovery',
      isLoading: false,
      error: null
    });
  }, [updateState]);

  // Handle recovery phrase entry completion
  const handleRecoveryComplete = useCallback(async () => {
    console.log("Recovery phrase entry completed");
    setLoading(true);
    
    try {
      updateState({
        step: 'recovery-complete',
        isLoading: false,
        error: null
      });
      
      // Complete recovery
      setTimeout(() => {
        if (onComplete) {
          onComplete();
        }
      }, 2000);
    } catch (error) {
      console.error("Failed to complete recovery:", error);
      setLoading(false, `Failed to complete recovery: ${error}`);
    }
  }, [setLoading, updateState, onComplete]);

  // Handle recovery errors
  const handleRecoveryError = useCallback((error: string) => {
    console.error("Recovery error:", error);
    setLoading(false, error);
  }, [setLoading]);



  // Handle device label completion
  const handleLabelComplete = useCallback(async (label: string) => {
    console.log(`Setting device label: "${label}"`);
    setLoading(true);
    
    try {
      if (label.trim()) {
        await invoke('set_device_label', { deviceId, label: label.trim() });
      }
      updateState({ 
        deviceLabel: label.trim(), 
        step: 'pin',
        isLoading: false,
        error: null 
      });
    } catch (error) {
      console.error("Failed to set device label:", error);
      setLoading(false, `Failed to set device label: ${error}`);
    }
  }, [deviceId, setLoading, updateState]);

  // Handle device label skip
  const handleLabelSkip = useCallback(() => {
    console.log("Skipping device label");
    updateState({ deviceLabel: '', step: 'pin' });
  }, [updateState]);

  // Handle PIN creation completion (both create and confirm)
  const handlePinComplete = useCallback(async (session: PinCreationSession) => {
    console.log("🏗️ WALLET CREATION PIN COMPLETION - handlePinComplete called with session:", session);
    console.log("🏗️ Current state flow type:", state.flowType);
    console.log("🏗️ Current step:", state.step);
    console.log("🏗️ This should ONLY be called for wallet creation, not recovery!");
    
    try {
      console.log("🏗️ Setting step to backup-display for wallet creation");
      updateState({ 
        pinSession: session,
        step: 'backup-display',
        isLoading: false,
        error: null 
      });
      
      // Proceed to device initialization
      await handleDeviceInitialization();
    } catch (error) {
      console.error("Failed to proceed after PIN creation:", error);
      setLoading(false, `Failed to proceed after PIN creation: ${error}`);
    }
  }, [setLoading, updateState]);

  // Handle device initialization
  const handleDeviceInitialization = useCallback(async () => {
    console.debug('[WalletCreationWizard] handleDeviceInitialization using deviceId:', deviceId);
    console.log("Initializing device wallet");
    setLoading(true);
    
    try {
      console.debug('[WalletCreationWizard] Invoking initialize_device_wallet with deviceId:', deviceId);
      await invoke('initialize_device_wallet', { 
        deviceId, 
        label: state.deviceLabel || 'KeepKey' 
      });
      
      updateState({ 
        step: 'backup-display',
        isLoading: false,
        error: null 
      });
    } catch (error) {
      console.error("Failed to initialize device:", error);
      setLoading(false, `Failed to initialize device: ${error}`);
    }
  }, [deviceId, state.deviceLabel, setLoading, updateState]);

  // Handle backup phrase display completion
  const handleBackupComplete = useCallback(async () => {
    console.log("Completing wallet creation");
    setLoading(true);
    
    try {
      await invoke('complete_wallet_creation', { deviceId });
      
      updateState({ 
        step: 'complete',
        isLoading: false,
        error: null 
      });
      
      // Call completion callback after a short delay
      setTimeout(() => {
        if (onComplete) {
          onComplete();
        }
      }, 2000);
    } catch (error) {
      console.error("Failed to complete wallet creation:", error);
      setLoading(false, `Failed to complete wallet creation: ${error}`);
    }
  }, [deviceId, setLoading, updateState, onComplete]);

  // Handle back navigation
  const handleBack = useCallback(() => {
    switch (state.step) {
      case 'label':
        updateState({ step: 'factory-state', flowType: null });
        break;
      case 'pin':
        updateState({ step: 'label' });
        break;
      case 'recovery-settings':
        updateState({ step: 'factory-state', flowType: null });
        break;
      case 'recovery-flow':
        updateState({ step: 'recovery-settings' });
        break;
      default:
        // For other steps, we don't allow going back
        break;
    }
  }, [state.step, updateState]);

  // Render current step
  const renderCurrentStep = () => {
    console.log(`🎭 RENDERING STEP: ${state.step} (flowType: ${state.flowType})`);
    console.log(`🎭 Full state:`, state);
    
    // SAFETY CHECK: If we're in recovery flow but somehow got to a wallet creation step, redirect
    if (state.flowType === 'recovery' && (state.step === 'backup-display' || state.step === 'complete')) {
      console.error(`❌ FLOW ERROR: Recovery flow should never reach ${state.step}! Redirecting to recovery-flow`);
      if (state.recoverySettings) {
        updateState({ step: 'recovery-flow' });
        return <div>Redirecting to recovery...</div>;
      } else {
        console.error(`❌ No recovery settings available, redirecting to recovery settings`);
        updateState({ step: 'recovery-settings' });
        return <div>Redirecting to recovery settings...</div>;
      }
    }
    
    switch (state.step) {
      case 'factory-state':
        return (
          <FactoryState
            onCreateWallet={handleCreateWallet}
            onRecoverWallet={handleRecoverWallet}
            onClose={onClose}
          />
        );

      case 'recovery-settings':
        return (
          <RecoverySettings
            onComplete={handleRecoverySettingsComplete}
            onBack={handleBack}
            isLoading={state.isLoading}
            error={state.error}
          />
        );



      case 'recovery-flow':
        return state.recoverySettings ? (
          <RecoveryFlow
            deviceId={deviceId}
            wordCount={state.recoverySettings.wordCount}
            passphraseProtection={state.recoverySettings.usePassphrase}
            deviceLabel={state.deviceLabel || 'KeepKey Recovery'}
            onComplete={handleRecoveryComplete}
            onError={handleRecoveryError}
            onBack={handleBack}
          />
        ) : (
          <div>Loading recovery settings...</div>
        );

      case 'recovery-complete':
        return (
          <RecoveryComplete
            deviceLabel={state.deviceLabel}
            onClose={onClose}
          />
        );

      case 'label':
        return (
          <DeviceLabel
            onComplete={handleLabelComplete}
            onSkip={handleLabelSkip}
            isLoading={state.isLoading}
          />
        );

      case 'pin':
        return (
          <DevicePin
            deviceId={deviceId}
            deviceLabel={state.deviceLabel}
            mode="create"
            onComplete={handlePinComplete}
            onBack={handleBack}
            isLoading={state.isLoading}
            error={state.error}
          />
        );

      case 'backup-display':
        console.debug('[WalletCreationWizard] Rendering BackupPhraseDisplay with deviceId:', deviceId);
        return (
          <BackupPhraseDisplay
            deviceId={deviceId}
            onComplete={handleBackupComplete}
            isLoading={state.isLoading}
            error={state.error}
          />
        );

      case 'complete':
        console.debug('[WalletCreationWizard] Rendering WalletCreationComplete with deviceId:', deviceId);
        return (
          <WalletCreationComplete
            deviceLabel={state.deviceLabel}
            onClose={onClose}
          />
        );

      default:
        return (
          <div>Unknown step: {state.step}</div>
        );
    }
  };

  return renderCurrentStep();
}

// Recovery Complete component
function RecoveryComplete({ 
  deviceLabel, 
  onClose 
}: { 
  deviceLabel: string; 
  onClose?: () => void; 
}) {
  return (
    <div style={{ 
      position: 'fixed', 
      top: 0, 
      left: 0, 
      right: 0, 
      bottom: 0, 
      backgroundColor: 'rgba(0, 0, 0, 0.8)', 
      display: 'flex', 
      alignItems: 'center', 
      justifyContent: 'center', 
      zIndex: 9999 
    }}>
      <div style={{ 
        backgroundColor: '#2D3748', 
        padding: '2rem', 
        borderRadius: '1rem', 
        textAlign: 'center',
        color: 'white'
      }}>
        <h2>🎉 Wallet Recovered Successfully!</h2>
        <p>Your KeepKey {deviceLabel && `"${deviceLabel}"`} has been restored with your recovery phrase.</p>
        <p>You can now securely access your cryptocurrency assets.</p>
        <button 
          onClick={onClose}
          style={{ 
            marginTop: '1rem', 
            padding: '0.5rem 1rem', 
            backgroundColor: '#38A169',
            color: 'white',
            border: 'none',
            borderRadius: '0.5rem',
            cursor: 'pointer'
          }}
        >
          Continue to Wallet
        </button>
      </div>
    </div>
  );
}

// Placeholder components for backup display and completion
// These will be implemented in the next phase

function BackupPhraseDisplay({ 
  deviceId, 
  onComplete, 
  isLoading, 
  error 
}: { 
  deviceId: string; 
  onComplete: () => void; 
  isLoading: boolean; 
  error: string | null; 
}) {
  return (
    <div style={{ 
      position: 'fixed', 
      top: 0, 
      left: 0, 
      right: 0, 
      bottom: 0, 
      backgroundColor: 'rgba(0, 0, 0, 0.8)', 
      display: 'flex', 
      alignItems: 'center', 
      justifyContent: 'center', 
      zIndex: 9999 
    }}>
      <div style={{ 
        backgroundColor: '#2D3748', 
        padding: '2rem', 
        borderRadius: '1rem', 
        textAlign: 'center',
        color: 'white'
      }}>
        <h2>Backup Your Recovery Phrase</h2>
        <p>Your recovery phrase is displayed on your KeepKey device.</p>
        <p>Write it down exactly as shown and store it safely.</p>
        {error && <p style={{ color: 'red' }}>{error}</p>}
        <button 
          onClick={onComplete}
          disabled={isLoading}
          style={{ 
            marginTop: '1rem', 
            padding: '0.5rem 1rem', 
            backgroundColor: '#38A169',
            color: 'white',
            border: 'none',
            borderRadius: '0.5rem',
            cursor: isLoading ? 'not-allowed' : 'pointer'
          }}
        >
          {isLoading ? 'Processing...' : 'I Have Written Down My Recovery Phrase'}
        </button>
      </div>
    </div>
  );
}

function WalletCreationComplete({ 
  deviceLabel, 
  onClose 
}: { 
  deviceLabel: string; 
  onClose?: () => void; 
}) {
  return (
    <div style={{ 
      position: 'fixed', 
      top: 0, 
      left: 0, 
      right: 0, 
      bottom: 0, 
      backgroundColor: 'rgba(0, 0, 0, 0.8)', 
      display: 'flex', 
      alignItems: 'center', 
      justifyContent: 'center', 
      zIndex: 9999 
    }}>
      <div style={{ 
        backgroundColor: '#2D3748', 
        padding: '2rem', 
        borderRadius: '1rem', 
        textAlign: 'center',
        color: 'white'
      }}>
        <h2>🎉 Wallet Created Successfully!</h2>
        <p>Your KeepKey {deviceLabel && `"${deviceLabel}"`} is now ready to use.</p>
        <p>You can now securely manage your cryptocurrency assets.</p>
        <button 
          onClick={onClose}
          style={{ 
            marginTop: '1rem', 
            padding: '0.5rem 1rem', 
            backgroundColor: '#38A169',
            color: 'white',
            border: 'none',
            borderRadius: '0.5rem',
            cursor: 'pointer'
          }}
        >
          Get Started
        </button>
      </div>
    </div>
  );
} 