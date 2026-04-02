use serde::Deserialize;
use std::path::PathBuf;

/// Project-level config loaded from `aft.toml` or `~/.config/aft/config.toml`.
#[derive(Debug, Deserialize, Default)]
pub struct AftConfig {
    /// If set, only these languages are active (must be compiled-in).
    pub languages: Option<Vec<String>>,
    /// Project root override.
    pub root: Option<PathBuf>,
    /// Framework-specific entry point configuration.
    #[serde(default)]
    pub entry_points: EntryPointsConfig,
}

/// Framework lifecycle methods to treat as entry points.
/// Configured per-project in aft.toml because frameworks (LWC, React, Angular, Vue)
/// are project-specific, not language-specific.
#[derive(Debug, Deserialize, Default, Clone)]
pub struct EntryPointsConfig {
    /// Method names that are entry points (e.g. connectedCallback for LWC).
    #[serde(default)]
    pub lifecycle: Vec<String>,
}

impl AftConfig {
    /// Load config from CWD/aft.toml, then ~/.config/aft/config.toml.
    pub fn load() -> Self {
        let mut paths = vec![PathBuf::from("aft.toml")];
        if let Some(config_dir) = dirs_config_dir() {
            paths.push(config_dir.join("aft/config.toml"));
        }
        for p in &paths {
            if let Ok(content) = std::fs::read_to_string(p) {
                if let Ok(config) = toml::from_str(&content) {
                    return config;
                }
            }
        }
        Self::default()
    }
}

/// Cross-platform config directory (no external dep needed).
fn dirs_config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        std::env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join("Library/Application Support"))
    }
    #[cfg(target_os = "linux")]
    {
        std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".config")))
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA").ok().map(PathBuf::from)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        None
    }
}

/// Runtime configuration for the aft process.
///
/// Holds project-scoped settings and tuning knobs. Values are set at startup
/// and remain immutable for the lifetime of the process.
#[derive(Debug, Clone)]
pub struct Config {
    /// Root directory of the project being analyzed. `None` if not scoped.
    pub project_root: Option<PathBuf>,
    /// How many levels of call-graph edges to follow during validation (default: 1).
    pub validation_depth: u32,
    /// Hours before a checkpoint expires and is eligible for cleanup (default: 24).
    pub checkpoint_ttl_hours: u32,
    /// Maximum depth for recursive symbol resolution (default: 10).
    pub max_symbol_depth: u32,
    /// Seconds before killing a formatter subprocess (default: 10).
    pub formatter_timeout_secs: u32,
    /// Seconds before killing a type-checker subprocess (default: 30).
    pub type_checker_timeout_secs: u32,
    /// Whether to auto-format files after edits (default: true).
    pub format_on_edit: bool,
    /// Whether to auto-validate files after edits (default: false).
    /// When "syntax", only tree-sitter parse check. When "full", runs type checker.
    pub validate_on_edit: Option<String>,
    /// Per-language formatter overrides. Keys: "typescript", "python", "rust", "go".
    /// Values: "biome", "prettier", "deno", "ruff", "black", "rustfmt", "goimports", "gofmt", "none".
    pub formatter: std::collections::HashMap<String, String>,
    /// Per-language type checker overrides. Keys: "typescript", "python", "rust", "go".
    /// Values: "tsc", "biome", "pyright", "ruff", "cargo", "go", "staticcheck", "none".
    pub checker: std::collections::HashMap<String, String>,
    /// Whether to restrict file operations to within `project_root` (default: false).
    /// When true, write-capable commands reject paths outside the project root.
    pub restrict_to_project_root: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            project_root: None,
            validation_depth: 1,
            checkpoint_ttl_hours: 24,
            max_symbol_depth: 10,
            formatter_timeout_secs: 10,
            type_checker_timeout_secs: 30,
            format_on_edit: true,
            validate_on_edit: None,
            formatter: std::collections::HashMap::new(),
            checker: std::collections::HashMap::new(),
            restrict_to_project_root: false,
        }
    }
}
