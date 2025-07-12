export type DeviceResponse = 
  | {
      Xpub: {
        request_id: string;
        device_id: string;
        path: string;
        xpub: string;
        success: boolean;
        error?: string;
      };
    }
  | {
      Address: {
        request_id: string;
        device_id: string;
        path: string;
        address: string;
        success: boolean;
        error?: string;
      };
    }
  | {
      SignedTransaction: {
        request_id: string;
        device_id: string;
        signed_tx: string;
        txid?: string;
        success: boolean;
        error?: string;
      };
    };

export interface QueueStatus {
  device_id: string | null;
  total_queued: number;
  active_operations: number;
  status: string;
  last_response?: DeviceResponse;
} 