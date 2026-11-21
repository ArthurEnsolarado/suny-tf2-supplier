import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { useI18n } from "./i18n";

/* ---------- selo de estado: a assinatura do app (faixa em ângulo, como no placar do TF2) ---------- */

export type Tone = "ok" | "warn" | "block" | "neutral" | "info";

const ICONS: Record<Tone, string> = {
  ok: "M4 12l5 5L20 6",
  warn: "M12 4v10M12 18v2",
  block: "M5 5l14 14M19 5L5 19",
  neutral: "M5 12h14",
  info: "M12 8v1M12 12v7",
};

/** Estado nunca depende só de cor: todo selo tem palavra e símbolo. */
export function Badge({ tone, children, small }: { tone: Tone; children: ReactNode; small?: boolean }) {
  return (
    <span className={`badge ${tone === "ok" ? "" : tone} ${small ? "small" : ""}`.trim()}>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="4" strokeLinecap="square" aria-hidden="true">
        <path d={ICONS[tone]} />
      </svg>
      {children}
    </span>
  );
}

/** Bolinha de situação (como o ponto "Online" do jogo): verde com visto ou vermelha com X. */
export function Box({ ok }: { ok: boolean }) {
  return (
    <span className={`box ${ok ? "" : "bad"}`} aria-hidden="true">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="4.5" strokeLinecap="square">
        <path d={ok ? ICONS.ok : ICONS.block} />
      </svg>
    </span>
  );
}

/* ---------- ícones dos botões (desenhados aqui, sem arte da Valve) ---------- */

const GLYPHS = {
  search: "M10 4a6 6 0 100 12 6 6 0 000-12zM15 15l6 6",
  folder: "M3 7h6l2 2h10v10H3z",
  refresh: "M20 12a8 8 0 11-3-6.2M20 4v5h-5",
  sound: "M4 9h4l5-4v14l-5-4H4zM16 9a4 4 0 010 6",
  box: "M4 8l8-4 8 4v8l-8 4-8-4zM4 8l8 4 8-4M12 12v8",
  play: "M7 4l13 8-13 8z",
  pause: "M8 5v14M16 5v14",
  check: "M4 12l5 5L20 6",
} as const;

export function Icon({ name, filled }: { name: keyof typeof GLYPHS; filled?: boolean }) {
  return (
    <svg viewBox="0 0 24 24" fill={filled ? "currentColor" : "none"} stroke="currentColor" strokeWidth="3" strokeLinecap="square" strokeLinejoin="miter" aria-hidden="true">
      <path d={GLYPHS[name]} />
    </svg>
  );
}

/* ---------- carregamento assíncrono ---------- */

export interface Async<T> {
  data: T | undefined;
  error: string | undefined;
  loading: boolean;
  reload: () => void;
}

/** Carrega `fn` sempre que `deps` mudam. Respostas atrasadas de pedidos antigos são descartadas. */
export function useAsync<T>(fn: () => Promise<T>, deps: unknown[]): Async<T> {
  const [state, setState] = useState<{ data?: T; error?: string; loading: boolean }>({ loading: true });
  const [tick, setTick] = useState(0);
  const latest = useRef(0);

  useEffect(() => {
    const id = ++latest.current;
    setState((s) => ({ data: s.data, loading: true }));
    fn().then(
      (data) => id === latest.current && setState({ data, loading: false }),
      (e: unknown) =>
        id === latest.current && setState({ error: e instanceof Error ? e.message : String(e), loading: false }),
    );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [...deps, tick]);

  const reload = useCallback(() => setTick((t) => t + 1), []);
  return { data: state.data, error: state.error, loading: state.loading, reload };
}

export function ErrorState({ title, message, onRetry }: { title: string; message: string; onRetry?: () => void }) {
  const { t } = useI18n();
  return (
    <div className="state error" role="alert">
      <h2>{title}</h2>
      <p>{message}</p>
      {onRetry && (
        <button className="btn" onClick={onRetry}>
          {t("common.retry")}
        </button>
      )}
    </div>
  );
}
