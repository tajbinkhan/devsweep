//! Updates from GitHub Releases.
//!
//! On start the window asks GitHub for `latest.json`, which the release workflow publishes
//! with every release. It names the newest version and, per platform, a download and its
//! signature. The updater checks that signature against the public key in `tauri.conf.json`
//! before installing anything, so a tampered download is refused.
//!
//! Only two kinds of install can replace themselves without asking for more rights:
//!
//! * the Windows installer (NSIS), which installs per user, so the new installer runs
//!   without a UAC prompt and starts DevSweep again when it is done
//! * the Linux AppImage, a single file in the user's own folders
//!
//! The `.deb` and `.rpm` packages update through apt or dnf, which needs root, and the
//! portable zip has no installer. Those builds are told about the new version and offered
//! the download page instead.

use crate::Shared;
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::utils::config::BundleType;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::{Update, UpdaterExt};

/// Where people download DevSweep by hand.
pub const RELEASES_PAGE: &str = "https://github.com/tajbinkhan/devsweep/releases/latest";

/// How this copy of DevSweep can be updated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Download, check and install the new version, then start it.
    Install,
    /// Say a new version exists and link to the download page.
    Download,
}

/// Decides the mode from how DevSweep was packaged and where it runs from.
///
/// The bundler writes the package type into each build, but the portable zip carries the
/// same executable as the installer, so the installer's `uninstall.exe` next to the running
/// executable is what tells an installed copy from an unzipped one.
pub fn mode_for(bundle: Option<BundleType>, next_to_uninstaller: bool, inside_appimage: bool) -> Mode {
    match bundle {
        Some(BundleType::Nsis) if next_to_uninstaller => Mode::Install,
        Some(BundleType::AppImage) if inside_appimage => Mode::Install,
        _ => Mode::Download,
    }
}

pub fn mode() -> Mode {
    let next_to_uninstaller = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.join("uninstall.exe").is_file()))
        .unwrap_or(false);
    let inside_appimage = std::env::var_os("APPIMAGE").is_some();
    mode_for(tauri::utils::platform::bundle_type(), next_to_uninstaller, inside_appimage)
}

/// The update found by the last check, kept for `install_update`.
#[derive(Default)]
pub struct Pending(Mutex<Option<Update>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    /// The release notes from GitHub.
    pub notes: Option<String>,
    pub mode: Mode,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

/// Asks GitHub whether a newer version exists. `None` means this is the newest.
#[tauri::command]
pub async fn check_update(app: AppHandle, pending: State<'_, Pending>) -> Result<Option<UpdateInfo>, String> {
    let updater = app
        .updater_builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let found = updater.check().await.map_err(|e| format!("Could not check for updates: {e}"))?;
    let info = found.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        notes: u.body.clone().filter(|b| !b.trim().is_empty()),
        mode: mode(),
    });
    *pending.0.lock().unwrap() = found;
    Ok(info)
}

/// Downloads the update found by the last check, verifies its signature and installs it.
/// On Windows the installer takes over and this process ends; on Linux the AppImage file is
/// replaced and DevSweep starts again. Refused while a scan or delete runs.
#[tauri::command]
pub async fn install_update(app: AppHandle, pending: State<'_, Pending>, shared: State<'_, Arc<Shared>>) -> Result<(), String> {
    if mode() != Mode::Install {
        return Err("This copy of DevSweep cannot update itself. Download the new version instead.".into());
    }
    let update = pending.0.lock().unwrap().clone().ok_or("Check for updates first")?;
    let _busy = crate::claim(&shared)?;

    let mut downloaded = 0u64;
    let mut last = Instant::now() - Duration::from_secs(1);
    update
        .download_and_install(
            |chunk, total| {
                downloaded += chunk as u64;
                // A few updates a second are plenty for a progress bar.
                if last.elapsed() >= Duration::from_millis(100) || total == Some(downloaded) {
                    last = Instant::now();
                    let _ = app.emit("update-progress", UpdateProgress { downloaded, total });
                }
            },
            || {},
        )
        .await
        .map_err(|e| format!("Could not install the update: {e}"))?;
    app.restart()
}

/// Opens the download page in the browser. The address is fixed here, so the window never
/// needs permission to open arbitrary links.
#[tauri::command]
pub fn open_releases_page(app: AppHandle) -> Result<(), String> {
    app.opener().open_url(RELEASES_PAGE, None::<&str>).map_err(|e| e.to_string())
}
