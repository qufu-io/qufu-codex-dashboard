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
fn intervals(
    samples: &[Sample],
    points: &[Point],
    minutes: i64,
    plan: Option<&str>,
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
    for observations in timeline.into_values() {
        let resets: BTreeSet<_> = observations.iter().map(|p| p.reset).collect();
        let plans: BTreeSet<_> = observations
            .iter()
            .filter_map(|p| p.plan.as_deref())
            .collect();
        if resets.len() != 1 || plans.len() > 1 {
            anchor = None;
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
            || p.percent < a.percent
        {
            anchor = Some(p);
            continue;
        }
        if p.percent - a.percent < 2. || p.at - a.at < 30 {
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
    if out.len() > 240 {
        out.drain(..out.len() - 240);
    }
    out
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
        for _ in 0..100 {
            let old = beta;
            for j in 0..3 {
                let mut numerator = 0.05 * base;
                let mut denominator = 0.05;
                for (r, w) in rows.iter().zip(&weights) {
                    let x = r.x.map(|v| v / 100_000.);
                    let residual = r.percent - dot(&beta, &x) + beta[j] * x[j];
                    numerator += w * x[j] * residual;
                    denominator += w * x[j] * x[j];
                }
                beta[j] = (numerator / denominator).clamp(base * 0.01, base * 50.);
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
    let regression =
        rows.len() >= 12 && rows.iter().map(|r| r.reset).collect::<BTreeSet<_>>().len() >= 3;
    let mut m = Model {
        common: fit_coefficients(rows, regression),
        groups: BTreeMap::new(),
        regression,
    };
    if regression {
        let mut groups = BTreeMap::<String, Vec<&Interval>>::new();
        for r in rows {
            if r.groups.len() == 1 {
                groups
                    .entry(r.groups.keys().next().unwrap().clone())
                    .or_default()
                    .push(r);
            }
        }
        for (name, rows) in groups {
            if rows.len() >= 12 && rows.iter().map(|r| r.reset).collect::<BTreeSet<_>>().len() >= 3
            {
                m.groups.insert(name, fit_coefficients(&rows, true));
            }
        }
    }
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
    let mut mix = BTreeMap::<String, [f64; 3]>::new();
    let mut local = 0.;
    for s in samples
        .iter()
        .filter(|s| s.at > reset - minutes * 60 && s.at <= at)
    {
        local += s.tokens();
        let x = mix.entry(s.group.clone()).or_default();
        for j in 0..3 {
            x[j] += s.x[j];
        }
    }
    let rows = intervals(samples, points, minutes, plan);
    let mut value = json!({"used":null,"capacity":null,"localTokens":if samples.is_empty(){None}else{Some(local)},"approximate":true,"intervals":rows.len(),"resets":rows.iter().map(|r|r.reset).collect::<BTreeSet<_>>().len(),"medianErrorPercent":null,"p90ErrorPercent":null,"validationCount":0,"reliability":"insufficient","method":"ratio","priceIndependent":true});
    if rows.is_empty() {
        return value;
    }
    let (typical, p90, count) = validation(&rows);
    value["medianErrorPercent"] = json!(typical);
    value["p90ErrorPercent"] = json!(p90);
    value["validationCount"] = json!(count);
    if typical.is_some_and(|v| v > 35.) || p90.is_some_and(|v| v > 100.) {
        value["reliability"] = json!("unstable");
        return value;
    }
    let model = fit(&rows.iter().collect::<Vec<_>>());
    if mix.is_empty() {
        for r in &rows {
            for (name, x) in &r.groups {
                let g = mix.entry(name.clone()).or_default();
                for j in 0..3 {
                    g[j] += x[j];
                }
            }
        }
    }
    let tokens: f64 = mix.values().map(|v| v.iter().sum::<f64>()).sum();
    let prediction = model.predict(&mix);
    let capacity = 100. * tokens / prediction;
    if !capacity.is_finite() || !(1. ..=1e13).contains(&capacity) {
        return value;
    }
    value["capacity"] = json!(capacity);
    value["used"] = json!(capacity * percent / 100.);
    value["method"] = json!(if model.regression {
        "robust-ridge"
    } else {
        "ratio"
    });
    value["reliability"] = json!(if count >= 8 { "validated" } else { "learning" });
    value["specificModels"] = json!(model.groups.len());
    value
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
                            reader
                                .cache
                                .insert(thread.id.clone(), data::Rollout::restore_cursor(&state));
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
            if r.record_mode {
                transaction
                    .execute(
                        "DELETE FROM usage WHERE thread=?1 AND id LIKE 'legacy:%'",
                        [&thread.id],
                    )
                    .map_err(|e| e.to_string())?;
            }
            for s in &r.window_samples {
                if s.usage.cache > s.usage.input || s.at <= 0 {
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
            historical.extend(r.quota_history.iter().cloned());
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
        self.load(unreadable)
    }
    fn load(&self, unreadable: usize) -> Result<Observations, String> {
        let mut query = self
            .connection
            .prepare("SELECT id,at,model,tier,noncached,cached,output FROM usage ORDER BY at,id")
            .map_err(|e| e.to_string())?;
        let samples: Vec<Sample> = query
            .query_map([], |r| {
                Ok(Sample {
                    id: r.get(0)?,
                    at: r.get(1)?,
                    group: format!("{}|{}", r.get::<_, String>(2)?, r.get::<_, String>(3)?),
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
            .prepare("SELECT at,minutes,reset,percent,plan FROM points ORDER BY at")
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
        std::thread::spawn(move || {
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
                });
                last_at = at;
            }
            window = json!({"window_minutes":minutes,"resets_at":reset,"used_percent":percent});
        }
        (samples, points, window, last_at)
    }
    #[test]
    fn initial_ratio_uses_real_tokens_without_any_price() {
        for (minutes, percent, capacity) in [(300, 5., 800_000.), (10080, 2., 2_000_000.)] {
            let s = vec![Sample {
                id: "r".into(),
                at: 100,
                group: "no-api-rate|standard".into(),
                x: [10_000., 20_000., 10_000.],
            }];
            let reset = minutes * 60;
            let p = vec![
                Point {
                    at: 1,
                    minutes,
                    reset,
                    percent: 0.,
                    plan: Some("pro".into()),
                },
                Point {
                    at: 100,
                    minutes,
                    reset,
                    percent,
                    plan: Some("pro".into()),
                },
            ];
            let e = estimate(
                &s,
                &p,
                &json!({"window_minutes":minutes,"resets_at":reset,"used_percent":percent}),
                Some("pro"),
                100,
            );
            assert!((e["capacity"].as_f64().unwrap() - capacity).abs() < 1e-6);
            assert_eq!(e["localTokens"], 40_000.);
            assert_eq!(e["reliability"], "learning");
            assert!(e["medianErrorPercent"].is_null());
        }
    }
    #[test]
    fn independent_hourly_and_weekly_regression_validate_on_other_reset_windows() {
        for (minutes, multiplier) in [(300, 1.), (10080, 10.)] {
            let (s, p, w, at) = fixture(minutes, multiplier);
            let e = estimate(&s, &p, &w, Some("pro"), at);
            assert_eq!(e["method"], "robust-ridge");
            assert_eq!(e["reliability"], "validated");
            assert!(e["validationCount"].as_u64().unwrap() >= 8);
            assert!(e["medianErrorPercent"].as_f64().unwrap() < 10., "{e}");
            let current: Vec<_> = s
                .iter()
                .filter(|v| v.at > w["resets_at"].as_i64().unwrap() - minutes * 60)
                .collect();
            let total: f64 = current.iter().map(|s| s.tokens()).sum();
            let predicted: f64 = current
                .iter()
                .map(|s| {
                    dot(
                        &[
                            0.00004 / multiplier,
                            0.000004 / multiplier,
                            0.0002 / multiplier,
                        ],
                        &s.x,
                    )
                })
                .sum();
            let expected = 100. * total / predicted;
            assert!(
                (e["capacity"].as_f64().unwrap() / expected - 1.).abs() < 0.1,
                "{e}"
            );
            assert_eq!(e["priceIndependent"], true);
        }
    }
    #[test]
    fn resets_plan_changes_missing_intervals_and_expiring_usage_do_not_train_as_spend() {
        let (s, mut p, w, at) = fixture(300, 1.);
        let valid = intervals(&s, &p, 300, Some("pro")).len();
        p[2].percent = 0.;
        assert!(intervals(&s, &p, 300, Some("pro")).len() < valid);
        assert!(estimate(&s, &p, &w, Some("plus"), at)["capacity"].is_null());
        assert!(
            estimate(&s, &p, &w, Some("pro"), w["resets_at"].as_i64().unwrap())["capacity"]
                .is_null()
        );
        assert!(estimate(&[], &p, &w, Some("pro"), at)["localTokens"].is_null());
        assert!(intervals(&[], &p, 300, Some("pro")).is_empty());
        let mut bad = w.clone();
        bad["used_percent"] = json!(105);
        assert!(estimate(&s, &p, &bad, Some("pro"), at).is_null());
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
            json!({"type":"token_usage_record","timestamp":"2020-01-01T00:00:01Z","payload":{"thread_id":"root","response_id":id,"usage":{"input_tokens":raw,"output_tokens":0},"thread_token_usage":{"input_tokens":total,"output_tokens":0}}}).to_string()
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
        })
        .collect();
        assert!(intervals(&samples, &points, 300, Some("pro")).is_empty());
    }
}
