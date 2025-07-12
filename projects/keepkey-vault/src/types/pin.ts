// TypeScript bindings for Rust PIN creation flow

export interface PinCreationSession {
  device_id: string;
  session_id: string;
  current_step: PinStep;
  is_active: boolean;
}

export enum PinStep {
  AwaitingFirst = 'AwaitingFirst',
  AwaitingSecond = 'AwaitingSecond', 
  Completed = 'Completed',
  Failed = 'Failed'
}

export interface PinMatrixResult {
  success: boolean;
  next_step?: string;
  session_id: string;
  error?: string;
}

// PIN matrix positions (1-9)
export type PinPosition = 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9;

// Standard PIN matrix layout
export const PIN_MATRIX_LAYOUT = [7, 8, 9, 4, 5, 6, 1, 2, 3] as const;

export interface PinMatrixState {
  positions: PinPosition[];
  maxLength: number;
  isScrambled: boolean;
} 