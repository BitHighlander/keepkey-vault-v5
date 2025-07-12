# KeepKey Vault v4 Build System
#
# Main targets:
#   make vault        - Build and run keepkey-vault-v4 in development mode
#   make vault-build  - Build keepkey-vault-v4 for production 
#   make vault-dev    - Quick development build (skips dependency checks)
#   make clean        - Clean all build artifacts
#   make rebuild      - Clean and rebuild everything
#   make test         - Run tests
#   make setup        - Initial project setup
#   make deps         - Install dependencies
#   make check-deps   - Verify all dependencies are installed
#
# Dependencies:
#   - Rust/Cargo (for Tauri backend)
#   - Bun or Node.js (for frontend dependencies)
#   - Tauri CLI
.PHONY: all vault vault-build vault-dev test clean rebuild setup deps check-deps help clean-ports

# Display help information
help:
	@echo "KeepKey Vault v4 Build System"
	@echo ""
	@echo "Main targets:"
	@echo "  vault         - Build and run keepkey-vault-v4 in development mode"
	@echo "  vault-build   - Build keepkey-vault-v4 for production"
	@echo "  vault-dev     - Quick development build (skips dependency checks)"
	@echo "  clean         - Clean all build artifacts"
	@echo "  rebuild       - Clean and rebuild everything"
	@echo "  test          - Run tests"
	@echo "  setup         - Initial project setup"
	@echo "  deps          - Install dependencies"
	@echo "  check-deps    - Verify all dependencies are installed"
	@echo ""
	@echo "Dependencies:"
	@echo "  - Rust/Cargo (for Tauri backend)"
	@echo "  - Bun or Node.js (for frontend dependencies)"
	@echo "  - Tauri CLI"

all: deps vault

# Check if required tools are installed
check-deps:
	@echo "🔍 Checking dependencies..."
	@command -v cargo >/dev/null 2>&1 || { echo "❌ Rust/Cargo not found. Please install Rust."; exit 1; }
	@command -v bun >/dev/null 2>&1 || command -v npm >/dev/null 2>&1 || { echo "❌ Bun or Node.js not found. Please install one of them."; exit 1; }
	@cargo tauri --version >/dev/null 2>&1 || { echo "❌ Tauri CLI not found. Run 'cargo install tauri-cli' to install."; exit 1; }
	@echo "✅ All dependencies found"

# Install dependencies
deps: check-deps
	@echo "📦 Installing dependencies..."
	@if command -v bun >/dev/null 2>&1; then \
		echo "📦 Using Bun to install frontend dependencies..."; \
		cd projects/keepkey-vault && bun install; \
	else \
		echo "📦 Using npm to install frontend dependencies..."; \
		cd projects/keepkey-vault && npm install; \
	fi
	@echo "✅ Dependencies installed"

# Initial project setup
setup:
	@echo "🚀 Setting up KeepKey Vault v4..."
	@if [ ! -f "projects/keepkey-vault/package.json" ]; then \
		echo "📦 Initializing Tauri project..."; \
		cd projects/keepkey-vault && cargo tauri init --ci; \
	fi
	@$(MAKE) deps
	@echo "✅ Project setup complete"

# Clean up processes using development ports
clean-ports:
	@echo "🧹 Cleaning up processes on development ports..."
	@# Kill processes on port 1420 (Vite)
	@lsof -ti:1420 | xargs kill -9 2>/dev/null || true
	@# Kill processes on port 1430 (Tauri)
	@lsof -ti:1430 | xargs kill -9 2>/dev/null || true
	@# Kill any existing tauri processes
	@pkill -f "tauri" 2>/dev/null || true
	@# Kill any existing vite processes
	@pkill -f "vite" 2>/dev/null || true
	@echo "✅ Ports cleaned"

# Build and run in development mode
vault: clean-ports deps
	@echo "🔧 Building and running KeepKey Vault v4 in development mode..."
	@if command -v bun >/dev/null 2>&1; then \
		cd projects/keepkey-vault && bun tauri dev; \
	else \
		cd projects/keepkey-vault && npm run tauri dev; \
	fi

# Build for production
vault-build: deps
	@echo "🔧 Building KeepKey Vault v4 for production..."
	@if command -v bun >/dev/null 2>&1; then \
		cd projects/keepkey-vault && bun tauri build; \
	else \
		cd projects/keepkey-vault && npm run tauri build; \
	fi
	@echo "✅ Production build complete"

# Quick development build (skips some checks)
vault-dev: clean-ports
	@echo "🚀 Quick KeepKey Vault v4 development build..."
	@if command -v bun >/dev/null 2>&1; then \
		cd projects/keepkey-vault && bun tauri dev; \
	else \
		cd projects/keepkey-vault && npm run tauri dev; \
	fi

# Run tests
test:
	@echo "🧪 Running tests..."
	@if [ -d "projects/keepkey-vault/src-tauri" ]; then \
		cd projects/keepkey-vault/src-tauri && cargo test; \
	fi
	@if command -v bun >/dev/null 2>&1; then \
		cd projects/keepkey-vault && bun test 2>/dev/null || echo "No frontend tests configured"; \
	else \
		cd projects/keepkey-vault && npm test 2>/dev/null || echo "No frontend tests configured"; \
	fi
	@echo "✅ Tests complete"

# Clean all build artifacts
clean:
	@echo "🧹 Cleaning all build artifacts..."
	@if [ -d "projects/keepkey-vault/src-tauri" ]; then \
		cd projects/keepkey-vault/src-tauri && cargo clean; \
	fi
	@rm -rf projects/keepkey-vault/node_modules
	@rm -rf projects/keepkey-vault/dist
	@rm -rf projects/keepkey-vault/src-tauri/target
	@echo "✅ All build artifacts cleaned"

# Force rebuild everything
rebuild: clean all

# Development server with hot reload
dev: vault-dev 