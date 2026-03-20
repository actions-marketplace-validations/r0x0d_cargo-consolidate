use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DepKind {
    Normal,
    Dev,
    Build,
}

impl std::fmt::Display for DepKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DepKind::Normal => write!(f, "normal"),
            DepKind::Dev => write!(f, "dev"),
            DepKind::Build => write!(f, "build"),
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DeclaredDep {
    pub name: String,
    pub crate_name: String,
    pub crate_path: PathBuf,
    pub version: String,
    pub kind: DepKind,
}

#[derive(Debug)]
pub struct DepReport {
    pub name: String,
    pub usages: Vec<DeclaredDep>,
    pub has_mismatch: bool,
    pub suggested_version: String,
    pub in_workspace: bool,
}

#[derive(Debug)]
pub struct WorkspaceReport {
    pub workspace_root: PathBuf,
    pub crate_count: usize,
    pub unique_dep_count: usize,
    pub shared_deps: Vec<DepReport>,
    pub mismatch_count: usize,
}
