# Device Invalid State Handling

## Overview

This document describes the improved device invalid state handling and dialog management system implemented in vault-v2.

## Problem

Previously, when a device timeout error occurred ("Failed to get features for keepkey_xxx: Timeout while fetching device features"), the system would either:
- Show a complex multi-step troubleshooting wizard
- Allow multiple dialogs to stack on top of each other
- Have z-index issues where PIN dialogs could be hidden behind other dialogs

## Solution

### 1. Simple Device Invalid State Dialog

Created a new `DeviceInvalidStateDialog` component that:
- Shows a single page with clear instructions
- Tells users to unplug and reconnect their device normally
- Warns against holding buttons during reconnection
- Has an orange warning border for visibility
- Displays the actual error for debugging

### 2. Improved Dialog Management

Enhanced the `DialogContext` to:
- Prevent multiple dialogs from opening simultaneously
- Automatically close lower priority dialogs when higher priority ones need to show
- Give PIN dialogs the highest priority (z-index: 99999)
- Implement a proper queueing system
- Add `hideAllExcept` function for critical dialogs

### 3. Priority System

Dialog priorities are now:
1. **PIN dialogs**: Always highest priority (critical + special handling)
2. **Device invalid state**: High priority
3. **Update dialogs**: Normal/high priority
4. **Other dialogs**: Lower priority

### 4. Backend Error Detection

Updated `event_controller.rs` to:
- Detect timeout errors specifically
- Emit `device:invalid-state` event
- Log "OOPS this should never happen" for debugging
- Include detailed error information in the event payload

## Implementation Details

### Files Changed

1. **`src/components/DeviceInvalidStateDialog.tsx`** - New dialog component
2. **`src/contexts/DialogContext.tsx`** - Enhanced dialog management
3. **`src/components/DeviceUpdateManager.tsx`** - Added invalid state event listener
4. **`src-tauri/src/event_controller.rs`** - Added timeout detection and event emission
5. **`src/components/PinUnlockDialog.tsx`** - Increased z-index to 99999

### Event Flow

1. Device timeout occurs in backend
2. Backend emits `device:invalid-state` event
3. DeviceUpdateManager receives event
4. All existing dialogs are cleared
5. DeviceInvalidStateDialog is shown
6. User reconnects device
7. Normal device detection flow resumes

### Testing

Use the test script at `skills/test_device_invalid_state.sh` to verify the functionality.

## Future Improvements

1. Add timeout prevention mechanisms to avoid the error entirely
2. Implement device health monitoring
3. Add automatic recovery attempts before showing the dialog
4. Track frequency of invalid states for diagnostics 