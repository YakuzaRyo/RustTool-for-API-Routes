use anyhow::{Context, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Configuration for the ARM tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Default repository path for API management
    pub repo_path: String,
    /// Whether this is the first run
    #[serde(default)]
    pub first_run: bool,
    /// User name for commits
    pub user_name: Option<String>,
    /// User email for commits
    pub user_email: Option<String>,
    /// Language setting (zh/en)
    #[serde(default = "default_language")]
    pub user_language: Option<String>,
}

fn default_language() -> Option<String> {
    Some("zh".to_string())
}

impl Default for Config {
    fn default() -> Self {
        Self {
            repo_path: String::from("."),
            first_run: true,
            user_name: None,
            user_email: None,
            user_language: Some("zh".to_string()),
        }
    }
}

impl Config {
    /// Get the config directory path
    pub fn config_dir() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Failed to get home directory")?;
        let config_dir = home.join(".config").join("arm");
        Ok(config_dir)
    }

    /// Get the config file path
    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    /// Load configuration from file, or create default if not exists
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let content = fs::read_to_string(&config_path).context("Failed to read config file")?;
            let config: Config = toml::from_str(&content).context("Failed to parse config file")?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let config_dir = Self::config_dir()?;
        let config_path = Self::config_path()?;

        // Create config directory if it doesn't exist
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).context("Failed to create config directory")?;
        }

        let content = toml::to_string_pretty(self).context("Failed to serialize config")?;
        fs::write(&config_path, content).context("Failed to write config file")?;

        Ok(())
    }

    /// Check if this is the first run
    pub fn is_first_run(&self) -> bool {
        self.first_run
    }

    /// Mark first run as complete
    #[allow(dead_code)]
    pub fn mark_initialized(&mut self) {
        self.first_run = false;
    }

    /// Set the repository path
    pub fn set_repo_path(&mut self, path: String) {
        self.repo_path = path;
    }

    /// Get the repository path
    pub fn get_repo_path(&self) -> &str {
        &self.repo_path
    }
}

/// Interactive configuration setup
#[allow(dead_code)]
pub fn interactive_setup() -> Result<Config> {
    use colored::Colorize;
    use dialoguer::{Input, theme::ColorfulTheme};

    println!(
        "{}",
        "╔═══════════════════════════════════════════════════════════╗".cyan()
    );
    println!(
        "{}",
        "║      Welcome to API Routes Manager (ARM) Setup            ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "╚═══════════════════════════════════════════════════════════╝".cyan()
    );
    println!();
    println!(
        "{}",
        "This appears to be your first time running ARM.".yellow()
    );
    println!("{}", "Let's set up your configuration.".yellow());
    println!();

    let theme = ColorfulTheme::default();

    // Ask for repository path
    let default_path = std::env::current_dir()
        .map(|p| p.join("api-repo").to_string_lossy().to_string())
        .unwrap_or_else(|_| "./api-repo".to_string());

    let repo_path: String = Input::with_theme(&theme)
        .with_prompt("Enter the repository path for API management")
        .default(default_path)
        .interact_text()
        .context("Failed to get repository path")?;

    // Ask for user name
    let user_name: String = Input::with_theme(&theme)
        .with_prompt("Enter your name (for Git commits)")
        .default(whoami::realname())
        .interact_text()
        .context("Failed to get user name")?;

    // Ask for user email
    let user_email: String = Input::with_theme(&theme)
        .with_prompt("Enter your email (for Git commits)")
        .default(format!("{}@example.com", whoami::username()))
        .interact_text()
        .context("Failed to get user email")?;

    // Ask for language
    let language_options = vec!["zh", "en"];
    let language_idx = dialoguer::Select::with_theme(&theme)
        .with_prompt("Select language / 选择语言")
        .items(&language_options)
        .default(0)
        .interact()
        .context("Failed to get language")?;
    let language = language_options[language_idx].to_string();

    let config = Config {
        repo_path: repo_path.clone(),
        first_run: false,
        user_name: Some(user_name),
        user_email: Some(user_email),
        user_language: Some(language),
    };

    config.save()?;

    println!();
    println!("{}", "✓ Configuration saved successfully!".green().bold());
    println!("  Repository path: {}", repo_path.cyan());
    println!();
    println!(
        "{}",
        "You can change these settings anytime by running:".dimmed()
    );
    println!("{}", "  arm config --repo <path>".dimmed());
    println!();

    Ok(config)
}

/// Get or initialize configuration
#[allow(dead_code)]
pub fn get_or_init_config() -> Result<Config> {
    let mut config = Config::load()?;

    if config.is_first_run() {
        // Check if we're in an interactive terminal
        if atty::is(atty::Stream::Stdout) {
            config = interactive_setup()?;
        } else {
            // Non-interactive mode: use defaults and create default repo path
            let default_path = std::env::current_dir()
                .map(|p| p.join("api-repo").to_string_lossy().to_string())
                .unwrap_or_else(|_| "./api-repo".to_string());

            config.repo_path = default_path;
            config.first_run = false;
            config.user_language = Some("zh".to_string());
            config.save()?;

            eprintln!(
                "{}",
                "Running in non-interactive mode. Using default configuration.".yellow()
            );
            eprintln!("  Repository path: {}", config.repo_path.cyan());
        }
    }

    Ok(config)
}
