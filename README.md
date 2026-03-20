# cargo-consolidate

A Cargo subcommand that analyzes Rust workspaces to identify shared dependencies and flag version mismatches — helping you consolidate deps into `[workspace.dependencies]`.

**Read-only and safe for CI.** Unlike tools that automatically modify your `Cargo.toml` files, `cargo-consolidate` only reports findings and exits with a non-zero code when mismatches are found.

## Installation

```sh
cargo install cargo-consolidate
```

## Usage

Run from anywhere inside a Cargo workspace:

```sh
cargo consolidate
```

### Example Output

```
Workspace: /home/user/my-project
Crates analyzed: 4 | Unique dependencies: 12
Shared dependencies: 4 | Version mismatches: 2

!! Version Mismatches

╭────────────┬───────────────┬────────────────────────┬───────────╮
│ Dependency ┆ Versions      ┆ Used In                ┆ Suggested │
╞════════════╪═══════════════╪════════════════════════╪═══════════╡
│ serde      ┆ 1.0, ^1.0.197 ┆ api, cli, core, server ┆ ^1.0.197  │
├╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
│ tokio      ┆ 1.35, 1.37    ┆ api, server            ┆ 1.37      │
╰────────────┴───────────────┴────────────────────────┴───────────╯

Shared Dependencies (candidates for [workspace.dependencies])

╭────────────┬─────────┬────────────────╮
│ Dependency ┆ Version ┆ Used In        │
╞════════════╪═════════╪════════════════╡
│ anyhow     ┆ 1       ┆ api, cli, core │
├╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ tempfile   ┆ 3       ┆ cli, core      │
╰────────────┴─────────┴────────────────╯
```

Dependencies already declared in `[workspace.dependencies]` are marked with `*` and dimmed so you can see what's left to consolidate.

## Options

| Flag | Description |
|------|-------------|
| `--path <DIR>` | Path to the workspace root (default: current directory) |
| `--mismatches-only` | Only show version mismatches, skip the shared deps table |
| `--no-color` | Disable colored output (also respects the `NO_COLOR` env var) |

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | No version mismatches found |
| `1` | Version mismatches detected |
| `2` | Error (e.g., no workspace found) |

This makes `cargo-consolidate` ideal for CI pipelines — add it as a check to catch version drift before it becomes a problem.

## What It Detects

- **Shared dependencies**: any dependency used by 2+ workspace crates that isn't yet in `[workspace.dependencies]`
- **Version mismatches**: the same dependency declared with different version requirements across crates
- **Already centralized deps**: dependencies that are already in `[workspace.dependencies]` (shown for completeness)

### What It Skips

- `workspace = true` entries (already inherited)
- Path-only dependencies (`{ path = "..." }` with no version)
- Git-only dependencies (`{ git = "..." }` with no version)
- Dependencies used by only one crate (nothing to consolidate)

## CI Integration

### GitHub Actions (reusable action)

This repository provides a reusable composite action:

```yaml
- uses: r0x0d/cargo-consolidate/.github/actions/unify@v1
  with:
    fail-on-mismatches: "true"  # default
    working-directory: "."       # default
```

### Manual CI step

```yaml
- name: Check dependency consistency
  run: |
    cargo install cargo-consolidate
    cargo consolidate
```

## How It Works

`cargo-consolidate` parses `Cargo.toml` files directly (no `cargo metadata`, no network access) which makes it:

- **Fast** — just TOML parsing, no dependency resolution
- **Reliable** — works even on broken workspaces that don't compile
- **Accurate** — reports the raw declared versions, not resolved ones

The analysis covers all dependency sections: `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]`, and `[target.*.dependencies]`.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.
