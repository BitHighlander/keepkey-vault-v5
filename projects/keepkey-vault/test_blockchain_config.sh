#!/bin/bash

# Test script for blockchain configuration validation
# This validates that the new blockchains.json configuration is working

set -e

TAG="[BLOCKCHAIN CONFIG TEST]"

echo "$TAG Starting blockchain configuration validation..."

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

# Check if blockchains.json exists
BLOCKCHAIN_CONFIG="src-tauri/src/data/blockchains.json"

if [ ! -f "$BLOCKCHAIN_CONFIG" ]; then
    print_status "ERROR" "blockchains.json not found at $BLOCKCHAIN_CONFIG"
    exit 1
fi

print_status "SUCCESS" "Found blockchains.json configuration"

# Validate JSON syntax
if ! jq . "$BLOCKCHAIN_CONFIG" > /dev/null 2>&1; then
    print_status "ERROR" "Invalid JSON syntax in blockchains.json"
    exit 1
fi

print_status "SUCCESS" "JSON syntax is valid"

# Extract key metrics
TOTAL_BLOCKCHAINS=$(jq -r '.metadata.total_blockchains' "$BLOCKCHAIN_CONFIG")
EVM_CHAINS=$(jq -r '.metadata.evm_chains' "$BLOCKCHAIN_CONFIG")
ENABLED_COUNT=$(jq -r '[.blockchains[] | select(.enabled == true)] | length' "$BLOCKCHAIN_CONFIG")

print_status "INFO" "Configuration summary:"
echo "  📊 Total blockchains: $TOTAL_BLOCKCHAINS"
echo "  🌐 EVM chains: $EVM_CHAINS"
echo "  ✅ Enabled: $ENABLED_COUNT"

# Validate expected blockchains are present
EXPECTED_CHAINS=("bitcoin" "ethereum" "base" "arbitrum" "optimism" "polygon" "cosmos" "thorchain" "mayachain")
MISSING_CHAINS=()

for chain in "${EXPECTED_CHAINS[@]}"; do
    if ! jq -e ".blockchains[] | select(.id == \"$chain\")" "$BLOCKCHAIN_CONFIG" > /dev/null; then
        MISSING_CHAINS+=("$chain")
    fi
done

if [ ${#MISSING_CHAINS[@]} -eq 0 ]; then
    print_status "SUCCESS" "All expected blockchains are present"
else
    print_status "WARNING" "Missing blockchains: ${MISSING_CHAINS[*]}"
fi

# Check that BASE is configured correctly
BASE_CONFIG=$(jq -r '.blockchains[] | select(.id == "base")' "$BLOCKCHAIN_CONFIG")
if [ "$BASE_CONFIG" != "null" ]; then
    BASE_NETWORK_ID=$(echo "$BASE_CONFIG" | jq -r '.network_id')
    BASE_ENABLED=$(echo "$BASE_CONFIG" | jq -r '.enabled')
    
    if [ "$BASE_NETWORK_ID" = "eip155:8453" ] && [ "$BASE_ENABLED" = "true" ]; then
        print_status "SUCCESS" "BASE configuration is correct (eip155:8453, enabled: $BASE_ENABLED)"
    else
        print_status "ERROR" "BASE configuration is incorrect (network_id: $BASE_NETWORK_ID, enabled: $BASE_ENABLED)"
    fi
else
    print_status "ERROR" "BASE blockchain not found in configuration"
fi

# Check EVM chains have correct type and slip44
print_status "INFO" "Validating EVM chain configurations..."
EVM_VALIDATION_ERRORS=0

while IFS= read -r chain_config; do
    CHAIN_ID=$(echo "$chain_config" | jq -r '.id')
    CHAIN_TYPE=$(echo "$chain_config" | jq -r '.type')
    SLIP44=$(echo "$chain_config" | jq -r '.slip44')
    NETWORK_ID=$(echo "$chain_config" | jq -r '.network_id')
    
    if [ "$CHAIN_TYPE" = "evm" ]; then
        if [ "$SLIP44" = "60" ]; then
            print_status "SUCCESS" "  $CHAIN_ID: type=evm, slip44=60 ✓"
        else
            print_status "ERROR" "  $CHAIN_ID: wrong slip44 ($SLIP44, expected 60)"
            ((EVM_VALIDATION_ERRORS++))
        fi
        
        if [[ "$NETWORK_ID" == eip155:* ]]; then
            print_status "SUCCESS" "  $CHAIN_ID: network_id format correct ($NETWORK_ID)"
        else
            print_status "ERROR" "  $CHAIN_ID: wrong network_id format ($NETWORK_ID, expected eip155:*)"
            ((EVM_VALIDATION_ERRORS++))
        fi
    fi
done < <(jq -c '.blockchains[]' "$BLOCKCHAIN_CONFIG")

if [ $EVM_VALIDATION_ERRORS -eq 0 ]; then
    print_status "SUCCESS" "All EVM chain configurations are valid"
else
    print_status "ERROR" "$EVM_VALIDATION_ERRORS EVM configuration errors found"
fi

# Test that configuration matches integration-coins approach
print_status "INFO" "Checking compatibility with integration-coins approach..."

# These are the chains from integration-coins AllChainsSupported
INTEGRATION_CHAINS=("ETH" "DOGE" "OP" "MATIC" "AVAX" "BASE" "BSC" "BTC" "BCH" "GAIA" "OSMO" "XRP" "DASH" "MAYA" "LTC" "THOR")
MAPPED_CHAINS=()

for ic_chain in "${INTEGRATION_CHAINS[@]}"; do
    case $ic_chain in
        "ETH") MAPPED_CHAINS+=("ethereum") ;;
        "DOGE") MAPPED_CHAINS+=("dogecoin") ;;
        "OP") MAPPED_CHAINS+=("optimism") ;;
        "MATIC") MAPPED_CHAINS+=("polygon") ;;
        "AVAX") MAPPED_CHAINS+=("avalanche") ;;
        "BASE") MAPPED_CHAINS+=("base") ;;
        "BSC") MAPPED_CHAINS+=("bsc") ;;
        "BTC") MAPPED_CHAINS+=("bitcoin") ;;
        "BCH") MAPPED_CHAINS+=("bitcoincash") ;;
        "GAIA") MAPPED_CHAINS+=("cosmos") ;;
        "OSMO") MAPPED_CHAINS+=("osmosis") ;;
        "XRP") MAPPED_CHAINS+=("ripple") ;;
        "DASH") MAPPED_CHAINS+=("dash") ;;
        "MAYA") MAPPED_CHAINS+=("mayachain") ;;
        "LTC") MAPPED_CHAINS+=("litecoin") ;;
        "THOR") MAPPED_CHAINS+=("thorchain") ;;
    esac
done

COMPATIBILITY_ERRORS=0
for mapped_chain in "${MAPPED_CHAINS[@]}"; do
    if jq -e ".blockchains[] | select(.id == \"$mapped_chain\")" "$BLOCKCHAIN_CONFIG" > /dev/null; then
        print_status "SUCCESS" "  $mapped_chain: present in configuration ✓"
    else
        print_status "ERROR" "  $mapped_chain: missing from configuration"
        ((COMPATIBILITY_ERRORS++))
    fi
done

if [ $COMPATIBILITY_ERRORS -eq 0 ]; then
    print_status "SUCCESS" "Configuration is compatible with integration-coins approach"
else
    print_status "ERROR" "$COMPATIBILITY_ERRORS compatibility issues found"
fi

echo ""
print_status "INFO" "Blockchain configuration validation completed"

# Summary
if [ $EVM_VALIDATION_ERRORS -eq 0 ] && [ $COMPATIBILITY_ERRORS -eq 0 ] && [ ${#MISSING_CHAINS[@]} -eq 0 ]; then
    print_status "SUCCESS" "All validation checks passed! 🎉"
    print_status "INFO" "The vault should now properly handle BASE and other EVM chains"
    echo ""
    echo "Next steps:"
    echo "  1. Restart the vault to load new configuration"
    echo "  2. Trigger frontload to test multi-EVM balance fetching"
    echo "  3. Check that BASE balances appear in portfolio"
    exit 0
else
    print_status "ERROR" "Some validation checks failed"
    exit 1
fi 