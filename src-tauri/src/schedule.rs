//! Scheduled cleanups.
//!
//! The window saves a [`Schedule`] and registers a task with the system's own scheduler:
//! Task Scheduler on Windows, a systemd user timer on Linux, a launchd agent on macOS. The
//! task starts `devsweep scheduled`, which reads the saved schedule, scans, and deletes what
//! "Select unused" would pick, with the same rules, keep list and checks as the window.
//! Folders marked "Check first" are never deleted by a schedule, so WSL disks, Python
//! environments and LM Studio models stay, and Docker rows are never old enough to be picked.
//!
//! The task only holds the program's path. How often it runs is in the task; what it
//! deletes is read from `schedule.json` each time, so a disabled schedule whose task could
//! not be removed does nothing.

use crate::cli::{self, Options};
use crate::scan::Group;
use crate::{Emit, ScanProgress, Shared, command, store};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Every {
    Day,
    /// Sundays.
    Week,
    /// The first of each month.
    Month,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Schedule {
    pub enabled: bool,
    pub every: Every,
    /// Only folders unused for at least this many days are deleted. Never below 1.
    pub older_than: u64,
    /// Also clean global package caches.
    pub caches: bool,
    /// Folders to scan. Empty scans the home folder and every drive except the system one.
    pub roots: Vec<String>,
    /// The program the registered task starts, to notice when DevSweep has moved.
    pub program: Option<String>,
}

impl Default for Schedule {
    fn default() -> Self {
        Schedule { enabled: false, every: Every::Week, older_than: 30, caches: false, roots: Vec::new(), program: None }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Failed {
    pub path: String,
    pub error: String,
}

/// What one scheduled cleanup did.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    /// When it finished, in Unix seconds.
    pub when: i64,
    pub deleted: usize,
    /// What the deleted folders were expected to free.
    pub counted_bytes: u64,
    /// The change in free disk space.
    pub measured_bytes: i64,
    pub failed: Vec<Failed>,
    /// Folders that were not found, and what Docker said if it could not be asked.
    pub notes: Vec<String>,
}

/// The command-line options a schedule stands for.
pub fn options(s: &Schedule) -> Options {
    Options {
        clean: true,
        roots: if s.roots.is_empty() { crate::default_roots() } else { s.roots.clone() },
        caches: s.caches,
        older_than: s.older_than.max(1),
        include_caution: false,
        dry_run: false,
        yes: true,
        json: false,
    }
}

/// `devsweep scheduled`: the cleanup the task starts. Prints nothing, since no one is
/// watching, and records what it did for the window to show.
pub fn run_scheduled() -> i32 {
    let schedule = store::load_schedule();
    if !schedule.enabled {
        return 0;
    }
    let o = options(&schedule);
    let shared = Arc::new(Shared::default());
    *shared.keep.lock().unwrap() = store::load_keep();
    let Ok(_busy) = crate::claim(&shared) else { return 1 };

    let quiet: Emit<ScanProgress> = Arc::new(|_| {});
    let result = crate::run_scan(quiet, &shared, o.roots.clone(), o.caches);
    let now = cli::now();
    let ids: Vec<String> = result
        .items
        .iter()
        .filter(|i| (o.caches || i.group == Group::Project) && cli::picks(i, &o, now))
        .map(|i| i.id.clone())
        .collect();

    let mut notes: Vec<String> = result.missing_roots.iter().map(|r| format!("Not found: {r}")).collect();
    notes.extend(result.notes);
    let mut run = Run { when: now, deleted: 0, counted_bytes: 0, measured_bytes: 0, failed: Vec::new(), notes };
    if !ids.is_empty() {
        let report = crate::run_clean(&|_| {}, &shared, ids);
        run.deleted = report.outcomes.iter().filter(|o| o.ok).count();
        run.counted_bytes = report.counted_bytes;
        run.measured_bytes = report.measured_bytes;
        run.failed = report
            .outcomes
            .into_iter()
            .filter(|o| !o.ok)
            .map(|o| Failed { path: o.path, error: o.error.unwrap_or_default() })
            .collect();
    }
    run.when = cli::now();
    let failed = !run.failed.is_empty();
    let _ = store::add_run(run);
    i32::from(failed)
}

/// The file the task starts. An AppImage runs from a temporary mount that changes every
/// time, so the AppImage file itself is used.
pub fn program() -> Result<PathBuf, String> {
    if let Some(p) = std::env::var_os("APPIMAGE").map(PathBuf::from).filter(|p| p.is_absolute() && p.is_file()) {
        return Ok(p);
    }
    std::env::current_exe().map_err(|e| format!("Could not find DevSweep's own program file: {e}"))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

/// Task Scheduler's definition: at noon, run again as soon as possible after a missed
/// start, never on battery, as the user without administrator rights, for at most 2 hours.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn windows_task_xml(program: &str, every: Every) -> String {
    let when = match every {
        Every::Day => "<ScheduleByDay><DaysInterval>1</DaysInterval></ScheduleByDay>".to_string(),
        Every::Week => "<ScheduleByWeek><DaysOfWeek><Sunday /></DaysOfWeek><WeeksInterval>1</WeeksInterval></ScheduleByWeek>".to_string(),
        Every::Month => {
            let months: String = [
                "January", "February", "March", "April", "May", "June", "July", "August", "September", "October",
                "November", "December",
            ]
            .iter()
            .map(|m| format!("<{m} />"))
            .collect();
            format!("<ScheduleByMonth><DaysOfMonth><Day>1</Day></DaysOfMonth><Months>{months}</Months></ScheduleByMonth>")
        }
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Deletes build output and caches that DevSweep finds unused. Change or turn it off in DevSweep.</Description>
  </RegistrationInfo>
  <Triggers>
    <CalendarTrigger>
      <StartBoundary>2026-01-04T12:00:00</StartBoundary>
      <Enabled>true</Enabled>
      {when}
    </CalendarTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>true</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <StartWhenAvailable>true</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <ExecutionTimeLimit>PT2H</ExecutionTimeLimit>
    <Priority>7</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{}</Command>
      <Arguments>scheduled</Arguments>
    </Exec>
  </Actions>
</Task>
"#,
        xml_escape(program)
    )
}

/// systemd treats `%` and `$` in ExecStart as substitutions, and `"` and `\` as quoting.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn systemd_quote(s: &str) -> String {
    let s = s.replace('\\', "\\\\").replace('"', "\\\"").replace('%', "%%").replace('$', "$$");
    format!("\"{s}\"")
}

/// The service and timer units. `Persistent` runs a start missed while the computer was
/// off once it is on again.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn systemd_units(program: &str, every: Every) -> (String, String) {
    let service = format!(
        "[Unit]\nDescription=DevSweep scheduled cleanup\n\n[Service]\nType=oneshot\nExecStart={} scheduled\nNice=10\nIOSchedulingClass=idle\n",
        systemd_quote(program)
    );
    let calendar = match every {
        Every::Day => "*-*-* 12:00:00",
        Every::Week => "Sun *-*-* 12:00:00",
        Every::Month => "*-*-01 12:00:00",
    };
    let timer = format!(
        "[Unit]\nDescription=Run DevSweep's scheduled cleanup\n\n[Timer]\nOnCalendar={calendar}\nPersistent=true\n\n[Install]\nWantedBy=timers.target\n"
    );
    (service, timer)
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const LAUNCHD_LABEL: &str = "dev.devsweep.app.cleanup";

/// The launchd agent. launchd runs a start missed while the Mac slept when it wakes.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn launchd_plist(program: &str, every: Every) -> String {
    let when = match every {
        Every::Day => String::new(),
        Every::Week => "<key>Weekday</key><integer>0</integer>".to_string(),
        Every::Month => "<key>Day</key><integer>1</integer>".to_string(),
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{LAUNCHD_LABEL}</string>
  <key>ProgramArguments</key>
  <array><string>{}</string><string>scheduled</string></array>
  <key>StartCalendarInterval</key>
  <dict><key>Hour</key><integer>12</integer><key>Minute</key><integer>0</integer>{when}</dict>
  <key>ProcessType</key><string>Background</string>
  <key>LowPriorityIO</key><true/>
  <key>Nice</key><integer>10</integer>
</dict>
</plist>
"#,
        xml_escape(program)
    )
}

const LIMIT: Duration = Duration::from_secs(30);

fn run(cmd: std::process::Command, what: &str) -> Result<(), String> {
    let out = command::output(cmd, LIMIT).map_err(|e| format!("{what}: {e}"))?;
    if out.status.success() { Ok(()) } else { Err(format!("{what}: {}", command::message(&out))) }
}

#[cfg(windows)]
const TASK_NAME: &str = r"\DevSweep\Scheduled cleanup";

#[cfg(windows)]
fn register(program: &Path, every: Every) -> Result<(), String> {
    let xml = windows_task_xml(&program.to_string_lossy(), every);
    // Task Scheduler reads the file as UTF-16, as its first line says.
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(xml.encode_utf16().flat_map(u16::to_le_bytes));
    let file = std::env::temp_dir().join(format!("devsweep-task-{}.xml", std::process::id()));
    std::fs::write(&file, bytes).map_err(|e| format!("Could not write the task definition: {e}"))?;
    let mut cmd = command::system("schtasks.exe").map_err(|e| e.to_string())?;
    cmd.args(["/Create", "/F", "/TN", TASK_NAME, "/XML"]).arg(&file);
    let result = run(cmd, "Task Scheduler refused the task");
    let _ = std::fs::remove_file(&file);
    result
}

#[cfg(windows)]
fn unregister() -> Result<(), String> {
    let mut query = command::system("schtasks.exe").map_err(|e| e.to_string())?;
    query.args(["/Query", "/TN", TASK_NAME]);
    if run(query, "").is_err() {
        return Ok(()); // no task to remove
    }
    let mut cmd = command::system("schtasks.exe").map_err(|e| e.to_string())?;
    cmd.args(["/Delete", "/F", "/TN", TASK_NAME]);
    run(cmd, "Task Scheduler could not remove the task")
}

#[cfg(target_os = "linux")]
fn unit_dir() -> Result<PathBuf, String> {
    Ok(dirs::config_dir().ok_or("No config folder on this system")?.join("systemd").join("user"))
}

#[cfg(target_os = "linux")]
fn systemctl(args: &[&str], what: &str) -> Result<(), String> {
    let mut cmd = command::new("systemctl");
    cmd.arg("--user").args(args);
    run(cmd, what)
}

#[cfg(target_os = "linux")]
fn register(program: &Path, every: Every) -> Result<(), String> {
    let program = program.to_string_lossy();
    systemctl(&["show-environment"], "").map_err(|_| {
        format!(
            "This system has no systemd user session to run the schedule. Add a line like this to crontab -e instead:\n0 12 * * 0 {} scheduled",
            program
        )
    })?;
    let dir = unit_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let (service, timer) = systemd_units(&program, every);
    std::fs::write(dir.join("devsweep-cleanup.service"), service).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("devsweep-cleanup.timer"), timer).map_err(|e| e.to_string())?;
    systemctl(&["daemon-reload"], "systemd could not read the timer")?;
    // `restart` picks up a changed OnCalendar on a timer that was already running.
    systemctl(&["enable", "devsweep-cleanup.timer"], "systemd could not turn the timer on")?;
    systemctl(&["restart", "devsweep-cleanup.timer"], "systemd could not start the timer")
}

#[cfg(target_os = "linux")]
fn unregister() -> Result<(), String> {
    let dir = unit_dir()?;
    if !dir.join("devsweep-cleanup.timer").exists() {
        return Ok(());
    }
    let _ = systemctl(&["disable", "--now", "devsweep-cleanup.timer"], "");
    for name in ["devsweep-cleanup.timer", "devsweep-cleanup.service"] {
        let _ = std::fs::remove_file(dir.join(name));
    }
    let _ = systemctl(&["daemon-reload"], "");
    Ok(())
}

#[cfg(target_os = "macos")]
fn plist_path() -> Result<PathBuf, String> {
    Ok(dirs::home_dir()
        .ok_or("No home folder")?
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{LAUNCHD_LABEL}.plist")))
}

#[cfg(target_os = "macos")]
fn register(program: &Path, every: Every) -> Result<(), String> {
    let path = plist_path()?;
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let mut unload = command::new("launchctl");
    unload.arg("unload").arg(&path);
    let _ = run(unload, ""); // not loaded yet the first time
    std::fs::write(&path, launchd_plist(&program.to_string_lossy(), every)).map_err(|e| e.to_string())?;
    let mut load = command::new("launchctl");
    load.args(["load", "-w"]).arg(&path);
    run(load, "launchd refused the schedule")
}

#[cfg(target_os = "macos")]
fn unregister() -> Result<(), String> {
    let path = plist_path()?;
    if !path.exists() {
        return Ok(());
    }
    let mut unload = command::new("launchctl");
    unload.arg("unload").arg(&path);
    let _ = run(unload, "");
    std::fs::remove_file(&path).map_err(|e| e.to_string())
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn register(_: &Path, _: Every) -> Result<(), String> {
    Err("Scheduled cleanups are not available on this system".into())
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn unregister() -> Result<(), String> {
    Ok(())
}

/// Registers or removes the task to match `s`, then saves it. Nothing is saved when the
/// system scheduler refuses, so the window never shows a schedule that will not run.
pub fn apply(mut s: Schedule) -> Result<Schedule, String> {
    s.older_than = s.older_than.max(1);
    if s.enabled {
        let program = program()?;
        register(&program, s.every)?;
        s.program = Some(program.to_string_lossy().into_owned());
    } else {
        unregister()?;
        s.program = None;
    }
    store::save_schedule(&s)?;
    Ok(s)
}

/// Registers the task again when DevSweep now runs from somewhere else, such as a portable
/// copy that was moved. Skipped in development builds, which would point it at `target`.
pub fn refresh() {
    if cfg!(debug_assertions) {
        return;
    }
    let s = store::load_schedule();
    let Ok(program) = program() else { return };
    if s.enabled && s.program.as_deref() != Some(&*program.to_string_lossy()) {
        let _ = apply(s);
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub schedule: Schedule,
    /// The most recent runs, newest first.
    pub runs: Vec<Run>,
}

#[tauri::command]
pub fn get_schedule() -> Status {
    Status { schedule: store::load_schedule(), runs: store::load_runs().into_iter().take(5).collect() }
}

#[tauri::command]
pub async fn set_schedule(schedule: Schedule) -> Result<Status, String> {
    // schtasks, systemctl and launchctl can take a moment, so this runs off the main thread.
    tauri::async_runtime::spawn_blocking(move || apply(schedule))
        .await
        .map_err(|e| e.to_string())??;
    Ok(get_schedule())
}
