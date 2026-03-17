use anyhow::{Context, Result};
use clap::Parser;
use colored::Colorize;

mod cli;
mod commands;
mod config;
mod git;

use cli::{Cli, Commands, RegistryCommands};
use config::Config;
use git::GitRepo;

fn main() -> Result<()> {
    let args = Cli::parse();

    // Initialize colored output for Windows
    #[cfg(windows)]
    colored::control::set_virtual_terminal(true).ok();

    // Handle ShowRepos command separately (doesn't require Git repo)
    if let Commands::ShowRepos = args.command {
        commands::registry::show_repos()?;
        return Ok(());
    }

    // Handle Config command separately (doesn't require Git repo)
    if let Commands::Config { .. } = args.command {
        handle_config_command(
            Some(args.repo.clone()),
            None,
            None,
            None,
            false,
            false,
        )?;
        return Ok(());
    }

    // Handle Init with name separately (creates repo in ~/.arm/<name>)
    if let Commands::Init { ref name } = args.command {
        if name.is_some() {
            commands::registry::init_with_name(name.as_ref().unwrap())?;
            return Ok(());
        }
    }

    // Handle Scan command separately (scans ~/.arm for repos)
    if let Commands::Scan = args.command {
        commands::registry::scan()?;
        return Ok(());
    }

    // Determine repository path with priority:
    // 1. -r parameter (highest priority)
    // 2. local config (.arm/repo.json -> find in global repos.json)
    // 3. current directory (lowest priority)
    let repo_path = determine_repo_path(&args)?;

    // Open or initialize Git repository
    let repo = if git::GitRepo::is_valid(&repo_path) {
        GitRepo::open(&repo_path).context("Failed to open Git repository")?
    } else {
        println!("{} Initializing new Git repository...", "→".yellow());
        git::init_repo(&repo_path).context("Failed to initialize Git repository")?
    };

    if args.verbose {
        println!("{} Working in: {}", "→".dimmed(), repo_path.cyan());
    }

    // Execute command
    match args.command {
        Commands::Config { .. } => {
            // Already handled above
        }
        Commands::Init { name: _ } => {
            // name is None here (handled above when Some)
            commands::registry::init(&repo)?;
        }

        Commands::Scan => {
            // Already handled above
        }

        Commands::Registry(registry_cmd) => match registry_cmd {
            RegistryCommands::New { description } => {
                commands::registry::create_version(&repo, description.as_deref())?;
            }
            RegistryCommands::Category { name, description } => {
                commands::registry::create_category(&repo, &name, description.as_deref())?;
            }
            RegistryCommands::Endpoint { path, description } => {
                commands::registry::create_endpoint(&repo, &path, description.as_deref())?;
            }
            RegistryCommands::Method { path, description } => {
                commands::registry::create_method(&repo, &path, description.as_deref())?;
            }
            RegistryCommands::Error {
                code,
                message,
                status,
            } => {
                commands::registry::create_error(&repo, &code, &message, status)?;
            }
        },

        Commands::Show(args) => {
            commands::show::execute(&repo, &args.path)?;
        }

        Commands::ShowVersion => {
            commands::registry::show_version(&repo)?;
        }

        Commands::Update(args) => {
            commands::update::execute(&repo, &args.path, &args.update)?;
        }

        Commands::Mount { path } => {
            let mount_repo = GitRepo::open(&path).context("Failed to open Git repository")?;
            commands::registry::mount_repo(&mount_repo, &path)?;
        }

        Commands::Check { path } => {
            let check_path = path.unwrap_or_else(|| ".".to_string());
            let check_repo = GitRepo::open(&check_path).context("Failed to open Git repository")?;
            commands::registry::check_repo(&check_repo, &check_path)?;
        }

        Commands::ShowRepos => {
            commands::registry::show_repos()?;
        }
    }

    Ok(())
}

/// Handle the config command
fn handle_config_command(
    repo: Option<String>,
    name: Option<String>,
    email: Option<String>,
    lang: Option<String>,
    show: bool,
    reset: bool,
) -> Result<()> {
    if reset {
        let config = Config {
            first_run: true,
            ..Default::default()
        };
        config.save()?;
        println!("{} Configuration reset to defaults.", "✓".green().bold());
        println!("  Run any command to re-run the setup wizard.");
        return Ok(());
    }

    if show {
        let config = Config::load()?;
        println!("{}", "Current Configuration:".cyan().bold());
        println!();
        println!("  Repository path: {}", config.get_repo_path().cyan());
        println!(
            "  User name:       {}",
            config.user_name.as_deref().unwrap_or("Not set").cyan()
        );
        println!(
            "  User email:      {}",
            config.user_email.as_deref().unwrap_or("Not set").cyan()
        );
        println!(
            "  Language:        {}",
            config.user_language.as_deref().unwrap_or("zh").cyan()
        );
        println!(
            "  First run:       {}",
            if config.is_first_run() {
                "Yes".yellow()
            } else {
                "No".green()
            }
        );
        return Ok(());
    }

    let mut config = Config::load()?;
    let mut updated = false;

    if let Some(repo_or_name) = repo {
        // Check if this is a known repo name in global repos.json
        let final_path = if let Ok(Some(path)) = commands::registry::find_repo_path(&repo_or_name) {
            // It's a known repo name, use the stored path
            // Save the repo name to local config
            commands::registry::save_local_repo_name(&repo_or_name)?;
            println!("  {} Saved repo name '{}' to .arm/repo.json", "→".dimmed(), repo_or_name.cyan());
            path
        } else {
            // Not found in global repos - treat as a direct path
            // Record it in global repos.json
            let path = std::path::Path::new(&repo_or_name);
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&repo_or_name);
            commands::registry::add_repo(name, &repo_or_name)?;
            commands::registry::save_local_repo_name(name)?;
            println!("  {} Recorded repository: {} -> {}", "→".dimmed(), name.cyan(), repo_or_name.dimmed());
            repo_or_name
        };

        config.set_repo_path(final_path);
        updated = true;
    }

    if let Some(user_name) = name {
        config.user_name = Some(user_name);
        updated = true;
    }

    if let Some(user_email) = email {
        config.user_email = Some(user_email);
        updated = true;
    }

    if let Some(language) = lang {
        config.user_language = Some(language);
        updated = true;
    }

    if updated {
        config.save()?;
        println!("{} Configuration updated.", "✓".green().bold());
    } else {
        println!("{}", "Configuration Management".cyan().bold());
        println!();
        println!("Usage:");
        println!("  arm config [OPTIONS]");
        println!();
        println!("Options:");
        println!("  -r, --repo <PATH>    Set the repository path (by name or path)");
        println!("                      If a name is provided, saves to .arm/repo.json");
        println!("  -n, --name <NAME>    Set user name for Git commits");
        println!("  -e, --email <EMAIL>  Set user email for Git commits");
        println!("  -l, --lang <LANG>    Set language (zh/en)");
        println!("      --show           Display current configuration");
        println!("      --reset          Reset configuration to defaults");
    }

    Ok(())
}

/// Determine repository path with priority:
/// 1. -r parameter (highest priority)
/// 2. local config (.arm/repo.json -> find in global repos.json)
/// 3. current directory (lowest priority)
fn determine_repo_path(args: &Cli) -> Result<String> {
    // Priority order:
    // 1. -r parameter (if explicitly provided with non-default value)
    // 2. local config (.arm/repo.json -> find in global repos.json)
    // 3. current directory

    // Check if -r was explicitly provided (not just default value)
    let explicit_repo = std::env::args()
        .collect::<Vec<_>>()
        .windows(2)
        .any(|w| w[0] == "-r" || w[0] == "--repo");

    if explicit_repo {
        let repo = args.repo.clone();
        if repo != "." {
            // First check if it's a known repo name in global repos.json
            if let Ok(Some(path)) = commands::registry::find_repo_path(&repo) {
                return Ok(path);
            }
            // Otherwise treat as a direct path
            return Ok(repo);
        }
    }

    // Try to load from local config (.arm/repo.json)
    if let Ok(Some(repo_name)) = commands::registry::load_local_repo_name() {
        // Try to find the path in global repos.json
        if let Ok(Some(path)) = commands::registry::find_repo_path(&repo_name) {
            return Ok(path);
        }
        // If not found in global repos, treat it as a direct path
        return Ok(repo_name);
    }

    // Default to current directory
    Ok(".".to_string())
}
