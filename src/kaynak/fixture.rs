//! Testler ve `--source fixture` kipi için statik envanter kaynağı.
//!
//! Bu kaynak **hiçbir komut çalıştırmaz**, hiçbir dosya okumaz ve hiçbir
//! ağ isteği yapmaz. Tanımladığı veri seti bilinçli olarak yedi kuralın
//! her birini tetikleyen ve tetiklemeyen kayıtları içerir; böylece kural
//! motoru platformdan bağımsız olarak test edilebilir.
//!
//! Veri seti Windows'a özgüdür (`tasklist`/`netstat` sütun düzeni), ama
//! kural motoru platform tanımaz; Linux kaynağı da aynı kayıt şeklini üretir.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;

use crate::hata::Sonuc;
use crate::kaynak::InventorySource;
use crate::model::{
    Baglanti, BaglantiDurumu, Protokol, StartupGirdi, StartupTuru, Surec, SurecDurumu,
};

/// Sabit, deterministik veri seti sağlayan envanter kaynağı.
#[derive(Debug, Clone, Default)]
pub struct FixtureSource {
    surecler: Vec<Surec>,
    baglantilar: Vec<Baglanti>,
    baslangic: Vec<StartupGirdi>,
}

impl FixtureSource {
    /// Boş bir kaynak üretir.
    pub fn bos() -> Self {
        FixtureSource::default()
    }

    /// Testlerin ve demo kipinin kullandığı zengin veri setini üretir.
    ///
    /// Veri setinde şu kayıtlar bulunur:
    ///
    /// | pid | amaç |
    /// |-----|-------|
    /// | 9001 | gizli ad + şüpheli konum + yüksek bellek + beklenmeyen bağlantı |
    /// | 9002 | yalnızca şüpheli konum (AppData) |
    /// | 9003 | üst süreçten farklı kullanıcı kimliği (yetki geçişi) |
    /// | 4321, 555, 1044 | kuralların **tetiklenmediği** temiz kayıtlar |
    pub fn varsayilan() -> Self {
        FixtureSource {
            surecler: varsayilan_surecler(),
            baglantilar: varsayilan_baglantilar(),
            baslangic: varsayilan_baslangic(),
        }
    }

    /// Testlerin kendi kayıtlarını yükleyebilmesi için üretici.
    pub fn with_records(
        surecler: Vec<Surec>,
        baglantilar: Vec<Baglanti>,
        baslangic: Vec<StartupGirdi>,
    ) -> Self {
        FixtureSource {
            surecler,
            baglantilar,
            baslangic,
        }
    }
}

impl InventorySource for FixtureSource {
    fn ad(&self) -> &str {
        "fixture"
    }

    fn surecler(&self) -> Sonuc<Vec<Surec>> {
        Ok(self.surecler.clone())
    }

    fn ag_baglantilari(&self) -> Sonuc<Vec<Baglanti>> {
        Ok(self.baglantilar.clone())
    }

    fn baslangic_girdileri(&self) -> Sonuc<Vec<StartupGirdi>> {
        Ok(self.baslangic.clone())
    }

    fn kisit_notu(&self) -> String {
        "statik veri seti; canlı sistemi gormez".to_string()
    }
}

/// Varsayılan veri kümesi için süreç kaydı üretir.
///
/// `uid` tek başına oturum ve kullanıcı alanlarını da belirler: `0` ise
/// `NT AUTHORITY\SYSTEM` oturum `0`, aksi hâlde `DESKTOP\KULLANICI` oturum `1`.
/// Bu kural veri kümesini tutarlı kılar ve çağrıları kısa tutar.
fn surec_kaydi(
    pid: u32,
    ppid: u32,
    ad: &str,
    komut: &str,
    yol: Option<&str>,
    bellek_kib: u64,
    uid: u32,
) -> Surec {
    let (oturum, kullanici) = if uid == 0 {
        (0, "NT AUTHORITY\\SYSTEM")
    } else {
        (1, "DESKTOP\\KULLANICI")
    };
    Surec {
        pid,
        ppid,
        ad: ad.to_string(),
        komut_satiri: komut.to_string(),
        yol: yol.map(PathBuf::from),
        cpu_yuzde: None,
        bellek_kib,
        oturum: Some(oturum),
        kullanici: Some(kullanici.to_string()),
        uid: Some(uid),
        durum: SurecDurumu::Calisiyor,
    }
}

fn varsayilan_surecler() -> Vec<Surec> {
    vec![
        surec_kaydi(4, 0, "System", "System", None, 2_048, 0),
        surec_kaydi(
            1044,
            4,
            "svchost.exe",
            "C:\\Windows\\System32\\svchost.exe -k netsvcs",
            Some("C:\\Windows\\System32\\svchost.exe"),
            21_300,
            0,
        ),
        surec_kaydi(
            4321,
            1044,
            "explorer.exe",
            "C:\\Windows\\explorer.exe",
            Some("C:\\Windows\\explorer.exe"),
            98_412,
            1000,
        ),
        surec_kaydi(
            555,
            4321,
            "notepad.exe",
            "C:\\Windows\\Notepad\\notepad.exe",
            Some("C:\\Windows\\Notepad\\notepad.exe"),
            12_288,
            1000,
        ),
        surec_kaydi(
            77,
            4,
            "wmiprvse.exe",
            "C:\\Windows\\System32\\wbem\\wmiprvse.exe",
            Some("C:\\Windows\\System32\\wbem\\wmiprvse.exe"),
            14_336,
            0,
        ),
        // Kural 1 (gizli ad), 2 (şüpheli konum), 3 (yüksek bellek),
        // 5 (beklenmeyen bağlantı) ve 7 (dünyaya açık konumdan çalışma).
        surec_kaydi(
            9001,
            4321,
            "Xk3jdH9s7qW.exe",
            "C:\\Users\\KULLANICI\\AppData\\Local\\Temp\\Xk3jdH9s7qW.exe",
            Some("C:\\Users\\KULLANICI\\AppData\\Local\\Temp\\Xk3jdH9s7qW.exe"),
            1_572_864,
            1000,
        ),
        // Yalnızca şüpheli konum (AppData) tetiklenir.
        surec_kaydi(
            9002,
            4321,
            "updater.exe",
            "C:\\Users\\KULLANICI\\AppData\\Roaming\\Updater\\upd.exe",
            Some("C:\\Users\\KULLANICI\\AppData\\Roaming\\Updater\\upd.exe"),
            40_960,
            1000,
        ),
        // Kural 7 (üst süreçten farklı kullanıcı kimliği).
        surec_kaydi(
            9003,
            1044,
            "svchost-helper.exe",
            "C:\\Windows\\System32\\svchost-helper.exe",
            Some("C:\\Windows\\System32\\svchost-helper.exe"),
            8_192,
            1000,
        ),
    ]
}

fn tcp(
    yerel: (IpAddr, u16),
    uzak: Option<(IpAddr, u16)>,
    durum: BaglantiDurumu,
    pid: u32,
) -> Baglanti {
    Baglanti {
        protokol: Protokol::Tcp,
        yerel_adres: yerel.0,
        yerel_port: yerel.1,
        uzak_adres: uzak.map(|u| u.0),
        uzak_port: uzak.map(|u| u.1),
        durum,
        pid: Some(pid),
    }
}

fn varsayilan_baglantilar() -> Vec<Baglanti> {
    let dongusel: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let tum_adres: IpAddr = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
    let ev: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 5));
    let ev_yonlendirici: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10));
    let ornek_site: IpAddr = IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34));
    let yabanci: IpAddr = IpAddr::V4(Ipv4Addr::new(45, 83, 220, 17));
    let belgeleme: IpAddr = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 9));

    vec![
        // Dinleyen soketler: "dışarı çıkan bağlantı" değildir.
        tcp((tum_adres, 135), None, BaglantiDurumu::Dinliyor, 4),
        tcp((tum_adres, 445), None, BaglantiDurumu::Dinliyor, 4),
        // İyi bilinen port + genel adres: kurall tetiklenmez.
        tcp(
            (dongusel, 51_000),
            Some((ornek_site, 443)),
            BaglantiDurumu::Kuruldu,
            4321,
        ),
        // Bilinmeyen port + genel adres: kural 5 tetiklenir.
        tcp(
            (dongusel, 51_001),
            Some((yabanci, 8443)),
            BaglantiDurumu::Kuruldu,
            9001,
        ),
        // Özel ağ içi: kurall tetiklenmez.
        tcp(
            (ev, 51_002),
            Some((ev_yonlendirici, 445)),
            BaglantiDurumu::Kuruldu,
            9001,
        ),
        // Belgeleme bloğu küresel sayılmaz: kural tetiklenmez.
        tcp(
            (dongusel, 51_003),
            Some((belgeleme, 9001)),
            BaglantiDurumu::Kuruldu,
            9002,
        ),
        Baglanti {
            protokol: Protokol::Udp,
            yerel_adres: IpAddr::V6(Ipv6Addr::UNSPECIFIED),
            yerel_port: 53_533,
            uzak_adres: None,
            uzak_port: None,
            durum: BaglantiDurumu::Bos,
            pid: Some(4321),
        },
    ]
}

fn varsayilan_baslangic() -> Vec<StartupGirdi> {
    let standart =
        "C:\\Users\\KULLANICI\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\Programs\\Startup";
    vec![
        // Negatif: standart Başlangıç klasöründe, hedefi temiz bir konumda.
        StartupGirdi {
            konum: PathBuf::from(format!("{}\\{}", standart, "OneDrive.lnk")),
            ad: "OneDrive".into(),
            tur: StartupTuru::BaslangicKlasoru,
            etkin: true,
            hedef: Some("C:\\Program Files\\Microsoft OneDrive\\OneDrive.exe".into()),
        },
        // Pozitif: hedef geçici konumda.
        StartupGirdi {
            konum: PathBuf::from(format!("{}\\{}", standart, "Kurulum.lnk")),
            ad: "Kurulum".into(),
            tur: StartupTuru::BaslangicKlasoru,
            etkin: true,
            hedef: Some("C:\\Users\\KULLANICI\\AppData\\Local\\Temp\\kurulum.exe".into()),
        },
        // Negatif: Run anahtarı, hedefi sistem konumunda.
        StartupGirdi {
            konum: PathBuf::from("HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            ad: "SecurityHealth".into(),
            tur: StartupTuru::RunAnahtari,
            etkin: true,
            hedef: Some("C:\\Windows\\system32\\SecurityHealthSystray.exe".into()),
        },
        // Pozitif: Run anahtarı, hedefi profil içi gizli konumda.
        StartupGirdi {
            konum: PathBuf::from("HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            ad: "UpdaterTask".into(),
            tur: StartupTuru::RunAnahtari,
            etkin: false,
            hedef: Some("C:\\Users\\KULLANICI\\AppData\\Roaming\\Updater\\upd.exe".into()),
        },
    ]
}

#[cfg(test)]
// Gerekçe: unwrap/expect yalnızca test içinde kullanılır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::model::supheli_konum_mi;

    #[test]
    fn bos_kaynak_hicbir_kayit_uretmez() {
        let kaynak = FixtureSource::bos();
        assert!(kaynak.surecler().unwrap().is_empty());
        assert!(kaynak.ag_baglantilari().unwrap().is_empty());
        assert!(kaynak.baslangic_girdileri().unwrap().is_empty());
    }

    #[test]
    fn varsayilan_veri_kumesi_yedi_urune_sahiptir() {
        let kaynak = FixtureSource::varsayilan();
        assert_eq!(kaynak.surecler().unwrap().len(), 8);
        assert_eq!(kaynak.ag_baglantilari().unwrap().len(), 7);
        assert_eq!(kaynak.baslangic_girdileri().unwrap().len(), 4);
    }

    #[test]
    fn kayitlar_ppid_zinciri_kurarli() {
        let kaynak = FixtureSource::varsayilan();
        let surecler = kaynak.surecler().unwrap();
        for s in &surecler {
            if s.ppid == 0 {
                continue;
            }
            assert!(
                surecler.iter().any(|a| a.pid == s.ppid),
                "{} ppid {} bulunamadi",
                s.ad,
                s.ppid
            );
        }
    }

    #[test]
    fn kayitlar_uyumlu_kimlikler_tasir() {
        let kaynak = FixtureSource::varsayilan();
        for b in kaynak.ag_baglantilari().unwrap() {
            if let Some(pid) = b.pid {
                assert!(
                    kaynak.surecler().unwrap().iter().any(|s| s.pid == pid),
                    "baglanti sahibi {} yok",
                    pid
                );
            }
        }
    }

    #[test]
    fn en_fazla_bir_suspicious_konumlu_surec_vardir_kural_icin() {
        // Kural 2 testleri bu kayda dayanır; adı sabit tutulur.
        let kaynak = FixtureSource::varsayilan();
        let sayilan = kaynak
            .surecler()
            .unwrap()
            .iter()
            .filter(|s| s.yol.as_deref().and_then(supheli_konum_mi).is_some())
            .count();
        assert_eq!(sayilan, 2);
    }
}
