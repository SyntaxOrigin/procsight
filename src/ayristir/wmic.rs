//! `wmic process ... /format:csv` çıktısının ayrıştırıcısı.
//!
//! `wmic` Windows'un kendi WMI istemcisidir ve `tasklist`in vermediği iki
//! alanı sağlar: **komut satırı** ve **üst süreç**. Bu alanlar olmadan
//! "hangi program ne yaptı" sorusu yanıtlanamaz; bu yüzden ayrıştırıcı var.
//!
//! `wmic` Windows 11 24H2 sürümünde varsayılan olarak kaldırılmıştır. Bu
//! nedenle `WindowsSource` bu veriyi **isteğe bağlı zenginleştirme** olarak
//! toplar: `wmic` yoksa araç hata vermez, yalnızca `komut_satiri` ve `ppid`
//! alanları boş kalır ve bu durum raporda açıkça yazılır.
//!
//! `wmic /format:csv` sütunları alfabetik sırada basar:
//! `CommandLine,ExecutablePath,HandleCount,Name,ParentProcessId,ProcessId`.

use std::path::PathBuf;

use crate::ayristir::AyristirmaOzeti;

/// Sütun adları eşleşmediğinde kullanılan varsayılan sıra.
const VARSAYILAN_SUTUNLAR: [&str; 5] = [
    "CommandLine",
    "ExecutablePath",
    "Name",
    "ParentProcessId",
    "ProcessId",
];

/// Ayrıştırılmış bir `wmic process` satırı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WmicSatiri {
    /// Süreç kimliği.
    pub pid: u32,
    /// Üst süreç kimliği.
    pub ppid: u32,
    /// Süreç adı.
    pub ad: String,
    /// Komut satırı; `WMI` sunmadığında boş.
    pub komut_satiri: String,
    /// Yürütülebilir dosyanın yolu; erişilemezse boş.
    pub yol: Option<PathBuf>,
}

/// `wmic process ... /format:csv` çıktısının tamamını ayrıştırır.
///
/// Başlık satırı bulunamazsa alfabetik varsayılan sütun sırası kullanılır.
pub fn ayristir(cikti: &str) -> (Vec<WmicSatiri>, AyristirmaOzeti) {
    let mut satirlar = Vec::new();
    let mut ozet = AyristirmaOzeti::yeni();
    let mut sutunlar: Vec<String> = VARSAYILAN_SUTUNLAR.iter().map(|s| s.to_string()).collect();
    let mut baslik_goruldu = false;

    for satir in cikti.lines() {
        let kirp = satir.trim();
        if kirp.is_empty() {
            continue;
        }
        let alanlar = crate::ayristir::tasklist::csv_bol(kirp);

        if !baslik_goruldu && alanlar.iter().any(|a| a == "ProcessId") {
            sutunlar = alanlar;
            baslik_goruldu = true;
            ozet.bilgi_say();
            continue;
        }
        // Başlık yoksa varsayılan sırayla devam et; ilk satır veridir.

        let deger = |ad: &str| -> Option<&str> {
            sutunlar
                .iter()
                .position(|s| s == ad)
                .and_then(|i| alanlar.get(i))
                .map(String::as_str)
        };
        let pid = deger("ProcessId").and_then(|s| s.trim().parse::<u32>().ok());
        let pid = match pid {
            Some(p) => p,
            None => {
                if kirp.to_ascii_lowercase().contains("node") && !baslik_goruldu {
                    ozet.bilgi_say();
                } else {
                    ozet.atla();
                }
                continue;
            }
        };
        let ppid = deger("ParentProcessId")
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(0);
        let ad = deger("Name").unwrap_or_default().trim().to_string();
        if ad.is_empty() {
            ozet.atla();
            continue;
        }
        ozet.say();
        satirlar.push(WmicSatiri {
            pid,
            ppid,
            ad,
            komut_satiri: deger("CommandLine").unwrap_or_default().trim().to_string(),
            yol: deger("ExecutablePath")
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(PathBuf::from),
        });
    }
    (satirlar, ozet)
}

/// Bir `wmic` satırını platformdan bağımsız süreç kaydına dönüştürür.
///
/// `taban` kaydı (genellikle `tasklist`dan gelen) bellek, oturum ve durum
/// bilgisini taşır; `wmic` yalnızca eksik alanları tamamlar.
pub fn surece_cevir(taban: &crate::model::Surec, satiri: &WmicSatiri) -> crate::model::Surec {
    crate::model::Surec {
        pid: taban.pid,
        ppid: satiri.ppid,
        ad: if satiri.ad.is_empty() {
            taban.ad.clone()
        } else {
            satiri.ad.clone()
        },
        komut_satiri: if satiri.komut_satiri.is_empty() {
            taban.komut_satiri.clone()
        } else {
            satiri.komut_satiri.clone()
        },
        yol: satiri.yol.clone().or_else(|| taban.yol.clone()),
        cpu_yuzde: taban.cpu_yuzde,
        bellek_kib: taban.bellek_kib,
        oturum: taban.oturum,
        kullanici: taban.kullanici.clone(),
        uid: taban.uid,
        durum: taban.durum,
    }
}

#[cfg(test)]
// Gerekçe: unwrap/expect yalnızca test içinde kullanılır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::model::Surec;
    use crate::model::SurecDurumu;

    const ORNEK: &str = "\r\n\
Node,CommandLine,ExecutablePath,HandleCount,Name,ParentProcessId,ProcessId\r\n\
MASA,notepad.exe,C:\\Windows\\Notepad\\notepad.exe,320,Notepad.exe,4321,555\r\n\
MASA,\"C:\\Users\\K\\AppData\\Local\\Temp\\X.exe\" --run,C:\\Users\\K\\AppData\\Local\\Temp\\X.exe,88,X.exe,4321,9001\r\n\
";

    #[test]
    fn iki_satir_ayristirilir() {
        let (satirlar, ozet) = ayristir(ORNEK);
        assert_eq!(satirlar.len(), 2);
        assert_eq!(ozet.okunan, 2);
        assert_eq!(ozet.bilgi, 1);
        assert_eq!(satirlar[0].pid, 555);
        assert_eq!(satirlar[0].ppid, 4321);
        assert_eq!(satirlar[0].ad, "Notepad.exe");
    }

    #[test]
    fn tirnakli_komut_satiri_ayraclari_korur() {
        let (satirlar, _) = ayristir(ORNEK);
        assert_eq!(
            satirlar[1].komut_satiri,
            "C:\\Users\\K\\AppData\\Local\\Temp\\X.exe --run"
        );
        assert_eq!(
            satirlar[1].yol,
            Some(PathBuf::from("C:\\Users\\K\\AppData\\Local\\Temp\\X.exe"))
        );
    }

    #[test]
    fn eksik_sutunlarda_ppidsifir_olur() {
        let cikti = "CommandLine,Name,ProcessId\r\nnotepad.exe,notepad.exe,77\r\n";
        let (satirlar, _) = ayristir(cikti);
        assert_eq!(satirlar[0].ppid, 0);
        assert_eq!(satirlar[0].pid, 77);
    }

    #[test]
    fn bos_yol_none_doner() {
        let cikti = "CommandLine,ExecutablePath,Name,ParentProcessId,ProcessId\r\n\
                    x.exe,,x.exe,1,5\r\n";
        let (satirlar, _) = ayristir(cikti);
        assert!(satirlar[0].yol.is_none());
    }

    #[test]
    fn sayisal_pid_olmayan_satir_bozuk_sayilir() {
        let cikti = "CommandLine,Name,ParentProcessId,ProcessId\r\nx.exe,x.exe,1,pid\r\n";
        let (satirlar, ozet) = ayristir(cikti);
        assert!(satirlar.is_empty());
        assert_eq!(ozet.atlanan, 1);
    }

    #[test]
    fn ad_bos_satir_bozuk_sayilir() {
        let cikti = "CommandLine,Name,ParentProcessId,ProcessId\r\nx.exe,,1,5\r\n";
        let (_, ozet) = ayristir(cikti);
        assert_eq!(ozet.atlanan, 1);
    }

    #[test]
    fn bos_girdi_kayit_uretmez() {
        let (satirlar, ozet) = ayristir("");
        assert!(satirlar.is_empty());
        assert_eq!(ozet, AyristirmaOzeti::yeni());
    }

    #[test]
    fn surece_cevirme_eksik_alanlari_korur() {
        let (satirlar, _) = ayristir(ORNEK);
        let taban = Surec {
            pid: 555,
            ppid: 0,
            ad: "Notepad.exe".into(),
            komut_satiri: String::new(),
            yol: None,
            cpu_yuzde: None,
            bellek_kib: 12_288,
            oturum: Some(1),
            kullanici: Some("kullanici".into()),
            uid: Some(1000),
            durum: SurecDurumu::Calisiyor,
        };
        let kayit = surece_cevir(&taban, &satirlar[0]);
        assert_eq!(kayit.ppid, 4321);
        assert_eq!(kayit.bellek_kib, 12_288);
        assert_eq!(kayit.kullanici, Some("kullanici".to_string()));
        assert_eq!(
            kayit.yol,
            Some(PathBuf::from("C:\\Windows\\Notepad\\notepad.exe"))
        );
    }
}
