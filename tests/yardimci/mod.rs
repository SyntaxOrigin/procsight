//! Test içinde geçici dosya ve dizin üreten, `Drop` ile temizleyen yardımcı.
//!
//! ## Neden `tempfile` crate'i yok?
//!
//! `WORKER_CONTRACT.md` § 3.2-F `tempfile`'i hiçbir projeye vermez; bağımlılık
//! politikası bağımsız yardımcı yazmayı zorunlu kılar. Rastgelelik crate'i de
//! yasak olduğu için benzersizlik `std::process::id()` + etiket ile sağlanır.
//!
//! ## Temizlik hatası neden yutuluyor?
//!
//! `Drop` içinden hata döndürülemez. Temizleme başarısız olursa testi düşürmek
//! yerine sessizce geçilir; bu, sözleşmenin "sessiz yutma" yasağına istisnadır
//! ve README'de belgelenmiştir.
//!
//! ## Neden `dead_code` izni?
//!
//! Bu modül birden çok test ikili tarafından `mod yardimci;` ile eklenir ve
//! her ikili yalnızca bir bölümünü kullanır. Kullanılmayan yardımcılar
//! (örneğin `GeciciDizin`) diğer ikililerde "ölü kod" görünür; bu bir
//! eksiklik değil, ortak yardımcının doğal sonucudur.

// Gerekçe: ortak yardımcı modülü; her test ikilisi yalnızca bir bölümünü kullanır.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// Test içinde geçici dosya/dizin üreten, `Drop` ile temizleyen kapsayıcı.
pub struct GeciciDizin {
    yol: PathBuf,
}

impl GeciciDizin {
    /// `std::env::temp_dir()` altında, etiketten türetilmiş benzersiz bir dizin oluşturur.
    pub fn yeni(etiket: &str) -> std::io::Result<Self> {
        let kok =
            std::env::temp_dir().join(format!("procsight-test-{}-{}", etiket, std::process::id()));
        // Aynı testin iki kez çalışması olasıdır; eski içerik önce temizlenir.
        let _ = std::fs::remove_dir_all(&kok);
        std::fs::create_dir_all(&kok)?;
        Ok(Self { yol: kok })
    }

    /// Dizin içine göreli yol döndürür.
    pub fn yol(&self) -> &Path {
        &self.yol
    }

    /// Dizin içine göreli yol birleştirir.
    pub fn birlestir(&self, ad: &str) -> PathBuf {
        self.yol.join(ad)
    }
}

impl Drop for GeciciDizin {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.yol);
    }
}

/// Depodaki `tests/fixtures` dizinini döndürür.
///
/// Fixture dosyaları **derleme sırasında kopyalanmaz**; `CARGO_MANIFEST_DIR`
/// ile kaynak ağacına erişilir. Böylece dosya içeriği ile test beklentisi
/// her zaman aynı yerde durur ve sürükle-bırak derlemeye gerek kalmaz.
pub fn fixture_yolu(ad: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(ad)
}

/// Bir fixture dosyasını UTF-8 olarak okur.
pub fn fixture_oku(ad: &str) -> String {
    let yol = fixture_yolu(ad);
    std::fs::read_to_string(&yol).unwrap_or_else(|hata| {
        panic!("fixture okunamadi ({}): {}", yol.display(), hata);
    })
}

#[cfg(test)]
mod testler {
    use super::*;

    #[test]
    fn gecici_dizin_olusturulur_ve_temizlenir() {
        let yol = {
            let gecici = GeciciDizin::yeni("yardimci").expect("gecici dizin yok");
            let icerik = gecici.birlestir("a.txt");
            std::fs::write(&icerik, b"x").expect("yazilamadi");
            assert!(icerik.is_file());
            gecici.yol().to_path_buf()
        };
        assert!(!yol.exists(), "Drop sonrasi dizin kalmamali");
    }

    #[test]
    fn fixture_dosyalari_bulunur() {
        assert!(fixture_yolu("tasklist_fo_csv.txt").is_file());
        assert!(!fixture_oku("netstat_ano.txt").is_empty());
    }
}
