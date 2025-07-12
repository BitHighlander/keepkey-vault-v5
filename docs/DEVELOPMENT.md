# Development Guide

## Getting Started

### Prerequisites

1. **Rust** - Install from [rustup.rs](https://rustup.rs/)
2. **Node.js/Bun** - Install Node.js from [nodejs.org](https://nodejs.org/) or Bun from [bun.sh](https://bun.sh/)
3. **Tauri CLI** - Install with `cargo install tauri-cli`

### Initial Setup

```bash
# Clone and enter the project
cd keepkey-vault-v4

# Run initial setup
make setup

# Or manually:
# cargo tauri init --ci  # Initialize Tauri project
# bun install           # Install dependencies
```

### Development Workflow

```bash
# Start development server with hot reload
make vault

# Quick development build (skips dependency checks)
make vault-dev

# Build for production
make vault-build

# Run tests
make test

# Clean build artifacts
make clean
```

## Project Structure

```
keepkey-vault-v4/
├── src/                 # Frontend source code
├── src-tauri/          # Rust backend (created by tauri init)
├── public/             # Static assets
├── docs/               # Documentation
├── Makefile           # Build system
├── package.json       # Frontend dependencies
└── README.md          # Project overview
```

## Development Tips

### Hot Reload

The development server supports hot reload for both frontend and backend changes:
- Frontend changes reload instantly
- Rust changes trigger a rebuild and restart

### Debugging

- Use `console.log()` in frontend code
- Use `println!()` or `dbg!()` in Rust code
- Enable Tauri's dev tools in development mode

### Building

- Development builds are unoptimized and include debug symbols
- Production builds are optimized and stripped
- Use `make vault-build` for production builds

## Common Issues

### Port Conflicts

If you encounter port conflicts, run:
```bash
make clean-ports
```

### Dependency Issues

If dependencies seem out of sync:
```bash
make clean
make deps
```

### Tauri CLI Issues

If Tauri CLI is not found:
```bash
cargo install tauri-cli
```

## Contributing

1. Create a feature branch
2. Make your changes
3. Test thoroughly
4. Submit a pull request

## Security Considerations

- Never commit sensitive data
- Use environment variables for configuration
- Test security-critical code paths
- Follow Rust security best practices 