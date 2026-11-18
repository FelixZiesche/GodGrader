# GodGrader

KI-gestützte **Vorkorrektur** handschriftlicher Klausuren (Wirtschaft und Recht, Gymnasium Thüringen).
Die KI macht nur Vorschläge, die Bewertung verantwortet immer die Lehrkraft.

Desktop-App mit Tauri 2 (Rust) und Svelte 5. Zuerst für Windows, später Linux und Android.

> Die ausführliche Anleitung (Installation, API-Schlüssel, Bedienung) folgt mit Meilenstein 7.

## Aufbau

- `crates/godgrader-core` – Fachlogik ohne Tauri (Modelle, OpenRouter, Einstellungen, später Parser, Pipeline, Exporte)
- `src-tauri` – dünne Tauri-Schicht (Commands)
- `src` – Oberfläche (Svelte 5, TypeScript)
- `docs/ENTSCHEIDUNGEN.md` – getroffene Entscheidungen
- `samples/` – Testdateien (keine echten Schülerarbeiten)

## Entwickeln

```bash
npm ci
npm run tauri dev        # App starten
cargo test --workspace   # Tests
```

Der Windows-Installer wird bei jedem Push von GitHub Actions gebaut (Artefakt „GodGrader-Windows-Installer“).
