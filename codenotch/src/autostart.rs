//! Start at sign-in: an HKCU\...\Run registry value (per user, no administrator needed).
//! The command carries --silent: wait in the background, show no bar without sessions, appear when one starts.
//! Implemented with reg.exe, so no new dependency.

use std::process::Command;

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const NAME: &str = "Codenotch";

fn reg(args: &[&str]) -> Option<(bool, String)> {
    let mut c = Command::new("reg");
    c.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    c.output().ok().map(|o| {
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        );
        (o.status.success(), text)
    })
}

pub fn is_enabled() -> bool {
    reg(&["query", RUN_KEY, "/v", NAME])
        .map(|(ok, out)| ok && out.contains(NAME))
        .unwrap_or(false)
}

pub fn enable() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let val = format!("\"{}\" --silent", exe.display());
    match reg(&["add", RUN_KEY, "/v", NAME, "/t", "REG_SZ", "/d", &val, "/f"]) {
        Some((true, _)) => Ok("start at sign-in enabled (silent until a session appears)".into()),
        Some((false, out)) => Err(out),
        None => Err("reg.exe failed to run".into()),
    }
}

/// First-run onboarding: offer — never force — starting with Windows (PRD FR2).
/// Asked exactly once; the answer is remembered in the config either way, and the
/// switch stays available in Settings and the tray menu.
pub fn offer_on_first_run(app: &tauri::AppHandle) {
    use tauri::Manager;
    let app = app.clone();
    std::thread::spawn(move || {
        let st = app.state::<crate::AppState>();
        let lang = {
            let cfg = st.cfg.lock().unwrap();
            if cfg.autostart_offered {
                return;
            }
            cfg.lang.clone()
        };
        // An existing Run entry means the question is already answered
        if !is_enabled()
            && ask_yes_no(
                crate::i18n::tr(&lang, "autostart_offer_title"),
                crate::i18n::tr(&lang, "autostart_offer"),
            )
        {
            match enable() {
                Ok(m) => crate::applog(&format!("onboarding: {m}")),
                Err(e) => crate::applog(&format!("onboarding: autostart failed: {e}")),
            }
        }
        let mut cfg = st.cfg.lock().unwrap();
        cfg.autostart_offered = true;
        crate::config::save(&cfg);
    });
}

#[cfg(windows)]
fn ask_yes_no(title: &str, text: &str) -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, IDYES, MB_ICONQUESTION, MB_TOPMOST, MB_YESNO,
    };
    let title: Vec<u16> = title.encode_utf16().chain([0]).collect();
    let text: Vec<u16> = text.encode_utf16().chain([0]).collect();
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(text.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_YESNO | MB_ICONQUESTION | MB_TOPMOST,
        ) == IDYES
    }
}

#[cfg(not(windows))]
fn ask_yes_no(_title: &str, _text: &str) -> bool {
    false
}

pub fn disable() -> Result<String, String> {
    match reg(&["delete", RUN_KEY, "/v", NAME, "/f"]) {
        Some((true, _)) => Ok("start at sign-in disabled".into()),
        Some((false, out)) => {
            if out.to_lowercase().contains("unable to find") || out.contains("找不到") { // reg.exe answers in the OS language; "找不到" is the Chinese "unable to find"
                Ok("start at sign-in was not enabled".into())
            } else {
                Err(out)
            }
        }
        None => Err("reg.exe failed to run".into()),
    }
}
