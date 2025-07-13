# KeepKey Desktop Code Signing

This document provides information about the code signing process for KeepKey Desktop application, with self-signing implemented for Windows and Linux platforms.

## Overview

Code signing is a security practice that verifies the authenticity and integrity of software. It ensures that:

1. The application comes from a legitimate source (authenticity)
2. The application has not been tampered with (integrity)

KeepKey Desktop implements:
- A self-signing mechanism for Windows and Linux platforms
- Apple's standard notarization and signing for macOS

## How Code Signing Works in KeepKey Desktop

### Windows and Linux

For Windows and Linux, the signing process consists of the following steps:

1. **Certificate Generation**: Self-signed certificates are created using OpenSSL
2. **Application Signing**: 
   - Windows: Using signtool.exe from the Windows SDK
   - Linux: Using OpenSSL to create detached signatures
3. **Verification**: Users can verify the signed application before installation/execution

### macOS

For macOS, the standard Apple notarization and signing process is used:
- Code signing with Apple Developer certificates
- Notarization through Apple's notary service
- Stapling of the notarization ticket to the application

## Setting Up for Development

### Prerequisites

- OpenSSL (required for Windows and Linux)
- For Windows: Windows SDK (includes signtool.exe)
- For macOS: XCode Command Line Tools (includes codesign utility) and Apple Developer credentials

### Generating Signing Certificates (Windows and Linux)

To generate self-signed certificates for development on Windows and Linux:

```bash
# Run the self-signing script
./scripts/self_signing.sh
```

This will create:
- A private key (`./certs/private.key`)
- A certificate file (`./certs/certificate.pem`)
- A PFX file for Windows (`./certs/certificate.pfx`)

The script also stores the necessary credentials in a `.env` file for the build process.

## Signing Process in CI/CD

In the GitHub Actions workflow:

1. For Windows and Linux:
   - Self-signed certificates are generated if they don't exist
   - The environment is configured with signing information
   - The built artifacts are signed using platform-specific methods
   - The signed artifacts are verified and uploaded

2. For macOS:
   - The original Apple signing and notarization process is used
   - No self-signing is applied

## Manual Signing (Windows and Linux)

You can manually sign applications using the provided script:

```bash
# Example usage
node scripts/sign_app.js /path/to/application.exe
```

## Testing the Signing Process Locally

You can test the signing and verification process locally using the provided test script:

```bash
# Run the test script
./scripts/test_signing.sh
```

This script will:
1. Generate self-signed certificates if they don't exist
2. Create a sample test file
3. Sign the file using OpenSSL (detached signature)
4. Verify the signature
5. Report the results

The test script works on all platforms (macOS, Windows with Git Bash/WSL, and Linux) and provides a quick way to verify that the signing tools are working correctly in your development environment.

## Verifying Signed Applications

### Windows and Linux

You can verify the signature of applications using the provided script:

```bash
# Basic verification
node scripts/verify_app.js /path/to/application.exe

# Verification with certificate
node scripts/verify_app.js /path/to/application.exe /path/to/certificate.pem
```

### Platform-Specific Verification

#### Windows

```bash
# Verify signature
signtool verify /pa /v /d /path/to/KeepKey-Desktop.exe
```

#### Linux

On Linux, the application has a detached signature file (`.sig`). Verification requires:

```bash
# Verify with certificate
openssl dgst -sha256 -verify certificate.pem -signature /path/to/KeepKey-Desktop.AppImage.sig /path/to/KeepKey-Desktop.AppImage
```

#### macOS

For macOS, use the standard Apple verification:

```bash
# Verify signature
codesign -v --verify /path/to/KeepKey-Desktop.app

# Show signature details
codesign -d -vv /path/to/KeepKey-Desktop.app
```

## Security Considerations

- Self-signed certificates should be protected and not shared publicly
- The `.env` file contains sensitive information and should be kept secure
- For production releases on Windows and Linux, consider using certificates from a trusted Certificate Authority
- macOS uses Apple's signing infrastructure which is already backed by a trusted Certificate Authority

## Troubleshooting

### Common Issues

- **Missing OpenSSL**: Ensure OpenSSL is installed and available in your PATH (Windows and Linux)
- **Signing Failed**: Check that the certificate files exist and have proper permissions
- **Verification Failed**: Ensure you're using the correct certificate for verification

### Debugging

For detailed logging during the signing process:

```bash
# Enable DEBUG for signing scripts
DEBUG=1 node scripts/sign_app.js /path/to/application
``` 