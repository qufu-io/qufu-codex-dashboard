use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    time::Duration,
};

#[derive(Clone, Default)]
pub struct Feed {
    pub latest: Arc<Mutex<Option<Value>>>,
    child: Arc<Mutex<Option<Child>>>,
    stopped: Arc<AtomicBool>,
}
fn executable() -> Option<PathBuf> {
    let mut candidates = vec![];
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(PathBuf::from(local).join("Programs/OpenAI/Codex/bin/codex.exe"));
    }
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(
            std::env::split_paths(&path)
                .map(|p| p.join(if cfg!(windows) { "codex.exe" } else { "codex" })),
        );
    }
    candidates.into_iter().find(|p| p.is_file())
}
fn normalized(v: &Value) -> Value {
    let window = |w: &Value| json!({"used_percent":w["usedPercent"],"window_minutes":w["windowDurationMins"],"resets_at":w["resetsAt"]});
    json!({"primary":window(&v["primary"]),"secondary":window(&v["secondary"]),"plan_type":v["planType"],"limit_id":v["limitId"]})
}
fn scoped_snapshot(result: &Value) -> Option<Value> {
    use sha2::{Digest, Sha256};
    let raw = result
        .pointer("/rateLimitsByLimitId/codex")
        .or_else(|| result.get("rateLimits"))?;
    let mut value = normalized(raw);
    if let Some(id) = result["accountId"].as_str().filter(|s| !s.is_empty()) {
        value["account_key"] = json!(format!(
            "{:x}",
            Sha256::digest(format!("qufu-account-v1:{id}"))
        ));
    }
    Some(value)
}
fn request(
    input: &mut ChildStdin,
    rx: &mpsc::Receiver<Value>,
    id: u64,
    method: &str,
    params: Value,
) -> Result<Value, ()> {
    writeln!(
        input,
        "{}",
        json!({"id":id,"method":method,"params":params})
    )
    .map_err(|_| ())?;
    input.flush().map_err(|_| ())?;
    let deadline = std::time::Instant::now() + Duration::from_secs(12);
    while let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) {
        let v = rx.recv_timeout(left).map_err(|_| ())?;
        if v["id"].as_u64() == Some(id) {
            return v.get("result").cloned().ok_or(());
        }
    }
    Err(())
}
impl Feed {
    pub fn start(&self) {
        let feed = self.clone();
        std::thread::spawn(move || {
            while !feed.stopped.load(Ordering::Relaxed) {
                let _ = feed.connected();
                feed.disconnect();
                for _ in 0..30 {
                    if feed.stopped.load(Ordering::Relaxed) {
                        return;
                    }
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        });
    }
    fn connected(&self) -> Result<(), ()> {
        let exe = executable().ok_or(())?;
        let mut cmd = Command::new(exe);
        cmd.arg("app-server")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .env_remove("OPENAI_API_KEY");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        let mut child = cmd.spawn().map_err(|_| ())?;
        let mut input = child.stdin.take().ok_or(())?;
        let output = child.stdout.take().ok_or(())?;
        *self.child.lock().map_err(|_| ())? = Some(child);
        let (tx, rx) = mpsc::sync_channel(32);
        std::thread::spawn(move || {
            for line in BufReader::new(output).lines().map_while(Result::ok) {
                if let Ok(v) = serde_json::from_str::<Value>(&line) {
                    if tx.send(v).is_err() {
                        break;
                    }
                }
            }
        });
        request(
            &mut input,
            &rx,
            1,
            "initialize",
            json!({"clientInfo":{"name":"qufu_codex_dashboard","title":"Qufu Codex Dashboard","version":env!("CARGO_PKG_VERSION")}}),
        )?;
        writeln!(input, "{}", json!({"method":"initialized","params":{}})).map_err(|_| ())?;
        let result = request(&mut input, &rx, 2, "account/rateLimits/read", json!({}))?;
        let value = scoped_snapshot(&result).ok_or(())?;
        if value["primary"]["used_percent"].is_number()
            || value["secondary"]["used_percent"].is_number()
        {
            *self.latest.lock().map_err(|_| ())? = Some(
                json!({"at":chrono::Utc::now().timestamp(),"value":value,"source":"Codex account/rateLimits/read","stale":false}),
            );
        }
        Ok(())
    }
    fn disconnect(&self) {
        if let Ok(mut p) = self.child.lock() {
            if let Some(mut c) = p.take() {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
    }
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Relaxed);
        self.disconnect();
    }
    pub fn snapshot(&self) -> Option<Value> {
        let mut v = self.latest.lock().ok()?.clone()?;
        v["stale"] = json!(chrono::Utc::now().timestamp() - v["at"].as_i64().unwrap_or(0) > 90);
        Some(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn account_scope_uses_only_explicit_snapshot_identity() {
        let raw = json!({"rateLimits":{"primary":{"windowDurationMins":10080,"usedPercent":16,"resetsAt":800000}}});
        assert!(scoped_snapshot(&raw).unwrap()["account_key"].is_null());
        let mut a = raw.clone();
        a["accountId"] = json!("synthetic-account-A");
        let mut b = raw.clone();
        b["accountId"] = json!("synthetic-account-B");
        let x = scoped_snapshot(&a).unwrap();
        assert_eq!(x["primary"]["window_minutes"], 10080);
        assert!(x["secondary"]["used_percent"].is_null());
        assert_eq!(x["account_key"].as_str().unwrap().len(), 64);
        assert_ne!(
            x["account_key"],
            scoped_snapshot(&b).unwrap()["account_key"]
        );
        assert!(!x.to_string().contains("synthetic-account-A"));
    }
    #[test]
    fn normalize_does_not_swap_windows() {
        let n = normalized(
            &json!({"planType":"pro","primary":{"usedPercent":42,"windowDurationMins":300,"resetsAt":100},"secondary":{"usedPercent":68,"windowDurationMins":10080,"resetsAt":200}}),
        );
        assert_eq!(n["primary"]["window_minutes"], 300);
        assert_eq!(n["plan_type"], "pro");
        assert_eq!(n["secondary"]["used_percent"], 68);
    }
}
