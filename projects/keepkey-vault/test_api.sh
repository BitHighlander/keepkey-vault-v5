#!/bin/bash

# KeepKey Vault REST API Test Script
# This script tests all the main API endpoints to verify they're working correctly

BASE_URL="http://localhost:1646"
VERBOSE=false

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_header() {
    echo -e "${BLUE}=================================${NC}"
    echo -e "${BLUE}KeepKey Vault API Test Suite${NC}"
    echo -e "${BLUE}=================================${NC}"
    echo "Base URL: $BASE_URL"
    echo ""
}

print_test() {
    echo -e "${YELLOW}Testing: $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

test_endpoint() {
    local method=$1
    local endpoint=$2
    local data=$3
    local description=$4
    
    print_test "$description"
    
    if [ "$method" = "GET" ]; then
        response=$(curl -s -w "%{http_code}" -H "Content-Type: application/json" "$BASE_URL$endpoint")
    else
        if [ -n "$data" ]; then
            response=$(curl -s -w "%{http_code}" -X "$method" -H "Content-Type: application/json" -d "$data" "$BASE_URL$endpoint")
        else
            response=$(curl -s -w "%{http_code}" -X "$method" -H "Content-Type: application/json" "$BASE_URL$endpoint")
        fi
    fi
    
    http_code="${response: -3}"
    body="${response%???}"
    
    if [ "$http_code" -eq 200 ] || [ "$http_code" -eq 201 ]; then
        print_success "$description - Status: $http_code"
        if [ "$VERBOSE" = true ]; then
            echo "Response: $body" | jq '.' 2>/dev/null || echo "Response: $body"
        fi
    elif [ "$http_code" -eq 404 ]; then
        print_warning "$description - Status: $http_code (endpoint may not be implemented)"
    else
        print_error "$description - Status: $http_code"
        if [ "$VERBOSE" = true ]; then
            echo "Response: $body"
        fi
    fi
    echo ""
}

# Check if server is running
check_server() {
    print_test "Server connectivity"
    if curl -s -f "$BASE_URL/api/health" > /dev/null; then
        print_success "Server is running"
    else
        print_error "Server is not responding at $BASE_URL"
        echo "Make sure the KeepKey Vault is running and the REST API is enabled"
        exit 1
    fi
    echo ""
}

# Main test suite
run_tests() {
    print_header
    check_server
    
    echo -e "${BLUE}=== System Endpoints ===${NC}"
    test_endpoint "GET" "/api/health" "" "Health Check"
    test_endpoint "GET" "/api/devices" "" "List Devices"
    test_endpoint "POST" "/system/info/get-features" '{}' "Get Device Features"
    
    echo -e "${BLUE}=== Authentication Endpoints ===${NC}"
    test_endpoint "GET" "/auth/pair" "" "Auth Verify"
    test_endpoint "POST" "/auth/pair" '{"name":"Test App","url":"http://localhost","image_url":"","added_on":null}' "Auth Pair"
    
    echo -e "${BLUE}=== Address Generation Endpoints ===${NC}"
    test_endpoint "POST" "/addresses/utxo" '{"address_n":[2147483692,2147483648,2147483648,0,0],"coin":"Bitcoin","script_type":"p2pkh","show_display":false}' "Bitcoin Address"
    test_endpoint "POST" "/addresses/eth" '{"address_n":[2147483692,2147483708,2147483648,0,0],"show_display":false}' "Ethereum Address"
    test_endpoint "POST" "/addresses/cosmos" '{"address_n":[2147483692,2147483766,2147483648,0,0],"show_display":false}' "Cosmos Address"
    
    echo -e "${BLUE}=== System Operations Endpoints ===${NC}"
    test_endpoint "POST" "/system/ping" '{"message":"Test ping","button_protection":false}' "System Ping"
    test_endpoint "POST" "/system/info/get-entropy" '{"size":32}' "Get Entropy"
    test_endpoint "POST" "/system/info/get-public-key" '{"address_n":[2147483692,2147483648,2147483648],"ecdsa_curve_name":"secp256k1","show_display":false}' "Get Public Key"
    
    echo -e "${BLUE}=== API Documentation ===${NC}"
    test_endpoint "GET" "/docs" "" "Swagger UI"
    test_endpoint "GET" "/api-docs/openapi.json" "" "OpenAPI Spec"
    
    echo -e "${BLUE}=== Summary ===${NC}"
    echo "Test complete! Check the results above."
    echo "If any critical endpoints are failing, check:"
    echo "1. Device is connected and unlocked"
    echo "2. REST API server is running correctly"
    echo "3. CORS settings are configured properly"
    echo ""
    echo "For verbose output with response bodies, run: $0 -v"
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [-v|--verbose] [-h|--help]"
            echo "  -v, --verbose    Show response bodies"
            echo "  -h, --help      Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use -h for help"
            exit 1
            ;;
    esac
done

# Check dependencies
if ! command -v curl &> /dev/null; then
    print_error "curl is required but not installed"
    exit 1
fi

if ! command -v jq &> /dev/null; then
    print_warning "jq is not installed - JSON responses won't be formatted"
fi

# Run the tests
run_tests 