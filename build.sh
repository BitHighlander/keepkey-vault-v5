#!/bin/bash

# KeepKey Vault v5 Build Script
# This script builds the Tauri app for the current platform

set -e

echo "🚀 Building KeepKey Vault v5..."

# Validate notarization requirements for macOS
if [[ "$OSTYPE" == "darwin"* ]]; then
    echo "🔍 Validating macOS notarization requirements..."
    
    # Source environment variables from .env file if it exists (for local development)
    if [ -f ".env" ]; then
        echo "📄 Loading environment variables from .env file..."
        source .env
    else
        echo "📄 No .env file found, using environment variables..."
    fi
    
    # Check required environment variables
    if [ -z "$APPLE_ID" ]; then
        echo "❌ ERROR: APPLE_ID environment variable is required for notarization"
        echo "   Please set APPLE_ID in .env file (local) or GitHub Secrets (CI)"
        exit 1
    fi
    
    if [ -z "$APPLE_PASSWORD" ]; then
        echo "❌ ERROR: APPLE_PASSWORD environment variable is required for notarization"
        echo "   Please set APPLE_PASSWORD in .env file (local) or GitHub Secrets (CI)"
        echo "   Use app-specific password from Apple ID settings"
        exit 1
    fi
    
    if [ -z "$APPLE_TEAM_ID" ]; then
        echo "❌ ERROR: APPLE_TEAM_ID environment variable is required for notarization"
        echo "   Please set APPLE_TEAM_ID in .env file (local) or GitHub Secrets (CI)"
        exit 1
    fi
    
    # CRITICAL: Export the variables so they're available to child processes
    export APPLE_ID
    export APPLE_PASSWORD
    export APPLE_TEAM_ID
    
    echo "✅ Notarization requirements validated and exported"
    echo "   APPLE_ID: $APPLE_ID"
    echo "   APPLE_PASSWORD: [${#APPLE_PASSWORD} characters]"
    echo "   APPLE_TEAM_ID: $APPLE_TEAM_ID"
fi

# Clean up old build artifacts
echo "🧹 Cleaning up old build artifacts..."
cd projects/keepkey-vault

# Only clean if target directory exists
if [ -d "target/release/bundle" ]; then
    # Remove old build artifacts
    rm -rf target/release/bundle/dmg/rw.*.dmg 2>/dev/null || true
    rm -rf target/release/bundle/macos/rw.*.dmg 2>/dev/null || true
    rm -rf target/release/bundle/deb/rw.*.deb 2>/dev/null || true
    rm -rf target/release/bundle/appimage/rw.*.AppImage 2>/dev/null || true
    rm -rf target/release/bundle/msi/rw.*.msi 2>/dev/null || true
    rm -rf target/release/bundle/nsis/rw.*.exe 2>/dev/null || true

    # Remove old final build outputs
    rm -rf target/release/bundle/macos/*.dmg 2>/dev/null || true
    rm -rf target/release/bundle/macos/*.app 2>/dev/null || true
    rm -rf target/release/bundle/deb/*.deb 2>/dev/null || true
    rm -rf target/release/bundle/appimage/*.AppImage 2>/dev/null || true
    rm -rf target/release/bundle/msi/*.msi 2>/dev/null || true
    rm -rf target/release/bundle/nsis/*.exe 2>/dev/null || true

    # Clean up any .DS_Store files
    find target/release/bundle -name ".DS_Store" -delete 2>/dev/null || true
fi

echo "✅ Cleanup complete"

# Navigate to the vault directory
cd ../../projects/keepkey-vault

# Install dependencies if needed
if [ ! -d "node_modules" ]; then
    echo "📦 Installing dependencies..."
    bun install
fi

# Build the app
echo "🔨 Building Tauri app..."
if [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS - build with notarization (environment already validated and exported)
    echo "🍎 Building for macOS with notarization..."
    echo "🔍 Verifying environment variables are exported:"
    echo "   APPLE_ID: ${APPLE_ID:-NOT_SET}"
    echo "   APPLE_PASSWORD: ${APPLE_PASSWORD:+SET} ${APPLE_PASSWORD:-NOT_SET}"
    echo "   APPLE_TEAM_ID: ${APPLE_TEAM_ID:-NOT_SET}"
    
    # Double-check that variables are exported
    bun tauri build --target universal-apple-darwin
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

# Verify notarization succeeded on macOS
if [[ "$OSTYPE" == "darwin"* ]]; then
    echo "🔍 Verifying build results..."
    
    APP_PATH="target/release/bundle/macos/KeepKey Vault.app"
    if [ -d "$APP_PATH" ]; then
        echo "✅ App bundle created: $APP_PATH"
        
        # Check if app passes Gatekeeper
        if spctl -a -v "$APP_PATH" 2>&1 | grep -q "accepted"; then
            echo "✅ App passes Gatekeeper validation"
            
            # Check if notarized
            if spctl -a -v "$APP_PATH" 2>&1 | grep -q "Notarized Developer ID"; then
                echo "✅ App is properly notarized"
            else
                echo "❌ ERROR: App is signed but not notarized"
                echo "   This indicates the notarization process failed during build"
                echo "   Check the build output above for notarization errors"
                exit 1
            fi
        else
            echo "❌ ERROR: App failed Gatekeeper validation"
            spctl -a -v "$APP_PATH"
            exit 1
        fi
    else
        echo "❌ ERROR: App bundle not found at $APP_PATH"
        exit 1
    fi
fi

echo "✅ Build complete! Check target/release/bundle/ for the output." 