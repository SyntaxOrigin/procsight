//! Platform çıktısı ayrıştırıcıları.
//!
//! Bu alt modül, üç kaynaktan gelen **metin** çıktılarını saf fonksiyonlarla
//! kayıt tipine çevirir. Hiçbir ayrıştırıcı komut çalıştırmaz, dosya okumaz
//! veya ağa çıkmaz; hepsi yalnızca bir `&str` alır. Bu sayede Windows ve Linux
//! ayrıştırıcıları gerçek komut çalıştırmadan fixture dosyalarıyla test
//! edilebilir.
//!
//! - [`tasklist`]: `tasklist /fo csv` süreç listesi.
//! - [`wmic`]: `wmic process /format:csv` komut satırı ve üst süreç zenginleştirmesi.
//! - [`netstat`]: `netstat -ano` bağlantı tablosu.
//! - [`procstat`]: `/proc/<pid>/stat`, `/cmdline`, `/status` ve `/proc/net/tcp*`.

pub mod netstat;
pub mod procstat;
pub mod tasklist;
pub mod wmic;

use serde::{Deserialize, Serialize};

/// Bir ayrıştırma işleminin sayısal özeti.
///
/// Bozuk satır sayısı **gizlenmez**: rapor § 07'nin "asla sessizce yutma yok"
/// kuralı gereği sayacı ayrıca taşırız. Başlık ve bilgi satırları "bozuk"
/// sayılmaz; onlar için ayrı bir sayaç vardır.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AyristirmaOzeti {
    /// Başarıyla kayda çevrilen satır sayısı.
    pub okunan: usize,
    /// Biçim nedeniyle atlanan satır sayısı.
    pub atlanan: usize,
    /// Başlıktan, boş satırdan veya yerelleştirilmiş bilgi satırından gelen
    /// kayıt üretmeyen satır sayısı.
    pub bilgi: usize,
}

impl AyristirmaOzeti {
    /// Yeni, boş bir özet üretir.
    pub fn yeni() -> Self {
        Self::default()
    }

    /// Bir satırı başarıyla kayda çevirdiğinde sayacı artırır.
    pub fn say(&mut self) {
        self.okunan += 1;
    }

    /// Bir satırı biçim nedeniyle atladığında sayacı artırır.
    pub fn atla(&mut self) {
        self.atlanan += 1;
    }

    /// Kayıt üretmeyen bir başlık/bilgi satırı gördüğünde sayacı artırır.
    pub fn bilgi_say(&mut self) {
        self.bilgi += 1;
    }

    /// İki özeti toplar.
    pub fn birlestir(&mut self, diger: &AyristirmaOzeti) {
        self.okunan += diger.okunan;
        self.atlanan += diger.atlanan;
        self.bilgi += diger.bilgi;
    }

    /// "okunan N, atlanan M" biçiminde tek satırlık özet.
    pub fn ozet(&self) -> String {
        format!("okunan {}, atlanan {}", self.okunan, self.atlanan)
    }
}

#[cfg(test)]
// Gerekçe: expect yalnızca test içinde kullanılır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn ozet_sayaclari_birlestirir() {
        let mut a = AyristirmaOzeti::yeni();
        a.say();
        a.say();
        a.atla();
        let mut b = AyristirmaOzeti::yeni();
        b.say();
        b.bilgi_say();
        a.birlestir(&b);
        assert_eq!(a.okunan, 3);
        assert_eq!(a.atlanan, 1);
        assert_eq!(a.bilgi, 1);
        assert!(a.ozet().contains("okunan 3"));
    }

    #[test]
    fn bos_ozet_sifir_baslar() {
        let a = AyristirmaOzeti::yeni();
        assert_eq!(a, AyristirmaOzeti::default());
        assert_eq!(a.ozet(), "okunan 0, atlanan 0");
    }
}
