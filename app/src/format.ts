const NBSP = " ";

const LOCALES = { pt: "pt-BR", en: "en-US", es: "es-ES" } as const;
let locale: string = LOCALES.pt;
let int = new Intl.NumberFormat(locale);
let one = new Intl.NumberFormat(locale, { minimumFractionDigits: 1, maximumFractionDigits: 1 });

/** Chamado pelo provedor de idioma: números e datas seguem o idioma escolhido. */
export function setLocale(lang: keyof typeof LOCALES) {
  locale = LOCALES[lang];
  int = new Intl.NumberFormat(locale);
  one = new Intl.NumberFormat(locale, { minimumFractionDigits: 1, maximumFractionDigits: 1 });
}

export const fmtInt = (n: number) => int.format(n);

/** Bytes legíveis, com espaço inseparável antes da unidade. */
export function fmtBytes(n: number): string {
  if (n < 1024) return `${n}${NBSP}B`;
  const units = ["KB", "MB", "GB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${one.format(v)}${NBSP}${units[i]}`;
}

/** MB (como o núcleo informa memória e disco) → texto legível. */
export const fmtMb = (mb: number) => fmtBytes(mb * 1024 * 1024);

/** Data (segundos desde 1970) no idioma escolhido. */
export const fmtDate = (seconds: number) => new Date(seconds * 1000).toLocaleDateString(locale, { year: "numeric", month: "short", day: "numeric" });
