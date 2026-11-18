// klausur_bausteine.js – Bausteine für Klausur und Erwartungshorizont (Skill godtest, docx-js 9.x)
const fs = require('fs');
const {
  Document, Packer, Paragraph, TextRun, Tab, Table, TableRow, TableCell, ImageRun, PageBreak,
  WidthType, BorderStyle, ShadingType, AlignmentType, TabStopType, LevelFormat, VerticalAlign, LineRuleType,
} = require('docx');

const FONT = 'Google Sans';
const SCHWARZ = '000000', KOPF = 'E6E6E6', LINIE = 'A6A6A6';
const BREITE = 9921;   // A4 11906 − links 2 cm (1134) − rechts 1,5 cm (851)
const EINZUG = 397;    // 0,7 cm hängender Einzug für Aufgabennummern
const cm = (x) => Math.round(x * 567);

// Text mit **fett**, *kursiv* und \t (Tabulator)
function rich(text, o = {}) {
  const runs = [];
  String(text).split(/(\*\*[^*]+\*\*|\*[^*\s][^*]*\*)/).filter(Boolean).forEach((t) => {
    const b = t.startsWith('**') && t.endsWith('**') && t.length > 4;
    const i = !b && t.startsWith('*') && t.endsWith('*') && t.length > 2;
    const s = b ? t.slice(2, -2) : i ? t.slice(1, -1) : t;
    const children = [];
    s.split('\t').forEach((teil, k) => { if (k > 0) children.push(new Tab()); if (teil) children.push(teil); });
    runs.push(new TextRun({ children, font: FONT, size: (o.pt || 11) * 2, bold: !!(o.b || b), italics: !!(o.i || i), color: SCHWARZ }));
  });
  return runs;
}

const text = (t, o = {}) => new Paragraph({
  indent: o.einzug ? { left: EINZUG } : undefined, alignment: o.ausr, keepNext: !!o.keepNext,
  spacing: { before: o.vor || 0, after: o.nach ?? 100 }, children: rich(t, o),
});
const umbruch = () => new Paragraph({ children: [new PageBreak()] });

// Kopf wie in der Vorlage: Titel, Schule + SJ, Thema, Linie / Name – Kurs – Datum / Linie
function kopf({ titel, thema, datum, sj, zusatz, gruppe = 'Kurs' }) {
  const dick = { style: BorderStyle.SINGLE, size: 12, color: SCHWARZ, space: 3 };
  const p = [
    new Paragraph({ alignment: AlignmentType.CENTER, spacing: { after: 160 }, children: rich(titel, { pt: 14, b: true }) }),
    new Paragraph({ tabStops: [{ type: TabStopType.CENTER, position: cm(6) }, { type: TabStopType.CENTER, position: cm(13) }],
      spacing: { after: 160 }, children: rich(`\tStaatliches Gymnasium Bergschule Apolda\t${sj}`) }),
    new Paragraph({ alignment: AlignmentType.CENTER, spacing: { after: 160 }, children: rich(`Thema: ${thema}`, { b: true, i: true }) }),
    new Paragraph({ tabStops: [{ type: TabStopType.CENTER, position: Math.round(BREITE / 2) }, { type: TabStopType.RIGHT, position: BREITE }],
      border: { top: dick, bottom: dick }, spacing: { after: zusatz ? 120 : 240 }, children: rich(`Name:\t${gruppe}:\tDatum: ${datum}`) }),
  ];
  if (zusatz) p.push(text(zusatz, { nach: 240 }));
  return p;
}

// Aufgabe: Nummer mit hängendem Einzug, Absätze, [x BE] rechtsbündig am Ende
function aufgabe(nr, absaetze, be) {
  const arr = [].concat(absaetze);
  return arr.map((t, k) => new Paragraph({
    indent: { left: EINZUG, hanging: k === 0 ? EINZUG : 0 },
    tabStops: [{ type: TabStopType.LEFT, position: EINZUG }, { type: TabStopType.RIGHT, position: BREITE }],
    spacing: { before: k === 0 ? 280 : 0, after: 60 }, keepNext: k < arr.length - 1,
    children: rich((k === 0 ? `${nr}\t` : '') + t + (k === arr.length - 1 && be != null ? `\t[${be} BE]` : '')),
  }));
}

// Lösung im Erwartungshorizont (eingerückt unter der Aufgabe)
const loesung = (t) => text(t, { einzug: true });

// Zitat / These im Wellenrahmen (wie Vorlage)
function zitat(t) {
  const w = { style: BorderStyle.DOUBLE_WAVE, size: 6, color: SCHWARZ, space: 6 };
  return new Paragraph({ alignment: AlignmentType.CENTER, border: { top: w, bottom: w, left: w, right: w },
    indent: { left: 120, right: 120 }, spacing: { before: 360, after: 60 }, keepNext: true, children: rich(`„${t}“`) });
}

// Material
const materialTitel = (t) => new Paragraph({ keepNext: true, spacing: { before: 360, after: 120 }, children: rich(t, { b: true }) });
const quelle = (t) => new Paragraph({ spacing: { before: 60, after: 240 }, children: rich(`Quelle: ${t}`, { pt: 9, i: true }) });

// Bild (nur PNG, z. B. aus matplotlib)
function bild(pfad, breiteCm, o = {}) {
  const buf = fs.readFileSync(pfad);
  const w = buf.readUInt32BE(16), h = buf.readUInt32BE(20);
  const px = Math.round((breiteCm / 2.54) * 96);
  return new Paragraph({ alignment: o.mitte ? AlignmentType.CENTER : AlignmentType.LEFT, keepNext: !!o.keepNext,
    children: [new ImageRun({ type: 'png', data: buf, transformation: { width: px, height: Math.round((px * h) / w) } })] });
}
// Bilder nebeneinander mit Unterschrift (Abb. A, Abb. B)
function nebeneinander(bilder) {
  const w = Math.floor(BREITE / bilder.length);
  const ohne = { style: BorderStyle.NONE, size: 0, color: 'FFFFFF' };
  const rand = { top: ohne, bottom: ohne, left: ohne, right: ohne };
  return new Table({ width: { size: w * bilder.length, type: WidthType.DXA }, columnWidths: bilder.map(() => w),
    borders: { ...rand, insideHorizontal: ohne, insideVertical: ohne },
    rows: [new TableRow({ cantSplit: true, children: bilder.map((b) => new TableCell({ width: { size: w, type: WidthType.DXA }, borders: rand,
      children: [bild(b.pfad, b.cm, { mitte: true }), text(b.text || '', { ausr: AlignmentType.CENTER })] })) })] });
}

// Tabellen: Kopf hellgrau, Linien grau
const numbering = { config: [{ reference: 'punkt', levels: [{ level: 0, format: LevelFormat.BULLET, text: '•', alignment: AlignmentType.LEFT,
  style: { paragraph: { indent: { left: 284, hanging: 200 } }, run: { font: FONT } } }] }] };
const punkt = (t, o = {}) => new Paragraph({ numbering: { reference: 'punkt', level: 0 }, spacing: { after: 40 }, children: rich(t, o) });

function zelle(inhalt, w, o = {}) {
  const l = { style: BorderStyle.SINGLE, size: 4, color: LINIE };
  const kinder = [].concat(inhalt).map((x) => (x instanceof Paragraph ? x
    : String(x).startsWith('• ') ? punkt(String(x).slice(2)) : new Paragraph({ alignment: o.ausr, keepNext: !!o.keepNext, children: rich(x, { b: o.b }) })));
  return new TableCell({ width: { size: w, type: WidthType.DXA }, verticalAlign: VerticalAlign.CENTER,
    shading: o.grau ? { type: ShadingType.CLEAR, color: 'auto', fill: KOPF } : undefined,
    borders: { top: l, bottom: l, left: l, right: l }, margins: { top: 60, bottom: 60, left: 100, right: 100 }, children: kinder });
}
function breiten(anteile) {
  const s = anteile.reduce((a, b) => a + b, 0);
  const w = anteile.map((x) => Math.floor((x * BREITE) / s));
  w[w.length - 1] += BREITE - w.reduce((a, b) => a + b, 0);
  return w;
}
// kopfzeile: Array oder null; zeilen: Array von Arrays (Zelle = String, Paragraph oder Array davon)
// o.anteile: relative Spaltenbreiten; o.zentriert: Spaltenindizes; o.ersteSpalteFett; o.grauZeilen: Indizes grau hinterlegter Zeilen;
// o.zusammen: Tabelle nicht über einen Seitenumbruch trennen (nur für kurze Tabellen)
function tabelle(kopfzeile, zeilen, o = {}) {
  const n = (kopfzeile || zeilen[0]).length;
  const w = breiten(o.anteile || Array(n).fill(1));
  const ausr = (j) => (o.zentriert || []).includes(j) ? AlignmentType.CENTER : undefined;
  const rows = [];
  if (kopfzeile) rows.push(new TableRow({ tableHeader: true, cantSplit: true, children: kopfzeile.map((k, j) => zelle(k, w[j], { grau: true, b: true, ausr: ausr(j), keepNext: true })) }));
  zeilen.forEach((z, r) => rows.push(new TableRow({ cantSplit: true, children: z.map((c, j) => zelle(c, w[j], {
    b: (o.ersteSpalteFett && j === 0) || (o.grauZeilen || []).includes(r), grau: (o.grauZeilen || []).includes(r), ausr: ausr(j),
    keepNext: o.zusammen && r < zeilen.length - 1 })) })));
  return new Table({ width: { size: BREITE, type: WidthType.DXA }, columnWidths: w, rows });
}

// Bewertungsraster für die Erörterungs-/Beurteilungsaufgabe (wie Vorlage)
// zeilen: [{ kriterium: ['Einleitungssatz', '• Punkt', ...], max: 3 }]
function raster(zeilen, titel = 'Anforderungsbereich III') {
  const summe = zeilen.reduce((a, z) => a + z.max, 0);
  return tabelle([titel, 'erreichte BE', 'maximale BE'],
    [...zeilen.map((z) => [z.kriterium, '', `${z.max} BE`]), ['Gesamtpunktzahl', '', `${summe} BE`]],
    { anteile: [72, 14, 14], zentriert: [1, 2], grauZeilen: [zeilen.length] });
}

// AFB-Übersicht: aufgaben = [{ nr: '1', afb: 1, be: 12 }, ...]
function afbTabelle(aufgaben) {
  const gesamt = aufgaben.reduce((a, x) => a + x.be, 0);
  const sp = [1, 2, 3].map((k) => aufgaben.filter((x) => x.afb === k));
  const be = sp.map((l) => l.reduce((a, x) => a + x.be, 0));
  return tabelle(['', 'AFB I', 'AFB II', 'AFB III', 'gesamt'], [
    ['Aufgaben', ...sp.map((l) => l.map((x) => x.nr).join(', ') || '–'), ''],
    ['BE', ...sp.map((l) => l.map((x) => x.be).join(' + ') || '0'), String(gesamt)],
    ['Anteil', ...be.map((b) => `${Math.round((b * 100) / gesamt)} %`), '100 %'],
  ], { anteile: [16, 21, 21, 21, 21], zentriert: [1, 2, 3, 4], ersteSpalteFett: true, zusammen: true });
}

// Notenschlüssel nach Felix' Tabelle "Punkte_Noten_11_12": ab BE = RUNDEN(Gesamt-BE × Anteil; 0)
const SCHLUESSEL = [[15, '1+', 95], [14, '1', 90], [13, '1-', 85], [12, '2+', 80], [11, '2', 75], [10, '2-', 70], [9, '3+', 65], [8, '3', 60],
  [7, '3-', 55], [6, '4+', 50], [5, '4', 45], [4, '4-', 40], [3, '5+', 33], [2, '5', 27], [1, '5-', 20], [0, '6', 0]];
const abBE = (gesamt, prozent) => Math.floor(Math.round(gesamt * prozent * 100) / 10000 + 0.5 + 1e-9); // kaufmännisch
function notenTabelle(gesamt) {
  return tabelle(null, [
    ['Notenpunkte', ...SCHLUESSEL.map((s) => String(s[0]))],
    ['Note', ...SCHLUESSEL.map((s) => s[1])],
    ['ab BE', ...SCHLUESSEL.map((s) => String(abBE(gesamt, s[2])))],
  ], { anteile: [3.4, ...Array(16).fill(1)], zentriert: [...Array(17).keys()].slice(1), ersteSpalteFett: true, grauZeilen: [0], zusammen: true });
}
// Sek I nach Felix' "Notentabelle Sek I" (Klasse 5–10): ab BE = AUFRUNDEN(Gesamt-BE × Anteil)
const SEK1 = [['1', 96], ['2', 80], ['3', 65], ['4', 50], ['5', 25], ['6', 0]];
const abBESek1 = (gesamt, prozent) => Math.ceil(Math.round(gesamt * prozent * 100) / 10000 - 1e-9);
function notenTabelleSek1(gesamt) {
  return tabelle(null, [['Note', ...SEK1.map((s) => s[0])], ['ab BE', ...SEK1.map((s) => String(abBESek1(gesamt, s[1])))]],
    { anteile: [3.4, ...Array(6).fill(2)], zentriert: [1, 2, 3, 4, 5, 6], ersteSpalteFett: true, grauZeilen: [0], zusammen: true });
}

// Summenprüfung: wirft einen Fehler, wenn BE nicht aufgehen
function pruefeBE(aufgaben, gesamt) {
  const s = aufgaben.reduce((a, x) => a + x.be, 0);
  if (s !== gesamt) throw new Error(`BE-Summe ${s} ≠ Gesamt ${gesamt}`);
  aufgaben.forEach((x) => { if (![1, 2, 3].includes(x.afb)) throw new Error(`AFB fehlt bei Aufgabe ${x.nr}`); });
}

async function speichern(children, pfad) {
  const doc = new Document({
    styles: { default: { document: { run: { font: FONT, size: 22, color: SCHWARZ }, paragraph: { spacing: { line: 264, lineRule: LineRuleType.AUTO } } } } },
    numbering,
    sections: [{ properties: { page: { size: { width: 11906, height: 16838 }, margin: { top: 567, bottom: 567, left: 1134, right: 851 } } }, children }],
  });
  fs.writeFileSync(pfad, await Packer.toBuffer(doc));
}

module.exports = { rich, text, umbruch, kopf, aufgabe, loesung, zitat, materialTitel, quelle, bild, nebeneinander, punkt,
  tabelle, raster, afbTabelle, notenTabelle, notenTabelleSek1, abBE, abBESek1, pruefeBE, speichern, cm, BREITE };
