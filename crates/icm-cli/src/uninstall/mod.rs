//! Reverse `icm init`: remove every configuration mutation across detected
//! AI tools, with timestamped backups, dry-run preview, audit, and check.
//!
//! Issue #229: <https://github.com/rtk-ai/icm/issues/229>.
//!
//! See the crate-level docs at `crates/icm-cli/src/uninstall/locations.rs`
//! for the catalog of paths mirrored from `cmd_init`. The high-level flow
//! is `build_locations -> discover::scan -> report or mutate -> verify`.

use std::path::PathBuf;

use anyhow::Result;
use clap::Args;

/// CLI surface for `icm uninstall`. Kept here so the rest of the crate only
/// imports `UninstallOpts` from this module.
#[derive(Args, Debug, Clone)]
pub struct UninstallOpts {
    /// Preview removals without modifying anything. Always exits 0.
    #[arg(long)]
    pub dry_run: bool,

    /// Group output by file with full discovery detail. Read-only, exits 0.
    #[arg(long)]
    pub audit: bool,

    /// Exit 0 iff no ICM residue is found. No mutation, no backup.
    #[arg(long)]
    pub check: bool,

    /// Also delete the SQLite memory database and the fastembed model cache.
    /// Off by default — your personal memories are preserved.
    #[arg(long)]
    pub purge_data: bool,

    /// Additionally scan this project tree for free-form ICM references in
    /// instruction files (CLAUDE.md, AGENTS.md, .windsurfrules, etc.).
    #[arg(long, value_name = "PATH")]
    pub scan_dir: Option<PathBuf>,

    /// Skip the interactive confirmation prompt.
    #[arg(short = 'y', long)]
    pub yes: bool,

    /// Override the backup root. Defaults to `~/.icm-uninstall-backups/<ts>`.
    #[arg(long, value_name = "PATH")]
    pub backup_dir: Option<PathBuf>,

    /// Disable backups entirely. Not recommended.
    #[arg(long)]
    pub no_backup: bool,
}

/// Exit codes published in `--help`.
///
/// | code | meaning |
/// |------|---------|
/// | 0    | clean / dry-run / audit succeeded |
/// | 1    | `--check` found residue |
/// | 2    | user declined the confirmation prompt |
/// | 3    | partial success — residue remains after mutation (e.g. ambiguous YAML) |
/// | 4    | I/O or parse error during mutation |
#[allow(dead_code)] // populated incrementally across follow-up commits in this PR
pub mod exit_codes {
    pub const CLEAN: i32 = 0;
    pub const CHECK_RESIDUE: i32 = 1;
    pub const USER_DECLINED: i32 = 2;
    pub const PARTIAL: i32 = 3;
    pub const MUTATION_ERROR: i32 = 4;
}

/// Entry point. Returns the process exit code; the caller is responsible
/// for invoking `std::process::exit`.
pub fn run(opts: UninstallOpts) -> Result<i32> {
    // Scaffold stage — subsequent commits flesh out discover/mutate/report.
    println!("icm uninstall (scaffold) — opts: {opts:#?}");
    println!();
    println!("This command is being implemented incrementally. See issue #229.");
    Ok(exit_codes::CLEAN)
}
