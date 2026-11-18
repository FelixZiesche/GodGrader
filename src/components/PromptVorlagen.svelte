<script lang="ts">
  import { onMount } from 'svelte'
  import { api, fehlertext } from '../lib/api'
  import type { PromptArt, PromptVorlage } from '../lib/types'

  let vorlagen = $state<PromptVorlage[]>([])
  let art = $state<PromptArt>('abschreiben')
  let text = $state('')
  let geaendert = $state(false)
  let meldung = $state<string | null>(null)
  let fehler = $state<string | null>(null)

  const aktuell = $derived(vorlagen.find((v) => v.art === art))
  const fehlend = $derived(aktuell ? aktuell.platzhalter.filter((p) => !text.includes(p)) : [])

  async function lade() {
    vorlagen = await api.promptsLesen()
    text = vorlagen.find((v) => v.art === art)?.text ?? ''
    geaendert = false
  }
  onMount(() => {
    lade().catch((e) => (fehler = fehlertext(e)))
  })

  function wechseln(neu: PromptArt) {
    art = neu
    text = vorlagen.find((v) => v.art === neu)?.text ?? ''
    geaendert = false
    meldung = null
  }

  async function speichern() {
    fehler = meldung = null
    try {
      await api.promptSpeichern(art, text)
      await lade()
      meldung = 'Gespeichert. Gilt ab dem nächsten Aufruf.'
    } catch (e) {
      fehler = fehlertext(e)
    }
  }

  async function zuruecksetzen() {
    fehler = meldung = null
    try {
      await api.promptZuruecksetzen(art)
      await lade()
      meldung = 'Startfassung wiederhergestellt.'
    } catch (e) {
      fehler = fehlertext(e)
    }
  }
</script>

<section class="karte breit">
  <h2>Prompt-Vorlagen</h2>
  <p class="hinweis">
    Die Anweisungen an die Modelle. Platzhalter in geschweiften Klammern werden beim Senden ersetzt. Die Vorlagen liegen als
    Markdown-Dateien im Ordner „prompts“ des Datenordners.
  </p>
  <div class="reiter" role="tablist">
    {#each vorlagen as v (v.art)}
      <button role="tab" aria-selected={art === v.art} class:aktiv={art === v.art} onclick={() => wechseln(v.art)}>
        {v.titel}{v.ist_standard ? '' : ' •'}
      </button>
    {/each}
  </div>
  {#if aktuell}
    <textarea bind:value={text} oninput={() => (geaendert = true)} rows="14" aria-label="Vorlage {aktuell.titel}"></textarea>
    <p class="hinweis">Platzhalter: {aktuell.platzhalter.join(', ')}</p>
    {#if fehlend.length}<p class="hinweis rot">Es fehlt: {fehlend.join(', ')}. Ohne Platzhalter fehlt dem Modell diese Angabe.</p>{/if}
    <div class="zeile">
      <button class="primaer" onclick={speichern} disabled={!geaendert}>Speichern</button>
      <button onclick={zuruecksetzen} disabled={aktuell.ist_standard && !geaendert}>Vorlage zurücksetzen</button>
      {#if meldung}<span class="ok">{meldung}</span>{/if}
    </div>
  {/if}
  {#if fehler}<div class="warnung">{fehler}</div>{/if}
</section>

<style>
  .breit { grid-column: 1 / -1; }
  .reiter { display: flex; flex-wrap: wrap; gap: 4px; margin-bottom: 8px; }
  .reiter button.aktiv { background: var(--schwarz); color: var(--weiss); }
  textarea { width: 100%; font-family: ui-monospace, 'Cascadia Mono', Consolas, monospace; font-size: 13px; line-height: 1.5; resize: vertical; }
  .rot { color: var(--akzent); }
</style>
