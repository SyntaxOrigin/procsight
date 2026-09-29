//! Şüpheli davranış kuralları: yedi kural, açıklama ve doğrulama adımı.
//!
//! ## Etik sınır
//!
//! Bu modül bir savunma aracı değildir ve olay tespiti yapmaz. Her kural bir
//! **gözlem** üretir, bir **açıklama** ve bir **doğrulama adımı** taşır.
//! Hiçbir kural "zararlı" demez, hiçbir kural engelleme önermez, hiçbir kural
//! bir süreci sonlandırma, dosya silme veya girdi devre dışı bırakma önermez.
//! Bu sınır yalnızca sözlü değildir: [`Bulgu`] yapısında bir müdahale alanı
//! **yoktur**; alanları sayesizdir ve yalnızca kanıt taşır.
//!
//! ## Kuralların kaynağı
//!
//! Raporda "her kural açık metin, eşleşen örnek ve kapamak için kullanıcı
//! adımı taşır" ve "hiçbir kural 'zararlı' demiyor" denir. Yedi kural
//! raporun kabul kriterleriyle birebir eşleşir:
//!
//! 1. Gizli ad — makine üretimi görünen ad.
//! 2. Şüpheli konum — geçici/gizli dosya sistemi konumu.
//! 3. Yüksek bellek — eşiği aşan bellek kullanımı.
//! 4. Kalıcılık girdisi — şüpheli hedefe işaret eden başlangıç girdisi.
//! 5. Beklenmeyen ağ bağlantısı — bilinmeyen port üzerinden dışarı çıkış.
//! 6. Kısa ömürlü çok sayıda süreç — yoklama aralığında doğan ve ölen kütle.
//! 7. Yetki yükseltme girişimi — kullanıcı kimliği geçişi veya dünyaya açık
//!    konumdan çalıştırma.

use serde::{Deserialize, Serialize};

use crate::model::{supheli_konum_mi, Baglanti, StartupGirdi, Surec};
use crate::motor::YasamOzeti;

/// Yedi şüpheli davranış kuralından biri.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Kural {
    /// Görünen adı makine üretimli görünen süreç.
    GizliAd,
    /// Yürütülebilir dosyası geçici veya gizli konumda olan süreç.
    SupheliKonum,
    /// Bellek kullanımı eşiği aşan süreç.
    YuksekBellek,
    /// Hedefi şüpheli konumda olan başlangıç girdisi.
    KalicilikGirdisi,
    /// Bilinmeyen bir port üzerinden dışarı çıkan bağlantı.
    BeklenmeyenAgBaglantisi,
    /// Tek yoklamada doğup ölen süreç kütleşmesi.
    KisaOmurluCokSayidaSurec,
    /// Üst süreçten farklı kullanıcı kimliğiyle çalışan süreç.
    YetkiYukseltmeGirisimi,
}

/// Yedi kuralın tamamı, rapordaki sırayla.
pub const TUM_KURALLAR: [Kural; 7] = [
    Kural::GizliAd,
    Kural::SupheliKonum,
    Kural::YuksekBellek,
    Kural::KalicilikGirdisi,
    Kural::BeklenmeyenAgBaglantisi,
    Kural::KisaOmurluCokSayidaSurec,
    Kural::YetkiYukseltmeGirisimi,
];

impl Kural {
    /// Kuralın kısa Türkçe adı.
    pub fn ad(self) -> &'static str {
        match self {
            Kural::GizliAd => "gizli ad",
            Kural::SupheliKonum => "supheli konum",
            Kural::YuksekBellek => "yuksek bellek",
            Kural::KalicilikGirdisi => "kalicilik girdisi",
            Kural::BeklenmeyenAgBaglantisi => "beklenmeyen ag baglantisi",
            Kural::KisaOmurluCokSayidaSurec => "kisa omurlu cok sayida surec",
            Kural::YetkiYukseltmeGirisimi => "yetki yukseltme girisimi",
        }
    }

    /// Kuralın neyi **gözlemlediğini** anlatan zorunlu açıklama.
    pub fn aciklama(self) -> &'static str {
        match self {
            Kural::GizliAd => {
                "Gorsel ad, insanin secmekte zorlanacagi bicimde makine uretilmis \
                 gorunuyor: harf-rakam karmasi, kabuk/kontrol karakteri veya asiri uzun ad. \
                 Bosluk iceren meşru adlar (orn. System Idle Process) bu kurala girmez."
            }
            Kural::SupheliKonum => {
                "Yurutulebilir dosya gecici veya profile gizlenmis bir konumdan \
                 calisiyor. Bu konumlar normalde yalnizca indirilen/uretilen dosyalar icin kullanilir."
            }
            Kural::YuksekBellek => {
                "Surec belirlenen esigin uzerinde bellek tuketiyor. Bu tek basina bir \
                 sorun degildir; tarayici ve video duzenleyici gibi uygulamalar da \
                 bu esigi asabilir."
            }
            Kural::KalicilikGirdisi => {
                "Sistem acilista kendiliginden calisan bir girdi, gecici veya gizli \
                 konumdaki bir dosyayi isaret ediyor. Kisayolun kendisi standart baslangic \
                 klasorunde olsa bile hedefi denetlenir."
            }
            Kural::BeklenmeyenAgBaglantisi => {
                "Kurulmus bir baglanti, ev/ofis agi disindaki bir adrese ve gunluk \
                 hayatta sik gorulen portlar disindaki bir porta gidiyor."
            }
            Kural::KisaOmurluCokSayidaSurec => {
                "Belirli sayinin uzerinde surec, yoklama araligindan kisa surede \
                 baslayip bitti. Kurulum, guncellestirme ve derleme gibi isler de bu \
                 gorunumu uretebilir."
            }
            Kural::YetkiYukseltmeGirisimi => {
                "Surec, ust surucunden farkli bir kullanici kimligiyle calisiyor veya \
                 herkesin yazabildigi bir konumdan baslatiliyor. Ikisi de yazma yetkisi \
                 ele gecirme icin kullanilabilecek bir yol olabilir."
            }
        }
    }

    /// Kullanıcının bulguyu kendi doğrulaması için izleyeceği adım.
    ///
    /// Bu adım **her zaman gözlem ve sorgulamadır**; hiçbir zaman müdahale
    /// önermez (dosya silme, girdi kaldırma, surec sonlandirma yoktur).
    pub fn sonraki_adim(self) -> &'static str {
        match self {
            Kural::GizliAd => {
                "Dosyaya sag tikla > Ozellikler > Ayrintilar sekmesinde yayimcivertifikasini \
                 ve imzayi incele; bu adi hangi yazilimin kullandigini arayandan dogrula."
            }
            Kural::SupheliKonum => {
                "Dosyanin bulundugu konumu ac ve hangi programin buraya yazdigini \
                 programin kendi ayarlarindan kontrol et; konum gecici ise program \
                 gecici dosyadan calisiyor demektir."
            }
            Kural::YuksekBellek => {
                "Gorev Yoneticisi > Ayrintilar sekmesinde bu surecin normalde kac MiB \
                 kullandigini karsilastir; surekli buyuyorsa uygulamanin kendi \
                 ayar sayfasindan bellege sinir koy."
            }
            Kural::KalicilikGirdisi => {
                "Girdinin hedefini ac ve uygulamanin kurulumunu kimin yaptigini sor; \
                 programin kendi ayarlarinda bu baslangic girdisini kapatma secenegi \
                 var mi bak."
            }
            Kural::BeklenmeyenAgBaglantisi => {
                "Baglantiyi yapan uygulamayi ac ve ayarlarindaki ag erisimini incele; \
                 sunucunun adi bu programin hizmetidir; tanimiyorsan ayri bir agda \
                 (misafir Wi-Fi) baglanip ayni davranisi gozlemle."
            }
            Kural::KisaOmurluCokSayidaSurec => {
                "O donemde bir guncellestirme, derleme veya kurulum yapip yapmadigini \
                 hatirla; yapmadysan surec adlarini rapordan incele."
            }
            Kural::YetkiYukseltmeGirisimi => {
                "Ust surecin hangi program oldugunu ve bu surecin neden farkli bir \
                 kullanici kimligiyle basladigini program ayarlarindan dogrula; \
                 yonetici calistirma gerekiyorsa bunu yapan ayari bul."
            }
        }
    }
}

/// Bir kuralın ürettiği tek bir gözlem.
///
/// `mudahale` alanı **yoktur**: bu yapı kanıt, açıklama ve sorgulama adımı
/// taşır; hiçbir şeyi değiştiremez.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bulgu {
    /// Hangi kural tetiklendi.
    pub kural: Kural,
    /// Gözlemin konusu (süreç kimliği, yol veya bağlantı tanımı).
    pub konu: String,
    /// Kuralı tetikleyen somut kanıt satırları.
    pub kanit: Vec<String>,
    /// Kuralın neyi gözlemlediğini hatırlatan metin.
    pub aciklama: String,
    /// Kullanıcının izleyeceği doğrulama adımı.
    pub sonraki_adim: String,
}

impl Bulgu {
    /// Yeni bir bulgu kurar.
    pub fn yeni(kural: Kural, konu: impl Into<String>, kanit: Vec<String>) -> Self {
        Bulgu {
            kural,
            konu: konu.into(),
            kanit,
            aciklama: kural.aciklama().to_string(),
            sonraki_adim: kural.sonraki_adim().to_string(),
        }
    }

    /// Tek satırlık insan okunur özet.
    pub fn ozet(&self) -> String {
        format!(
            "[{}] {} — {}",
            self.kural.ad(),
            self.konu,
            self.kanit.join("; ")
        )
    }
}

/// Kural eşikleri. Varsayılanlar raporun "tipik kullanım" senaryosuna göre seçilmiştir.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KuralEsikleri {
    /// [`Kural::YuksekBellek`] için MiB eşiği.
    pub yuksek_bellek_mib: u64,
    /// [`Kural::KisaOmurluCokSayidaSurec`] için adet eşiği.
    pub kisa_omurlu_esik: usize,
    /// [`Kural::BeklenmeyenAgBaglantisi`] için "tanıdık" sayılan portlar.
    pub bilinen_portlar: Vec<u16>,
}

impl Default for KuralEsikleri {
    fn default() -> Self {
        KuralEsikleri {
            yuksek_bellek_mib: 512,
            kisa_omurlu_esik: 5,
            // Günlük hayatta sık görülen hizmet portları: DNS, web, e-posta,
            // SSH ve anlık mesajlasma. Bunlar "tanıdık" sayılır; geri kalan
            // her port rapor § 05'in S4 kabul kriteri gereği incelenir.
            bilinen_portlar: vec![22, 53, 80, 123, 443, 465, 587, 993, 995, 5222, 5223],
        }
    }
}

impl KuralEsikleri {
    /// Verilen port tanıdık mı?
    pub fn port_tanidik_mi(&self, port: u16) -> bool {
        self.bilinen_portlar.contains(&port)
    }
}

/// Kuralların değerlendirileceği salt-okunur bağlam.
#[derive(Debug, Clone, Copy)]
pub struct KuralBaglami<'a> {
    /// Anlık görüntüdeki süreçler.
    pub surecler: &'a [Surec],
    /// Anlık görüntüdeki bağlantılar.
    pub baglantilar: &'a [Baglanti],
    /// Anlık görüntüdeki başlangıç girdileri.
    pub baslangic: &'a [StartupGirdi],
    /// Yoklama döngüsünün ürettiği yaşam istatistiği.
    pub yasam: &'a YasamOzeti,
}

/// Hiçbir yoklama yapılmadığını belirten paylaşılan boş yaşam özeti.
///
/// `static` olmasının nedeni `KuralBaglami::bos()`'un geçici bir `&` döndürmesini
/// önlemektir: `Vec::new()` sabit ifade olduğu için `static` içinde tanımlanabilir.
static BOS_YASAM: YasamOzeti = YasamOzeti {
    donem: 0,
    yeni: Vec::new(),
    kayboldu: Vec::new(),
    kisa_omurlu: Vec::new(),
    yasayan: 0,
};

impl<'a> KuralBaglami<'a> {
    /// Boş bir bağlam kurar (kural kataloğunu yazdırmak için).
    pub fn bos() -> Self {
        KuralBaglami {
            surecler: &[],
            baglantilar: &[],
            baslangic: &[],
            yasam: &BOS_YASAM,
        }
    }
}

/// Bir sürecin görünen adının "gizli" olup olmadığını ve nedenini söyler.
///
/// Uzantı atılır; kalan **taban** üzerinde üç ölçüt uygulanır:
///
/// 1. Alışılmadık karakter (kontrol karakteri, kabuk işareti ya da ASCII dışı
///    karakter) bulunması. **Boşluk istisnadır**: "System Idle Process" ve
///    "Windows Defender" gibi meşru Windows süreç adları boşluk içerir ve
///    işaretlenmemelidir.
/// 2. En az 6 karakter, en az 3 rakam, en az bir büyük ve bir küçük harf —
///    klasik "rastgele üretilmiş ad" görünümü.
/// 3. 32 karakteri aşan taban.
pub fn gizli_ad_mi(ad: &str) -> Option<&'static str> {
    let taban = match ad.rsplit_once('.') {
        Some((on, _uzanti)) if !on.is_empty() => on,
        _ => ad,
    };
    if taban.is_empty() {
        return None;
    }
    let izinli = |c: char| c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '-' | '.' | '+');
    if taban.chars().any(|c| !izinli(c)) {
        return Some("adda beklenmeyen karakter var");
    }
    let rakam = taban.chars().filter(char::is_ascii_digit).count();
    let buyuk = taban.chars().filter(|c| c.is_ascii_uppercase()).count();
    let kucuk = taban.chars().filter(|c| c.is_ascii_lowercase()).count();
    if taban.chars().count() >= 6 && rakam >= 3 && buyuk >= 1 && kucuk >= 1 {
        return Some("ad makine uretilmis gorunuyor (harf-rakam karmasi)");
    }
    if taban.chars().count() > 32 {
        return Some("ad asiri uzun");
    }
    None
}

/// Bir başlangıç girdisinin neden şüpheli olduğunu söyler.
///
/// Standart Başlangıç klasörü (Windows) ve XDG autostart dizininin **kendisi**
/// muaftır: bu konumlar modern sistemlerde `AppData` altındadır ve muaf tutulmazsa
/// kural her girdiyi işaretlerdi. Denetim **hedefe** yapılır.
pub fn kalicilik_suspicious_mi(girdi: &StartupGirdi) -> Option<&'static str> {
    if let Some(hedef) = &girdi.hedef {
        return supheli_konum_mi(std::path::Path::new(hedef));
    }
    if crate::kaynak::windows::standart_baslangic_klasoru_mi(&girdi.konum) {
        return None;
    }
    supheli_konum_mi(&girdi.konum)
}

fn surec_yolu(surec: &Surec) -> Option<std::path::PathBuf> {
    if let Some(yol) = &surec.yol {
        return Some(yol.clone());
    }
    surec
        .komut_satiri
        .split_whitespace()
        .next()
        .map(std::path::PathBuf::from)
}

/// Tek bir kuralı değerlendirir ve ürettiği gözlemleri döndürür.
///
/// Kural bulgu üretmezse boş vektör döner; **her kuralın pozitif ve negatif
/// yolu test edilir**.
pub fn degerlendir(kural: Kural, baglam: &KuralBaglami<'_>, esikler: &KuralEsikleri) -> Vec<Bulgu> {
    match kural {
        Kural::GizliAd => {
            let mut bulgular = Vec::new();
            for surec in baglam.surecler {
                if let Some(gerekce) = gizli_ad_mi(&surec.ad) {
                    bulgular.push(Bulgu::yeni(
                        kural,
                        format!("pid={} ad={}", surec.pid, surec.ad),
                        vec![
                            gerekce.to_string(),
                            format!("ust surec={}", surec.ppid),
                            surec
                                .yol
                                .as_deref()
                                .map(|y| format!("yol={}", y.display()))
                                .unwrap_or_else(|| "yol=bilinmiyor".into()),
                        ],
                    ));
                }
            }
            bulgular
        }
        Kural::SupheliKonum => {
            let mut bulgular = Vec::new();
            for surec in baglam.surecler {
                let Some(yol) = surec_yolu(surec) else {
                    continue;
                };
                if let Some(gerekce) = supheli_konum_mi(&yol) {
                    bulgular.push(Bulgu::yeni(
                        kural,
                        format!("pid={} ad={}", surec.pid, surec.ad),
                        vec![format!("konum={}", yol.display()), gerekce.to_string()],
                    ));
                }
            }
            bulgular
        }
        Kural::YuksekBellek => {
            let mut bulgular = Vec::new();
            for surec in baglam.surecler {
                let mib = surec.bellek_mib();
                if mib >= esikler.yuksek_bellek_mib {
                    bulgular.push(Bulgu::yeni(
                        kural,
                        format!("pid={} ad={}", surec.pid, surec.ad),
                        vec![
                            format!("{} MiB (esik {} MiB)", mib, esikler.yuksek_bellek_mib),
                            format!("ust surec={}", surec.ppid),
                        ],
                    ));
                }
            }
            bulgular
        }
        Kural::KalicilikGirdisi => {
            let mut bulgular = Vec::new();
            for girdi in baglam.baslangic {
                if let Some(gerekce) = kalicilik_suspicious_mi(girdi) {
                    bulgular.push(Bulgu::yeni(
                        kural,
                        format!("{} ({})", girdi.ad, girdi.tur.etiket()),
                        vec![
                            format!("konum={}", girdi.konum.display()),
                            girdi
                                .hedef
                                .clone()
                                .map(|h| format!("hedef={}", h))
                                .unwrap_or_else(|| "hedef=belirtilmemis".into()),
                            gerekce.to_string(),
                        ],
                    ));
                }
            }
            bulgular
        }
        Kural::BeklenmeyenAgBaglantisi => {
            let mut bulgular = Vec::new();
            for baglanti in baglam.baglantilar {
                if !baglanti.disari_cikan_mi() {
                    continue;
                }
                let port = baglanti.uzak_port.unwrap_or(0);
                if esikler.port_tanidik_mi(port) {
                    continue;
                }
                bulgular.push(Bulgu::yeni(
                    kural,
                    baglanti.tanim(),
                    vec![
                        format!("uzak port {} tanidik port listesinde degil", port),
                        format!(
                            "sahip pid={}",
                            baglanti
                                .pid
                                .map(|p| p.to_string())
                                .unwrap_or_else(|| "cozulemedi".into())
                        ),
                    ],
                ));
            }
            bulgular
        }
        Kural::KisaOmurluCokSayidaSurec => {
            let sayi = baglam.yasam.kisa_omurlu.len();
            if sayi >= esikler.kisa_omurlu_esik {
                vec![Bulgu::yeni(
                    kural,
                    format!("{} kisa omurlu surec", sayi),
                    vec![
                        format!("{} pid (esik {})", sayi, esikler.kisa_omurlu_esik),
                        format!(
                            "pid listesi: {}",
                            crate::model::truncate(
                                &baglam
                                    .yasam
                                    .kisa_omurlu
                                    .iter()
                                    .map(u32::to_string)
                                    .collect::<Vec<String>>()
                                    .join(", "),
                                120
                            )
                        ),
                    ],
                )]
            } else {
                Vec::new()
            }
        }
        Kural::YetkiYukseltmeGirisimi => {
            let mut bulgular = Vec::new();
            for surec in baglam.surecler {
                if surec.ppid == 0 {
                    continue;
                }
                let Some(ust) = baglam.surecler.iter().find(|s| s.pid == surec.ppid) else {
                    continue;
                };
                let mut kanit: Vec<String> = Vec::new();
                if let (Some(alt_uid), Some(ust_uid)) = (surec.uid, ust.uid) {
                    if alt_uid != ust_uid {
                        kanit.push(format!(
                            "kullanici kimligi ust={} alt={} degisti",
                            ust_uid, alt_uid
                        ));
                    }
                }
                let alt_yolu = surec_yolu(surec);
                let ust_yolu = surec_yolu(ust);
                if let Some(yol) = &alt_yolu {
                    let alt_supheli = supheli_konum_mi(yol).is_some();
                    let ust_supheli = ust_yolu
                        .as_deref()
                        .map(supheli_konum_mi)
                        .unwrap_or(None)
                        .is_some();
                    if alt_supheli && !ust_supheli {
                        kanit.push(format!(
                            "ust surec konumu guvenli, alt surec herkese acik konumdan calisiyor: {}",
                            yol.display()
                        ));
                    }
                }
                if !kanit.is_empty() {
                    bulgular.push(Bulgu::yeni(
                        kural,
                        format!("pid={} ad={} (ust={})", surec.pid, surec.ad, ust.ad),
                        kanit,
                    ));
                }
            }
            bulgular
        }
    }
}

/// Yedi kuralın tümünü değerlendirir ve bulguları kural sırasına göre döndürür.
pub fn tum_kurallar(baglam: &KuralBaglami<'_>, esikler: &KuralEsikleri) -> Vec<Bulgu> {
    let mut bulgular = Vec::new();
    for kural in TUM_KURALLAR {
        bulgular.extend(degerlendir(kural, baglam, esikler));
    }
    bulgular
}

/// Hiçbir bulgu üretmediği doğrulanmış boş katalog için kullanılan kural tablosu.
pub fn kural_katalogu() -> Vec<(String, String, String)> {
    TUM_KURALLAR
        .iter()
        .map(|k| {
            (
                k.ad().to_string(),
                k.aciklama().to_string(),
                k.sonraki_adim().to_string(),
            )
        })
        .collect()
}

#[cfg(test)]
// Gerekçe: unwrap/expect yalnızca test içinde kullanılır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};
    use std::path::PathBuf;

    use crate::model::{BaglantiDurumu, Protokol, StartupTuru, SurecDurumu};

    fn surec_kur(
        pid: u32,
        ppid: u32,
        ad: &str,
        yol: Option<&str>,
        bellek_kib: u64,
        uid: u32,
    ) -> Surec {
        Surec {
            pid,
            ppid,
            ad: ad.to_string(),
            komut_satiri: String::new(),
            yol: yol.map(PathBuf::from),
            cpu_yuzde: None,
            bellek_kib,
            oturum: Some(1),
            kullanici: None,
            uid: Some(uid),
            durum: SurecDurumu::Calisiyor,
        }
    }

    fn baglanti_kur(uzak: &str, port: u16, pid: u32) -> Baglanti {
        Baglanti {
            protokol: Protokol::Tcp,
            yerel_adres: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            yerel_port: 50_000,
            uzak_adres: Some(uzak.parse().unwrap()),
            uzak_port: Some(port),
            durum: BaglantiDurumu::Kuruldu,
            pid: Some(pid),
        }
    }

    fn baglam<'a>(
        s: &'a [Surec],
        b: &'a [Baglanti],
        g: &'a [StartupGirdi],
        y: &'a YasamOzeti,
    ) -> KuralBaglami<'a> {
        KuralBaglami {
            surecler: s,
            baglantilar: b,
            baslangic: g,
            yasam: y,
        }
    }

    // ---- Kural 1: gizli ad -------------------------------------------------

    #[test]
    fn kural1_gizli_ad_pozitif() {
        let s = vec![surec_kur(9001, 4321, "Xk3jdH9s7qW.exe", None, 1024, 1000)];
        let bulgular = degerlendir(
            Kural::GizliAd,
            &baglam(&s, &[], &[], &YasamOzeti::bos()),
            &KuralEsikleri::default(),
        );
        assert_eq!(bulgular.len(), 1);
        assert!(bulgular[0].kanit[0].contains("makine"));
        assert!(!bulgular[0].aciklama.is_empty());
        assert!(!bulgular[0].sonraki_adim.is_empty());
    }

    #[test]
    fn kural1_gizli_ad_negatif() {
        let s = vec![
            surec_kur(1044, 4, "svchost.exe", None, 21_300, 0),
            surec_kur(4321, 1044, "explorer.exe", None, 98_412, 1000),
            surec_kur(555, 4321, "notepad.exe", None, 12_288, 1000),
        ];
        let bulgular = degerlendir(
            Kural::GizliAd,
            &baglam(&s, &[], &[], &YasamOzeti::bos()),
            &KuralEsikleri::default(),
        );
        assert!(bulgular.is_empty());
    }

    #[test]
    fn gizli_ad_olcutleri_tek_tek_dogrulanir() {
        assert!(gizli_ad_mi("svchost.exe").is_none());
        assert!(gizli_ad_mi("360se.exe").is_none(), "kısa ad tetiklenmemeli");
        assert!(gizli_ad_mi("MicrosoftEdgeUpdate.exe").is_none());
        assert!(
            gizli_ad_mi("System Idle Process").is_none(),
            "boşluk meşrudur"
        );
        assert!(gizli_ad_mi("Windows Defender").is_none());
        assert!(gizli_ad_mi("Memory Compression").is_none());
        assert!(
            gizli_ad_mi("svchost&evil.exe").is_some(),
            "kabuk işareti tetikler"
        );
        assert!(
            gizli_ad_mi("a\u{1}b.exe").is_some(),
            "kontrol karakteri tetikler"
        );
        assert!(
            gizli_ad_mi("başlat.exe").is_some(),
            "ASCII dışı karakter tetikler"
        );
        assert!(
            gizli_ad_mi("A1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6.exe").is_some(),
            "aşırı uzun ad tetikler"
        );
        assert!(gizli_ad_mi("").is_none());
    }

    // ---- Kural 2: şüpheli konum -------------------------------------------

    #[test]
    fn kural2_supheli_konum_pozitif() {
        let s = vec![surec_kur(
            9001,
            4321,
            "updater.exe",
            Some("C:\\Users\\K\\AppData\\Local\\Temp\\a.exe"),
            40_960,
            1000,
        )];
        let bulgular = degerlendir(
            Kural::SupheliKonum,
            &baglam(&s, &[], &[], &YasamOzeti::bos()),
            &KuralEsikleri::default(),
        );
        assert_eq!(bulgular.len(), 1);
        assert!(bulgular[0].kanit[0].contains("Temp"));
    }

    #[test]
    fn kural2_supheli_konum_negatif() {
        let s = vec![surec_kur(
            1044,
            4,
            "svchost.exe",
            Some("C:\\Windows\\System32\\svchost.exe"),
            21_300,
            0,
        )];
        let bulgular = degerlendir(
            Kural::SupheliKonum,
            &baglam(&s, &[], &[], &YasamOzeti::bos()),
            &KuralEsikleri::default(),
        );
        assert!(bulgular.is_empty());
    }

    #[test]
    fn kural2_yol_yoksa_ve_komut_satiri_yoksa_bulgu_yok() {
        let s = vec![surec_kur(1, 0, "isimsiz", None, 1024, 0)];
        let bulgular = degerlendir(
            Kural::SupheliKonum,
            &baglam(&s, &[], &[], &YasamOzeti::bos()),
            &KuralEsikleri::default(),
        );
        assert!(bulgular.is_empty());
    }

    // ---- Kural 3: yüksek bellek -------------------------------------------

    #[test]
    fn kural3_yuksek_bellek_pozitif() {
        let s = vec![surec_kur(9001, 1, "a.exe", None, 1_572_864, 1000)];
        let bulgular = degerlendir(
            Kural::YuksekBellek,
            &baglam(&s, &[], &[], &YasamOzeti::bos()),
            &KuralEsikleri::default(),
        );
        assert_eq!(bulgular.len(), 1);
        assert!(bulgular[0].kanit[0].contains("1536 MiB"));
    }

    #[test]
    fn kural3_yuksek_bellek_negatif() {
        let s = vec![surec_kur(1044, 4, "svchost.exe", None, 21_300, 0)];
        let bulgular = degerlendir(
            Kural::YuksekBellek,
            &baglam(&s, &[], &[], &YasamOzeti::bos()),
            &KuralEsikleri::default(),
        );
        assert!(bulgular.is_empty());
    }

    #[test]
    fn kural3_esik_sinirinda_tetiklenir() {
        let s = vec![surec_kur(1, 1, "a.exe", None, 512 * 1024, 0)];
        let bulgular = degerlendir(
            Kural::YuksekBellek,
            &baglam(&s, &[], &[], &YasamOzeti::bos()),
            &KuralEsikleri::default(),
        );
        assert_eq!(bulgular.len(), 1, "esik degerinde >= esik tetiklenmeli");

        let s = vec![surec_kur(1, 1, "a.exe", None, 512 * 1024 - 1, 0)];
        let bulgular = degerlendir(
            Kural::YuksekBellek,
            &baglam(&s, &[], &[], &YasamOzeti::bos()),
            &KuralEsikleri::default(),
        );
        assert!(
            bulgular.is_empty(),
            "esigin bir bayt altinda tetiklenmemeli"
        );
    }

    // ---- Kural 4: kalıcılık girdisi ---------------------------------------

    fn girdi(ad: &str, konum: &str, hedef: Option<&str>) -> StartupGirdi {
        StartupGirdi {
            konum: PathBuf::from(konum),
            ad: ad.to_string(),
            tur: StartupTuru::RunAnahtari,
            etkin: true,
            hedef: hedef.map(str::to_string),
        }
    }

    #[test]
    fn kural4_kalicilik_girdisi_pozitif() {
        let g = vec![girdi(
            "UpdaterTask",
            "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            Some("C:\\Users\\K\\AppData\\Roaming\\Updater\\upd.exe"),
        )];
        let bulgular = degerlendir(
            Kural::KalicilikGirdisi,
            &baglam(&[], &[], &g, &YasamOzeti::bos()),
            &KuralEsikleri::default(),
        );
        assert_eq!(bulgular.len(), 1);
        assert!(bulgular[0].konu.contains("UpdaterTask"));
    }

    #[test]
    fn kural4_kalicilik_girdisi_negatif() {
        let g = vec![girdi(
            "SecurityHealth",
            "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            Some("C:\\Windows\\system32\\SecurityHealthSystray.exe"),
        )];
        let bulgular = degerlendir(
            Kural::KalicilikGirdisi,
            &baglam(&[], &[], &g, &YasamOzeti::bos()),
            &KuralEsikleri::default(),
        );
        assert!(bulgular.is_empty());
    }

    #[test]
    fn kural4_standart_baslangic_klasoru_hedefsiz_muaf() {
        let g = vec![StartupGirdi {
            konum: PathBuf::from(
                "C:\\Users\\K\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\a.lnk",
            ),
            ad: "a.lnk".into(),
            tur: StartupTuru::BaslangicKlasoru,
            etkin: true,
            hedef: None,
        }];
        assert!(kalicilik_suspicious_mi(&g[0]).is_none());
        let bulgular = degerlendir(
            Kural::KalicilikGirdisi,
            &baglam(&[], &[], &g, &YasamOzeti::bos()),
            &KuralEsikleri::default(),
        );
        assert!(bulgular.is_empty());
    }

    // ---- Kural 5: beklenmeyen ağ bağlantısı -------------------------------

    #[test]
    fn kural5_beklenmeyen_baglanti_pozitif() {
        let b = vec![baglanti_kur("45.83.220.17", 8443, 9001)];
        let bulgular = degerlendir(
            Kural::BeklenmeyenAgBaglantisi,
            &baglam(&[], &b, &[], &YasamOzeti::bos()),
            &KuralEsikleri::default(),
        );
        assert_eq!(bulgular.len(), 1);
        assert!(bulgular[0].konu.contains("8443"));
    }

    #[test]
    fn kural5_beklenmeyen_baglanti_negatif() {
        let b = vec![
            baglanti_kur("93.184.216.34", 443, 4321),
            baglanti_kur("127.0.0.1", 5000, 4321),
            baglanti_kur("192.168.1.10", 445, 4321),
            baglanti_kur("203.0.113.9", 9001, 4321),
        ];
        let bulgular = degerlendir(
            Kural::BeklenmeyenAgBaglantisi,
            &baglam(&[], &b, &[], &YasamOzeti::bos()),
            &KuralEsikleri::default(),
        );
        assert!(bulgular.is_empty());
    }

    #[test]
    fn kural5_dinleyen_soket_tetiklemez() {
        let b = vec![Baglanti {
            protokol: Protokol::Tcp,
            yerel_adres: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            yerel_port: 135,
            uzak_adres: None,
            uzak_port: None,
            durum: BaglantiDurumu::Dinliyor,
            pid: Some(4),
        }];
        let bulgular = degerlendir(
            Kural::BeklenmeyenAgBaglantisi,
            &baglam(&[], &b, &[], &YasamOzeti::bos()),
            &KuralEsikleri::default(),
        );
        assert!(bulgular.is_empty());
    }

    #[test]
    fn kural5_tanidik_port_listesi_guncellenebilir() {
        let esikler = KuralEsikleri {
            bilinen_portlar: vec![443],
            ..KuralEsikleri::default()
        };
        let b = vec![baglanti_kur("93.184.216.34", 443, 1)];
        assert!(degerlendir(
            Kural::BeklenmeyenAgBaglantisi,
            &baglam(&[], &b, &[], &YasamOzeti::bos()),
            &esikler
        )
        .is_empty());
        let b = vec![baglanti_kur("93.184.216.34", 80, 1)];
        assert_eq!(
            degerlendir(
                Kural::BeklenmeyenAgBaglantisi,
                &baglam(&[], &b, &[], &YasamOzeti::bos()),
                &esikler
            )
            .len(),
            1
        );
    }

    // ---- Kural 6: kısa ömürlü çok sayıda süreç ---------------------------

    #[test]
    fn kural6_kisa_omurlu_pozitif() {
        let yasam = YasamOzeti::kisa_omurlu_ile((100..105).collect());
        let bulgular = degerlendir(
            Kural::KisaOmurluCokSayidaSurec,
            &baglam(&[], &[], &[], &yasam),
            &KuralEsikleri::default(),
        );
        assert_eq!(bulgular.len(), 1);
        assert!(bulgular[0].konu.contains("5"));
    }

    #[test]
    fn kural6_kisa_omurlu_negatif() {
        let yasam = YasamOzeti::kisa_omurlu_ile(vec![100, 101]);
        let bulgular = degerlendir(
            Kural::KisaOmurluCokSayidaSurec,
            &baglam(&[], &[], &[], &yasam),
            &KuralEsikleri::default(),
        );
        assert!(bulgular.is_empty());
    }

    #[test]
    fn kural6_esik_sinirinda_tetiklenir() {
        let yasam = YasamOzeti::kisa_omurlu_ile(vec![1, 2, 3, 4]);
        assert!(degerlendir(
            Kural::KisaOmurluCokSayidaSurec,
            &baglam(&[], &[], &[], &yasam),
            &KuralEsikleri::default()
        )
        .is_empty());
        let yasam = YasamOzeti::kisa_omurlu_ile(vec![1, 2, 3, 4, 5]);
        assert_eq!(
            degerlendir(
                Kural::KisaOmurluCokSayidaSurec,
                &baglam(&[], &[], &[], &yasam),
                &KuralEsikleri::default()
            )
            .len(),
            1
        );
    }

    // ---- Kural 7: yetki yükseltme girişimi --------------------------------

    #[test]
    fn kural7_kullanici_kimligi_degisimi_pozitif() {
        let s = vec![
            surec_kur(
                1044,
                4,
                "svchost.exe",
                Some("C:\\Windows\\System32\\svchost.exe"),
                21_300,
                0,
            ),
            surec_kur(
                9003,
                1044,
                "svchost-helper.exe",
                Some("C:\\Windows\\System32\\svchost-helper.exe"),
                8_192,
                1000,
            ),
        ];
        let bulgular = degerlendir(
            Kural::YetkiYukseltmeGirisimi,
            &baglam(&s, &[], &[], &YasamOzeti::bos()),
            &KuralEsikleri::default(),
        );
        assert_eq!(bulgular.len(), 1);
        assert!(bulgular[0].kanit[0].contains("kullanici kimligi"));
    }

    #[test]
    fn kural7_dunya_acik_konumdan_calisma_pozitif() {
        let s = vec![
            surec_kur(
                4321,
                1044,
                "explorer.exe",
                Some("C:\\Windows\\explorer.exe"),
                98_412,
                1000,
            ),
            surec_kur(
                9001,
                4321,
                "Xk3jdH9s7qW.exe",
                Some("C:\\Users\\K\\AppData\\Local\\Temp\\X.exe"),
                40_960,
                1000,
            ),
        ];
        let bulgular = degerlendir(
            Kural::YetkiYukseltmeGirisimi,
            &baglam(&s, &[], &[], &YasamOzeti::bos()),
            &KuralEsikleri::default(),
        );
        assert_eq!(bulgular.len(), 1);
        assert!(bulgular[0].kanit[0].contains("herkese acik konum"));
    }

    #[test]
    fn kural7_negatif() {
        let s = vec![
            surec_kur(
                1044,
                4,
                "svchost.exe",
                Some("C:\\Windows\\System32\\svchost.exe"),
                21_300,
                0,
            ),
            surec_kur(
                4321,
                1044,
                "explorer.exe",
                Some("C:\\Windows\\explorer.exe"),
                98_412,
                0,
            ),
            surec_kur(
                555,
                4321,
                "notepad.exe",
                Some("C:\\Windows\\Notepad\\notepad.exe"),
                12_288,
                0,
            ),
        ];
        let bulgular = degerlendir(
            Kural::YetkiYukseltmeGirisimi,
            &baglam(&s, &[], &[], &YasamOzeti::bos()),
            &KuralEsikleri::default(),
        );
        assert!(bulgular.is_empty());
    }

    #[test]
    fn kural7_ust_surec_listede_yoksa_bulgu_yok() {
        let s = vec![surec_kur(
            9001,
            9999,
            "a.exe",
            Some("C:\\Temp\\a.exe"),
            1024,
            0,
        )];
        let bulgular = degerlendir(
            Kural::YetkiYukseltmeGirisimi,
            &baglam(&s, &[], &[], &YasamOzeti::bos()),
            &KuralEsikleri::default(),
        );
        assert!(bulgular.is_empty());
    }

    // ---- Katalog ve bütünlük ---------------------------------------------

    #[test]
    fn yedi_kuralin_hepsi_hasi_katalogu_doldurur() {
        assert_eq!(TUM_KURALLAR.len(), 7);
        let katalog = kural_katalogu();
        assert_eq!(katalog.len(), 7);
        for (ad, aciklama, adim) in &katalog {
            assert!(!ad.is_empty());
            assert!(aciklama.len() > 30, "aciklama çok kısa: {}", aciklama);
            assert!(adim.len() > 20, "adım çok kısa: {}", adim);
        }
    }

    #[test]
    fn hicbir_kural_mudahale_onermez() {
        for kural in TUM_KURALLAR {
            let metin = kural.sonraki_adim().to_ascii_lowercase();
            for yasak in ["sonlandir", "öldür", "sil ", "kaldir", "engelle", "kill"] {
                assert!(
                    !metin.contains(yasak),
                    "{} kuralı müdahale öneriyor: {}",
                    kural.ad(),
                    metin
                );
            }
        }
    }

    #[test]
    fn tum_kurallar_bos_baglamda_bulgu_uretmez() {
        let bulgular = tum_kurallar(&KuralBaglami::bos(), &KuralEsikleri::default());
        assert!(bulgular.is_empty());
    }

    #[test]
    fn bulgu_ozeti_kural_ve_konuyu_icerir() {
        let bulgu = Bulgu::yeni(Kural::GizliAd, "pid=1 a.exe", vec!["kanit".to_string()]);
        assert!(bulgu.ozet().contains("gizli ad"));
        assert!(bulgu.ozet().contains("pid=1"));
    }
}
