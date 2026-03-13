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

    // Determine repository path
    let repo_path = if std::env::args().any(|arg| arg == "-r" || arg == "--repo") {
        args.repo.clone()
    } else {
        ".".to_string()
    };

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
        Commands::Init => {
            commands::registry::init(&repo)?;
        }

        Commands::Registry(registry_cmd) => {
            match registry_cmd {
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
                RegistryCommands::Error { code, message, status } => {
                    commands::registry::create_error(&repo, &code, &message, status)?;
                }
            }
        }

        Commands::Show(args) => {
            commands::show::execute(&repo, &args.path)?;
        }

        Commands::Update(args) => {
            commands::update::execute(&repo, &args.path, &args.update)?;
        }

        Commands::Config { repo, name, email, lang, show, reset } => {
            handle_config_command(
                repo.clone(),
                name.clone(),
                email.clone(),
                lang.clone(),
                show,
                reset,
            )?;
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
        let mut config = Config::default();
        config.first_run = true;
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
            if config.is_first_run() { "Yes".yellow() } else { "No".green() }
        );
        return Ok(());
    }

    let mut config = Config::load()?;
    let mut updated = false;

    if let Some(repo_path) = repo {
        config.set_repo_path(repo_path);
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
        println!("  -r, --repo <PATH>    Set the repository path");
        println!("  -n, --name <NAME>    Set user name for Git commits");
        println!("  -e, --email <EMAIL>  Set user email for Git commits");
        println!("  -l, --lang <LANG>    Set language (zh/en)");
        println!("      --show           Display current configuration");
        println!("      --reset          Reset configuration to defaults");
    }

    Ok(())
}
