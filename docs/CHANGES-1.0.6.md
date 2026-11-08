# Qufu Codex Dashboard 1.0.6

Author / Yazar: alper@qufu.io · MIT · Windows & Linux

## English
- Keep the full quota token fraction visible at 300 px, with Learning status on its own side of the row. English K/M/B suffixes are consistent: B means one billion tokens.
- Add local per-task notices for unusually high CLI Used or raw volume, comparing up to 20 preceding complete tasks. Cached volume is not described as an official quota charge.
- Explain CLI Used (Input − Cache + Output) and Raw Total (Input + Output) in task charts. Reasoning is already included in Output.
- Fix duplicate task consumption when legacy and modern records describe the same response without a cumulative turn snapshot.
- Preserve the last verified response across legacy counter resets; mark uncertain coverage and cost as partial.

**Update:** In the application select **Check for updates → Download and install**. Signed updates are supported from 1.0.4/1.0.5. Windows users can also use the x64 setup executable; Linux packages are DEB and AppImage. Older legacy-endpoint versions need a manual install.

**History limitation:** Original historical records are preserved. Previously saved task-chart measurements are not retroactively recomputed. The current task and comparison baseline are rebuilt from rollouts after restart; quota-learning token evidence was not affected by these task-only accounting bugs. Usage notices are local heuristics, not official quota or billing measurements. No raw account identifiers or conversations are uploaded.

## Türkçe
- 300 px genişlikte kota token miktarı tamamen görünür; Learning ayrı alanda durur. İngilizce K/M/B kısaltmaları tutarlıdır: B, bir milyar token demektir.
- Önceki en çok 20 tamamlanan görevle karşılaştırarak olağandışı CLI Used veya ham hacim için yerel uyarı eklendi. Cache hacmi resmî kota harcaması gibi sunulmaz.
- Grafiklerde CLI Used (Input − Cache + Output) ve Raw Total (Input + Output) açıklandı. Reasoning zaten Output içindedir.
- Eski ve yeni kayıt aynı yanıtı temsil ettiğinde görev tüketiminin iki kez sayılması düzeltildi.
- Eski sayaç sıfırlandığında doğrulanan son yanıt korundu; belirsiz kapsam ve maliyet kısmi işaretlendi.

**Güncelleme:** Uygulamada **Güncellemeleri kontrol et → İndir ve kur**. 1.0.4/1.0.5 üzerinden imzalı güncelleme desteklenir. Windows için x64 kurulum dosyası, Linux için DEB veya AppImage kullanılabilir. Eski güncelleme deposuna bağlı sürümler elle kurulmalıdır.

**Geçmiş sınırı:** Özgün geçmiş kayıtları korunur. Önceden kaydedilmiş görev grafikleri geriye dönük yeniden hesaplanmaz. Güncel görev ve karşılaştırma verisi açılışta rollout'lardan yeniden okunur; kota öğreniminin token verileri bu görev hesabı hatalarından etkilenmemiştir. Uyarılar yerel kullanım kurallarıdır; resmî kota veya fatura ölçümü değildir. Hesap kimliği veya konuşma içeriği dışarı yüklenmez.
