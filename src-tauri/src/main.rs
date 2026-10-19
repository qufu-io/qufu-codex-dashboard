#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    if std::env::args().any(|a| a == "--prices-diagnose") {
        if let Some(path) = std::env::args().nth(2) {
            let path = std::path::PathBuf::from(path);
            let parent = path.parent().unwrap();
            qufu_dashboard::pricing::start(parent.to_path_buf());
            for _ in 0..40 {
                if !qufu_dashboard::pricing::info()["checking"]
                    .as_bool()
                    .unwrap_or(false)
                {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
            std::fs::write(
                path,
                serde_json::to_vec_pretty(&qufu_dashboard::pricing::info()).unwrap(),
            )
            .unwrap();
        }
        return;
    }
    if std::env::args().any(|a| a == "--quota-diagnose") {
        let feed = qufu_dashboard::quota::Feed::default();
        feed.start();
        let mut result = None;
        for _ in 0..60 {
            if let Some(v) = feed.snapshot() {
                result = Some(v);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        feed.stop();
        if let Some(p) = std::env::args().nth(2) {
            std::fs::write(p, serde_json::to_vec_pretty(&result).unwrap()).unwrap();
        }
        if result.is_none() {
            std::process::exit(1);
        }
        return;
    }
    if std::env::args().any(|a| a == "--diagnose") {
        match qufu_dashboard::diagnose() {
            Ok(v) => {
                if let Some(p) = std::env::args().nth(2) {
                    std::fs::write(p, serde_json::to_vec_pretty(&v).unwrap()).unwrap();
                }
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }
    qufu_dashboard::run();
}
