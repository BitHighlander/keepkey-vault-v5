#!/bin/bash

# KeepKey Vault Proxy Server Test Script
# This script tests the proxy server functionality to verify it's working correctly

API_URL="http://localhost:1646"
PROXY_URL="http://localhost:8080"
VAULT_URL="https://vault.keepkey.com"
VERBOSE=false

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

print_header() {
    echo -e "${BLUE}=================================${NC}"
    echo -e "${BLUE}KeepKey Vault Proxy Test Suite${NC}"
    echo -e "${BLUE}=================================${NC}"
    echo "API URL: $API_URL"
    echo "Proxy URL: $PROXY_URL"
    echo "Target URL: $VAULT_URL"
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

print_info() {
    echo -e "${PURPLE}ℹ️  $1${NC}"
}

# Test basic connectivity
test_connectivity() {
    echo -e "${BLUE}=== Connectivity Tests ===${NC}"
    
    print_test "API Server connectivity"
    if curl -s -f "$API_URL/api/health" > /dev/null; then
        print_success "API Server is running at $API_URL"
    else
        print_error "API Server is not responding at $API_URL"
        echo "Make sure the KeepKey Vault is running"
        exit 1
    fi
    
    print_test "Proxy Server connectivity"
    if curl -s -f "$PROXY_URL/" > /dev/null; then
        print_success "Proxy Server is running at $PROXY_URL"
    else
        print_error "Proxy Server is not responding at $PROXY_URL"
        echo "The proxy server should start automatically with the API server"
        exit 1
    fi
    
    print_test "External connectivity to vault.keepkey.com"
    if curl -s -f "$VAULT_URL" > /dev/null; then
        print_success "Can reach vault.keepkey.com directly"
    else
        print_warning "Cannot reach vault.keepkey.com directly (may be network/firewall issue)"
    fi
    
    echo ""
}

# Test proxy functionality
test_proxy_basic() {
    echo -e "${BLUE}=== Proxy Basic Functionality ===${NC}"
    
    print_test "Proxy root request"
    response=$(curl -s -w "%{http_code}" "$PROXY_URL/")
    http_code="${response: -3}"
    body="${response%???}"
    
    if [ "$http_code" -eq 200 ]; then
        print_success "Proxy root request - Status: $http_code"
        
        # Check if it's proxying vault.keepkey.com content
        if echo "$body" | grep -q "KeepKey"; then
            print_success "Response contains KeepKey content (proxying correctly)"
        else
            print_warning "Response doesn't contain expected KeepKey content"
        fi
        
        # Check for proxy headers
        headers=$(curl -s -I "$PROXY_URL/")
        if echo "$headers" | grep -q "x-proxy-by: keepkey-vault"; then
            print_success "Proxy headers present"
        else
            print_warning "Proxy headers missing"
        fi
        
    else
        print_error "Proxy root request failed - Status: $http_code"
        if [ "$VERBOSE" = true ]; then
            echo "Response: $body"
        fi
    fi
    echo ""
}

# Test CORS headers
test_cors() {
    echo -e "${BLUE}=== CORS Configuration ===${NC}"
    
    print_test "CORS headers"
    headers=$(curl -s -I "$PROXY_URL/")
    
    if echo "$headers" | grep -q "access-control-allow-origin: \*"; then
        print_success "CORS allow-origin header present"
    else
        print_error "CORS allow-origin header missing or incorrect"
    fi
    
    if echo "$headers" | grep -q "access-control-allow-methods"; then
        print_success "CORS allow-methods header present"
    else
        print_error "CORS allow-methods header missing"
    fi
    
    if echo "$headers" | grep -q "access-control-allow-headers"; then
        print_success "CORS allow-headers header present"
    else
        print_error "CORS allow-headers header missing"
    fi
    
    print_test "CORS preflight request"
    preflight_response=$(curl -s -w "%{http_code}" -X OPTIONS -H "Origin: http://localhost:3000" -H "Access-Control-Request-Method: GET" "$PROXY_URL/")
    preflight_code="${preflight_response: -3}"
    
    if [ "$preflight_code" -eq 200 ]; then
        print_success "CORS preflight request successful - Status: $preflight_code"
    else
        print_warning "CORS preflight request - Status: $preflight_code"
    fi
    
    echo ""
}

# Test URL rewriting
test_url_rewriting() {
    echo -e "${BLUE}=== URL Rewriting ===${NC}"
    
    print_test "HTML content URL rewriting"
    response=$(curl -s "$PROXY_URL/")
    
    # Check for base tag insertion
    if echo "$response" | grep -q '<base href="http://localhost:8080/'; then
        print_success "Base tag injection working"
    else
        print_warning "Base tag injection not detected"
    fi
    
    # Check for URL rewriting to localhost:8080
    if echo "$response" | grep -q "localhost:8080"; then
        print_success "URL rewriting to localhost:8080 detected"
    else
        print_warning "URL rewriting not detected in content"
    fi
    
    # Check for proxy metadata
    if echo "$response" | grep -q 'proxy-rewritten.*keepkey-vault'; then
        print_success "Proxy rewriting metadata present"
    else
        print_warning "Proxy rewriting metadata missing"
    fi
    
    echo ""
}

# Test different HTTP methods
test_http_methods() {
    echo -e "${BLUE}=== HTTP Methods ===${NC}"
    
    methods=("GET" "POST" "PUT" "DELETE" "PATCH" "OPTIONS" "HEAD")
    
    for method in "${methods[@]}"; do
        print_test "$method request"
        
        if [ "$method" = "HEAD" ]; then
            response=$(curl -s -w "%{http_code}" -I -X "$method" "$PROXY_URL/")
        elif [ "$method" = "POST" ] || [ "$method" = "PUT" ] || [ "$method" = "PATCH" ]; then
            response=$(curl -s -w "%{http_code}" -X "$method" -H "Content-Type: application/json" -d '{}' "$PROXY_URL/api/test" 2>/dev/null || echo "000")
        else
            response=$(curl -s -w "%{http_code}" -X "$method" "$PROXY_URL/" 2>/dev/null || echo "000")
        fi
        
        http_code="${response: -3}"
        
        if [ "$http_code" -ge 200 ] && [ "$http_code" -lt 500 ]; then
            print_success "$method request - Status: $http_code"
        else
            print_warning "$method request - Status: $http_code (may be expected for some methods)"
        fi
    done
    
    echo ""
}

# Test proxy error handling
test_error_handling() {
    echo -e "${BLUE}=== Error Handling ===${NC}"
    
    print_test "Invalid path handling"
    response=$(curl -s -w "%{http_code}" "$PROXY_URL/nonexistent/path/that/should/404")
    http_code="${response: -3}"
    body="${response%???}"
    
    if [ "$http_code" -eq 404 ]; then
        print_success "404 handling - Status: $http_code"
    else
        print_info "Non-404 response for invalid path - Status: $http_code"
    fi
    
    print_test "Proxy error response format"
    if echo "$body" | grep -q '"proxy".*"keepkey-vault"'; then
        print_success "Proxy error response contains proper metadata"
    else
        print_warning "Proxy error response format may need improvement"
    fi
    
    echo ""
}

# Test subdomain handling
test_subdomain_handling() {
    echo -e "${BLUE}=== Subdomain Handling ===${NC}"
    
    print_test "Vault subdomain routing"
    response=$(curl -s -w "%{http_code}" -H "Host: vault.keepkey.com" "$PROXY_URL/")
    http_code="${response: -3}"
    
    if [ "$http_code" -eq 200 ]; then
        print_success "Vault subdomain routing - Status: $http_code"
    else
        print_warning "Vault subdomain routing - Status: $http_code"
    fi
    
    print_test "Custom subdomain header"
    response=$(curl -s -w "%{http_code}" -H "x-keepkey-subdomain: app" "$PROXY_URL/")
    http_code="${response: -3}"
    
    if [ "$http_code" -eq 200 ]; then
        print_success "Custom subdomain header routing - Status: $http_code"
    else
        print_warning "Custom subdomain header routing - Status: $http_code"
    fi
    
    echo ""
}

# Performance test
test_performance() {
    echo -e "${BLUE}=== Performance Test ===${NC}"
    
    print_test "Response time measurement"
    start_time=$(date +%s%N)
    response=$(curl -s -w "%{http_code}" "$PROXY_URL/")
    end_time=$(date +%s%N)
    
    duration=$(( (end_time - start_time) / 1000000 )) # Convert to milliseconds
    http_code="${response: -3}"
    
    if [ "$http_code" -eq 200 ]; then
        if [ "$duration" -lt 1000 ]; then
            print_success "Fast response time: ${duration}ms"
        elif [ "$duration" -lt 3000 ]; then
            print_success "Acceptable response time: ${duration}ms"
        else
            print_warning "Slow response time: ${duration}ms"
        fi
    else
        print_error "Performance test failed - Status: $http_code"
    fi
    
    echo ""
}

# Server readiness test
test_server_readiness() {
    echo -e "${BLUE}=== Server Readiness ===${NC}"
    
    print_test "Server readiness endpoint"
    response=$(curl -s -w "%{http_code}" "$API_URL/api/health")
    http_code="${response: -3}"
    body="${response%???}"
    
    if [ "$http_code" -eq 200 ]; then
        print_success "API health check - Status: $http_code"
        
        if echo "$body" | grep -q '"status".*"ready"'; then
            print_success "Server reports ready status"
        else
            print_warning "Server status unclear from health check"
        fi
    else
        print_error "API health check failed - Status: $http_code"
    fi
    
    echo ""
}

# Main test suite
run_tests() {
    print_header
    
    test_connectivity
    test_server_readiness
    test_proxy_basic
    test_cors
    test_url_rewriting
    test_http_methods
    test_subdomain_handling
    test_error_handling
    test_performance
    
    echo -e "${BLUE}=== Summary ===${NC}"
    echo "Proxy test suite complete!"
    echo ""
    echo "If any tests are failing, check:"
    echo "1. Both API (1646) and Proxy (8080) servers are running"
    echo "2. Network connectivity to vault.keepkey.com"
    echo "3. CORS configuration is correct"
    echo "4. URL rewriting is functioning properly"
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
            echo "  -v, --verbose    Show response bodies and detailed output"
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

# Run the tests
run_tests 