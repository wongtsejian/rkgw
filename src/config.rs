use anyhow::Result;
use clap::Parser;
use std::io::IsTerminal;
use std::path::PathBuf;
// PathBuf is still needed for tls_cert_path / tls_key_path and expand_tilde().

/// Kiro Gateway - Rust Implementation
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CliArgs {
    /// Server host/bind address
    #[arg(long, env = "SERVER_HOST", default_value = "127.0.0.1")]
    pub host: String,

    /// Server port
    #[arg(short, long, env = "SERVER_PORT", default_value = "8000")]
    pub port: u16,

    /// Proxy API key for client authentication
    #[arg(long, env = "PROXY_API_KEY")]
    pub proxy_api_key: Option<String>,

    /// AWS region for Kiro API
    #[arg(long, env = "KIRO_REGION", default_value = "us-east-1")]
    pub kiro_region: String,

    /// Log level
    #[arg(long, env = "LOG_LEVEL", default_value = "info")]
    pub log_level: String,

    /// Debug mode (off, errors, all)
    #[arg(long, env = "DEBUG_MODE", default_value = "off")]
    pub debug_mode: String,

    /// PostgreSQL database URL for config persistence
    #[arg(long, env = "DATABASE_URL")]
    pub database_url: Option<String>,

    /// Path to TLS certificate file (PEM format)
    #[arg(long, env = "TLS_CERT")]
    pub tls_cert: Option<String>,

    /// Path to TLS private key file (PEM format)
    #[arg(long, env = "TLS_KEY")]
    pub tls_key: Option<String>,

    /// Enable web UI dashboard (served at /_ui/)
    #[arg(long, env = "WEB_UI", default_value = "true")]
    pub web_ui: bool,

    /// Enable monitoring dashboard TUI
    #[arg(long, default_value = "false")]
    pub dashboard: bool,

}

#[derive(Clone, Debug)]
pub struct Config {
    // Server settings
    pub server_host: String,
    pub server_port: u16,

    // Authentication
    pub proxy_api_key: String,

    // Kiro credentials
    pub kiro_region: String,

    // Timeouts
    #[allow(dead_code)]
    pub streaming_timeout: u64,
    pub token_refresh_threshold: u64,
    pub first_token_timeout: u64,

    // HTTP client
    pub http_max_connections: usize,
    pub http_connect_timeout: u64,
    pub http_request_timeout: u64,
    pub http_max_retries: u32,

    // Debug
    pub debug_mode: DebugMode,
    pub log_level: String,

    // Converter settings
    pub tool_description_max_length: usize,
    pub fake_reasoning_enabled: bool,
    pub fake_reasoning_max_tokens: u32,
    #[allow(dead_code)]
    pub fake_reasoning_handling: FakeReasoningHandling,

    // Truncation recovery
    pub truncation_recovery: bool,

    // Dashboard
    pub dashboard: bool,

    // TLS (always on — self-signed cert generated when no custom cert/key provided)
    pub tls_cert_path: Option<PathBuf>,
    pub tls_key_path: Option<PathBuf>,

    // Web UI
    pub web_ui_enabled: bool,

    // Database
    pub database_url: Option<String>,

}

#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub enum FakeReasoningHandling {
    AsReasoningContent, // Extract to reasoning_content field (OpenAI-compatible)
    Remove,             // Remove thinking block completely
    Pass,               // Pass through with original tags
    StripTags,          // Remove tags but keep content
}

#[derive(Clone, Debug, PartialEq)]
pub enum DebugMode {
    Off,
    Errors,
    All,
}

impl Config {
    /// Create a Config with sensible defaults for "setup mode".
    ///
    /// All fields have safe defaults so the gateway can start with no DB config.
    /// The DB overlay (`load_into_config`) fills in real values once setup is complete.
    pub fn with_defaults() -> Self {
        Config {
            server_host: "127.0.0.1".to_string(),
            server_port: 8000,
            proxy_api_key: String::new(),
            kiro_region: "us-east-1".to_string(),
            streaming_timeout: 300,
            token_refresh_threshold: 300,
            first_token_timeout: 15,
            http_max_connections: 20,
            http_connect_timeout: 30,
            http_request_timeout: 300,
            http_max_retries: 3,
            debug_mode: DebugMode::Off,
            log_level: "info".to_string(),
            tool_description_max_length: 10000,
            fake_reasoning_enabled: true,
            fake_reasoning_max_tokens: 4000,
            fake_reasoning_handling: FakeReasoningHandling::AsReasoningContent,
            truncation_recovery: true,
            dashboard: false,
            tls_cert_path: None,
            tls_key_path: None,
            web_ui_enabled: true,
            database_url: None,
        }
    }

    /// Load configuration from bootstrap CLI args only.
    ///
    /// Returns a Config with defaults for all DB-managed fields.
    /// Call `config_db.load_into_config()` afterwards to overlay persisted values.
    pub fn load() -> Result<Self> {
        // Load .env file if it exists
        dotenvy::dotenv().ok();

        // Parse CLI arguments (bootstrap-only subset)
        let args = CliArgs::parse();

        let mut config = Self::with_defaults();

        // Apply CLI / env overrides
        config.server_host = args.host;
        config.server_port = args.port;
        if let Some(key) = args.proxy_api_key {
            config.proxy_api_key = key;
        }
        config.kiro_region = args.kiro_region;
        config.log_level = args.log_level;
        config.debug_mode = match args.debug_mode.to_lowercase().as_str() {
            "errors" => DebugMode::Errors,
            "all" => DebugMode::All,
            _ => DebugMode::Off,
        };
        config.dashboard = args.dashboard;
        config.web_ui_enabled = args.web_ui;
        config.tls_cert_path = args.tls_cert.map(|s| expand_tilde(&s));
        config.tls_key_path = args.tls_key.map(|s| expand_tilde(&s));

        config.database_url = args.database_url;

        Ok(config)
    }

    /// Validate configuration (bootstrap-level checks only).
    ///
    /// DB-managed fields (proxy_api_key, region, etc.) are NOT validated here
    /// because the gateway may be starting in setup mode with no config yet.
    pub fn validate(&self) -> Result<()> {
        if self.dashboard && !std::io::stdout().is_terminal() {
            anyhow::bail!(
                "--dashboard requires a terminal (TTY). Cannot run dashboard mode when stdout is not a terminal."
            );
        }

        // Validate TLS configuration
        if let Some(ref cert) = self.tls_cert_path {
            if self.tls_key_path.is_none() {
                anyhow::bail!(
                    "--tls-cert was provided without --tls-key. Both are required when using custom certificates."
                );
            }
            if !cert.exists() {
                anyhow::bail!("TLS certificate file not found: {}", cert.display());
            }
        }
        if let Some(ref key) = self.tls_key_path {
            if self.tls_cert_path.is_none() {
                anyhow::bail!(
                    "--tls-key was provided without --tls-cert. Both are required when using custom certificates."
                );
            }
            if !key.exists() {
                anyhow::bail!("TLS key file not found: {}", key.display());
            }
        }

        Ok(())
    }

    /// Whether custom TLS certificates were provided (vs auto-generated self-signed).
    pub fn has_custom_tls(&self) -> bool {
        self.tls_cert_path.is_some() && self.tls_key_path.is_some()
    }
}

/// Expand tilde (~) in file paths to user's home directory
pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }
    }
    PathBuf::from(path)
}

/// Parse debug mode from string
#[allow(dead_code)]
pub fn parse_debug_mode(s: &str) -> DebugMode {
    match s.to_lowercase().as_str() {
        "errors" => DebugMode::Errors,
        "all" => DebugMode::All,
        _ => DebugMode::Off,
    }
}

/// Parse fake reasoning handling mode from string
#[cfg(test)]
fn parse_fake_reasoning_handling(s: &str) -> FakeReasoningHandling {
    match s.to_lowercase().as_str() {
        "remove" => FakeReasoningHandling::Remove,
        "pass" => FakeReasoningHandling::Pass,
        "strip_tags" => FakeReasoningHandling::StripTags,
        _ => FakeReasoningHandling::AsReasoningContent, // default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tilde() {
        let path = expand_tilde("~/test/file.txt");
        assert!(path.to_string_lossy().contains("test/file.txt"));
        assert!(!path.to_string_lossy().starts_with("~"));

        let path = expand_tilde("/absolute/path");
        assert_eq!(path, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn test_expand_tilde_relative_path() {
        let path = expand_tilde("relative/path");
        assert_eq!(path, PathBuf::from("relative/path"));
    }

    #[test]
    fn test_expand_tilde_just_tilde() {
        // Just "~" without slash should not expand
        let path = expand_tilde("~");
        assert_eq!(path, PathBuf::from("~"));
    }

    #[test]
    fn test_parse_debug_mode() {
        assert_eq!(parse_debug_mode("off"), DebugMode::Off);
        assert_eq!(parse_debug_mode("errors"), DebugMode::Errors);
        assert_eq!(parse_debug_mode("all"), DebugMode::All);
        assert_eq!(parse_debug_mode("invalid"), DebugMode::Off);
        assert_eq!(parse_debug_mode(""), DebugMode::Off);
    }

    #[test]
    fn test_parse_debug_mode_case_insensitive() {
        assert_eq!(parse_debug_mode("ERRORS"), DebugMode::Errors);
        assert_eq!(parse_debug_mode("Errors"), DebugMode::Errors);
        assert_eq!(parse_debug_mode("ALL"), DebugMode::All);
        assert_eq!(parse_debug_mode("All"), DebugMode::All);
        assert_eq!(parse_debug_mode("OFF"), DebugMode::Off);
    }

    #[test]
    fn test_parse_fake_reasoning_handling() {
        assert_eq!(
            parse_fake_reasoning_handling(""),
            FakeReasoningHandling::AsReasoningContent
        );
        assert_eq!(
            parse_fake_reasoning_handling("remove"),
            FakeReasoningHandling::Remove
        );
        assert_eq!(
            parse_fake_reasoning_handling("pass"),
            FakeReasoningHandling::Pass
        );
        assert_eq!(
            parse_fake_reasoning_handling("strip_tags"),
            FakeReasoningHandling::StripTags
        );
    }

    #[test]
    fn test_parse_fake_reasoning_handling_case_insensitive() {
        assert_eq!(
            parse_fake_reasoning_handling("REMOVE"),
            FakeReasoningHandling::Remove
        );
        assert_eq!(
            parse_fake_reasoning_handling("Remove"),
            FakeReasoningHandling::Remove
        );
        assert_eq!(
            parse_fake_reasoning_handling("PASS"),
            FakeReasoningHandling::Pass
        );
        assert_eq!(
            parse_fake_reasoning_handling("STRIP_TAGS"),
            FakeReasoningHandling::StripTags
        );
    }

    #[test]
    fn test_parse_fake_reasoning_handling_default() {
        assert_eq!(
            parse_fake_reasoning_handling("unknown"),
            FakeReasoningHandling::AsReasoningContent
        );
        assert_eq!(
            parse_fake_reasoning_handling("invalid"),
            FakeReasoningHandling::AsReasoningContent
        );
    }

    #[test]
    fn test_debug_mode_equality() {
        assert_eq!(DebugMode::Off, DebugMode::Off);
        assert_eq!(DebugMode::Errors, DebugMode::Errors);
        assert_eq!(DebugMode::All, DebugMode::All);
        assert_ne!(DebugMode::Off, DebugMode::Errors);
        assert_ne!(DebugMode::Errors, DebugMode::All);
    }

    #[test]
    fn test_fake_reasoning_handling_equality() {
        assert_eq!(
            FakeReasoningHandling::AsReasoningContent,
            FakeReasoningHandling::AsReasoningContent
        );
        assert_eq!(FakeReasoningHandling::Remove, FakeReasoningHandling::Remove);
        assert_eq!(FakeReasoningHandling::Pass, FakeReasoningHandling::Pass);
        assert_eq!(
            FakeReasoningHandling::StripTags,
            FakeReasoningHandling::StripTags
        );
        assert_ne!(FakeReasoningHandling::Remove, FakeReasoningHandling::Pass);
    }

    fn create_test_config(server_host: &str) -> Config {
        Config {
            server_host: server_host.to_string(),
            ..Config::with_defaults()
        }
    }

    #[test]
    fn test_with_defaults() {
        let config = Config::with_defaults();
        assert_eq!(config.server_host, "127.0.0.1");
        assert_eq!(config.server_port, 8000);
        assert_eq!(config.proxy_api_key, "");
        assert_eq!(config.kiro_region, "us-east-1");
        assert_eq!(config.debug_mode, DebugMode::Off);
        assert!(config.fake_reasoning_enabled);
        assert!(config.truncation_recovery);
        assert!(config.tls_cert_path.is_none());
        assert!(config.tls_key_path.is_none());
    }

    #[test]
    fn test_validate_any_host_passes() {
        // TLS is always on, so any host should pass validation
        for host in &["127.0.0.1", "::1", "0.0.0.0", "192.168.1.100", "localhost"] {
            let config = create_test_config(host);
            assert!(config.validate().is_ok(), "validate() failed for host '{}'", host);
        }
    }

    #[test]
    fn test_has_custom_tls() {
        let mut config = Config::with_defaults();
        assert!(!config.has_custom_tls());

        config.tls_cert_path = Some(PathBuf::from("/tmp/cert.pem"));
        assert!(!config.has_custom_tls()); // only cert, no key

        config.tls_key_path = Some(PathBuf::from("/tmp/key.pem"));
        assert!(config.has_custom_tls()); // both provided
    }
}
