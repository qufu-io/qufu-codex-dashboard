use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use std::time::Duration;
use tauri::State;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::{Update, UpdaterExt};

const RELEASES: &str = "https://github.com/qufu-io/qufu-codex-dashboard/releases";
const DOWNLOAD_PREFIX: &str = "https://github.com/qufu-io/qufu-codex-dashboard/releases/download/";

#[derive(Default)]
pub struct UpdateState {
    pending: Mutex<Option<Update>>,
    busy: AtomicBool,
}
struct Busy<'a>(&'a AtomicBool);
impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
impl UpdateState {
    fn begin(&self) -> Result<Busy<'_>, String> {
        self.busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "update_busy".to_string())?;
        Ok(Busy(&self.busy))
    }
}
fn trusted_download(url: &str) -> bool {
    url.starts_with(DOWNLOAD_PREFIX) && !url[DOWNLOAD_PREFIX.len()..].is_empty()
}
#[tauri::command]
pub async fn check_update(
    app: tauri::AppHandle,
    state: State<'_, UpdateState>,
) -> Result<Value, String> {
    let _busy = state.begin()?;
    *state.pending.lock().map_err(|_| "update_failed")? = None;
    let updater = app
        .updater_builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| "update_failed")?;
    match updater.check().await {
        Ok(Some(mut update)) => {
            if !trusted_download(update.download_url.as_str()) {
                return Err("update_failed".into());
            }
            update.timeout = Some(Duration::from_secs(300));
            let result = json!({"available":true,"version":update.version,"currentVersion":update.current_version});
            *state.pending.lock().map_err(|_| "update_failed")? = Some(update);
            Ok(result)
        }
        Ok(None) => {
            Ok(json!({"available":false,"currentVersion":app.package_info().version.to_string()}))
        }
        Err(_) => Err("update_check_failed".into()),
    }
}
#[tauri::command]
pub async fn install_update(
    app: tauri::AppHandle,
    state: State<'_, UpdateState>,
) -> Result<(), String> {
    let _busy = state.begin()?;
    let update = state
        .pending
        .lock()
        .map_err(|_| "update_failed")?
        .take()
        .ok_or("update_check_required")?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|_| "update_install_failed")?;
    #[cfg(not(windows))]
    app.restart();
    #[cfg(windows)]
    {
        let _ = app;
        Ok(())
    }
}
#[tauri::command]
pub fn open_release_page(app: tauri::AppHandle) -> Result<(), String> {
    app.opener()
        .open_url(RELEASES, None::<&str>)
        .map_err(|_| "update_open_failed".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn download_is_restricted_to_our_release_assets() {
        assert!(trusted_download(&format!(
            "{DOWNLOAD_PREFIX}v1.1.0/app.exe"
        )));
        for url in ["http://github.com/qufu-io/qufu-codex-dashboard/releases/download/v1/app.exe", "https://github.com.evil.test/qufu-io/qufu-codex-dashboard/releases/download/v1/app.exe", "https://github.com/another/project/releases/download/v1/app.exe", DOWNLOAD_PREFIX] {
            assert!(!trusted_download(url));
        }
    }
    #[test]
    fn check_and_install_do_not_overlap_and_unlock_after_failure() {
        let state = UpdateState::default();
        let busy = state.begin().unwrap();
        assert!(state.begin().is_err());
        drop(busy);
        assert!(state.begin().is_ok());
        assert!(state.pending.lock().unwrap().is_none());
    }
}
