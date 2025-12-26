#!/usr/bin/env python3
"""
GuardRail Orchestrator
======================
Dedicated daemon for monitoring, installing, booting, and self-healing
the GuardRail compliance platform.

Usage:
    python guardrail_orchestrator.py start      # Start all services
    python guardrail_orchestrator.py stop       # Stop all services  
    python guardrail_orchestrator.py daemon     # Run with auto-healing
    python guardrail_orchestrator.py status     # Check status
    python guardrail_orchestrator.py install    # Install dependencies
    python guardrail_orchestrator.py logs       # Tail logs
    python guardrail_orchestrator.py check      # Check dependencies

To compile to .exe:
    pip install pyinstaller
    pyinstaller --onefile --name guardrail-orchestrator guardrail_orchestrator.py
"""

import os
import sys
import json
import time
import signal
import socket
import shutil
import logging
import threading
import subprocess
from pathlib import Path
from datetime import datetime, timedelta
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Any
from enum import Enum
import urllib.request
import urllib.error

# ============================================================================
# Configuration
# ============================================================================

VERSION = "1.0.1"
CONFIG_FILE = "guardrail-orchestrator.json"
LOG_DIR = "logs"
PID_FILE = ".guardrail-orchestrator.pid"

class ServiceStatus(Enum):
    STOPPED = "STOPPED"
    STARTING = "STARTING"
    RUNNING = "RUNNING"
    UNHEALTHY = "UNHEALTHY"
    FAILED = "FAILED"
    RESTARTING = "RESTARTING"

@dataclass
class ServiceConfig:
    name: str
    port: int
    command: str
    args: List[str]
    working_dir: str
    health_endpoint: str
    depends_on: List[str]
    env: Dict[str, str]

@dataclass
class ServiceState:
    config: ServiceConfig
    status: ServiceStatus = ServiceStatus.STOPPED
    process: Optional[subprocess.Popen] = None
    pid: Optional[int] = None
    start_time: Optional[datetime] = None
    last_health_check: Optional[datetime] = None
    health_check_failures: int = 0
    restart_count: int = 0
    log_file: Optional[Path] = None

    def uptime_str(self) -> str:
        if not self.start_time:
            return "-"
        delta = datetime.now() - self.start_time
        secs = int(delta.total_seconds())
        if secs < 60:
            return f"{secs}s"
        elif secs < 3600:
            return f"{secs // 60}m {secs % 60}s"
        else:
            return f"{secs // 3600}h {(secs % 3600) // 60}m"

@dataclass
class Config:
    project_root: str = "."
    log_level: str = "INFO"
    health_check_interval_secs: int = 10
    restart_delay_secs: int = 5
    max_restart_attempts: int = 3
    postgres_port: int = 5432
    redis_port: int = 6379
    docker_compose_file: str = "infrastructure/docker-compose.yml"

# ============================================================================
# Color Helpers (cross-platform, Windows-safe)
# ============================================================================

class Colors:
    RESET = "\033[0m"
    RED = "\033[91m"
    GREEN = "\033[92m"
    YELLOW = "\033[93m"
    BLUE = "\033[94m"
    CYAN = "\033[96m"
    WHITE = "\033[97m"
    BOLD = "\033[1m"

def colored(text: str, color: str) -> str:
    # Enable ANSI on Windows
    if sys.platform == "win32":
        os.system("")
    return f"{color}{text}{Colors.RESET}"

# Windows-safe symbols (ASCII only)
SYM_CHECK = "[OK]"
SYM_CROSS = "[X]"
SYM_WARN = "[!]"
SYM_ARROW = "->"
SYM_BULLET = "*"

# ============================================================================
# Logger (Windows-compatible)
# ============================================================================

class OrchestratorLogger:
    def __init__(self, log_dir: Path, level: str = "INFO"):
        self.log_dir = log_dir
        self.log_dir.mkdir(parents=True, exist_ok=True)
        
        log_file = log_dir / f"orchestrator-{datetime.now().strftime('%Y%m%d')}.log"
        
        # Use UTF-8 encoding for log file
        self.log_file_handle = open(log_file, 'a', encoding='utf-8')
        self.level = getattr(logging, level.upper(), logging.INFO)

    def _log(self, level: int, service: str, message: str, color: str):
        if level < self.level:
            return
            
        timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S.%f")[:-3]
        level_name = logging.getLevelName(level)
        service_name = service if service else "orchestrator"
        
        # Console output with colors
        level_colored = {
            logging.DEBUG: colored("DEBUG", Colors.CYAN),
            logging.INFO: colored("INFO ", Colors.GREEN),
            logging.WARNING: colored("WARN ", Colors.YELLOW),
            logging.ERROR: colored("ERROR", Colors.RED),
        }.get(level, level_name)
        
        # Print to console (with colors)
        console_msg = f"[{timestamp}] {level_colored} [{service_name}] {message}"
        print(console_msg)
        
        # Write to file (plain text, UTF-8)
        file_msg = f"[{timestamp}] {level_name:<5} [{service_name}] {message}\n"
        try:
            self.log_file_handle.write(file_msg)
            self.log_file_handle.flush()
        except Exception:
            pass  # Ignore file write errors

    def debug(self, service: str, message: str):
        self._log(logging.DEBUG, service, message, Colors.CYAN)

    def info(self, service: str, message: str):
        self._log(logging.INFO, service, message, Colors.GREEN)

    def warn(self, service: str, message: str):
        self._log(logging.WARNING, service, message, Colors.YELLOW)

    def error(self, service: str, message: str):
        self._log(logging.ERROR, service, message, Colors.RED)

    def close(self):
        try:
            self.log_file_handle.close()
        except:
            pass

# ============================================================================
# Service Definitions
# ============================================================================

def get_default_services() -> List[ServiceConfig]:
    # Load from environment or use defaults
    db_url = os.getenv("DATABASE_URL", "postgresql://guardrail:guardrail_dev@localhost:5432/guardrail")
    redis_url = os.getenv("REDIS_URL", "redis://localhost:6379")
    jwt_secret = os.getenv("JWT_SECRET", "dev_secret_change_in_production")
    
    base_env = {
        "DATABASE_URL": db_url,
        "REDIS_URL": redis_url,
        "JWT_SECRET": jwt_secret,
        "RUST_LOG": "info",
    }
    
    return [
        ServiceConfig(
            name="identity-service",
            port=3001,
            command="cargo",
            args=["run", "--release", "--bin", "identity-service"],
            working_dir="backend",
            health_endpoint="/health",
            depends_on=["postgres", "redis"],
            env={**base_env, "PORT": "3001"},
        ),
        ServiceConfig(
            name="policy-engine",
            port=3002,
            command="cargo",
            args=["run", "--release", "--bin", "policy-engine"],
            working_dir="backend",
            health_endpoint="/health",
            depends_on=["postgres", "redis"],
            env={**base_env, "PORT": "3002"},
        ),
        ServiceConfig(
            name="movement-ledger",
            port=3003,
            command="cargo",
            args=["run", "--release", "--bin", "movement-ledger"],
            working_dir="backend",
            health_endpoint="/health",
            depends_on=["postgres", "redis"],
            env={**base_env, "PORT": "3003"},
        ),
        ServiceConfig(
            name="chain-anchor",
            port=3004,
            command="cargo",
            args=["run", "--release", "--bin", "chain-anchor"],
            working_dir="backend",
            health_endpoint="/health",
            depends_on=["postgres"],
            env={**base_env, "PORT": "3004"},
        ),
        ServiceConfig(
            name="api-gateway",
            port=3000,
            command="cargo",
            args=["run", "--release", "--bin", "api-gateway"],
            working_dir="backend",
            health_endpoint="/health",
            depends_on=["identity-service", "policy-engine", "movement-ledger", "chain-anchor"],
            env={
                **base_env,
                "PORT": "3000",
                "IDENTITY_SERVICE_URL": "http://localhost:3001",
                "POLICY_ENGINE_URL": "http://localhost:3002",
                "MOVEMENT_LEDGER_URL": "http://localhost:3003",
                "CHAIN_ANCHOR_URL": "http://localhost:3004",
                "JWT_SECRET": "dev_secret_change_in_production",
            },
        ),
        ServiceConfig(
            name="frontend",
            port=3010,
            command="npm.cmd" if sys.platform == "win32" else "npm",
            args=["run", "dev"],
            working_dir="frontend",
            health_endpoint="/",
            depends_on=["api-gateway"],
            env={
                "NEXT_PUBLIC_API_URL": "http://localhost:3000",
                "PORT": "3010",
            },
        ),
    ]

# ============================================================================
# Orchestrator
# ============================================================================

class Orchestrator:
    def __init__(self, project_root: Path):
        self.project_root = project_root
        self.config = Config(project_root=str(project_root))
        self.log_dir = project_root / LOG_DIR
        self.logger = OrchestratorLogger(self.log_dir, self.config.log_level)
        
        self.services: Dict[str, ServiceState] = {}
        for svc_config in get_default_services():
            self.services[svc_config.name] = ServiceState(config=svc_config)
        
        self.running = True
        self._setup_signal_handlers()

    def _setup_signal_handlers(self):
        def handler(signum, frame):
            self.logger.info("", "Received shutdown signal...")
            self.running = False
        
        signal.signal(signal.SIGINT, handler)
        signal.signal(signal.SIGTERM, handler)

    # ========== Dependency Checks ==========

    def check_command(self, cmd: str) -> Optional[str]:
        """Check if a command exists and return its version."""
        try:
            result = subprocess.run(
                [cmd, "--version"],
                capture_output=True,
                text=True,
                timeout=10
            )
            if result.returncode == 0:
                return result.stdout.strip().split('\n')[0]
        except (subprocess.SubprocessError, FileNotFoundError):
            pass
        return None

    def check_dependencies(self) -> bool:
        self.logger.info("", "Checking system dependencies...")
        all_ok = True
        
        # Docker
        version = self.check_command("docker")
        if version:
            self.logger.info("", f"{SYM_CHECK} Docker: {version}")
        else:
            self.logger.error("", f"{SYM_CROSS} Docker not found. Please install Docker.")
            all_ok = False

        # Docker Compose
        try:
            result = subprocess.run(
                ["docker", "compose", "version"],
                capture_output=True, text=True, timeout=10
            )
            if result.returncode == 0:
                self.logger.info("", f"{SYM_CHECK} Docker Compose: {result.stdout.strip()}")
            else:
                raise Exception()
        except:
            self.logger.error("", f"{SYM_CROSS} Docker Compose not found.")
            all_ok = False

        # Rust
        version = self.check_command("rustc")
        if version:
            self.logger.info("", f"{SYM_CHECK} Rust: {version}")
        else:
            self.logger.error("", f"{SYM_CROSS} Rust not found. Install from rustup.rs")
            all_ok = False

        # Cargo
        version = self.check_command("cargo")
        if version:
            self.logger.info("", f"{SYM_CHECK} Cargo: {version}")
        else:
            self.logger.error("", f"{SYM_CROSS} Cargo not found.")
            all_ok = False

        # Node.js
        version = self.check_command("node")
        if version:
            self.logger.info("", f"{SYM_CHECK} Node.js: {version}")
        else:
            self.logger.warn("", f"{SYM_WARN} Node.js not found. Frontend won't work.")

        # npm
        version = self.check_command("npm.cmd" if sys.platform == "win32" else "npm")
        if version:
            self.logger.info("", f"{SYM_CHECK} npm: {version}")
        else:
            self.logger.warn("", f"{SYM_WARN} npm not found. Frontend won't work.")

        return all_ok

    # ========== Port Checking ==========

    def check_port(self, port: int, timeout: float = 1.0) -> bool:
        """Check if a port is open (something listening)."""
        try:
            sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            sock.settimeout(timeout)
            result = sock.connect_ex(('127.0.0.1', port))
            sock.close()
            return result == 0
        except:
            return False

    def wait_for_port(self, port: int, timeout: int = 30) -> bool:
        """Wait for a port to become available."""
        for _ in range(timeout):
            if self.check_port(port):
                return True
            time.sleep(1)
        return False

    # ========== Infrastructure ==========

    def start_infrastructure(self) -> bool:
        self.logger.info("", "Starting infrastructure (PostgreSQL, Redis)...")
        
        compose_file = self.project_root / self.config.docker_compose_file
        if not compose_file.exists():
            self.logger.error("", f"Docker Compose file not found: {compose_file}")
            return False

        # Check if ports are already in use (containers might already be running)
        postgres_running = self.check_port(self.config.postgres_port)
        redis_running = self.check_port(self.config.redis_port)

        if postgres_running and redis_running:
            self.logger.info("", f"{SYM_CHECK} PostgreSQL already running on port {self.config.postgres_port}")
            self.logger.info("", f"{SYM_CHECK} Redis already running on port {self.config.redis_port}")
            return True

        # Determine which services to start
        services_to_start = []
        if not postgres_running:
            services_to_start.append("postgres")
        else:
            self.logger.info("", f"{SYM_CHECK} PostgreSQL already running on port {self.config.postgres_port}")
        
        if not redis_running:
            services_to_start.append("redis")
        else:
            self.logger.info("", f"{SYM_CHECK} Redis already running on port {self.config.redis_port}")

        if not services_to_start:
            return True

        try:
            cmd = ["docker", "compose", "-f", str(compose_file), "up", "-d"] + services_to_start
            result = subprocess.run(
                cmd,
                cwd=self.project_root,
                capture_output=True,
                text=True,
                timeout=180  # 3 minutes for image pulls
            )
            
            if result.returncode != 0:
                # Check if it's just a port conflict
                if "port is already allocated" in result.stderr:
                    self.logger.warn("", "Port conflict detected - services may already be running")
                    # Verify ports are actually working
                    if self.check_port(self.config.postgres_port) and self.check_port(self.config.redis_port):
                        self.logger.info("", "Infrastructure ports are accessible - continuing")
                        return True
                
                self.logger.error("", f"Failed to start infrastructure: {result.stderr[-500:]}")  # Last 500 chars
                return False

            self.logger.info("", "Infrastructure containers started")

            # Wait for PostgreSQL
            if "postgres" in services_to_start:
                self.logger.info("", "Waiting for PostgreSQL...")
                if not self.wait_for_port(self.config.postgres_port, 30):
                    self.logger.error("", "PostgreSQL failed to start within 30 seconds")
                    return False
                self.logger.info("", f"{SYM_CHECK} PostgreSQL is ready")

            # Wait for Redis
            if "redis" in services_to_start:
                self.logger.info("", "Waiting for Redis...")
                if not self.wait_for_port(self.config.redis_port, 30):
                    self.logger.error("", "Redis failed to start within 30 seconds")
                    return False
                self.logger.info("", f"{SYM_CHECK} Redis is ready")

            return True

        except subprocess.TimeoutExpired:
            self.logger.error("", "Infrastructure startup timed out (this may be due to image downloads)")
            self.logger.info("", "Try running: docker compose -f infrastructure/docker-compose.yml up -d")
            return False
        except Exception as e:
            self.logger.error("", f"Failed to start infrastructure: {e}")
            return False

    def stop_infrastructure(self):
        self.logger.info("", "Stopping infrastructure...")
        compose_file = self.project_root / self.config.docker_compose_file
        
        try:
            subprocess.run(
                ["docker", "compose", "-f", str(compose_file), "down"],
                cwd=self.project_root,
                capture_output=True,
                timeout=60
            )
            self.logger.info("", "Infrastructure stopped")
        except Exception as e:
            self.logger.warn("", f"Error stopping infrastructure: {e}")

    # ========== Service Management ==========

    def start_service(self, name: str) -> bool:
        service = self.services.get(name)
        if not service:
            self.logger.error("", f"Service '{name}' not found")
            return False

        if service.status in (ServiceStatus.RUNNING, ServiceStatus.STARTING):
            self.logger.warn(name, "Service already running or starting")
            return True

        service.status = ServiceStatus.STARTING
        self.logger.info(name, "Starting service...")

        # Create log file
        log_file = self.log_dir / f"{name}.log"
        service.log_file = log_file

        try:
            # Build environment
            env = os.environ.copy()
            env.update(service.config.env)

            # Build command
            working_dir = self.project_root / service.config.working_dir
            cmd = [service.config.command] + service.config.args

            # Check if working directory exists
            if not working_dir.exists():
                self.logger.error(name, f"Working directory not found: {working_dir}")
                service.status = ServiceStatus.FAILED
                return False

            # Open log file
            log_handle = open(log_file, 'a', encoding='utf-8')

            # Start process
            if sys.platform == "win32":
                # Windows: use CREATE_NEW_PROCESS_GROUP
                process = subprocess.Popen(
                    cmd,
                    cwd=working_dir,
                    env=env,
                    stdout=log_handle,
                    stderr=subprocess.STDOUT,
                    creationflags=subprocess.CREATE_NEW_PROCESS_GROUP
                )
            else:
                # Unix: use start_new_session
                process = subprocess.Popen(
                    cmd,
                    cwd=working_dir,
                    env=env,
                    stdout=log_handle,
                    stderr=subprocess.STDOUT,
                    start_new_session=True
                )

            service.process = process
            service.pid = process.pid
            service.start_time = datetime.now()
            service.health_check_failures = 0

            self.logger.info(name, f"Started with PID {process.pid}")

            # Wait briefly and check if still running
            time.sleep(3)
            
            if process.poll() is not None:
                self.logger.error(name, f"Process exited immediately with code {process.returncode}")
                # Show last few lines of log
                try:
                    with open(log_file, 'r', encoding='utf-8', errors='ignore') as f:
                        lines = f.readlines()
                        for line in lines[-5:]:
                            self.logger.error(name, f"  {line.rstrip()}")
                except:
                    pass
                service.status = ServiceStatus.FAILED
                return False

            service.status = ServiceStatus.RUNNING
            self.logger.info(name, f"{SYM_CHECK} Service is running")
            return True

        except FileNotFoundError as e:
            self.logger.error(name, f"Command not found: {service.config.command}")
            self.logger.error(name, f"Make sure {service.config.command} is installed and in PATH")
            service.status = ServiceStatus.FAILED
            return False
        except Exception as e:
            self.logger.error(name, f"Failed to start: {e}")
            service.status = ServiceStatus.FAILED
            return False

    def stop_service(self, name: str) -> bool:
        service = self.services.get(name)
        if not service:
            return False

        if service.status == ServiceStatus.STOPPED:
            return True

        self.logger.info(name, "Stopping service...")

        if service.process:
            try:
                # Try graceful shutdown
                service.process.terminate()
                
                # Wait up to 10 seconds
                for _ in range(20):
                    if service.process.poll() is not None:
                        break
                    time.sleep(0.5)
                else:
                    # Force kill
                    service.process.kill()
                    service.process.wait()

            except Exception as e:
                self.logger.warn(name, f"Error stopping process: {e}")

        service.process = None
        service.pid = None
        service.status = ServiceStatus.STOPPED
        service.start_time = None

        self.logger.info(name, "Service stopped")
        return True

    def restart_service(self, name: str) -> bool:
        self.stop_service(name)
        time.sleep(self.config.restart_delay_secs)
        return self.start_service(name)

    # ========== Health Checks ==========

    def check_health(self, service: ServiceState) -> bool:
        if service.status != ServiceStatus.RUNNING:
            return False

        url = f"http://localhost:{service.config.port}{service.config.health_endpoint}"
        
        try:
            req = urllib.request.Request(url, method='GET')
            with urllib.request.urlopen(req, timeout=5) as response:
                return response.status < 400
        except:
            return False

    def health_check_all(self):
        for name, service in self.services.items():
            if service.status != ServiceStatus.RUNNING:
                continue

            is_healthy = self.check_health(service)
            service.last_health_check = datetime.now()

            if is_healthy:
                service.health_check_failures = 0
            else:
                service.health_check_failures += 1
                self.logger.warn(name, f"Health check failed ({service.health_check_failures}/3)")

                if service.health_check_failures >= 3:
                    if service.restart_count < self.config.max_restart_attempts:
                        self.logger.warn(name, "Initiating self-heal restart...")
                        service.restart_count += 1
                        service.status = ServiceStatus.RESTARTING
                        
                        if self.restart_service(name):
                            self.logger.info(name, f"{SYM_CHECK} Self-heal restart successful")
                        else:
                            self.logger.error(name, f"{SYM_CROSS} Self-heal restart failed")
                            service.status = ServiceStatus.FAILED
                    else:
                        self.logger.error(name, "Max restart attempts reached")
                        service.status = ServiceStatus.FAILED

    # ========== Start/Stop All ==========

    def start_all(self) -> bool:
        self.logger.info("", "=" * 50)
        self.logger.info("", "Starting GuardRail Platform")
        self.logger.info("", "=" * 50)

        # Check dependencies
        if not self.check_dependencies():
            self.logger.error("", "Dependency check failed")
            return False

        # Start infrastructure
        if not self.start_infrastructure():
            return False

        # Start services in order
        service_order = [
            "identity-service",
            "policy-engine", 
            "movement-ledger",
            "chain-anchor",
            "api-gateway",
            "frontend",
        ]

        for name in service_order:
            if not self.start_service(name):
                self.logger.error("", f"Failed to start {name}")
                self.logger.info("", "Continuing with remaining services...")
                # Don't abort - try to start what we can
            
            # Wait for service to stabilize
            time.sleep(3)

        # Check what's actually running
        running_count = sum(1 for s in self.services.values() if s.status == ServiceStatus.RUNNING)
        
        self.logger.info("", "=" * 50)
        self.logger.info("", f"GuardRail Platform Started ({running_count}/{len(self.services)} services)")
        self.logger.info("", "")
        self.logger.info("", "  API Gateway:  http://localhost:3000")
        self.logger.info("", "  Console:      http://localhost:3010")
        self.logger.info("", "=" * 50)

        return running_count > 0

    def stop_all(self):
        self.logger.info("", "=" * 50)
        self.logger.info("", "Stopping GuardRail Platform")
        self.logger.info("", "=" * 50)

        # Stop services in reverse order
        service_order = [
            "frontend",
            "api-gateway",
            "chain-anchor",
            "movement-ledger",
            "policy-engine",
            "identity-service",
        ]

        for name in service_order:
            self.stop_service(name)

        self.stop_infrastructure()

        self.logger.info("", "=" * 50)
        self.logger.info("", "GuardRail Platform Stopped")
        self.logger.info("", "=" * 50)

    # ========== Status Display ==========

    def print_status(self):
        print()
        print(colored("+" + "=" * 68 + "+", Colors.CYAN))
        print(colored("|" + "          GuardRail Orchestrator Status".center(68) + "|", Colors.CYAN))
        print(colored("+" + "=" * 68 + "+", Colors.CYAN))
        print()
        
        header = f"{'SERVICE':<20} {'STATUS':<12} {'PORT':<8} {'UPTIME':<12} {'RESTARTS':<10}"
        print(colored(header, Colors.BOLD))
        print("-" * 68)

        for name in ["identity-service", "policy-engine", "movement-ledger", 
                     "chain-anchor", "api-gateway", "frontend"]:
            service = self.services.get(name)
            if not service:
                continue

            status_colors = {
                ServiceStatus.RUNNING: Colors.GREEN,
                ServiceStatus.STARTING: Colors.YELLOW,
                ServiceStatus.STOPPED: Colors.WHITE,
                ServiceStatus.UNHEALTHY: Colors.YELLOW,
                ServiceStatus.FAILED: Colors.RED,
                ServiceStatus.RESTARTING: Colors.YELLOW,
            }
            
            status_str = colored(
                service.status.value.ljust(10),
                status_colors.get(service.status, Colors.WHITE)
            )

            print(f"{name:<20} {status_str:<22} {service.config.port:<8} "
                  f"{service.uptime_str():<12} {service.restart_count:<10}")

        print()

    # ========== Daemon Mode ==========

    def run_daemon(self):
        self.logger.info("", "Starting in daemon mode...")

        if not self.start_all():
            self.logger.error("", "Failed to start services")
            return

        # Save PID file
        pid_file = self.project_root / PID_FILE
        with open(pid_file, 'w') as f:
            f.write(str(os.getpid()))

        self.logger.info("", "Daemon running. Press Ctrl+C to stop.")

        try:
            while self.running:
                time.sleep(self.config.health_check_interval_secs)
                self.health_check_all()
        except KeyboardInterrupt:
            pass

        self.logger.info("", "Shutting down...")
        self.stop_all()

        # Remove PID file
        try:
            pid_file.unlink()
        except:
            pass

        self.logger.close()

# ============================================================================
# Installation
# ============================================================================

def install_dependencies(project_root: Path, logger: OrchestratorLogger):
    logger.info("", colored("Installing dependencies...", Colors.CYAN))

    # Frontend
    frontend_dir = project_root / "frontend"
    if frontend_dir.exists():
        logger.info("", colored("Installing frontend dependencies...", Colors.YELLOW))
        try:
            result = subprocess.run(
                ["npm.cmd" if sys.platform == "win32" else "npm", "install"],
                cwd=frontend_dir,
                capture_output=True,
                text=True,
                timeout=300
            )
            if result.returncode == 0:
                logger.info("", colored(f"{SYM_CHECK} Frontend dependencies installed", Colors.GREEN))
            else:
                logger.error("", f"{SYM_CROSS} npm install failed: {result.stderr}")
        except Exception as e:
            logger.error("", f"{SYM_CROSS} Failed: {e}")

    # Backend
    backend_dir = project_root / "backend"
    if backend_dir.exists():
        logger.info("", colored("Building Rust services...", Colors.YELLOW))
        try:
            result = subprocess.run(
                ["cargo", "build", "--release"],
                cwd=backend_dir,
                capture_output=True,
                text=True,
                timeout=600
            )
            if result.returncode == 0:
                logger.info("", colored(f"{SYM_CHECK} Rust services built", Colors.GREEN))
            else:
                logger.error("", f"{SYM_CROSS} cargo build failed: {result.stderr[-500:]}")
        except Exception as e:
            logger.error("", f"{SYM_CROSS} Failed: {e}")

    logger.info("", colored(f"{SYM_CHECK} Installation complete!", Colors.GREEN))

def tail_logs(project_root: Path):
    log_dir = project_root / LOG_DIR
    print(f"Tailing logs from {log_dir}...")
    print("Press Ctrl+C to exit\n")

    if not log_dir.exists():
        print("No logs directory found")
        return

    for log_file in sorted(log_dir.glob("*.log")):
        print(colored(f"=== {log_file.stem} ===", Colors.CYAN))
        try:
            with open(log_file, 'r', encoding='utf-8', errors='ignore') as f:
                lines = f.readlines()
                for line in lines[-20:]:
                    print(line.rstrip())
        except Exception as e:
            print(f"Error reading {log_file}: {e}")
        print()

# ============================================================================
# CLI
# ============================================================================

def print_help():
    print(f"""
{colored('+' + '=' * 68 + '+', Colors.CYAN)}
{colored('|' + f'      GuardRail Orchestrator v{VERSION}'.center(68) + '|', Colors.CYAN)}
{colored('+' + '=' * 68 + '+', Colors.CYAN)}

{colored('USAGE:', Colors.BOLD)}
    guardrail-orchestrator <COMMAND>

{colored('COMMANDS:', Colors.BOLD)}
    start       Start all GuardRail services
    stop        Stop all GuardRail services
    restart     Restart all GuardRail services
    status      Show status of all services
    daemon      Run as daemon with auto-healing
    logs        Tail logs from all services
    install     Install dependencies (npm install, cargo build)
    check       Check system dependencies
    help        Show this help message

{colored('EXAMPLES:', Colors.BOLD)}
    guardrail-orchestrator start      # Start the platform
    guardrail-orchestrator daemon     # Run with auto-healing
    guardrail-orchestrator status     # Check service status

{colored('CONFIG:', Colors.BOLD)}
    Logs directory: logs/
    
{colored('AUTO-HEALING:', Colors.BOLD)}
    - Health checks every 10 seconds
    - Restarts unhealthy services (3 failures = restart)
    - Max 3 restart attempts per service
    - All events logged to logs/orchestrator-YYYYMMDD.log
""")

def find_project_root() -> Path:
    """Find the GuardRail project root by looking for key files."""
    current = Path.cwd()
    
    # Check current directory
    if (current / "backend").exists() and (current / "frontend").exists():
        return current
    
    # Check parent directories
    for parent in current.parents:
        if (parent / "backend").exists() and (parent / "frontend").exists():
            return parent
    
    # Default to current
    return current

def main():
    if len(sys.argv) < 2:
        print_help()
        # Pause on Windows when double-clicked (no args) so user can read output
        if sys.platform == "win32":
            print("\nPress Enter to exit...")
            input()
        sys.exit(0)

    command = sys.argv[1].lower()
    project_root = find_project_root()

    if command in ("help", "--help", "-h"):
        print_help()
        sys.exit(0)

    orchestrator = Orchestrator(project_root)

    if command == "start":
        orchestrator.start_all()
        orchestrator.print_status()

    elif command == "stop":
        orchestrator.stop_all()

    elif command == "restart":
        orchestrator.stop_all()
        time.sleep(2)
        orchestrator.start_all()
        orchestrator.print_status()

    elif command == "status":
        # Quick port check to update status
        for name, service in orchestrator.services.items():
            if orchestrator.check_port(service.config.port):
                if orchestrator.check_health(service):
                    service.status = ServiceStatus.RUNNING
                else:
                    service.status = ServiceStatus.UNHEALTHY
        orchestrator.print_status()

    elif command == "daemon":
        orchestrator.run_daemon()

    elif command == "logs":
        tail_logs(project_root)

    elif command == "install":
        install_dependencies(project_root, orchestrator.logger)

    elif command == "check":
        orchestrator.check_dependencies()

    else:
        print(f"Unknown command: {command}")
        print_help()
        sys.exit(1)

if __name__ == "__main__":
    main()
