# GitHub Actions Setup for macOS Code Signing

## Overview

This document provides step-by-step instructions for setting up GitHub Actions to automatically build, sign, and notarize KeepKey Vault v5 for macOS.

## Prerequisites

1. **Apple Developer Account** (paid)
2. **Developer ID Application Certificate** in your keychain
3. **App-specific password** for Apple ID
4. **GitHub repository** with admin access

## Step 1: Prepare Certificate for GitHub Actions

### 1.1 Export Certificate as P12

```bash
# Find your certificate identity
security find-identity -v -p codesigning | grep "KeepKey"

# Export certificate (replace with your actual certificate name)
security export -k login.keychain -t identities -f pkcs12 -o keepkey-certificate.p12 "Developer ID Application: KeepKey LLC (DR57X8Z394)"
```

### 1.2 Encode Certificate to Base64

```bash
# Encode the P12 certificate to base64
base64 -i keepkey-certificate.p12 | pbcopy

# This copies the base64 string to your clipboard
# You'll paste this as MACOS_CERTIFICATE_BASE64 secret
```

### 1.3 Clean Up Local Certificate File

```bash
# Remove the P12 file after encoding (security best practice)
rm keepkey-certificate.p12
```

## Step 2: Create App-Specific Password

### 2.1 Generate Password

1. Go to [Apple ID Account Management](https://appleid.apple.com/account/manage)
2. Sign in with your Apple ID
3. Navigate to "Security" → "App-Specific Passwords"
4. Click "Generate Password"
5. Enter label: "KeepKey Vault GitHub Actions"
6. Copy the generated password (format: `xxxx-xxxx-xxxx-xxxx`)

### 2.2 Test Password (Optional)

```bash
# Test the app-specific password works
xcrun altool --list-apps -u "your-apple-id@example.com" -p "your-app-specific-password"
```

## Step 3: Configure GitHub Repository Secrets

### 3.1 Navigate to Repository Settings

1. Go to your GitHub repository
2. Click "Settings" tab
3. Navigate to "Secrets and variables" → "Actions"
4. Click "New repository secret"

### 3.2 Add Required Secrets

Add these secrets one by one:

#### Apple Developer Account
```
Name: APPLE_ID
Value: bithighlander@gmail.com
```

```
Name: APPLE_PASSWORD
Value: your-app-specific-password-here
```

```
Name: APPLE_TEAM_ID
Value: DR57X8Z394
```

#### Code Signing Certificate
```
Name: MACOS_CERTIFICATE_BASE64
Value: [paste the base64 encoded certificate from Step 1.2]
```

```
Name: MACOS_CERTIFICATE_PASSWORD
Value: [password for the P12 certificate, if any]
```

#### Code Signing Identity
```
Name: CODESIGN_IDENTITY
Value: Developer ID Application: KeepKey LLC (DR57X8Z394)
```

#### Keychain Password (Optional)
```
Name: KEYCHAIN_PASSWORD
Value: temp-keychain-password-for-ci
```

## Step 4: Verify Secrets Configuration

### 4.1 Check All Secrets Are Set

Your repository secrets should include:
- ✅ `APPLE_ID`
- ✅ `APPLE_PASSWORD`
- ✅ `APPLE_TEAM_ID`
- ✅ `MACOS_CERTIFICATE_BASE64`
- ✅ `MACOS_CERTIFICATE_PASSWORD`
- ✅ `CODESIGN_IDENTITY`
- ✅ `KEYCHAIN_PASSWORD` (optional)

### 4.2 Test Secrets Format

```bash
# Verify your certificate identity format locally
security find-identity -v -p codesigning | grep "KeepKey"

# Output should match your CODESIGN_IDENTITY secret exactly
# Example: "Developer ID Application: KeepKey LLC (DR57X8Z394)"
```

## Step 5: Trigger Build

### 5.1 Manual Trigger

1. Go to "Actions" tab in your repository
2. Click "Build KeepKey Vault v5 (macOS Universal)"
3. Click "Run workflow"
4. Select branch (usually `master`)
5. Click "Run workflow"

### 5.2 Automatic Trigger

The workflow automatically runs on:
- Push to `master` branch
- Pull requests to `master` branch

## Step 6: Monitor Build Process

### 6.1 Build Steps

The GitHub Action will:
1. ✅ Check out code
2. ✅ Set up build environment
3. ✅ Install dependencies
4. ✅ Import certificate to temporary keychain
5. ✅ Build universal binary
6. ✅ Sign and notarize app
7. ✅ Create DMG
8. ✅ Verify signatures
9. ✅ Upload artifacts
10. ✅ Create GitHub release (on master branch)

### 6.2 Expected Output

Successful build should show:
```
✅ Apple Developer certificate found
✅ Apple notarization credentials found
✅ App bundle found
✅ Universal binary (x86_64 arm64)
✅ Code signature is valid
✅ Gatekeeper will allow execution
✅ DMG is properly stapled
```

## Step 7: Download and Test

### 7.1 Download Artifacts

1. Go to completed workflow run
2. Scroll to "Artifacts" section
3. Download "macos-universal-build"
4. Extract and test the DMG

### 7.2 Test Installation

```bash
# Test the built DMG
hdiutil attach "KeepKey Vault_0.1.0_universal.dmg"
cp -r "/Volumes/KeepKey Vault/KeepKey Vault.app" /Applications/
hdiutil detach "/Volumes/KeepKey Vault"

# Launch app (should work without errors)
open "/Applications/KeepKey Vault.app"
```

## Troubleshooting

### Common Issues

#### 1. Certificate Import Failed
```
Error: security: SecKeychainItemImport: The specified item could not be found in the keychain.
```

**Solution**: Verify `MACOS_CERTIFICATE_BASE64` and `MACOS_CERTIFICATE_PASSWORD` are correct.

#### 2. Code Signing Failed
```
Error: No identity found
```

**Solution**: Check `CODESIGN_IDENTITY` matches exactly what's in your keychain.

#### 3. Notarization Failed
```
Error: Could not find the username or password.
```

**Solution**: Verify `APPLE_ID` and `APPLE_PASSWORD` are correct.

#### 4. Build Script Not Found
```
Error: ./build.sh: No such file or directory
```

**Solution**: Ensure `build.sh` is executable and in repository root.

### Debug Commands

```bash
# Check certificate in keychain
security find-identity -v -p codesigning

# Verify app signature
codesign -vvv --deep --strict "KeepKey Vault.app"

# Check notarization status
xcrun stapler validate "KeepKey Vault.dmg"
```

## Security Best Practices

### 1. Secret Management
- ✅ Use GitHub repository secrets (never commit secrets)
- ✅ Use app-specific passwords (not main Apple ID password)
- ✅ Rotate secrets regularly
- ✅ Limit access to repository secrets

### 2. Certificate Security
- ✅ Use password-protected P12 certificates
- ✅ Store certificates securely outside repository
- ✅ Monitor certificate expiration
- ✅ Use separate certificates for different environments

### 3. Build Security
- ✅ Verify all signatures after build
- ✅ Use temporary keychains in CI
- ✅ Clean up certificates after build
- ✅ Monitor build logs for anomalies

## Maintenance

### Regular Tasks
- [ ] Monitor certificate expiration (renew 30 days before)
- [ ] Rotate app-specific passwords quarterly
- [ ] Update build dependencies monthly
- [ ] Test builds on different macOS versions

### Certificate Renewal
1. Generate new certificate in Apple Developer Portal
2. Export new P12 certificate
3. Encode to base64
4. Update `MACOS_CERTIFICATE_BASE64` secret
5. Update `CODESIGN_IDENTITY` if needed
6. Test build process

## Success Criteria

A successful GitHub Actions setup should:
- ✅ Build universal binary automatically
- ✅ Sign with valid Developer ID certificate
- ✅ Notarize with Apple successfully
- ✅ Create distributable DMG
- ✅ Pass all signature verifications
- ✅ Work for all users without security warnings

---

**Last Updated**: July 2025
**Tested With**: GitHub Actions on macOS runners
**Success Rate**: 100% (with proper setup) 