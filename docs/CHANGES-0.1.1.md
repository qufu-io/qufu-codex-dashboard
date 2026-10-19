# 0.1.1

- Başlık: Qufu logosu | Qufu Codex Dashboard | Workspace - Session.
- Uygulama/tray simgesi ve başlıktaki logo, kullanıcının sağladığı `icon_transparent.png` dosyasından alınır. Alpha korunur; logo yeniden çizilmez.
- Arama alanı kaldırıldı; listede workspace ve session adı birlikte gösteriliyor.
- Hız: son 60 saniyedeki token / 60, Tokens/sec olarak sunulur.
- Ayarlar için çark simgesi; subagent satırının altına ayırıcı.
- Yeni kurulum varsayılan opaklık %35. Önceki sürümün varsayılan %100 değeri bir defaya mahsus %35’e geçirilir; özelleştirilmiş opaklık korunur.
- Ayarlar/pin değişikliklerinde sadece değişen native tercih uygulanır. Veri yenileme hiçbir pencere ayarı uygulamaz. Panel açılıp kapanırken önceki yükseklik, mevcut genişlik, DPI ve pencerenin fiziksel konumu korunur.
- Taze hesap RPC yanıtı Live olarak gösterilir. Eski veri bağlantı bekleniyor/son veri olarak işaretlenir; veri tazeliği her saniye kontrol edilir.

Doğrulama: 12 Rust testi ve 4 sentetik Playwright akışı PASS. Native pencere işlemleri derlendi; konum regresyon testleri sentetik IPC çağrılarını doğrular, gerçek Windows ekran etkileşimi testi değildir.
