//! Cursor usage adapter, implemented from the upstream Codenotch's documented behaviour.
//!
//! Data path (same trade-off as upstream: borrow the editor's own session):
//!   1. Credential: the editor keeps its sign-in in the global state database it inherited from
//!      VS Code, `%APPDATA%\Cursor\User\globalStorage\state.vscdb` (SQLite, table ItemTable(key,value)):
//!      `cursorAuth/accessToken` + `cursorAuth/stripeMembershipAuthId`, joined into the cookie
//!      `WorkosCursorSessionToken=<authId>::<token>`. Non-secret identity cache:
//!      `cursorAuth/cachedEmail`, `cursorAuth/stripeMembershipType` (only the plan is shown).
//!   2. Endpoint: `GET https://cursor.com/api/usage-summary` (Cookie + Accept: application/json, 15 s).
//!      Reply:
//!      ```text
//!      { billingCycleEnd, membershipType, isUnlimited,
//!        individualUsage: { plan: { totalPercentUsed, apiPercentUsed, used, limit, breakdown },
//!                           onDemand: { enabled, used, limit } } }
//!      ```
//!      Cursor meters a percentage of the allowance, not requests: the dashboard's
//!      "Included usage · N% used" is totalPercentUsed. On the free plan used/limit are always 0
//!      (the allowance arrives as breakdown.bonus), so reading used/limit would report 10 % as 0 %.
//!      0 is a reading, not a gap (upstream's lesson). "API usage" is listed separately when
//!      apiPercentUsed > 0; "On demand" when onDemand has a real limit.
//!
//! SQLite opening rule: `mode=ro` first (it sees the token the editor just rotated into the WAL),
//! then `immutable=1` (once the editor has exited and the -shm is gone, mode=ro fails to open; by
//! then the WAL has been checkpointed, so ignoring it costs nothing).
//! Read only, never written; token values never reach logs, events or the UI.

use crate::usage::{LimitWindow, UsageSnapshot};
use crate::AppState;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

const ENDPOINT: &str = "https://cursor.com/api/usage-summary";
/// Grok Bot is Cursor's "Sand" product: its own weekly allowance with its own reset,
/// served by the dashboard's Connect RPC rather than usage-summary (endpoint and
/// response shape per mstallone/runway#115).
const SAND_ENDPOINT: &str =
    "https://api2.cursor.sh/aiserver.v1.DashboardService/GetSandUsageStatus";
const POLL_SECS: u64 = 300;

static REFRESH: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn request_refresh() {
    REFRESH.store(true, std::sync::atomic::Ordering::Relaxed);
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Windows: %APPDATA%\Cursor\User\globalStorage\state.vscdb (macOS: ~/Library/Application Support/Cursor/...)
pub fn store_url() -> Option<PathBuf> {
    dirs::config_dir().map(|c| c.join("Cursor").join("User").join("globalStorage").join("state.vscdb"))
}

fn store_path() -> PathBuf {
    crate::config::config_path().with_file_name("cursor.json")
}

pub fn load_persisted() -> UsageSnapshot {
    std::fs::read_to_string(store_path())
        .ok()
        .and_then(|t| serde_json::from_str::<UsageSnapshot>(&t).ok())
        .map(|mut s| {
            if !s.windows.is_empty() {
                s.status = "stale".into();
            }
            s
        })
        .unwrap_or_default()
}

fn persist(s: &UsageSnapshot) {
    if let Ok(t) = serde_json::to_string_pretty(s) {
        let _ = std::fs::write(store_path(), t);
    }
}

pub fn present() -> bool {
    store_url().map(|p| p.is_file()).unwrap_or(false)
}

// ---------------- SQLite, read only ----------------

/// mode=ro first, immutable=1 as the fallback (see the module doc)
fn open_ro(path: &std::path::Path) -> Option<rusqlite::Connection> {
    use rusqlite::OpenFlags;
    if !path.is_file() {
        return None;
    }
    if let Ok(c) = rusqlite::Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        // Actually verify that reads work (with the -shm missing, open can succeed and the first query fail)
        if c.prepare("SELECT 1 FROM ItemTable LIMIT 1").and_then(|mut s| s.query([]).map(|_| ())).is_ok() {
            return Some(c);
        }
    }
    // Only the URI form takes immutable=1; a Windows path becomes file:///C:/... with \ → /
    let mut uri = String::from("file:///");
    uri.push_str(&path.to_string_lossy().replace('\\', "/").trim_start_matches('/').replace('#', "%23").replace('?', "%3F"));
    uri.push_str("?immutable=1");
    rusqlite::Connection::open_with_flags(
        &uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()
}

fn item(conn: &rusqlite::Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM ItemTable WHERE key = ?1", [key], |r| r.get::<_, String>(0))
        .ok()
        .filter(|s| !s.is_empty())
}

struct Creds {
    cookie: String,
    /// Raw access token — the Connect RPC endpoints take it as a Bearer header
    token: String,
    plan: Option<String>,
}

/// Base64url without padding — enough to open the JWT payload, nothing more.
fn b64url_decode(s: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a' + 26) as u32),
            b'0'..=b'9' => Some((c - b'0' + 52) as u32),
            b'-' => Some(62),
            b'_' => Some(63),
            _ => None,
        }
    }
    let bytes = s.trim_end_matches('=').as_bytes();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        if chunk.len() < 2 {
            return None;
        }
        let mut acc: u32 = 0;
        for (j, &c) in chunk.iter().enumerate() {
            acc |= val(c)? << (18 - 6 * j as u32);
        }
        out.push((acc >> 16) as u8);
        if chunk.len() > 2 {
            out.push((acc >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(acc as u8);
        }
    }
    Some(out)
}

/// The `sub` claim of the access token: Auth0/enterprise sign-ins (and accounts migrated after
/// the SpaceX acquisition) have no `cursorAuth/stripeMembershipAuthId`, and the cookie's account
/// id is the same value the JWT already carries (upstream vinzdg/codenotch#34).
fn jwt_sub(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let json: serde_json::Value = serde_json::from_slice(&b64url_decode(payload)?).ok()?;
    json.get("sub")
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Re-read every time: the editor rotates the token, and holding on to an old value signs us out
fn read_credentials() -> Option<Creds> {
    let path = store_url()?;
    let conn = open_ro(&path)?;
    let token = item(&conn, "cursorAuth/accessToken")?;
    let auth_id = item(&conn, "cursorAuth/stripeMembershipAuthId").or_else(|| jwt_sub(&token))?;
    let plan = item(&conn, "cursorAuth/stripeMembershipType");
    Some(Creds { cookie: format!("WorkosCursorSessionToken={auth_id}::{token}"), token, plan })
}

/// For doctor: contains no secret values
pub fn probe() -> String {
    let Some(p) = store_url() else { return "Cursor: cannot locate %APPDATA%".into() };
    if !p.is_file() {
        return format!("Cursor: {} not found (not installed, or not signed in)", p.display());
    }
    match read_credentials() {
        Some(c) => format!(
            "Cursor: session borrowed (cookie {} chars, plan={})",
            c.cookie.len(),
            c.plan.unwrap_or_else(|| "?".into())
        ),
        None => {
            let detail = match open_ro(&p) {
                None => "SQLite failed to open".into(),
                Some(conn) => format!(
                    "accessToken {}, stripeMembershipAuthId {} (editor signed out?)",
                    if item(&conn, "cursorAuth/accessToken").is_some() { "present" } else { "missing" },
                    if item(&conn, "cursorAuth/stripeMembershipAuthId").is_some() { "present" } else { "missing" },
                ),
            };
            format!("Cursor: {} exists but the session could not be borrowed — {}", p.display(), detail)
        }
    }
}

/// Redacted shape of a JSON value: keys, numbers and booleans survive; strings are
/// reduced to their length so no email, id or token can reach doctor.log.
fn redact_shape(v: &serde_json::Value) -> serde_json::Value {
    use serde_json::Value::{Array, Object, String as S};
    match v {
        Object(m) => Object(m.iter().map(|(k, x)| (k.clone(), redact_shape(x))).collect()),
        Array(a) => Array(a.iter().take(3).map(redact_shape).collect()),
        S(s) => S(format!("<str:{}>", s.len())),
        other => other.clone(),
    }
}

/// For doctor: the live usage-summary reply with every string stripped — enough to see
/// which buckets the account's reply actually carries (e.g. a Grok Bot allowance).
pub fn probe_summary() -> String {
    let Some(creds) = read_credentials() else {
        return "Cursor summary: no session to borrow".into();
    };
    let summary = match fetch_once(&creds.cookie) {
        Ok(v) => format!("Cursor usage-summary shape: {}", redact_shape(&v)),
        Err(FetchErr::NeedsAuth) => "Cursor summary: session rejected (401/403)".into(),
        Err(FetchErr::Other(m)) => format!("Cursor summary: {m}"),
    };
    let sand = match fetch_sand(&creds.token) {
        Ok(v) => format!("Grok Bot (GetSandUsageStatus) shape: {}", redact_shape(&v)),
        Err(m) => format!("Grok Bot (GetSandUsageStatus): {m}"),
    };
    format!("{summary}\n  {sand}")
}

// ---------------- Parsing ----------------

fn pct(v: Option<&serde_json::Value>) -> Option<f64> {
    v.and_then(|x| x.as_f64()).map(|p| (p / 100.0).clamp(0.0, 1.0))
}

fn parse_iso(v: Option<&serde_json::Value>) -> Option<u64> {
    v.and_then(|x| x.as_str())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.timestamp_millis().max(0) as u64)
}

/// usage-summary → (windows, note). When there are no windows the note says why (Unlimited / free plan without an allowance)
pub fn parse_summary(v: &serde_json::Value) -> (Vec<LimitWindow>, String) {
    let resets_at = parse_iso(v.get("billingCycleEnd"));
    let usage = v.get("individualUsage").cloned().unwrap_or(serde_json::Value::Null);
    let plan = usage.get("plan").cloned().unwrap_or(serde_json::Value::Null);
    let mut out = Vec::new();
    // Headline = the dashboard number; 0 is a reading too. The dashboard row is Auto usage;
    // totalPercentUsed blends Auto and API, so it is only the fallback for older replies
    // (upstream vinzdg/codenotch#19).
    if let Some(auto) = pct(plan.get("autoPercentUsed")) {
        out.push(LimitWindow { id: "auto".into(), label: "Auto usage".into(), used: auto, resets_at, ..Default::default() });
    } else if let Some(total) = pct(plan.get("totalPercentUsed")) {
        out.push(LimitWindow { id: "included".into(), label: "Included usage".into(), used: total, resets_at, ..Default::default() });
    }
    if let Some(api) = pct(plan.get("apiPercentUsed")) {
        if api > 0.0 {
            out.push(LimitWindow { id: "api".into(), label: "API usage".into(), used: api, resets_at, ..Default::default() });
        }
    }
    if let Some(od) = usage.get("onDemand") {
        let enabled = od.get("enabled").and_then(|x| x.as_bool()).unwrap_or(false);
        let limit = od.get("limit").and_then(|x| x.as_f64()).unwrap_or(0.0);
        let used = od.get("used").and_then(|x| x.as_f64());
        if enabled && limit > 0.0 {
            if let Some(u) = used {
                out.push(LimitWindow {
                    id: "on_demand".into(),
                    label: "On demand".into(),
                    used: (u / limit).clamp(0.0, 1.0),
                    resets_at, ..Default::default()
                });
            }
        }
    }
    // Enterprise/team replies meter absolute used/limit under individualUsage.overall
    // instead of plan percentages (upstream vinzdg/codenotch#34)
    if out.is_empty() {
        if let Some(ov) = usage.get("overall") {
            let used = ov.get("used").and_then(|x| x.as_f64());
            let limit = ov.get("limit").and_then(|x| x.as_f64()).unwrap_or(0.0);
            if let Some(u) = used {
                if limit > 0.0 {
                    out.push(LimitWindow {
                        id: "included".into(),
                        label: "Included usage".into(),
                        used: (u / limit).clamp(0.0, 1.0),
                        resets_at, ..Default::default()
                    });
                }
            }
        }
    }
    if !out.is_empty() {
        return (out, String::new());
    }
    let membership = v.get("membershipType").and_then(|x| x.as_str()).unwrap_or("this");
    let note = if v.get("isUnlimited").and_then(|x| x.as_bool()) == Some(true) {
        format!("Unlimited on the {membership} plan — nothing to meter")
    } else {
        format!("The {membership} plan has nothing for Cursor to meter yet")
    };
    (out, note)
}

/// GetSandUsageStatus reply → the Grok Bot weekly window. Pooled enterprise seats and
/// accounts without a personal included allowance have no separate meter and return None
/// (guards mirror mstallone/runway#115).
pub fn parse_sand(v: &serde_json::Value) -> Option<LimitWindow> {
    let flag = |k: &str| v.get(k).and_then(|x| x.as_bool());
    if flag("usesPooledEnterpriseAllowance") == Some(true)
        || flag("hasNonZeroIncludedLimit") == Some(false)
        || flag("includedLimitZero") == Some(true)
    {
        return None;
    }
    let percent = v.get("usagePercent").and_then(|x| x.as_f64()).filter(|p| *p >= 0.0)?;
    Some(LimitWindow {
        id: "grok_bot".into(),
        label: "Grok Bot".into(),
        used: (percent / 100.0).clamp(0.0, 1.0),
        resets_at: parse_iso(v.get("nextResetTimestampUtc")),
        ..Default::default()
    })
}

fn fetch_sand(token: &str) -> Result<serde_json::Value, String> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(10)).build();
    agent
        .post(SAND_ENDPOINT)
        .set("Authorization", &format!("Bearer {token}"))
        .set("Content-Type", "application/json")
        .set("Connect-Protocol-Version", "1")
        .send_string("{}")
        .map_err(|e| match e {
            ureq::Error::Status(code, _) => format!("HTTP {code}"),
            other => format!("{other}"),
        })
        .and_then(|r| r.into_json::<serde_json::Value>().map_err(|e| format!("parse: {e}")))
}

/// Nonfatal: a Grok Bot failure never drops the primary Cursor windows
fn fetch_grok_bot(token: &str) -> Option<LimitWindow> {
    fetch_sand(token).ok().and_then(|v| parse_sand(&v))
}

enum FetchErr {
    NeedsAuth,
    Other(String),
}

fn fetch_once(cookie: &str) -> Result<serde_json::Value, FetchErr> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(15)).build();
    match agent.get(ENDPOINT).set("Cookie", cookie).set("Accept", "application/json").call() {
        Ok(r) => r.into_json::<serde_json::Value>().map_err(|e| FetchErr::Other(format!("parse: {e}"))),
        Err(ureq::Error::Status(401, _)) | Err(ureq::Error::Status(403, _)) => Err(FetchErr::NeedsAuth),
        Err(ureq::Error::Status(code, _)) => Err(FetchErr::Other(format!("HTTP {code}"))),
        Err(e) => Err(FetchErr::Other(format!("{e}"))),
    }
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn read_once(prev: &UsageSnapshot) -> UsageSnapshot {
    let mut snap = prev.clone();
    let Some(creds) = read_credentials() else {
        snap.status = "needsAuth".into();
        snap.note = "Sign in to Cursor (the editor) to see usage.".into();
        return snap;
    };
    match fetch_once(&creds.cookie) {
        Ok(v) => {
            let (mut windows, note) = parse_summary(&v);
            if let Some(gb) = fetch_grok_bot(&creds.token) {
                windows.push(gb);
            }
            snap.fetched_at = now_ms();
            if windows.is_empty() {
                snap.status = "none".into();
                snap.windows.clear();
                snap.note = note;
            } else {
                snap.status = "ok".into();
                snap.windows = windows;
                snap.note = match (&creds.plan, v.get("membershipType").and_then(|x| x.as_str())) {
                    (_, Some(m)) => format!("{} · via Cursor", cap(m)),
                    (Some(p), None) => format!("{} · via Cursor", cap(p)),
                    _ => String::new(),
                };
            }
        }
        Err(FetchErr::NeedsAuth) => {
            snap.status = "needsAuth".into();
            snap.note = "Cursor session was rejected — sign in again in the editor".into();
        }
        Err(FetchErr::Other(msg)) => {
            // Stale beats invented: keep the old reading, marked stale
            snap.status = if snap.windows.is_empty() { "error" } else { "stale" }.into();
            snap.note = msg;
        }
    }
    snap
}

fn broadcast(app: &AppHandle, snap: UsageSnapshot) {
    let st = app.state::<AppState>();
    *st.cursor.lock().unwrap() = snap.clone();
    persist(&snap);
    let _ = app.emit("cursor", &snap);
}

fn sleep_interruptible(secs: u64) {
    for _ in 0..secs {
        if REFRESH.swap(false, std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b64url_encode(data: &[u8]) -> String {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut out = String::new();
        for chunk in data.chunks(3) {
            let mut acc: u32 = 0;
            for (j, &b) in chunk.iter().enumerate() {
                acc |= (b as u32) << (16 - 8 * j as u32);
            }
            for j in 0..=chunk.len() {
                out.push(T[((acc >> (18 - 6 * j as u32)) & 0x3f) as usize] as char);
            }
        }
        out
    }

    fn fake_jwt(payload: serde_json::Value) -> String {
        format!(
            "{}.{}.sig",
            b64url_encode(br#"{"alg":"RS256"}"#),
            b64url_encode(payload.to_string().as_bytes())
        )
    }

    #[test]
    fn jwt_sub_reads_auth0_subject() {
        let tok = fake_jwt(serde_json::json!({ "sub": "auth0|user_ABC123", "exp": 1 }));
        assert_eq!(jwt_sub(&tok).as_deref(), Some("auth0|user_ABC123"));
    }

    #[test]
    fn jwt_sub_rejects_garbage() {
        assert_eq!(jwt_sub("not-a-jwt"), None);
        assert_eq!(jwt_sub(""), None);
        let no_sub = fake_jwt(serde_json::json!({ "exp": 1 }));
        assert_eq!(jwt_sub(&no_sub), None);
    }

    #[test]
    fn summary_prefers_auto_over_blended_total() {
        let v = serde_json::json!({
            "billingCycleEnd": "2026-10-01T00:00:00Z",
            "individualUsage": { "plan": { "autoPercentUsed": 20.0, "totalPercentUsed": 25.0, "apiPercentUsed": 100.0 } }
        });
        let (w, _) = parse_summary(&v);
        assert_eq!(w[0].label, "Auto usage");
        assert!((w[0].used - 0.20).abs() < 1e-9);
        assert_eq!(w[1].label, "API usage");
    }

    #[test]
    fn sand_maps_weekly_window() {
        let v = serde_json::json!({
            "usagePercent": 41.5,
            "currentPeriodStart": "2026-09-08T00:00:00Z",
            "nextResetTimestampUtc": "2026-09-15T00:00:00Z"
        });
        let w = parse_sand(&v).unwrap();
        assert_eq!(w.label, "Grok Bot");
        assert!((w.used - 0.415).abs() < 1e-9);
        assert!(w.resets_at.is_some());
    }

    #[test]
    fn sand_hides_pooled_and_no_allowance() {
        let pooled = serde_json::json!({ "usagePercent": 10.0, "usesPooledEnterpriseAllowance": true });
        assert!(parse_sand(&pooled).is_none());
        let zero = serde_json::json!({ "usagePercent": 10.0, "includedLimitZero": true });
        assert!(parse_sand(&zero).is_none());
        let no_pct = serde_json::json!({ "nextResetTimestampUtc": "2026-09-15T00:00:00Z" });
        assert!(parse_sand(&no_pct).is_none());
    }

    #[test]
    fn summary_enterprise_overall_used_limit() {
        let v = serde_json::json!({
            "membershipType": "enterprise",
            "individualUsage": { "plan": {}, "overall": { "used": 6907.0, "limit": 45000.0 } }
        });
        let (w, note) = parse_summary(&v);
        assert_eq!(w.len(), 1);
        assert!((w[0].used - 6907.0 / 45000.0).abs() < 1e-9);
        assert!(note.is_empty());
    }
}

pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        {
            let st = app.state::<AppState>();
            let snap = st.cursor.lock().unwrap().clone();
            let _ = app.emit("cursor", &snap);
        }
        if !present() {
            broadcast(&app, UsageSnapshot { status: "absent".into(), ..Default::default() });
            loop {
                sleep_interruptible(600); // Cursor is not installed: look again every 10 minutes
                if present() {
                    break;
                }
            }
        }
        loop {
            let prev = {
                let st = app.state::<AppState>();
                let s = st.cursor.lock().unwrap().clone();
                s
            };
            let snap = read_once(&prev);
            if snap.status == "error" || snap.status == "stale" {
                crate::applog(&format!("cursor: {}", snap.note));
            }
            broadcast(&app, snap);
            sleep_interruptible(POLL_SECS);
        }
    });
}
