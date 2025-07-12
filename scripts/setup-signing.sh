#!/bin/bash

# KeepKey Vault v5 - Signing Setup Script
# This script helps set up environment variables for code signing

echo "🔐 KeepKey Vault v5 - Signing Setup"
echo "=================================="
echo ""
echo "This script will help you set up environment variables for:"
echo "1. Tauri updater signing"
echo "2. macOS code signing (optional)"
echo "3. Windows code signing (optional)"
echo ""

# Tauri Updater Keys
echo "📝 Tauri Updater Configuration"
echo "------------------------------"

if [ -f "$HOME/.tauri/keepkey-vault-v5.key" ]; then
    echo "✅ Private key found at: ~/.tauri/keepkey-vault-v5.key"
    echo ""
    echo "Add these to your shell profile (~/.zshrc or ~/.bashrc):"
    echo ""
    echo "# Tauri Updater Signing"
    echo "export TAURI_SIGNING_PRIVATE_KEY=\"$HOME/.tauri/keepkey-vault-v5.key\""
    echo "# export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=\"your-password-here\""
    echo ""
else
    echo "❌ Private key not found!"
    echo "Generate it with: cargo tauri signer generate -w ~/.tauri/keepkey-vault-v5.key"
    echo ""
fi

# GitHub Actions Secrets
echo "🔒 GitHub Actions Secrets"
echo "------------------------"
echo ""
echo "For GitHub Actions, add these repository secrets:"
echo ""
echo "1. TAURI_PRIVATE_KEY - Contents of the private key file (base64 encoded):"
echo "   cat ~/.tauri/keepkey-vault-v5.key | base64"
echo ""
echo "2. TAURI_KEY_PASSWORD - The password you used when generating the key"
echo ""

# macOS Code Signing (Optional)
echo "🍎 macOS Code Signing (Optional)"
echo "--------------------------------"
echo ""
echo "If you have an Apple Developer account, add these secrets:"
echo "- APPLE_CERTIFICATE - Base64 encoded .p12 certificate"
echo "- APPLE_CERTIFICATE_PASSWORD - Certificate password"
echo "- APPLE_SIGNING_IDENTITY - Identity from the certificate"
echo "- APPLE_ID - Your Apple ID"
echo "- APPLE_PASSWORD - App-specific password"
echo "- APPLE_TEAM_ID - Your Apple Developer Team ID"
echo ""

# Windows Code Signing (Optional)
echo "🪟 Windows Code Signing (Optional)"
echo "----------------------------------"
echo ""
echo "For Windows code signing, you'll need a code signing certificate."
echo "This is typically more complex and requires purchasing a certificate"
echo "from a trusted Certificate Authority."
echo ""

echo "📚 Documentation"
echo "---------------"
echo "For more information, see:"
echo "- Tauri Updater: https://tauri.app/v1/guides/distribution/updater"
echo "- macOS Signing: https://tauri.app/v1/guides/distribution/sign-macos"
echo "- Windows Signing: https://tauri.app/v1/guides/distribution/sign-windows" 