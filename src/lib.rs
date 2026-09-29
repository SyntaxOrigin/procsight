//! ProcSight — süreç, ağ bağlantısı ve başlangıç girdisi gözlemleyicisi.
//!
//! Araç, bir bilgisayarda "şu anda ne çalışıyor, nereye bağlanıyor, kim
//! başlangıçta kendiliğinden açılıyor" sorularını periyodik yoklama ile
//! toplayan **salt okunur** bir gözlem aracıdır.
//!
//! # Güvenlik modeli
//!
//! ProcSight hiçbir koşulda:
//!
//! - bir süreci sonlandırmaz, önceliğini değiştirmez, askıya almaz;
//! - bir dosyayı silmez, taşımaz veya değiştirmez;
//! - kayıt defterine yazmaz, kayıt anahtarı oluşturmaz;
//! - bir ağ bağlantısı kurmaz, paket göndermez, DNS çözümlemesi yapmaz.
//!
//! Yazdığı yerler yalnızca kullanıcının **açıkça verdiği** çıktı yollarıdır:
//! JSONL olay günlüğü, HTML raporu ve CSV dışa aktarımları. Hiçbir yazma
//! işlemi bir gözlenen nesneye (sürece, dosyaya, kayıt anahtarına) yönelmez.
//!
//! Modüller:
//!
//! - [`hata`]: hata tipi ve `Sonuc` takma adı.
//! - [`model`]: platformdan bağımsız veri modeli ve adres/konum yardımcıları.
//! - [`ayristir`]: `tasklist`, `wmic`, `netstat` ve `/proc` çıktı ayrıştırıcıları.
//! - [`kaynak`]: `InventorySource` soyutlaması ve platform kaynakları.
//! - [`motor`]: yoklama döngüsü, yaşam takibi ve olay üretimi.
//! - [`kurallar`]: yedi şüpheli davranış kuralı.
//! - [`gunluk`]: JSONL olay günlüğü yazımı ve okuması.
//! - [`rapor`]: HTML ve CSV dışa aktarımı.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![warn(clippy::unwrap_used, clippy::expect_used)]

pub mod ayristir;
pub mod gunluk;
pub mod hata;
pub mod kaynak;
pub mod kurallar;
pub mod model;
pub mod motor;
pub mod rapor;

pub use gunluk::{Gunluk, GunlukOzeti};
pub use hata::{Hata, Sonuc};
pub use kaynak::{AnlikGoruntu, InventorySource, KaynakTuru};
pub use kurallar::{Bulgu, Kural, KuralBaglami, KuralEsikleri};
pub use model::{Baglanti, StartupGirdi, Surec};
pub use motor::{Olay, OlayTuru, YasamOzeti, YasamTakip};

/// Aracın sürüm dizesi.
pub const SURUM: &str = env!("CARGO_PKG_VERSION");

/// Aracın adı; rapor başlıklarında ve günlükte kullanılır.
pub const ARAC_ADI: &str = "ProcSight";

/// Etik sınır bildirimi; her alt komutun çıktı başında gösterilir.
pub const ETIK_SINIR: &str = "ProcSight bir gozlem aracidir: hicbir sureci sonlandirmaz, \
hiçbir dosyayi silmez, kayit defterine yazmaz ve aga cikmaz. Bir bulgu sucluluk \
hukmu degil, kendin dogrulayacagin bir gozlemdir.";
