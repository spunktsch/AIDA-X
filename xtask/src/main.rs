//! `cargo xtask bundle aidadx_plugin --release` builds the plugin cdylib and lays it out as
//! `target/bundled/AIDA-X.clap` and `target/bundled/AIDA-X.vst3` (bundle names come from
//! `bundler.toml`). `scripts/validate-plugin.{ps1,sh}` run the validators on the result.
//!
//! This wraps `nih_plug_xtask::{build, bundle}` instead of calling `nih_plug_xtask::main()`
//! because the latter locates the workspace as the *outermost* ancestor directory containing
//! a `Cargo.toml`. That breaks for checkouts nested inside another Cargo project (e.g. git
//! worktrees under `.claude/worktrees/` of the main checkout), where it would silently build
//! and bundle the outer project instead. Here the workspace root is this crate's parent.

use std::path::Path;

use anyhow::{bail, Context, Result};

const USAGE: &str = "Usage:
  cargo xtask bundle <package> [cargo build args]
  cargo xtask bundle -p <package1> -p <package2> ... [cargo build args]

Example: cargo xtask bundle aidadx_plugin --release";

fn main() -> Result<()> {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives in a workspace subdirectory");
    std::env::set_current_dir(workspace_root)
        .with_context(|| format!("Could not change to '{}'", workspace_root.display()))?;

    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path("./Cargo.toml")
        .no_deps()
        .exec()
        .context("Could not run `cargo metadata`")?;
    let target_dir = metadata.target_directory.as_std_path();

    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("bundle") => {
            let (packages, other_args) = split_bundle_args(args)?;
            nih_plug_xtask::build(&packages, &other_args)?;
            for package in &packages {
                nih_plug_xtask::bundle(target_dir, package, &other_args, false)?;
            }
            Ok(())
        }
        Some(command) => bail!("Unknown command '{command}'\n\n{USAGE}"),
        None => bail!("Missing command name\n\n{USAGE}"),
    }
}

/// `bundle foo --release` or `bundle -p foo -p bar --release`, as in `nih_plug_xtask`.
fn split_bundle_args(args: impl Iterator<Item = String>) -> Result<(Vec<String>, Vec<String>)> {
    let mut args = args.peekable();
    let mut packages = Vec::new();
    if let Some(first) = args.next_if(|a| !a.starts_with('-')) {
        packages.push(first);
    } else {
        while args.next_if(|a| a == "-p" || a == "--package").is_some() {
            match args.next() {
                Some(package) => packages.push(package),
                None => bail!("Missing package name after '-p'\n\n{USAGE}"),
            }
        }
    }
    if packages.is_empty() {
        bail!("Missing package name\n\n{USAGE}");
    }
    Ok((packages, args.collect()))
}
