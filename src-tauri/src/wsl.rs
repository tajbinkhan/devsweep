//! WSL 2 virtual disks on Windows. Each Linux distribution, and Docker Desktop's data, lives
//! in a `.vhdx` file that grows as files are written inside it and never shrinks when they
//! are deleted. Compacting the file gives the empty part back to the Windows drive. Nothing
//! inside the disk is removed.
//!
//! Compacting needs WSL shut down and administrator rights (diskpart), so the rows are
//! marked "Check first" and the clean refuses to start without rights before stopping WSL.

// The parsers are only called on Windows; elsewhere the tests still use them.
#![cfg_attr(not(windows), allow(dead_code))]

use crate::scan::Item;
use std::path::{Path, PathBuf};

/// A virtual disk that can be compacted.
#[derive(Clone, Debug, PartialEq)]
pub struct Disk {
    /// The WSL distribution that uses it, or `None` for Docker Desktop's data disk.
    pub distro: Option<String>,
    pub path: PathBuf,
}

impl Disk {
    pub fn label(&self) -> String {
        match &self.distro {
            Some(d) => format!("WSL disk: {d}"),
            None => "Docker Desktop data disk".to_string(),
        }
    }
}

/// Programs that start WSL again as soon as it stops. Compacting would fail while they run.
pub const BLOCKERS: &[&str] = &["docker desktop", "com.docker.backend"];

/// Reads the distributions from `reg query HKCU\...\Lxss /s`: (name, folder, disk file name).
pub fn parse_registry(text: &str) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    let mut current: Option<(Option<String>, Option<String>, Option<String>)> = None;
    let flush = |c: Option<(Option<String>, Option<String>, Option<String>)>, out: &mut Vec<_>| {
        if let Some((Some(name), Some(base), vhd)) = c {
            out.push((name, base, vhd.unwrap_or_else(|| "ext4.vhdx".to_string())));
        }
    };
    for line in text.lines() {
        if line.starts_with("HKEY_") {
            flush(current.take(), &mut out);
            current = Some((None, None, None));
            continue;
        }
        let Some(c) = current.as_mut() else { continue };
        let mut parts = line.trim_start().splitn(3, "    ");
        let (Some(name), Some(kind), Some(value)) = (parts.next(), parts.next(), parts.next()) else { continue };
        if !kind.starts_with("REG_") {
            continue;
        }
        let value = value.trim().to_string();
        match name {
            "DistributionName" => c.0 = Some(value),
            "BasePath" => c.1 = Some(value.strip_prefix(r"\\?\").unwrap_or(&value).to_string()),
            "VhdFileName" => c.2 = Some(value),
            _ => {}
        }
    }
    flush(current, &mut out);
    out
}

/// Decodes what wsl.exe prints. It writes UTF-16 unless `WSL_UTF8=1` is honoured, and older
/// versions ignore that variable.
pub fn decode(bytes: &[u8]) -> String {
    let nuls = bytes.iter().filter(|b| **b == 0).count();
    if bytes.len() >= 2 && nuls * 3 >= bytes.len() {
        let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
        String::from_utf16_lossy(&units).trim_start_matches('\u{feff}').to_string()
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

/// Bytes in use on the root file system, from `df -Pk /`. Works with GNU and busybox df.
pub fn parse_df_used(text: &str) -> Option<u64> {
    let line = text.lines().filter(|l| !l.trim().is_empty()).nth(1)?;
    let used: u64 = line.split_whitespace().nth(2)?.parse().ok()?;
    Some(used * 1024)
}

/// How much compacting a disk file of `file_bytes` should give back when `used` bytes are in
/// use inside it. ext4 keeps some blocks for itself, so a little is held back.
pub fn estimate_frees(file_bytes: u64, used: u64) -> u64 {
    let overhead = (used / 50).max(64 * 1024 * 1024);
    file_bytes.saturating_sub(used + overhead)
}

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::clean::Failure;
    use crate::command;
    use crate::rules::Safety;
    use crate::scan::{Action, Group};
    use std::collections::HashMap;
    use std::fs;
    use std::time::{Duration, UNIX_EPOCH};

    const LXSS: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Lxss";

    /// Runs wsl.exe from the system folder, printing UTF-8.
    fn wsl(args: &[&str], limit: Duration) -> std::io::Result<std::process::Output> {
        let mut cmd = command::system("wsl.exe")?;
        cmd.env("WSL_UTF8", "1").args(args);
        command::output(cmd, limit)
    }

    /// Every WSL disk file on this machine: registered distributions, plus Docker Desktop's
    /// data disk, which Docker mounts itself and does not register.
    pub fn disks() -> Vec<Disk> {
        let mut out = Vec::new();
        let reg = command::system("reg.exe").and_then(|mut cmd| {
            cmd.args(["query", LXSS, "/s"]);
            command::output(cmd, Duration::from_secs(10))
        });
        if let Ok(o) = reg
            && o.status.success()
        {
            for (name, base, vhd) in parse_registry(&String::from_utf8_lossy(&o.stdout)) {
                out.push(Disk { distro: Some(name), path: Path::new(&base).join(vhd) });
            }
        }
        if let Some(local) = dirs::data_local_dir() {
            let docker = local.join("Docker").join("wsl");
            for p in [docker.join("disk").join("docker_data.vhdx"), docker.join("data").join("ext4.vhdx")] {
                out.push(Disk { distro: None, path: p });
            }
        }
        let mut seen = std::collections::HashSet::new();
        out.retain(|d| d.path.is_file() && seen.insert(d.path.to_string_lossy().to_lowercase()));
        out
    }

    /// Distributions running right now.
    pub fn running() -> Vec<String> {
        let Ok(o) = wsl(&["--list", "--running", "--quiet"], Duration::from_secs(10)) else { return Vec::new() };
        if !o.status.success() {
            return Vec::new();
        }
        decode(&o.stdout).lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()
    }

    /// Bytes in use inside a running distribution. Never starts one that is stopped.
    fn used_inside(distro: &str) -> Option<u64> {
        let o = wsl(&["--distribution", distro, "--user", "root", "--", "df", "-Pk", "/"], Duration::from_secs(20)).ok()?;
        o.status.success().then(|| parse_df_used(&String::from_utf8_lossy(&o.stdout)))?
    }

    fn key(p: &Path) -> String {
        p.to_string_lossy().to_lowercase()
    }

    /// The disks as list rows. `docker_desktop_used` is what Docker Desktop reports it
    /// stores, when it runs. Estimates measured now are written to `estimates`, and an older
    /// one is used for a disk that cannot be looked into now.
    pub fn scan(docker_desktop_used: Option<u64>, estimates: &mut HashMap<String, (u64, i64)>, now: i64) -> Vec<Item> {
        let running = running();
        disks()
            .into_iter()
            .filter_map(|d| {
                let md = fs::metadata(&d.path).ok()?;
                let bytes = md.len();
                if bytes < MIN_DISK {
                    return None;
                }
                let mtime = md.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |t| t.as_secs() as i64);
                let live = match &d.distro {
                    Some(name) if running.contains(name) => used_inside(name),
                    Some(_) => None,
                    None => docker_desktop_used,
                };
                if let Some(used) = live {
                    estimates.insert(key(&d.path), (used, now));
                }
                let known = live.map(|u| (u, now)).or_else(|| estimates.get(&key(&d.path)).copied());
                let frees = known.map_or(bytes, |(used, _)| estimate_frees(bytes, used));
                if known.is_some() && frees < 256 * 1024 * 1024 {
                    return None; // already compact
                }
                Some(item(&d, bytes, frees, known.map(|(_, when)| when != now), mtime, &running))
            })
            .collect()
    }

    /// Disk files smaller than this are not worth stopping WSL for.
    const MIN_DISK: u64 = 1024 * 1024 * 1024;

    /// `estimate` is `None` when DevSweep never looked inside the disk, `Some(true)` when the
    /// estimate comes from an earlier scan.
    fn item(d: &Disk, bytes: u64, frees: u64, estimate: Option<bool>, mtime: i64, running: &[String]) -> Item {
        let mut note = match &d.distro {
            Some(_) => "Compacting shuts down WSL first, which stops every running Linux distribution and the programs and containers in it.".to_string(),
            None => "Quit Docker Desktop first. Compacting shuts down WSL, which stops every running Linux distribution.".to_string(),
        };
        note.push_str(" It needs administrator rights and can take several minutes. Nothing inside the disk is deleted.");
        if !running.is_empty() {
            note.push_str(&format!(" Running now: {}.", running.join(", ")));
        }
        let (what, start) = match &d.distro {
            Some(name) => (name.clone(), format!("Start {name} and scan again for an estimate.")),
            None => (
                "Docker Desktop".to_string(),
                "Start Docker Desktop and scan again for an estimate, then quit it before compacting.".to_string(),
            ),
        };
        match estimate {
            None => note.push_str(&format!(
                " {what} was not running during the scan, so DevSweep could not look inside the disk and the size shown is the whole file. Compacting gives back only the part the Linux files do not use. {start}"
            )),
            Some(true) => note.push_str(&format!(" The size is estimated from an earlier scan, while {what} was running.")),
            Some(false) => {}
        }
        let path = d.path.display().to_string();
        Item {
            id: format!("wsl:{path}"),
            group: Group::Cache,
            rule_id: "wsl_disk".to_string(),
            label: d.label(),
            ecosystem: if d.distro.is_some() { "WSL".to_string() } else { "Docker".to_string() },
            path,
            project: None,
            bytes,
            frees,
            files: 1,
            last_write: mtime,
            project_active: 0,
            last_used: mtime,
            safety: Safety::Caution,
            restore: "Nothing to restore. Compacting keeps every file and gives back the empty space".to_string(),
            note: Some(note),
            in_use_by: Vec::new(),
            kept: false,
            action: Action::CompactDisk,
            tools: BLOCKERS.iter().map(|t| t.to_string()).collect(),
            space_on: None,
            in_vm: false,
        }
    }

    /// Writes a diskpart script to the temp folder and hands back a read-only handle that
    /// keeps it locked. The temp folder belongs to the user, and diskpart runs as
    /// administrator, so another program could swap the file between writing and running.
    /// The handle shares only reading, so nothing can change, replace or delete the file while
    /// it is open, and its content is checked through that handle before diskpart sees it.
    pub(crate) fn locked_script(text: &str) -> Result<(std::path::PathBuf, fs::File), String> {
        use std::io::Read;
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 1;
        let n = std::process::id();
        let stamp = std::time::SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let path = std::env::temp_dir().join(format!("devsweep-diskpart-{n}-{stamp}.txt"));
        let fail = |e: std::io::Error| format!("Could not write the diskpart script: {e}");
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .share_mode(0)
            .open(&path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, text.as_bytes()))
            .map_err(fail)?;
        let mut locked = fs::OpenOptions::new().read(true).share_mode(FILE_SHARE_READ).open(&path).map_err(fail)?;
        let mut back = String::new();
        locked.read_to_string(&mut back).map_err(fail)?;
        if back != text {
            drop(locked);
            let _ = fs::remove_file(&path);
            return Err("The diskpart script was changed by another program before it could run".into());
        }
        Ok((path, locked))
    }

    /// Runs a diskpart script. diskpart stops at the first failing line and says why.
    fn diskpart(lines: &[String], limit: Duration) -> Result<(), String> {
        let (script, lock) = locked_script(&format!("{}\nexit\n", lines.join("\n")))?;
        let result = command::system("diskpart.exe")
            .and_then(|mut cmd| {
                cmd.arg("/s").arg(&script);
                command::output(cmd, limit)
            });
        drop(lock);
        let _ = fs::remove_file(&script);
        let out = result.map_err(|e| format!("Could not run diskpart: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            // diskpart prints a banner and then the failing step; the last lines say what went wrong.
            let text = command::message(&out);
            let tail: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).rev().take(2).collect();
            Err(tail.into_iter().rev().collect::<Vec<_>>().join(" "))
        }
    }

    /// Compacts a disk file. Checks again that it is still a WSL disk and that DevSweep has
    /// administrator rights before stopping WSL, so WSL is never shut down for nothing.
    pub fn compact(path: &Path) -> Result<(), Failure> {
        let Some(disk) = disks().into_iter().find(|d| key(&d.path) == key(path)) else {
            return Err("No longer a WSL disk. Scan again first.".into());
        };
        if !crate::elevate::is_elevated() {
            return Err(Failure {
                message: "Compacting a virtual disk needs administrator rights.".into(),
                denied: true,
            });
        }
        // Tell the disk which blocks are free, so compacting can drop them. Only for a
        // distribution that is already running; WSL is about to stop anyway.
        if let Some(name) = &disk.distro
            && running().contains(name)
        {
            let _ = wsl(&["--distribution", name, "--user", "root", "--", "fstrim", "/"], Duration::from_secs(300));
        }
        let out = wsl(&["--shutdown"], Duration::from_secs(120)).map_err(|e| format!("Could not stop WSL: {e}"))?;
        if !out.status.success() {
            return Err(format!("Could not stop WSL: {}", command::message(&out)).into());
        }

        let select = format!("select vdisk file=\"{}\"", disk.path.display());
        // WSL can take a moment to let go of the file after it stops.
        let mut attached = Err(String::new());
        for _ in 0..5 {
            attached = diskpart(&[select.clone(), "attach vdisk readonly".into()], Duration::from_secs(60));
            if attached.is_ok() {
                break;
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        if let Err(e) = attached {
            return Err(format!("Could not open the disk file, probably because a program still uses it: {e}").into());
        }
        let compacted = diskpart(&[select.clone(), "compact vdisk".into()], Duration::from_secs(4 * 60 * 60));
        let detached = diskpart(&[select, "detach vdisk".into()], Duration::from_secs(60));
        match (compacted, detached) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(e), _) => Err(format!("Compacting failed: {e}").into()),
            (Ok(()), Err(e)) => Err(format!(
                "Compacted, but could not detach the disk file: {e}. Restart Windows before starting WSL again."
            )
            .into()),
        }
    }
}

#[cfg(windows)]
pub use imp::{compact, scan};
#[cfg(all(windows, test))]
pub(crate) use imp::locked_script;

#[cfg(not(windows))]
pub fn scan(
    _docker_desktop_used: Option<u64>,
    _estimates: &mut std::collections::HashMap<String, (u64, i64)>,
    _now: i64,
) -> Vec<Item> {
    Vec::new()
}

#[cfg(not(windows))]
pub fn compact(_path: &Path) -> Result<(), crate::clean::Failure> {
    Err("Compacting WSL disks only works on Windows".into())
}
