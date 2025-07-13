# Certificate Management Guide

## Overview

This document covers the proper management of Apple Developer certificates and signing credentials for KeepKey Vault v5.

## Certificate Types

### 1. Developer ID Application Certificate
- **Purpose**: Code signing applications for distribution outside the App Store
- **Required for**: Signing the .app bundle
- **Validity**: 3 years
- **Location**: Keychain Access

### 2. Developer ID Installer Certificate (Optional)
- **Purpose**: Signing .pkg installers
- **Required for**: Package installers (not DMG)
- **Validity**: 3 years

## Certificate Files in `certs/` Directory

### Current Certificate Files
```
certs/
├── keepkey-developer-id-application-combined-nopass.p12  # Main certificate (no password)
├── keepkey-developer-id-application-combined.p12        # Main certificate (with password)
├── keepkey-llc-developer-id-application.key            # Private key
├── certificate.pem                                     # Certificate in PEM format
├── developerID_application (3-5).cer                   # Various certificate versions
├── DeveloperIDG2CA.cer                                # Apple CA certificate
└── more/                                               # Additional certificate files
    ├── keepkey-llc-developer-id.key
    └── keepkey-developer-id-new.key
```

### Certificate Formats

1. **P12 Files** (`.p12`)
   - Contains both certificate and private key
   - Password protected (usually)
   - Best for importing into Keychain

2. **PEM Files** (`.pem`)
   - Text format certificate
   - Used for verification and scripts

3. **CER Files** (`.cer`)
   - Binary certificate format
   - Contains only public certificate (no private key)

4. **KEY Files** (`.key`)
   - Private key files
   - Must be kept secure

## Security Best Practices

### 1. Certificate Storage
- ✅ **Store in `certs/` directory** (ignored by git)
- ✅ **Keep backups** in secure location outside repo
- ✅ **Use password-protected P12 files**
- ❌ **Never commit certificates to git**

### 2. Environment Variables
```bash
# .env file (never commit)
APPLE_ID=your-apple-id@example.com
APPLE_PASSWORD=your-app-specific-password
APPLE_TEAM_ID=DR57X8Z394
CODESIGN_IDENTITY="Developer ID Application: KeepKey LLC (DR57X8Z394)"
```

### 3. Keychain Management
```bash
# List certificates in keychain
security find-identity -v -p codesigning

# Import P12 certificate
security import "certs/keepkey-developer-id-application-combined.p12" -k ~/Library/Keychains/login.keychain-db

# Verify certificate
codesign -vvv --deep --strict "path/to/app"
```

## Certificate Setup Process

### 1. Generate Certificate Signing Request (CSR)
```bash
# Generate private key and CSR
openssl req -new -newkey rsa:2048 -keyout keepkey-developer-id.key -out keepkey-developer-id.csr -subj "/CN=KeepKey LLC/O=KeepKey LLC/C=US"
```

### 2. Download Certificate from Apple
1. Go to [Apple Developer Portal](https://developer.apple.com)
2. Navigate to Certificates, Identifiers & Profiles
3. Create new "Developer ID Application" certificate
4. Upload your CSR file
5. Download the certificate

### 3. Convert to P12 Format
```bash
# Combine certificate and private key into P12
openssl pkcs12 -export -out keepkey-developer-id-application.p12 -inkey keepkey-developer-id.key -in developerID_application.cer -certfile DeveloperIDG2CA.cer
```

### 4. Import to Keychain
```bash
# Import P12 to keychain
security import "keepkey-developer-id-application.p12" -k ~/Library/Keychains/login.keychain-db -P "your-password"
```

## Troubleshooting

### Common Issues

#### 1. "No identity found"
```bash
# Check available identities
security find-identity -v -p codesigning

# If empty, import certificate
security import "certs/keepkey-developer-id-application-combined.p12" -k ~/Library/Keychains/login.keychain-db
```

#### 2. "Certificate expired"
- Generate new CSR
- Create new certificate in Apple Developer Portal
- Import new certificate to keychain
- Update `CODESIGN_IDENTITY` in `.env`

#### 3. "Wrong certificate format"
```bash
# Convert CER to PEM
openssl x509 -inform DER -in certificate.cer -out certificate.pem

# Extract info from P12
openssl pkcs12 -info -in certificate.p12
```

#### 4. "Keychain access denied"
```bash
# Unlock keychain
security unlock-keychain ~/Library/Keychains/login.keychain-db

# Set keychain search path
security list-keychains -s ~/Library/Keychains/login.keychain-db
```

## Certificate Verification

### 1. Verify Certificate in Keychain
```bash
# List all certificates
security find-identity -v

# Check specific certificate
security find-identity -v -p codesigning | grep "KeepKey"
```

### 2. Test Code Signing
```bash
# Sign a test file
codesign -s "Developer ID Application: KeepKey LLC (DR57X8Z394)" test_file.txt

# Verify signature
codesign -vvv test_file.txt
```

### 3. Check Certificate Expiration
```bash
# Check certificate details
security find-certificate -c "KeepKey LLC" -p | openssl x509 -text -noout | grep "Not After"
```

## Maintenance Schedule

### Monthly
- [ ] Check certificate expiration dates
- [ ] Verify signing still works
- [ ] Update app-specific passwords if needed

### Before Expiration (30 days)
- [ ] Generate new CSR
- [ ] Request new certificate from Apple
- [ ] Test new certificate
- [ ] Update build scripts

### After Certificate Renewal
- [ ] Update keychain with new certificate
- [ ] Update `CODESIGN_IDENTITY` in `.env`
- [ ] Test complete build process
- [ ] Archive old certificates

## Emergency Procedures

### Certificate Compromised
1. **Immediate**: Revoke certificate in Apple Developer Portal
2. **Generate**: New private key and CSR
3. **Request**: New certificate from Apple
4. **Update**: All build environments
5. **Test**: Complete build and distribution

### Lost Certificate
1. **Check**: Keychain and backups
2. **Generate**: New CSR if needed
3. **Request**: New certificate from Apple
4. **Document**: New certificate details

### Build Failures
1. **Verify**: Certificate in keychain
2. **Check**: Environment variables
3. **Test**: Manual signing
4. **Review**: Build logs for errors

## File Organization

### Recommended Structure
```
certs/
├── current/                    # Active certificates
│   ├── keepkey-app-cert.p12   # Main certificate
│   ├── keepkey-app-cert.pem   # PEM version
│   └── private.key            # Private key
├── archive/                   # Old certificates
│   └── 2024/
│       └── old-cert.p12
└── README.md                  # Certificate inventory
```

### Certificate Inventory
Maintain a `certs/README.md` with:
- Certificate names and purposes
- Expiration dates
- Team ID and identities
- Backup locations

## Security Checklist

- [ ] Certificates stored in `certs/` directory
- [ ] `certs/` directory in `.gitignore`
- [ ] Private keys password protected
- [ ] Environment variables in `.env` (ignored)
- [ ] Certificates backed up securely
- [ ] Regular expiration monitoring
- [ ] Access limited to authorized personnel

---

**Last Updated**: July 2025
**Next Review**: Before certificate expiration
**Responsible**: Build team lead 