# Rnsurvivors

WoW 1.12 dünyasında geçen, haritadan haritaya ilerlenen bir *survivors* oyunu. Kendi tasarımımız;
kurallar [CLAUDE.md](CLAUDE.md), tasarım [DESIGN.md](DESIGN.md).

Oyun, açık kaynak [wenilla/benilla](https://github.com/Arnesen/wenilla) motoru (Rust + Bevy,
MIT OR Apache-2.0) üzerine kurulu. Sunucu yok; oyun mantığı tamamen istemcide çalışır.

## Durum: Aşama 1 (Temel)

- Motor sunucusuz, girişsiz ve arayüzsüz açılır, dünya ayarlanabilir bir başlangıç noktasında
  (varsayılan: Elwynn Forest, Goldshire'ın hemen dışı) akmaya başlar.
- Kahraman WASD (veya ok tuşları) ile arazide yürür, yer yüksekliğini takip eder.
- Kamera kahramanı arkadan-yukarıdan, açılı tepeden izler. Fare tekerleği yakınlaştırır,
  Q/E kamerayı döndürür.
- Kahraman şimdilik **yer tutucu** bir şekil (sarı kapsül + baktığı yönü gösteren kırmızı burun).
  Gerçek WoW karakter modeli sonraki aşamada (aşağıda "Bilinen eksikler").

## Dizin yapısı

```
Cargo.toml          workspace; motor git bağımlılığı olarak, tek bir commit'e sabit
Cargo.lock          motorun kendi Cargo.lock'undan başlatıldı (aynı crate sürümleri)
rust-toolchain.toml motorunkiyle aynı Rust sürümü (1.98.1)
game/               bizim crate'imiz: `rnsurvivors` binary'si
  src/main.rs       motoru başlatır (motorun world viewer'ından uyarlandı)
  src/game.rs       GamePlugin: kahraman, hareket, takip kamerası
  src/motion.rs     saf hareket/kamera matematiği (birim testli)
  src/config.rs     ortam değişkenlerini okuma (birim testli)
  src/run/          bir koşunun saf kuralları (Aşama 2 taslağı, henüz oyuna bağlı değil)
    progression.rs  XP ve level
    skills.rs       Mage skill'leri, loadout, level-up'ta 3 seçenek
    combat.rs       can, hasar/kritik, cooldown, hedef seçme, yavaşlatma/kök/yanma
    director.rs     mob dalgaları, doğma halkası, zorluk artışı, boss zamanlayıcısı
    rng.rs          tohumlu rastgele sayı üreteci (tekrar üretilebilir koşular)
```

## Motor nasıl getirildi (ve neden)

Motor **Cargo git bağımlılığı** olarak alındı, kopyalanmadı:

```toml
benilla-world = { git = "https://github.com/Arnesen/wenilla", rev = "ea514d8bb724c0571fc74183f23de7b1bbb303cf" }
```

- Sabitlenen commit: `ea514d8bb724c0571fc74183f23de7b1bbb303cf` (2026-10-31, wenilla `main`).
- Kullandığımız crate'ler: `benilla-world`, `benilla-assets`, `benilla-formats` (ve onların
  bağımlılıkları). `benilla-app` (sunucu/arayüz tarafı) kullanılmıyor.
- **Neden kopyalamadık:** `benilla-world` motorun içinde sadece kendi alt crate'lerine yol
  bağımlılığı veriyor; cargo bunları git deposu içinden sorunsuz çözüyor. Böylece depoya 50 MB
  motor kodu girmiyor ve motor güncellemesi tek satırlık bir `rev` değişikliği oluyor.
- **Tek tuzak:** cargo, bir git bağımlılığının kendi workspace'indeki `[patch.crates-io]`
  tablosunu yok sayar. Motor `wgpu-hal`'i yamalı kullanıyor (sunum takılmalarını "kayıp cihaz"
  hatasına çevirmiyor). Bu yüzden aynı yamayı bizim `Cargo.toml`'umuzda, **aynı motor
  commit'inden** tekrarladık. Motorun `kira` ve `alsa` yamaları gerekmiyor (ses yok, `kira`
  bağlanmıyor).
- `Cargo.lock` motorun kendi kilit dosyasından başlatıldı; böylece Bevy, avian, wgpu vb. motorun
  test ettiği sürümlerle aynı.
- Motor dosyalarında **hiçbir değişiklik yapılmadı**. Lisans ve yazar bilgileri motorun deposunda
  (`LICENSE-APACHE`, `LICENSE-MIT`) duruyor; `game/src/main.rs`'deki başlatma kodu motorun
  `benilla-world/src/worldview.rs` dosyasından uyarlandı (MIT OR Apache-2.0).

Motoru güncellemek: `Cargo.toml`'daki dört `rev` değerini birlikte değiştirip `cargo update`.

## Derleme

Rust kurulu olmalı ([rustup](https://rustup.rs)); doğru sürüm `rust-toolchain.toml`'dan otomatik
seçilir.

```bash
cargo build --release -p rnsurvivors
```

İlk derleme uzun sürer (Bevy + motor, 10–30 dk). Çıktı: `target/release/rnsurvivors`
(Windows'ta `target\release\rnsurvivors.exe`).

Linux'ta ek sistem paketleri gerekir:

```bash
sudo apt-get install libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev pkg-config
```

Testler (saf hareket/kamera/ayar mantığı):

```bash
cargo test -p rnsurvivors
```

## Çalıştırma (Windows)

Oyun, WoW dosyalarını **senin kendi** WoW 1.12.1 kurulumunun `Data` klasöründen okur. Bu
dosyalar projeye kopyalanmaz, depoya girmez (CLAUDE.md §3).

PowerShell:

```powershell
$env:WOW_DATA = "C:\Games\WoW-1.12.1\Data"
.\target\release\rnsurvivors.exe
```

cmd:

```bat
set WOW_DATA=C:\Games\WoW-1.12.1\Data
target\release\rnsurvivors.exe
```

`WOW_DATA` verilmezse sırayla şunlara bakılır: `<repo>\WoW\Data` (bir kurulumun bağlantısı;
git'e girmez), exe'nin yanındaki `Data` ve `WoW\Data`. Bulunamazsa konsola nerelere bakıldığı
yazılır.

### Kontroller

| Tuş | İş |
|---|---|
| W A S D / ok tuşları | Yürü (kameraya göre) |
| Fare tekerleği | Yakınlaş / uzaklaş |
| Q / E | Kamerayı döndür |

### Ortam değişkenleri

| Değişken | Örnek | Ne yapar |
|---|---|---|
| `WOW_DATA` | `C:\Games\WoW-1.12.1\Data` | WoW 1.12.1 `Data` klasörü (motorun değişkeni) |
| `RNS_START` | `-9464,62` veya `-9464,62,60` | Başlangıç noktası, ham WoW dünya koordinatı `x,y[,z]` (Eastern Kingdoms). `z` verilmezse kahraman zemine oturur. Varsayılan `-9464,62` (Goldshire dışı) |
| `RNS_SPEED` | `14` | Kahramanın koşu hızı, yard/sn (varsayılan 7) |
| `RNS_SURVEY` | `1` | Teşhis: eksik bir motor kaynağı paniğe değil uyarıya döner |
| `WOW_WIN` | `1280x720` | Pencere boyutu (motorun değişkeni, varsayılan 1600x900) |
| `WOW_NOVSYNC` | `1` | VSync kapalı (motorun değişkeni) |

## Koşu kuralları (Aşama 2 taslağı)

Bütün sayılar bu projenin kendi tasarımı (CLAUDE.md §2) ve `game/src/run/` içinde tek yerde
duruyor; değiştirmek kolay. Henüz ekranda görünmüyorlar; sıradaki iş bunları oyuna bağlamak.

- **Level:** sonraki level için gereken XP `10 + 6n + n²` (n = level − 1): 10, 17, 26, 37, 50…
  En fazla level 60. Kill başına XP: normal 2, elit 15, mini boss 60, boss 200.
- **Sınıf:** şimdilik Mage, koşuya Frostbolt ile başlıyor.
- **Skill'ler:** en fazla 4 aktif ve 2 pasif, her biri 5 rank. Her rank +%25 hasar ve −%6
  cooldown verir; skill'e özel ek: Frostbolt 4. rankta 2 mermi, Fireball 5. rankta 2 mermi,
  Arcane Missiles rank başına +1 mermi, alan skill'lerinin yarıçapı büyür.

  | Skill | Tür | Hasar | Cooldown | Etki |
  |---|---|---|---|---|
  | Frostbolt | mermi | 12 | 1.4 sn | %30 yavaşlatma, 2 sn |
  | Fireball | mermi | 20 | 2.2 sn | vuruşun %25'i 3 sn'de yanma |
  | Arcane Missiles | 3 mermi | 5 | 2.0 sn | — |
  | Frost Nova | çevre patlaması, 6 yd | 8 | 6.0 sn | 1.5 sn kök |
  | Blizzard | yer alanı, 5 yd, 4 sn | 4 / 0.5 sn | 7.0 sn | %40 yavaşlatma |
  | Arcane Explosion | çevre patlaması, 4.5 yd | 7 | 2.5 sn | — |
  | Arcane Intellect | pasif | rank başına +%8 hasar | | |
  | Presence of Mind | pasif | rank başına −%5 cooldown | | |

- **Level-up:** birbirinden farklı 3 seçenek gelir: sahip olunan skill'lerin yükseltmeleri ve boş
  slota sığan yeni skill'ler. Yükseltmelerin çıkma ağırlığı biraz daha fazla. Seçecek bir şey
  kalmayınca tek seçenek "canın %30'unu yenile".
- **Savaş:** %5 kritik şansı, kritik ×1.5 hasar. Kahramanın canı 100; bir temas vuruşundan sonra
  0.5 sn hasar almaz.
- **Mob'lar:** ilk dalga hemen gelir. Dalga aralığı 1.5 sn'den başlar, her 3 dakikada yarıya
  iner (en az 0.3 sn). Dalga başına mob sayısı dakikada +1 artar (en fazla 6). Mob canı dakikada
  +%15 artar. Mob'lar kahramanın 24–30 yarda uzağında doğar; aynı anda en fazla 150 mob olur.
  Hiçbir mob kahramandan (7 yd/sn) hızlı değildir.
- **Map boss'u:** 10. dakikada kendiliğinden gelir.

## Bilinen eksikler (Aşama 1)

- **Gerçek WoW verisiyle henüz denenmedi.** Bulut ortamında WoW verisi yok; burada yalnızca
  derleme ve birim testleri yapıldı. İlk deneme Windows'ta gerçek `Data` ile yapılacak.
- **Kahraman modeli yer tutucu.** Motorun yaratık/karakter çizimi (görünüm kimliği, deri, iskelet
  animasyonu) `benilla-app` tarafında, sunucu birimlerine bağlı. `benilla-world`'ün düşük seviyeli
  model API'si (`spawn_model_entities`) bunun için çok ham; gerçek model sonraki aşamada.
- **Yalnızca arazi yüksekliği takip ediliyor.** Binalar (WMO) ve çarpışma henüz yok; kahraman
  binaların içinden geçer, köprülerin altından yürür.
- Dünya kameranın etrafında akar (motorun `ViewFocus::camera()` varsayılanı); kamera kahramana
  yakın olduğu için bu yeterli.
