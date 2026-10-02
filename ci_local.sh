#!/bin/bash
# ScreenBuddy Local CI/CD Pipeline
# Runs entirely on-device: check → test → build → package → install

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log() { echo -e "${GREEN}[CI]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
err() { echo -e "${RED}[ERROR]${NC} $1"; }

# Configuration
RELEASE_DIR="target/release"
INSTALLER_DIR="target/installer"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*= *"\(.*\)"/\1/')

echo "============================================"
echo "  ScreenBuddy Local CI/CD Pipeline"
echo "  Version: $VERSION"
echo "  Timestamp: $TIMESTAMP"
echo "============================================"

# Step 1: Format Check
log "Step 1: Checking code formatting..."
if command -v cargo-fmt &> /dev/null; then
    cargo fmt --all -- --check || { err "Format check failed"; exit 1; }
    log "Format check passed"
else
    warn "cargo-fmt not found, skipping format check"
fi

# Step 2: Clippy Lints
log "Step 2: Running Clippy lints..."
cargo clippy --all-targets -- -D warnings 2>/dev/null || {
    warn "Clippy found issues (non-blocking)"
}
log "Clippy check complete"

# Step 3: Run Tests
log "Step 3: Running all tests..."
cargo test --workspace --verbose 2>&1 | tee "target/test-results-$TIMESTAMP.log"
TEST_EXIT=${PIPESTATUS[0]}
if [ $TEST_EXIT -ne 0 ]; then
    err "Tests failed with exit code $TEST_EXIT"
    exit 1
fi
log "All tests passed"

# Step 4: Build Release
log "Step 4: Building release binary..."
cargo build --release --verbose 2>&1 | tee "target/build-$TIMESTAMP.log"
if [ ! -f "$RELEASE_DIR/screenbuddy.exe" ]; then
    err "Release binary not found"
    exit 1
fi
log "Release build complete: $RELEASE_DIR/screenbuddy.exe"

# Step 5: Package
log "Step 5: Packaging..."
mkdir -p "$INSTALLER_DIR"
cp "$RELEASE_DIR/screenbuddy.exe" "$INSTALLER_DIR/"
cp -r assets "$INSTALLER_DIR/" 2>/dev/null || true
cp -r toolkit "$INSTALLER_DIR/" 2>/dev/null || true

# Create zip archive
if command -v zip &> /dev/null; then
    cd "$INSTALLER_DIR"
    zip -r "../screenbuddy-${VERSION}-${TIMESTAMP}.zip" . > /dev/null
    cd "$SCRIPT_DIR"
    log "Package created: target/screenbuddy-${VERSION}-${TIMESTAMP}.zip"
else
    warn "zip not found, skipping archive creation"
fi

# Step 6: Generate Installer (if NSIS available)
if command -v makensis &> /dev/null; then
    log "Step 6: Generating NSIS installer..."
    if [ -f "installer.nsi" ]; then
        makensis installer.nsi
        log "Installer created"
    else
        warn "installer.nsi not found, skipping installer generation"
    fi
else
    log "Step 6: NSIS not available, skipping installer generation"
fi

# Step 7: Generate Report
log "Step 7: Generating build report..."
cat > "target/build-report-$TIMESTAMP.txt" << EOF
ScreenBuddy Build Report
========================
Version: $VERSION
Timestamp: $TIMESTAMP
Binary: $RELEASE_DIR/screenbuddy.exe
Size: $(stat -c%s "$RELEASE_DIR/screenbuddy.exe" 2>/dev/null || stat -f%z "$RELEASE_DIR/screenbuddy.exe" 2>/dev/null || echo "unknown") bytes
Tests: passed
Build: success
EOF

echo ""
echo "============================================"
echo "  CI/CD Pipeline Complete!"
echo "  Binary: $RELEASE_DIR/screenbuddy.exe"
echo "  Package: $INSTALLER_DIR/"
echo "  Report: target/build-report-$TIMESTAMP.txt"
echo "============================================"
