# 1.0.4 — Account-isolated quota learning

## English

Quota learning now separates accounts using the explicit account ID returned by the official Codex rate-limit interface. The dashboard stores a SHA-256 pseudonym, not the raw account ID or email. Details shows a short local account code. Each account keeps independent five-hour/weekly observations and fitted coefficients; switching accounts does not erase the other account’s history.

Account transitions, unidentified observations and gaps over 90 seconds do not form training intervals. Old unidentified history is retained but is not assigned to an account. Initial weights treat uncached input, cached input and output equally; independent weights are learned only when sufficient diverse observations exist. At least three percentage points are accumulated to reduce integer-percent noise. A reset or decreasing percentage invalidates an interval.

Weekly-only accounts remain supported. The reported duration determines the UI lane; missing five-hour quota is not zero and does not prevent weekly learning. Missing account identity leaves official percentages visible but capacity unavailable. Local observations cannot reliably separate concurrent accounts, rapid unobserved switches or cloud/other-device usage. Session/turn totals remain the whole local conversation, not an account bill. Learned capacities stay approximate.

Source code, installers and future updates now live in **this repository**. Older versions still use the former downloads endpoint: install 1.0.4 manually before that repository is removed. The old repository and published Git history are not deleted or rewritten by this release.

## Türkçe

Kota öğrenimi artık resmî limit yanıtının hesap ID’sine göre ayrılır. Ham ID veya e-posta yerine yerel SHA-256 kodu saklanır; Details bölümünde kısa bir kod görünür. Her hesabın beş saatlik ve haftalık verisi ayrı öğrenilir. Hesap değişince önceki hesabın geçmişi silinmez.

Hesap geçişini, kimliği belirsiz okumayı ve 90 saniyeden uzun boşluğu aşan aralıklar eğitime girmez. Eski kimliksiz geçmiş korunur fakat hesaba tahminen atanmaz. Başlangıçta cache dışı Input, Cache ve Output eşit ağırlıklıdır; farklı dağılımlarda yeterli veri geldikçe ağırlıklar öğrenilir. Yuvarlama nedeniyle en az 3 yüzde puanlık artış biriktirilir. Reset veya yüzde düşüşü tüketim olarak sayılmaz.

Yalnızca haftalık kotası olan hesapta haftalık gösterge çalışır; beş saatlik kota uydurulmaz. Hesap kimliği gelmezse resmî yüzde görünür, kapasite tahmini üretilmez. Aynı anda kullanılan hesaplar veya poll arasında gözlenemeyen hızlı geçişler kesin ayrılamaz. Session toplamı aynı sohbetin tüm geçmişini kapsar; hesap faturası değildir.

Kaynak kodu, kurulumlar ve yeni güncellemeler aynı depoda toplandı. Eski sürümlerin güncelleme adresi eski depoya bağlı kaldığı için **1.0.4’ü elle indirip kur**; sonrasında güncellemeler bu depodan gelir.

[Downloads](https://github.com/qufu-io/qufu-codex-dashboard/releases/tag/v1.0.4) · [Measurements](MEASUREMENTS.md) · [Privacy](PRIVACY.md)

Author / Yazar: [alper@qufu.io](mailto:alper@qufu.io) · MIT.
