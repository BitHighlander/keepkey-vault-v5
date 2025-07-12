#!/bin/bash

# Test Device Invalid State Dialog
# This script demonstrates the new simple troubleshooting dialog

set -e

echo "🔍 Testing Device Invalid State Dialog"
echo "====================================="

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

cd /Users/highlander/keepkey-bitcoin-only/projects/vault-v2

echo ""
echo -e "${GREEN}✨ New Feature: Simple Device Invalid State Dialog${NC}"
echo ""
echo "📋 What's been implemented:"
echo ""
echo "1. Automatic Detection:"
echo "   • Detects 'Timeout while fetching device features' errors"
echo "   • Shows a simple, single-page dialog instead of complex wizard"
echo "   • Clear instructions: Unplug and reconnect device normally"
echo ""
echo "2. Improved Dialog Management:"
echo "   • Prevents multiple dialogs from opening simultaneously"
echo "   • PIN dialog always has highest priority (z-index: 99999)"
echo "   • Lower priority dialogs are automatically closed"
echo "   • Queuing system prevents dialog conflicts"
echo ""
echo "3. Better Error Handling:"
echo "   • Specific handling for device timeout errors"
echo "   • Emits 'device:invalid-state' event from backend"
echo "   • Frontend shows DeviceInvalidStateDialog component"
echo "   • All other dialogs are closed to prevent confusion"
echo ""
echo -e "${BLUE}🧪 How to Test:${NC}"
echo ""
echo "1. Connect a KeepKey device"
echo "2. Cause a timeout error by:"
echo "   • Disconnecting during communication"
echo "   • Using device with another app"
echo "   • Device in unexpected state"
echo ""
echo "3. You'll see the new dialog appear with:"
echo "   • Orange warning border"
echo "   • Clear error message"
echo "   • Simple reconnection instructions"
echo "   • No complex wizard steps"
echo ""
echo -e "${YELLOW}⚠️  Important Changes:${NC}"
echo ""
echo "• Old behavior: Complex troubleshooting wizard with multiple steps"
echo "• New behavior: Simple one-page dialog with reconnection instructions"
echo "• PIN dialogs now always stay on top when device is ready"
echo "• Prevents dialog stacking and z-axis issues"
echo ""
echo -e "${GREEN}📱 Dialog Priority System:${NC}"
echo ""
echo "1. PIN dialogs: Always highest priority (critical)"
echo "2. Device invalid state: High priority"
echo "3. Update dialogs: Normal/high priority"
echo "4. Other dialogs: Lower priority"
echo ""
echo "Lower priority dialogs are automatically closed when"
echo "higher priority dialogs need to be shown."
echo ""
echo -e "${BLUE}🚀 Starting vault-v2...${NC}"
echo "Watch for the new dialog when device timeouts occur!"

# Start the application
bun run tauri dev 