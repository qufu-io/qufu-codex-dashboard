use crate::data::Usage;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::{
    collections::BTreeMap,
    io::Read,
    path::Path,
    sync::{OnceLock, RwLock},
    time::Duration,
};

const URL: &str = "https://developers.openai.com/api/docs/pricing.md";

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Rate {
    input: f64,
    cache: Option<f64>,
    write: Option<f64>,
    output: f64,
    #[serde(default)]
    write_long: Option<f64>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Catalog {
    #[serde(default)]
    pub schema_version: u32,
    pub updated_at: i64,
    pub source: String,
    pub rates: BTreeMap<String, Rate>,
    #[serde(default)]
    retained: BTreeSet<String>,
}
#[derive(Clone)]
struct State {
    catalog: Catalog,
    origin: String,
    checking: bool,
    coverage: BTreeMap<String, bool>,
}
static STATE: OnceLock<RwLock<State>> = OnceLock::new();
fn state() -> &'static RwLock<State> {
    STATE.get_or_init(|| {
        RwLock::new(State {
            catalog: serde_json::from_str(include_str!("../prices.json")).unwrap_or_default(),
            origin: "bundled".into(),
            checking: false,
            coverage: BTreeMap::new(),
        })
    })
}
fn money(s: &str) -> Option<f64> {
    s.trim()
        .strip_prefix('$')?
        .replace(',', "")
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && *n >= 0.)
}
fn key(model: &str, tier: &str, long: bool) -> String {
    format!("{model}|{tier}|{}", if long { "long" } else { "short" })
}
pub fn normalize_model(model: &str) -> String {
    let mut name = model.trim().trim_matches('`').to_ascii_lowercase();
    if let Some(v) = name.strip_prefix("openai/") {
        name = v.into();
    }
    loop {
        let previous = name.clone();
        name = name.trim_end_matches([')', ']']).trim().to_owned();
        for effort in [
            "very_high",
            "very-high",
            "xhigh",
            "medium",
            "minimal",
            "high",
            "low",
            "none",
            "max",
            "ultra",
        ] {
            for separator in [
                "-reasoning-",
                "_reasoning_",
                "-reasoning-effort-",
                "_reasoning_effort_",
                " reasoning_effort=",
                " reasoning=",
                "-",
                "_",
                " ",
                " (",
                " [",
                "/",
            ] {
                if let Some(v) = name.strip_suffix(&format!("{separator}{effort}")) {
                    name = v.trim().into();
                }
            }
        }
        if name == previous {
            break;
        }
    }
    name
}
pub fn normalize_tier(tier: &str) -> String {
    match tier.trim().to_ascii_lowercase().as_str() {
        "" | "default" | "standard" => "standard".into(),
        "priority" => "fast".into(),
        v => v.into(),
    }
}
fn cells(line: &str) -> Vec<String> {
    line.trim_matches('|')
        .split('|')
        .map(|v| v.trim().trim_matches('`').to_ascii_lowercase())
        .collect()
}
pub fn parse(markdown: &str) -> Result<Catalog, String> {
    let mut book = Catalog {
        schema_version: 2,
        updated_at: chrono::Utc::now().timestamp(),
        source: URL.into(),
        rates: BTreeMap::new(),
        retained: BTreeSet::new(),
    };
    let mut tier = String::new();
    let mut headers = Vec::<String>::new();
    for line in markdown.lines().map(str::trim) {
        let heading = line.trim_start_matches('#').trim().to_ascii_lowercase();
        if let Some(t) = ["standard", "priority", "fast", "ultrafast", "batch", "flex"]
            .iter()
            .find(|t| heading == **t || heading == format!("{t} pricing data"))
        {
            tier = normalize_tier(t);
            continue;
        }
        if !line.starts_with('|') {
            continue;
        }
        let raw: Vec<_> = line.trim_matches('|').split('|').map(str::trim).collect();
        let names = cells(line);
        if names.iter().any(|v| v == "model") {
            headers = names;
            continue;
        }
        let Some(model_index) = headers.iter().position(|v| v == "model") else {
            continue;
        };
        if tier.is_empty() || raw.len() != headers.len() {
            continue;
        }
        let model = raw[model_index]
            .trim_matches('`')
            .split_whitespace()
            .next()
            .unwrap_or("");
        if !(model.starts_with("gpt-")
            || model.starts_with("o1")
            || model.starts_with("o3")
            || model.starts_with("o4"))
        {
            continue;
        }
        for long in [false, true] {
            let mut r = Rate::default();
            let mut has_input = false;
            let mut has_output = false;
            let mut occurrences = BTreeMap::<String, usize>::new();
            for (index, header) in headers.iter().enumerate() {
                if index == model_index {
                    continue;
                }
                let kind = if header.contains("write") {
                    if header.contains("12h") || header.contains("24h") {
                        "write_long"
                    } else {
                        "write"
                    }
                } else if header.contains("cache") {
                    "cache"
                } else if header.contains("output") {
                    "output"
                } else if header.contains("input") {
                    "input"
                } else {
                    continue;
                };
                let occurrence = occurrences.entry(kind.into()).or_default();
                let is_long = header.contains("long")
                    || (!header.contains("short") && !header.contains("long") && *occurrence > 0);
                *occurrence += 1;
                if is_long != long {
                    continue;
                }
                let value = money(raw[index]);
                match kind {
                    "input" => {
                        if let Some(v) = value {
                            r.input = v;
                            has_input = true;
                        }
                    }
                    "output" => {
                        if let Some(v) = value {
                            r.output = v;
                            has_output = true;
                        }
                    }
                    "cache" => r.cache = value,
                    "write" => r.write = value,
                    "write_long" => r.write_long = value,
                    _ => {}
                }
            }
            if has_input && has_output {
                book.rates.insert(key(model, &tier, long), r);
            }
        }
    }
    if !valid(&book) {
        return Err("Unrecognized price table".into());
    }
    Ok(book)
}
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    pub usd: Option<f64>,
    pub approximate: bool,
    pub partial: bool,
    pub priced_tokens: u64,
    pub total_tokens: u64,
}
impl Quote {
    pub fn combine(values: impl IntoIterator<Item = Quote>) -> Self {
        let mut out = Self::default();
        let mut found = false;
        for v in values {
            out.approximate |= v.approximate;
            out.partial |= v.partial;
            out.priced_tokens += v.priced_tokens;
            out.total_tokens += v.total_tokens;
            if let Some(n) = v.usd {
                *out.usd.get_or_insert(0.) += n;
                found = true;
            } else if v.total_tokens > 0 {
                out.partial = true;
            }
        }
        if !found && out.total_tokens == 0 {
            out.usd = Some(0.);
        }
        out
    }
}
fn family(model: &str) -> Option<(String, u32)> {
    let suffix = model.strip_prefix("gpt-")?;
    let (version, flavor) = suffix.split_once('-')?;
    if !matches!(flavor, "sol" | "astra" | "luna" | "terra") {
        return None;
    }
    let (major, minor) = version.split_once('.').unwrap_or((version, "0"));
    Some((format!("{major}-{flavor}"), minor.parse().ok()?))
}
fn resolve<'a>(book: &'a Catalog, model: &str, tier: &str, long: bool) -> Option<(&'a Rate, bool)> {
    if let Some(r) = book.rates.get(&key(model, tier, long)) {
        return Some((r, book.retained.contains(&key(model, tier, long))));
    }
    let normalized = normalize_model(model);
    if let Some(r) = book.rates.get(&key(&normalized, tier, long)) {
        return Some((r, book.retained.contains(&key(&normalized, tier, long))));
    }
    let mut base = normalized
        .strip_suffix("-latest")
        .unwrap_or(&normalized)
        .to_owned();
    let parts: Vec<_> = base.rsplitn(4, '-').collect();
    if parts.len() == 4
        && parts[0].len() == 2
        && parts[1].len() == 2
        && parts[2].len() == 4
        && parts[..3]
            .iter()
            .all(|v| v.bytes().all(|c| c.is_ascii_digit()))
    {
        base = parts[3].into();
    }
    if base != normalized {
        if let Some(r) = book.rates.get(&key(&base, tier, long)) {
            return Some((r, true));
        }
    }
    let (group, minor) = family(&base)?;
    let candidates: Vec<_> = book
        .rates
        .iter()
        .filter_map(|(k, r)| {
            let mut p = k.split('|');
            let name = p.next()?;
            if p.next()? != tier || p.next()? != if long { "long" } else { "short" } {
                return None;
            }
            let (g, m) = family(name)?;
            (g == group && m.abs_diff(minor) == 1).then_some((m.abs_diff(minor), r))
        })
        .collect();
    for component in [0, 1] {
        let min = candidates
            .iter()
            .map(|(_, r)| if component == 0 { r.input } else { r.output })
            .fold(f64::INFINITY, f64::min);
        let max = candidates
            .iter()
            .map(|(_, r)| if component == 0 { r.input } else { r.output })
            .fold(0., f64::max);
        if max > min * 2. {
            return None;
        }
    }
    candidates
        .into_iter()
        .min_by_key(|(d, _)| *d)
        .map(|(_, r)| (r, true))
}
pub fn quote_in(book: &Catalog, model: &str, tier: &str, usage: &Usage, long: bool) -> Quote {
    let mut q = Quote {
        total_tokens: usage.total,
        ..Default::default()
    };
    if usage
        .cache
        .checked_add(usage.write)
        .is_none_or(|v| v > usage.input)
        || usage.write_long > usage.write
    {
        return q;
    }
    let Some((r, approximate)) = resolve(book, model, &normalize_tier(tier), long) else {
        return q;
    };
    let mut total =
        (usage.input - usage.cache - usage.write) as f64 * r.input + usage.output as f64 * r.output;
    let mut priced = usage.input - usage.cache - usage.write + usage.output;
    for (count, rate) in [
        (usage.cache, r.cache),
        (usage.write - usage.write_long, r.write),
        (usage.write_long, r.write_long),
    ] {
        if count > 0 {
            if let Some(rate) = rate {
                total += count as f64 * rate;
                priced += count;
            } else {
                q.partial = true;
            }
        }
    }
    q.usd = if priced > 0 || usage.total == 0 {
        Some(total / 1e6)
    } else {
        None
    };
    q.approximate = approximate;
    q.priced_tokens = priced;
    q
}
pub fn quote(model: &str, tier: &str, usage: &Usage, long: bool) -> Quote {
    let q = quote_in(&state().read().unwrap().catalog, model, tier, usage, long);
    if usage.total > 0 {
        state().write().unwrap().coverage.insert(
            key(&normalize_model(model), &normalize_tier(tier), long),
            q.usd.is_some() && !q.partial,
        );
    }
    q
}
pub fn cost(model: &str, tier: &str, usage: &Usage, long: bool) -> Option<f64> {
    quote(model, tier, usage, long).usd
}
pub fn merge(mut previous: Catalog, incoming: Catalog) -> Catalog {
    previous.retained = previous
        .rates
        .keys()
        .filter(|k| !incoming.rates.contains_key(*k))
        .cloned()
        .chain(incoming.retained.iter().cloned())
        .collect();
    previous.schema_version = incoming.schema_version;
    previous.rates.extend(incoming.rates);
    previous.updated_at = incoming.updated_at;
    previous.source = incoming.source;
    previous
}
fn cached_catalog(bundled: Catalog, cached: Catalog) -> Catalog {
    if cached.schema_version != 2 || !valid(&cached) {
        return bundled;
    }
    if cached.updated_at >= bundled.updated_at {
        merge(bundled, cached)
    } else {
        merge(cached, bundled)
    }
}
pub fn info() -> Value {
    let s = state().read().unwrap();
    json!({"updatedAt":s.catalog.updated_at,"origin":s.origin,"checking":s.checking,"missingModels":s.coverage.values().filter(|v|!**v).count(),"matchedModels":s.coverage.values().filter(|v|**v).count(),"source":"https://developers.openai.com/api/docs/pricing"})
}
fn valid(c: &Catalog) -> bool {
    c.rates.len() >= 3
        && c.rates.values().all(|r| {
            r.input.is_finite()
                && r.input >= 0.
                && r.output.is_finite()
                && r.output >= 0.
                && r.cache.is_none_or(|v| v.is_finite() && v >= 0.)
                && r.write.is_none_or(|v| v.is_finite() && v >= 0.)
                && r.write_long.is_none_or(|v| v.is_finite() && v >= 0.)
        })
}
pub fn refresh(directory: &Path) -> Result<(), String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .connect_timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("QufuCodexDashboard/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get(URL)
        .send()
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let mut text = String::new();
    response
        .take(2_000_001)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    if text.len() > 2_000_000 {
        return Err("Price table too large".into());
    }
    let incoming = parse(&text)?;
    let book = merge(state().read().unwrap().catalog.clone(), incoming);
    let bytes = serde_json::to_vec(&book).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let temp = directory.join("prices.tmp");
    std::fs::write(&temp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(temp, directory.join("prices.json")).map_err(|e| e.to_string())?;
    let mut s = state().write().unwrap();
    s.catalog = book;
    s.origin = "live".into();
    Ok(())
}
pub fn start(directory: std::path::PathBuf) {
    if let Some(book) = std::fs::read(directory.join("prices.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<Catalog>(&b).ok())
        .filter(|book| book.schema_version == 2 && valid(book))
    {
        let mut s = state().write().unwrap();
        s.catalog = cached_catalog(s.catalog.clone(), book);
        s.origin = "cache".into();
    }
    state().write().unwrap().checking = true;
    std::thread::spawn(move || {
        let _ = refresh(&directory);
        state().write().unwrap().checking = false;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_separate_tiers_and_long_context() {
        let text="Standard\n| Model | Input | Cached | Write | Output | Long input | Long cached | Long write | Long output |\n| gpt-a | $2 | $0.1 | $2.5 | $10 | $4 | $0.2 | $5 | $15 |\n| gpt-b | $1 | $0.1 | - | $5 | - | - | - | - |\n| gpt-c | $1 | $0.1 | - | $5 | - | - | - | - |\nFast\n| gpt-a | $4 | $0.2 | $5 | $20 | $8 | $0.4 | $10 | $30 |";
        let c = parse(text).unwrap();
        assert_eq!(c.rates["gpt-a|standard|long"].output, 15.);
        assert_eq!(c.rates["gpt-a|fast|short"].input, 4.);
        assert!(!c.rates.contains_key("gpt-b|standard|long"));
    }
    #[test]
    fn rejects_error_pages_and_nonfinite_prices() {
        assert!(parse("<html>Access denied</html>").is_err());
        assert!(money("$NaN").is_none());
        assert!(money("-$1").is_none());
    }
    #[test]
    fn bundled_catalog_is_valid() {
        assert!(valid(
            &serde_json::from_str::<Catalog>(include_str!("../prices.json")).unwrap()
        ));
    }
}

#[cfg(test)]
mod regression_tests {
    use super::*;
    fn catalog() -> Catalog {
        parse(include_str!(
            "../../tests/fixtures/openai-pricing-2026-10-26.md"
        ))
        .unwrap()
    }
    fn usage() -> Usage {
        Usage {
            input: 100_000,
            cache: 60_000,
            write: 20_000,
            output: 10_000,
            total: 110_000,
            ..Default::default()
        }
    }
    #[test]
    fn official_columns_cache_lifetimes_and_context_are_not_positional() {
        let c = catalog();
        let u = usage();
        assert!(
            (quote_in(&c, "gpt-6.1-sol", "standard", &u, false)
                .usd
                .unwrap()
                - 0.196)
                .abs()
                < 1e-10
        );
        assert!(
            (quote_in(&c, "gpt-6.1-sol", "priority", &u, false)
                .usd
                .unwrap()
                - 0.392)
                .abs()
                < 1e-10
        );
        assert!(
            (quote_in(&c, "gpt-6.1-sol", "standard", &u, true)
                .usd
                .unwrap()
                - 0.342)
                .abs()
                < 1e-10
        );
        let mut long_write = u.clone();
        long_write.write_long = 20_000;
        assert!(
            (quote_in(&c, "gpt-6.1-sol", "standard", &long_write, false)
                .usd
                .unwrap()
                - 0.246)
                .abs()
                < 1e-10
        );
        assert!(
            (quote_in(&c, "gpt-6.1-sol", "standard", &long_write, true)
                .usd
                .unwrap()
                - 0.442)
                .abs()
                < 1e-10
        );
        assert_eq!(c.rates["gpt-4o|standard|short"].output, 10.);
        assert_eq!(c.rates["gpt-5.3-codex|standard|short"].input, 1.75);
    }
    #[test]
    fn efforts_dates_and_safe_family_fallback_preserve_tiers() {
        let c = catalog();
        let u = usage();
        let exact = quote_in(&c, "gpt-6.1-sol", "standard", &u, false)
            .usd
            .unwrap();
        for name in [
            "gpt-6.1-sol-high",
            "gpt-6.1-sol (xhigh)",
            "openai/gpt-6.1-sol_very_high",
            "gpt-6.1-sol-reasoning-effort-medium",
        ] {
            let q = quote_in(&c, name, "default", &u, false);
            assert_eq!(q.usd, Some(exact));
            assert!(!q.approximate, "{name}");
        }
        assert!(quote_in(&c, "gpt-6.2-sol", "standard", &u, false).approximate);
        assert!(quote_in(&c, "gpt-6.1-sol-latest", "standard", &u, false).approximate);
        assert!(quote_in(&c, "gpt-6.1-sol-2026-10-01", "standard", &u, false).approximate);
        for name in [
            "gpt-6.2-pro",
            "gpt-7-sol",
            "gpt-6.1-sol-cyber",
            "codex-auto-review",
            "completely-unknown",
        ] {
            assert!(
                quote_in(&c, name, "standard", &u, false).usd.is_none(),
                "{name}"
            );
        }
        assert!(quote_in(&c, "gpt-6.1-sol", "mystery", &u, false)
            .usd
            .is_none());
        let plain = Usage {
            input: 1_000_000,
            total: 1_000_000,
            ..Default::default()
        };
        assert_eq!(
            quote_in(&c, "gpt-4o-2024-05-13", "standard", &plain, false).usd,
            Some(5.)
        );
        assert_eq!(
            quote_in(&c, "gpt-4o", "standard", &plain, false).usd,
            Some(2.5)
        );
    }
    #[test]
    fn merge_preserves_missing_models_but_uses_new_verified_rates() {
        let old = catalog();
        let mut incoming = old.clone();
        incoming.rates.retain(|k, _| k.starts_with("gpt-4o"));
        incoming
            .rates
            .get_mut("gpt-4o|standard|short")
            .unwrap()
            .input = 3.;
        let c = merge(old.clone(), incoming);
        assert_eq!(c.rates.len(), old.rates.len());
        assert_eq!(c.rates["gpt-4o|standard|short"].input, 3.);
        assert!(quote_in(&c, "gpt-6.1-sol", "standard", &usage(), false).approximate);
        assert!(!quote_in(&c, "gpt-4o", "standard", &usage(), false).approximate);
    }
    #[test]
    fn missing_cache_price_is_a_partial_subtotal_not_fabricated_zero() {
        let c = catalog();
        let u = usage();
        let q = quote_in(&c, "gpt-4o", "standard", &u, false);
        assert!(q.partial);
        assert_eq!(q.priced_tokens, 90_000);
        assert!((q.usd.unwrap() - 0.225).abs() < 1e-10);
        let q = Quote::combine([q, quote_in(&c, "unknown", "standard", &u, false)]);
        assert!(q.partial);
        assert_eq!(q.total_tokens, 220_000);
        assert_eq!(q.priced_tokens, 90_000);
        let mut invalid = u;
        invalid.cache = invalid.input + 1;
        assert!(quote_in(&c, "gpt-4o", "standard", &invalid, false)
            .usd
            .is_none());
    }
    #[test]
    fn family_fallback_refuses_conflicting_nearby_price_bands() {
        let mut c = catalog();
        let mut expensive = c.rates["gpt-6.1-sol|standard|short"].clone();
        expensive.input = 100.;
        expensive.output = 500.;
        c.rates
            .insert("gpt-6.3-sol|standard|short".into(), expensive);
        assert!(quote_in(&c, "gpt-6.2-sol", "standard", &usage(), false)
            .usd
            .is_none());
        assert!(quote_in(&c, "gpt-6.1-sol", "standard", &usage(), false)
            .usd
            .is_some());
    }
    #[test]
    fn pre_header_parser_cache_cannot_poison_offline_prices() {
        let current = catalog();
        let expected = quote_in(&current, "gpt-6.1-sol", "standard", &usage(), false).usd;
        let mut legacy = current.clone();
        legacy.schema_version = 0;
        legacy.updated_at = current.updated_at + 100;
        legacy
            .rates
            .get_mut("gpt-6.1-sol|standard|short")
            .unwrap()
            .output = 0.1;
        let safe = cached_catalog(current.clone(), legacy);
        assert_eq!(
            quote_in(&safe, "gpt-6.1-sol", "standard", &usage(), false).usd,
            expected
        );
        let mut old = current.clone();
        old.updated_at -= 100;
        old.rates
            .get_mut("gpt-6.1-sol|standard|short")
            .unwrap()
            .input = 99.;
        let safe = cached_catalog(current, old);
        assert_eq!(
            quote_in(&safe, "gpt-6.1-sol", "standard", &usage(), false).usd,
            expected
        );
    }
}
