import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { call } from "../api";
import { fmtBytes, fmtDate, fmtInt } from "../format";
import { useI18n, type TKey } from "../i18n";
import type { Category, Details, GameIcons, Kind, Page, Sort, Summary } from "../types";
import { Badge, ErrorState, Icon, useAsync } from "../ui";
import { CatGlyph } from "../catIcons";
import { useSoundPreview } from "../useSoundPreview";
import { Analyzer, AUTO_ANALYZE_MAX } from "./Analyzer";
import { InstallPanel, type CasualLink } from "./InstallPanel";
import { SUPPORTED } from "./InstallBox";
import { Modal } from "./Modal";

/** Itens pedidos por vez. A grade tem 5 colunas, então 25 fecha 5 linhas. */
const PER_PAGE = 25;

const SORTS: { value: Sort; label: TKey }[] = [
  { value: "MostDownloaded", label: "browse.sort.downloaded" },
  { value: "MostLiked", label: "browse.sort.liked" },
  { value: "Newest", label: "browse.sort.newest" },
];

interface BrowseProps {
  /** Raiz do TF2 escolhida (ou nada, se ainda não há instalação). */
  installRoot: string | undefined;
  /** Itens já instalados pelo app, no formato `Mod:123` / `Sound:456`. */
  installed: Set<string>;
  /** Ícones de classe lidos do jogo (vazio se o jogo não foi encontrado). */
  icons: GameIcons["icons"];
  /** Retratos das classes (o rosto dos personagens), lidos do jogo. */
  portraits: GameIcons["icons"];
  onChanged: () => void;
  /** O TF2 está aberto agora. */
  gameRunning: boolean;
  /** Casual Preloader dentro do app (aba Casual). */
  casual: CasualLink;
}

/** "The Demoman" → "demoman": chave do ícone de classe, ou `undefined` se não for uma classe. */
function classKey(name: string): string | undefined {
  const k = name.toLowerCase().replace(/^the\s+/, "").trim();
  return ["scout", "soldier", "pyro", "demoman", "heavy", "engineer", "medic", "sniper", "spy"].includes(k) ? k : undefined;
}

const CLASS_ORDER = ["scout", "soldier", "pyro", "demoman", "heavy", "engineer", "medic", "sniper", "spy"] as const;
const CLASS_LABEL: Record<string, string> = { scout: "Scout", soldier: "Soldier", pyro: "Pyro", demoman: "Demoman", heavy: "Heavy", engineer: "Engineer", medic: "Medic", sniper: "Sniper", spy: "Spy" };

type ClassCats = Record<string, { id: number; path: string[]; chain: number[] }>;
const classCatsCache = new Map<Kind, Promise<ClassCats>>();

/** Acha, no GameBanana, a categoria de cada classe: nos mods `Skins > The Scout`, nos sons `Character Voice > Scout Voice`. */
function loadClassCats(kind: Kind): Promise<ClassCats> {
  let p = classCatsCache.get(kind);
  if (!p) {
    const parentName = kind === "Mod" ? "skins" : "character voice";
    p = call<Category[]>("gb_categories", { kind })
      .then(async (roots) => {
        const parent = roots.find((c) => c.name.toLowerCase() === parentName);
        if (!parent) return {};
        const kids = await call<Category[]>("gb_categories", { kind, parent: parent.id });
        const out: ClassCats = {};
        for (const k of kids) {
          const key = classKey(k.name.replace(/\s+voice$/i, ""));
          if (key) out[key] = { id: k.id, path: [parent.name, k.name], chain: [parent.id, k.id] };
        }
        return out;
      })
      .catch(() => {
        classCatsCache.delete(kind);
        return {};
      });
    classCatsCache.set(kind, p);
  }
  return p;
}

/** Fileira de classes no topo: o rosto de cada personagem (do próprio jogo). Atalho para a categoria da classe. */
function ClassRow({
  kind,
  portraits,
  selected,
  catPath,
  onPick,
}: {
  kind: Kind;
  portraits: GameIcons["icons"];
  selected: number | undefined;
  /** Caminho (nomes) da categoria escolhida, para a classe continuar marcada quando se entra numa subcategoria dela. */
  catPath: string[];
  onPick: (id: number | undefined, path: string[], chain: number[]) => void;
}) {
  const { t } = useI18n();
  const cats = useAsync<ClassCats>(() => loadClassCats(kind), [kind]);
  const have = CLASS_ORDER.filter((k) => cats.data?.[k]);
  if (have.length === 0) return null;
  return (
    <div className="classes" role="group" aria-label={t("browse.classes")}>
      {have.map((k) => {
        const c = cats.data![k]!;
        const on = selected === c.id || (catPath.length >= c.path.length && c.path.every((n, i) => catPath[i] === n));
        return (
          <button
            key={k}
            type="button"
            className={`class-chip ${on ? "on" : ""}`}
            aria-pressed={on}
            title={t(kind === "Mod" ? "browse.class.mod" : "browse.class.sound", { name: CLASS_LABEL[k] ?? k })}
            onClick={() => (on ? onPick(undefined, [], []) : onPick(c.id, c.path, c.chain))}
          >
            {portraits[k] ? <img src={portraits[k]} alt="" /> : <span className="class-face" aria-hidden="true">{(CLASS_LABEL[k] ?? k).slice(0, 1)}</span>}
            <span>{CLASS_LABEL[k] ?? k}</span>
          </button>
        );
      })}
    </div>
  );
}

/** Lista que cresce conforme se rola. Trocar `key` zera tudo; respostas atrasadas de uma busca antiga são descartadas. */
function useInfinite(key: string, fetchPage: (page: number) => Promise<Page<Summary>>) {
  type S = { items: Summary[]; total: number; page: number; done: boolean; status: "loading" | "idle" | "error"; error?: string };
  const [s, setS] = useState<S>({ items: [], total: 0, page: 0, done: false, status: "loading" });
  const gen = useRef(0);
  const cur = useRef(s);
  cur.current = s;
  const fetchRef = useRef(fetchPage);
  fetchRef.current = fetchPage;

  const load = useCallback((page: number) => {
    const g = gen.current;
    setS((p) => ({ ...p, status: "loading", error: undefined }));
    fetchRef.current(page).then(
      (res) => {
        if (g !== gen.current) return;
        setS((p) => {
          const seen = new Set(p.items.map((i) => i.id));
          const items = page === 1 ? res.records : [...p.items, ...res.records.filter((r) => !seen.has(r.id))];
          return { items, total: res.total, page, done: res.records.length === 0 || items.length >= res.total, status: "idle" };
        });
      },
      (e: unknown) => {
        if (g !== gen.current) return;
        setS((p) => ({ ...p, status: "error", error: e instanceof Error ? e.message : String(e) }));
      },
    );
  }, []);

  useEffect(() => {
    gen.current++;
    setS({ items: [], total: 0, page: 0, done: false, status: "loading" });
    load(1);
  }, [key, load]);

  const more = useCallback(() => {
    const c = cur.current;
    if (c.status !== "idle" || c.done || c.page === 0) return;
    load(c.page + 1);
  }, [load]);

  const retry = useCallback(() => load(Math.max(1, cur.current.page + (cur.current.page === 0 ? 0 : 1))), [load]);

  return { ...s, more, retry };
}

export function Browse({ installRoot, installed, icons, portraits, onChanged, gameRunning, casual }: BrowseProps) {
  const { t, tc } = useI18n();
  const sound = useSoundPreview();
  const [kind, setKind] = useState<Kind>("Mod");
  const [sort, setSort] = useState<Sort>("MostDownloaded");
  const [category, setCategory] = useState<number | undefined>();
  const [catPath, setCatPath] = useState<string[]>([]);
  /** Ids de categorias que a árvore deve abrir (o ramo da classe escolhida). */
  const [reveal, setReveal] = useState<number[]>([]);
  const [draft, setDraft] = useState("");
  const [search, setSearch] = useState("");
  const [open, setOpen] = useState<{ kind: Kind; id: number; root?: string } | undefined>();
  const key = JSON.stringify([kind, sort, category ?? null, search]);
  const list = useInfinite(key, (page) =>
    call<Page<Summary>>("gb_list", {
      kind,
      sort,
      query: { page, per_page: PER_PAGE, category: category ?? null, search: search || null },
    }),
  );

  // Rolagem infinita: quando o fim da lista chega perto da tela, pede a próxima página.
  const sentinel = useRef<HTMLDivElement>(null);
  const { more, items, status, done } = list;
  useEffect(() => {
    const el = sentinel.current;
    if (!el) return;
    const io = new IntersectionObserver((es) => es.some((e) => e.isIntersecting) && more(), { root: el.closest(".sheet"), rootMargin: "700px 0px" });
    io.observe(el);
    return () => io.disconnect();
  }, [more, items.length, status, done]);

  const searching = search.length > 0;
  const pickKind = (k: Kind) => {
    setKind(k);
    setCategory(undefined);
    setCatPath([]);
    setReveal([]);
  };

  return (
    <>
      <div className="sheet-head">
        <div>
          <h1>{t("tab.browse")}</h1>
          <p>{t("browse.subtitle")}</p>
        </div>
      </div>

      <ClassRow
        kind={kind}
        portraits={portraits}
        selected={category}
        catPath={catPath}
        onPick={(id, path, chain) => {
          setCategory(id);
          setCatPath(path);
          setReveal(chain);
        }}
      />

      <form
        className="row"
        onSubmit={(e) => {
          e.preventDefault();
          setSearch(draft.trim());
        }}
      >
        <div className="toggle" role="group" aria-label={t("browse.type")}>
          <button type="button" aria-pressed={kind === "Mod"} onClick={() => pickKind("Mod")}>
            {t("browse.mods")}
          </button>
          <button type="button" aria-pressed={kind === "Sound"} onClick={() => pickKind("Sound")}>
            {t("browse.sounds")}
          </button>
        </div>
        <label className="field grow">
          <span>{t("browse.search.label")}</span>
          <input className="blank" value={draft} onChange={(e) => setDraft(e.target.value)} placeholder={t("browse.search.placeholder")} spellCheck={false} />
        </label>
        <label className="field">
          <span>{t("browse.sort")}</span>
          <select className="blank" value={sort} onChange={(e) => setSort(e.target.value as Sort)}>
            {SORTS.map((s) => (
              <option key={s.value} value={s.value}>
                {t(s.label)}
              </option>
            ))}
          </select>
        </label>
        <button className="btn primary" type="submit">
          <Icon name="search" />
          {t("browse.search.button")}
        </button>
        {searching && (
          <button
            className="btn flat"
            type="button"
            onClick={() => {
              setDraft("");
              setSearch("");
            }}
          >
            {t("browse.search.clear")}
          </button>
        )}
      </form>

      <audio ref={sound.audioRef} onEnded={sound.onEnded} />
      <div className="browse">
        <CategoryTree
          kind={kind}
          selected={category}
          reveal={reveal}
          icons={icons}
          onSelect={(id, path) => {
            setCategory(id);
            setCatPath(path);
          }}
        />

        <section className="results" aria-label={t("browse.results")}>
          {catPath.length > 0 && (
            <p className="crumb">
              <span className="muted">{t("browse.category")}</span> <b>{catPath.join(" › ")}</b>
            </p>
          )}

          {list.status === "error" && list.items.length === 0 ? (
            <ErrorState title={t("browse.error")} message={tc(list.error ?? t("browse.error.unknown"))} onRetry={list.retry} />
          ) : (
            <>
              <ul className="grid" aria-busy={list.status === "loading"}>
                {list.items.length === 0 && list.status === "loading"
                  ? Array.from({ length: 10 }, (_, i) => (
                      <li key={i}>
                        <div className="card skeleton-card" />
                      </li>
                    ))
                  : list.items.map((r) => (
                      <li key={r.id} className="cardwrap">
                        <button className="card" onClick={() => setOpen({ kind, id: r.id, root: r.root_category?.name })} aria-label={t("browse.open", { name: r.name })}>
                          <span className={`card-img ${r.thumb ? "" : "noimg"}`}>
                            {r.thumb ? (
                              <img src={r.thumb} alt="" loading="lazy" referrerPolicy="no-referrer" />
                            ) : (
                              <span className="card-empty" aria-hidden="true">
                                <Icon name={kind === "Sound" ? "sound" : "box"} />
                              </span>
                            )}
                            {installed.has(`${kind}:${r.id}`) && (
                              <span className="card-flag">
                                <Badge tone="ok" small>
                                  {t("card.installed")}
                                </Badge>
                              </span>
                            )}
                          </span>
                          <span className="card-name">
                            <span className="clamp">{r.name}</span>
                          </span>
                          <span className="card-author">{r.submitter?.name ?? "—"}</span>
                          <span className="card-foot">
                            <span className="tag">{r.root_category?.name ?? "—"}</span>
                            <span className="likes">♥ {fmtInt(r.like_count)}</span>
                          </span>
                        </button>
                        {kind === "Sound" && (
                          <button
                            className={`card-play ${sound.cur?.id === r.id ? sound.cur.state : ""}`}
                            onClick={() => sound.toggle(r.id)}
                            aria-label={`${sound.cur?.id === r.id && sound.cur.state === "playing" ? t("card.stop") : t("card.listen")}: ${r.name}`}
                            title={sound.cur?.id === r.id && sound.cur.state === "error" ? t("card.previewFailed", { msg: tc(sound.cur.msg ?? "") }) : t("card.listen")}
                          >
                            <Icon name={sound.cur?.id === r.id ? (sound.cur.state === "loading" ? "spinner" : sound.cur.state === "playing" ? "pause" : "x") : "play"} filled={!(sound.cur?.id === r.id)} />
                          </button>
                        )}
                        {kind === "Sound" && sound.cur?.id === r.id && sound.cur.state === "playing" && (
                          <span className="card-play-msg" role="status">
                            {sound.cur.name}
                            {(sound.cur.total ?? 0) > 1 ? ` · ${t("card.moreSounds", { n: (sound.cur.total ?? 1) - 1 })}` : ""}
                          </span>
                        )}
                      </li>
                    ))}
              </ul>

              {list.items.length === 0 && list.status === "idle" && (
                <div className="state">
                  <h2>{t("browse.empty.title")}</h2>
                  <p>{t("browse.empty.body")}</p>
                </div>
              )}

              <div ref={sentinel} className="sentinel" aria-hidden="true" />
              <p className="status-line" role="status" aria-live="polite">
                {list.status === "loading" && list.items.length > 0 && t("browse.loadingMore")}
                {list.status === "error" && list.items.length > 0 && (
                  <>
                    {t("browse.loadMoreFailed", { err: tc(list.error ?? "") })}{" "}
                    <button className="link" onClick={list.retry}>
                      {t("common.retry")}
                    </button>
                  </>
                )}
                {list.status === "idle" && list.items.length > 0 && (list.done ? t("browse.end", { n: list.items.length }) : t("browse.progress", { shown: fmtInt(list.items.length), total: fmtInt(list.total) }))}
              </p>
            </>
          )}
        </section>
      </div>

      {kind === "Sound" && sound.cur?.state === "error" && (
        <div className="toast" role="alert">
          <Icon name="x" />
          <span>{t("card.previewFailed", { msg: tc(sound.cur.msg ?? "") })}</span>
          <button className="btn flat mini" onClick={sound.stop}>
            {t("common.close")}
          </button>
        </div>
      )}

      {open && (
        <Modal onClose={() => setOpen(undefined)} labelledBy="ficha-titulo">
          <Ficha kind={open.kind} id={open.id} root={open.root} installRoot={installRoot} icons={icons} onChanged={onChanged} gameRunning={gameRunning} casual={casual} onClose={() => setOpen(undefined)} />
        </Modal>
      )}
    </>
  );
}

/** Árvore de categorias: as filhas só são buscadas quando a categoria é aberta. */
function CategoryTree({
  kind,
  selected,
  reveal,
  icons,
  onSelect,
}: {
  kind: Kind;
  selected: number | undefined;
  /** Categorias que devem ficar abertas (o ramo da classe escolhida). */
  reveal: number[];
  icons: GameIcons["icons"];
  onSelect: (id: number | undefined, path: string[]) => void;
}) {
  const { t, tc } = useI18n();
  const roots = useAsync<Category[]>(() => call<Category[]>("gb_categories", { kind }).then(sortCats), [kind]);
  const [kids, setKids] = useState<Record<number, Category[] | "loading" | "error">>({});
  const [expanded, setExpanded] = useState<Set<number>>(new Set());

  // outra aba (Mods/Sons) = outra árvore
  useEffect(() => {
    setKids({});
    setExpanded(new Set());
  }, [kind]);

  // A classe escolhida no topo abre o ramo dela na árvore, para dar para escolher a subcategoria (chapéus, armas...).
  useEffect(() => {
    for (const id of reveal) {
      setExpanded((prev) => (prev.has(id) ? prev : new Set(prev).add(id)));
      setKids((prev) => {
        if (prev[id] !== undefined) return prev;
        call<Category[]>("gb_categories", { kind, parent: id }).then(
          (list) => setKids((p) => ({ ...p, [id]: sortCats(list) })),
          () => setKids((p) => ({ ...p, [id]: "error" })),
        );
        return { ...prev, [id]: "loading" };
      });
    }
  }, [reveal, kind]);

  const toggle = (c: Category) => {
    const isOpen = expanded.has(c.id);
    setExpanded((prev) => {
      const next = new Set(prev);
      if (isOpen) next.delete(c.id);
      else next.add(c.id);
      return next;
    });
    if (!isOpen && kids[c.id] === undefined) {
      setKids((p) => ({ ...p, [c.id]: "loading" }));
      call<Category[]>("gb_categories", { kind, parent: c.id }).then(
        (list) => setKids((p) => ({ ...p, [c.id]: sortCats(list) })),
        () => setKids((p) => ({ ...p, [c.id]: "error" })),
      );
    }
  };

  const node = (c: Category, depth: number, path: string[]): ReactNode => {
    const myPath = [...path, c.name];
    const children = kids[c.id];
    // Só mostra a seta quando a API diz que há subcategorias (antes todas tinham seta, e muitas abriam vazias).
    const isLeaf = c.child_count === 0 || (Array.isArray(children) && children.length === 0);
    const isOpen = expanded.has(c.id);
    const ck = classKey(c.name);
    const icon = ck ? icons[ck] : undefined;
    return (
      <li key={c.id}>
        <div className="cat-row" style={{ paddingLeft: depth * 14 }}>
          <button
            className="chev"
            onClick={() => toggle(c)}
            aria-expanded={isOpen}
            aria-label={`${isOpen ? t("browse.collapse") : t("browse.expand")} ${c.name}`}
            disabled={isLeaf}
            tabIndex={isLeaf ? -1 : 0}
          >
            {isLeaf ? "" : isOpen ? "▾" : "▸"}
          </button>
          <button className="cat" aria-pressed={selected === c.id} onClick={() => onSelect(c.id, myPath)}>
            {icon ? <img className="cat-icon" src={icon} alt="" /> : <CatGlyph name={c.name} />}
            <span>{ck ? c.name.replace(/^the\s+/i, "") : c.name}</span>
            <span className="count">{fmtInt(c.item_count)}</span>
          </button>
        </div>
        {isOpen && (
          <ul>
            {children === "loading" && <li className="muted tree-note">{t("common.loading")}</li>}
            {children === "error" && <li className="muted tree-note">{t("browse.tree.failed")}</li>}
            {Array.isArray(children) && children.map((k) => node(k, depth + 1, myPath))}
          </ul>
        )}
      </li>
    );
  };

  return (
    <nav className="cats" aria-label={t("browse.categories")}>
      <h2>{t("browse.categories")}</h2>
      <ul>
        <li>
          <div className="cat-row">
            <span className="chev" aria-hidden="true" />
            <button className="cat" aria-pressed={selected === undefined} onClick={() => onSelect(undefined, [])}>
              {icons.all && <img className="cat-icon" src={icons.all} alt="" />}
              <span>{t("browse.all")}</span>
            </button>
          </div>
        </li>
        {roots.data?.map((c) => node(c, 0, []))}
      </ul>
      {roots.error && <p className="muted tree-note">{t("browse.tree.unavailable", { err: tc(roots.error) })}</p>}
      {roots.data && roots.data.length >= 50 && <p className="muted small tree-note">{t("browse.tree.max")}</p>}
    </nav>
  );
}

const sortCats = (list: Category[]): Category[] => [...new Map(list.map((c) => [c.id, c])).values()].sort((a, b) => a.name.localeCompare(b.name));

function Ficha({
  kind,
  id,
  root,
  installRoot,
  icons,
  onChanged,
  gameRunning,
  casual,
  onClose,
}: {
  kind: Kind;
  id: number;
  /** Categoria raiz do item (vem do cartão). `Maps` = mapa. */
  root?: string;
  installRoot: string | undefined;
  icons: GameIcons["icons"];
  onChanged: () => void;
  gameRunning: boolean;
  casual: CasualLink;
  onClose: () => void;
}) {
  const { t, tc } = useI18n();
  const d = useAsync<Details>(() => call<Details>("gb_details", { kind, id }), [kind, id]);
  const [copied, setCopied] = useState(false);
  const [idx, setIdx] = useState(0);
  const [zoom, setZoom] = useState(false);
  const [moreDesc, setMoreDesc] = useState(false);

  const isMap = kind === "Mod" && (root ?? "").toLowerCase() === "maps";
  const imgs = d.data?.images ?? [];
  const count = imgs.length;
  const go = useCallback((delta: number) => setIdx((i) => (count ? (i + delta + count) % count : 0)), [count]);

  // ← → passam as imagens; Esc fecha o zoom (o Modal deixa o Esc para ele enquanto o zoom está aberto)
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const el = e.target as HTMLElement | null;
      if (el && ["INPUT", "SELECT", "TEXTAREA"].includes(el.tagName)) return;
      if (e.key === "ArrowLeft") go(-1);
      else if (e.key === "ArrowRight") go(1);
      else if (e.key === "Escape") setZoom(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [go]);

  const cur = imgs[idx];
  const soundFile =
    kind === "Sound"
      ? d.data?.files.find((f) => !(f.analysis_warnings && "contains_exe" in f.analysis_warnings) && SUPPORTED.includes(f.file_name.toLowerCase().split(".").pop() ?? "") && f.size <= AUTO_ANALYZE_MAX)
      : undefined;
  const desc = d.data?.description ?? null;
  const longDesc = !!desc && desc.length > 700;
  const shown = desc && longDesc && !moreDesc ? `${desc.slice(0, 700).trimEnd()}…` : desc;

  return (
    <>
      <button className="btn flat modal-close" onClick={onClose}>
        {t("common.close")}
      </button>
      {d.error ? (
        <ErrorState title={t("ficha.error")} message={tc(d.error)} onRetry={d.reload} />
      ) : !d.data ? (
        <h2 id="ficha-titulo">{t("ficha.opening")}</h2>
      ) : (
        <>
          <h2 id="ficha-titulo" className="modal-title">
            {d.data.name}
          </h2>
          <div className="modal-body">
            <div className="modal-media">
              <div className="gallery">
                {cur ? (
                  <button className="gallery-main" onClick={() => setZoom(true)} aria-label={t("ficha.zoom")}>
                    <img src={cur.url} alt={t("ficha.imageOf", { name: d.data.name, i: idx + 1, total: count })} referrerPolicy="no-referrer" />
                  </button>
                ) : (
                  <div className="gallery-main empty">
                    <Icon name={kind === "Sound" ? "sound" : "box"} />
                    <span>{t("ficha.noImages")}</span>
                  </div>
                )}
                {count > 1 && (
                  <>
                    <button className="gal-nav prev" onClick={() => go(-1)} aria-label={t("ficha.prev")}>
                      <Icon name="left" />
                    </button>
                    <button className="gal-nav next" onClick={() => go(1)} aria-label={t("ficha.next")}>
                      <Icon name="right" />
                    </button>
                    <span className="gal-count" aria-hidden="true">
                      {idx + 1} / {count}
                    </span>
                  </>
                )}
              </div>
              {count > 1 && (
                <div className="gallery-strip" role="group" aria-label={t("ficha.gallery")}>
                  {imgs.map((im, i) => (
                    <button key={im.url} className="thumb" aria-current={i === idx ? "true" : undefined} onClick={() => setIdx(i)} aria-label={t("ficha.showImage", { i: i + 1 })}>
                      <img src={im.thumb} alt="" loading="lazy" referrerPolicy="no-referrer" />
                    </button>
                  ))}
                </div>
              )}

              {soundFile && (
                <>
                  <h3>{t("ficha.listen")}</h3>
                  <Analyzer key={`s${soundFile.id}`} installRoot={installRoot} kind={kind} modId={id} file={soundFile} icons={icons} auto view="sounds" />
                </>
              )}

              <h3>{t("ficha.about")}</h3>
              {shown ? (
                <div className="desc">
                  {shown.split(/\n{2,}/).map((para, i) => (
                    <p key={i}>
                      {para.split("\n").map((line, j) => (
                        <span key={j}>
                          {j > 0 && <br />}
                          {line}
                        </span>
                      ))}
                    </p>
                  ))}
                  {longDesc && (
                    <button className="link" onClick={() => setMoreDesc((v) => !v)}>
                      {moreDesc ? t("ficha.showLess") : t("ficha.showMore")}
                    </button>
                  )}
                </div>
              ) : (
                <p className="muted">{t("ficha.noDescription")}</p>
              )}
            </div>

            <div className="modal-side">
              <div className="stats" aria-label={t("ficha.stats")}>
                <span>
                  <b>{fmtInt(d.data.download_count)}</b> {t("ficha.downloads")}
                </span>
                <span>
                  <b>♥ {fmtInt(d.data.like_count)}</b> {t("ficha.likes")}
                </span>
                <span>
                  <b>{fmtInt(d.data.view_count)}</b> {t("ficha.views")}
                </span>
              </div>
              <h3>{t("ficha.files", { n: d.data.files.length })}</h3>
              {d.data.files.length === 0 && <p className="muted">{t("ficha.noFiles")}</p>}
              {d.data.files.map((f) => {
                const exe = f.analysis_warnings && "contains_exe" in f.analysis_warnings;
                return (
                  <div className="file" key={f.id}>
                    <span className="legend">{f.file_name}</span>
                    <b>{fmtBytes(f.size)}</b>
                    <div>
                      {t("ficha.antivirus")} <b>{f.av_result ?? t("ficha.noResult")}</b>
                    </div>
                    {exe && (
                      <p>
                        <Badge tone="block" small>
                          {t("ficha.hasExe")}
                        </Badge>
                      </p>
                    )}
                    {f.md5 && (
                      <div>
                        <code>MD5 {f.md5}</code>
                      </div>
                    )}
                    <Analyzer key={`a${f.id}`} installRoot={installRoot} kind={kind} modId={id} file={f} icons={icons} hideSounds={soundFile?.id === f.id} />
                    <InstallPanel key={f.id} installRoot={installRoot} kind={kind} modId={id} file={f} gameRunning={gameRunning} casual={casual} isMap={isMap} onChanged={onChanged} />
                  </div>
                );
              })}

              <dl className="ficha-dl">
                <dt>{t("ficha.author")}</dt>
                <dd>{d.data.submitter?.name ?? "—"}</dd>
                <dt>{t("ficha.category")}</dt>
                <dd>{d.data.category?.name ?? "—"}</dd>
                <dt>{t("ficha.version")}</dt>
                <dd>{d.data.version || t("ficha.notInformed")}</dd>
                {d.data.date_added && (
                  <>
                    <dt>{t("ficha.added")}</dt>
                    <dd>{fmtDate(d.data.date_added)}</dd>
                  </>
                )}
                {d.data.date_modified && d.data.date_modified !== d.data.date_added && (
                  <>
                    <dt>{t("ficha.updated")}</dt>
                    <dd>{fmtDate(d.data.date_modified)}</dd>
                  </>
                )}
                <dt>{t("ficha.license")}</dt>
                <dd title={d.data.license ? stripHtml(d.data.license) : undefined}>{d.data.license ? stripHtml(d.data.license).slice(0, 90) : t("ficha.notInformed")}</dd>
                <dt>{t("ficha.page")}</dt>
                <dd>
                  <button className="link" onClick={() => navigator.clipboard?.writeText(d.data!.profile_url).then(() => setCopied(true))}>
                    {copied ? t("ficha.linkCopied") : t("ficha.copyLink")}
                  </button>
                </dd>
              </dl>
              {d.data.tags.length > 0 && (
                <p className="taglist">
                  {d.data.tags.map((tag) => (
                    <span className="tag" key={tag}>
                      {tag}
                    </span>
                  ))}
                </p>
              )}
              {d.data.is_obsolete && (
                <Badge tone="warn" small>
                  {t("ficha.obsolete")}
                </Badge>
              )}

            </div>
          </div>
          {zoom && cur && (
            <div className="lightbox" role="dialog" aria-modal="true" aria-label={t("ficha.zoom")} onClick={() => setZoom(false)}>
              <img src={cur.url} alt={t("ficha.imageOf", { name: d.data.name, i: idx + 1, total: count })} referrerPolicy="no-referrer" />
              {count > 1 && (
                <>
                  <button
                    className="gal-nav prev"
                    onClick={(e) => {
                      e.stopPropagation();
                      go(-1);
                    }}
                    aria-label={t("ficha.prev")}
                  >
                    <Icon name="left" />
                  </button>
                  <button
                    className="gal-nav next"
                    onClick={(e) => {
                      e.stopPropagation();
                      go(1);
                    }}
                    aria-label={t("ficha.next")}
                  >
                    <Icon name="right" />
                  </button>
                </>
              )}
              <button className="btn flat lightbox-close" onClick={() => setZoom(false)}>
                {t("common.close")}
              </button>
              <span className="gal-count">
                {idx + 1} / {count}
              </span>
            </div>
          )}
        </>
      )}
    </>
  );
}

function stripHtml(s: string): string {
  return s.replace(/<[^>]*>/g, " ").replace(/&nbsp;| /g, " ").replace(/\s+/g, " ").trim();
}
