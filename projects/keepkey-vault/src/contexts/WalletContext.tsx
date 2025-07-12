/*
    Primary Wallet Context

    Manages wallet state, device sync, and portfolio data
    All device calls go through backend queue - NO direct device communication

*/

import React, { createContext, useContext, useState, useEffect, useRef, ReactNode, useCallback } from 'react';
import { listen } from '@tauri-apps/api/event';

// Import organized types and services
import { Asset, Portfolio, QueueStatus } from '../types';
import { PortfolioAPI, DeviceQueueAPI, PioneerAPI } from '../lib';

const TAG = " | WalletContext | ";

// ---- Global Event Bridge for Receive Address Requests ---------------------------------------------------------
type AddrResolver = { resolve: (addr: string) => void; reject: (e: any) => void };
const pendingReceiveRequests: Map<string, AddrResolver> = new Map();
const pendingSigningRequests: Map<string, { resolve: (signedTx: string) => void; reject: (error: any) => void }> = new Map();
// ----------------------------------------------------------------------------------------------------------------

// Context Type
interface WalletContextType {
  portfolio: Portfolio | null;
  selectedAsset: Asset | null;
  loading: boolean;
  error: string | null;
  isSync: boolean;
  lastReceiveAddress: string | null;
  fetchedXpubs: Array<{path: string, xpub: string, caip: string}>;
  refreshPortfolio: () => Promise<void>;
  selectAsset: (asset: Asset | null) => void;
  sendAsset: (toAddress: string, amount: string) => Promise<boolean>;
  getReceiveAddress: () => Promise<string | null>;
  requestXpubFromDevice: (deviceId: string, path: string) => Promise<string>;
  getQueueStatus: (deviceId: string) => Promise<QueueStatus>;
  signTransaction: (
    deviceId: string,
    coin: string,
    inputs: any[],
    outputs: any[],
    version?: number,
    lockTime?: number
  ) => Promise<string>;
  /**
   * Explicitly rerun the wallet initialization logic. Useful when the backend
   * restarts and the app needs to resync device/portfolio state without a full refresh.
   */
  reinitialize: () => void;
}

// Create Context
const WalletContext = createContext<WalletContextType | undefined>(undefined);

// Provider Props
interface WalletProviderProps {
  children: ReactNode;
}

// Utility to extract canonical device ID (hardware unique_id)
function getCanonicalDeviceId(device: any): string {
  if (device && typeof device === 'object') {
    if (device.unique_id) return device.unique_id;
    if (device.device && device.device.unique_id) return device.device.unique_id;
  }
  throw new Error('Invalid device object: cannot extract unique_id');
}

// Provider Component
export const WalletProvider: React.FC<WalletProviderProps> = ({ children }) => {
  const [portfolio, setPortfolio] = useState<Portfolio | null>(null);
  const [selectedAsset, setSelectedAsset] = useState<Asset | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [isSync, setIsSync] = useState(false);
  const [isInitializing, setIsInitializing] = useState(false);
  const [lastReceiveAddress, setLastReceiveAddress] = useState<string | null>(null);
  const initializingRef = useRef(false);

  // Track pending xpub requests to prevent duplicates
  const pendingXpubRequests = new Set<string>();
  
  // Store fetched xpubs in memory (not database for v2)
  const [fetchedXpubs, setFetchedXpubs] = useState<Array<{path: string, xpub: string, caip: string}>>([]);

  const refreshPortfolio = useCallback(async () => {
    const tag = TAG + " | refreshPortfolio | ";
    setLoading(true);
    
    try {
      console.log(tag, `Refreshing portfolio with ${fetchedXpubs.length} xpubs in memory`);
      
      if (fetchedXpubs.length === 0) {
        console.log(tag, 'No xpubs available yet, automatically fetching from device...');
        setPortfolio(null);
        
        // Automatically fetch xpubs instead of just showing empty portfolio
        try {
          await getXpubsFromDeviceQueue();
          console.log(tag, '✅ Initiated xpub fetching, portfolio will update when xpubs arrive via events');
        } catch (error) {
          console.error(tag, '❌ Failed to fetch xpubs automatically:', error);
          setError('Failed to fetch device information: ' + (error instanceof Error ? error.message : 'Unknown error'));
        }
        return;
      }
      
      // Convert in-memory xpubs to Pioneer API format and fetch portfolio
      const requests = fetchedXpubs.map(x => ({
        caip: x.caip,
        pubkey: x.xpub
      }));
      
      console.log(tag, 'Calling Pioneer API with in-memory xpubs:', requests);
      
      // Call Pioneer API directly with in-memory xpubs
      const portfolioData = await PioneerAPI.getPortfolio(requests);
      
      // Transform to portfolio format (simplified version of PortfolioAPI.transformToPortfolio)
      let totalValueUsd = 0;
      const assets: Asset[] = [];
      const networks = [
        {
          id: 1,
          network_name: "Bitcoin",
          symbol: "BTC", 
          chain_id_caip2: "bip122:000000000019d6689c085ae165831e93/slip44:0",
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

      for (const item of portfolioData) {
        const balance = parseFloat(item.balance) || 0;
        const valueUsd = parseFloat(item.valueUsd) || 0;
        const priceUsd = parseFloat(item.priceUsd) || 0;
        
        if (symbolGroups.has(item.symbol)) {
          const existing = symbolGroups.get(item.symbol)!;
          existing.balance += balance;
          existing.valueUsd += valueUsd;
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
          change_24h: 0
        });
      }

      const portfolio: Portfolio = {
        total_value_usd: totalValueUsd.toFixed(2),
        assets,
        networks
      };
      
      setPortfolio(portfolio);
      setError(null);
      console.log(tag, 'Portfolio refreshed successfully:', portfolio);
      
    } catch (error) {
      console.error(tag, 'Failed to refresh portfolio:', error);
      setError('Failed to refresh portfolio: ' + (error instanceof Error ? error.message : 'Unknown error'));
    } finally {
      setLoading(false);
    }
  }, [fetchedXpubs]);

  // Requests all required xpubs from device queue (event-driven, no polling!)
  async function getXpubsFromDeviceQueue() {
    const tag = TAG + " | getXpubsFromDeviceQueue | ";
    
    try {
      // Get fresh list of connected devices to ensure we have current device IDs
      const connectedDevicesResponse = await DeviceQueueAPI.getConnectedDevices();
      console.log(tag, 'Fresh connected devices:', connectedDevicesResponse);
      
      if (!connectedDevicesResponse || connectedDevicesResponse.length === 0) {
        console.log(tag, 'No devices connected for xpub fetching');
        return;
      }

      // Use the first connected device (in a multi-device setup, we'd handle all)
      const deviceData = connectedDevicesResponse[0];
      const device = deviceData.device || deviceData;
      const deviceId = getCanonicalDeviceId(device);
      console.debug('[WalletContext] deviceId from getCanonicalDeviceId:', deviceId);
      
      console.log(tag, `Using device: ${deviceId}`);
      
      const requiredPaths = [
        { path: "m/44'/0'/0'", caip: "bip122:000000000019d6689c085ae165831e93/slip44:0" },  // Legacy P2PKH (Account level)
        { path: "m/49'/0'/0'", caip: "bip122:000000000019d6689c085ae165831e93/slip44:0" },  // SegWit P2SH (Account level)
        { path: "m/84'/0'/0'", caip: "bip122:000000000019d6689c085ae165831e93/slip44:0" }   // Native SegWit P2WPKH (Account level)
      ];

      // Send all xpub requests at once (no polling needed - events will handle responses)
      for (const pathInfo of requiredPaths) {
        // Always use canonical deviceId
        const requestKey = `${deviceId}:${pathInfo.path}`;
        
        // Skip if request is already pending
        if (pendingXpubRequests.has(requestKey)) {
          console.log(tag, `Skipping duplicate request for ${requestKey}`);
          continue;
        }

        // Skip if we already have this xpub in memory
        const alreadyHas = fetchedXpubs.some(x => x.path === pathInfo.path && x.caip === pathInfo.caip);
        if (alreadyHas) {
          console.log(tag, `Xpub already fetched for ${deviceId} ${pathInfo.path}`);
          continue;
        }

        // Mark request as pending
        pendingXpubRequests.add(requestKey);

        try {
          console.log(tag, `🔄 Requesting xpub for ${deviceId} ${pathInfo.path}`);
          
          // Send xpub request (response will come via event)
          // Always use canonical deviceId for requests
          const requestId = await DeviceQueueAPI.requestXpubFromDevice(deviceId, pathInfo.path);
          console.log(tag, `✅ Queued xpub request (requestId: ${requestId}) - waiting for event...`);
          
        } catch (err) {
          console.error(tag, `❌ Failed to queue xpub request for ${deviceId} ${pathInfo.path}:`, err);
          pendingXpubRequests.delete(requestKey);
        }
      }
      
    } catch (error) {
      console.error(tag, 'Error in getXpubsFromDeviceQueue:', error);
    }
  }

  async function onStart() {
    const tag = TAG + " | onStart | ";
    
    // Prevent multiple simultaneous initializations
    if (isInitializing || initializingRef.current) {
      console.log(tag, 'Already initializing, skipping duplicate onStart call');
      return;
    }
    
    initializingRef.current = true;
    setIsInitializing(true);
    setLoading(true);
    
    try {
      // Check if onboarded
      const isOnboarded = true;
      console.log(tag, 'isOnboarded:', isOnboarded);

      // Get connected devices from backend (live detection)
      console.log(tag, '🔍 Detecting connected devices...');
      const connectedDevicesResponse = await DeviceQueueAPI.getConnectedDevices();
      console.log(tag, 'Connected devices:', connectedDevicesResponse);

      if (!connectedDevicesResponse || connectedDevicesResponse.length === 0) {
        console.log(tag, 'No devices connected');
        setIsSync(false);
        setError('No KeepKey device connected. Please connect your device and try again.');
        return;
      }

      // Store detected devices in database for future reference
      

      // We have connected devices from the API call above
      // Skip the database check since we confirmed devices are connected
      console.log(tag, 'Connected devices confirmed, proceeding with initialization');

      // Don't fetch xpubs immediately - wait for device:ready event instead
      // This prevents trying to fetch xpubs from PIN-locked devices
      console.log(tag, '🔄 Skipping immediate xpub fetch - waiting for device:ready event...');

      // Set sync status based on whether we have in-memory xpubs
      // Since we removed the database, we'll use in-memory xpubs to determine sync status
      const requiredPaths = [
        { path: "m/44'/0'/0'", caip: "bip122:000000000019d6689c085ae165831e93/slip44:0" },  // Legacy P2PKH (Account level)
        { path: "m/49'/0'/0'", caip: "bip122:000000000019d6689c085ae165831e93/slip44:0" },  // SegWit P2SH (Account level)
        { path: "m/84'/0'/0'", caip: "bip122:000000000019d6689c085ae165831e93/slip44:0" }   // Native SegWit P2WPKH (Account level)
      ];
      
      // Check if we have all required xpubs in memory
      const allXpubsPresent = requiredPaths.every(requiredPath => 
        fetchedXpubs.some(x => x.path === requiredPath.path && x.caip === requiredPath.caip)
      );

      setIsSync(allXpubsPresent);
      console.log(tag, 'Final sync status based on in-memory xpubs:', allXpubsPresent);
      // Skip the old portfolio API call - we'll use in-memory xpubs via refreshPortfolio
      console.log(tag, 'Skipping database portfolio call - using in-memory xpubs instead');
      
      // If we have xpubs in memory, refresh portfolio immediately
      if (fetchedXpubs.length > 0) {
        console.log(tag, `We have ${fetchedXpubs.length} xpubs in memory, refreshing portfolio`);
        await refreshPortfolio();
      } else {
        console.log(tag, 'No xpubs in memory yet, portfolio will refresh when xpubs arrive via events');
        setPortfolio(null);
      }
      
      setError(null);

    } catch (error) {
      console.error(tag, '❌ Failed to initialize:', error);
      setError('❌ Failed to initialize: ' + (error instanceof Error ? error.message : 'Unknown error'));
    } finally {
      setLoading(false);
      setIsInitializing(false);
      initializingRef.current = false;
    }
  }

  const selectAsset = (asset: Asset | null) => {
    setSelectedAsset(asset);
    console.log('🎯 Asset selected:', asset?.symbol);
  };

  const sendAsset = async (toAddress: string, amount: string): Promise<boolean> => {
    if (!selectedAsset) {
      console.error('❌ No asset selected for sending');
      return false;
    }

    try {
      console.log(`📤 Sending ${amount} ${selectedAsset.symbol} to ${toAddress}`);
      await PortfolioAPI.sendAsset(selectedAsset, toAddress, amount);
      
      console.log('✅ Send transaction successful');
      await refreshPortfolio();
      return true;
      
    } catch (error) {
      console.error('❌ Send transaction failed:', error);
      // Re-throw the error so the UI can display the specific error message
      throw error;
    }
  };

  const getReceiveAddress = async (): Promise<string | null> => {
    const tag = TAG + " | getReceiveAddress | ";
    
    // Ensure BTC asset is selected
    const btcCaip = "bip122:000000000019d6689c085ae165831e93/slip44:0";
    let asset = selectedAsset;
    if (!asset || asset.caip !== btcCaip) {
      asset = portfolio?.assets.find((a: any) => a.caip === btcCaip) || null;
      if (!asset) {
        console.error(tag, '❌ No BTC asset found in portfolio');
        return null;
      }
      selectAsset(asset);
      // Wait a tick for context update
      await new Promise(res => setTimeout(res, 50));
    }

    console.log(tag, `📥 Getting receive address for ${asset.symbol} using device queue`);
    
    try {
      // Get connected devices from the API instead of database
      const connectedDevices = await DeviceQueueAPI.getConnectedDevices();
      if (!connectedDevices || connectedDevices.length === 0) {
        throw new Error('No devices available');
      }
      // Derive the canonical hardware id from the device object returned by Rust
      const deviceId = getCanonicalDeviceId(connectedDevices[0]);
      console.log(tag, '🔑 Using deviceId:', deviceId);
      if (!deviceId || typeof deviceId !== 'string' || deviceId.trim() === '') {
        throw new Error('Invalid device_id resolved from device object: ' + JSON.stringify(connectedDevices[0]));
      }

      // For Bitcoin, use the first available path (can be enhanced later)
      const receivePath = "m/84'/0'/0'/0/0"; // Native SegWit receive path

      // (1) Generate a unique request ID first
      const requestId = `addr_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
      console.log(tag, '🆔 Generated requestId:', requestId);

      // (2) Store resolver functions in variables
      let resolveAddress: (addr: string) => void;
      let rejectAddress: (error: any) => void;

      // (3) Create the promise and capture the resolvers
      const addrPromise = new Promise<string>((resolve, reject) => {
        resolveAddress = resolve;
        rejectAddress = reject;
      });

      // (4) Set up the resolver in the map BEFORE sending the request
      pendingReceiveRequests.set(requestId, { 
        resolve: resolveAddress!, 
        reject: rejectAddress! 
      });

      console.log(tag, `📋 Added request ${requestId} to pending map BEFORE device call. Current pending count:`, pendingReceiveRequests.size);

      // (5) NOW queue the request with our pre-generated ID
      try {
        await DeviceQueueAPI.requestReceiveAddressFromDeviceWithId(
          deviceId,
          receivePath,
          'Bitcoin',
          'p2wpkh',
          true, // Always show on device
          requestId // Pass our pre-generated ID
        );
        
        console.log(tag, '🟢 Device request queued with requestId:', requestId);
      } catch (error) {
        // If the device call fails, clean up the pending request
        pendingReceiveRequests.delete(requestId);
        throw error;
      }

      // Hard timeout so the UI is never stuck forever
      setTimeout(() => {
        if (pendingReceiveRequests.delete(requestId)) {
          console.log(tag, `⏰ Request ${requestId} timed out`);
          rejectAddress!(new Error('Timeout waiting for device response'));
        }
      }, 60_000);

      return addrPromise; // Receive.tsx can still await this
    } catch (error) {
      console.error(tag, '❌ Failed to get receive address from device:', error);
      throw error;
    }
  };

  const requestXpubFromDevice = async (deviceId: string, path: string): Promise<string> => {
    return DeviceQueueAPI.requestXpubFromDevice(deviceId, path);
  };

  const getQueueStatus = async (deviceId: string): Promise<QueueStatus> => {
    return DeviceQueueAPI.getQueueStatus(deviceId);
  };

  const signTransaction = async (
    deviceId: string,
    coin: string,
    inputs: any[],
    outputs: any[],
    version: number = 1,
    lockTime: number = 0
  ): Promise<string> => {
    const tag = TAG + " | signTransaction | ";
    
    try {
      console.log(tag, `🔐 Signing transaction on device ${deviceId}`);
      
      // (1) Generate a unique request ID first
      const requestId = `sign_tx_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
      let finalRequestId = requestId; // Track the actual request ID for cleanup
      console.log(tag, '🆔 Generated requestId:', requestId);
      
      // (2) Create promise resolvers
      let resolveSignature: (signedTx: string) => void;
      let rejectSignature: (error: any) => void;
      
      const signingPromise = new Promise<string>((resolve, reject) => {
        resolveSignature = resolve;
        rejectSignature = reject;
      });
      
      // (3) Store the resolvers using the pre-generated request ID BEFORE device call
      pendingSigningRequests.set(requestId, {
        resolve: resolveSignature!,
        reject: rejectSignature!
      });
      
      console.log(tag, `📋 Added signing request ${requestId} to pending map BEFORE device call`);
      
      // (4) NOW queue the signing request with our pre-generated ID
      try {
        // We need to modify the API to accept a pre-generated request ID
        const actualRequestId = await DeviceQueueAPI.signTransactionWithId(deviceId, coin, inputs, outputs, version, lockTime, requestId);
        console.log(tag, '🆔 Device call completed with requestId:', actualRequestId);
        
        // Verify the request ID matches what we expect
        if (actualRequestId !== requestId) {
          console.warn(tag, `⚠️ Request ID mismatch! Expected: ${requestId}, Got: ${actualRequestId}`);
          // Update the map with the actual ID if they differ
          const resolver = pendingSigningRequests.get(requestId);
          if (resolver) {
            pendingSigningRequests.delete(requestId);
            pendingSigningRequests.set(actualRequestId, resolver);
            finalRequestId = actualRequestId; // Update for timeout cleanup
          }
        }
      } catch (error) {
        // If the device call fails, clean up the pending request
        pendingSigningRequests.delete(requestId);
        throw error;
      }
      
      console.log(tag, '🟢 Transaction signing request queued - waiting for device:response event');
      
      // Set timeout to prevent infinite waiting
      setTimeout(() => {
        // Try to clean up using either the original or updated request ID
        const deleted = pendingSigningRequests.delete(finalRequestId) || pendingSigningRequests.delete(requestId);
        if (deleted) {
          console.log(tag, `⏰ Signing request ${finalRequestId} timed out`);
          rejectSignature!(new Error('Timeout waiting for transaction signing'));
        }
      }, 120_000); // 2-minute timeout for signing
      
      return signingPromise;
    } catch (error) {
      console.error(tag, '❌ Failed to sign transaction:', error);
      throw error;
    }
  };

  // Listen for device responses (event-driven xpub and address handling)
  useEffect(() => {
    let unlistenResponse: Promise<() => void>;
    
    (async () => {
      unlistenResponse = listen('device:response', async (event: any) => {
        const tag = TAG + " | device:response | ";
        
        console.log(tag, `📡 Raw event received:`, event);
        console.log(tag, `📡 Event payload:`, event.payload);
        
        // Validate event structure
        if (!event.payload) {
          console.error(tag, '❌ Event payload is missing');
          return;
        }
        
        const { device_id, request_id, response } = event.payload;
        
        if (!device_id || !request_id || !response) {
          console.error(tag, '❌ Event payload missing required fields:', { device_id, request_id, response });
          return;
        }
        
        console.log(tag, `📥 Extracted - device_id: ${device_id}, request_id: ${request_id}`);
        console.log(tag, `📥 Response type:`, Object.keys(response || {}));
        console.log(tag, `📥 Response content:`, response);
        
        // EXPLICIT DEBUGGING FOR SIGNING EVENTS
        if ('SignedTransaction' in response) {
          console.log(tag, `🔐 FOUND SignedTransaction event!`);
          console.log(tag, `🔐 SignedTransaction data:`, response.SignedTransaction);
          console.log(tag, `🔐 Current pending signing requests:`, Array.from(pendingSigningRequests.keys()));
        }
        
        // Handle xpub responses
        if (response && 'Xpub' in response) {
          const xpubResponse = response.Xpub;
          const requestKey = `${device_id}:${xpubResponse.path}`;
          
          // Remove from pending requests
          pendingXpubRequests.delete(requestKey);
          
          if (xpubResponse.success) {
            // Find the path info to get the CAIP
            const requiredPaths = [
              { path: "m/44'/0'/0'", caip: "bip122:000000000019d6689c085ae165831e93/slip44:0" },  // Legacy P2PKH (Account level)
              { path: "m/49'/0'/0'", caip: "bip122:000000000019d6689c085ae165831e93/slip44:0" },  // SegWit P2SH (Account level)
              { path: "m/84'/0'/0'", caip: "bip122:000000000019d6689c085ae165831e93/slip44:0" }   // Native SegWit P2WPKH (Account level)
            ]; // No DB, use static array
            const pathInfo = requiredPaths.find(p => p.path === xpubResponse.path);
            
            if (pathInfo) {
              const xpubData = {
                path: xpubResponse.path,
                xpub: xpubResponse.xpub,
                caip: pathInfo.caip
              };
              
              setFetchedXpubs(prev => {
                // Avoid duplicates
                const exists = prev.some(x => x.path === xpubData.path && x.caip === xpubData.caip);
                if (exists) {
                  console.log(tag, `Duplicate xpub for ${xpubData.path}, skipping`);
                  return prev;
                }

                console.log(tag, `✅ Added xpub for ${xpubData.path}: ${xpubData.xpub.substring(0, 20)}...`);
                const newXpubs = [...prev, xpubData];
                console.log(tag, `Now have ${newXpubs.length} xpubs in memory:`, newXpubs.map(x => x.path));

                // Let useEffect handle portfolio refresh when fetchedXpubs updates
                const requiredPaths = [
                  { path: "m/44'/0'/0'", caip: "bip122:000000000019d6689c085ae165831e93/slip44:0" },  // Legacy P2PKH (Account level)
                  { path: "m/49'/0'/0'", caip: "bip122:000000000019d6689c085ae165831e93/slip44:0" },  // SegWit P2SH (Account level)
                  { path: "m/84'/0'/0'", caip: "bip122:000000000019d6689c085ae165831e93/slip44:0" }   // Native SegWit P2WPKH (Account level)
                ]; // No DB, use static array
                const expectedXpubCount = requiredPaths.length;
                if (newXpubs.length === expectedXpubCount) {
                  console.log(tag, `All expected xpubs (${expectedXpubCount}) are present. Portfolio will refresh via useEffect.`);
                } else {
                  console.log(tag, `Waiting for all xpubs: have ${newXpubs.length}, need ${expectedXpubCount}`);
                }

                return newXpubs;
              });
            }
          } else {
            console.error(tag, `❌ Xpub request failed for ${xpubResponse.path}:`, xpubResponse.error);
          }
        }
        
        // Handle address responses
        if (response && 'Address' in response) {
          const addressResponse = response.Address;
          console.log(tag, `📥 Address response:`, addressResponse);
          
          console.log(tag, `🔍 Looking for request_id: ${request_id} in pending map`);
          console.log(tag, `📋 Current pending requests:`, Array.from(pendingReceiveRequests.keys()));
          
          const entry = pendingReceiveRequests.get(request_id);
          if (!entry) {
            console.log(tag, 'Address response for unknown request_id:', request_id);
            return; // Response for something else
          }

          pendingReceiveRequests.delete(request_id);

          if (addressResponse.success) {
            console.log(tag, `✅ Address generated: ${addressResponse.address}`);
            entry.resolve(addressResponse.address); // unblocks await
            setLastReceiveAddress(addressResponse.address); // <-- context state
          } else {
            console.error(tag, `❌ Address request failed:`, addressResponse.error);
            entry.reject(new Error(addressResponse.error || 'Device error'));
          }
        }
        
        // Handle transaction signing responses
        if (response && 'SignedTransaction' in response) {
          const signingResponse = response.SignedTransaction;
          console.log(tag, `🔐 Transaction signing response received:`, signingResponse);
          console.log(tag, `🆔 Response request_id: ${request_id}`);
          console.log(tag, `📊 Response device_id: ${device_id}`);
          
          console.log(tag, `🔍 Looking for signing request_id: ${request_id} in pending map`);
          console.log(tag, `📋 Current pending signing requests:`, Array.from(pendingSigningRequests.keys()));
          
          const entry = pendingSigningRequests.get(request_id);
          if (!entry) {
            console.error(tag, `❌ Transaction signing response for unknown request_id: ${request_id}`);
            console.error(tag, `❌ Available pending requests:`, Array.from(pendingSigningRequests.keys()));
            return; // Response for something else
          }

          console.log(tag, `✅ Found matching pending signing request for: ${request_id}`);
          pendingSigningRequests.delete(request_id);

          if (signingResponse.success) {
            console.log(tag, `✅ Transaction signed successfully via event!`);
            console.log(tag, `🔐 Signed transaction hex length:`, signingResponse.signed_tx.length);
            console.log(tag, `🔐 Signed transaction preview:`, signingResponse.signed_tx.substring(0, 40) + '...');
            console.log(tag, `🚀 Resolving promise with signed transaction`);
            entry.resolve(signingResponse.signed_tx);
          } else {
            console.error(tag, `❌ Transaction signing failed:`, signingResponse.error);
            entry.reject(new Error(signingResponse.error || 'Device signing error'));
          }
        }
      });
      
      console.log(TAG, '✅ Device response event listener established');
    })();

    return () => {
      console.log(TAG, '🧹 Cleaning up device response event listener');
      unlistenResponse?.then(fn => fn());
    };
  }, [refreshPortfolio]);

  // Listen for device ready events and automatically fetch xpubs
  useEffect(() => {
    const tag = TAG + " | device:ready useEffect | ";
    console.log(tag, '🎧 Setting up device:ready listener...');
    
    let unlistenDeviceReady: Promise<() => void>;
    (async () => {
      try {
        unlistenDeviceReady = listen('device:ready', async (event: any) => {
          const listenerTag = TAG + " | device:ready listener | ";
          console.log(listenerTag, '📡 Device ready event received in WalletContext!');
          console.log(listenerTag, '📄 Full event object:', event);
          console.log(listenerTag, '📄 Event payload:', event.payload);
          
          if (event.payload?.device && event.payload?.features) {
            const device = event.payload.device;
            const features = event.payload.features;
            
            console.log(listenerTag, `✅ Device ready: ${features.label || 'Unlabeled'} v${features.version}`);
            
            // Double-check that device is truly ready before fetching xpubs
            if (features.bootloader_mode || features.bootloaderMode) {
              console.log(listenerTag, '⚠️ Device in bootloader mode, skipping xpub fetch');
              return;
            }
            
            if (!features.initialized) {
              console.log(listenerTag, '⚠️ Device not initialized, skipping xpub fetch');
              return;
            }
            
            console.log(listenerTag, '🔄 Device is properly ready, fetching xpubs...');
            
            try {
              await getXpubsFromDeviceQueue();
              console.log(listenerTag, '✅ Xpub fetching initiated successfully');
            } catch (error) {
              console.error(listenerTag, '❌ Failed to fetch xpubs on device ready:', error);
            }
          } else {
            console.log(listenerTag, '⚠️ Device ready event payload missing device or features');
            console.log(listenerTag, '⚠️ Payload structure:', Object.keys(event.payload || {}));
          }
        });
        console.log(tag, '✅ Device ready listener set up successfully');
      } catch (error) {
        console.error(tag, '❌ Failed to set up device:ready listener:', error);
      }
    })();

    return () => {
      console.log(tag, '🧹 Cleaning up device:ready listener');
      unlistenDeviceReady?.then(fn => fn());
    };
  }, []);

  // Listen for device reconnects and purge queue
  useEffect(() => {
    let unlistenConnect: Promise<() => void>;
    let unlistenDisconnect: Promise<() => void>;
    
    (async () => {
      // Handle device connections
      unlistenConnect = listen('device:connected', async (event: any) => {
        const deviceId = event.payload?.device_id || event.payload;
        console.debug('[WalletContext] deviceId from connect event payload:', deviceId);
        try {
          console.log(TAG, 'Device reconnected', deviceId, '- resetting queue');
          await DeviceQueueAPI.resetDeviceQueue(deviceId);
          // Don't call getXpubsFromDeviceQueue here - wait for device:ready instead
        } catch (e) {
          console.error(TAG, 'Failed to reset queue on reconnect:', e);
        }
      });
      
      // Handle device disconnections  
      unlistenDisconnect = listen('device:disconnected', async (event: any) => {
        const deviceId = event.payload;
        console.debug('[WalletContext] deviceId from disconnect event payload:', deviceId);
        try {
          console.log(TAG, 'Device disconnected', deviceId, '- cleaning up queue');
          await DeviceQueueAPI.resetDeviceQueue(deviceId);
          
          // Clear any pending requests for this device
          const keysToDelete = [];
          for (const requestKey of pendingXpubRequests) {
            if (requestKey.startsWith(deviceId + ':')) {
              keysToDelete.push(requestKey);
            }
          }
          keysToDelete.forEach(key => pendingXpubRequests.delete(key));
          
          // Clear pending receive and signing requests that might be waiting for this device
          // Note: These use request_id as keys, so we can't easily match by device_id
          // The timeout mechanisms will handle cleanup, and errors will be thrown when device is gone
          
          console.log(TAG, `Cleared ${keysToDelete.length} pending xpub requests for disconnected device`);
          
        } catch (e) {
          console.error(TAG, 'Failed to cleanup queue on disconnect:', e);
        }
      });
    })();

    return () => {
      unlistenConnect?.then(fn => fn());
      unlistenDisconnect?.then(fn => fn());
    };
  }, []);

  // Watch fetchedXpubs and refresh portfolio when all expected xpubs are present
  useEffect(() => {
    const tag = TAG + " | fetchedXpubs useEffect | ";
    
    if (fetchedXpubs.length === 0) {
      console.log(tag, 'No xpubs in memory yet');
      return;
    }
    
    const requiredPaths = [
      { path: "m/44'/0'/0'", caip: "bip122:000000000019d6689c085ae165831e93/slip44:0" },  // Legacy P2PKH (Account level)
      { path: "m/49'/0'/0'", caip: "bip122:000000000019d6689c085ae165831e93/slip44:0" },  // SegWit P2SH (Account level)
      { path: "m/84'/0'/0'", caip: "bip122:000000000019d6689c085ae165831e93/slip44:0" }   // Native SegWit P2WPKH (Account level)
    ]; // No DB, use static array
    const expectedXpubCount = requiredPaths.length;
    
    console.log(tag, `Current xpubs: ${fetchedXpubs.length}, expected: ${expectedXpubCount}`);
    
    if (fetchedXpubs.length === expectedXpubCount) {
      console.log(tag, `All ${expectedXpubCount} xpubs present, refreshing portfolio with current state`);
      refreshPortfolio();
    }
  }, [fetchedXpubs, refreshPortfolio]);

  // Initial load
  useEffect(() => {
    onStart();
  }, []);

  // Public context value ------------------------------------------------------
  const contextValue: WalletContextType = {
    portfolio,
    selectedAsset,
    loading,
    error,
    isSync,
    lastReceiveAddress,
    fetchedXpubs,
    refreshPortfolio,
    selectAsset,
    sendAsset,
    getReceiveAddress,
    requestXpubFromDevice,
    getQueueStatus,
    signTransaction,
    reinitialize: onStart,
  };

  return (
    <WalletContext.Provider value={contextValue}>
      {children}
    </WalletContext.Provider>
  );
};

// Hook to use the context
export const useWallet = (): WalletContextType => {
  const context = useContext(WalletContext);
  if (context === undefined) {
    throw new Error('useWallet must be used within a WalletProvider');
  }
  return context;
}; 