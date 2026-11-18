# Entscheidungen

Kleinere Entscheidungen, die während der Entwicklung getroffen wurden. Neueste unten.

## Meilenstein 1 – Gerüst

| Thema | Entscheidung | Begründung |
|---|---|---|
| Name | Die App heißt **GodGrader** (Auftrag sprach von „GodKorrektur“). Crate `godgrader-core`, Header `X-Title: GodGrader`, Datenordner `%APPDATA%\GodGrader`. | Wunsch von Felix. |
| Repo | `FelixZiesche/godgrader` (privat). | Arbeitsumgebung von Claude ist flüchtig, daher Remote nötig. |
| Windows-Installer | wird über **GitHub Actions** (Windows-Runner) gebaut, nicht lokal. | Felix muss nichts installieren; Entwicklung läuft unter Linux. |
| Datenordner | `data_dir()/GodGrader` statt Tauri-`app_data_dir()` (das wäre `%APPDATA%\de.felixziesche.godgrader`). | Auftrag verlangt `%APPDATA%\GodGrader\…`. |
| Speicherung | Einstellungen als JSON mit atomarem Schreiben (temporäre Datei + Umbenennen). Für Projekte ab M2/M4 **eine SQLite-Datenbank pro Projekt** (`rusqlite`, gebündelt) plus Ordner für Bilder. | SQLite-Transaktionen machen jeden Pipeline-Schritt einzeln absturzsicher und fortsetzbar. |
| Schlüsselbund | `keyring` **3.x** (nicht 4.x). | 4.x hat ein neues Store-Konzept, 3.x ist erprobt und deckt Windows/Linux/macOS ab. Für Android wird später ein eigener `SecretStore` ergänzt (Trait ist schon vorhanden). |
| HTTP | `reqwest` 0.13 mit rustls. | Wie empfohlen, keine OpenSSL-Abhängigkeit unter Windows. |
| Datenschutz-Felder | Jede Chat-Anfrage enthält `provider.data_collection = "deny"`; mit der Einstellung „Nur Anbieter ohne Datenspeicherung (ZDR)“ zusätzlich `provider.zdr = true`. Der Client **verweigert** Chat-Anfragen ohne `data_collection: "deny"`. | OpenRouter-Doku „Provider Routing“, Stand 11/2026. Doppelte Absicherung im Code. |
| Verbindung prüfen | nutzt `GET /api/v1/key` (zeigt Verbrauch und Limit, kostet nichts). | Keine Klausurdaten, keine Kosten. |
| Wiederholungen | exponentieller Backoff ab 1 s (max. 120 s), `Retry-After` wird beachtet, Standard 4 Wiederholungen. Nur bei 429, 5xx und Verbindungsfehlern. | |
| Drag & Drop der Modellkarten | eigene Umsetzung mit Zeiger-Ereignissen statt HTML5-Drag-and-Drop. Zusätzlich: Karte anklicken, dann Slot anklicken. | Tauris natives Datei-Drop (für PDFs in M3, liefert echte Dateipfade) blockiert unter Windows HTML5-Drag-and-Drop. So funktioniert beides. |
| Slug-Prüfung | Fehlt ein Standardmodell, wird das neueste Modell desselben Anbieters mit denselben Familien-Wörtern vorgeschlagen (z. B. `gemini` + `pro`, `claude` + `opus`); feste Slugs vor `~`-Aliasen. Abschreiben nur mit Bildmodellen. | |
| Schrift | `'Google Sans', 'Google Sans Flex', system-ui, sans-serif`; **keine Schriftdatei gebündelt**. | Lizenz von Google Sans erlaubt das Bündeln nicht. |
| Netzwerk in der Oberfläche | Die Weboberfläche darf laut CSP nur mit dem Rust-Teil sprechen; alle OpenRouter-Aufrufe laufen im Rust-Teil. | Keine Netzwerkaufrufe außer zu OpenRouter, Schlüssel bleibt im Backend. |
| Entwicklungstests | Keine echten OpenRouter-Aufrufe während der Entwicklung, nur `wiremock`. Die Adresse lässt sich für Tests über `GODGRADER_OPENROUTER_URL` umstellen. | |
| Installer | NSIS, Sprache Deutsch, Installation pro Benutzer (keine Adminrechte nötig). | Schulrechner/Dienstlaptops haben oft keine Adminrechte. |

## Erwartungshorizont (Vorbereitung M2)

| Thema | Entscheidung | Begründung |
|---|---|---|
| Formate | Import aus **Word (.docx) und PDF**. | Wunsch von Felix. Word liefert Fettdruck und Tabellenstruktur zuverlässiger; PDF wird über Textlage rekonstruiert und landet immer in der Prüfansicht. |
| BE-Angaben | Neben „(x BE)“ am Elementende werden auch BE in Tabellenköpfen („Indikator (1 BE)“) und Sammelangaben („je 1 BE, 6 BE“) erkannt. | So sieht der echte Beispiel-Erwartungshorizont 11.1 aus. |
| Pseudonyme | **Optional statt Pflicht** (Wunsch von Felix, 18.11.2026). Jede Klausur hat eine frei wählbare Bezeichnung (Pseudonym aus der Liste, Nummer oder Name). Die Pseudonymliste ist ein optionaler Import. Die Bezeichnung wird nie an OpenRouter gesendet; Schwärzen bleibt. In Exporten heißt das Feld weiter „Pseudonym:“, wenn eine Pseudonymliste genutzt wird, sonst „Name:“. | Änderung gegenüber Auftrag Abschnitt 2.1. |
| Schwärzen | **Optional, pro Projekt schaltbar** (Wunsch von Felix, 18.11.2026). Ist es aus, erscheint vor der ersten Übertragung ein kurzer Hinweis, aber nichts wird gesperrt. | Änderung gegenüber Auftrag Abschnitt 2.2. |

## Meilenstein 2 – Erwartungshorizont

| Thema | Entscheidung | Begründung |
|---|---|---|
| Zwischenform | Word und PDF werden zuerst in dieselbe Zwischenform (Absätze mit fett/kursiv, Tabellen mit grau hinterlegten Zellen) übersetzt; danach arbeitet **ein** godtest-Parser. | Beide Formate liefern dasselbe Ergebnis; Tests prüfen das am Nachbau des Erwartungshorizonts 11.1. |
| PDF-Tabellen | Tabellen werden aus den Linien erkannt (Word zeichnet sie als dünne gefüllte Rechtecke in `A6A6A6`), der graue Kopf (`E6E6E6`) aus Flächen. Tabellen über einen Seitenumbruch werden zusammengeführt, wiederholte Kopfzeilen entfernt. | Funktioniert mit „Als PDF speichern“ aus Word. Eingescannte PDFs werden abgelehnt (kein Text). |
| PDF-Bibliothek | PDFium **chromium/8125** (passend zu `pdfium-render` 0.9, Feature `pdfium_8125`), wird mit dem Installer ausgeliefert (`resources/pdfium`). Skript: `scripts/pdfium-holen.sh`. | Wird in M3 ohnehin für das Rendern der Scans gebraucht. |
| Bewertungselemente | Ein Element entsteht aus „(x BE)“ im Absatz, aus Tabellen mit „(x BE)“ im Spaltenkopf (eine Zelle je Zeile und BE-Spalte), aus Rasterzeilen und aus „je“-Angaben: „drei Ziele, je Ziel Indikator (1 BE) und Zielerreichung (1 BE)“ → zwei Elemente mit je 3 BE; die Beispieltabelle hängt als „Mögliche Lösungen“ daran. „(je 1 BE, 6 BE)“ → ein Element mit 6 BE. Ein Absatz wie „Kein vollkommener Wettbewerb (2 BE):“ mit passenden Unterpunkten wird zur Überschrift. „(je 1 BE)“ ohne Anzahl gilt als Hinweis, nicht als Element. | So ergibt der echte Erwartungshorizont 11.1 genau 31 Elemente mit den richtigen Summen. |
| Titel der Elemente | fetter Anfang („**Formel:**“, „**Vergleich** (1 BE)“), sonst der Text vor der BE-Angabe, gekürzt auf etwa 65 Zeichen. | |
| Musterlösung | Die vollständige Lösung einer Aufgabe (auch Hinweise ohne BE, z. B. „Ebenfalls voll richtig: …“) wird als Markdown gespeichert und später dem Bewerten-Modell mitgegeben. | Nichts aus dem Erwartungshorizont geht verloren. |
| Material in Aufgaben | Tabellen ohne BE und „Quelle: …“ direkt nach dem Aufgabentext gehören zum Aufgabentext (z. B. Fälle A–C in 1b). Eine These „…“ direkt vor einer Aufgabe ebenfalls. | |
| AFB | Aus der AFB-Übersicht je Aufgabe, auch geteilt (Aufgabe 6: 2 BE AFB II „Fachbegriffe“, 12 BE AFB III). Elemente erben den AFB der Aufgabe; „(AFB II)“ im Elementtext hat Vorrang. | Für die Statistik je AFB in M6. |
| Notenschlüssel | Exakt in Ganzzahlen gerechnet (kein Gleitkomma-Rundungsfehler bei x,5). Unit-Tests mit allen Prüfwerten (30, 50 BE) und dem Schlüssel aus 11.1 (52 BE). | |
| Speichern | Änderungen im Editor werden nach 0,6 s automatisch gespeichert, Prüfungen laufen bei jeder Änderung. Element-IDs (`2a-1` …) werden beim Speichern neu vergeben. | Kein Datenverlust durch vergessenes Speichern. |
| Projekte | Ein Ordner pro Projekt mit `projekt.sqlite` (WAL-Modus). Projekt-IDs sind UUIDs; Pfade werden gegen Manipulation geprüft. „Löschen“ entfernt den ganzen Ordner. | |
| Testdateien | `samples/generator/bauen.js` erzeugt Word-Testdateien mit den Bausteinen aus godtest: Nachbau 11.1 (Oberstufe, 52 BE) und eine erfundene Klassenarbeit Klasse 9 (Sek I, 50 BE, 5 Aufgaben, mit und ohne Notenschlüssel). | Parser ohne echte Dateien testbar. |
| KI-Hilfe beim Import | kommt mit Meilenstein 4, weil sie den OpenRouter-Chat-Aufruf braucht. | |
