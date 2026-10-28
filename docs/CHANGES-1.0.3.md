# 1.0.3 — Independent quota calibration and a persistent footer

## English

Five-hour and weekly quota capacity now learn independently. Each window fits nonnegative N/C/O weights against its own observed percentage changes, validates on its own earlier reset windows, and derives its own uncached-input-equivalent capacity. There is no weekly/5-hour conversion ratio or shared learned coefficients. Weekly estimates can be learned without five-hour snapshots. Missing weekly evidence remains unknown; it is not filled from the five-hour model.

The **Version** row, **Check for updates**, GitHub link and centered **qufu.io / alper@qufu.io** footer remain visible when Settings is closed. Existing settings and content-fitted window sizing are preserved.

Historical shared-model fits remain stored and clearly labeled. Graph lines do not join different calibration methods. Official account percentages remain unchanged; inferred capacities are not official OpenAI token quotas. USD remains an API-equivalent estimate, not your subscription bill.

Local verification: 61 Rust and 32 Playwright tests passed, including independent synthetic weights, cross-window isolation, weekly restart/history preservation, always-visible update controls and English/Turkish behavior. Platform CI/signature results are recorded in the published release.

## Türkçe

Beş saatlik ve haftalık kapasite ayrı hesaplanır. Her pencere kendi yüzde değişimleriyle N/C/O katsayılarını öğrenir, kendi reset pencerelerinde doğrulanır ve kendi cache dışı input eşdeğeri kapasitesini üretir. Haftalık/beş saatlik dönüşüm oranı ve ortak öğrenilmiş katsayı yoktur. Haftalık veri yetersizse beş saatlik modelden kapasite uydurulmaz.

Settings kapalıyken de **Sürüm**, **Güncellemeleri kontrol et**, GitHub bağlantısı ve ortalanmış **qufu.io / alper@qufu.io** alt bilgisi görünür. Ayarlar ve otomatik pencere boyutlandırması korunur. Eski geçmiş silinmez; yöntem değişimleri grafiklerde tek çizgiyle birleştirilmez.

Yerel doğrulamada 61 Rust ve 32 arayüz testi geçti. Resmî hesap yüzdeleri değiştirilmez; token eşdeğeri kapasite tahmindir. USD abonelik faturası değildir.

[Downloads](https://github.com/qufu-io/qufu-codex-dashboard-releases/releases/tag/v1.0.3) · [Measurement methodology](MEASUREMENTS.md) · [Privacy](PRIVACY.md)

Author / Yazar: [alper@qufu.io](mailto:alper@qufu.io). MIT.
