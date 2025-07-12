#!/bin/bash

# Test Log Viewer in Settings
# This script demonstrates the new terminal-style log viewer

set -e

echo "🔍 Testing Terminal-Style Log Viewer"
echo "====================================="

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}📁 Checking existing logs...${NC}"
LOG_DIR="$HOME/.keepkey/logs"
echo "Log directory: $LOG_DIR"

if [ -d "$LOG_DIR" ]; then
    echo -e "${GREEN}✅ Log directory exists${NC}"
    echo "Recent log files:"
    ls -la "$LOG_DIR" | tail -5
    
    # Count total log entries
    LATEST_LOG=$(ls -t "$LOG_DIR"/device-communications-*.log 2>/dev/null | head -1 || true)
    if [ -n "$LATEST_LOG" ] && [ -f "$LATEST_LOG" ]; then
        ENTRY_COUNT=$(wc -l < "$LATEST_LOG" 2>/dev/null || echo "0")
        echo "Latest log file: $(basename "$LATEST_LOG")"
        echo "Total entries: $ENTRY_COUNT"
    fi
else
    echo -e "${YELLOW}⚠️  Log directory doesn't exist yet${NC}"
fi

echo ""
echo -e "${BLUE}🚀 Starting vault-v2 with new log viewer...${NC}"

cd /Users/highlander/keepkey-bitcoin-only/projects/vault-v2

echo ""
echo -e "${GREEN}✨ New Features Added:${NC}"
echo ""
echo "📊 Terminal-Style Log Viewer in Settings:"
echo "   • Navigate to Settings → Logs tab"
echo "   • View device communications in terminal format:"
echo "     -> GetFeatures"
echo "     <- Features: Key Hodler v7.10.0 ✅"
echo "     🔧 Bootloader check: 2.1.4 -> needs update: false"
echo ""
echo "🔧 Available Controls:"
echo "   • Auto Refresh: Live updates every 2 seconds"
echo "   • Manual Refresh: Update logs on demand"
echo "   • Download: Copy log file path to clipboard"
echo "   • Cleanup: Remove old log files (30+ days)"
echo "   • Search: Filter logs by content"
echo "   • Filter: Show only requests, responses, or errors"
echo "   • Limit: Control number of entries displayed (25-200)"
echo ""
echo "🎨 Color Coding:"
echo "   • Blue: Outgoing requests (-> GetFeatures)"
echo "   • Green: Successful responses (<- Features: Device v1.0.0 ✅)"
echo "   • Red: Failed responses (<- Error ❌)"
echo "   • Yellow: System checks (🔧 Bootloader check)"
echo "   • Cyan: Network operations (📡 Fetching features)"
echo ""
echo "💡 Usage Tips:"
echo "   • Enable 'Auto Refresh' for live monitoring"
echo "   • Use search to find specific device IDs or operations"
echo "   • Filter by 'errors' to debug issues quickly"
echo "   • Download logs for external analysis"

echo ""
echo -e "${BLUE}🏃 Running application...${NC}"
echo "Open Settings → Logs tab to view the new terminal-style log viewer!"

# Start the application
bun run tauri dev 