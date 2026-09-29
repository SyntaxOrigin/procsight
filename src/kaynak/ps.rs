//! macOS/BSD envanter kaynağı: `ps` ve `netstat` çıktısı.
//!
//! Raporda macOS için `sysctl`/`kqueue` önerilmişti. `sysctl` üzerinden süreç
//! tablosu okumak FFI gerektirir ve `forbid(unsafe_code)` ile bağdaşmaz;
//! `kqueue` ise gerçek zamanlı olay akışıdır ve bu sürümde ertelenmiştir.
//! `ps`/`netstat` çıktısını ayrıştırmak **hiçbir FFI olmadan** aynı veri
//! kümesini verir ve ayrıştırıcıları fixture ile test edilebilir.
//!
//! ## Biçim seçimi
//!
//! `ps -axo pid=,ppid=,user=,%cpu=,rss=,state=,command=` kullanılır. BSD ve
//! macOS `ps` bu biçimde **alan sayısına göre** ayrıştırılabilir; ancak
//! `command=` alanı boşluk içerdiği için **sondan sayma** (kaçıncı alandan
//! itibaren birleştirileceği) kullanılır.

use std::path::PathBuf;
use std::process::Command;

use crate::hata::{Hata, Sonuc};
use crate::kaynak::{dosya_haritasi, InventorySource};
use crate::model::{StartupGirdi, StartupTuru, Surec, SurecDurumu};

/// `ps -axo ...` çıktısından okunan ham satır.
#[derive(Debug, Clone, PartialEq)]
pub struct PsSatiri {
    /// Süreç kimliği.
    pub pid: u32,
    /// Üst süreç kimliği.
    pub ppid: u32,
    /// Sürecin çalıştıran kullanıcı adı.
    pub kullanici: String,
    /// Yüzde cinsinden CPU kullanımı.
    pub cpu_yuzde: f64,
    /// KiB cinsinden bellek kullanımı.
    pub bellek_kib: u64,
    /// Tek harflik durum kodu.
    pub durum_harfi: char,
    /// Komut satırı.
    pub komut_satiri: String,
}

impl PsSatiri {
    /// Bu satırdan platformdan bağımsız süreç kaydı üretir.
    pub fn surece_cevir(&self) -> Surec {
        Surec {
            pid: self.pid,
            ppid: self.ppid,
            ad: self
                .komut_satiri
                .split_whitespace()
                .next()
                .unwrap_or("bilinmiyor")
                .rsplit('/')
                .next()
                .unwrap_or("bilinmiyor")
                .to_string(),
            komut_satiri: self.komut_satiri.clone(),
            yol: None,
            cpu_yuzde: Some(self.cpu_yuzde),
            bellek_kib: self.bellek_kib,
            oturum: None,
            kullanici: Some(self.kullanici.clone()),
            uid: None,
            durum: SurecDurumu::proc_harfinden(self.durum_harfi),
        }
    }
}

/// `ps -axo pid=,ppid=,user=,%cpu=,rss=,state=,command=` çıktısını ayrıştırır.
///
/// Yedi alanlı biçim beklenir; altı alanlı (durum kodu olmayan) BSD varyantı
/// da kabul edilir ve durumu `?` ile doldurur.
pub fn ayristir(cikti: &str) -> (Vec<PsSatiri>, crate::ayristir::AyristirmaOzeti) {
    let mut satirlar = Vec::new();
    let mut ozet = crate::ayristir::AyristirmaOzeti::yeni();

    for satir in cikti.lines() {
        let kirp = satir.trim();
        if kirp.is_empty() {
            continue;
        }
        let parcalar: Vec<&str> = kirp.split_whitespace().collect();
        if parcalar.len() < 6 {
            ozet.atla();
            continue;
        }
        let pid = match parcalar[0].parse::<u32>() {
            Ok(p) => p,
            Err(_) => {
                // Başlık satırı (`PID PPID ...`) bilgi sayacına gider.
                if parcalar[0].eq_ignore_ascii_case("PID") {
                    ozet.bilgi_say();
                } else {
                    ozet.atla();
                }
                continue;
            }
        };
        let ppid = match parcalar[1].parse::<u32>() {
            Ok(p) => p,
            Err(_) => {
                ozet.atla();
                continue;
            }
        };
        let cpu_yuzde = match parcalar[3].trim_end_matches('%').parse::<f64>() {
            Ok(v) => v,
            Err(_) => {
                ozet.atla();
                continue;
            }
        };
        let bellek_kib = match parcalar[4].parse::<u64>() {
            Ok(v) => v,
            Err(_) => {
                ozet.atla();
                continue;
            }
        };
        let (durum_harfi, komut_satiri) = if parcalar.len() >= 7 {
            (
                parcalar[5].chars().next().unwrap_or('?'),
                parcalar[6..].join(" "),
            )
        } else {
            ('?', parcalar[5..].join(" "))
        };

        ozet.say();
        satirlar.push(PsSatiri {
            pid,
            ppid,
            kullanici: parcalar[2].to_string(),
            cpu_yuzde,
            bellek_kib,
            durum_harfi,
            komut_satiri,
        });
    }
    (satirlar, ozet)
}

/// `ps`/`netstat` çıktısından beslenen macOS/BSD envanter kaynağı.
#[derive(Debug, Clone, Default)]
pub struct PsSource {
    autostart_kok: Option<PathBuf>,
}

impl PsSource {
    /// Yeni kaynak üretir.
    pub fn yeni() -> Self {
        PsSource::default()
    }

    /// XDG autostart kökünü test için dışarıdan verir.
    pub fn autostart_kok(mut self, kok: PathBuf) -> Self {
        self.autostart_kok = Some(kok);
        self
    }
}

fn komut_calistir(program: &'static str, args: &[&str]) -> Sonuc<String> {
    if !crate::kaynak::arac_salt_okunur_mu(program, None) {
        return Err(Hata::Komut {
            program,
            ayrinti: "salt okunur izin listesinde yok".into(),
        });
    }
    let cikti = Command::new(program)
        .args(args)
        .output()
        .map_err(|hata| Hata::Komut {
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

impl InventorySource for PsSource {
    fn ad(&self) -> &str {
        "ps"
    }

    fn surecler(&self) -> Sonuc<Vec<Surec>> {
        let cikti = komut_calistir(
            "ps",
            &["-axo", "pid=,ppid=,user=,%cpu=,rss=,state=,command="],
        )?;
        let (satirlar, _) = ayristir(&cikti);
        Ok(satirlar.iter().map(PsSatiri::surece_cevir).collect())
    }

    fn ag_baglantilari(&self) -> Sonuc<Vec<crate::model::Baglanti>> {
        // macOS `netstat` çıktısı sütun sayısı işletim sistemi sürümüne göre
        // değişir. Bu sürümde bağlantı tablosu macOS üzerinde **kapsam dışı**
        // bırakılmıştır ve bu durum `kisit_notu` ile bildirilir; sessizce
        // boş liste dönmek yanıltıcı olurdu.
        Err(Hata::KaynakYok {
            ad: "ps/netstat",
            ayrinti: "macOS baglanti tablosu bu surumde kapsam disi".into(),
        })
    }

    fn baslangic_girdileri(&self) -> Sonuc<Vec<StartupGirdi>> {
        let kok = match self.autostart_kok.clone() {
            Some(k) => k,
            None => match std::env::var("HOME") {
                Ok(ev) => PathBuf::from(ev).join(".config").join("autostart"),
                Err(_) => return Ok(Vec::new()),
            },
        };
        Ok(dosya_haritasi(&kok, 2)
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
                let hedef = super::procfs::desktop_exec(&yol);
                StartupGirdi {
                    konum: yol,
                    ad,
                    tur: StartupTuru::XdgAutostart,
                    etkin: true,
                    hedef,
                }
            })
            .collect())
    }

    fn kisit_notu(&self) -> String {
        "macOS/BSD baglanti tablosu bu surumde kapsam disi; komut satiri ps'ten, autostart ~/.config/autostart'tan okunur".to_string()
    }
}

#[cfg(test)]
// Gerkçe: unwrap/expect yalnızca test içinde kullanılır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const ORNEK: &str = "\
  PID  PPID USER     %CPU RSS    ST COMMAND
    1     0 root       0.0  33160  Ss  /sbin/launchd
  432     1 xXx        1.2 102400  S    /Applications/Firefox.app/Contents/MacOS/firefox --new-window
  555   432 xXx        0.0   8192  S    /usr/bin/open -a TextEdit
";

    #[test]
    fn uc_satir_ayristirilir() {
        let (satirlar, ozet) = ayristir(ORNEK);
        assert_eq!(satirlar.len(), 3);
        assert_eq!(ozet.okunan, 3);
        assert_eq!(ozet.bilgi, 1, "başlık bilgi sayacına girmeli");
        assert_eq!(satirlar[0].pid, 1);
        assert_eq!(satirlar[1].ppid, 1);
        assert_eq!(satirlar[1].kullanici, "xXx");
        assert_eq!(satirlar[1].bellek_kib, 102_400);
    }

    #[test]
    fn komut_satiri_bosluklari_korur() {
        let (satirlar, _) = ayristir(ORNEK);
        assert_eq!(
            satirlar[1].komut_satiri,
            "/Applications/Firefox.app/Contents/MacOS/firefox --new-window"
        );
        assert_eq!(satirlar[1].durum_harfi, 'S');
        assert_eq!(satirlar[1].cpu_yuzde, 1.2);
    }

    #[test]
    fn surece_cevirme_ad_dizin_sonundan_alir() {
        let (satirlar, _) = ayristir(ORNEK);
        let kayit = satirlar[1].surece_cevir();
        assert_eq!(kayit.ad, "firefox");
        assert_eq!(kayit.cpu_yuzde, Some(1.2));
        assert_eq!(kayit.durum, SurecDurumu::Uyuyor);
        assert_eq!(kayit.kullanici, Some("xXx".into()));
    }

    #[test]
    fn altyi_sutunlu_bsd_varyanti_kabul_edilir() {
        let cikti = "  10  1 root 0.0 4096 /usr/bin/sshd\n";
        let (satirlar, _) = ayristir(cikti);
        assert_eq!(satirlar.len(), 1);
        assert_eq!(satirlar[0].durum_harfi, '?');
        assert_eq!(satirlar[0].komut_satiri, "/usr/bin/sshd");
    }

    #[test]
    fn bozuk_satirlar_bozuk_sayilir() {
        let cikti = "  10  x root 0.0 4096 S /usr/bin/sshd\n";
        let (_, ozet) = ayristir(cikti);
        assert_eq!(ozet.atlanan, 1);
    }

    #[test]
    fn bos_girdi_kayit_uretmez() {
        let (satirlar, ozet) = ayristir("");
        assert!(satirlar.is_empty());
        assert_eq!(ozet, crate::ayristir::AyristirmaOzeti::yeni());
    }

    #[test]
    fn macos_baglanti_tablosu_kapsam_disi_bildirir() {
        let kaynak = PsSource::yeni();
        assert!(kaynak.ag_baglantilari().is_err());
        assert!(kaynak.kisit_notu().contains("kapsam disi"));
    }

    #[test]
    fn izin_siz_program_calistirilmaz() {
        assert!(komut_calistir("kill", &["-9"]).is_err());
    }
}
