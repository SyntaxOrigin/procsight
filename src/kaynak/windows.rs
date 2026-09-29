//! Windows envanter kaynağı: `tasklist`, `netstat` ve (isteğe bağlı) `wmic`.
//!
//! Bu yol **harici bağımlılık getirmez**: `tasklist.exe`, `netstat.exe` ve
//! `wmic.exe` Windows'un kendi araçlarıdır. Hiçbiri bir soket açmaz; yalnızca
//! işletim sisteminin zaten açık olan tablolarını okur.
//!
//! ## Başlangıç girdileri: neden dizin taraması birincil yol?
//!
//! Başlangıç girdisi üç şekilde bulunabilir: (1) Başlangıç klasörü dosya
//! taraması, (2) `reg query` ile kayıt defteri okuma, (3) görev zamanlayıcı.
//! Raporda istenen 1. madde "basit ve taşınabilir olanı seç" dediği için
//! **dizin taraması birincil** yol olarak seçilmiştir; gerekçesi:
//!
//! - `reg query` bir alt süreç başlatır ve `reg.exe`nin `PATH` üzerinde
//!   bulunmasına bağlıdır; kısıtlı ortamlarda yoktur.
//! - Kayıt defteri anahtar yolları ve değer adları yerelleştirmeye duyarlıdır;
//!   çıktı ayrıştırıcısının dile göre değişmesi gerekir.
//! - Yönetici olmayan oturumda `HKLM\...\Run` anahtarının tamamı okunamaz.
//!   Bu eksiklik arayüzde "tam görünüm" gibi sunulmaz, `kisit_notu` ile açıkça
//!   yazılır.
//!
//! Buna rağmen `reg query` **kapatılmış olarak** uygulanmıştır ve
//! `--reg` bayrağıyla açılır: kalıcılık girdilerinin envanterde görünmesi,
//! bu kaynakların atlanmasından daha değerlidir. `reg query` yalnızca okuma
//! yapar; `reg add`/`reg delete` hiçbir koşulda çağrılmaz (bkz.
//! [`crate::kaynak::arac_salt_okunur_mu`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ayristir::{netstat, tasklist, wmic};
use crate::hata::{Hata, Sonuc};
use crate::kaynak::{dosya_haritasi, InventorySource};
use crate::model::{StartupGirdi, StartupTuru, Surec};

/// `HKCU` altındaki çalıştırma anahtarı.
const HKCU_RUN: &str = "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run";
/// `HKLM` altındaki çalıştırma anahtarı.
const HKLM_RUN: &str = "HKLM\\Software\\Microsoft\\Windows\\CurrentVersion\\Run";

/// Windows envanter kaynağı.
#[derive(Debug, Clone)]
pub struct WindowsSource {
    wmic_zenginlestir: bool,
    reg_sorgula: bool,
    kok_dizinler: Vec<PathBuf>,
}

impl WindowsSource {
    /// Yeni kaynak üretir.
    ///
    /// `wmic_zenginlestir` açıkken komut satırı ve üst süreç bilgisi
    /// `wmic`ten alınır; `reg_sorgula` açıkken `Run` anahtarları
    /// `reg query` ile okunur. İkisi de kapatıldığında kaynak yalnızca
    /// `tasklist` ve `netstat` kullanır.
    pub fn yeni(wmic_zenginlestir: bool) -> Self {
        WindowsSource {
            wmic_zenginlestir,
            reg_sorgula: false,
            kok_dizinler: Vec::new(),
        }
    }

    /// `reg query` ile `Run` anahtarlarının okunmasını açar.
    pub fn reg_ac(mut self) -> Self {
        self.reg_sorgula = true;
        self
    }

    /// Başlangıç klasörlerini test için dışarıdan verir.
    pub fn kok_dizinlerle(mut self, koklar: Vec<PathBuf>) -> Self {
        self.kok_dizinler = koklar;
        self
    }

    fn kok_dizinleri(&self) -> Vec<PathBuf> {
        if !self.kok_dizinler.is_empty() {
            return self.kok_dizinler.clone();
        }
        let mut koklar = Vec::new();
        for (degisken, alt) in [
            (
                "APPDATA",
                "Microsoft\\Windows\\Start Menu\\Programs\\Startup",
            ),
            (
                "PROGRAMDATA",
                "Microsoft\\Windows\\Start Menu\\Programs\\Startup",
            ),
        ] {
            if let Ok(kok) = std::env::var(degisken) {
                koklar.push(PathBuf::from(kok).join(alt));
            }
        }
        koklar
    }

    fn kisit(&self, wmic_var: bool, reg_var: bool) -> String {
        let mut parcalar: Vec<&str> = Vec::new();
        parcalar.push("ust süreç, komut satiri ve yürütülebilir yol tasklist'te yoktur");
        if !wmic_var {
            parcalar.push("wmic bulunamadi veya kaldirildi; komut satiri ve ust surec bos");
        }
        if self.reg_sorgula && !reg_var {
            parcalar.push("reg query basarisiz; Run anahtarlari okunamadi");
        }
        if !self.reg_sorgula {
            parcalar.push("Run anahtarlari --reg bayragi ile okunur");
        }
        parcalar.join("; ")
    }
}

/// `reg query` çıktısındaki `değer adı  REG_SZ  değer` satırlarını çözer.
///
/// Başlık satırı atlanır. Değer türü `REG_SZ`, `REG_EXPAND_SZ` veya
/// `REG_MULTI_SZ` olmalıdır; diğer türler (örneğin `REG_DWORD`) başlangıç
/// girdisi değildir ve atlanır.
pub fn reg_query_coz(cikti: &str) -> Vec<(String, String)> {
    let mut ciftler = Vec::new();
    for satir in cikti.lines() {
        let kirp = satir.trim();
        if kirp.is_empty() {
            continue;
        }
        let parcalar: Vec<&str> = kirp.split_whitespace().collect();
        if parcalar.len() < 3 {
            // Anahtar yolu tek parça olarak yazılır; değer satırı en az üç parça
            // içerir (ad, tür, değer).
            continue;
        }
        // Anahtar yolu ters eğik çizgi içerir, değer adı içermez.
        if parcalar[0].contains('\\') {
            continue;
        }
        let tur = parcalar[1];
        if !matches!(tur, "REG_SZ" | "REG_EXPAND_SZ" | "REG_MULTI_SZ") {
            continue;
        }
        let ad = parcalar[0].to_string();
        // Değer, boşluk içerebileceğinden kalan metnin tamamıdır.
        let deger = kirp
            .split_once(tur)
            .map(|(_, kalan)| kalan.trim().to_string())
            .unwrap_or_default();
        if deger.is_empty() {
            continue;
        }
        ciftler.push((ad, deger));
    }
    ciftler
}

fn komut_calistir(program: &'static str, alt_komut: Option<&str>, args: &[&str]) -> Sonuc<String> {
    if !crate::kaynak::arac_salt_okunur_mu(program, alt_komut) {
        return Err(Hata::Komut {
            program,
            ayrinti: "salt okunur izin listesinde yok".into(),
        });
    }
    let mut komut = Command::new(program);
    if let Some(alt) = alt_komut {
        komut.arg(alt);
    }
    komut.args(args);
    let cikti = komut.output().map_err(|hata| Hata::Komut {
        program,
        ayrinti: hata.to_string(),
    })?;
    if !cikti.status.success() {
        return Err(Hata::Komut {
            program,
            ayrinti: format!("çıkış kodu {:?}", cikti.status.code()),
        });
    }
    String::from_utf8(cikti.stdout).map_err(|_| Hata::Ayristirma {
        program,
        ayrinti: "çıktı UTF-8 değil".into(),
    })
}

impl InventorySource for WindowsSource {
    fn ad(&self) -> &str {
        "windows"
    }

    fn surecler(&self) -> Sonuc<Vec<Surec>> {
        let cikti = komut_calistir("tasklist", None, &["/fo", "csv", "/nh"])?;
        let (satirlar, _) = tasklist::ayristir(&cikti);
        let mut kayitlar = tasklist::surece_cevir(&satirlar);

        if self.wmic_zenginlestir {
            if let Ok(cikti) = komut_calistir(
                "wmic",
                None,
                &[
                    "process",
                    "get",
                    "CommandLine,ExecutablePath,Name,ParentProcessId,ProcessId",
                    "/format:csv",
                ],
            ) {
                let (satirlar, _) = wmic::ayristir(&cikti);
                let indeks: HashMap<u32, &wmic::WmicSatiri> =
                    satirlar.iter().map(|s| (s.pid, s)).collect();
                for kayit in kayitlar.iter_mut() {
                    if let Some(satir) = indeks.get(&kayit.pid) {
                        *kayit = wmic::surece_cevir(kayit, satir);
                    }
                }
            }
        }
        Ok(kayitlar)
    }

    fn ag_baglantilari(&self) -> Sonuc<Vec<crate::model::Baglanti>> {
        let cikti = komut_calistir("netstat", None, &["-ano"])?;
        let (satirlar, _) = netstat::ayristir(&cikti);
        Ok(netstat::baglantiya_cevir(&satirlar))
    }

    fn baslangic_girdileri(&self) -> Sonuc<Vec<StartupGirdi>> {
        let mut girdiler: Vec<StartupGirdi> = Vec::new();

        for kok in self.kok_dizinleri() {
            for yol in dosya_haritasi(&kok, 3) {
                let tur = StartupTuru::yoldan(&yol);
                // Klasör veya desteklenmeyen uzantı → kayıt değil.
                if tur == StartupTuru::Bilinmiyor {
                    continue;
                }
                girdiler.push(StartupGirdi {
                    ad: yol
                        .file_name()
                        .and_then(|a| a.to_str())
                        .unwrap_or("bilinmiyor")
                        .to_string(),
                    tur,
                    etkin: true,
                    hedef: None,
                    konum: yol,
                });
            }
        }

        if self.reg_sorgula {
            let mut reg_var = false;
            for anahtar in [HKCU_RUN, HKLM_RUN] {
                if let Ok(cikti) = komut_calistir("reg", Some("query"), &[anahtar]) {
                    reg_var = true;
                    for (ad, deger) in reg_query_coz(&cikti) {
                        girdiler.push(StartupGirdi {
                            konum: PathBuf::from(anahtar),
                            ad,
                            tur: StartupTuru::RunAnahtari,
                            etkin: true,
                            hedef: Some(deger),
                        });
                    }
                }
            }
            if !reg_var && girdiler.is_empty() {
                return Err(Hata::Komut {
                    program: "reg",
                    ayrinti: "Run anahtarlarının hiçbiri okunamadı".into(),
                });
            }
        }

        girdiler.sort_by(|a, b| a.ad.cmp(&b.ad));
        Ok(girdiler)
    }

    fn kisit_notu(&self) -> String {
        self.kisit(self.wmic_zenginlestir, self.reg_sorgula)
    }
}

/// Standart Başlangıç klasörü yolunu tanır.
///
/// Girdi bir **dosya** olduğu için yol klasörle birebir eşleşmez; bu yüzden
/// "bu yol standart Başlangıç klasörünün içinde mi" sorusu sorulur. Modern
/// Windows'ta Başlangıç klasörü `%APPDATA%` altında yer alır; bu dizin muaf
/// tutulmazsa kural her girdiyi işaretlerdi.
pub fn standart_baslangic_klasoru_mi(yol: &Path) -> bool {
    let kucuk = yol
        .to_string_lossy()
        .to_ascii_lowercase()
        .replace('/', "\\");
    ["\\start menu\\programs\\startup", "\\.config\\autostart"]
        .iter()
        .any(|desen| kucuk.contains(&format!("{}\\", desen)) || kucuk.ends_with(desen))
}

#[cfg(test)]
// Gerekçe: unwrap/expect yalnızca test içinde kullanılır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const REG_CIKTI: &str = "\r\n\
HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Run\r\n\
    SecurityHealth    REG_SZ    %windir%\\system32\\SecurityHealthSystray.exe\r\n\
    UpdaterTask    REG_EXPAND_SZ    C:\\Users\\K\\AppData\\Roaming\\Updater\\upd.exe\r\n\
    AnchorCount    REG_DWORD    0x2\r\n\
    BosDeger    REG_SZ\r\n";

    #[test]
    fn reg_query_satirlari_cozulur() {
        let ciftler = reg_query_coz(REG_CIKTI);
        assert_eq!(ciftler.len(), 2, "REG_DWORD ve bos deger atlanmali");
        assert_eq!(ciftler[0].0, "SecurityHealth");
        assert!(ciftler[0].1.contains("SecurityHealthSystray.exe"));
    }

    #[test]
    fn reg_query_degerde_bosluk_korunur() {
        let cikti = "    X    REG_SZ    C:\\Program Files\\Ornek\\uygulama.exe\r\n";
        let ciftler = reg_query_coz(cikti);
        assert_eq!(ciftler[0].1, "C:\\Program Files\\Ornek\\uygulama.exe");
    }

    #[test]
    fn reg_query_bos_girdi_liste_donmez() {
        assert!(reg_query_coz("").is_empty());
        assert!(reg_query_coz("HKEY_CURRENT_USER\\...\\Run\r\n").is_empty());
    }

    #[test]
    fn standart_baslangic_klasoru_taninir() {
        assert!(standart_baslangic_klasoru_mi(Path::new(
            "C:\\Users\\K\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\Programs\\Startup"
        )));
        assert!(standart_baslangic_klasoru_mi(Path::new(
            "/home/k/.config/autostart"
        )));
        assert!(!standart_baslangic_klasoru_mi(Path::new(
            "C:\\Users\\K\\AppData\\Roaming\\Updater\\upd.exe"
        )));
    }

    #[test]
    fn kisit_notu_eksik_alanlari_durustce_soyler() {
        let kaynak = WindowsSource::yeni(false);
        let not = kaynak.kisit_notu();
        assert!(not.contains("ust surec"), "{}", not);
        assert!(not.contains("--reg"), "{}", not);
        let kaynak = WindowsSource::yeni(true).reg_ac();
        let not = kaynak.kisit_notu();
        assert!(!not.contains("wmic bulunamadi"), "{}", not);
    }

    #[test]
    fn izin_siz_program_calistirilmaz() {
        let sonuc = komut_calistir("taskkill", None, &["/f"]);
        assert!(sonuc.is_err());
    }
}
