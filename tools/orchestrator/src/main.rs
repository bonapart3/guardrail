//! GuardRail Orchestrator
//!
//! A dedicated daemon for monitoring, installing, booting, and self-healing
//! the GuardRail compliance platform.
//!
//! Build: cargo build --release
//! Run:   ./guardrail-orchestrator [command]

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Local, Utc};
use colored::*;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{self, ClearType},
};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

// ============================================================================
// Configuration
// ============================================================================

const VERSION: &str = "1.0.0";
const CONFIG_FILE: &str = "guardrail-orchestrator.toml";
const LOG_DIR: &str = "logs";
const PID_FILE: &str = ".guardrail-orchestrator.pid";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    project_root: String,
    log_level: String,
    health_check_interval_secs: u64,
    restart_delay_secs: u64,
    max_restart_attempts: u32,
    services: Vec<ServiceConfig>,
    infrastructure: InfraConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ServiceConfig {
    name: String,
    port: u16,
    command: String,
    args: Vec<String>,
    working_dir: String,
    health_endpoint: String,
    depends_on: Vec<String>,
    env: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InfraConfig {
    postgres_port: u16,
    redis_port: u16,
    docker_compose_file: String,
}

impl Default for Config {
    fn default() -> Self {
        let mut services = Vec::new();

        // Identity Service
        services.push(ServiceConfig {
            name: "identity-service".to_string(),
            port: 3001,
            command: "cargo".to_string(),
            args: vec!["run".to_string(), "--release".to_string(), "--bin".to_string(), "identity-service".to_string()],
            working_dir: "backend".to_string(),
            health_endpoint: "/health".to_string(),
            depends_on: vec!["postgres".to_string(), "redis".to_string()],
            env: HashMap::from([
                ("DATABASE_URL".to_string(), "postgresql://guardrail:guardrail_dev@localhost:5432/guardrail".to_string()),
                ("REDIS_URL".to_string(), "redis://localhost:6379".to_string()),
                ("PORT".to_string(), "3001".to_string()),
                ("RUST_LOG".to_string(), "info".to_string()),
            ]),
        });

        // Policy Engine
        services.push(ServiceConfig {
            name: "policy-engine".to_string(),
            port: 3002,
            command: "cargo".to_string(),
            args: vec!["run".to_string(), "--release".to_string(), "--bin".to_string(), "policy-engine".to_string()],
            working_dir: "backend".to_string(),
            health_endpoint: "/health".to_string(),
            depends_on: vec!["postgres".to_string(), "redis".to_string()],
            env: HashMap::from([
                ("DATABASE_URL".to_string(), "postgresql://guardrail:guardrail_dev@localhost:5432/guardrail".to_string()),
                ("REDIS_URL".to_string(), "redis://localhost:6379".to_string()),
                ("PORT".to_string(), "3002".to_string()),
                ("RUST_LOG".to_string(), "info".to_string()),
            ]),
        });

        // Movement Ledger
        services.push(ServiceConfig {
            name: "movement-ledger".to_string(),
            port: 3003,
            command: "cargo".to_string(),
            args: vec!["run".to_string(), "--release".to_string(), "--bin".to_string(), "movement-ledger".to_string()],
            working_dir: "backend".to_string(),
            health_endpoint: "/health".to_string(),
            depends_on: vec!["postgres".to_string(), "redis".to_string()],
            env: HashMap::from([
                ("DATABASE_URL".to_string(), "postgresql://guardrail:guardrail_dev@localhost:5432/guardrail".to_string()),
                ("REDIS_URL".to_string(), "redis://localhost:6379".to_string()),
                ("PORT".to_string(), "3003".to_string()),
                ("RUST_LOG".to_string(), "info".to_string()),
            ]),
        });

        // Chain Anchor
        services.push(ServiceConfig {
            name: "chain-anchor".to_string(),
            port: 3004,
            command: "cargo".to_string(),
            args: vec!["run".to_string(), "--release".to_string(), "--bin".to_string(), "chain-anchor".to_string()],
            working_dir: "backend".to_string(),
            health_endpoint: "/health".to_string(),
            depends_on: vec!["postgres".to_string()],
            env: HashMap::from([
                ("DATABASE_URL".to_string(), "postgresql://guardrail:guardrail_dev@localhost:5432/guardrail".to_string()),
                ("PORT".to_string(), "3004".to_string()),
                ("RUST_LOG".to_string(), "info".to_string()),
            ]),
        });

        // API Gateway
        services.push(ServiceConfig {
            name: "api-gateway".to_string(),
            port: 3000,
            command: "cargo".to_string(),
            args: vec!["run".to_string(), "--release".to_string(), "--bin".to_string(), "api-gateway".to_string()],
            working_dir: "backend".to_string(),
            health_endpoint: "/health".to_string(),
            depends_on: vec![
                "identity-service".to_string(),
                "policy-engine".to_string(),
                "movement-ledger".to_string(),
                "chain-anchor".to_string(),
            ],
            env: HashMap::from([
                ("DATABASE_URL".to_string(), "postgresql://guardrail:guardrail_dev@localhost:5432/guardrail".to_string()),
                ("REDIS_URL".to_string(), "redis://localhost:6379".to_string()),
                ("IDENTITY_SERVICE_URL".to_string(), "http://localhost:3001".to_string()),
                ("POLICY_ENGINE_URL".to_string(), "http://localhost:3002".to_string()),
                ("MOVEMENT_LEDGER_URL".to_string(), "http://localhost:3003".to_string()),
                ("CHAIN_ANCHOR_URL".to_string(), "http://localhost:3004".to_string()),
                ("JWT_SECRET".to_string(), "dev_secret_change_in_production".to_string()),
                ("PORT".to_string(), "3000".to_string()),
                ("RUST_LOG".to_string(), "info".to_string()),
            ]),
        });

        // Frontend
        services.push(ServiceConfig {
            name: "frontend".to_string(),
            port: 3010,
            command: "npm".to_string(),
            args: vec!["run".to_string(), "dev".to_string()],
            working_dir: "frontend".to_string(),
            health_endpoint: "/".to_string(),
            depends_on: vec!["api-gateway".to_string()],
            env: HashMap::from([
                ("NEXT_PUBLIC_API_URL".to_string(), "http://localhost:3000".to_string()),
                ("PORT".to_string(), "3010".to_string()),
            ]),
        });

        Self {
            project_root: ".".to_string(),
            log_level: "info".to_string(),
            health_check_interval_secs: 10,
            restart_delay_secs: 5,
            max_restart_attempts: 3,
            services,
            infrastructure: InfraConfig {
                postgres_port: 5432,
                redis_port: 6379,
                docker_compose_file: "infrastructure/docker-compose.yml".to_string(),
            },
        }
    }
}

// ============================================================================
// Service State
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
enum ServiceStatus {
    Stopped,
    Starting,
    Running,
    Unhealthy,
    Failed,
    Restarting,
}

impl std::fmt::Display for ServiceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceStatus::Stopped => write!(f, "STOPPED"),
            ServiceStatus::Starting => write!(f, "STARTING"),
            ServiceStatus::Running => write!(f, "RUNNING"),
            ServiceStatus::Unhealthy => write!(f, "UNHEALTHY"),
            ServiceStatus::Failed => write!(f, "FAILED"),
            ServiceStatus::Restarting => write!(f, "RESTARTING"),
        }
    }
}

#[derive(Debug)]
struct ServiceState {
    config: ServiceConfig,
    status: ServiceStatus,
    process: Option<Child>,
    pid: Option<u32>,
    start_time: Option<Instant>,
    last_health_check: Option<Instant>,
    health_check_failures: u32,
    restart_count: u32,
    log_file: Option<PathBuf>,
}

impl ServiceState {
    fn new(config: ServiceConfig) -> Self {
        Self {
            config,
            status: ServiceStatus::Stopped,
            process: None,
            pid: None,
            start_time: None,
            last_health_check: None,
            health_check_failures: 0,
            restart_count: 0,
            log_file: None,
        }
    }

    fn uptime(&self) -> Option<Duration> {
        self.start_time.map(|t| t.elapsed())
    }

    fn uptime_str(&self) -> String {
        match self.uptime() {
            Some(d) => {
                let secs = d.as_secs();
                if secs < 60 {
                    format!("{}s", secs)
                } else if secs < 3600 {
                    format!("{}m {}s", secs / 60, secs % 60)
                } else {
                    format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
                }
            }
            None => "-".to_string(),
        }
    }
}

// ============================================================================
// Logger
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Ord, PartialOrd, Eq)]
enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

struct Logger {
    level: LogLevel,
    file: Option<Mutex<File>>,
}

impl Logger {
    fn new(level: &str, log_dir: &Path) -> Self {
        let level = match level.to_lowercase().as_str() {
            "debug" => LogLevel::Debug,
            "info" => LogLevel::Info,
            "warn" => LogLevel::Warn,
            "error" => LogLevel::Error,
            _ => LogLevel::Info,
        };

        fs::create_dir_all(log_dir).ok();
        let log_file = log_dir.join(format!(
            "orchestrator-{}.log",
            Utc::now().format("%Y%m%d")
        ));

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
            .ok()
            .map(Mutex::new);

        Self { level, file }
    }

    fn log(&self, level: LogLevel, service: &str, message: &str) {
        if level < self.level {
            return;
        }

        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let level_str = match level {
            LogLevel::Debug => "DEBUG".cyan(),
            LogLevel::Info => "INFO ".green(),
            LogLevel::Warn => "WARN ".yellow(),
            LogLevel::Error => "ERROR".red(),
        };

        let service_str = if service.is_empty() {
            "orchestrator".to_string()
        } else {
            service.to_string()
        };

        let log_line = format!("[{}] {} [{}] {}", timestamp, level_str, service_str, message);
        
        // Print to console
        println!("{}", log_line);

        // Write to file
        if let Some(ref file) = self.file {
            if let Ok(mut f) = file.lock() {
                writeln!(f, "[{}] {:?} [{}] {}", timestamp, level, service_str, message).ok();
            }
        }
    }

    fn debug(&self, service: &str, message: &str) {
        self.log(LogLevel::Debug, service, message);
    }

    fn info(&self, service: &str, message: &str) {
        self.log(LogLevel::Info, service, message);
    }

    fn warn(&self, service: &str, message: &str) {
        self.log(LogLevel::Warn, service, message);
    }

    fn error(&self, service: &str, message: &str) {
        self.log(LogLevel::Error, service, message);
    }
}

// ============================================================================
// Orchestrator
// ============================================================================

struct Orchestrator {
    config: Config,
    services: HashMap<String, ServiceState>,
    logger: Arc<Logger>,
    http_client: Client,
    running: Arc<AtomicBool>,
    project_root: PathBuf,
}

impl Orchestrator {
    fn new(config: Config) -> Self {
        let project_root = PathBuf::from(&config.project_root);
        let log_dir = project_root.join(LOG_DIR);
        let logger = Arc::new(Logger::new(&config.log_level, &log_dir));

        let mut services = HashMap::new();
        for svc_config in &config.services {
            services.insert(svc_config.name.clone(), ServiceState::new(svc_config.clone()));
        }

        let http_client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config,
            services,
            logger,
            http_client,
            running: Arc::new(AtomicBool::new(true)),
            project_root,
        }
    }

    // ========== Dependency Checks ==========

    fn check_dependencies(&self) -> bool {
        self.logger.info("", "Checking system dependencies...");
        let mut all_ok = true;

        // Check Docker
        match Command::new("docker").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout);
                self.logger.info("", &format!("✓ Docker: {}", version.trim()));
            }
            _ => {
                self.logger.error("", "✗ Docker not found. Please install Docker.");
                all_ok = false;
            }
        }

        // Check Docker Compose
        match Command::new("docker").args(["compose", "version"]).output() {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout);
                self.logger.info("", &format!("✓ Docker Compose: {}", version.trim()));
            }
            _ => {
                // Try docker-compose (older version)
                match Command::new("docker-compose").arg("--version").output() {
                    Ok(output) if output.status.success() => {
                        let version = String::from_utf8_lossy(&output.stdout);
                        self.logger.info("", &format!("✓ docker-compose: {}", version.trim()));
                    }
                    _ => {
                        self.logger.error("", "✗ Docker Compose not found.");
                        all_ok = false;
                    }
                }
            }
        }

        // Check Rust
        match Command::new("rustc").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout);
                self.logger.info("", &format!("✓ Rust: {}", version.trim()));
            }
            _ => {
                self.logger.error("", "✗ Rust not found. Please install from rustup.rs");
                all_ok = false;
            }
        }

        // Check Cargo
        match Command::new("cargo").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout);
                self.logger.info("", &format!("✓ Cargo: {}", version.trim()));
            }
            _ => {
                self.logger.error("", "✗ Cargo not found.");
                all_ok = false;
            }
        }

        // Check Node.js
        match Command::new("node").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout);
                self.logger.info("", &format!("✓ Node.js: {}", version.trim()));
            }
            _ => {
                self.logger.warn("", "⚠ Node.js not found. Frontend will not work.");
            }
        }

        // Check npm
        match Command::new("npm").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout);
                self.logger.info("", &format!("✓ npm: {}", version.trim()));
            }
            _ => {
                self.logger.warn("", "⚠ npm not found. Frontend will not work.");
            }
        }

        all_ok
    }

    // ========== Infrastructure ==========

    fn start_infrastructure(&self) -> bool {
        self.logger.info("", "Starting infrastructure (PostgreSQL, Redis)...");

        let compose_file = self.project_root.join(&self.config.infrastructure.docker_compose_file);
        
        if !compose_file.exists() {
            self.logger.error("", &format!("Docker Compose file not found: {:?}", compose_file));
            return false;
        }

        let status = Command::new("docker")
            .args(["compose", "-f"])
            .arg(&compose_file)
            .args(["up", "-d", "postgres", "redis"])
            .current_dir(&self.project_root)
            .status();

        match status {
            Ok(s) if s.success() => {
                self.logger.info("", "Infrastructure started successfully");
                
                // Wait for services to be ready
                self.logger.info("", "Waiting for PostgreSQL to be ready...");
                for i in 0..30 {
                    if self.check_port(self.config.infrastructure.postgres_port) {
                        self.logger.info("", "PostgreSQL is ready");
                        break;
                    }
                    if i == 29 {
                        self.logger.error("", "PostgreSQL failed to start within 30 seconds");
                        return false;
                    }
                    thread::sleep(Duration::from_secs(1));
                }

                self.logger.info("", "Waiting for Redis to be ready...");
                for i in 0..30 {
                    if self.check_port(self.config.infrastructure.redis_port) {
                        self.logger.info("", "Redis is ready");
                        break;
                    }
                    if i == 29 {
                        self.logger.error("", "Redis failed to start within 30 seconds");
                        return false;
                    }
                    thread::sleep(Duration::from_secs(1));
                }

                true
            }
            Ok(_) => {
                self.logger.error("", "Failed to start infrastructure");
                false
            }
            Err(e) => {
                self.logger.error("", &format!("Failed to run docker compose: {}", e));
                false
            }
        }
    }

    fn stop_infrastructure(&self) {
        self.logger.info("", "Stopping infrastructure...");

        let compose_file = self.project_root.join(&self.config.infrastructure.docker_compose_file);
        
        let _ = Command::new("docker")
            .args(["compose", "-f"])
            .arg(&compose_file)
            .args(["down"])
            .current_dir(&self.project_root)
            .status();

        self.logger.info("", "Infrastructure stopped");
    }

    fn check_port(&self, port: u16) -> bool {
        use std::net::TcpStream;
        TcpStream::connect(format!("127.0.0.1:{}", port)).is_ok()
    }

    // ========== Service Management ==========

    fn start_service(&mut self, name: &str) -> bool {
        let service = match self.services.get_mut(name) {
            Some(s) => s,
            None => {
                self.logger.error("", &format!("Service '{}' not found", name));
                return false;
            }
        };

        if matches!(service.status, ServiceStatus::Running | ServiceStatus::Starting) {
            self.logger.warn(name, "Service is already running or starting");
            return true;
        }

        // Check dependencies
        for dep in &service.config.depends_on.clone() {
            if dep == "postgres" || dep == "redis" {
                continue; // Infrastructure deps handled separately
            }
            if let Some(dep_svc) = self.services.get(dep) {
                if dep_svc.status != ServiceStatus::Running {
                    self.logger.warn(name, &format!("Dependency '{}' is not running", dep));
                }
            }
        }

        service.status = ServiceStatus::Starting;
        self.logger.info(name, "Starting service...");

        // Create log file
        let log_dir = self.project_root.join(LOG_DIR);
        fs::create_dir_all(&log_dir).ok();
        let log_file = log_dir.join(format!("{}.log", name));
        service.log_file = Some(log_file.clone());

        let stdout_file = File::create(&log_file).ok();
        let stderr_file = File::options().append(true).open(&log_file).ok();

        // Build command
        let working_dir = self.project_root.join(&service.config.working_dir);
        
        let mut cmd = Command::new(&service.config.command);
        cmd.args(&service.config.args)
            .current_dir(&working_dir)
            .envs(&service.config.env);

        if let Some(f) = stdout_file {
            cmd.stdout(f);
        }
        if let Some(f) = stderr_file {
            cmd.stderr(f);
        }

        match cmd.spawn() {
            Ok(child) => {
                let pid = child.id();
                service.process = Some(child);
                service.pid = Some(pid);
                service.start_time = Some(Instant::now());
                service.health_check_failures = 0;
                
                self.logger.info(name, &format!("Started with PID {}", pid));

                // Wait a bit and check if still running
                thread::sleep(Duration::from_secs(2));
                
                if let Some(ref mut proc) = service.process {
                    match proc.try_wait() {
                        Ok(Some(status)) => {
                            self.logger.error(name, &format!("Process exited immediately with status: {}", status));
                            service.status = ServiceStatus::Failed;
                            return false;
                        }
                        Ok(None) => {
                            service.status = ServiceStatus::Running;
                            self.logger.info(name, "Service is running");
                            return true;
                        }
                        Err(e) => {
                            self.logger.error(name, &format!("Failed to check process status: {}", e));
                        }
                    }
                }

                true
            }
            Err(e) => {
                self.logger.error(name, &format!("Failed to start: {}", e));
                service.status = ServiceStatus::Failed;
                false
            }
        }
    }

    fn stop_service(&mut self, name: &str) -> bool {
        let service = match self.services.get_mut(name) {
            Some(s) => s,
            None => return false,
        };

        if service.status == ServiceStatus::Stopped {
            return true;
        }

        self.logger.info(name, "Stopping service...");

        if let Some(ref mut process) = service.process {
            // Try graceful shutdown first
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                if let Some(pid) = service.pid {
                    unsafe {
                        libc::kill(pid as i32, libc::SIGTERM);
                    }
                }
            }
            
            #[cfg(windows)]
            {
                let _ = process.kill();
            }

            // Wait for graceful shutdown
            for _ in 0..10 {
                match process.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) => thread::sleep(Duration::from_millis(500)),
                    Err(_) => break,
                }
            }

            // Force kill if still running
            let _ = process.kill();
            let _ = process.wait();
        }

        service.process = None;
        service.pid = None;
        service.status = ServiceStatus::Stopped;
        service.start_time = None;

        self.logger.info(name, "Service stopped");
        true
    }

    fn restart_service(&mut self, name: &str) -> bool {
        self.stop_service(name);
        thread::sleep(Duration::from_secs(self.config.restart_delay_secs));
        self.start_service(name)
    }

    // ========== Health Checks ==========

    fn check_health(&self, service: &ServiceState) -> bool {
        if service.status != ServiceStatus::Running {
            return false;
        }

        let url = format!(
            "http://localhost:{}{}",
            service.config.port,
            service.config.health_endpoint
        );

        match self.http_client.get(&url).send() {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    fn health_check_all(&mut self) {
        let service_names: Vec<String> = self.services.keys().cloned().collect();

        for name in service_names {
            let (is_healthy, should_restart) = {
                let service = self.services.get(&name).unwrap();
                
                if service.status != ServiceStatus::Running {
                    continue;
                }

                let healthy = self.check_health(service);
                let failures = if healthy { 0 } else { service.health_check_failures + 1 };
                let should_restart = failures >= 3 && service.restart_count < self.config.max_restart_attempts;

                (healthy, should_restart)
            };

            let service = self.services.get_mut(&name).unwrap();
            
            if is_healthy {
                service.health_check_failures = 0;
                service.last_health_check = Some(Instant::now());
            } else {
                service.health_check_failures += 1;
                self.logger.warn(&name, &format!(
                    "Health check failed ({}/3)",
                    service.health_check_failures
                ));

                if should_restart {
                    self.logger.warn(&name, "Initiating self-heal restart...");
                    service.restart_count += 1;
                    service.status = ServiceStatus::Restarting;
                }
            }
        }

        // Handle restarts
        let to_restart: Vec<String> = self.services.iter()
            .filter(|(_, s)| s.status == ServiceStatus::Restarting)
            .map(|(n, _)| n.clone())
            .collect();

        for name in to_restart {
            if self.restart_service(&name) {
                self.logger.info(&name, "Self-heal restart successful");
            } else {
                self.logger.error(&name, "Self-heal restart failed");
                if let Some(service) = self.services.get_mut(&name) {
                    service.status = ServiceStatus::Failed;
                }
            }
        }
    }

    // ========== Start/Stop All ==========

    fn start_all(&mut self) -> bool {
        self.logger.info("", "=== Starting GuardRail Platform ===");

        // Check dependencies
        if !self.check_dependencies() {
            self.logger.error("", "Dependency check failed. Please install missing components.");
            return false;
        }

        // Start infrastructure
        if !self.start_infrastructure() {
            return false;
        }

        // Start services in dependency order
        let order = vec![
            "identity-service",
            "policy-engine",
            "movement-ledger",
            "chain-anchor",
            "api-gateway",
            "frontend",
        ];

        for name in order {
            if !self.start_service(name) {
                self.logger.error("", &format!("Failed to start {}, aborting", name));
                return false;
            }
            
            // Wait for service to be healthy
            thread::sleep(Duration::from_secs(3));
        }

        self.logger.info("", "=== GuardRail Platform Started ===");
        self.logger.info("", "");
        self.logger.info("", "  API Gateway:  http://localhost:3000");
        self.logger.info("", "  Console:      http://localhost:3010");
        self.logger.info("", "");

        true
    }

    fn stop_all(&mut self) {
        self.logger.info("", "=== Stopping GuardRail Platform ===");

        // Stop services in reverse order
        let order = vec![
            "frontend",
            "api-gateway",
            "chain-anchor",
            "movement-ledger",
            "policy-engine",
            "identity-service",
        ];

        for name in order {
            self.stop_service(name);
        }

        self.stop_infrastructure();

        self.logger.info("", "=== GuardRail Platform Stopped ===");
    }

    // ========== Status Display ==========

    fn print_status(&self) {
        println!();
        println!("{}", "╔══════════════════════════════════════════════════════════════╗".cyan());
        println!("{}", "║              GuardRail Orchestrator Status                   ║".cyan());
        println!("{}", "╚══════════════════════════════════════════════════════════════╝".cyan());
        println!();
        println!("{:<20} {:<12} {:<8} {:<10} {:<10}", 
            "SERVICE".bold(), 
            "STATUS".bold(), 
            "PORT".bold(), 
            "UPTIME".bold(), 
            "RESTARTS".bold()
        );
        println!("{}", "─".repeat(60));

        for name in ["identity-service", "policy-engine", "movement-ledger", "chain-anchor", "api-gateway", "frontend"] {
            if let Some(service) = self.services.get(name) {
                let status_str = match service.status {
                    ServiceStatus::Running => "RUNNING".green(),
                    ServiceStatus::Starting => "STARTING".yellow(),
                    ServiceStatus::Stopped => "STOPPED".white(),
                    ServiceStatus::Unhealthy => "UNHEALTHY".yellow(),
                    ServiceStatus::Failed => "FAILED".red(),
                    ServiceStatus::Restarting => "RESTARTING".yellow(),
                };

                println!("{:<20} {:<12} {:<8} {:<10} {:<10}",
                    name,
                    status_str,
                    service.config.port,
                    service.uptime_str(),
                    service.restart_count,
                );
            }
        }

        println!();
    }

    // ========== Daemon Mode ==========

    fn run_daemon(&mut self) {
        self.logger.info("", "Starting in daemon mode...");

        // Start all services
        if !self.start_all() {
            self.logger.error("", "Failed to start services");
            return;
        }

        // Save PID file
        if let Ok(mut file) = File::create(PID_FILE) {
            writeln!(file, "{}", std::process::id()).ok();
        }

        let running = self.running.clone();

        // Set up signal handler
        ctrlc::set_handler(move || {
            running.store(false, Ordering::SeqCst);
        }).expect("Error setting Ctrl-C handler");

        self.logger.info("", "Daemon running. Press Ctrl+C to stop.");

        // Main loop
        while self.running.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_secs(self.config.health_check_interval_secs));
            self.health_check_all();
        }

        // Cleanup
        self.logger.info("", "Shutting down...");
        self.stop_all();

        // Remove PID file
        fs::remove_file(PID_FILE).ok();
    }
}

// ============================================================================
// CLI
// ============================================================================

fn print_help() {
    println!("{}", "
╔═══════════════════════════════════════════════════════════════════╗
║              GuardRail Orchestrator v{}                         ║
╚═══════════════════════════════════════════════════════════════════╝

USAGE:
    guardrail-orchestrator <COMMAND>

COMMANDS:
    start       Start all GuardRail services
    stop        Stop all GuardRail services
    restart     Restart all GuardRail services
    status      Show status of all services
    daemon      Run as daemon with auto-healing
    logs        Tail logs from all services
    install     Install dependencies (npm install, cargo build)
    check       Check system dependencies
    init        Generate default config file
    help        Show this help message

EXAMPLES:
    guardrail-orchestrator start      # Start the platform
    guardrail-orchestrator daemon     # Run with auto-healing
    guardrail-orchestrator status     # Check service status

CONFIG:
    Config file: guardrail-orchestrator.toml
    Logs directory: logs/
".trim(), VERSION);
}

fn load_config() -> Config {
    if Path::new(CONFIG_FILE).exists() {
        match fs::read_to_string(CONFIG_FILE) {
            Ok(content) => {
                match toml::from_str(&content) {
                    Ok(config) => return config,
                    Err(e) => eprintln!("Failed to parse config: {}", e),
                }
            }
            Err(e) => eprintln!("Failed to read config: {}", e),
        }
    }
    Config::default()
}

fn save_config(config: &Config) {
    match toml::to_string_pretty(config) {
        Ok(content) => {
            if let Err(e) = fs::write(CONFIG_FILE, content) {
                eprintln!("Failed to write config: {}", e);
            } else {
                println!("Config saved to {}", CONFIG_FILE);
            }
        }
        Err(e) => eprintln!("Failed to serialize config: {}", e),
    }
}

fn tail_logs(project_root: &Path) {
    let log_dir = project_root.join(LOG_DIR);
    println!("Tailing logs from {:?}...", log_dir);
    println!("Press Ctrl+C to exit\n");

    // Get all log files
    let files: Vec<_> = fs::read_dir(&log_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().map(|s| s == "log").unwrap_or(false))
        .collect();

    if files.is_empty() {
        println!("No log files found");
        return;
    }

    // Simple tail implementation
    for entry in files {
        let path = entry.path();
        let name = path.file_stem().unwrap_or_default().to_string_lossy();
        
        if let Ok(content) = fs::read_to_string(&path) {
            let lines: Vec<_> = content.lines().collect();
            let start = if lines.len() > 20 { lines.len() - 20 } else { 0 };
            
            println!("=== {} ===", name.cyan());
            for line in &lines[start..] {
                println!("{}", line);
            }
            println!();
        }
    }
}

fn install_dependencies(project_root: &Path) {
    println!("{}", "Installing dependencies...".cyan());

    // Install frontend deps
    let frontend_dir = project_root.join("frontend");
    if frontend_dir.exists() {
        println!("\n{}", "Installing frontend dependencies...".yellow());
        let status = Command::new("npm")
            .arg("install")
            .current_dir(&frontend_dir)
            .status();
        
        match status {
            Ok(s) if s.success() => println!("{}", "✓ Frontend dependencies installed".green()),
            _ => println!("{}", "✗ Failed to install frontend dependencies".red()),
        }
    }

    // Build Rust
    let backend_dir = project_root.join("backend");
    if backend_dir.exists() {
        println!("\n{}", "Building Rust services...".yellow());
        let status = Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(&backend_dir)
            .status();
        
        match status {
            Ok(s) if s.success() => println!("{}", "✓ Rust services built".green()),
            _ => println!("{}", "✗ Failed to build Rust services".red()),
        }
    }

    println!("\n{}", "Installation complete!".green());
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("help");

    let config = load_config();
    let mut orchestrator = Orchestrator::new(config.clone());

    match command {
        "start" => {
            orchestrator.start_all();
            orchestrator.print_status();
        }
        "stop" => {
            orchestrator.stop_all();
        }
        "restart" => {
            orchestrator.stop_all();
            thread::sleep(Duration::from_secs(2));
            orchestrator.start_all();
            orchestrator.print_status();
        }
        "status" => {
            // Quick health check
            for name in ["identity-service", "policy-engine", "movement-ledger", "chain-anchor", "api-gateway", "frontend"] {
                if let Some(service) = orchestrator.services.get_mut(name) {
                    if orchestrator.check_port(service.config.port) {
                        service.status = if orchestrator.check_health(service) {
                            ServiceStatus::Running
                        } else {
                            ServiceStatus::Unhealthy
                        };
                    }
                }
            }
            orchestrator.print_status();
        }
        "daemon" => {
            orchestrator.run_daemon();
        }
        "logs" => {
            tail_logs(&orchestrator.project_root);
        }
        "install" => {
            install_dependencies(&orchestrator.project_root);
        }
        "check" => {
            orchestrator.check_dependencies();
        }
        "init" => {
            save_config(&Config::default());
        }
        "help" | "--help" | "-h" => {
            print_help();
        }
        _ => {
            eprintln!("Unknown command: {}", command);
            print_help();
            std::process::exit(1);
        }
    }
}
