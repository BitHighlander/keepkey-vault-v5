export interface Asset {
  symbol: string;
  name: string;
  balance: string;
  value_usd: number;
  network_id: string;
  caip: string;
  price_usd?: number;
  change_24h?: number;
}

export interface Network {
  id: number;
  network_name: string;
  symbol: string;
  chain_id_caip2: string;
  is_evm: boolean;
}

export interface Portfolio {
  total_value_usd: string;
  assets: Asset[];
  networks: Network[];
} 