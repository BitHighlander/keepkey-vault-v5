import { invoke } from '@tauri-apps/api/core';
import axios from 'axios';

import { Asset, Portfolio } from '../types/wallet';
import { QueueStatus } from '../types/queue';

const TAG = " | API | ";
const PIONEER_BASE_URL = 'https://pioneers.dev';
const CACHE_TTL_MINUTES = 10;


// Pioneer API Types
export interface PioneerPortfolioRequest {
  caip: string;
  pubkey: string;
}

export interface PioneerPortfolioResponse {
  caip: string;
  pubkey: string;
  balance: string;
  valueUsd: string;
  priceUsd: string;
  symbol: string;
}

export interface PioneerFeeRateResponse {
  fastest: number;
  fast: number;
  average: number;
}

export interface PioneerAddressResponse {
  address: string;
  addressIndex: number;
}

export interface PioneerUtxoResponse {
  txid: string;
  vout: number;
  value: string;
  address: string;
  confirmations: number;
  path: string;
  scriptType: string;
  hex?: string;
  tx?: any;
}

export interface PioneerChangeAddressResponse {
  changeIndex: number;
  address: string;
}

// Database Cache Types
export interface BalanceCache {
  id: number;
  pubkey: string;
  caip: string;
  balance: string;
  balance_usd: string;
  price_usd: string;
  symbol: string | null;
  last_updated: number;
}

export class PioneerAPI {
  static async getPortfolio(requests: PioneerPortfolioRequest[]): Promise<PioneerPortfolioResponse[]> {
    try {
      console.log('🌐 Calling Pioneer portfolio API with', requests.length, 'xpubs');
      console.log('📤 Requests:', requests);
      
      const response = await axios.post(
        `${PIONEER_BASE_URL}/api/v1/portfolio`,
        requests,
        { 
          headers: { 
            'Content-Type': 'application/json',
            'accept': 'application/json' 
          },
          timeout: 30000
        }
      );

      console.log('✅ Pioneer portfolio API successful:', response.data.length, 'entries');
      console.log('📥 Raw response:', response.data);
      return response.data;
    } catch (error) {
      console.error('❌ Pioneer portfolio API failed:', error);
      throw error;
    }
  }

  static async getFeeRates(caip: string): Promise<PioneerFeeRateResponse> {
    try {
      console.log('💰 Getting fee rates for', caip);
      
      const encodedCaip = encodeURIComponent(caip);
      const response = await axios.get(
        `${PIONEER_BASE_URL}/api/v1/GetFeeRate/${encodedCaip}`,
        { 
          headers: { 'accept': 'application/json' },
          timeout: 10000
        }
      );

      console.log('✅ Fee rates retrieved:', response.data);
      return response.data;
    } catch (error) {
      console.error('❌ Fee rates API failed:', error);
      throw error;
    }
  }

  static async getReceiveAddress(coin: string, xpub: string): Promise<PioneerAddressResponse> {
    try {
      console.log('🏠 Getting receive address for', coin, 'xpub');
      
      // Use REAL Pioneer API endpoint from test-pioneer-live.js
      const response = await axios.get(
        `${PIONEER_BASE_URL}/api/v1/getNewAddress/BTC/${xpub}`,
        { 
          headers: { 'accept': 'application/json' },
          timeout: 10000
        }
      );

      console.log('✅ Receive address retrieved:', response.data);
      return response.data;
    } catch (error) {
      console.error('❌ Receive address API failed:', error);
      throw error;
    }
  }

  static async listUnspent(network: string, xpub: string): Promise<PioneerUtxoResponse[]> {
    try {
      console.log('🪙 Getting UTXOs for', network, 'xpub:', xpub.substring(0, 20) + '...');
      
      // Use REAL Pioneer API endpoint from test-pioneer-live.js
      const response = await axios.get(
        `${PIONEER_BASE_URL}/api/v1/listUnspent/BTC/${xpub}`,
        { 
          headers: { 'accept': 'application/json' },
          timeout: 30000
        }
      );

      console.log('✅ UTXOs retrieved:', response.data.length, 'utxos');
      return response.data;
    } catch (error) {
      console.error('❌ List unspent API failed:', error);
      throw error;
    }
  }

  static async getChangeAddress(network: string, xpub: string): Promise<PioneerChangeAddressResponse> {
    try {
      console.log('🔄 Getting change address for', network, 'xpub:', xpub.substring(0, 20) + '...');
      
      // Use REAL Pioneer API endpoint from test-pioneer-live.js
      const response = await axios.get(
        `${PIONEER_BASE_URL}/api/v1/getChangeAddress/BTC/${xpub}`,
        { 
          headers: { 'accept': 'application/json' },
          timeout: 10000
        }
      );

      console.log('✅ Change address retrieved:', response.data);
      return response.data;
    } catch (error) {
      console.error('❌ Get change address API failed:', error);
      throw error;
    }
  }

  static async broadcastTransaction(networkId: string, serializedTx: string): Promise<{ txid: string }> {
    try {
      console.log('📡 Broadcasting transaction to network:', networkId);
      console.log('📡 Transaction hex length:', serializedTx.length);
      console.log('📡 Transaction hex preview:', serializedTx.substring(0, 80) + '...');
      
      const response = await axios.post(
        `${PIONEER_BASE_URL}/api/v1/broadcast`,
        {
          networkId,
          serialized: serializedTx
        },
        { 
          headers: { 
            'accept': 'application/json',
            'Content-Type': 'application/json'
          },
          timeout: 30000
        }
      );

      console.log('✅ Transaction broadcast successful!');
      console.log('📡 Raw broadcast response:', response.data);
      
      if (response.data && response.data.txid) {
        console.log('🆔 Transaction ID:', response.data.txid);
        return { txid: response.data.txid };
      } else {
        throw new Error('Broadcast response missing txid: ' + JSON.stringify(response.data));
      }
    } catch (error) {
      console.error('❌ Transaction broadcast failed:', error);
      
      // Extract meaningful error message
      let errorMessage = 'Unknown broadcast error';
      if (axios.isAxiosError(error)) {
        errorMessage = error.response?.data?.message || error.response?.data?.error || error.message;
      } else if (error instanceof Error) {
        errorMessage = error.message;
      }
      
      throw new Error(`Transaction broadcast failed: ${errorMessage}`);
    }
  }

  static isCacheExpired(lastUpdated: number): boolean {
    const now = Math.floor(Date.now() / 1000);
    const cacheAge = now - lastUpdated;
    const maxAge = CACHE_TTL_MINUTES * 60;
    return cacheAge > maxAge;
  }

  // Create a client compatible with createUnsignedUxtoTx expectations
  static createClient() {
    return {
      async ListUnspent({ network, xpub }: { network: string; xpub: string }) {
        // Use real Pioneer API endpoints - network parameter is ignored since endpoint is hardcoded to BTC
        const data = await PioneerAPI.listUnspent('Bitcoin', xpub);
        return { data };
      },
      async GetChangeAddress({ network, xpub }: { network: string; xpub: string }) {
        // Use real Pioneer API endpoints - network parameter is ignored since endpoint is hardcoded to BTC
        const data = await PioneerAPI.getChangeAddress('Bitcoin', xpub);
        return { data };
      },
      async GetFeeRate({ networkId }: { networkId: string }) {
        // Convert networkId to caip format for our API
        const caip = `${networkId}/slip44:0`;
        const data = await PioneerAPI.getFeeRates(caip);
        return { data };
      }
    };
  }
}

// Global flag to enable/disable SQL caching (default: ON for xpub storage)

export class PortfolioAPI {
  static async getPortfolio(): Promise<Portfolio> {
    const tag = TAG + " | getPortfolio | ";
    try {
      console.log(tag, 'Getting portfolio from live Pioneer API data');
      // Only use Pioneer API (no DB or cache)
      // TODO: Replace with real xpubs from context if available
      const xpubs: any[] = [];
      const requests = xpubs.map(x => ({ caip: x.caip, pubkey: x.pubkey }));
      const portfolioData: any[] = await PioneerAPI.getPortfolio(requests);
      return this.transformToPortfolio(portfolioData);
    } catch (error) {
      console.error(tag, 'Portfolio API Error:', error);
      return this.getEmptyPortfolio();
    }
  }

  private static transformToPortfolio(data: PioneerPortfolioResponse[]): Portfolio {
    const tag = TAG + " | transformToPortfolio | ";
    
    let totalValueUsd = 0;
    const assets: Asset[] = [];
    const networks = [
      {
        id: 1,
        network_name: "Bitcoin",
        symbol: "BTC", 
        chain_id_caip2: "bip122:000000000019d6689c085ae165831e93",
        is_evm: false
      }
    ];

    // Group by symbol and sum balances
    const symbolGroups = new Map<string, {
      balance: number,
      valueUsd: number,
      priceUsd: number,
      caip: string
    }>();

    for (const item of data) {
      const balance = parseFloat(item.balance) || 0;
      const valueUsd = parseFloat(item.valueUsd) || 0;
      const priceUsd = parseFloat(item.priceUsd) || 0;
      
      if (symbolGroups.has(item.symbol)) {
        const existing = symbolGroups.get(item.symbol)!;
        existing.balance += balance;
        existing.valueUsd += valueUsd;
        // Keep the same price (should be same for same symbol)
      } else {
        symbolGroups.set(item.symbol, {
          balance,
          valueUsd,
          priceUsd,
          caip: item.caip
        });
      }
      
      totalValueUsd += valueUsd;
    }

    // Convert to assets
    for (const [symbol, group] of symbolGroups) {
      assets.push({
        symbol,
        name: symbol === 'BTC' ? 'Bitcoin' : symbol,
        balance: group.balance.toString(),
        value_usd: group.valueUsd,
        network_id: 'bitcoin',
        caip: group.caip,
        price_usd: group.priceUsd,
        change_24h: 0 // TODO: Add 24h change if Pioneer provides it
      });
    }

    console.log(tag, `Portfolio: ${assets.length} assets, total $${totalValueUsd.toFixed(2)}`);

    return {
      total_value_usd: totalValueUsd.toFixed(2),
      assets,
      networks
    };
  }

  private static getEmptyPortfolio(): Portfolio {
    return {
      total_value_usd: '0.00',
      assets: [],
      networks: [{
        id: 1,
        network_name: "Bitcoin",
        symbol: "BTC",
        chain_id_caip2: "bip122:000000000019d6689c085ae165831e93",
        is_evm: false
      }]
    };
  }

  static async sendAsset(asset: Asset, toAddress: string, amount: string): Promise<boolean> {
    const tag = TAG + " | sendAsset | ";
    
    try {
      console.log(tag, 'Send transaction request:', { 
        asset: asset.symbol, 
        caip: asset.caip,
        toAddress, 
        amount,
        availableBalance: asset.balance
      });

      // Validate inputs
      if (!toAddress || !amount) {
        throw new Error('Missing required parameters: address and amount');
      }

      const sendAmount = parseFloat(amount);
      const availBalance = parseFloat(asset.balance);

      if (sendAmount <= 0) {
        throw new Error('Amount must be greater than 0');
      }

      if (sendAmount > availBalance) {
        throw new Error(`Insufficient balance. Available: ${asset.balance} ${asset.symbol}, Requested: ${amount} ${asset.symbol}`);
      }

      // TODO: Implement actual transaction building and signing
      console.warn(tag, '⚠️ TRANSACTION NOT IMPLEMENTED - This would:');
      console.warn(tag, '  1. Call Pioneer API to build transaction');
      console.warn(tag, '  2. Get UTXOs for the xpub');
      console.warn(tag, '  3. Build PSBT with proper fees');
      console.warn(tag, '  4. Send to device queue for signing');
      console.warn(tag, '  5. Broadcast signed transaction');
      console.warn(tag, '  6. Return transaction ID');

      // For now, return false to indicate the feature is not implemented
      // This prevents the UI from showing "success" when nothing actually happened
      throw new Error('❌ Transaction sending not yet implemented. This is a placeholder.');

    } catch (error) {
      console.error(tag, 'Send transaction error:', error);
      throw error;
    }
  }

  static async getReceiveAddress(asset: Asset): Promise<string> {
    const tag = TAG + " | getReceiveAddress | ";
    try {
      console.log(tag, 'Getting receive address for asset:', asset.symbol);
      // DB is removed, so always throw for now
      throw new Error('getReceiveAddress: DB code removed. Cannot fetch xpub for asset.');
    } catch (error) {
      console.error(tag, 'Failed to get receive address:', error);
      throw error;
    }
  }
}

// Utility to extract canonical device ID (hardware unique_id)
function getCanonicalDeviceId(device: any): string {
  if (device && typeof device === 'object') {
    if (device.unique_id) return device.unique_id;
    if (device.device && device.device.unique_id) return device.device.unique_id;
  }
  throw new Error('Invalid device object: cannot extract unique_id');
}

/**
 * DeviceQueueAPI expects all device IDs to be the canonical unique_id (hardware ID).
 * Do NOT use friendly names or composite keys for device queue operations.
 */
export class DeviceQueueAPI {

  static async getConnectedDevices(): Promise<any[]> {
    let tag = TAG + " | getConnectedDevices | ";
    try {
      const devices = await invoke('get_connected_devices');
      console.log(tag,'devices: ',devices);
      return devices as any[];
    } catch (error) {
      console.error('Failed to get connected devices:', error);
      throw error;
    }
  }

  static async requestXpubFromDevice(deviceId: string, path: string): Promise<string> {
    // Validation: deviceId must be present and valid
    if (!deviceId || typeof deviceId !== 'string' || deviceId.trim() === '') {
      throw new Error('DeviceQueueAPI: Cannot queue xpub request: deviceId is missing or invalid.');
    }
    // Validation: device must be connected
    const connected = await DeviceQueueAPI.getConnectedDevices();
    if (!connected.some(d => getCanonicalDeviceId(d) === deviceId)) {
      throw new Error(`DeviceQueueAPI: Cannot queue xpub request: device ${deviceId} is not connected.`);
    }
    try {
      const requestId = `xpub_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
      const request = {
        device_id: deviceId,
        request_id: requestId,
        request: {
          GetXpub: { path }
        }
      };
      
      console.log('🔄 Requesting xpub from device:', { deviceId, path, requestId });
      await invoke('add_to_device_queue', { request });
      return requestId;
    } catch (error) {
      console.error('Failed to add xpub request to device queue:', error);
      throw error;
    }
  }

  static async requestReceiveAddressFromDevice(
    deviceId: string, 
    path: string, 
    coinName: string, 
    scriptType?: string,
    showDisplay?: boolean
  ): Promise<string> {
    // Validation: deviceId must be present and valid
    if (!deviceId || typeof deviceId !== 'string' || deviceId.trim() === '') {
      throw new Error('DeviceQueueAPI: Cannot queue address request: deviceId is missing or invalid.');
    }
    // Validation: device must be connected
    const connected = await DeviceQueueAPI.getConnectedDevices();
    if (!connected.some(d => getCanonicalDeviceId(d) === deviceId)) {
      throw new Error(`DeviceQueueAPI: Cannot queue address request: device ${deviceId} is not connected.`);
    }
    try {
      const requestId = `addr_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
      return await this.requestReceiveAddressFromDeviceWithId(
        deviceId, path, coinName, scriptType, showDisplay, requestId
      );
    } catch (error) {
      console.error('Failed to add address request to device queue:', error);
      throw error;
    }
  }

  static async requestReceiveAddressFromDeviceWithId(
    deviceId: string, 
    path: string, 
    coinName: string, 
    scriptType: string | undefined,
    showDisplay: boolean | undefined,
    requestId: string
  ): Promise<string> {
    // Validation: deviceId must be present and valid
    if (!deviceId || typeof deviceId !== 'string' || deviceId.trim() === '') {
      throw new Error('DeviceQueueAPI: Cannot queue address request (withId): deviceId is missing or invalid.');
    }
    // Validation: device must be connected
    const connected = await DeviceQueueAPI.getConnectedDevices();
    if (!connected.some(d => getCanonicalDeviceId(d) === deviceId)) {
      throw new Error(`DeviceQueueAPI: Cannot queue address request (withId): device ${deviceId} is not connected.`);
    }
    try {
      const request = {
        device_id: deviceId,
        request_id: requestId,
        request: {
          GetAddress: { 
            path, 
            coin_name: coinName, 
            script_type: scriptType, 
            show_display: showDisplay // Will be undefined if not provided, which becomes None in Rust
          }
        }
      };
      
      console.log('🔄 Requesting address from device:', { deviceId, path, requestId });
      await invoke('add_to_device_queue', { request });
      return requestId;
    } catch (error) {
      console.error('Failed to add address request to device queue:', error);
      throw error;
    }
  }

  static async resetDeviceQueue(deviceId: string): Promise<void> {
    await invoke('reset_device_queue', { deviceId });
  }

  static async getQueueStatus(deviceId: string): Promise<QueueStatus> {
    try {
      if(!deviceId) throw Error('DeviceQueueAPI: Cannot get queue status without a device id');
      const payload: { deviceId?: string } = {};
      if (deviceId !== undefined) {
        payload.deviceId = deviceId;
      }
      console.trace('🛠 getQueueStatus call stack for deviceId:', deviceId);
      console.log('🛠 getQueueStatus payload', payload);
      const status = await invoke('get_queue_status', payload);
      return status as QueueStatus;
    } catch (error) {
      console.error('Failed to get queue status:', error);
      throw error;
    }
  }

  static async signTransaction(
    deviceId: string,
    coin: string,
    inputs: any[],
    outputs: any[],
    version: number = 1,
    lockTime: number = 0
  ): Promise<string> {
    // Validation: deviceId must be present and valid
    if (!deviceId || typeof deviceId !== 'string' || deviceId.trim() === '') {
      throw new Error('DeviceQueueAPI: Cannot queue signTransaction: deviceId is missing or invalid.');
    }
    // Validation: device must be connected
    const connected = await DeviceQueueAPI.getConnectedDevices();
    if (!connected.some(d => getCanonicalDeviceId(d) === deviceId)) {
      throw new Error(`DeviceQueueAPI: Cannot queue signTransaction: device ${deviceId} is not connected.`);
    }
    try {
      const requestId = `sign_tx_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
      return await this.signTransactionWithId(deviceId, coin, inputs, outputs, version, lockTime, requestId);
    } catch (error) {
      console.error('Failed to add transaction signing request to device queue:', error);
      throw error;
    }
  }

  static async signTransactionWithId(
    deviceId: string,
    coin: string,
    inputs: any[],
    outputs: any[],
    version: number = 1,
    lockTime: number = 0,
    requestId: string
  ): Promise<string> {
    // Validation: deviceId must be present and valid
    if (!deviceId || typeof deviceId !== 'string' || deviceId.trim() === '') {
      throw new Error('DeviceQueueAPI: Cannot queue signTransaction: deviceId is missing or invalid.');
    }
    // Validation: device must be connected
    const connected = await DeviceQueueAPI.getConnectedDevices();
    if (!connected.some(d => getCanonicalDeviceId(d) === deviceId)) {
      throw new Error(`DeviceQueueAPI: Cannot queue signTransaction: device ${deviceId} is not connected.`);
    }
    try {
      const request = {
        device_id: deviceId,
        request_id: requestId,
        request: {
          SignTransaction: {
            coin,
            inputs,
            outputs,
            version,
            lock_time: lockTime
          }
        }
      };
      
      console.log('🔐 Requesting transaction signing from device:', { deviceId, coin, inputCount: inputs.length, outputCount: outputs.length, requestId });
      await invoke('add_to_device_queue', { request });
      return requestId;
    } catch (error) {
      console.error('Failed to add transaction signing request to device queue:', error);
      throw error;
    }
  }
} 