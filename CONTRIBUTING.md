# Contributing / Katkı Rehberi

## English

Qufu Codex Dashboard is an MIT-licensed desktop application maintained by [qufu-io](https://github.com/qufu-io), contact **alper@qufu.io**. Code, tests, documentation and Turkish/English translations are welcome. Follow the [Code of Conduct](CODE_OF_CONDUCT.md).

1. Check existing issues and the [README](README.md). Use the issue forms for bugs or feature proposals; explain larger changes before implementing them.
2. Fork this source repository and create a focused branch. See [development setup](docs/DEVELOPMENT.md).
3. Add meaningful regression coverage for behavior changes. Keep unknown usage unknown, avoid counting cache or subagent consumption twice, and preserve manual selection when Desktop selection cannot be verified.
4. Run `npm run build`, `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib`, and `npm test`. Report which checks ran and any platform limitations in your PR. CI repeats these on Windows and Linux.
5. Submit a PR with the problem, behavior change and actual verification. Contributions are provided under the project's MIT license. Preserve third-party notices and disclose reused code and its license.

Use synthetic fixtures. Never commit authentication files, signing keys, local Codex databases, session logs, private paths or conversations. Screenshots must be sanitized. No credentials are needed for normal build/test; release signing belongs to maintainers. Report vulnerabilities privately using [SECURITY.md](SECURITY.md).

## Türkçe

Qufu Codex Dashboard MIT lisanslıdır. Proje sahibi [qufu-io](https://github.com/qufu-io), iletişim **alper@qufu.io**. Kod, test, dokümantasyon ve Türkçe/İngilizce çeviri katkıları kabul edilir. [Davranış Kuralları'na](CODE_OF_CONDUCT.md) uy.

1. Mevcut issue'ları ve [README'yi](README.md) kontrol et. Hata ve özellikler için formları kullan; büyük değişiklikleri uygulamadan önce önerini açıkla.
2. Kaynak depoyu fork edip odaklı bir dal oluştur. [Geliştirme kurulumunu](docs/DEVELOPMENT.md) izle.
3. Davranış değişikliklerine anlamlı regresyon kapsamı ekle. Bilinmeyen tüketimi sıfıra dönüştürme; cache veya subagent tüketimini çift sayma; Desktop seçimi doğrulanamıyorsa manuel seçimi koru.
4. `npm run build`, `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib` ve `npm test` çalıştır. Gerçekte yaptığın kontrolleri ve platform sınırlamalarını PR'da belirt. CI Windows ve Linux'ta bunları tekrarlar.
5. Sorunu, değişen davranışı ve doğrulamayı açıklayan PR aç. Katkılar proje MIT lisansı altında sunulur. Üçüncü taraf bildirimlerini koru; kullanılan başka kodları ve lisanslarını belirt.

Sentetik test verisi kullan. Kimlik doğrulama dosyası, imzalama anahtarı, yerel Codex veritabanı, session logu, özel dosya yolu veya konuşma commit etme. Görselleri anonimleştir. Normal derleme/test kimlik bilgisi gerektirmez; sürüm imzalama bakımcılara aittir. Güvenlik açıklarını [SECURITY.md](SECURITY.md) üzerinden özel olarak bildir.
