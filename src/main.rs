//! ProcSight komut satırı arayüzü.
//!
//! Dört alt komut sunulur:
//!
//! - `snapshot` — tek seferlik envanter: süreç, bağlantı ve başlangıç girdisi.
//! - `watch`    — periyodik yoklama; JSONL olay günlüğü üretir.
//! - `rules`    — yedi kuralı listeler ve isteğe bağlı olarak değerlendirir.
//! - `report`   — günlükten HTML ve CSV dışa aktarımı üretir.
//!
//! Hiçbir alt komut bir süreci etkilemez, bir dosyayı silmez veya bir ağ
//! bağlantısı kurmaz. Tek yazma işlemi `--gunluk`, `--html` ve `--csv` ile
//! **kullanıcının açıkça verdiği** yollardır.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};

use procsight::kurallar::{KuralBaglami, KuralEsikleri};
use procsight::motor::{tur_olaylari, YasamTakip};
use procsight::rapor::RaporUstBilgi;
use procsight::{
    hata::{Hata, Sonuc},
    kaynak::{AnlikGoruntu, KaynakTuru},
    kurallar,
    motor::YasamOzeti,
};

/// ProcSight — süreç, ağ ve başlangıç girdisi gözlemleyicisi.
#[derive(Debug, Parser)]
#[command(name = "procsight", version, about, long_about = None)]
struct Komut {
    #[command(subcommand)]
    eylem: Eylem,
}

#[derive(Debug, Subcommand)]
enum Eylem {
    /// Tek seferlik süreç/bağlantı/başlangıç girdisi envanterini yazar.
    Snapshot(SnapshotArg),
    /// Periyodik yoklama yapar ve JSONL olay günlüğü üretir.
    Watch(WatchArg),
    /// Yedi şüpheli davranış kuralını listeler ve değerlendirir.
    Rules(RulesArg),
    /// Olay günlüğünden HTML ve CSV raporu üretir.
    Report(ReportArg),
}

/// Kaynak seçimiyle ilgili ortak bayraklar.
#[derive(Debug, Args, Clone)]
struct KaynakArg {
    /// Envanter kaynağı.
    #[arg(long, value_enum, default_value_t = KaynakTuruSecim::Otomatik)]
    kaynak: KaynakTuruSecim,

    /// `wmic` ile komut satırı ve üst süreç bilgisini de toplar (yalnız Windows).
    #[arg(long)]
    wmic: bool,

    /// `reg query` ile Run anahtarlarını da okur (yalnız Windows).
    #[arg(long)]
    reg: bool,

    /// Ayrıntılı çıktı: komut satırı ve çözülmemiş bağlantı sahipleri.
    #[arg(long)]
    ayrinti: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum KaynakTuruSecim {
    Otomatik,
    Windows,
    Procfs,
    Ps,
    Fixture,
}

impl From<KaynakTuruSecim> for KaynakTuru {
    fn from(deger: KaynakTuruSecim) -> Self {
        match deger {
            KaynakTuruSecim::Otomatik => KaynakTuru::Otomatik,
            KaynakTuruSecim::Windows => KaynakTuru::Windows,
            KaynakTuruSecim::Procfs => KaynakTuru::Procfs,
            KaynakTuruSecim::Ps => KaynakTuru::Ps,
            KaynakTuruSecim::Fixture => KaynakTuru::Fixture,
        }
    }
}

/// `snapshot` alt komutunun bayrakları.
#[derive(Debug, Args)]
struct SnapshotArg {
    #[command(flatten)]
    kaynak: KaynakArg,

    /// Çıktı biçimi.
    #[arg(long, value_enum, default_value_t = Bicim::Metin)]
    bicim: Bicim,

    /// Çıktıyı dosyaya yazar; verilmezse standart çıktıya basılır.
    #[arg(long)]
    cikti: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Bicim {
    /// Sütunlu, insan okunur metin.
    Metin,
    /// Makine tarafından okunacak JSON.
    Json,
}

/// `watch` alt komutunun bayrakları.
#[derive(Debug, Args)]
struct WatchArg {
    #[command(flatten)]
    kaynak: KaynakArg,

    /// JSONL olay günlüğünün yolu.
    #[arg(long, default_value = "procsight-gunlugu.jsonl")]
    gunluk: PathBuf,

    /// Yoklama aralığı (milisaniye). Varsayılan 1000 ms.
    #[arg(long, default_value_t = 1000)]
    aralik_ms: u64,

    /// Kaç tur yapılacağı. `0` sonsuza kadar çalışır.
    #[arg(long, default_value_t = 0)]
    tur: u32,

    /// `aralik_ms` kadar bekleyip çıkar.
    #[arg(long, default_value_t = 60)]
    bekle_saniye: u64,
}

/// `rules` alt komutunun bayrakları.
#[derive(Debug, Args)]
struct RulesArg {
    #[command(flatten)]
    kaynak: KaynakArg,

    /// Yalnızca kural kataloğunu yazar; envanter toplamaz.
    #[arg(long)]
    sadece_katalog: bool,

    /// Yüksek bellek kuralının MiB eşiği.
    #[arg(long, default_value_t = 512)]
    bellek_mib: u64,

    /// Kısa ömürlü süreç kuralının adet eşiği.
    #[arg(long, default_value_t = 5)]
    kisa_omurlu: usize,

    /// Kısa ömürlü süreç istatistiğini bu günlükten hesaplar.
    #[arg(long)]
    gunluk: Option<PathBuf>,

    /// Çıktı biçimi.
    #[arg(long, value_enum, default_value_t = Bicim::Metin)]
    bicim: Bicim,
}

/// `report` alt komutunun bayrakları.
#[derive(Debug, Args)]
struct ReportArg {
    /// Kaynak JSONL olay günlüğü.
    #[arg(long)]
    gunluk: PathBuf,

    /// Üretilecek HTML raporunun yolu.
    #[arg(long)]
    html: Option<PathBuf>,

    /// Üretilecek CSV dosyasının yolu (olay günlüğü).
    #[arg(long)]
    csv: Option<PathBuf>,

    /// Rapor başlığı.
    #[arg(long, default_value = "ProcSight gozlem raporu")]
    baslik: String,

    /// Envanter kaynağının adı (rapor başlığına yazılır).
    #[arg(long, default_value = "bilinmiyor")]
    kaynak_adi: String,
}

fn main() -> ExitCode {
    let komut = Komut::parse();
    match calistir(komut) {
        Ok(()) => ExitCode::SUCCESS,
        Err(hata) => {
            let _ = writeln!(std::io::stderr(), "hata: {}", hata);
            ExitCode::FAILURE
        }
    }
}

fn kaynak_kur(secim: &KaynakArg) -> Sonuc<Box<dyn procsight::InventorySource>> {
    let tur = KaynakTuru::from(secim.kaynak);
    procsight::kaynak::kaynagi_dogrula(tur)?;

    // `--wmic` ve `--reg` yalnızca Windows yolunda anlamlıdır. Diğer
    // platformlarda bayraklar yok sayılır; kullanıcı yanlış bilgilendirilmez.
    let _ = (secim.wmic, secim.reg);

    #[cfg(target_os = "windows")]
    {
        if matches!(tur, KaynakTuru::Otomatik | KaynakTuru::Windows) {
            let mut windows = procsight::kaynak::windows::WindowsSource::yeni(secim.wmic);
            if secim.reg {
                windows = windows.reg_ac();
            }
            return Ok(Box::new(windows));
        }
    }

    #[cfg(target_os = "linux")]
    {
        if matches!(tur, KaynakTuru::Otomatik | KaynakTuru::Procfs) {
            return Ok(Box::new(procsight::kaynak::procfs::ProcFsSource::yeni(
                "/proc",
            )));
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        if matches!(tur, KaynakTuru::Otomatik | KaynakTuru::Ps) {
            return Ok(Box::new(procsight::kaynak::ps::PsSource::yeni()));
        }
    }

    procsight::kaynak::kesif(tur)
}

fn calistir(komut: Komut) -> Sonuc<()> {
    match komut.eylem {
        Eylem::Snapshot(arg) => snapshot(arg),
        Eylem::Watch(arg) => watch(arg),
        Eylem::Rules(arg) => rules(arg),
        Eylem::Report(arg) => report(arg),
    }
}

fn baslik_yaz(kaynak: &str) {
    println!("{}", procsight::ETIK_SINIR);
    println!("kaynak: {}", kaynak);
    println!();
}

fn snapshot(arg: SnapshotArg) -> Sonuc<()> {
    let kaynak = kaynak_kur(&arg.kaynak)?;
    let goruntu = AnlikGoruntu::topla(kaynak.as_ref());
    let yasam = YasamOzeti::bos();
    let bulgular = kurallar::tum_kurallar(
        &KuralBaglami {
            surecler: &goruntu.surecler,
            baglantilar: &goruntu.baglantilar,
            baslangic: &goruntu.baslangic,
            yasam: &yasam,
        },
        &KuralEsikleri::default(),
    );

    let metin = match arg.bicim {
        Bicim::Json => serde_json::to_string_pretty(&goruntu).map_err(|hata| Hata::Ayristirma {
            program: "proc_serilestirme",
            ayrinti: hata.to_string(),
        })?,
        Bicim::Metin => metin_goruntu(&goruntu, &bulgular),
    };

    match &arg.cikti {
        Some(yol) => procsight::rapor::csv_yaz(yol, &metin)?,
        None => println!("{}", metin),
    }
    Ok(())
}

fn metin_goruntu(goruntu: &AnlikGoruntu, bulgular: &[kurallar::Bulgu]) -> String {
    let mut cikti = String::new();
    cikti.push_str(procsight::ETIK_SINIR);
    cikti.push('\n');
    cikti.push_str(&format!("kaynak: {}\n\n", goruntu.kaynak));
    if !goruntu.kisit_notu.is_empty() {
        cikti.push_str(&format!("kapsam notu: {}\n\n", goruntu.kisit_notu));
    }
    if let Some(uyari) = goruntu.bozuk_satir_uyarisi() {
        cikti.push_str(&format!("{}\n\n", uyari));
    }

    cikti.push_str(&format!("SURECLER ({})\n", goruntu.surecler.len()));
    cikti.push_str("PID     PPID    AD                     MiB  DURUM      KULLANICI\n");
    for s in &goruntu.surecler {
        let yol = s
            .yol
            .as_deref()
            .map(|y| format!("  {}", y.display()))
            .unwrap_or_default();
        cikti.push_str(&format!(
            "{:<8}{:<8}{:<24}{:>5}  {:<10}{}{}\n",
            s.pid,
            s.ppid,
            s.ad,
            s.bellek_mib(),
            s.durum.metin(),
            s.kullanici.as_deref().unwrap_or("-"),
            yol
        ));
    }

    cikti.push_str(&format!("\nBAGLANTILAR ({})\n", goruntu.baglantilar.len()));
    cikti.push_str("PROTO  UCLAR                                           PID    SAHIBI\n");
    for b in &goruntu.baglantilar {
        let sahibi = b
            .pid
            .and_then(|p| goruntu.surec(p))
            .map(|s| s.ad.clone())
            .unwrap_or_else(|| "-".into());
        cikti.push_str(&format!(
            "{:<6} {:<47} {:<6} {}\n",
            b.protokol.etiket(),
            b.ucl_listesi(),
            b.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into()),
            sahibi
        ));
    }

    cikti.push_str(&format!(
        "\nBASLANGIC GIRDILERI ({})\n",
        goruntu.baslangic.len()
    ));
    cikti.push_str("AD                   TUR                 DURUM    KONUM\n");
    for g in &goruntu.baslangic {
        cikti.push_str(&format!(
            "{:<20} {:<18} {:<9} {}\n",
            g.ad,
            g.tur.etiket(),
            if g.etkin { "etkin" } else { "kapali" },
            g.konum.display()
        ));
    }

    cikti.push_str(&format!("\nKURAL BULGULARI ({})\n", bulgular.len()));
    for b in bulgular {
        cikti.push_str(&format!("{}\n", b.ozet()));
        cikti.push_str(&format!("  dogrulama: {}\n", b.sonraki_adim));
    }
    cikti
}

fn watch(arg: WatchArg) -> Sonuc<()> {
    if arg.aralik_ms == 0 {
        return Err(Hata::Parametre {
            ad: "--aralik-ms",
            ayrinti: "sifir olamaz".into(),
        });
    }
    let kaynak = kaynak_kur(&arg.kaynak)?;
    let mut gunluk = procsight::Gunluk::ac(&arg.gunluk)?;
    let mut takip = YasamTakip::yeni();
    let mut onceki: std::collections::HashMap<u32, procsight::Surec> =
        std::collections::HashMap::new();

    baslik_yaz(kaynak.ad());
    println!("gunluk: {}", arg.gunluk.display());
    println!(
        "aralik: {} ms, tur: {}",
        arg.aralik_ms,
        if arg.tur == 0 {
            "sonsuz".into()
        } else {
            arg.tur.to_string()
        }
    );
    println!();

    let baslangic = std::time::Instant::now();
    let bekleme = std::time::Duration::from_millis(arg.aralik_ms);
    let son = std::time::Duration::from_secs(arg.bekle_saniye);
    let mut tur = 0u32;

    while arg.tur == 0 || tur < arg.tur {
        tur += 1;
        let goruntu = AnlikGoruntu::topla(kaynak.as_ref());
        let yasam = takip.guncelle(&goruntu.surecler);
        let mut olaylar = tur_olaylari(&goruntu, &yasam, &onceki);
        if !goruntu.kisit_notu.is_empty() {
            olaylar.push(procsight::Olay::yeni(
                goruntu.zaman,
                procsight::OlayTuru::KaynakHatasi,
                None,
                &goruntu.kaynak,
                format!("kapsam notu: {}", goruntu.kisit_notu),
            ));
        }
        let yazilan = gunluk.ekle(&olaylar)?;
        onceki = procsight::motor::surece_haritasi(&goruntu);

        println!(
            "tur {:>4}  surec {:>4}  yeni {:>3}  bitti {:>3}  kisa omurlu {:>3}  yazilan {}",
            tur,
            goruntu.surecler.len(),
            yasam.yeni.len(),
            yasam.kayboldu.len(),
            yasam.kisa_omurlu.len(),
            yazilan
        );

        if arg.tur != 0 && tur >= arg.tur {
            break;
        }
        std::thread::sleep(bekleme);
        if arg.bekle_saniye > 0 && baslangic.elapsed() >= son {
            println!("{}\n surecinde bitti", son.as_secs());
            break;
        }
    }
    Ok(())
}

fn rules(arg: RulesArg) -> Sonuc<()> {
    if arg.sadece_katalog {
        baslik_yaz("katalog");
        for (ad, aciklama, adim) in kurallar::kural_katalogu() {
            println!(
                "{}\n  ne gozlemliyor: {}\n  nasil dogrulanir: {}",
                ad, aciklama, adim
            );
        }
        return Ok(());
    }

    let esikler = KuralEsikleri {
        yuksek_bellek_mib: arg.bellek_mib,
        kisa_omurlu_esik: arg.kisa_omurlu,
        ..KuralEsikleri::default()
    };
    let kaynak = kaynak_kur(&arg.kaynak)?;
    let goruntu = AnlikGoruntu::topla(kaynak.as_ref());

    let yasam = match &arg.gunluk {
        Some(yol) => {
            let (olaylar, ozet) = procsight::gunluk::oku(yol)?;
            println!(
                "gunluk: {} ({}; bozuk satir sayisi acikca raporlandi)",
                yol.display(),
                ozet.ozet()
            );
            YasamOzeti::kisa_omurlu_ile(procsight::gunluk::kisa_omurlu_derle(&olaylar))
        }
        None => {
            println!(
                "kisa omurlu sures kurali: --gunluk verilmedigi icin 0 sayildi \
(yoklama suresi disinda kisa omurlu surec sayilamaz)"
            );
            YasamOzeti::bos()
        }
    };

    let bulgular = kurallar::tum_kurallar(
        &KuralBaglami {
            surecler: &goruntu.surecler,
            baglantilar: &goruntu.baglantilar,
            baslangic: &goruntu.baslangic,
            yasam: &yasam,
        },
        &esikler,
    );

    match arg.bicim {
        Bicim::Json => {
            let veri = serde_json::json!({
                "kaynak": goruntu.kaynak,
                "surec": goruntu.surecler.len(),
                "baglanti": goruntu.baglantilar.len(),
                "baslangic": goruntu.baslangic.len(),
                "kisa_omurlu": yasam.kisa_omurlu.len(),
                "bulgu": bulgular,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&veri).map_err(|hata| Hata::Ayristirma {
                    program: "proc_serilestirme",
                    ayrinti: hata.to_string(),
                })?
            );
        }
        Bicim::Metin => {
            baslik_yaz(&goruntu.kaynak);
            println!(
                "envanter: {} surec, {} baglanti, {} baslangic girdisi",
                goruntu.surecler.len(),
                goruntu.baglantilar.len(),
                goruntu.baslangic.len()
            );
            println!(
                "esikler: bellek >= {} MiB, kisa omurlu >= {} adet",
                esikler.yuksek_bellek_mib, esikler.kisa_omurlu_esik
            );
            println!();
            if bulgular.is_empty() {
                println!("hicbir kural tetiklenmedi");
            }
            for bulgu in &bulgular {
                println!("[{}] {}", bulgu.kural.ad(), bulgu.konu);
                for kanit in &bulgu.kanit {
                    println!("    kanit: {}", kanit);
                }
                println!("    gozlem: {}", bulgu.aciklama);
                println!("    dogrulama: {}", bulgu.sonraki_adim);
                println!();
            }
        }
    }
    Ok(())
}

fn report(arg: ReportArg) -> Sonuc<()> {
    let (olaylar, ozet) = procsight::gunluk::oku(&arg.gunluk)?;
    println!("{} {}", arg.gunluk.display(), ozet.ozet());
    if ozet.bozuk_satir > 0 {
        println!(
            "uyari: {} satir bozuk oldu ve atlandi; gunluk bu satirlar icin eksiktir",
            ozet.bozuk_satir
        );
    }

    let yasam = YasamOzeti::kisa_omurlu_ile(procsight::gunluk::kisa_omurlu_derle(&olaylar));
    let bos = AnlikGoruntu {
        zaman: procsight::kaynak::epoch_millis(),
        kaynak: arg.kaynak_adi.clone(),
        surecler: Vec::new(),
        baglantilar: Vec::new(),
        baslangic: Vec::new(),
        ayristirma: Default::default(),
        kisit_notu: "bu rapor yalnizca olay gunlugunden uretildi; canli envanter icermez".into(),
    };
    let bulgular = kurallar::tum_kurallar(
        &KuralBaglami {
            surecler: &bos.surecler,
            baglantilar: &bos.baglantilar,
            baslangic: &bos.baslangic,
            yasam: &yasam,
        },
        &KuralEsikleri::default(),
    );

    let ust = RaporUstBilgi {
        zaman: bos.zaman,
        kaynak: arg.kaynak_adi.clone(),
        baslik: arg.baslik.clone(),
        kisit_notu:
            "kural 6 gunlukten yeniden uretildi; diger kurallar icin canli envanter gerekir".into(),
    };

    let mut uretilen: Vec<PathBuf> = Vec::new();
    if let Some(yol) = &arg.html {
        let icerik = procsight::rapor::html_uret(&ust, &bos, &bulgular, &olaylar);
        procsight::rapor::html_yaz(yol, &icerik)?;
        uretilen.push(yol.clone());
    }
    if let Some(yol) = &arg.csv {
        let icerik = procsight::rapor::olaylar_csv(&olaylar);
        procsight::rapor::csv_yaz(yol, &icerik)?;
        uretilen.push(yol.clone());
    }
    if uretilen.is_empty() {
        println!("uyari: --html veya --csv verilmedi; hicbir dosya yazilmadi");
    }
    for yol in uretilen {
        println!("yazildi: {}", yol.display());
    }
    Ok(())
}

#[cfg(test)]
// Gerekçe: test içinde `expect` kullanımı kabul edilir; üretim kodunda yasaktır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn komut_satiri_sozlesmesi_gecerli() {
        Komut::command().debug_assert();
    }

    #[test]
    fn varsayilan_alt_komut_zorunlu() {
        let sonuc = Komut::try_parse_from(["procsight"]);
        assert!(sonuc.is_err(), "alt komut verilmeden calismamali");
    }

    #[test]
    fn bilinmeyen_bayrak_reddedilir() {
        let sonuc = Komut::try_parse_from(["procsight", "snapshot", "--sonlandir"]);
        assert!(sonuc.is_err());
    }

    #[test]
    fn kaynak_secimi_kaynak_turune_cevilir() {
        assert_eq!(
            KaynakTuru::from(KaynakTuruSecim::Fixture),
            KaynakTuru::Fixture
        );
        assert_eq!(
            KaynakTuru::from(KaynakTuruSecim::Otomatik),
            KaynakTuru::Otomatik
        );
    }

    #[test]
    fn snapshot_fixture_kaynagi_ile_calisir() {
        let komut = Komut::try_parse_from(["procsight", "snapshot", "--kaynak", "fixture"])
            .expect("ayrıştırılamadı");
        let Eylem::Snapshot(arg) = komut.eylem else {
            panic!("snapshot bekleniyordu");
        };
        assert_eq!(arg.bicim, Bicim::Metin);
        snapshot(arg).expect("snapshot başarısız");
    }

    #[test]
    fn sifir_aralik_reddedilir() {
        let komut = Komut::try_parse_from(["procsight", "watch", "--aralik-ms", "0", "--tur", "1"])
            .expect("ayrıştırılamadı");
        let Eylem::Watch(arg) = komut.eylem else {
            panic!("watch bekleniyordu");
        };
        assert!(watch(arg).is_err());
    }

    #[test]
    fn kurallar_sadece_katalog_donusu_basar() {
        let komut = Komut::try_parse_from(["procsight", "rules", "--sadece-katalog"])
            .expect("ayrıştırılamadı");
        let Eylem::Rules(arg) = komut.eylem else {
            panic!("rules bekleniyordu");
        };
        rules(arg).expect("katalog başarısız");
    }

    #[test]
    fn etik_sinir_bildirimi_her_komutta_var() {
        assert!(procsight::ETIK_SINIR.contains("sonlandirmaz"));
        assert!(procsight::ETIK_SINIR.contains("kayit defterine yazmaz"));
    }
}
