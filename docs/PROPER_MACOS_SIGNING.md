# Proper macOS Signing and Notarization for KeepKey Vault

## 🎯 **Why Proper Signing Matters**

Ad-hoc signing (what we currently have) works for development but causes Gatekeeper to reject the app for end users. Proper signing with a Developer ID Application certificate and notarization ensures:

- ✅ No "can't be opened" errors for users
- ✅ Trusted by macOS Gatekeeper
- ✅ Professional distribution
- ✅ App Store compatibility (if needed)

---

## 📋 **Prerequisites**

1. **Apple Developer Account** (paid $99/year)
2. **Xcode** installed
3. **Developer ID Application Certificate**

---

## 🔐 **Step 1: Get Developer ID Certificate**

### Option A: Through Xcode (Recommended)
1. Open **Xcode**
2. Go to **Xcode** → **Settings** → **Accounts**
3. Add your Apple ID if not already added
4. Select your team → **Manage Certificates**
5. Click **+** → **Developer ID Application**
6. The certificate will be automatically installed in your keychain

### Option B: Through Developer Portal
1. Go to [Apple Developer Portal](https://developer.apple.com/account/)
2. Navigate to **Certificates, IDs & Profiles**
3. Create a **Developer ID Application** certificate
4. Download and install it

### Verify Certificate Installation
```bash
security find-identity -v -p codesigning
```
You should see something like:
```
1) ABC123DEF456 "Developer ID Application: Your Name (TEAMID)"
```

---

## 🛠️ **Step 2: Update Build Configuration**

### Update Tauri Configuration
Edit `src-tauri/tauri.conf.json`:

```json
{
  "bundle": {
    "macOS": {
      "signingIdentity": "Developer ID Application: Your Name (TEAMID)",
      "hardenedRuntime": true,
      "entitlements": "./Entitlements.plist"
    }
  }
}
```

### Set Environment Variables
```bash
export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)"
export APPLE_TEAM_ID="TEAMID"
```

---

## 🔑 **Step 3: Set Up Notarization**

### Option A: App Store Connect API Key (Recommended)
1. Go to [App Store Connect](https://appstoreconnect.apple.com/)
2. Navigate to **Users and Access** → **Integrations** → **App Store Connect API**
3. Create a new API key with **Developer** access
4. Download the `.p8` key file

```bash
export APPLE_API_ISSUER="your-issuer-id"
export APPLE_API_KEY="your-key-id"
export APPLE_API_KEY_PATH="/path/to/AuthKey_KEYID.p8"
```

### Option B: Apple ID with App-Specific Password
1. Go to [Apple ID Account](https://appleid.apple.com/)
2. Sign in → **App-Specific Passwords**
3. Generate a new password for "KeepKey Vault Notarization"

```bash
export APPLE_ID="your-apple-id@example.com"
export APPLE_PASSWORD="your-app-specific-password"
export APPLE_TEAM_ID="TEAMID"
```

---

## 🚀 **Step 4: Updated Build Script**

I'll create an enhanced build script that handles proper signing:

```bash
#!/bin/bash
# Enhanced build script with proper signing

set -e

echo "🍎 Building KeepKey Vault with proper signing..."

# Check for Developer ID certificate
CERT_NAME=$(security find-identity -v -p codesigning | grep "Developer ID Application" | head -1 | sed 's/.*"\(.*\)"/\1/')

if [ -z "$CERT_NAME" ]; then
    echo "❌ No Developer ID Application certificate found!"
    echo "Please install one through Xcode → Settings → Accounts"
    exit 1
fi

echo "✅ Found certificate: $CERT_NAME"
export APPLE_SIGNING_IDENTITY="$CERT_NAME"

# Build with proper signing
bun run build
tauri build --target aarch64-apple-darwin
tauri build --target x86_64-apple-darwin

# Manual signing with deep verification
for TARGET in "aarch64-apple-darwin" "x86_64-apple-darwin"; do
    APP_PATH="target/$TARGET/release/bundle/macos/KeepKey Vault.app"
    
    if [ -f "$APP_PATH" ]; then
        echo "🔐 Signing $TARGET build..."
        
        # Sign with proper options
        codesign --force --options runtime --deep \
                 --sign "$APPLE_SIGNING_IDENTITY" \
                 "$APP_PATH"
        
        echo "✅ Signed successfully"
        
        # Verify signature
        codesign --verify --deep --strict "$APP_PATH"
        echo "✅ Signature verified"
        
        # Notarize if credentials are available
        if [ -n "$APPLE_API_ISSUER" ] && [ -n "$APPLE_API_KEY" ]; then
            echo "📤 Notarizing $TARGET build..."
            
            # Create temporary zip for notarization
            ZIP_PATH="target/$TARGET/release/bundle/KeepKey-Vault-$TARGET.zip"
            ditto -c -k --keepParent "$APP_PATH" "$ZIP_PATH"
            
            # Submit for notarization
            xcrun notarytool submit "$ZIP_PATH" \
                  --key-id "$APPLE_API_KEY" \
                  --issuer-id "$APPLE_API_ISSUER" \
                  --key "$APPLE_API_KEY_PATH" \
                  --wait
            
            # Staple the ticket
            xcrun stapler staple "$APP_PATH"
            echo "✅ Notarization complete"
            
            # Clean up zip
            rm "$ZIP_PATH"
            
        elif [ -n "$APPLE_ID" ] && [ -n "$APPLE_PASSWORD" ]; then
            echo "📤 Notarizing $TARGET build with Apple ID..."
            
            ZIP_PATH="target/$TARGET/release/bundle/KeepKey-Vault-$TARGET.zip"
            ditto -c -k --keepParent "$APP_PATH" "$ZIP_PATH"
            
            xcrun notarytool submit "$ZIP_PATH" \
                  --apple-id "$APPLE_ID" \
                  --team-id "$APPLE_TEAM_ID" \
                  --password "$APPLE_PASSWORD" \
                  --wait
            
            xcrun stapler staple "$APP_PATH"
            echo "✅ Notarization complete"
            
            rm "$ZIP_PATH"
        else
            echo "⚠️ No notarization credentials found"
            echo "App is signed but not notarized"
        fi
        
        # Final verification
        echo "🔍 Final verification..."
        spctl -a -vvv -t exec "$APP_PATH"
        
    fi
done

echo "🎉 Build complete with proper signing!"
```

---

## 🧪 **Step 5: Testing**

### Verify Proper Signing
```bash
# Check signature
codesign -dv --verbose=4 "KeepKey Vault.app"

# Verify entitlements
codesign -d --entitlements - "KeepKey Vault.app"

# Test Gatekeeper
spctl -a -vvv -t exec "KeepKey Vault.app"
```

**Expected output for notarized app:**
```
source=Notarized Developer ID
```

### Test Installation
1. Create DMG from signed app
2. Test on a different Mac
3. Verify no Gatekeeper warnings

---

## 🔧 **Step 6: GitHub Actions Setup**

Update `.github/workflows/build-macos-only.yml` with proper secrets:

### Required GitHub Secrets:
```
APPLE_CERTIFICATE              # Base64 encoded .p12 file
APPLE_CERTIFICATE_PASSWORD     # Certificate password
APPLE_SIGNING_IDENTITY         # Certificate name
APPLE_API_ISSUER              # App Store Connect API Issuer ID
APPLE_API_KEY                 # App Store Connect API Key ID
APPLE_API_KEY_PATH            # API Key file content (base64)
APPLE_TEAM_ID                 # Your team ID
```

### Export Certificate for CI:
```bash
# Export certificate from keychain
security export -t p12 \
    -f pkcs12 \
    -k ~/Library/Keychains/login.keychain-db \
    -P "your-export-password" \
    -o certificate.p12 \
    "Developer ID Application: Your Name (TEAMID)"

# Convert to base64 for GitHub secret
base64 -i certificate.p12 | pbcopy
```

---

## 🚨 **Troubleshooting**

### Certificate Issues
```bash
# List all certificates
security find-identity -v

# Check certificate validity
security find-certificate -c "Developer ID Application" -p

# Fix keychain access
security set-key-partition-list -S apple-tool:,apple: -s \
    -k "keychain-password" ~/Library/Keychains/login.keychain-db
```

### Notarization Issues
```bash
# Check notarization history
xcrun notarytool history --key-id "$APPLE_API_KEY" \
    --issuer-id "$APPLE_API_ISSUER" \
    --key "$APPLE_API_KEY_PATH"

# Get detailed submission info
xcrun notarytool info "submission-id" \
    --key-id "$APPLE_API_KEY" \
    --issuer-id "$APPLE_API_ISSUER" \
    --key "$APPLE_API_KEY_PATH"
```

### Gatekeeper Issues
```bash
# Reset Gatekeeper for testing
sudo spctl --master-disable  # Disable (for testing only)
sudo spctl --master-enable   # Re-enable

# Remove quarantine
xattr -d com.apple.quarantine "KeepKey Vault.app"
```

---

## 📚 **Resources**

- [Apple Code Signing Guide](https://developer.apple.com/library/archive/documentation/Security/Conceptual/CodeSigningGuide/)
- [Notarizing macOS Software](https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution)
- [Tauri macOS Signing Docs](https://tauri.app/distribute/sign/macos/)

---

## ✅ **Next Steps**

1. **Get Developer ID Certificate** through Xcode
2. **Set up notarization credentials** (API Key recommended)
3. **Update build configuration** with proper signing identity
4. **Test locally** with the enhanced build script
5. **Set up CI/CD** with GitHub secrets
6. **Distribute** with confidence! 🚀 