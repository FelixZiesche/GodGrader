//! Stapelverarbeitung: Abschreiben → Bewerten (→ Zweitmeinung).
//!
//! - Jeder Schritt wird einzeln gespeichert; ein neuer Lauf überspringt
//!   fertige Schritte (Fortsetzen nach Abbruch oder Absturz).
//! - Höchstens `parallel` Anfragen gleichzeitig.
//! - „Anhalten“ startet keine neuen Anfragen mehr, laufende werden fertig.
//!   „Abbrechen“ beendet auch laufende Anfragen sofort.
//! - Vor jeder Anfrage wird das Budget geprüft (bisherige Kosten plus
//!   Schätzung der laufenden Anfragen).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

use super::anfragen::{
    AbschreibAnfrage, BewertAnfrage, BewertAufgabe, ModellParameter, Seitenbild,
};
use super::aufruf::{self, Antwort};
use super::prompts::Vorlagen;
use super::schaetzung;
use super::schema::{self, BewertErgebnis, ElementBewertung, Sicherheit};
use super::speicher::{Aufruf, SchrittArt, SchrittStatus};
use crate::ewh::{Aufgabe, Erwartungshorizont};
use crate::models::ModelInfo;
use crate::openrouter::{provider_prefs, OpenRouter};
use crate::projekt::{Projekt, ProjektMeta};
use crate::scans::KlausurStatus;
use crate::slots::{SlotKind, SlotSet};
use crate::{Error, Result};

#[derive(Debug, Clone)]
pub struct PipelineKonfig {
    pub slots: SlotSet,
    /// Modellbibliothek (Fähigkeiten und Preise).
    pub modelle: Vec<ModelInfo>,
    pub zdr_only: bool,
    pub parallel: usize,
    pub vorlagen: Vorlagen,
}

impl PipelineKonfig {
    fn info(&self, k: SlotKind) -> Option<&ModelInfo> {
        let id = &self.slots.get(k).model;
        self.modelle.iter().find(|m| &m.id == id)
    }
}

/// Steuerung eines laufenden Durchgangs.
#[derive(Debug, Clone, Default)]
pub struct Steuerung {
    anhalten: CancellationToken,
    abbrechen: CancellationToken,
}

impl Steuerung {
    pub fn neu() -> Self {
        Self::default()
    }
    /// Keine neuen Anfragen mehr, laufende werden fertig.
    pub fn anhalten(&self) {
        self.anhalten.cancel();
    }
    /// Sofort beenden, auch laufende Anfragen.
    pub fn abbrechen(&self) {
        self.abbrechen.cancel();
        self.anhalten.cancel();
    }
    pub fn angehalten(&self) -> bool {
        self.anhalten.is_cancelled()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "art", rename_all = "snake_case")]
pub enum Ereignis {
    Schritt {
        klausur_id: String,
        schritt: SchrittArt,
        aufgabe: String,
        status: SchrittStatus,
        fehler: Option<String>,
    },
    Kosten {
        gesamt: f64,
    },
    Klausur {
        klausur_id: String,
        status: KlausurStatus,
    },
    Hinweis {
        text: String,
    },
}

pub trait Beobachter: Send + Sync {
    fn melden(&self, e: Ereignis);
}

impl<F: Fn(Ereignis) + Send + Sync> Beobachter for F {
    fn melden(&self, e: Ereignis) {
        self(e)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Ende {
    Fertig,
    Angehalten,
    Abgebrochen,
    Budget,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LaufErgebnis {
    pub ende: Ende,
    pub fehler: usize,
    /// Kosten dieses Laufs.
    pub kosten: f64,
}

struct Ctx {
    p: Mutex<Projekt>,
    client: OpenRouter,
    konfig: PipelineKonfig,
    ewh: Erwartungshorizont,
    meta: ProjektMeta,
    provider: Value,
    sem: Semaphore,
    st: Steuerung,
    beob: Arc<dyn Beobachter>,
    reserviert: Mutex<f64>,
    budget_erreicht: AtomicBool,
    fehler: AtomicUsize,
    start_kosten: f64,
}

/// Grund, warum ein Schritt nicht lief.
enum Abbruch {
    /// Angehalten, abgebrochen oder Budget: Schritt bleibt offen.
    Gestoppt,
    Fehler(Error),
}

impl From<Error> for Abbruch {
    fn from(e: Error) -> Self {
        match e {
            Error::Abgebrochen => Abbruch::Gestoppt,
            e => Abbruch::Fehler(e),
        }
    }
}

impl Ctx {
    fn projekt(&self) -> std::sync::MutexGuard<'_, Projekt> {
        self.p.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn melden(&self, e: Ereignis) {
        self.beob.melden(e);
    }

    #[allow(clippy::too_many_arguments)]
    fn schritt(
        &self,
        kid: &str,
        art: SchrittArt,
        aufgabe: &str,
        status: SchrittStatus,
        modell: Option<&str>,
        fehler: Option<&str>,
        hinweis: Option<&str>,
    ) {
        if let Err(e) = self
            .projekt()
            .schritt_setzen(kid, art, aufgabe, status, modell, fehler, hinweis)
        {
            tracing::error!("Schritt speichern: {e}");
        }
        self.melden(Ereignis::Schritt {
            klausur_id: kid.into(),
            schritt: art,
            aufgabe: aufgabe.into(),
            status,
            fehler: fehler.map(str::to_string),
        });
    }

    /// Reserviert Budget für einen Aufruf. `false` = Budget erreicht.
    fn reservieren(&self, schaetzung: f64) -> Result<bool> {
        let Some(budget) = self.meta.budget_usd else {
            return Ok(true);
        };
        let mut r = self.reserviert.lock().unwrap_or_else(|e| e.into_inner());
        let bisher = self.projekt().kosten_gesamt()?;
        if bisher + *r + schaetzung > budget {
            if !self.budget_erreicht.swap(true, Ordering::SeqCst) {
                self.melden(Ereignis::Hinweis {
                    text: format!(
                        "Budget von {budget:.2} $ erreicht (bisher {bisher:.2} $). Der Lauf wurde angehalten."
                    ),
                });
            }
            self.st.anhalten();
            return Ok(false);
        }
        *r += schaetzung;
        Ok(true)
    }

    fn freigeben(&self, schaetzung: f64) {
        if self.meta.budget_usd.is_some() {
            let mut r = self.reserviert.lock().unwrap_or_else(|e| e.into_inner());
            *r = (*r - schaetzung).max(0.0);
        }
    }

    /// Ein Modellaufruf mit Parallelitätsgrenze, Budget, Abbruch und Kostenprotokoll.
    async fn aufrufen(
        &self,
        kid: &str,
        art: SchrittArt,
        aufgabe: &str,
        kind: SlotKind,
        body: &Value,
        schaetzung: f64,
    ) -> std::result::Result<Antwort, Abbruch> {
        if self.st.angehalten() {
            return Err(Abbruch::Gestoppt);
        }
        let _permit = tokio::select! {
            p = self.sem.acquire() => p.map_err(|_| Abbruch::Gestoppt)?,
            _ = self.st.anhalten.cancelled() => return Err(Abbruch::Gestoppt),
        };
        if self.st.angehalten() || !self.reservieren(schaetzung)? {
            return Err(Abbruch::Gestoppt);
        }
        let modell = self.konfig.slots.get(kind).model.clone();
        self.schritt(
            kid,
            art,
            aufgabe,
            SchrittStatus::Laeuft,
            Some(&modell),
            None,
            None,
        );
        let start = Instant::now();
        let info = self.konfig.info(kind);
        let res = tokio::select! {
            r = aufruf::senden(&self.client, body, info) => r,
            _ = self.st.abbrechen.cancelled() => Err(Error::Abgebrochen),
        };
        self.freigeben(schaetzung);
        let mut protokoll = Aufruf {
            klausur_id: Some(kid.into()),
            art: art.as_str().into(),
            aufgabe: (!aufgabe.is_empty()).then(|| aufgabe.to_string()),
            modell: modell.clone(),
            dauer_ms: start.elapsed().as_millis() as u64,
            ..Default::default()
        };
        match &res {
            Ok(a) => {
                protokoll.ok = true;
                protokoll.tokens_ein = a.nutzung.tokens_ein;
                protokoll.tokens_aus = a.nutzung.tokens_aus;
                protokoll.kosten = a.nutzung.kosten;
                protokoll.geschaetzt = a.nutzung.geschaetzt;
                if !a.modell.is_empty() {
                    protokoll.modell = a.modell.clone();
                }
            }
            Err(Error::Abgeschnitten(k)) => protokoll.kosten = *k,
            // Abgebrochene Anfragen: Kosten unbekannt, nichts protokollieren
            Err(Error::Abgebrochen) => return Err(Abbruch::Gestoppt),
            Err(_) => {}
        }
        let gesamt = {
            let p = self.projekt();
            if let Err(e) = p.aufruf_protokollieren(&protokoll) {
                tracing::error!("Kosten speichern: {e}");
            }
            p.kosten_gesamt().unwrap_or(0.0)
        };
        self.melden(Ereignis::Kosten { gesamt });
        Ok(res?)
    }

    fn fehler(&self, kid: &str, art: SchrittArt, aufgabe: &str, e: Abbruch) {
        match e {
            Abbruch::Gestoppt => {
                self.schritt(kid, art, aufgabe, SchrittStatus::Offen, None, None, None)
            }
            Abbruch::Fehler(e) => {
                self.fehler.fetch_add(1, Ordering::SeqCst);
                self.schritt(
                    kid,
                    art,
                    aufgabe,
                    SchrittStatus::Fehler,
                    None,
                    Some(&e.to_string()),
                    None,
                );
            }
        }
    }
}

fn seitenbilder(p: &Projekt, kid: &str) -> Result<Vec<Seitenbild>> {
    let seiten = p.seiten(kid)?;
    let mut quellen: Vec<u32> = seiten.iter().map(|s| s.quelle_seite).collect();
    quellen.sort_unstable();
    quellen.dedup();
    seiten
        .iter()
        .map(|s| {
            let n = quellen
                .iter()
                .position(|&q| q == s.quelle_seite)
                .unwrap_or(0)
                + 1;
            let bezeichnung = match s.halbteil {
                Some(crate::scans::Halbteil::Oben) => format!("Seite {n}, obere Hälfte"),
                Some(crate::scans::Halbteil::Unten) => format!("Seite {n}, untere Hälfte"),
                None => format!("Seite {n}"),
            };
            Ok(Seitenbild {
                bezeichnung,
                jpeg: std::fs::read(p.seitenbild(&s.id)?)?,
            })
        })
        .collect()
}

/// Bis zu zwei Versuche, falls die Antwort nicht dem Schema entspricht.
const VERSUCHE: usize = 2;

async fn abschreiben(ctx: &Ctx, kid: &str) -> std::result::Result<(), Abbruch> {
    let bilder = seitenbilder(&ctx.projekt(), kid)?;
    if bilder.is_empty() {
        return Err(Abbruch::Fehler(Error::Ungueltig(
            "Keine Seitenbilder vorhanden.".into(),
        )));
    }
    let n = bilder.len();
    let anfrage = AbschreibAnfrage::neu(&ctx.ewh, bilder);
    let slot = &ctx.konfig.slots.abschreiben;
    let info = ctx.konfig.info(SlotKind::Abschreiben);
    let body = anfrage.body(
        &ctx.konfig.vorlagen.abschreiben,
        &ModellParameter::aus(slot, info),
        &ctx.provider,
    );
    let schaetzung = schaetzung::kosten_abschreiben(n, slot, info);
    let mut letzter = None;
    for _ in 0..VERSUCHE {
        let a = ctx
            .aufrufen(
                kid,
                SchrittArt::Abschreiben,
                "",
                SlotKind::Abschreiben,
                &body,
                schaetzung,
            )
            .await?;
        match schema::json_aus_text(&a.inhalt)
            .and_then(|v| schema::abschrift_lesen(v, anfrage.aufgabennummern()))
        {
            Ok(erg) => {
                let unsicher: usize = erg.aufgaben.iter().map(|x| x.unsichere_stellen.len()).sum();
                ctx.projekt()
                    .abschrift_speichern(kid, &erg, anfrage.aufgabennummern())?;
                let hinweis = (unsicher > 0).then(|| format!("{unsicher} unsichere Stellen"));
                ctx.schritt(
                    kid,
                    SchrittArt::Abschreiben,
                    "",
                    SchrittStatus::Fertig,
                    Some(&a.modell),
                    None,
                    hinweis.as_deref(),
                );
                return Ok(());
            }
            Err(e) => letzter = Some(e),
        }
    }
    Err(Abbruch::Fehler(letzter.expect("mindestens ein Versuch")))
}

fn leere_bewertung(a: &BewertAufgabe) -> BewertErgebnis {
    BewertErgebnis {
        aufgabe: a.nr.clone(),
        elemente: a
            .elemente
            .iter()
            .map(|e| ElementBewertung {
                element_id: e.id.clone(),
                max_be: e.max_be,
                vorschlag_be: 0.0,
                begruendung: "Keine Antwort in der Abschrift gefunden.".into(),
                beleg: String::new(),
                sicherheit: Sicherheit::Niedrig,
                haengt_an_unsicherer_stelle: false,
            })
            .collect(),
        summe_vorschlag: 0.0,
        hinweis_fuer_lehrkraft: "Keine Antwort gefunden. Bitte auf dem Scan prüfen.".into(),
    }
}

async fn bewerten(
    ctx: &Ctx,
    kid: &str,
    aufgabe: &Aufgabe,
    art: SchrittArt,
) -> std::result::Result<(), Abbruch> {
    let kind = if art == SchrittArt::Zweitmeinung {
        SlotKind::Zweitmeinung
    } else {
        SlotKind::Bewerten
    };
    let zweit = art == SchrittArt::Zweitmeinung;
    let ba = BewertAufgabe::aus(aufgabe);
    if ba.elemente.is_empty() {
        return Err(Abbruch::Fehler(Error::Ungueltig(format!(
            "Aufgabe {} hat keine Bewertungselemente. Bitte im Erwartungshorizont ergänzen.",
            aufgabe.nr
        ))));
    }
    let abschrift = ctx
        .projekt()
        .abschrift_text(kid, &aufgabe.nr)?
        .unwrap_or_default();
    if abschrift.trim().is_empty() {
        let b = leere_bewertung(&ba);
        ctx.projekt().bewertung_speichern(kid, &b, zweit)?;
        ctx.schritt(
            kid,
            art,
            &aufgabe.nr,
            SchrittStatus::Fertig,
            None,
            None,
            Some(&b.hinweis_fuer_lehrkraft),
        );
        return Ok(());
    }
    let tokens = (abschrift.chars().count() as f64 / 3.2) as u64;
    let slot = ctx.konfig.slots.get(kind);
    let info = ctx.konfig.info(kind);
    let schaetzung = schaetzung::kosten_bewerten(aufgabe, tokens, slot, info);
    let anfrage = BewertAnfrage {
        aufgabe: ba,
        abschrift,
        stufe: ctx.ewh.stufe,
        halbe_be: ctx.meta.halbe_be,
    };
    let body = anfrage.body(
        &ctx.konfig.vorlagen.bewerten,
        &ModellParameter::aus(slot, info),
        &ctx.provider,
    );
    let mut letzter = None;
    for _ in 0..VERSUCHE {
        let a = ctx
            .aufrufen(kid, art, &aufgabe.nr, kind, &body, schaetzung)
            .await?;
        match schema::json_aus_text(&a.inhalt)
            .and_then(|v| schema::bewertung_lesen(v, &anfrage.aufgabe, ctx.meta.halbe_be))
        {
            Ok((b, korrekturen)) => {
                ctx.projekt().bewertung_speichern(kid, &b, zweit)?;
                let mut hinweis = b.hinweis_fuer_lehrkraft.clone();
                if !korrekturen.is_empty() {
                    if !hinweis.is_empty() {
                        hinweis.push('\n');
                    }
                    hinweis.push_str(&format!(
                        "Automatisch korrigiert: {}",
                        korrekturen.join(" ")
                    ));
                }
                ctx.schritt(
                    kid,
                    art,
                    &aufgabe.nr,
                    SchrittStatus::Fertig,
                    Some(&a.modell),
                    None,
                    (!hinweis.is_empty()).then_some(hinweis.as_str()),
                );
                return Ok(());
            }
            Err(e) => letzter = Some(e),
        }
    }
    Err(Abbruch::Fehler(letzter.expect("mindestens ein Versuch")))
}

fn ist_fertig(ctx: &Ctx, kid: &str, art: SchrittArt, aufgabe: &str) -> bool {
    matches!(ctx.projekt().schritt(kid, art, aufgabe), Ok(Some(s)) if s.status == SchrittStatus::Fertig)
}

async fn klausur(ctx: Arc<Ctx>, kid: String) {
    if !ist_fertig(&ctx, &kid, SchrittArt::Abschreiben, "") {
        if let Err(e) = abschreiben(&ctx, &kid).await {
            ctx.fehler(&kid, SchrittArt::Abschreiben, "", e);
            return;
        }
    }
    let mut arten = vec![SchrittArt::Bewerten];
    if ctx.konfig.slots.zweitmeinung.enabled {
        arten.push(SchrittArt::Zweitmeinung);
    }
    let mut set = tokio::task::JoinSet::new();
    for (ai, a) in ctx.ewh.aufgaben.iter().enumerate() {
        for &art in &arten {
            if ist_fertig(&ctx, &kid, art, &a.nr) {
                continue;
            }
            let ctx = ctx.clone();
            let kid = kid.clone();
            set.spawn(async move {
                let a = &ctx.ewh.aufgaben[ai];
                if let Err(e) = bewerten(&ctx, &kid, a, art).await {
                    ctx.fehler(&kid, art, &a.nr, e);
                }
            });
        }
    }
    while set.join_next().await.is_some() {}
    let alles = ctx
        .ewh
        .aufgaben
        .iter()
        .all(|a| arten.iter().all(|&art| ist_fertig(&ctx, &kid, art, &a.nr)));
    if alles {
        let p = ctx.projekt();
        if p.klausur_status(&kid).ok() == Some(KlausurStatus::Offen) {
            let _ = p.klausur_status_setzen(&kid, KlausurStatus::Vorkorrigiert);
            drop(p);
            ctx.melden(Ereignis::Klausur {
                klausur_id: kid.clone(),
                status: KlausurStatus::Vorkorrigiert,
            });
        }
    }
}

/// Prüft, ob ein Lauf starten kann. Gibt den Erwartungshorizont zurück.
pub fn pruefen(p: &Projekt, konfig: &PipelineKonfig) -> Result<Erwartungshorizont> {
    let ewh = p.ewh()?.ok_or_else(|| {
        Error::Ungueltig("Bitte zuerst einen Erwartungshorizont importieren.".into())
    })?;
    if ewh.aufgaben.is_empty() {
        return Err(Error::Ungueltig(
            "Der Erwartungshorizont enthält keine Aufgaben.".into(),
        ));
    }
    for k in [SlotKind::Abschreiben, SlotKind::Bewerten] {
        if konfig.slots.get(k).model.trim().is_empty() {
            return Err(Error::Ungueltig(format!(
                "Slot „{}“ hat kein Modell.",
                k.label()
            )));
        }
    }
    if let Some(m) = konfig.info(SlotKind::Abschreiben) {
        if !m.reads_images {
            return Err(Error::Ungueltig(format!(
                "Das Modell im Slot „Abschreiben“ ({}) kann keine Bilder lesen.",
                m.name
            )));
        }
    }
    Ok(ewh)
}

/// Führt die Pipeline für die gewählten Klausuren aus.
pub async fn ausfuehren(
    projekt: Projekt,
    client: OpenRouter,
    konfig: PipelineKonfig,
    klausuren: Vec<String>,
    st: Steuerung,
    beob: Arc<dyn Beobachter>,
) -> Result<LaufErgebnis> {
    let ewh = pruefen(&projekt, &konfig)?;
    projekt.laufende_schritte_zuruecksetzen()?;
    let meta = projekt.meta()?;
    let alle = projekt.klausuren()?;
    let start_kosten = projekt.kosten_gesamt()?;
    let auswahl: Vec<String> = klausuren
        .into_iter()
        .filter(|id| {
            alle.iter()
                .any(|k| &k.id == id && k.status != KlausurStatus::Freigegeben)
        })
        .collect();
    let ctx = Arc::new(Ctx {
        p: Mutex::new(projekt),
        provider: provider_prefs(konfig.zdr_only),
        sem: Semaphore::new(konfig.parallel.clamp(1, 16)),
        client,
        konfig,
        ewh,
        meta,
        st,
        beob,
        reserviert: Mutex::new(0.0),
        budget_erreicht: AtomicBool::new(false),
        fehler: AtomicUsize::new(0),
        start_kosten,
    });
    let mut set = tokio::task::JoinSet::new();
    for kid in auswahl {
        set.spawn(klausur(ctx.clone(), kid));
    }
    while set.join_next().await.is_some() {}
    let kosten = ctx.projekt().kosten_gesamt()? - ctx.start_kosten;
    let ende = if ctx.budget_erreicht.load(Ordering::SeqCst) {
        Ende::Budget
    } else if ctx.st.abbrechen.is_cancelled() {
        Ende::Abgebrochen
    } else if ctx.st.angehalten() {
        Ende::Angehalten
    } else {
        Ende::Fertig
    };
    Ok(LaufErgebnis {
        ende,
        fehler: ctx.fehler.load(Ordering::SeqCst),
        kosten,
    })
}
