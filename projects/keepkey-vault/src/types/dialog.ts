// TypeScript bindings for Rust dialog queue system

/**
 * Dialog request structure that matches the Rust DialogRequest struct
 * Used for backend-controlled dialog priority queue
 */
export interface DialogRequest {
  id: string;
  dialog_type: string;
  device_id?: string;
  priority_points: number;
  persistent: boolean;
  metadata: Record<string, any>;
}

/**
 * Dialog queue status response
 */
export interface DialogQueueStatus {
  active: DialogRequest | null;
  queue: DialogRequest[];
}

/**
 * Standard dialog types for the system
 */
export enum DialogType {
  WALLET_CREATION = 'wallet_creation',
  RECOVERY_WIZARD = 'recovery_wizard',
  BOOTLOADER_UPDATE = 'bootloader_update',
  FIRMWARE_UPDATE = 'firmware_update',
  SETTINGS = 'settings',
  ONBOARDING = 'onboarding',
  TROUBLESHOOTING = 'troubleshooting',
}

/**
 * Priority levels with corresponding point values
 */
export enum DialogPriority {
  LOW = 10,
  NORMAL = 50,
  HIGH = 100,
  CRITICAL = 200,
}

/**
 * Helper function to create a dialog request
 */
export function createDialogRequest(
  dialogType: DialogType | string,
  options: {
    deviceId?: string;
    priority?: DialogPriority;
    persistent?: boolean;
    metadata?: Record<string, any>;
  } = {}
): Omit<DialogRequest, 'id'> {
  return {
    dialog_type: dialogType,
    device_id: options.deviceId,
    priority_points: options.priority ?? DialogPriority.NORMAL,
    persistent: options.persistent ?? false,
    metadata: options.metadata ?? {},
  };
} 