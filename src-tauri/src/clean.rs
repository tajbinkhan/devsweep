//! Deletes folders. Never follows symlinks or junctions, and refuses paths that are too
//! close to a drive root or the home folder.

use crate::rules;
use rayon::prelude::*;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Why a delete failed. `denied` is set when the OS refused access. On Windows running
/// DevSweep as administrator may fix it; on Linux the files usually belong to root.
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
    if let Some(home) = dirs::home_dir()
        && (same(path, &home) || home.starts_with(path)) {
            return Err("Refusing to delete the home folder or a folder that contains it".into());
        }
    if let Some(win) = std::env::var_os("SystemRoot")
        && path.starts_with(Path::new(&win)) {
            return Err("Refusing to delete inside the Windows folder".into());
        }
    #[cfg(unix)]
    if let Some(dir) = SYSTEM_DIRS
        .iter()
        .find(|d| path.starts_with(d) && !path.starts_with("/run/media"))
    {
        return Err(format!("Refusing to delete inside the system folder {dir}"));
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

/// Folders that belong to the operating system and installed packages on Linux. Global npm
/// packages in /usr/lib/node_modules, for example, look exactly like a project.
#[cfg(unix)]
const SYSTEM_DIRS: &[&str] = &[
    "/bin", "/boot", "/dev", "/efi", "/etc", "/lib", "/lib32", "/lib64", "/libx32", "/nix",
    "/gnu", "/opt", "/proc", "/root", "/run", "/sbin", "/snap", "/srv", "/sys", "/usr", "/var",
];

#[cfg(windows)]
fn same(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().to_lowercase().trim_end_matches('\\')
        == b.to_string_lossy().to_lowercase().trim_end_matches('\\')
}

#[cfg(not(windows))]
fn same(a: &Path, b: &Path) -> bool {
    a == b // Path equality ignores a trailing slash
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
#[cfg(windows)]
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

/// Gives the owner read, write and search permission on `dir` and every folder below it.
/// On Linux a file can only be removed from a writable folder, and some caches (the Go
/// module cache, for one) make all their folders read-only. Files themselves need no change.
#[cfg(unix)]
fn clear_readonly(dir: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let Ok(md) = fs::symlink_metadata(dir) else { return };
    if !md.is_dir() || md.file_type().is_symlink() {
        return;
    }
    let mode = md.permissions().mode();
    if mode & 0o700 != 0o700 {
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(mode | 0o700));
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            clear_readonly(&entry.path());
        }
    }
}

/// Makes `path` removable after a first attempt failed.
fn make_removable(path: &Path) {
    let Ok(md) = fs::symlink_metadata(path) else { return };
    #[cfg(windows)]
    {
        if md.is_dir() {
            clear_readonly(path);
        } else {
            let mut perms = md.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            perms.set_readonly(false);
            let _ = fs::set_permissions(path, perms);
        }
    }
    #[cfg(unix)]
    {
        // Removing an entry needs write permission on the folder that holds it.
        if let Some(parent) = path.parent() {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(pm) = fs::metadata(parent) {
                let mode = pm.permissions().mode();
                if mode & 0o700 != 0o700 {
                    let _ = fs::set_permissions(parent, fs::Permissions::from_mode(mode | 0o700));
                }
            }
        }
        if md.is_dir() && !md.file_type().is_symlink() {
            clear_readonly(path);
        }
    }
}

/// Removes one entry. Links are removed as links; their targets are untouched.
fn remove_entry(path: &Path) -> io::Result<()> {
    let md = fs::symlink_metadata(path)?;
    let link = md.file_type().is_symlink() || is_reparse(&md);
    if !link && md.is_dir() {
        remove_tree(path, SPLIT_DEPTH)
    } else if link && is_dir_link(&md) {
        fs::remove_dir(path) // removes the junction or directory symlink, not its target
    } else {
        fs::remove_file(path)
    }
}

/// How many folder levels below each top entry are deleted in parallel. Deeper than this,
/// the standard library's `remove_dir_all` takes over; it never follows links and guards
/// against a folder being swapped for a link while it runs. Two levels is enough to spread
/// a pnpm `node_modules/.pnpm/<package>` or a Cargo `target/debug/deps` over every core.
const SPLIT_DEPTH: u32 = 2;

/// Deletes a real folder and everything in it, sub-folders in parallel down to `depth`.
fn remove_tree(dir: &Path, depth: u32) -> io::Result<()> {
    if depth == 0 {
        return fs::remove_dir_all(dir);
    }
    let entries: Vec<_> = fs::read_dir(dir)?.flatten().collect();
    let first_error = entries
        .par_iter()
        .filter_map(|entry| {
            let path = entry.path();
            let md = match fs::symlink_metadata(&path) {
                Ok(md) => md,
                Err(e) if e.kind() == io::ErrorKind::NotFound => return None,
                Err(e) => return Some(e),
            };
            let link = md.file_type().is_symlink() || is_reparse(&md);
            let result = if !link && md.is_dir() {
                remove_tree(&path, depth - 1)
            } else if link && is_dir_link(&md) {
                fs::remove_dir(&path)
            } else {
                fs::remove_file(&path)
            };
            result.err().filter(|e| e.kind() != io::ErrorKind::NotFound)
        })
        .reduce_with(|a, _| a);
    match first_error {
        Some(e) => Err(e),
        None => fs::remove_dir(dir),
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
            make_removable(p);
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

/// The name a folder gets while it is being deleted: `node_modules` becomes
/// `node_modules.1731900000.devsweep-trash` in the same parent.
fn trash_path(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_string_lossy();
    if rules::is_trash_name(&name) {
        return None; // already moved aside by an earlier, unfinished delete
    }
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    Some(path.with_file_name(format!("{name}.{secs}{}", rules::TRASH_SUFFIX)))
}

/// Deletes a build folder. The folder is first renamed in place, which is instant, so the
/// project never sees a half-deleted `node_modules` or `target`: if deleting stops part way,
/// what is left sits under the `.devsweep-trash` name and the next scan offers it again.
/// When the rename fails (a file inside is open on Windows, for one), it deletes in place.
pub fn remove_dir(path: &Path) -> Result<(), Failure> {
    guard(path)?;
    let target = match trash_path(path) {
        Some(t) if !t.exists() && fs::rename(path, &t).is_ok() => t,
        _ => path.to_path_buf(),
    };
    let mut errors = remove_children(&target);
    if errors.is_empty()
        && let Err(e) = fs::remove_dir(&target)
            && target.exists() {
                errors.push((format!("{}: {e}", target.display()), denied(&e)));
            }
    let moved = target != path;
    summarize(errors).map_err(|mut f| {
        if moved {
            f.message.push_str(&format!(
                ". The rest was moved to {} and is offered again after the next scan.",
                target.display()
            ));
        }
        f
    })
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
        let mut c = std::process::Command::new(pnpm_program());
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

/// pnpm's path. A Linux app started from the desktop menu does not get the PATH set up by
/// the shell's startup files, so look where pnpm's own installer and corepack put it.
fn pnpm_program() -> std::path::PathBuf {
    let on_path = std::env::var_os("PATH")
        .is_some_and(|p| std::env::split_paths(&p).any(|d| d.join("pnpm").is_file()));
    if !on_path {
        let home = dirs::home_dir().unwrap_or_default();
        let candidates = [
            std::env::var_os("PNPM_HOME").map(|h| Path::new(&h).join("pnpm")),
            dirs::data_local_dir().map(|d| d.join("pnpm").join("pnpm")),
            Some(home.join(".local").join("bin").join("pnpm")),
            Some(home.join(".volta").join("bin").join("pnpm")),
        ];
        if let Some(found) = candidates.into_iter().flatten().find(|c| c.is_file()) {
            return found;
        }
    }
    "pnpm".into()
}
