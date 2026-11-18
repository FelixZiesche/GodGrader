const eur = new Intl.NumberFormat('de-DE', { minimumFractionDigits: 2, maximumFractionDigits: 2 })
const ganz = new Intl.NumberFormat('de-DE')

export function preis(v: number | null): string {
  if (v === null) return 'variabel'
  if (v === 0) return 'kostenlos'
  return `${eur.format(v)} $`
}

export function kontext(v: number | null): string {
  if (!v) return '–'
  if (v >= 1_000_000) return `${eur.format(v / 1_000_000).replace(/,00$/, '')} Mio.`
  return `${ganz.format(Math.round(v / 1000))} Tsd.`
}

export function datum(unix: number): string {
  if (!unix) return '–'
  return new Date(unix * 1000).toLocaleString('de-DE', { dateStyle: 'medium', timeStyle: 'short' })
}

/** US-Dollar mit deutschem Komma, kleine Beträge genauer. */
export function dollar(v: number | null | undefined): string {
  if (v == null) return '–'
  const stellen = v !== 0 && Math.abs(v) < 1 ? 3 : 2
  return `${v.toLocaleString('de-DE', { minimumFractionDigits: stellen, maximumFractionDigits: stellen })} $`
}
