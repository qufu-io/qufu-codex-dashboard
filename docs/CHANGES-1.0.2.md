# 1.0.2 — Task accounting and usage history

## Same-version refresh / Aynı sürüm yenilemesi

The refreshed 1.0.2 removes the Compact view setting and always fits the dashboard to its content on startup and after panel changes. Legacy appearance preferences migrate without losing theme, language, transparency or selected session. A new screenshot uses illustrative data only. Existing 1.0.2 users must download/reinstall manually; the updater intentionally offers only higher versions. The original source tag is preserved; the release notes identify the refreshed source commit and include its source archive.

Yenilenen 1.0.2'de Compact view seçeneği kaldırıldı; pencere ilk açılışta ve panel değişimlerinde içeriğe uyar. Tema, dil, şeffaflık ve session seçimi korunur. Yeni ekran görüntüsü yalnızca örnek veri kullanır. Zaten 1.0.2 kullanıyorsan kurulumu yeniden indir; aynı sürüm otomatik güncelleme olarak sunulmaz. Eski kaynak etiketi korunur; yeni kaynak commit'i ve arşivi yayın notlarında belirtilir.

## English

**Understand a whole task, not just its final response.** Last Request now follows the user-task turn and prices each response with its recorded model and service tier. Active, completed and interrupted states remain distinct; missing measurements stay unknown or partial.

### What changed

- Token counters follow official Codex definitions: Input includes Cache, Output includes Reasoning, Raw Total is Input + Output, and CLI Used is Input − Cache + Output. Context uses the official 12,000-token baseline and rounding.
- Click Last Request, Context, the five-hour limit or the weekly limit to open a small history window. Task bars offer CLI Used, raw tokens or estimated USD. Context points require a measured end-of-task snapshot; missing periods are not filled in.
- Quota calibration learns nonnegative uncached-input, cached-input and output weights from observed five-hour percentage changes. Weekly capacity shares these weights and calibrates a conversion ratio, starting with an explicitly assumed 0.15. Capacities use a fixed uncached-input-equivalent unit, not a claimed official token allowance.
- Existing local history is preserved by an additive database migration with a consistent backup. Recoverable task facts are imported; unavailable measurements are not invented.
- Standalone builds embed the frontend instead of requiring a localhost development server.

### Installation and updates

Download the Windows x64 installer, Linux amd64 Debian package or AppImage from [Release 1.0.2](https://github.com/qufu-io/qufu-codex-dashboard-releases/releases/tag/v1.0.2). Existing users can check Settings → Check for updates. A newer signed package requires an explicit install action. Windows portable remains available; it requires WebView2 already installed.

### Accuracy and limitations

Official account percentages are not replaced by predictions. USD remains an API-equivalent estimate, not a subscription bill. Quota fitting excludes unreliable intervals and reports empirical errors; sparse/collinear data cannot establish independent weights. Other-device/cloud usage cannot be reconstructed, and an estimated capacity change does not prove an OpenAI backend change. Native Linux desktop behavior and clean-host installation remain environment-dependent acceptance checks.

Local verification: 61 Rust tests, 29 Playwright tests and 36 available official-measurement comparisons across six accessible local sessions passed. Three inaccessible rollouts were skipped. A copied legacy database retained all 13,989 usage rows and 32,790 quota snapshots. Actual packaged Windows main-window loading was verified with the development server unavailable. Platform CI results and source provenance are recorded with the published release.

## Türkçe

**Son yanıtı değil, görevin tamamını takip et.** Last Request artık kullanıcı görev turunun tüm tüketimini gösterir. Her yanıt kendi modeli ve servis tarifesiyle fiyatlanır; devam eden, tamamlanan ve kesilen görevler ayrılır. Eksik veri sıfıra dönüştürülmez.

- Token ve Context hesapları resmî Codex tanımlarına uyarlandı. Cache, Input'un; Reasoning, Output'un alt kümesidir. CLI Used = Input − Cache + Output. Context, 12.000 token baseline düzeltmesini ve resmî yuvarlamayı kullanır.
- Last Request, Context, beş saatlik ve haftalık alanlara tıklayarak küçük geçmiş grafiklerini açabilirsin. Yalnızca gerçekten ölçülmüş veriler gösterilir.
- Beş saatlik kota modeli gerçek yüzde değişimlerinden N/C/O ağırlıklarını öğrenir. Haftalık model aynı ağırlıkları ve veriyle kalibre edilen oranı kullanır; ilk oran 0,15 varsayımıdır. Kapasite, sabit cache dışı input eşdeğerinde bir tahmindir; resmî token kotası değildir.
- Yerel veritabanı yedeklenerek genişletilir; geçmiş silinmez. Geri kazanılabilen görev bilgileri içeri alınır.
- Bağımsız Windows derlemesinin localhost gerektirmesi düzeltildi.

[1.0.2 sürümünden](https://github.com/qufu-io/qufu-codex-dashboard-releases/releases/tag/v1.0.2) platformuna uygun paketi indir veya Ayarlar → Güncellemeleri kontrol et seçeneğini kullan. USD abonelik faturası değildir. Başka cihazlardaki kullanımın etkisi tam ayrılamadığı için tahmini kapasite değişimi, OpenAI'ın kotayı değiştirdiğinin kesin kanıtı olarak sunulmaz.

Yerel doğrulamada 61 Rust testi, 29 Playwright testi ve erişilebilir altı session'daki 36 ölçüm karşılaştırması geçti. Erişilemeyen üç rollout atlandı. Veritabanı kopyasında 13.989 kullanım kaydı ve 32.790 kota snapshot'ı korundu. Windows EXE'sinin localhost kapalıyken açıldığı doğrulandı. Platform CI sonuçları yayın notlarında belirtilir.

Author / Yazar: [alper@qufu.io](mailto:alper@qufu.io). MIT. [Measurement methodology](MEASUREMENTS.md) · [Privacy](PRIVACY.md).
