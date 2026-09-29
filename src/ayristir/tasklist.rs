//! `tasklist /fo csv` çıktısının ayrıştırıcısı.
//!
//! Windows'un kendi aracı olan `tasklist` kullanılır; harici bağımlılık
//! değildir. Çıktı yerelleştirilmiş olabilir, bu yüzden ayrıştırıcı **sütun
//! başlıklarına değil, alan sayısına ve tipine** bakar.
//!
//! Beklenen sütunlar: `Image Name`, `PID`, `Session Name`, `Session#`,
//! `Mem Usage`. `Mem Usage` alanı yerelleştirmeye göre `"1,234 K"`,
//! `"1.234 K"` veya `"8 K"` biçiminde olabilir; ayrıştırıcı yalnızca rakam
//! ve birim harfini okur.

use crate::ayristir::AyristirmaOzeti;
use crate::model::Surec;

/// `tasklist /fo csv` çıktısından okunan ham satır.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TasklistSatiri {
    /// `Image Name` sütunu.
    pub ad: String,
    /// `PID` sütunu.
    pub pid: u32,
    /// `Session Name` sütunu (`Services`, `Console` vb.).
    pub oturum_adi: String,
    /// `Session#` sütunu.
    pub oturum: u32,
    /// `Mem Usage` sütununun KiB cinsine çevrilmiş değeri.
    pub bellek_kib: u64,
}

/// CSV satırını sütunlara böler; tırnak içindeki ayraçları korur.
///
/// Windows'un `tasklist` çıktısında değerler her zaman çift tırnak içindedir
/// ve iç içe tırnak yoktur; yine de bozuk girdide güvenli davranmak için
/// tırnak kapalıysa ayıracı normal kabul ederiz.
pub fn csv_bol(satir: &str) -> Vec<String> {
    let mut alanlar: Vec<String> = Vec::new();
    let mut mevcut = String::new();
    let mut tirnakta = false;
    for karakter in satir.chars() {
        match karakter {
            '"' => tirnakta = !tirnakta,
            ',' if !tirnakta => {
                alanlar.push(mevcut.trim().to_string());
                mevcut = String::new();
            }
            diger => mevcut.push(diger),
        }
    }
    alanlar.push(mevcut.trim().to_string());
    alanlar
}

/// Bir `Mem Usage` alanını KiB cinsine çevirir.
///
/// `"8 K"` -> `8`, `"1,234 K"` -> `1234`, `"512 M"` -> `524288`,
/// `"2 G"` -> `2097152`. Tanınmayan biçim `0` döner.
pub fn bellek_kib_coz(alan: &str) -> u64 {
    let rakamlar: String = alan.chars().filter(|c| c.is_ascii_digit()).collect();
    if rakamlar.is_empty() {
        return 0;
    }
    let deger = match rakamlar.parse::<u64>() {
        Ok(d) => d,
        Err(_) => return 0,
    };
    let kucuk = alan.to_ascii_lowercase();
    if kucuk.contains('g') {
        deger.saturating_mul(1024 * 1024)
    } else if kucuk.contains('m') {
        deger.saturating_mul(1024)
    } else {
        deger
    }
}

/// `tasklist /fo csv` çıktısının tamamını ayrıştırır.
///
/// Başlık satırı, boş satırlar ve "bilgi" satırları atlanır ve
/// `ozet.bilgi` sayacına eklenir; en az beş sütunu olmayan satırlar bozuk
/// sayılır ve `ozet.atlanan` sayacına eklenir. Böylece hiçbir satır sessizce
/// yutulmaz.
///
/// **Başlık tespiti yerelleştirmeye bağlı değildir.** `PID` ve `Session#`
/// sütunları aynı anda sayısal değilse satır başlıktır (Türkçe
/// `"KİMLİK"` / `"Oturum#"` gibi); yalnızca `PID` sayısal değilse satır bozuk
/// veridir.
pub fn ayristir(cikti: &str) -> (Vec<TasklistSatiri>, AyristirmaOzeti) {
    let mut satirlar = Vec::new();
    let mut ozet = AyristirmaOzeti::yeni();

    for satir in cikti.lines() {
        let kirp = satir.trim();
        if kirp.is_empty() {
            continue;
        }
        // Yerelleştirilmiş bilgi satırı: `BILGI: ...` / `INFO: ...`
        if !kirp.starts_with('"') {
            ozet.bilgi_say();
            continue;
        }
        let alanlar = csv_bol(kirp);
        if alanlar.len() < 5 {
            ozet.atla();
            continue;
        }
        let pid = alanlar[1].parse::<u32>();
        let oturum = alanlar[3].parse::<u32>();
        let (pid, oturum) = match (pid, oturum) {
            (Ok(p), Ok(o)) => (p, o),
            (Err(_), Err(_)) => {
                ozet.bilgi_say();
                continue;
            }
            _ => {
                ozet.atla();
                continue;
            }
        };
        ozet.say();
        satirlar.push(TasklistSatiri {
            ad: alanlar[0].clone(),
            pid,
            oturum_adi: alanlar[2].clone(),
            oturum,
            bellek_kib: bellek_kib_coz(&alanlar[4]),
        });
    }
    (satirlar, ozet)
}

/// Ayrıştırılmış `tasklist` satırlarını platformdan bağımsız süreç
/// kayıtlarına çevirir.
pub fn surece_cevir(satirlar: &[TasklistSatiri]) -> Vec<Surec> {
    satirlar
        .iter()
        .map(|s| Surec {
            pid: s.pid,
            ppid: 0,
            ad: s.ad.clone(),
            komut_satiri: String::new(),
            yol: None,
            cpu_yuzde: None,
            bellek_kib: s.bellek_kib,
            oturum: Some(s.oturum),
            kullanici: None,
            uid: None,
            durum: crate::model::SurecDurumu::Bilinmiyor,
        })
        .collect()
}

#[cfg(test)]
// Gerekçe: unwrap/expect yalnızca test içinde kullanılır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const ORNEK: &str = r#""Image Name","PID","Session Name","Session#","Mem Usage"
"System Idle Process","0","Services","0","8 K"
"explorer.exe","4321","Console","1","98,412 K"
"svchost.exe","1044","Services","0","21,300 K"
"#;

    #[test]
    fn baslikli_cikti_uc_surec_verir() {
        let (satirlar, ozet) = ayristir(ORNEK);
        assert_eq!(satirlar.len(), 3);
        assert_eq!(ozet.okunan, 3);
        assert_eq!(ozet.atlanan, 0);
        assert_eq!(ozet.bilgi, 1, "başlık bilgi sayacına girmeli");
        assert_eq!(satirlar[0].ad, "System Idle Process");
        assert_eq!(satirlar[1].pid, 4321);
        assert_eq!(satirlar[1].oturum, 1);
    }

    #[test]
    fn bellek_ayraci_yerellestirmeden_etkilenmez() {
        assert_eq!(bellek_kib_coz("8 K"), 8);
        assert_eq!(bellek_kib_coz("98,412 K"), 98412);
        assert_eq!(bellek_kib_coz("98.412 K"), 98412);
        assert_eq!(bellek_kib_coz("512 M"), 524_288);
        assert_eq!(bellek_kib_coz("2 G"), 2_097_152);
    }

    #[test]
    fn bellek_alani_bos_veya_bozuksa_sifir() {
        assert_eq!(bellek_kib_coz(""), 0);
        assert_eq!(bellek_kib_coz("Bilinmiyor"), 0);
        assert_eq!(bellek_kib_coz("K"), 0);
    }

    #[test]
    fn csv_bol_tirnak_ici_ayraci_korur() {
        let alanlar = csv_bol("\"a, b\",\"c\",\"d\",\"e\",\"f\"");
        assert_eq!(alanlar[0], "a, b");
        assert_eq!(alanlar.len(), 5);
    }

    #[test]
    fn basliksiz_cikti_da_cozulur() {
        let cikti = "\"chrome.exe\",\"9000\",\"Console\",\"2\",\"250,000 K\"\n";
        let (satirlar, ozet) = ayristir(cikti);
        assert_eq!(satirlar.len(), 1);
        assert_eq!(ozet.bilgi, 0);
        assert_eq!(satirlar[0].bellek_kib, 250_000);
    }

    #[test]
    fn turkce_yerellestirilmis_baslik_yine_cozulur() {
        let cikti = "\"Görüntü Adı\",\"KİMLİK\",\"Oturum Adı\",\"Oturum#\",\"Bellek Kullanımı\"\n\
                     \"notepad.exe\",\"555\",\"Konsol\",\"1\",\"12 K\"\n";
        let (satirlar, ozet) = ayristir(cikti);
        assert_eq!(satirlar.len(), 1, "yerelleştirilmiş başlık tanınmamalı");
        assert_eq!(ozet.bilgi, 1);
        assert_eq!(satirlar[0].ad, "notepad.exe");
    }

    #[test]
    fn eksik_sutunlu_satir_bozuk_sayilir() {
        let cikti = "\"a.exe\",\"1\"\n\"b.exe\",\"2\",\"Console\",\"1\",\"4 K\"\n";
        let (satirlar, ozet) = ayristir(cikti);
        assert_eq!(satirlar.len(), 1);
        assert_eq!(ozet.atlanan, 1);
    }

    #[test]
    fn pid_sayisal_degilse_satir_bozuk_sayilir() {
        let cikti = "\"a.exe\",\"pid-degil\",\"Console\",\"1\",\"4 K\"\n";
        let (_, ozet) = ayristir(cikti);
        assert_eq!(ozet.atlanan, 1);
        assert_eq!(ozet.okunan, 0);
    }

    #[test]
    fn bos_girdi_hicbir_kayit_uretmez() {
        let (satirlar, ozet) = ayristir("");
        assert!(satirlar.is_empty());
        assert_eq!(ozet.okunan, 0);
        assert_eq!(ozet.atlanan, 0);
    }

    #[test]
    fn bilgi_satiri_kayit_uretmez() {
        let cikti = "BILGI: No tasks are running which match the specified criteria.\n";
        let (satirlar, ozet) = ayristir(cikti);
        assert!(satirlar.is_empty());
        assert_eq!(ozet.bilgi, 1);
        assert_eq!(ozet.atlanan, 0);
    }

    #[test]
    fn cok_lu_sutunlu_varyant_ilk_bes_sutunu_kullanir() {
        // `tasklist /fo csv /v` çok daha fazla sütun üretir; ilk beş aynıdır.
        let cikti = "\"a.exe\",\"77\",\"Console\",\"1\",\"4 K\",\"Oturum Adı\",\"Durum\"\n";
        let (satirlar, ozet) = ayristir(cikti);
        assert_eq!(satirlar.len(), 1);
        assert_eq!(ozet.okunan, 1);
        assert_eq!(satirlar[0].pid, 77);
    }

    #[test]
    fn surece_cevirme_alanlari_tasir() {
        let (satirlar, _) = ayristir(ORNEK);
        let kayitlar = surece_cevir(&satirlar);
        assert_eq!(kayitlar.len(), 3);
        assert_eq!(kayitlar[0].pid, 0);
        assert_eq!(kayitlar[0].bellek_kib, 8);
        assert_eq!(kayitlar[0].oturum, Some(0));
        assert!(kayitlar[0].yol.is_none());
    }
}
