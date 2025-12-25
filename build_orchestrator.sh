sudo cp dist/guardrail-orchestrator /usr/local/bin/#!/bin/bash
# ============================================================================
# Build GuardRail Orchestrator to native binary
# ============================================================================

echo ""
echo "╔════════════════════════════════════════════════════════════════╗"
echo "║       Building GuardRail Orchestrator                         ║"
echo "╚════════════════════════════════════════════════════════════════╝"
echo ""

# Check for Python
if ! command -v python3 &> /dev/null; then
    echo "ERROR: Python3 not found. Please install Python 3.9+"
    exit 1
fi

# Install PyInstaller if needed
echo "Installing PyInstaller..."
pip3 install pyinstaller --quiet 2>/dev/null || pip install pyinstaller --quiet

# Build the executable
echo ""
echo "Building executable..."
echo ""

pyinstaller --onefile \
    --name guardrail-orchestrator \
    --console \
    --clean \
    guardrail_orchestrator.py

if [ -f "dist/guardrail-orchestrator" ]; then
    chmod +x dist/guardrail-orchestrator
    echo ""
    echo "════════════════════════════════════════════════════════════════"
    echo "BUILD SUCCESSFUL!"
    echo "════════════════════════════════════════════════════════════════"
    echo ""
    echo "Executable: dist/guardrail-orchestrator"
    echo "Size: $(du -h dist/guardrail-orchestrator | cut -f1)"
    echo ""
    echo "Usage:"
    echo "  ./guardrail-orchestrator start    - Start all services"
    echo "  ./guardrail-orchestrator daemon   - Run with auto-healing"
    echo "  ./guardrail-orchestrator status   - Check status"
    echo "  ./guardrail-orchestrator stop     - Stop all services"
    echo "  ./guardrail-orchestrator --help   - Show all commands"
    echo ""
    echo "To install system-wide:"
    echo "  sudo cp dist/guardrail-orchestrator /usr/local/bin/"
    echo ""
    echo "════════════════════════════════════════════════════════════════"
else
    echo ""
    echo "BUILD FAILED"
    exit 1
fi
