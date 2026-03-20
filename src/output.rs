use std::collections::BTreeSet;
use std::io::IsTerminal;

use comfy_table::{modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Cell, Color, Table};

use crate::model::WorkspaceReport;

pub struct OutputConfig {
    pub color: bool,
    pub mismatches_only: bool,
}

impl OutputConfig {
    pub fn new(no_color: bool, mismatches_only: bool) -> Self {
        let color =
            !no_color && std::env::var("NO_COLOR").is_err() && std::io::stdout().is_terminal();
        Self {
            color,
            mismatches_only,
        }
    }
}

pub fn print_report(report: &WorkspaceReport, config: &OutputConfig) {
    print_summary(report, config);

    let mismatches: Vec<_> = report
        .shared_deps
        .iter()
        .filter(|d| d.has_mismatch)
        .collect();

    if !mismatches.is_empty() {
        println!();
        if config.color {
            println!(
                "{}",
                owo_colors::OwoColorize::yellow(&"!! Version Mismatches")
            );
        } else {
            println!("!! Version Mismatches");
        }
        println!();

        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS);

        table.set_header(vec!["Dependency", "Versions", "Used In", "Suggested"]);

        for dep in &mismatches {
            let unique_versions: BTreeSet<&str> =
                dep.usages.iter().map(|u| u.version.as_str()).collect();
            let versions_str = unique_versions
                .iter()
                .copied()
                .collect::<Vec<_>>()
                .join(", ");

            let crate_names: BTreeSet<&str> =
                dep.usages.iter().map(|u| u.crate_name.as_str()).collect();
            let crates_str = crate_names.iter().copied().collect::<Vec<_>>().join(", ");

            if config.color {
                table.add_row(vec![
                    Cell::new(&dep.name).fg(Color::Red),
                    Cell::new(&versions_str).fg(Color::Yellow),
                    Cell::new(&crates_str),
                    Cell::new(&dep.suggested_version).fg(Color::Green),
                ]);
            } else {
                table.add_row(vec![
                    &dep.name,
                    &versions_str,
                    &crates_str,
                    &dep.suggested_version,
                ]);
            }
        }

        println!("{table}");
    }

    if !config.mismatches_only {
        let shared: Vec<_> = report
            .shared_deps
            .iter()
            .filter(|d| !d.has_mismatch)
            .collect();

        if !shared.is_empty() {
            println!();
            println!("Shared Dependencies (candidates for [workspace.dependencies])");
            println!();

            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL)
                .apply_modifier(UTF8_ROUND_CORNERS);

            table.set_header(vec!["Dependency", "Version", "Used In"]);

            for dep in &shared {
                let unique_versions: BTreeSet<&str> =
                    dep.usages.iter().map(|u| u.version.as_str()).collect();
                let version_str = unique_versions
                    .iter()
                    .copied()
                    .collect::<Vec<_>>()
                    .join(", ");

                let crate_names: BTreeSet<&str> =
                    dep.usages.iter().map(|u| u.crate_name.as_str()).collect();
                let crates_str = if dep.in_workspace {
                    format!(
                        "(already in workspace) {}",
                        crate_names.iter().copied().collect::<Vec<_>>().join(", ")
                    )
                } else {
                    crate_names.iter().copied().collect::<Vec<_>>().join(", ")
                };

                let dep_label = if dep.in_workspace {
                    format!("* {}", dep.name)
                } else {
                    dep.name.clone()
                };

                if config.color {
                    if dep.in_workspace {
                        table.add_row(vec![
                            Cell::new(&dep_label).fg(Color::DarkGrey),
                            Cell::new(&version_str).fg(Color::DarkGrey),
                            Cell::new(&crates_str).fg(Color::DarkGrey),
                        ]);
                    } else {
                        table.add_row(vec![
                            Cell::new(&dep_label).fg(Color::Cyan),
                            Cell::new(&version_str),
                            Cell::new(&crates_str),
                        ]);
                    }
                } else {
                    table.add_row(vec![&dep_label, &version_str, &crates_str]);
                }
            }

            println!("{table}");
        }
    }
}

fn print_summary(report: &WorkspaceReport, config: &OutputConfig) {
    let shared_count = report.shared_deps.len();

    if config.color {
        use owo_colors::OwoColorize;
        println!("Workspace: {}", report.workspace_root.display().bold());
        println!(
            "Crates analyzed: {} | Unique dependencies: {}",
            report.crate_count.to_string().cyan(),
            report.unique_dep_count.to_string().cyan()
        );
        println!(
            "Shared dependencies: {} | Version mismatches: {}",
            shared_count.to_string().cyan(),
            if report.mismatch_count > 0 {
                report.mismatch_count.to_string().red().to_string()
            } else {
                report.mismatch_count.to_string().green().to_string()
            }
        );
    } else {
        println!("Workspace: {}", report.workspace_root.display());
        println!(
            "Crates analyzed: {} | Unique dependencies: {}",
            report.crate_count, report.unique_dep_count
        );
        println!(
            "Shared dependencies: {} | Version mismatches: {}",
            shared_count, report.mismatch_count
        );
    }
}
