// Send Page Types
export interface UTXO {
  txid: string;
  vout: number;
  address: string;
  amount_sat: number;
  confirmations: number;
  is_locked: boolean;
  label?: string;
}

export interface SendWalletState {
  selectedDeviceId: string;
  selectedAccount: number;           // BIP44 account
  scriptType: 'p2pkh' | 'p2sh-p2wpkh' | 'p2wpkh';
  feeRatePreset: 'slow' | 'medium' | 'fast' | 'custom';
  customFeeRate?: number;            // sat/vB
  isEasyMode: boolean;               // true = slider, false = coin control
  sliderPercent: number;             // 0–100 when easy mode
  selectedInputs: Set<string>;       // txid:vout identifiers
  lockedInputs: Set<string>;         // persisted between sessions
  outputs: Array<{ address: string; amount: string }>;  // BTC in decimals
  changeAddress?: string;            // chosen by backend
  draftTx?: UnsignedTx;              // hex or PSBT
  signProgress?: {
    step: 'waiting' | 'pin' | 'passphrase' | 'signing' | 'done';
    message?: string;
  };
  broadcastStatus?: 'idle' | 'broadcasting' | 'success' | 'error';
  broadcastTxId?: string;
  error?: string;
}

export interface UnsignedTx {
  psbt: string;
  fee: number;
  size: number;
  inputs: Array<{
    txid: string;
    vout: number;
    amount: number;
    address: string;
  }>;
  outputs: Array<{
    address: string;
    amount: number;
    isChange: boolean;
  }>;
}

export interface FeeEstimate {
  slow: number;    // sat/vB
  medium: number;  // sat/vB
  fast: number;    // sat/vB
}

export interface BuildTxRequest {
  recipients: Array<{ address: string; amount: string }>;
  feeRate: number;
  selectedInputs?: string[];  // txid:vout format
  sliderPercent?: number;
}

export interface BuildTxResponse {
  success: boolean;
  error?: string;
  tx?: UnsignedTx;
}

export interface BroadcastResponse {
  success: boolean;
  error?: string;
  txid?: string;
}

export type SendStep = 'compose' | 'review' | 'sign' | 'broadcast';

export interface SendPageProps {
  onBack?: () => void;
} 