#!/bin/bash

# Post-build script to notarize DMG
# This ensures users never see "can't be opened" errors

set -e

echo "🔖 Post-build: Notarizing DMG..."

# Check if we're on macOS
if [[ "$OSTYPE" != "darwin"* ]]; then
    echo "⚠️  Skipping DMG notarization - not on macOS"
    exit 0
fi

# Check required environment variables
if [ -z "$APPLE_ID" ] || [ -z "$APPLE_PASSWORD" ] || [ -z "$APPLE_TEAM_ID" ]; then
    echo "⚠️  Skipping DMG notarization - missing credentials"
    echo "   Set APPLE_ID, APPLE_PASSWORD, and APPLE_TEAM_ID to enable"
    exit 0
fi

DMG_PATH="target/universal-apple-darwin/release/bundle/dmg/KeepKey Vault_0.1.0_universal.dmg"

if [ ! -f "$DMG_PATH" ]; then
    echo "❌ DMG not found at $DMG_PATH"
    exit 1
fi

echo "📦 Notarizing DMG: $DMG_PATH"

# Submit DMG for notarization
echo "⏳ Submitting DMG for notarization..."
if xcrun notarytool submit "$DMG_PATH" \
    --apple-id "$APPLE_ID" \
    --password "$APPLE_PASSWORD" \
    --team-id "$APPLE_TEAM_ID" \
    --wait; then
    
    echo "✅ DMG notarization successful"
    
    # Try to staple the DMG
    echo "🔖 Stapling DMG..."
    if xcrun stapler staple "$DMG_PATH"; then
        echo "✅ DMG successfully stapled"
    else
        echo "⚠️  DMG stapling failed, but notarization succeeded"
    fi
    
    # Verify the DMG
    if spctl -a -v --type install "$DMG_PATH" 2>&1 | grep -q "accepted"; then
        echo "✅ DMG passes installation validation"
        echo "🎯 Users will NOT see 'can't be opened' errors!"
    else
        echo "⚠️  DMG validation failed"
    fi
else
    echo "❌ DMG notarization failed"
    exit 1
fi

echo "✅ Post-build complete!" 