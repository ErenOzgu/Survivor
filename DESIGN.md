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

Skill listeleri, sayılar ve skill'lerin nasıl güçlendiği bu projede tasarlanır (bkz. CLAUDE.md §2).

## Açık sorular

- Kaç harita, hangi bölgeler?
- İlk sınıf hangisi olsun?
- Mini boss'lar map boss'unu etkiliyor mu (örn. hepsi kesilirse boss zayıflar)?
- Gear slotları ve drop nadirlikleri

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
