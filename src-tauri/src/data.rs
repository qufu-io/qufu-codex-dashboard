use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{BufRead, BufReader, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Usage {
    pub input: u64,
    pub cache: u64,
    pub write: u64,
    pub output: u64,
    pub reasoning: u64,
    pub total: u64,
}
impl Usage {
    pub fn from(v: &Value) -> Option<Self> {
        let input = v.get("input_tokens")?.as_u64()?;
        let output = v.get("output_tokens")?.as_u64()?;
        Some(Self {
            input,
            output,
            cache: v["cached_input_tokens"].as_u64().unwrap_or(0),
            write: v["cache_write_input_tokens"].as_u64().unwrap_or(0),
            reasoning: v["reasoning_output_tokens"].as_u64().unwrap_or(0),
            total: v["total_tokens"].as_u64().unwrap_or(input + output),
        })
    }
    fn delta(&self, prev: &Self) -> Self {
        Self {
            input: self.input.saturating_sub(prev.input),
            cache: self.cache.saturating_sub(prev.cache),
            write: self.write.saturating_sub(prev.write),
            output: self.output.saturating_sub(prev.output),
            reasoning: self.reasoning.saturating_sub(prev.reasoning),
            total: self.total.saturating_sub(prev.total),
        }
    }
    pub fn add(&mut self, v: &Self) {
        self.input += v.input;
        self.cache += v.cache;
        self.write += v.write;
        self.output += v.output;
        self.reasoning += v.reasoning;
        self.total += v.total;
    }
}

#[derive(Clone, Default, Serialize)]
pub struct ModelUsage {
    pub model: String,
    pub usage: Usage,
    pub usd: Option<f64>,
    #[serde(skip)]
    pub pricing_model: String,
    #[serde(skip)]
    pub pricing_tier: String,
    #[serde(skip)]
    pub short: Usage,
    #[serde(skip)]
    pub long: Usage,
    #[serde(skip)]
    pub unpriced: bool,
}
impl ModelUsage {
    fn current_usd(&self) -> Option<f64> {
        if self.unpriced {
            return None;
        }
        let a = if self.short.total == 0 {
            0.
        } else {
            crate::pricing::cost(&self.pricing_model, &self.pricing_tier, &self.short, false)?
        };
        let b = if self.long.total == 0 {
            0.
        } else {
            crate::pricing::cost(&self.pricing_model, &self.pricing_tier, &self.long, true)?
        };
        Some(a + b)
    }
    fn repriced(&self) -> Self {
        let mut result = self.clone();
        result.usd = self.current_usd();
        result
    }
}
#[derive(Clone)]
pub struct UsageSample {
    pub at: i64,
    pub usage: Usage,
    pub model: String,
    pub tier: String,
}
#[derive(Clone, Default)]
pub struct Rollout {
    pub offset: u64,
    pub modified: Option<SystemTime>,
    pub usage: Option<Usage>,
    pub last_usage: Option<Usage>,
    last_model: String,
    last_tier: String,
    context_capacity: Option<u64>,
    context_used: Option<u64>,
    previous: Usage,
    pub model: String,
    pub tier: String,
    pub models: HashMap<String, ModelUsage>,
    pub events: Vec<(i64, u64)>,
    pub quota: Option<(i64, Value)>,
    pub window_samples: Vec<UsageSample>,
    pub quota_history: Vec<(i64, Value)>,
    pub status: String,
    pub parent: Option<String>,
    pub id: String,
    pub warnings: HashSet<String>,
    pub seen: HashSet<String>,
    pub created: i64,
    pub record_mode: bool,
}
pub fn timestamp(v: &Value) -> i64 {
    v.as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|t| t.timestamp())
        .unwrap_or(0)
}
#[cfg(test)]
fn price(model: &str, u: &Usage, tier: &str) -> Option<f64> {
    crate::pricing::cost(model, tier, u, u.input > 272_000)
}
impl Rollout {
    pub fn ingest(&mut self, e: Value, expected: &str) {
        let p = &e["payload"];
        let t = timestamp(&e["timestamp"]);
        let ty = e["type"].as_str().unwrap_or("");
        if ty == "session_meta" {
            self.id = p["id"].as_str().unwrap_or(expected).into();
            self.created = timestamp(&p["timestamp"]);
            self.parent = p["parent_thread_id"]
                .as_str()
                .or_else(|| {
                    p.pointer("/source/subagent/thread_spawn/parent_thread_id")
                        .and_then(Value::as_str)
                })
                .map(str::to_owned);
            return;
        }
        if ty == "compacted" && t >= self.created {
            self.context_used = None;
            return;
        }
        if ty == "turn_context" {
            if p["model"]
                .as_str()
                .is_some_and(|m| !self.model.is_empty() && m != self.model)
            {
                self.context_used = None;
            }
            self.model = p["model"].as_str().unwrap_or("").into();
            if let Some(tier) = p["service_tier"].as_str() {
                self.tier = tier.into();
            }
            return;
        }
        if ty == "event_msg" {
            if p["type"] == "thread_settings_applied" {
                if let Some(tier) = p
                    .pointer("/thread_settings/service_tier")
                    .and_then(Value::as_str)
                {
                    self.tier = tier.into();
                }
            }
            match p["type"].as_str().unwrap_or("") {
                "task_started" => {
                    self.status = "Çalışıyor".into();
                    if t >= self.created {
                        let capacity = p["model_context_window"].as_u64().filter(|v| *v > 0);
                        if capacity != self.context_capacity {
                            self.context_used = None;
                        }
                        self.context_capacity = capacity;
                    }
                }
                "task_complete" | "turn_complete" => self.status = "Tamamlandı".into(),
                "turn_aborted" => self.status = "Kesildi".into(),
                _ => {}
            }
            if p["type"] == "token_count" && t >= self.created && p["info"].is_object() {
                self.context_capacity = p["info"]["model_context_window"]
                    .as_u64()
                    .filter(|v| *v > 0);
                self.context_used = p["info"]["last_token_usage"]["total_tokens"].as_u64();
                if !self.record_mode {
                    self.last_usage = Usage::from(&p["info"]["last_token_usage"]);
                    self.last_model = self.model.clone();
                    self.last_tier = self.tier.clone();
                }
            }
            if p["type"] != "token_count" {
                return;
            }
            if p["rate_limits"].is_object()
                && matches!(p["rate_limits"]["limit_id"].as_str(), None | Some("codex"))
            {
                self.quota = Some((t, p["rate_limits"].clone()));
                if t >= self.created && t >= chrono::Utc::now().timestamp() - 14 * 86400 {
                    self.quota_history.push((t, json!({"primary":p["rate_limits"]["primary"],"secondary":p["rate_limits"]["secondary"],"plan_type":p["rate_limits"]["plan_type"]})));
                }
            }
        }
        if t < self.created {
            return;
        }
        let is_record = ty == "token_usage_record";
        let (current, raw) = if is_record {
            if p["thread_id"].as_str() != Some(expected) {
                return;
            }
            if let Some(id) = p["response_id"].as_str() {
                if !self.seen.insert(id.into()) {
                    return;
                }
            }
            if !self.record_mode {
                self.record_mode = true;
                self.last_usage = None;
                self.usage = None;
                self.previous = Usage::default();
                self.models.clear();
                self.events.clear();
                self.window_samples.clear();
                self.warnings.clear();
                if let (Some(cumulative), Some(response)) = (
                    Usage::from(&p["thread_token_usage"]),
                    Usage::from(&p["usage"]),
                ) {
                    let baseline = cumulative.delta(&response);
                    if baseline.total > 0 {
                        self.usage = Some(baseline.clone());
                        self.models.insert(
                            "Geçmiş / model verisi yok".into(),
                            ModelUsage {
                                model: "Geçmiş / model verisi yok".into(),
                                usage: baseline,
                                usd: None,
                                unpriced: true,
                                ..Default::default()
                            },
                        );
                        self.warnings.insert("Geçmiş toplam mevcut; eski yanıtların model/tarife bilgisi eksik olduğundan tam USD hesaplanamıyor".into());
                    }
                }
            }
            (
                Usage::from(&p["thread_token_usage"]),
                Usage::from(&p["usage"]),
            )
        } else if ty == "event_msg" && p["type"] == "token_count" {
            if self.record_mode {
                return;
            }
            (
                Usage::from(&p["info"]["total_token_usage"]),
                Usage::from(&p["info"]["last_token_usage"]),
            )
        } else {
            return;
        };
        if is_record {
            self.last_usage = raw.clone();
            self.last_model = self.model.clone();
            self.last_tier = self.tier.clone();
            self.context_used = raw.as_ref().map(|u| u.total);
        }
        let cur = if is_record {
            current.unwrap_or_default()
        } else {
            let Some(c) = current else {
                if raw.is_some() {
                    self.warnings
                        .insert("Kümülatif sayaç eksik; belirsiz kayıt eklenmedi".into());
                }
                return;
            };
            c
        };
        let d = if is_record {
            let Some(u) = raw else {
                self.warnings.insert("Yanıt token verisi eksik".into());
                return;
            };
            u
        } else if cur.total < self.previous.total {
            self.warnings
                .insert("Token sayacı sıfırlandı; son yanıt üzerinden devam edildi".into());
            raw.clone().unwrap_or_default()
        } else {
            cur.delta(&self.previous)
        };
        self.previous = cur;
        self.usage.get_or_insert_with(Usage::default).add(&d);
        if d.total == 0 {
            return;
        }
        self.events.push((t, d.total));
        if is_record && t >= chrono::Utc::now().timestamp() - 14 * 86400 {
            self.window_samples.push(UsageSample {
                at: t,
                usage: d.clone(),
                model: self.model.clone(),
                tier: self.tier.clone(),
            });
        }
        let key = format!(
            "{}{}",
            self.model,
            if self.tier.is_empty() {
                String::new()
            } else {
                format!(" / {}", self.tier)
            }
        );

        let entry = self
            .models
            .entry(key.clone())
            .or_insert_with(|| ModelUsage {
                model: key,
                usage: Usage::default(),
                usd: Some(0.),
                pricing_model: self.model.clone(),
                pricing_tier: self.tier.clone(),
                ..Default::default()
            });
        entry.usage.add(&d);
        if d.input > 272_000 {
            entry.long.add(&d);
        } else {
            entry.short.add(&d);
        }
    }
    pub fn read(&mut self, path: &Path, id: &str) -> Result<(), String> {
        let meta = fs::metadata(path).map_err(|_| "Session dosyası okunamıyor".to_string())?;
        if self.modified == meta.modified().ok() && self.offset == meta.len() {
            return Ok(());
        }
        if meta.len() < self.offset {
            *self = Self::default();
        }
        let mut f = File::open(path).map_err(|e| e.to_string())?;
        f.seek(SeekFrom::Start(self.offset))
            .map_err(|e| e.to_string())?;
        let mut reader = BufReader::new(f);
        let mut line = Vec::new();
        loop {
            line.clear();
            let n = reader
                .read_until(b'\n', &mut line)
                .map_err(|e| e.to_string())?;
            if n == 0 || !line.ends_with(b"\n") {
                break;
            }
            self.offset += n as u64;
            if ![
                b"\"token_count\"".as_slice(),
                b"\"token_usage_record\"",
                b"\"session_meta\"",
                b"\"turn_context\"",
                b"\"compacted\"",
                b"\"task_started\"",
                b"\"thread_settings_applied\"",
                b"\"task_complete\"",
                b"\"turn_aborted\"",
            ]
            .iter()
            .any(|s| line.windows(s.len()).any(|w| w == *s))
            {
                continue;
            }
            if let Ok(e) = serde_json::from_slice(&line) {
                self.ingest(e, id);
            } else {
                self.warnings.insert("Bozuk JSONL kaydı atlandı".into());
            }
        }
        self.modified = meta.modified().ok();
        let now = chrono::Utc::now().timestamp();
        self.events.retain(|(t, _)| *t > now - 60);
        self.window_samples.retain(|s| s.at >= now - 14 * 86400);
        self.quota_history.retain(|(t, _)| *t >= now - 14 * 86400);
        Ok(())
    }
    pub fn last_usd(&self) -> Option<f64> {
        let usage = self.last_usage.as_ref()?;
        crate::pricing::cost(
            &self.last_model,
            &self.last_tier,
            usage,
            usage.input > 272_000,
        )
    }
    pub fn context(&self) -> Option<Value> {
        let capacity = self.context_capacity.filter(|v| *v > 0)?;
        let used = self.context_used?;
        Some(
            json!({"used":used,"capacity":capacity,"percent":(used as f64 / capacity as f64 * 100.).min(100.)}),
        )
    }
    pub fn usd(&self) -> Option<f64> {
        if self.usage.is_none() {
            None
        } else {
            self.models
                .values()
                .try_fold(0., |s, m| m.current_usd().map(|v| s + v))
        }
    }
    pub fn tpm(&self, now: i64) -> Option<u64> {
        self.usage.as_ref().map(|_| {
            self.events
                .iter()
                .filter(|(t, _)| *t > now - 60 && *t <= now)
                .map(|(_, v)| v)
                .sum()
        })
    }
}

#[derive(Clone, Serialize)]
pub struct Thread {
    pub id: String,
    pub title: String,
    pub workspace: String,
    pub group: String,
    pub path: String,
    pub source: String,
    pub archived: bool,
    pub tokens: Option<i64>,
    pub updated: i64,
    pub parent: Option<String>,
}
#[derive(Default)]
pub struct Reader {
    pub home: PathBuf,
    pub cache: HashMap<String, Rollout>,
}
impl Reader {
    pub fn new() -> Self {
        Self {
            home: std::env::var_os("CODEX_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    PathBuf::from(
                        std::env::var_os("USERPROFILE")
                            .or_else(|| std::env::var_os("HOME"))
                            .unwrap_or_default(),
                    )
                    .join(".codex")
                }),
            cache: HashMap::new(),
        }
    }
    pub fn catalog(&self) -> Result<(Vec<Thread>, Value), String> {
        let db = fs::read_dir(&self.home)
            .map_err(|_| "Codex klasörü bulunamadı".to_string())?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|s| s.starts_with("state_") && s.ends_with(".sqlite"))
            })
            .max_by_key(|p| {
                p.file_stem()
                    .and_then(|s| s.to_str())
                    .and_then(|s| s.strip_prefix("state_"))
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(0)
            })
            .ok_or("Codex state veritabanı bulunamadı")?;
        let c = Connection::open_with_flags(
            db,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| e.to_string())?;
        c.busy_timeout(std::time::Duration::from_millis(300))
            .map_err(|e| e.to_string())?;
        let global = fs::read(self.home.join(".codex-global-state.json"))
            .ok()
            .and_then(|s| serde_json::from_slice::<Value>(&s).ok())
            .unwrap_or(json!({}));
        let columns: HashSet<String> = c
            .prepare("PRAGMA table_info(threads)")
            .map_err(|e| e.to_string())?
            .query_map([], |r| r.get(1))
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .collect();
        let name = if columns.contains("name") {
            "COALESCE(NULLIF(name,''),title)"
        } else {
            "title"
        };
        let proj = if columns.contains("project_id") {
            "project_id"
        } else {
            "NULL"
        };
        let mut stmt=c.prepare(&format!("SELECT id,{name},cwd,rollout_path,source,archived,tokens_used,updated_at,{proj} FROM threads ORDER BY updated_at DESC")).map_err(|e|e.to_string())?;
        let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
        let mut result = vec![];
        while let Some(r) = rows.next().map_err(|e| e.to_string())? {
            let id: String = r.get(0).map_err(|e| e.to_string())?;
            let cwd: String = r.get(2).unwrap_or_default();
            let source: String = r.get(4).unwrap_or_default();
            let src: Value = serde_json::from_str(&source).unwrap_or(Value::Null);
            let parent = src
                .pointer("/subagent/thread_spawn/parent_thread_id")
                .and_then(Value::as_str)
                .map(str::to_owned);
            let pid = global
                .pointer(&format!("/thread-project-assignments/{id}/projectId"))
                .and_then(Value::as_str)
                .map(str::to_owned)
                .or_else(|| r.get::<_, Option<String>>(8).ok().flatten());
            let group = pid
                .as_ref()
                .and_then(|p| global.pointer(&format!("/local-projects/{p}/name")))
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    if pid.is_none() {
                        "Sohbetler".into()
                    } else {
                        cwd.trim_end_matches(['\\', '/'])
                            .rsplit(['\\', '/'])
                            .next()
                            .unwrap_or("Workspace")
                            .into()
                    }
                });
            result.push(Thread {
                id,
                title: r.get(1).unwrap_or_else(|_| "Adsız session".into()),
                workspace: cwd.trim_start_matches("\\\\?\\").into(),
                group,
                path: r.get(3).unwrap_or_default(),
                source,
                archived: r.get::<_, i64>(5).unwrap_or(1) != 0,
                tokens: r.get(6).ok(),
                updated: r.get(7).unwrap_or(0),
                parent,
            });
        }
        if let Ok(mut s) =
            c.prepare("SELECT parent_thread_id,child_thread_id FROM thread_spawn_edges")
        {
            if let Ok(edges) =
                s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            {
                for edge in edges.flatten() {
                    if let Some(t) = result
                        .iter_mut()
                        .find(|t| t.id == edge.1 && t.source.contains("subagent"))
                    {
                        t.parent = Some(edge.0);
                    }
                }
            }
        }
        Ok((result, global))
    }
    pub fn read_thread(&mut self, t: &Thread) -> Result<&Rollout, String> {
        let cached = self.cache.entry(t.id.clone()).or_default();
        cached.read(Path::new(&t.path), &t.id)?;
        Ok(cached)
    }
    pub fn snapshot(&mut self, selected: Option<&str>) -> Result<Value, String> {
        let now = chrono::Utc::now().timestamp();
        let (all, _global) = self.catalog()?;
        let mut sessions = vec![];
        let mut excluded = HashMap::<String, u64>::new();
        let mut errors = vec![];
        for t in &all {
            let reason = if t.archived {
                Some("archived")
            } else if t.parent.is_some() || t.source.contains("subagent") {
                Some("subagent/internal")
            } else if !matches!(
                t.source.as_str(),
                "vscode" | "cli" | "appServer" | "app-server"
            ) {
                Some("unsupported source")
            } else {
                None
            };
            if let Some(r) = reason {
                *excluded.entry(r.into()).or_default() += 1;
                continue;
            }
            if t.tokens.is_none_or(|n| n <= 0) {
                match self.read_thread(t) {
                    Ok(r) if r.usage.as_ref().is_some_and(|u| u.total > 0) => {}
                    _ => {
                        *excluded.entry("no confirmed usage".into()).or_default() += 1;
                        continue;
                    }
                }
            }
            sessions.push(t.clone());
        }
        let mut detail = Value::Null;
        if let Some(t) = sessions.iter().find(|t| Some(t.id.as_str()) == selected) {
            if let Err(e) = self.read_thread(t) {
                errors.push(e);
            }
            let mut ids = HashSet::from([t.id.clone()]);
            let mut changed = true;
            while changed {
                changed = false;
                for child in &all {
                    if child.parent.as_ref().is_some_and(|p| ids.contains(p))
                        && ids.insert(child.id.clone())
                    {
                        changed = true;
                    }
                }
            }
            for child in all
                .iter()
                .filter(|c| c.source.contains("subagent") && c.parent.is_none())
            {
                let c = self.cache.entry(child.id.clone()).or_default();
                if c.id.is_empty() {
                    if let Ok(f) = File::open(&child.path) {
                        if let Some(Ok(l)) = BufReader::new(f).lines().next() {
                            if let Ok(v) = serde_json::from_str(&l) {
                                c.ingest(v, &child.id);
                            }
                        }
                    }
                }
            }
            changed = true;
            while changed {
                changed = false;
                for (id, r) in &self.cache {
                    if r.parent.as_ref().is_some_and(|p| ids.contains(p)) && ids.insert(id.clone())
                    {
                        changed = true;
                    }
                }
            }
            let mut children = vec![];
            let mut child_usage = Usage::default();
            let mut child_usd = Some(0.);
            let mut complete = true;
            for c in all.iter().filter(|c| c.id != t.id && ids.contains(&c.id)) {
                match self.read_thread(c) {
                    Ok(r) => {
                        if let Some(u) = &r.usage {
                            child_usage.add(u);
                        } else {
                            complete = false;
                        }
                        child_usd = child_usd.zip(r.usd()).map(|(a, b)| a + b);
                        children.push(json!({"id":c.id,"title":c.title,"usage":r.usage,"usd":r.usd(),"status":r.status,"internal":c.source.contains("guardian")}));
                    }
                    Err(_) => {
                        complete = false;
                        child_usd = None;
                        children.push(json!({"id":c.id,"title":c.title,"usage":null,"usd":null,"status":"Kayıt okunamıyor","internal":c.source.contains("guardian")}));
                    }
                }
            }
            if let Some(r) = self.cache.get(&t.id) {
                detail = json!({"thread":t,"usage":r.usage,"lastUsage":r.last_usage,"lastUsd":r.last_usd(),"context":r.context(),"contextCapacity":r.context_capacity,"usd":r.usd(),"tpm":r.tpm(now),"model":r.model,"usageSource":if r.record_mode{"response records"}else{"legacy cumulative counters"},"status":if r.status.is_empty(){"Bilinmiyor"}else{&r.status},"models":r.models.values().map(ModelUsage::repriced).collect::<Vec<_>>(),"warnings":r.warnings,"children":children,"childUsage":if complete{Some(child_usage)}else{None},"childUsd":child_usd,"childrenComplete":complete});
            }
        }
        let mut quota = self
            .cache
            .values()
            .filter_map(|r| r.quota.clone())
            .max_by_key(|(t, _)| *t);
        for t in all.iter().take(5) {
            if let Ok(mut f) = File::open(&t.path) {
                if let Ok(m) = f.metadata() {
                    let start = m.len().saturating_sub(262144);
                    let _ = f.seek(SeekFrom::Start(start));
                    for l in BufReader::new(f).lines().map_while(Result::ok) {
                        if !l.contains("\"rate_limits\"") {
                            continue;
                        }
                        if let Ok(v) = serde_json::from_str::<Value>(&l) {
                            let p = &v["payload"];
                            let q = &p["rate_limits"];
                            let ts = timestamp(&v["timestamp"]);
                            if q.is_object()
                                && matches!(q["limit_id"].as_str(), None | Some("codex"))
                                && quota.as_ref().is_none_or(|(old, _)| ts > *old)
                            {
                                quota = Some((ts, q.clone()));
                            }
                        }
                    }
                }
            }
        }
        Ok(
            json!({"sessions":sessions,"detail":detail,"quota":quota.map(|(at,value)|json!({"at":at,"value":value,"stale":now-at>120,"source":"Yerel Codex limit snapshot"})),"excluded":excluded,"errors":errors,"at":now,"follow":{"id":null,"reason":"Desktop seçimi için doğrulanmış bir kaynak yok; manuel seçim korunuyor."}}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(i: u64, c: u64, o: u64) -> Value {
        json!({"type":"event_msg","timestamp":"2026-10-18T12:00:00Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":i,"cached_input_tokens":c,"output_tokens":o,"total_tokens":i+o}}}})
    }
    #[test]
    fn cache_and_reasoning_are_subsets() {
        let mut r = Rollout::default();
        r.model = "gpt-6.1-sol".into();
        r.ingest(event(100, 80, 20), "a");
        r.ingest(event(100, 80, 20), "a");
        assert_eq!(r.usage.as_ref().unwrap().total, 120);
        assert!((r.usd().unwrap() - 0.000248).abs() < 1e-10);
    }
    #[test]
    fn observed_fast_tier_and_cache_writes_are_priced_separately() {
        let u = Usage {
            input: 100,
            cache: 60,
            write: 20,
            output: 10,
            total: 110,
            ..Default::default()
        };
        let standard = price("gpt-6.1-sol", &u, "standard").unwrap();
        assert!((standard - 0.000196).abs() < 1e-10);
        assert!((price("gpt-6.1-sol", &u, "priority").unwrap() - 2. * standard).abs() < 1e-10);
        assert!(price("gpt-6.1-sol", &u, "unknown").is_none());
        let mut r = Rollout::default();
        r.ingest(json!({"type":"event_msg","payload":{"type":"thread_settings_applied","thread_settings":{"service_tier":"priority"}}}),"a");
        r.ingest(
            json!({"type":"turn_context","payload":{"model":"gpt-6.1-sol"}}),
            "a",
        );
        assert_eq!(r.tier, "priority");
    }
    #[test]
    fn record_and_event_do_not_double_count() {
        let mut r = Rollout::default();
        r.ingest(event(100, 80, 20), "a");
        let u = json!({"input_tokens":100,"cached_input_tokens":80,"output_tokens":20,"total_tokens":120});
        r.ingest(json!({"type":"token_usage_record","payload":{"thread_id":"a","response_id":"r","thread_token_usage":u,"usage":u}}),"a");
        assert_eq!(r.usage.unwrap().total, 120);
    }
    #[test]
    fn unknown_is_not_zero() {
        assert!(Rollout::default().usage.is_none());
        assert!(price("unknown", &Usage::default(), "").is_none());
    }
    #[test]
    fn copied_child_records_ignored() {
        let mut r = Rollout::default();
        r.ingest(json!({"type":"token_usage_record","payload":{"thread_id":"parent","thread_token_usage":{"input_tokens":100,"output_tokens":5}}}),"child");
        assert!(r.usage.is_none());
    }
    #[test]
    fn rolling_minute_expires() {
        let r = Rollout {
            usage: Some(Usage::default()),
            events: vec![(100, 30), (150, 50), (180, 20)],
            ..Default::default()
        };
        assert_eq!(r.tpm(180), Some(70));
        assert_eq!(r.tpm(250), Some(0));
    }
    #[test]
    fn counter_reset_is_not_entire_history() {
        let mut r = Rollout::default();
        r.ingest(event(100, 0, 20), "a");
        let mut e = event(10, 0, 2);
        e["payload"]["info"]["last_token_usage"] = json!({"input_tokens":10,"output_tokens":2});
        r.ingest(e, "a");
        assert_eq!(r.usage.unwrap().total, 132);
    }
    fn record(id: &str, response: &str, total: u64, raw: u64) -> Value {
        json!({"type":"token_usage_record","timestamp":"2026-10-18T12:00:00Z","payload":{"thread_id":id,"response_id":response,"thread_token_usage":{"input_tokens":total,"output_tokens":0},"usage":{"input_tokens":raw,"output_tokens":0}}})
    }
    #[test]
    fn separate_counter_streams_and_duplicate_responses() {
        let mut r = Rollout::default();
        r.ingest(record("a", "r1", 100, 100), "a");
        r.ingest(event(80, 0, 0), "a");
        r.ingest(record("a", "r2", 120, 20), "a");
        r.ingest(record("a", "r2", 120, 20), "a");
        r.ingest(event(105, 0, 0), "a");
        assert_eq!(r.usage.unwrap().total, 120);
    }
    #[test]
    fn compacted_history_preserved_but_not_priced_as_current_model() {
        let mut r = Rollout::default();
        r.model = "gpt-6.1-sol".into();
        r.ingest(record("a", "r1", 1000, 100), "a");
        assert_eq!(r.usage.as_ref().unwrap().total, 1000);
        assert!(r.usd().is_none());
        assert_eq!(r.tpm(timestamp(&json!("2026-10-18T12:00:00Z"))), Some(100));
    }
    #[test]
    fn discovery_no_date_cutoff_and_no_internal_sessions() {
        let root = std::env::temp_dir().join(format!(
            "qufu-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let c = Connection::open(root.join("state_5.sqlite")).unwrap();
        c.execute_batch("CREATE TABLE threads(id TEXT PRIMARY KEY,name TEXT,title TEXT,cwd TEXT,rollout_path TEXT,source TEXT,archived INTEGER,tokens_used INTEGER,updated_at INTEGER);CREATE TABLE thread_spawn_edges(parent_thread_id TEXT,child_thread_id TEXT);").unwrap();
        for (id, source, archived, tokens) in [
            ("old", "vscode", 0, Some(100)),
            ("archived", "vscode", 1, Some(100)),
            (
                "child",
                "{\"subagent\":{\"other\":\"guardian\"}}",
                0,
                Some(100),
            ),
            ("unknown", "vscode", 0, None),
            ("stalezero", "vscode", 0, Some(0)),
        ] {
            let path = root.join(format!("{id}.jsonl"));
            if id == "stalezero" {
                fs::write(&path, format!("{}\n", record(id, "r", 50, 50))).unwrap();
            }
            c.execute(
                "INSERT INTO threads VALUES(?1,?1,'different title','C:/test',?2,?3,?4,?5,1)",
                rusqlite::params![id, path.to_string_lossy(), source, archived, tokens],
            )
            .unwrap();
        }
        c.execute("INSERT INTO thread_spawn_edges VALUES('another','old')", [])
            .unwrap();
        drop(c);
        let mut r = Reader {
            home: root.clone(),
            cache: HashMap::new(),
        };
        let v = r.snapshot(None).unwrap();
        let ids: Vec<_> = v["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"old"));
        assert!(ids.contains(&"stalezero"));
        assert_eq!(v["sessions"][0]["title"], "old");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn partial_jsonl_is_retried_after_newline() {
        let path = std::env::temp_dir().join(format!(
            "qufu-partial-{}.jsonl",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let line = record("a", "r", 10, 10).to_string();
        fs::write(&path, &line).unwrap();
        let mut r = Rollout::default();
        r.read(&path, "a").unwrap();
        assert!(r.usage.is_none());
        fs::write(&path, format!("{line}\n")).unwrap();
        r.read(&path, "a").unwrap();
        assert_eq!(r.usage.unwrap().total, 10);
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn last_response_is_not_the_session_delta_or_a_copied_record() {
        let mut r = Rollout::default();
        r.ingest(record("a", "r1", 1000, 100), "a");
        r.ingest(record("a", "r2", 1050, 50), "a");
        r.ingest(record("a", "r1", 1000, 100), "a");
        r.ingest(record("parent", "foreign", 9000, 9000), "a");
        let mut e = event(900, 0, 0);
        e["payload"]["info"]["last_token_usage"] = json!({"input_tokens":800,"output_tokens":0});
        r.ingest(e, "a");
        assert_eq!(r.last_usage.as_ref().unwrap().total, 50);
        assert_eq!(r.usage.as_ref().unwrap().total, 1050);
    }
    #[test]
    fn context_is_latest_measured_usage_with_an_explicit_capacity() {
        let mut r = Rollout::default();
        assert!(r.context().is_none());
        r.ingest(record("a", "r1", 5000, 100), "a");
        assert!(r.context().is_none());
        let mut e = event(5000, 0, 0);
        e["payload"]["info"]["last_token_usage"] = json!({"input_tokens":120,"cached_input_tokens":80,"output_tokens":30,"total_tokens":150});
        e["payload"]["info"]["model_context_window"] = json!(1000);
        r.ingest(e, "a");
        assert_eq!(r.context().unwrap()["percent"], 15.);
        assert_eq!(r.last_usage.as_ref().unwrap().total, 100);
        r.ingest(record("a", "r2", 5200, 200), "a");
        assert_eq!(r.context().unwrap()["used"], 200);
        assert_eq!(r.context().unwrap()["percent"], 20.);
        r.ingest(json!({"type":"compacted"}), "a");
        assert!(r.context().is_none());
        assert_eq!(r.last_usage.as_ref().unwrap().total, 200);
        r.ingest(record("a", "r3", 5500, 300), "a");
        r.ingest(
            json!({"type":"event_msg","payload":{"type":"task_started","model_context_window":0}}),
            "a",
        );
        assert!(r.context().is_none());
    }
    #[test]
    fn legacy_last_response_and_missing_usage_remain_distinct() {
        let mut r = Rollout::default();
        r.ingest(event(4000, 3000, 100), "a");
        assert!(r.last_usage.is_none());
        let mut e = event(5000, 3800, 150);
        e["payload"]["info"]["last_token_usage"] =
            json!({"input_tokens":1000,"cached_input_tokens":800,"output_tokens":50});
        r.ingest(e, "a");
        let u = r.last_usage.as_ref().unwrap();
        assert_eq!((u.input, u.output, u.cache, u.total), (1000, 50, 800, 1050));
        r.ingest(json!({"type":"token_usage_record","payload":{"thread_id":"a","response_id":"missing"}}), "a");
        assert!(r.last_usage.is_none());
    }
    #[test]
    fn last_response_cost_uses_its_recorded_model_and_tier() {
        let mut r = Rollout::default();
        assert!(r.last_usd().is_none());
        r.model = "gpt-6.1-sol".into();
        r.tier = "priority".into();
        let mut e = record("a", "r1", 120, 100);
        e["payload"]["usage"] = json!({"input_tokens":100,"cached_input_tokens":80,"output_tokens":20,"total_tokens":120});
        e["payload"]["thread_token_usage"] = e["payload"]["usage"].clone();
        r.ingest(e, "a");
        r.model = "unknown-new-model".into();
        r.tier = "unknown".into();
        assert!((r.last_usd().unwrap() - 0.000496).abs() < 1e-10);
        r.ingest(record("a", "r2", 220, 100), "a");
        assert!(r.last_usd().is_none());
    }
}
