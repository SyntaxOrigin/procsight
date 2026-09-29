//! HTML ve CSV dışa aktarımı.
//!
//! ## Ağsızlık garantisi
//!
//! Üretilen HTML **tek dosyadır ve hiçbir uzak kaynak içermez**: yazı tipi,
//! betik, stil veya resim bağlantısı yoktur. Raporu açmak için internet
//! bağlantısı gerekmez; bu, raporun S6 kabul kriteridir.
//!
//! ## CSV formül enjeksiyonu
//!
//! Rapor § 12'deki R8 riski: `=`, `+`, `-`, `@` ile başlayan alanlar Excel ve
//! LibreOffice'de formül olarak çalıştırılabilir. `csv_alan` bu dört karakteri
//! ve iki kontrol karakterini (TAB, CR) ele alıp değerin başına `'` koyar.
//!
//! ## XSS
//!
//! HTML üretiminde her metin değeri `html_kaçış` ile kaçırılır. Süreç adları,
//! komut satırları ve yollar sistemden gelir ve kullanıcı tarafından kontrol
//! edilemez; kaçış yapılmazsa `<script>` yazan bir dosya adı raporu bozabilir.

use std::fmt::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::hata::{Hata, Sonuc};
use crate::kaynak::AnlikGoruntu;
use crate::kurallar::Bulgu;
use crate::motor::Olay;

/// Rapor başlığında kullanılacak ürün adı.
pub const RAPOR_ADI: &str = "ProcSight";

/// Bir dışa aktarımın üretildiği an ve kaynağı.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaporUstBilgi {
    /// Raporun üretildiği an (Unix epoch milisaniyesi).
    pub zaman: u64,
    /// Görüntüyü üreten kaynağın adı.
    pub kaynak: String,
    /// Rapor başlığı.
    pub baslik: String,
    /// Kaynağın bilinen kısıtları.
    pub kisit_notu: String,
}

/// HTML metnindeki `&`, `<`, `>`, `"` ve `'` karakterlerini kaçırır.
pub fn html_kaçış(metin: &str) -> String {
    let mut sonuc = String::with_capacity(metin.len());
    for karakter in metin.chars() {
        match karakter {
            '&' => sonuc.push_str("&amp;"),
            '<' => sonuc.push_str("&lt;"),
            '>' => sonuc.push_str("&gt;"),
            '"' => sonuc.push_str("&quot;"),
            '\'' => sonuc.push_str("&#39;"),
            diger => sonuc.push(diger),
        }
    }
    sonuc
}

/// CSV alanını güvenli biçime çevirir.
///
/// Formül enjeksiyonuna yol açabilecek başlangıç karakterleri `'` ile
/// öneklenir; alan içinde tırnak veya virgül varsa çift tırnakla sarılır.
pub fn csv_alan(deger: &str) -> String {
    let guvenli = match deger.chars().next() {
        Some(c) => !matches!(c, '=' | '+' | '-' | '@' | '\t' | '\r'),
        None => true,
    };
    let duzeltilmis = if guvenli {
        deger.to_string()
    } else {
        format!("'{}", deger)
    };
    if duzeltilmis.contains(',') || duzeltilmis.contains('"') || duzeltilmis.contains('\n') {
        format!("\"{}\"", duzeltilmis.replace('"', "\"\""))
    } else {
        duzeltilmis
    }
}

/// Bir tabloyu CSV satırlarına çevirir (başlık dahil).
pub fn csv_tablo(basliklar: &[&str], satirlar: &[Vec<String>]) -> String {
    let mut cikti = String::new();
    let _ = writeln!(
        cikti,
        "{}",
        basliklar
            .iter()
            .map(|b| csv_alan(b))
            .collect::<Vec<String>>()
            .join(",")
    );
    for satir in satirlar {
        let _ = writeln!(
            cikti,
            "{}",
            satir
                .iter()
                .map(|h| csv_alan(h))
                .collect::<Vec<String>>()
                .join(",")
        );
    }
    cikti
}

/// Süreç tablosunun CSV gövdesini üretir.
pub fn surecleri_csv(goruntu: &AnlikGoruntu) -> String {
    let basliklar = [
        "pid",
        "ppid",
        "ad",
        "yol",
        "komut_satiri",
        "bellek_kib",
        "cpu_yuzde",
        "oturum",
        "kullanici",
        "uid",
        "durum",
    ];
    let satirlar: Vec<Vec<String>> = goruntu
        .surecler
        .iter()
        .map(|s| {
            vec![
                s.pid.to_string(),
                s.ppid.to_string(),
                s.ad.clone(),
                s.yol
                    .as_deref()
                    .map(|y| y.display().to_string())
                    .unwrap_or_default(),
                s.komut_satiri.clone(),
                s.bellek_kib.to_string(),
                s.cpu_yuzde.map(|c| format!("{:.2}", c)).unwrap_or_default(),
                s.oturum.map(|o| o.to_string()).unwrap_or_default(),
                s.kullanici.clone().unwrap_or_default(),
                s.uid.map(|u| u.to_string()).unwrap_or_default(),
                s.durum.metin().to_string(),
            ]
        })
        .collect();
    csv_tablo(&basliklar, &satirlar)
}

/// Bağlantı tablosunun CSV gövdesini üretir.
pub fn baglantilari_csv(goruntu: &AnlikGoruntu) -> String {
    let basliklar = ["protokol", "yerel", "uzak", "durum", "pid", "sahibi"];
    let satirlar: Vec<Vec<String>> = goruntu
        .baglantilar
        .iter()
        .map(|b| {
            vec![
                b.protokol.etiket().to_string(),
                format!("{}:{}", b.yerel_adres, b.yerel_port),
                match (b.uzak_adres, b.uzak_port) {
                    (Some(a), p) => {
                        format!("{}{}", a, p.map(|x| format!(":{}", x)).unwrap_or_default())
                    }
                    _ => String::new(),
                },
                b.durum.metin().to_string(),
                b.pid.map(|p| p.to_string()).unwrap_or_default(),
                b.pid
                    .and_then(|p| goruntu.surec(p))
                    .map(|s| s.ad.clone())
                    .unwrap_or_default(),
            ]
        })
        .collect();
    csv_tablo(&basliklar, &satirlar)
}

/// Başlangıç girdileri tablosunun CSV gövdesini üretir.
pub fn baslangic_csv(goruntu: &AnlikGoruntu) -> String {
    let basliklar = ["ad", "tur", "etkin", "konum", "hedef"];
    let satirlar: Vec<Vec<String>> = goruntu
        .baslangic
        .iter()
        .map(|g| {
            vec![
                g.ad.clone(),
                g.tur.etiket().to_string(),
                if g.etkin { "evet" } else { "hayir" }.to_string(),
                g.konum.display().to_string(),
                g.hedef.clone().unwrap_or_default(),
            ]
        })
        .collect();
    csv_tablo(&basliklar, &satirlar)
}

/// Olay günlüğünün CSV gövdesini üretir.
pub fn olaylar_csv(olaylar: &[Olay]) -> String {
    let basliklar = ["zaman", "olay", "pid", "kaynak", "ayrinti"];
    let satirlar: Vec<Vec<String>> = olaylar
        .iter()
        .map(|o| {
            vec![
                o.zaman.to_string(),
                o.olay.etiket().to_string(),
                o.pid.map(|p| p.to_string()).unwrap_or_default(),
                o.kaynak.clone(),
                o.ayrinti.clone(),
            ]
        })
        .collect();
    csv_tablo(&basliklar, &satirlar)
}

fn html_baslik(ust: &RaporUstBilgi) -> String {
    format!("{} — {}", html_kaçış(&ust.baslik), RAPOR_ADI)
}

/// Tek dosya HTML raporunu üretir.
///
/// Rapor; üst bilgi, kural bulguları, süreç tablosu, bağlantı tablosu,
/// başlangıç girdileri ve olay günlüğü bölümlerinden oluşur. Hiçbir bölümde
/// uzak kaynak referansı bulunmaz.
pub fn html_uret(
    ust: &RaporUstBilgi,
    goruntu: &AnlikGoruntu,
    bulgular: &[Bulgu],
    olaylar: &[Olay],
) -> String {
    let mut cikti = String::with_capacity(16_384);

    let _ = writeln!(cikti, "<!DOCTYPE html>");
    let _ = writeln!(cikti, "<html lang=\"tr\">");
    let _ = writeln!(cikti, "<head>");
    let _ = writeln!(cikti, "<meta charset=\"utf-8\">");
    let _ = writeln!(
        cikti,
        "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">"
    );
    let _ = writeln!(cikti, "<title>{}</title>", html_baslik(ust));
    let _ = writeln!(cikti, "<style>{}</style>", HTML_STIL);
    let _ = writeln!(cikti, "</head>");
    let _ = writeln!(cikti, "<body>");
    let _ = writeln!(cikti, "<h1>{}</h1>", html_kaçış(&ust.baslik));

    let _ = writeln!(
        cikti,
        "<p class=\"meta\">Kaynak: <b>{}</b> · Üretim anı: {} ms (Unix epoch) · \
Ayrıştırma: okunan {}, atlanan {}</p>",
        html_kaçış(&ust.kaynak),
        ust.zaman,
        goruntu.ayristirma.okunan,
        goruntu.ayristirma.atlanan
    );

    if !ust.kisit_notu.is_empty() {
        let _ = writeln!(
            cikti,
            "<p class=\"uyari\"><b>Kapsam notu:</b> {}</p>",
            html_kaçış(&ust.kisit_notu)
        );
    }
    if !goruntu.kisit_notu.is_empty() {
        let _ = writeln!(
            cikti,
            "<p class=\"uyari\"><b>Kaynak kısıtı:</b> {}</p>",
            html_kaçış(&goruntu.kisit_notu)
        );
    }
    if let Some(uyari) = goruntu.bozuk_satir_uyarisi() {
        let _ = writeln!(cikti, "<p class=\"uyari\">{}</p>", html_kaçış(&uyari));
    }

    let _ = writeln!(cikti, "<h2>Özet</h2>");
    let _ = writeln!(cikti, "<ul class=\"ozet\">");
    let _ = writeln!(cikti, "<li>Süreç: {}</li>", goruntu.surecler.len());
    let _ = writeln!(cikti, "<li>Bağlantı: {}</li>", goruntu.baglantilar.len());
    let _ = writeln!(
        cikti,
        "<li>Başlangıç girdisi: {}</li>",
        goruntu.baslangic.len()
    );
    let _ = writeln!(cikti, "<li>Olay: {}</li>", olaylar.len());
    let _ = writeln!(cikti, "<li>Kural bulgusu: {}</li>", bulgular.len());
    let _ = writeln!(cikti, "</ul>");

    bulgulari_yaz(&mut cikti, bulgular);
    surecleri_yaz(&mut cikti, goruntu);
    baglantilari_yaz(&mut cikti, goruntu);
    baslangici_yaz(&mut cikti, goruntu);
    olaylari_yaz(&mut cikti, olaylar);

    let _ = writeln!(
        cikti,
        "<footer><p>{} salt okunur bir gözlem aracıdır. Hiçbir süreci sonlandırmaz, \
hiçbir dosyayı silmez, hiçbir bağlantı kurmaz. Bulgu bir suçluluk hükmü değil, \
kendin doğrulayacağın bir gözlemdir.</p></footer>",
        RAPOR_ADI
    );
    let _ = writeln!(cikti, "</body>");
    let _ = writeln!(cikti, "</html>");
    cikti
}

fn bulgulari_yaz(cikti: &mut String, bulgular: &[Bulgu]) {
    let _ = writeln!(cikti, "<h2>Kural Bulguları ({})</h2>", bulgular.len());
    if bulgular.is_empty() {
        let _ = writeln!(
            cikti,
            "<p class=\"temiz\">Bu görüntüde yedi kuralın hiçbiri tetiklenmedi.</p>"
        );
        return;
    }
    let _ = writeln!(
        cikti,
        "<table><thead><tr>\
<th>Kural</th><th>Konu</th><th>Kanıt</th><th>Doğrulama adımı</th>\
</tr></thead><tbody>"
    );
    for bulgu in bulgular {
        let _ = writeln!(
            cikti,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td class=\"adim\">{}</td></tr>",
            html_kaçış(bulgu.kural.ad()),
            html_kaçış(&bulgu.konu),
            bulgu
                .kanit
                .iter()
                .map(|k| html_kaçış(k))
                .collect::<Vec<String>>()
                .join("<br>"),
            html_kaçış(&bulgu.sonraki_adim)
        );
    }
    let _ = writeln!(cikti, "</tbody></table>");
}

fn tablo_yaz(cikti: &mut String, baslik: &str, basliklar: &[&str], satirlar: Vec<Vec<String>>) {
    let _ = writeln!(
        cikti,
        "<h2>{} ({})</h2>",
        html_kaçış(baslik),
        satirlar.len()
    );
    let _ = writeln!(cikti, "<table><thead><tr>");
    for baslik in basliklar {
        let _ = writeln!(cikti, "<th>{}</th>", html_kaçış(baslik));
    }
    let _ = writeln!(cikti, "</tr></thead><tbody>");
    for satir in satirlar {
        let _ = writeln!(cikti, "<tr>");
        for hucre in satir {
            let _ = writeln!(cikti, "<td>{}</td>", html_kaçış(&hucre));
        }
        let _ = writeln!(cikti, "</tr>");
    }
    let _ = writeln!(cikti, "</tbody></table>");
}

fn surecleri_yaz(cikti: &mut String, goruntu: &AnlikGoruntu) {
    let satirlar = goruntu
        .surecler
        .iter()
        .map(|s| {
            vec![
                s.pid.to_string(),
                s.ppid.to_string(),
                s.ad.clone(),
                s.yol
                    .as_deref()
                    .map(|y| y.display().to_string())
                    .unwrap_or_default(),
                s.komut_satiri.clone(),
                format!("{}", s.bellek_mib()),
                s.oturum.map(|o| o.to_string()).unwrap_or_default(),
                s.kullanici.clone().unwrap_or_default(),
                s.durum.metin().to_string(),
            ]
        })
        .collect();
    tablo_yaz(
        cikti,
        "Süreçler",
        &[
            "PID",
            "Üst PID",
            "Ad",
            "Yol",
            "Komut satırı",
            "MiB",
            "Oturum",
            "Kullanıcı",
            "Durum",
        ],
        satirlar,
    );
}

fn baglantilari_yaz(cikti: &mut String, goruntu: &AnlikGoruntu) {
    let satirlar = goruntu
        .baglantilar
        .iter()
        .map(|b| {
            vec![
                b.protokol.etiket().to_string(),
                b.ucl_listesi(),
                b.durum.metin().to_string(),
                b.pid.map(|p| p.to_string()).unwrap_or_default(),
                b.pid
                    .and_then(|p| goruntu.surec(p))
                    .map(|s| s.ad.clone())
                    .unwrap_or_else(|| "çözülemedi".into()),
            ]
        })
        .collect();
    tablo_yaz(
        cikti,
        "Ağ Bağlantıları",
        &["Protokol", "Uçlar", "Durum", "PID", "Sahibi"],
        satirlar,
    );
}

fn baslangici_yaz(cikti: &mut String, goruntu: &AnlikGoruntu) {
    let satirlar = goruntu
        .baslangic
        .iter()
        .map(|g| {
            vec![
                g.ad.clone(),
                g.tur.etiket().to_string(),
                if g.etkin { "evet" } else { "hayır" }.to_string(),
                g.konum.display().to_string(),
                g.hedef.clone().unwrap_or_default(),
            ]
        })
        .collect();
    tablo_yaz(
        cikti,
        "Başlangıç Girdileri",
        &["Ad", "Tür", "Etkin", "Konum", "Hedef"],
        satirlar,
    );
}

fn olaylari_yaz(cikti: &mut String, olaylar: &[Olay]) {
    let satirlar = olaylar
        .iter()
        .map(|o| {
            vec![
                o.zaman.to_string(),
                o.olay.etiket().to_string(),
                o.pid.map(|p| p.to_string()).unwrap_or_default(),
                o.kaynak.clone(),
                o.ayrinti.clone(),
            ]
        })
        .collect();
    tablo_yaz(
        cikti,
        "Olay Günlüğü",
        &["Zaman", "Olay", "PID", "Kaynak", "Ayrıntı"],
        satirlar,
    );
}

/// Raporu HTML olarak yazar. Var olan dosyanın **üzerine yazılır**; bu, tek
/// yazma işlemidir ve kullanıcı tarafından açıkça verilen çıktı yoludur.
pub fn html_yaz(yol: &Path, icerik: &str) -> Sonuc<()> {
    std::fs::write(yol, icerik).map_err(|kaynak| Hata::Yazma {
        yol: yol.to_path_buf(),
        kaynak,
    })
}

/// Raporu CSV olarak yazar.
pub fn csv_yaz(yol: &Path, icerik: &str) -> Sonuc<()> {
    std::fs::write(yol, icerik).map_err(|kaynak| Hata::Yazma {
        yol: yol.to_path_buf(),
        kaynak,
    })
}

/// Uzak kaynak içermeyen, gömülü stil bloğu.
const HTML_STIL: &str = "body{font-family:system-ui,-apple-system,Segoe UI,Roboto,sans-serif;\
margin:1.5rem;line-height:1.5;color:#1a1a1a;background:#fbfbfb}\
h1{font-size:1.6rem;margin:0 0 .25rem}h2{font-size:1.15rem;margin:1.8rem 0 .5rem;\
border-bottom:1px solid #ddd;padding-bottom:.25rem}\
p.meta{color:#555;margin:.25rem 0}\
p.uyari{background:#fff6e0;border-left:4px solid #e8a33d;padding:.5rem .75rem;margin:.75rem 0}\
p.temiz{background:#eaf6ea;border-left:4px solid #4a9c4a;padding:.5rem .75rem}\
ul.ozet{list-style:none;padding:0;display:flex;flex-wrap:wrap;gap:.5rem 1.5rem}\
table{border-collapse:collapse;width:100%;margin:.5rem 0;font-size:.9rem}\
th,td{border:1px solid #ddd;padding:.35rem .5rem;text-align:left;vertical-align:top}\
th{background:#f0f0f0}\
td.adim{color:#444;font-style:italic}\
footer{margin-top:2rem;padding-top:.75rem;border-top:1px solid #ddd;color:#666;font-size:.85rem}";

#[cfg(test)]
// Gerekçe: unwrap/expect yalnızca test içinde kullanılır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::kurallar::{Kural, KuralBaglami, KuralEsikleri};
    use crate::motor::{Olay, OlayTuru};

    fn goruntu() -> AnlikGoruntu {
        AnlikGoruntu::topla(&crate::kaynak::fixture::FixtureSource::varsayilan())
    }

    #[test]
    fn html_kacis_tum_karakterleri_kapsar() {
        assert_eq!(
            html_kaçış("<script>alert('x' & \"y\")</script>"),
            "&lt;script&gt;alert(&#39;x&#39; &amp; &quot;y&quot;)&lt;/script&gt;"
        );
    }

    #[test]
    fn csv_formul_enjeksiyonu_onlenir() {
        assert_eq!(csv_alan("=1+1"), "'=1+1");
        assert_eq!(csv_alan("+A1"), "'+A1");
        assert_eq!(csv_alan("-2"), "'-2");
        assert_eq!(csv_alan("@SUM(A1)"), "'@SUM(A1)");
        assert_eq!(csv_alan("\tx"), "'\tx");
    }

    #[test]
    fn csv_normal_degerler_degismez() {
        assert_eq!(csv_alan("explorer.exe"), "explorer.exe");
        assert_eq!(csv_alan(""), "");
        assert_eq!(csv_alan("a,b"), "\"a,b\"");
        assert_eq!(csv_alan("de\"ne"), "\"de\"\"ne\"");
    }

    #[test]
    fn html_uzak_kaynak_icermez() {
        let g = goruntu();
        let ust = RaporUstBilgi {
            zaman: 1,
            kaynak: "fixture".into(),
            baslik: "Test".into(),
            kisit_notu: "kisit yok".into(),
        };
        let bulgular = crate::kurallar::tum_kurallar(
            &KuralBaglami {
                surecler: &g.surecler,
                baglantilar: &g.baglantilar,
                baslangic: &g.baslangic,
                yasam: &crate::motor::YasamOzeti::bos(),
            },
            &KuralEsikleri::default(),
        );
        let icerik = html_uret(&ust, &g, &bulgular, &[]);
        assert!(!icerik.contains("http://"));
        assert!(!icerik.contains("https://"));
        assert!(!icerik.contains("<script"));
        assert!(!icerik.contains("<img"));
        assert!(!icerik.contains("src="));
    }

    #[test]
    fn html_bulgulari_ve_bolumleri_icerir() {
        let g = goruntu();
        let ust = RaporUstBilgi {
            zaman: 1,
            kaynak: "fixture".into(),
            baslik: "Test".into(),
            kisit_notu: String::new(),
        };
        let bulgular = vec![Bulgu::yeni(
            Kural::GizliAd,
            "pid=1 <b>a.exe</b>",
            vec!["k".into()],
        )];
        let icerik = html_uret(
            &ust,
            &g,
            &bulgular,
            &[Olay::yeni(
                5,
                OlayTuru::SurecBasladi,
                Some(1),
                "fixture",
                "x",
            )],
        );
        assert!(icerik.contains("Kural Bulguları (1)"));
        assert!(icerik.contains("Süreçler ("));
        assert!(icerik.contains("Ağ Bağlantıları ("));
        assert!(icerik.contains("Başlangıç Girdileri ("));
        assert!(icerik.contains("Olay Günlüğü (1)"));
        assert!(icerik.contains("&lt;b&gt;a.exe&lt;/b&gt;"));
        assert!(icerik.contains("salt okunur bir gözlem aracıdır"));
    }

    #[test]
    fn html_bulgu_yokken_temiz_mesaji_verir() {
        let g = AnlikGoruntu {
            zaman: 0,
            kaynak: "fixture".into(),
            surecler: Vec::new(),
            baglantilar: Vec::new(),
            baslangic: Vec::new(),
            ayristirma: Default::default(),
            kisit_notu: String::new(),
        };
        let icerik = html_uret(&bos_ust_bilgi(), &g, &[], &[]);
        assert!(icerik.contains("hiçbiri tetiklenmedi"));
    }

    fn bos_ust_bilgi() -> RaporUstBilgi {
        RaporUstBilgi {
            zaman: 0,
            kaynak: "test".into(),
            baslik: "Bos".into(),
            kisit_notu: String::new(),
        }
    }

    #[test]
    fn csv_govdeleri_baslik_icerir() {
        let g = goruntu();
        let s = surecleri_csv(&g);
        assert!(s.starts_with("pid,ppid,ad"));
        assert_eq!(s.lines().count(), g.surecler.len() + 1);
        let b = baglantilari_csv(&g);
        assert!(b.starts_with("protokol,yerel"));
        let bas = baslangic_csv(&g);
        assert!(bas.starts_with("ad,tur,etkin"));
    }

    #[test]
    fn olay_csv_si_bos_girdide_de_baslik_yazar() {
        let c = olaylar_csv(&[]);
        assert_eq!(c.trim(), "zaman,olay,pid,kaynak,ayrinti");
    }

    #[test]
    fn csv_yazma_ve_icerik_kontrolu() {
        let yol = std::env::temp_dir().join(format!("procsight-rapor-{}.csv", std::process::id()));
        csv_yaz(&yol, "a,b\n1,2\n").unwrap();
        let icerik = std::fs::read_to_string(&yol).unwrap();
        assert_eq!(icerik, "a,b\n1,2\n");
        let _ = std::fs::remove_file(&yol);
    }

    #[test]
    fn csv_yazma_hatasi_yol_bilgisi_tasir() {
        let sonuc = csv_yaz(Path::new("C:/procsight-yok/olmayan/dizin/a.csv"), "x");
        assert!(sonuc.is_err());
    }
}
