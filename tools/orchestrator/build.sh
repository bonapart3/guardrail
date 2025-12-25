#!/bin/bash
# Build script for GuardRail Orchestrator
# Creates native executables for the current platform

set -e

echo "╔════════════════════════════════════════════════╗"
echo "║    Building GuardRail Orchestrator             ║"
echo "╚════════════════════════════════════════════════╝"
echo

cd "$(dirname "$0")"

# Check for Rust
if ! command -v cargo &> /dev/null; then
    echo "Error: Rust/Cargo not found. Please install from https://rustup.rs"
    exit 1
fi

echo "Building release binary..."
cargo build --release

# Get the binary path
if [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "win32" ]]; then
    BINARY="target/release/guardrail-orchestrator.exe"
else
    BINARY="target/release/guardrail-orchestrator"
fi

if [ -f "$BINARY" ]; then
    echo
    echo "✓ Build successful!"
    echo
    echo "Binary location: $BINARY"
    echo "Binary size: $(du -h "$BINARY" | cut -f1)"
    echo
    echo "To install system-wide (Linux/macOS):"
    echo "  sudo cp $BINARY /usr/local/bin/"
    echo
    echo "Usage:"
    echo "  ./guardrail-orchestrator start    # Start all services"
    echo "  ./guardrail-orchestrator daemon   # Run with auto-healing"
    echo "  ./guardrail-orchestrator status   # Check status"
    echo "  ./guardrail-orchestrator --help   # Show all commands"
else
    echo "Error: Build failed, binary not found"
    exit 1
fi
