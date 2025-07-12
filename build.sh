#!/bin/bash

# KeepKey Vault v5 Build Script
# This script builds the Tauri app for the current platform

set -e

echo "🚀 Building KeepKey Vault v5..."

# Navigate to the vault directory
cd projects/keepkey-vault

# Install dependencies if needed
if [ ! -d "node_modules" ]; then
    echo "📦 Installing dependencies..."
    bun install
fi

# Build the app
echo "🔨 Building Tauri app..."
if [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS - build for both architectures
    echo "🍎 Building for macOS..."
    bun tauri build --target aarch64-apple-darwin
    bun tauri build --target x86_64-apple-darwin
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    # Linux
    echo "🐧 Building for Linux..."
    bun tauri build
elif [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "cygwin" ]] || [[ "$OSTYPE" == "win32" ]]; then
    # Windows
    echo "🪟 Building for Windows..."
    bun tauri build
else
    echo "❌ Unsupported OS: $OSTYPE"
    exit 1
fi

echo "✅ Build complete! Check src-tauri/target/release/bundle/ for the output." 