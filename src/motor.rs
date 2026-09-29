//! Yoklama döngüsü: iki anlık görüntü arasındaki farkı olaylara çevirir.
//!
//! Raporun ETW önerimi gerçek zamanlı, kayıpsız bir akış sunar; ancak ETW
//! `advapi32` FFI'si gerektirir ve `forbid(unsafe_code)` ile bağdaşmaz. Bu
//! sürüm bilinçli olarak **periyodik yoklamaya** düşer. Bunun ölçülebilir
//! bir bilgi kaybı vardır: yoklama aralığından kısa ömürlü süreçler kaçar.
//! Bu kayıp README'nin `## Bilinen Sınırlamalar` bölümünde açıkça yazılıdır.
//!
//! Modül iki şey üretir:
//!
//! - [`YasamOzeti`]: bir yoklamada başlayan, biten ve **yalnızca bir
//!   yoklamada görülen** süreç kimlikleri.
//! - [`Olay`] dönüşümü: başlangıç, bitiş ve bağlantı gözlemi için olaylar.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::kaynak::AnlikGoruntu;
use crate::model::{Baglanti, Surec};

/// Bir yoklama turunun yaşam istatistiği.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct YasamOzeti {
    /// Özetin üretildiği yoklama numarası.
    pub donem: u64,
    /// Bu turda ilk kez görülen süreç kimlikleri.
    pub yeni: Vec<u32>,
    /// Önceki turda varken bu turda kaybolan süreç kimlikleri.
    pub kayboldu: Vec<u32>,
    /// **Yalnızca bir yoklamada görülen** süreç kimlikleri (kısa ömürlü).
    pub kisa_omurlu: Vec<u32>,
    /// Bu turda hâlâ yaşayan süreç sayısı.
    pub yasayan: usize,
}

impl YasamOzeti {
    /// Boş bir özet üretir (herhangi bir yoklama yapılmamış).
    pub fn bos() -> Self {
        YasamOzeti::default()
    }

    /// Bir kısa ömürlü süreç listesiyle özet kurar (günlükten yeniden üretim
    /// için).
    pub fn kisa_omurlu_ile(pidler: Vec<u32>) -> Self {
        YasamOzeti {
            kisa_omurlu: pidler,
            ..YasamOzeti::default()
        }
    }
}

/// Bir sürecin iki yoklama arasındaki yaşam kaydı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Konum {
    ilk_donem: u64,
    son_donem: u64,
}

/// Ardışık yoklamalar arasında süreç kimliğini izleyen durum makinesi.
///
/// Anahtar sınıf kısıtı: bu yapı **hiçbir süreci etkilemez**. Yalnızca
/// okunan `&[Surec]` girdisinden kimlik listesi tutar ve hiçbir kille, hiçbir
/// sistem çağrısıyla, hiçbir yazma işlemiyle temas etmez.
#[derive(Debug, Clone, Default)]
pub struct YasamTakip {
    konumlar: HashMap<u32, Konum>,
    donem: u64,
}

impl YasamTakip {
    /// Yeni, boş bir takipçi üretir.
    pub fn yeni() -> Self {
        YasamTakip::default()
    }

    /// Şu ana kadar işlenen yoklama sayısı.
    pub fn donem(&self) -> u64 {
        self.donem
    }

    /// Bir yoklama turunu işler ve özeti döner.
    ///
    /// İlk turda tüm süreçler "yeni" sayılır; hiçbir süreç kısa ömürlü
    /// işaretlenmez, çünkü henüz "yaşamı bitmiş" kanıtı yoktur.
    pub fn guncelle(&mut self, surecler: &[Surec]) -> YasamOzeti {
        self.donem += 1;
        let donem = self.donem;

        let mut yeni: Vec<u32> = Vec::new();
        let gorulen: Vec<u32> = surecler.iter().map(|s| s.pid).collect();
        let gorulen_kume: std::collections::HashSet<u32> = gorulen.iter().copied().collect();

        for &pid in &gorulen {
            match self.konumlar.get_mut(&pid) {
                Some(konum) => {
                    konum.son_donem = donem;
                }
                None => {
                    self.konumlar.insert(
                        pid,
                        Konum {
                            ilk_donem: donem,
                            son_donem: donem,
                        },
                    );
                    if donem > 1 {
                        yeni.push(pid);
                    }
                }
            }
        }

        let mut kayboldu: Vec<u32> = Vec::new();
        let mut kisa_omurlu: Vec<u32> = Vec::new();
        let mut hayatta: Vec<u32> = Vec::new();
        for (pid, konum) in &self.konumlar {
            if gorulen_kume.contains(pid) {
                hayatta.push(*pid);
            } else {
                kayboldu.push(*pid);
                // Yalnızca tek bir tur görüldüyse "kısa ömürlü" tanımına girer.
                if konum.ilk_donem == konum.son_donem {
                    kisa_omurlu.push(*pid);
                }
            }
        }
        for pid in &kayboldu {
            self.konumlar.remove(pid);
        }
        yeni.sort_unstable();
        kayboldu.sort_unstable();
        kisa_omurlu.sort_unstable();

        YasamOzeti {
            donem,
            yeni,
            kayboldu,
            kisa_omurlu,
            yasayan: hayatta.len(),
        }
    }
}

/// Gözlem motorunun bir turda ürettiği olay çeşidi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OlayTuru {
    /// Bu yoklamada ilk kez görülen süreç.
    SurecBasladi,
    /// Önceki yoklamada varken bu yoklamada kaybolan süreç.
    SurecBitti,
    /// Bu turda görülen bağlantı.
    BaglantiGoruldu,
    /// Envanterde bulunan başlangıç girdisi.
    BaslangicGirdisi,
    /// Kural motorunun ürettiği gözlem.
    KuralBulgusu,
    /// Kaynaktan gelen, gözlemi kesmeyen hata.
    KaynakHatasi,
}

impl OlayTuru {
    /// Olayın JSONL'deki sabit etiketi.
    pub fn etiket(self) -> &'static str {
        match self {
            OlayTuru::SurecBasladi => "surec_basladi",
            OlayTuru::SurecBitti => "surec_bitti",
            OlayTuru::BaglantiGoruldu => "baglanti_goruldu",
            OlayTuru::BaslangicGirdisi => "baslangic_girdisi",
            OlayTuru::KuralBulgusu => "kural_bulgusu",
            OlayTuru::KaynakHatasi => "kaynak_hatasi",
        }
    }

    /// Etiketten olay türünü çözer; bilinmeyen etiket `None` döner.
    pub fn etiketten(metin: &str) -> Option<Self> {
        [
            OlayTuru::SurecBasladi,
            OlayTuru::SurecBitti,
            OlayTuru::BaglantiGoruldu,
            OlayTuru::BaslangicGirdisi,
            OlayTuru::KuralBulgusu,
            OlayTuru::KaynakHatasi,
        ]
        .into_iter()
        .find(|t| t.etiket() == metin)
    }
}

/// Tek bir günlük olayının gövdesi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Olay {
    /// Unix epoch milisaniyesi.
    pub zaman: u64,
    /// Olay türü.
    pub olay: OlayTuru,
    /// İlgili süreç kimliği; süreçle ilgisi yoksa `None`.
    pub pid: Option<u32>,
    /// Olayın üretildiği kaynak (`tasklist`, `/proc`, `fixture` ...).
    pub kaynak: String,
    /// İnsan okunur, tek satırlık açıklama.
    pub ayrinti: String,
}

impl Olay {
    /// Yeni bir olay kurar.
    pub fn yeni(
        zaman: u64,
        tur: OlayTuru,
        pid: Option<u32>,
        kaynak: &str,
        ayrinti: impl Into<String>,
    ) -> Self {
        Olay {
            zaman,
            olay: tur,
            pid,
            kaynak: kaynak.to_string(),
            ayrinti: ayrinti.into(),
        }
    }

    /// Bir satırlık insan okunur biçim.
    pub fn satir(&self) -> String {
        match self.pid {
            Some(pid) => format!(
                "[{}] {} pid={} — {}",
                self.zaman,
                self.olay.etiket(),
                pid,
                self.ayrinti
            ),
            None => format!("[{}] {} — {}", self.zaman, self.olay.etiket(), self.ayrinti),
        }
    }
}

/// Bir yoklama turunu olay listesine çevirir.
///
/// Aynı bağlantı her turda rapor edilir; `Baglanti` kaydı küçük olduğu için
/// günlük boyutu yoklama aralığıyla doğru orantılı büyür ve kullanıcı
/// `--aralik` bayrağıyla kontrol eder.
pub fn tur_olaylari(
    goruntu: &AnlikGoruntu,
    yasam: &YasamOzeti,
    onceki: &HashMap<u32, Surec>,
) -> Vec<Olay> {
    let mut olaylar = Vec::new();
    let kaynak = goruntu.kaynak.clone();

    for pid in &yasam.yeni {
        if let Some(surec) = goruntu.surec(*pid) {
            olaylar.push(Olay::yeni(
                goruntu.zaman,
                OlayTuru::SurecBasladi,
                Some(*pid),
                &kaynak,
                format!(
                    "{} (ppid={}, oturum={}, {} MiB) basladi",
                    surec.ad,
                    surec.ppid,
                    surec
                        .oturum
                        .map(|o| o.to_string())
                        .unwrap_or_else(|| "-".into()),
                    surec.bellek_mib()
                ),
            ));
        }
    }

    for pid in &yasam.kayboldu {
        let aciklama = onceki.get(pid).map_or_else(
            || format!("{} sureci kayboldu", pid),
            |s| format!("{} (ppid={}) surucu kayboldu", s.ad, s.ppid),
        );
        olaylar.push(Olay::yeni(
            goruntu.zaman,
            OlayTuru::SurecBitti,
            Some(*pid),
            &kaynak,
            aciklama,
        ));
    }

    for baglanti in &goruntu.baglantilar {
        if baglanti.dinleyen_mi() {
            continue;
        }
        olaylar.push(Olay::yeni(
            goruntu.zaman,
            OlayTuru::BaglantiGoruldu,
            baglanti.pid,
            &kaynak,
            format!("{} {}", baglanti.tanim(), baglanti.durum.metin()),
        ));
    }

    for girdi in &goruntu.baslangic {
        olaylar.push(Olay::yeni(
            goruntu.zaman,
            OlayTuru::BaslangicGirdisi,
            None,
            &kaynak,
            format!(
                "{} ({}) — {}",
                girdi.ad,
                girdi.tur.etiket(),
                girdi.konum.display()
            ),
        ));
    }

    olaylar
}

/// İki anlık görüntü arasındaki süreç farkını süreç haritası olarak verir.
pub fn surece_haritasi(goruntu: &AnlikGoruntu) -> HashMap<u32, Surec> {
    goruntu
        .surecler
        .iter()
        .map(|s| (s.pid, s.clone()))
        .collect()
}

/// Bir bağlantının kısa tanımını döner (rapor ve günlük için ortak).
pub fn baglanti_ozeti(baglanti: &Baglanti) -> String {
    format!("{} {}", baglanti.tanim(), baglanti.durum.metin())
}

#[cfg(test)]
// Gerekçe: unwrap/expect yalnızca test içinde kullanılır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::model::SurecDurumu;

    fn surec(pid: u32) -> Surec {
        Surec {
            pid,
            ppid: 1,
            ad: format!("surec-{}", pid),
            komut_satiri: String::new(),
            yol: None,
            cpu_yuzde: None,
            bellek_kib: 1024,
            oturum: Some(1),
            kullanici: None,
            uid: None,
            durum: SurecDurumu::Calisiyor,
        }
    }

    #[test]
    fn ilk_tur_yeni_surec_uretmez() {
        let mut takip = YasamTakip::yeni();
        let ozet = takip.guncelle(&[surec(1), surec(2)]);
        assert_eq!(ozet.donem, 1);
        assert!(ozet.yeni.is_empty(), "ilk turda 'yeni' anlamsizdir");
        assert_eq!(ozet.yasayan, 2);
        assert!(ozet.kayboldu.is_empty());
    }

    #[test]
    fn ikinci_tur_gercekten_yeni_surecleri_bulur() {
        let mut takip = YasamTakip::yeni();
        takip.guncelle(&[surec(1), surec(2)]);
        let ozet = takip.guncelle(&[surec(1), surec(2), surec(3)]);
        assert_eq!(ozet.yeni, vec![3]);
        assert_eq!(ozet.yasayan, 3);
    }

    #[test]
    fn kaybolan_surec_kisa_omurlu_isaretlenir() {
        let mut takip = YasamTakip::yeni();
        takip.guncelle(&[surec(1)]);
        // pid 2 yalnızca 2. turda görüldü.
        takip.guncelle(&[surec(1), surec(2)]);
        let ozet = takip.guncelle(&[surec(1)]);
        assert_eq!(ozet.kayboldu, vec![2]);
        assert_eq!(ozet.kisa_omurlu, vec![2]);
        assert_eq!(ozet.yasayan, 1);
    }

    #[test]
    fn uzun_omurlu_surec_kisa_omurlu_sayilmaz() {
        let mut takip = YasamTakip::yeni();
        takip.guncelle(&[surec(1)]);
        takip.guncelle(&[surec(1)]);
        takip.guncelle(&[surec(1)]);
        let ozet = takip.guncelle(&[]);
        assert_eq!(ozet.kayboldu, vec![1]);
        assert!(ozet.kisa_omurlu.is_empty());
    }

    #[test]
    fn pid_yeniden_kullanilirsa_yeni_sayilir() {
        let mut takip = YasamTakip::yeni();
        takip.guncelle(&[surec(7)]);
        let ozet = takip.guncelle(&[]);
        assert_eq!(ozet.kayboldu, vec![7]);
        let ozet = takip.guncelle(&[surec(7)]);
        assert_eq!(ozet.yeni, vec![7]);
    }

    #[test]
    fn coklu_kisa_omurlu_surec_sayilir() {
        let mut takip = YasamTakip::yeni();
        takip.guncelle(&[surec(1)]);
        let ikinci: Vec<Surec> = (10..20).map(surec).collect();
        takip.guncelle(&ikinci);
        let ozet = takip.guncelle(&[surec(1)]);
        assert_eq!(ozet.kisa_omurlu.len(), 10);
    }

    #[test]
    fn bos_tur_tum_surecleri_kaybettirir() {
        let mut takip = YasamTakip::yeni();
        takip.guncelle(&[surec(1), surec(2)]);
        let ozet = takip.guncelle(&[]);
        assert_eq!(ozet.kayboldu, vec![1, 2]);
        assert_eq!(ozet.yasayan, 0);
    }

    #[test]
    fn olay_turu_etiketten_cozulur() {
        for tur in [
            OlayTuru::SurecBasladi,
            OlayTuru::SurecBitti,
            OlayTuru::BaglantiGoruldu,
            OlayTuru::BaslangicGirdisi,
            OlayTuru::KuralBulgusu,
            OlayTuru::KaynakHatasi,
        ] {
            assert_eq!(OlayTuru::etiketten(tur.etiket()), Some(tur));
        }
        assert_eq!(OlayTuru::etiketten("bilinmeyen"), None);
    }

    #[test]
    fn olay_satiri_pid_ile_ve_pid_siz_yazilir() {
        let pidli = Olay::yeni(1, OlayTuru::SurecBasladi, Some(4), "fixture", "aciklama");
        assert!(pidli.satir().contains("pid=4"));
        let pid_siz = Olay::yeni(1, OlayTuru::BaslangicGirdisi, None, "fixture", "aciklama");
        assert!(!pid_siz.satir().contains("pid="));
    }

    #[test]
    fn surece_haritasi_kimlikleri_tekiller() {
        let goruntu = AnlikGoruntu {
            zaman: 0,
            kaynak: "fixture".into(),
            surecler: vec![surec(1), surec(1), surec(2)],
            baglantilar: Vec::new(),
            baslangic: Vec::new(),
            ayristirma: Default::default(),
            kisit_notu: String::new(),
        };
        assert_eq!(surece_haritasi(&goruntu).len(), 2);
    }
}
