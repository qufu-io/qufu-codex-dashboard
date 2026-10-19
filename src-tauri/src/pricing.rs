use crate::data::Usage;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
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
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Catalog {
    pub updated_at: i64,
    pub source: String,
    pub rates: BTreeMap<String, Rate>,
}
#[derive(Clone)]
struct State {
    catalog: Catalog,
    origin: String,
    checking: bool,
}
static STATE: OnceLock<RwLock<State>> = OnceLock::new();
fn state() -> &'static RwLock<State> {
    STATE.get_or_init(|| {
        RwLock::new(State {
            catalog: serde_json::from_str(include_str!("../prices.json")).unwrap_or_default(),
            origin: "bundled".into(),
            checking: false,
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
fn rate(c: &[&str]) -> Option<Rate> {
    if c.len() == 4 {
        Some(Rate {
            input: money(c[0])?,
            cache: money(c[1]),
            write: money(c[2]),
            output: money(c[3])?,
        })
    } else if c.len() == 3 {
        Some(Rate {
            input: money(c[0])?,
            cache: money(c[1]),
            write: None,
            output: money(c[2])?,
        })
    } else {
        None
    }
}
pub fn parse(markdown: &str) -> Result<Catalog, String> {
    let mut book = Catalog {
        updated_at: chrono::Utc::now().timestamp(),
        source: URL.into(),
        rates: BTreeMap::new(),
    };
    let mut tier = "";
    for line in markdown.lines() {
        let line = line.trim();
        if matches!(line, "Standard" | "Fast" | "Ultrafast" | "Flex" | "Batch") {
            tier = match line {
                "Standard" => "standard",
                "Fast" => "fast",
                "Ultrafast" => "ultrafast",
                "Flex" => "flex",
                _ => "batch",
            };
            continue;
        }
        if !line.starts_with('|') || tier.is_empty() {
            continue;
        }
        let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        let Some(index) = cells.iter().position(|s| {
            s.starts_with("gpt-")
                || s.starts_with("o1")
                || s.starts_with("o3")
                || s.starts_with("o4")
        }) else {
            continue;
        };
        let model = cells[index].split_whitespace().next().unwrap_or("");
        let values = &cells[index + 1..];
        if values.len() == 8 {
            if let Some(r) = rate(&values[..4]) {
                book.rates.insert(key(model, tier, false), r);
            }
            if let Some(r) = rate(&values[4..]) {
                book.rates.insert(key(model, tier, true), r);
            }
        } else if let Some(r) = rate(values) {
            book.rates.insert(key(model, tier, false), r);
        }
    }
    if book
        .rates
        .keys()
        .filter(|k| k.ends_with("|standard|short"))
        .count()
        < 3
    {
        return Err("Unrecognized price table".into());
    }
    Ok(book)
}
pub fn cost(model: &str, tier: &str, usage: &Usage, long: bool) -> Option<f64> {
    if usage.cache + usage.write > usage.input {
        return None;
    }
    let tier = match tier {
        "" | "default" | "standard" => "standard",
        "priority" | "fast" => "fast",
        v => v,
    };
    let guard = state().read().ok()?;
    let r = guard.catalog.rates.get(&key(model, tier, long))?;
    let cached = if usage.cache == 0 {
        0.
    } else {
        usage.cache as f64 * r.cache?
    };
    let written = if usage.write == 0 {
        0.
    } else {
        usage.write as f64 * r.write?
    };
    Some(
        ((usage.input - usage.cache - usage.write) as f64 * r.input
            + cached
            + written
            + usage.output as f64 * r.output)
            / 1e6,
    )
}
pub fn info() -> Value {
    let s = state().read().unwrap();
    json!({"updatedAt":s.catalog.updated_at,"origin":s.origin,"checking":s.checking,"source":"https://developers.openai.com/api/docs/pricing"})
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
    let book = parse(&text)?;
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
        .filter(valid)
    {
        let mut s = state().write().unwrap();
        s.catalog = book;
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
