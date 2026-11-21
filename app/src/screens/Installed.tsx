import { useState } from "react";
import { call } from "../api";
import type { CustomEntry, Inspect, Role } from "../types";
import { fmtBytes } from "../format";
import { useI18n } from "../i18n";
import { Badge } from "../ui";
import { ModCard } from "./ModCard";

interface Props {
  inspect: Inspect | undefined;
  loading: boolean;
  onGoHome: () => void;
  onGoCatalog: () => void;
  /** Chamado depois de remover, para a lista se atualizar. */
  onChanged: () => void;
}

export function Installed({ inspect, loading, onGoHome, onGoCatalog, onChanged }: Props) {
  const { t, T, tc } = useI18n();
  const [confirm, setConfirm] = useState<string | undefined>();
  const [busy, setBusy] = useState<string | undefined>();
  const [error, setError] = useState<string | undefined>();

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

      {error && (
        <div className="state error" role="alert">
          <h2>{t("installed.remove.failed")}</h2>
          <p>{tc(error)}</p>
        </div>
      )}

      <section aria-labelledby="h-mine">
        <h2 id="h-mine">{t("installed.mine", { n: mine.length })}</h2>
        {running && (
          <p>
            <Badge tone="warn" small>
              {t("status.open")}
            </Badge>{" "}
            {t("installed.closeToRemove")}
          </p>
        )}
        {mine.length === 0 ? (
          <p>
            {t("installed.none")}{" "}
            <button className="link" onClick={onGoCatalog}>
              {t("installed.browse")}
            </button>
          </p>
        ) : (
          <ul className="mgrid">
            {mine.map((e) => {
              const m = e.managed!;
              return (
                <ModCard
                  key={e.name}
                  kind={m.kind}
                  gbId={m.gb_id}
                  title={m.name}
                  subtitle={m.submitter ? t("common.by", { who: m.submitter }) : undefined}
                  meta={`${t("files", { n: e.file_count })} · ${fmtBytes(e.size_bytes)}${m.version ? ` · v${m.version}` : ""}`}
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
                    )
                  }
                />
              );
            })}
          </ul>
        )}
      </section>

      <section aria-labelledby="h-manual">
        <h2 id="h-manual">{t("installed.manual", { n: manual.length })}</h2>
        <p className="muted">{t("installed.manual.note")}</p>
        {manual.length > 0 && (
          <ul className="mgrid">
            {manual.map((e) => (
              <ModCard
                key={e.name}
                kind={null}
                gbId={null}
                title={e.name}
                meta={`${t("files", { n: e.file_count })} · ${fmtBytes(e.size_bytes)}`}
                actions={<span className="muted small">{t("installed.manual.readonly")}</span>}
              />
            ))}
          </ul>
        )}
      </section>

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
