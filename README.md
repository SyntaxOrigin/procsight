# ProcSight (SüreçGöz)

Bu bilgisayarda **şu anda ne çalışıyor, nereye bağlanıyor ve kim başlangıçta
kendiliğinden açılıyor** sorularını periyodik yoklama ile toplayan, sonucu
JSONL olay günlüğü ve tek dosya HTML/CSV raporu olarak veren **salt okunur**
bir gözlem aracıdır.

ProcSight bir savunma ürünü değildir ve kendini öyle sunmaz. "Şüpheli davranış"
ifadesi kötücül yazılım tespiti değil, **anormallik gözlemidir**. Her kural
bir kanıt, bir açıklama ve "bunu nasıl doğrulayabilirsin" adımı üretir; hiçbir
kural engelleme önermez.

**Etik sınır — araç hiçbir koşulda:** bir süreci sonlandırmaz veya askıya
almaz, bir dosyayı silmez/taşımaz/değiştirmez, kayıt defterine yazmaz, hiçbir
ağ bağlantısı kurmaz, DNS çözümlemesi yapmaz, telemetri göndermez. Bu sözün
statik ve çalışma zamanı kanıtı `tests/salt_okunur_kanit.rs` dosyasındadır.

---

## Özellikler

- **Platform soyutlaması:** `InventorySource` tek arayüzün arkasına üç kaynağı
  koyar — `WindowsSource` (`tasklist`/`netstat`/`wmic`/`reg query`),
  `ProcFsSource` (`/proc` dosyaları), `PsSource` (`ps`) — ve testler için
  `FixtureSource`.
- **Süreç envanteri:** pid, üst pid, ad, komut satırı, yürütülebilir yol,
  bellek (KiB/MiB), CPU %, oturum, kullanıcı, durum, kullanıcı kimliği.
- **Ağ bağlantı tablosu:** yerel/uzak uç, protokol, durum ve sahip pid.
  DNS çözümlemesi **yapılmaz** (çevrimdışı ilkesi).
- **Başlangıç girdisi envanteri:** Windows Başlangıç klasörleri (dizin
  taraması), `HKCU`/`HKLM\...\Run` kayıtları (`reg query`, opt-in), Linux/BSD
  XDG `autostart` dizini (`.desktop`).
- **Yedi şüpheli davranış kuralı**, her biri açıklanabilir gerekçe ve
  zorunlu doğrulama adımıyla: gizli ad, şüpheli konum, yüksek bellek,
  kalıcılık girdisi, beklenmeyen ağ bağlantısı, kısa ömürlü çok sayıda süreç,
  yetki yükseltme girişimi.
- **JSONL olay günlüğü** (`olay`, `zaman`, `pid`, `kaynak`, `ayrinti`) ve
  bozuk satır toleranslı okuma — bozuk satır **sayılır**, gizlenmez.
- **HTML ve CSV dışa aktarımı.** HTML tek dosyadır ve hiçbir uzak kaynak
  içermez; CSV'de formül enjeksiyonu (`= + - @` ve TAB/CR) kaçırılır.
- **Dört alt komut:** `snapshot`, `watch`, `rules`, `report`.
- `#![forbid(unsafe_code)]`, üretim kodunda `unwrap`/`expect`/`panic!` yok.
- Etik sınırı ihlal eden saldırı işlevi (port tarama, rastgele adres üretme,
  dağıtılmış tarama, müdahale) **yoktur** ve yoktur.

### Yedi kural ve nasıl doğrulanır

| # | Kural | Ne gözlemlenir | Doğrulama adımı (kısa) |
|---|-------|----------------|--------------------------|
| 1 | gizli ad | Harf/rakam karması, kabuk-kontrol karakteri veya aşırı uzun ad | Dosya imzasını ve yayımcıyı incele |
| 2 | şüpheli konum | Geçici/gizli konumdan çalışan yürütülebilir | Hangi program buraya yazıyor, program ayarlarından gör |
| 3 | yüksek bellek | Eşiği aşan bellek kullanımı | Görev Yöneticisi ile karşılaştır, uygulama ayarından sınır koy |
| 4 | kalıcılık girdisi | Hedefi şüpheli konumda başlangıç girdisi | Girdinin hedefini aç, programın kendi kapatma seçeneğini ara |
| 5 | beklenmeyen ağ bağlantısı | Küresel adrese tanıdık olmayan porttan çıkış | Programın ağ ayarını incele, ayrı ağda tekrar gözlemle |
| 6 | kısa ömürlü çok sayıda süreç | Yoklama aralığında doğup ölen süreç kütleşmesi | O sırada güncelleme/kurulum yapıldı mı hatırla |
| 7 | yetki yükseltme girişimi | Üst süreçten farklı kullanıcı kimliği veya dünyaya açık konumdan çalışma | Üst sürecin ne olduğunu ve ayarı kimin yaptığını sor |

---

## Kurulum

Gereksinimler: Rust **1.74+** (MSRV, `rust-version = "1.74"`), Windows üzerinde
ayrıca MinGW/GNU bağlantıcısı yoktur — `cargo` varsayılan `link.exe`/MSVC
kullanır. Bu depoda doğrulama MinGW-w64 (GCC 16.2.0) ve Rust 1.98.1 ile yapıldı.

```bash
git clone https://github.com/SyntaxOrigin/procsight.git
cd procsight
cargo build --release
```

İkili `target\release\procsight.exe` (Windows) veya `target/release/procsight`
(Linux/macOS) olarak üretilir. Tek dosyadır; kurulum, kayıt defteri girdisi ve
arka planda hizmet yoktur.

İkiliyi `cargo` ile kurmak da mümkündür:

```bash
cargo install --path .
```

Bu kurulum `~/.cargo/bin` (Windows'ta `%USERPROFILE%\.cargo\bin`) altına bir
dosya yazar. Araç, çalışma sırasında **program dizininde hiçbir şey
yazmaz**; yalnızca kullanıcının verdiği çıktı yollarına yazar.

---

## Kullanım

Aşağıdaki çıktıların **tamamı bu depoda gerçekten çalıştırılmıştır**
(Windows 11, Rust 1.98.1). Süreç listeleri makineye göre değişir; sayılar
örnektir.

### 1) Kural kataloğunu görmek (hiçbir sistem komutu çalıştırmaz)

```console
$ procsight rules --sadece-katalog
ProcSight bir gozlem aracidir: hicbir sureci sonlandirmaz, hiçbir dosyayi silmez, kayit defterine yazmaz ve aga cikmaz. Bir bulgu sucluluk hukmu degil, kendin dogrulayacagin bir gozlemdir.
kaynak: katalog

gizli ad
  ne gozlemliyor: Gorsel ad, insanin secmekte zorlanacagi bicimde makine uretilmis gorunuyor: harf-rakam karmasi, kabuk/kontrol karakteri veya asiri uzun ad. Bosluk iceren meşru adlar (orn. System Idle Process) bu kurala girmez.
  nasil dogrulanir: Dosyaya sag tikla > Ozellikler > Ayrintilar sekmesinde yayimcivertifikasini ve imzayi incele; bu adi hangi yazilimin kullandigini arayandan dogrula.
supheli konum
  ne gozlemliyor: Yurutulebilir dosya gecici veya profile gizlenmis bir konumdan calisiyor. Bu konumlar normalde yalnizca indirilen/uretilen dosyalar icin kullanilir.
  nasil dogrulanir: Dosyanin bulundugu konumu ac ve hangi programin buraya yazdigini programin kendi ayarlarindan kontrol et; konum gecici ise program gecici dosyadan calisiyor demektir.
yuksek bellek
  ne gozlemliyor: Surec belirlenen esigin uzerinde bellek tuketiyor. Bu tek basina bir sorun degildir; tarayici ve video duzenleyici gibi uygulamalar da bu esigi asabilir.
  nasil dogrulanir: Gorev Yoneticisi > Ayrintilar sekmesinde bu surecin normalde kac MiB kullandigini karsilastir; surekli buyuyorsa uygulamanin kendi ayar sayfasindan bellege sinir koy.
kalicilik girdisi
  ne gozlemliyor: Sistem acilista kendiliginden calisan bir girdi, gecici veya gizli konumdaki bir dosyayi isaret ediyor. Kisayolun kendisi standart baslangic klasorunde olsa bile hedefi denetlenir.
  nasil dogrulanir: Girdinin hedefini ac ve uygulamanin kurulumunu kimin yaptigini sor; programin kendi ayarlarinda bu baslangic girdisini kapatma secenegi var mi bak.
beklenmeyen ag baglantisi
  ne gozlemliyor: Kurulmus bir baglanti, ev/ofis agi disindaki bir adrese ve gunluk hayatta sik gorulen portlar disindaki bir porta gidiyor.
  nasil dogrulanir: Baglantiyi yapan uygulamayi ac ve ayarlarindaki ag erisimini incele; sunucunun adi bu programin hizmetidir; tanimiyorsan ayri bir agda (misafir Wi-Fi) baglanip ayni davranisi gozlemle.
kisa omurlu cok sayida surec
  ne gozlemliyor: Belirli sayinin uzerinde surec, yoklama araligindan kisa surede baslayip bitti. Kurulum, guncellestirme ve derleme gibi isler de bu gorunumu uretebilir.
  nasil dogrulanir: O donemde bir guncellestirme, derleme veya kurulum yapip yapmadigini hatirla; yapmadysan surec adlarini rapordan incele.
yetki yukseltme girisimi
  ne gozlemliyor: Surec, ust surucunden farkli bir kullanici kimligiyle calisiyor veya herkesin yazabildigi bir konumdan baslatiliyor. Ikisi de yazma yetkisi ele gecirme icin kullanilabilecek bir yol olabilir.
  nasil dogrulanir: Ust surecin hangi program oldugunu ve bu surecin neden farkli bir kullanici kimligiyle basladigini program ayarlarindan dogrula; yonetici calistirma gerekiyorsa bunu yapan ayari bul.
```

### 2) Tek seferlik envanter (canlı Windows kaynağı)

```console
$ procsight snapshot
ProcSight bir gozlem aracidir: hicbir sureci sonlandirmaz, hiçbir dosyayi silmez, kayit defterine yazmaz ve aga cikmaz. Bir bulgu sucluluk hukmu degil, kendin dogrulayacagin bir gozlemdir.
kaynak: windows

kapsam notu: ust süreç, komut satiri ve yürütülebilir yol tasklist'te yoktur; Run anahtarlari --reg bayragi ile okunur

SURECLER (273)
PID     PPID    AD                     MiB  DURUM      KULLANICI
0       0       System Idle Process         0  bilinmiyor-
4       0       System                     21  bilinmiyor-
88      0       Secure System             135  bilinmiyor-
156     0       Registry                   84  bilinmiyor-
732     0       smss.exe                    1  bilinmiyor-
848     0       csrss.exe                   4  bilinmiyor-
...
BAGLANTILAR (247)
PROTO  UCLAR                                           PID    SAHIBI
TCP    0.0.0.0:135 -> -                                4      System
TCP    0.0.0.0:445 -> -                                4      System
...
BASLANGIC GIRDILERI (0)

KURAL BULGULARI (0)
```

`procsight snapshot --wmic --reg` ile üst süreç, komut satırı, yürütülebilir
yol ve `Run` kayıtları da toplanır:

```console
$ procsight snapshot --wmic --reg
...
kapsam notu: ust süreç, komut satiri ve yürütülebilir yol tasklist'te yoktur

SURECLER (273)
PID     PPID    AD                     MiB  DURUM      KULLANICI
0       0       System Idle Process         0  bilinmiyor-
4       0       System                     21  bilinmiyor-
88      4       Secure System             135  bilinmiyor-
156     4       Registry                   84  bilinmiyor-
732     4       smss.exe                    1  bilinmiyor-
848     836     csrss.exe                   4  bilinmiyor-
952     836     wininit.exe                 5  bilinmiyor-
...

BASLANGIC GIRDILERI (4)
AD                   TUR                 DURUM    KONUM
MicrosoftEdgeAutoLaunch_DF0087B7B1DFD5D79A433D2727F9A314 run-anahtari       etkin     HKCU\Software\Microsoft\Windows\CurrentVersion\Run
Mozilla-Firefox-308046B0AF4A39CB run-anahtari       etkin     HKCU\Software\Microsoft\Windows\CurrentVersion\Run
SecurityHealth       run-anahtari       etkin     HKLM\Software\Microsoft\Windows\CurrentVersion\Run
Warp                 run-anahtari       etkin     HKCU\Software\Microsoft\Windows\CurrentVersion\Run
```

### 3) Deterministik örnek kümesiyle uçtan uca deneme

`--kaynak fixture`, `--kaynak windows|procfs|ps` seçilmezse aracın hiçbir
sistem komutuna dokunmadan, gömülü sabit veri kümesiyle çalışmasını sağlar.

```console
$ procsight snapshot --kaynak fixture
ProcSight bir gozlem aracidir: hicbir sureci sonlandirmaz, hiçbir dosyayi silmez, kayit defterine yazmaz ve aga cikmaz. Bir bulgu sucluluk hukmu degil, kendin dogrulayacagin bir gozlemdir.
kaynak: fixture

kapsam notu: statik veri seti; canlı sistemi gormez

SURECLER (8)
PID     PPID    AD                     MiB  DURUM      KULLANICI
4       0       System                      2  calisiyor NT AUTHORITY\SYSTEM
1044    4       svchost.exe                20  calisiyor NT AUTHORITY\SYSTEM  C:\Windows\System32\svchost.exe
4321    1044    explorer.exe               96  calisiyor DESKTOP\KULLANICI  C:\Windows\explorer.exe
555     4321    notepad.exe                12  calisiyor DESKTOP\KULLANICI  C:\Windows\Notepad\notepad.exe
77      4       wmiprvse.exe               14  calisiyor NT AUTHORITY\SYSTEM  C:\Windows\System32\wbem\wmiprvse.exe
9001    4321    Xk3jdH9s7qW.exe          1536  calisiyor DESKTOP\KULLANICI  C:\Users\KULLANICI\AppData\Local\Temp\Xk3jdH9s7qW.exe
9002    4321    updater.exe                40  calisiyor DESKTOP\KULLANICI  C:\Users\KULLANICI\AppData\Roaming\Updater\upd.exe
9003    1044    svchost-helper.exe          8  calisiyor DESKTOP\KULLANICI  C:\Windows\System32\svchost-helper.exe

BAGLANTILAR (7)
PROTO  UCLAR                                           PID    SAHIBI
TCP    0.0.0.0:135 -> -                                4      System
TCP    0.0.0.0:445 -> -                                4      System
TCP    127.0.0.1:51000 -> 93.184.216.34:443            4321   explorer.exe
TCP    127.0.0.1:51001 -> 45.83.220.17:8443            9001   Xk3jdH9s7qW.exe
TCP    192.168.1.5:51002 -> 192.168.1.10:445           9001   Xk3jdH9s7qW.exe
TCP    127.0.0.1:51003 -> 203.0.113.9:9001             9002   updater.exe
UDP    :::53533 -> -                                   4321   explorer.exe

BASLANGIC GIRDILERI (4)
AD                   TUR                 DURUM    KONUM
OneDrive             baslangic-klasoru  etkin     C:\Users\KULLANICI\AppData\Roaming\Microsoft\Windows\Start Menu\Programs\Startup\OneDrive.lnk
Kurulum              baslangic-klasoru  etkin     C:\Users\KULLANICI\AppData\Roaming\Microsoft\Windows\Start Menu\Programs\Startup\Kurulum.lnk
SecurityHealth       run-anahtari       etkin     HKCU\Software\Microsoft\Windows\CurrentVersion\Run
UpdaterTask          run-anahtari       kapali    HKCU\Software\Microsoft\Windows\CurrentVersion\Run

KURAL BULGULARI (11)
[gizli ad] pid=9001 ad=Xk3jdH9s7qW.exe — ad makine uretilmis gorunuyor (harf-rakam karmasi); ust surec=4321; yol=C:\Users\KULLANICI\AppData\Local\Temp\Xk3jdH9s7qW.exe
  dogrulama: Dosyaya sag tikla > Ozellikler > Ayrintilar sekmesinde yayimcivertifikasini ve imzayi incele; bu adi hangi yazilimin kullandigini arayandan dogrula.
...
```

### 4) Periyodik yoklama ve JSONL olay günlüğü

```console
$ procsight watch --kaynak fixture --tur 3 --aralik-ms 200 --gunluk "$env:TEMP\procsight-demo\gunluk.jsonl"
ProcSight bir gozlem aracidir: hicbir sureci sonlandirmaz, hiçbir dosyayi silmez, kayit defterine yazmaz ve aga cikmaz. Bir bulgu sucluluk hukmu degil, kendin dogrulayacagin bir gozlemdir.
kaynak: fixture

gunluk: %USERPROFILE%\AppData\Local\Temp\procsight-demo\gunluk.jsonl
aralik: 200 ms, tur: 3

tur    1  surec    8  yeni   0  bitti   0  kisa omurlu   0  yazilan 10
tur    2  surec    8  yeni   0  bitti   0  kisa omurlu   0  yazilan 10
tur    3  surec    8  yeni   0  bitti   0  kisa omurlu   0  yazilan 10
```

Günlüğün ilk satırları (JSON Lines; bir satır = bir olay):

```json
{"zaman":1790680782746,"olay":"baglanti_goruldu","pid":4321,"kaynak":"fixture","ayrinti":"TCP 127.0.0.1:51000 -> 93.184.216.34:443 kuruldu"}
{"zaman":1790680782746,"olay":"baglanti_goruldu","pid":9001,"kaynak":"fixture","ayrinti":"TCP 127.0.0.1:51001 -> 45.83.220.17:8443 kuruldu"}
{"zaman":1790680782746,"olay":"baslangic_girdisi","pid":null,"kaynak":"fixture","ayrinti":"OneDrive (baslangic-klasoru) — C:\\Users\\KULLANICI\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\OneDrive.lnk"}
```

### 5) Rapor dışa aktarımı (HTML + CSV)

```console
$ procsight report --gunluk "$env:TEMP\procsight-demo\gunluk.jsonl" --html "$env:TEMP\procsight-demo\rapor.html" --csv "$env:TEMP\procsight-demo\olaylar.csv" --baslik "Demo gozlem raporu" --kaynak-adi fixture
%USERPROFILE%\AppData\Local\Temp\procsight-demo\gunluk.jsonl okunan 30, bozuk 0
yazildi: %USERPROFILE%\AppData\Local\Temp\procsight-demo\rapor.html
yazildi: %USERPROFILE%\AppData\Local\Temp\procsight-demo\olaylar.csv
```

CSV'nin ilk satırları:

```csv
zaman,olay,pid,kaynak,ayrinti
1790680782746,baglanti_goruldu,4321,fixture,TCP 127.0.0.1:51000 -> 93.184.216.34:443 kuruldu
1790680782746,baglanti_goruldu,9001,fixture,TCP 127.0.0.1:51001 -> 45.83.220.17:8443 kuruldu
```

Üretilen HTML tek dosyadır; `http://` veya `https://` içermez, `<script>`,
`<img>` ve `src=` içermez. Bu, `tests/ayristir_entegrasyon.rs` ve
`src/rapor.rs` içinde testle sabitlenmiştir.

### 6) Kuralları günlükle birlikte değerlendirmek

```console
$ procsight rules --kaynak fixture --gunluk "$env:TEMP\procsight-demo\gunluk.jsonl"
gunluk: %USERPROFILE%\AppData\Local\Temp\procsight-demo\gunluk.jsonl (okunan 30, bozuk 0; bozuk satir sayisi acikca raporlandi)
ProcSight bir gozlem aracidir: hicbir sureci sonlandirmaz, hiçbir dosyayi silmez, kayit defterine yazmaz ve aga cikmaz. Bir bulgu sucluluk hukmu degil, kendin dogrulayacagin bir gozlemdir.
kaynak: fixture

envanter: 8 surec, 7 baglanti, 4 baslangic girdisi
esikler: bellek >= 512 MiB, kisa omurlu >= 5 adet

[gizli ad] pid=9001 ad=Xk3jdH9s7qW.exe
    kanit: ad makine uretilmis gorunuyor (harf-rakam karmasi)
    kanit: ust surec=4321
    kanit: yol=C:\Users\KULLANICI\AppData\Local\Temp\Xk3jdH9s7qW.exe
    gozlem: Gorsel ad, insanin secmekte zorlanacagi bicimde makine uretilmis gorunuyor: harf-rakam karmasi, kabuk/kontrol karakteri veya asiri uzun ad. Bosluk iceren meşru adlar (orn. System Idle Process) bu kurala girmez.
    dogrulama: Dosyaya sag tikla > Ozellikler > Ayrintilar sekmesinde yayimcivertifikasini ve imzayi incele; bu adi hangi yazilimin kullandigini arayandan dogrula.
...
```

---

## Test

```bash
cargo test
```

Gerçek çıktı (bu depoda ölçüldü):

```
test result: ok. 160 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
   Running unittests src\main.rs
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
   Running tests\ayristir_entegrasyon.rs
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
   Running tests\salt_okunur_kanit.rs
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
   Running tests\uctan_ucta.rs
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
   Doc-tests procsight
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

**test sonucu: okunan 211; başarısız 0** (160 + 8 + 21 + 13 + 9 = 211).

Ek kalite kapıları:

```bash
cargo build --release          # hatasız
cargo clippy --all-targets -- -D warnings   # temiz
cargo fmt --check              # temiz
```

### Test vektörleri ve kapsanan uç durumlar

- **`tasklist /fo csv` ayrıştırıcısı** — fixture dosyasıyla (`tests/fixtures/tasklist_fo_csv.txt`),
  gerçek komut çalıştırılmadan: başlıklı/başlıksız çıktı, **Türkçe** sütun
  başlıkları (`"Görüntü Adı"`, `"KİMLİK"`), yerelleştirilmiş bellek ayracı
  (`98,412 K` ve `98.412 K`), `M`/`G` birimleri, tırnak içi ayraç, 5'ten az
  sütunlu satır, sayısal olmayan PID, boş girdi, `INFO/BILGI` satırı,
  `/fo csv /v` gibi çok sütunlu varyant.
- **`netstat -ano` ayrıştırıcısı** — fixture dosyasıyla: Türkçe başlıklar,
  TCP/UDP ayrımı, `[::]:445` parantezli IPv6, `[fe80::1%12]:546` kapsam
  kimliği, `*:*` ve `0.0.0.0:0` yer tutucuları, eksik sütunlu satır, bozuk yerel
  adres, durum sözcüğü eşlemesi, boş girdi, sahiplikle eşleştirme.
- **`wmic process /format:csv` ayrıştırıcısı** — başlık bulma, sütun
  eşleme, tırnaklı komut satırı, eksik sütunda `ppid=0`, boş yol → `None`,
  sayısal olmayan PID.
- **`/proc` ayrıştırıcıları** — `stat` (boşluk ve parantez içeren `comm` alanı,
  kısa satır, sayısal olmayan pid), `cmdline` (NUL ayrımı, boş içerik = çekirdek
  iş parçacığı, geçersiz UTF-8), `status` (`Uid:`, `Name:`), `/proc/net/tcp`
  (başlık, **küçük bayt sırası** ile adres çevirme, inode eşlemesi, bozuk adres,
  bozuk durum kodu, boş girdi), `/proc/net/tcp6` (32 onaltılık adres).
- **Yedi kuralın her biri için pozitif + negatif** test, ayrıca **eşik sınırı**
  (512 MiB tam eşik tetiklenir, bir KiB altı tetiklenmez; 5 kısa ömürlü süreç
  tam eşikte tetiklenir, 4'ü tetiklenmez), **muafiyet testleri**
  (standart Başlangıç klasörü, `AppData\Local\Programs`,
  `WindowsApps`, `WinGet\Packages`), **sözcük uyumu testleri** (boşluk içeren
  meşru Windows adları işaretlenmez) ve **müdahale önermeme testleri**.
- **Süreç kayboldu** — `YasamTakip` gözlenen sürecin kaybolmasını hata saymaz,
  `kayboldu` listesine alır ve yalnızca tek turda görülenleri `kisa_omurlu`
  olarak işaretler; PID yeniden kullanımı "yeni" olarak sayılır.
- **JSONL şema** — dört zorunlu alan + `kaynak`, gidiş-dönüş
  serileştirme, bozuk/yarım satır atlanır **ve sayılır**, boş satırlar
  sayılmaz, ekleme modu mevcut içeriği silmez.
- **HTML dışa aktarımı** — uzak kaynak yok, `<script>`/`<img>`/`src=` yok,
  tüm metin değerleri kaçırılmış (`<script>` yazan süreç adı `<` olarak çıkar),
  boş bulguda "hiçbiri tetiklenmedi" mesajı, tüm bölüm başlıkları.
- **CSV** — `=`, `+`, `-`, `@`, TAB, CR önek kaçırması; virgül/tırnak içeren
  alanların tırnaklanması ve tırnak kaçırma; **süreç adından gelen formül
  enjeksiyonunun** uçtan uca testi.
- **Salt okunurluk kanıtı** — aşağıda ayrı başlıkta.

### "Salt okunur" kuralının test kanıtı

`tests/salt_okunur_kanit.rs` üç ayrı katmanda kanıt sunar:

1. **Statik tarama** — `src/` altındaki **üretim** kodunda (her dosyanın ilk
   `#[cfg(test)]` işaretinden öncesi, yorumlar ayıklanmış hâli) şu kalıpların
   hiçbiri bulunmaz: `remove_file`, `remove_dir`, `remove_dir_all`,
   `taskkill`, `TerminateProcess`, `OpenProcess`, `reg add`, `reg delete`,
   `schtasks /change`, `sc config`, `TcpStream::connect`, `UdpSocket::bind`,
   `Command::new("curl`, `ptr::write`, `libc::kill`. Ayrıca `set_permissions`
   ve `set_modified` yoktur.
2. **İzin listesi** — çalıştırılabilir programların kapalı listesi
   (`tasklist`, `netstat`, `ps`, `wmic`, `reg`) dışındaki hiçbir program
   çalıştırılamaz; `reg` **yalnızca `query`** alt komutuyla çağrılabilir,
   `add`/`delete`/`import`/`export` reddedilir. Kaynaktaki beş `komut_calistir`
   çağrısının hepsi izinli listeyle eşleşir.
3. **Çalışma zamanı** — bir gözlem turu çalıştırılır; gözlenen dosyanın içeriği
   ve zaman damgası değişmez, gözlenen dizine yeni dosya düşmez, üretilen
   çıktı dosyaları yalnızca kullanıcının verdiği dizinde bulunur ve depo
   kökünde `.jsonl`/`.html`/`.csv` artığı kalmaz.
4. **Yapısal** — `Bulgu` yapısı tam olarak `kural`, `konu`, `kanit`,
   `aciklama`, `sonraki_adim` alanlarından oluşur; `eylem`/`mudahale` alanı
   **yoktur** ve JSON çıktısında da bulunmaz.

---

## Proje Yapısı

```
07-procsight/
├── Cargo.toml
├── Cargo.lock              (üretilir, commit edilir)
├── LICENSE.txt             (MIT)
├── README.md
├── .gitignore
├── src/
│   ├── main.rs             CLI kabuğu: snapshot / watch / rules / report
│   ├── lib.rs              modül yönlendirmesi + güvenlik nitelikleri
│   ├── hata.rs             hata tipi ve `Sonuc` takma adı
│   ├── model.rs            platformdan bağımsız veri modeli, adres/konum yardımcıları
│   ├── motor.rs            yoklama döngüsü, `YasamTakip`, olay üretimi
│   ├── kurallar.rs         yedi şüpheli davranış kuralı ve eşikler
│   ├── gunluk.rs           JSONL günlük yazımı/okuması, bozuk satır sayacı
│   ├── rapor.rs            HTML ve CSV dışa aktarımı
│   ├── ayristir/
│   │   ├── mod.rs          ayrıştırma sayacı (`okunan` / `atlanan` / `bilgi`)
│   │   ├── tasklist.rs     `tasklist /fo csv`
│   │   ├── wmic.rs         `wmic process /format:csv`
│   │   ├── netstat.rs      `netstat -ano`
│   │   └── procstat.rs     `/proc/<pid>/{stat,cmdline,status}`, `/proc/net/tcp*`
│   └── kaynak/
│       ├── mod.rs          `InventorySource` trait'i, izin listesi, dizin gezgini
│       ├── windows.rs      WindowsSource + `reg query` ayrıştırıcısı
│       ├── procfs.rs       ProcFsSource (`/proc`)
│       ├── ps.rs           PsSource (macOS/BSD, `ps`)
│       └── fixture.rs      FixtureSource (statik veri, komut çalıştırmaz)
└── tests/
    ├── ayristir_entegrasyon.rs   fixture dosyalarıyla ayrıştırıcı denemeleri
    ├── salt_okunur_kanit.rs      salt okunurluk kanıtı (statik + çalışma zamanı)
    ├── uctan_ucta.rs             gözlem → yaşam → günlük → kural → rapor
    ├── yardimci/mod.rs           geçici dizin ve fixture okuma yardımcısı
    └── fixtures/
        ├── tasklist_fo_csv.txt
        ├── netstat_ano.txt
        ├── wmic_process_csv.txt
        ├── proc_pid_stat.txt
        ├── proc_net_tcp.txt
        └── reg_query_run.txt
```

---

## Yapılandırma

Araç bir yapılandırma **dosyası** kullanmaz; tüm ayarlar bayrakla verilir.
Böylece salt okunur bir dizinden çalıştırılabilir ve yanlışlıkla bozulan bir
`config/` dosyası aracı çalışmaz hale getiremez.

| Bayrak / seçenek | Alt komut | Varsayılan | Etkisi |
|------------------|-----------|------------|--------|
| `--kaynak <otomatik\|windows\|procfs\|ps\|fixture>` | snapshot, watch, rules | `otomatik` | Envanter kaynağını seçer. `otomatik`, derleme hedefine göre Windows/Linux/diğer seçim yapar. `fixture` hiçbir sistem komutu çalıştırmaz. |
| `--wmic` | snapshot, watch, rules | kapalı | Windows'ta `wmic` ile komut satırı, üst süreç ve yürütülebilir yolu da toplar. `wmic` yoksa araç hata vermez, ilgili alanlar boş kalır ve `kapsam notu` buna açıkça işaret eder. |
| `--reg` | snapshot, watch, rules | kapalı | Windows'ta `HKCU`/`HKLM\...\CurrentVersion\Run` kayıtlarını `reg query` ile okur. Yalnızca `query` alt komutu çağrılır. |
| `--ayrinti` | snapshot, watch, rules | kapalı | Ayırt edici çıktı üretir. |
| `--bicim <metin\|json>` | snapshot, rules | `metin` | `snapshot --bicim json` tam envanteri JSON olarak verir. |
| `--cikti <yol>` | snapshot | stdout | Metni/JSON'u dosyaya yazar. |
| `--gunluk <yol>` | watch | `procsight-gunlugu.jsonl` | JSONL olay günlüğü yolu. Dosya yoksa oluşturulur; var olan içerik **eklenir**, üzerine yazılmaz. |
| `--aralik-ms <n>` | watch | `1000` | Yoklama aralığı. `0` reddedilir. Düşük değer kısa ömürlü süreç kaçırma riskini azaltır ama günlük boyutunu büyütür. |
| `--tur <n>` | watch | `0` (sonsuz) | Kaç tur yapılacağı. |
| `--bekle-saniye <n>` | watch | `60` | `aralik_ms` kadar bekleyip bu süre dolunca çıkar. |
| `--sadece-katalog` | rules | kapalı | Yalnızca yedi kuralın adını, açıklamasını ve doğrulama adımını yazar; envanter toplamaz. |
| `--bellek-mib <n>` | rules | `512` | Yüksek bellek kuralının MiB eşiği. |
| `--kisa-omurlu <n>` | rules | `5` | Kısa ömürlü süreç kuralının adet eşiği. |
| `--gunluk <yol>` | rules | yok | Kısa ömürlü süreç istatistiğini bu JSONL günlükten hesaplar. Verilmezse bu kural 0 sayılır ve bunu açıkça yazar. |
| `--bicim <metin\|json>` | rules | `metin` | Bulgu çıktısının biçimi. |
| `--gunluk <yol>` | report | zorunlu | Kaynak JSONL günlük. |
| `--html <yol>` | report | yok | Tek dosya HTML raporu yazılır. |
| `--csv <yol>` | report | yok | Olay günlüğünün CSV hâli yazılır. |
| `--baslik <metin>` | report | `ProcSight gozlem raporu` | Rapor başlığı. |
| `--kaynak-adi <metin>` | report | `bilinmiyor` | Raporda görünecek kaynak adı. |

Ortam değişkeni kullanımı yalnızca Windows kaynağındadır: `%APPDATA%` ve
`%PROGRAMDATA%` (Başlangıç klasörü), `%HOME%` (XDG autostart). Eksikse ilgili
klasör atlanır; araç hata vermez.

---

## Bilinen Sınırlamalar

Bu bölüm kasıtlı olarak uzundur. Bir gözlem aracının **neyi göremediğini**
bilmeden kullanılmamalıdır.

### MANIFEST kartından ertelenenler

- **ETW tabanlı gerçek zamanlı olay akışı.** Windows'ta ETW `advapi32` FFI'si
  gerektirir ve `#![forbid(unsafe_code)]` ile bağdaşmaz. Bu sürüm **periyodik
  yoklamaya** düşer.
- **Temiz başlangıç karşılaştırması.** Bir kez kaydedip sonraki açılışlarla
  fark gösterme özelliği bu sürümde yoktur; başlangıç girdileri yalnızca
  listelenir.
- **macOS `kqueue` / `sysctl` gerçek zamanlı akışı.** macOS'ta süreç tablosu
  `ps` çıktısından okunur (FFI yok); **bağlantı tablosu macOS'ta kapsam
  dışıdır** ve `ag_baglantilari()` bilinçli olarak hata döner — boş liste
  döndürmek yanıltıcı olurdu.

### Bu sürümde kendi eklediğimiz, MANIFEST'te ertelenmiş sayılan iki özellik

MANIFEST kartı "şüpheli davranış kuralları" ve "HTML/CSV dışa aktarma"
maddelerini **ertelenenler** arasında sayar. Bu sürümde ikisi de uygulanmıştır;
aşağıdaki sınırlarıyla birlikte:

- **Kurallar veri tabanlı bir dosyadan değil, koddan gelir.** Kullanıcı
  kuralı düzenleyemez, eşiği yalnızca komut satırından değiştirebilir. Bu,
  "her kural zorunlu açıklama alanı taşır" ilkesini korur ama esneklik kazanmaz.
- **CSV çıktısı yalnızca olay günlüğü içindir.** Süreç/bağlantı/başlangıç
  tablolarının CSV üreticileri kütüphanede mevcuttur (`rapor.rs`) ancak CLI'de
  `--csv` bayrağına bağlı değildir; `report` alt komutu günlükten HTML ve CSV
  üretir.

### Ölçülebilir bilgi kaybı

- **Kısa ömürlü süreçler kaçar.** Yoklama aralığından (`--aralik-ms`,
  varsayılan 1000 ms) kısa sürede başlayıp biten süreçler hiç görünmez.
  Raporda ETW'nin seçilme gerekçesi tam olarak budur; periyodik yoklama bu
  kaybı kapatamaz. Bu sürümde kayıp **ölçülmedi**.
- **Bağlantı tablosu anlıktır.** `netstat -ano` ve `/proc/net/tcp` bağlantı
  geçmişini değil, o anda açık olan bağlantıları gösterir. "On dakika önce
  hangi program bağlandı?" sorusu yalnızca `watch` ile üretilen günlükten
  yanıtlanabilir.
- **Sahiplik çözülemezse gizlenmez.** Linux'ta `/proc/net/tcp` inode taşır;
  eşleşme `/proc/<pid>/fd` ile yapılır ve yetki hatasında bağlantının `pid`
  alanı `None` kalır. Raporda sahibi "çözülemedi" olarak görünür.
- **DNS çözümlemesi yapılmaz.** Uzak adresler IP olarak yazılır. Bu çevrimdışı
  ilkesinin gereğidir ve gizlilik lehinedir.

### Windows kapsamı

- **`tasklist /fo csv` üst süreç, komut satırı, yürütülebilir yol ve kullanıcı
  vermez.** Bu alanlar yalnızca `--wmic` ile gelir. `wmic` Windows 11 24H2'de
  varsayılan olarak kaldırılmıştır; o durumda alanlar boş kalır ve `kapsam
  notu` bunu söyler. Alternatif (PowerShell/COM) FFI gerektirdiği için
  kullanılmamıştır.
- **Süreç durumu bilinmez.** `tasklist` durum vermez; metin çıktısında
  `bilinmiyor` görünür. `/proc` yolunda durum `/proc/<pid>/stat`'ten gelir.
- **CPU yüzdesi Windows'ta boştur.** Tek yoklamada hesaplanamaz; iki yoklama
  arasında fark gerekir ve bu sürümde hesaplanmamıştır.
- **Başlangıç klasörü kısayollarının hedefi okunmaz.** `.lnk`/`.url`
  dosyaları ikili kabuk bağlantısıdır; hedef çözülmediğinden kural 4 yalnızca
  `reg query` ile okunan `Run` anahtarlarında hedef denetleyebilir. Dizin
  taramasıyla bulunan girdiler bu nedenle muaf tutulur (aksi hâlde modern
  Windows'ta her Başlangıç klasörü girdisi işaretlenirdi).
- **Yönetici gerektirmeyen modun sınırı.** Yönetici olmayan oturumda
  `HKLM\...\Run` anahtarının tamamı okunamayabilir; `reg_var` başarısızsa bu
  durum `kapsam notu` ile bildirilir. Araç eksik kapsamı "tam görünüm" gibi
  sunmaz.

### Kural kaynaklı yanlış pozitifler

Rapordan R1 (yanlış pozitifler kullanıcının güvenini götürür) kabul edilmiş
bir risktir. Kurallar **kötücül yazılım tespiti değildir** ve şu yanlış
pozitifler üretmeyi kabul eder:

- `yuksek bellek`: tarayıcılar, sanal makineler (`vmmem`), bellek birleştirme
  (`Memory Compression`) ve geliştirme araçları eşiği kolayca aşar.
- `gizli ad`: `M365Copilot.exe` gibi **meşru** ürün adları "harf-rakam
  karması" ölçütüne takılabilir. Bu nedenle kural, imza kontrolü adımıyla birlikte
  sunulur.
- `beklenmeyen ag baglantisi`: tanıdık port listesi (`22, 53, 80, 123, 443, 465,
  587, 993, 995, 5222, 5223`) dar tutulmuştur; VPN, oyun ve geliştirme araçları
  başka portlar kullanır.
- `kisa omurlu cok sayida surec`: derleme, paket kurulumu ve güncelleme aynı
  görünümü üretir.
- Muafiyet listesi (`AppData\Local\Programs`, `WindowsApps`, `WinGet\Packages`,
  `AppData\Roaming\Microsoft\Windows`, standart Başlangıç klasörü) elle
  tutulur ve yeni meşru kurulum konumları için **genişletilmelidir**.

### Diğer

- **Bellek bütçesi ölçülmedi.** Kartta ≤ 120 MB tepe RSS hedefi vardır; bu
  sürümde ölçüm yapılmamıştır. Gözlem aracı yalnızca bir anda okunan tabloları
  ve olay listesini bellekte tutar; sabit boyutlu bir halka tampon **yoktur**,
  bu yüzden günlük dosyası diske taşar, belleğe değil.
- **`GeciciDizin` test yardımcısında `Drop` temizliği hata olursa sessizce
  geçilir.** `Drop` içinden hata döndürülemez; bu, sözleşmenin "sessiz yutma"
  yasağının bilinçli bir istisnasıdır.
- **`#[allow(dead_code)]`** yalnızca `tests/yardimci/mod.rs` dosyasının
  başında vardır: ortak yardımcı modülü birden çok test ikilisine eklenir ve
  her ikili yalnızca bir bölümünü kullanır. Üretim kodunda hiçbir `allow`
  kullanılmaz.
- **Günlük dosyası saatlik bölünmez ve otomatik silinmez.** Kartın önerdiği
  gün sayısı/üst boyut sınırı bu sürümde yoktur; kullanıcı `--gunluk` ile
  yol seçer ve kendisi döngüsüyle temizler.
- **Diğer kullanıcılara ait süreç adlarını maskeleme uygulanmaz.** Bu sürümde
  ad maskeleme yoktur; `HKCU`/`HKLM` kapsam farkı `kapsam notu` ile
  belirtilir. Raporun önerdiği "başka kullanıcının adları varsayılan kapalı"
  davranışı **eksiktir**.

---

## Gelecek Geliştirmeler

Karttaki ertelenenler ve doğal sonraki adımlar:

1. **ETW tabanlı gerçek zamanlı akış** (yalnız Windows). FFI gerektirdiği
   için ayrı bir çalışma dalı ve `forbid` → `deny` değişikliği gerektirir.
2. **Temiz başlangıç tabanı ve fark hesabı.** `SnapshotItem` tabanı kaydedip
   sonraki açılışlarda yeni girdileri göstermek.
3. **Kural dosyası** (`rules.json`) — her kural zorunlu açıklama ve doğrulama
   adımı alanlarıyla, kullanıcı tarafından düzenlenebilir.
4. **Bant genişliği örnekleme**: `GetIfEntry2`/`/proc/net/dev` ile bağlantı
   başına gönderilen/alınan bayt; `netstat` bunu vermez.
5. **Saatlik günlük bölme ve üst boyut sınırı** (kartın R9 azaltımı).
6. **Komut satırı sır maskeleme**: raporun R3 riski için `--token`,
   `--password` gibi kalıpların değerlerini günlüğe yazmadan maskeleme.
7. **macOS bağlantı tablosu**: `netstat -anv` çıktısının ayrıştırılması.
8. **Diğer kullanıcı adlarını maskeleme** ve oturum farkı görünümü (S5).
9. **Paketleyici**: tek dosya dağıtım için sürüm anahtarı ve imzalı sürüm.

---

## Troubleshooting

| Belirti | Neden | Çözüm |
|---------|-------|-------|
| `error: no such command` / `kaynak yok (procfs): bu platformda /proc yok` | `--kaynak` ile çalışılan platform eşleşmiyor (ör. Linux'ta `--kaynak windows`) | `--kaynak otomatik` kullanın veya platforma uygun kaynağı seçin. |
| `SURECLER (0)` ve `kapsam notu: .../proc yok` | Linux'ta `--kaynak procfs` verilmiş ama `/proc` bağlı değil (konteyner/sandbox) | `--kaynak otomatik` deneyin; yoksa `--kaynak ps` deneyin. |
| `ust surec, komut satiri ve yurutulebilir yol tasklist'te yoktur` ve alanlar boş | `--wmic` verilmemiş, ya da `wmic` kaldırılmış (Windows 11 24H2+) | `--wmic` ekleyin. Çalışmıyorsa bu alanlar bu sürümde doldurulamaz; kapsam notu bunu açıkça söyler. |
| `Run anahtarlari okunamadi` / `BASLANGIC GIRDILERI (0)` | `--reg` verilmemiş, ya da oturum `HKLM\...\Run` anahtarını okuyamıyor | `--reg` ekleyin. Yönetici gerekiyorsa standart kullanıcıyla yönetici arasındaki farkı not edin; araç eksik kapsamı "tam" diye sunmaz. |
| `Komut satırı sıfır olamaz` (`watch --aralik-ms 0`) | Sıfır aralık sonsuz döngü demektir ve reddedilir | `--aralik-ms 1000` gibi pozitif bir değer verin. |
| `wrote ... bozuk (…): ...` hatası | `--gunluk`/`--html`/`--csv` için verilen dizin yok veya yazılabilir değil | Dizin var mı ve yazılabilir mi kontrol edin. Araç program dizinini asla yazılabilir kabul etmez. |
| Raporda `uyari: N satir bozuk oldu` | JSONL günlüğü bir alt süreç tarafından yazılırken kesilmiş veya elle düzenlenmiş | Bu normaldir: bozuk satır **sayılır ve atlanır**, günlüğün eksik olduğu raporda belirtilir. Satırları elle düzeltmek isterseniz günlüğü kopyalayın. |
| Gözlenen günlük beklenenden büyük | Varsayılan aralık 1000 ms; her bağlantı her turda yazılır | `--aralik-ms 5000` ile aralığı büyütün veya `--tur` ile süreyi sınırlayın. Günlüğün kendiniz sileceği dosyadır; araç silmez. |

---

## Atıflar

- Rust standart kütüphane belgeleri — <https://doc.rust-lang.org/std/>
- Rust 2021 edition rehberi — <https://doc.rust-lang.org/edition-guide/edition-2021.html>
- `cargo` yerel rehberi — <https://doc.rust-lang.org/cargo/>
- `serde` ve `serde_json` — <https://serde.rs/> · <https://docs.rs/serde_json/>
- `clap` — <https://docs.rs/clap/>
- Rust standart kütüphanesi `Command` API'si (alt süreç çalıştırma, yalnızca
  salt okunur sistem araçları için) — <https://doc.rust-lang.org/std/process/struct.Command.html>
- Linux çekirdek dokümantasyonu — procfs: <https://www.kernel.org/doc/html/latest/filesystems/proc.html>
- Linux çekirdek `proc(5)` — `/proc/<pid>/stat` alan düzeni ve `USER_HZ`:
  <https://man7.org/linux/man-pages/man5/proc.5.html>
- `/proc/net/tcp` biçimi ve durum kodları — <https://man7.org/linux/man-pages/man4/proc_net_tcp.7.html>
- Microsoft Learn — `tasklist`: <https://learn.microsoft.com/windows-server/administration/windows-commands/tasklist>
- Microsoft Learn — `netstat`: <https://learn.microsoft.com/windows-server/administration/windows-commands/netstat>
- Microsoft Learn — `reg query`: <https://learn.microsoft.com/windows-server/administration/windows-commands/reg>
- Microsoft Learn — WMI kullanan `wmic`: <https://learn.microsoft.com/windows/win32/wmisdk/querying-wmi-with-wmic>
- Microsoft Learn — Event Tracing for Windows (karşılaştırma amaçlı, bu sürümde
  **kullanılmıyor**): <https://learn.microsoft.com/windows/win32/etw/about-event-tracing>
- XDG Desktop Entry Specification (`$XDG_CONFIG_HOME/autostart`) — <https://specifications.freedesktop.org/desktop-entry-spec/latest/>
- RFC 5737 — IPv4 ile IPv6 Belgeleme (Ayrılmış) Adres Blokları — <https://www.rfc-editor.org/rfc/rfc5737>
- RFC 8200 — IPv6 — <https://www.rfc-editor.org/rfc/rfc8200>
- OWASP — CSV Injection — <https://owasp.org/www-community/attacks/CSV_Injection>
- Semantic Versioning — <https://semver.org/>
- Tasarım kaynağı (yerel dosya, URL değildir):
  `%USERPROFILE%\Desktop\Fikirler\07-surec-gozlemleyici.html` — bu projenin
  iç tasarımının kaynağıdır. Rapordaki sayısal iddialar **ölçülmemiş
  tahminlerdir** ve bu depodaki hiçbir ölçüm o rapordan alınmamıştır.

Doğrudan kopyalanan üçüncü taraf kodu yoktur; tüm ayrıştırıcılar sıfırdan bu
depoda yazılmış ve resmî dokümanlara dayanan test vektörleriyle sınanmıştır.

---

## Üretim Atfı

Bu depo **OpenCode** ajanı tarafından, **`space-bunny-free`** modeli
(`opencode/space-bunny-free`) kullanılarak üretilmiştir.

- **Arac:** OpenCode
- **Model:** `opencode/space-bunny-free` (Space Bunny Free)
- **Tür:** Rust, `cargo build` / `cargo test` ile üretilmiş ve doğrulanmıştır.

Kaynak kod, testler ve dokümantasyon bu model tarafından yazılmıştır. İnsan
katkısı: gereksinim tanımı, kabul ölçütleri ve son kontroller.

## Lisans

MIT lisansı. Tam metin için [LICENSE.txt](LICENSE.txt) dosyasına bakın.

Telemetri, analiz kodu, güncelleme denetimi ve ağ bağlantısı **yoktur**.
Veri toplanmaz, satılmaz. Araç yalnızca yerel diske, kullanıcının verdiği
yollara yazar. Kullanıcı bu dosyaları istediği zaman kendisi siler; araç
silme işlevi içermez.
