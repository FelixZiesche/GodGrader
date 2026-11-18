use thiserror::Error;

/// Fehler der Fachlogik. Die Texte sind deutsch, weil sie in der Oberfläche
/// angezeigt werden. Sie enthalten nie Klausurinhalte.
#[derive(Debug, Error)]
pub enum Error {
    #[error("Dateizugriff fehlgeschlagen: {0}")]
    Io(#[from] std::io::Error),

    #[error("Daten konnten nicht gelesen werden: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Netzwerkfehler: {0}")]
    Http(#[from] reqwest::Error),

    #[error("OpenRouter meldet Fehler {status}: {message}")]
    Api { status: u16, message: String },

    #[error("Kein API-Schlüssel hinterlegt. Bitte unter Einstellungen eintragen.")]
    KeinSchluessel,

    #[error("Schlüsselbund: {0}")]
    Schluesselbund(String),

    #[error("{0}")]
    Ungueltig(String),

    /// Antwort wegen „max. Tokens“ abgeschnitten; enthält die angefallenen Kosten.
    #[error("Die Antwort des Modells wurde abgeschnitten. Bitte unter „Modelle“ die max. Tokens des Slots erhöhen.")]
    Abgeschnitten(f64),

    #[error("Abgebrochen.")]
    Abgebrochen,
}

pub type Result<T> = std::result::Result<T, Error>;

impl serde::Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
