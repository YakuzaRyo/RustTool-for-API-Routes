use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "arm",
    about = "API Routes Manager - Manage API documentation and error codes using Git branches",
    version,
    author
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Path to the Git repository
    #[arg(short, long, global = true, default_value = ".")]
    pub repo: String,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize the API management structure (master, api, error branches)
    Init,

    /// Registry commands for managing API structure
    #[command(subcommand)]
    Registry(RegistryCommands),

    /// Show endpoint/error details by path
    Show(ShowArgs),

    /// Update endpoint or error information
    Update(UpdateArgs),

    /// Configure ARM settings
    Config {
        /// Set repository path
        #[arg(short, long)]
        repo: Option<String>,
        /// Set user name for commits
        #[arg(short, long)]
        name: Option<String>,
        /// Set user email for commits
        #[arg(short, long)]
        email: Option<String>,
        /// Set language (zh/en)
        #[arg(short, long)]
        lang: Option<String>,
        /// Show current configuration
        #[arg(long)]
        show: bool,
        /// Reset to default configuration
        #[arg(long)]
        reset: bool,
    },
}

/// Registry commands
#[derive(Subcommand)]
pub enum RegistryCommands {
    /// Create a new version (auto-increment from latest)
    New {
        /// Description of the new version
        #[arg(short, long)]
        description: Option<String>,
    },

    /// Create a new category (e.g., registry category auth)
    Category {
        /// Category name (e.g., auth, users)
        #[arg(value_name = "NAME")]
        name: String,
        /// Description of the category
        #[arg(short, long)]
        description: Option<String>,
    },

    /// Create a new endpoint (e.g., registry endpoint auth/users)
    Endpoint {
        /// Endpoint path (e.g., auth/users)
        #[arg(value_name = "PATH")]
        path: String,
        /// Description of the endpoint
        #[arg(short, long)]
        description: Option<String>,
    },

    /// Create a new method for an endpoint (e.g., registry method auth/users/POST)
    /// Auto-creates endpoint if not exists
    Method {
        /// Method path (e.g., auth/users/POST)
        #[arg(value_name = "PATH")]
        path: String,
        /// Description of the method
        #[arg(short, long)]
        description: Option<String>,
    },

    /// Create a new error code (e.g., registry error E001)
    Error {
        /// Error code (e.g., E001, E002)
        #[arg(value_name = "CODE")]
        code: String,
        /// Error message/description
        #[arg(value_name = "MESSAGE")]
        message: String,
        /// HTTP status code
        #[arg(short, long, default_value = "400")]
        status: u16,
    },
}

#[derive(Args)]
pub struct ShowArgs {
    /// API path (e.g., auth/users/POST) or error code (e.g., error/E001)
    #[arg(value_name = "PATH")]
    pub path: String,
}

#[derive(Args)]
pub struct UpdateArgs {
    /// API path (e.g., auth/users/POST) or error code (e.g., error/E001)
    #[arg(value_name = "PATH")]
    pub path: String,
    /// Update in key:content format
    #[arg(value_name = "KEY:CONTENT")]
    pub update: String,
}
