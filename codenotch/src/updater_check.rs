//! Background auto-update from GitHub Releases.
//!
//! Only an NSIS-installed copy self-updates: the updater's payload is the setup
//! executable, so running it from the portable build would silently convert the
//! portable copy into an installation. The portable build just logs that a newer
//! version exists. Checks run at startup and every 24 hours; a failed check waits
//! for the next round rather than retrying.

use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

const CHECK_SECS: u64 = 24 * 60 * 60;

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

pub fn start(app: AppHandle) {
    std::thread::spawn(move || loop {
        check_once(&app);
        std::thread::sleep(std::time::Duration::from_secs(CHECK_SECS));
    });
}

fn check_once(app: &AppHandle) {
    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            crate::applog(&format!("updater: unavailable: {e}"));
            return;
        }
    };
    let outcome = tauri::async_runtime::block_on(async {
        match updater.check().await {
            Ok(Some(update)) => {
                if !is_installed_copy() {
                    crate::applog(&format!(
                        "updater: {} is available — portable copy, not self-updating (download it from the Releases page)",
                        update.version
                    ));
                    return Ok(false);
                }
                crate::applog(&format!("updater: downloading {}", update.version));
                update.download_and_install(|_, _| {}, || {}).await.map(|_| true)
            }
            Ok(None) => Ok(false),
            Err(e) => Err(e),
        }
    });
    match outcome {
        Ok(true) => {
            crate::applog("updater: installed — restarting");
            app.restart();
        }
        Ok(false) => {}
        Err(e) => crate::applog(&format!("updater: check failed: {e}")),
    }
}
