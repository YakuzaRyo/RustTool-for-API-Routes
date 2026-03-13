use anyhow::{Context, Result, bail};
use chrono::Local;
use colored::Colorize;
use rand::Rng;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::git::GitRepo;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappingEntry {
    pub path: String,
    pub branch: String,
    pub entry_type: String,
    pub parent: Option<String>,
    pub created: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PathMapping {
    pub entries: HashMap<String, MappingEntry>,
    pub branches: HashMap<String, String>,
}

impl PathMapping {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            branches: HashMap::new(),
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
    }

    pub fn get_by_path(&self, path: &str) -> Option<&MappingEntry> {
        self.entries.get(path)
    }
}

const MAPPING_PATH: &str = ".arm/mapping.json";

fn generate_random_code() -> String {
    let mut rng = rand::thread_rng();
    let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyz0123456789".chars().collect();
    (0..8)
        .map(|_| chars[rng.gen_range(0..chars.len())])
        .collect()
}

pub fn load_mapping(repo: &GitRepo) -> Result<PathMapping> {
    // Always load mapping from master branch
    let current_branch = repo.current_branch()?;
    if current_branch != "master" {
        repo.checkout("master")?;
    }

    let mapping = if let Ok(content) = fs::read_to_string(MAPPING_PATH) {
        serde_json::from_str(&content).unwrap_or_else(|_| PathMapping::new())
    } else {
        PathMapping::new()
    };

    // Switch back to original branch
    if current_branch != "master" {
        repo.checkout(&current_branch)?;
    }

    Ok(mapping)
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
    println!("\n{}", "Initialization complete!".green().bold());
    println!("  Use 'arm registry new' to create the first API version.");
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
