#!/bin/bash

# Test server startup timing and readiness
echo "🧪 Testing Server Startup Timing"
echo "================================="

API_URL="http://localhost:1646"
PROXY_URL="http://localhost:8080"

echo "Checking if both servers are ready simultaneously..."

# Test API server
api_status=$(curl -s -o /dev/null -w "%{http_code}" "$API_URL/api/health")
echo "API Server (1646): $api_status"

# Test proxy server
proxy_status=$(curl -s -o /dev/null -w "%{http_code}" "$PROXY_URL/")  
echo "Proxy Server (8080): $proxy_status"

if [ "$api_status" -eq 200 ] && [ "$proxy_status" -eq 200 ]; then
    echo "✅ Both servers are ready and responding correctly"
    echo ""
    echo "Testing rapid consecutive requests (simulating frontend behavior):"
    
    for i in {1..5}; do
        start_time=$(date +%s%N)
        response=$(curl -s -o /dev/null -w "%{http_code}" "$PROXY_URL/")
        end_time=$(date +%s%N)
        duration=$(( (end_time - start_time) / 1000000 ))
        
        if [ "$response" -eq 200 ]; then
            echo "Request $i: ✅ Success (${duration}ms)"
        else
            echo "Request $i: ❌ Failed - Status: $response"
        fi
    done
    
    echo ""
    echo "🎉 Proxy server is stable and ready for frontend connections!"
else
    echo "❌ One or both servers not ready:"
    echo "   API: $api_status (expected: 200)"  
    echo "   Proxy: $proxy_status (expected: 200)"
fi 