use crate::{data, pricing};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::mpsc::{self, Receiver, Sender},
    time::Duration,
};

#[derive(Clone)]
struct Sample {
    at: i64,
    tokens: f64,
    usd: Option<f64>,
}

#[derive(Clone)]
struct Point {
    at: i64,
    minutes: i64,
    reset: i64,
    percent: f64,
    plan: Option<String>,
}

struct Interval {
    reset: i64,
    usd: f64,
    percent: f64,
}

struct Calibration {
    usd_per_percent: f64,
    median_error: f64,
    p90_error: f64,
    intervals: usize,
    resets: usize,
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[middle - 1] + values[middle]) / 2.
    } else {
        values[middle]
    }
}

fn totals(samples: &[Sample], start: i64, end: i64) -> (f64, f64, f64) {
    samples.iter().filter(|s| s.at > start && s.at <= end).fold(
        (0., 0., 0.),
        |(tokens, usd, priced), s| {
            (
                tokens + s.tokens,
                usd + s.usd.unwrap_or(0.),
                priced + if s.usd.is_some() { s.tokens } else { 0. },
            )
        },
    )
}

fn calibrate(
    samples: &[Sample],
    points: &[Point],
    minutes: i64,
    plan: Option<&str>,
) -> Option<Calibration> {
    let mut groups: BTreeMap<i64, BTreeMap<i64, f64>> = BTreeMap::new();
    for p in points.iter().filter(|p| {
        p.minutes == minutes && p.percent.is_finite() && (0. ..=100.).contains(&p.percent)
    }) {
        if plan.zip(p.plan.as_deref()).is_some_and(|(a, b)| a != b) {
            continue;
        }
        groups
            .entry(p.reset)
            .or_default()
            .entry(p.at)
            .and_modify(|v| *v = v.max(p.percent))
            .or_insert(p.percent);
    }
    let mut intervals = Vec::new();
    for (reset, points) in groups {
        let mut anchor: Option<(i64, f64)> = None;
        for (at, percent) in points {
            let Some((start, previous)) = anchor else {
                anchor = Some((at, percent));
                continue;
            };
            if percent < previous {
                anchor = Some((at, percent));
                continue;
            }
            if percent - previous < 5. || at - start < 120 {
                continue;
            }
            let (tokens, usd, priced) = totals(samples, start, at);
            if tokens > 0. && usd > 0. && priced / tokens >= 0.98 {
                intervals.push(Interval {
                    reset,
                    usd,
                    percent: percent - previous,
                });
            }
            anchor = Some((at, percent));
        }
    }
    let resets: BTreeSet<_> = intervals.iter().map(|v| v.reset).collect();
    if intervals.len() < 8 || resets.len() < 3 {
        return None;
    }
    let mut errors = Vec::new();
    for test in &intervals {
        let mut train: Vec<_> = intervals
            .iter()
            .filter(|v| v.reset != test.reset)
            .map(|v| v.usd / v.percent)
            .collect();
        if train.len() < 3 {
            continue;
        }
        let scale = median(&mut train);
        errors.push((test.usd / scale - test.percent).abs() / test.percent * 100.);
    }
    if errors.len() < 8 {
        return None;
    }
    let typical = median(&mut errors);
    let p90 = errors[((errors.len() as f64 * 0.9) as usize).min(errors.len() - 1)];
    if typical > 35. || p90 > 100. {
        return None;
    }
    let mut scales: Vec<_> = intervals.iter().map(|v| v.usd / v.percent).collect();
    Some(Calibration {
        usd_per_percent: median(&mut scales),
        median_error: typical,
        p90_error: p90,
        intervals: intervals.len(),
        resets: resets.len(),
    })
}

fn estimate(
    samples: &[Sample],
    points: &[Point],
    window: &Value,
    plan: Option<&str>,
    at: i64,
) -> Option<Value> {
    let minutes = window["window_minutes"].as_i64()?;
    let reset = window["resets_at"].as_i64()?;
    let percent = window["used_percent"].as_f64()?;
    if !matches!(minutes, 300 | 10080)
        || reset <= at
        || !percent.is_finite()
        || !(0. ..=100.).contains(&percent)
    {
        return None;
    }
    let calibration = calibrate(samples, points, minutes, plan)?;
    let (local_tokens, local_usd, priced) = totals(samples, reset - minutes * 60, at);
    if local_tokens <= 0. || local_usd <= 0. || priced / local_tokens < 0.98 {
        return None;
    }
    let capacity = calibration.usd_per_percent * 100. * local_tokens / local_usd;
    if !capacity.is_finite() || capacity <= 0. {
        return None;
    }
    Some(
        json!({"used":capacity * percent / 100.,"capacity":capacity,"localTokens":local_tokens,"method":"price-weighted","approximate":true,"scope":"local-calibration","intervals":calibration.intervals,"resets":calibration.resets,"medianErrorPercent":calibration.median_error,"p90ErrorPercent":calibration.p90_error}),
    )
}

type Observations = (Vec<Sample>, Vec<Point>);

pub struct Estimator {
    jobs: Sender<()>,
    incoming: Receiver<Observations>,
    scanning: bool,
    last_scan: i64,
    samples: Vec<Sample>,
    points: Vec<Point>,
    calibration_at: i64,
    cached: Value,
}

fn collect(reader: &mut data::Reader) -> Observations {
    let now = chrono::Utc::now().timestamp();
    let mut samples = Vec::new();
    let mut points = Vec::new();
    if let Ok((threads, _)) = reader.catalog() {
        samples.clear();
        points.clear();
        for thread in threads {
            if thread.source.contains("guardian") {
                continue;
            }
            let Ok(r) = reader.read_thread(&thread) else {
                continue;
            };
            for s in r
                .window_samples
                .iter()
                .filter(|s| s.at >= now - 14 * 86400 && s.model != "codex-auto-review")
            {
                samples.push(Sample {
                    at: s.at,
                    tokens: s.usage.total as f64,
                    usd: pricing::cost(&s.model, &s.tier, &s.usage, s.usage.input > 272_000),
                });
            }
            for (at, q) in r
                .quota_history
                .iter()
                .filter(|(at, _)| *at >= now - 14 * 86400)
            {
                for key in ["primary", "secondary"] {
                    let w = &q[key];
                    if let (Some(minutes), Some(reset), Some(percent)) = (
                        w["window_minutes"].as_i64(),
                        w["resets_at"].as_i64(),
                        w["used_percent"].as_f64(),
                    ) {
                        points.push(Point {
                            at: *at,
                            minutes,
                            reset,
                            percent,
                            plan: q["plan_type"].as_str().map(str::to_owned),
                        });
                    }
                }
            }
        }
    }

    (samples, points)
}

impl Default for Estimator {
    fn default() -> Self {
        let (jobs, requests) = mpsc::channel();
        let (outputs, incoming) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = data::Reader::new();
            while requests.recv().is_ok() {
                if outputs.send(collect(&mut reader)).is_err() {
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
        }
    }
}

impl Estimator {
    pub fn update_blocking(&mut self, quota: &Value) -> Value {
        self.update(quota);
        if self.scanning {
            if let Ok((samples, points)) = self.incoming.recv_timeout(Duration::from_secs(60)) {
                self.samples = samples;
                self.points = points;
                self.scanning = false;
                self.calibration_at = 0;
            }
        }
        self.update(quota)
    }
    pub fn diagnostics(&self, quota: &Value) -> Value {
        let value = &quota["value"];
        let at = quota["at"].as_i64().unwrap_or(0);
        let window = |key: &str| {
            let w = &value[key];
            let Some(minutes) = w["window_minutes"].as_i64() else {
                return Value::Null;
            };
            let reset = w["resets_at"].as_i64().unwrap_or(0);
            let (tokens, usd, priced) = totals(&self.samples, reset - minutes * 60, at);
            let c = calibrate(
                &self.samples,
                &self.points,
                minutes,
                value["plan_type"].as_str(),
            );
            json!({"localTokens":tokens,"localUsd":usd,"pricedCoverage":if tokens>0. {Some(priced/tokens)} else {None},"calibration":c.map(|c|json!({"intervals":c.intervals,"resets":c.resets,"medianErrorPercent":c.median_error,"p90ErrorPercent":c.p90_error}))})
        };
        json!({"samples":self.samples.len(),"points":self.points.len(),"primary":window("primary"),"secondary":window("secondary")})
    }
    pub fn update(&mut self, quota: &Value) -> Value {
        let now = chrono::Utc::now().timestamp();
        if let Ok((samples, points)) = self.incoming.try_recv() {
            self.samples = samples;
            self.points = points;
            self.scanning = false;
            self.calibration_at = 0;
        }
        if now - self.last_scan >= 30 && !self.scanning {
            self.last_scan = now;
            self.scanning = self.jobs.send(()).is_ok();
        }
        if now - self.calibration_at >= 30 || self.cached["quotaAt"] != quota["at"] {
            self.calibration_at = now;
            let value = &quota["value"];
            let at = quota["at"].as_i64().unwrap_or(0);
            let plan = value["plan_type"].as_str();
            self.cached = json!({"quotaAt":quota["at"],"primary":estimate(&self.samples,&self.points,&value["primary"],plan,at),"secondary":estimate(&self.samples,&self.points,&value["secondary"],plan,at)});
        }
        self.cached.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Vec<Sample>, Vec<Point>) {
        let mut samples = Vec::new();
        let mut points = Vec::new();
        for group in 0..4 {
            let start = group * 20000;
            let reset = start + 18000;
            for i in 0..5 {
                points.push(Point {
                    at: start + i * 200,
                    minutes: 300,
                    reset,
                    percent: i as f64 * 10.,
                    plan: Some("plus".into()),
                });
                if i > 0 {
                    samples.push(Sample {
                        at: start + i * 200 - 10,
                        tokens: 1000.,
                        usd: Some(1.),
                    });
                }
            }
        }
        (samples, points)
    }
    #[test]
    fn calibrated_equivalents_use_account_percent_and_are_explicit_estimates() {
        let (samples, points) = fixture();
        let q = json!({"window_minutes":300,"resets_at":78000,"used_percent":35.});
        let v = estimate(&samples, &points, &q, Some("plus"), 60800).unwrap();
        assert_eq!(v["capacity"], 10000.);
        assert_eq!(v["used"], 3500.);
        assert_eq!(v["localTokens"], 4000.);
        assert_eq!(v["approximate"], true);
    }
    #[test]
    fn sparse_or_different_plan_history_is_not_an_estimate() {
        let (samples, points) = fixture();
        assert!(calibrate(&samples, &points[..5], 300, Some("plus")).is_none());
        assert!(calibrate(&samples, &points, 300, Some("pro")).is_none());
    }
    #[test]
    fn missing_prices_and_out_of_range_percent_do_not_generate_capacity() {
        let (mut samples, points) = fixture();
        for s in &mut samples {
            s.usd = None;
        }
        assert!(calibrate(&samples, &points, 300, Some("plus")).is_none());
        let (samples, points) = fixture();
        assert!(estimate(
            &samples,
            &points,
            &json!({"window_minutes":300,"resets_at":78000,"used_percent":101.}),
            Some("plus"),
            60800
        )
        .is_none());
    }
    #[test]
    fn unstable_cross_window_calibration_is_rejected() {
        let (mut samples, points) = fixture();
        for s in &mut samples {
            if s.at > 40000 {
                s.usd = Some(30.);
            }
        }
        assert!(calibrate(&samples, &points, 300, Some("plus")).is_none());
    }
}
