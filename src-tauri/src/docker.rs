//! Docker's own storage: build cache, images no container uses, stopped containers and
//! volumes no container uses. Docker measures these itself (`docker system df`) and removes
//! them itself (`docker ... prune`). DevSweep never touches Docker's files directly.

use crate::clean::Failure;
use crate::command;
use crate::rules::Safety;
use crate::scan::{Action, Group, Item};
use std::time::Duration;

/// The Docker engine the `docker` command talks to.
#[derive(Debug, PartialEq)]
pub struct Engine {
    /// "linux" or "windows": the kind of containers it runs.
    pub os_type: String,
    /// "Docker Desktop", "Ubuntu 24.04 LTS" and so on.
    pub os: String,
    /// Where the engine keeps its data, as the engine sees it.
    pub root: String,
}

impl Engine {
    /// Whether the engine keeps its data inside a virtual machine. Pruning then frees space
    /// inside the VM's disk file, and the host drive only gets it back once that file is
    /// compacted. Docker Desktop always runs a VM, and so does any Linux engine on Windows.
    pub fn in_vm(&self) -> bool {
        self.os == "Docker Desktop" || (cfg!(not(target_os = "linux")) && self.os_type == "linux")
    }
}

const INFO_FORMAT: &str = "{{.OSType}}\t{{.OperatingSystem}}\t{{.DockerRootDir}}";

/// The running engine. `Ok(None)` when Docker is not installed or not running, which is not
/// worth a message; `Err` when Docker runs but refuses DevSweep, which is.
pub fn engine() -> Result<Option<Engine>, String> {
    let mut cmd = command::new("docker");
    cmd.args(["info", "--format", INFO_FORMAT]);
    let out = match command::output(cmd, Duration::from_secs(20)) {
        Ok(out) => out,
        Err(_) => return Ok(None), // not installed, or the daemon hangs
    };
    if !out.status.success() {
        let msg = command::message(&out);
        if msg.to_lowercase().contains("permission denied") {
            return Err(
                "Docker is installed, but DevSweep may not talk to it, so Docker's images and build cache are not listed. \
                 Add yourself to the docker group (sudo usermod -aG docker \"$USER\"), log in again and scan again."
                    .into(),
            );
        }
        return Ok(None);
    }
    Ok(parse_info(&String::from_utf8_lossy(&out.stdout)))
}

pub fn parse_info(text: &str) -> Option<Engine> {
    let line = text.lines().find(|l| !l.trim().is_empty())?;
    let mut parts = line.trim().split('\t');
    let os_type = parts.next()?.trim().to_string();
    if os_type.is_empty() {
        return None;
    }
    Some(Engine {
        os_type,
        os: parts.next().unwrap_or("").trim().to_string(),
        root: parts.next().unwrap_or("").trim().to_string(),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Images,
    Containers,
    Volumes,
    BuildCache,
}

/// One line of `docker system df`.
#[derive(Debug, PartialEq)]
pub struct Usage {
    pub kind: Kind,
    /// Everything of this kind, in use or not.
    pub size: u64,
    /// What a prune can remove.
    pub reclaimable: u64,
}

/// Reads `docker system df --format '{{json .}}'`, one JSON object per line.
pub fn parse_df(text: &str) -> Vec<Usage> {
    text.lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line.trim()).ok())
        .filter_map(|v| {
            let kind = match v.get("Type")?.as_str()? {
                "Images" => Kind::Images,
                "Containers" => Kind::Containers,
                "Local Volumes" => Kind::Volumes,
                "Build Cache" => Kind::BuildCache,
                _ => return None,
            };
            Some(Usage {
                kind,
                size: parse_size(v.get("Size")?.as_str()?)?,
                reclaimable: parse_size(v.get("Reclaimable")?.as_str()?)?,
            })
        })
        .collect()
}

/// Docker's sizes, such as `252.5MB (15%)`, `184.3kB` or `0B`. Docker counts in powers of
/// 1000; the binary units are accepted too, in case a version prints them.
pub fn parse_size(s: &str) -> Option<u64> {
    let s = s.split_whitespace().next()?;
    let at = s.find(|c: char| !(c.is_ascii_digit() || c == '.'))?;
    let (num, unit) = s.split_at(at);
    let num: f64 = num.parse().ok()?;
    let scale = match unit.to_ascii_lowercase().as_str() {
        "b" => 1.0,
        "kb" => 1e3,
        "mb" => 1e6,
        "gb" => 1e9,
        "tb" => 1e12,
        "pb" => 1e15,
        "kib" => 1024.0,
        "mib" => 1024f64.powi(2),
        "gib" => 1024f64.powi(3),
        "tib" => 1024f64.powi(4),
        _ => return None,
    };
    Some((num * scale).round() as u64)
}

/// Programs whose work a prune of `kind` would get in the way of.
fn tools(kind: Kind) -> Vec<String> {
    match kind {
        // A build running now uses cache entries a prune would remove.
        Kind::BuildCache => vec!["docker-build".into(), "docker-buildx".into()],
        // Docker itself refuses to remove what a container uses.
        _ => Vec::new(),
    }
}

/// The command that cleans `kind`. It is also what the list shows in place of a path.
pub fn prune_args(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::BuildCache => &["builder", "prune", "--all", "--force"],
        Kind::Images => &["image", "prune", "--all", "--force"],
        Kind::Containers => &["container", "prune", "--force"],
        Kind::Volumes => &["volume", "prune", "--all", "--force"],
    }
}

fn shown_command(kind: Kind) -> String {
    let args: Vec<&str> = prune_args(kind).iter().copied().filter(|a| *a != "--force").collect();
    format!("docker {}", args.join(" "))
}

pub fn action(kind: Kind) -> Action {
    match kind {
        Kind::BuildCache => Action::DockerBuildCache,
        Kind::Images => Action::DockerImages,
        Kind::Containers => Action::DockerContainers,
        Kind::Volumes => Action::DockerVolumes,
    }
}

/// What Docker can free, as list rows. Also returns everything Docker stores, used to work
/// out how much of Docker Desktop's disk file is empty.
pub fn scan(engine: &Engine, now: i64) -> (Vec<Item>, u64) {
    let mut cmd = command::new("docker");
    cmd.args(["system", "df", "--format", "{{json .}}"]);
    let Ok(out) = command::output(cmd, Duration::from_secs(60)) else { return (Vec::new(), 0) };
    if !out.status.success() {
        return (Vec::new(), 0);
    }
    let usage = parse_df(&String::from_utf8_lossy(&out.stdout));
    let stored = usage.iter().map(|u| u.size).sum();
    let items = usage.iter().filter(|u| u.reclaimable > 0).map(|u| item(u, engine, now)).collect();
    (items, stored)
}

pub fn item(u: &Usage, engine: &Engine, now: i64) -> Item {
    let in_vm = engine.in_vm();
    let vm_note = "Docker frees this inside its virtual disk. Compact that disk afterwards to give the space back to the drive.";
    let (id, label, safety, restore, note): (&str, &str, Safety, &str, Option<&str>) = match u.kind {
        Kind::BuildCache => ("build-cache", "Docker build cache", Safety::Safe, "Filled again by the next docker build", None),
        Kind::Images => (
            "images",
            "Unused Docker images",
            Safety::Reinstall,
            "Pulled again by docker pull or the next build, needs network",
            Some("Removes every image no container uses, including ones you pulled and have not run yet."),
        ),
        Kind::Containers => (
            "containers",
            "Stopped Docker containers",
            Safety::Caution,
            "Not recoverable. Create them again with docker run or docker compose up",
            Some("A stopped container keeps the files it wrote. Removing it loses them."),
        ),
        Kind::Volumes => (
            "volumes",
            "Unused Docker volumes",
            Safety::Caution,
            "Not recoverable. The data in them is gone for good",
            Some("Volumes often hold database files. Only volumes no container uses are removed, but their data cannot be brought back."),
        ),
    };
    let note = match (note, in_vm) {
        (Some(n), true) => Some(format!("{n} {vm_note}")),
        (Some(n), false) => Some(n.to_string()),
        (None, true) => Some(vm_note.to_string()),
        (None, false) => None,
    };
    Item {
        id: format!("docker:{id}"),
        group: Group::Cache,
        rule_id: format!("docker_{}", id.replace('-', "_")),
        label: label.to_string(),
        ecosystem: "Docker".to_string(),
        path: shown_command(u.kind),
        project: None,
        bytes: u.reclaimable,
        frees: u.reclaimable,
        files: 0,
        last_write: 0,
        project_active: 0,
        // Docker does not record when an image or cache entry was last used. Counting it as
        // used now keeps "unused for N days" from picking it; the window says "not tracked".
        last_used: now,
        safety,
        restore: restore.to_string(),
        note,
        in_use_by: Vec::new(),
        kept: false,
        action: action(u.kind),
        tools: tools(u.kind),
        space_on: if in_vm || engine.root.is_empty() { None } else { Some(engine.root.clone()) },
        in_vm,
    }
}

pub fn kind_of(action: Action) -> Option<Kind> {
    match action {
        Action::DockerBuildCache => Some(Kind::BuildCache),
        Action::DockerImages => Some(Kind::Images),
        Action::DockerContainers => Some(Kind::Containers),
        Action::DockerVolumes => Some(Kind::Volumes),
        _ => None,
    }
}

/// Runs the prune for `kind`. Docker refuses on its own to remove anything a container uses.
pub fn prune(kind: Kind) -> Result<(), Failure> {
    let run = |args: &[&str]| {
        let mut cmd = command::new("docker");
        cmd.args(args);
        command::output(cmd, Duration::from_secs(60 * 60))
    };
    let args = prune_args(kind);
    let mut out = run(args).map_err(|e| format!("Could not run docker: {e}"))?;
    // Docker before 23.0 has no --all for volumes and removes every unused volume without it.
    if !out.status.success() && kind == Kind::Volumes && command::message(&out).contains("unknown flag") {
        out = run(&["volume", "prune", "--force"]).map_err(|e| format!("Could not run docker: {e}"))?;
    }
    if out.status.success() {
        Ok(())
    } else {
        Err(format!("{} failed: {}", shown_command(kind), command::message(&out)).into())
    }
}
