//! Envanter kaynakları ve aralarındaki soyutlama.
//!
//! `InventorySource` üç platformun üçünü de tek arayüzün arkasına toplar:
//!
//! - [`windows::WindowsSource`] — `tasklist /fo csv` ve `netstat -ano` çıktısını
//!   ayrıştırır, başlangıç girdilerini dizin taramasıyla (ve isteğe bağlı
//!   `reg query` ile) toplar.
//! - [`procfs::ProcFsSource`] — `/proc/<pid>/stat`, `/cmdline`, `/status` ve
//!   `/proc/net/tcp*` dosyalarını doğrudan okur.
//! - [`ps::PsSource`] — macOS ve BSD türevlerinde `ps`/`netstat` çıktısını
//!   ayrıştırır.
//! - [`fixture::FixtureSource`] — testlerin ve `--source fixture` kipinin
//!   kullandığı, hiçbir komut çalıştırmayan statik kaynak.
//!
//! **Güvenlik sınırı:** Hiçbir kaynak bir süreci sonlandırmaz, bir dosyayı
//! silmez veya kayıt defterine yazmaz. `WindowsSource` ve `PsSource` yalnızca
//! `arac_salt_okunur_mu` izin listesindeki programları çalıştırır; bu liste
//! `tests/salt_okunur_kanit.rs` içinde statik olarak denetlenir.

pub mod fixture;
pub mod procfs;
pub mod ps;
pub mod windows;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::ayristir::AyristirmaOzeti;
use crate::hata::{Hata, Sonuc};
use crate::model::{Baglanti, StartupGirdi, Surec};

/// Salt okunur gözlem arayüzü.
///
/// Uygulamalar hiçbir zaman yazma işlemi yapmaz. `baslangic_girdileri`
/// yalnızca **listeler**; girdileri devre dışı bırakma veya silme işlevi
/// bilinçli olarak yoktur.
pub trait InventorySource {
    /// Kaynağın kısa adı (arayüzde ve günlükte görünür).
    fn ad(&self) -> &str;

    /// Anlık süreç listesini döndürür.
    fn surecler(&self) -> Sonuc<Vec<Surec>>;

    /// Anlık ağ bağlantı tablosunu döndürür.
    fn ag_baglantilari(&self) -> Sonuc<Vec<Baglanti>>;

    /// Başlangıç girdileri envanterini döndürür.
    fn baslangic_girdileri(&self) -> Sonuc<Vec<StartupGirdi>>;

    /// Kaynağın kullanılamama nedenini tek satırda anlatır (rapor altbilgisi).
    fn kisit_notu(&self) -> String {
        String::new()
    }
}

/// Komut satırından seçilebilen envanter kaynağı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
pub enum KaynakTuru {
    /// Çalışılan platforma göre seç: Windows, Linux veya macOS.
    Otomatik,
    /// Windows: `tasklist` + `netstat`.
    Windows,
    /// Linux: `/proc` dosyaları.
    Procfs,
    /// macOS/BSD: `ps` + `netstat`.
    Ps,
    /// Yerleşik statik veri; hiçbir komut çalıştırmaz.
    Fixture,
}

impl KaynakTuru {
    /// Türün `--source` bayrağı için kısa adı.
    pub fn etiket(self) -> &'static str {
        match self {
            KaynakTuru::Otomatik => "otomatik",
            KaynakTuru::Windows => "windows",
            KaynakTuru::Procfs => "procfs",
            KaynakTuru::Ps => "ps",
            KaynakTuru::Fixture => "fixture",
        }
    }
}

/// Tek bir yoklama turunun tamamı: süreçler, bağlantılar, başlangıç girdileri.
///
/// `zaman` alanı Unix epoch milisaniyesidir; olay günlüğünde kullanılır.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnlikGoruntu {
    /// Toplamanın bittiği an (Unix epoch milisaniyesi).
    pub zaman: u64,
    /// Görüntüyü üreten kaynağın adı.
    pub kaynak: String,
    /// Süreç kayıtları.
    pub surecler: Vec<Surec>,
    /// Ağ bağlantısı kayıtları.
    pub baglantilar: Vec<Baglanti>,
    /// Başlangıç girdisi kayıtları.
    pub baslangic: Vec<StartupGirdi>,
    /// Ayrıştırma sayacı özeti; bozuk satır sayısı gizlenmez.
    pub ayristirma: AyristirmaOzeti,
    /// Kaynağın bilinen kısıtları.
    pub kisit_notu: String,
}

impl AnlikGoruntu {
    /// Bir kaynaktan eksiksiz bir görüntü toplar.
    ///
    /// Hata durumunda **kısmi sonuç kaybolmaz**: toplanabilen bölümler
    /// döndürülür ve hata `kisit_notu` içinde belirtilir. Gözlem aracının
    /// kendisinin çökmesi, eldeki kanıtın kaybolmasına yol açmamalıdır.
    pub fn topla(kaynak: &dyn InventorySource) -> Self {
        let mut kisit_notu = kaynak.kisit_notu();
        let surecler = match kaynak.surecler() {
            Ok(v) => v,
            Err(h) => {
                kisit_notu = birlestir_not(kisit_notu, &h.to_string());
                Vec::new()
            }
        };
        let baglantilar = match kaynak.ag_baglantilari() {
            Ok(v) => v,
            Err(h) => {
                kisit_notu = birlestir_not(kisit_notu, &h.to_string());
                Vec::new()
            }
        };
        let baslangic = match kaynak.baslangic_girdileri() {
            Ok(v) => v,
            Err(h) => {
                kisit_notu = birlestir_not(kisit_notu, &h.to_string());
                Vec::new()
            }
        };
        AnlikGoruntu {
            zaman: epoch_millis(),
            kaynak: kaynak.ad().to_string(),
            surecler,
            baglantilar,
            baslangic,
            ayristirma: AyristirmaOzeti::yeni(),
            kisit_notu,
        }
    }

    /// Verilen süreç kimliğinin kaydını bulur.
    pub fn surec(&self, pid: u32) -> Option<&Surec> {
        self.surecler.iter().find(|s| s.pid == pid)
    }

    /// Verilen sürecin bağlantılarını döndürür.
    pub fn baglantilar(&self, pid: u32) -> Vec<&Baglanti> {
        self.baglantilar
            .iter()
            .filter(|b| b.pid == Some(pid))
            .collect()
    }

    /// Görüntüde atlanan ayrıştırma satırı varsa uyarı satırı üretir.
    pub fn bozuk_satir_uyarisi(&self) -> Option<String> {
        if self.ayristirma.atlanan == 0 {
            None
        } else {
            Some(format!(
                "{} satır biçim nedeniyle atlandı ({}); ayrıntı için --ayrinti bayrağını kullanın",
                self.ayristirma.atlanan, self.kaynak
            ))
        }
    }
}

fn birlestir_not(mevcut: String, ekle: &str) -> String {
    if mevcut.is_empty() {
        ekle.to_string()
    } else {
        format!("{}; {}", mevcut, ekle)
    }
}

/// Bir dizin ağacındaki dosyaları sınırlı derinlikte listeler.
///
/// Başlangıç klasörü ve XDG autostart taraması için kullanılır. Sembolik
/// bağlantı takip edilmez (döngüsel gezinme ve yetki yükseltme riski), hata
/// veren dizinler atlanır ve liste `azami_derinlik` ile sınırlıdır.
///
/// Hiçbir dosya **açılmaz veya değiştirilmez**; yalnızca dizin girdileri
/// okunur.
pub fn dosya_haritasi(kok: &Path, azami_derinlik: usize) -> Vec<PathBuf> {
    let mut sonuc: Vec<PathBuf> = Vec::new();
    let mut yigin: Vec<(PathBuf, usize)> = vec![(kok.to_path_buf(), 0)];
    let mut ziyaret_edilen: Vec<PathBuf> = Vec::new();

    while let Some((dizin, derinlik)) = yigin.pop() {
        if derinlik > azami_derinlik {
            continue;
        }
        ziyaret_edilen.push(dizin.clone());
        let Ok(girdiler) = std::fs::read_dir(&dizin) else {
            continue;
        };
        for girid in girdiler.flatten() {
            let yol = girid.path();
            let Ok(tur) = girid.file_type() else {
                continue;
            };
            if tur.is_symlink() {
                continue;
            }
            if tur.is_dir() {
                yigin.push((yol, derinlik + 1));
            } else if tur.is_file() {
                sonuc.push(yol);
            }
        }
    }
    sonuc.sort();
    sonuc
}

/// Şu anki anı Unix epoch milisaniyesi olarak döndürür.
///
/// `SystemTime::now` yalnızca çıktı üretiminde kullanılır; hiçbir test kararı
/// zamana bağlı değildir.
pub fn epoch_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|s| s.as_millis() as u64)
        .unwrap_or(0)
}

/// ProcSight'in çalıştırmasına izin verdiği salt okunur sistem araçları.
///
/// Bu liste **kapalıdır**: listeye girmeyen hiçbir program çalıştırılamaz.
/// Süreç sonlandıran (`taskkill`, `kill`, `sc`), kayıt defteri yazan
/// (`reg add`, `reg delete`) ve paket gönderen araçlar bilinçli olarak yoktur.
pub const IZINLI_ARACLAR: [&str; 5] = ["tasklist", "netstat", "ps", "reg", "wmic"];

/// Bir programın ProcSight tarafından çalıştırılmasına izin verilip
/// verilmediğini söyler.
///
/// `reg.exe` yalnızca `reg query` alt komutuyla çağrılabilir; yazma yapan
/// `reg add`/`reg delete` alt komutları bu fonksiyonu geçemez. Karar
/// `tests/salt_okunur_kanit.rs` içinde pozitif ve negatif testlerle sabitlenir.
pub fn arac_salt_okunur_mu(program: &str, alt_komut: Option<&str>) -> bool {
    let kucuk = program.to_ascii_lowercase();
    if !IZINLI_ARACLAR.contains(&kucuk.as_str()) {
        return false;
    }
    match kucuk.as_str() {
        "reg" => matches!(alt_komut, Some("query")),
        _ => alt_komut.is_none(),
    }
}

/// Seçilen kaynak türüne göre somut bir `InventorySource` üretir.
///
/// `Otomatik` seçildiğinde derleme hedefi belirleyicidir: Windows için
/// `tasklist`, Linux için `/proc`, diğer her şey için `ps`.
pub fn kesif(tur: KaynakTuru) -> Sonuc<Box<dyn InventorySource>> {
    match tur {
        KaynakTuru::Fixture => Ok(Box::new(fixture::FixtureSource::varsayilan())),
        KaynakTuru::Windows => Ok(Box::new(windows::WindowsSource::yeni(false))),
        KaynakTuru::Procfs => Ok(Box::new(procfs::ProcFsSource::yeni("/proc"))),
        KaynakTuru::Ps => Ok(Box::new(ps::PsSource::yeni())),
        KaynakTuru::Otomatik => {
            #[cfg(target_os = "windows")]
            {
                Ok(Box::new(windows::WindowsSource::yeni(false)))
            }
            #[cfg(target_os = "linux")]
            {
                Ok(Box::new(procfs::ProcFsSource::yeni("/proc")))
            }
            #[cfg(not(any(target_os = "windows", target_os = "linux")))]
            {
                Ok(Box::new(ps::PsSource::yeni()))
            }
        }
    }
}

/// İstenen kaynak bu platformda varsa doğrular, yoksa anlaşılır hata döner.
pub fn kaynagi_dogrula(tur: KaynakTuru) -> Sonuc<()> {
    match tur {
        KaynakTuru::Fixture | KaynakTuru::Otomatik => Ok(()),
        KaynakTuru::Windows => {
            if cfg!(target_os = "windows") {
                Ok(())
            } else {
                Err(Hata::KaynakYok {
                    ad: "windows",
                    ayrinti: "bu platformda tasklist/netstat yok".into(),
                })
            }
        }
        KaynakTuru::Procfs => {
            if cfg!(target_os = "linux") {
                Ok(())
            } else {
                Err(Hata::KaynakYok {
                    ad: "procfs",
                    ayrinti: "bu platformda /proc yok".into(),
                })
            }
        }
        KaynakTuru::Ps => {
            if cfg!(any(
                target_os = "macos",
                target_os = "freebsd",
                target_os = "openbsd"
            )) {
                Ok(())
            } else {
                Err(Hata::KaynakYok {
                    ad: "ps",
                    ayrinti: "bu platformda ps/netstat yok".into(),
                })
            }
        }
    }
}

#[cfg(test)]
// Gerekçe: unwrap/expect yalnızca test içinde kullanılır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn izinli_arac_listesi_kapatilir() {
        assert!(arac_salt_okunur_mu("tasklist", None));
        assert!(arac_salt_okunur_mu("TASKLIST", None));
        assert!(arac_salt_okunur_mu("netstat", None));
        assert!(arac_salt_okunur_mu("reg", Some("query")));
    }

    #[test]
    fn surec_sonlandiran_araclar_izinsizdir() {
        assert!(!arac_salt_okunur_mu("taskkill", None));
        assert!(!arac_salt_okunur_mu("kill", None));
        assert!(!arac_salt_okunur_mu("sc", None));
        assert!(!arac_salt_okunur_mu("powershell", Some("-Command")));
    }

    #[test]
    fn kayit_yazan_reg_alt_komutlari_izinsizdir() {
        assert!(!arac_salt_okunur_mu("reg", Some("add")));
        assert!(!arac_salt_okunur_mu("reg", Some("delete")));
        assert!(!arac_salt_okunur_mu("reg", None));
    }

    #[test]
    fn fixture_kaynagi_her_zaman_kesfedilir() {
        let kaynak = kesif(KaynakTuru::Fixture).expect("fixture keşfedilemedi");
        assert_eq!(kaynak.ad(), "fixture");
        assert!(!kaynak.surecler().expect("süreç okunamadı").is_empty());
    }

    #[test]
    fn kaynak_turu_etiketleri_kisa_ve_ascii() {
        for tur in [
            KaynakTuru::Otomatik,
            KaynakTuru::Windows,
            KaynakTuru::Procfs,
            KaynakTuru::Ps,
            KaynakTuru::Fixture,
        ] {
            let etiket = tur.etiket();
            assert!(etiket.is_ascii(), "{}", etiket);
            assert!(!etiket.is_empty());
        }
    }

    #[test]
    fn otomatik_kesif_calisan_platformda_hata_vermez() {
        let kaynak = kesif(KaynakTuru::Otomatik).expect("otomatik keşif başarısız");
        assert!(!kaynak.ad().is_empty());
    }
}
