import { useEffect, useMemo, useRef, useState } from "react";
import { call } from "./api";
import { useI18n, type TKey } from "./i18n";
import logo from "./assets/logo.png";
import { Browse } from "./screens/Browse";
import { Casual } from "./screens/Casual";
import { Home } from "./screens/Home";
import { Installed } from "./screens/Installed";
import { Settings } from "./screens/Settings";
import { UpdateDialog } from "./screens/UpdateDialog";
import { useUpdate } from "./update";
import type { ArtSet, CasualStatus, Detect, GameIcons, Inspect } from "./types";
import { Icon, useAsync } from "./ui";

type Tab = "home" | "browse" | "casual" | "installed" | "settings";

const TABS: { id: Tab; label: TKey }[] = [
  { id: "home", label: "tab.home" },
  { id: "browse", label: "tab.browse" },
  { id: "casual", label: "tab.casual" },
  { id: "installed", label: "tab.installed" },
  { id: "settings", label: "tab.settings" },
];

const STORE_KEY = "suny.installPath";
const QP_KEY = "suny.quickprecache";
const AUTO_UPDATE_KEY = "suny.autoUpdate";
const SKIP_UPDATE_KEY = "suny.skipUpdate";

function readStored(key: string): string | undefined {
  try {
    return localStorage.getItem(key) ?? undefined;
  } catch {
    return undefined;
  }
}

function writeStored(key: string, value: string | null) {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, value);
  } catch {
    /* sem armazenamento: vale só nesta sessão */
  }
}

/** Pergunta ao núcleo, a cada poucos segundos, se o TF2 está aberto. Barato: só olha os processos. */
function useGameRunning(path: string | undefined): boolean {
  const [running, setRunning] = useState(false);
  useEffect(() => {
    let alive = true;
    const tick = () =>
      call<{ running: boolean }>("game_state", { path: path ?? null }).then(
        (r) => alive && setRunning(r.running),
        () => undefined,
      );
    tick();
    const timer = setInterval(tick, 3000);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [path]);
  return running;
}

export function App() {
  const { t, T, tc } = useI18n();
  const [tab, setTab] = useState<Tab>("home");
  // Abas já abertas continuam montadas (escondidas): o Catálogo não perde filtros nem itens ao ir e voltar.
  const [seen, setSeen] = useState<Set<Tab>>(new Set<Tab>(["home"]));
  const go = (to: Tab) => {
    setTab(to);
    setSeen((s) => new Set(s).add(to));
    document.querySelector(".sheet")?.scrollTo(0, 0);
  };
  const [path, setPath] = useState<string | undefined>(() => readStored(STORE_KEY));
  const [pathError, setPathError] = useState<string | undefined>();
  const [quickprecache, setQuickprecache] = useState(() => readStored(QP_KEY) === "1");

  // Atualizações do app: confere ao abrir (dá para desligar nas Configurações).
  const [autoUpdate, setAutoUpdate] = useState(() => readStored(AUTO_UPDATE_KEY) !== "0");
  const update = useUpdate(autoUpdate);
  const [skipped, setSkipped] = useState<string | undefined>(() => readStored(SKIP_UPDATE_KEY));
  const [updateOpen, setUpdateOpen] = useState(false);
  const [bannerHidden, setBannerHidden] = useState(false);
  const latest = update.info?.latest ?? undefined;
  const showBanner = !!update.info?.available && latest !== skipped && !bannerHidden;

  // Sem pasta guardada: procura o jogo pelo Steam.
  const detect = useAsync<Detect | null>(async () => (path ? null : await call<Detect>("detect")), [path]);
  useEffect(() => {
    const first = detect.data?.installations[0];
    if (!path && first) setPath(first.root);
  }, [detect.data, path]);

  const inspect = useAsync<Inspect | null>(async () => (path ? await call<Inspect>("inspect", { path }) : null), [path]);

  // Pasta guardada que deixou de servir (jogo desinstalado, disco trocado): volta a procurar.
  useEffect(() => {
    if (path && inspect.error) setPathError(inspect.error);
  }, [path, inspect.error]);

  const usePath = (p: string) => {
    setPathError(undefined);
    call<Inspect>("inspect", { path: p }).then(
      () => {
        writeStored(STORE_KEY, p);
        setPath(p);
      },
      (e: unknown) => setPathError(e instanceof Error ? e.message : String(e)),
    );
  };

  const autoDetect = () => {
    writeStored(STORE_KEY, null);
    setPathError(undefined);
    setPath(undefined);
  };

  // Arte do jogo: lida do Steam do usuário. Se falhar, o app segue sem imagens.
  const art = useAsync<ArtSet | null>(() => call<ArtSet>("art").catch(() => null), []);
  // Fundo: a versão desfocada do Steam (não repete o banner nítido do Início). Sem ela, borra o banner por CSS.
  const backdrop = art.data?.hero_blur ?? art.data?.hero ?? null;
  const backdropBlurred = !art.data?.hero_blur;

  // Casual Preloader dentro do app: configurado? quais mods? o jogo está com ele aplicado?
  const casual = useAsync<CasualStatus>(() => call<CasualStatus>("casual_status", { path }), [path]);

  // Estado do jogo ao vivo. Quando abre ou fecha, o resto da tela se atualiza sozinho.
  const running = useGameRunning(path);
  const first = useRef(true);
  const { reload: reloadInspect } = inspect;
  const { reload: reloadCasual } = casual;
  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    reloadInspect();
    reloadCasual();
  }, [running, reloadInspect, reloadCasual]);

  const inspectView = useMemo<Inspect | undefined>(
    () => (inspect.data ? { ...inspect.data, system: { ...inspect.data.system, game_running: running } } : undefined),
    [inspect.data, running],
  );

  // Itens já instalados pelo app, para o Catálogo marcar os cards.
  const installed = new Set((inspect.data?.custom ?? []).flatMap((e) => (e.managed ? [`${e.managed.kind}:${e.managed.gb_id}`] : [])));

  // Ícones de classe do próprio jogo (do disco do usuário). Sem o jogo, o app segue sem eles.
  const icons = useAsync<GameIcons | null>(() => (path ? call<GameIcons>("game_icons", { path }).catch(() => null) : Promise.resolve(null)), [path]);

  // Botão de jogar: abre o TF2 pelo Steam. Fica indisponível enquanto o jogo abre e enquanto ele está aberto.
  const [launching, setLaunching] = useState(false);
  const [launchError, setLaunchError] = useState<string | undefined>();
  useEffect(() => {
    if (running) setLaunching(false);
  }, [running]);
  useEffect(() => {
    if (!launching) return;
    const timer = setTimeout(() => setLaunching(false), 120_000); // o Steam não abriu o jogo: libera o botão de novo
    return () => clearTimeout(timer);
  }, [launching]);
  const canPlay = !!inspect.data && !running && !launching;
  const play = () => {
    setLaunching(true);
    setLaunchError(undefined);
    call("launch_game").catch((e: unknown) => {
      setLaunching(false);
      setLaunchError(e instanceof Error ? e.message : String(e));
      setTimeout(() => setLaunchError(undefined), 8000);
    });
  };

  const build = inspect.data?.installation.build_id;

  return (
    <div className="app">
      {backdrop && <div className={`backdrop ${backdropBlurred ? "soften" : ""}`} style={{ backgroundImage: `url(${backdrop})` }} aria-hidden="true" />}
      <header className="topbar">
        <div className="brand">
          <img src={logo} alt="" width={44} height={44} />
          <span>
            suny&rsquo;s <b>tf2</b> supplier
          </span>
        </div>
        <nav className="tabs" aria-label={t("nav.sections")}>
          {TABS.map((x) => (
            <button key={x.id} className="tab" aria-current={tab === x.id ? "page" : undefined} onClick={() => go(x.id)}>
              {t(x.label)}
            </button>
          ))}
        </nav>
        <div className="top-actions">
          {launchError && (
            <span className="launch-msg bad" role="status" aria-live="polite">
              {tc(launchError)}
            </span>
          )}
          <button
            className={`btn play ${running ? "is-open" : ""}`}
            onClick={play}
            disabled={!canPlay}
            aria-busy={launching}
            title={running ? t("play.title.open") : launching ? t("play.title.launching") : !inspect.data ? t("play.title.noInstall") : t("play.title.ready")}
          >
            <Icon name={running ? "check" : "play"} filled={!running} />
            {running ? t("play.open") : launching ? t("play.launching") : t("play.button")}
          </button>
          <div className="chip" title={t("build.title")}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="square" aria-hidden="true">
              <path d="M4 7h16M4 12h16M4 17h10" />
            </svg>
            Build {build ?? "—"}
          </div>
        </div>
      </header>
      <main className="sheet">
        {showBanner && (
          <div className="update-banner" role="status">
            <span>
              <T k="update.banner" v={{ latest: latest ?? "", current: update.info?.current ?? "" }} />
            </span>
            <span className="update-banner-actions">
              <button className="btn primary mini" onClick={() => setUpdateOpen(true)}>
                {t("update.see")}
              </button>
              <button className="btn flat mini" onClick={() => setBannerHidden(true)}>
                {t("update.later")}
              </button>
            </span>
          </div>
        )}
        {tab === "home" && (
          <Home
            inspect={inspectView}
            error={pathError}
            loading={inspect.loading}
            steamError={detect.data?.steam_error ?? null}
            detecting={detect.loading && !path}
            onUsePath={usePath}
            onRefresh={() => {
              inspect.reload();
              casual.reload();
            }}
            art={art.data ?? null}
            onGoCatalog={() => go("browse")}
            onGoSettings={() => go("settings")}
          />
        )}
        {seen.has("browse") && (
          <div hidden={tab !== "browse"}>
            <Browse
              installRoot={inspect.data?.installation.root}
              installed={installed}
              icons={icons.data?.icons ?? {}}
              onChanged={inspect.reload}
              gameRunning={running}
              casual={{ ready: casual.data?.runtime === "ready", unknown: !casual.data, onGo: () => go("casual"), onAdded: casual.reload }}
            />
          </div>
        )}
        {tab === "casual" && (
          <Casual
            status={casual.data}
            statusError={casual.error}
            loading={casual.loading}
            inspect={inspectView}
            quickprecache={quickprecache}
            onReload={casual.reload}
            onGameChanged={inspect.reload}
            onGoCatalog={() => go("browse")}
            onGoHome={() => go("home")}
          />
        )}
        {tab === "installed" && (
          <Installed inspect={inspectView} loading={inspect.loading} onGoHome={() => go("home")} onGoCatalog={() => go("browse")} onChanged={inspect.reload} />
        )}
        {tab === "settings" && (
          <Settings
            inspect={inspectView}
            pathError={pathError}
            onUsePath={usePath}
            onAutoDetect={autoDetect}
            quickprecache={quickprecache}
            onQuickprecache={(v) => {
              setQuickprecache(v);
              writeStored(QP_KEY, v ? "1" : "0");
            }}
            update={update}
            autoUpdate={autoUpdate}
            onAutoUpdate={(v) => {
              setAutoUpdate(v);
              writeStored(AUTO_UPDATE_KEY, v ? "1" : "0");
            }}
            onOpenUpdate={() => setUpdateOpen(true)}
          />
        )}
      </main>
      {updateOpen && update.info?.available && (
        <UpdateDialog
          info={update.info}
          onClose={() => setUpdateOpen(false)}
          onSkip={() => {
            if (latest) {
              writeStored(SKIP_UPDATE_KEY, latest);
              setSkipped(latest);
            }
            setUpdateOpen(false);
          }}
        />
      )}
      <footer className="footbar">
        <span>
          <T k="footer.left" />
        </span>
        <span>{t("footer.right")}</span>
      </footer>
    </div>
  );
}
