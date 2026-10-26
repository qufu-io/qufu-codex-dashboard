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
    #[serde(default)]
    pub write_long: u64,
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
            write_long: v["cache_write_12h_input_tokens"].as_u64().unwrap_or(0),
            reasoning: v["reasoning_output_tokens"].as_u64().unwrap_or(0),
            total: v["total_tokens"]
                .as_u64()
                .unwrap_or(input.checked_add(output)?),
        })
    }
    fn delta(&self, prev: &Self) -> Self {
        Self {
            input: self.input.saturating_sub(prev.input),
            cache: self.cache.saturating_sub(prev.cache),
            write: self.write.saturating_sub(prev.write),
            write_long: self.write_long.saturating_sub(prev.write_long),
            output: self.output.saturating_sub(prev.output),
            reasoning: self.reasoning.saturating_sub(prev.reasoning),
            total: self.total.saturating_sub(prev.total),
        }
    }
    pub fn add(&mut self, v: &Self) {
        self.input += v.input;
        self.cache += v.cache;
        self.write += v.write;
        self.write_long += v.write_long;
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
    #[serde(rename = "usdApproximate")]
    pub usd_approximate: bool,
    #[serde(rename = "usdPartial")]
    pub usd_partial: bool,
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
    fn quote(&self) -> crate::pricing::Quote {
        if self.unpriced {
            return crate::pricing::Quote {
                total_tokens: self.usage.total,
                ..Default::default()
            };
        }
        crate::pricing::Quote::combine([
            crate::pricing::quote(&self.pricing_model, &self.pricing_tier, &self.short, false),
            if self.long.total > 0 {
                crate::pricing::quote(&self.pricing_model, &self.pricing_tier, &self.long, true)
            } else {
                crate::pricing::Quote::default()
            },
        ])
    }
    fn repriced(&self) -> Self {
        let mut result = self.clone();
        let q = self.quote();
        result.usd = q.usd;
        result.usd_approximate = q.approximate;
        result.usd_partial = q.partial;
        result
    }
}

#[derive(Clone)]
pub struct UsageSample {
    pub id: String,
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
    last_at: i64,
    cache_long: bool,
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
fn parent_id(value: &Value) -> Option<String> {
    if let Some(id) = value
        .get("parent_thread_id")
        .or_else(|| value.get("parent_session_id"))
        .and_then(Value::as_str)
    {
        return Some(id.into());
    }
    match value {
        Value::Object(v) => v.values().find_map(parent_id),
        _ => None,
    }
}
fn is_main_source(source: &str) -> bool {
    matches!(source, "vscode" | "cli" | "appServer" | "app-server")
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
                .map(str::to_owned)
                .or_else(|| parent_id(&p["source"]));
            return;
        }
        if !self.id.is_empty() && self.id != expected {
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
            self.cache_long = p
                .pointer("/prompt_cache_options/ttl")
                .and_then(Value::as_str)
                == Some("12h");
            return;
        }
        if ty == "event_msg" {
            if p["type"] == "thread_settings_applied" {
                if let Some(tier) = p.pointer("/thread_settings/service_tier") {
                    self.tier = tier.as_str().unwrap_or("").into();
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
                if t >= self.created {
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
            let response_key = p["response_id"]
                .as_str()
                .filter(|id| !id.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    format!(
                        "anonymous:{expected}:{t}:{}:{}",
                        p["thread_token_usage"], p["usage"]
                    )
                });
            if !self.seen.insert(response_key) {
                return;
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
        let event_model = p["model"]
            .as_str()
            .or_else(|| p.pointer("/response/model").and_then(Value::as_str))
            .unwrap_or(&self.model)
            .to_owned();
        let event_tier = p["service_tier"].as_str().unwrap_or(&self.tier).to_owned();
        if is_record && t >= self.last_at {
            self.last_at = t;
            self.last_usage = raw.clone().map(|mut u| {
                if self.cache_long {
                    u.write_long = u.write;
                }
                u
            });
            self.last_model = event_model.clone();
            self.last_tier = event_tier.clone();
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
        let long_context = raw.as_ref().is_some_and(|u| u.input > 272_000);
        let had_previous = self.usage.is_some();
        let raw_known = raw.clone();
        let mut d = if is_record {
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
        if !is_record && !had_previous {
            if let Some(response) = &raw_known {
                let baseline = cur.delta(response);
                if baseline.total > 0 {
                    self.usage = Some(baseline.clone());
                    self.models.insert(
                        "Geçmiş / model verisi yok".into(),
                        ModelUsage {
                            model: "Geçmiş / model verisi yok".into(),
                            usage: baseline,
                            unpriced: true,
                            ..Default::default()
                        },
                    );
                    d = response.clone();
                }
            }
        }
        if self.cache_long {
            d.write_long = d.write;
        }
        self.previous = cur;
        self.usage.get_or_insert_with(Usage::default).add(&d);
        if d.total == 0 {
            return;
        }
        self.events.push((t, d.total));
        if (is_record || had_previous || raw_known.is_some()) && t > 0 {
            self.window_samples.push(UsageSample {
                id: if is_record {
                    format!(
                        "response:{}",
                        p["response_id"]
                            .as_str()
                            .filter(|id| !id.is_empty())
                            .map(str::to_owned)
                            .unwrap_or_else(|| format!(
                                "anonymous:{expected}:{t}:{}:{}",
                                p["thread_token_usage"], p["usage"]
                            ))
                    )
                } else {
                    format!("legacy:{expected}:{t}:{}", self.previous.total)
                },
                at: t,
                usage: d.clone(),
                model: event_model.clone(),
                tier: event_tier.clone(),
            });
        }
        let key = format!(
            "{}{}",
            event_model,
            if event_tier.is_empty() {
                String::new()
            } else {
                format!(" / {}", event_tier)
            }
        );

        let entry = self
            .models
            .entry(key.clone())
            .or_insert_with(|| ModelUsage {
                model: key,
                usage: Usage::default(),
                usd: Some(0.),
                pricing_model: event_model.clone(),
                pricing_tier: event_tier.clone(),
                ..Default::default()
            });
        entry.usage.add(&d);
        if long_context || d.input > 272_000 {
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
            if n == 0 {
                break;
            }
            if !line.ends_with(b"\n") {
                if let Ok(event) = serde_json::from_slice(&line) {
                    self.ingest(event, id);
                }
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

        Ok(())
    }
    pub(crate) fn cursor(&self) -> Value {
        json!({"offset":self.offset,"previous":self.previous,"created":self.created,"id":self.id,"model":self.model,"tier":self.tier,"recordMode":self.record_mode,"cacheLong":self.cache_long})
    }
    pub(crate) fn restore_cursor(state: &Value) -> Self {
        Self {
            offset: state["offset"].as_u64().unwrap_or(0),
            previous: serde_json::from_value(state["previous"].clone()).unwrap_or_default(),
            created: state["created"].as_i64().unwrap_or(0),
            id: state["id"].as_str().unwrap_or("").into(),
            model: state["model"].as_str().unwrap_or("").into(),
            tier: state["tier"].as_str().unwrap_or("").into(),
            record_mode: state["recordMode"].as_bool().unwrap_or(false),
            cache_long: state["cacheLong"].as_bool().unwrap_or(false),
            usage: Some(Usage::default()),
            ..Default::default()
        }
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
    pub fn cost_quote(&self) -> crate::pricing::Quote {
        if self.usage.is_none() {
            return crate::pricing::Quote::default();
        }
        crate::pricing::Quote::combine(self.models.values().map(ModelUsage::quote))
    }
    pub fn usd(&self) -> Option<f64> {
        self.cost_quote().usd
    }
    pub fn last_quote(&self) -> crate::pricing::Quote {
        self.last_usage
            .as_ref()
            .map(|u| crate::pricing::quote(&self.last_model, &self.last_tier, u, u.input > 272_000))
            .unwrap_or_default()
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
    paths: HashMap<String, PathBuf>,
    paths_scanned: Option<SystemTime>,
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
            ..Default::default()
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
            let parent = parent_id(&src);
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
                    if let Some(t) = result.iter_mut().find(|t| {
                        t.id == edge.1 && (!is_main_source(&t.source) || t.parent.is_some())
                    }) {
                        t.parent = Some(edge.0);
                    }
                }
            }
        }
        Ok((result, global))
    }
    pub(crate) fn rollout_path(&mut self, t: &Thread) -> PathBuf {
        let path = PathBuf::from(&t.path);
        if path.is_file() {
            return path;
        }
        if self
            .paths_scanned
            .is_none_or(|at| at.elapsed().map(|d| d.as_secs() >= 30).unwrap_or(true))
        {
            self.paths.clear();
            let mut pending = vec![
                self.home.join("sessions"),
                self.home.join("archived_sessions"),
            ];
            while let Some(directory) = pending.pop() {
                if let Ok(entries) = fs::read_dir(directory) {
                    for entry in entries.flatten() {
                        let Ok(kind) = entry.file_type() else {
                            continue;
                        };
                        if kind.is_dir() {
                            pending.push(entry.path());
                        } else if kind.is_file()
                            && entry.path().extension().is_some_and(|v| v == "jsonl")
                        {
                            self.paths.insert(
                                entry.file_name().to_string_lossy().into_owned(),
                                entry.path(),
                            );
                        }
                    }
                }
            }
            self.paths_scanned = Some(SystemTime::now());
        }
        self.paths
            .iter()
            .find(|(name, _)| {
                name.ends_with(&format!("-{}.jsonl", t.id)) || **name == format!("{}.jsonl", t.id)
            })
            .map(|(_, p)| p.clone())
            .unwrap_or(path)
    }
    pub fn read_thread(&mut self, t: &Thread) -> Result<&Rollout, String> {
        let path = self.rollout_path(t);
        let cached = self.cache.entry(t.id.clone()).or_default();
        cached.read(&path, &t.id)?;
        Ok(cached)
    }
    pub fn snapshot(&mut self, selected: Option<&str>) -> Result<Value, String> {
        let now = chrono::Utc::now().timestamp();
        let (mut all, _global) = self.catalog()?;
        for t in &mut all {
            if !is_main_source(&t.source) && t.parent.is_none() {
                if let Ok(f) = File::open(self.rollout_path(t)) {
                    for line in BufReader::new(f).lines().map_while(Result::ok).take(8) {
                        if let Ok(v) = serde_json::from_str::<Value>(&line) {
                            if v["type"] == "session_meta" {
                                t.parent = v["payload"]["parent_thread_id"]
                                    .as_str()
                                    .map(str::to_owned)
                                    .or_else(|| parent_id(&v["payload"]["source"]));
                                break;
                            }
                        }
                    }
                }
            }
        }
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
                    if all
                        .iter()
                        .any(|t| t.id == *id && (!is_main_source(&t.source) || t.parent.is_some()))
                        && r.parent.as_ref().is_some_and(|p| ids.contains(p))
                        && ids.insert(id.clone())
                    {
                        changed = true;
                    }
                }
            }
            let mut children = vec![];
            let mut child_usage = Usage::default();
            let mut quotes = Vec::new();
            let mut known = 0;
            let mut complete = true;
            for c in all.iter().filter(|c| c.id != t.id && ids.contains(&c.id)) {
                let _ = self.read_thread(c);
                let r = self.cache.get(&c.id);
                let usage = r.and_then(|r| r.usage.as_ref());
                if let Some(u) = usage {
                    child_usage.add(u);
                    known += 1;
                } else {
                    complete = false;
                }
                let q = r.map(Rollout::cost_quote).unwrap_or_default();
                quotes.push(q.clone());
                children.push(json!({"id":c.id,"parentId":c.parent,"title":c.title,"usage":usage,"usd":q.usd,"usdApproximate":q.approximate,"usdPartial":q.partial,"status":r.map(|r|r.status.as_str()).unwrap_or("Kayıt okunamıyor"),"internal":c.source.contains("guardian")}));
            }
            let mut child_quote = crate::pricing::Quote::combine(quotes);
            child_quote.partial |= !complete;
            if known == 0 && !children.is_empty() {
                child_quote.usd = None;
            }
            if let Some(r) = self.cache.get(&t.id) {
                detail = json!({"thread":t,"usage":r.usage,"lastUsage":r.last_usage,"lastUsd":r.last_usd(),"context":r.context(),"contextCapacity":r.context_capacity,"usd":r.usd(),"usdApproximate":r.cost_quote().approximate,"usdPartial":r.cost_quote().partial,"lastUsdApproximate":r.last_quote().approximate,"lastUsdPartial":r.last_quote().partial,"tpm":r.tpm(now),"model":r.model,"usageSource":if r.record_mode{"response records"}else{"legacy cumulative counters"},"status":if r.status.is_empty(){"Bilinmiyor"}else{&r.status},"models":r.models.values().map(ModelUsage::repriced).collect::<Vec<_>>(),"warnings":r.warnings,"children":children,"childUsage":if known>0 || children.is_empty(){Some(child_usage)}else{None},"childUsd":child_quote.usd,"childUsdApproximate":child_quote.approximate,"childUsdPartial":child_quote.partial,"childrenComplete":complete});
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
        assert!((r.usd().unwrap() - 0.0002).abs() < 1e-10);
        assert!(r.cost_quote().partial);
        assert_eq!(r.cost_quote().priced_tokens, 100);
        assert_eq!(r.cost_quote().total_tokens, 1000);
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
            ..Default::default()
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
        assert_eq!(r.usage.as_ref().unwrap().total, 10);
        assert_eq!(r.offset, 0);
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

#[cfg(test)]
mod regression_tests {
    use super::*;
    use rusqlite::params;
    fn temp() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "qufu-subagents-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }
    fn record(id: &str, response: &str, tokens: u64) -> Value {
        json!({"type":"token_usage_record","timestamp":"2026-10-26T12:00:00Z","payload":{"thread_id":id,"response_id":response,"usage":{"input_tokens":tokens,"cached_input_tokens":0,"output_tokens":0},"thread_token_usage":{"input_tokens":tokens,"output_tokens":0}}})
    }
    #[test]
    fn nested_archived_and_partial_children_keep_the_known_subtotal_once() {
        let root = temp();
        let db = Connection::open(root.join("state_5.sqlite")).unwrap();
        db.execute_batch("CREATE TABLE threads(id TEXT PRIMARY KEY,name TEXT,title TEXT,cwd TEXT,rollout_path TEXT,source TEXT,archived INTEGER,tokens_used INTEGER,updated_at INTEGER);CREATE TABLE thread_spawn_edges(parent_thread_id TEXT,child_thread_id TEXT);").unwrap();
        for (id, source, archived) in [
            ("root", "cli", 0),
            (
                "a",
                r#"{"subagent":{"thread_spawn":{"parent_thread_id":"root"}}}"#,
                1,
            ),
            ("b", "worker", 1),
            ("missing", "subagent", 0),
            ("fork", "cli", 0),
        ] {
            let path = root.join(format!("rollout-{id}.jsonl"));
            if id != "missing" {
                let mut lines=vec![json!({"type":"session_meta","payload":{"id":id,"source":if id=="b"{json!({"subagent":{"other":{"parent_thread_id":"a"}}})}else{json!(source)}}}).to_string(),json!({"type":"turn_context","payload":{"model":"gpt-6.1-sol"}}).to_string()];
                if id == "a" {
                    lines.push(json!({"type":"event_msg","timestamp":"2026-10-26T12:00:00Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":20,"output_tokens":0},"last_token_usage":{"input_tokens":20,"output_tokens":0}}}}).to_string());
                } else {
                    lines.push(
                        record(id, &format!("r-{id}"), if id == "b" { 30 } else { 10 }).to_string(),
                    );
                }
                if id == "b" {
                    lines.push(record("a", "copied", 1000).to_string());
                }
                fs::write(&path, lines.join("\n")).unwrap();
            }
            db.execute(
                "INSERT INTO threads VALUES(?1,?1,?1,'C:/example',?2,?3,?4,100,1)",
                params![id, path.to_string_lossy(), source, archived],
            )
            .unwrap();
        }
        for (parent, child) in [
            ("root", "a"),
            ("root", "a"),
            ("a", "b"),
            ("root", "missing"),
            ("root", "fork"),
        ] {
            db.execute(
                "INSERT INTO thread_spawn_edges VALUES(?1,?2)",
                params![parent, child],
            )
            .unwrap();
        }
        let archived = root.join("archived_sessions/2026");
        fs::create_dir_all(&archived).unwrap();
        fs::rename(
            root.join("rollout-a.jsonl"),
            archived.join("rollout-a.jsonl"),
        )
        .unwrap();
        drop(db);
        let mut reader = Reader {
            home: root.clone(),
            ..Default::default()
        };
        let fork_path = root.join("rollout-fork.jsonl");
        let text = fs::read_to_string(&fork_path).unwrap();
        let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
        let mut meta: Value = serde_json::from_str(&lines[0]).unwrap();
        meta["payload"]["parent_thread_id"] = json!("root");
        lines[0] = meta.to_string();
        fs::write(&fork_path, lines.join("\n")).unwrap();
        reader.snapshot(Some("fork")).unwrap();
        let v = reader.snapshot(Some("root")).unwrap();
        let d = &v["detail"];
        assert_eq!(d["usage"]["total"], 10);
        assert_eq!(d["children"].as_array().unwrap().len(), 3);
        assert_eq!(d["childUsage"]["total"], 50);
        assert_eq!(d["childrenComplete"], false);
        assert!((d["childUsd"].as_f64().unwrap() - 0.0001).abs() < 1e-10);
        assert_eq!(d["childUsdPartial"], true);
        let missing = d["children"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"] == "missing")
            .unwrap();
        assert!(missing["usage"].is_null());
        let sessions: Vec<_> = v["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["id"].as_str().unwrap())
            .collect();
        assert!(sessions.contains(&"fork"));
        assert!(!sessions.contains(&"b"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn incomplete_json_waits_but_complete_tail_is_read_and_deduplicated() {
        let root = temp();
        let path = root.join("rollout-a.jsonl");
        let line = record("a", "r1", 20).to_string();
        fs::write(&path, &line[..line.len() / 2]).unwrap();
        let mut r = Rollout::default();
        r.read(&path, "a").unwrap();
        assert!(r.usage.is_none());
        fs::write(&path, &line).unwrap();
        r.read(&path, "a").unwrap();
        assert_eq!(r.usage.as_ref().unwrap().total, 20);
        fs::write(&path, format!("{line}\n{}\n", record("a", "r2", 5))).unwrap();
        r.read(&path, "a").unwrap();
        assert_eq!(r.usage.as_ref().unwrap().total, 25);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn explicit_response_model_and_tier_win_over_a_later_turn_context() {
        let mut r = Rollout {
            model: "gpt-6.1-sol".into(),
            ..Default::default()
        };
        let mut v = record("a", "r", 100);
        v["payload"]["model"] = json!("gpt-6-astra-high");
        v["payload"]["service_tier"] = json!("priority");
        r.ingest(v, "a");
        assert!((r.last_usd().unwrap() - 0.002).abs() < 1e-10);
        assert!((r.usd().unwrap() - 0.002).abs() < 1e-10);
        r.ingest(
            json!({"type":"turn_context","payload":{"model":"gpt-6-luna"}}),
            "a",
        );
        assert!((r.last_usd().unwrap() - 0.002).abs() < 1e-10);
    }
    #[test]
    fn missing_model_cost_keeps_other_models_without_guessing_internal_prices() {
        let mut r = Rollout {
            model: "gpt-6.1-sol".into(),
            ..Default::default()
        };
        r.ingest(record("a", "r1", 100), "a");
        r.model = "codex-auto-review".into();
        r.ingest(record("a", "r2", 200), "a");
        assert_eq!(r.usage.as_ref().unwrap().total, 300);
        assert!(r.cost_quote().partial);
        assert!((r.usd().unwrap() - 0.0002).abs() < 1e-10);
        assert!(r.last_usd().is_none());
    }
    #[test]
    fn explicit_null_settings_tier_clears_priority_but_absent_turn_tier_does_not() {
        let mut r = Rollout {
            tier: "priority".into(),
            ..Default::default()
        };
        r.ingest(
            json!({"type":"turn_context","payload":{"model":"gpt-6.1-sol"}}),
            "a",
        );
        assert_eq!(r.tier, "priority");
        r.ingest(json!({"type":"event_msg","payload":{"type":"thread_settings_applied","thread_settings":{"service_tier":null}}}),"a");
        assert_eq!(r.tier, "");
    }
    #[test]
    fn complete_tail_without_response_id_is_not_recounted_on_refresh_or_newline() {
        let root = temp();
        let path = root.join("anonymous.jsonl");
        let mut first = record("a", "unused", 10);
        first["payload"]
            .as_object_mut()
            .unwrap()
            .remove("response_id");
        let line = first.to_string();
        std::fs::write(&path, &line).unwrap();
        let mut r = Rollout::default();
        r.read(&path, "a").unwrap();
        r.read(&path, "a").unwrap();
        assert_eq!(r.usage.as_ref().unwrap().total, 10);
        std::fs::write(&path, format!("{line}\n")).unwrap();
        r.read(&path, "a").unwrap();
        assert_eq!(r.usage.as_ref().unwrap().total, 10);
        let mut second = first.clone();
        second["payload"]["thread_token_usage"]["input_tokens"] = json!(20);
        std::fs::write(&path, format!("{line}\n{}\n", second)).unwrap();
        r.read(&path, "a").unwrap();
        assert_eq!(r.usage.as_ref().unwrap().total, 20);
        assert_eq!(r.window_samples.len(), 2);
        std::fs::remove_dir_all(root).unwrap();
    }
}
