# Kaynak incelemesi — 18 Ekim 2026

Kullanıcının sonraki yönlendirmesiyle ana referans `remriel/codex-usage-counter` olarak netleştirildi. Kaynak sürümleri `.research` altındaki Git checkout’larında kayıtlıdır.

- https://github.com/Flexible-Universe/Token-Usage-Dashboard : `README.md`, `metrics.py` incelendi. Python standart kitaplıklarıyla JSON export analizi ve browser arayüzü sunuyor; native status-line uygulamasının doğrudan temeli değil. Export/toplama yaklaşımı karşılaştırıldı.
- https://github.com/capisoft-lib/codex_usage : `src/analyzer.mjs`, `src/usage-collector.mjs`, `public/usage-pricing.js`, `public/pricing-catalog.js` ve Windows pencere testleri incelendi. Gözlenen service-tier ile fiyatlandırma ve `thread_settings_applied` davranışı uygulandı; açıkça bilinen Standard/Fast/Ultrafast fiyatları desteklenir. Collector, cache-write ve bilinmeyen fiyat davranışı karşılaştırıldı. Yerel ana sohbet filtrelemesi ve Desktop seçimi için hazır bir çözüm sunmuyor. AGPL kaynak kodu doğrudan kopyalanmadı; veri/protokol yaklaşımı bağımsız Rust uygulamasında değerlendirildi.

- https://github.com/zhang-mengjia/codex-usage-dashboard : HTTP 404; erişilemedi. Kullanıcıdan güncel bağlantı istendi.
- https://github.com/itvincent-git/codex-usage-desktop : `src-tauri/src/scanner.rs`, `project_sessions.rs`, `codex_limits.rs` incelendi. Yerel JSONL okuma, token ayrıştırma, proje metadata’sı, Tauri tray/limit yaklaşımı referans alındı. Bu depodaki geniş dashboard yerine kullanıcının kompakt pencere isteği uygulandı. Arşivleri topluca tarayan yaklaşım doğrudan benimsenmedi.
- https://github.com/remriel/codex-usage-counter : `account_usage.py` ve `codex_usage_counter.py` incelendi. Kurulu Codex CLI üzerinden stdio `initialize` → `initialized` → `account/rateLimits/read` yaklaşımı Rust ile yeniden uygulandı. Credential dosyası okunmaz; yalnızca hesabın limitleri istenir. Ağ hatasında tarihli yerel snapshot kullanılır.
- https://github.com/openai/codex : `codex-rs/protocol/src/protocol.rs` içindeki `TokenUsage`, `TokenUsageRecord`, `SessionSource` veri sözleşmeleri incelendi. Cache/read/write Input’un, reasoning Output’un alt kümesidir. Yeni response kayıtları ve eski token_count sayaçları aynı değildir.
- https://developers.openai.com/api/docs/pricing : 18.10.2026 standart kısa/uzun context fiyatları kaynak alındı. Bilinmeyen modeller/tarifeler fiyatlandırılmaz. Abonelik gerçek faturası değildir. 1.0 sürümünde resmi Markdown tablo her açılışta indirilir; hata halinde son geçerli önbellek korunur.
- https://v2.tauri.app/plugin/autostart/ ve https://v2.tauri.app/learn/system-tray/ : resmi masaüstü API sözleşmeleri kullanıldı.

## Gerçek verinin ortaya çıkardığı farklar

DB `title` alanı bazen uzun ilk prompt’tur; Desktop görünür adı `name` alanıdır. Proje taşınmış olsa da `thread-project-assignments` ile grup korunur. Guardian kaynakları ana session listesine alınmaz; session_meta.parent_thread_id ve thread_spawn_edges ile ilişkilendirilir. Parent ilişkisi tek başına bağımsız kullanıcı sohbetini subagent yapmaz.

Eski/kompakt edilmiş rollout’un ilk response kaydı önceki toplamı taşıyabilir. Bu başlangıç toplamı korunur; model geçmişi yoksa USD hesaplanmaz ve eksiklik açıklanır. Response ve event sayaçları birbirinden çıkarılmaz. Benzersiz response_id kayıtları işlenir; timestamp’i oluşturulmadan önceye ait kopya geçmiş atlanır.

Desktop kalıcı durumunda gerçekten seçili session’ı doğrulayan bir alan bulunamadı. Beta, çalışma/son güncelleme zamanını seçim gibi kullanmaz ve manuel seçimi korur. Cloud ve ChatGPT sohbetlerinin yerel token dökümü olmadığı için yerel Codex ana sohbet listesi hedeflenir.

Kullanıcının söz ettiği screenshot/logo henüz eklenmedi. Sol menü için bağımsız `list_threads` çıktısı karşılaştırma referansı olarak alındı; görsel eşleşme testi yerine geçtiği iddia edilmez.
