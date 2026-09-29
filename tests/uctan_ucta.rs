//! Uçtan uca akış denemesi: gözlem → yaşam takibi → olay günlüğü → kural →
//! HTML/CSV rapor.
//!
//! Test **fixture kaynağı** üzerinden çalışır; hiçbir sistem komutu
//! çalıştırmaz, hiçbir canlı sürece dokunmaz ve yalnızca geçici dizine yazar.
//! Böylece test her platformda aynı sonucu verir.

mod yardimci;

use std::collections::HashMap;

use procsight::kaynak::fixture::FixtureSource;
use procsight::kurallar::{Kural, KuralBaglami, KuralEsikleri};
use procsight::model::{Surec, SurecDurumu};
use procsight::motor::{tur_olaylari, OlayTuru, YasamTakip};
use procsight::{AnlikGoruntu, Bulgu, Gunluk, YasamOzeti};

fn goruntu_al(surecler: Vec<Surec>) -> AnlikGoruntu {
    let kaynak = FixtureSource::with_records(surecler, Vec::new(), Vec::new());
    AnlikGoruntu::topla(&kaynak)
}

fn surec(pid: u32, ppid: u32, ad: &str) -> Surec {
    Surec {
        pid,
        ppid,
        ad: ad.to_string(),
        komut_satiri: format!("/usr/bin/{}", ad),
        yol: None,
        cpu_yuzde: None,
        bellek_kib: 4_096,
        oturum: Some(1),
        kullanici: Some("kullanici".into()),
        uid: Some(1000),
        durum: SurecDurumu::Calisiyor,
    }
}

#[test]
fn iki_turda_yeni_ve_kaybolan_surec_uretilir() {
    let mut takip = YasamTakip::yeni();

    let ilk = goruntu_al(vec![surec(1, 0, "init"), surec(2, 1, "a")]);
    let ozet1 = takip.guncelle(&ilk.surecler);
    assert_eq!(
        ozet1.yeni,
        Vec::<u32>::new(),
        "ilk turda 'yeni' bildirilmez"
    );

    let ikinci = goruntu_al(vec![surec(1, 0, "init"), surec(3, 1, "b")]);
    let onceki: HashMap<u32, Surec> = procsight::motor::surece_haritasi(&ilk);
    let ozet2 = takip.guncelle(&ikinci.surecler);
    assert_eq!(ozet2.yeni, vec![3]);
    assert_eq!(ozet2.kayboldu, vec![2]);
    assert_eq!(
        ozet2.kisa_omurlu,
        vec![2],
        "pid 2 yalnızca bir turda görüldü"
    );

    let olaylar = tur_olaylari(&ikinci, &ozet2, &onceki);
    let baslayanlar: Vec<&str> = olaylar
        .iter()
        .filter(|o| o.olay == OlayTuru::SurecBasladi)
        .map(|o| o.olay.etiket())
        .collect();
    assert_eq!(baslayanlar, vec!["surec_basladi"]);
    let bitenler = olaylar
        .iter()
        .find(|o| o.olay == OlayTuru::SurecBitti)
        .expect("bitiş olayı yok");
    assert_eq!(bitenler.pid, Some(2));
    assert!(bitenler.ayrinti.contains("a"));
}

#[test]
fn gunluk_yazilir_bozuk_satir_atlanir_ve_geri_okunur() {
    let gecici = yardimci::GeciciDizin::yeni("uctan-ucta-gunluk").expect("gecici dizin yok");
    let yol = gecici.birlestir("olaylar.jsonl");

    let mut takip = YasamTakip::yeni();
    let kaynak = FixtureSource::varsayilan();

    let mut toplam = 0usize;
    for _ in 0..3 {
        let goruntu = AnlikGoruntu::topla(&kaynak);
        let onceki = procsight::motor::surece_haritasi(&goruntu);
        let _ = onceki;
        let yasam = takip.guncelle(&goruntu.surecler);
        let olaylar = tur_olaylari(&goruntu, &yasam, &HashMap::new());
        let mut gunluk = Gunluk::ac(&yol).expect("günlük açılamadı");
        toplam += gunluk.ekle(&olaylar).expect("yazılamadı");
    }
    assert!(toplam > 0, "günlüğe en az bir olay yazılmalı");

    // Ortasına bozuk bir satır ekle: yarım kalmış günlük gibi davranmalı.
    let mevcut = std::fs::read_to_string(&yol).expect("okunamadi");
    let yarim = format!(
        "{}{}\n",
        mevcut, "{\"zaman\":1,\"olay\":\"surec_bitti\",\"pid\":"
    );
    std::fs::write(&yol, yarim).expect("bozuk satır yazılamadı");

    let (olaylar, ozet) = procsight::gunluk::oku(&yol).expect("günlük okunamadı");
    assert_eq!(olaylar.len(), toplam, "geçerli satırların hepsi okunmalı");
    assert_eq!(ozet.bozuk_satir, 1, "yarım satır sayılmalı");
    assert!(ozet.ozet().contains("bozuk 1"));
}

#[test]
fn gunlukten_kisa_omurlu_surecler_kural6yi_tetikler() {
    let gecici = yardimci::GeciciDizin::yeni("uctan-ucta-kural6").expect("gecici dizin yok");
    let yol = gecici.birlestir("olaylar.jsonl");

    let mut gunluk = Gunluk::ac(&yol).expect("günlük açılamadı");
    let olaylar: Vec<procsight::Olay> = (10..16)
        .flat_map(|pid| {
            [
                procsight::Olay::yeni(
                    1000,
                    OlayTuru::SurecBasladi,
                    Some(pid),
                    "fixture",
                    "basladi",
                ),
                procsight::Olay::yeni(1100, OlayTuru::SurecBitti, Some(pid), "fixture", "bitti"),
            ]
        })
        .collect();
    gunluk.ekle(&olaylar).expect("yazılamadı");

    let (okunan, _) = procsight::gunluk::oku(&yol).expect("okunamadı");
    let yasam = YasamOzeti::kisa_omurlu_ile(procsight::gunluk::kisa_omurlu_derle(&okunan));
    assert_eq!(yasam.kisa_omurlu.len(), 6);

    let bulgular = procsight::kurallar::degerlendir(
        Kural::KisaOmurluCokSayidaSurec,
        &KuralBaglami {
            surecler: &[],
            baglantilar: &[],
            baslangic: &[],
            yasam: &yasam,
        },
        &KuralEsikleri::default(),
    );
    assert_eq!(bulgular.len(), 1, "6 kısa ömürlü süreç eşiği aşıyor");
    assert!(bulgular[0].konu.contains("6"));
    assert!(!bulgular[0].sonraki_adim.is_empty());
}

#[test]
fn fixture_verisi_yedi_kuralin_tamamini_tetikleyebilir() {
    let kaynak = FixtureSource::varsayilan();
    let goruntu = AnlikGoruntu::topla(&kaynak);
    let yasam = YasamOzeti::kisa_omurlu_ile((200..206).collect());
    let bulgular = procsight::kurallar::tum_kurallar(
        &KuralBaglami {
            surecler: &goruntu.surecler,
            baglantilar: &goruntu.baglantilar,
            baslangic: &goruntu.baslangic,
            yasam: &yasam,
        },
        &KuralEsikleri::default(),
    );

    let tetiklenen: Vec<&str> = procsight::kurallar::TUM_KURALLAR
        .iter()
        .filter(|k| bulgular.iter().any(|b| b.kural == **k))
        .map(|k| k.ad())
        .collect();
    assert_eq!(
        tetiklenen.len(),
        7,
        "fixture yedi kuralın tümünü tetiklemeli; tetiklenenler: {:?}",
        tetiklenen
    );

    for bulgu in &bulgular {
        assert!(
            !bulgu.kanit.is_empty(),
            "{} kanıtsız bulgu",
            bulgu.kural.ad()
        );
        assert!(!bulgu.aciklama.is_empty());
        assert!(bulgu.sonraki_adim.len() > 20);
    }
}

#[test]
fn uctan_ucta_rapor_dosyalari_uretir() {
    let gecici = yardimci::GeciciDizin::yeni("uctan-ucta-rapor").expect("gecici dizin yok");

    let kaynak = FixtureSource::varsayilan();
    let goruntu = AnlikGoruntu::topla(&kaynak);
    let yasam = YasamOzeti::bos();
    let bulgular: Vec<Bulgu> = procsight::kurallar::tum_kurallar(
        &KuralBaglami {
            surecler: &goruntu.surecler,
            baglantilar: &goruntu.baglantilar,
            baslangic: &goruntu.baslangic,
            yasam: &yasam,
        },
        &KuralEsikleri::default(),
    );
    let olaylar = vec![procsight::Olay::yeni(
        42,
        OlayTuru::SurecBasladi,
        Some(9001),
        "fixture",
        "Xk3jdH9s7qW.exe basladi",
    )];

    let ust = procsight::rapor::RaporUstBilgi {
        zaman: 42,
        kaynak: goruntu.kaynak.clone(),
        baslik: "Uçtan uca deneme".into(),
        kisit_notu: String::new(),
    };
    let html = procsight::rapor::html_uret(&ust, &goruntu, &bulgular, &olaylar);
    let html_yolu = gecici.birlestir("rapor.html");
    procsight::rapor::html_yaz(&html_yolu, &html).expect("html yazılamadı");

    let csv = procsight::rapor::olaylar_csv(&olaylar);
    let csv_yolu = gecici.birlestir("olaylar.csv");
    procsight::rapor::csv_yaz(&csv_yolu, &csv).expect("csv yazılamadı");

    let okunan_html = std::fs::read_to_string(&html_yolu).expect("html okunamadı");
    assert_eq!(okunan_html, html);
    assert!(okunan_html.starts_with("<!DOCTYPE html>"));
    assert!(okunan_html.contains("Uçtan uca deneme"));
    assert!(okinan_bulgu_sayi(&okunan_html) > 0);

    let okunan_csv = std::fs::read_to_string(&csv_yolu).expect("csv okunamadı");
    assert!(okunan_csv.starts_with("zaman,olay,pid,kaynak,ayrinti"));
    assert!(okunan_csv.contains("surec_basladi"));
}

fn okinan_bulgu_sayi(html: &str) -> usize {
    html.matches("<td>").count()
}

#[test]
fn csv_formul_enjeksiyonu_surec_adindan_da_koru() {
    // Gecici dizin bu testte kullanılmaz; kuralın kendisi saf bir dönüşümdür.
    let zararli = surec(66, 1, "=cmd|' /C calc'!A1");
    let goruntu = goruntu_al(vec![zararli]);
    let csv = procsight::rapor::surecleri_csv(&goruntu);
    assert!(
        csv.contains("'=cmd|' /C calc'!A1"),
        "formül enjeksiyonu kaçırılmadı: {}",
        csv
    );
    // Alan `'` önekiyle başlamalı; hiçbir alan doğrudan `=` ile başlamamalı.
    let veri_satiri = csv.lines().nth(1).expect("veri satırı yok");
    assert!(!veri_satiri.starts_with('='), "{}", veri_satiri);
    assert!(veri_satiri.contains(",'="), "{}", veri_satiri);
}

#[test]
fn html_ozeti_yedi_kuralin_adini_yazmaz_ama_bulgu_sayisini_yazar() {
    let kaynak = FixtureSource::varsayilan();
    let goruntu = AnlikGoruntu::topla(&kaynak);
    let bulgular = procsight::kurallar::tum_kurallar(
        &KuralBaglami {
            surecler: &goruntu.surecler,
            baglantilar: &goruntu.baglantilar,
            baslangic: &goruntu.baslangic,
            yasam: &YasamOzeti::bos(),
        },
        &KuralEsikleri::default(),
    );
    let ust = procsight::rapor::RaporUstBilgi {
        zaman: 0,
        kaynak: "fixture".into(),
        baslik: "Kapsam denemesi".into(),
        kisit_notu: String::new(),
    };
    let html = procsight::rapor::html_uret(&ust, &goruntu, &bulgular, &[]);
    assert!(html.contains(&format!("Kural Bulguları ({})", bulgular.len())));
    assert!(html.contains("Süreçler (8)"));
    assert!(html.contains("Ağ Bağlantıları (7)"));
    assert!(html.contains("Başlangıç Girdileri (4)"));
}
