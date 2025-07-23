#!/bin/bash

echo "🔍 Portfolio Troubleshooting Script"
echo "=================================="

# Check if server is running
echo -e "\n1. Checking if server is running on port 1646..."
if lsof -i :1646 > /dev/null 2>&1; then
    echo "✅ Server is running on port 1646"
else
    echo "❌ Server is NOT running on port 1646"
    echo "   Please start the application first"
    exit 1
fi

# Check environment variables
echo -e "\n2. Checking environment variables..."
if [ -z "$PIONEER_API_KEY" ]; then
    echo "⚠️  PIONEER_API_KEY is not set"
    echo "   This is required for portfolio data fetching"
    echo "   Set it with: export PIONEER_API_KEY='your-api-key'"
else
    echo "✅ PIONEER_API_KEY is set"
fi

# Test portfolio endpoints
echo -e "\n3. Testing portfolio endpoints..."

# Test combined portfolio
echo -e "\n   Testing /api/portfolio..."
RESPONSE=$(curl -s http://localhost:1646/api/portfolio)
if [ $? -eq 0 ]; then
    echo "   Response: $RESPONSE" | head -c 200
    if echo "$RESPONSE" | grep -q '"success":true'; then
        echo -e "\n   ✅ Endpoint responds successfully"
        if echo "$RESPONSE" | grep -q '"balances":\[\]'; then
            echo "   ⚠️  But balances are empty"
        fi
    else
        echo -e "\n   ❌ Endpoint returned error"
    fi
else
    echo "   ❌ Failed to connect to endpoint"
fi

# Test with refresh parameter
echo -e "\n   Testing /api/portfolio?refresh=true..."
RESPONSE=$(curl -s "http://localhost:1646/api/portfolio?refresh=true")
if [ $? -eq 0 ]; then
    echo "   Response: $RESPONSE" | head -c 200
    if echo "$RESPONSE" | grep -q '"balances":\[\]'; then
        echo -e "\n   ⚠️  Still empty after refresh attempt"
    fi
else
    echo "   ❌ Failed to connect to endpoint"
fi

# Check database
echo -e "\n4. Checking database state..."
DB_PATH="$HOME/.config/keepkey-vault/cache.db"
if [ -f "$DB_PATH" ]; then
    echo "✅ Database exists at: $DB_PATH"
    
    # Check xpubs table
    echo -e "\n   Checking wallet_xpubs table..."
    XPUB_COUNT=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM wallet_xpubs;" 2>/dev/null || echo "error")
    if [ "$XPUB_COUNT" = "error" ]; then
        echo "   ❌ Failed to query wallet_xpubs table"
    else
        echo "   📊 XPubs in database: $XPUB_COUNT"
        if [ "$XPUB_COUNT" -eq "0" ]; then
            echo "   ⚠️  No XPubs found - this is why portfolio is empty!"
            echo "   🔧 Solution: Connect your KeepKey device to populate XPubs"
        fi
    fi
    
    # Check portfolio_balances table
    echo -e "\n   Checking portfolio_balances table..."
    BALANCE_COUNT=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM portfolio_balances;" 2>/dev/null || echo "error")
    if [ "$BALANCE_COUNT" = "error" ]; then
        echo "   ❌ Failed to query portfolio_balances table"
    else
        echo "   📊 Portfolio balances in database: $BALANCE_COUNT"
    fi
else
    echo "❌ Database not found at expected location"
fi

echo -e "\n=================================="
echo "📋 Summary & Recommendations:"
echo "=================================="

if [ -z "$PIONEER_API_KEY" ]; then
    echo "1. Set PIONEER_API_KEY environment variable"
fi

if [ "$XPUB_COUNT" = "0" ] || [ "$XPUB_COUNT" = "error" ]; then
    echo "2. Connect your KeepKey device to populate XPubs"
    echo "3. Check device communication logs for errors"
fi

echo "4. Enable debug logging: export RUST_LOG=vault_v2=debug"
echo "5. Restart the application after setting environment variables"

echo -e "\n✨ Quick fix command:"
echo "export PIONEER_API_KEY='your-api-key' && export RUST_LOG=vault_v2=debug"