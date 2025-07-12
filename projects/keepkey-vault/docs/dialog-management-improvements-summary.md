# Dialog Management Improvements Summary

## Issues Fixed

### 1. Device Timeout Error Handling
- **Problem**: "Failed to get features for keepkey_xxx: Timeout while fetching device features" errors
- **Solution**: Created a simple `DeviceInvalidStateDialog` component that shows clear reconnection instructions
- **Implementation**: 
  - Added `device:invalid-state` event emission in `event_controller.rs`
  - Created new dialog component with orange warning border
  - Simple one-page instructions instead of complex wizard

### 2. Gray Screen Blocking UI
- **Problem**: Dialog overlay (z-index: 9999) was showing without content, blocking the entire screen
- **Solution**: 
  - Added dialog queue debugging in `DialogContext`
  - Implemented `hideAll()` in `VaultInterface` on mount to clear stuck dialogs
  - Added dialog monitoring in `App.tsx` to detect and clear stuck dialogs

### 3. Dialog Priority System
- **Problem**: Multiple dialogs could stack, PIN dialogs could be hidden behind others
- **Solution**: Implemented proper priority system:
  - PIN dialogs: Always highest priority (z-index: 99999) 
  - Device invalid state: High priority
  - Update dialogs: Normal/high priority
  - Other dialogs: Lower priority

### 4. Portfolio Not Loading
- **Problem**: Portfolio showed as `null` despite successful API calls
- **Root Cause**: Dialog overlay was blocking UI, not a data issue
- **Fixed By**: Clearing stuck dialogs when VaultInterface mounts

## Key Changes

### Files Modified

1. **`src/components/DeviceInvalidStateDialog.tsx`** (NEW)
   - Simple dialog for device invalid state
   - Clear reconnection instructions
   - Orange warning styling

2. **`src/contexts/DialogContext.tsx`**
   - Added `hideAllExcept()` function
   - Improved dialog queueing logic
   - Special handling for PIN dialogs
   - Debug logging for active dialogs

3. **`src-tauri/src/event_controller.rs`**
   - Detect timeout errors specifically
   - Emit `device:invalid-state` event
   - Log "OOPS this should never happen"

4. **`src/components/DeviceUpdateManager.tsx`**
   - Listen for `device:invalid-state` events
   - Show DeviceInvalidStateDialog
   - Clear other dialogs when showing invalid state

5. **`src/components/PinUnlockDialog.tsx`**
   - Increased z-index to 99999
   - Added lifecycle logging

6. **`src/App.tsx`**
   - Created `AppContent` component with dialog context access
   - Added dialog queue monitoring
   - Auto-clear stuck dialogs before showing VaultInterface

7. **`src/components/VaultInterface.tsx`**
   - Added `hideAll()` on mount to clear any stuck dialogs

## How It Works

1. **Device Timeout Detection**:
   ```
   Backend timeout → device:invalid-state event → DeviceInvalidStateDialog shown
   ```

2. **Dialog Priority**:
   ```
   PIN dialog requested → Clear lower priority dialogs → Show PIN with z-index: 99999
   ```

3. **Stuck Dialog Prevention**:
   ```
   VaultInterface mounts → hideAll() called → Any stuck dialogs cleared
   ```

## Testing

Use `skills/test_device_invalid_state.sh` to test the new functionality.

## Future Improvements

1. Add timeout prevention in device communication
2. Implement retry logic before showing error dialog
3. Add dialog history tracking for debugging
4. Create dialog timeout mechanism to auto-close stuck dialogs 