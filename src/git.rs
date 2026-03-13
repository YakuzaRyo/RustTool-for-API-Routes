use anyhow::{Context, Result};
use git2::{BranchType, ObjectType, Repository, Signature, Tree};
use std::path::Path;

pub struct GitRepo {
    repo: Repository,
}

impl GitRepo {
    /// Open a Git repository at the given path
    pub fn open(path: &str) -> Result<Self> {
        let repo = Repository::open(path)
            .with_context(|| format!("Failed to open Git repository at: {}", path))?;
        Ok(Self { repo })
    }

    /// Check if a branch exists
    pub fn branch_exists(&self, name: &str) -> Result<bool> {
        Ok(self.repo.find_branch(name, BranchType::Local).is_ok())
    }

    /// Get current branch name
    pub fn current_branch(&self) -> Result<String> {
        let head = self.repo.head()?;
        let branch = head.shorthand().context("Failed to get branch name")?;
        Ok(branch.to_string())
    }

    /// Create a new branch from the current HEAD
    #[allow(dead_code)]
    pub fn create_branch(&self, name: &str) -> Result<()> {
        let head = self.repo.head()?;
        let commit = head.peel_to_commit()?;
        self.repo.branch(name, &commit, false)?;
        Ok(())
    }

    /// Create a branch from a specific branch
    pub fn create_branch_from(&self, new_branch: &str, from_branch: &str) -> Result<()> {
        // Find the source branch
        let branch = self.repo.find_branch(from_branch, BranchType::Local)?;
        let commit = branch.get().peel_to_commit()?;
        self.repo.branch(new_branch, &commit, false)?;
        Ok(())
    }

    /// Checkout a branch
    pub fn checkout(&self, branch_name: &str) -> Result<()> {
        let branch = self.repo.find_branch(branch_name, BranchType::Local)?;
        let tree = branch.get().peel_to_tree()?;
        let mut checkout_opts = git2::build::CheckoutBuilder::new();
        checkout_opts.force();
        checkout_opts.remove_untracked(true);
        self.repo
            .checkout_tree(tree.as_object(), Some(&mut checkout_opts))?;
        self.repo.set_head(&format!("refs/heads/{}", branch_name))?;
        Ok(())
    }

    /// Create and checkout a new branch
    #[allow(dead_code)]
    pub fn checkout_new_branch(&self, name: &str) -> Result<()> {
        self.create_branch(name)?;
        self.checkout(name)?;
        Ok(())
    }

    /// Create and checkout a new branch from specific branch
    pub fn checkout_new_branch_from(&self, new_branch: &str, from_branch: &str) -> Result<()> {
        self.create_branch_from(new_branch, from_branch)?;
        self.checkout(new_branch)?;
        Ok(())
    }

    /// Get all local branches
    pub fn list_branches(&self) -> Result<Vec<(String, String)>> {
        let mut branches = Vec::new();
        for branch in self.repo.branches(Some(BranchType::Local))? {
            let (branch, _) = branch?;
            let name = branch.name()?.unwrap_or("unknown").to_string();
            let oid = branch
                .get()
                .target()
                .map(|o| o.to_string())
                .unwrap_or_default();
            branches.push((name, oid));
        }
        Ok(branches)
    }

    /// Get branches matching a pattern
    #[allow(dead_code)]
    pub fn list_branches_matching(&self, pattern: &str) -> Result<Vec<String>> {
        let branches = self.list_branches()?;
        let filtered: Vec<String> = branches
            .into_iter()
            .filter(|(name, _)| name.starts_with(pattern))
            .map(|(name, _)| name)
            .collect();
        Ok(filtered)
    }

    /// Commit files with a message
    pub fn commit(&self, message: &str) -> Result<()> {
        let mut index = self.repo.index()?;
        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = self.repo.find_tree(tree_id)?;

        let signature = Signature::now("API Routes Manager", "arm@localhost")?;

        let parent_commit = self.repo.head()?.peel_to_commit()?;

        self.repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &[&parent_commit],
        )?;

        Ok(())
    }

    /// Commit with files specified
    pub fn commit_files(&self, paths: &[&Path], message: &str) -> Result<()> {
        let mut index = self.repo.index()?;

        // Convert paths to glob patterns for add_all
        let patterns: Vec<String> = paths
            .iter()
            .map(|p| p.to_str().unwrap_or("*").to_string())
            .collect();

        // Add files to index using add_all with glob patterns
        let pattern_refs: Vec<&str> = patterns.iter().map(|s| s.as_str()).collect();
        index.add_all(pattern_refs.iter(), git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = self.repo.find_tree(tree_id)?;

        let signature = Signature::now("API Routes Manager", "arm@localhost")?;
        let parent_commit = self.repo.head()?.peel_to_commit()?;

        self.repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &[&parent_commit],
        )?;

        Ok(())
    }

    /// Check if an ancestor relationship exists
    #[allow(dead_code)]
    pub fn is_ancestor(&self, ancestor: &str, descendant: &str) -> Result<bool> {
        let ancestor_commit = self.repo.revparse_single(ancestor)?.peel_to_commit()?;
        let descendant_commit = self.repo.revparse_single(descendant)?.peel_to_commit()?;

        Ok(self
            .repo
            .graph_descendant_of(descendant_commit.id(), ancestor_commit.id())?)
    }

    /// Get the merge base of two branches
    #[allow(dead_code)]
    pub fn merge_base(&self, branch1: &str, branch2: &str) -> Result<git2::Oid> {
        let commit1 = self.repo.revparse_single(branch1)?.peel_to_commit()?;
        let commit2 = self.repo.revparse_single(branch2)?.peel_to_commit()?;
        Ok(self.repo.merge_base(commit1.id(), commit2.id())?)
    }

    /// Get file content from a branch
    #[allow(dead_code)]
    pub fn get_file_from_branch(&self, branch: &str, path: &str) -> Result<Option<String>> {
        let branch_ref = self.repo.find_branch(branch, BranchType::Local)?;
        let commit = branch_ref.get().peel_to_commit()?;
        let tree = commit.tree()?;

        match tree.get_path(Path::new(path)) {
            Ok(entry) => {
                let blob = entry.to_object(&self.repo)?.peel_to_blob()?;
                let content = std::str::from_utf8(blob.content())?.to_string();
                Ok(Some(content))
            }
            Err(_) => Ok(None),
        }
    }

    /// List all files in a branch
    #[allow(dead_code)]
    pub fn list_files_in_branch(
        &self,
        branch: &str,
        extension: Option<&str>,
    ) -> Result<Vec<String>> {
        let branch_ref = self.repo.find_branch(branch, BranchType::Local)?;
        let commit = branch_ref.get().peel_to_commit()?;
        let tree = commit.tree()?;

        let mut files = Vec::new();
        self.walk_tree(&tree, "", &mut files, extension)?;

        Ok(files)
    }

    #[allow(dead_code)]
    fn walk_tree(
        &self,
        tree: &Tree,
        prefix: &str,
        files: &mut Vec<String>,
        extension: Option<&str>,
    ) -> Result<()> {
        for entry in tree {
            let name = entry.name().unwrap_or("unknown");
            let path = if prefix.is_empty() {
                name.to_string()
            } else {
                format!("{}/{}", prefix, name)
            };

            match entry.kind() {
                Some(ObjectType::Tree) => {
                    let subtree = entry.to_object(&self.repo)?.peel_to_tree()?;
                    self.walk_tree(&subtree, &path, files, extension)?;
                }
                Some(ObjectType::Blob) => {
                    if let Some(ext) = extension {
                        if path.ends_with(ext) {
                            files.push(path);
                        }
                    } else {
                        files.push(path);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Initialize a new repository if needed
    pub fn init(path: &str) -> Result<Self> {
        let repo = Repository::init(path)?;
        Ok(Self { repo })
    }

    /// Check if this is a valid git repository
    pub fn is_valid(path: &str) -> bool {
        Repository::open(path).is_ok()
    }

    /// Get repository path
    #[allow(dead_code)]
    pub fn path(&self) -> &Path {
        self.repo.path()
    }

    /// Get workdir path
    #[allow(dead_code)]
    pub fn workdir(&self) -> Option<&Path> {
        self.repo.workdir()
    }

    /// Delete a branch
    #[allow(dead_code)]
    pub fn delete_branch(&self, name: &str) -> Result<()> {
        let mut branch = self.repo.find_branch(name, BranchType::Local)?;
        branch.delete()?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct BranchInfo {
    pub name: String,
    pub branch_type: BranchType,
    pub is_head: bool,
}

/// Initialize a new Git repository with initial commit
pub fn init_repo(path: &str) -> Result<GitRepo> {
    let repo = GitRepo::init(path)?;

    // Create initial commit
    let signature = Signature::now("API Routes Manager", "arm@localhost")?;
    let tree_id = {
        let mut index = repo.repo.index()?;
        index.write_tree()?
    };

    {
        let tree = repo.repo.find_tree(tree_id)?;
        repo.repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &tree,
            &[],
        )?;
    }

    Ok(repo)
}
