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
.PHONY: all vault vault-build vault-dev test clean clean-build rebuild setup deps check-deps help clean-ports

# Display help information
help:
	@echo "KeepKey Vault v4 Build System"
	@echo ""
	@echo "Main targets:"
	@echo "  vault         - Build and run keepkey-vault-v4 in development mode"
	@echo "  vault-build   - Build keepkey-vault-v4 for production"
	@echo "  vault-dev     - Quick development build (skips dependency checks)"
	@echo "  clean         - Clean all build artifacts"
	@echo "  clean-build   - Clean only build outputs (keep dependencies)"
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
vault-build: clean-build deps
	@echo "🔧 Building KeepKey Vault v4 for production..."
	@# Validate notarization requirements for macOS
	@if [[ "$$OSTYPE" == "darwin"* ]]; then \
		echo "🔍 Validating macOS notarization requirements..."; \
		if [ ! -f ".env" ]; then \
			echo "❌ ERROR: .env file not found in project root"; \
			echo "   Please create .env file with APPLE_ID, APPLE_PASSWORD, and APPLE_TEAM_ID"; \
			exit 1; \
		fi; \
		source .env; \
		if [ -z "$$APPLE_ID" ]; then \
			echo "❌ ERROR: APPLE_ID environment variable is required for notarization"; \
			echo "   Please set APPLE_ID in .env file"; \
			exit 1; \
		fi; \
		if [ -z "$$APPLE_PASSWORD" ]; then \
			echo "❌ ERROR: APPLE_PASSWORD environment variable is required for notarization"; \
			echo "   Please set APPLE_PASSWORD in .env file (use app-specific password)"; \
			exit 1; \
		fi; \
		if [ -z "$$APPLE_TEAM_ID" ]; then \
			echo "❌ ERROR: APPLE_TEAM_ID environment variable is required for notarization"; \
			echo "   Please set APPLE_TEAM_ID in .env file"; \
			exit 1; \
		fi; \
		export APPLE_ID; \
		export APPLE_PASSWORD; \
		export APPLE_TEAM_ID; \
		echo "✅ Notarization requirements validated and exported"; \
		echo "   APPLE_ID: $$APPLE_ID"; \
		echo "   APPLE_PASSWORD: [$${#APPLE_PASSWORD} characters]"; \
		echo "   APPLE_TEAM_ID: $$APPLE_TEAM_ID"; \
	fi
	@# Build with environment variables properly exported
	@if [[ "$$OSTYPE" == "darwin"* ]]; then \
		source .env && export APPLE_ID && export APPLE_PASSWORD && export APPLE_TEAM_ID; \
	fi; \
	if command -v bun >/dev/null 2>&1; then \
		cd projects/keepkey-vault && if [[ "$$OSTYPE" == "darwin"* ]]; then source ../../.env && export APPLE_ID && export APPLE_PASSWORD && export APPLE_TEAM_ID && bun tauri build --target universal-apple-darwin; else bun tauri build; fi; \
	else \
		cd projects/keepkey-vault && if [[ "$$OSTYPE" == "darwin"* ]]; then source ../../.env && export APPLE_ID && export APPLE_PASSWORD && export APPLE_TEAM_ID && npm run tauri build -- --target universal-apple-darwin; else npm run tauri build; fi; \
	fi
	@# Verify notarization succeeded on macOS
	@if [[ "$$OSTYPE" == "darwin"* ]]; then \
		echo "🔍 Verifying build results..."; \
		APP_PATH="projects/keepkey-vault/target/universal-apple-darwin/release/bundle/macos/KeepKey Vault.app"; \
		if [ -d "$$APP_PATH" ]; then \
			echo "✅ App bundle created: $$APP_PATH"; \
			if spctl -a -v "$$APP_PATH" 2>&1 | grep -q "accepted"; then \
				echo "✅ App passes Gatekeeper validation"; \
				if spctl -a -v "$$APP_PATH" 2>&1 | grep -q "Notarized Developer ID"; then \
					echo "✅ App is properly notarized"; \
				else \
					echo "❌ ERROR: App is signed but not notarized"; \
					echo "   This indicates the notarization process failed during build"; \
					echo "   Check the build output above for notarization errors"; \
					exit 1; \
				fi; \
			else \
				echo "❌ ERROR: App failed Gatekeeper validation"; \
				spctl -a -v "$$APP_PATH"; \
				exit 1; \
			fi; \
		else \
			echo "❌ ERROR: App bundle not found at $$APP_PATH"; \
			exit 1; \
		fi; \
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

# Clean only build outputs (keep dependencies)
clean-build:
	@echo "🧹 Cleaning build outputs..."
	@if [ -d "projects/keepkey-vault/target/release/bundle" ]; then \
		cd projects/keepkey-vault && \
		rm -rf target/release/bundle/dmg/rw.*.dmg 2>/dev/null || true; \
		rm -rf target/release/bundle/macos/rw.*.dmg 2>/dev/null || true; \
		rm -rf target/release/bundle/deb/rw.*.deb 2>/dev/null || true; \
		rm -rf target/release/bundle/appimage/rw.*.AppImage 2>/dev/null || true; \
		rm -rf target/release/bundle/msi/rw.*.msi 2>/dev/null || true; \
		rm -rf target/release/bundle/nsis/rw.*.exe 2>/dev/null || true; \
		find target/release/bundle -name ".DS_Store" -delete 2>/dev/null || true; \
	fi
	@echo "✅ Build outputs cleaned"

# Force rebuild everything
rebuild: clean all

# Development server with hot reload
dev: vault-dev 