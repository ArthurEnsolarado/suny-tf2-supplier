import { createContext, createElement, Fragment, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { fmtInt, setLocale } from "../format";
import { translateCore } from "./core";
import { en } from "./en";
import { es } from "./es";
import { pt, type Key } from "./pt";

export type Lang = "pt" | "en" | "es";

export const LANGS: { id: Lang; label: string }[] = [
  { id: "pt", label: "Português" },
  { id: "en", label: "English" },
  { id: "es", label: "Español" },
];

const DICTS: Record<Lang, Record<string, string>> = { pt, en, es };
const STORE_KEY = "suny.lang";

/** `files_one` / `files_other` contam como a chave `files` (o plural é escolhido pelo `n`). */
export type TKey = Key extends infer K ? (K extends `${infer B}_${"one" | "other"}` ? B : K) : never;
export type Vars = Record<string, ReactNode>;

function detect(): Lang {
  try {
    const saved = localStorage.getItem(STORE_KEY);
    if (saved === "pt" || saved === "en" || saved === "es") return saved;
  } catch {
    /* sem armazenamento: usa o idioma do sistema */
  }
  const sys = (typeof navigator !== "undefined" ? navigator.language : "pt").toLowerCase();
  return sys.startsWith("pt") ? "pt" : sys.startsWith("es") ? "es" : "en";
}

function lookup(lang: Lang, key: string, vars?: Vars): string {
  const dict = DICTS[lang];
  const n = vars?.n;
  const k = typeof n === "number" && dict[`${key}_one`] !== undefined ? `${key}_${n === 1 ? "one" : "other"}` : key;
  return dict[k] ?? (pt as Record<string, string>)[k] ?? key;
}

function fill(text: string, vars?: Vars): ReactNode[] {
  return text.split(/\{(\w+)\}/).map((part, i) => {
    if (i % 2 === 0) return part;
    const v = vars?.[part];
    // `n` é sempre uma contagem: sai com separador de milhar do idioma
    return typeof v === "number" && part === "n" ? fmtInt(v) : (v ?? `{${part}}`);
  });
}

/** Texto com `<b>`, `<i>`, `<code>` e `<br/>`: vira elementos do React, nunca HTML solto. */
function rich(text: string, vars?: Vars): ReactNode {
  type Frame = { tag: string | null; kids: ReactNode[] };
  const stack: Frame[] = [{ tag: null, kids: [] }];
  let k = 0;
  const top = (): Frame => stack[stack.length - 1] as Frame;
  const close = () => {
    const f = stack.pop() as Frame;
    top().kids.push(createElement(f.tag as string, { key: k++ }, ...f.kids));
  };
  for (const part of text.split(/(<\/?(?:b|i|code)>|<br\/>)/)) {
    if (!part) continue;
    if (part === "<br/>") top().kids.push(createElement("br", { key: k++ }));
    else if (/^<(b|i|code)>$/.test(part)) stack.push({ tag: part.slice(1, -1), kids: [] });
    else if (/^<\/(b|i|code)>$/.test(part)) {
      if (stack.length > 1) close();
    } else top().kids.push(createElement(Fragment, { key: k++ }, ...fill(part, vars)));
  }
  while (stack.length > 1) close();
  return createElement(Fragment, null, ...(stack[0] as Frame).kids);
}

interface Ctx {
  lang: Lang;
  setLang: (l: Lang) => void;
  /** Texto simples (para atributos como aria-label e placeholder). */
  t: (key: TKey, vars?: Vars) => string;
  /** Texto com negrito/itálico/código. */
  T: (props: { k: TKey; v?: Vars }) => ReactNode;
  /** Mensagem vinda do núcleo (em português), traduzida quando conhecida. */
  tc: (msg: string) => string;
}

const I18n = createContext<Ctx | null>(null);

export function LangProvider({ children }: { children: ReactNode }) {
  const [lang, setLangState] = useState<Lang>(detect);

  useEffect(() => {
    setLocale(lang);
    document.documentElement.lang = lang === "pt" ? "pt-BR" : lang;
  }, [lang]);

  const setLang = useCallback((l: Lang) => {
    setLangState(l);
    try {
      localStorage.setItem(STORE_KEY, l);
    } catch {
      /* vale só nesta sessão */
    }
  }, []);

  const value = useMemo<Ctx>(() => {
    const t = (key: TKey, vars?: Vars) =>
      fill(lookup(lang, key, vars).replace(/<[^>]+>/g, ""), vars)
        .map((x) => (typeof x === "string" || typeof x === "number" ? String(x) : ""))
        .join("");
    const T = ({ k, v }: { k: TKey; v?: Vars }) => rich(lookup(lang, k, v), v);
    return { lang, setLang, t, T, tc: (msg: string) => translateCore(msg, lang) };
  }, [lang, setLang]);

  return createElement(I18n.Provider, { value }, children);
}

export function useI18n(): Ctx {
  const c = useContext(I18n);
  if (!c) throw new Error("useI18n fora do LangProvider");
  return c;
}
