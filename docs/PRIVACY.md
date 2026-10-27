# Privacy / Gizlilik

## English

Qufu Codex Dashboard is a local desktop monitor. It has no analytics service and does not upload your conversations or usage records to a dashboard server.

### Local access and storage

The app reads Codex metadata, local SQLite state and rollout files without modifying them. `CODEX_HOME` selects the source directory; otherwise it uses `~/.codex`. Some rollout formats contain conversation text, but the dashboard's history stores identifiers, timestamps, model metadata and numeric measurements, not message bodies. Authentication token files are not read directly by the dashboard.

Preferences, cached prices and the dashboard's own `quota-learning.sqlite3` live in the operating system's application-data/config directories. The database contains account percentage snapshots, token observations, task facts and learned calibration history. Schema upgrades preserve records and back up populated older databases. Uninstalling the app may leave these files; removing them manually deletes the local preferences/history. Source Codex data is separate and should not be removed to reset the dashboard.

### Network access

| Purpose | Destination / mechanism |
| --- | --- |
| Refresh API-equivalent prices | Official OpenAI pricing documentation |
| Read account limits | Installed, signed-in Codex CLI App Server; Codex handles its authentication and network access |
| Check/download an update | Public GitHub release manifest and assets; installation requires an explicit action and a valid updater signature |
| Open external links | Your default browser, when you choose a link |

There is no dashboard account, advertising identifier or mandatory telemetry. No cloud statistics collection is implemented. Any future optional statistics feature must document its fields and consent separately before activation.

### Sharing diagnostics

Do not attach raw rollout logs, authentication files or databases to public issues. They can contain private titles, paths, identifiers or conversations. Use synthetic reproductions and sanitized screenshots. Security concerns belong in the private channel described in [SECURITY.md](../SECURITY.md).

## Türkçe

Uygulama yerel çalışır; konuşmaları veya kullanım kayıtlarını bir dashboard sunucusuna yüklemez. Codex metadata, veritabanı ve rollout dosyaları salt okunur kullanılır. Dashboard geçmişi mesaj içerikleri yerine kimlik, zaman, model ve sayısal ölçümleri saklar. Kimlik doğrulama token dosyalarını doğrudan okumaz; hesap limitlerini kurulu Codex üzerinden alır.

Ayarlar, fiyat önbelleği ve `quota-learning.sqlite3` işletim sisteminin uygulama veri/config klasörlerinde tutulur. Migration geçmişi korur ve eski dolu veritabanını yedekler. Uygulamayı kaldırmak bu dosyaları silmeyebilir; elle silmek yerel ayar ve geçmişi kaldırır. Codex'in kendi kaynak verileri ayrıdır.

Ağ erişimi resmî OpenAI fiyatları, kurulu Codex'in hesap limit okuyucusu ve GitHub sürüm kontrolü/indirmesi içindir. Güncelleme kurulumu açık kullanıcı eylemi ve geçerli imza gerektirir. Telemetri veya global istatistik toplama şu anda uygulanmamıştır; gelecekteki isteğe bağlı özellikler ayrı bilgilendirme ve izin gerektirir.

Public issue'lara ham session logu, kimlik dosyası veya veritabanı ekleme. Sentetik örnek ve anonimleştirilmiş görsel kullan. Güvenlik bildirimleri için [özel kanalı](../SECURITY.md) kullan.

Maintainer / İletişim: [alper@qufu.io](mailto:alper@qufu.io).
