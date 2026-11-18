//! Scan-Import mit PDFium (übersprungen, wenn PDFium fehlt).

use std::path::PathBuf;

use godgrader_core::paths::AppPaths;
use godgrader_core::projekt::{Projekt, ProjektMeta};
use godgrader_core::scans::{Bereich, ImportAuftrag, ScanOptionen, Schwaerzung, SeitenAuswahl};

fn sample(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn stapel_import_mit_schwaerzung_und_teilen() {
    if godgrader_core::pdfium::pdfium().is_err() {
        eprintln!("PDFium nicht verfügbar, Test übersprungen");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(dir.path());
    paths.ensure().unwrap();
    let p = Projekt::create(
        &paths,
        ProjektMeta {
            titel: "Test".into(),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(p.beispielseite(400).unwrap().is_none());
    p.set_schwaerzung(&[Schwaerzung {
        bereich: Bereich {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 0.15,
        },
        seiten: SeitenAuswahl::Seiten(vec![1]),
    }])
    .unwrap();
    let pdf = sample("1_Klausur_WR_11.1.pdf");
    assert_eq!(
        godgrader_core::scans::seitenzahl(std::path::Path::new(&pdf)).unwrap(),
        4
    );
    let auftraege = vec![
        ImportAuftrag {
            bezeichnung: "Edler Kranich".into(),
            pdf: pdf.clone(),
            seiten: vec![0, 1],
        },
        ImportAuftrag {
            bezeichnung: "Stiller Fuchs".into(),
            pdf: pdf.clone(),
            seiten: vec![2, 3],
        },
    ];
    let mut schritte = 0;
    let ks = p
        .scans_importieren(&auftraege, &ScanOptionen { teilen: false }, |_| {
            schritte += 1
        })
        .unwrap();
    assert_eq!(ks.len(), 2);
    assert!(schritte >= 4);
    assert_eq!(ks[0].seiten.len(), 2);
    assert_eq!(ks[1].seiten[0].quelle_seite, 3);

    // Seite 1 oben geschwärzt, Seite 2 nicht
    let s1 = image::open(p.seitenbild(&ks[0].seiten[0].id).unwrap())
        .unwrap()
        .to_luma8();
    let s2 = image::open(p.seitenbild(&ks[0].seiten[1].id).unwrap())
        .unwrap()
        .to_luma8();
    assert!(s1.width() > 2400, "300 dpi: {}", s1.width());
    let dunkel = |g: &image::GrayImage, y: u32| {
        (0..g.width())
            .step_by(50)
            .filter(|&x| g.get_pixel(x, y).0[0] < 30)
            .count()
    };
    assert!(dunkel(&s1, 100) > 40, "Schwärzung fehlt");
    assert!(dunkel(&s2, 100) < 10, "Seite 2 darf nicht geschwärzt sein");

    // Umbenennen, Löschen, Originale aufräumen
    p.klausur_umbenennen(&ks[1].id, "Kluger Dachs").unwrap();
    // Beispielseite für das Schwärzen-Werkzeug kommt ungeschwärzt aus dem Original
    assert!(p
        .beispielseite(400)
        .unwrap()
        .unwrap()
        .starts_with("data:image/png;base64,"));
    assert_eq!(p.klausuren().unwrap()[1].bezeichnung, "Kluger Dachs");
    p.klausur_loeschen(&ks[0].id).unwrap();
    assert_eq!(p.klausuren().unwrap().len(), 1);

    // Teilen
    let ks = p
        .scans_importieren(
            &[ImportAuftrag {
                bezeichnung: "Halb".into(),
                pdf,
                seiten: vec![0],
            }],
            &ScanOptionen { teilen: true },
            |_| {},
        )
        .unwrap();
    assert_eq!(ks[0].seiten.len(), 2);
    p.seiten_neu_erzeugen(|_| {}).unwrap();
    assert!(p
        .seiten_miniatur(&ks[0].seiten[0].id, 120)
        .unwrap()
        .starts_with("data:image/jpeg;base64,"));
}
