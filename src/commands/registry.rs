use anyhow::{Context, Result, bail};
use dirs;
use chrono::Local;
use colored::Colorize;
use once_cell::sync::Lazy;
use rand::Rng;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

use crate::git::{self, GitRepo};

/// repos.json file path - program level (arm.exe同级目录)
const GLOBAL_REPOS_FILE: &str = "repos.json";
/// Local repo.json file path - project level (.arm/repo.json)
const LOCAL_REPO_FILE: &str = ".arm/repo.json";

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct RepoList {
    pub repos: Vec<RepoEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RepoEntry {
    pub name: String,
    pub path: String,
}

/// Get the global repos.json file path (same directory as the executable)
fn get_global_repos_path() -> std::path::PathBuf {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            return exe_dir.join(GLOBAL_REPOS_FILE);
        }
    }
    std::path::PathBuf::from(GLOBAL_REPOS_FILE)
}

/// Load repos list from global directory
fn load_repos() -> Result<RepoList> {
    let global_path = get_global_repos_path();
    if let Ok(content) = fs::read_to_string(&global_path) {
        let repos: RepoList = serde_json::from_str(&content).unwrap_or_default();
        Ok(repos)
    } else {
        Ok(RepoList::default())
    }
}

/// Save repos list to global directory
fn save_repos(repos: &RepoList) -> Result<()> {
    let global_path = get_global_repos_path();
    let content = serde_json::to_string_pretty(repos)?;
    fs::write(global_path, content)?;
    Ok(())
}

/// Add a repo with name and path to the list (deduplicates by name)
pub fn add_repo(name: &str, path: &str) -> Result<()> {
    let mut repos = load_repos()?;
    let name_str = name.to_string();
    let path_str = path.to_string();

    // Check if already exists, update path if so
    if let Some(existing) = repos.repos.iter_mut().find(|r| r.name == name_str) {
        existing.path = path_str;
    } else {
        repos.repos.push(RepoEntry {
            name: name_str,
            path: path_str,
        });
    }
    save_repos(&repos)?;
    Ok(())
}

/// Show all recorded repository names
pub fn show_repos() -> Result<()> {
    let repos = load_repos()?;
    if repos.repos.is_empty() {
        println!("{}", "No repositories recorded.".yellow());
    } else {
        println!("{}", "Recorded Repositories:".cyan().bold());
        println!();
        for entry in &repos.repos {
            println!("  {} -> {}", entry.name.cyan(), entry.path.dimmed());
        }
    }
    Ok(())
}

/// Show current version info and all endpoints
pub fn show_version(repo: &GitRepo) -> Result<()> {
    let latest = get_latest_version(repo)?
        .context("No API version found. Create one with 'arm registry new'")?;

    // Load VERSION.md to get version description
    repo.checkout("master")?;
    let version_content = fs::read_to_string("VERSION.md").unwrap_or_default();

    println!("{}", "=".repeat(50));
    println!();
    println!("{} Current Version: {}", "→".yellow(), latest.cyan().bold());
    println!();

    // Parse description from VERSION.md
    let description = version_content
        .lines()
        .skip_while(|l| !l.contains("## Current Version"))
        .skip(1)
        .next()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .unwrap_or("No description");

    println!("Description: {}", description);
    println!();

    // Load mapping to show all endpoints
    let mapping = load_mapping(repo)?;

    // Filter entries belonging to the current version
    let version_prefix = format!("{}/", latest);
    let endpoints: Vec<_> = mapping
        .entries
        .iter()
        .filter(|(path, _)| path.starts_with(&version_prefix))
        .collect();

    if endpoints.is_empty() {
        println!("{}", "No endpoints found in this version.".yellow());
    } else {
        println!("{} Endpoints:", "→".yellow());
        println!();

        for (path, entry) in &endpoints {
            let entry_type = entry.entry_type.as_str();
            match entry_type {
                "endpoint" => {
                    // v1/category/resource -> show as path
                    let display = path.replace(&format!("{}/", latest), "");
                    println!("  {}", display.yellow());
                }
                "method" => {
                    // v1/category/resource/GET -> show as path/method
                    let parts: Vec<&str> = path.split('/').collect();
                    if parts.len() >= 4 {
                        let method = parts[3].green();
                        let display = parts[1..parts.len()-1].join("/");
                        println!("  {}/{}", display.yellow(), method);
                    }
                }
                _ => {}
            }
        }
    }

    // Return to original branch
    repo.checkout(&latest)?;

    Ok(())
}

/// Find repo path by name in global repos.json
pub fn find_repo_path(name: &str) -> Result<Option<String>> {
    let repos = load_repos()?;
    if let Some(entry) = repos.repos.iter().find(|r| r.name == name) {
        Ok(Some(entry.path.clone()))
    } else {
        Ok(None)
    }
}

/// Load local repo name from .arm/repo.json
pub fn load_local_repo_name() -> Result<Option<String>> {
    if let Ok(content) = fs::read_to_string(LOCAL_REPO_FILE) {
        let repo_name: serde_json::Value = serde_json::from_str(&content)?;
        if let Some(name) = repo_name.get("repo_name").and_then(|v| v.as_str()) {
            return Ok(Some(name.to_string()));
        }
    }
    Ok(None)
}

/// Save local repo name to .arm/repo.json
pub fn save_local_repo_name(name: &str) -> Result<()> {
    let local_path = std::path::Path::new(LOCAL_REPO_FILE);
    if let Some(parent) = local_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::json!({ "repo_name": name });
    fs::write(LOCAL_REPO_FILE, serde_json::to_string_pretty(&content)?)?;
    Ok(())
}

/// 全局映射缓存：仓库路径 -> (mapping, 是否有效)
static MAPPING_CACHE: Lazy<Mutex<Option<(String, PathMapping)>>> =
    Lazy::new(|| Mutex::new(None));

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappingEntry {
    pub path: String,
    pub branch: String,
    pub entry_type: String,
    pub parent: Option<String>,
    pub created: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PathMapping {
    pub entries: HashMap<String, MappingEntry>,
    pub branches: HashMap<String, String>,
    #[serde(skip)]
    pub is_dirty: bool,
}

impl Default for PathMapping {
    fn default() -> Self {
        Self::new()
    }
}

impl PathMapping {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            branches: HashMap::new(),
            is_dirty: false,
        }
    }

    pub fn add(&mut self, path: &str, branch: &str, entry_type: &str, parent: Option<&str>) {
        let entry = MappingEntry {
            path: path.to_string(),
            branch: branch.to_string(),
            entry_type: entry_type.to_string(),
            parent: parent.map(|s| s.to_string()),
            created: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        };
        self.entries.insert(path.to_string(), entry);
        self.branches.insert(branch.to_string(), path.to_string());
        self.is_dirty = true;
    }

    pub fn get_by_path(&self, path: &str) -> Option<&MappingEntry> {
        self.entries.get(path)
    }
}

const MAPPING_PATH: &str = ".arm/mapping.json";

/// 从磁盘加载 mapping（不走缓存）
fn load_mapping_from_disk(repo: &GitRepo) -> Result<PathMapping> {
    let current_branch = repo.current_branch()?;
    if current_branch != "master" {
        repo.checkout("master")?;
    }

    let mapping = if let Ok(content) = fs::read_to_string(MAPPING_PATH) {
        serde_json::from_str(&content).unwrap_or_else(|_| PathMapping::new())
    } else {
        PathMapping::new()
    };

    if current_branch != "master" {
        repo.checkout(&current_branch)?;
    }

    Ok(mapping)
}

/// 带缓存的 mapping 加载
pub fn load_mapping(repo: &GitRepo) -> Result<PathMapping> {
    let repo_path = repo.path().to_string_lossy().to_string();

    // 检查缓存是否有效（同一仓库且未脏）
    {
        let cache = MAPPING_CACHE.lock().unwrap();
        if let Some((ref path, ref mapping)) = *cache {
            if path == &repo_path && !mapping.is_dirty {
                return Ok(mapping.clone());
            }
        }
    }

    // 缓存无效，从磁盘加载
    let mapping = load_mapping_from_disk(repo)?;

    // 更新缓存
    let mut cache = MAPPING_CACHE.lock().unwrap();
    *cache = Some((repo_path, mapping.clone()));

    Ok(mapping)
}

fn generate_random_code() -> String {
    let mut rng = rand::thread_rng();
    let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyz0123456789".chars().collect();
    (0..8)
        .map(|_| chars[rng.gen_range(0..chars.len())])
        .collect()
}

fn save_mapping(repo: &GitRepo, mapping: &PathMapping) -> Result<()> {
    // Save mapping on master branch
    let current_branch = repo.current_branch()?;

    // Checkout to master
    if current_branch != "master" {
        repo.checkout("master")?;
    }

    fs::create_dir_all(".arm")?;
    let content = serde_json::to_string_pretty(mapping)?;
    fs::write(MAPPING_PATH, content)?;
    repo.commit("[MAPPING] Update path mapping")?;

    // Switch back to original branch
    if current_branch != "master" {
        repo.checkout(&current_branch)?;
    }

    // 更新缓存
    let repo_path = repo.path().to_string_lossy().to_string();
    let mut cache = MAPPING_CACHE.lock().unwrap();
    let mut mapping_clone = mapping.clone();
    mapping_clone.is_dirty = false;
    *cache = Some((repo_path, mapping_clone));

    Ok(())
}

pub fn get_latest_version(repo: &GitRepo) -> Result<Option<String>> {
    let branches = repo.list_branches()?;
    let version_regex = Regex::new(r"^v(\d+)$").unwrap();
    let mut max_version = 0;
    let mut latest_version = None;
    for (name, _) in branches {
        if let Some(caps) = version_regex.captures(&name) {
            let num: u32 = caps[1].parse().unwrap_or(0);
            if num > max_version {
                max_version = num;
                latest_version = Some(name);
            }
        }
    }
    Ok(latest_version)
}

pub fn init(repo: &GitRepo) -> Result<()> {
    println!("{}", "Initializing API Routes Manager...".cyan().bold());
    if !repo.branch_exists("master")? {
        bail!("No master branch found. Please initialize git first.");
    }
    repo.checkout("master")?;
    let version_content = format!(
        r#"# API Versions

## Current Version
None

## Version History

## Last Updated
{}
"#,
        Local::now().format("%Y-%m-%d %H:%M:%S")
    );
    fs::write("VERSION.md", version_content)?;
    repo.commit("[INIT] Create VERSION.md on master")?;
    println!("{} Updated master branch with VERSION.md", "✓".green());
    if !repo.branch_exists("api")? {
        repo.checkout_new_branch_from("api", "master")?;
        fs::write(
            "INFO.md",
            "# API Root\n\nThis is the root branch for all API versions.\n",
        )?;
        repo.commit("[INIT] Create api root branch")?;
        println!("{} Created api root branch", "✓".green());
    }
    if !repo.branch_exists("error")? {
        repo.checkout_new_branch_from("error", "master")?;
        fs::write(
            "INFO.md",
            "# Error Root\n\nThis is the root branch for all error codes.\n",
        )?;
        repo.commit("[INIT] Create error root branch")?;
        println!("{} Created error root branch", "✓".green());
    }
    repo.checkout("master")?;
    fs::create_dir_all(".arm")?;
    let mapping = PathMapping::new();
    fs::write(MAPPING_PATH, serde_json::to_string_pretty(&mapping)?)?;
    repo.commit("[INIT] Create mapping file")?;
    println!("{} Created mapping file", "✓".green());

    // Record this repository
    let repo_name = repo
        .workdir()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    let repo_path = repo
        .workdir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string());
    add_repo(repo_name, &repo_path)?;
    println!("{} Recorded repository: {} -> {}", "✓".green(), repo_name.cyan(), repo_path.dimmed());

    println!("\n{}", "Initialization complete!".green().bold());
    println!("  Use 'arm registry new' to create the first API version.");
    Ok(())
}

/// Initialize a new repository in ~/.arm/<name>
pub fn init_with_name(name: &str) -> Result<()> {
    // Get home directory and create ARM directory path
    let home = dirs::home_dir().context("Failed to get home directory")?;
    let arm_dir = home.join(".arm").join(name);
    let arm_dir_str = arm_dir.to_string_lossy().to_string();

    println!("{}", "Initializing new ARM repository...".cyan().bold());
    println!("  Path: {}", arm_dir_str.cyan());

    // Check if directory already exists
    if arm_dir.exists() {
        // Check if it's already a git repository
        if GitRepo::is_valid(&arm_dir_str) {
            println!("  {} Repository already exists at this path", "⚠".yellow());
            // Still record it in repos.json
            add_repo(name, &arm_dir_str)?;
            println!("  {} Recorded repository: {} -> {}", "✓".green(), name.cyan(), arm_dir_str.dimmed());
            return Ok(());
        } else {
            bail!("Directory already exists but is not a git repository: {}", arm_dir_str);
        }
    }

    // Create the directory and initialize git
    fs::create_dir_all(&arm_dir)?;
    println!("  {} Created directory", "✓".green());

    // Initialize git repository
    let repo = git::init_repo(&arm_dir_str)?;
    println!("  {} Initialized git repository", "✓".green());

    // Now initialize the ARM structure (reuse the logic from init function)
    // We need to work from the new repo
    init(&repo)?;

    // Update the message to reflect the new location
    println!("\n{}", "Initialization complete!".green().bold());
    println!("  Repository created at: {}", arm_dir_str.cyan());
    println!("  Use 'arm -r {} <command>' to work with this repository", name.cyan());

    Ok(())
}

/// Scan ~/.arm directory for existing repositories
pub fn scan() -> Result<()> {
    // Get home directory and .arm directory path
    let home = dirs::home_dir().context("Failed to get home directory")?;
    let arm_base_dir = home.join(".arm");

    println!("{}", "Scanning ~/.arm for existing repositories...".cyan().bold());
    println!("  Base path: {}", arm_base_dir.to_string_lossy().cyan());
    println!();

    // Check if .arm directory exists
    if !arm_base_dir.exists() {
        println!("  {} ~/.arm directory does not exist", "⚠".yellow());
        println!("  Create one with 'arm init --name <name>'");
        return Ok(());
    }

    // Read all entries in .arm directory
    let mut count = 0;
    let mut added = 0;

    for entry in fs::read_dir(&arm_base_dir)? {
        let entry = entry?;
        let path = entry.path();

        // Only check directories
        if !path.is_dir() {
            continue;
        }

        let dir_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // Skip hidden directories
        if dir_name.starts_with('.') {
            continue;
        }

        count += 1;
        let path_str = path.to_string_lossy().to_string();

        // Check if it's a valid git repository
        if GitRepo::is_valid(&path_str) {
            // Try to open it to check if it's at least a valid repo
            if let Ok(_repo) = GitRepo::open(&path_str) {
                // Add to repos.json
                add_repo(dir_name, &path_str)?;
                added += 1;
                println!("  {} {} -> {}", "✓".green(), dir_name.cyan(), path_str.dimmed());
            } else {
                println!("  {} {} (invalid git repository)", "⚠".yellow(), dir_name.cyan());
            }
        } else {
            println!("  {} {} (not a git repository)", "✗".red(), dir_name.cyan());
        }
    }

    println!();
    println!("{}", "Scan complete:".bold());
    println!("  Total directories: {}", count);
    println!("  Repositories added: {}", added);

    if added == 0 {
        println!("\n  {} No valid ARM repositories found.", "ℹ".blue());
        println!("  Create one with 'arm init --name <name>'");
    }

    Ok(())
}

pub fn create_version(repo: &GitRepo, description: Option<&str>) -> Result<()> {
    let latest = get_latest_version(repo)?;
    let new_version_num = latest
        .as_ref()
        .map(|v| {
            Regex::new(r"v(\d+)")
                .unwrap()
                .captures(v)
                .and_then(|c| c[1].parse::<u32>().ok())
                .unwrap_or(0)
                + 1
        })
        .unwrap_or(1);
    let new_version = format!("v{}", new_version_num);
    let source = latest.unwrap_or_else(|| "api".to_string());
    println!(
        "{} Creating new version '{}' from '{}'...",
        "→".yellow(),
        new_version.cyan(),
        source.yellow()
    );

    // Load mapping from current branch before creating new branch
    let mut mapping = load_mapping(repo)?;

    repo.checkout_new_branch_from(&new_version, &source)?;
    let date = Local::now().format("%Y-%m-%d %H:%M:%S");
    let desc = description.unwrap_or("New API version");
    fs::write(
        "INFO.md",
        format!(
            "# {}\n\n## Description\n{}\n\n## Created\n{}\n\n## Source Version\n{}\n\n## Categories\n- None yet\n\n## Last Updated\n{}\n",
            new_version, desc, date, source, date
        ),
    )?;

    // Add to mapping and save on the new branch
    mapping.add(&new_version, &new_version, "version", Some("api"));
    save_mapping(repo, &mapping)?;

    repo.commit(&format!("[VERSION] Create {} from {}", new_version, source))?;
    repo.checkout("master")?;
    let version_md = fs::read_to_string("VERSION.md").unwrap_or_default();
    let updated = version_md
        .replace(
            "## Current Version\nNone",
            &format!("## Current Version\n{}", new_version),
        )
        .replace(
            "## Version History",
            &format!(
                "## Version History\n- {}: {} (from {})\n",
                new_version, desc, source
            ),
        );
    fs::write("VERSION.md", updated)?;
    repo.commit(&format!("[VERSION] Record {} in master", new_version))?;
    repo.checkout(&new_version)?;
    println!(
        "{} Created version branch: {}",
        "✓".green().bold(),
        new_version.cyan()
    );
    Ok(())
}

pub fn create_category(repo: &GitRepo, name: &str, description: Option<&str>) -> Result<()> {
    let latest = get_latest_version(repo)?
        .context("No version found. Create one with 'arm registry new'")?;
    let branch_code = generate_random_code();
    let branch_name = format!("{}-{}", latest, branch_code);
    println!(
        "{} Creating category '{}' in {}...",
        "→".yellow(),
        name.cyan(),
        latest.yellow()
    );

    // Load mapping before creating new branch
    let mut mapping = load_mapping(repo)?;

    repo.checkout_new_branch_from(&branch_name, &latest)?;
    let date = Local::now().format("%Y-%m-%d %H:%M:%S");
    let desc = description.unwrap_or("New category");
    let info_content = format!(
        "# {}\n\n## Type\ncategory\n\n## Path\n{}\n\n## Description\n{}\n\n## Version\n{}\n\n## Created\n{}\n\n## Endpoints\n- None yet\n\n## Last Updated\n{}\n",
        name, name, desc, latest, date, date
    );
    fs::write("INFO.md", &info_content)?;

    let path = format!("{}/{}", latest, name);
    mapping.add(&path, &branch_name, "category", Some(&latest));
    save_mapping(repo, &mapping)?;

    // Re-write INFO.md after save_mapping (which switches branches)
    fs::write("INFO.md", &info_content)?;

    repo.commit_files(
        &[Path::new("INFO.md")],
        &format!("[CATEGORY] Create {}", name),
    )?;
    println!("{} Created category: {}", "✓".green().bold(), name.cyan());

    // Return to master
    repo.checkout("master")?;
    Ok(())
}

pub fn create_endpoint(repo: &GitRepo, path: &str, description: Option<&str>) -> Result<()> {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() < 2 {
        bail!(
            "Invalid endpoint path '{}'. Must be in format 'category/resource'",
            path
        );
    }
    let category = parts[..parts.len() - 1].join("/");
    let resource = parts.last().unwrap();
    let latest = get_latest_version(repo)?
        .context("No version found. Create one with 'arm registry new'")?;

    // Load mapping before any branch operations
    let mut mapping = load_mapping(repo)?;

    let category_path = format!("{}/{}", latest, category);
    let parent_branch = mapping
        .get_by_path(&category_path)
        .context(format!("Category '{}' not found", category))?
        .branch
        .clone();
    let branch_code = generate_random_code();
    let branch_name = format!("{}-{}", latest, branch_code);
    println!(
        "{} Creating endpoint '{}' in category '{}'...",
        "→".yellow(),
        resource.cyan(),
        category.yellow()
    );
    repo.checkout_new_branch_from(&branch_name, &parent_branch)?;
    let date = Local::now().format("%Y-%m-%d %H:%M:%S");
    let desc = description.unwrap_or("New endpoint");
    fs::write(
        "INFO.md",
        format!(
            "# {}\n\n## Type\nendpoint\n\n## Path\n{}\n\n## Resource\n{}\n\n## Category\n{}\n\n## Description\n{}\n\n## Version\n{}\n\n## Created\n{}\n\n## Methods\n- None yet\n\n## Last Updated\n{}\n",
            resource, path, resource, category, desc, latest, date, date
        ),
    )?;

    let full_path = format!("{}/{}/{}", latest, category, resource);
    mapping.add(&full_path, &branch_name, "endpoint", Some(&parent_branch));
    save_mapping(repo, &mapping)?;

    // Re-write INFO.md after save_mapping (which switches branches)
    fs::write(
        "INFO.md",
        format!(
            "# {}\n\n## Type\nendpoint\n\n## Path\n{}\n\n## Resource\n{}\n\n## Category\n{}\n\n## Description\n{}\n\n## Version\n{}\n\n## Created\n{}\n\n## Methods\n- None yet\n\n## Last Updated\n{}\n",
            resource, path, resource, category, desc, latest, date, date
        ),
    )?;

    repo.commit_files(
        &[Path::new("INFO.md")],
        &format!("[ENDPOINT] Create {}", path),
    )?;
    println!("{} Created endpoint: {}", "✓".green().bold(), path.cyan());

    // Return to master
    repo.checkout("master")?;
    Ok(())
}

/// Internal helper to create endpoint without reloading mapping
/// Returns the branch name of the created endpoint
fn create_endpoint_internal(
    repo: &GitRepo,
    latest: &str,
    path: &str,
    mapping: &mut PathMapping,
    description: Option<&str>,
) -> Result<String> {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() < 2 {
        bail!(
            "Invalid endpoint path '{}'. Must be in format 'category/resource'",
            path
        );
    }
    let category = parts[..parts.len() - 1].join("/");
    let resource = parts.last().unwrap();

    let category_path = format!("{}/{}", latest, category);
    let parent_branch = mapping
        .get_by_path(&category_path)
        .context(format!("Category '{}' not found", category))?
        .branch
        .clone();
    let branch_code = generate_random_code();
    let branch_name = format!("{}-{}", latest, branch_code);
    println!(
        "{} Creating endpoint '{}' in category '{}'...",
        "→".yellow(),
        resource.cyan(),
        category.yellow()
    );
    repo.checkout_new_branch_from(&branch_name, &parent_branch)?;
    let date = Local::now().format("%Y-%m-%d %H:%M:%S");
    let desc = description.unwrap_or("New endpoint");
    fs::write(
        "INFO.md",
        format!(
            "# {}\n\n## Type\nendpoint\n\n## Path\n{}\n\n## Resource\n{}\n\n## Category\n{}\n\n## Description\n{}\n\n## Version\n{}\n\n## Created\n{}\n\n## Methods\n- None yet\n\n## Last Updated\n{}\n",
            resource, path, resource, category, desc, latest, date, date
        ),
    )?;

    let full_path = format!("{}/{}/{}", latest, category, resource);
    mapping.add(&full_path, &branch_name, "endpoint", Some(&parent_branch));
    save_mapping(repo, mapping)?;

    repo.commit(&format!("[ENDPOINT] Create {}", path))?;
    println!("{} Created endpoint: {}", "✓".green().bold(), path.cyan());
    Ok(branch_name)
}

pub fn create_method(repo: &GitRepo, path: &str, description: Option<&str>) -> Result<()> {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() < 3 {
        bail!(
            "Invalid method path '{}'. Must be in format 'category/resource/METHOD'",
            path
        );
    }
    let method = parts.last().unwrap().to_uppercase();
    let valid_methods = ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"];
    if !valid_methods.contains(&method.as_str()) {
        bail!("Invalid HTTP method '{}'", method);
    }
    let endpoint_path = parts[..parts.len() - 1].join("/");
    let latest = get_latest_version(repo)?.context("No version found")?;

    // Load mapping once before any operations
    let mut mapping = load_mapping(repo)?;

    let parent_branch =
        if let Some(entry) = mapping.get_by_path(&format!("{}/{}", latest, endpoint_path)) {
            entry.branch.clone()
        } else {
            println!(
                "{} Endpoint '{}' not found, creating...",
                "→".yellow(),
                endpoint_path
            );
            // We need to create endpoint and update our mapping without reloading
            // Since create_endpoint modifies the repo state, we call it but then
            // manually add the endpoint entry to our mapping
            create_endpoint_internal(
                repo,
                &latest,
                &endpoint_path,
                &mut mapping,
                Some("Auto-created"),
            )?
        };
    let branch_code = generate_random_code();
    let branch_name = format!("{}-{}", latest, branch_code);
    println!(
        "{} Creating method '{}' for endpoint '{}'...",
        "→".yellow(),
        method.cyan(),
        endpoint_path.yellow()
    );
    repo.checkout_new_branch_from(&branch_name, &parent_branch)?;
    let date = Local::now().format("%Y-%m-%d %H:%M:%S");
    let desc = description.unwrap_or("New method");
    let full_path = format!("{}/{}", latest, path);
    let content = format!(
        r#"# {method} {path}

## Type
method

## Method
{method}

## Path
{path}

## Full Path
/api/{version}/{category}/{resource}

## Description
{desc}

## Version
{version}

## Created
{date}

## Request

### Path Parameters
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| TBD | | | |

### Query Parameters
| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| TBD | | | | |

### Request Body
```json
{{
  "example": "Please fill in request body parameters"
}}
```

## Response

### Success Response
```json
{{
  "code": 0,
  "data": {{}},
  "message": "success"
}}
```

### Error Codes
| Error Code | Description | Trigger Scenario |
|------------|-------------|------------------|
| TBD | | |

## Related Errors
- None

## Change History
- {date}: Initial creation
"#,
        method = method,
        path = path,
        version = latest,
        category = parts[..parts.len() - 2].join("/"),
        resource = parts[parts.len() - 2],
        desc = desc,
        date = date
    );
    fs::write("INFO.md", &content)?;
    mapping.add(&full_path, &branch_name, "method", Some(&parent_branch));
    save_mapping(repo, &mapping)?;

    // Re-write INFO.md after save_mapping (which switches branches)
    fs::write("INFO.md", &content)?;

    repo.commit_files(
        &[Path::new("INFO.md")],
        &format!("[METHOD] Create {} {}", path, method),
    )?;
    println!("{} Created method: {}", "✓".green().bold(), path.cyan());

    // Return to master
    repo.checkout("master")?;
    Ok(())
}

/// Check if a repository meets ARM requirements
pub fn check_repo(_current_repo: &GitRepo, path: &str) -> Result<()> {
    println!("{} Checking repository: {}", "→".yellow(), path.cyan());
    println!();

    // Check if the path exists and is a valid git repository
    if !GitRepo::is_valid(path) {
        bail!("No valid Git repository found at: {}", path);
    }

    // Open the target repository
    let target_repo = GitRepo::open(path)?;

    let mut issues = Vec::new();
    let mut passed = Vec::new();

    // Check for required branches
    let has_master = target_repo.branch_exists("master")?;
    let has_api = target_repo.branch_exists("api")?;
    let has_error = target_repo.branch_exists("error")?;

    println!("{}", "1. Required Branches:".bold());
    if has_master {
        println!("  {} master branch exists", "✓".green());
        passed.push("master");
    } else {
        println!("  {} master branch missing", "✗".red());
        issues.push("Missing master branch");
    }
    if has_api {
        println!("  {} api branch exists", "✓".green());
        passed.push("api");
    } else {
        println!("  {} api branch missing", "✗".red());
        issues.push("Missing api branch");
    }
    if has_error {
        println!("  {} error branch exists", "✓".green());
        passed.push("error");
    } else {
        println!("  {} error branch missing", "✗".red());
        issues.push("Missing error branch");
    }

    // Check mapping file
    println!();
    println!("{}", "2. Mapping File:".bold());
    let mapping_path = format!("{}/.arm/mapping.json", path);
    if std::path::Path::new(&mapping_path).exists() {
        let content = fs::read_to_string(&mapping_path)?;
        let mapping: PathMapping =
            serde_json::from_str(&content).context("Failed to parse mapping.json")?;
        println!("  {} mapping.json exists", "✓".green());
        println!("  {} entries: {}", "→".yellow(), mapping.entries.len());
        passed.push("mapping.json");
    } else {
        println!("  {} mapping.json missing", "✗".red());
        issues.push("Missing .arm/mapping.json");
    }

    // Check VERSION.md
    println!();
    println!("{}", "3. Version File:".bold());
    let version_path = format!("{}/VERSION.md", path);
    if std::path::Path::new(&version_path).exists() {
        println!("  {} VERSION.md exists", "✓".green());
        passed.push("VERSION.md");
    } else {
        println!("  {} VERSION.md missing", "✗".red());
        issues.push("Missing VERSION.md");
    }

    // Check version branches
    println!();
    println!("{}", "4. API Versions:".bold());
    let branches = target_repo.list_branches()?;
    let version_regex = Regex::new(r"^v(\d+)$").unwrap();
    let versions: Vec<_> = branches
        .iter()
        .filter(|(name, _)| version_regex.is_match(name))
        .collect();

    if versions.is_empty() {
        println!("  {} No version branches found", "⚠".yellow());
        issues.push("No version branches (v1, v2, etc.)");
    } else {
        println!("  {} Found {} version(s)", "✓".green(), versions.len());
        for (name, _) in &versions {
            println!("    - {}", name.cyan());
        }
        passed.push("version branches");
    }

    // Check error code branches
    println!();
    println!("{}", "5. Error Codes:".bold());
    let error_branches: Vec<_> = branches
        .iter()
        .filter(|(name, _)| name.starts_with("error-E"))
        .collect();

    if error_branches.is_empty() {
        println!("  {} No error code branches found", "⚠".yellow());
    } else {
        println!(
            "  {} Found {} error code(s)",
            "✓".green(),
            error_branches.len()
        );
        for (name, _) in &error_branches {
            let code = name.strip_prefix("error-").unwrap_or(name);
            println!("    - {}", code.cyan());
        }
        passed.push("error branches");
    }

    // Summary
    println!();
    println!("{}", "=".repeat(50));
    println!();
    println!("{}", "Summary:".bold());
    println!("  {} Checks passed: {}", "✓".green(), passed.len());
    println!("  {} Issues found: {}", "✗".red().bold(), issues.len());
    println!();

    if issues.is_empty() {
        println!(
            "{} Repository is a valid ARM repository!",
            "✓".green().bold()
        );
    } else {
        println!("{}", "Issues:".bold());
        for issue in &issues {
            println!("  {} {}", "✗".red(), issue);
        }
        println!();
        println!(
            "{} Run 'arm -r {} init' to initialize the repository",
            "→".yellow(),
            path.cyan()
        );
    }

    Ok(())
}

/// Mount an existing API repository and show its status
pub fn mount_repo(_current_repo: &GitRepo, path: &str) -> Result<()> {
    println!("{} Mounting repository from: {}", "→".yellow(), path.cyan());

    // Check if the path exists and is a valid git repository
    if !GitRepo::is_valid(path) {
        bail!("No valid Git repository found at: {}", path);
    }

    // Open the target repository
    let target_repo = GitRepo::open(path)?;

    // Check for required branches
    let has_master = target_repo.branch_exists("master")?;
    let has_api = target_repo.branch_exists("api")?;
    let has_error = target_repo.branch_exists("error")?;

    println!();
    println!("{}", "Repository Status:".cyan().bold());
    println!();

    // Show branch structure
    println!("{}", "Branches:".bold());
    if has_master {
        println!("  {} master", "✓".green());
    } else {
        println!("  {} master (missing)", "✗".red());
    }
    if has_api {
        println!("  {} api", "✓".green());
    } else {
        println!("  {} api (missing)", "✗".red());
    }
    if has_error {
        println!("  {} error", "✓".green());
    } else {
        println!("  {} error (missing)", "✗".red());
    }

    // Check if it's a valid ARM repository
    let is_arm_repo = has_master && has_api && has_error;

    println!();

    if is_arm_repo {
        // Try to load mapping from master branch
        let current_branch = target_repo.current_branch()?;
        if current_branch != "master" {
            target_repo.checkout("master")?;
        }

        let mapping_path = format!("{}/.arm/mapping.json", path);
        if std::path::Path::new(&mapping_path).exists() {
            let content = fs::read_to_string(&mapping_path)?;
            let mapping: PathMapping = serde_json::from_str(&content)?;
            println!("{}", "Mapping:".bold());
            println!("  {} entries: {}", "→".yellow(), mapping.entries.len());
            println!(
                "  {} branches tracked: {}",
                "→".yellow(),
                mapping.branches.len()
            );
        }

        // Show version branches
        let branches = target_repo.list_branches()?;
        let version_regex = Regex::new(r"^v(\d+)$").unwrap();
        let versions: Vec<_> = branches
            .iter()
            .filter(|(name, _)| version_regex.is_match(name))
            .collect();

        if !versions.is_empty() {
            println!();
            println!("{}", "API Versions:".bold());
            for (name, _) in versions {
                println!("  {} {}", "→".yellow(), name.cyan());
            }
        }

        // Show error branches
        let error_branches: Vec<_> = branches
            .iter()
            .filter(|(name, _)| name.starts_with("error-E"))
            .collect();

        if !error_branches.is_empty() {
            println!();
            println!("{}", "Error Codes:".bold());
            for (name, _) in error_branches {
                let code = name.strip_prefix("error-").unwrap_or(name);
                println!("  {} {}", "→".yellow(), code.cyan());
            }
        }

        // Restore original branch
        if current_branch != "master" {
            target_repo.checkout(&current_branch)?;
        }

        println!();
        println!(
            "{} Successfully mounted ARM repository!",
            "✓".green().bold()
        );
        println!(
            "  Use 'arm -r {} <command>' to work with this repository",
            path.cyan()
        );

        // Record this repository
        let repo_name = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        add_repo(repo_name, path)?;
        println!("  {} Recorded repository: {} -> {}", "✓".green(), repo_name.cyan(), path.dimmed());
    } else {
        println!(
            "{} Not a valid ARM repository. Run 'arm -r {} init' to initialize.",
            "⚠".yellow().bold(),
            path.cyan()
        );
    }

    Ok(())
}

pub fn create_error(repo: &GitRepo, code: &str, message: &str, status: u16) -> Result<()> {
    let error_regex = Regex::new(r"^E\d{3,}$").unwrap();
    if !error_regex.is_match(code) {
        bail!(
            "Invalid error code '{}'. Must be in format 'E001', etc.",
            code
        );
    }
    let branch_name = format!("error-{}", code);
    println!("{} Creating error code '{}'...", "→".yellow(), code.cyan());

    // Load mapping before creating new branch
    let mut mapping = load_mapping(repo)?;

    repo.checkout_new_branch_from(&branch_name, "error")?;
    let date = Local::now().format("%Y-%m-%d %H:%M:%S");
    fs::write(
        "ERROR.md",
        format!(
            r#"# Error Code: {code}

## Code
{code}

## HTTP Status
{status}

## Message
{message}

## Description
TBD

## Possible Causes
1. TBD

## Solutions
TBD

## Related Endpoints
- None

## Created
{date}

## Change History
- {date}: Initial definition
"#,
            code = code,
            status = status,
            message = message,
            date = date
        ),
    )?;

    mapping.add(
        &format!("error/{}", code),
        &branch_name,
        "error",
        Some("error"),
    );
    save_mapping(repo, &mapping)?;

    // Re-write ERROR.md after save_mapping (which switches branches)
    let error_content = format!(
        r#"# Error Code {code}

## Type
error

## Code
{code}

## HTTP Status
{status}

## Message
{message}

## Created
{date}

## Description
TBD

## Possible Causes
- None yet

## Solutions
- None yet

## Related Endpoints
- None

## Change History
- {date}: Initial definition
"#,
        code = code,
        status = status,
        message = message,
        date = date
    );
    fs::write("ERROR.md", &error_content)?;

    repo.commit_files(
        &[Path::new("ERROR.md")],
        &format!("[ERROR] Create {} - {}", code, message),
    )?;
    println!("{} Created error code: {}", "✓".green().bold(), code.cyan());

    // Return to master
    repo.checkout("master")?;
    Ok(())
}
