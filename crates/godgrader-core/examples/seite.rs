//! Entwicklerwerkzeug: rendert eine PDF-Seite wie beim Scan-Import.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let pdfium = godgrader_core::pdfium::pdfium().unwrap();
    let doc = pdfium.load_pdf_from_file(&a[1], None).unwrap();
    let page = doc.pages().get(a[2].parse::<i32>().unwrap()).unwrap();
    use pdfium_render::prelude::*;
    let img = page
        .render_with_config(&PdfRenderConfig::new().set_target_width(2480))
        .unwrap()
        .as_image()
        .unwrap();
    let g = godgrader_core::scans::aufbereiten(
        img,
        &[godgrader_core::scans::Bereich {
            x: 0.05,
            y: 0.1,
            w: 0.4,
            h: 0.03,
        }],
    );
    std::fs::write(&a[3], godgrader_core::scans::jpeg_bytes(&g).unwrap()).unwrap();
}
