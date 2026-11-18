//! Command-line mode, for scripts and scheduled cleanups:
//!
//! ```text
//! devsweep scan  [FOLDER...] [--caches] [--older-than DAYS] [--include-caution] [--json]
//! devsweep clean [FOLDER...] [--caches] [--older-than DAYS] [--include-caution] [--dry-run] [--yes] [--json]
//! ```
//!
//! It uses the same rules, keep list and checks as the window. `clean` picks what the
//! window's "Select unused" would pick: not in use, not kept, not marked "Check first"
//! (unless `--include-caution`), and unused for at least `--older-than` days (30 by default).

use crate::rules::Safety;
use crate::scan::{Group, Item};
use crate::{CleanProgress, Emit, ScanProgress, Shared};
use std::io::{IsTerminal, Write};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const USAGE: &str = "\
DevSweep finds build output and package caches that your tools can rebuild.

Usage:
  devsweep scan  [FOLDER...] [options]   List what can be deleted
  devsweep clean [FOLDER...] [options]   Delete it

With no FOLDER, DevSweep scans your home folder and every drive except the system one.

Options:
  --caches            Include global package caches (npm, Cargo, Gradle and others)
  --older-than DAYS   Only pick folders unused for at least this many days (default 30, 0 for any)
  --include-caution   Also pick folders marked \"Check first\" (Python environments, PHP vendor)
  --dry-run           clean: show what would be deleted, delete nothing
  -y, --yes           clean: do not ask before deleting
  --json              Print the result as JSON
  -h, --help          Show this help

Folders on the keep list, and folders a running program uses, are never deleted.
Exit status: 0 when everything picked was deleted, 1 when something failed, 2 for bad usage.
";

#[derive(Debug, PartialEq)]
pub struct Options {
    pub clean: bool,
    pub roots: Vec<String>,
    pub caches: bool,
    pub older_than: u64,
    pub include_caution: bool,
    pub dry_run: bool,
    pub yes: bool,
    pub json: bool,
}

/// Runs the command line when the first argument is a command. Returns the exit code, or
/// `None` to start the window.
pub fn run(args: &[String]) -> Option<i32> {
    let command = args.get(1)?.as_str();
    if !matches!(command, "scan" | "clean" | "help" | "-h" | "--help") {
        return None;
    }
    attach_console();
    if matches!(command, "help" | "-h" | "--help") {
        print!("{USAGE}");
        return Some(0);
    }
    Some(match parse(command, &args[2..]) {
        Ok(Some(opts)) => execute(opts),
        Ok(None) => {
            print!("{USAGE}");
            0
        }
        Err(e) => {
            eprintln!("devsweep: {e}\nRun `devsweep --help` for usage.");
            2
        }
    })
}

/// Parses everything after the command. `Ok(None)` means help was asked for.
pub fn parse(command: &str, args: &[String]) -> Result<Option<Options>, String> {
    let mut o = Options {
        clean: command == "clean",
        roots: Vec::new(),
        caches: false,
        older_than: 30,
        include_caution: false,
        dry_run: false,
        yes: false,
        json: false,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" => return Ok(None),
            "--caches" => o.caches = true,
            "--include-caution" => o.include_caution = true,
            "--dry-run" | "-n" => o.dry_run = true,
            "--yes" | "-y" => o.yes = true,
            "--json" => o.json = true,
            "--older-than" => {
                let v = it.next().ok_or("--older-than needs a number of days")?;
                o.older_than = days(v)?;
            }
            s if s.starts_with("--older-than=") => o.older_than = days(&s["--older-than=".len()..])?,
            s if s.starts_with('-') && s.len() > 1 => return Err(format!("unknown option {s}")),
            folder => o.roots.push(folder.to_string()),
        }
    }
    if !o.clean && (o.dry_run || o.yes) {
        return Err("--dry-run and --yes only apply to clean".into());
    }
    Ok(Some(o))
}

fn days(v: &str) -> Result<u64, String> {
    v.trim_end_matches(['d', 'D'])
        .parse()
        .map_err(|_| format!("--older-than wants a number of days, not {v:?}"))
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// Days since the folder or its project was last used. Empty folders count as unused forever.
pub fn age_days(item: &Item, now: i64) -> f64 {
    if item.last_used == 0 { f64::INFINITY } else { (now - item.last_used) as f64 / 86400.0 }
}

/// Whether `clean` deletes this item: the same rule as "Select unused" in the window.
pub fn picks(item: &Item, o: &Options, now: i64) -> bool {
    item.in_use_by.is_empty()
        && !item.kept
        && (o.include_caution || item.safety != Safety::Caution)
        && (o.caches || item.group == Group::Project)
        && age_days(item, now) >= o.older_than as f64
}

fn execute(mut o: Options) -> i32 {
    if o.roots.is_empty() {
        o.roots = crate::default_roots();
    }
    let shared = Arc::new(Shared::default());
    *shared.keep.lock().unwrap() = crate::store::load_keep();
    let _busy = crate::claim(&shared).expect("nothing else runs in command-line mode");

    let live = std::io::stderr().is_terminal() && !o.json;
    let emit: Emit<ScanProgress> = Arc::new(move |p: ScanProgress| {
        if live {
            let line = match p.phase.as_str() {
                "folders" if p.to_size == 0 => format!("{} folders checked, {} found", p.dirs, p.found),
                "caches" => "Measuring package caches".to_string(),
                "processes" => "Checking running programs".to_string(),
                _ => format!("Measuring {} of {}", p.sized, p.to_size),
            };
            eprint!("\r\x1b[K{line}");
        }
    });
    let result = crate::run_scan(emit, &shared, o.roots.clone(), o.caches);
    if live {
        eprint!("\r\x1b[K");
    }
    for root in &result.missing_roots {
        eprintln!("devsweep: not found: {root}");
    }

    let now = now();
    let items: Vec<&Item> = result.items.iter().filter(|i| o.caches || i.group == Group::Project).collect();
    let picked: Vec<&Item> = items.iter().copied().filter(|i| picks(i, &o, now)).collect();

    if !o.clean {
        if o.json {
            println!("{}", serde_json::to_string_pretty(&items).unwrap_or_default());
        } else {
            print_table(&items, &o, now);
            println!(
                "\n{}, {} in all. {} unused for {} days or more would be picked by clean ({}).",
                folders(items.len()),
                fmt_bytes(items.iter().map(|i| i.frees).sum()),
                picked.len(),
                o.older_than,
                fmt_bytes(picked.iter().map(|i| i.frees).sum()),
            );
        }
        return 0;
    }

    let total: u64 = picked.iter().map(|i| i.frees).sum();
    if picked.is_empty() {
        if o.json {
            println!("{{\"outcomes\":[],\"countedBytes\":0,\"measuredBytes\":0,\"cancelled\":false}}");
        } else {
            println!("Nothing to delete: no folder unused for {} days or more.", o.older_than);
        }
        return 0;
    }
    if !o.json || o.dry_run {
        print_table(&picked, &o, now);
        println!("\n{}, {}.", folders(picked.len()), fmt_bytes(total));
    }
    if o.dry_run {
        println!("Dry run: nothing was deleted.");
        return 0;
    }
    if !o.yes {
        if !std::io::stdin().is_terminal() {
            eprintln!("devsweep: add --yes to delete without asking.");
            return 2;
        }
        print!("Delete {} permanently? [y/N] ", folders(picked.len()));
        let _ = std::io::stdout().flush();
        let mut answer = String::new();
        let _ = std::io::stdin().read_line(&mut answer);
        if !matches!(answer.trim().to_lowercase().as_str(), "y" | "yes") {
            println!("Nothing was deleted.");
            return 0;
        }
    }

    let ids: Vec<String> = picked.iter().map(|i| i.id.clone()).collect();
    let progress = |p: CleanProgress| {
        if live && !p.current.is_empty() {
            eprint!("\r\x1b[KDeleting {} of {}: {}", p.done + 1, p.total, p.current);
        }
    };
    let report = crate::run_clean(&progress, &shared, ids);
    if live {
        eprint!("\r\x1b[K");
    }
    let failed: Vec<_> = report.outcomes.iter().filter(|o| !o.ok).collect();
    if o.json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
    } else {
        for f in &failed {
            println!("Failed: {}\n  {}", f.path, f.error.as_deref().unwrap_or(""));
        }
        println!(
            "Deleted {}. Free space went up by {}.",
            folders(report.outcomes.len() - failed.len()),
            fmt_bytes(report.measured_bytes.max(0) as u64)
        );
    }
    if failed.is_empty() { 0 } else { 1 }
}

/// "1 folder", "3 folders".
fn folders(n: usize) -> String {
    format!("{n} folder{}", if n == 1 { "" } else { "s" })
}

fn print_table(items: &[&Item], o: &Options, now: i64) {
    println!("{:>10}  {:<14}  {:<12}  PATH", "SIZE", "LAST USED", "TYPE");
    for i in items {
        let status = if i.kept {
            "  [kept]".to_string()
        } else if !i.in_use_by.is_empty() {
            format!("  [in use by {}]", i.in_use_by.join(", "))
        } else if i.safety == Safety::Caution {
            "  [check first]".to_string()
        } else if !o.clean && picks(i, o, now) {
            "  *".to_string()
        } else {
            String::new()
        };
        println!(
            "{:>10}  {:<14}  {:<12}  {}{status}",
            fmt_bytes(i.frees),
            fmt_age(age_days(i, now)),
            i.ecosystem,
            i.path
        );
    }
}

pub fn fmt_bytes(n: u64) -> String {
    if n < 1024 {
        return format!("{n} B");
    }
    let mut v = n as f64 / 1024.0;
    let mut unit = 0;
    let units = ["KB", "MB", "GB", "TB"];
    while v >= 1024.0 && unit < units.len() - 1 {
        v /= 1024.0;
        unit += 1;
    }
    if v >= 99.95 { format!("{v:.0} {}", units[unit]) } else { format!("{v:.1} {}", units[unit]) }
}

fn fmt_age(days: f64) -> String {
    if days.is_infinite() {
        "empty".into()
    } else if days < 1.0 {
        "today".into()
    } else if days < 2.0 {
        "yesterday".into()
    } else if days < 60.0 {
        format!("{} days ago", days as u64)
    } else if days < 730.0 {
        format!("{} months ago", (days / 30.0) as u64)
    } else {
        format!("{} years ago", (days / 365.0) as u64)
    }
}

/// A release build on Windows has no console of its own. Writing to the terminal it was
/// started from needs attaching to that console first.
#[cfg(windows)]
fn attach_console() {
    use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
    // SAFETY: plain Win32 call with a constant argument. Fails harmlessly with a console.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(windows))]
fn attach_console() {}
