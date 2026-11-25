import { useEffect, useState } from "react";
import { call } from "../api";
import { fmtBytes } from "../format";
import { useI18n } from "../i18n";
import type { CasualJob, CasualStatus, Clash, ClashSide, Inspect } from "../types";
import { Badge, ErrorState, useAsync } from "../ui";
import { useJob } from "../useJob";
import { casualClashes, ClashList, sideActive } from "./Clashes";
import { JobBar } from "./JobBar";
import { Modal } from "./Modal";
import { ModCard } from "./ModCard";

const LAUNCH_OPTION = "+exec w/config.cfg";

interface Props {
  status: CasualStatus | undefined;
  statusError: string | undefined;
  loading: boolean;
  inspect: Inspect | undefined;
  /** QuickPrecache ligado nas Configurações. */
  quickprecache: boolean;
  onReload: () => void;
  /** Depois de aplicar ou reverter, o jogo mudou: a leitura do jogo também precisa ser refeita. */
  onGameChanged: () => void;
  onGoCatalog: () => void;
  onGoHome: () => void;
}

type Confirm = "setup" | "apply" | "revert" | undefined;

export function Casual({ status, statusError, loading, inspect, quickprecache, onReload, onGameChanged, onGoCatalog, onGoHome }: Props) {
  const { t, T, tc } = useI18n();
  const [confirm, setConfirm] = useState<Confirm>();
  const [removing, setRemoving] = useState<string | undefined>();
  const [confirmRemove, setConfirmRemove] = useState<string | undefined>();
  const [error, setError] = useState<string | undefined>();
  const [copied, setCopied] = useState(false);
  // O que está marcado para entrar na próxima aplicação (começa pelo que o núcleo guardou).
  const [checked, setChecked] = useState<Set<string>>(new Set());
  // Marcar este mod bate de frente com outros: o usuário escolhe quem fica.
  const [choice, setChoice] = useState<{ name: string; blockers: Clash[] } | undefined>();

  // Quem briga com quem (lista do casual e custom). Só leitura; refeito quando a lista ou o jogo mudam.
  const root = inspect?.installation.root;
  const listKey = status?.addons.map((a) => `${a.name}:${a.size_bytes}`).join("|") ?? "";
  const customKey = inspect?.custom.map((e) => `${e.name}:${e.size_bytes}`).join("|") ?? "";
  const found = useAsync<{ clashes: Clash[] } | null>(
    () => (status?.runtime === "ready" ? call<{ clashes: Clash[] }>("conflicts", { path: root ?? null }).catch(() => null) : Promise.resolve(null)),
    [status?.runtime, root, listKey, customKey],
  );

  const { job, track, dismiss } = useJob((j: CasualJob) => {
    onReload();
    if (j.kind === "apply" || j.kind === "revert") onGameChanged();
  });

  useEffect(() => {
    if (status) setChecked(new Set(status.addons.filter((a) => a.enabled).map((a) => a.name)));
  }, [status]);

  // Voltou para esta aba com uma tarefa ainda rodando: retoma o acompanhamento.
  useEffect(() => {
    if (status?.job?.state === "running" && !job) track(status.job);
  }, [status, job, track]);

  if (!status) {
    return statusError ? (
      <ErrorState title={t("casual.readFailed")} message={tc(statusError)} onRetry={onReload} />
    ) : (
      <div className="sheet-head">
        <h1>{loading ? t("casual.reading") : t("tab.casual")}</h1>
      </div>
    );
  }

  const busy = job?.state === "running";
  const gameRunning = inspect?.system.game_running ?? false;
  const ready = status.runtime === "ready";
  const addons = status.addons;
  const appliedNames = new Set(status.game_applied ? (status.applied_here?.addons ?? []) : []);
  const pending = ready && checked.size > 0 && (!status.game_applied || checked.size !== appliedNames.size || [...checked].some((n) => !appliedNames.has(n)));

  const start = (name: string, args: Record<string, unknown> = {}) => {
    setError(undefined);
    setConfirm(undefined);
    call<CasualJob>(name, args).then(track, (e: unknown) => setError(e instanceof Error ? e.message : String(e)));
  };

  const clashes = found.data?.clashes ?? [];
  const live = casualClashes(clashes, checked);
  const blocks = live.filter((c) => c.level === "block");
  const warns = live.filter((c) => c.level === "warn");

  const save = (next: Set<string>) => {
    setChecked(next);
    setError(undefined);
    call("casual_enable", { path: root ?? null, names: [...next] }).catch((e: unknown) => {
      setError(e instanceof Error ? e.message : String(e));
      onReload(); // o núcleo recusou: volta as marcas ao que está guardado
    });
  };

  const isSelf = (s: ClashSide, name: string) => s.place === "casual" && s.id === name;
  const rivalOf = (c: Clash, name: string) => (isSelf(c.a, name) ? c.b : c.a);

  const toggle = (name: string) => {
    const next = new Set(checked);
    if (next.has(name)) {
      next.delete(name);
      save(next);
      return;
    }
    const blockers = clashes.filter((c) => c.level === "block" && (isSelf(c.a, name) || isSelf(c.b, name)) && sideActive(rivalOf(c, name), checked));
    if (blockers.length > 0) {
      setChoice({ name, blockers });
      return;
    }
    next.add(name);
    save(next);
  };

  const shownName = (name: string) => addons.find((a) => a.name === name)?.source?.name ?? name;
  const rivals = choice ? choice.blockers.map((c) => rivalOf(c, choice.name)) : [];
  const fromCustom = rivals.filter((r) => r.place === "custom");
  const takeInstead = () => {
    if (!choice) return;
    const next = new Set(checked);
    rivals.forEach((r) => next.delete(r.id));
    next.add(choice.name);
    setChoice(undefined);
    save(next);
  };

  const remove = (name: string) => {
    setRemoving(name);
    setError(undefined);
    call("casual_remove", { name }).then(
      () => {
        setRemoving(undefined);
        onReload();
      },
      (e: unknown) => {
        setRemoving(undefined);
        setError(e instanceof Error ? e.message : String(e));
      },
    );
  };

  const stateOf = (name: string) => {
    if (appliedNames.has(name))
      return checked.has(name) ? (
        <Badge tone="ok" small>
          {t("casual.state.inGame")}
        </Badge>
      ) : (
        <Badge tone="warn" small>
          {t("casual.state.leaves")}
        </Badge>
      );
    return checked.has(name) ? (
      <Badge tone="info" small>
        {t("casual.state.enters")}
      </Badge>
    ) : (
      <Badge tone="neutral" small>
        {t("casual.state.outside")}
      </Badge>
    );
  };

  return (
    <>
      <div className="sheet-head">
        <div>
          <h1>{t("tab.casual")}</h1>
          <p>
            <T k="casual.subtitle" />
          </p>
        </div>
        {status.game_applied === true ? (
          <Badge tone="ok">{t("casual.badge.applied")}</Badge>
        ) : status.game_applied === false ? (
          <Badge tone="neutral">{t("casual.badge.clean")}</Badge>
        ) : (
          <Badge tone="neutral">{t("casual.badge.noGame")}</Badge>
        )}
      </div>

      {error && (
        <div className="state error" role="alert">
          <h2>{t("casual.error.title")}</h2>
          <p>{tc(error)}</p>
        </div>
      )}

      {job && <JobBar job={job} onDismiss={dismiss} />}

      {!ready && (
        <section className="state" aria-labelledby="h-setup">
          <h2 id="h-setup">{t("casual.setup.title")}</h2>
          <p>{status.runtime === "needs_deps" ? t("casual.setup.needsDeps") : t("casual.setup.missing")}</p>
          {confirm === "setup" ? (
            <div className="confirm">
              <p>
                <T k="casual.setup.confirm" v={{ version: status.pinned_version }} />
              </p>
              <p className="muted">
                <T k="casual.setup.where" v={{ dir: status.root }} />
              </p>
              <div className="row">
                <button className="btn primary" disabled={busy} onClick={() => start("casual_setup")}>
                  {t("casual.setup.go")}
                </button>
                <button className="btn flat" onClick={() => setConfirm(undefined)}>
                  {t("common.cancel")}
                </button>
              </div>
            </div>
          ) : (
            <button className="btn primary" disabled={busy} onClick={() => setConfirm("setup")}>
              {status.runtime === "needs_deps" ? t("casual.setup.installLibs") : t("casual.setup.button")}
            </button>
          )}
        </section>
      )}

      {ready && (
        <>
          <section aria-labelledby="h-list">
            <h2 id="h-list">{t("casual.list.title", { n: addons.length })}</h2>
            {addons.length === 0 ? (
              <p>
                <T k="casual.list.empty" />{" "}
                <button className="link" onClick={onGoCatalog}>
                  {t("casual.list.goCatalog")}
                </button>
              </p>
            ) : (
              <ul className="mgrid">
                {addons.map((a) => (
                  <ModCard
                    key={a.name}
                    kind={a.source?.kind ?? null}
                    gbId={a.source?.gb_id ?? null}
                    title={a.source?.name ?? a.name}
                    subtitle={a.source?.submitter ? t("common.by", { who: a.source.submitter }) : a.source ? (a.source.kind === "Local" ? t("installed.local") : undefined) : t("casual.list.external")}
                    meta={
                      <>
                        {`${t("files", { n: a.file_count })} · ${fmtBytes(a.size_bytes)}`}
                        {(a.has_models || a.also_in_custom) && (
                          <span className="card-tags">
                            {a.has_models && <span className="tag warn">{t("casual.tag.models")}</span>}
                            {a.also_in_custom && <span className="tag warn">{t("casual.tag.alsoCustom")}</span>}
                          </span>
                        )}
                      </>
                    }
                    flag={stateOf(a.name)}
                    actions={
                      confirmRemove === a.name ? (
                        <div className="confirm">
                          <p>
                            <T k="casual.list.removeConfirm" v={{ name: a.source?.name ?? a.name }} />
                          </p>
                          <div className="row">
                            <button
                              className="btn primary mini"
                              disabled={removing === a.name}
                              onClick={() => {
                                remove(a.name);
                                setConfirmRemove(undefined);
                              }}
                            >
                              {t("casual.list.removeButton")}
                            </button>
                            <button className="btn flat mini" onClick={() => setConfirmRemove(undefined)}>
                              {t("common.cancel")}
                            </button>
                          </div>
                        </div>
                      ) : (
                        <div className="mcard-row">
                          <label className="use">
                            <input type="checkbox" className="pick" checked={checked.has(a.name)} disabled={busy} onChange={() => toggle(a.name)} />
                            {t("casual.list.use")}
                          </label>
                          <button className="btn flat mini" disabled={busy} onClick={() => setConfirmRemove(a.name)}>
                            {t("casual.list.take")}
                          </button>
                        </div>
                      )
                    }
                  />
                ))}
              </ul>
            )}
            {blocks.length > 0 && (
              <div className="clashes bad" role="alert">
                <h3>{t("clash.blocked.title")}</h3>
                <p className="muted">{t("clash.blocked.body")}</p>
                <ClashList clashes={blocks} />
              </div>
            )}
            {warns.length > 0 && (
              <div className="clashes">
                <h3>{t("clash.warn.title")}</h3>
                <p className="muted">{t("clash.warn.body")}</p>
                <ClashList clashes={warns} />
              </div>
            )}
            {addons.some((a) => a.has_models) && (
              <p className="note">
                <T k="casual.models.note" />
              </p>
            )}
            {addons.some((a) => a.also_in_custom) && (
              <p className="note">
                <T k="casual.conflict.note" v={{ names: addons.filter((a) => a.also_in_custom).map((a) => a.source?.name ?? a.name).join(", ") }} />
              </p>
            )}
            <p className="muted small">{t("casual.list.particlesNote")}</p>
          </section>

          <div className="cols2">
          <section className="state" aria-labelledby="h-apply">
            <h2 id="h-apply">{t("casual.apply.title")}</h2>
            {!root ? (
              <p>
                {t("casual.apply.needInstall")}{" "}
                <button className="link" onClick={onGoHome}>
                  {t("common.goHome")}
                </button>
              </p>
            ) : (
              <>
                <p>
                  {checked.size === 0
                    ? t("casual.apply.pickOne")
                    : pending
                      ? t("casual.apply.pending", { n: checked.size })
                      : t("casual.apply.inSync", { n: checked.size })}
                </p>
                {gameRunning && (
                  <p>
                    <Badge tone="warn" small>
                      {t("status.open")}
                    </Badge>{" "}
                    {t("casual.apply.closeGame")}
                  </p>
                )}
                {blocks.length > 0 && (
                  <p>
                    <Badge tone="block" small>
                      {t("clash.badge")}
                    </Badge>{" "}
                    {t("clash.apply")}
                  </p>
                )}
                {confirm === "apply" ? (
                  <div className="confirm">
                    <p>
                      <T k="casual.apply.confirm" />
                    </p>
                    <div className="row">
                      <button
                        className="btn primary"
                        disabled={busy || gameRunning || blocks.length > 0}
                        onClick={() => start("casual_apply", { path: root, addons: [...checked], options: { quickprecache } })}
                      >
                        {t("casual.apply.confirmButton")}
                      </button>
                      <button className="btn flat" onClick={() => setConfirm(undefined)}>
                        {t("common.cancel")}
                      </button>
                    </div>
                  </div>
                ) : confirm === "revert" ? (
                  <div className="confirm">
                    <p>{t("casual.revert.confirm")}</p>
                    <div className="row">
                      <button className="btn primary" disabled={busy || gameRunning} onClick={() => start("casual_revert", { path: root })}>
                        {t("casual.revert.confirmButton")}
                      </button>
                      <button className="btn flat" onClick={() => setConfirm(undefined)}>
                        {t("common.cancel")}
                      </button>
                    </div>
                  </div>
                ) : (
                  <div className="row">
                    <button className="btn primary" disabled={busy || gameRunning || checked.size === 0 || !pending || blocks.length > 0} onClick={() => setConfirm("apply")}>
                      {t("casual.apply.button")}
                    </button>
                    <button className="btn flat" disabled={busy || gameRunning || status.game_applied !== true} onClick={() => setConfirm("revert")}>
                      {t("casual.revert.button")}
                    </button>
                  </div>
                )}
                {status.game_applied === true && !status.applied_here && <p className="muted small">{t("casual.apply.external")}</p>}
              </>
            )}
          </section>

          <section className="state" aria-labelledby="h-launch">
            <h2 id="h-launch">{t("casual.launch.title")}</h2>
            <ol className="steps">
              <li>
                <T k="casual.launch.option" v={{ opt: LAUNCH_OPTION }} />{" "}
                {status.launch_options_ok === true && (
                  <Badge tone="ok" small>
                    {t("casual.launch.there")}
                  </Badge>
                )}
                {status.launch_options_ok === false && (
                  <Badge tone="warn" small>
                    {t("casual.launch.missing")}
                  </Badge>
                )}
                {status.launch_options_ok === null && (
                  <Badge tone="neutral" small>
                    {t("casual.launch.unknown")}
                  </Badge>
                )}
                {status.launch_options && (
                  <span className="muted">
                    {" "}
                    {t("casual.launch.today")} <code>{status.launch_options}</code>
                  </span>
                )}
                <p className="muted small">
                  <T k="casual.launch.playHint" />
                </p>
                <div>
                  <button className="link" onClick={() => navigator.clipboard?.writeText(LAUNCH_OPTION).then(() => setCopied(true), () => setCopied(false))}>
                    {copied ? t("common.copied") : t("casual.launch.copy", { opt: LAUNCH_OPTION })}
                  </button>
                </div>
              </li>
              <li>
                <T k="casual.launch.firstBoot" />
              </li>
              <li>
                <T k="casual.launch.verify" />
              </li>
              <li>
                <T k="casual.launch.update" />
              </li>
            </ol>
          </section>
          </div>
        </>
      )}

      {choice && (
        <Modal onClose={() => setChoice(undefined)} labelledBy="clash-title">
          <h2 id="clash-title" className="modal-title">
            {t("clash.choice.title")}
          </h2>
          <p className="clash-choice">{t("clash.choice.body", { name: shownName(choice.name) })}</p>
          <ClashList clashes={choice.blockers} />
          {fromCustom.length > 0 ? (
            <>
              <p>{t("clash.choice.custom", { names: fromCustom.map((r) => r.name).join(", ") })}</p>
              <div className="row">
                <button className="btn primary" onClick={() => setChoice(undefined)}>
                  {t("common.close")}
                </button>
              </div>
            </>
          ) : (
            <div className="row">
              <button className="btn primary" disabled={busy} onClick={takeInstead}>
                {t("clash.choice.take", { name: shownName(choice.name) })}
              </button>
              <button className="btn" onClick={() => setChoice(undefined)}>
                {t("clash.choice.keep", { names: rivals.map((r) => r.name).join(", ") })}
              </button>
            </div>
          )}
        </Modal>
      )}

      <p className="muted small">
        {t("casual.credits", { version: status.pinned_version })}
        {status.version && status.version !== status.pinned_version ? ` ${t("casual.credits.other", { version: status.version })}` : ""}
      </p>
    </>
  );
}
