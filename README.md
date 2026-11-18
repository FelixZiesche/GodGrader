# GodGrader

GodGrader hilft beim Korrigieren handschriftlicher Klausuren. Eine KI liest die Scans, schreibt die Antworten ab und schlägt
für jedes Bewertungselement eine Punktzahl vor. **Die Entscheidung trifft immer die Lehrkraft.** Keine Note verlässt das
Programm, bevor Sie die Klausur freigegeben haben.

Gebaut für Wirtschaft und Recht am Gymnasium in Thüringen (Oberstufe mit Notenpunkten, Sek I mit Noten).

---

## Inhalt

1. [Bevor Sie anfangen](#1-bevor-sie-anfangen)
2. [Installieren](#2-installieren)
3. [API-Schlüssel einrichten](#3-api-schlüssel-einrichten)
4. [Modelle wählen](#4-modelle-wählen)
5. [Eine Klausur korrigieren: Schritt für Schritt](#5-eine-klausur-korrigieren-schritt-für-schritt)
6. [Kosten](#6-kosten)
7. [Datenschutz](#7-datenschutz)
8. [Häufige Fragen](#8-häufige-fragen)
9. [Für Entwickler](#9-für-entwickler)

---

## 1. Bevor Sie anfangen

- Klausuren sind personenbezogene Daten. Klären Sie den Einsatz mit echten Klausuren bitte **vorher mit dem
  Datenschutzbeauftragten Ihrer Schule** ab.
- Sie brauchen ein Konto bei [OpenRouter](https://openrouter.ai) mit etwas Guthaben. Über OpenRouter erreicht GodGrader
  die KI-Modelle. Eine Klausur kostet je nach Modell meist nur wenige Cent (siehe [Kosten](#6-kosten)).
- Sie brauchen:
  - den **Erwartungshorizont** als Word- oder PDF-Datei,
  - die **Scans** der Klausuren als PDF (ein PDF pro Schüler oder ein großes PDF mit allen Klausuren).

## 2. Installieren

1. Öffnen Sie die Seite [Releases](https://github.com/FelixZiesche/GodGrader/releases) und laden Sie die Datei
   `GodGrader_…_x64-setup.exe` herunter.
2. Starten Sie die Datei mit Doppelklick.
3. Windows zeigt eventuell „Der Computer wurde durch Windows geschützt“. Das liegt daran, dass das Programm nicht
   kostenpflichtig signiert ist. Klicken Sie auf **Weitere Informationen** und dann auf **Trotzdem ausführen**.
4. Folgen Sie dem Installationsassistenten. Sie brauchen **keine Administratorrechte**. GodGrader wird nur für Ihr
   Benutzerkonto installiert.

Beim ersten Start erscheint ein Hinweis zum Datenschutz. Lesen Sie ihn und klicken Sie auf **Verstanden**.

**Deinstallieren:** Windows-Einstellungen → Apps → GodGrader → Deinstallieren. Ihre Projekte bleiben dabei erhalten.
Wenn Sie alles entfernen wollen, löschen Sie zusätzlich den Ordner `%APPDATA%\GodGrader`.

## 3. API-Schlüssel einrichten

Der API-Schlüssel ist wie ein Passwort für Ihr OpenRouter-Konto.

1. Melden Sie sich bei [openrouter.ai](https://openrouter.ai) an.
2. Laden Sie unter **Credits** etwas Guthaben auf (5 bis 10 $ reichen für viele Klausuren).
3. Öffnen Sie **Keys** und klicken Sie auf **Create Key**. Ein Name wie „GodGrader“ genügt. Wenn Sie wollen, setzen
   Sie dort ein Ausgabenlimit.
4. Kopieren Sie den Schlüssel. Er beginnt mit `sk-or-`.
5. Öffnen Sie in GodGrader **Einstellungen**, fügen Sie den Schlüssel ein und klicken Sie auf **Speichern**.
6. Klicken Sie auf **Verbindung prüfen**. Steht dort „Verbindung funktioniert“, ist alles bereit.

Der Schlüssel liegt danach nur in der Windows-Anmeldeinformationsverwaltung, nie in einer Datei.

**Wichtig für den Datenschutz:** Öffnen Sie bei OpenRouter die Datenschutz-Einstellungen (**Settings → Privacy**) und
schalten Sie alles aus, was das Speichern von Eingaben oder das Training erlaubt. GodGrader verbietet den Anbietern bei jeder Anfrage zusätzlich, Daten zum Training zu
speichern.

## 4. Modelle wählen

Unter **Modelle** legen Sie fest, welches KI-Modell welche Aufgabe übernimmt. Dafür gibt es vier „Slots“:

| Slot | Aufgabe | Worauf achten |
|---|---|---|
| 1 · Abschreiben | Liest die Seitenbilder und schreibt die Handschrift ab. | Muss Bilder lesen können (Kennzeichen „Bild“). |
| 2 · Bewerten | Vergleicht die Abschrift mit dem Erwartungshorizont und schlägt BE vor. | Ein starkes Modell lohnt sich hier am meisten. |
| 3 · Feedback | Schreibt nach der Freigabe eine Rückmeldung für den Schüler. | Optional. |
| 4 · Zweitmeinung | Ein zweites Modell bewertet unabhängig. Bei Abweichungen wird das Element markiert. | Optional, kostet etwa doppelt so viel. |

So weisen Sie ein Modell zu: Ziehen Sie die Karte links auf einen Slot. Oder klicken Sie erst die Karte und dann den Slot.
Ein roter Punkt neben „Modelle“ bedeutet, dass ein gewähltes Modell nicht mehr verfügbar ist. Dann schlägt GodGrader
einen Ersatz vor, den Sie mit **Übernehmen** bestätigen.

Für den Anfang können Sie die Voreinstellung lassen. Mit **Standard wiederherstellen** kommen Sie jederzeit dorthin
zurück.

## 5. Eine Klausur korrigieren: Schritt für Schritt

Die Seiten in der linken Leiste folgen dem Ablauf von oben nach unten.

### Schritt 1: Projekt anlegen (Seite „Projekte“)

Ein Projekt ist eine Klausur für eine Lerngruppe, zum Beispiel „11.1 Soziale Marktwirtschaft“.

1. Klicken Sie auf **Neues Projekt**.
2. Tragen Sie Titel, Lerngruppe und Datum ein und wählen Sie **Oberstufe** oder **Sek I**.
3. Weitere Einstellungen:
   - **halbe BE erlaubt:** wenn Sie auch 0,5 BE vergeben.
   - **Bereiche vor dem Senden schwärzen:** empfohlen. Dann wird zum Beispiel die Namenszeile überdeckt, bevor ein Bild an die KI geht.
   - **Budget:** Höchstbetrag für die KI in diesem Projekt. Ist er erreicht, hält die Verarbeitung an.
   - **Warnschwelle:** Ab welchem Anteil schlechter Noten (Oberstufe unter 5 NP, Sek I Noten 5 und 6) die Statistik warnt.

### Schritt 2: Erwartungshorizont (Seite „Erwartungshorizont“)

1. Klicken Sie auf **Erwartungshorizont importieren** und wählen Sie Ihre Word- oder PDF-Datei. Sie können die Datei auch in das Fenster ziehen.
2. GodGrader liest Aufgaben, Bewertungselemente, BE, Anforderungsbereiche und den Notenschlüssel aus.
3. Prüfen Sie das Ergebnis. Alles ist bearbeitbar. Hinweise zeigen, wo etwas fehlt oder nicht zusammenpasst
   (zum Beispiel wenn die BE einer Aufgabe nicht zur Summe der Elemente passen).
4. Steht bei einer Aufgabe nur ein Fließtext ohne einzelne Elemente, kann **KI-Hilfe** Elemente vorschlagen. Sie
   übernehmen oder verwerfen den Vorschlag.

### Schritt 3: Scans einlesen (Seite „Scans“)

1. **Pseudonymliste (optional):** Wenn Sie mit Pseudonymen arbeiten, laden Sie eine Liste (CSV oder Excel) oder tippen
   Sie die Namen ein. Sie können jede Klausur aber auch frei benennen, zum Beispiel „Klausur 1“.
   Die Bezeichnung wird **nie** an die KI geschickt.
2. **Schwärzen:** Ziehen Sie auf der Beispielseite ein Rechteck über die Namenszeile. Dieser Bereich wird auf allen
   Seiten überdeckt.
3. **Klausuren importieren:**
   - *Ein PDF pro Schüler:* Ziehen Sie die PDFs in das Fenster oder wählen Sie sie aus.
   - *Stapel-PDF:* Ein PDF mit allen Klausuren. Sie sehen alle Seiten klein und klicken zwischen zwei Seiten, um dort
     eine neue Klausur beginnen zu lassen.
   - Bei sehr dicht beschriebenen Seiten kann jede Seite in eine obere und untere Hälfte geteilt werden. Die KI liest
     dann genauer, es kostet aber etwas mehr.
4. Vergeben Sie für jede Klausur eine Bezeichnung und klicken Sie auf **Klausuren importieren**.

### Schritt 4: KI-Vorkorrektur (Seite „Verarbeitung“)

1. Wählen Sie die Klausuren aus (Standard: alle noch nicht fertigen).
2. Klicken Sie auf **Klausuren verarbeiten**. Sie sehen zuerst eine **Kostenschätzung**. Erst mit **Starten** geht es los.
3. Die KI schreibt zuerst ab und bewertet dann Aufgabe für Aufgabe. Der Fortschritt wird laufend angezeigt.
4. **Anhalten** startet nichts Neues mehr. **Abbrechen** beendet sofort. In beiden Fällen macht ein neuer Start dort
   weiter, wo es aufgehört hat. Fertige Schritte werden nie doppelt bezahlt.

Sie können die App während der Verarbeitung offen lassen und nebenbei schon prüfen.

### Schritt 5: Prüfen und freigeben (Seite „Prüfen“)

Hier entscheiden Sie. Die Ansicht hat drei Spalten:

- **links** das Seitenbild (Zoom mit Strg + Mausrad, Verschieben mit der Maus),
- **Mitte** die Abschrift der gewählten Aufgabe (Fehler der KI können Sie direkt korrigieren),
- **rechts** die Bewertungselemente mit dem KI-Vorschlag, der Begründung und dem Beleg aus der Antwort.

**Rot markiert** sind unsichere Stellen: Die KI war sich nicht sicher, die Handschrift war schwer lesbar oder die
Zweitmeinung weicht ab. Mit **nur Unsicheres** sehen Sie nur diese.

Schnell mit der Tastatur:

| Taste | Wirkung |
|---|---|
| J / K | nächstes / voriges Element |
| 0 bis 9 | BE setzen |
| , oder . | +0,5 BE (wenn halbe BE erlaubt sind) |
| Enter | Wert bestätigen und zum nächsten offenen Element |

Jedes Element muss bestätigt werden. Danach klicken Sie auf **Freigeben**. Erst freigegebene Klausuren erscheinen in
Statistik und Export. Mit **Freigabe zurücknehmen** können Sie später noch etwas ändern. Jede Änderung wird im
**Änderungsprotokoll** festgehalten.

Hat die KI eine Aufgabe falsch verstanden, korrigieren Sie die Abschrift und klicken Sie auf **Diese Aufgabe neu bewerten**.

### Schritt 6: Statistik (Seite „Statistik“)

Zeigt für alle freigegebenen Klausuren: Durchschnitt, Median, Spannweite, Notenspiegel und Lösungsquoten je Aufgabe,
Anforderungsbereich und Bewertungselement. Dazu kommen die drei schwächsten Elemente und wie oft Sie einen KI-Vorschlag
geändert haben. Liegt der Anteil schlechter Noten über Ihrer Warnschwelle, erscheint ein roter Hinweis.

### Schritt 7: Rückmeldungen und Export (Seite „Export“)

1. **Rückmeldungen (optional):** **Fehlende erzeugen** lässt die KI für jede freigegebene Klausur eine Rückmeldung
   schreiben: 2 bis 3 Stärken, 2 bis 3 Entwicklungsfelder und einen Tipp (Oberstufe mit „Sie“, Sek I mit „du“). Sie
   können jeden Text frei ändern. Er wird gespeichert, sobald Sie das Feld verlassen.
2. Wählen Sie, was exportiert werden soll:
   - **Notenliste** als Excel (mit einem Blatt „Statistik“) und als CSV
   - **Korrekturbogen** je Klausur (Word): Tabelle mit Aufgabe, max. BE, erreichten BE und Kurzbegründung, Raster für den
     AFB III, Summe, NP bzw. Note und am Ende die Rückmeldung
   - **Alle Korrekturbögen in einer Datei** zum Drucken
   - **Rückmeldung** je Klausur (Word)
   - **Statistik** (Word)
3. Klicken Sie auf **Ordner wählen …** und dann auf **Exportieren**. Mit **Ordner öffnen** sehen Sie die Dateien.

Exportiert werden nur freigegebene Klausuren. Fehlen welche, nennt GodGrader sie.

## 6. Kosten

- Vor jedem Start sehen Sie eine Schätzung. Nach den ersten Klausuren rechnet GodGrader mit den echten Kosten
  und wird damit genauer.
- Auf der Seite **Verarbeitung** steht, was das Projekt bisher gekostet hat. Im **Kostenprotokoll** steht jeder einzelne
  Aufruf.
- Mit einem **Budget** im Projekt hält die Verarbeitung an, bevor es überschritten wird. Erhöhen Sie es und starten Sie
  erneut, dann geht es weiter.
- Grober Richtwert: Mit günstigen Modellen kostet eine Klausur mit 6 Seiten meist zwischen 2 und 10 Cent, mit den stärksten
  Modellen auch 20 bis 50 Cent.

## 7. Datenschutz

- **Alles bleibt auf Ihrem Rechner.** Projekte, Scans und Ergebnisse liegen in `%APPDATA%\GodGrader`. Es gibt keine
  Cloud, keine Telemetrie und keine automatischen Updates. GodGrader verbindet sich nur mit OpenRouter.
- **Was an die KI geht:**
  - Zum Abschreiben gehen nur die Seitenbilder (auf Wunsch mit geschwärzter Namenszeile) und die Aufgabennummern,
    **kein** Erwartungshorizont.
  - Zum Bewerten gehen nur Text (Abschrift und Erwartungshorizont), **keine** Bilder.
  - Name oder Pseudonym gehen **nie** mit.
- Jede Anfrage verbietet den Anbietern, Daten zu speichern oder zum Training zu nutzen. Unter **Einstellungen** können
  Sie zusätzlich **Nur Anbieter ohne Datenspeicherung (ZDR)** einschalten.
- **Projekt löschen** (Seite „Projekte“) entfernt alle Dateien des Projekts von der Festplatte.
- GodGrader schreibt keine Protokolldateien mit Inhalten aus Klausuren.

Hinweis: In manchen Schulnetzen wird der Ordner `%APPDATA%` mit einem Server abgeglichen („servergespeichertes Profil“).
Fragen Sie im Zweifel Ihren Administrator, wenn Sie GodGrader auf einem Schulrechner nutzen.

## 8. Häufige Fragen

**Die KI hat eine Seite falsch gelesen.** Korrigieren Sie die Abschrift in der Prüfansicht und klicken Sie auf
**Diese Aufgabe neu bewerten**.

**Eine Klausur zeigt „Fehler“.** Lesen Sie auf der Seite „Verarbeitung“ die rote Zeile unter der Klausur. Meist hilft es, einfach erneut zu starten. Nur der fehlgeschlagene Schritt läuft dann noch einmal.

**„Kein API-Schlüssel“ oder „Schlüssel ungültig“.** Tragen Sie den Schlüssel unter **Einstellungen** neu ein und
klicken Sie auf **Verbindung prüfen**.

**„Budget erreicht“.** Erhöhen Sie das Budget unter **Projekte → Bearbeiten** und starten Sie erneut.

**Kann ich die Anweisungen an die KI ändern?** Ja, unter **Einstellungen**, Abschnitt **Prompt-Vorlagen**. Mit
**Vorlage zurücksetzen** kommen Sie zur Originalfassung zurück.

**Ich habe die Freigabe zu früh erteilt.** In der Prüfansicht auf **Freigabe zurücknehmen** klicken.

---

## 9. Für Entwickler

Tauri 2 (Rust) und Svelte 5 mit TypeScript. Zuerst für Windows, der Code ist plattformneutral.

- `crates/godgrader-core`: Fachlogik ohne Tauri (Erwartungshorizont, Scans, OpenRouter, Pipeline, Prüfung, Exporte)
- `src-tauri`: dünne Tauri-Schicht (Commands)
- `src`: Oberfläche
- `docs/ENTSCHEIDUNGEN.md`: getroffene Entscheidungen
- `samples/`: Testdateien, keine echten Schülerarbeiten (`samples/echt/` und `projekte/` sind in `.gitignore`)

```bash
npm ci
bash scripts/pdfium-holen.sh   # PDFium nach vendor/pdfium
npm run tauri dev              # App starten
cargo test --workspace         # Tests (OpenRouter wird mit wiremock simuliert)
```

GitHub Actions baut bei jedem Push den Windows-Installer (Artefakt „GodGrader-Windows-Installer“). Bei einem Tag
`v*` wird der Installer zusätzlich als Release veröffentlicht.
