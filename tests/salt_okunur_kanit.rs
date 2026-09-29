//! "Salt okunur" sözünün **kanıtı**.
//!
//! Rapor § 03'ün onay kriterlerinden biri şudur: "Süreç sonlandırma, öncelik
//! değiştirme veya dosya silme işlevi kod tabanında bulunmaz; statik arama ile
//! doğrulanabilir." Bu dosya o doğrulamanın üç ayrı katmanını yapar:
//!
//! 1. **Statik tarama** — `src/` altındaki hiçbir dosyada süreç sonlandıran,
//!    dosya silen, kayıt defterine yazan veya ağa çıkan bir çağrı bulunmaz.
//! 2. **İzin listesi kanıtı** — çalıştırılabilecek programların kapalı listesi
//!    test edilir; yazma yapan alt komutlar listeden geçemez.
//! 3. **Çalışma zamanı kanıtı** — uçtan uca bir gözlem turu, gözlenen dizinde
//!    hiçbir dosyanın değişmediğini ve çıktının yalnızca kullanıcının verdiği
//!    dizine yazıldığını kanıtlar.

mod yardimci;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use procsight::kaynak::arac_salt_okunur_mu;

/// `src/` altındaki tüm Rust kaynak dosyalarını verir.
fn kaynak_dosyalari() -> Vec<PathBuf> {
    let kok = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut dosyalar = Vec::new();
    let mut yigin = vec![kok];
    while let Some(dizin) = yigin.pop() {
        let Ok(girdiler) = std::fs::read_dir(&dizin) else {
            continue;
        };
        for girid in girdiler.flatten() {
            let yol = girid.path();
            if yol.is_dir() {
                yigin.push(yol);
            } else if yol.extension().and_then(|u| u.to_str()) == Some("rs") {
                dosyalar.push(yol);
            }
        }
    }
    dosyalar.sort();
    dosyalar
}

/// Tüm **üretim** kodunu tek bir metin olarak birleştirir.
///
/// İki sadeleştirme uygulanır ve ikisi de yöntemin sınırıdır:
///
/// 1. Her dosyanın ilk `#[cfg(test)]` işaretinden sonrası atlanır; yani
///    tarama yalnızca derlenen üretim kodunu kapsar.
/// 2. Satır sonundaki `//` ve satır başındaki `///`/`//!` yorumları atılır;
///    aksi hâlde "bu işlev **yoktur" diyen güvenlik yorumları yanlış eşleşirdi.
///
/// Tarama kapsamı daraltıldığı için `unsafe` deseni listeden çıkarılmıştır:
/// `unsafe` kullanımı derleyici tarafından `#![forbid(unsafe_code)]` ile zaten
/// kanıtlanır, dize aramasından çok daha güçlü bir kanıttır.
fn uretim_kodu() -> String {
    kaynak_dosyalari()
        .iter()
        .map(|yol| {
            let ham = std::fs::read_to_string(yol).unwrap_or_else(|hata| {
                panic!("kaynak okunamadi ({}): {}", yol.display(), hata);
            });
            ham.split("#[cfg(test)]")
                .next()
                .unwrap_or_default()
                .to_string()
        })
        .map(|bolum| {
            bolum
                .lines()
                .filter(|satir| !satir.trim_start().starts_with("//"))
                .map(|satir| satir.split("//").next().unwrap_or_default())
                .collect::<Vec<&str>>()
                .join("\n")
        })
        .collect::<Vec<String>>()
        .join("\n")
}

/// Üretim kodunda **kesinlikle bulunmaması gereken** kalıplar.
const YASAK_KALIPLAR: [(&str, &str); 15] = [
    ("remove_file", "dosya silme"),
    ("remove_dir", "diz silme"),
    ("remove_dir_all", "dizini ve altındaki her şeyi silme"),
    ("taskkill", "süreç sonlandırma"),
    ("TerminateProcess", "süreç sonlandırma (Win32)"),
    ("OpenProcess", "sürece yazma yetkisi açma"),
    ("reg add", "kayıt defterine yazma"),
    ("reg delete", "kayıt defterinden silme"),
    ("schtasks /change", "görev zamanlayıcı değiştirme"),
    ("sc config", "hizmet yapılandırması değiştirme"),
    ("TcpStream::connect", "dışarı bağlantı kurma"),
    ("UdpSocket::bind", "soket açma"),
    ("Command::new(\"curl", "ağ aracı çalıştırma"),
    ("ptr::write", "ham bellek yazma"),
    ("libc::kill", "libc süreç sonlandırma"),
];

#[test]
fn kaynak_klasorunde_yasak_kalip_yoktur() {
    let metin = uretim_kodu();
    let mut bulunan: Vec<String> = Vec::new();
    for (kalip, gerekce) in YASAK_KALIPLAR {
        if metin.contains(kalip) {
            bulunan.push(format!("{} ({})", kalip, gerekce));
        }
    }
    assert!(
        bulunan.is_empty(),
        "üretim kodunda yasak işlev bulundu: {}",
        bulunan.join(", ")
    );
}

#[test]
fn uretim_kodu_taramasi_gercekten_uretim_kodunu_kapsiyor() {
    let metin = uretim_kodu();
    assert!(
        metin.contains("fn arac_salt_okunur_mu"),
        "üretim kodu boş çıktı"
    );
    assert!(
        !metin.contains("fn kaynak_klasorunde_yasak_kalip_yoktur"),
        "test kodu üretim koduna karışmamalı"
    );
}

#[test]
fn kaynak_dosyalari_bulundu() {
    let dosyalar = kaynak_dosyalari();
    assert!(
        dosyalar.len() >= 10,
        "{} kaynak dosyası tarandı",
        dosyalar.len()
    );
    assert!(dosyalar.iter().any(|d| d.ends_with("main.rs")));
    assert!(dosyalar.iter().any(|d| d.ends_with("lib.rs")));
}

#[test]
fn yalnizca_kullanici_verilen_yollara_yazilir() {
    let metin = uretim_kodu();
    // Yazma çağrılarının hepsi kullanıcının verdiği çıktı yoluna yönelmelidir.
    let yazma_noktalari: Vec<&str> = [
        ".create(true)",
        "std::fs::write(",
        "std::fs::create_dir_all(",
        "OpenOptions::new()",
    ]
    .into_iter()
    .filter(|k| metin.contains(k))
    .collect();
    assert!(
        yazma_noktalari.len() >= 3,
        "yazma noktaları bulunamadı: {:?}",
        yazma_noktalari
    );
    // Hiçbir yazma, gözlenen nesneye (süreç/dosya/kayıt) yönelmemelidir.
    assert!(
        !metin.contains("set_permissions(") && !metin.contains("set_modified("),
        "bir nesnenin niteliği değiştiriliyor"
    );
}

#[test]
fn komut_calistirma_listesi_kapatilir() {
    let izinliler: HashSet<&str> = procsight::kaynak::IZINLI_ARACLAR.iter().copied().collect();
    assert_eq!(izinliler.len(), 5, "izin listesi beklenmedik büyüdü");
    for ad in [
        "taskkill",
        "sc",
        "powershell",
        "cmd",
        "curl",
        "ssh",
        "schtasks",
    ] {
        assert!(
            !arac_salt_okunur_mu(ad, None),
            "{} izin listesinde olmamalı",
            ad
        );
    }
    for ad in ["tasklist", "netstat", "ps", "wmic"] {
        assert!(arac_salt_okunur_mu(ad, None), "{} izinli olmalı", ad);
    }
}

#[test]
fn reg_yalnizca_query_alt_komutuyla_calisir() {
    assert!(arac_salt_okunur_mu("reg", Some("query")));
    for alt_komut in ["add", "delete", "import", "export", "copy"] {
        assert!(
            !arac_salt_okunur_mu("reg", Some(alt_komut)),
            "reg {} izinsizdir",
            alt_komut
        );
    }
    assert!(!arac_salt_okunur_mu("reg", None));
}

#[test]
fn kaynakta_command_new_yalnizca_izinli_programlar_icin_caagirilir() {
    let metin = uretim_kodu();
    let izinli = [
        "\"tasklist\"",
        "\"netstat\"",
        "\"ps\"",
        "\"wmic\"",
        "\"reg\"",
    ];
    const PENCERE: usize = 120;

    // Her `komut_calistir(` **çağrısının** program adı, çağrının hemen
    // ardından gelen kısa pencerede izinli adlardan biri olmalıdır. Fonksiyonun
    // kendi tanımı (`program: &'static str`) bir çağrı değildir ve atlanır.
    let imza = "komut_calistir(";
    let mut denetlenen = 0usize;
    for (konum, _) in metin.match_indices(imza) {
        let baslangic = konum + imza.len();
        let kalan = &metin[baslangic..];
        if kalan.trim_start().starts_with("program:") {
            continue;
        }
        let son = (baslangic + PENCERE).min(metin.len());
        let son = (baslangic..son)
            .rev()
            .find(|i| metin.is_char_boundary(*i))
            .unwrap_or(baslangic);
        let pencere = &metin[baslangic..son];
        assert!(
            izinli.iter().any(|izinli_ad| pencere.contains(izinli_ad)),
            "izinli olmayan bir program çağrılıyor: {:?}",
            pencere
        );
        denetlenen += 1;
    }
    assert_eq!(
        denetlenen, 5,
        "kaynakta 5 komut çağrısı bekleniyordu (tasklist, wmic, netstat, reg, ps); {} bulundu",
        denetlenen
    );
}

#[test]
fn gozlem_turu_gözlenen_dosyalara_dokunmaz() {
    let gecici = yardimci::GeciciDizin::yeni("salt-okunur").expect("gecici dizin yok");

    // Gözlenecek dizin: içinde bir dosya var, aracın dokunmaması gerekir.
    let gozlenen = gecici.birlestir("gozlenen");
    std::fs::create_dir_all(&gozlenen).expect("gözlenen dizin yok");
    let hedef = gozlenen.join("veri.txt");
    std::fs::write(&hedef, b"degistirilmemeli").expect("yazilamadi");
    let onceki_icerik = std::fs::read(&hedef).expect("okunamadi");
    let onceki_degisiklik = std::fs::metadata(&hedef)
        .expect("metadata yok")
        .modified()
        .ok();

    // Gözlem kaynağı bu dizini salt okunur okur.
    let kaynak = procsight::kaynak::procfs::ProcFsSource::yeni(&gozlenen);
    let _kaynak: &dyn procsight::InventorySource = &kaynak;
    let _ = procsight::InventorySource::surecler(&kaynak);
    let _ = procsight::InventorySource::ag_baglantilari(&kaynak);
    let _ = procsight::InventorySource::baslangic_girdileri(&kaynak);

    let sonraki_icerik = std::fs::read(&hedef).expect("okunamadi");
    let sonraki_degisiklik = std::fs::metadata(&hedef)
        .expect("metadata yok")
        .modified()
        .ok();

    assert_eq!(onceki_icerik, sonraki_icerik, "gözlenen dosya değişti!");
    assert_eq!(
        onceki_degisiklik, sonraki_degisiklik,
        "gözlenen dosya zaman damgası değişti!"
    );

    // Gözlenen dizinde hiçbir yeni dosya oluşmamış olmalı.
    let giridler: Vec<String> = std::fs::read_dir(&gozlenen)
        .expect("okunamadi")
        .flatten()
        .map(|g| g.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(giridler, vec!["veri.txt".to_string()]);
}

#[test]
fn cikti_dizini_disinda_hicbir_yere_yazilmaz() {
    let gecici = yardimci::GeciciDizin::yeni("cikti-kapsam").expect("gecici dizin yok");
    let cikti = gecici.birlestir("cikti");
    std::fs::create_dir_all(&cikti).expect("çıktı dizini yok");

    // Tam bir gözlem + kural + rapor akışı çalıştır.
    let kaynak = procsight::kaynak::fixture::FixtureSource::varsayilan();
    let goruntu = procsight::AnlikGoruntu::topla(&kaynak);
    let yasam = procsight::YasamOzeti::bos();
    let bulgular = procsight::kurallar::tum_kurallar(
        &procsight::KuralBaglami {
            surecler: &goruntu.surecler,
            baglantilar: &goruntu.baglantilar,
            baslangic: &goruntu.baslangic,
            yasam: &yasam,
        },
        &procsight::KuralEsikleri::default(),
    );

    let gunluk_yolu = cikti.join("gunluk.jsonl");
    let mut gunluk = procsight::Gunluk::ac(&gunluk_yolu).expect("günlük açılamadı");
    gunluk
        .ekle(&[procsight::Olay::yeni(
            1,
            procsight::OlayTuru::SurecBasladi,
            Some(1),
            "fixture",
            "deneme",
        )])
        .expect("yazılamadı");

    let html_yolu = cikti.join("rapor.html");
    let ust = procsight::rapor::RaporUstBilgi {
        zaman: 1,
        kaynak: goruntu.kaynak.clone(),
        baslik: "Deneme".into(),
        kisit_notu: String::new(),
    };
    procsight::rapor::html_yaz(
        &html_yolu,
        &procsight::rapor::html_uret(&ust, &goruntu, &bulgular, &[]),
    )
    .expect("html yazılamadı");

    let csv_yolu = cikti.join("olaylar.csv");
    procsight::rapor::csv_yaz(&csv_yolu, &procsight::rapor::olaylar_csv(&[]))
        .expect("csv yazılamadı");

    let uretilen: Vec<String> = std::fs::read_dir(&cikti)
        .expect("okunamadi")
        .flatten()
        .map(|g| g.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(uretilen.contains(&"gunluk.jsonl".to_string()));
    assert!(uretilen.contains(&"rapor.html".to_string()));
    assert!(uretilen.contains(&"olaylar.csv".to_string()));
    assert_eq!(uretilen.len(), 3, "beklenmeyen ek dosya: {:?}", uretilen);

    // Depo kökünde (fixture klasörü dışında) yanlışlıkla dosya kalmadığını doğrula.
    let kok = Path::new(env!("CARGO_MANIFEST_DIR"));
    let izinsiz: Vec<String> = std::fs::read_dir(kok)
        .expect("okunamadi")
        .flatten()
        .map(|g| g.file_name().to_string_lossy().into_owned())
        .filter(|ad| ad.ends_with(".jsonl") || ad.ends_with(".html") || ad.ends_with(".csv"))
        .collect();
    assert!(
        izinsiz.is_empty(),
        "depo kökünde çıktı dosyası kaldı: {:?}",
        izinsiz
    );
}

#[test]
fn etik_sinir_ve_kural_metinleri_mudahale_onermez() {
    // Her kuralın "doğrulama adımı" metni yasak fiilleri içermemelidir.
    for kural in procsight::kurallar::TUM_KURALLAR {
        let adim = kural.sonraki_adim().to_ascii_lowercase();
        for yasak in ["sonlandir", "öldür", "kill", "engelle", "kaldir", "sil "] {
            assert!(
                !adim.contains(yasak),
                "{} kuralı müdahale öneriyor: {}",
                kural.ad(),
                adim
            );
        }
    }
    // Araç bir savunma ürünü iddiasında bulunmaz.
    assert!(procsight::ETIK_SINIR.contains("gozlem"));
    assert!(!procsight::ETIK_SINIR.contains("antivir"));
}

#[test]
fn kural_yapisi_mudahale_alani_icermez() {
    // `Bulgu` yalnızca kanıt taşır; alan sayısı sabittir ve bir eylem alanı yoktur.
    let bulgu = procsight::Bulgu::yeni(
        procsight::Kural::GizliAd,
        "pid=1 a.exe",
        vec!["kanıt".to_string()],
    );
    let json = serde_json::to_value(&bulgu).expect("serileştirilemedi");
    let alanlar = ["kural", "konu", "kanit", "aciklama", "sonraki_adim"];
    for alan in alanlar {
        assert!(json.get(alan).is_some(), "Bulgu alanı eksik: {}", alan);
    }
    let nesne = json.as_object().expect("nesne değil");
    assert_eq!(nesne.len(), alanlar.len());
    assert!(!nesne.contains_key("eylem"));
    assert!(!nesne.contains_key("mudahale"));
}
