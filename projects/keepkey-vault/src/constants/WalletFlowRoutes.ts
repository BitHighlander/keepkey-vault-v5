export enum WalletFlowRoutes {
  // Factory state - initial choice
  FACTORY_STATE = '/wallet/factory-state',
  
  // Shared components
  LABEL_ENTRY = '/wallet/label',
  PIN_ENTRY = '/wallet/pin',
  PIN_CONFIRM = '/wallet/pin-confirm',
  
  // Create wallet specific
  CREATE_DEVICE_INIT = '/wallet/create/device-init',
  CREATE_BACKUP_DISPLAY = '/wallet/create/backup-display',
  CREATE_COMPLETE = '/wallet/create/complete',
  
  // Recovery wallet specific (for future implementation)
  RECOVERY_SETTINGS = '/wallet/recovery/settings',
  RECOVERY_PHRASE_ENTRY = '/wallet/recovery/phrase-entry',
  RECOVERY_PROCESSING = '/wallet/recovery/processing',
  RECOVERY_INVALID = '/wallet/recovery/invalid',
  RECOVERY_COMPLETE = '/wallet/recovery/complete',
}

export type WalletFlowStep = 
  | 'factory-state'
  | 'label'
  | 'pin'
  | 'pin-confirm' 
  | 'device-init'
  | 'backup-display'
  | 'complete'; 