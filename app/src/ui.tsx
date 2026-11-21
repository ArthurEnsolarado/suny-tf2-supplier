import {
  Check,
  ChevronLeft,
  ChevronRight,
  CircleX,
  Download,
  FolderOpen,
  Gamepad2,
  House,
  Info,
  LayoutGrid,
  LoaderCircle,
  Minus,
  Package,
  PackageCheck,
  Pause,
  Play,
  RefreshCw,
  Search,
  Settings,
  TriangleAlert,
  Volume2,
  X,
  type LucideIcon,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { useI18n } from "./i18n";

/* ---------- ícones: Lucide (ISC), traço uniforme em todo o app ---------- */

const ICONS = {
  search: Search,
  folder: FolderOpen,
  refresh: RefreshCw,
  sound: Volume2,
  box: Package,
  play: Play,
  pause: Pause,
  check: Check,
  x: X,
  left: ChevronLeft,
  right: ChevronRight,
  download: Download,
  spinner: LoaderCircle,
  home: House,
  catalog: LayoutGrid,
  casual: Gamepad2,
  installed: PackageCheck,
  settings: Settings,
} satisfies Record<string, LucideIcon>;

export type IconName = keyof typeof ICONS;

/** `filled` preenche o desenho (usado no triângulo de play). */
export function Icon({ name, filled }: { name: IconName; filled?: boolean }) {
  const C = ICONS[name];
  return <C aria-hidden="true" strokeWidth={2.4} absoluteStrokeWidth fill={filled ? "currentColor" : "none"} className={name === "spinner" ? "spin" : undefined} />;
}

/* ---------- selo de estado: a assinatura do app (faixa em ângulo, como no placar do TF2) ---------- */

export type Tone = "ok" | "warn" | "block" | "neutral" | "info";

const TONE_ICON: Record<Tone, LucideIcon> = {
  ok: Check,
  warn: TriangleAlert,
  block: CircleX,
  neutral: Minus,
  info: Info,
};

/** Estado nunca depende só de cor: todo selo tem palavra e símbolo. */
export function Badge({ tone, children, small }: { tone: Tone; children: ReactNode; small?: boolean }) {
  const C = TONE_ICON[tone];
  return (
    <span className={`badge ${tone === "ok" ? "" : tone} ${small ? "small" : ""}`.trim()}>
      <C aria-hidden="true" strokeWidth={3} absoluteStrokeWidth />
      {children}
    </span>
  );
}

/** Bolinha de situação (como o ponto "Online" do jogo): verde com visto ou vermelha com X. */
export function Box({ ok }: { ok: boolean }) {
  const C = ok ? Check : X;
  return (
    <span className={`box ${ok ? "" : "bad"}`} aria-hidden="true">
      <C strokeWidth={3.4} absoluteStrokeWidth />
    </span>
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
