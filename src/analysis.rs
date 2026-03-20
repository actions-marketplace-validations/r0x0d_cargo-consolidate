use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::model::{DeclaredDep, DepKind, DepReport, WorkspaceReport};

pub fn find_workspace_root(start: &Path) -> Result<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        let cargo_toml = dir.join("Cargo.toml");
        if cargo_toml.exists() {
            let content = std::fs::read_to_string(&cargo_toml)
                .with_context(|| format!("Failed to read {}", cargo_toml.display()))?;
            let doc: toml::Table = content
                .parse()
                .with_context(|| format!("Failed to parse {}", cargo_toml.display()))?;
            if doc.contains_key("workspace") {
                return Ok(dir);
            }
        }
        if !dir.pop() {
            bail!("No workspace root found from {}", start.display());
        }
    }
}

pub fn discover_members(workspace_root: &Path) -> Result<Vec<PathBuf>> {
    let cargo_toml = workspace_root.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml)
        .with_context(|| format!("Failed to read {}", cargo_toml.display()))?;
    let doc: toml::Table = content.parse()?;

    let workspace = doc
        .get("workspace")
        .and_then(|v| v.as_table())
        .context("No [workspace] table found")?;

    let members = workspace
        .get("members")
        .and_then(|v| v.as_array())
        .context("No workspace.members array found")?;

    let mut paths = Vec::new();
    for member in members {
        let pattern = member.as_str().context("member is not a string")?;
        let full_pattern = workspace_root.join(pattern);
        let pattern_str = full_pattern.to_string_lossy();

        let matched: Vec<_> = glob::glob(&pattern_str)
            .with_context(|| format!("Invalid glob pattern: {pattern_str}"))?
            .filter_map(|entry| entry.ok())
            .filter(|p| p.join("Cargo.toml").exists())
            .collect();

        paths.extend(matched);
    }

    // Also check if the workspace root itself is a crate (has [package])
    if doc.contains_key("package") {
        paths.push(workspace_root.to_path_buf());
    }

    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn parse_deps_from_table(
    table: &toml::Table,
    crate_name: &str,
    crate_path: &Path,
    kind: DepKind,
) -> Vec<DeclaredDep> {
    let mut deps = Vec::new();
    for (name, value) in table {
        let version = match value {
            toml::Value::String(s) => s.clone(),
            toml::Value::Table(t) => {
                // Skip workspace = true
                if t.get("workspace")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    continue;
                }
                // Skip path-only or git-only deps (no version)
                match t.get("version").and_then(|v| v.as_str()) {
                    Some(v) => v.to_string(),
                    None => continue,
                }
            }
            _ => continue,
        };

        deps.push(DeclaredDep {
            name: name.clone(),
            crate_name: crate_name.to_string(),
            crate_path: crate_path.to_path_buf(),
            version,
            kind: kind.clone(),
        });
    }
    deps
}

fn extract_deps_from_doc(
    doc: &toml::Table,
    crate_name: &str,
    crate_path: &Path,
) -> Vec<DeclaredDep> {
    let mut all_deps = Vec::new();

    let sections = [
        ("dependencies", DepKind::Normal),
        ("dev-dependencies", DepKind::Dev),
        ("build-dependencies", DepKind::Build),
    ];

    for (section, kind) in &sections {
        if let Some(toml::Value::Table(table)) = doc.get(*section) {
            all_deps.extend(parse_deps_from_table(
                table,
                crate_name,
                crate_path,
                kind.clone(),
            ));
        }
    }

    // Handle [target.*.dependencies] etc.
    if let Some(toml::Value::Table(targets)) = doc.get("target") {
        for (_target_spec, target_value) in targets {
            if let toml::Value::Table(target_table) = target_value {
                for (section, kind) in &sections {
                    if let Some(toml::Value::Table(table)) = target_table.get(*section) {
                        all_deps.extend(parse_deps_from_table(
                            table,
                            crate_name,
                            crate_path,
                            kind.clone(),
                        ));
                    }
                }
            }
        }
    }

    all_deps
}

fn crate_name_from_path(crate_path: &Path, workspace_root: &Path) -> String {
    // Try to read the package name from Cargo.toml
    let cargo_toml = crate_path.join("Cargo.toml");
    if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
        if let Ok(doc) = content.parse::<toml::Table>() {
            if let Some(name) = doc
                .get("package")
                .and_then(|p| p.as_table())
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
            {
                return name.to_string();
            }
        }
    }
    // Fallback: use relative path
    crate_path
        .strip_prefix(workspace_root)
        .unwrap_or(crate_path)
        .to_string_lossy()
        .to_string()
}

fn get_workspace_deps(workspace_root: &Path) -> Result<BTreeSet<String>> {
    let cargo_toml = workspace_root.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml)?;
    let doc: toml::Table = content.parse()?;

    let mut ws_deps = BTreeSet::new();
    if let Some(deps) = doc
        .get("workspace")
        .and_then(|w| w.as_table())
        .and_then(|w| w.get("dependencies"))
        .and_then(|d| d.as_table())
    {
        for name in deps.keys() {
            ws_deps.insert(name.clone());
        }
    }
    Ok(ws_deps)
}

fn pick_highest_version(versions: &[String]) -> String {
    versions
        .iter()
        .max_by(|a, b| {
            let a_req = semver::VersionReq::parse(a);
            let b_req = semver::VersionReq::parse(b);
            match (a_req, b_req) {
                (Ok(a_req), Ok(b_req)) => a_req.to_string().cmp(&b_req.to_string()),
                _ => a.cmp(b),
            }
        })
        .cloned()
        .unwrap_or_default()
}

fn has_version_mismatch(versions: &[String]) -> bool {
    if versions.len() <= 1 {
        return false;
    }
    // Normalize and compare: parse as VersionReq and compare string representations
    let normalized: BTreeSet<String> = versions
        .iter()
        .map(|v| {
            semver::VersionReq::parse(v)
                .map(|r| r.to_string())
                .unwrap_or_else(|_| v.clone())
        })
        .collect();
    normalized.len() > 1
}

pub fn analyze_workspace(workspace_root: &Path) -> Result<WorkspaceReport> {
    let members = discover_members(workspace_root)?;
    let ws_deps = get_workspace_deps(workspace_root)?;

    let mut all_deps: BTreeMap<String, Vec<DeclaredDep>> = BTreeMap::new();
    let mut unique_deps = BTreeSet::new();

    for member_path in &members {
        let cargo_toml = member_path.join("Cargo.toml");
        let content = std::fs::read_to_string(&cargo_toml)
            .with_context(|| format!("Failed to read {}", cargo_toml.display()))?;
        let doc: toml::Table = content
            .parse()
            .with_context(|| format!("Failed to parse {}", cargo_toml.display()))?;

        let crate_name = crate_name_from_path(member_path, workspace_root);
        let deps = extract_deps_from_doc(&doc, &crate_name, member_path);

        for dep in deps {
            unique_deps.insert(dep.name.clone());
            all_deps.entry(dep.name.clone()).or_default().push(dep);
        }
    }

    // Filter to shared deps (2+ usages) and build reports
    let mut shared_deps = Vec::new();
    let mut mismatch_count = 0;

    for (name, usages) in &all_deps {
        // Count unique crates using this dep
        let unique_crates: BTreeSet<&str> = usages.iter().map(|u| u.crate_name.as_str()).collect();
        let in_workspace = ws_deps.contains(name);

        if unique_crates.len() < 2 && !in_workspace {
            continue;
        }

        let versions: Vec<String> = usages.iter().map(|u| u.version.clone()).collect();
        let mismatch = has_version_mismatch(&versions);
        if mismatch {
            mismatch_count += 1;
        }

        let suggested = pick_highest_version(&versions);

        shared_deps.push(DepReport {
            name: name.clone(),
            usages: usages.clone(),
            has_mismatch: mismatch,
            suggested_version: suggested,
            in_workspace,
        });
    }

    // Sort: mismatches first, then alphabetically
    shared_deps.sort_by(|a, b| {
        b.has_mismatch
            .cmp(&a.has_mismatch)
            .then(a.name.cmp(&b.name))
    });

    Ok(WorkspaceReport {
        workspace_root: workspace_root.to_path_buf(),
        crate_count: members.len(),
        unique_dep_count: unique_deps.len(),
        shared_deps,
        mismatch_count,
    })
}

#[cfg(test)]
pub fn analyze_from_toml(
    workspace_toml: &str,
    member_tomls: &[(&str, &str)], // (crate_name, toml_content)
) -> Result<WorkspaceReport> {
    let ws_doc: toml::Table = workspace_toml.parse()?;

    // Extract workspace deps
    let mut ws_deps = BTreeSet::new();
    if let Some(deps) = ws_doc
        .get("workspace")
        .and_then(|w| w.as_table())
        .and_then(|w| w.get("dependencies"))
        .and_then(|d| d.as_table())
    {
        for name in deps.keys() {
            ws_deps.insert(name.clone());
        }
    }

    let mut all_deps: BTreeMap<String, Vec<DeclaredDep>> = BTreeMap::new();
    let mut unique_deps = BTreeSet::new();

    for (crate_name, toml_content) in member_tomls {
        let doc: toml::Table = toml_content.parse()?;
        let deps = extract_deps_from_doc(&doc, crate_name, Path::new(crate_name));

        for dep in deps {
            unique_deps.insert(dep.name.clone());
            all_deps.entry(dep.name.clone()).or_default().push(dep);
        }
    }

    let mut shared_deps = Vec::new();
    let mut mismatch_count = 0;

    for (name, usages) in &all_deps {
        let unique_crates: BTreeSet<&str> = usages.iter().map(|u| u.crate_name.as_str()).collect();
        let in_workspace = ws_deps.contains(name);

        if unique_crates.len() < 2 && !in_workspace {
            continue;
        }

        let versions: Vec<String> = usages.iter().map(|u| u.version.clone()).collect();
        let mismatch = has_version_mismatch(&versions);
        if mismatch {
            mismatch_count += 1;
        }

        let suggested = pick_highest_version(&versions);

        shared_deps.push(DepReport {
            name: name.clone(),
            usages: usages.clone(),
            has_mismatch: mismatch,
            suggested_version: suggested,
            in_workspace,
        });
    }

    shared_deps.sort_by(|a, b| {
        b.has_mismatch
            .cmp(&a.has_mismatch)
            .then(a.name.cmp(&b.name))
    });

    Ok(WorkspaceReport {
        workspace_root: PathBuf::from("."),
        crate_count: member_tomls.len(),
        unique_dep_count: unique_deps.len(),
        shared_deps,
        mismatch_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_version_shared_dep() {
        let ws = r#"
[workspace]
members = ["a", "b"]
"#;
        let a = r#"
[package]
name = "a"

[dependencies]
serde = "1.0"
"#;
        let b = r#"
[package]
name = "b"

[dependencies]
serde = "1.0"
"#;
        let report = analyze_from_toml(ws, &[("a", a), ("b", b)]).unwrap();
        assert_eq!(report.shared_deps.len(), 1);
        assert_eq!(report.shared_deps[0].name, "serde");
        assert!(!report.shared_deps[0].has_mismatch);
        assert_eq!(report.mismatch_count, 0);
    }

    #[test]
    fn test_version_mismatch() {
        let ws = r#"
[workspace]
members = ["a", "b"]
"#;
        let a = r#"
[package]
name = "a"

[dependencies]
tokio = "1.35"
"#;
        let b = r#"
[package]
name = "b"

[dependencies]
tokio = "1.37"
"#;
        let report = analyze_from_toml(ws, &[("a", a), ("b", b)]).unwrap();
        assert_eq!(report.shared_deps.len(), 1);
        assert!(report.shared_deps[0].has_mismatch);
        assert_eq!(report.mismatch_count, 1);
    }

    #[test]
    fn test_workspace_true_skipped() {
        let ws = r#"
[workspace]
members = ["a", "b"]

[workspace.dependencies]
serde = "1.0"
"#;
        let a = r#"
[package]
name = "a"

[dependencies]
serde = { workspace = true }
"#;
        let b = r#"
[package]
name = "b"

[dependencies]
serde = { workspace = true }
"#;
        let report = analyze_from_toml(ws, &[("a", a), ("b", b)]).unwrap();
        // Both use workspace = true, so no declared deps to report
        assert!(report.shared_deps.is_empty());
    }

    #[test]
    fn test_single_crate_dep_not_reported() {
        let ws = r#"
[workspace]
members = ["a", "b"]
"#;
        let a = r#"
[package]
name = "a"

[dependencies]
serde = "1.0"
unique-to-a = "0.1"
"#;
        let b = r#"
[package]
name = "b"

[dependencies]
serde = "1.0"
unique-to-b = "0.2"
"#;
        let report = analyze_from_toml(ws, &[("a", a), ("b", b)]).unwrap();
        assert_eq!(report.shared_deps.len(), 1);
        assert_eq!(report.shared_deps[0].name, "serde");
    }

    #[test]
    fn test_dev_and_build_deps() {
        let ws = r#"
[workspace]
members = ["a", "b"]
"#;
        let a = r#"
[package]
name = "a"

[dev-dependencies]
tempfile = "3"
"#;
        let b = r#"
[package]
name = "b"

[build-dependencies]
tempfile = "3"
"#;
        let report = analyze_from_toml(ws, &[("a", a), ("b", b)]).unwrap();
        assert_eq!(report.shared_deps.len(), 1);
        assert_eq!(report.shared_deps[0].name, "tempfile");
        assert!(!report.shared_deps[0].has_mismatch);
    }

    #[test]
    fn test_path_only_dep_skipped() {
        let ws = r#"
[workspace]
members = ["a", "b"]
"#;
        let a = r#"
[package]
name = "a"

[dependencies]
my-lib = { path = "../my-lib" }
"#;
        let b = r#"
[package]
name = "b"

[dependencies]
my-lib = { path = "../my-lib" }
"#;
        let report = analyze_from_toml(ws, &[("a", a), ("b", b)]).unwrap();
        assert!(report.shared_deps.is_empty());
    }

    #[test]
    fn test_already_in_workspace_shown() {
        let ws = r#"
[workspace]
members = ["a"]

[workspace.dependencies]
serde = "1.0"
"#;
        let a = r#"
[package]
name = "a"

[dependencies]
serde = "1.0"
"#;
        let report = analyze_from_toml(ws, &[("a", a)]).unwrap();
        // Single crate but in workspace deps, so it should be shown
        assert_eq!(report.shared_deps.len(), 1);
        assert!(report.shared_deps[0].in_workspace);
    }

    #[test]
    fn test_table_dep_with_version() {
        let ws = r#"
[workspace]
members = ["a", "b"]
"#;
        let a = r#"
[package]
name = "a"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
"#;
        let b = r#"
[package]
name = "b"

[dependencies]
serde = "1.0"
"#;
        let report = analyze_from_toml(ws, &[("a", a), ("b", b)]).unwrap();
        assert_eq!(report.shared_deps.len(), 1);
        assert!(!report.shared_deps[0].has_mismatch);
    }

    #[test]
    fn test_target_deps() {
        let ws = r#"
[workspace]
members = ["a", "b"]
"#;
        let a = r#"
[package]
name = "a"

[target.'cfg(windows)'.dependencies]
winapi = "0.3"
"#;
        let b = r#"
[package]
name = "b"

[target.'cfg(windows)'.dependencies]
winapi = "0.3"
"#;
        let report = analyze_from_toml(ws, &[("a", a), ("b", b)]).unwrap();
        assert_eq!(report.shared_deps.len(), 1);
        assert_eq!(report.shared_deps[0].name, "winapi");
    }
}
