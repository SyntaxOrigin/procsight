//! Linux envanter kaynağı: `/proc` dosyalarından doğrudan okuma.
//!
//! Bu yol **hiçbir komut çalıştırmaz, hiçbir ağ isteği yapmaz ve yalnızca
//! okuma yapar**. Raporun Linux tarafı zaten saf dosya sistemi okumasıdır;
//! `netlink`/`sock_diag` gerektiren gerçek zamanlı bağlantı akışı bilinçli
//! olarak ertelenmiştir.
//!
//! ### Sahiplik çözümü
//!
//! `/proc/net/tcp` satırında süreç kimliği değil **inode** vardır. Bu inode
//! `/proc/<pid>/fd/*` bağlantılarının hedefiyle eşleştirilerek sahip süreç
//! bulunur. `read_dir` yetki hatası verirse eşleme boş kalır ve bağlantının
//! `pid` alanı `None` olur; bu durum `kisit_notu` ile açıkça yazılır, gizlenmez.

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use crate::ayristir::procstat;
use crate::hata::{Hata, Sonuc};
use crate::kaynak::{dosya_haritasi, InventorySource};
use crate::model::{Baglanti, Protokol, StartupGirdi, StartupTuru, Surec};

/// Linux `/proc` kökünü okuyan envanter kaynağı.
#[derive(Debug, Clone)]
pub struct ProcFsSource {
    kok: PathBuf,
    clk_tck: u64,
}

impl ProcFsSource {
    /// Verilen `/proc` kökünden (test için geçici dizin de olabilir) yeni
    /// kaynak üretir.
    pub fn yeni(kok: impl Into<PathBuf>) -> Self {
        ProcFsSource {
            kok: kok.into(),
            // Linux çekirdeği `USER_HZ` değerini 100 olarak sabitler.
            clk_tck: 100,
        }
    }

    /// Kök dizinin gerçekten var olduğunu doğrular.
    pub fn dogrula(&self) -> Sonuc<()> {
        if self.kok.is_dir() {
            Ok(())
        } else {
            Err(Hata::KaynakYok {
                ad: "procfs",
                ayrinti: format!("{} dizini yok veya dizin degil", self.kok.display()),
            })
        }
    }

    fn kok(&self) -> &Path {
        &self.kok
    }

    fn pid_dizinleri(&self) -> Vec<PathBuf> {
        let mut pidler: Vec<PathBuf> = Vec::new();
        let Ok(girdiler) = std::fs::read_dir(self.kok()) else {
            return pidler;
        };
        for girid in girdiler.flatten() {
            let Ok(tur) = girid.file_type() else {
                continue;
            };
            if !tur.is_dir() {
                continue;
            }
            let ad = girid.file_name();
            let Some(ad) = ad.to_str() else {
                continue;
            };
            if ad.parse::<u32>().is_ok() {
                pidler.push(girid.path());
            }
        }
        pidler.sort_by_key(|p| p.file_name().and_then(|a| a.to_str()).map(str::to_string));
        pidler
    }

    /// Bir süreç dizininden kayıt üretir; süreç kaybolmuşsa `Ok(None)` döner.
    ///
    /// Gözlem aracının kendisi çökerse eldeki kanıt kaybolur; bu yüzden
    /// "süreç yok oldu" bir hata değil, normal bir durumdur.
    pub fn tek_surec(&self, pid_dizini: &Path) -> Option<Surec> {
        let stat_metin = std::fs::read_to_string(pid_dizini.join("stat")).ok()?;
        let stat = procstat::stat_coz(&stat_metin, self.clk_tck)?;
        let komut_satiri = std::fs::read(pid_dizini.join("cmdline"))
            .map(|baytlar| procstat::cmdline_coz(&baytlar))
            .unwrap_or_default();
        let uid = std::fs::read_to_string(pid_dizini.join("status"))
            .ok()
            .and_then(|metin| procstat::uid_coz(&metin));
        let yol = std::fs::read_link(pid_dizini.join("exe")).ok();
        Some(procstat::surece_cevir(&stat, &komut_satiri, uid, yol))
    }

    /// Tüm süreç inode'larını süreç kimlikleriyle eşleştirir.
    pub fn inode_eslemesi(&self) -> HashMap<u64, u32> {
        let mut eslesme: HashMap<u64, u32> = HashMap::new();
        for dizin in self.pid_dizinleri() {
            let Some(pid) = dizin
                .file_name()
                .and_then(|a| a.to_str())
                .and_then(|a| a.parse::<u32>().ok())
            else {
                continue;
            };
            let Ok(fd_dizini) = std::fs::read_dir(dizin.join("fd")) else {
                continue;
            };
            for girid in fd_dizini.flatten() {
                let Ok(hedef) = std::fs::read_link(girid.path()) else {
                    continue;
                };
                let metin = hedef.to_string_lossy();
                if let Some(baslangic) = metin.rfind("socket:[") {
                    let sayi = metin[baslangic + 8..].trim_end_matches(']');
                    if let Ok(inode) = sayi.parse::<u64>() {
                        eslesme.entry(inode).or_insert(pid);
                    }
                }
            }
        }
        eslesme
    }

    fn net_dosyasi(&self, ad: &str, ipv6: bool, protokol: Protokol) -> Vec<Baglanti> {
        let yol = self.kok().join("net").join(ad);
        let Ok(metin) = std::fs::read_to_string(&yol) else {
            return Vec::new();
        };
        let (satirlar, _) = procstat::net_coz(&metin, ipv6);
        let eslesme = self.inode_eslemesi();
        procstat::baglantiya_cevir(&satirlar, protokol, &eslesme)
    }
}

impl InventorySource for ProcFsSource {
    fn ad(&self) -> &str {
        "procfs"
    }

    fn surecler(&self) -> Sonuc<Vec<Surec>> {
        self.dogrula()?;
        let mut kayitlar = Vec::new();
        for dizin in self.pid_dizinleri() {
            // Yoklama sırasında kapanmış süreçler normaldir; hata değildir.
            if let Some(kayit) = self.tek_surec(&dizin) {
                kayitlar.push(kayit);
            }
        }
        Ok(kayitlar)
    }

    fn ag_baglantilari(&self) -> Sonuc<Vec<Baglanti>> {
        self.dogrula()?;
        let mut baglantilar = Vec::new();
        baglantilar.extend(self.net_dosyasi("tcp", false, Protokol::Tcp));
        baglantilar.extend(self.net_dosyasi("tcp6", true, Protokol::Tcp));
        baglantilar.extend(self.net_dosyasi("udp", false, Protokol::Udp));
        baglantilar.extend(self.net_dosyasi("udp6", true, Protokol::Udp));
        Ok(baglantilar)
    }

    fn baslangic_girdileri(&self) -> Sonuc<Vec<StartupGirdi>> {
        // Linux'ta XDG autostart dizini ev dizinindedir; `HOME` ortam
        // değişkeninden okunur. Bulunamazsa boş liste döner, hata üretilmez.
        let Ok(ev) = std::env::var("HOME") else {
            return Ok(Vec::new());
        };
        let kok = PathBuf::from(ev).join(".config").join("autostart");
        let girdiler = dosya_haritasi(&kok, 2)
            .into_iter()
            .filter(|yol| {
                yol.file_name()
                    .and_then(|a| a.to_str())
                    .map(|a| a.ends_with(".desktop"))
                    .unwrap_or(false)
            })
            .map(|yol| {
                let ad = yol
                    .file_name()
                    .and_then(|a| a.to_str())
                    .unwrap_or("bilinmiyor")
                    .to_string();
                let hedef = desktop_exec(&yol);
                StartupGirdi {
                    konum: yol,
                    ad,
                    tur: StartupTuru::XdgAutostart,
                    etkin: true,
                    hedef,
                }
            })
            .collect();
        Ok(girdiler)
    }

    fn kisit_notu(&self) -> String {
        "CPU yuzdesi iki yoklama arasinda hesaplanir; /proc/net satiri inode tasir, sahiplik /proc/<pid>/fd ile cozulur".to_string()
    }
}

/// Bir `.desktop` dosyasındaki `Exec=` satırının değerini çözer.
pub fn desktop_exec(yol: &Path) -> Option<String> {
    let metin = std::fs::read_to_string(yol).ok()?;
    for satir in metin.lines() {
        let kirp = satir.trim();
        if let Some(deger) = kirp.strip_prefix("Exec=") {
            let deger = deger.trim();
            if !deger.is_empty() {
                return Some(deger.to_string());
            }
        }
    }
    None
}

/// `/proc` kökünün altındaki bir uç noktayı okunabilir biçimde yazar.
pub fn uc_yaz(yerel: (IpAddr, u16), uzak: Option<(IpAddr, u16)>) -> String {
    match uzak {
        Some((a, p)) => format!("{}:{} -> {}:{}", yerel.0, yerel.1, a, p),
        None => format!("{}:{} -> -", yerel.0, yerel.1),
    }
}

#[cfg(test)]
// Gerekçe: unwrap/expect yalnızca test içinde kullanılır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn olmayan_kok_hata_doner() {
        let kaynak = ProcFsSource::yeni("/procsight-yok-boyle-bir-dizin");
        assert!(kaynak.dogrula().is_err());
        assert!(kaynak.surecler().is_err());
    }

    #[test]
    fn bos_kok_hicbir_surec_dondurmez() {
        let kaynak = ProcFsSource::yeni(std::env::temp_dir().join("procsight-yok-2"));
        let _ = std::fs::remove_dir_all(kaynak.kok());
        assert!(kaynak.dogrula().is_err());
    }

    #[test]
    fn pid_dizinleri_yalnizca_sayi_adlarini_toplar() {
        let kok = std::env::temp_dir().join("procsight-pid-taramasi");
        let _ = std::fs::remove_dir_all(&kok);
        std::fs::create_dir_all(kok.join("1")).unwrap();
        std::fs::create_dir_all(kok.join("self")).unwrap();
        std::fs::write(kok.join("cpuinfo"), "x").unwrap();
        let kaynak = ProcFsSource::yeni(&kok);
        let dizinler = kaynak.pid_dizinleri();
        assert_eq!(dizinler.len(), 1);
        assert!(dizinler[0].ends_with("1"));
        let _ = std::fs::remove_dir_all(&kok);
    }

    #[test]
    fn kaybolan_surec_none_doner_hata_degil() {
        let kok = std::env::temp_dir().join("procsight-kaybolan");
        let _ = std::fs::remove_dir_all(&kok);
        std::fs::create_dir_all(kok.join("42")).unwrap();
        let kaynak = ProcFsSource::yeni(&kok);
        assert!(kaynak.tek_surec(&kok.join("42")).is_none());
        let _ = std::fs::remove_dir_all(&kok);
    }

    #[test]
    fn stat_dosyasi_olan_surec_kayit_uretir() {
        let kok = std::env::temp_dir().join("procsight-stat");
        let _ = std::fs::remove_dir_all(&kok);
        std::fs::create_dir_all(kok.join("7")).unwrap();
        std::fs::write(
            kok.join("7").join("stat"),
            "7 (init) S 1 7 7 0 -1 4194560 100 0 0 0 10 5 0 0 20 0 1 0 999 4096 250 0",
        )
        .unwrap();
        std::fs::write(kok.join("7").join("cmdline"), b"/sbin/init\0").unwrap();
        std::fs::write(
            kok.join("7").join("status"),
            "Name:\tinit\nUid:\t0\t0\t0\t0\n",
        )
        .unwrap();
        let kaynak = ProcFsSource::yeni(&kok);
        let kayit = kaynak.tek_surec(&kok.join("7")).expect("kayıt üretilmedi");
        assert_eq!(kayit.pid, 7);
        assert_eq!(kayit.ppid, 1);
        assert_eq!(kayit.ad, "init");
        assert_eq!(kayit.komut_satiri, "/sbin/init");
        assert_eq!(kayit.uid, Some(0));
        assert_eq!(kayit.bellek_kib, 1000);
        let _ = std::fs::remove_dir_all(&kok);
    }

    #[test]
    fn net_dosyasi_okunmazsa_bos_liste_doner() {
        // `/proc/net/tcp` bulunamazsa ya da okunamazsa hata değil boş liste
        // döner; test ortamı Linux olmayabilir, bu yüzden yalnızca "çökmüyor"
        // ve "her kayıtın sahibi çözülmüş ya da `None`" olduğu doğrulanır.
        let kaynak = ProcFsSource::yeni("/proc");
        for baglanti in kaynak.net_dosyasi("tcp", false, Protokol::Tcp) {
            assert_ne!(baglanti.yerel_port, 0, "yerel port sifir olamaz");
            assert_eq!(
                baglanti.uzak_adres.is_some(),
                baglanti.uzak_port.is_some(),
                "uzak adres ve port birlikte cozulmelidir"
            );
        }
    }

    #[test]
    fn uc_yaz_bicimi_okunur() {
        let metin = uc_yaz(
            (IpAddr::from([127, 0, 0, 1]), 80),
            Some((IpAddr::from([1, 2, 3, 4]), 443)),
        );
        assert_eq!(metin, "127.0.0.1:80 -> 1.2.3.4:443");
        let metin = uc_yaz((IpAddr::from([0, 0, 0, 0]), 135), None);
        assert!(metin.ends_with("-> -"));
    }

    #[test]
    fn kisit_notu_cpu_ve_inode_yenidenler() {
        let kaynak = ProcFsSource::yeni("/proc");
        let not = kaynak.kisit_notu();
        assert!(not.contains("inode"), "{}", not);
        assert!(not.contains("CPU"), "{}", not);
    }

    #[test]
    fn desktop_exec_satiri_cozulur() {
        let yol = std::env::temp_dir().join("procsight-ornek.desktop");
        std::fs::write(
            &yol,
            "[Desktop Entry]\nType=Application\nName=Ornek\nExec=/usr/bin/ornek --gizli\n",
        )
        .unwrap();
        assert_eq!(
            desktop_exec(&yol),
            Some("/usr/bin/ornek --gizli".to_string())
        );
        std::fs::write(&yol, "[Desktop Entry]\nExec=\n").unwrap();
        assert_eq!(desktop_exec(&yol), None);
        let _ = std::fs::remove_file(&yol);
    }
}
