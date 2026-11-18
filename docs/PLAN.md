# Plan

1. **Gerüst** – Workspace, Tauri-App, Navigation, Einstellungen, API-Schlüssel im Schlüsselbund, Modellbibliothek, Drag-&-Drop-Slots mit Standardmodellen und Slug-Prüfung, „Verbindung prüfen“.
2. **Erwartungshorizont** – Parser (Word und PDF, godtest-Format), Prüfansicht mit Editor, Validierung, Notenschlüssel-Berechnung mit Tests, Projekte anlegen.
3. **Scans** – beide Importmodi, Rendering (pdfium, 300 dpi, Graustufen), Schwärzen, Pseudonymliste und -zuordnung.
4. **Pipeline** – Abschreiben, Bewerten, Zweitmeinung, Kosten, Budget, Fortsetzen; Fake-OpenRouter-Tests inkl. Trennungstest.
5. **Prüfansicht** – drei Spalten, Tastatur, Freigabe, Änderungsprotokoll.
6. **Feedback und Exporte** – Notenliste (xlsx/csv), Korrekturbogen, Feedback, Statistik (docx).
7. **Paket** – NSIS-Installer über GitHub Actions, README.

Nach jedem Meilenstein: `cargo fmt`, `cargo clippy -- -D warnings`, Tests, Commit, Statusbericht.
