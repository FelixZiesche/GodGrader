// Erzeugt Word-Testdateien im godtest-Format für die Parser-Tests.
// Aufruf: npm install && npm run build  (Ausgabe in ../)
const k = require('./klausur_bausteine.js');
const path = require('path');
const out = (n) => path.join(__dirname, '..', n);

// ---------- 1) Nachbau des Erwartungshorizonts 11.1 (Oberstufe, 52 BE) ----------
async function oberstufe() {
  const KOPF = { thema: 'Soziale Marktwirtschaft im Wandel – Ziele, Marktversagen und Strukturwandel', datum: '24.11.2026', sj: 'SJ 2026/27' };
  const AUFGABEN = [{ nr: '1a', afb: 1, be: 12 }, { nr: '1b', afb: 2, be: 6 }, { nr: '2', afb: 2, be: 4 }, { nr: '3', afb: 2, be: 4 }, { nr: '4', afb: 2, be: 6 }, { nr: '5', afb: 2, be: 6 }, { nr: '6', afb: 3, be: 14 }];
  const THESE = 'Den KI-Wandel sollte der Staat dem Markt überlassen.';
  await k.speichern([
    ...k.kopf({ titel: '11.1 Klausur im Fach Wirtschaft und Recht – Erwartungshorizont', ...KOPF, zusatz: '**Variante B.** Andere sachlich richtige Lösungen sind gleichwertig zu bewerten.' }),
    ...k.aufgabe('1a', ['Nennen Sie die sechs Ziele des *magischen* Sechsecks.', 'Beschreiben Sie für drei Ziele den Indikator und wann das Ziel erreicht ist.'], 12),
    k.loesung('**Ziele des magischen Sechsecks** (je 1 BE, 6 BE): **stabiles** Preisniveau, **hoher** Beschäftigungsstand, **außenwirtschaftliches** Gleichgewicht, **stetiges und angemessenes Wirtschaftswachstum**, Erhalt einer lebenswerten Umwelt, gerechte Einkommens- und Vermögensverteilung.'),
    k.loesung('**Indikatoren und Zielerreichung** – drei Ziele, je Ziel Indikator (1 BE) und Zielerreichung (1 BE), 6 BE, z. B.:'),
    k.tabelle(['Ziel', 'Indikator (1 BE)', 'Ziel erreicht, wenn … (1 BE)'], [
      ['Preisniveaustabilität', 'Inflationsrate (VPI bzw. EU-weit HVPI)', 'Inflationsrate mittelfristig bei 2 %'],
      ['hoher Beschäftigungsstand', 'Arbeitslosenquote', 'Quote unter etwa 3 bis 4 %'],
      ['Wirtschaftswachstum', 'reales BIP, Veränderung gegenüber dem Vorjahr', 'oft 2 bis 3 %, stetig'],
    ], { anteile: [25, 40, 35], ersteSpalteFett: true }),
    k.loesung('*Maßstab sind die Richtwerte der Zusammenfassung „Das magische Sechseck“.*'),
    ...k.aufgabe('1b', ['Analysieren Sie die Fälle A–C: Bestimmen Sie die zwei betroffenen Ziele und ihre Beziehung zueinander. Begründen Sie Ihre Einschätzung. *(Tabelle)*'], 6),
    k.tabelle(['Fall', 'Situation'], [['A', 'Der Staat senkt die Energiesteuer.'], ['B', 'Die Exporte in die USA sanken um 9,4 %.'], ['C', 'Das BIP wuchs, die Emissionen sanken.']], { anteile: [10, 90] }),
    k.quelle('nach Destatis, 2026'),
    k.tabelle(['Fall', 'betroffene Ziele und Zielbeziehung (1 BE)', 'Begründung (1 BE)'], [
      ['A', 'Preisniveaustabilität und Umwelt: Zielkonflikt', 'Kraftstoffe sind Teil des Warenkorbs …'],
      ['B', 'außenwirtschaftliches Gleichgewicht und Wachstum: Zielkonflikt', 'Der Exportüberschuss sinkt …'],
      ['C', 'Wachstum und Umwelt: Zielharmonie oder Zielkonflikt', 'Entkopplung durch erneuerbare Energien …'],
    ], { anteile: [10, 40, 50] }),
    ...k.aufgabe('2', ['Berechnen Sie die Arbeitslosenquote in Thüringen (M1). *(Formel, Rechenweg, Lösung)*', 'Vergleichen Sie Ihr Ergebnis mit dem Richtwert für einen *hohen* Beschäftigungsstand.', 'Erklären Sie anhand von M1, warum es zugleich Arbeitslose und offene Stellen gibt.'], 4),
    k.loesung('**Formel:** Arbeitslosenquote = Arbeitslose ÷ (Erwerbstätige + Arbeitslose) × 100 (1 BE)'),
    k.loesung('**Rechenweg und Lösung:** 68.681 ÷ 1.091.946 × 100 ≈ **6,3 %** (1 BE)'),
    k.loesung('*Ebenfalls voll richtig: Rechnung mit den Erwerbstätigen 2025, ≈ 6,4 %.*'),
    k.loesung('**Vergleich** (1 BE): 6,3 % liegen deutlich über dem Richtwert für Vollbeschäftigung.'),
    k.loesung('**Erklärung** (1 BE): **strukturelle Arbeitslosigkeit (Mismatch)** – die Qualifikation passt nicht zu den offenen Stellen.'),
    ...k.aufgabe('3', ['Erläutern Sie an zwei Merkmalen, inwiefern KI eine *Basisinnovation* ist (M2, M3).'], 4),
    k.loesung('**Zwei** Merkmale, je Merkmal Erläuterung am Beispiel KI (1 BE) und Materialbeleg (1 BE), z. B.:'),
    k.tabelle(['Merkmal', 'Erläuterung am Beispiel KI (1 BE)', 'Beleg aus dem Material (1 BE)'], [
      ['Querschnittstechnik', 'in fast allen Branchen einsetzbar', 'M3: 5 % → 25 % der Betriebe'],
      ['Produktivitätssprung', 'Aufgaben werden schneller erledigt', 'M3: 40 % schneller'],
    ], { anteile: [25, 40, 35], ersteSpalteFett: true }),
    k.loesung('*Der Bezug auf den Begriff gehört zur Erläuterung (je 1 BE).*'),
    ...k.aufgabe('4', ['Ordnen Sie die Maßnahmen A–C aus M4 jeweils einer Art der *Strukturpolitik* zu. Begründen Sie Ihre Zuordnung. *(Tabelle)*'], 6),
    k.tabelle(['Maßnahme', 'Art (1 BE)', 'Begründung (1 BE)'], [
      ['A Investitionsförderung', 'erhaltend', 'sichert bestehende Arbeitsplätze'],
      ['B Transformationsteam', 'anpassend', 'federt den Wandel sozial ab'],
      ['C Luft- und Raumfahrt', 'gestaltend', 'baut eine Zukunftsbranche auf'],
    ], { anteile: [30, 20, 50] }),
    ...k.aufgabe('5', ['Erklären Sie anhand von M5, warum auf dem KI-Markt kein *vollkommener Wettbewerb* herrscht.', 'Erläutern Sie an zwei Instrumenten des GWB, wie der Staat das *Wettbewerbsprinzip* durchsetzen kann (M5).'], 6),
    k.loesung('**Kein vollkommener Wettbewerb** (2 BE):'),
    k.punkt('Beim **vollkommenen Wettbewerb** gibt es viele Anbieter; am KI-Markt fehlen **viele Anbieter** (Oligopol, 61 %, M5). (1 BE)'),
    k.punkt('**Grund mit Beleg aus** M5, z. B. **hohe Fixkosten** als Markteintrittsschranke. (1 BE)'),
    k.loesung('**Wettbewerbsprinzip und GWB** (4 BE) – zwei Instrumente, je Instrument Funktionsweise (1 BE) und Anwendung auf den KI-Markt mit Bezug zu M5 (1 BE), z. B.:'),
    k.tabelle(['Instrument', 'Funktionsweise (1 BE)', 'Anwendung auf den KI-Markt (1 BE)'], [
      ['Fusionskontrolle', 'Zusammenschlüsse prüft das Bundeskartellamt', 'Übernahme eines KI-Start-ups'],
      ['Missbrauchsaufsicht', 'Marktmacht darf nicht ausgenutzt werden', '§ 19a GWB gegen Microsoft'],
      ['Kartellverbot', 'Absprachen sind verboten', 'Preisabsprachen bei Cloud-Rechenleistung'],
    ], { anteile: [25, 35, 40] }),
    k.zitat(THESE),
    ...k.aufgabe('6', ['Erörtern Sie diese Aussage mithilfe der Materialien (M1–M6). *(Argumente dafür und dagegen, begründetes Urteil)*'], 14),
    k.raster([
      { kriterium: ['klärt die Fachbegriffe (AFB II), z. B.:', '• **Strukturwandel:** dauerhafte Verschiebung der Wirtschaftsstruktur'], max: 2 },
      { kriterium: ['arbeitet Argumente heraus, die die Aussage stützen, mit Materialbelegen, z. B.:', '• Marktkonformitätsprinzip'], max: 4 },
      { kriterium: ['arbeitet Argumente heraus, die die Aussage relativieren oder widerlegen, z. B.:', '• Sozialprinzip'], max: 3 },
      { kriterium: ['stellt Zusammenhänge her (wirtschaftlich, politisch, rechtlich, sozial), z. B.:', '• Zielkonflikte im magischen Sechseck'], max: 2 },
      { kriterium: ['Fazit: gelangt zu einem eigenständigen, abgewogenen und begründeten Urteil'], max: 2 },
      { kriterium: ['Methodenkompetenz: belegt Aussagen konkret mit Zahlen und Angaben aus den Materialien'], max: 1 },
    ], 'Anforderungsbereiche II und III'),
    k.text('AFB', { b: true, vor: 360, keepNext: true }),
    k.tabelle(['', 'AFB I', 'AFB II', 'AFB III', 'gesamt'], [
      ['Aufgaben', '1a', '1b, 2, 3, 4, 5, 6 (Fachbegriffe)', '6', ''],
      ['BE', '12', '6 + 4 + 4 + 6 + 6 + 2', '12', '52'],
      ['Anteil', '23 %', '54 %', '23 %', '100 %'],
    ], { anteile: [16, 21, 21, 21, 21], ersteSpalteFett: true }),
    k.text('Notenschlüssel', { b: true, vor: 360, keepNext: true }),
    k.notenTabelle(52),
  ], out('2_Erwartungshorizont_WR_11.1_Nachbau.docx'));
}

// ---------- 2) Erfundene Sek-I-Klassenarbeit, 50 BE, 5 Aufgaben ----------
async function sek1(mitNotenschluessel) {
  const KOPF = { thema: 'Geld und Konsum', datum: '15.03.2027', sj: 'SJ 2026/27', gruppe: 'Klasse' };
  const AUFGABEN = [{ nr: '1', afb: 1, be: 8 }, { nr: '2', afb: 2, be: 10 }, { nr: '3', afb: 2, be: 8 }, { nr: '4', afb: 2, be: 9 }, { nr: '5', afb: 3, be: 15 }];
  k.pruefeBE(AUFGABEN, 50);
  const doc = [
    ...k.kopf({ titel: 'Klassenarbeit Klasse 9 im Fach Wirtschaft und Recht – Erwartungshorizont', ...KOPF, zusatz: 'Andere sachlich richtige Lösungen sind gleichwertig zu bewerten.' }),
    ...k.aufgabe(1, ['Nenne vier Funktionen des Geldes und beschreibe zwei davon.'], 8),
    k.loesung('**Funktionen** (je 1 BE, 4 BE): Tauschmittel, Wertaufbewahrungsmittel, Recheneinheit, Wertübertragungsmittel'),
    k.loesung('**Beschreibung** – zwei Funktionen, je Funktion Beschreibung (2 BE), z. B.:'),
    k.tabelle(['Funktion', 'Beschreibung (2 BE)'], [['Tauschmittel', 'Geld wird gegen Waren getauscht.'], ['Recheneinheit', 'Preise machen Güter vergleichbar.'], ['Wertaufbewahrung', 'Geld kann gespart werden.']], { anteile: [30, 70] }),
    ...k.aufgabe(2, ['Berechne die Inflationsrate. *(Formel, Rechenweg, Lösung)* Erkläre, was sie für dein Taschengeld bedeutet.'], 10),
    k.loesung('**Formel:** Inflationsrate = (VPI neu − VPI alt) ÷ VPI alt × 100 (2 BE)'),
    k.loesung('**Rechenweg:** (121,2 − 117,6) ÷ 117,6 × 100 (2 BE)'),
    k.loesung('**Lösung:** ≈ **3,1 %** (2 BE)'),
    k.loesung('**Erklärung** (4 BE): Die Kaufkraft sinkt; für dasselbe Taschengeld bekommt man weniger.'),
    ...k.aufgabe(3, ['Vergleiche Barzahlung und Kartenzahlung anhand von zwei Merkmalen. *(Tabelle + Fazit)*'], 8),
    k.tabelle(['Merkmal', 'Barzahlung (1 BE)', 'Kartenzahlung (1 BE)'], [['Überblick über Ausgaben', 'gut sichtbar', 'leicht zu verlieren'], ['Sicherheit', 'Verlust endgültig', 'Karte sperrbar']], { anteile: [30, 35, 35] }),
    k.loesung('**Fazit** (4 BE): begründete Abwägung, z. B. Kartenzahlung bequem, Barzahlung hilft beim Sparen.'),
    ...k.aufgabe(4, ['Ordne die Fälle A–C einer Vertragsart zu und begründe.'], 9),
    k.tabelle(['Fall', 'Vertragsart (1 BE)', 'Begründung (2 BE)'], [['A', 'Kaufvertrag', 'Ware gegen Geld'], ['B', 'Mietvertrag', 'Gebrauch auf Zeit gegen Entgelt'], ['C', 'Werkvertrag', 'Erfolg geschuldet']], { anteile: [10, 30, 60] }),
    k.zitat('Wer jung ist, braucht kein Sparbuch.'),
    ...k.aufgabe(5, ['Nimm Stellung zu dieser Aussage. *(Argumente dafür und dagegen, begründetes Urteil)*'], 15),
    k.raster([
      { kriterium: ['klärt die Fachbegriffe (AFB II):', '• **Sparen:** Verzicht auf Konsum heute'], max: 2 },
      { kriterium: ['nennt Argumente für die Aussage'], max: 4 },
      { kriterium: ['nennt Argumente gegen die Aussage'], max: 4 },
      { kriterium: ['Fazit: begründetes eigenes Urteil'], max: 3 },
      { kriterium: ['Darstellung: sachlich, zusammenhängend'], max: 2 },
    ]),
    k.text('AFB', { b: true, vor: 360, keepNext: true }),
    k.afbTabelle(AUFGABEN),
  ];
  if (mitNotenschluessel) {
    doc.push(k.text('Notenschlüssel', { b: true, vor: 360, keepNext: true }), k.notenTabelleSek1(50));
  }
  await k.speichern(doc, out(mitNotenschluessel ? '2_Erwartungshorizont_Klasse9_Geld.docx' : '2_Erwartungshorizont_Klasse9_Geld_ohne_Notenschluessel.docx'));
}

(async () => {
  await oberstufe();
  await sek1(true);
  await sek1(false);
  console.log('fertig');
})();
