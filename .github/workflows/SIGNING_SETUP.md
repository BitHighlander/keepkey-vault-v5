# GitHub Actions Signing Setup for KeepKey Vault v5

This guide explains how to set up the required GitHub secrets for automatic signing of Tauri updates.

## Required Secrets

### 1. TAURI_PRIVATE_KEY

The base64-encoded content of your Tauri signing private key.

**How to get this value:**
```bash
# From your local machine where you generated the key:
cat ~/.tauri/keepkey-vault-v5.key | base64 | pbcopy
```

Then add this as a secret in your GitHub repository settings.

### 2. TAURI_KEY_PASSWORD

The password you used when generating the Tauri signing key.

## Adding Secrets to GitHub

1. Go to your repository on GitHub
2. Click on **Settings** → **Secrets and variables** → **Actions**
3. Click **New repository secret**
4. Add each secret:
   - Name: `TAURI_PRIVATE_KEY`
   - Value: (paste the base64-encoded key)
   
   - Name: `TAURI_KEY_PASSWORD`
   - Value: (your key password)

## How It Works

When you push a tag starting with `v` (e.g., `v1.0.0`), the release workflow will:

1. Build the app for all platforms
2. Sign the binaries with your Tauri signing key
3. Generate update manifests (`latest.json`) for each platform
4. Upload everything to a GitHub release

The Tauri updater in your app will check the `latest.json` file and verify signatures using the public key embedded in `tauri.conf.json`.

## Security Notes

- **NEVER** commit the private key to the repository
- The private key is stored encrypted in GitHub secrets
- Only users with admin access to the repository can view/modify secrets
- The signing happens automatically in GitHub Actions, so developers don't need the private key locally

## Testing Updates

After setting up signing and creating a release:

1. Install an older version of the app
2. The app should automatically check for updates
3. When an update is found, it will download and verify the signature
4. If valid, it will prompt the user to install the update 