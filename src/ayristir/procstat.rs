//! procfs çıktısının ayrıştırıcıları.
//!
//! Bu modül Linux'a özgü dosyaların **metnini** saf fonksiyonlarla kayıt
//! tipine çevirir. Dosya okuma işi `kaynak::procfs` içindedir; burada yalnızca
//! ayrıştırma yapılır. Böylece Linux üzerinde çalışmadan da fixture dosyaları
//! ile test edilebilir.
//!
//! Kapsanan dosyalar:
//!
//! - `/proc/<pid>/stat` — süreç kimliği, durum, üst süreç, oturum, CPU tikleri,
//!   RSS.
//! - `/proc/<pid>/cmdline` — NUL ayrılmış komut satırı.
//! - `/proc/<pid>/status` — `Uid:` satırı (kullanıcı kimliği).
//! - `/proc/<pid>/exe` hedefi — çalıştırılabilir yol.
//! - `/proc/net/tcp`, `/proc/net/tcp6`, `/proc/net/udp` — bağlantı tablosu.

use std::net::IpAddr;

use crate::ayristir::AyristirmaOzeti;
use crate::model::{proc_ipv4_coz, proc_ipv6_coz, Baglanti, BaglantiDurumu, Protokol, SurecDurumu};

/// `/proc/<pid>/stat` içindeki sayısal alanlar (çekirdek sırası).
///
/// Alanlar çekirdeğin `task_struct` yazım sırasıyla birebir aynıdır; yeni
/// alan eklendiğinde bu sayı sabit kalmaz, bu yüzden yalnızca ihtiyaç duyulan
/// indeksler sabitlenmiştir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcStat {
    /// Süreç kimliği (1. alan).
    pub pid: u32,
    /// Komut adı; parantez içinde, boşluk içerebilir.
    pub komut: String,
    /// Durum harfi (3. alan).
    pub durum_harfi: char,
    /// Üst süreç kimliği (4. alan).
    pub ppid: u32,
    /// Oturum kimliği (6. alan).
    pub oturum: u32,
    /// `utime` tik sayısı (14. alan).
    pub utime: u64,
    /// `stime` tik sayısı (15. alan).
    pub stime: u64,
    /// Başlangıç zamanı, açılış anından itibaren tik (22. alan).
    pub baslangic_tik: u64,
    /// RSS sayfa sayısı (24. alan).
    pub rss_sayfa: u64,
    /// Çekirdek `CLK_TCK` değeri; CPU yüzdesini hesaplamak için gerekir.
    pub clk_tck: u64,
}

impl ProcStat {
    /// Sürecin çalışma durumunu çözer.
    pub fn durum(&self) -> SurecDurumu {
        SurecDurumu::proc_harfinden(self.durum_harfi)
    }

    /// Toplam CPU tüketimini tik cinsine çevirir.
    pub fn toplam_tik(&self) -> u64 {
        self.utime.saturating_add(self.stime)
    }

    /// RSS değerini KiB cinsine çevirir (Linux sayfa boyutu 4 KiB).
    pub fn bellek_kib(&self) -> u64 {
        self.rss_sayfa.saturating_mul(4)
    }
}

/// `/proc/<pid>/stat` metnini ayrıştırır.
///
/// Çekirdek komut alanını parantez içine alır ve bu alan **boşluk ve
/// parantez içerebilir**; bu yüzden ayrıştırma son `)` karakterinden sonra
/// başlar. Kutu adları boşluk içerdiği için `comm` alanı güvenilir biçimde
/// ancak bu yöntemle ayrılabilir.
pub fn stat_coz(metin: &str, clk_tck: u64) -> Option<ProcStat> {
    let acilis = metin.find('(')?;
    let kapanis = metin.rfind(')')?;
    if kapanis < acilis + 2 {
        return None;
    }
    let komut = metin[acilis + 1..kapanis].to_string();
    let pid: u32 = metin[..acilis].trim().parse().ok()?;
    let kalan: Vec<&str> = metin[kapanis + 1..].split_whitespace().collect();
    // Kalan alanlar 3. alandan (durum) başlar: dizin 0 = durum.
    if kalan.len() <= 21 {
        return None;
    }
    let durum_harfi = kalan[0].chars().next()?;
    let ppid = kalan[1].parse().ok()?;
    let oturum = kalan[3].parse().ok()?;
    let utime = kalan[11].parse().ok()?;
    let stime = kalan[12].parse().ok()?;
    let baslangic_tik = kalan[19].parse().ok()?;
    let rss_sayfa = kalan[21].parse().ok()?;
    Some(ProcStat {
        pid,
        komut,
        durum_harfi,
        ppid,
        oturum,
        utime,
        stime,
        baslangic_tik,
        rss_sayfa,
        clk_tck,
    })
}

/// `/proc/<pid>/cmdline` bayt dizisini komut satırına çevirir.
///
/// Linux komut satırını NUL ile ayırır ve sonuna da NUL koyar; boş içerik
/// "çekirdek iş parçacığı, komut satırı yok" anlamına gelir.
pub fn cmdline_coz(baytlar: &[u8]) -> String {
    let parcalar: Vec<&[u8]> = baytlar
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .collect();
    parcalar
        .iter()
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .collect::<Vec<String>>()
        .join(" ")
}

/// `/proc/<pid>/status` metninden `Uid:` satırındaki gerçek kullanıcı kimliğini
/// çözer. Satır yoksa `None` döner.
pub fn uid_coz(metin: &str) -> Option<u32> {
    for satir in metin.lines() {
        let kirp = satir.trim();
        if let Some(deger) = kirp.strip_prefix("Uid:") {
            return deger.split_whitespace().next()?.parse().ok();
        }
    }
    None
}

/// `/proc/<pid>/status` metninden `Name:` satırını çözer.
pub fn ad_coz(metin: &str) -> Option<String> {
    for satir in metin.lines() {
        let kirp = satir.trim();
        if let Some(deger) = kirp.strip_prefix("Name:") {
            return Some(deger.trim().to_string());
        }
    }
    None
}

/// `/proc/<pid>/stat` ve `/proc/<pid>/status` verisinden bir süreç kaydı üretir.
pub fn surece_cevir(
    stat: &ProcStat,
    cmdline: &str,
    uid: Option<u32>,
    yol: Option<std::path::PathBuf>,
) -> crate::model::Surec {
    crate::model::Surec {
        pid: stat.pid,
        ppid: stat.ppid,
        ad: stat.komut.clone(),
        komut_satiri: cmdline.to_string(),
        yol,
        cpu_yuzde: None,
        bellek_kib: stat.bellek_kib(),
        oturum: Some(stat.oturum),
        kullanici: None,
        uid,
        durum: stat.durum(),
    }
}

/// `/proc/net/tcp` benzeri bir dosyanın satır biçimi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcNetSatiri {
    /// Yerel uç adresi.
    pub yerel_adres: IpAddr,
    /// Yerel uç portu.
    pub yerel_port: u16,
    /// Uzak uç adresi; yer tutucu ise `None`.
    pub uzak_adres: Option<IpAddr>,
    /// Uzak uç portu; yer tutucu ise `None`.
    pub uzak_port: Option<u16>,
    /// Onaltılık durum kodu.
    pub durum_kodu: u8,
    /// Dosya tanıtıcısı sahibi kullanıcı kimliği.
    pub uid: u32,
    /// Soket inode numarası; süreç eşleme için kullanılır.
    pub inode: u64,
}

/// `/proc/net/tcp`, `/proc/net/tcp6`, `/proc/net/udp` dosyalarından birini
/// ayrıştırır.
///
/// Başlık satırı (`sl local_address rem_address st ...`) atlanır; sütun sayısı
/// yetersiz satırlar bozuk sayılır. IPv4 adresleri 8, IPv6 adresleri 32
/// onaltılık karakter taşır; hangi biçimin kullanılacağı `ipv6` bayrağıyla
/// belirlenir.
pub fn net_coz(metin: &str, ipv6: bool) -> (Vec<ProcNetSatiri>, AyristirmaOzeti) {
    let mut satirlar = Vec::new();
    let mut ozet = AyristirmaOzeti::yeni();

    for satir in metin.lines() {
        let parcalar: Vec<&str> = satir.split_whitespace().collect();
        // Başlık `sl local_address ...` ile başlar ve 4. sütunu `st`'dir.
        if parcalar.len() >= 4 && parcalar[3] == "st" {
            ozet.bilgi_say();
            continue;
        }
        if parcalar.len() < 4 {
            if satir.trim().is_empty() {
                continue;
            }
            ozet.atla();
            continue;
        }
        let yerel_adres = match adres_coz(parcalar[1], ipv6) {
            Some(a) => a,
            None => {
                ozet.atla();
                continue;
            }
        };
        let yerel_port = match port_coz(parcalar[1]) {
            Some(p) => p,
            None => {
                ozet.atla();
                continue;
            }
        };
        let uzak_adres = adres_coz(parcalar[2], ipv6);
        let uzak_port = port_coz(parcalar[2]);
        let durum_kodu = match u8::from_str_radix(parcalar[3], 16) {
            Ok(k) => k,
            Err(_) => {
                ozet.atla();
                continue;
            }
        };
        let uid = parcalar
            .get(7)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        let inode = parcalar
            .get(9)
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        ozet.say();
        satirlar.push(ProcNetSatiri {
            yerel_adres,
            yerel_port,
            uzak_adres,
            uzak_port,
            durum_kodu,
            uid,
            inode,
        });
    }
    (satirlar, ozet)
}

/// `0100007F:1F90` gibi `adres:port` hücresini iki parçaya böler.
fn ayir(hucre: &str) -> Option<(&str, &str)> {
    hucre.rsplit_once(':')
}

fn adres_coz(hucre: &str, ipv6: bool) -> Option<IpAddr> {
    let (adres, _) = ayir(hucre)?;
    let cozulen = if ipv6 {
        proc_ipv6_coz(adres)?
    } else {
        proc_ipv4_coz(adres)?
    };
    // `00000000` / `::` yer tutucusu "belirtilmemiş" demektir.
    if cozulen.is_unspecified() {
        return None;
    }
    Some(cozulen)
}

fn port_coz(hucre: &str) -> Option<u16> {
    let (_, port) = ayir(hucre)?;
    let deger = u16::from_str_radix(port, 16).ok()?;
    if deger == 0 {
        return None;
    }
    Some(deger)
}

/// Ayrıştırılmış `/proc/net/*` satırlarını bağlantı kayıtlarına çevirir.
///
/// `inode -> pid` eşlemesi verilirse sahiplik çözülür; verilmezse `pid`
/// alanı `None` kalır ve bu durum raporda açıkça yazılır.
pub fn baglantiya_cevir(
    satirlar: &[ProcNetSatiri],
    protokol: Protokol,
    inode_eslemesi: &std::collections::HashMap<u64, u32>,
) -> Vec<Baglanti> {
    satirlar
        .iter()
        .map(|s| Baglanti {
            protokol,
            yerel_adres: s.yerel_adres,
            yerel_port: s.yerel_port,
            uzak_adres: s.uzak_adres,
            uzak_port: s.uzak_port,
            durum: BaglantiDurumu::proc_kodundan(s.durum_kodu),
            pid: inode_eslemesi.get(&s.inode).copied(),
        })
        .collect()
}

#[cfg(test)]
// Gerekçe: unwrap/expect yalnızca test içinde kullanılır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    const STAT_ORNEK: &str = "1234 (systemd) S 1 1234 1234 0 -1 4194560 9262 0 0 0 \
36 21 0 0 20 0 8 0 1234567 4096 1588 18446744073709551615 \
4194368 4194032 140737488347987 140737488347888 140737488352371 1 1 0 0 0 0 0 0 17 2 0 0 0 0 0";

    #[test]
    fn stat_ayristirilir_ve_alanlar_tasinir() {
        let s = stat_coz(STAT_ORNEK, 100).expect("ayrıştırılamadı");
        assert_eq!(s.pid, 1234);
        assert_eq!(s.komut, "systemd");
        assert_eq!(s.ppid, 1);
        assert_eq!(s.durum(), SurecDurumu::Uyuyor);
        assert_eq!(s.toplam_tik(), 57);
        assert_eq!(s.baslangic_tik, 1234567);
    }

    #[test]
    fn stat_komut_alaninda_bosluk_ve_parantez_ayristirilir() {
        let metin = "99 ((sd-pam)) S 1 99 99 0 -1 0 0 0 0 0 1 2 0 0 20 0 1 0 500 0 0";
        let s = stat_coz(metin, 100).expect("ayrıştırılamadı");
        assert_eq!(s.komut, "(sd-pam)");
        assert_eq!(s.ppid, 1);
    }

    #[test]
    fn stat_eksik_alanlarla_basarisiz_olur() {
        assert!(stat_coz("1234 (kisa) S 1 1234", 100).is_none());
        assert!(stat_coz("", 100).is_none());
        assert!(stat_coz("abc (x) S 1", 100).is_none());
    }

    #[test]
    fn cmdline_nul_ayraclari_birlestirir() {
        let baytlar = b"/usr/bin/firefox\0--profile\0/tmp/a b\0";
        assert_eq!(cmdline_coz(baytlar), "/usr/bin/firefox --profile /tmp/a b");
    }

    #[test]
    fn cmdline_bos_icerik_bos_doner() {
        assert_eq!(cmdline_coz(b""), "");
        assert_eq!(cmdline_coz(b"\0\0"), "");
    }

    #[test]
    fn cmdline_gecersiz_utf8_kayip_isaretiyle_cozulur() {
        let baytlar: &[u8] = &[b'a', 0xFF, 0xFE, b'b'];
        let metin = cmdline_coz(baytlar);
        assert!(metin.starts_with('a'));
        assert!(metin.ends_with('b'));
    }

    #[test]
    fn status_uid_satiri_cozulur() {
        let status = "Name:\tfirefox\nUmask:\t0022\nUid:\t1000\t1000\t1000\t1000\nGid:\t1000\n";
        assert_eq!(uid_coz(status), Some(1000));
    }

    #[test]
    fn status_uid_satiri_yoksa_none() {
        assert_eq!(uid_coz("Name:\tkonuk\n"), None);
        assert_eq!(uid_coz(""), None);
    }

    #[test]
    fn status_ad_satiri_cozulur() {
        assert_eq!(ad_coz("Name:\tfirefox\nUid:\t0\n"), Some("firefox".into()));
        assert_eq!(ad_coz("Uid:\t0\n"), None);
    }

    const NET_TCP: &str = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n\
   0: 0100007F:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 12345 1 0000 100 0 0 10 0\n\
   1: 0100007F:C1E2 14086D34:01BB 01 00000000:00000000 00:00000000 00000000  1000        0 23456 1 0000 100 0 0 10 0\n";

    #[test]
    fn proc_net_tcp_basligi_atlanir_ve_satirlar_cozulur() {
        let (satirlar, ozet) = net_coz(NET_TCP, false);
        assert_eq!(satirlar.len(), 2);
        assert_eq!(ozet.okunan, 2);
        assert_eq!(ozet.bilgi, 1);
        assert_eq!(satirlar[0].yerel_adres, IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert_eq!(satirlar[0].yerel_port, 0x1F90);
        assert_eq!(satirlar[0].durum_kodu, 0x0A);
        assert!(satirlar[0].uzak_adres.is_none());
    }

    #[test]
    fn proc_net_tcp_uzak_adres_ters_byte_order_cozulur() {
        let (_, ozet) = net_coz(NET_TCP, false);
        let _ = ozet;
        let (satirlar, _) = net_coz(NET_TCP, false);
        // 14086D34 ters çevrilince 34.6D.08.14 = 52.109.8.20
        assert_eq!(
            satirlar[1].uzak_adres,
            Some(IpAddr::V4(Ipv4Addr::new(52, 109, 8, 20)))
        );
        assert_eq!(satirlar[1].uzak_port, Some(443));
        assert_eq!(satirlar[1].durum_kodu, 0x01);
        assert_eq!(satirlar[1].uid, 1000);
        assert_eq!(satirlar[1].inode, 23456);
    }

    #[test]
    fn proc_net_ipv6_dosyasi_cozulur() {
        let metin = "  sl  local_address                         remote_address                        st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n\
   0: 00000000000000000000000001000000:1F90 00000000000000000000000000000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 999 1 0000 100 0 0 10 0\n";
        let (satirlar, _ozet) = net_coz(metin, true);
        assert_eq!(satirlar.len(), 1);
        assert_eq!(satirlar[0].yerel_adres, IpAddr::V6(Ipv6Addr::LOCALHOST));
        assert_eq!(satirlar[0].yerel_port, 8080);
    }

    #[test]
    fn proc_net_bozuk_satirlar_bozuk_sayilir() {
        let metin = "0: ZZZZ:1F90 00000000:0000 0A 0 0 0 0 0 1000 0 1\n";
        let (satirlar, ozet) = net_coz(metin, false);
        assert!(satirlar.is_empty());
        assert_eq!(ozet.atlanan, 1);
    }

    #[test]
    fn proc_net_durum_kodu_onaltilik_degilse_bozuk() {
        let metin = "0: 0100007F:1F90 00000000:0000 ZZ 0 0 0 0 0 1000 0 1\n";
        let (_, ozet) = net_coz(metin, false);
        assert_eq!(ozet.atlanan, 1);
    }

    #[test]
    fn proc_net_bos_girdi_kayit_uretmez() {
        let (satirlar, ozet) = net_coz("", false);
        assert!(satirlar.is_empty());
        assert_eq!(ozet, AyristirmaOzeti::yeni());
    }

    #[test]
    fn inode_eslemesi_sahipligi_cozer() {
        let (satirlar, _) = net_coz(NET_TCP, false);
        let mut eslesme = std::collections::HashMap::new();
        eslesme.insert(23456u64, 4321u32);
        let baglantilar = baglantiya_cevir(&satirlar, Protokol::Tcp, &eslesme);
        assert_eq!(baglantilar[1].pid, Some(4321));
        assert!(baglantilar[1].disari_cikan_mi());
        assert!(baglantilar[0].dinleyen_mi());
    }

    #[test]
    fn inode_eslemesi_yoksa_pid_none_kalir() {
        let (satirlar, _) = net_coz(NET_TCP, false);
        let baglantilar =
            baglantiya_cevir(&satirlar, Protokol::Tcp, &std::collections::HashMap::new());
        assert!(baglantilar.iter().all(|b| b.pid.is_none()));
    }
}
