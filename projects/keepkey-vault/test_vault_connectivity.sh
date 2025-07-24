#!/bin/bash

# Test script to verify connectivity to vault.keepkey.com
# This helps diagnose DNS and network connectivity issues

set -e

TAG="[VAULT CONNECTIVITY TEST]"

echo "$TAG Testing connectivity to vault.keepkey.com..."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print status
print_status() {
    local status=$1
    local message=$2
    case $status in
        "SUCCESS")
            echo -e "${GREEN}✅ $message${NC}"
            ;;
        "WARNING")
            echo -e "${YELLOW}⚠️  $message${NC}"
            ;;
        "ERROR")
            echo -e "${RED}❌ $message${NC}"
            ;;
        "INFO")
            echo -e "${BLUE}ℹ️  $message${NC}"
            ;;
    esac
}

# Test 1: DNS Resolution
print_status "INFO" "Testing DNS resolution for vault.keepkey.com..."

if nslookup vault.keepkey.com > /dev/null 2>&1; then
    print_status "SUCCESS" "DNS resolution for vault.keepkey.com works"
    IP=$(nslookup vault.keepkey.com | grep -A1 "Name:" | tail -1 | awk '{print $2}')
    print_status "INFO" "Resolved IP: $IP"
else
    print_status "ERROR" "DNS resolution failed for vault.keepkey.com"
    print_status "INFO" "Trying alternative DNS resolution methods..."
    
    # Try with dig if available
    if command -v dig > /dev/null 2>&1; then
        print_status "INFO" "Using dig to resolve vault.keepkey.com..."
        dig vault.keepkey.com +short
    fi
    
    # Try with host if available  
    if command -v host > /dev/null 2>&1; then
        print_status "INFO" "Using host to resolve vault.keepkey.com..."
        host vault.keepkey.com
    fi
fi

# Test 2: Basic connectivity
print_status "INFO" "Testing basic connectivity to vault.keepkey.com..."

if ping -c 3 vault.keepkey.com > /dev/null 2>&1; then
    print_status "SUCCESS" "Can ping vault.keepkey.com"
else
    print_status "WARNING" "Cannot ping vault.keepkey.com (may be normal - many servers block ICMP)"
fi

# Test 3: HTTPS connectivity
print_status "INFO" "Testing HTTPS connectivity to vault.keepkey.com..."

HTTP_STATUS=$(curl -s -o /dev/null -w "%{http_code}" --connect-timeout 10 --max-time 30 https://vault.keepkey.com/ 2>/dev/null || echo "000")

if [ "$HTTP_STATUS" != "000" ]; then
    print_status "SUCCESS" "HTTPS connection successful - Status: $HTTP_STATUS"
    
    # Get some basic info about the site
    print_status "INFO" "Getting site information..."
    curl -s -I --connect-timeout 10 --max-time 30 https://vault.keepkey.com/ | head -10
    
else
    print_status "ERROR" "HTTPS connection failed"
    print_status "INFO" "Trying detailed curl to see the error..."
    curl -v --connect-timeout 10 --max-time 30 https://vault.keepkey.com/ || true
fi

# Test 4: Check for corporate/firewall restrictions
print_status "INFO" "Checking for potential network restrictions..."

# Test if we can resolve and connect to other sites
if curl -s --connect-timeout 5 --max-time 10 https://google.com > /dev/null 2>&1; then
    print_status "SUCCESS" "General internet connectivity works (google.com reachable)"
else
    print_status "ERROR" "General internet connectivity issues detected"
fi

# Test 5: Check local DNS settings
print_status "INFO" "Checking local DNS configuration..."

if [ -f /etc/resolv.conf ]; then
    print_status "INFO" "DNS servers configured:"
    grep nameserver /etc/resolv.conf | head -3
else
    print_status "WARNING" "Cannot read /etc/resolv.conf"
fi

# Test 6: Test with the vault proxy
print_status "INFO" "Testing if the vault proxy can handle the connection..."

# Build the project to ensure latest changes
print_status "INFO" "Building vault with latest proxy changes..."
if cargo build --quiet; then
    print_status "SUCCESS" "Vault built successfully"
else
    print_status "ERROR" "Vault build failed"
    exit 1
fi

print_status "INFO" "Starting vault briefly to test proxy connectivity..."

# Start vault in background with timeout
timeout 20s cargo run > vault_connectivity_test.log 2>&1 &
VAULT_PID=$!

# Wait a bit for startup
sleep 8

# Test proxy endpoint
if curl -s --connect-timeout 5 "http://localhost:8080/" > proxy_test_response.html 2>/dev/null; then
    print_status "SUCCESS" "Local proxy is responding"
    
    # Check if we got content from vault.keepkey.com or an error
    if grep -q "KeepKey" proxy_test_response.html; then
        print_status "SUCCESS" "Proxy successfully connected to vault.keepkey.com!"
        print_status "INFO" "Response contains KeepKey content"
    elif grep -q "DNS resolution failed" proxy_test_response.html; then
        print_status "ERROR" "Proxy reports DNS resolution failure"
        print_status "INFO" "This confirms the DNS issue in the proxy"
    elif grep -q "connection failed\|timeout\|unreachable" proxy_test_response.html; then
        print_status "ERROR" "Proxy reports connection failure"
        print_status "INFO" "Network connectivity issue detected"
    else
        print_status "WARNING" "Proxy responded but with unexpected content"
        print_status "INFO" "First few lines of response:"
        head -5 proxy_test_response.html
    fi
else
    print_status "WARNING" "Local proxy not responding yet (may need more time to start)"
fi

# Cleanup
if kill -0 $VAULT_PID 2>/dev/null; then
    kill $VAULT_PID 2>/dev/null || true
    sleep 2
    kill -9 $VAULT_PID 2>/dev/null || true
fi

# Summary
echo ""
print_status "INFO" "=== CONNECTIVITY TEST SUMMARY ==="

if [ "$HTTP_STATUS" != "000" ]; then
    print_status "SUCCESS" "✅ vault.keepkey.com is reachable and responding"
    print_status "INFO" "The original DNS error was likely a temporary issue or environment-specific"
    print_status "INFO" "The proxy should now work correctly with the real site"
else
    print_status "ERROR" "❌ vault.keepkey.com is not reachable from this environment"
    print_status "INFO" "Possible causes:"
    print_status "INFO" "  - Network connectivity issues"
    print_status "INFO" "  - DNS resolution problems"
    print_status "INFO" "  - Firewall/proxy restrictions"
    print_status "INFO" "  - Corporate network filtering"
fi

# Cleanup test files
rm -f vault_connectivity_test.log proxy_test_response.html

echo "$TAG Test completed" 