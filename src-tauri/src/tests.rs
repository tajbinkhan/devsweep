//! Builds a set of fake projects on disk and checks what the scanner finds and what
//! deletion removes. Run with `cargo test`.

use crate::{clean, rules, scan};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

fn file(path: &Path, bytes: usize) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, vec![0u8; bytes]).unwrap();
}

fn fixture(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("devsweep-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

#[cfg(windows)]
fn junction(link: &Path, target: &Path) {
    // mklink wants backslashes and an existing parent folder.
    let link = PathBuf::from(link.to_string_lossy().replace('/', "\\"));
    let target = PathBuf::from(target.to_string_lossy().replace('/', "\\"));
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    let ok = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(&target)
        .output()
        .unwrap()
        .status
        .success();
    assert!(ok, "could not create junction");
}

fn rel(root: &Path, items: &[scan::Item]) -> Vec<String> {
    let mut v: Vec<String> = items
        .iter()
        .map(|i| {
            Path::new(&i.path)
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    v.sort();
    v
}

#[test]
fn finds_only_folders_owned_by_a_project() {
    let t = fixture("find");
    // Real projects
    file(&t.join("web/package.json"), 2);
    file(&t.join("web/node_modules/react/index.js"), 1000);
    file(&t.join("web/node_modules/react/node_modules/x/i.js"), 10);
    file(&t.join("web/.next/cache/a"), 500);
    file(&t.join("web/src/app.ts"), 10);
    file(&t.join("rs/Cargo.toml"), 2);
    file(&t.join("rs/target/CACHEDIR.TAG"), 2);
    file(&t.join("rs/target/debug/incremental/a"), 3000);
    file(&t.join("net/App.csproj"), 2);
    file(&t.join("net/bin/Debug/app.dll"), 100);
    file(&t.join("net/obj/project.assets.json"), 100);
    file(&t.join("py/pyproject.toml"), 2);
    file(&t.join("py/.venv/pyvenv.cfg"), 2);
    file(&t.join("gr/build.gradle"), 2);
    file(&t.join("gr/build/intermediates/x"), 50);

    // Decoys: right name, wrong place
    file(&t.join("decoy/node_modules/a"), 10); // no package.json
    file(&t.join("decoy/target/debug/a"), 10); // no Cargo.toml
    file(&t.join("decoy2/Cargo.toml"), 2);
    file(&t.join("decoy2/target/notes.txt"), 10); // Cargo.toml but not Cargo output
    file(&t.join("decoy3/build.gradle"), 2);
    file(&t.join("decoy3/build/readme.md"), 10); // Gradle project but not Gradle output
    file(&t.join("decoy4/package.json"), 2);
    file(&t.join("decoy4/bin/Debug/x"), 10); // bin without a .NET project

    // Installed programs are never entered
    file(&t.join("AppData/Local/Programs/Editor/resources/app/package.json"), 2);
    file(&t.join("AppData/Local/Programs/Editor/resources/app/node_modules/a"), 10);
    file(&t.join("Games/X/resources/app/package.json"), 2);
    file(&t.join("Games/X/resources/app/node_modules/a"), 10);
    file(&t.join("repo/.git/package.json"), 2);
    file(&t.join("repo/.git/node_modules/a"), 10);

    let cancel = AtomicBool::new(false);
    let progress = scan::Progress::default();
    let items = scan::scan_projects(&[t.clone()], &cancel, &progress);

    assert_eq!(
        rel(&t, &items),
        vec![
            "gr/build",
            "net/bin",
            "net/obj",
            "py/.venv",
            "rs/target",
            "web/.next",
            "web/node_modules",
        ]
    );

    let nm = items.iter().find(|i| i.path.ends_with("node_modules")).unwrap();
    assert_eq!(nm.bytes, 1010, "nested node_modules is measured as part of the outer one");
    assert_eq!(nm.files, 2);
    let venv = items.iter().find(|i| i.path.ends_with(".venv")).unwrap();
    assert_eq!(venv.safety, rules::Safety::Caution);

    let _ = fs::remove_dir_all(&t);
}

#[cfg(windows)]
#[test]
fn deletes_without_following_junctions_and_clears_read_only() {
    let t = fixture("delete");
    file(&t.join("web/package.json"), 2);
    file(&t.join("web/src/app.ts"), 10);
    file(&t.join("web/node_modules/a/index.js"), 100);
    file(&t.join("web/node_modules/pkg/.git/objects/ab"), 100);
    let ro = t.join("web/node_modules/pkg/.git/objects/ab");
    let mut perms = fs::metadata(&ro).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(&ro, perms).unwrap();

    // A junction inside node_modules that points at a folder which must survive.
    file(&t.join("precious/keep.txt"), 10);
    junction(&t.join("web/node_modules/linked"), &t.join("precious"));

    let cancel = AtomicBool::new(false);
    let progress = scan::Progress::default();
    let items = scan::scan_projects(&[t.clone()], &cancel, &progress);
    assert_eq!(rel(&t, &items), vec!["web/node_modules"]);
    assert_eq!(items[0].bytes, 200, "junction target is not counted");

    clean::remove_dir(Path::new(&items[0].path)).unwrap();

    assert!(!t.join("web/node_modules").exists());
    assert!(t.join("precious/keep.txt").exists(), "junction target must survive");
    assert!(t.join("web/package.json").exists());
    assert!(t.join("web/src/app.ts").exists());

    let _ = fs::remove_dir_all(&t);
}

#[test]
fn guard_refuses_dangerous_paths() {
    let t = fixture("guard");
    assert!(clean::guard(Path::new("C:\\")).is_err());
    assert!(clean::guard(Path::new("C:\\Users")).is_err());
    if let Some(home) = dirs::home_dir() {
        assert!(clean::guard(&home).is_err());
    }
    if let Some(win) = std::env::var_os("SystemRoot") {
        assert!(clean::guard(&Path::new(&win).join("Temp")).is_err());
    }
    assert!(clean::guard(Path::new("relative\\path")).is_err());
    fs::create_dir_all(t.join("a/b")).unwrap();
    assert!(clean::guard(&t.join("a/b")).is_ok());
    let _ = fs::remove_dir_all(&t);
}

#[cfg(windows)]
#[test]
fn guard_refuses_a_junction_itself() {
    let t = fixture("guard-junction");
    file(&t.join("real/x.txt"), 1);
    junction(&t.join("proj/node_modules"), &t.join("real"));
    assert!(clean::guard(&t.join("proj/node_modules")).is_err());
    let _ = fs::remove_dir_all(&t);
}

#[test]
fn remove_contents_keeps_the_folder() {
    let t = fixture("contents");
    file(&t.join("cache/sub/a"), 10);
    file(&t.join("cache/b"), 10);
    clean::remove_contents(&t.join("cache")).unwrap();
    assert!(t.join("cache").is_dir());
    assert_eq!(fs::read_dir(t.join("cache")).unwrap().count(), 0);
    let _ = fs::remove_dir_all(&t);
}

#[test]
fn clean_command_only_deletes_scanned_and_still_valid_folders() {
    let t = fixture("clean-cmd");
    file(&t.join("a/package.json"), 2);
    file(&t.join("a/node_modules/x/i.js"), 100);
    file(&t.join("b/package.json"), 2);
    file(&t.join("b/.next/cache/c"), 100);
    file(&t.join("c/Cargo.toml"), 2);
    file(&t.join("c/target/CACHEDIR.TAG"), 2);
    file(&t.join("c/src/main.rs"), 2);

    let cancel = AtomicBool::new(false);
    let progress = scan::Progress::default();
    let items = scan::scan_projects(&[t.clone()], &cancel, &progress);
    assert_eq!(items.len(), 3);
    let shared = std::sync::Arc::new(crate::Shared::default());
    *shared.items.lock().unwrap() = items.iter().map(|i| (i.id.clone(), i.clone())).collect();

    let id = |suffix: &str| items.iter().find(|i| i.path.ends_with(suffix)).unwrap().id.clone();
    // The project file behind c/target disappears between scan and delete.
    fs::remove_file(t.join("c/Cargo.toml")).unwrap();
    let outside = t.join("a/package.json").display().to_string();

    let ids = vec![id("node_modules"), id(".next"), id("target"), outside.clone()];
    let report = crate::run_clean(&|_| {}, &shared, ids);
    let by = |id: &str| report.outcomes.iter().find(|o| o.id == id).unwrap();

    assert!(by(&id("node_modules")).ok);
    assert!(by(&id(".next")).ok);
    assert!(!by(&id("target")).ok, "must refuse once Cargo.toml is gone");
    assert!(!by(&outside).ok, "must refuse a path that was not in the scan");

    assert!(!t.join("a/node_modules").exists());
    assert!(!t.join("b/.next").exists());
    assert!(t.join("c/target").exists());
    assert!(t.join("a/package.json").exists());
    assert!(t.join("c/src/main.rs").exists());
    assert_eq!(report.counted_bytes, 200);

    let _ = fs::remove_dir_all(&t);
}

#[test]
fn incremental_project_detection_survives_rule_recheck() {
    // clean re-runs match_project right before deleting; it must agree with the scan.
    let t = fixture("recheck");
    file(&t.join("rs/Cargo.toml"), 2);
    file(&t.join("rs/target/.rustc_info.json"), 2);
    let rule = rules::match_project(&t.join("rs/target")).unwrap();
    assert_eq!(rule.id, "cargo_target");
    fs::remove_file(t.join("rs/Cargo.toml")).unwrap();
    assert!(rules::match_project(&t.join("rs/target")).is_none());
    let _ = fs::remove_dir_all(&t);
}

#[cfg(windows)]
fn icacls(args: &[&str]) {
    let ok = std::process::Command::new("icacls").args(args).output().unwrap().status.success();
    assert!(ok, "icacls {args:?} failed");
}

#[cfg(windows)]
#[test]
fn access_denied_is_reported_as_needing_admin() {
    let t = fixture("denied");
    file(&t.join("web/package.json"), 2);
    file(&t.join("web/node_modules/locked/a.js"), 10);
    // DOMAIN\user, because a bare name can match the computer account instead.
    let domain = std::env::var("USERDOMAIN").unwrap();
    let user = format!("{domain}\\{}", std::env::var("USERNAME").unwrap());
    let locked = t.join("web/node_modules/locked");
    // Deleting a file needs DELETE on it or "delete child" on its folder. Deny both.
    icacls(&[locked.join("a.js").to_str().unwrap(), "/deny", &format!("{user}:(D)")]);
    icacls(&[locked.to_str().unwrap(), "/deny", &format!("{user}:(DC)")]);

    let err = clean::remove_dir(&t.join("web/node_modules")).unwrap_err();
    assert!(err.denied, "access denied must be flagged: {}", err.message);
    assert!(locked.join("a.js").exists());

    icacls(&[t.join("web").to_str().unwrap(), "/reset", "/T", "/Q"]);
    let _ = fs::remove_dir_all(&t);
}

#[test]
fn ordinary_failures_do_not_ask_for_admin() {
    let failure: clean::Failure = "In use by node".into();
    assert!(!failure.denied);
}
