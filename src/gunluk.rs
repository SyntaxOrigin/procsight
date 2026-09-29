//! JSONL olay günlüğü: yazma ve bozuk satır toleranslı okuma.
//!
//! ## Neden JSONL?
//!
//! Raporın § 07 kararı "sabit kayıt boyutu, bellek bütçesini ispatlanabilir
//! kılar; JSON ise insan tarafından okunabilir ve denetlenebilir" der. JSON
//! satır başına bağımsızdır; yarım kalan bir satır diğer satırları bozmaz.
//!
//! ## Şema
//!
//! Her satır tek bir JSON nesnesidir ve dört zorunlu alanı vardır
//! (`olay`, `zaman`, `pid`, `ayrinti`) — MANIFEST kartındaki tanımın birebir
//! karşılığı. Buna iki alan daha eklenmiştir: `olay` etiketi tip güvenliği
//! için enum, `kaynak` ise gözlemin hangi platformdan geldiğini belirtmek için.
//!
//! ## Bozuk satır politikası
//!
//! Yarım yazılmış veya bozuk bir satır **okunurken atlanır ve sayılır**; hiçbir
//! satır sessizce yutulmaz. `GunlukOzeti.bozuk_satir` sayacı rapora yazılır.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::hata::{Hata, Sonuc};
use crate::motor::Olay;

/// Bir JSONL günlüğünün okunmasından kalan sayısal özet.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GunlukOzeti {
    /// Şemaya uyan ve çözümlenen satır sayısı.
    pub okunan: usize,
    /// Bozuk veya yarım kalmış olduğu için atlanan satır sayısı.
    pub bozuk_satir: usize,
}

impl GunlukOzeti {
    /// "okunan N, bozuk M" biçiminde tek satırlık özet.
    pub fn ozet(&self) -> String {
        format!("okunan {}, bozuk {}", self.okunan, self.bozuk_satir)
    }
}

/// Eklenebilir bir JSONL günlüğü.
///
/// Yazıcı hiçbir zaman var olmayan bir dosyayı **silmez veya üzerine
/// yazmaz**: `append` modunda açılır, böylece iki gözlem oturumu aynı dosyayı
/// paylaşabilir. Dosya oluşturmak, günlüğün tek ve zorunlu yazma işlemidir;
/// bunun dışında araç hiçbir yere yazmaz.
pub struct Gunluk {
    dosya: File,
    yol: std::path::PathBuf,
    yazilan: usize,
}

impl Gunluk {
    /// Günlüğü ekleme modunda açar; dosya yoksa oluşturur.
    pub fn ac(yol: &Path) -> Sonuc<Self> {
        if let Some(ust) = yol.parent() {
            if !ust.as_os_str().is_empty() && !ust.exists() {
                std::fs::create_dir_all(ust).map_err(|kaynak| Hata::Yazma {
                    yol: ust.to_path_buf(),
                    kaynak,
                })?;
            }
        }
        let dosya = OpenOptions::new()
            .create(true)
            .append(true)
            .open(yol)
            .map_err(|kaynak| Hata::Yazma {
                yol: yol.to_path_buf(),
                kaynak,
            })?;
        Ok(Gunluk {
            dosya,
            yol: yol.to_path_buf(),
            yazilan: 0,
        })
    }

    /// Olay listesini günlüğe ekler ve yazılan satır sayısını döner.
    ///
    /// Olaylar tek tek yazılır; bir olayın serileştirilememesi diğerlerinin
    /// kaybolmasına yol açmaz — hata `Display` metnine dönüştürülür ve yazma
    /// durdurulur.
    pub fn ekle(&mut self, olaylar: &[Olay]) -> Sonuc<usize> {
        let mut yazilan = 0;
        for olay in olaylar {
            let satir = serde_json::to_string(olay).map_err(|hata| Hata::Ayristirma {
                program: "proc_serilestirme",
                ayrinti: hata.to_string(),
            })?;
            writeln!(self.dosya, "{}", satir).map_err(|kaynak| Hata::Yazma {
                yol: self.yol.clone(),
                kaynak,
            })?;
            yazilan += 1;
        }
        self.dosya.flush().map_err(|kaynak| Hata::Yazma {
            yol: self.yol.clone(),
            kaynak,
        })?;
        self.yazilan += yazilan;
        Ok(yazilan)
    }

    /// Şu ana kadar yazılan toplam satır sayısı.
    pub fn toplam(&self) -> usize {
        self.yazilan
    }

    /// Günlüğün yoludur.
    pub fn yol(&self) -> &Path {
        &self.yol
    }
}

/// Bir JSONL günlüğünü okur; bozuk satırlar atlanır ve sayılır.
pub fn oku(yol: &Path) -> Sonuc<(Vec<Olay>, GunlukOzeti)> {
    let dosya = File::open(yol).map_err(|kaynak| Hata::Okuma {
        eylem: "gunluk ac",
        yol: yol.to_path_buf(),
        kaynak,
    })?;
    let okuyucu = BufReader::new(dosya);
    let mut olaylar = Vec::new();
    let mut ozet = GunlukOzeti::default();

    for satir in okuyucu.lines() {
        let satir = match satir {
            Ok(s) => s,
            // Dosya ortasında bozulan UTF-8 veya yarım satır: atla ve say.
            Err(_) => {
                ozet.bozuk_satir += 1;
                continue;
            }
        };
        if satir.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Olay>(&satir) {
            Ok(olay) => {
                ozet.okunan += 1;
                olaylar.push(olay);
            }
            Err(_) => ozet.bozuk_satir += 1,
        }
    }
    Ok((olaylar, ozet))
}

/// Bir JSONL günlüğünden kısa ömürlü süreç kimliklerini yeniden üretir.
///
/// Kural 6 (`kisa omurlu cok sayida surec`) canlı yoklamadan gelen
/// `YasamOzeti`'ne bakar; `report` ve `rules --log` kipleri ise günlükten
/// çalışır. Bir kimlik için **başlangıç ve bitiş olayı** bulunursa ve bitiş
/// olayı, başlangıç olayından sonraki ilk gözlemdeyse kısa ömürlü sayılır.
pub fn kisa_omurlu_derle(olaylar: &[Olay]) -> Vec<u32> {
    let mut baslayan: Vec<(u32, u64)> = Vec::new();
    let mut bitti: Vec<(u32, u64)> = Vec::new();
    for olay in olaylar {
        let Some(pid) = olay.pid else {
            continue;
        };
        match olay.olay {
            crate::motor::OlayTuru::SurecBasladi => baslayan.push((pid, olay.zaman)),
            crate::motor::OlayTuru::SurecBitti => bitti.push((pid, olay.zaman)),
            _ => {}
        }
    }
    let mut kisa = Vec::new();
    for (pid, bitis) in &bitti {
        if let Some((_, baslangic)) = baslayan
            .iter()
            .filter(|(p, _)| p == pid)
            .min_by_key(|(_, z)| *z)
        {
            if bitis >= baslangic {
                kisa.push(*pid);
            }
        }
    }
    kisa.sort_unstable();
    kisa.dedup();
    kisa
}

#[cfg(test)]
// Gerekçe: unwrap/expect yalnızca test içinde kullanılır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::motor::OlayTuru;

    struct Gecici {
        yol: std::path::PathBuf,
    }

    impl Gecici {
        fn yeni(etiket: &str) -> Self {
            let yol = std::env::temp_dir().join(format!(
                "procsight-gunluk-{}-{}",
                etiket,
                std::process::id()
            ));
            let _ = std::fs::remove_file(&yol);
            Gecici { yol }
        }
    }

    impl Drop for Gecici {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.yol);
        }
    }

    fn olay(zaman: u64, tur: OlayTuru, pid: Option<u32>) -> Olay {
        Olay::yeni(zaman, tur, pid, "fixture", "aciklama")
    }

    #[test]
    fn yazip_okuma_gidis_donusu_dogru() {
        let gecici = Gecici::yeni("gidis-donus");
        let mut gunluk = Gunluk::ac(&gecici.yol).unwrap();
        gunluk
            .ekle(&[
                olay(100, OlayTuru::SurecBasladi, Some(1)),
                olay(200, OlayTuru::SurecBitti, Some(1)),
            ])
            .unwrap();
        assert_eq!(gunluk.toplam(), 2);

        let (olaylar, ozet) = oku(&gecici.yol).unwrap();
        assert_eq!(olaylar.len(), 2);
        assert_eq!(ozet.okunan, 2);
        assert_eq!(ozet.bozuk_satir, 0);
        assert_eq!(olaylar[0].olay, OlayTuru::SurecBasladi);
        assert_eq!(olaylar[1].pid, Some(1));
    }

    #[test]
    fn ekleme_modu_mevcut_icerigi_silmez() {
        let gecici = Gecici::yeni("ekleme-modu");
        std::fs::write(&gecici.yol, "{\"zaman\":1,\"olay\":\"surec_basladi\",\"pid\":9,\"kaynak\":\"x\",\"ayrinti\":\"onceki\"}\n").unwrap();
        let mut gunluk = Gunluk::ac(&gecici.yol).unwrap();
        gunluk
            .ekle(&[olay(2, OlayTuru::SurecBitti, Some(9))])
            .unwrap();
        let (olaylar, _) = oku(&gecici.yol).unwrap();
        assert_eq!(olaylar.len(), 2, "ilk oturumun kaydi korunmali");
    }

    #[test]
    fn bozuk_satir_atlanir_ve_sayilir() {
        let gecici = Gecici::yeni("bozuk");
        std::fs::write(
            &gecici.yol,
            "{\"zaman\":1,\"olay\":\"surec_basladi\",\"pid\":9,\"kaynak\":\"x\",\"ayrinti\":\"iyi\"}\n\
             bu bir json degil\n\
             {\"zaman\":2,\"olay\":\"surec_bitti\",\"pid\":9,\"kaynak\":\"x\",\"ayrinti\":\"iyi\"}\n",
        )
        .unwrap();
        let (olaylar, ozet) = oku(&gecici.yol).unwrap();
        assert_eq!(olaylar.len(), 2);
        assert_eq!(ozet.bozuk_satir, 1);
        assert!(ozet.ozet().contains("bozuk 1"));
    }

    #[test]
    fn eksik_alanli_satir_bozuk_sayilir() {
        let gecici = Gecici::yeni("eksik-alan");
        std::fs::write(&gecici.yol, "{\"zaman\":1,\"olay\":\"surec_basladi\"}\n").unwrap();
        let (olaylar, ozet) = oku(&gecici.yol).unwrap();
        assert!(olaylar.is_empty());
        assert_eq!(ozet.bozuk_satir, 1);
    }

    #[test]
    fn bos_satirlar_sayilmaz() {
        let gecici = Gecici::yeni("bos-satir");
        std::fs::write(&gecici.yol, "\n\n   \n").unwrap();
        let (olaylar, ozet) = oku(&gecici.yol).unwrap();
        assert!(olaylar.is_empty());
        assert_eq!(ozet.bozuk_satir, 0);
        assert_eq!(ozet.okunan, 0);
    }

    #[test]
    fn olmayan_dosya_hata_doner() {
        let sonuc = oku(Path::new("C:/procsight-yok-boyle-bir-gunluk.jsonl"));
        assert!(sonuc.is_err());
    }

    #[test]
    fn kisa_omurlu_surecler_gunlukten_derlenir() {
        let olaylar = vec![
            olay(100, OlayTuru::SurecBasladi, Some(1)),
            olay(150, OlayTuru::SurecBasladi, Some(2)),
            olay(200, OlayTuru::SurecBitti, Some(1)),
            olay(300, OlayTuru::SurecBitti, Some(2)),
        ];
        let kisa = kisa_omurlu_derle(&olaylar);
        assert_eq!(kisa, vec![1, 2]);
    }

    #[test]
    fn pid_siz_ve_baslangicsiz_olaylar_yoksayilir() {
        let olaylar = vec![
            Olay::yeni(1, OlayTuru::SurecBitti, None, "fixture", "kimliksiz bitis"),
            olay(200, OlayTuru::SurecBitti, Some(5)),
        ];
        assert!(kisa_omurlu_derle(&olaylar).is_empty());
    }

    #[test]
    fn tekrarli_bitis_satiri_tekilleştirilir() {
        let olaylar = vec![
            olay(100, OlayTuru::SurecBasladi, Some(3)),
            olay(200, OlayTuru::SurecBitti, Some(3)),
            olay(300, OlayTuru::SurecBitti, Some(3)),
        ];
        assert_eq!(kisa_omurlu_derle(&olaylar), vec![3]);
    }
}
