#!/bin/bash

echo "🚀 Starting KeepKey Vault development environment..."

# Kill any existing processes on our ports
echo "🧹 Cleaning up existing processes..."
lsof -ti:1420 | xargs kill -9 2>/dev/null || true

# Start Vite in the background
echo "📦 Starting Vite dev server..."
cd projects/keepkey-vault
bun run dev &
VITE_PID=$!

# Wait for Vite to be ready
echo "⏳ Waiting for Vite dev server to be ready..."
for i in {1..30}; do
  if curl -s http://localhost:1420 > /dev/null 2>&1; then
    echo "✅ Vite dev server is ready!"
    break
  fi
  if [ $i -eq 30 ]; then
    echo "❌ Vite dev server failed to start after 30 seconds"
    kill $VITE_PID 2>/dev/null || true
    exit 1
  fi
  sleep 1
done

# Start Tauri
echo "🦀 Starting Tauri application..."
bun tauri dev

# Cleanup on exit
cleanup() {
  echo "🧹 Cleaning up processes..."
  kill $VITE_PID 2>/dev/null || true
  lsof -ti:1420 | xargs kill -9 2>/dev/null || true
}
trap cleanup EXIT 