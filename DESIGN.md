# Rnsurvivors: tasarım

WoW 1.12 dünyasında geçen, haritadan haritaya ilerlenen bir survivors oyunu.

## Oyun döngüsü

```
Harita ──► mob'lar doğar, karakter otomatik saldırır, XP toplanır
  │        level atlayınca 3 skill seçeneğinden biri seçilir
  ├── Elit mob'lar: haritadaki sabit noktalarda
  ├── Mini boss'lar
  └── Map boss'u: zamanlayıcıyla kendiliğinden gelir
           │
           ├── ölünce ──► haritanın başına dönülür
           └── kesilince ──► drop ──► sonraki harita açılır

Koşular arası: talent ağacı (haritada kazanılan XP ile) · boss drop'ları · gear
```

## Kararlar

| Konu | Karar |
|---|---|
| Ölüm | Haritanın başına dönülür |
| Map boss'u | Zamanlayıcıyla kendiliğinden gelir |
| Skill'ler | Her koşuda sıfırlanır |
| Kalıcı ilerleme | Haritada kazanılan XP ile talentlar açılır; boss drop'ları ve gear |
| Sınıflar | Birden çok WoW sınıfı, her birinin kendi skill havuzu (ör. Mage: Frostbolt, Fireball, Blizzard…) |
| Mini boss'lar ve map boss'u | Bağ yok: mini boss kesmek map boss'unu zayıflatmaz (gerekirse sonra eklenir) |

Skill listeleri, sayılar ve skill'lerin nasıl güçlendiği bu projede tasarlanır (bkz. CLAUDE.md §2).

## İlk sınıf: Mage

Level atlayınca 3 kart çıkar; her kart ya yeni bir skill ya da sahip olunan bir skill'in bir sonraki seviyesidir. Aynı anda en fazla **6 skill** taşınır. Skill'ler kendiliğinden atılır.

| Skill | Ne yapar | Güçlenince |
|---|---|---|
| Frostbolt | En yakın düşmana buz oku, yavaşlatır | Daha çok ok, sonra delip geçer |
| Fireball | Yavaş ama güçlü; çarptığı yerde patlar | Patlama alanı büyür |
| Arcane Missiles | Rastgele düşmanlara küçük füzeler | Füze sayısı artar |
| Frost Nova | Etraftaki herkesi dondurur | Daha sık, daha geniş |
| Blizzard | Rastgele bir yere buz yağmuru | Süresi ve alanı artar |
| Flamestrike | Yerde yanan alan bırakır | Yanma hasarı artar |
| Arcane Explosion | Etrafa patlama dalgası | Hasar artar |
| Blink | Tehlikede kendiliğinden ışınlanır | Bekleme süresi kısalır |

### Pasifler ve evrimler

Kartlar skill'lerin yanında **pasif** de getirir (en fazla 6; hasar, bekleme, hız, can, XP çekimi,
kritik, XP). İki skill en üst seviyede buluşunca **evrim** kartı çıkar: Frostbolt + Frost Nova →
Kış Okları, Fireball + Flamestrike → Pyroblast, Arcane Missiles + Arcane Explosion → Arkan Fırtına.
Sayılar ve ayrıntılar `game/src/content/` ve README'de.

### Ganimet (Aşama 4)

- 6 yuva: baş, göğüs, eller, bacaklar, ayaklar, silah. 4 nadirlik: Sıradan, Kaliteli, Nadir, Destansı.
- Eşyalar listelenmez, üretilir: nadirlik bonus sayısını (1/2/3/3) ve gücünü belirler; bonuslar
  pasiflerle aynı istatistiklerdir. Görünüm istemcinin o yuvaya uyan eşya görünümlerinden seçilir.
- Düşüş: elit ve mini boss 1 eşya (kese), map boss'u sandığı 3 eşya (daha iyi oranlar).
- Çanta 24 eşya; eşyalar koşular arası kalır.

### İlerleme (Aşama 5)

- Koşularda toplanan XP talent XP'sine eklenir; puan maliyeti artar (150 + 60 × n).
- Mage talent ağacı: Buz (savunma), Ateş (saldırı), Arkan (fayda); her kolda 3 kademe, kademe kolda
  3 ve 5 puanla açılır. Puanlar ücretsiz geri alınabilir.
- Launcher: WoW Data klasörünü bir kez sorar, ayar dosyasına yazar, oyunu açar.

## İlk harita: Elwynn (Goldshire çevresi)

- Normal mob'lar: kobold, kurt, örümcek; zamanla daha kalabalık ve güçlü dalgalar
- Elit: bir gnoll şefi, sabit bir noktada (ör. Fargodeep Mine girişi)
- Map boss'u: Defias lideri, zamanlayıcıyla gelir

## Açık sorular

- Kaç harita, hangi bölgeler?

## Teknik temel

- Motor: [wenilla/benilla](https://github.com/Arnesen/wenilla) (Rust + Bevy), Apache-2.0/MIT.
- Başlangıç noktası: `benilla-worldview`, motoru sunucu ve arayüz olmadan sadece dünyayla açar. Oyun, kendi crate'imizde bir Bevy eklentisi olarak bunun üzerine kurulur. Sunucu yok; oyun mantığı istemcide çalışır.

## Yol haritası

| # | Aşama | Bitince ne oluyor |
|---|---|---|
| 1 | Temel | Motor açılıyor, karakter küçük bir haritada WASD ile yürüyor, kamera yukarıdan |
| 2 | Savaş | Mob'lar doğuyor, otomatik saldırı, XP, level atlayınca 3 skill seçimi |
| 3 | Harita | Elit noktaları, mini boss'lar, zamanlayıcılı map boss'u, portal ve sonraki harita |
| 4 | Ganimet | Boss drop'ları, envanter, gear etkileri |
| 5 | İlerleme | Talent ağacı, kayıt, launcher |
