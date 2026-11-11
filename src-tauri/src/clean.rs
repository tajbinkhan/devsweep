//! Deletes folders. Never follows symlinks or junctions, and refuses paths that are too
//! close to a drive root or the home folder.

use rayon::prelude::*;
use std::fs;
use std::io;
use std::path::{Component, Path};

/// Why a delete failed. `denied` is set when Windows refused access, which running
/// DevSweep as administrator may fix.
#[derive(Debug)]
pub struct Failure {
    pub message: String,
    pub denied: bool,
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Failure { message, denied: false }
    }
}

impl From<&str> for Failure {
    fn from(message: &str) -> Self {
        message.to_string().into()
    }
}

/// Last line of defence, checked right before anything is deleted.
pub fn guard(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("Path is not absolute".into());
    }
    let named = path
        .components()
        .filter(|c| matches!(c, Component::Normal(_)))
        .count();
    if named < 2 {
        return Err("Refusing to delete a folder this close to the drive root".into());
    }
    if let Some(home) = dirs::home_dir() {
        if same(path, &home) || home.starts_with(path) {
            return Err("Refusing to delete the home folder or a folder that contains it".into());
        }
    }
    if let Some(win) = std::env::var_os("SystemRoot") {
        if path.starts_with(Path::new(&win)) {
            return Err("Refusing to delete inside the Windows folder".into());
        }
    }
    let md = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if md.file_type().is_symlink() || is_reparse(&md) {
        return Err("Path is a link or junction, not a real folder".into());
    }
    if !md.is_dir() {
        return Err("Path is not a folder".into());
    }
    Ok(())
}

fn same(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().to_lowercase().trim_end_matches('\\')
        == b.to_string_lossy().to_lowercase().trim_end_matches('\\')
}

#[cfg(windows)]
fn is_reparse(md: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    md.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse(_md: &fs::Metadata) -> bool {
    false
}

/// Clears the read-only flag on every file below `dir` (git objects in packages often have it).
fn clear_readonly(dir: &Path) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        let Ok(md) = fs::symlink_metadata(entry.path()) else { continue };
        if md.file_type().is_symlink() || is_reparse(&md) {
            continue;
        }
        if md.is_dir() {
            clear_readonly(&entry.path());
        }
        let mut perms = md.permissions();
        if perms.readonly() {
            #[allow(clippy::permissions_set_readonly_false)]
            perms.set_readonly(false);
            let _ = fs::set_permissions(entry.path(), perms);
        }
    }
}

/// Removes one entry. Links are removed as links; their targets are untouched.
fn remove_entry(path: &Path) -> io::Result<()> {
    let md = fs::symlink_metadata(path)?;
    let link = md.file_type().is_symlink() || is_reparse(&md);
    if !link && md.is_dir() {
        fs::remove_dir_all(path)
    } else if link && is_dir_link(&md) {
        fs::remove_dir(path) // removes the junction or directory symlink, not its target
    } else {
        fs::remove_file(path)
    }
}

/// Directory symlinks and junctions carry the directory attribute on Windows and must be
/// removed with RemoveDirectory, not DeleteFile.
#[cfg(windows)]
fn is_dir_link(md: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    md.file_attributes() & 0x10 != 0
}

#[cfg(not(windows))]
fn is_dir_link(_md: &fs::Metadata) -> bool {
    false
}

fn denied(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::PermissionDenied
}

/// Deletes everything inside `dir`, children in parallel. Returns each failure and whether
/// it was an access-denied error.
fn remove_children(dir: &Path) -> Vec<(String, bool)> {
    let children: Vec<_> = match fs::read_dir(dir) {
        Ok(rd) => rd.flatten().map(|e| e.path()).collect(),
        Err(e) => return vec![(format!("{}: {e}", dir.display()), denied(&e))],
    };
    children
        .par_iter()
        .filter_map(|p| {
            if remove_entry(p).is_ok() || !p.exists() {
                return None;
            }
            // Second attempt after clearing read-only flags.
            if let Ok(md) = fs::symlink_metadata(p) {
                if md.is_dir() {
                    clear_readonly(p);
                } else {
                    let mut perms = md.permissions();
                    #[allow(clippy::permissions_set_readonly_false)]
                    perms.set_readonly(false);
                    let _ = fs::set_permissions(p, perms);
                }
            }
            match remove_entry(p) {
                Ok(()) => None,
                Err(_) if !p.exists() => None,
                Err(e) => Some((format!("{}: {}", p.display(), e), denied(&e))),
            }
        })
        .collect()
}

fn summarize(errors: Vec<(String, bool)>) -> Result<(), Failure> {
    if errors.is_empty() {
        return Ok(());
    }
    let denied = errors.iter().any(|(_, d)| *d);
    let first = &errors[0].0;
    let message = if errors.len() == 1 {
        format!("Could not remove {first}")
    } else {
        format!("Could not remove {} entries, for example {first}", errors.len())
    };
    Err(Failure { message, denied })
}

pub fn remove_dir(path: &Path) -> Result<(), Failure> {
    guard(path)?;
    let mut errors = remove_children(path);
    if errors.is_empty() {
        if let Err(e) = fs::remove_dir(path) {
            if path.exists() {
                errors.push((format!("{}: {e}", path.display()), denied(&e)));
            }
        }
    }
    summarize(errors)
}

pub fn remove_contents(path: &Path) -> Result<(), Failure> {
    guard(path)?;
    summarize(remove_children(path))
}

pub fn pnpm_prune(store: &Path) -> Result<(), String> {
    let mut cmd = if cfg!(windows) {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "pnpm", "store", "prune", "--store-dir"]);
        c
    } else {
        let mut c = std::process::Command::new("pnpm");
        c.args(["store", "prune", "--store-dir"]);
        c
    };
    cmd.arg(store);
    if let Some(parent) = store.parent() {
        cmd.current_dir(parent);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().map_err(|e| format!("Could not run pnpm: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let msg = String::from_utf8_lossy(&out.stderr);
        let msg = if msg.trim().is_empty() { String::from_utf8_lossy(&out.stdout) } else { msg };
        Err(format!("pnpm store prune failed: {}", msg.trim()))
    }
}
