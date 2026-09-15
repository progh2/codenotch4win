//! Background auto-update from GitHub Releases.
//!
//! Only an NSIS-installed copy self-updates: the updater's payload is the setup
//! executable, so running it from the portable build would silently convert the
//! portable copy into an installation. The portable build shows a notice on the
//! notch instead. Checks run at startup and every 24 hours; a failed check waits
//! for the next round rather than retrying. Settings has a "check now" button
//! that runs the same check and reports the outcome in words.

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

const CHECK_SECS: u64 = 24 * 60 * 60;

pub enum Outcome {
    UpToDate,
    /// Downloaded and installed; the caller decides when to restart
    Installed(String),
    /// A newer version exists but this is a portable copy
    PortableNewer(String),
}

/// NSIS leaves its uninstaller next to the exe, and the default per-user install
/// lands under %LOCALAPPDATA% — either mark identifies an installed copy.
fn is_installed_copy() -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    if exe.parent().map(|d| d.join("uninstall.exe").is_file()) == Some(true) {
        return true;
    }
    match (dirs::data_local_dir(), exe.parent()) {
        (Some(local), Some(dir)) => dir.starts_with(local.join("Codenotch")),
        _ => false,
    }
}

pub async fn check_async(app: &AppHandle) -> Result<Outcome, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await {
        Ok(Some(update)) => {
            let version = update.version.clone();
            if !is_installed_copy() {
                return Ok(Outcome::PortableNewer(version));
            }
            crate::applog(&format!("updater: downloading {version}"));
            update
                .download_and_install(|_, _| {}, || {})
                .await
                .map_err(|e| e.to_string())?;
            Ok(Outcome::Installed(version))
        }
        Ok(None) => Ok(Outcome::UpToDate),
        Err(e) => Err(e.to_string()),
    }
}

fn check_blocking(app: &AppHandle) -> Result<Outcome, String> {
    tauri::async_runtime::block_on(check_async(app))
}

fn lang(app: &AppHandle) -> String {
    let st = app.state::<crate::AppState>();
    let l = st.cfg.lock().unwrap().lang.clone();
    l
}

/// "{v} is available — download it from the Releases page", localized
pub fn portable_notice(app: &AppHandle, version: &str) -> String {
    crate::i18n::tr(&lang(app), "update_portable").replace("{v}", version)
}

pub fn start(app: AppHandle) {
    std::thread::spawn(move || loop {
        match check_blocking(&app) {
            Ok(Outcome::Installed(v)) => {
                crate::applog(&format!("updater: {v} installed — restarting"));
                app.restart();
            }
            Ok(Outcome::PortableNewer(v)) => {
                crate::applog(&format!(
                    "updater: {v} is available — portable copy, not self-updating"
                ));
                let _ = app.emit("notice", portable_notice(&app, &v));
            }
            Ok(Outcome::UpToDate) => {}
            Err(e) => crate::applog(&format!("updater: check failed: {e}")),
        }
        std::thread::sleep(std::time::Duration::from_secs(CHECK_SECS));
    });
}
