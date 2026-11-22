import { useMemo, useState, type ReactNode } from "react";
import { call } from "../api";
import type { CustomEntry, CustomReport, Inspect, ItemContents, ManagedInfo, Role } from "../types";
import { fmtBytes } from "../format";
import { useI18n } from "../i18n";
import { Badge, useAsync } from "../ui";
import { Modal } from "./Modal";
import { ModCard } from "./ModCard";

interface Props {
  inspect: Inspect | undefined;
  loading: boolean;
  onGoHome: () => void;
  onGoCatalog: () => void;
  /** Chamado depois de remover, para a lista se atualizar. */
  onChanged: () => void;
  /** O usuário escolheu (ou colou) arquivos ou uma pasta de um mod que já tem. */
  onImport: (paths: string[]) => void;
}

/** Um mod que ocupa vários itens em `custom` (um pacote com dezenas de `.vpk`, por exemplo) vira UM cartão. */
interface Group {
  key: string;
  title: string;
  managed: ManagedInfo | null;
  entries: CustomEntry[];
}

/** "eyecrafter_a", "Eyecrafter_B", "sword2": a mesma raiz sem o sufixo de variante. Só serve para agrupar o que foi posto à mão. */
const stemOf = (name: string): string =>
  name
    .replace(/\.vpk$/i, "")
    .replace(/([\s._-]+([a-z]|v?\d+|\(\d+\))|\(\d+\)|\d+)$/i, "")
    .trim();

export function Installed({ inspect, loading, onGoHome, onGoCatalog, onChanged, onImport }: Props) {
  const { t, T, tc } = useI18n();
  const [showAllConflicts, setShowAllConflicts] = useState(false);
  const [confirm, setConfirm] = useState<string | undefined>();
  const [busy, setBusy] = useState<string | undefined>();
  const [error, setError] = useState<string | undefined>();
  const [openGroup, setOpenGroup] = useState<string | undefined>();
  const [pasted, setPasted] = useState("");
  const [pickError, setPickError] = useState<string | undefined>();
  const pick = (folder: boolean) => {
    setPickError(undefined);
    call<{ paths: string[] }>("pick_files", { folder }).then(
      (r) => r.paths.length > 0 && onImport(r.paths),
      (e: unknown) => setPickError(e instanceof Error ? e.message : String(e)),
    );
  };

  const roleLabel: Record<Role, string> = {
    mod: t("installed.role.mod"),
    preloader: t("installed.role.preloader"),
    preloader_backup: t("installed.role.backup"),
    workshop: t("installed.role.workshop"),
  };
  const systemHelp: Record<Role, string> = {
    mod: "",
    preloader: t("installed.help.preloader"),
    preloader_backup: t("installed.help.backup"),
    workshop: t("installed.help.workshop"),
  };

  // O que cada item muda e quem disputa arquivo com quem: lido do conteúdo, só leitura. Roda à parte para a lista não esperar.
  const root0 = inspect?.installation.root;
  const signature = useMemo(() => (inspect ? inspect.custom.map((e) => `${e.name}:${e.size_bytes}`).join("|") : ""), [inspect]);
  const contents = useAsync<CustomReport | null>(() => (root0 ? call<CustomReport>("custom_report", { path: root0 }).catch(() => null) : Promise.resolve(null)), [root0, signature]);

  if (!inspect) {
    return (
      <>
        <div className="sheet-head">
          <h1>{loading ? t("installed.reading") : t("tab.installed")}</h1>
        </div>
        {!loading && (
          <div className="state">
            <h2>{t("installed.noinstall.title")}</h2>
            <p>{t("installed.noinstall.body")}</p>
            <button className="btn primary" onClick={onGoHome}>
              {t("common.goHome")}
            </button>
          </div>
        )}
      </>
    );
  }

  const root = inspect.installation.root;
  const running = inspect.system.game_running;
  const list: CustomEntry[] = inspect.custom;
  const mine = list.filter((e) => e.managed);
  const manual = list.filter((e) => e.role === "mod" && !e.managed);
  const system = list.filter((e) => e.role !== "mod" && !e.managed);

  const report = contents.data ?? null;
  const byItem = new Map<string, ItemContents>((report?.items ?? []).map((i) => [i.name, i]));
  const entryByName = new Map(list.map((e) => [e.name, e]));
  /** Nome legível de um item de custom: o do GameBanana quando o app instalou. */
  const nice = (n: string): string => {
    const m = entryByName.get(n)?.managed;
    return m ? `${m.name} (#${m.gb_id})` : n;
  };
  const tags = (name: string): ReactNode => {
    const c = byItem.get(name);
    if (!c) return null;
    return (
      <span className="card-tags">
        {c.groups.slice(0, 2).map((g) => (
          <span className="tag" key={g.label}>
            {tc(g.label)}
          </span>
        ))}
        {c.truncated && <span className="tag">{t("installed.contents.partial")}</span>}
        {c.loses > 0 && (
          <span className="tag warn" title={c.rivals.map(nice).join(", ")}>
            {t("installed.contents.loses", { files: t("files", { n: c.loses }) })}
          </span>
        )}
        {c.loses === 0 && c.wins > 0 && <span className="tag">{t("installed.contents.wins", { files: t("files", { n: c.wins }) })}</span>}
      </span>
    );
  };
  const metaOf = (e: CustomEntry, extra = ""): ReactNode => (
    <>
      {`${t("files", { n: e.file_count })} · ${fmtBytes(e.size_bytes)}${extra}`}
      {tags(e.name)}
    </>
  );

  const groupMeta = (g: Group, extra: string): ReactNode => (
    <>
      {`${t("installed.group.items", { n: g.entries.length })} · ${t("files", { n: g.entries.reduce((s, e) => s + e.file_count, 0) })} · ${fmtBytes(g.entries.reduce((s, e) => s + e.size_bytes, 0))}${extra}`}
    </>
  );
  // Instalados pelo app: um cartão por mod (a chave é o slug), mesmo que ele ocupe dezenas de itens.
  const mineGroups: Group[] = [];
  for (const e of mine) {
    const slug = e.managed!.slug;
    const g = mineGroups.find((x) => x.key === slug);
    if (g) g.entries.push(e);
    else mineGroups.push({ key: slug, title: e.managed!.name, managed: e.managed, entries: [e] });
  }
  // À mão: só agrupa quando 3 ou mais itens têm a mesma raiz (ex.: pacote_01 ... pacote_32).
  const stems = new Map<string, CustomEntry[]>();
  for (const e of manual) {
    const s = stemOf(e.name).toLowerCase();
    stems.set(s, [...(stems.get(s) ?? []), e]);
  }
  const manualGroups: Group[] = [];
  const seenStem = new Set<string>();
  for (const e of manual) {
    const s = stemOf(e.name).toLowerCase();
    const bunch = stems.get(s) ?? [e];
    if (s.length >= 4 && bunch.length >= 3) {
      if (!seenStem.has(s)) {
        seenStem.add(s);
        manualGroups.push({ key: `stem:${s}`, title: stemOf(bunch[0]!.name), managed: null, entries: bunch });
      }
    } else {
      manualGroups.push({ key: `one:${e.name}`, title: e.name, managed: null, entries: [e] });
    }
  }
  const shownGroup = [...mineGroups, ...manualGroups].find((g) => g.key === openGroup);

  const remove = (slug: string) => {
    setBusy(slug);
    setError(undefined);
    call("uninstall_mod", { path: root, slug }).then(
      () => {
        setBusy(undefined);
        setConfirm(undefined);
        onChanged();
      },
      (e: unknown) => {
        setBusy(undefined);
        setError(e instanceof Error ? e.message : String(e));
      },
    );
  };

  return (
    <>
      <div className="sheet-head">
        <div>
          <h1>{t("tab.installed")}</h1>
          <p>{t("installed.subtitle")}</p>
        </div>
        {inspect.preloader_applied ? (
          <Badge tone="ok">{t("installed.preloader.applied")}</Badge>
        ) : inspect.preloader_leftover ? (
          <Badge tone="warn">{t("installed.preloader.leftover")}</Badge>
        ) : (
          <Badge tone="neutral">{t("installed.preloader.none")}</Badge>
        )}
      </div>

      <section aria-labelledby="h-own" className="own">
        <h2 id="h-own">{t("import.entry.title")}</h2>
        <p className="muted">{t("import.entry.body")}</p>
        <div className="row">
          <button className="btn primary" onClick={() => pick(false)}>
            {t("import.entry.files")}
          </button>
          <button className="btn" onClick={() => pick(true)}>
            {t("import.entry.folder")}
          </button>
        </div>
        <form
          className="row own-paste"
          onSubmit={(e) => {
            e.preventDefault();
            const v = pasted.trim().replace(/^"(.*)"$/, "$1");
            if (v) {
              onImport([v]);
              setPasted("");
            }
          }}
        >
          <label className="field">
            <span>{t("import.entry.paste")}</span>
            <input className="blank" value={pasted} onChange={(e) => setPasted(e.target.value)} placeholder="C:\Users\…\meu_mod.zip" spellCheck={false} />
          </label>
          <button className="btn flat" type="submit" disabled={!pasted.trim()}>
            {t("import.entry.use")}
          </button>
        </form>
        {pickError && (
          <p className="muted small" role="alert">
            {tc(pickError)}
          </p>
        )}
      </section>

      {error && (
        <div className="state error" role="alert">
          <h2>{t("installed.remove.failed")}</h2>
          <p>{tc(error)}</p>
        </div>
      )}

      <section aria-labelledby="h-mine">
        <h2 id="h-mine">{t("installed.mine", { n: mineGroups.length })}</h2>
        {running && (
          <p>
            <Badge tone="warn" small>
              {t("status.open")}
            </Badge>{" "}
            {t("installed.closeToRemove")}
          </p>
        )}
        {mineGroups.length === 0 ? (
          <p>
            {t("installed.none")}{" "}
            <button className="link" onClick={onGoCatalog}>
              {t("installed.browse")}
            </button>
          </p>
        ) : (
          <ul className="mgrid">
            {mineGroups.map((g) => {
              const m = g.managed!;
              const many = g.entries.length > 1;
              return (
                <ModCard
                  key={g.key}
                  kind={m.kind}
                  gbId={m.gb_id}
                  title={m.name}
                  subtitle={m.submitter ? t("common.by", { who: m.submitter }) : m.kind === "Local" ? t("installed.local") : undefined}
                  meta={many ? groupMeta(g, m.version ? ` · v${m.version}` : "") : metaOf(g.entries[0]!, m.version ? ` · v${m.version}` : "")}
                  flag={
                    <Badge tone="ok" small>
                      {t("card.installed")}
                    </Badge>
                  }
                  actions={
                    confirm === m.slug ? (
                      <div className="confirm">
                        <p>
                          <T k="installed.remove.confirm" v={{ name: m.name }} />
                        </p>
                        <div className="row">
                          <button className="btn primary mini" disabled={busy === m.slug || running} onClick={() => remove(m.slug)}>
                            {busy === m.slug ? t("installed.removing") : t("common.remove")}
                          </button>
                          <button className="btn flat mini" disabled={!!busy} onClick={() => setConfirm(undefined)}>
                            {t("common.cancel")}
                          </button>
                        </div>
                      </div>
                    ) : (
                      <div className="row group-actions">
                        {many && (
                          <button className="btn flat mini" onClick={() => setOpenGroup(g.key)}>
                            {t("installed.group.view", { n: g.entries.length })}
                          </button>
                        )}
                        <button
                          className="btn flat mini"
                          disabled={running}
                          title={running ? t("installed.closeToRemove") : undefined}
                          onClick={() => {
                            setError(undefined);
                            setConfirm(m.slug);
                          }}
                        >
                          {t("common.remove")}
                        </button>
                      </div>
                    )
                  }
                />
              );
            })}
          </ul>
        )}
      </section>

      <section aria-labelledby="h-manual">
        <h2 id="h-manual">{t("installed.manual", { n: manualGroups.length })}</h2>
        <p className="muted">{t("installed.manual.note")}</p>
        {manualGroups.length > 0 && (
          <ul className="mgrid">
            {manualGroups.map((g) =>
              g.entries.length > 1 ? (
                <ModCard
                  key={g.key}
                  kind={null}
                  gbId={null}
                  title={g.title}
                  subtitle={t("installed.group.manual")}
                  meta={groupMeta(g, "")}
                  flag={
                    <Badge tone="neutral" small>
                      {t("installed.group.items", { n: g.entries.length })}
                    </Badge>
                  }
                  actions={
                    <button className="btn flat mini" onClick={() => setOpenGroup(g.key)}>
                      {t("installed.group.view", { n: g.entries.length })}
                    </button>
                  }
                />
              ) : (
                <ModCard key={g.key} kind={null} gbId={null} title={g.entries[0]!.name} meta={metaOf(g.entries[0]!)} actions={<span className="muted small">{t("installed.manual.readonly")}</span>} />
              ),
            )}
          </ul>
        )}
      </section>

      {shownGroup && (
        <Modal onClose={() => setOpenGroup(undefined)} labelledBy="group-title">
          <button className="btn flat modal-close" onClick={() => setOpenGroup(undefined)}>
            {t("installed.group.close")}
          </button>
          <h2 id="group-title" className="modal-title">
            {shownGroup.title}
          </h2>
          <p className="muted">
            {shownGroup.managed ? (shownGroup.managed.submitter ? t("common.by", { who: shownGroup.managed.submitter }) : "") : t("installed.group.manual")}
            {" · "}
            {t("installed.group.items", { n: shownGroup.entries.length })} · {t("files", { n: shownGroup.entries.reduce((s, e) => s + e.file_count, 0) })} · {fmtBytes(shownGroup.entries.reduce((s, e) => s + e.size_bytes, 0))}
          </p>
          <ul className="group-list">
            {shownGroup.entries.map((e) => (
              <li key={e.name}>
                <div>
                  <b>{e.name}</b>
                  <div className="muted small">{`${t("files", { n: e.file_count })} · ${fmtBytes(e.size_bytes)}`}</div>
                </div>
                {tags(e.name)}
              </li>
            ))}
          </ul>
        </Modal>
      )}

      {contents.loading && !report && (
        <p className="muted small" role="status">
          {t("installed.contents.reading")}
        </p>
      )}

      {report && report.conflicts.length > 0 && (
        <section aria-labelledby="h-conflicts">
          <h2 id="h-conflicts">{t("installed.conflicts", { n: report.conflict_files })}</h2>
          <p className="note">
            <T k="installed.conflicts.note" />
          </p>
          <ul className="conflicts">
            {(showAllConflicts ? report.conflicts : report.conflicts.slice(0, 8)).map((c) => (
              <li key={c.items.join("|")}>
                <div>
                  <Badge tone="warn" small>
                    {t("files", { n: c.count })}
                  </Badge>{" "}
                  <b>{nice(c.winner)}</b> <span className="muted">{t("installed.conflicts.wins")}</span>
                </div>
                <div className="muted small">{t("installed.conflicts.against", { names: c.items.slice(1).map(nice).join(", ") })}</div>
                <div className="muted small">
                  <code>{c.examples.join("  ·  ")}</code>
                </div>
              </li>
            ))}
          </ul>
          {report.conflicts.length > 8 && (
            <button className="link" onClick={() => setShowAllConflicts((v) => !v)}>
              {showAllConflicts ? t("installed.conflicts.less") : t("installed.conflicts.more", { n: report.conflicts.length - 8 })}
            </button>
          )}
        </section>
      )}

      {report && report.unreadable.length > 0 && <p className="muted small">{t("installed.contents.unreadable", { names: report.unreadable.join(", ") })}</p>}

      {system.length > 0 && (
        <section aria-labelledby="h-system">
          <h2 id="h-system">{t("installed.system", { n: system.length })}</h2>
          <p className="muted">{t("installed.system.note")}</p>
          <table className="ledger">
            <caption className="sr-only">{t("installed.system.caption")}</caption>
            <tbody>
              {system.map((e) => (
                <tr key={e.name}>
                  <td>
                    <span className="tag">{roleLabel[e.role]}</span>
                  </td>
                  <td>
                    <b>{e.name}</b>
                    <div className="muted">{systemHelp[e.role]}</div>
                  </td>
                  <td className="num">{fmtBytes(e.size_bytes)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}
    </>
  );
}
