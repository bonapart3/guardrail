@echo off
REM ============================================================================
REM Build GuardRail Orchestrator to Windows .exe
REM ============================================================================

echo.
echo ╔════════════════════════════════════════════════════════════════╗
echo ║       Building GuardRail Orchestrator .exe                     ║
echo ╚════════════════════════════════════════════════════════════════╝
echo.

REM Activate virtual environment
if exist ".venv\Scripts\activate.bat" (
    call .venv\Scripts\activate.bat
) else (
    echo ERROR: Virtual environment not found. Please run 'python -m venv .venv' first.
    exit /b 1
)

REM Check for Python
where python >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo ERROR: Python not found. Please install Python 3.9+
    echo Download from: https://python.org
    exit /b 1
)

REM Check for pip
where pip >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo ERROR: pip not found.
    exit /b 1
)

REM Install PyInstaller if needed
echo Installing PyInstaller...
pip install pyinstaller --quiet

REM Build the executable
echo.
echo Building executable...
echo.

pyinstaller --onefile ^
    --name guardrail-orchestrator ^
    --icon=NONE ^
    --console ^
    --clean ^
    guardrail_orchestrator.py

if exist "dist\guardrail-orchestrator.exe" (
    echo.
    echo ════════════════════════════════════════════════════════════════
    echo BUILD SUCCESSFUL!
    echo ════════════════════════════════════════════════════════════════
    echo.
    echo Executable: dist\guardrail-orchestrator.exe
    echo.
    echo Usage:
    echo   guardrail-orchestrator.exe start    - Start all services
    echo   guardrail-orchestrator.exe daemon   - Run with auto-healing
    echo   guardrail-orchestrator.exe status   - Check status
    echo   guardrail-orchestrator.exe stop     - Stop all services
    echo   guardrail-orchestrator.exe --help   - Show all commands
    echo.
    echo Copy the .exe to your GuardRail project folder and run it!
    echo ════════════════════════════════════════════════════════════════
) else (
    echo.
    echo BUILD FAILED
    exit /b 1
)
