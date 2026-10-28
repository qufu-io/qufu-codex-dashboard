use crate::data;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    time::Duration,
};
use std::{
    hash::{Hash, Hasher},
    io::Read,
};

#[derive(Clone, Debug)]
struct Sample {
    id: String,
    at: i64,
    group: String,
    x: [f64; 3],
}
impl Sample {
    fn tokens(&self) -> f64 {
        self.x.iter().sum()
    }
}
#[derive(Clone, Debug)]
struct Point {
    at: i64,
    minutes: i64,
    reset: i64,
    percent: f64,
    plan: Option<String>,
    limit: String,
}
#[derive(Clone)]
struct Interval {
    reset: i64,
    at: i64,
    start: i64,
    x: [f64; 3],
    groups: BTreeMap<String, [f64; 3]>,
    percent: f64,
    weight: f64,
}
#[derive(Clone)]
struct Model {
    common: [f64; 3],
    groups: BTreeMap<String, [f64; 3]>,
    regression: bool,
}
impl Model {
    fn predict(&self, groups: &BTreeMap<String, [f64; 3]>) -> f64 {
        groups
            .iter()
            .map(|(name, x)| dot(self.groups.get(name).unwrap_or(&self.common), x) / 100_000.)
            .sum()
    }
}
fn dot(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    (0..3).map(|j| a[j] * b[j]).sum()
}
fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    let n = values.len();
    if n % 2 == 0 {
        (values[n / 2 - 1] + values[n / 2]) / 2.
    } else {
        values[n / 2]
    }
}
fn intervals_with_threshold(
    samples: &[Sample],
    points: &[Point],
    minutes: i64,
    plan: Option<&str>,
    minimum: f64,
) -> Vec<Interval> {
    let mut timeline = BTreeMap::<i64, Vec<&Point>>::new();
    for p in points.iter().filter(|p| {
        p.minutes == minutes
            && p.percent.is_finite()
            && (0. ..=100.).contains(&p.percent)
            && p.at < p.reset
            && p.at >= p.reset - minutes * 60
    }) {
        timeline.entry(p.at).or_default().push(p);
    }
    let mut out = Vec::new();
    let mut anchor: Option<Point> = None;
    let mut last_observed: Option<Point> = None;
    for observations in timeline.into_values() {
        let resets: BTreeSet<_> = observations.iter().map(|p| p.reset).collect();
        let plans: BTreeSet<_> = observations
            .iter()
            .filter_map(|p| p.plan.as_deref())
            .collect();
        if resets.len() != 1 || plans.len() > 1 {
            anchor = None;
            last_observed = None;
            continue;
        }
        let p = (*observations
            .into_iter()
            .max_by(|a, b| {
                a.percent
                    .total_cmp(&b.percent)
                    .then(a.plan.is_some().cmp(&b.plan.is_some()))
            })
            .unwrap())
        .clone();
        if plan.zip(p.plan.as_deref()).is_some_and(|(a, b)| a != b) {
            anchor = None;
            last_observed = None;
            continue;
        }
        let interrupted = last_observed.as_ref().is_some_and(|last| {
            last.reset != p.reset
                || last.limit != p.limit
                || last
                    .plan
                    .as_ref()
                    .zip(p.plan.as_ref())
                    .is_some_and(|(a, b)| a != b)
                || p.percent < last.percent
        });
        last_observed = Some(p.clone());
        if interrupted {
            anchor = Some(p);
            continue;
        }
        let Some(a) = anchor.as_ref() else {
            anchor = Some(p);
            continue;
        };
        if a.reset != p.reset
            || a.plan
                .as_ref()
                .zip(p.plan.as_ref())
                .is_some_and(|(a, b)| a != b)
            || a.limit != p.limit
            || p.percent < a.percent
            || p.percent >= 99.
            || a.percent >= 99.
        {
            anchor = Some(p);
            continue;
        }
        if p.percent - a.percent < minimum || p.at - a.at < 30 {
            continue;
        }
        let lo = samples.partition_point(|s| s.at <= a.at);
        let hi = samples.partition_point(|s| s.at <= p.at);
        let mut x = [0.; 3];
        let mut groups = BTreeMap::<String, [f64; 3]>::new();
        for sample in &samples[lo..hi] {
            let group = groups.entry(sample.group.clone()).or_default();
            for j in 0..3 {
                x[j] += sample.x[j];
                group[j] += sample.x[j];
            }
        }
        if x.iter().sum::<f64>() > 0. {
            out.push(Interval {
                reset: p.reset,
                at: p.at,
                start: a.at,
                x,
                groups,
                percent: p.percent - a.percent,
                weight: if a.plan.is_some() && p.plan.is_some() {
                    1.
                } else {
                    0.25
                },
            });
        }
        anchor = Some(p);
    }
    out.sort_by_key(|v| v.at);
    out
}
fn intervals(
    samples: &[Sample],
    points: &[Point],
    minutes: i64,
    plan: Option<&str>,
) -> Vec<Interval> {
    let mut rows = intervals_with_threshold(samples, points, minutes, plan, 3.);
    if rows.len() > 240 {
        rows.drain(..rows.len() - 240);
    }
    rows
}
fn fit_coefficients(rows: &[&Interval], regression: bool) -> [f64; 3] {
    let mut ratios: Vec<_> = rows
        .iter()
        .map(|r| r.percent * 100_000. / r.x.iter().sum::<f64>())
        .collect();
    let base = median(&mut ratios);
    let mut beta = [base; 3];
    if !regression {
        return beta;
    }
    let mut weights: Vec<_> = rows.iter().map(|r| r.weight).collect();
    for _ in 0..6 {
        for _ in 0..500 {
            let old = beta;
            for j in 0..3 {
                let mut numerator = 0.0001 * base;
                let mut denominator = 0.0001;
                for (r, w) in rows.iter().zip(&weights) {
                    let x = r.x.map(|v| v / 100_000.);
                    let residual = r.percent - dot(&beta, &x) + beta[j] * x[j];
                    numerator += w * x[j] * residual;
                    denominator += w * x[j] * x[j];
                }
                beta[j] = (numerator / denominator).clamp(0., base * 50.);
            }
            if (0..3).map(|j| (beta[j] - old[j]).abs()).sum::<f64>() < 1e-8 {
                break;
            }
        }
        let mut residuals: Vec<_> = rows
            .iter()
            .map(|r| r.percent - dot(&beta, &r.x) / 100_000.)
            .collect();
        let center = median(&mut residuals);
        let mut deviations: Vec<_> = residuals.iter().map(|v| (v - center).abs()).collect();
        let cutoff = (1.4826 * median(&mut deviations) * 1.5).max(0.5);
        for (i, r) in rows.iter().enumerate() {
            let error = (r.percent - dot(&beta, &r.x) / 100_000.).abs();
            weights[i] = r.weight * (cutoff / error.max(cutoff));
        }
    }
    beta
}
fn fit(rows: &[&Interval]) -> Model {
    let regression = rows.len() >= 12
        && rows.iter().map(|r| r.reset).collect::<BTreeSet<_>>().len() >= 3
        && identifiable(rows);
    let m = Model {
        common: fit_coefficients(rows, regression),
        groups: BTreeMap::new(),
        regression,
    };
    m
}
fn validation(rows: &[Interval]) -> (Option<f64>, Option<f64>, usize) {
    let resets: BTreeSet<_> = rows.iter().map(|r| r.reset).collect();
    let mut errors = Vec::new();
    for reset in resets.into_iter().skip(2) {
        let start = rows
            .iter()
            .filter(|r| r.reset == reset)
            .map(|r| r.start)
            .min()
            .unwrap();
        let train: Vec<_> = rows
            .iter()
            .filter(|r| r.reset != reset && r.at + 30 < start)
            .collect();
        if train.len() < 4 {
            continue;
        }
        let model = fit(&train);
        for r in rows.iter().filter(|r| r.reset == reset) {
            errors.push((model.predict(&r.groups) - r.percent).abs() / r.percent * 100.);
        }
    }
    if errors.len() < 4 {
        return (None, None, errors.len());
    }
    let typical = median(&mut errors);
    let p90 = errors[((errors.len() - 1) as f64 * 0.9).ceil() as usize];
    (Some(typical), Some(p90), errors.len())
}
fn identifiable(rows: &[&Interval]) -> bool {
    let mut g = [[0.; 3]; 3];
    for r in rows {
        for j in 0..3 {
            for k in 0..3 {
                g[j][k] += r.x[j] * r.x[k];
            }
        }
    }
    if (0..3).any(|j| g[j][j] <= 0.) {
        return false;
    }
    let d = g[0][0] * g[1][1] * g[2][2] + 2. * g[0][1] * g[1][2] * g[0][2]
        - g[0][0] * g[1][2].powi(2)
        - g[1][1] * g[0][2].powi(2)
        - g[2][2] * g[0][1].powi(2);
    d / (g[0][0] * g[1][1] * g[2][2]) > 0.001
}
fn estimate(
    samples: &[Sample],
    points: &[Point],
    window: &Value,
    plan: Option<&str>,
    at: i64,
) -> Value {
    let (Some(minutes), Some(reset), Some(percent)) = (
        window["window_minutes"].as_i64(),
        window["resets_at"].as_i64(),
        window["used_percent"].as_f64(),
    ) else {
        return Value::Null;
    };
    if ![300, 10080].contains(&minutes)
        || !percent.is_finite()
        || !(0. ..=100.).contains(&percent)
        || at >= reset
        || at < reset - minutes * 60
    {
        return Value::Null;
    }
    let group = points
        .iter()
        .filter(|p| {
            p.at <= at
                && p.minutes == minutes
                && plan.zip(p.plan.as_deref()).is_none_or(|(a, b)| a == b)
        })
        .max_by_key(|p| p.at)
        .map(|p| p.limit.as_str())
        .unwrap_or("");
    let past: Vec<_> = points
        .iter()
        .filter(|p| p.at <= at && p.limit == group)
        .cloned()
        .collect();
    let all_rows = intervals_with_threshold(samples, &past, minutes, plan, 3.);
    let rows: Vec<_> = all_rows
        .iter()
        .rev()
        .take(240)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let mut local = None;
    for s in samples
        .iter()
        .filter(|s| s.at > reset - minutes * 60 && s.at <= at)
    {
        *local.get_or_insert(0.) += s.tokens();
    }
    let mut value = json!({"used":null,"capacity":null,"localTokens":local,"percent":percent,"reset":reset,"approximate":true,"intervals":rows.len(),"resets":rows.iter().map(|r|r.reset).collect::<BTreeSet<_>>().len(),"medianErrorPercent":null,"p90ErrorPercent":null,"validationCount":0,"reliability":"insufficient","method":"initial-ratio","priceIndependent":true,"reference":"uncached-input-equivalent","weights":null,"calibrationMode":"independent-v1","windowMinutes":minutes});
    if rows.is_empty() {
        return value;
    }
    let model = fit(&rows.iter().collect::<Vec<_>>());
    let (typical, p90, count) = validation(&rows);
    value["medianErrorPercent"] = json!(typical);
    value["p90ErrorPercent"] = json!(p90);
    value["validationCount"] = json!(count);
    value["weights"] = json!(model.common.map(|c| c / 100_000.));
    value["identifiable"] = json!(model.regression);
    let mut model_mix = BTreeMap::<String, f64>::new();
    for sample in samples
        .iter()
        .filter(|s| s.at > reset - minutes * 60 && s.at <= at)
    {
        *model_mix.entry(sample.group.clone()).or_default() += sample.tokens();
    }
    let total = model_mix.values().sum::<f64>();
    if total > 0. {
        for n in model_mix.values_mut() {
            *n /= total;
        }
    }
    value["modelMix"] = json!(model_mix);
    if model.regression {
        let mut ranges = [[f64::INFINITY, f64::NEG_INFINITY]; 3];
        for reset in rows.iter().map(|r| r.reset).collect::<BTreeSet<_>>() {
            let training: Vec<_> = rows.iter().filter(|r| r.reset != reset).collect();
            if training.len() < 12 || !identifiable(&training) {
                continue;
            }
            let coefficients = fit_coefficients(&training, true);
            for j in 0..3 {
                ranges[j][0] = ranges[j][0].min(coefficients[j] / 100000.);
                ranges[j][1] = ranges[j][1].max(coefficients[j] / 100000.);
            }
        }
        if ranges.iter().all(|r| r[0].is_finite() && r[1].is_finite()) {
            value["leaveOneWindowRanges"] = json!(ranges);
        }
    }
    value["method"] = json!(if model.regression {
        "robust-ridge-independent"
    } else {
        "initial-ratio"
    });
    if typical.is_some_and(|v| v > 35.) || p90.is_some_and(|v| v > 100.) {
        value["reliability"] = json!("unstable");
        return value;
    }
    if model.common[0] <= 0. {
        return value;
    }
    let capacity = 100. * 100_000. / model.common[0];
    if !capacity.is_finite() || !(1. ..=1e13).contains(&capacity) {
        return value;
    }
    value["capacity"] = json!(capacity);
    value["used"] = json!(capacity * percent / 100.);
    value["reliability"] = json!(if model.regression && count >= 8 {
        "validated"
    } else {
        "learning"
    });
    value
}
fn capacity_change(previous: &Value, current: &Value) -> Value {
    let Some((a, b)) = previous["capacity"]
        .as_f64()
        .zip(current["capacity"].as_f64())
        .filter(|(a, b)| *a > 0. && *b > 0.)
    else {
        return json!({"status":"insufficient","backendChangeVerified":false});
    };
    let delta = (b / a - 1.) * 100.;
    let error = previous["p90ErrorPercent"]
        .as_f64()
        .zip(current["p90ErrorPercent"].as_f64())
        .map(|(a, b)| a.max(b) * 2.);
    let weights_changed = (1..3).any(|j| {
        let norm = |v: &Value| {
            v["weights"][j]
                .as_f64()
                .zip(v["weights"][0].as_f64())
                .filter(|(_, n)| *n > 0.)
                .map(|(v, n)| v / n)
        };
        norm(previous)
            .zip(norm(current))
            .is_none_or(|(a, b)| (b - a).abs() > 0.15 * a.abs().max(0.01))
    });
    let mix_changed = previous["modelMix"] != current["modelMix"];
    let status = if !previous["identifiable"].as_bool().unwrap_or(false)
        || !current["identifiable"].as_bool().unwrap_or(false)
        || error.is_none()
    {
        "insufficient"
    } else if previous["calibrationMode"] != current["calibrationMode"] {
        "calibration_changed"
    } else if weights_changed {
        "weights_relearned"
    } else if mix_changed {
        "usage_mix_changed"
    } else if delta.abs() > error.unwrap().max(20.) {
        "estimated_capacity_change"
    } else {
        "within_model_error"
    };
    json!({"status":status,"relativeChangePercent":delta,"empiricalErrorThresholdPercent":error,"backendChangeVerified":false})
}
fn fingerprint(path: &Path, length: u64) -> Option<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(length)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 != length {
        return None;
    }
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hash);
    Some(format!("{:x}", hash.finish()))
}
struct Store {
    connection: Connection,
}
impl Store {
    fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let c = Connection::open(path).map_err(|e| e.to_string())?;
        c.busy_timeout(Duration::from_secs(2))
            .map_err(|e| e.to_string())?;
        c.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS cursors(thread TEXT PRIMARY KEY,state TEXT NOT NULL,head_length INTEGER NOT NULL,fingerprint TEXT NOT NULL); CREATE TABLE IF NOT EXISTS usage(id TEXT PRIMARY KEY,thread TEXT NOT NULL,at INTEGER NOT NULL,model TEXT NOT NULL,tier TEXT NOT NULL,noncached REAL NOT NULL,cached REAL NOT NULL,output REAL NOT NULL); CREATE INDEX IF NOT EXISTS usage_at ON usage(at); CREATE INDEX IF NOT EXISTS usage_thread ON usage(thread); CREATE TABLE IF NOT EXISTS points(at INTEGER NOT NULL,minutes INTEGER NOT NULL,reset INTEGER NOT NULL,percent REAL NOT NULL,plan TEXT NOT NULL,PRIMARY KEY(at,minutes,reset,plan));").map_err(|e|e.to_string())?;
        let version: i64 = c
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if version < 4 {
            if c.query_row("SELECT COUNT(*) FROM usage", [], |r| r.get::<_, i64>(0))
                .unwrap_or(0)
                > 0
            {
                let backup = path.with_extension(format!(
                    "pre-v4-{}.sqlite3",
                    chrono::Utc::now().timestamp_millis()
                ));
                c.execute("VACUUM INTO ?1", [backup.to_string_lossy().as_ref()])
                    .map_err(|e| e.to_string())?;
            }
            c.execute_batch("BEGIN IMMEDIATE;
                CREATE TABLE IF NOT EXISTS quota_points(at INTEGER NOT NULL,minutes INTEGER NOT NULL,reset INTEGER NOT NULL,percent REAL NOT NULL,plan TEXT NOT NULL,limit_id TEXT NOT NULL,PRIMARY KEY(at,minutes,reset,plan,limit_id));
                INSERT OR IGNORE INTO quota_points SELECT at,minutes,reset,percent,plan,'' FROM points;
                CREATE TABLE IF NOT EXISTS usage_superseded(id TEXT PRIMARY KEY,by_id TEXT NOT NULL);
                CREATE TABLE IF NOT EXISTS usage_metadata(id TEXT PRIMARY KEY,turn_id TEXT,root_turn_id TEXT,effort TEXT,raw_model TEXT NOT NULL);
                CREATE TABLE IF NOT EXISTS turn_measurements(thread TEXT NOT NULL,id TEXT NOT NULL,data TEXT NOT NULL,updated INTEGER NOT NULL,PRIMARY KEY(thread,id));
                CREATE TABLE IF NOT EXISTS calibration_history(at INTEGER NOT NULL,minutes INTEGER NOT NULL,plan TEXT NOT NULL,limit_id TEXT NOT NULL,data TEXT NOT NULL,PRIMARY KEY(at,minutes,plan,limit_id));
                CREATE TABLE IF NOT EXISTS observation_intervals(start INTEGER NOT NULL,at INTEGER NOT NULL,reset INTEGER NOT NULL,minutes INTEGER NOT NULL,plan TEXT NOT NULL,limit_id TEXT NOT NULL,data TEXT NOT NULL,PRIMARY KEY(start,at,reset,minutes,plan,limit_id));
                CREATE TABLE IF NOT EXISTS interval_diagnostics(start INTEGER NOT NULL,at INTEGER NOT NULL,minutes INTEGER NOT NULL,reason TEXT NOT NULL,data TEXT NOT NULL,PRIMARY KEY(start,at,minutes,reason));
                PRAGMA user_version=4; COMMIT;").map_err(|e|e.to_string())?;
        }
        Ok(Self { connection: c })
    }
    fn point(&self, at: i64, q: &Value) -> Result<(), String> {
        for key in ["primary", "secondary"] {
            let w = &q[key];
            if let (Some(m), Some(r), Some(p)) = (
                w["window_minutes"].as_i64(),
                w["resets_at"].as_i64(),
                w["used_percent"].as_f64(),
            ) {
                if [300, 10080].contains(&m)
                    && p.is_finite()
                    && (0. ..=100.).contains(&p)
                    && at > 0
                    && r > at
                {
                    self.connection
                        .execute(
                            "INSERT OR IGNORE INTO points VALUES(?1,?2,?3,?4,?5)",
                            params![at, m, r, p, q["plan_type"].as_str().unwrap_or("")],
                        )
                        .map_err(|e| e.to_string())?;
                    self.connection
                        .execute(
                            "INSERT OR IGNORE INTO quota_points VALUES(?1,?2,?3,?4,?5,?6)",
                            params![
                                at,
                                m,
                                r,
                                p,
                                q["plan_type"].as_str().unwrap_or(""),
                                q["limit_id"].as_str().unwrap_or("")
                            ],
                        )
                        .map_err(|e| e.to_string())?;
                }
            }
        }
        Ok(())
    }
    fn collect(
        &mut self,
        reader: &mut data::Reader,
        quota: &Value,
    ) -> Result<Observations, String> {
        let (threads, _) = reader.catalog()?;
        let transaction = self.connection.transaction().map_err(|e| e.to_string())?;
        let mut historical = Vec::new();
        let mut unreadable = 0;
        for thread in threads {
            let path = reader.rollout_path(&thread);
            if !reader.cache.contains_key(&thread.id) {
                if let Ok(Some((state, length, head))) = transaction
                    .query_row(
                        "SELECT state,head_length,fingerprint FROM cursors WHERE thread=?1",
                        [&thread.id],
                        |r| {
                            Ok((
                                r.get::<_, String>(0)?,
                                r.get::<_, i64>(1)?,
                                r.get::<_, String>(2)?,
                            ))
                        },
                    )
                    .optional()
                {
                    if (0..=4096).contains(&length)
                        && fingerprint(&path, length as u64).as_ref() == Some(&head)
                    {
                        if let Ok(state) = serde_json::from_str::<Value>(&state) {
                            if state["cursorVersion"] == 2 {
                                reader.cache.insert(
                                    thread.id.clone(),
                                    data::Rollout::restore_cursor(&state),
                                );
                            }
                        }
                    }
                }
            }
            let read_ok = reader.read_thread(&thread).is_ok();
            if !read_ok {
                unreadable += 1;
            }
            let Some(r) = reader.cache.get_mut(&thread.id) else {
                continue;
            };
            for s in &r.window_samples {
                if s.usage.cache_missing || s.usage.cache > s.usage.input || s.at <= 0 {
                    continue;
                }
                transaction
                    .execute(
                        "INSERT OR IGNORE INTO usage VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                        params![
                            s.id,
                            thread.id,
                            s.at,
                            crate::pricing::normalize_model(&s.model),
                            crate::pricing::normalize_tier(&s.tier),
                            (s.usage.input - s.usage.cache) as f64,
                            s.usage.cache as f64,
                            s.usage.output as f64
                        ],
                    )
                    .map_err(|e| e.to_string())?;
            }
            for sample in r
                .window_samples
                .iter()
                .filter(|s| s.id.starts_with("response:"))
            {
                transaction.execute("INSERT OR IGNORE INTO usage_superseded SELECT id,?1 FROM usage WHERE thread=?2 AND id LIKE 'legacy:%' AND at=?3 AND noncached=?4 AND cached=?5 AND output=?6 AND id NOT IN (SELECT id FROM usage_superseded) AND NOT EXISTS (SELECT 1 FROM usage_superseded WHERE by_id=?1) ORDER BY id LIMIT 1",params![sample.id,thread.id,sample.at,sample.usage.input.saturating_sub(sample.usage.cache) as f64,sample.usage.cache as f64,sample.usage.output as f64]).map_err(|e|e.to_string())?;
            }
            for turn in r.turns.turns.values() {
                transaction.execute("INSERT INTO turn_measurements VALUES(?1,?2,?3,?4) ON CONFLICT(thread,id) DO UPDATE SET data=excluded.data,updated=excluded.updated WHERE turn_measurements.data!=excluded.data",params![thread.id,turn.id,serde_json::to_string(turn).map_err(|e|e.to_string())?,chrono::Utc::now().timestamp()]).map_err(|e|e.to_string())?;
            }
            for sample in &r.window_samples {
                transaction
                    .execute(
                        "INSERT OR IGNORE INTO usage_metadata VALUES(?1,?2,?3,?4,?5)",
                        params![
                            sample.id,
                            sample.turn_id,
                            sample.root_turn_id,
                            sample.effort,
                            sample.model
                        ],
                    )
                    .map_err(|e| e.to_string())?;
            }
            historical.extend(r.quota_history.iter().cloned());
            r.turns.compact_scanner_state();
            if read_ok {
                let length = r.offset.min(4096);
                if let Some(head) = fingerprint(&path, length) {
                    transaction
                        .execute(
                            "INSERT OR REPLACE INTO cursors VALUES(?1,?2,?3,?4)",
                            params![thread.id, r.cursor().to_string(), length as i64, head],
                        )
                        .map_err(|e| e.to_string())?;
                }
            }
        }
        transaction.commit().map_err(|e| e.to_string())?;
        for (at, q) in historical {
            self.point(at, &q)?;
        }
        if !quota["stale"].as_bool().unwrap_or(true) {
            if let Some(at) = quota["at"].as_i64() {
                self.point(at, &quota["value"])?;
            }
        }
        for r in reader.cache.values_mut() {
            r.window_samples.clear();
            r.quota_history.clear();
        }
        let data = self.load(unreadable)?;
        self.save_calibration(&data, quota)?;
        Ok(data)
    }
    fn save_calibration(&self, data: &Observations, quota: &Value) -> Result<(), String> {
        let plan = quota["value"]["plan_type"].as_str();
        let at = quota["at"].as_i64().unwrap_or(0);
        if at > 0 && !quota["stale"].as_bool().unwrap_or(true) {
            for key in ["primary", "secondary"] {
                let w = &quota["value"][key];
                let mut v = estimate(&data.samples, &data.points, w, plan, at);
                if !v.is_object() {
                    continue;
                }
                let previous:Option<String>=self.connection.query_row("SELECT data FROM calibration_history WHERE minutes=?1 AND plan=?2 AND limit_id=?3 AND at<?4 ORDER BY at DESC LIMIT 1",params![w["window_minutes"].as_i64(),plan.unwrap_or(""),quota["value"]["limit_id"].as_str().unwrap_or(""),at],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
                if let Some(previous) =
                    previous.and_then(|v| serde_json::from_str::<Value>(&v).ok())
                {
                    v["changeAnalysis"] = capacity_change(&previous, &v);
                }
                if !v.is_object() {
                    continue;
                }
                v["localComplete"] = json!(data.unreadable == 0);
                if data.unreadable > 0 && v["reliability"] == "validated" {
                    v["reliability"] = json!("limited");
                }
                self.connection
                    .execute(
                        "INSERT OR IGNORE INTO calibration_history VALUES(?1,?2,?3,?4,?5)",
                        params![
                            at,
                            w["window_minutes"].as_i64(),
                            plan.unwrap_or(""),
                            quota["value"]["limit_id"].as_str().unwrap_or(""),
                            v.to_string()
                        ],
                    )
                    .map_err(|e| e.to_string())?;
            }
        }
        for minutes in [300, 10080] {
            let identity = data
                .points
                .iter()
                .filter(|p| p.minutes == minutes && (at == 0 || p.at <= at))
                .max_by_key(|p| p.at)
                .map(|p| p.limit.as_str())
                .unwrap_or("");
            let same_group: Vec<_> = data
                .points
                .iter()
                .filter(|p| p.limit == identity && (at == 0 || p.at <= at))
                .cloned()
                .collect();
            let rows = intervals(&data.samples, &same_group, minutes, plan);
            if rows.is_empty() {
                continue;
            }
            let model = fit(&rows.iter().collect::<Vec<_>>());
            for row in &rows {
                let predicted = model.predict(&row.groups);
                let residual = predicted - row.percent;
                let evidence = json!({"noncached":row.x[0],"cached":row.x[1],"output":row.x[2],"groups":row.groups,"deltaPercent":row.percent,"predictedDeltaPercent":predicted,"residualPercentagePoints":residual,"roundingUncertaintyPercentagePoints":1.,"quality":if residual.abs()>row.percent*0.35+1. {"suspect_unobserved_usage_or_model_change"}else {"usable"},"weight":row.weight,"calibrationMode":"independent-v1"});
                self.connection
                    .execute(
                        "INSERT OR IGNORE INTO observation_intervals VALUES(?1,?2,?3,?4,?5,?6,?7)",
                        params![
                            row.start,
                            row.at,
                            row.reset,
                            minutes,
                            plan.unwrap_or(""),
                            identity,
                            evidence.to_string()
                        ],
                    )
                    .map_err(|e| e.to_string())?;
            }
        }
        let mut prefix = vec![0.];
        for sample in &data.samples {
            prefix.push(prefix.last().copied().unwrap_or(0.) + sample.tokens());
        }
        for minutes in [300, 10080] {
            let timeline: Vec<_> = data
                .points
                .iter()
                .filter(|p| p.minutes == minutes)
                .collect();
            for pair in timeline.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let lo = data.samples.partition_point(|s| s.at <= a.at);
                let hi = data.samples.partition_point(|s| s.at <= b.at);
                let n = prefix[hi] - prefix[lo];
                let reason = if a.reset != b.reset {
                    "reset_or_window_shift"
                } else if a.plan != b.plan {
                    "plan_change"
                } else if a.limit != b.limit {
                    "limit_group_change"
                } else if b.percent < a.percent {
                    "expired_usage"
                } else if b.percent >= 99. {
                    "saturation"
                } else if b.percent > a.percent && n == 0. {
                    "unobserved_account_usage"
                } else if b.percent - a.percent < 3. {
                    "rounding_or_insufficient_delta"
                } else {
                    "candidate"
                };
                self.connection
                    .execute(
                        "INSERT OR IGNORE INTO interval_diagnostics VALUES(?1,?2,?3,?4,?5)",
                        params![
                            a.at,
                            b.at,
                            minutes,
                            reason,
                            json!({"localTokens":n,"deltaPercent":b.percent-a.percent}).to_string()
                        ],
                    )
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
    fn load(&self, unreadable: usize) -> Result<Observations, String> {
        let mut query = self
            .connection
            .prepare("SELECT u.id,u.at,COALESCE(m.raw_model,u.model),u.tier,u.noncached,u.cached,u.output,COALESCE(m.effort,'') FROM usage u LEFT JOIN usage_metadata m ON m.id=u.id WHERE u.id NOT IN (SELECT id FROM usage_superseded) ORDER BY u.at,u.id")
            .map_err(|e| e.to_string())?;
        let samples: Vec<Sample> = query
            .query_map([], |r| {
                Ok(Sample {
                    id: r.get(0)?,
                    at: r.get(1)?,
                    group: format!(
                        "{}|{}|{}",
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(7)?
                    ),
                    x: [r.get(4)?, r.get(5)?, r.get(6)?],
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        let mut unique = BTreeSet::new();
        let samples = samples
            .into_iter()
            .filter(|s| unique.insert(s.id.clone()))
            .collect();
        let mut query = self
            .connection
            .prepare("SELECT at,minutes,reset,percent,plan,limit_id FROM quota_points ORDER BY at")
            .map_err(|e| e.to_string())?;
        let points = query
            .query_map([], |r| {
                let plan: String = r.get(4)?;
                Ok(Point {
                    at: r.get(0)?,
                    minutes: r.get(1)?,
                    reset: r.get(2)?,
                    percent: r.get(3)?,
                    plan: if plan.is_empty() { None } else { Some(plan) },
                    limit: r.get(5)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(Observations {
            samples,
            points,
            unreadable,
        })
    }
}
struct Observations {
    samples: Vec<Sample>,
    points: Vec<Point>,
    unreadable: usize,
}
pub struct Estimator {
    directory: PathBuf,
    jobs: Sender<Value>,
    incoming: Receiver<Result<Observations, String>>,
    scanning: bool,
    last_scan: i64,
    samples: Vec<Sample>,
    points: Vec<Point>,
    calibration_at: i64,
    cached: Value,
    error: Option<String>,
    unreadable: usize,
}
impl Default for Estimator {
    fn default() -> Self {
        let directory = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::var_os("XDG_DATA_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| {
                        PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                            .join(".local/share")
                    })
            })
            .join("com.qufu.codexdashboard");
        Self::new(directory)
    }
}
impl Estimator {
    pub fn new(directory: PathBuf) -> Self {
        let (jobs, requests) = mpsc::channel::<Value>();
        let (outputs, incoming) = mpsc::channel();
        let worker_directory = directory.clone();
        std::thread::spawn(move || {
            let directory = worker_directory;
            let mut reader = data::Reader::new();
            let mut store = Store::open(&directory.join("quota-learning.sqlite3"));
            while let Ok(quota) = requests.recv() {
                let result = match &mut store {
                    Ok(s) => s.collect(&mut reader, &quota),
                    Err(e) => Err(e.clone()),
                };
                if outputs.send(result).is_err() {
                    break;
                }
            }
        });
        Self {
            directory,
            jobs,
            incoming,
            scanning: false,
            last_scan: 0,
            samples: Vec::new(),
            points: Vec::new(),
            calibration_at: 0,
            cached: Value::Null,
            error: None,
            unreadable: 0,
        }
    }
    fn accept(&mut self, result: Result<Observations, String>) {
        match result {
            Ok(v) => {
                self.samples = v.samples;
                self.points = v.points;
                self.unreadable = v.unreadable;
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
        self.scanning = false;
        self.calibration_at = 0;
    }
    pub fn history(&self, metric: &str, selected: Option<&str>) -> Result<Value, String> {
        let c = Connection::open_with_flags(
            self.directory.join("quota-learning.sqlite3"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(|e| e.to_string())?;
        if matches!(metric, "turns" | "context") {
            let Some(thread) = selected else {
                return Ok(json!([]));
            };
            let mut q = c
                .prepare("SELECT data FROM turn_measurements WHERE thread=?1 ORDER BY updated,id")
                .map_err(|e| e.to_string())?;
            let rows = q
                .query_map([thread], |r| r.get::<_, String>(0))
                .map_err(|e| e.to_string())?;
            let mut turns = Vec::new();
            for row in rows {
                if let Ok(turn) =
                    serde_json::from_str::<crate::turns::Turn>(&row.map_err(|e| e.to_string())?)
                {
                    turns.push(turn);
                }
            }
            turns.sort_by_key(|t| (t.started_at.or(t.completed_at), t.sequence));
            return Ok(json!(turns
                .iter()
                .map(crate::turns::Turn::value)
                .collect::<Vec<_>>()));
        }
        let minutes = match metric {
            "five" => 300,
            "week" => 10080,
            _ => return Err("Unknown history metric".into()),
        };
        let mut q=c.prepare("SELECT at,plan,limit_id,data FROM calibration_history WHERE minutes=?1 ORDER BY at").map_err(|e|e.to_string())?;
        let rows = q
            .query_map([minutes], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        let mut fits = Vec::new();
        for row in rows {
            let (at, plan, limit, data) = row.map_err(|e| e.to_string())?;
            if let Ok(mut v) = serde_json::from_str::<Value>(&data) {
                v["at"] = json!(at);
                v["plan"] = json!(plan);
                v["limitId"] = json!(limit);
                fits.push(v);
            }
        }
        let mut q=c.prepare("SELECT at,reset,percent,plan,limit_id FROM quota_points WHERE minutes=?1 ORDER BY at").map_err(|e|e.to_string())?;
        let rows=q.query_map([minutes],|r|Ok(json!({"at":r.get::<_,i64>(0)?,"reset":r.get::<_,i64>(1)?,"percent":r.get::<_,f64>(2)?,"plan":r.get::<_,String>(3)?,"limitId":r.get::<_,String>(4)?}))).map_err(|e|e.to_string())?;
        let points = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(
            json!({"fits":fits,"points":points,"changeNotice":"Estimated capacity changes; backend quota changes cannot be isolated from usage/model calibration."}),
        )
    }
    pub fn update_blocking(&mut self, quota: &Value) -> Value {
        self.update(quota);
        if self.scanning {
            if let Ok(v) = self.incoming.recv_timeout(Duration::from_secs(120)) {
                self.accept(v);
            }
        }
        self.update(quota)
    }
    pub fn diagnostics(&self, quota: &Value) -> Value {
        json!({"samples":self.samples.len(),"points":self.points.len(),"unreadable":self.unreadable,"error":self.error,"primary":estimate(&self.samples,&self.points,&quota["value"]["primary"],quota["value"]["plan_type"].as_str(),quota["at"].as_i64().unwrap_or(0)),"secondary":estimate(&self.samples,&self.points,&quota["value"]["secondary"],quota["value"]["plan_type"].as_str(),quota["at"].as_i64().unwrap_or(0))})
    }
    pub fn update(&mut self, quota: &Value) -> Value {
        let now = chrono::Utc::now().timestamp();
        if let Ok(v) = self.incoming.try_recv() {
            self.accept(v);
        }
        if now - self.last_scan >= 30 && !self.scanning {
            self.last_scan = now;
            self.scanning = self.jobs.send(quota.clone()).is_ok();
        }
        if now - self.calibration_at >= 30 || self.cached["quotaAt"] != quota["at"] {
            self.calibration_at = now;
            let v = &quota["value"];
            let at = quota["at"].as_i64().unwrap_or(0);
            let plan = v["plan_type"].as_str();
            self.cached = json!({"quotaAt":quota["at"],"primary":estimate(&self.samples,&self.points,&v["primary"],plan,at),"secondary":estimate(&self.samples,&self.points,&v["secondary"],plan,at)});
            for key in ["primary", "secondary"] {
                if self.cached[key].is_object() {
                    self.cached[key]["localComplete"] = json!(
                        self.unreadable == 0 && self.error.is_none() && !self.samples.is_empty()
                    );
                    if self.cached[key]["reliability"] == "validated"
                        && (self.unreadable > 0
                            || self.cached[key]["medianErrorPercent"]
                                .as_f64()
                                .is_some_and(|v| v > 20.)
                            || self.cached[key]["p90ErrorPercent"]
                                .as_f64()
                                .is_some_and(|v| v > 50.))
                    {
                        self.cached[key]["reliability"] = json!("limited");
                    }
                }
            }
        }
        self.cached.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(minutes: i64, multiplier: f64) -> (Vec<Sample>, Vec<Point>, Value, i64) {
        let mut samples = Vec::new();
        let mut points = Vec::new();
        let mut last_at = 0;
        let mut window = Value::Null;
        for w in 0..7 {
            let start = 1_700_000_000 + w * minutes * 60;
            let reset = start + minutes * 60;
            let mut percent = 0.;
            points.push(Point {
                at: start + 1,
                minutes,
                reset,
                percent,
                plan: Some("pro".into()),
                limit: "codex".into(),
            });
            for j in 0..6 {
                let i = w * 6 + j;
                let at = start + 60 + (j + 1) * 600;
                let x = [
                    (10_000 + (i % 3) * 7_000) as f64 * multiplier,
                    (30_000 + (i * 3 % 7) * 5_000) as f64 * multiplier,
                    (10_000 + (i * 5 % 4) * 4_000) as f64 * multiplier,
                ];
                let coefficients = [
                    0.00004 / multiplier,
                    0.000004 / multiplier,
                    0.0002 / multiplier,
                ];
                percent += dot(&coefficients, &x);
                samples.push(Sample {
                    id: format!("response:{minutes}:{i}"),
                    at,
                    group: "unknown-model|standard".into(),
                    x,
                });
                points.push(Point {
                    at,
                    minutes,
                    reset,
                    percent,
                    plan: Some("pro".into()),
                    limit: "codex".into(),
                });
                last_at = at;
            }
            window = json!({"window_minutes":minutes,"resets_at":reset,"used_percent":percent});
        }
        (samples, points, window, last_at)
    }
    #[test]
    fn independent_weekly_fit_and_legacy_history_survive_restart() {
        let path = std::env::temp_dir().join(format!(
            "qufu-independent-{}.sqlite3",
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let (samples, points, window, at) = fixture(10080, 4.);
        let data = Observations {
            samples,
            points,
            unreadable: 0,
        };
        let store = Store::open(&path).unwrap();
        let legacy = json!({"capacity":1000000.,"weeklyRatio":0.15});
        store
            .connection
            .execute(
                "INSERT INTO calibration_history VALUES(?1,10080,'pro','codex',?2)",
                params![at - 1, legacy.to_string()],
            )
            .unwrap();
        store.save_calibration(&data,&json!({"at":at,"stale":false,"value":{"plan_type":"pro","limit_id":"codex","secondary":window}})).unwrap();
        let count: i64 = store
            .connection
            .query_row(
                "SELECT COUNT(*) FROM observation_intervals WHERE minutes=10080",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(count > 0);
        let count: i64 = store
            .connection
            .query_row(
                "SELECT COUNT(*) FROM observation_intervals WHERE minutes=300",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
        drop(store);
        let reopened = Store::open(&path).unwrap();
        let saved: String = reopened
            .connection
            .query_row(
                "SELECT data FROM calibration_history WHERE at=?1",
                [at],
                |r| r.get(0),
            )
            .unwrap();
        let saved: Value = serde_json::from_str(&saved).unwrap();
        assert_eq!(saved["calibrationMode"], "independent-v1");
        assert_eq!(saved["reliability"], "validated");
        let prior: String = reopened
            .connection
            .query_row(
                "SELECT data FROM calibration_history WHERE at=?1",
                [at - 1],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(serde_json::from_str::<Value>(&prior).unwrap(), legacy);
        drop(reopened);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn weekly_calibration_is_independent_of_hourly_points() {
        let (samples, points, window, at) = fixture(10080, 4.);
        let weekly = estimate(&samples, &points, &window, Some("pro"), at);
        assert_eq!(weekly["reliability"], "validated", "{weekly}");
        assert!(weekly["capacity"].as_f64().unwrap() > 0.);
        for (j, expected) in [0.00001, 0.000001, 0.00005].into_iter().enumerate() {
            assert!(
                (weekly["weights"][j].as_f64().unwrap() / expected - 1.).abs() < 0.2,
                "{weekly}"
            );
        }
        let mut mixed = points.clone();
        mixed.extend(points.iter().map(|p| Point {
            minutes: 300,
            reset: p.at + 18000,
            percent: 99.,
            limit: "different-hourly-group".into(),
            ..p.clone()
        }));
        let unchanged = estimate(&samples, &mixed, &window, Some("pro"), at);
        assert_eq!(weekly, unchanged);
        assert!(weekly.get("weeklyRatio").is_none());
    }
    #[test]
    fn initial_ratio_has_a_fixed_reference_and_missing_own_window_stays_unknown() {
        let samples = vec![Sample {
            id: "r".into(),
            at: 100,
            group: "no-api-rate|standard".into(),
            x: [10000., 20000., 10000.],
        }];
        let points = vec![
            Point {
                at: 1,
                minutes: 300,
                reset: 18000,
                percent: 0.,
                plan: Some("pro".into()),
                limit: "codex".into(),
            },
            Point {
                at: 100,
                minutes: 300,
                reset: 18000,
                percent: 5.,
                plan: Some("pro".into()),
                limit: "codex".into(),
            },
        ];
        let value = estimate(
            &samples,
            &points,
            &json!({"window_minutes":300,"resets_at":18000,"used_percent":5}),
            Some("pro"),
            100,
        );
        assert!((value["capacity"].as_f64().unwrap() - 800000.).abs() < 1e-6);
        assert_eq!(value["localTokens"], 40000.);
        assert_eq!(value["reliability"], "learning");
        assert_eq!(value["identifiable"], false);
        assert!(value["medianErrorPercent"].is_null());
        let no_hour = estimate(
            &samples,
            &[],
            &json!({"window_minutes":10080,"resets_at":604800,"used_percent":2}),
            Some("pro"),
            100,
        );
        assert!(no_hour["capacity"].is_null());
        assert_eq!(no_hour["localTokens"], 40000.);
    }
    #[test]
    fn each_window_recovers_its_own_weights_and_capacity() {
        for (minutes, multiplier) in [(300, 1.), (10080, 4.)] {
            let (samples, points, window, at) = fixture(minutes, multiplier);
            let value = estimate(&samples, &points, &window, Some("pro"), at);
            assert_eq!(value["method"], "robust-ridge-independent");
            assert_eq!(value["reliability"], "validated", "{value}");
            assert!(value["medianErrorPercent"].as_f64().unwrap() < 10.);
            for (j, truth) in [0.00004, 0.000004, 0.0002].into_iter().enumerate() {
                assert!(
                    (value["weights"][j].as_f64().unwrap() / (truth / multiplier) - 1.).abs() < 0.2,
                    "{value}"
                );
            }
            assert!(
                (value["capacity"].as_f64().unwrap()
                    - 100. / value["weights"][0].as_f64().unwrap())
                .abs()
                    < 1e-5
            );
        }
    }
    #[test]
    fn hourly_calibration_is_independent_of_weekly_points() {
        let (samples, mut points, window, at) = fixture(300, 1.);
        let expected = estimate(&samples, &points, &window, Some("pro"), at);
        points.extend(points.clone().iter().map(|p| Point {
            minutes: 10080,
            reset: p.at + 604800,
            percent: 99.,
            limit: "weekly-only-group".into(),
            ..p.clone()
        }));
        assert_eq!(
            expected,
            estimate(&samples, &points, &window, Some("pro"), at)
        );
    }
    #[test]
    fn collinear_observations_cannot_claim_three_identified_coefficients() {
        let (mut samples, points, window, at) = fixture(300, 1.);
        for s in &mut samples {
            s.x = [10000., 20000., 1000.];
        }
        let estimate = estimate(&samples, &points, &window, Some("pro"), at);
        assert_eq!(estimate["identifiable"], false);
        assert_ne!(estimate["reliability"], "validated");
        assert!(estimate.get("weeklyRatio").is_none());
    }
    #[test]
    fn an_intermediate_drop_invalidates_an_accumulating_cohort() {
        for (minimum, levels) in [
            (3., [20., 22., 21., 23., 24.]),
            (10., [20., 29., 24., 30., 34.]),
        ] {
            let points: Vec<_> = levels
                .into_iter()
                .enumerate()
                .map(|(i, percent)| Point {
                    at: 1 + i as i64 * 30,
                    minutes: 300,
                    reset: 18000,
                    percent,
                    plan: Some("pro".into()),
                    limit: "codex".into(),
                })
                .collect();
            let samples: Vec<_> = (1..5)
                .map(|i| Sample {
                    id: format!("r{i}"),
                    at: 1 + i * 30,
                    group: "model".into(),
                    x: [1000., 0., 0.],
                })
                .collect();
            let rows = intervals_with_threshold(&samples, &points, 300, Some("pro"), minimum);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].start, 61);
            assert_eq!(rows[0].at, 121);
            assert_eq!(rows[0].percent, minimum);
            assert_eq!(rows[0].x[0], 2000.);
        }
    }
    #[test]
    fn future_observations_cannot_change_a_historical_estimate() {
        let (mut samples, mut points, window, at) = fixture(300, 1.);
        let before = estimate(&samples, &points, &window, Some("pro"), at);
        samples.push(Sample {
            id: "future".into(),
            at: at + 100,
            group: "new-model|fast|high".into(),
            x: [10000000., 10000000., 10000000.],
        });
        points.push(Point {
            at: at + 100,
            minutes: 300,
            reset: at + 18000,
            percent: 99.,
            plan: Some("pro".into()),
            limit: "new-group".into(),
        });
        assert_eq!(
            estimate(&samples, &points, &window, Some("pro"), at),
            before
        );
    }
    #[test]
    fn resets_plan_changes_missing_intervals_and_expiring_usage_do_not_train_as_spend() {
        for minutes in [300, 10080] {
            let (s, mut p, w, at) = fixture(minutes, 1.);
            let valid = intervals(&s, &p, minutes, Some("pro")).len();
            p[2].percent = 0.;
            assert!(intervals(&s, &p, minutes, Some("pro")).len() < valid);
            assert!(estimate(&s, &p, &w, Some("plus"), at)["capacity"].is_null());
            assert!(
                estimate(&s, &p, &w, Some("pro"), w["resets_at"].as_i64().unwrap())["capacity"]
                    .is_null()
            );
            assert!(estimate(&[], &p, &w, Some("pro"), at)["localTokens"].is_null());
            assert!(intervals(&[], &p, minutes, Some("pro")).is_empty());
            let mut bad = w.clone();
            bad["used_percent"] = json!(105);
            assert!(estimate(&s, &p, &bad, Some("pro"), at).is_null());
        }
    }
    #[test]
    fn one_outlier_and_a_new_unpriced_model_do_not_destroy_the_robust_fit() {
        let (mut s, mut p, w, at) = fixture(300, 1.);
        for point in p
            .iter_mut()
            .filter(|p| p.reset == p_reset(300, 3) && p.at >= 1_700_000_000 + 3 * 300 * 60 + 1260)
        {
            point.percent += 25.;
        }
        for sample in s.iter_mut().rev().take(3) {
            sample.group = "brand-new-unpriced|priority".into();
        }
        let e = estimate(&s, &p, &w, Some("pro"), at);
        assert!(e["capacity"].is_number(), "{e}");
        assert!(e["medianErrorPercent"].as_f64().unwrap() < 20., "{e}");
    }
    fn p_reset(minutes: i64, w: i64) -> i64 {
        1_700_000_000 + (w + 1) * minutes * 60
    }
    #[test]
    fn incompatible_external_usage_is_rejected_by_held_out_error() {
        let (s, mut p, w, at) = fixture(300, 1.);
        for point in &mut p {
            if point.reset > p_reset(300, 2) {
                point.percent *= 3.;
            }
        }
        let e = estimate(&s, &p, &w, Some("pro"), at);
        assert_eq!(e["reliability"], "unstable");
        assert!(e["capacity"].is_null());
        assert!(e["localTokens"].as_f64().unwrap() > 0.);
    }
    #[test]
    fn persistent_store_deduplicates_responses_and_retains_old_history() {
        let dir = std::env::temp_dir().join(format!(
            "qufu-regression-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("learn.sqlite3");
        {
            let store = Store::open(&path).unwrap();
            for thread in ["main", "child", "copied"] {
                store.connection.execute("INSERT OR IGNORE INTO usage VALUES('response:shared',?1,100,'no-price','standard',10000,20000,10000)",[thread]).unwrap();
            }
            let q = json!({"plan_type":"pro","primary":{"window_minutes":300,"resets_at":18000,"used_percent":5}});
            store.point(100, &q).unwrap();
            store.point(100, &q).unwrap();
        }
        let store = Store::open(&path).unwrap();
        let data = store.load(0).unwrap();
        assert_eq!(data.samples.len(), 1);
        assert_eq!(data.samples[0].tokens(), 40_000.);
        assert_eq!(data.points.len(), 1);
        assert_eq!(data.samples[0].at, 100);
        drop(store);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn scanner_restart_resumes_persisted_history_and_only_adds_new_responses() {
        let dir = std::env::temp_dir().join(format!(
            "qufu-cursor-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let home = dir.join("codex");
        std::fs::create_dir_all(&home).unwrap();
        let file = home.join("rollout-root.jsonl");
        let db = Connection::open(home.join("state_5.sqlite")).unwrap();
        db.execute_batch("CREATE TABLE threads(id TEXT PRIMARY KEY,name TEXT,title TEXT,cwd TEXT,rollout_path TEXT,source TEXT,archived INTEGER,tokens_used INTEGER,updated_at INTEGER);").unwrap();
        db.execute(
            "INSERT INTO threads VALUES('root','Example','Example','/example',?1,'cli',0,100,1)",
            [file.to_string_lossy()],
        )
        .unwrap();
        drop(db);
        let record = |id: &str, total: u64, raw: u64| {
            json!({"type":"token_usage_record","timestamp":"2020-01-01T00:00:01Z","payload":{"thread_id":"root","response_id":id,"usage":{"input_tokens":raw,"cached_input_tokens":0,"output_tokens":0},"thread_token_usage":{"input_tokens":total,"output_tokens":0}}}).to_string()
        };
        let header = json!({"type":"turn_context","payload":{"model":"no-api-price"}}).to_string();
        let first = format!("{header}\n{}\n", record("r1", 100, 100));
        std::fs::write(&file, &first).unwrap();
        let mut store = Store::open(&dir.join("learning.sqlite3")).unwrap();
        let mut a = data::Reader::new();
        a.home = home.clone();
        let data = store.collect(&mut a, &Value::Null).unwrap();
        assert_eq!(data.samples.len(), 1);
        assert!(data.samples[0].at < chrono::Utc::now().timestamp() - 14 * 86400);
        let mut b = data::Reader::new();
        b.home = home.clone();
        let data = store.collect(&mut b, &Value::Null).unwrap();
        assert_eq!(data.samples.len(), 1);
        assert!(b.cache["root"].seen.is_empty());
        std::fs::write(&file, format!("{first}{}\n", record("r2", 150, 50))).unwrap();
        let data = store.collect(&mut b, &Value::Null).unwrap();
        assert_eq!(data.samples.len(), 2);
        assert_eq!(data.samples.iter().map(Sample::tokens).sum::<f64>(), 150.);
        std::fs::write(
            &file,
            format!(
                "{}\n{}\n",
                json!({"type":"turn_context","payload":{"model":"different-unpriced-model"}}),
                record("r3", 20, 20)
            ),
        )
        .unwrap();
        let mut c = data::Reader::new();
        c.home = home;
        let data = store.collect(&mut c, &Value::Null).unwrap();
        assert_eq!(data.samples.len(), 3);
        assert!(c.cache["root"].seen.contains("r3"));
        drop(store);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn interleaved_resets_do_not_create_overlapping_training_intervals() {
        let samples: Vec<_> = (1..=4)
            .map(|i| Sample {
                id: format!("r{i}"),
                at: i * 100,
                group: "unknown|standard".into(),
                x: [40000., 0., 0.],
            })
            .collect();
        let points: Vec<_> = [
            (50, 18000, 0.),
            (150, 18100, 0.),
            (250, 18000, 5.),
            (350, 18100, 5.),
        ]
        .into_iter()
        .map(|(at, reset, percent)| Point {
            at,
            reset,
            percent,
            minutes: 300,
            plan: Some("pro".into()),
            limit: "codex".into(),
        })
        .collect();
        assert!(intervals(&samples, &points, 300, Some("pro")).is_empty());
    }
    #[test]
    fn old_database_migrates_without_loss_and_backs_up_before_schema_changes() {
        let dir = std::env::temp_dir().join(format!(
            "qufu-migration-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("quota-learning.sqlite3");
        let c = Connection::open(&path).unwrap();
        c.execute_batch("CREATE TABLE usage(id TEXT PRIMARY KEY,thread TEXT NOT NULL,at INTEGER NOT NULL,model TEXT NOT NULL,tier TEXT NOT NULL,noncached REAL NOT NULL,cached REAL NOT NULL,output REAL NOT NULL);CREATE TABLE points(at INTEGER,minutes INTEGER,reset INTEGER,percent REAL,plan TEXT,PRIMARY KEY(at,minutes,reset,plan));INSERT INTO usage VALUES('legacy:a:100:40000','a',100,'model','standard',10000,20000,10000);INSERT INTO points VALUES(100,300,18000,5,'pro');").unwrap();
        drop(c);
        let store = Store::open(&path).unwrap();
        assert_eq!(
            store
                .connection
                .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            4
        );
        let data = store.load(0).unwrap();
        assert_eq!(data.samples.len(), 1);
        assert_eq!(data.points.len(), 1);
        assert_eq!(data.samples[0].tokens(), 40000.);
        assert_eq!(
            store
                .connection
                .query_row("SELECT COUNT(*) FROM points", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert!(std::fs::read_dir(&dir).unwrap().any(|p| p
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("pre-v4")));
        let mut turn = crate::turns::Turn::default();
        turn.id = "t".into();
        turn.status = "completed".into();
        turn.usage = Some(data::Usage {
            input: 100,
            output: 20,
            total: 120,
            ..Default::default()
        });
        store
            .connection
            .execute(
                "INSERT INTO turn_measurements VALUES('a','t',?1,100)",
                [serde_json::to_string(&turn).unwrap()],
            )
            .unwrap();
        store.save_calibration(&data,&json!({"at":100,"stale":false,"value":{"plan_type":"pro","limit_id":"codex","primary":{"window_minutes":300,"resets_at":18000,"used_percent":5}}})).unwrap();
        drop(store);
        let estimator = Estimator::new(dir.clone());
        assert_eq!(
            estimator.history("turns", Some("a")).unwrap()[0]["rawTotal"],
            120
        );
        let history = estimator.history("five", None).unwrap();
        assert_eq!(history["points"][0]["percent"], 5.);
        assert_eq!(history["fits"].as_array().unwrap().len(), 1);
        drop(estimator);
    }
    #[test]
    fn capacity_changes_never_assert_backend_changes_and_control_relearning() {
        let original = json!({"capacity":1000000.,"identifiable":true,"weights":[0.0001,0.00001,0.0003],"weeklyRatio":0.15,"p90ErrorPercent":5.,"modelMix":{"gpt|standard":1.}});
        let mut next = original.clone();
        next["capacity"] = json!(1400000.);
        let change = capacity_change(&original, &next);
        assert_eq!(change["status"], "estimated_capacity_change");
        assert_eq!(change["backendChangeVerified"], false);
        next["weights"][1] = json!(0.00008);
        assert_eq!(
            capacity_change(&original, &next)["status"],
            "weights_relearned"
        );
        next = original.clone();
        next["calibrationMode"] = json!("independent-v1");
        assert_eq!(
            capacity_change(&original, &next)["status"],
            "calibration_changed"
        );
        next = original.clone();
        next["p90ErrorPercent"] = Value::Null;
        assert_eq!(capacity_change(&original, &next)["status"], "insufficient");
    }
    #[test]
    fn supersession_matches_one_response_once_across_restart() {
        let dir = std::env::temp_dir().join(format!(
            "qufu-supersession-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let home = dir.join("codex");
        std::fs::create_dir_all(&home).unwrap();
        let file = home.join("rollout-root.jsonl");
        let db = Connection::open(home.join("state_5.sqlite")).unwrap();
        db.execute_batch("CREATE TABLE threads(id TEXT PRIMARY KEY,name TEXT,title TEXT,cwd TEXT,rollout_path TEXT,source TEXT,archived INTEGER,tokens_used INTEGER,updated_at INTEGER);").unwrap();
        db.execute(
            "INSERT INTO threads VALUES('root','example','example','/example',?1,'cli',0,100,1)",
            [file.to_string_lossy()],
        )
        .unwrap();
        drop(db);
        let at = data::timestamp(&json!("2026-10-27T12:00:00Z"));
        std::fs::write(&file,json!({"type":"token_usage_record","timestamp":"2026-10-27T12:00:00Z","payload":{"thread_id":"root","response_id":"one","turn_id":"task","usage":{"input_tokens":100,"cached_input_tokens":0,"output_tokens":0},"thread_token_usage":{"input_tokens":100,"cached_input_tokens":0,"output_tokens":0},"turn_token_usage":{"input_tokens":100,"cached_input_tokens":0,"output_tokens":0}}}).to_string()).unwrap();
        let mut store = Store::open(&dir.join("quota-learning.sqlite3")).unwrap();
        for id in ["legacy:root:t:100", "legacy:root:t:200"] {
            store
                .connection
                .execute(
                    "INSERT INTO usage VALUES(?1,'root',?2,'model','standard',100,0,0)",
                    params![id, at],
                )
                .unwrap();
        }
        for _ in 0..2 {
            let mut reader = data::Reader::new();
            reader.home = home.clone();
            let data = store.collect(&mut reader, &Value::Null).unwrap();
            assert_eq!(data.samples.iter().map(Sample::tokens).sum::<f64>(), 200.);
        }
        assert_eq!(
            store
                .connection
                .query_row("SELECT COUNT(*) FROM usage", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            3
        );
        assert_eq!(
            store
                .connection
                .query_row("SELECT COUNT(*) FROM usage_superseded", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            store
                .connection
                .query_row("SELECT COUNT(*) FROM turn_measurements", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}
