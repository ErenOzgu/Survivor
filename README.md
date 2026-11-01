# Rnsurvivors

WoW 1.12 dünyasında geçen, haritadan haritaya ilerlenen bir *survivors* oyunu. Kendi tasarımımız;
kurallar [CLAUDE.md](CLAUDE.md), tasarım [DESIGN.md](DESIGN.md).

Oyun, açık kaynak [wenilla/benilla](https://github.com/Arnesen/wenilla) motoru (Rust + Bevy,
MIT OR Apache-2.0) üzerine kurulu. Sunucu yok; oyun mantığı tamamen istemcide çalışır.

## Durum: Aşama 2 (Savaş)

Aşama 1 (temel) bitti: motor sunucusuz açılır, kahraman WASD ile Goldshire dışında arazide
yürür, kamera açılı tepeden izler. Aşama 2 bunun üstüne oynanabilir bir savaş döngüsü kurar:

- **Düşman dalgaları.** Kahramanın etrafında, ekranın hemen dışında kobold, kurt (0:45'ten
  sonra, sürü halinde) ve örümcek (2:00'den sonra) doğar. Dalgalar zamanla sıklaşır, kalabalıklaşır
  ve güçlenir. Düşmanlar kahramana koşar, arazide yürür ve **değdikçe** hasar verir.
- **Mage, otomatik büyü.** Koşu Frostbolt ile başlar. Sahip olunan skill'ler kendi bekleme
  süreleriyle **kendiliğinden** atılır; menzilde düşman yoksa skill hazır bekler.
- **XP ve level.** Ölen düşman yeşil bir XP küresi bırakır; yaklaşınca küre kahramana uçar.
  Level atlayınca oyun durur ve **3 kart** çıkar: ya yeni bir skill ya da sahip olunan bir
  skill'in bir sonraki seviyesi. En fazla **6 skill**; en üst seviyedeki skill'e kart çıkmaz.
  Öğrenilecek bir şey kalmazsa "Can iksiri" kartı çıkar.
- **Ölüm.** Can sıfırlanınca koşu biter: haritanın başına dönülür, level, XP ve skill'ler
  sıfırlanır (DESIGN.md). Ekranda koşunun özeti birkaç saniye görünür.
- **HUD.** Üstte XP çubuğu; sol üstte level ve can çubuğu; ortada süre ve öldürülen düşman
  sayısı; sol altta skill'ler ve seviyeleri.
- Görseller şimdilik **yer tutucu**: kahraman sarı kapsül; kobold kahverengi kapsül, kurt gri
  uzun kutu, örümcek koyu mor yassı küre (sarı göz baktığı yönü gösterir); büyüler renkli
  küre/ok, yer büyüleri yarı saydam diskler. Gerçek WoW modelleri ayrı bir çalışmada
  (`game/src/models/`) geliyor; düşman görünümü tek bir fonksiyonda
  (`combat/visuals.rs: attach_enemy_visual`) toplandı, oraya bağlanacak.

### Mage skill'leri

| Skill | Ne yapar | Güçlenince (5 seviye) |
|---|---|---|
| Frostbolt | En yakın düşmana buz oku, yavaşlatır | 2, sonra 3 ok; oklar 1, sonra 3 düşmanı deler |
| Fireball | Yavaş ateş topu, çarptığı yerde patlar | Patlama alanı büyür; 5. seviyede 2 top |
| Arcane Missiles | Rastgele düşmanlara güdümlü füzeler | 3 → 8 füze |
| Frost Nova | Etraftakileri dondurur | Daha geniş, daha sık (9 → 5 sn), daha uzun donma |
| Blizzard | Yakında bir yere buz yağmuru, yavaşlatır | Süre ve alan büyür; 4. seviyede 2 yağmur |
| Flamestrike | En yakın düşmanın altında yanan alan | Yanma hasarı 4 → 15 |
| Arcane Explosion | Etrafa patlama dalgası | Hasar 8 → 28 |
| Blink | Can %35'in altındayken ya da 4+ düşman dibindeyken kendiliğinden ışınlanır, kısa süre hasar almaz | Bekleme 20 → 7 sn |

Bütün sayılar `game/src/content/` altında, bu projenin kendi tasarımı.

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
  src/content/      oyun verisi: skill'ler ve seviyeleri, düşmanlar, dalga takvimi
  src/combat/       CombatPlugin: savaş sistemleri
    rules.rs        saf kurallar: XP eğrisi, kart çekme, bekleme, hasar, Blink (birim testli)
    enemies.rs      dalga üretici, kovalama, durum etkileri (donma/yavaşlama), ölüm
    skills.rs       otomatik büyü, mermiler, güdümlü füzeler, yer alanları
    hero.rs         can, temas hasarı, ölüm ve koşunun sıfırlanması
    xp.rs           XP küreleri, toplama, level
    cards.rs        level atlama kart ekranı
    hud.rs          can/XP çubuğu, level, süre, skill listesi
    visuals.rs      yer tutucu görseller (düşman görünümü tek fonksiyonda)
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

Testler (saf hareket/kamera/ayar mantığı, savaş kuralları, oyun verisinin tutarlılığı):

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
| 1 / 2 / 3 (ya da tıkla) | Level atlayınca kart seç |

Saldırı tuşu yok: skill'ler kendiliğinden atılır. İş, düşmanların arasından sıyrılıp XP
kürelerini toplamak.

### Ortam değişkenleri

| Değişken | Örnek | Ne yapar |
|---|---|---|
| `WOW_DATA` | `C:\Games\WoW-1.12.1\Data` | WoW 1.12.1 `Data` klasörü (motorun değişkeni) |
| `RNS_START` | `-9464,62` veya `-9464,62,60` | Başlangıç noktası, ham WoW dünya koordinatı `x,y[,z]` (Eastern Kingdoms). `z` verilmezse kahraman zemine oturur. Varsayılan `-9464,62` (Goldshire dışı) |
| `RNS_SPEED` | `14` | Kahramanın koşu hızı, yard/sn (varsayılan 7) |
| `RNS_LEVEL` | `10` | Test: koşu bu level'da başlar; aradaki her level için kart ekranı sırayla açılır |
| `RNS_SKILLS` | `all` veya `frostbolt:3,blizzard,blink:5` | Test: başlangıç skill'leri. `all` = 8 skill'in hepsi en üst seviyede (6 sınırı yok sayılır). Liste: `anahtar[:seviye]`; anahtarlar `frostbolt fireball missiles nova blizzard flamestrike explosion blink` |
| `RNS_GOD` | `1` | Test: kahraman hasar almaz |
| `RNS_SURVEY` | `1` | Teşhis: eksik bir motor kaynağı paniğe değil uyarıya döner |
| `WOW_WIN` | `1280x720` | Pencere boyutu (motorun değişkeni, varsayılan 1600x900) |
| `WOW_NOVSYNC` | `1` | VSync kapalı (motorun değişkeni) |

## Bilinen eksikler

- **Gerçek WoW verisiyle ve ekran kartıyla henüz oynanmadı.** Bulut ortamında ne WoW verisi ne
  GPU var; yalnızca derleme ve birim testleri yapıldı. Denge (hasar, can, dalga hızı) kâğıt
  üstünde ayarlandı, ilk oyunda düzeltme isteyecektir.
- **Arayüz fontu WoW'dan okunur** (`Fonts\ARIALN.TTF`, Türkçe harfleri var). WoW verisi yoksa
  yazılar görünmez (kutular görünür).
- Düşmanlar yalnızca arazi yüksekliğini takip eder; binaların ve ağaçların içinden geçer.
- Elit gnoll şefi ve Defias boss'u veri olarak hazır (`content/enemies.rs`, `content/waves.rs`)
  ama henüz doğmuyor (Aşama 3). Gnoll şefinin noktası yaklaşık, oyunda kontrol edilecek.
- Ses yok, hasar sayıları yok, düşman can çubukları yok.

### Aşama 1'den kalanlar

- **Kahraman modeli yer tutucu.** Motorun yaratık/karakter çizimi (görünüm kimliği, deri, iskelet
  animasyonu) `benilla-app` tarafında, sunucu birimlerine bağlı. `benilla-world`'ün düşük seviyeli
  model API'si (`spawn_model_entities`) bunun için çok ham; gerçek model sonraki aşamada.
- **Yalnızca arazi yüksekliği takip ediliyor.** Binalar (WMO) ve çarpışma henüz yok; kahraman
  binaların içinden geçer, köprülerin altından yürür.
- Dünya kameranın etrafında akar (motorun `ViewFocus::camera()` varsayılanı); kamera kahramana
  yakın olduğu için bu yeterli.
