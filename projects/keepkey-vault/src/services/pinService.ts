import { invoke } from '@tauri-apps/api/core';
import { PinCreationSession, PinMatrixResult, PinPosition } from '../types/pin';

/**
 * Service for managing KeepKey PIN creation flow
 */
export class PinService {
  /**
   * Start PIN creation process for a device
   */
  static async startPinCreation(deviceId: string, label?: string): Promise<PinCreationSession> {
    try {
      const session = await invoke<PinCreationSession>('initialize_device_pin', {
        deviceId,
        label: label || null
      });
      console.log('PIN creation session started:', session);
      return session;
    } catch (error) {
      console.error('Failed to start PIN creation:', error);
      throw error;
    }
  }

  /**
   * Send PIN matrix response (positions clicked by user)
   */
  static async sendPinResponse(
    sessionId: string, 
    positions: PinPosition[]
  ): Promise<PinMatrixResult> {
    try {
      const result = await invoke<PinMatrixResult>('send_pin_matrix_response', {
        sessionId,
        positions
      });
      console.log('PIN matrix response sent:', result);
      return result;
    } catch (error) {
      console.error('Failed to send PIN response:', error);
      throw error;
    }
  }

  /**
   * Get current PIN session status
   */
  static async getSessionStatus(sessionId: string): Promise<PinCreationSession | null> {
    try {
      const session = await invoke<PinCreationSession | null>('get_pin_session_status', {
        sessionId
      });
      return session;
    } catch (error) {
      console.error('Failed to get session status:', error);
      throw error;
    }
  }

  /**
   * Cancel PIN creation session
   */
  static async cancelPinCreation(sessionId: string): Promise<boolean> {
    try {
      const cancelled = await invoke<boolean>('cancel_pin_creation', {
        sessionId
      });
      console.log('PIN creation cancelled:', cancelled);
      return cancelled;
    } catch (error) {
      console.error('Failed to cancel PIN creation:', error);
      throw error;
    }
  }

  /**
   * Convert matrix positions to PIN string for display (debugging only)
   */
  static positionsToString(positions: PinPosition[]): string {
    return positions.join('');
  }

  /**
   * Validate PIN positions
   */
  static validatePositions(positions: PinPosition[]): { valid: boolean; error?: string } {
    if (positions.length === 0) {
      return { valid: false, error: 'PIN cannot be empty' };
    }
    
    if (positions.length > 9) {
      return { valid: false, error: 'PIN cannot be longer than 9 digits' };
    }
    
    for (const pos of positions) {
      if (pos < 1 || pos > 9) {
        return { valid: false, error: 'Invalid PIN position: must be 1-9' };
      }
    }
    
    return { valid: true };
  }
} 