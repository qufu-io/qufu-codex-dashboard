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
    #[serde(default)]
    pub cache_missing: bool,
    #[serde(default)]
    pub reasoning_missing: bool,
}
impl Usage {
    pub fn from(v: &Value) -> Option<Self> {
        let input = v.get("input_tokens")?.as_u64()?;
        let output = v.get("output_tokens")?.as_u64()?;
        Some(Self {
            input,
            output,
            cache_missing: !v["cached_input_tokens"].is_u64(),
            reasoning_missing: !v["reasoning_output_tokens"].is_u64(),
            cache: v["cached_input_tokens"].as_u64().unwrap_or(0),
            write: v["cache_write_input_tokens"].as_u64().unwrap_or(0),
            write_long: v["cache_write_12h_input_tokens"].as_u64().unwrap_or(0),
            reasoning: v["reasoning_output_tokens"].as_u64().unwrap_or(0),
            total: v["total_tokens"]
                .as_u64()
                .unwrap_or(input.checked_add(output)?),
        })
    }
    pub fn raw_total(&self) -> u64 {
        self.input.saturating_add(self.output)
    }
    pub fn used(&self) -> u64 {
        self.input
            .saturating_sub(self.cache)
            .saturating_add(self.output)
    }
    pub fn delta(&self, prev: &Self) -> Self {
        Self {
            cache_missing: self.cache_missing || prev.cache_missing,
            reasoning_missing: self.reasoning_missing || prev.reasoning_missing,
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
        self.cache_missing |= v.cache_missing;
        self.reasoning_missing |= v.reasoning_missing;
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
    pub turn_id: Option<String>,
    pub root_turn_id: Option<String>,
    pub effort: Option<String>,
}
#[derive(Clone, Default)]
pub struct Rollout {
    pub offset: u64,
    pub observed_bytes: u64,
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
    pub turns: crate::turns::Book,
    pub cli_usage: Option<Usage>,
    cli_pending: bool,
    cli_last: Option<Usage>,
    cli_fresh: bool,
    cli_at: i64,
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
        if e["method"] == "thread/tokenUsage/updated" {
            let p = &e["params"];
            if p["threadId"].as_str() != Some(expected) {
                return;
            }
            let convert = |v: &Value| json!({"input_tokens":v["inputTokens"],"cached_input_tokens":v["cachedInputTokens"],"cache_write_input_tokens":v["cacheWriteInputTokens"],"output_tokens":v["outputTokens"],"reasoning_output_tokens":v["reasoningOutputTokens"],"total_tokens":v["totalTokens"]});
            self.ingest(json!({"type":"event_msg","timestamp":e["timestamp"],"payload":{"type":"token_count","turn_id":p["turnId"],"info":{"total_token_usage":convert(&p["tokenUsage"]["total"]),"last_token_usage":convert(&p["tokenUsage"]["last"]),"model_context_window":p["tokenUsage"]["modelContextWindow"]}}}),expected);
            return;
        }
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
        if t >= self.created {
            self.turns.observe(&e, expected, self.context());
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
                if let Some(model) = p.pointer("/thread_settings/model").and_then(Value::as_str) {
                    if self.model != model {
                        self.context_used = None;
                    }
                    self.model = model.into();
                }
                if let Some(tier) = p.pointer("/thread_settings/service_tier") {
                    self.tier = tier.as_str().unwrap_or("").into();
                }
            }
            match p["type"].as_str().unwrap_or("") {
                "task_started" | "turn_started" => {
                    self.status = "Çalışıyor".into();
                    if t >= self.created {
                        let capacity = p["model_context_window"].as_u64().filter(|v| *v > 0);
                        if capacity != self.context_capacity {
                            self.context_used = None;
                        }
                        self.context_capacity = capacity;
                    }
                }
                "task_complete" | "turn_complete" | "turn_completed" => {
                    self.status = "Tamamlandı".into()
                }
                "turn_aborted" => self.status = "Kesildi".into(),
                _ => {}
            }
            if p["type"] == "token_count" && t >= self.created && p["info"].is_object() {
                if let Some(usage) = Usage::from(&p["info"]["total_token_usage"]) {
                    let changed = self.cli_usage.as_ref() != Some(&usage);
                    if self.cli_pending {
                        self.cli_fresh = false;
                    } else if changed {
                        self.cli_fresh = true;
                    }
                    if changed {
                        self.cli_at = t;
                    }
                    self.cli_usage = Some(usage);
                    self.cli_pending = false;
                }
                self.cli_last = Usage::from(&p["info"]["last_token_usage"]);
                self.context_capacity = p["info"]["model_context_window"]
                    .as_u64()
                    .filter(|v| *v > 0);
                if let Some(used) = p["info"]["last_token_usage"]["total_tokens"].as_u64() {
                    self.context_used = Some(used);
                }
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
                    self.quota_history.push((t, json!({"primary":p["rate_limits"]["primary"],"secondary":p["rate_limits"]["secondary"],"plan_type":p["rate_limits"]["plan_type"],"limit_id":p["rate_limits"]["limit_id"]})));
                }
            }
        }
        if t < self.created {
            return;
        }
        let is_record = ty == "token_usage_record";
        let transition = is_record && !self.record_mode;
        let prior_last = self.last_usage.clone();
        let prior_at = self.last_at;
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
            let raw = Usage::from(&p["usage"]);
            let matched = raw
                .as_ref()
                .zip(self.cli_last.as_ref())
                .is_some_and(|(a, b)| a.input == b.input && a.output == b.output);
            self.cli_pending = !(self.cli_fresh
                && (0..=5).contains(&(t - self.cli_at))
                && (matched || raw.is_none()));
            self.cli_fresh = false;
            if !self.record_mode {
                self.record_mode = true;
                if let (Some(cumulative), Some(response)) = (
                    Usage::from(&p["thread_token_usage"]),
                    Usage::from(&p["usage"]),
                ) {
                    let baseline = cumulative.delta(&response);
                    let prior = self.usage.clone().unwrap_or_default();
                    if baseline.total > prior.total {
                        let missing = baseline.delta(&prior);
                        self.usage.get_or_insert_with(Usage::default).add(&missing);
                        self.models.insert(
                            "Earlier usage / model unavailable".into(),
                            ModelUsage {
                                model: "Earlier usage / model unavailable".into(),
                                usage: missing,
                                unpriced: true,
                                ..Default::default()
                            },
                        );
                    }
                    if let Some(pos) = self
                        .window_samples
                        .iter()
                        .rposition(|s| s.at == t && s.usage == response)
                    {
                        self.window_samples.remove(pos);
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
        if is_record && (t >= self.last_at || t == 0) {
            self.last_at = t;
            self.last_usage = raw.clone().map(|mut u| {
                if self.cache_long {
                    u.write_long = u.write;
                }
                u
            });
            self.last_model = event_model.clone();
            self.last_tier = event_tier.clone();
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
        let overlap = transition
            && self.previous.total > 0
            && ((cur == self.previous)
                || (raw.as_ref() == prior_last.as_ref()
                    && t == prior_at
                    && self.previous.total >= cur.total));
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
        if !overlap {
            self.usage.get_or_insert_with(Usage::default).add(&d);
        }
        if d.total == 0 {
            return;
        }
        if !overlap {
            self.events.push((t, d.total));
        }
        if !is_record {
            self.last_at = t;
        }
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
                turn_id: p["turn_id"]
                    .as_str()
                    .map(str::to_owned)
                    .or_else(|| self.turns.active.clone()),
                root_turn_id: p["root_turn_id"].as_str().map(str::to_owned),
                effort: self.turns.latest().and_then(|t| t.effort.clone()),
            });
        }
        if overlap {
            return;
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
        self.observed_bytes = self.offset;
        let mut line = Vec::new();
        loop {
            line.clear();
            let n = reader
                .read_until(b'\n', &mut line)
                .map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            self.observed_bytes = self.offset + n as u64;
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
                b"\"turn_started\"",
                b"\"turn_complete\"",
                b"\"turn_completed\"",
                b"\"thread/tokenUsage/updated\"",
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
        json!({"offset":self.offset,"previous":self.previous,"created":self.created,"id":self.id,"model":self.model,"tier":self.tier,"recordMode":self.record_mode,"cacheLong":self.cache_long,"turns":self.turns,"cursorVersion":2})
    }
    pub(crate) fn restore_cursor(state: &Value) -> Self {
        Self {
            turns: serde_json::from_value(state["turns"].clone()).unwrap_or_default(),
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
    pub fn displayed_usage(&self) -> Option<&Usage> {
        if !self.cli_pending {
            self.cli_usage.as_ref().or(self.usage.as_ref())
        } else {
            self.usage.as_ref()
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
        Some(crate::turns::context(used, capacity))
    }
    pub fn cost_quote(&self) -> crate::pricing::Quote {
        if self.usage.is_none() {
            return crate::pricing::Quote::default();
        }
        let mut q = crate::pricing::Quote::combine(self.models.values().map(ModelUsage::quote));
        q.partial |= self
            .displayed_usage()
            .is_some_and(|u| u.raw_total() != q.total_tokens);
        q
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
            let root_turn = self
                .cache
                .get(&t.id)
                .and_then(|r| r.turns.latest())
                .map(|t| t.root_turn_id.as_deref().unwrap_or(&t.id).to_owned());
            let mut task_child_usage: Option<Usage> = None;
            let mut task_child_quotes = Vec::new();
            let mut task_child_count = 0;
            let mut task_children_unknown = 0;
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
                let linked: Vec<_> = r
                    .map(|r| {
                        r.turns
                            .turns
                            .values()
                            .filter(|turn| {
                                turn.root_turn_id
                                    .as_ref()
                                    .zip(root_turn.as_ref())
                                    .is_some_and(|(a, b)| a == b)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                if linked.is_empty() {
                    task_children_unknown += 1;
                }
                for turn in linked {
                    if let Some(u) = &turn.usage {
                        task_child_usage.get_or_insert_with(Usage::default).add(u);
                        task_child_count += 1;
                    } else {
                        task_children_unknown += 1;
                    }
                    task_child_quotes.push(turn.quote());
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
                let turn = r.turns.latest();
                let turn_quote = turn.map(crate::turns::Turn::quote).unwrap_or_default();
                detail = json!({"turnChildUsage":task_child_usage,"turnChildUsd":if task_child_count>0 {crate::pricing::Quote::combine(task_child_quotes).usd}else {None},"turnChildCount":task_child_count,"turnChildrenPartial":task_children_unknown>0,"turn":turn.map(crate::turns::Turn::value),"activeTurn":r.turns.active,"lastCompletedTurn":r.turns.latest_completed().map(crate::turns::Turn::value),"used":r.displayed_usage().filter(|u|!u.cache_missing).map(Usage::used),"rawTotal":r.displayed_usage().map(Usage::raw_total),"cliUsage":r.cli_usage,"responseUsage":r.usage,"rolloutBytes":r.observed_bytes,"thread":t,"usage":r.displayed_usage(),"lastUsage":turn.and_then(|t|t.usage.as_ref()),"lastUsd":turn_quote.usd,"context":r.context(),"contextCapacity":r.context_capacity,"usd":r.usd(),"usdApproximate":r.cost_quote().approximate,"usdPartial":r.cost_quote().partial,"lastUsdApproximate":turn_quote.approximate,"lastUsdPartial":turn_quote.partial,"tpm":r.tpm(now),"model":r.model,"usageSource":if r.cli_usage.is_some() && !r.cli_pending {"Codex token_count.info.total_token_usage"}else if r.record_mode{"response records (CLI snapshot pending)"}else{"legacy cumulative counters"},"status":if r.status.is_empty(){"Bilinmiyor"}else{&r.status},"models":r.models.values().map(ModelUsage::repriced).collect::<Vec<_>>(),"warnings":r.warnings,"children":children,"childUsage":if known>0 || children.is_empty(){Some(child_usage)}else{None},"childUsd":child_quote.usd,"childUsdApproximate":child_quote.approximate,"childUsdPartial":child_quote.partial,"childrenComplete":complete});
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
        json!({"type":"token_usage_record","timestamp":"2026-10-18T12:00:00Z","payload":{"thread_id":id,"response_id":response,"thread_token_usage":{"input_tokens":total,"output_tokens":0},"usage":{"input_tokens":raw,"cached_input_tokens":0,"output_tokens":0}}})
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
        assert_eq!(r.observed_bytes, line.len() as u64);
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
        assert_eq!(r.context().unwrap()["percent"], 100.);
        assert_eq!(r.last_usage.as_ref().unwrap().total, 100);
        r.ingest(record("a", "r2", 5200, 200), "a");
        assert_eq!(r.context().unwrap()["used"], 150);
        assert_eq!(r.context().unwrap()["percent"], 100.);
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
    #[test]
    fn legacy_transition_preserves_history_and_does_not_inflate_rate() {
        let at = "2026-10-27T12:00:00Z";
        let mut r = Rollout::default();
        r.ingest(json!({"type":"event_msg","timestamp":at,"payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1100,"cached_input_tokens":800,"output_tokens":0},"last_token_usage":{"input_tokens":100,"cached_input_tokens":80,"output_tokens":0}}}}),"a");
        r.ingest(json!({"type":"token_usage_record","timestamp":at,"payload":{"thread_id":"a","turn_id":"t","response_id":"r","usage":{"input_tokens":100,"cached_input_tokens":80,"output_tokens":0},"thread_token_usage":{"input_tokens":100,"cached_input_tokens":80,"output_tokens":0},"turn_token_usage":{"input_tokens":100,"cached_input_tokens":80,"output_tokens":0}}}),"a");
        assert_eq!(r.usage.as_ref().unwrap().input, 1100);
        assert_eq!(r.tpm(timestamp(&json!(at))), Some(100));
        assert_eq!(r.window_samples.len(), 1);
        r.ingest(json!({"type":"event_msg","timestamp":at,"payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1100,"cached_input_tokens":800,"output_tokens":0}}}}),"a");
        assert_eq!(r.displayed_usage().unwrap().input, 1100);
        assert_eq!(r.displayed_usage().unwrap().used(), 300);
    }
    #[test]
    fn missing_cache_is_not_a_zero_used_measurement_or_a_fully_priced_input() {
        let usage = Usage::from(&json!({"input_tokens":100,"output_tokens":10})).unwrap();
        assert!(usage.cache_missing && usage.reasoning_missing);
        let q = crate::pricing::quote("gpt-6.1-sol", "standard", &usage, false);
        assert!(q.partial);
        assert_eq!(q.priced_tokens, 10);
        assert!((q.usd.unwrap() - 0.0001).abs() < 1e-10);
    }
    #[test]
    fn official_app_server_notification_matches_rollout_measurements() {
        let mut r = Rollout::default();
        let u = json!({"inputTokens":1000,"cachedInputTokens":800,"outputTokens":200,"reasoningOutputTokens":100,"totalTokens":1200});
        let mut e = json!({"method":"thread/tokenUsage/updated","timestamp":"2026-10-27T12:00:00Z","params":{"threadId":"a","turnId":"t","tokenUsage":{"total":u,"last":u,"modelContextWindow":128000}}});
        r.ingest(e.clone(), "a");
        assert_eq!(r.displayed_usage().unwrap().used(), 400);
        assert_eq!(r.displayed_usage().unwrap().input, 1000);
        assert_eq!(r.context().unwrap()["used"], 1200);
        assert_eq!(r.context().unwrap()["percent"], 0.);
        e["params"]["threadId"] = json!("other");
        e["params"]["tokenUsage"]["total"]["inputTokens"] = json!(9000);
        r.ingest(e, "a");
        assert_eq!(r.displayed_usage().unwrap().input, 1000);
    }
    #[test]
    fn cli_snapshot_is_authoritative_in_both_response_event_orders() {
        let stamp = "2026-10-27T12:00:00Z";
        let usage = |i| json!({"input_tokens":i,"cached_input_tokens":0,"output_tokens":0});
        let count = |total| json!({"type":"event_msg","timestamp":stamp,"payload":{"type":"token_count","info":{"total_token_usage":usage(total),"last_token_usage":usage(100)}}});
        let response = |id, total| json!({"type":"token_usage_record","timestamp":stamp,"payload":{"thread_id":"a","response_id":id,"usage":usage(100),"thread_token_usage":usage(total)}});
        let mut r = Rollout::default();
        r.ingest(count(1000), "a");
        r.ingest(response("one", 100), "a");
        assert!(!r.cli_pending);
        assert_eq!(r.displayed_usage().unwrap().input, 1000);
        r.ingest(count(200), "a");
        r.ingest(response("two", 200), "a");
        assert!(!r.cli_pending);
        assert_eq!(r.displayed_usage().unwrap().input, 200);
        assert_eq!(r.usage.as_ref().unwrap().input, 1100);
        r.ingest(response("three", 300), "a");
        assert!(r.cli_pending);
        r.ingest(count(300), "a");
        assert!(!r.cli_pending);
        assert_eq!(r.displayed_usage().unwrap().input, 300);
        r.ingest(response("four", 400), "a");
        assert!(r.cli_pending);
    }
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
        json!({"type":"token_usage_record","timestamp":"2026-10-26T12:00:00Z","payload":{"thread_id":id,"response_id":response,"usage":{"input_tokens":tokens,"cached_input_tokens":0,"output_tokens":0},"thread_token_usage":{"input_tokens":tokens,"cached_input_tokens":0,"output_tokens":0}}})
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
                    lines.push(json!({"type":"event_msg","timestamp":"2026-10-26T12:00:00Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":20,"cached_input_tokens":0,"output_tokens":0},"last_token_usage":{"input_tokens":20,"cached_input_tokens":0,"output_tokens":0}}}}).to_string());
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
