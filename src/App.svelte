<script lang="ts">
  import { onMount } from 'svelte'
  import { api, fehlertext } from './lib/api'
  import { store } from './lib/store.svelte'
  import Modelle from './pages/Modelle.svelte'
  import Einstellungen from './pages/Einstellungen.svelte'
  import Platzhalter from './pages/Platzhalter.svelte'
  import Projekte from './pages/Projekte.svelte'
  import Erwartungshorizont from './pages/Erwartungshorizont.svelte'
  import Scans from './pages/Scans.svelte'

  type Seite =
    | 'projekte' | 'erwartungshorizont' | 'scans' | 'verarbeitung' | 'pruefen'
    | 'statistik' | 'export' | 'modelle' | 'einstellungen'

  const NAV: { id: Seite; titel: string; meilenstein?: number }[] = [
    { id: 'projekte', titel: 'Projekte' },
    { id: 'erwartungshorizont', titel: 'Erwartungshorizont' },
    { id: 'scans', titel: 'Scans' },
    { id: 'verarbeitung', titel: 'Verarbeitung', meilenstein: 4 },
    { id: 'pruefen', titel: 'Prüfen', meilenstein: 5 },
    { id: 'statistik', titel: 'Statistik', meilenstein: 6 },
    { id: 'export', titel: 'Export', meilenstein: 6 },
    { id: 'modelle', titel: 'Modelle' },
    { id: 'einstellungen', titel: 'Einstellungen' },
  ]

  let seite = $state<Seite>('projekte')
  let startfehler = $state<string | null>(null)
  let bereit = $state(false)

  const slotProbleme = $derived(store.slotChecks.filter((c) => !c.available && (c.kind !== 'zweitmeinung' || store.settings?.slots.zweitmeinung.enabled)).length)

  onMount(async () => {
    try {
      await store.init()
    } catch (e) {
      startfehler = fehlertext(e)
    }
    bereit = true
  })

  async function hinweisBestaetigen() {
    await api.ackHinweis()
    if (store.settings) store.settings.hinweis_bestaetigt = true
  }

  const aktuell = $derived(NAV.find((n) => n.id === seite)!)
</script>

<div class="rahmen">
  <nav>
    <div class="marke">GodGrader</div>
    {#if store.projekt}
      <div class="aktuell" title="geöffnetes Projekt">{store.projekt.titel}</div>
    {/if}
    {#each NAV as n (n.id)}
      <button class="navpunkt" class:aktiv={seite === n.id} onclick={() => (seite = n.id)}>
        {n.titel}
        {#if n.id === 'modelle' && slotProbleme > 0}
          <span class="punkt" title="Slot-Problem"></span>
        {/if}
      </button>
    {/each}
  </nav>

  <main>
    {#if startfehler}
      <div class="warnung">Start fehlgeschlagen: {startfehler}</div>
    {:else if !bereit}
      <p class="hinweis">Lade …</p>
    {:else if seite === 'projekte'}
      <Projekte weiter={() => (seite = 'erwartungshorizont')} />
    {:else if seite === 'erwartungshorizont'}
      {#key store.projektId}<Erwartungshorizont />{/key}
    {:else if seite === 'scans'}
      {#key store.projektId}<Scans />{/key}
    {:else if seite === 'modelle'}
      <Modelle />
    {:else if seite === 'einstellungen'}
      <Einstellungen />
    {:else}
      <Platzhalter titel={aktuell.titel} meilenstein={aktuell.meilenstein ?? 0} />
    {/if}
  </main>
</div>

{#if bereit && store.settings && !store.settings.hinweis_bestaetigt}
  <div class="schleier">
    <div class="karte dialog" role="dialog" aria-modal="true" aria-labelledby="hinweis-titel">
      <h2 id="hinweis-titel">Bevor Sie beginnen</h2>
      <p>GodGrader macht nur <strong>Vorschläge</strong>. Die Bewertung verantwortet immer die Lehrkraft.</p>
      <p>
        Klausuren sind personenbezogene Daten. Stimmen Sie den Einsatz mit echten Klausuren bitte vorher mit dem
        schulischen Datenschutzbeauftragten ab. Im Programm werden nur Pseudonyme verwendet, Namenszeilen werden vor
        dem Senden geschwärzt, und jede Anfrage verbietet den Anbietern die Datenspeicherung zum Training.
      </p>
      <div class="zeile" style="justify-content: flex-end">
        <button class="primaer" onclick={hinweisBestaetigen}>Verstanden</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .rahmen { display: grid; grid-template-columns: 210px 1fr; height: 100%; }
  nav {
    border-right: 1px solid var(--schwarz);
    padding: 14px 10px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow-y: auto;
  }
  .marke { font-weight: 700; font-size: 18px; padding: 4px 10px 16px; letter-spacing: 0.2px; }
  .navpunkt {
    text-align: left;
    border: none;
    border-radius: var(--radius);
    padding: 8px 10px;
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .navpunkt.aktiv { background: var(--schwarz); color: var(--weiss); }
  .navpunkt.aktiv:hover { background: var(--schwarz); }
  .aktuell { font-size: 12px; color: var(--grau-4); padding: 0 10px 12px; margin-top: -10px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .punkt { width: 8px; height: 8px; border-radius: 50%; background: var(--akzent); }
  main { overflow: auto; padding: 22px 28px; }
  .schleier {
    position: fixed; inset: 0; background: rgba(0, 0, 0, 0.35);
    display: grid; place-items: center; z-index: 50;
  }
  .dialog { max-width: 520px; }
</style>
