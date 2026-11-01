# Proje Anayasası: Rnsurvivors

Bu kurallar bu projede her işten önce gelir.

## 1. Sadece bu klasörde çalış

- Proje `Desktop/Rnsurvivors` klasörüdür. Yazma, derleme, kopyalama ve silme yalnızca bu klasörün içinde yapılır.
- Başka bir klasöre (`Desktop/AzerothSurvivors`, `Desktop/AZEROTH` vb.) ancak kullanıcı açıkça izin verirse, sadece izin verilen iş için dokunulur. Okuma izni yazma izni değildir.
- Kafana göre karar verme. Belirsizse önce sor. Kullanıcının istemediği bir değişikliği sorulmadan geri alma.

## 2. Bu oyun kendi oyunumuz, klon değil

- Rnsurvivors, Azeroth Survivors'tan (Aethus) ilham alan ama **kendi tasarımı olan** bir oyun. Aethus'un oyunundan şunlar **alınmaz**:
  - kodu (kapalı `wenilla-rogue` ve exe'den çıkarılan Lua dosyaları dahil)
  - veri tabloları (yetenek değerleri, bölge düşman listeleri, boss sıraları, talent, gear ve amulet tasarımları)
  - metinleri (`i18n_strings.txt`, sınıf açıklamaları vb.)
  - adı ve kimliği
- Yetenek listeleri, sayılar, güçlenme kuralları, haritalar ve ilerleme sistemi burada sıfırdan tasarlanır.
- Serbest olanlar:
  - açık kaynak motor (benilla/wenilla, Apache-2.0/MIT; lisans ve yazar notları korunur)
  - türün genel fikirleri
  - WoW'un dünyası, sınıfları ve büyü isimleri/görselleri (oyuncunun kendi Data klasöründen okunur, oyunla dağıtılmaz)

## 3. Oyun verisi

- WoW dosyaları (MPQ'lar) projeye kopyalanmaz ve commit edilmez. Yolu ayar ile verilir (`WOW_DATA`).
