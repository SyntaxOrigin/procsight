//! Ayrıştırıcıların **fixture dosyalarıyla** entegrasyon denemesi.
//!
//! Bu dosyadaki testler gerçek `tasklist`/`netstat`/`wmic` komutlarını
//! **çalıştırmaz**. Ham çıktı `tests/fixtures/` altında statik olarak durur ve
//! ayrıştırıcılar yalnızca bu metinler üzerinde sınanır. Böylece:
//!
//! - testler her platformda ve her makinede aynı sonucu verir,
//! - testler yönetici yetkisi gerektirmez,
//! - ayrıştırıcıdaki bir değişiklik, o platformun aracı çalışmadan yakalanır.

mod yardimci;

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};

use procsight::ayristir::{netstat, procstat, tasklist, wmic, AyristirmaOzeti};
use procsight::kaynak::windows::reg_query_coz;
use procsight::kurallar::{
    degerlendir, gizli_ad_mi, kalicilik_suspicious_mi, Kural, KuralBaglami, KuralEsikleri,
};
use procsight::model::{BaglantiDurumu, Protokol, StartupGirdi, StartupTuru};
use procsight::motor::{Olay, OlayTuru};
use procsight::YasamOzeti;

use yardimci::fixture_oku;

#[test]
fn tasklist_fixture_yedi_kayit_uretir() {
    let (satirlar, ozet) = tasklist::ayristir(&fixture_oku("tasklist_fo_csv.txt"));
    assert_eq!(satirlar.len(), 7, "{}", ozet.ozet());
    assert_eq!(ozet.atlanan, 0);
    assert_eq!(ozet.bilgi, 1, "başlık satırı bilgi sayacına girmeli");

    let sistem = &satirlar[0];
    assert_eq!(sistem.ad, "System Idle Process");
    assert_eq!(sistem.pid, 0);
    assert_eq!(sistem.bellek_kib, 8);

    let chrome = satirlar.iter().find(|s| s.pid == 9001).expect("chrome yok");
    assert_eq!(chrome.ad, "Chrome.exe");
    assert_eq!(chrome.bellek_kib, 1_572_864);
    assert_eq!(chrome.oturum, 1);
    assert_eq!(chrome.oturum_adi, "Console");
}

#[test]
fn tasklist_fixture_turkce_adi_dogru_kirpar() {
    let (satirlar, _) = tasklist::ayristir(&fixture_oku("tasklist_fo_csv.txt"));
    let turkce = satirlar
        .iter()
        .find(|s| s.pid == 9012)
        .expect("turkçe ad yok");
    assert_eq!(turkce.ad, "Ayarlar yardimcisi");
    assert_eq!(turkce.oturum, 1);
}

#[test]
fn netstat_fixture_dokuz_kayit_uretir() {
    let (satirlar, ozet) = netstat::ayristir(&fixture_oku("netstat_ano.txt"));
    assert_eq!(satirlar.len(), 9, "{}", ozet.ozet());
    assert_eq!(ozet.atlanan, 0);
    assert_eq!(ozet.bilgi, 2, "iki başlık satırı bilgi sayacına girmeli");

    let tcp: Vec<_> = satirlar
        .iter()
        .filter(|s| s.protokol == Protokol::Tcp)
        .collect();
    let udp: Vec<_> = satirlar
        .iter()
        .filter(|s| s.protokol == Protokol::Udp)
        .collect();
    assert_eq!(tcp.len(), 7);
    assert_eq!(udp.len(), 2);
}

#[test]
fn netstat_fixture_dinleyen_ve_disi_cikan_ayrilir() {
    let (satirlar, _) = netstat::ayristir(&fixture_oku("netstat_ano.txt"));
    let baglantilar = netstat::baglantiya_cevir(&satirlar);

    let dinleyen: Vec<_> = baglantilar.iter().filter(|b| b.dinleyen_mi()).collect();
    assert_eq!(dinleyen.len(), 3, "135, 445 ve [::]:445 dinleyen olmalı");
    for b in &dinleyen {
        assert!(!b.disari_cikan_mi(), "dinleyen soket dışarı çıkan sayılmaz");
    }

    let disari: Vec<_> = baglantilar.iter().filter(|b| b.disari_cikan_mi()).collect();
    // 52.109.8.20:443 ve 45.83.220.17:8443 küreseldir; özel ağ ve RFC 5737
    // belgeleme bloğu küresel sayılmaz.
    assert_eq!(disari.len(), 2);
    assert!(disari.iter().any(|b| b.uzak_port == Some(8443)));
    assert!(disari.iter().any(|b| b.uzak_port == Some(443)));
}

#[test]
fn netstat_fixture_belgeleme_ve_ozel_adres_kuresel_sayilmaz() {
    let (satirlar, _) = netstat::ayristir(&fixture_oku("netstat_ano.txt"));
    let baglantilar = netstat::baglantiya_cevir(&satirlar);

    let belgeleme = baglantilar
        .iter()
        .find(|b| b.uzak_port == Some(9001))
        .expect("belgeleme adresi satırı yok");
    assert_eq!(belgeleme.uzak_adres, Some("203.0.113.9".parse().unwrap()));
    assert!(
        !procsight::model::genel_adres_mi(belgeleme.uzak_adres.as_ref().unwrap()),
        "RFC 5737 belgeleme bloğu küresel sayılmaz"
    );

    let ozel = baglantilar
        .iter()
        .find(|b| b.uzak_port == Some(445) && b.yerel_port != 445)
        .expect("özel ağ satırı yok");
    assert_eq!(ozel.uzak_adres, Some("192.168.1.10".parse().unwrap()));
    assert!(
        !procsight::model::genel_adres_mi(ozel.uzak_adres.as_ref().unwrap()),
        "192.168/16 özel ağdır"
    );
}

#[test]
fn netstat_fixture_ipv6_kapsam_kimligi_cozulur() {
    let (satirlar, _) = netstat::ayristir(&fixture_oku("netstat_ano.txt"));
    let link = satirlar
        .iter()
        .find(|s| s.yerel_port == 546)
        .expect("IPv6 UDP satırı yok");
    assert_eq!(link.yerel_adres, "fe80::1".parse::<IpAddr>().unwrap());
    assert!(link.uzak_adres.is_none());
}

#[test]
fn proc_net_tcp_fixture_uc_gecerli_satir_uretir() {
    let (satirlar, ozet) = procstat::net_coz(&fixture_oku("proc_net_tcp.txt"), false);
    assert_eq!(satirlar.len(), 3, "{}", ozet.ozet());
    assert_eq!(
        ozet.atlanan, 2,
        "bozuk adres ve bozuk durum satırları sayılmalı"
    );
    assert_eq!(ozet.bilgi, 1, "başlık bilgi sayacına girmeli");

    let joker = &satirlar[0];
    assert_eq!(joker.yerel_adres, IpAddr::V4(Ipv4Addr::LOCALHOST));
    assert_eq!(joker.yerel_port, 8080);
    assert!(joker.uzak_adres.is_none(), "0.0.0.0:0 yer tutucudur");
    assert_eq!(joker.durum_kodu, 0x0A);
    assert_eq!(joker.inode, 12_345);
}

#[test]
fn proc_net_tcp_fixture_kucuk_bayt_sirasini_cevirir() {
    let (satirlar, _) = procstat::net_coz(&fixture_oku("proc_net_tcp.txt"), false);
    let dis = satirlar
        .iter()
        .find(|s| s.yerel_port == 0xC1E2)
        .expect("dış bağlantı satırı yok");
    assert_eq!(dis.uzak_adres, Some("52.109.8.20".parse().unwrap()));
    assert_eq!(dis.uzak_port, Some(443));
    assert_eq!(dis.uid, 1000);

    let ozel = satirlar
        .iter()
        .find(|s| s.yerel_port == 0xC1E3)
        .expect("özel ağ satırı yok");
    assert_eq!(ozel.uzak_adres, Some("10.0.0.10".parse().unwrap()));
    assert!(
        !procsight::model::genel_adres_mi(ozel.uzak_adres.as_ref().unwrap()),
        "10.0.0.0/8 özel ağdır"
    );
}

#[test]
fn proc_net_tcp_fixture_inode_ile_sahiplik_cozulur() {
    let (satirlar, _) = procstat::net_coz(&fixture_oku("proc_net_tcp.txt"), false);
    let mut eslesme = HashMap::new();
    eslesme.insert(23_456u64, 4_321u32);
    let baglantilar = procstat::baglantiya_cevir(&satirlar, Protokol::Tcp, &eslesme);

    let sahipli = baglantilar
        .iter()
        .find(|b| b.uzak_port == Some(443))
        .expect("443 bağlantısı yok");
    assert_eq!(sahipli.pid, Some(4_321));
    assert_eq!(sahipli.durum, BaglantiDurumu::Kuruldu);

    let sahipsiz = baglantilar
        .iter()
        .find(|b| b.durum == BaglantiDurumu::Dinliyor)
        .expect("dinleyen bağlantı yok");
    assert!(sahipsiz.pid.is_none(), "inode eşleşmediyse pid None kalır");
}

#[test]
fn proc_stat_fixture_dort_surec_ayristirir() {
    let icerik = fixture_oku("proc_pid_stat.txt");
    let satirlar: Vec<_> = icerik.lines().filter(|s| !s.trim().is_empty()).collect();
    assert_eq!(satirlar.len(), 4);

    let ilk = procstat::stat_coz(satirlar[0], 100).expect("systemd ayrıştırılamadı");
    assert_eq!(ilk.pid, 1);
    assert_eq!(ilk.komut, "systemd");
    assert_eq!(ilk.ppid, 0);
    assert_eq!(ilk.durum_harfi, 'S');
    assert_eq!(ilk.toplam_tik(), 129);
    assert_eq!(ilk.bellek_kib(), 12_400);

    let ikinci = procstat::stat_coz(satirlar[1], 100).expect("(sd-pam) ayrıştırılamadı");
    assert_eq!(ikinci.komut, "(sd-pam)");

    let ucuncu = procstat::stat_coz(satirlar[2], 100).expect("firefox ayrıştırılamadı");
    assert_eq!(ucuncu.durum(), procsight::model::SurecDurumu::Calisiyor);
    assert_eq!(ucuncu.bellek_kib(), 1_000_000);

    let dorduncu = procstat::stat_coz(satirlar[3], 100).expect("Xk3jdH9s7qW ayrıştırılamadı");
    assert_eq!(dorduncu.komut, "Xk3jdH9s7qW");
    assert_eq!(dorduncu.ppid, 4321);
    assert_eq!(
        dorduncu.bellek_kib(),
        512 * 1024,
        "RSS 131072 sayfa = 512 MiB"
    );
}

#[test]
fn proc_stat_fixture_bellek_esiginde_kural3_tetikler() {
    let icerik = fixture_oku("proc_pid_stat.txt");
    let stat = procstat::stat_coz(icerik.lines().nth(3).expect("4. satır yok"), 100)
        .expect("ayrıştırılamadı");
    let kayit = procstat::surece_cevir(&stat, "", Some(1000), None);

    let bulgular = degerlendir(
        Kural::YuksekBellek,
        &KuralBaglami {
            surecler: std::slice::from_ref(&kayit),
            baglantilar: &[],
            baslangic: &[],
            yasam: &YasamOzeti::bos(),
        },
        &KuralEsikleri::default(),
    );
    assert_eq!(bulgular.len(), 1, "512 MiB tam eşik tetiklenmeli");
    assert!(bulgular[0].kanit[0].contains("512 MiB"));
}

#[test]
fn wmic_fixture_ust_surec_ve_komut_satirini_tamamlar() {
    let (satirlar, ozet) = wmic::ayristir(&fixture_oku("wmic_process_csv.txt"));
    assert_eq!(satirlar.len(), 3, "{}", ozet.ozet());
    assert_eq!(
        ozet.atlanan, 1,
        "sayısal olmayan pid satırı bozuk sayılmalı"
    );
    assert_eq!(ozet.bilgi, 1);

    let firefox = satirlar
        .iter()
        .find(|s| s.pid == 4321)
        .expect("firefox yok");
    assert_eq!(firefox.ppid, 1);
    assert_eq!(
        firefox.komut_satiri,
        "C:\\Program Files\\Mozilla Firefox\\firefox.exe --new-window"
    );
    assert!(firefox.yol.is_some());

    let gizli = satirlar.iter().find(|s| s.pid == 9013).expect("gizli yok");
    assert_eq!(gizli.ppid, 4321);
    assert!(
        gizli_ad_mi(&gizli.ad).is_some(),
        "fixture kural 1 pozitif olmalı"
    );
}

#[test]
fn wmic_fixture_tasklist_kaydini_zenginlestirir() {
    let (wmic_satirlar, _) = wmic::ayristir(&fixture_oku("wmic_process_csv.txt"));
    let (tl_satirlar, _) = tasklist::ayristir(&fixture_oku("tasklist_fo_csv.txt"));

    let taban = tl_satirlar
        .iter()
        .find(|s| s.pid == 4321)
        .cloned()
        .expect("tasklist kaydı yok");
    let kayitlar = tasklist::surece_cevir(&[taban]);
    let mut kayit = kayitlar.into_iter().next().expect("kayıt üretilmedi");
    assert_eq!(kayit.ppid, 0, "tasklist üst süreç vermez");

    let zengin = wmic_satirlar
        .iter()
        .find(|s| s.pid == 4321)
        .expect("wmic kaydı yok");
    kayit = wmic::surece_cevir(&kayit, zengin);
    assert_eq!(kayit.ppid, 1);
    assert!(!kayit.komut_satiri.is_empty());
    assert_eq!(kayit.bellek_kib, 98_412, "bellek tasklist'ten korunmalı");
}

#[test]
fn reg_query_fixture_iki_girdi_uretir() {
    let ciftler = reg_query_coz(&fixture_oku("reg_query_run.txt"));
    assert_eq!(ciftler.len(), 2, "REG_DWORD ve boş değer atlanmalı");
    assert_eq!(ciftler[0].0, "SecurityHealth");
    assert_eq!(ciftler[1].0, "UpdaterTask");
    assert!(ciftler[1]
        .1
        .starts_with("C:\\Users\\KULLANICI\\AppData\\Roaming"));
}

#[test]
fn reg_query_girdileri_kural4_pozitif_ve_negatif_durur() {
    let ciftler = reg_query_coz(&fixture_oku("reg_query_run.txt"));
    let girdiler: Vec<StartupGirdi> = ciftler
        .iter()
        .map(|(ad, deger)| StartupGirdi {
            konum: "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run".into(),
            ad: ad.clone(),
            tur: StartupTuru::RunAnahtari,
            etkin: true,
            hedef: Some(deger.clone()),
        })
        .collect();

    let bulgular = degerlendir(
        Kural::KalicilikGirdisi,
        &KuralBaglami {
            surecler: &[],
            baglantilar: &[],
            baslangic: &girdiler,
            yasam: &YasamOzeti::bos(),
        },
        &KuralEsikleri::default(),
    );
    assert_eq!(bulgular.len(), 1, "yalnız UpdaterTask şüpheli");
    assert!(bulgular[0].konu.contains("UpdaterTask"));

    assert!(kalicilik_suspicious_mi(&girdiler[0]).is_none());
    assert!(kalicilik_suspicious_mi(&girdiler[1]).is_some());
}

#[test]
fn netstat_fixture_surec_kaydiyla_birlestirilir() {
    let (satirlar, _) = netstat::ayristir(&fixture_oku("netstat_ano.txt"));
    let baglantilar = netstat::baglantiya_cevir(&satirlar);
    let (surecler, _) = tasklist::ayristir(&fixture_oku("tasklist_fo_csv.txt"));
    let isimler: HashMap<u32, String> = surecler.iter().map(|s| (s.pid, s.ad.clone())).collect();

    let cozulen = baglantilar
        .iter()
        .filter(|b| b.pid.and_then(|p| isimler.get(&p)).is_some())
        .count();
    assert!(cozulen >= 6, "{} bağlantının sahibi çözüldü", cozulen);

    let bilinmeyen = baglantilar
        .iter()
        .filter(|b| b.pid.map(|p| !isimler.contains_key(&p)).unwrap_or(true))
        .count();
    assert_eq!(bilinmeyen, 1, "pid 1234 çözülemedi ve gizlenmedi");
}

#[test]
fn ozet_sayaclari_birlestirilebilir() {
    let (_, t) = tasklist::ayristir(&fixture_oku("tasklist_fo_csv.txt"));
    let (_, n) = netstat::ayristir(&fixture_oku("netstat_ano.txt"));
    let mut toplam = AyristirmaOzeti::yeni();
    toplam.birlestir(&t);
    toplam.birlestir(&n);
    assert_eq!(toplam.okunan, 7 + 9);
    assert_eq!(toplam.atlanan, 0);
    assert_eq!(toplam.bilgi, 1 + 2);
}

#[test]
fn olay_jsonl_sema_dort_zorunlu_alan_tasir() {
    let olay = Olay::yeni(
        1_700_000_000_000,
        OlayTuru::SurecBasladi,
        Some(9013),
        "fixture",
        "Xk3jdH9s7qW.exe basladi",
    );
    let json = serde_json::to_value(&olay).expect("serileştirilemedi");
    for alan in ["olay", "zaman", "pid", "ayrinti", "kaynak"] {
        assert!(json.get(alan).is_some(), "şemada {} alanı eksik", alan);
    }
    assert_eq!(json["olay"], "surec_basladi");
    let geri: Olay = serde_json::from_value(json).expect("çözümlenemedi");
    assert_eq!(geri, olay);
}

#[test]
fn ps_ayristirici_fixture_satiri_aynistirir() {
    // macOS/BSD çıktısı Windows fixture'ıyla aynı kayıt şeklini üretmelidir.
    let cikti = "  PID  PPID USER     %CPU RSS    ST COMMAND\n\
                 \x20 4321 1044 xXx        1.2 102400  S    /usr/bin/firefox --new-window\n";
    let (satirlar, ozet) = procsight::kaynak::ps::ayristir(cikti);
    assert_eq!(satirlar.len(), 1, "{}", ozet.ozet());
    assert_eq!(ozet.bilgi, 1);
    let kayit = satirlar[0].surece_cevir();
    assert_eq!(kayit.pid, 4321);
    assert_eq!(kayit.ppid, 1044);
    assert_eq!(kayit.ad, "firefox");
    assert_eq!(kayit.bellek_kib, 102_400);
    assert_eq!(kayit.komut_satiri, "/usr/bin/firefox --new-window");
}
