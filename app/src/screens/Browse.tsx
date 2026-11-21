import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { call } from "../api";
import { fmtBytes, fmtDate, fmtInt } from "../format";
import { useI18n, type TKey } from "../i18n";
import type { Category, Details, GameIcons, Kind, Page, Sort, Summary } from "../types";
import { Badge, ErrorState, Icon, useAsync } from "../ui";
import { Analyzer } from "./Analyzer";
import { CasualAddBox } from "./CasualAddBox";
import { InstallBox } from "./InstallBox";

/** Itens pedidos por vez. A grade tem 5 colunas, então 25 fecha 5 linhas. */
const PER_PAGE = 25;

const SORTS: { value: Sort; label: TKey }[] = [
  { value: "MostDownloaded", label: "browse.sort.downloaded" },
  { value: "MostLiked", label: "browse.sort.liked" },
  { value: "Newest", label: "browse.sort.newest" },
];

export interface CasualLink {
  ready: boolean;
  unknown: boolean;
  onGo: () => void;
  onAdded: () => void;
}

interface BrowseProps {
  /** Raiz do TF2 escolhida (ou nada, se ainda não há instalação). */
  installRoot: string | undefined;
  /** Itens já instalados pelo app, no formato `Mod:123` / `Sound:456`. */
  installed: Set<string>;
  /** Ícones de classe lidos do jogo (vazio se o jogo não foi encontrado). */
  icons: GameIcons["icons"];
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

export function Browse({ installRoot, installed, icons, onChanged, gameRunning, casual }: BrowseProps) {
  const { t, tc } = useI18n();
  const [kind, setKind] = useState<Kind>("Mod");
  const [sort, setSort] = useState<Sort>("MostDownloaded");
  const [category, setCategory] = useState<number | undefined>();
  const [catPath, setCatPath] = useState<string[]>([]);
  const [draft, setDraft] = useState("");
  const [search, setSearch] = useState("");
  const [open, setOpen] = useState<{ kind: Kind; id: number } | undefined>();

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
  };

  return (
    <>
      <div className="sheet-head">
        <div>
          <h1>{t("tab.browse")}</h1>
          <p>{t("browse.subtitle")}</p>
        </div>
      </div>

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
          <select className="blank" value={sort} onChange={(e) => setSort(e.target.value as Sort)} disabled={searching}>
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
      {searching && <p className="muted">{t("browse.search.note")}</p>}

      <div className="browse">
        <CategoryTree
          kind={kind}
          selected={category}
          disabled={searching}
          icons={icons}
          onSelect={(id, path) => {
            setCategory(id);
            setCatPath(path);
          }}
        />

        <section className="results" aria-label={t("browse.results")}>
          {catPath.length > 0 && !searching && (
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
                      <li key={r.id}>
                        <button className="card" onClick={() => setOpen({ kind, id: r.id })} aria-label={t("browse.open", { name: r.name })}>
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

      {open && (
        <Modal onClose={() => setOpen(undefined)} labelledBy="ficha-titulo">
          <Ficha kind={open.kind} id={open.id} installRoot={installRoot} icons={icons} onChanged={onChanged} gameRunning={gameRunning} casual={casual} onClose={() => setOpen(undefined)} />
        </Modal>
      )}
    </>
  );
}

/** Árvore de categorias: as filhas só são buscadas quando a categoria é aberta. */
function CategoryTree({
  kind,
  selected,
  disabled,
  icons,
  onSelect,
}: {
  kind: Kind;
  selected: number | undefined;
  disabled: boolean;
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
    const isLeaf = Array.isArray(children) && children.length === 0;
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
            disabled={disabled || isLeaf}
            tabIndex={isLeaf ? -1 : 0}
          >
            {isLeaf ? "" : isOpen ? "▾" : "▸"}
          </button>
          <button className="cat" aria-pressed={selected === c.id} disabled={disabled} onClick={() => onSelect(c.id, myPath)}>
            {icon && <img className="cat-icon" src={icon} alt="" />}
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
            <button className="cat" aria-pressed={selected === undefined} disabled={disabled} onClick={() => onSelect(undefined, [])}>
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

/** Janela sobre a tela: Esc fecha, o foco fica dentro e volta ao card ao fechar. */
function Modal({ children, onClose, labelledBy }: { children: ReactNode; onClose: () => void; labelledBy: string }) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    ref.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (document.querySelector(".lightbox")) return; // o zoom da imagem trata o próprio Esc
        onClose();
        return;
      }
      if (e.key !== "Tab" || !ref.current) return;
      const items = [...ref.current.querySelectorAll<HTMLElement>('button, [href], input, select, [tabindex]:not([tabindex="-1"])')].filter(
        (el) => !el.hasAttribute("disabled"),
      );
      const first = items[0];
      const last = items[items.length - 1];
      if (!first || !last) return;
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      previous?.focus();
    };
  }, [onClose]);

  return (
    <div
      className="modal-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="modal" role="dialog" aria-modal="true" aria-labelledby={labelledBy} tabIndex={-1} ref={ref}>
        {children}
      </div>
    </div>
  );
}

function Ficha({
  kind,
  id,
  installRoot,
  icons,
  onChanged,
  gameRunning,
  casual,
  onClose,
}: {
  kind: Kind;
  id: number;
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
                  <div className="gallery-main empty" aria-hidden="true">
                    <Icon name={kind === "Sound" ? "sound" : "box"} />
                  </div>
                )}
                {count > 1 && (
                  <>
                    <button className="gal-nav prev" onClick={() => go(-1)} aria-label={t("ficha.prev")}>
                      ‹
                    </button>
                    <button className="gal-nav next" onClick={() => go(1)} aria-label={t("ficha.next")}>
                      ›
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
                    <Analyzer key={`a${f.id}`} installRoot={installRoot} kind={kind} modId={id} file={f} icons={icons} />
                    <CasualAddBox key={`c${f.id}`} kind={kind} modId={id} file={f} ready={casual.ready} unknown={casual.unknown} onGoCasual={casual.onGo} onAdded={casual.onAdded} />
                    <InstallBox key={f.id} installRoot={installRoot} kind={kind} modId={id} file={f} gameRunning={gameRunning} onChanged={onChanged} />
                  </div>
                );
              })}
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
                    ‹
                  </button>
                  <button
                    className="gal-nav next"
                    onClick={(e) => {
                      e.stopPropagation();
                      go(1);
                    }}
                    aria-label={t("ficha.next")}
                  >
                    ›
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
