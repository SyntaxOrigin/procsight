//! `netstat -ano` çıktısının ayrıştırıcısı.
//!
//! Windows'un kendi aracı olan `netstat` kullanılır; harici bağımlılık
//! değildir ve hiçbir paket açmaz, yalnızca yerel bağlantı tablosunu basar.
//!
//! Beklenen biçim:
//!
//! ```text
//!   TCP    0.0.0.0:135       0.0.0.0:0       LISTENING      1044
//!   TCP    [::]:445          [::]:0          LISTENING      4
//!   UDP    0.0.0.0:500       *:*                             1234
//! ```
//!
//! Başlık satırları yerelleştirilmiştir ("Aktif Bağlantılar", "Proto",
//! "Yerel Adres", ...); bu yüzden ayrıştırıcı satırın `TCP`/`UDP` sözcüğüyle
//! başlayıp başlamadığına bakar.

use std::net::IpAddr;

use crate::ayristir::AyristirmaOzeti;
use crate::model::{Baglanti, BaglantiDurumu, Protokol};

/// Uç nokta ayrıştırılamadığında dönen hata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UcHatasi {
    /// Uç nokta metni yok.
    Bos,
    /// Adres veya port ayrımı bulunamadı.
    HataliBicim,
    /// Port sayısal değil veya 65535'i aşıyor.
    GecersizPort,
    /// Adres parantezli değil veya çözümlenemedi.
    GecersizAdres,
}

impl std::fmt::Display for UcHatasi {
    fn fmt(&self, bicik: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let metin = match self {
            UcHatasi::Bos => "uç nokta metni boş",
            UcHatasi::HataliBicim => "adres:port ayrımı bulunamadı",
            UcHatasi::GecersizPort => "port sayısal değil veya aralık dışı",
            UcHatasi::GecersizAdres => "adres çözümlenemedi",
        };
        bicik.write_str(metin)
    }
}

/// `adres:port` metnini ayrıştırır.
///
/// `*:*`, `0.0.0.0:0` ve `[::]:0` gibi yer tutucular `None` adres ve `None`
/// port ile döner. `0.0.0.0:135` ise **gerçek** bir dinleme ucudur ve
/// `(0.0.0.0, 135)` olarak döner. `[fe80::1%12]:546` gibi kapsam kimlikli IPv6
/// biçiminde kapsam ayrıştırılır.
pub fn uc_coz(metin: &str) -> Result<(Option<IpAddr>, Option<u16>), UcHatasi> {
    let kirp = metin.trim();
    if kirp.is_empty() {
        return Err(UcHatasi::Bos);
    }
    if kirp == "*:*" || kirp == "*" {
        return Ok((None, None));
    }
    let (adres_metni, port_metni) = if let Some(kapanis) = kirp.rfind(']') {
        // `[::]:445`
        let acilis = kirp.find('[').ok_or(UcHatasi::HataliBicim)?;
        let adres = kirp[acilis + 1..kapanis].trim();
        let port = kirp[kapanis + 1..]
            .trim()
            .strip_prefix(':')
            .ok_or(UcHatasi::HataliBicim)?;
        (adres, port)
    } else {
        kirp.rsplit_once(':').ok_or(UcHatasi::HataliBicim)?
    };

    // Port `0` bir yer tutucudur: karşı tarafta uç tanımlanmamıştır.
    if port_metni == "0" {
        return Ok((None, None));
    }
    // `[fe80::1%12]` — kapsam kimliği adresin parçası değildir.
    let adres_kismi = adres_metni.split('%').next().unwrap_or(adres_metni);
    let adres: IpAddr = adres_kismi
        .parse::<IpAddr>()
        .map_err(|_| UcHatasi::GecersizAdres)?;
    let port: u16 = port_metni.parse().map_err(|_| UcHatasi::GecersizPort)?;
    Ok((Some(adres), Some(port)))
}

/// Ayrıştırılmış bir `netstat` satırı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetstatSatiri {
    /// Taşıma katmanı protokolü.
    pub protokol: Protokol,
    /// Yerel uç adresi.
    pub yerel_adres: IpAddr,
    /// Yerel uç portu.
    pub yerel_port: u16,
    /// Uzak uç adresi; yer tutucu ise `None`.
    pub uzak_adres: Option<IpAddr>,
    /// Uzak uç portu; yer tutucu ise `None`.
    pub uzak_port: Option<u16>,
    /// Bağlantı durumu.
    pub durum: BaglantiDurumu,
    /// Sahip süreç kimliği; çözülemezse `None`.
    pub pid: Option<u32>,
}

/// `netstat -ano` çıktısının tamamını ayrıştırır.
///
/// Başlık ve boş satırlar `ozet.bilgi` sayacına, biçimsiz satırlar ise
/// `ozet.atlanan` sayacına yazılır.
pub fn ayristir(cikti: &str) -> (Vec<NetstatSatiri>, AyristirmaOzeti) {
    let mut satirlar = Vec::new();
    let mut ozet = AyristirmaOzeti::yeni();

    for satir in cikti.lines() {
        let kirp = satir.trim();
        if kirp.is_empty() {
            continue;
        }
        let ilk_sozcuk = kirp.split_whitespace().next().unwrap_or_default();
        let protokol = match ilk_sozcuk.to_ascii_uppercase().as_str() {
            "TCP" => Protokol::Tcp,
            "UDP" => Protokol::Udp,
            _ => {
                // "Aktif Bağlantılar", "Proto  Yerel Adres  Yabancı Adres  Durum  PID"
                ozet.bilgi_say();
                continue;
            }
        };

        let parcalar: Vec<&str> = kirp.split_whitespace().collect();
        // TCP: protokol, yerel, uzak, durum, pid  (bazı sürümlerde durum boş)
        // UDP: protokol, yerel, uzak yer tutucusu, pid
        let (yerel, uzak, durum, pid) = match protokol {
            Protokol::Tcp => {
                if parcalar.len() < 4 {
                    ozet.atla();
                    continue;
                }
                let pid = if parcalar.len() >= 5 {
                    parcalar[4].parse::<u32>().ok()
                } else {
                    None
                };
                (
                    parcalar[1],
                    parcalar[2],
                    BaglantiDurumu::netstat_sozcugunden(parcalar[3]),
                    pid,
                )
            }
            Protokol::Udp => {
                if parcalar.len() < 3 {
                    ozet.atla();
                    continue;
                }
                let pid = if parcalar.len() >= 4 {
                    parcalar[3].parse::<u32>().ok()
                } else {
                    None
                };
                (parcalar[1], parcalar[2], BaglantiDurumu::Bos, pid)
            }
        };

        let (yerel_adres, yerel_port) = match uc_coz(yerel) {
            Ok((Some(a), Some(p))) => (a, p),
            _ => {
                ozet.atla();
                continue;
            }
        };
        let (uzak_adres, uzak_port) = match uc_coz(uzak) {
            Ok(ikili) => ikili,
            Err(_) => {
                ozet.atla();
                continue;
            }
        };

        ozet.say();
        satirlar.push(NetstatSatiri {
            protokol,
            yerel_adres,
            yerel_port,
            uzak_adres,
            uzak_port,
            durum,
            pid,
        });
    }
    (satirlar, ozet)
}

/// Ayrıştırılmış `netstat` satırlarını platformdan bağımsız bağlantı
/// kayıtlarına çevirir.
pub fn baglantiya_cevir(satirlar: &[NetstatSatiri]) -> Vec<Baglanti> {
    satirlar
        .iter()
        .map(|s| Baglanti {
            protokol: s.protokol,
            yerel_adres: s.yerel_adres,
            yerel_port: s.yerel_port,
            uzak_adres: s.uzak_adres,
            uzak_port: s.uzak_port,
            durum: s.durum,
            pid: s.pid,
        })
        .collect()
}

/// Belirli bir sürece ait bağlantıları seçer.
pub fn surece_gore(baglantilar: &[Baglanti], pid: u32) -> Vec<&Baglanti> {
    baglantilar.iter().filter(|b| b.pid == Some(pid)).collect()
}

#[cfg(test)]
// Gerekçe: unwrap/expect yalnızca test içinde kullanılır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::net::Ipv6Addr;

    const TCP_ORNEK: &str = "Aktif Bağlantılar\r\n\
Proto  Yerel Adres            Yabancı Adres           Durum           PID\r\n\
TCP    0.0.0.0:135            0.0.0.0:0              LISTENING       1044\r\n\
TCP    127.0.0.1:52134        52.109.8.20:443        ESTABLISHED     9876\r\n\
TCP    [::]:445               [::]:0                 LISTENING       4\r\n";

    const UDP_ORNEK: &str =
        "UDP    0.0.0.0:500            *:*                                    1234\r\n\
UDP    [fe80::1%12]:546       *:*                                    77\r\n";

    #[test]
    fn turkce_basliklar_atlanir() {
        let (satirlar, ozet) = ayristir(TCP_ORNEK);
        assert_eq!(satirlar.len(), 3);
        assert_eq!(ozet.okunan, 3);
        assert_eq!(ozet.atlanan, 0);
        assert_eq!(ozet.bilgi, 2, "başlık satırları bilgi sayacına girmeli");
    }

    #[test]
    fn tcp_durum_sozcugu_cozulur() {
        let (satirlar, _) = ayristir(TCP_ORNEK);
        assert_eq!(satirlar[0].durum, BaglantiDurumu::Dinliyor);
        assert_eq!(satirlar[1].durum, BaglantiDurumu::Kuruldu);
        assert_eq!(satirlar[0].pid, Some(1044));
        assert_eq!(satirlar[1].uzak_port, Some(443));
    }

    #[test]
    fn ipv6_parantezli_bicim_cozulur() {
        let (satirlar, _) = ayristir(TCP_ORNEK);
        // `[::]:445` joker bir dinleyicidir: adres `::`, port 445.
        assert_eq!(satirlar[2].yerel_adres, IpAddr::V6(Ipv6Addr::UNSPECIFIED));
        assert_eq!(satirlar[2].yerel_port, 445);
        assert!(satirlar[2].uzak_adres.is_none());
    }

    #[test]
    fn kapsam_kimlikli_ipv6_cozulur() {
        let (satirlar, ozet) = ayristir(UDP_ORNEK);
        assert_eq!(ozet.okunan, 2);
        let beklenen: IpAddr = "fe80::1".parse().unwrap();
        assert_eq!(satirlar[1].yerel_adres, beklenen);
        assert_eq!(satirlar[1].yerel_port, 546);
    }

    #[test]
    fn udp_satiri_yok_soketi_durumu_uretir() {
        let (satirlar, ozet) = ayristir(UDP_ORNEK);
        assert_eq!(satirlar.len(), 2);
        assert_eq!(ozet.okunan, 2);
        assert_eq!(satirlar[0].protokol, Protokol::Udp);
        assert_eq!(satirlar[0].durum, BaglantiDurumu::Bos);
        assert_eq!(satirlar[0].pid, Some(1234));
        assert!(satirlar[0].uzak_adres.is_none());
    }

    #[test]
    fn uc_coz_yer_tutucuyu_none_doner() {
        assert_eq!(uc_coz("*:*"), Ok((None, None)));
        assert_eq!(uc_coz("0.0.0.0:0"), Ok((None, None)));
        assert_eq!(uc_coz("[::]:0"), Ok((None, None)));
    }

    #[test]
    fn uc_coz_bos_girdide_hata_doner() {
        assert_eq!(uc_coz(""), Err(UcHatasi::Bos));
    }

    #[test]
    fn uc_coz_ayrim_yoksa_hata_doner() {
        assert_eq!(uc_coz("adresdeport"), Err(UcHatasi::HataliBicim));
    }

    #[test]
    fn uc_coz_gecersiz_port_hata_doner() {
        assert_eq!(uc_coz("127.0.0.1:port"), Err(UcHatasi::GecersizPort));
        assert_eq!(uc_coz("127.0.0.1:70000"), Err(UcHatasi::GecersizPort));
    }

    #[test]
    fn uc_coz_hata_mesaji_okunur() {
        assert!(UcHatasi::GecersizAdres.to_string().contains("adres"));
    }

    #[test]
    fn eksik_sutunlu_tcp_satiri_bozuk_sayilir() {
        let (satirlar, ozet) = ayristir("TCP    0.0.0.0:135\r\n");
        assert!(satirlar.is_empty());
        assert_eq!(ozet.atlanan, 1);
    }

    #[test]
    fn bozuk_yerel_adres_satiri_bozuk_sayilir() {
        let (_, ozet) = ayristir("TCP    adres:135   0.0.0.0:0   LISTENING   4\r\n");
        assert_eq!(ozet.atlanan, 1);
    }

    #[test]
    fn bos_girdi_kayit_uretmez() {
        let (satirlar, ozet) = ayristir("");
        assert!(satirlar.is_empty());
        assert_eq!(ozet, AyristirmaOzeti::yeni());
    }

    #[test]
    fn baglantiya_cevirme_alanlari_tasir() {
        let (satirlar, _) = ayristir(TCP_ORNEK);
        let baglantilar = baglantiya_cevir(&satirlar);
        assert_eq!(baglantilar.len(), 3);
        assert!(baglantilar[0].dinleyen_mi());
        assert!(baglantilar[1].disari_cikan_mi());
        assert_eq!(
            baglantilar[1].tanim(),
            "TCP 127.0.0.1:52134 -> 52.109.8.20:443"
        );
    }

    #[test]
    fn surece_gore_baglanti_tasir() {
        let (satirlar, _) = ayristir(TCP_ORNEK);
        let baglantilar = baglantiya_cevir(&satirlar);
        let secilen = surece_gore(&baglantilar, 1044);
        assert_eq!(secilen.len(), 1);
        assert!(surece_gore(&baglantilar, 999).is_empty());
    }
}
