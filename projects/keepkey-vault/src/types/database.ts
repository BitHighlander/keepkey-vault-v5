export interface XpubInfo {
  id: number;
  device_id: string;
  path: string;
  label: string;
  caip: string;
  pubkey: string;
  created_at: number;
}

export interface DeviceInfo {
  id: number;
  device_id: string;
  label?: string;
  features_json: string;
  last_seen: number;
  created_at: number;
}

export interface RequiredPath {
  path: string;
  label: string;
  caip: string;
} 