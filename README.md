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
- Kahraman gerçek bir WoW 1.12 karakter modeli: varsayılan olarak **Troll kadın Mage**, robe
  giymiş ve elinde asa. Hareket ederken koşar, dururken bekleme animasyonu oynar, gittiği yöne
  bakar. Model yüklenene kadar (ya da yüklenemezse) eski yer tutucu görünür: sarı kapsül +
  baktığı yönü gösteren kırmızı burun. Ayrıntılar: "Modeller" bölümü.

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
  src/models/       karakter ve yaratık modelleri (oyun mantığından bağımsız katman)
    mod.rs          API: ModelRequest, ModelStatus, ModelAnim, ModelsPlugin
    build.rs        isteği çözer, modeli, iskeleti, paleti ve eşyaları kurar
    anim.rs         animasyon durumları ve sürücüsü (birim testli seçim mantığı)
    look.rs         ırk/cinsiyet/görünüm/ekipman, RNS_HERO_* okuma (birim testli)
    gear.rs         varsayılan mage robe + asa seçimi (birim testli)
    paths.rs        model yolları ve URL'ler (birim testli)
    catalog.rs      istemci tablolarını (DBC) bir kez yükler
    test_spawn.rs   test yaratıkları ve hata ayıklama tuşları
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
| F8 | Kahramanın çevresine test yaratıkları çıkar (bir kez) |
| F9 | Test yaratıklarının animasyonunu sıradakine geçir (idle → walk → run → cast → attack → hit → death) |
| F10 | Kahramana büyü atma animasyonu oynat |

### Ortam değişkenleri

| Değişken | Örnek | Ne yapar |
|---|---|---|
| `WOW_DATA` | `C:\Games\WoW-1.12.1\Data` | WoW 1.12.1 `Data` klasörü (motorun değişkeni) |
| `RNS_START` | `-9464,62` veya `-9464,62,60` | Başlangıç noktası, ham WoW dünya koordinatı `x,y[,z]` (Eastern Kingdoms). `z` verilmezse kahraman zemine oturur. Varsayılan `-9464,62` (Goldshire dışı) |
| `RNS_SPEED` | `14` | Kahramanın koşu hızı, yard/sn (varsayılan 7) |
| `RNS_SURVEY` | `1` | Teşhis: eksik bir motor kaynağı paniğe değil uyarıya döner |
| `WOW_WIN` | `1280x720` | Pencere boyutu (motorun değişkeni, varsayılan 1600x900) |
| `WOW_NOVSYNC` | `1` | VSync kapalı (motorun değişkeni) |
| `RNS_HERO_RACE` | `troll`, `human`, `orc`, `dwarf`, `nightelf`, `undead`, `tauren`, `gnome` veya `1`–`8` | Kahramanın ırkı (varsayılan `troll`) |
| `RNS_HERO_SEX` | `female` / `f` / `1`, `male` / `m` / `0` | Kahramanın cinsiyeti (varsayılan `female`) |
| `RNS_HERO_LOOK` | `2,1,3,0,0` | Görünüm: `ten,yüz,saç_stili,saç_rengi,sakal` (karakter yaratma ekranındaki sıra numaraları; eksik olanlar 0) |
| `RNS_HERO_GEAR` | `chest:12345,mainhand:472` veya `none` | Giyilen eşyalar, `yuva:ItemDisplayInfo_id` çiftleri. Yuvalar: `head`, `shirt`, `chest`/`robe`, `belt`, `legs`, `feet`, `wrist`, `hands`, `tabard`, `mainhand`/`staff`. Verilmezse varsayılan mage ekipmanı |
| `RNS_TEST_CREATURES` | `1` veya `1234,5678` | Kahramanın çevresine test yaratıkları çıkar: `1` = kobold, kurt, örümcek, gnoll; sayı listesi = o `CreatureDisplayInfo` id'leri |

## Modeller

`game/src/models/` modülü motorun **public** `benilla-world` / `benilla-assets` /
`benilla-formats` API'si üzerine kurulu ince bir katman. `benilla-app` kullanılmıyor: oradaki
birim çizimi tamamen `pub(crate)` ve sunucu birimlerine bağlı. Motor dosyalarına dokunulmadı.

- **Karakter:** ırk + cinsiyet → `ChrRaces.dbc` gövde görünümü → `CreatureDisplayInfo` →
  `Character\<Irk>\<Cinsiyet>\...` modeli. Deri, yüz, saç ve giyilen eşyaların dokuları
  `CharSections.dbc` ile tek bir gövde dokusunda birleştirilir; saç/sakal/robe eteği gibi geoset'ler
  `CharHairGeosets` / `CharacterFacialHairStyles` / `ItemDisplayInfo` ile seçilir. Silah sağ el
  bağlantı noktasına (attachment 1) takılır.
- **Yaratık:** `CreatureDisplayInfo.dbc` id'si ile. Deri varyasyonları uygulanır; karakter modelli
  NPC'ler (Defias vb.) `CreatureDisplayInfoExtra`'daki görünüm ve hazır gövde dokusuyla çizilir.
- **Animasyon:** oyun `ModelAnim` bileşenine durum yazar: `Idle`, `Walk`, `Run`, `Death` sürekli;
  `Cast`, `Attack`, `Hit` tek seferlik (`ModelAnim::play`). Her durum için modelde olan ilk
  animasyon seçilir (ör. Walk yoksa Run). Yürüme/koşu hızı modelin kendi adım hızına göre
  ayarlanır, ayak kayması olmaz.

Savaş kodu için kullanım:

```rust
commands.spawn((
    Transform::from_translation(ayak),   // model −Z yönüne bakar
    Visibility::default(),
    models::ModelRequest::Creature { display_id },
    models::ModelAnim::default(),
));
// sonra: anim.state = AnimState::Run;  anim.play(AnimState::Attack);
```

### Kahramanın ekipmanı

`RNS_HERO_GEAR` verilmezse robe ve asa **istemcinin kendi tablolarından** seçilir:

1. Önce `CharStartOutfit.dbc`'deki Mage başlangıç kıyafeti, kahramanın ırkı ve cinsiyeti için
   (Troll Mage için bu Apprentice's Robe ve Bent Staff olmalı). Robe (InventoryType 20/5) ve
   silah (13/17/21) alınır, diğer parçalar alınmaz.
2. O ırk 1.12'de Mage olamıyorsa (ör. Tauren) eksik parça `ItemDisplayInfo.dbc`'den biçimine
   göre seçilir: robe bitli (`geosetGroup[2]`) ve üst gövde + üst bacak dokusu olan en küçük id;
   modeli `Stave_*` olan en küçük id.

Seçilen id'ler, model ve doku adlarıyla birlikte başlangıçta konsola yazılır:
`rnsurvivors: hero wears Chest display …`. Başka bir görünüm denemek için bu id'leri
`RNS_HERO_GEAR` ile değiştir.

### Modelleri test etmek

```powershell
$env:WOW_DATA = "C:\Games\WoW-1.12.1\Data"
$env:RNS_TEST_CREATURES = "1"
.\target\release\rnsurvivors.exe
```

Ekranda görülmesi gerekenler:

- Robe giymiş, elinde asa tutan bir Troll kadın; dururken nefes alıp bekler, WASD ile koşar ve
  gittiği yöne döner. F10 büyü atma animasyonunu oynatır.
- Zemin yüklenince kahramanın 6 yard çevresinde, ona dönük bir kobold, kurt, örümcek ve gnoll.
  F9 hepsinin animasyonunu sırayla değiştirir. Konsolda hangi display id'lerin seçildiği yazar.
- Model yüklenemezse sarı kapsül kalır ve konsolda nedeni yazar (`model … failed: …`).

Başka ırk denemek: `$env:RNS_HERO_RACE = "undead"; $env:RNS_HERO_SEX = "m"`.

## Bilinen eksikler

- **Modeller gerçek veriyle henüz görülmedi.** Bulut ortamında WoW verisi ve GPU yok; burada
  derleme, birim testleri ve verisiz bir açılış denemesi (kapsüle düşüyor) yapıldı. Ekranda doğru
  görünüp görünmediği Windows'ta gerçek `Data` ile denenecek.
- Modellerde henüz olmayanlar: parçacık efektleri (ör. silah parıltısı), billboard kartları,
  pelerin, kapanan el (silahı tutan parmaklar açık kalabilir), su dalgası, belirme/kaybolma
  geçişi, fare ile seçme.
- **Yalnızca arazi yüksekliği takip ediliyor.** Binalar (WMO) ve çarpışma henüz yok; kahraman
  binaların içinden geçer, köprülerin altından yürür.
- Dünya kameranın etrafında akar (motorun `ViewFocus::camera()` varsayılanı); kamera kahramana
  yakın olduğu için bu yeterli.
