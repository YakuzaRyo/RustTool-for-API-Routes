use anyhow::Result;
use indexmap::IndexMap;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::git::GitRepo;

/// Node type in the tree
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Root,
    ApiRoot,
    ErrorRoot,
    Version,
    Endpoint,
    ErrorCode,
    Category,
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeType::Root => write!(f, "root"),
            NodeType::ApiRoot => write!(f, "api_root"),
            NodeType::ErrorRoot => write!(f, "error_root"),
            NodeType::Version => write!(f, "version"),
            NodeType::Endpoint => write!(f, "endpoint"),
            NodeType::ErrorCode => write!(f, "error_code"),
            NodeType::Category => write!(f, "category"),
        }
    }
}

/// A node in the API tree structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNode {
    pub id: String,
    pub name: String,
    pub node_type: NodeType,
    pub branch_name: String,
    pub parent: Option<String>,
    pub children: Vec<String>,
    pub metadata: HashMap<String, String>,
    pub files: Vec<String>,
}

impl TreeNode {
    pub fn new(
        id: String,
        name: String,
        node_type: NodeType,
        branch_name: String,
    ) -> Self {
        Self {
            id,
            name,
            node_type,
            branch_name,
            parent: None,
            children: Vec::new(),
            metadata: HashMap::new(),
            files: Vec::new(),
        }
    }

    pub fn add_child(&mut self, child_id: String) {
        if !self.children.contains(&child_id) {
            self.children.push(child_id);
        }
    }

    pub fn set_parent(&mut self, parent_id: String) {
        self.parent = Some(parent_id);
    }

    pub fn add_metadata(&mut self, key: &str, value: &str) {
        self.metadata.insert(key.to_string(), value.to_string());
    }
}

/// The API tree structure with fast lookup capabilities
#[derive(Debug, Clone)]
pub struct ApiTree {
    nodes: IndexMap<String, TreeNode>,
    branch_index: HashMap<String, String>, // branch_name -> node_id
    name_index: HashMap<String, Vec<String>>, // name -> node_ids
    type_index: HashMap<NodeType, Vec<String>>, // node_type -> node_ids
}

impl ApiTree {
    pub fn new() -> Self {
        Self {
            nodes: IndexMap::new(),
            branch_index: HashMap::new(),
            name_index: HashMap::new(),
            type_index: HashMap::new(),
        }
    }

    /// Build the tree from a Git repository
    pub fn build_from_repo(repo: &GitRepo) -> Result<Self> {
        let mut tree = Self::new();

        // Get all branches
        let branches = repo.list_branches()?;

        // Process branches in parallel for better performance
        let mut branch_infos: Vec<_> = branches
            .par_iter()
            .filter_map(|(name, _)| {
                Self::classify_branch(name).map(|(node_type, parent)| {
                    (name.clone(), node_type, parent)
                })
            })
            .collect();

        // Sort by branch name to ensure parent branches are processed first
        // Order: master/api/error -> v1 -> v1-auth -> v1-auth-login
        branch_infos.sort_by(|(name_a, _, _), (name_b, _, _)| {
            let depth_a = name_a.matches('-').count();
            let depth_b = name_b.matches('-').count();
            // First sort by depth (fewer dashes first)
            depth_a.cmp(&depth_b).then_with(|| name_a.cmp(name_b))
        });

        // Create nodes
        for (branch_name, node_type, parent_branch) in &branch_infos {
            let id = format!("{}-{}", node_type, branch_name);
            let node = TreeNode::new(
                id.clone(),
                branch_name.clone(),
                node_type.clone(),
                branch_name.clone(),
            );

            tree.add_node(node);
        }

        // Set parent relationships after all nodes are created
        for (branch_name, node_type, parent_branch) in &branch_infos {
            if let Some(parent) = parent_branch {
                let child_id = format!("{}-{}", node_type, branch_name);
                if let Some(parent_id) = tree.get_node_id_by_branch(parent) {
                    tree.set_parent(&child_id, &parent_id)?;
                }
            }
        }

        // Try to load file lists for each branch
        tree.load_branch_files(repo)?;

        Ok(tree)
    }

    /// Classify a branch name into node type and parent
    fn classify_branch(branch_name: &str) -> Option<(NodeType, Option<String>)> {
        match branch_name {
            "master" | "main" => Some((NodeType::Root, None)),
            "api" => Some((NodeType::ApiRoot, Some("master".to_string()))),
            "error" => Some((NodeType::ErrorRoot, Some("master".to_string()))),
            name if name.starts_with("error-") => {
                Some((NodeType::ErrorCode, Some("error".to_string())))
            }
            name if regex::Regex::new(r"^v\d+$").unwrap().is_match(name) => {
                Some((NodeType::Version, Some("api".to_string())))
            }
            name if regex::Regex::new(r"^v\d+-").unwrap().is_match(name) => {
                // Count parts to distinguish Category (2 parts) from Endpoint (3+ parts)
                let parts: Vec<_> = name.split('-').collect();
                if parts.len() == 2 {
                    // v1-auth format -> Category
                    Some((NodeType::Category, Some(parts[0].to_string())))
                } else if parts.len() >= 3 {
                    // v1-auth-login format -> Endpoint
                    let parent = format!("{}-{}", parts[0], parts[1]);
                    Some((NodeType::Endpoint, Some(parent)))
                } else {
                    Some((NodeType::Category, None))
                }
            }
            _ => Some((NodeType::Endpoint, None)),
        }
    }

    /// Add a node to the tree
    pub fn add_node(&mut self, node: TreeNode) {
        let id = node.id.clone();
        let name = node.name.clone();
        let branch = node.branch_name.clone();
        let node_type = node.node_type.clone();

        // Add to main storage
        self.nodes.insert(id.clone(), node);

        // Update indexes
        self.branch_index.insert(branch, id.clone());

        self.name_index
            .entry(name)
            .or_default()
            .push(id.clone());

        self.type_index
            .entry(node_type)
            .or_default()
            .push(id);
    }

    /// Set parent-child relationship
    pub fn set_parent(&mut self, child_id: &str, parent_id: &str,
    ) -> Result<()> {
        // Update child's parent
        if let Some(child) = self.nodes.get_mut(child_id) {
            child.set_parent(parent_id.to_string());
        }

        // Add child to parent's children list
        if let Some(parent) = self.nodes.get_mut(parent_id) {
            parent.add_child(child_id.to_string());
        }

        Ok(())
    }

    /// Get node by ID
    pub fn get_node(&self, id: &str) -> Option<&TreeNode> {
        self.nodes.get(id)
    }

    /// Get node by ID (mutable)
    pub fn get_node_mut(&mut self, id: &str) -> Option<&mut TreeNode> {
        self.nodes.get_mut(id)
    }

    /// Get node ID by branch name (fast O(1) lookup)
    pub fn get_node_id_by_branch(&self, branch_name: &str) -> Option<String> {
        self.branch_index.get(branch_name).cloned()
    }

    /// Get node by branch name
    pub fn get_node_by_branch(&self, branch_name: &str) -> Option<&TreeNode> {
        self.get_node_id_by_branch(branch_name)
            .and_then(|id| self.get_node(&id))
    }

    /// Find nodes by name (supports partial matching)
    pub fn find_by_name(&self,
        name: &str,
        fuzzy: bool,
    ) -> Vec<&TreeNode> {
        let mut results = Vec::new();

        if fuzzy {
            // Fuzzy matching using substring
            for (key, ids) in &self.name_index {
                if key.to_lowercase().contains(&name.to_lowercase()) {
                    for id in ids {
                        if let Some(node) = self.get_node(id) {
                            results.push(node);
                        }
                    }
                }
            }
        } else {
            // Exact match
            if let Some(ids) = self.name_index.get(name) {
                for id in ids {
                    if let Some(node) = self.get_node(id) {
                        results.push(node);
                    }
                }
            }
        }

        results
    }

    /// Find nodes by type
    pub fn find_by_type(&self,
        node_type: NodeType,
    ) -> Vec<&TreeNode> {
        self.type_index
            .get(&node_type)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.get_node(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Search across all fields
    pub fn search(&self,
        query: &str,
        search_type: &str,
        fuzzy: bool,
    ) -> Vec<&TreeNode> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();

        match search_type {
            "branch" => {
                for (branch, id) in &self.branch_index {
                    if fuzzy {
                        if branch.to_lowercase().contains(&query_lower)
                            && !seen.contains(id)
                        {
                            seen.insert(id.clone());
                            results.push(self.get_node(id).unwrap());
                        }
                    } else if branch == query && !seen.contains(id) {
                        seen.insert(id.clone());
                        results.push(self.get_node(id).unwrap());
                    }
                }
            }
            "endpoint" => {
                for node in self.nodes.values() {
                    if node.node_type == NodeType::Endpoint
                        || node.node_type == NodeType::Category
                    {
                        let matches = if fuzzy {
                            node.name.to_lowercase().contains(&query_lower)
                        } else {
                            node.name == query
                        };

                        if matches && !seen.contains(&node.id) {
                            seen.insert(node.id.clone());
                            results.push(node);
                        }
                    }
                }
            }
            "error" => {
                for node in self.nodes.values() {
                    if node.node_type == NodeType::ErrorCode {
                        let matches = if fuzzy {
                            node.name.to_lowercase().contains(&query_lower)
                        } else {
                            node.name == query
                        };

                        if matches && !seen.contains(&node.id) {
                            seen.insert(node.id.clone());
                            results.push(node);
                        }
                    }
                }
            }
            _ => {
                // Search all
                for node in self.nodes.values() {
                    let matches = if fuzzy {
                        node.name.to_lowercase().contains(&query_lower)
                            || node.branch_name.to_lowercase().contains(&query_lower)
                            || node.id.to_lowercase().contains(&query_lower)
                    } else {
                        node.name == query
                            || node.branch_name == query
                            || node.id == query
                    };

                    if matches && !seen.contains(&node.id) {
                        seen.insert(node.id.clone());
                        results.push(node);
                    }
                }
            }
        }

        results
    }

    /// Get all children of a node
    pub fn get_children(&self,
        node_id: &str,
    ) -> Vec<&TreeNode> {
        self.get_node(node_id)
            .map(|node| {
                node.children
                    .iter()
                    .filter_map(|id| self.get_node(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get parent of a node
    pub fn get_parent(&self,
        node_id: &str,
    ) -> Option<&TreeNode> {
        self.get_node(node_id)
            .and_then(|node| {
                node.parent.as_ref().and_then(|pid| self.get_node(pid))
            })
    }

    /// Get path from root to node
    pub fn get_path_to_root(&self,
        node_id: &str,
    ) -> Vec<&TreeNode> {
        let mut path = Vec::new();
        let mut current = self.get_node(node_id);

        while let Some(node) = current {
            path.push(node);
            current = node.parent.as_ref().and_then(|id| self.get_node(id));
        }

        path.reverse();
        path
    }

    /// Get all nodes
    pub fn get_all_nodes(&self,
    ) -> Vec<&TreeNode> {
        self.nodes.values().collect()
    }

    /// Get statistics
    pub fn get_stats(&self) -> HashMap<String, usize> {
        let mut stats = HashMap::new();

        stats.insert("total".to_string(), self.nodes.len());
        stats.insert(
            "error_codes".to_string(),
            self.type_index.get(&NodeType::ErrorCode).map(|v| v.len()).unwrap_or(0),
        );
        stats.insert(
            "versions".to_string(),
            self.type_index.get(&NodeType::Version).map(|v| v.len()).unwrap_or(0),
        );
        stats.insert(
            "endpoints".to_string(),
            self.type_index.get(&NodeType::Endpoint).map(|v| v.len()).unwrap_or(0),
        );
        stats.insert(
            "categories".to_string(),
            self.type_index.get(&NodeType::Category).map(|v| v.len()).unwrap_or(0),
        );

        stats
    }

    /// Load file lists for each branch
    fn load_branch_files(&mut self,
        repo: &GitRepo,
    ) -> Result<()> {
        for (branch_name, node_id) in &self.branch_index {
            if let Ok(files) = repo.list_files_in_branch(branch_name, None) {
                if let Some(node) = self.nodes.get_mut(node_id) {
                    node.files = files;
                }
            }
        }
        Ok(())
    }

    /// Export tree to JSON
    pub fn to_json(&self,
    ) -> Result<String> {
        let nodes: Vec<&TreeNode> = self.nodes.values().collect();
        Ok(serde_json::to_string_pretty(&nodes)?)
    }
}

impl Default for ApiTree {
    fn default() -> Self {
        Self::new()
    }
}

/// Search index for fast lookups
#[derive(Debug, Clone)]
pub struct SearchIndex {
    index: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl SearchIndex {
    pub fn new() -> Self {
        Self {
            index: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn build(tree: &ApiTree) -> Self {
        let index: Arc<RwLock<HashMap<String, Vec<String>>>> = Arc::new(RwLock::new(HashMap::new()));

        // Build trigram index for fuzzy search
        for node in tree.get_all_nodes() {
            let terms = vec![
                node.name.clone(),
                node.branch_name.clone(),
                node.id.clone(),
            ];

            for term in terms {
                // Index all substrings for fuzzy matching
                let term_lower = term.to_lowercase();
                for i in 0..term_lower.len() {
                    for j in i + 1..=term_lower.len().min(i + 10) {
                        let substring = &term_lower[i..j];
                        let mut idx = index.write().unwrap();
                        idx.entry(substring.to_string())
                            .or_default()
                            .push(node.id.clone());
                    }
                }
            }
        }

        Self { index }
    }

    pub fn search(&self,
        query: &str,
    ) -> Vec<String> {
        let query_lower = query.to_lowercase();
        let index = self.index.read().unwrap();

        index
            .get(&query_lower)
            .cloned()
            .unwrap_or_default()
    }
}

impl Default for SearchIndex {
    fn default() -> Self {
        Self::new()
    }
}
