#!/bin/bash

echo "🔍 Testing KeepKey Device Connection..."
echo ""

# Test 1: Check API health
echo "1. Testing API health..."
curl -s http://127.0.0.1:1646/api/health | jq .
echo ""

# Test 2: List connected devices
echo "2. Listing connected devices..."
curl -s http://127.0.0.1:1646/api/devices | jq .
echo ""

# Test 3: Ping the device
echo "3. Sending ping to device..."
curl -s -X POST http://127.0.0.1:1646/system/ping \
  -H "Content-Type: application/json" \
  -d '{
    "device_id": "343737340F4736331F003B00",
    "message": "Hello KeepKey!",
    "button_protection": false
  }' | jq .
echo ""

echo "✅ Tests completed!" 