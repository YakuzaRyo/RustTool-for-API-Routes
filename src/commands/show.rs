use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;

use crate::commands::registry::{get_latest_version, load_mapping};
use crate::git::GitRepo;

#[derive(Debug, Serialize, Deserialize)]
pub struct EndpointInfo {
    pub method: String,
    pub path: String,
    pub category: String,
    pub status: String,
    pub created: String,
    pub description: String,
    pub request: RequestInfo,
    pub response: ResponseInfo,
    pub related_errors: Vec<String>,
    pub change_history: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RequestInfo {
    pub path_params: Vec<ParamInfo>,
    pub query_params: Vec<ParamInfo>,
    pub body_example: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseInfo {
    pub success_example: Option<String>,
    pub error_codes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ParamInfo {
    pub name: String,
    pub param_type: String,
    pub required: bool,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub code: String,
    pub http_status: u16,
    pub message: String,
    pub created: String,
    pub description: String,
    pub possible_causes: Vec<String>,
    pub solutions: Vec<String>,
    pub related_endpoints: Vec<String>,
    pub change_history: Vec<String>,
}

pub fn execute(repo: &GitRepo, path: &str) -> Result<()> {
    if path.is_empty() {
        bail!("Path is required. Example: arm show auth/users/POST");
    }
    if path.starts_with("error/") {
        return show_error(repo, path);
    }
    let latest = get_latest_version(repo)?
        .context("No API version found. Create one with 'arm registry new'")?;
    let mapping = load_mapping(repo)?;
    let full_path = format!("{}/{}", latest, path);
    let entry = mapping
        .get_by_path(&full_path)
        .context(format!("'{}' not found in version {}", path, latest))?;
    repo.checkout(&entry.branch)?;
    let info = parse_info_md()?;
    let json = serde_json::to_string_pretty(&info)?;
    println!("{}", json);

    // Return to master
    repo.checkout("master")?;
    Ok(())
}

fn show_error(repo: &GitRepo, path: &str) -> Result<()> {
    let code = path
        .strip_prefix("error/")
        .context("Invalid error path format")?;
    let branch_name = format!("error-{}", code);
    if !repo.branch_exists(&branch_name)? {
        bail!("Error code '{}' not found", code);
    }
    repo.checkout(&branch_name)?;
    let error_info = parse_error_md()?;
    let json = serde_json::to_string_pretty(&error_info)?;
    println!("{}", json);

    // Return to master
    repo.checkout("master")?;
    Ok(())
}

fn parse_info_md() -> Result<EndpointInfo> {
    let content = fs::read_to_string("INFO.md").context("Failed to read INFO.md")?;
    let method = extract_field(&content, "Method").unwrap_or_default();
    let path = extract_field(&content, "Path").unwrap_or_default();
    let category = extract_field(&content, "Category").unwrap_or_default();
    let status = extract_field(&content, "Status").unwrap_or_else(|| "active".to_string());
    let created = extract_field(&content, "Created").unwrap_or_default();
    let description = extract_section(&content, "Description").unwrap_or_default();
    let request = RequestInfo {
        path_params: parse_params(&content, "Path Parameters"),
        query_params: parse_params(&content, "Query Parameters"),
        body_example: extract_code_block(&content, "Request Body"),
    };
    let response = ResponseInfo {
        success_example: extract_code_block(&content, "Success Response"),
        error_codes: parse_error_codes(&content),
    };
    let related_errors = parse_list(&content, "Related Errors");
    let change_history = parse_change_history(&content);
    Ok(EndpointInfo {
        method,
        path,
        category,
        status,
        created,
        description,
        request,
        response,
        related_errors,
        change_history,
    })
}

fn parse_error_md() -> Result<ErrorInfo> {
    let content = fs::read_to_string("ERROR.md").context("Failed to read ERROR.md")?;
    let code = extract_field(&content, "Code").unwrap_or_default();
    let http_status = extract_field(&content, "HTTP Status")
        .and_then(|s| s.parse().ok())
        .unwrap_or(400);
    let message = extract_field(&content, "Message").unwrap_or_default();
    let created = extract_field(&content, "Created").unwrap_or_default();
    let description = extract_section(&content, "Description").unwrap_or_default();
    let possible_causes = parse_list(&content, "Possible Causes");
    let solutions = parse_list(&content, "Solutions");
    let related_endpoints = parse_list(&content, "Related Endpoints");
    let change_history = parse_change_history(&content);
    Ok(ErrorInfo {
        code,
        http_status,
        message,
        created,
        description,
        possible_causes,
        solutions,
        related_endpoints,
        change_history,
    })
}

fn extract_field(content: &str, field: &str) -> Option<String> {
    // Normalize line endings for Windows compatibility
    let normalized = content.replace("\r\n", "\n");

    // Try header format: look for "## Field" and get the next non-empty line
    let header = format!("## {}", field);
    if let Some(pos) = normalized.find(&header) {
        let after_header = &normalized[pos + header.len()..];
        // Skip whitespace and empty lines after header
        for line in after_header.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            // Stop if we hit another header
            if trimmed.starts_with("##") || trimmed.starts_with("#") {
                break;
            }
            return Some(trimmed.to_string());
        }
    }

    // Fallback to old format: "**Field**: value"
    let pattern = format!(r"\*\*{}\*\*:\s*(.+)", regex::escape(field));
    regex::Regex::new(&pattern)
        .ok()?
        .captures(&normalized)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().trim().to_string())
}

fn extract_section(content: &str, section: &str) -> Option<String> {
    // Normalize line endings for Windows compatibility
    let normalized = content.replace("\r\n", "\n");

    // Find the section header
    let header = format!("## {}", section);
    let start = normalized.find(&header)?;
    let after_header = &normalized[start + header.len()..];

    // Find where the next section starts (## or # at beginning of line)
    let mut end_pos = after_header.len();
    for (i, _) in after_header.match_indices('\n') {
        let rest = &after_header[i + 1..];
        if rest.starts_with("##") || rest.starts_with("# ") {
            end_pos = i;
            break;
        }
    }

    let section_content = &after_header[..end_pos];
    let trimmed = section_content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn extract_code_block(content: &str, section: &str) -> Option<String> {
    let section_content = extract_section(content, section)?;
    regex::Regex::new(r"```(?:json)?\s*\n(.*?)```")
        .ok()?
        .captures(&section_content)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().trim().to_string())
}

fn parse_params(content: &str, section: &str) -> Vec<ParamInfo> {
    let mut params = Vec::new();
    if let Some(section_content) = extract_section(content, section) {
        for line in section_content.lines() {
            if line.starts_with('|') && !line.contains("Parameter") && !line.contains("---") {
                let cells: Vec<_> = line
                    .split('|')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .collect();
                if cells.len() >= 4 {
                    params.push(ParamInfo {
                        name: cells[0].to_string(),
                        param_type: cells[1].to_string(),
                        required: cells[2].to_lowercase() == "yes" || cells[2] == "true",
                        description: cells.get(3).unwrap_or(&"").to_string(),
                    });
                }
            }
        }
    }
    params
}

fn parse_error_codes(content: &str) -> Vec<String> {
    let mut codes = Vec::new();
    if let Some(section_content) = extract_section(content, "Error Codes") {
        for line in section_content.lines() {
            if line.starts_with('|') && !line.contains("Error Code") && !line.contains("---") {
                let cells: Vec<_> = line
                    .split('|')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .collect();
                if let Some(code) = cells.first() {
                    codes.push(code.to_string());
                }
            }
        }
    }
    codes
}

fn parse_list(content: &str, section: &str) -> Vec<String> {
    let mut items = Vec::new();
    if let Some(section_content) = extract_section(content, section) {
        for line in section_content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("- ") || trimmed.starts_with("1. ") {
                let item = trimmed
                    .trim_start_matches("- ")
                    .trim_start_matches("1. ")
                    .trim()
                    .to_string();
                if !item.is_empty() && item != "None" && item != "TBD" {
                    items.push(item);
                }
            }
        }
    }
    items
}

fn parse_change_history(content: &str) -> Vec<String> {
    parse_list(content, "Change History")
}
