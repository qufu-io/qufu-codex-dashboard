use crate::{data::Usage, pricing};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};

pub fn context(used: u64, capacity: u64) -> Value {
    let remaining = if capacity <= 12_000 {
        0.
    } else {
        let effective = capacity - 12_000;
        (effective.saturating_sub(used.saturating_sub(12_000)) as f64 / effective as f64 * 100.)
            .clamp(0., 100.)
            .round()
    };
    json!({"used":used,"capacity":capacity,"remainingPercent":remaining,"percent":100.-remaining,"source":"Codex CLI baseline 12000"})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn u(i: u64, c: u64, o: u64) -> Value {
        json!({"input_tokens":i,"cached_input_tokens":c,"output_tokens":o,"reasoning_output_tokens":o/2,"total_tokens":i+o})
    }
    fn event(kind: &str, id: &str) -> Value {
        json!({"type":"event_msg","timestamp":"2026-10-27T12:00:00Z","payload":{"type":kind,"turn_id":id}})
    }
    fn record(id: &str, response: &str, raw: Value, total: Value) -> Value {
        json!({"type":"token_usage_record","timestamp":"2026-10-27T12:00:00Z","payload":{"thread_id":"a","turn_id":id,"response_id":response,"usage":raw,"turn_token_usage":total,"model":"gpt-6.1-sol"}})
    }
    #[test]
    fn cli_baseline_rounding_and_subsets_match_official_source() {
        for (used, capacity, remaining) in [
            (0, 1000, 0.),
            (12000, 128000, 100.),
            (13000, 128000, 99.),
            (70000, 128000, 50.),
            (127999, 128000, 0.),
            (140000, 128000, 0.),
        ] {
            assert_eq!(context(used, capacity)["remainingPercent"], remaining);
            assert_eq!(context(used, capacity)["percent"], 100. - remaining);
        }
        let usage = Usage::from(&u(1000, 800, 200)).unwrap();
        assert_eq!(usage.used(), 400);
        assert_eq!(usage.raw_total(), 1200);
        assert_eq!(usage.reasoning, 100);
    }
    #[test]
    fn matches_compiled_official_codex_oracle() {
        let oracle: Value =
            serde_json::from_str(include_str!("../../tests/fixtures/codex-measurements.json"))
                .unwrap();
        for row in oracle["cases"].as_array().unwrap() {
            assert_eq!(
                context(
                    row["contextTokens"].as_u64().unwrap(),
                    row["capacity"].as_u64().unwrap()
                )["remainingPercent"]
                    .as_f64()
                    .unwrap(),
                row["remainingPercent"].as_f64().unwrap()
            );
            assert_eq!(
                Usage::from(&u(1000, 800, 200)).unwrap().used(),
                row["cliUsed"].as_u64().unwrap()
            );
        }
    }
    #[test]
    fn turn_snapshots_replace_and_mixed_model_quotes_sum_individual_responses() {
        let mut b = Book::default();
        b.observe(&event("task_started", "z"), "a", None);
        let first = record("z", "r1", u(100, 60, 10), u(100, 60, 10));
        b.observe(&first, "a", None);
        b.observe(&first, "a", None);
        let mut second = record("z", "r2", u(200, 100, 20), u(300, 160, 30));
        second["payload"]["model"] = json!("gpt-6-astra");
        b.observe(&second, "a", None);
        b.observe(&event("task_complete", "z"), "a", None);
        let t = b.latest().unwrap();
        assert_eq!(t.usage.as_ref().unwrap().total, 330);
        assert_eq!(t.responses.len(), 2);
        let a = pricing::quote(
            "gpt-6.1-sol",
            "",
            &Usage::from(&u(100, 60, 10)).unwrap(),
            false,
        )
        .usd
        .unwrap();
        let c = pricing::quote(
            "gpt-6-astra",
            "",
            &Usage::from(&u(200, 100, 20)).unwrap(),
            false,
        )
        .usd
        .unwrap();
        assert!((t.quote().usd.unwrap() - a - c).abs() < 1e-10);
        assert!(!t.quote().partial);
        b.observe(&event("task_started", "a"), "a", None);
        assert_eq!(b.latest().unwrap().id, "a");
        assert!(b.latest().unwrap().usage.is_none());
        b.observe(&event("turn_aborted", "a"), "a", None);
        assert_eq!(b.latest().unwrap().status, "interrupted");
        let mut fail = event("task_complete", "failed");
        fail["payload"]["error"] = json!({"message":"not persisted"});
        b.observe(&fail, "a", None);
        assert_eq!(b.latest().unwrap().status, "failed");
        assert!(!serde_json::to_string(&b).unwrap().contains("not persisted"));
    }
    #[test]
    fn missing_response_retains_cumulative_task_total_and_partial_quote() {
        let mut b = Book::default();
        b.observe(&record("t", "one", u(100, 0, 0), u(100, 0, 0)), "a", None);
        b.observe(&record("t", "two", Value::Null, u(150, 0, 0)), "a", None);
        let t = b.latest().unwrap();
        assert_eq!(t.usage.as_ref().unwrap().total, 150);
        assert!(t.quote().partial);
        assert!(t.quote().usd.is_some());
    }
    #[test]
    fn an_unpriced_task_snapshot_is_not_a_zero_dollar_measurement() {
        let mut b = Book::default();
        b.observe(
            &record("task", "missing", Value::Null, u(150, 0, 0)),
            "a",
            None,
        );
        let task = b.latest().unwrap();
        assert_eq!(task.usage.as_ref().unwrap().total, 150);
        assert!(task.quote().usd.is_none());
        assert!(task.quote().partial);
    }
    #[test]
    fn legacy_turn_prices_preserve_the_declared_cache_write_lifetime() {
        let mut b = Book::default();
        b.observe(&event("task_started", "t"), "a", None);
        b.observe(&json!({"type":"turn_context","payload":{"turn_id":"t","model":"gpt-6.1-sol","prompt_cache_options":{"ttl":"12h"}}}),"a",None);
        for multiplier in [1, 2] {
            let mut raw = u(100, 60, 10);
            raw["cache_write_input_tokens"] = json!(20);
            let mut total = u(100 * multiplier, 60 * multiplier, 10 * multiplier);
            total["cache_write_input_tokens"] = json!(20 * multiplier);
            let mut count = event("token_count", "t");
            count["payload"]["info"] = json!({"total_token_usage":total,"last_token_usage":raw});
            b.observe(&count, "a", None);
        }
        assert_eq!(b.latest().unwrap().usage.as_ref().unwrap().total, 220);
        assert!((b.latest().unwrap().quote().usd.unwrap() - 0.000492).abs() < 1e-10);
    }
    #[test]
    fn quota_only_update_keeps_measured_context_and_cursor_restart_keeps_task() {
        let mut b = Book::default();
        b.observe(&event("task_started", "t"), "a", None);
        let mut count = event("token_count", "t");
        count["payload"]["info"] =
            json!({"total_token_usage":u(100,80,20),"last_token_usage":u(100,80,20)});
        b.observe(&count, "a", None);
        b.observe(&event("token_count", "t"), "a", None);
        let encoded = serde_json::to_value(b).unwrap();
        let mut b: Book = serde_json::from_value(encoded).unwrap();
        b.observe(
            &event("task_complete", "t"),
            "a",
            Some(context(120, 128000)),
        );
        assert_eq!(b.latest().unwrap().context.as_ref().unwrap()["used"], 120);
        b.observe(
            &event("task_started", "new"),
            "a",
            Some(context(120, 128000)),
        );
        b.observe(
            &event("task_complete", "new"),
            "a",
            Some(context(120, 128000)),
        );
        assert!(b.latest().unwrap().context.is_none());
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Response {
    pub id: String,
    #[serde(default)]
    pub at: i64,
    pub usage: Usage,
    pub model: String,
    pub tier: String,
    pub long: bool,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Turn {
    pub id: String,
    #[serde(default)]
    pub sequence: u64,
    pub root_turn_id: Option<String>,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub duration_ms: Option<i64>,
    pub status: String,
    pub usage: Option<Usage>,
    #[serde(default)]
    pub usage_partial: bool,
    #[serde(default)]
    missing_response: bool,
    pub context: Option<Value>,
    pub compacted: bool,
    pub model: String,
    pub effort: Option<String>,
    pub responses: Vec<Response>,
    #[serde(default)]
    record_mode: bool,
    #[serde(default)]
    fresh_context: bool,
}
impl Turn {
    pub fn quote(&self) -> pricing::Quote {
        let mut q = pricing::Quote::combine(
            self.responses
                .iter()
                .map(|r| pricing::quote(&r.model, &r.tier, &r.usage, r.long)),
        );
        q.partial |= self.missing_response;
        if let Some(u) = &self.usage {
            q.partial |= q.total_tokens != u.total;
            q.total_tokens = u.total;
            if u.total > 0 && q.priced_tokens == 0 {
                q.usd = None;
            }
        } else {
            q.usd = None;
        }
        q
    }
    pub fn value(&self) -> Value {
        let mut v = serde_json::to_value(self).unwrap();
        let q = self.quote();
        v["usd"] = json!(q.usd);
        v["usdPartial"] = json!(q.partial);
        v["usdApproximate"] = json!(q.approximate);
        v["used"] = json!(self
            .usage
            .as_ref()
            .filter(|u| !u.cache_missing)
            .map(Usage::used));
        v["rawTotal"] = json!(self.usage.as_ref().map(Usage::raw_total));
        v["source"] = json!(if self.record_mode {
            "Codex turn_token_usage / response records"
        } else {
            "Codex turn boundary / cumulative counters"
        });
        v
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Book {
    pub turns: BTreeMap<String, Turn>,
    pub active: Option<String>,
    seen: HashSet<String>,
    legacy_previous: Option<Usage>,
    model: String,
    tier: String,
    effort: Option<String>,
    #[serde(default)]
    long_write: bool,
    #[serde(default)]
    order: Vec<String>,
    #[serde(default)]
    counter: u64,
}
impl Book {
    pub fn compact_scanner_state(&mut self) {
        let keep = self.active.clone().or_else(|| self.order.last().cloned());
        self.turns.retain(|id, _| Some(id) == keep.as_ref());
        self.order.retain(|id| self.turns.contains_key(id));
        self.seen = self
            .turns
            .values()
            .flat_map(|t| t.responses.iter().map(|r| r.id.clone()))
            .collect();
    }
    pub fn latest_completed(&self) -> Option<&Turn> {
        self.order
            .iter()
            .rev()
            .filter_map(|id| self.turns.get(id))
            .find(|t| t.status == "completed")
    }
    pub fn latest(&self) -> Option<&Turn> {
        self.active
            .as_ref()
            .and_then(|id| self.turns.get(id))
            .or_else(|| self.order.last().and_then(|id| self.turns.get(id)))
    }
    pub fn observe(&mut self, e: &Value, thread: &str, context: Option<Value>) {
        let ty = e["type"].as_str().unwrap_or("");
        let p = &e["payload"];
        let at = crate::data::timestamp(&e["timestamp"]);
        if ty == "session_meta" {
            return;
        }
        if ty == "turn_context" {
            self.model = p["model"].as_str().unwrap_or("").into();
            if let Some(tier) = p.get("service_tier") {
                self.tier = tier.as_str().unwrap_or("").into();
            }
            self.effort = p["effort"]
                .as_str()
                .or_else(|| p["reasoning_effort"].as_str())
                .map(str::to_owned);
            self.long_write = p
                .pointer("/prompt_cache_options/ttl")
                .and_then(Value::as_str)
                == Some("12h");
        }
        if ty == "event_msg" && p["type"] == "thread_settings_applied" {
            if let Some(model) = p.pointer("/thread_settings/model").and_then(Value::as_str) {
                self.model = model.into();
            }
            if let Some(effort) = p
                .pointer("/thread_settings/reasoning_effort")
                .and_then(Value::as_str)
            {
                self.effort = Some(effort.into());
            }
            if let Some(tier) = p.pointer("/thread_settings/service_tier") {
                self.tier = tier.as_str().unwrap_or("").into();
            }
        }
        if ty == "token_usage_record" && p["thread_id"].as_str() != Some(thread) {
            return;
        }
        let kind = p["type"].as_str().unwrap_or("");
        let id = p["turn_id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .map(str::to_owned)
            .or_else(|| self.active.clone());
        let Some(id) = id else {
            if kind == "token_count" {
                if let Some(u) = Usage::from(&p["info"]["total_token_usage"]) {
                    self.legacy_previous = Some(u);
                }
            }
            return;
        };
        if matches!(kind, "task_started" | "turn_started") {
            if let Some(old) = self.active.as_ref().filter(|old| **old != id) {
                if let Some(t) = self.turns.get_mut(old) {
                    if t.status == "in_progress" {
                        t.status = "incomplete".into();
                    }
                }
            }
            self.active = Some(id.clone());
        }
        if !self.turns.contains_key(&id) {
            self.counter += 1;
            self.order.push(id.clone());
        }
        let t = self.turns.entry(id.clone()).or_insert_with(|| Turn {
            id: id.clone(),
            sequence: self.counter,
            status: "incomplete".into(),
            ..Default::default()
        });
        if matches!(kind, "task_started" | "turn_started") {
            t.started_at = p["started_at"].as_i64().or((at > 0).then_some(at));
            t.status = "in_progress".into();
        }
        if let Some(root) = p["root_turn_id"].as_str() {
            t.root_turn_id = Some(root.into());
        }
        if ty == "turn_context" {
            if t.model != self.model {
                t.fresh_context = false;
            }
            t.model = self.model.clone();
            t.effort = self.effort.clone();
        }
        if ty == "compacted" || kind == "context_compacted" {
            t.compacted = true;
            t.fresh_context = false;
        }
        if ty == "token_usage_record" {
            let key = p["response_id"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    format!(
                        "{thread}:{id}:{at}:{}:{}",
                        p["thread_token_usage"], p["usage"]
                    )
                });
            if !self.seen.insert(key.clone()) {
                return;
            }
            t.record_mode = true;
            if let Some(snapshot) = Usage::from(&p["turn_token_usage"]) {
                t.usage = Some(snapshot);
                t.usage_partial = false;
            }
            if Usage::from(&p["usage"]).is_none() {
                t.missing_response = true;
                t.usage_partial |= Usage::from(&p["turn_token_usage"]).is_none();
            }
            if let Some(mut u) = Usage::from(&p["usage"]) {
                if self.long_write {
                    u.write_long = u.write;
                }
                let model = p["model"].as_str().unwrap_or(&self.model).to_owned();
                let tier = p["service_tier"].as_str().unwrap_or(&self.tier).to_owned();
                if let Some(pos) = t
                    .responses
                    .iter()
                    .rposition(|r| r.id.starts_with("legacy:") && r.at == at && r.usage == u)
                {
                    t.responses.remove(pos);
                }
                t.responses.push(Response {
                    id: key,
                    at,
                    model: model.clone(),
                    tier,
                    long: u.input > 272_000,
                    usage: u.clone(),
                });
                if let Some(snapshot) = Usage::from(&p["turn_token_usage"]) {
                    t.usage = Some(snapshot);
                } else {
                    t.usage_partial |= t.started_at.is_none();
                    t.usage.get_or_insert_with(Usage::default).add(&u);
                }
                t.model = model;
                t.effort = self.effort.clone();
            }
        }
        if kind == "token_count" {
            if let Some(cur) = Usage::from(&p["info"]["total_token_usage"]) {
                if !t.record_mode {
                    if let Some(previous) = &self.legacy_previous {
                        if cur.total >= previous.total {
                            let mut u = cur.delta(previous);
                            if self.long_write {
                                u.write_long = u.write;
                            }
                            if u.total > 0 {
                                t.usage.get_or_insert_with(Usage::default).add(&u);
                                t.responses.push(Response {
                                    id: format!("legacy:{at}:{}", cur.total),
                                    at,
                                    model: self.model.clone(),
                                    tier: self.tier.clone(),
                                    long: p["info"]["last_token_usage"]["input_tokens"]
                                        .as_u64()
                                        .is_some_and(|n| n > 272_000),
                                    usage: u,
                                });
                            }
                        }
                    } else if t.started_at.is_some() {
                        if let Some(mut u) = Usage::from(&p["info"]["last_token_usage"]) {
                            if self.long_write {
                                u.write_long = u.write;
                            }
                            t.usage = Some(u.clone());
                            t.usage_partial = cur.total > u.total;
                            t.responses.push(Response {
                                id: format!("legacy:{at}:{}", cur.total),
                                at,
                                model: self.model.clone(),
                                tier: self.tier.clone(),
                                long: u.input > 272_000,
                                usage: u,
                            });
                        }
                    }
                }
                self.legacy_previous = Some(cur);
            }
            if p["info"].is_object() {
                t.fresh_context = p["info"]["last_token_usage"]["total_tokens"].is_u64();
            }
        }
        if matches!(
            kind,
            "task_complete" | "turn_complete" | "turn_completed" | "turn_aborted"
        ) {
            if t.started_at.is_none() {
                t.started_at = p["started_at"].as_i64();
            }
            t.status = if kind == "turn_aborted" {
                "interrupted"
            } else if p["error"].is_object() {
                "failed"
            } else {
                "completed"
            }
            .into();
            t.completed_at = p["completed_at"].as_i64().or((at > 0).then_some(at));
            t.duration_ms = p["duration_ms"]
                .as_i64()
                .or_else(|| Some((t.completed_at? - t.started_at?) * 1000))
                .filter(|n| *n >= 0);
            if t.fresh_context {
                t.context = context;
            }
            if self.active.as_ref() == Some(&id) {
                self.active = None;
            }
        }
    }
}
