# Rnsurvivors

WoW 1.12 dünyasında geçen, haritadan haritaya ilerlenen bir *survivors* oyunu. Kendi tasarımımız;
kurallar [CLAUDE.md](CLAUDE.md), tasarım [DESIGN.md](DESIGN.md).

Oyun, açık kaynak [wenilla/benilla](https://github.com/Arnesen/wenilla) motoru (Rust + Bevy,
MIT OR Apache-2.0) üzerine kurulu. Sunucu yok; oyun mantığı tamamen istemcide çalışır.

## Durum: Aşama 3 (Harita)

Aşama 1 (temel) bitti: motor sunucusuz açılır, kahraman WASD ile Goldshire dışında arazide
yürür, kamera açılı tepeden izler. Kahraman gerçek bir WoW 1.12 karakter modeli (varsayılan
**Troll kadın Mage**, robe ve asa; ayrıntılar: "Modeller" bölümü). Aşama 2 bunun üstüne
oynanabilir bir savaş döngüsü kurar:

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
- Düşman ve büyü görselleri şimdilik **yer tutucu**: kobold kahverengi kapsül, kurt gri uzun
  kutu, örümcek koyu mor yassı küre (sarı göz baktığı yönü gösterir); büyüler renkli küre/ok,
  yer büyüleri yarı saydam diskler. Düşman görünümü tek bir fonksiyonda
  (`combat/visuals.rs: attach_enemy_visual`) toplandı; gerçek yaratık modellerine
  (`game/src/models/`) oradan bağlanacak.

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

### Aşama 3: Harita (elit, mini boss'lar, map boss'u, portal)

Elwynn haritası artık bir başı ve sonu olan bir koşu:

- **Elit: Gnoll Şefi.** Fargodeep Mine girişinde (`GNOLL_CHIEF_AT`, yaklaşık nokta) bekler.
  Kahraman 40 yarda yaklaşınca dünyada belirir, 16 yarda girince (ya da ona vurunca) saldırır;
  uzaklaşırsan ve ona dokunmadıysan yerinde beklemeye devam eder. Sağ üstteki gösterge onun
  uzaklığını ve ekrana göre yönünü yazar ("Elit Gnoll Şefi: 340 yard, sol önde"). Kesilince
  büyük bir XP yağmuru bırakır. **Koşu başına bir kez**: kesilen şef ancak yeni koşuda (ölüm ya
  da portal sonrası) geri gelir.
- **Mini boss'lar** (bu projenin kendi tasarımı, `content/bosses.rs`):
  | Ne zaman | Mini boss | Görünüm (şimdilik) | Özel saldırı |
  |---|---|---|---|
  | 3:00 | **Kobold Ustabaşı**: yavaş, dayanıklı | büyük turuncu halkalı kobold | *Maden Patlayıcısı*: kahramanın üstüne ve çevresine 3 patlama dairesi |
  | 6:30 | **Gri Pençe**: sürülerin yaşlı lideri, hızlı | açık mavi halkalı kurt | *Atılış*: kahramana doğru düz bir hatta hızla atılır |
  Gelmeden 8 saniye önce ekranın ortasında geri sayımlı uyarı çıkar ("Dikkat! Mini boss: …
  yaklaşıyor… 5"), gelince "… geldi!".
- **Map boss'u: Defias Lideri.** 10:00'da kendiliğinden gelir (uyarı 9:52'de başlar). Üç özel
  saldırısı var: *Kara Barut* (5 patlama dairesi), *Kılıç Hücumu* (düz hatta atılış), *Savuruş*
  (yakındaysan çevresine vuruş).
- **Özel saldırılar hep işaretlidir:** yerde önce kırmızı bir alan (daire ya da şerit) belirir ve
  dolmaya başlar; dolunca saldırı iner. İçinden çıkan hasar almaz. Boss saldırı hazırlarken
  yürümez. Frost Nova boss'ları en fazla 0,8 sn dondurur; donukken saldırı hazırlığı bekler.
- **Can çubukları:** her elit/boss'un başının üstünde adı ve küçük bir can çubuğu var (saldırı
  hazırlarken saldırının adı da yazar). Savaşan en önemli boss'un (map boss'u > mini boss >
  elit) büyük can çubuğu ekranın üstünde.
- **Boss'un canı kahramanın level'ına göre ölçeklenir:** her boss belli bir level için
  ayarlandı (şef 7, ustabaşı 9, Gri Pençe 14, Defias Lideri 18); level başına ±%5, en az ×0,75 en
  fazla ×1,6. Temas hasarı dalgaların hasar çarpanını izler.
- **Defias Lideri kesilince** yere bir **ganimet sandığı** (altın kutu; dokununca açılır, içi
  Aşama 4'te dolacak) ve yanında dönen mavi bir **portal** çıkar. Sağ üstteki gösterge portalın
  yerini gösterir. Portala girince oyun durur ve **"Harita tamamlandı!"** ekranı koşunun
  özetini gösterir (süre, level, öldürülen düşman, kesilen elit/mini boss'lar, skill'ler).
  Sonraki harita henüz yok: Enter/Boşluk ya da tıklama ile harita baştan başlar.
- **Hızlı deneme** (PowerShell; her satır ayrı bir oyun açılışı):
  ```powershell
  # Defias Lideri 10 sn sonra; ölmeden, bütün skill'lerle
  $env:RNS_TIME = "590"; $env:RNS_SKILLS = "all"; $env:RNS_GOD = "1"
  # Mini boss'lar: 2:52'de ustabaşı uyarısı, 6:22'de Gri Pençe uyarısı
  $env:RNS_TIME = "172"
  # Gnoll Şefi: başlangıç noktasını mağara girişinin yanına al
  $env:RNS_START = "-9780,130"
  ```
  Bir değişkeni kaldırmak: `Remove-Item Env:RNS_TIME`.
- **Açık soru (uygulanmadı):** mini boss'ları kesmek map boss'unu zayıflatsın mı? DESIGN.md'de
  açık soru olarak duruyor; şimdilik aralarında hiçbir bağ yok.

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
  src/content/      oyun verisi: skill'ler ve seviyeleri, düşmanlar, dalga takvimi,
                    boss'lar ve özel saldırıları (bosses.rs)
  src/combat/       CombatPlugin: savaş sistemleri
    rules.rs        saf kurallar: XP eğrisi, kart çekme, bekleme, hasar, Blink (birim testli)
    boss_rules.rs   saf kurallar: boss zamanları, uyandırma, özel saldırı zamanlaması, boss
                    ölçeklemesi, uyarı/özet metinleri (birim testli)
    bosses.rs       elit, mini boss'lar, map boss'u; özel saldırılar ve yerdeki işaretler
    boss_hud.rs     üst boss çubuğu, başların üstündeki can çubukları, uyarılar, yön göstergesi
    portal.rs       ganimet sandığı, portal, "Harita tamamlandı" ekranı
    enemies.rs      dalga üretici, kovalama, durum etkileri (donma/yavaşlama), ölüm
    skills.rs       otomatik büyü, mermiler, güdümlü füzeler, yer alanları
    hero.rs         can, temas hasarı, ölüm ve koşunun sıfırlanması
    xp.rs           XP küreleri, toplama, level
    cards.rs        level atlama kart ekranı
    hud.rs          can/XP çubuğu, level, süre, skill listesi
    visuals.rs      yer tutucu görseller (düşman görünümü tek fonksiyonda)
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
| Enter / Boşluk (ya da tıkla) | "Harita tamamlandı" ekranını kapat, haritaya baştan başla |
| F8 | Kahramanın çevresine test yaratıkları çıkar (bir kez) |
| F9 | Test yaratıklarının animasyonunu sıradakine geçir (idle → walk → run → cast → attack → hit → death) |
| F10 | Kahramana büyü atma animasyonu oynat |

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
| `RNS_TIME` | `590` veya `9:50` | Test: koşu saati buradan başlar (saniye ya da `dk:sn`, en fazla 1 saat). Bu andan önceki mini boss'lar atlanır; `590` ile uyarı 2 sn sonra başlar, boss 10 sn sonra (10:00'da) gelir. Dalgalar da o saatin zorluğundadır, `RNS_GOD`, `RNS_LEVEL`, `RNS_SKILLS` ile birlikte kullan. Ölünce ya da portaldan sonra yine bu saatten başlanır |
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

- **Savaş gerçek WoW verisiyle ve ekran kartıyla henüz oynanmadı.** Bulut ortamında ne WoW verisi
  ne GPU var; yalnızca derleme ve birim testleri yapıldı. Denge (hasar, can, dalga hızı) kâğıt
  üstünde ayarlandı, ilk oyunda düzeltme isteyecektir.
- **Arayüz fontu WoW'dan okunur** (`Fonts\ARIALN.TTF`, Türkçe harfleri var). WoW verisi yoksa
  yazılar görünmez (kutular görünür).
- Düşmanlar yalnızca arazi yüksekliğini takip eder; binaların ve ağaçların içinden geçer.
- **Aşama 3 (elit, mini boss'lar, map boss'u, portal) gerçek WoW verisiyle oynanmadı.** Yalnızca
  derleme ve birim testleri. Gnoll şefinin noktası (`GNOLL_CHIEF_AT`) yaklaşık, oyunda kontrol
  edilecek; özel saldırıların hasarı ve boss canları kâğıt üstünde ayarlandı.
- Mini boss'lar şimdilik ödünç aldıkları normal düşmanın görünümünde (kobold, kurt); onları
  ayıran altlarındaki renkli halka ve başlarındaki ad/can çubuğu. Sandık ve portal da yer tutucu
  şekiller. Yerdeki işaretler düz diskler; eğimli arazide yer yer toprağa gömülebilir.
- Boss savaşları sırasında normal dalgalar da sürer.
- Ses yok, hasar sayıları yok; normal düşmanların can çubuğu yok (yalnızca elit ve boss'larda).
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
