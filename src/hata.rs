//! Hata tipi ve sonuç takma adı.
//!
//! Bu modül yalnızca hata taşır: hiçbir iş yürütmez, hiçbir dosyaya yazmaz,
//! hiçbir komut başlatmaz. Okuma hataları, ayrıştırma hataları ve komut satırı
//! hataları burada toplanır; çağıran taraf `Display` çıktısını kullanıcıya
//! doğrudan gösterir.
//!
//! **Not:** Hiçbir hata çeşidi bir süreci sonlandırmayı, bir dosyayı silmeyi
//! veya kayıt defterini değiştirmeyi önermez. Araç salt okunurdur.

use std::fmt;
use std::io;
use std::path::PathBuf;

/// Tüm genel işlemlerin sonuç tipi.
pub type Sonuc<T> = Result<T, Hata>;

/// ProcSight'in ürettiği hataların tamamı.
///
/// `#[non_exhaustive]` ile işaretlidir: yeni bir hata çeşidi eklendiğinde
/// çağıran tarafın eşleşmesi derleme hatasına dönüşmez.
#[derive(Debug)]
#[non_exhaustive]
pub enum Hata {
    /// Salt okunur bir dosya okuması başarısız oldu.
    ///
    /// Bu hata hiçbir zaman yazma işlemi içermez; `eylem` alanı daima
    /// "oku", "oku ve ayrıştır" gibi okuma fiilleri taşır.
    Okuma {
        /// Yapılmak istenen okuma işlemi.
        eylem: &'static str,
        /// Okunan yol.
        yol: PathBuf,
        /// Altta yatan işletim sistemi hatası.
        kaynak: io::Error,
    },
    /// Salt okunur bir çıktı yazımı başarısız oldu (günlük, HTML, CSV).
    Yazma {
        /// Yazılan yol.
        yol: PathBuf,
        /// Altta yatan işletim sistemi hatası.
        kaynak: io::Error,
    },
    /// Salt okunur bir yardımcı komut başlatılamadı veya çalıştırılamadı.
    ///
    /// ProcSight yalnızca okuma yapan sistem araçlarını çalıştırır
    /// (`tasklist`, `netstat`, `ps`, `reg query`). Süreç sonlandıran hiçbir araç
    /// çağrılmaz.
    Komut {
        /// Çalıştırılmak istenen programın adı.
        program: &'static str,
        /// İşletim sisteminin döndürdüğü açıklama.
        ayrinti: String,
    },
    /// Komut çalıştı ama çıktısı beklenen biçimde ayrıştırılamadı.
    Ayristirma {
        /// Çıktısı çözümlenemeyen programın adı.
        program: &'static str,
        /// Ayrıntılı açıklama.
        ayrinti: String,
    },
    /// İstenen envanter kaynağı bu platformda yok.
    KaynakYok {
        /// Aranan kaynağın adı.
        ad: &'static str,
        /// Neden kullanılamadığı.
        ayrinti: String,
    },
    /// JSONL günlüğü okunamadı veya beklenen şemaya uymadı.
    GunlukBozuk {
        /// Günlük dosyasının yolu.
        dosya: PathBuf,
        /// Ayrıntılı açıklama.
        ayrinti: String,
    },
    /// Komut satırından gelen eksik veya tutarsız parametre.
    Parametre {
        /// Sorunlu parametrenin adı.
        ad: &'static str,
        /// Açıklama.
        ayrinti: String,
    },
}

impl Hata {
    /// Salt okunur dosya okuma hatasını yol ve eylem bağlamıyla sarar.
    pub fn oku(eylem: &'static str, yol: impl Into<PathBuf>, kaynak: io::Error) -> Self {
        Hata::Okuma {
            eylem,
            yol: yol.into(),
            kaynak,
        }
    }
}

impl fmt::Display for Hata {
    fn fmt(&self, bicik: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Hata::Okuma { eylem, yol, kaynak } => {
                write!(bicik, "{} başarısız ({}): {}", eylem, yol.display(), kaynak)
            }
            Hata::Yazma { yol, kaynak } => {
                write!(bicik, "yazma başarısız ({}): {}", yol.display(), kaynak)
            }
            Hata::Komut { program, ayrinti } => {
                write!(bicik, "{} çalıştırılamadı: {}", program, ayrinti)
            }
            Hata::Ayristirma { program, ayrinti } => {
                write!(bicik, "{} çıktısı çözümlenemedi: {}", program, ayrinti)
            }
            Hata::KaynakYok { ad, ayrinti } => {
                write!(bicik, "envanter kaynağı yok ({}): {}", ad, ayrinti)
            }
            Hata::GunlukBozuk { dosya, ayrinti } => {
                write!(
                    bicik,
                    "olay günlüğü bozuk ({}): {}",
                    dosya.display(),
                    ayrinti
                )
            }
            Hata::Parametre { ad, ayrinti } => {
                write!(bicik, "parametre hatası ({}): {}", ad, ayrinti)
            }
        }
    }
}

impl std::error::Error for Hata {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Hata::Okuma { kaynak, .. } | Hata::Yazma { kaynak, .. } => Some(kaynak),
            _ => None,
        }
    }
}

#[cfg(test)]
// Gerekçe: expect/unwrap yalnızca test içinde kullanılır ve testin
// başarısızlık mesajıdır. Üretim kodunda bu lintler açıktır
// (crate seviyesinde clippy::unwrap_used/clippy::expect_used).
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn okuma_hatasi_yol_ve_eylem_tasir() {
        let kaynak = io::Error::new(io::ErrorKind::NotFound, "yok");
        let h = Hata::oku("/proc/1/stat oku", "/proc/1/stat", kaynak);
        let metin = h.to_string();
        assert!(metin.contains("oku"), "{}", metin);
        assert!(metin.contains("/proc/1/stat"), "{}", metin);
    }

    #[test]
    fn komut_hatasi_program_adi_yazar() {
        let h = Hata::Komut {
            program: "tasklist",
            ayrinti: "bulunamadı".into(),
        };
        assert!(h.to_string().contains("tasklist"));
        assert!(h.to_string().contains("bulunamadı"));
    }

    #[test]
    fn kaynak_yok_hatasi_ad_ve_neden_yazar() {
        let h = Hata::KaynakYok {
            ad: "procfs",
            ayrinti: "bu platformda /proc yok".into(),
        };
        let metin = h.to_string();
        assert!(metin.contains("procfs"), "{}", metin);
        assert!(metin.contains("/proc yok"), "{}", metin);
    }

    #[test]
    fn io_hatasi_kaynak_olarak_ulasir() {
        let kaynak = io::Error::new(io::ErrorKind::PermissionDenied, "reddedildi");
        let h = Hata::Yazma {
            yol: PathBuf::from("gunluk.jsonl"),
            kaynak,
        };
        assert!(std::error::Error::source(&h).is_some());
    }
}
