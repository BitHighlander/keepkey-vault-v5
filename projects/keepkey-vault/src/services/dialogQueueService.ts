import { invoke } from '@tauri-apps/api/core';
import { DialogRequest, DialogQueueStatus, createDialogRequest, DialogType, DialogPriority } from '../types/dialog';

/**
 * Service for interacting with the backend dialog queue system
 */
export class DialogQueueService {
  /**
   * Queue a dialog request with the backend
   */
  static async queueDialog(request: Omit<DialogRequest, 'id'>): Promise<DialogRequest> {
    try {
      const result = await invoke<DialogRequest>('queue_dialog', {
        dialogRequest: request
      });
      return result;
    } catch (error) {
      console.error('Failed to queue dialog:', error);
      throw error;
    }
  }

  /**
   * Get the next dialog from the queue (for displaying)
   */
  static async getNextDialog(): Promise<DialogRequest | null> {
    try {
      const result = await invoke<DialogRequest | null>('get_next_dialog');
      return result;
    } catch (error) {
      console.error('Failed to get next dialog:', error);
      throw error;
    }
  }

  /**
   * Mark a dialog as completed
   */
  static async completeDialog(dialogId: string): Promise<DialogRequest | null> {
    try {
      const result = await invoke<DialogRequest | null>('complete_dialog', {
        dialogId
      });
      return result;
    } catch (error) {
      console.error('Failed to complete dialog:', error);
      throw error;
    }
  }

  /**
   * Get the current queue status
   */
  static async getQueueStatus(): Promise<DialogQueueStatus> {
    try {
      const [active, queue] = await invoke<[DialogRequest | null, DialogRequest[]]>('get_dialog_queue_status');
      return { active, queue };
    } catch (error) {
      console.error('Failed to get queue status:', error);
      throw error;
    }
  }

  /**
   * Helper: Queue a wallet creation dialog
   */
  static async queueWalletCreation(deviceId: string, metadata: Record<string, any> = {}): Promise<DialogRequest> {
    const request = createDialogRequest(DialogType.WALLET_CREATION, {
      deviceId,
      priority: DialogPriority.HIGH,
      persistent: true,
      metadata
    });
    return this.queueDialog(request);
  }

  /**
   * Helper: Queue a bootloader update dialog (critical priority)
   */
  static async queueBootloaderUpdate(deviceId: string, metadata: Record<string, any> = {}): Promise<DialogRequest> {
    const request = createDialogRequest(DialogType.BOOTLOADER_UPDATE, {
      deviceId,
      priority: DialogPriority.CRITICAL,
      persistent: true,
      metadata
    });
    return this.queueDialog(request);
  }

  /**
   * Helper: Queue a firmware update dialog (high priority)
   */
  static async queueFirmwareUpdate(deviceId: string, metadata: Record<string, any> = {}): Promise<DialogRequest> {
    const request = createDialogRequest(DialogType.FIRMWARE_UPDATE, {
      deviceId,
      priority: DialogPriority.HIGH,
      persistent: false,
      metadata
    });
    return this.queueDialog(request);
  }

  /**
   * Helper: Queue an onboarding dialog (normal priority)
   */
  static async queueOnboarding(metadata: Record<string, any> = {}): Promise<DialogRequest> {
    const request = createDialogRequest(DialogType.ONBOARDING, {
      priority: DialogPriority.NORMAL,
      persistent: true,
      metadata
    });
    return this.queueDialog(request);
  }

  /**
   * Helper: Queue settings dialog (low priority)
   */
  static async queueSettings(metadata: Record<string, any> = {}): Promise<DialogRequest> {
    const request = createDialogRequest(DialogType.SETTINGS, {
      priority: DialogPriority.LOW,
      persistent: false,
      metadata
    });
    return this.queueDialog(request);
  }
}

export default DialogQueueService; 