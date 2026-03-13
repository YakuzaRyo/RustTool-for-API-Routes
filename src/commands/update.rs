use anyhow::{Context, Result, bail};
use colored::Colorize;
use std::fs;

use crate::git::GitRepo;
use crate::commands::registry::{load_mapping, get_latest_version};

pub fn execute(repo: &GitRepo, path: &str, update: &str) -> Result<()> {
    if path.is_empty() {
        bail!("Path is required. Example: arm update auth/users/POST description:New description");
    }
    let (key, content) = parse_update(update)?;
    if path.starts_with("error/") {
        return update_error(repo, path, &key, &content);
    }
    let latest = get_latest_version(repo)?.context("No API version found")?;
    let mapping = load_mapping(repo)?;
    let full_path = format!("{}/{}", latest, path);
    let entry = mapping.get_by_path(&full_path).context(format!("'{}' not found", path))?;
    repo.checkout(&entry.branch)?;
    let mut file_content = fs::read_to_string("INFO.md").context("Failed to read INFO.md")?;
    file_content = replace_markdown_section(&file_content, &key, &content)?;
    fs::write("INFO.md", file_content)?;
    repo.commit(&format!("[UPDATE] {} - {}: {}", path, key, content))?;
    println!("{} Updated: {}", "✓".green().bold(), path.cyan());

    // Return to master
    repo.checkout("master")?;
    Ok(())
}

fn parse_update(update: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = update.splitn(2, ':').collect();
    if parts.len() != 2 {
        bail!("Invalid update format. Use 'key:content'");
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

fn update_error(repo: &GitRepo, path: &str, key: &str, content: &str) -> Result<()> {
    let code = path.strip_prefix("error/").context("Invalid error path format")?;
    let branch_name = format!("error-{}", code);
    if !repo.branch_exists(&branch_name)? {
        bail!("Error code '{}' not found", code);
    }
    repo.checkout(&branch_name)?;
    let mut file_content = fs::read_to_string("ERROR.md").context("Failed to read ERROR.md")?;
    file_content = replace_markdown_section(&file_content, key, content)?;
    fs::write("ERROR.md", file_content)?;
    repo.commit(&format!("[UPDATE-ERROR] {} - {}: {}", code, key, content))?;
    println!("{} Updated error: {}", "✓".green().bold(), code.cyan());

    // Return to master
    repo.checkout("master")?;
    Ok(())
}

fn replace_markdown_section(content: &str, key: &str, new_value: &str) -> Result<String> {
    // Normalize line endings for Windows compatibility
    let normalized = content.replace("\r\n", "\n");

    // Capitalize the key for section matching (description -> Description)
    let capitalized_key = if key.is_empty() {
        key.to_string()
    } else {
        let mut chars = key.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
        }
    };

    // Try section format first: "## Field\nvalue" - this is what our templates use
    let header = format!("## {}", capitalized_key);
    let lines: Vec<&str> = normalized.lines().collect();
    let mut result = Vec::new();
    let mut i = 0;
    let mut replaced = false;

    while i < lines.len() {
        let line = lines[i];
        result.push(line);

        if line.trim() == header {
            // Found the header, next non-empty line should be the value
            i += 1;
            // Skip any empty lines
            while i < lines.len() && lines[i].trim().is_empty() {
                result.push(lines[i]);
                i += 1;
            }
            // Replace the value line (if we're not at end and next is not a header)
            if i < lines.len() {
                if !lines[i].trim().starts_with("##") && !lines[i].trim().starts_with("# ") {
                    result.push(new_value);
                    i += 1;
                    replaced = true;
                } else {
                    // Next line is a header, insert the new value
                    result.push(new_value);
                    replaced = true;
                }
            } else {
                result.push(new_value);
                replaced = true;
            }
        } else {
            i += 1;
        }
    }

    if replaced {
        return Ok(result.join("\n"));
    }

    // Try field format: "**Field**: value" (for legacy or appended fields)
    let field_pattern = format!(r"(?im)^(\*\*{}\*\*):\s*(.+)$", regex::escape(&capitalized_key));
    if let Ok(re) = regex::Regex::new(&field_pattern) {
        if re.is_match(&normalized) {
            return Ok(re.replace_all(&normalized, format!("$1: {}", new_value)).to_string());
        }
    }

    // If no match, append as new field
    let mut result = normalized;
    result.push_str(&format!("\n\n**{}**: {}\n", capitalized_key, new_value));
    Ok(result)
}
