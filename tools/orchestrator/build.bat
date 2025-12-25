@echo off
REM Build script for GuardRail Orchestrator (Windows)
REM Creates native .exe for Windows

echo ╔════════════════════════════════════════════════╗
echo ║    Building GuardRail Orchestrator             ║
echo ╚════════════════════════════════════════════════╝
echo.

REM Check for Rust
where cargo >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo Error: Rust/Cargo not found. Please install from https://rustup.rs
    exit /b 1
)

echo Building release binary...
cargo build --release

if exist "target\release\guardrail-orchestrator.exe" (
    echo.
    echo Build successful!
    echo.
    echo Binary location: target\release\guardrail-orchestrator.exe
    echo.
    echo Usage:
    echo   guardrail-orchestrator.exe start    - Start all services
    echo   guardrail-orchestrator.exe daemon   - Run with auto-healing
    echo   guardrail-orchestrator.exe status   - Check status
    echo   guardrail-orchestrator.exe --help   - Show all commands
) else (
    echo Error: Build failed, binary not found
    exit /b 1
)
