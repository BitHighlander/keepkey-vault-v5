import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

// Cache onboarding state to prevent duplicate backend calls
let onboardingCache: { isFirstTime?: boolean; isOnboarded?: boolean } = {};
let isLoading = false;
let loadPromise: Promise<{ isFirstTime: boolean; isOnboarded: boolean }> | null = null;

export function useOnboardingState() {
  const [state, setState] = useState<{
    isFirstTime: boolean | null;
    isOnboarded: boolean | null;
    loading: boolean;
  }>({
    isFirstTime: onboardingCache.isFirstTime ?? null,
    isOnboarded: onboardingCache.isOnboarded ?? null,
    loading: false,
  });

  useEffect(() => {
    const loadState = async () => {
      // If already cached, use cached values
      if (onboardingCache.isFirstTime !== undefined && onboardingCache.isOnboarded !== undefined) {
        setState({
          isFirstTime: onboardingCache.isFirstTime,
          isOnboarded: onboardingCache.isOnboarded,
          loading: false,
        });
        return;
      }

      // If already loading, wait for existing promise
      if (loadPromise) {
        setState(prev => ({ ...prev, loading: true }));
        try {
          const result = await loadPromise;
          setState({
            isFirstTime: result.isFirstTime,
            isOnboarded: result.isOnboarded,
            loading: false,
          });
        } catch (error) {
          console.error('Failed to load onboarding state from promise:', error);
          setState(prev => ({ ...prev, loading: false }));
        }
        return;
      }

      // Start new load
      setState(prev => ({ ...prev, loading: true }));
      isLoading = true;

      loadPromise = (async () => {
        try {
          console.log('Loading onboarding state from backend...');
          const [isFirstTime, isOnboarded] = await Promise.all([
            invoke<boolean>('is_first_time_install'),
            invoke<boolean>('is_onboarded'),
          ]);

          // Cache the results
          onboardingCache = { isFirstTime, isOnboarded };
          console.log('Onboarding state cached:', onboardingCache);

          return { isFirstTime, isOnboarded };
        } finally {
          isLoading = false;
          loadPromise = null;
        }
      })();

      try {
        const result = await loadPromise;
        setState({
          isFirstTime: result.isFirstTime,
          isOnboarded: result.isOnboarded,
          loading: false,
        });
      } catch (error) {
        console.error('Failed to load onboarding state:', error);
        setState(prev => ({ ...prev, loading: false }));
      }
    };

    loadState();
  }, []);

  // Clear cache when onboarding is completed
  const clearCache = () => {
    onboardingCache = {};
    loadPromise = null;
  };

  const shouldShowOnboarding = state.isFirstTime === true || state.isOnboarded === false;

  return {
    ...state,
    shouldShowOnboarding,
    clearCache,
  };
} 