//! Platformdan bağımsız gözlem veri modeli.
//!
//! Bu modül, üç farklı envanter kaynağının (procfs, `tasklist`/`netstat`,
//! `ps`) ortak ürettiği kayıt tiplerini tanımlar. Hiçbir kayıt tipi bir
//! işletim sistemi çağrısı yapmaz; hepsi saf veri taşır.
//!
//! Ayrıca iki yardımcı yordam burada yaşar: `supheli_konum_mi` (şüpheli
//! konum tespiti) ve `genel_adres_mi` (küresel yönlendirilebilir adres).
//! Kural motoru bu ikisini doğrudan çağırır.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Bir sürecin çalışma durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurecDurumu {
    /// Çalışır durumda (`R`, `I`).
    Calisiyor,
    /// Uyuyor veya kesilebilir beklemede (`S`, `D`).
    Uyuyor,
    /// Durduruldu (`T`, `t`, `W`).
    Durdu,
    /// Bitti ama `wait` edilmedi (`Z`).
    Zombi,
    /// Çekirdeğin verdiği harf bu kapsam dışında.
    Bilinmiyor,
}

impl SurecDurumu {
    /// `/proc/<pid>/stat` içindeki tek harflik durum kodunu çözer.
    pub fn proc_harfinden(harf: char) -> Self {
        match harf {
            'R' | 'I' => SurecDurumu::Calisiyor,
            'S' | 'D' => SurecDurumu::Uyuyor,
            'T' | 't' | 'W' => SurecDurumu::Durdu,
            'Z' => SurecDurumu::Zombi,
            _ => SurecDurumu::Bilinmiyor,
        }
    }

    /// İnsan okunur tek sözcüklü ad.
    pub fn metin(self) -> &'static str {
        match self {
            SurecDurumu::Calisiyor => "calisiyor",
            SurecDurumu::Uyuyor => "uyuyor",
            SurecDurumu::Durdu => "durdu",
            SurecDurumu::Zombi => "zombi",
            SurecDurumu::Bilinmiyor => "bilinmiyor",
        }
    }
}

/// Bir ağ bağlantısının taşıdığı protokol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Protokol {
    /// TCP (bağlantı yönü ve durumu vardır).
    Tcp,
    /// UDP (yön bilgisi yoktur, durum `Bos` olur).
    Udp,
}

impl Protokol {
    /// Kısa büyük harfli etiket.
    pub fn etiket(self) -> &'static str {
        match self {
            Protokol::Tcp => "TCP",
            Protokol::Udp => "UDP",
        }
    }
}

/// Bir bağlantının taşıma katmanı durumu.
///
/// `tasklist`/`netstat` sözcükleri ile `/proc/net/tcp` onaltılık kodları tek
/// bir enum'a indirgenir; böylece kural motoru platformdan bağımsız çalışır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BaglantiDurumu {
    /// Dinleme soketi açık (`LISTENING`, procfs `0A`).
    Dinliyor,
    /// Aktif veri taşıyan bağlantı (`ESTABLISHED`, procfs `01`).
    Kuruldu,
    /// SYN gönderildi, yanıt bekleniyor (`SYN_SENT`, procfs `02`).
    SynGonderildi,
    /// SYN alındı, ACK bekleniyor (`SYN_RECV`, procfs `03`).
    SynAlindi,
    /// Yerel uç FIN gönderdi, uzak yanıt bekliyor (`FIN_WAIT1`, procfs `04`).
    FinWait1,
    /// Yerel uç FIN aldı, kapanışı bekliyor (`FIN_WAIT2`, procfs `05`).
    FinWait2,
    /// Zaman aşımlı bekleyen çift yönlü kapanış (`TIME_WAIT`, procfs `06`).
    ZamanBekliyor,
    /// Uzak uç kapandı, yerel uçun kapanışı bekliyor (`CLOSE_WAIT`, procfs `08`).
    KapatmaBekliyor,
    /// Son ACK gönderildi (`LAST_ACK`, procfs `09`).
    SonAck,
    /// Her iki uç da eş zamanlı FIN gönderdi (`CLOSING`, procfs `0B`).
    Kapaniyor,
    /// Bağlantı yok (`CLOSED`, procfs `07`).
    Kapali,
    /// UDP veya yönü olmayan durum.
    Bos,
    /// Eşlenmemiş bir durum kodu.
    Bilinmiyor,
}

impl BaglantiDurumu {
    /// `netstat -ano` çıktısındaki Türkçe/İngilizce durum sözcüğünü çözer.
    pub fn netstat_sozcugunden(sozcuk: &str) -> Self {
        match sozcuk.to_ascii_uppercase().as_str() {
            "LISTENING" => BaglantiDurumu::Dinliyor,
            "ESTABLISHED" => BaglantiDurumu::Kuruldu,
            "SYN_SENT" => BaglantiDurumu::SynGonderildi,
            "SYN_RECV" | "SYN_RECEIVED" => BaglantiDurumu::SynAlindi,
            "FIN_WAIT1" => BaglantiDurumu::FinWait1,
            "FIN_WAIT2" => BaglantiDurumu::FinWait2,
            "TIME_WAIT" => BaglantiDurumu::ZamanBekliyor,
            "CLOSE_WAIT" => BaglantiDurumu::KapatmaBekliyor,
            "LAST_ACK" => BaglantiDurumu::SonAck,
            "CLOSING" => BaglantiDurumu::Kapaniyor,
            "BOUND" => BaglantiDurumu::Bos,
            _ => BaglantiDurumu::Bilinmiyor,
        }
    }

    /// `/proc/net/tcp` içindeki iki onaltılık durum kodunu çözer.
    pub fn proc_kodundan(kod: u8) -> Self {
        match kod {
            0x01 => BaglantiDurumu::Kuruldu,
            0x02 => BaglantiDurumu::SynGonderildi,
            0x03 => BaglantiDurumu::SynAlindi,
            0x04 => BaglantiDurumu::FinWait1,
            0x05 => BaglantiDurumu::FinWait2,
            0x06 => BaglantiDurumu::ZamanBekliyor,
            0x07 => BaglantiDurumu::Kapali,
            0x08 => BaglantiDurumu::KapatmaBekliyor,
            0x09 => BaglantiDurumu::SonAck,
            0x0A => BaglantiDurumu::Dinliyor,
            0x0B => BaglantiDurumu::Kapaniyor,
            _ => BaglantiDurumu::Bilinmiyor,
        }
    }

    /// Kısa, Türkçe, insan okunur etiket.
    pub fn metin(self) -> &'static str {
        match self {
            BaglantiDurumu::Dinliyor => "dinliyor",
            BaglantiDurumu::Kuruldu => "kuruldu",
            BaglantiDurumu::SynGonderildi => "syn gonderildi",
            BaglantiDurumu::SynAlindi => "syn alindi",
            BaglantiDurumu::FinWait1 => "fin-wait-1",
            BaglantiDurumu::FinWait2 => "fin-wait-2",
            BaglantiDurumu::ZamanBekliyor => "zaman bekliyor",
            BaglantiDurumu::KapatmaBekliyor => "kapatma bekliyor",
            BaglantiDurumu::SonAck => "son ack",
            BaglantiDurumu::Kapaniyor => "kapaniyor",
            BaglantiDurumu::Kapali => "kapali",
            BaglantiDurumu::Bos => "bos",
            BaglantiDurumu::Bilinmiyor => "bilinmiyor",
        }
    }
}

/// Tek bir ağ bağlantısının platformdan bağımsız kaydı.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Baglanti {
    /// Taşıma katmanı protokolü.
    pub protokol: Protokol,
    /// Yerel uç adresi.
    pub yerel_adres: IpAddr,
    /// Yerel uç portu.
    pub yerel_port: u16,
    /// Uzak uç adresi; UDP'de ve `0.0.0.0:0` yer tutucusunda `None` olur.
    pub uzak_adres: Option<IpAddr>,
    /// Uzak uç portu.
    pub uzak_port: Option<u16>,
    /// Bağlantı durumu.
    pub durum: BaglantiDurumu,
    /// Bağlantıyı sahiplenen sürecin kimliği.
    pub pid: Option<u32>,
}

impl Baglanti {
    /// Bu bağlantı bir dinleme soketi mi?
    pub fn dinleyen_mi(&self) -> bool {
        self.durum == BaglantiDurumu::Dinliyor
    }

    /// Bu bağlantı küresel olarak yönlendirilebilir bir adrese mi çıkıyor?
    ///
    /// Döngüsel (loopback), özel (private), bağlantı-yerel (link-local) ve
    /// belirsiz (unspecified) adresler `false` döner. DNS çözümlemesi yapılmaz:
    /// burada yalnızca IP metni değerlendirilir.
    pub fn disari_cikan_mi(&self) -> bool {
        match self.uzak_adres {
            Some(adres) => self.durum == BaglantiDurumu::Kuruldu && genel_adres_mi(&adres),
            None => false,
        }
    }

    /// `yerel:port -> uzak:port` biçiminde uç listesi (protokol adı olmadan).
    pub fn ucl_listesi(&self) -> String {
        let uzak = match (self.uzak_adres, self.uzak_port) {
            (Some(a), Some(p)) => format!("{}:{}", a, p),
            _ => "-".to_string(),
        };
        format!("{}:{} -> {}", self.yerel_adres, self.yerel_port, uzak)
    }

    /// `PROTO yerel:port -> uzak:port` biçiminde kısa bir tanım.
    pub fn tanim(&self) -> String {
        format!("{} {}", self.protokol.etiket(), self.ucl_listesi())
    }
}

/// Tek bir sürecin platformdan bağımsız kaydı.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Surec {
    /// Süreç kimliği.
    pub pid: u32,
    /// Üst sürecin kimliği; bilinmiyorsa `0`.
    pub ppid: u32,
    /// Sürecin görünen adı (`Image Name`, `comm`).
    pub ad: String,
    /// Komut satırı; kaynak sağlamıyorsa boş.
    pub komut_satiri: String,
    /// Yürütülebilir dosyanın yolu; kaynak sağlamıyorsa `None`.
    pub yol: Option<PathBuf>,
    /// Yüzde cinsinden CPU kullanımı; tek ölçümde hesaplanamıyorsa `None`.
    pub cpu_yuzde: Option<f64>,
    /// KiB cinsinden bellek kullanımı.
    pub bellek_kib: u64,
    /// Oturum kimliği (Windows oturum no veya Linux oturum/sid).
    pub oturum: Option<u32>,
    /// Süreci çalıştıran kullanıcı adı; kaynak sağlamıyorsa `None`.
    pub kullanici: Option<String>,
    /// Kullanıcı kimliği (UID); Linux tarafında doldurulur.
    pub uid: Option<u32>,
    /// Sürecin çalışma durumu.
    pub durum: SurecDurumu,
}

impl Surec {
    /// Bellek kullanımını MiB cinsine çevirir.
    pub fn bellek_mib(&self) -> u64 {
        self.bellek_kib / 1024
    }

    /// Kaynak sağladıysa yürütülebilir dosyanın adını döner.
    pub fn dosya_adi(&self) -> Option<&str> {
        self.yol
            .as_deref()
            .and_then(|p| p.file_name())
            .and_then(|a| a.to_str())
    }

    /// Bir satırlık insan okunur özet.
    pub fn ozet(&self) -> String {
        format!(
            "{:<8} {:<8} {:<24} {:>8} MiB  {:<10} {}",
            self.pid,
            self.ppid,
            truncate(&self.ad, 24),
            self.bellek_mib(),
            self.durum.metin(),
            self.kullanici.as_deref().unwrap_or("-")
        )
    }
}

/// Başlangıçta kendiliğinden çalışan bir girdinin türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StartupTuru {
    /// Kullanıcının Başlangıç klasörü (Windows) veya autostart dizini (Linux).
    BaslangicKlasoru,
    /// XDG `~/.config/autostart` dizinindeki `.desktop` dosyası.
    XdgAutostart,
    /// `HKCU/HKLM\...\CurrentVersion\Run` kayıt defteri girdisi.
    RunAnahtari,
    /// Türü belirlenemeyen bir girdi.
    Bilinmiyor,
}

impl StartupTuru {
    /// Türü etiketleyen kısa Türkçe sözcük.
    pub fn etiket(self) -> &'static str {
        match self {
            StartupTuru::BaslangicKlasoru => "baslangic-klasoru",
            StartupTuru::XdgAutostart => "xdg-autostart",
            StartupTuru::RunAnahtari => "run-anahtari",
            StartupTuru::Bilinmiyor => "bilinmiyor",
        }
    }

    /// Bir dosya yolundan türü kestirir.
    pub fn yoldan(yol: &Path) -> Self {
        let uzantilar = ["lnk", "url", "exe", "bat", "cmd", "vbs"];
        let ad = yol
            .file_name()
            .and_then(|a| a.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if ad.ends_with(".desktop") {
            return StartupTuru::XdgAutostart;
        }
        if uzantilar.iter().any(|u| ad.ends_with(u)) {
            return StartupTuru::BaslangicKlasoru;
        }
        StartupTuru::Bilinmiyor
    }
}

/// Başlangıç girdisi envanterinin tek bir kaydı.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartupGirdi {
    /// Girdinin bulunduğu konum (klasör girdisinde dosya yolu).
    pub konum: PathBuf,
    /// Girdinin görünen adı.
    pub ad: String,
    /// Girdi türü.
    pub tur: StartupTuru,
    /// Girdi devre dışı bırakılmış mı?
    pub etkin: bool,
    /// Girdinin gösterdiği hedef (kayıt değeri veya `.desktop` `Exec` satırı).
    pub hedef: Option<String>,
}

impl StartupGirdi {
    /// Bir satırlık insan okunur özet.
    pub fn ozet(&self) -> String {
        format!(
            "{:<20} {:<18} {:<10} {}",
            truncate(&self.ad, 20),
            self.tur.etiket(),
            if self.etkin { "etkin" } else { "kapali" },
            self.konum.display()
        )
    }
}

/// RFC 5737 belgeleme bloklarını (`192.0.2.0/24`, `198.51.100.0/24`,
/// `203.0.113.0/24`) küresel saymayan yardımcı.
///
/// `Ipv4Addr::is_documentation` Rust 1.80'de kararlı hâle geldiği için
/// MSRV 1.74 gereksinimini karşılamak üzere elle uygulanır.
fn ipv4_belgeleme_mi(v4: &Ipv4Addr) -> bool {
    let o = v4.octets();
    (o[0] == 192 && o[1] == 0 && o[2] == 2)
        || (o[0] == 198 && o[1] == 51 && o[2] == 100)
        || (o[0] == 203 && o[1] == 0 && o[2] == 113)
}

/// Bir adresin küresel olarak yönlendirilebilir olup olmadığını söyler.
///
/// Döngüsel (`127.0.0.0/8`, `::1`), özel (`10/8`, `172.16/12`, `192.168/16`,
/// `fc00::/7`), bağlantı-yerel (`169.254/16`, `fe80::/10`), çok noktalı yayın
/// ve belirsiz (`0.0.0.0`, `::`) adresler küresel sayılmaz.
pub fn genel_adres_mi(adres: &IpAddr) -> bool {
    match adres {
        IpAddr::V4(v4) => {
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_unspecified()
                || v4.is_multicast()
                || ipv4_belgeleme_mi(v4)
                || v4.octets()[0] == 0
                || v4.octets()[0] >= 240)
        }
        IpAddr::V6(v6) => {
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                // fc00::/7 — benzersiz yerel adresler
                || (v6.octets()[0] & 0xFE) == 0xFC
                // fe80::/10 — bağlantı-yerel
                || (v6.octets()[0] == 0xFE && (v6.octets()[1] & 0xC0) == 0x80)
                // 2001:db8::/32 — belgeleme
                || (v6.octets()[0] == 0x20
                    && v6.octets()[1] == 0x01
                    && v6.octets()[2] == 0x0D
                    && v6.octets()[3] == 0xB8))
        }
    }
}

/// Bir yolun "şüpheli konum" listesinde olup olmadığını ve neden olduğunu söyler.
///
/// Liste, uygulamanın taşınabilir kırılma noktasını oluşturan konumları
/// kapsar: geçici dizinler, profil önbellekleri, indirme klasörleri ve
/// "herkese açık" konumlar. Eşleşme bulunamazsa `None` döner.
///
/// **Muaf konumlar.** `AppData` altındaki üç konum meşru program yükleme ve
/// uygulama verisi yeridir; bu konumlara yerleşen her programı şüpheli saymak
/// her kullanıcıda yüksek sayıda yanlış pozitif üretir (rapor § 12 / R1).
/// Muafiyet, şüpheli desen taramasından **önce** uygulanır.
pub fn supheli_konum_mi(yol: &Path) -> Option<&'static str> {
    let ham = yol
        .to_string_lossy()
        .to_ascii_lowercase()
        .replace('/', "\\");
    const MUAF: [&str; 4] = [
        "\\appdata\\local\\programs\\",
        "\\appdata\\local\\microsoft\\windowsapps\\",
        "\\appdata\\local\\microsoft\\winget\\",
        "\\appdata\\roaming\\microsoft\\windows\\",
    ];
    if MUAF.iter().any(|desen| ham.contains(desen)) {
        return None;
    }
    let adaylar: [(&str, &str); 10] = [
        ("\\appdata\\", "AppData (profil icinde gizli veri konumu)"),
        (
            "\\programdata\\",
            "ProgramData (tumu kullaniciya acik ortak konum)",
        ),
        ("\\downloads\\", "Downloads (indirilen dosya konumu)"),
        ("\\users\\public\\", "Users\\Public (herkese acik konum)"),
        ("\\windows\\temp\\", "Windows\\Temp (sistem gecici konumu)"),
        ("\\temp\\", "Temp (gecici konum)"),
        ("\\tmp\\", "gecici konum"),
        ("\\var\\tmp\\", "gecici konum"),
        ("\\dev\\shm\\", "paylasilan bellek (RAM) konumu"),
        ("\\.cache\\", "uygulama onbellegi konumu"),
    ];
    adaylar
        .iter()
        .find(|(desen, _)| ham.contains(desen))
        .map(|(_, gerekce)| *gerekce)
}

/// Dizeyi verilen genişliğe kısaltır; taşarsa sonuna `..` ekler.
pub fn truncate(girdi: &str, genislik: usize) -> String {
    if genislik == 0 {
        return String::new();
    }
    let karakterler: Vec<char> = girdi.chars().collect();
    if karakterler.len() <= genislik {
        return girdi.to_string();
    }
    if genislik <= 2 {
        return karakterler[..genislik].iter().collect();
    }
    let mut sonuc: String = karakterler[..genislik - 2].iter().collect();
    sonuc.push_str("..");
    sonuc
}

/// `/proc/net/tcp` ve `tasklist` çıktısında geçen onaltılık IPv4 adresini çözer.///
/// Linux çekirdeği adresi ağ baytı sırasında 32 bitlik bir sözcük olarak
/// yazar; bu yüzden dört onaltılık bayt ters çevrilerek okunur.
pub fn proc_ipv4_coz(ondortlilik: &str) -> Option<IpAddr> {
    let baytlar = onaltilik_dort_bayt(ondortlilik)?;
    let ters: [u8; 4] = [baytlar[3], baytlar[2], baytlar[1], baytlar[0]];
    Some(IpAddr::V4(Ipv4Addr::from(ters)))
}

/// `/proc/net/tcp6` çıktısındaki 32 onaltılık IPv6 adresini çözer.
///
/// Çekirdek adresi dört adet 32 bitlik sözcük hâlinde, her biri makine
/// baytı sırasında yazar; her dört bayt grubu ayrı ayrı ters çevrilir.
pub fn proc_ipv6_coz(onaltilik: &str) -> Option<IpAddr> {
    if onaltilik.len() != 32 {
        return None;
    }
    let baytlar = onaltilik_dort_bayt(onaltilik)?;
    let mut ters = [0u8; 16];
    for blok in 0..4 {
        let k = blok * 4;
        ters[k] = baytlar[k + 3];
        ters[k + 1] = baytlar[k + 2];
        ters[k + 2] = baytlar[k + 1];
        ters[k + 3] = baytlar[k];
    }
    Some(IpAddr::V6(Ipv6Addr::from(ters)))
}

/// Onaltılık metni bayt dizisine çevirir; uzunluk 4 veya 32 olmalıdır.
fn onaltilik_dort_bayt(onaltilik: &str) -> Option<Vec<u8>> {
    let beklenen = match onaltilik.len() {
        8 => 4,
        32 => 16,
        _ => return None,
    };
    (0..beklenen)
        .map(|i| u8::from_str_radix(&onaltilik[i * 2..i * 2 + 2], 16).ok())
        .collect()
}

impl fmt::Display for SurecDurumu {
    fn fmt(&self, bicik: &mut fmt::Formatter<'_>) -> fmt::Result {
        bicik.write_str(self.metin())
    }
}

impl fmt::Display for BaglantiDurumu {
    fn fmt(&self, bicik: &mut fmt::Formatter<'_>) -> fmt::Result {
        bicik.write_str(self.metin())
    }
}

#[cfg(test)]
// Gerekçe: expect/unwrap yalnızca test içinde kullanılır ve testin
// başarısızlık mesajıdır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn proc_durum_harfleri_ayirt_edilir() {
        assert_eq!(SurecDurumu::proc_harfinden('R'), SurecDurumu::Calisiyor);
        assert_eq!(SurecDurumu::proc_harfinden('S'), SurecDurumu::Uyuyor);
        assert_eq!(SurecDurumu::proc_harfinden('Z'), SurecDurumu::Zombi);
        assert_eq!(SurecDurumu::proc_harfinden('Q'), SurecDurumu::Bilinmiyor);
    }

    #[test]
    fn proc_ipv4_kucuk_bayt_sirasinda_ters_cozulur() {
        let adres = proc_ipv4_coz("0100007F").expect("cözülemedi");
        assert_eq!(adres, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
    }

    #[test]
    fn proc_ipv4_bozuk_girdi_none_doner() {
        assert!(proc_ipv4_coz("ZZZZ").is_none());
        assert!(proc_ipv4_coz("0100").is_none());
    }

    #[test]
    fn proc_ipv6_kucuk_bayt_sirasinda_ters_cozulur() {
        let onaltilik = "00000000000000000000000001000000";
        let adres = proc_ipv6_coz(onaltilik).expect("çözülemedi");
        assert_eq!(adres, IpAddr::V6(Ipv6Addr::LOCALHOST));
    }

    #[test]
    fn genel_adres_yerel_adresleri_eler() {
        assert!(!genel_adres_mi(&IpAddr::V4(Ipv4Addr::LOCALHOST)));
        assert!(!genel_adres_mi(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 5))));
        assert!(!genel_adres_mi(&IpAddr::V4(Ipv4Addr::new(169, 254, 3, 2))));
        assert!(!genel_adres_mi(&IpAddr::V4(Ipv4Addr::UNSPECIFIED)));
        assert!(genel_adres_mi(&IpAddr::V4(Ipv4Addr::new(52, 109, 8, 20))));
    }

    #[test]
    fn genel_adres_ipv6_yerel_kapsamlari_eler() {
        assert!(!genel_adres_mi(&IpAddr::V6(Ipv6Addr::LOCALHOST)));
        let benzersiz = Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 1);
        assert!(!genel_adres_mi(&IpAddr::V6(benzersiz)));
        let baglanti_yerel = Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1);
        assert!(!genel_adres_mi(&IpAddr::V6(baglanti_yerel)));
    }

    #[test]
    fn supheli_konum_appdata_ve_tmp_eslesir() {
        let yol = Path::new("C:\\Users\\x\\AppData\\Local\\Temp\\svchost.exe");
        assert_eq!(
            supheli_konum_mi(yol),
            Some("AppData (profil icinde gizli veri konumu)")
        );
        let yol = Path::new("/var/tmp/kurulum.sh");
        assert!(supheli_konum_mi(yol).is_some());
    }

    #[test]
    fn supheli_konum_program_dizini_eslesmez() {
        let yol = Path::new("C:\\Program Files\\OrnekUygulama\\uygulama.exe");
        assert!(supheli_konum_mi(yol).is_none());
        assert!(supheli_konum_mi(Path::new("/usr/bin/ls")).is_none());
    }

    #[test]
    fn supheli_konum_meşru_appdata_konumlarini_muaf_tutar() {
        // Kullanıcı başına kurulum ve uygulama verisi konumları meşrudur.
        for yol in [
            "C:\\Users\\K\\AppData\\Local\\Programs\\Proton\\Drive\\ProtonDrive.exe",
            "C:\\Users\\K\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\a.lnk",
            "C:\\Users\\K\\AppData\\Local\\Microsoft\\WindowsApps\\Notepad.exe",
            "C:\\Users\\K\\AppData\\Local\\Microsoft\\WinGet\\Packages\\acme.tool\\tool.exe",
        ] {
            assert!(
                supheli_konum_mi(Path::new(yol)).is_none(),
                "{} muaf olmalı",
                yol
            );
        }
    }

    #[test]
    fn truncate_kisa_girdede_dokunmaz() {
        assert_eq!(truncate("abc", 8), "abc");
        assert_eq!(truncate("abcdefgh", 5), "abc..");
        assert_eq!(truncate("abcdefgh", 0), "");
    }

    #[test]
    fn baslangic_turu_uzantidan_kestirilir() {
        assert_eq!(
            StartupTuru::yoldan(Path::new("/home/x/.config/autostart/a.desktop")),
            StartupTuru::XdgAutostart
        );
        assert_eq!(
            StartupTuru::yoldan(Path::new("C:/.../Startup/ucant.lnk")),
            StartupTuru::BaslangicKlasoru
        );
        assert_eq!(
            StartupTuru::yoldan(Path::new("C:/.../Startup/bilinmeyen.dat")),
            StartupTuru::Bilinmiyor
        );
    }

    #[test]
    fn baglanti_disi_cikan_kuruldu_ve_genel_adres_ister() {
        let dis = Baglanti {
            protokol: Protokol::Tcp,
            yerel_adres: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5)),
            yerel_port: 51000,
            uzak_adres: Some(IpAddr::V4(Ipv4Addr::new(52, 109, 8, 20))),
            uzak_port: Some(443),
            durum: BaglantiDurumu::Kuruldu,
            pid: Some(42),
        };
        assert!(dis.disari_cikan_mi());
        let yerel = Baglanti {
            uzak_adres: Some(IpAddr::V4(Ipv4Addr::LOCALHOST)),
            ..dis.clone()
        };
        assert!(!yerel.disari_cikan_mi());
    }
}
