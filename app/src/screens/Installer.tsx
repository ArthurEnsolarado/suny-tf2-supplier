import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { call } from "../api";
import { fmtBytes } from "../format";
import { useI18n } from "../i18n";
import type { CasualJob, CasualStatus, Clash, FileInfo, GameIcons, InstallOutcome, Kind, PreparedFile } from "../types";
import { Icon } from "../ui";
import { AnalysisView } from "./AnalysisView";
import { AUTO_ANALYZE_MAX, SUPPORTED } from "./Analyzer";
import { JobBar } from "./JobBar";
import { RestartButton } from "./RestartButton";
import { SoundPicker } from "./SoundPicker";
import { VersionPicker } from "./VersionPicker";

export interface CasualLink {
  ready: boolean;
  unknown: boolean;
  /** QuickPrecache ligado nas Configurações (vale ao aplicar). */
  quickprecache: boolean;
  onGo: () => void;
  onGoInstalled: () => void;
  onAdded: () => void;
}

interface Props {
  installRoot: string | undefined;
  kind: Kind;
  modId: number;
  modName: string;
  files: FileInfo[];
  gameRunning: boolean;
  casual: CasualLink;
  /** Mapa (.bsp): o Casual Preloader não carrega mapas, então só faz sentido em custom. */
  isMap: boolean;
  icons: GameIcons["icons"];
  /** Arquivo cujos sons já aparecem em outro lugar da ficha. */
  soundsShownFor?: number;
  onChanged: () => void;
}

const hasExe = (f: FileInfo) => !!f.analysis_warnings && "contains_exe" in f.analysis_warnings;
const extOf = (f: FileInfo) => f.file_name.toLowerCase().split(".").pop() ?? "";
const usable = (f: FileInfo) => !hasExe(f) && SUPPORTED.includes(extOf(f));
const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));

type Dest = "casual" | "custom";
type Prep = { t: "idle" } | { t: "busy" } | { t: "ok"; d: PreparedFile } | { t: "error"; msg: string };
type StepId = "add" | "enable" | "apply" | "install";
type Run =
  | { t: "idle" }
  | { t: "running"; steps: StepId[]; at: number; job?: CasualJob }
  | { t: "done"; dest: Dest; applied: boolean; out?: InstallOutcome }
  | { t: "versions"; apply: boolean; drop: string[] }
  | { t: "failed"; msg: string; job?: CasualJob };

type Level = "ok" | "warn" | "block" | "info" | "wait";

/** Uma linha da conferência: o estado em ícone e cor, o que foi visto e, se houver, o que fazer. */
function Check({ level, title, children }: { level: Level; title: ReactNode; children?: ReactNode }) {
  const { t } = useI18n();
  const icon = level === "ok" ? "check" : level === "warn" ? "warn" : level === "block" ? "x" : level === "wait" ? "spinner" : "info";
  return (
    <li className={`precheck ${level}`}>
      <span className="precheck-mark" aria-hidden="true">
        <Icon name={icon} />
      </span>
      <div>
        <span className="sr-only">{t(`flow.level.${level}`)}: </span>
        <b>{title}</b>
        {children && <div className="precheck-more">{children}</div>}
      </div>
    </li>
  );
}

/** Espera uma tarefa do casual terminar, contando o andamento. */
async function waitJob(job: CasualJob, onTick: (j: CasualJob) => void): Promise<CasualJob> {
  let current = job;
  onTick(current);
  while (current.state === "running") {
    await new Promise((r) => setTimeout(r, 700));
    const next = await call<CasualJob | null>("casual_job").catch(() => current);
    if (!next || next.id !== job.id) break;
    current = next;
    onTick(current);
  }
  return current;
}

/**
 * A instalação num só lugar. Ao abrir, o app confere o arquivo sozinho e diz em linguagem simples se dá para instalar.
 * Um botão faz o caminho todo (no casual: baixar, pôr na lista e aplicar no jogo), mostrando cada passo. Nada que mexa
 * no jogo acontece sem esse clique, e o botão diz o que vai fazer.
 */
export function Installer({ installRoot, kind, modId, modName, files, gameRunning, casual, isMap, icons, soundsShownFor, onChanged }: Props) {
  const { t, T, tc } = useI18n();
  const [fileId, setFileId] = useState(() => (files.find(usable) ?? files[0])?.id);
  const file = files.find((f) => f.id === fileId) ?? files[0];
  const [prep, setPrep] = useState<Prep>({ t: "idle" });
  const [picked, setPicked] = useState<Dest | undefined>();
  const [applyNow, setApplyNow] = useState(true);
  const [run, setRun] = useState<Run>({ t: "idle" });
  const [picking, setPicking] = useState<string | undefined>();
  const [again, setAgain] = useState(false);
  const [launch, setLaunch] = useState<"idle" | "busy" | "sent">("idle");
  const [launchError, setLaunchError] = useState<string | undefined>();
  const asked = useRef(0);

  const check = useCallback(
    (quiet = false) => {
      if (!file || !usable(file)) return;
      const turn = ++asked.current;
      if (!quiet) setPrep({ t: "busy" });
      call<PreparedFile>("prepare_file", { path: installRoot ?? null, kind, id: modId, file_id: file.id }).then(
        (d) => turn === asked.current && setPrep({ t: "ok", d }),
        (e: unknown) => turn === asked.current && setPrep({ t: "error", msg: errorText(e) }),
      );
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [file?.id, installRoot, kind, modId],
  );

  // arquivo pequeno é conferido sozinho; o grande espera um clique, para não baixar centenas de MB sem avisar
  useEffect(() => {
    asked.current++;
    setPrep({ t: "idle" });
    setRun({ t: "idle" });
    setPicked(undefined);
    setAgain(false);
    if (file && usable(file) && file.size <= AUTO_ANALYZE_MAX) check();
  }, [check]); // eslint-disable-line react-hooks/exhaustive-deps

  if (!file) return <p className="muted">{t("ficha.noFiles")}</p>;

  const d = prep.t === "ok" ? prep.d : undefined;
  const compat = d?.analysis.report.compat;
  const nothing = !!d && d.analysis.game_files === 0 && d.analysis.vpk_files === 0;
  const map = isMap || !!d?.analysis.report.flags.includes("map_content");
  const recommended: Dest = map || compat === "offline_only" || (!casual.ready && !casual.unknown) ? "custom" : "casual";
  const dest: Dest = map ? "custom" : (picked ?? recommended);

  const blockers = (d?.clashes ?? []).filter((c) => c.level === "block");
  const warnings = (d?.clashes ?? []).filter((c) => c.level === "warn");
  const customRivals = blockers.filter((c) => c.b.place === "custom");
  const casualRivals = blockers.filter((c) => c.b.place === "casual");
  // no casual dá para trocar um mod marcado por este; o que está em custom precisa ser removido antes
  const swap = dest === "casual" && casualRivals.length > 0 && customRivals.length === 0;
  const conflictStops = customRivals.length > 0 || (dest === "custom" && casualRivals.length > 0);

  const needsSetup = dest === "casual" && !casual.ready && !casual.unknown;
  const needsGame = dest === "custom" && !installRoot;
  const canApply = dest === "casual" && !!installRoot && !gameRunning;
  const willApply = canApply && applyNow;
  const stopped = !usable(file) || compat === "blocked" || nothing || conflictStops || needsSetup || needsGame || (dest === "casual" && casual.unknown);
  const ready = !!d && !stopped;
  const warned = warnings.length > 0 || swap || compat === "partial" || (dest === "casual" && compat === "offline_only") || file.av_result !== "clean";

  const mine = d?.in_casual ?? [];
  const sameFile = mine.filter((a) => a.file_id === file.id);
  // "no jogo" é: o que está marcado está aplicado (versões desmarcadas não contam)
  const inGame = sameFile.some((a) => a.enabled) && sameFile.filter((a) => a.enabled).every((a) => a.in_game);
  const running = run.t === "running";
  // quem já tem o mod vê o que fazer com ele; a conferência e o botão de instalar voltam só se pedir
  const have = sameFile.length > 0 || !!d?.in_custom;

  // ---------------------------------------------------------------- o caminho todo

  const finish = (next: Run) => {
    setRun(next);
    setAgain(false);
    casual.onAdded();
    onChanged();
    check(true); // relê o estado: agora o mod está lá
  };

  const enableAndApply = async (addons: string[], drop: string[], apply: boolean, steps: StepId[]) => {
    setRun({ t: "running", steps, at: steps.indexOf("enable") });
    const status = await call<CasualStatus>("casual_status", { path: installRoot ?? null });
    const enabled = status.addons.filter((a) => a.enabled).map((a) => a.name);
    const next = [...new Set([...enabled.filter((n) => !drop.includes(n)), ...addons])];
    if (next.length !== enabled.length || next.some((n) => !enabled.includes(n))) {
      await call("casual_enable", { path: installRoot ?? null, names: next });
    }
    if (!apply) return finish({ t: "done", dest: "casual", applied: false });
    const at = steps.indexOf("apply");
    const first = await call<CasualJob>("casual_apply", { path: installRoot, addons: next, options: { quickprecache: casual.quickprecache } });
    const job = await waitJob(first, (j) => setRun({ t: "running", steps, at, job: j }));
    finish(job.state === "done" ? { t: "done", dest: "casual", applied: true } : { t: "failed", msg: job.error ?? "", job });
  };

  const install = async () => {
    const apply = willApply;
    const drop = swap ? casualRivals.map((c) => c.b.id) : [];
    try {
      if (dest === "custom") {
        setRun({ t: "running", steps: ["install"], at: 0 });
        const out = await call<InstallOutcome>("install_mod", { path: installRoot, kind, id: modId, file_id: file.id });
        return finish({ t: "done", dest, applied: false, out });
      }
      const steps: StepId[] = apply ? ["add", "enable", "apply"] : ["add", "enable"];
      setRun({ t: "running", steps, at: 0 });
      const first = await call<CasualJob>("casual_add", { path: installRoot ?? null, kind, id: modId, file_id: file.id });
      const job = await waitJob(first, (j) => setRun({ t: "running", steps, at: 0, job: j }));
      if (job.state !== "done") return finish({ t: "failed", msg: job.error ?? "", job });
      const r = job.result as { added?: string[]; variants?: boolean } | null;
      if (r?.variants) return finish({ t: "versions", apply, drop }); // várias versões do mesmo mod: só a pessoa sabe qual quer
      let added = r?.added ?? [];
      if (added.length === 0) {
        const again = await call<PreparedFile>("prepare_file", { path: installRoot ?? null, kind, id: modId, file_id: file.id });
        added = again.in_casual.filter((a) => a.file_id === file.id).map((a) => a.name);
      }
      await enableAndApply(added, drop, apply, steps);
    } catch (e) {
      finish({ t: "failed", msg: errorText(e) });
    }
  };

  // fica na lista só a versão escolhida (as outras trocam os mesmos arquivos); depois segue o caminho de sempre
  const useVersions = async (chosen: string[], apply: boolean, drop: string[]) => {
    const steps: StepId[] = apply ? ["enable", "apply"] : ["enable"];
    setRun({ t: "running", steps, at: 0 });
    try {
      for (const other of sameFile.filter((a) => !chosen.includes(a.name))) {
        await call("casual_remove", { name: other.name });
      }
      await enableAndApply(chosen, drop, apply, steps);
    } catch (e) {
      finish({ t: "failed", msg: errorText(e) });
    }
  };

  // várias versões na lista e nenhuma marcada: falta escolher; com alguma marcada, valem as marcadas
  const undecided = sameFile.length > 1 && !sameFile.some((a) => a.enabled);

  const applyExisting = () => {
    const marked = sameFile.filter((a) => a.enabled);
    enableAndApply(
      (marked.length > 0 ? marked : sameFile).map((a) => a.name),
      casualRivals.map((c) => c.b.id),
      true,
      ["enable", "apply"],
    ).catch((e: unknown) => finish({ t: "failed", msg: errorText(e) }));
  };

  const play = () => {
    setLaunch("busy");
    setLaunchError(undefined);
    call("launch_game", { path: installRoot ?? null }).then(
      () => setLaunch("sent"),
      (e: unknown) => {
        setLaunch("idle");
        setLaunchError(errorText(e));
      },
    );
  };

  const playButton = !gameRunning && installRoot && (
    <button className="btn primary" disabled={launch !== "idle"} onClick={play}>
      <Icon name={launch === "busy" ? "spinner" : launch === "sent" ? "check" : "play"} filled={launch === "idle"} />
      {launch === "idle" ? t("play.button") : t("play.launching")}
    </button>
  );
  const withSounds = mine.find((a) => a.sound_groups > 0);
  const soundsButton = withSounds && (
    <button className="btn" onClick={() => setPicking(withSounds.name)}>
      {t("picker.open")}
    </button>
  );

  // ---------------------------------------------------------------- o que a tela mostra

  const names = (list: Clash[]) => [...new Set(list.map((c) => c.b.name))].join(", ");
  const what = (list: Clash[]) =>
    [...new Set(list.flatMap((c) => c.objects.map((o) => t(`clash.object.${o.kind}` as "clash.object.hud", { name: o.name }))))].join(", ");

  const checks = (
    <ul className="prechecks">
      <Check level={hasExe(file) ? "block" : file.av_result === "clean" ? "ok" : "warn"} title={hasExe(file) ? t("ficha.hasExe") : file.av_result === "clean" ? t("flow.av.clean") : t("flow.av.unknown", { x: file.av_result ?? t("ficha.noResult") })} />
      {d && (
        <>
          <Check
            level={compat === "blocked" || nothing ? "block" : compat === "casual_ok" ? "ok" : compat === "needs_index" ? "info" : "warn"}
            title={nothing ? t("flow.compat.nothing") : t(`flow.compat.${compat as "casual_ok"}`)}
          />
          {blockers.length > 0 ? (
            <Check level={swap ? "warn" : "block"} title={t("flow.clash.same", { names: names(blockers) })}>
              {swap ? (
                <p>{t("flow.clash.swap", { names: names(casualRivals) })}</p>
              ) : customRivals.length > 0 ? (
                <p>
                  {t("flow.clash.custom", { names: names(customRivals) })}{" "}
                  <button className="link" onClick={casual.onGoInstalled}>
                    {t("flow.goInstalled")}
                  </button>
                </p>
              ) : (
                <p>
                  {t("flow.clash.casual", { names: names(casualRivals) })}{" "}
                  <button className="link" onClick={casual.onGo}>
                    {t("add.openCasual")}
                  </button>
                </p>
              )}
              <code>{blockers[0]?.examples[0]}</code>
            </Check>
          ) : warnings.length > 0 ? (
            <Check level="warn" title={t("flow.clash.warn", { names: names(warnings) })}>
              <p>{t("clash.same", { what: what(warnings) })}</p>
            </Check>
          ) : (
            <Check level="ok" title={t("flow.clash.none")} />
          )}
        </>
      )}
      {needsSetup && (
        <Check level="block" title={t("flow.setup")}>
          <button className="btn mini" onClick={casual.onGo}>
            {t("flow.setup.button")}
          </button>
        </Check>
      )}
      {needsGame && <Check level="block" title={t("install.needInstall")} />}
      {gameRunning && <Check level="info" title={dest === "casual" ? t("flow.open.casual") : t("flow.open.custom")} />}
    </ul>
  );

  const verdict: { level: Level; text: string } | undefined = !d
    ? undefined
    : stopped
      ? { level: "block", text: t("flow.verdict.stop") }
      : warned
        ? { level: "warn", text: t("flow.verdict.warn") }
        : { level: "ok", text: t("flow.verdict.ok") };

  const action = dest === "custom" ? t("flow.do.custom") : swap ? (willApply ? t("flow.do.swapApply") : t("flow.do.swap")) : willApply ? t("flow.do.apply") : t("flow.do.casual");

  const stepName: Record<StepId, string> = { add: t("flow.step.add"), enable: t("flow.step.enable"), apply: t("flow.step.apply"), install: t("flow.step.install") };

  return (
    <div className="installer">
      {files.length > 1 && (
        <div className="file-choice" role="radiogroup" aria-label={t("flow.pickFile")}>
          <p className="dest-label">{t("flow.pickFile")}</p>
          {files.map((f) => (
            <button key={f.id} type="button" role="radio" aria-checked={f.id === file.id} className="file-opt" disabled={running} onClick={() => setFileId(f.id)}>
              <span>{f.file_name}</span>
              <span className="muted">{usable(f) ? fmtBytes(f.size) : hasExe(f) ? t("ficha.hasExe") : `.${extOf(f)}`}</span>
            </button>
          ))}
        </div>
      )}

      {undecided && (run.t === "idle" || run.t === "versions") && (
        <VersionPicker
          versions={sameFile}
          canApply={!!installRoot && !gameRunning && customRivals.length === 0}
          applyWanted={run.t === "versions" ? run.apply : applyNow}
          onUse={(chosen, apply) => void useVersions(chosen, apply, run.t === "versions" ? run.drop : casualRivals.map((c) => c.b.id))}
        />
      )}

      {sameFile.length > 0 && !undecided && run.t === "idle" && (
        <div className={`state-card ${inGame ? "good" : ""}`} role="status">
          <b>{inGame ? t("flow.have.inGame") : sameFile.some((a) => a.enabled) ? t("flow.have.pending") : t("flow.have.off")}</b>
          {sameFile.some((a) => a.variant) && (
            <p className="muted small">{t("casual.version", { name: sameFile.filter((a) => a.enabled).map((a) => a.variant ?? a.name).join(", ") || "—" })}</p>
          )}
          <div className="row">
            {inGame ? (
              playButton
            ) : (
              <button className="btn primary" disabled={!installRoot || gameRunning || customRivals.length > 0} onClick={applyExisting}>
                {casualRivals.length > 0 ? t("flow.do.swapApply") : t("flow.applyNow")}
              </button>
            )}
            {soundsButton}
            <button className="btn flat" onClick={casual.onGo}>
              {t("flow.seeCasual")}
            </button>
          </div>
          {gameRunning && !inGame && <p className="muted small">{t("casual.apply.closeGame")}</p>}
        </div>
      )}
      {d?.in_custom && run.t === "idle" && (
        <div className="state-card good" role="status">
          <b>{t("flow.have.custom")}</b>
          <div className="row">
            {playButton}
            <button className="btn flat" onClick={casual.onGoInstalled}>
              {t("flow.goInstalled")}
            </button>
          </div>
        </div>
      )}

      {!usable(file) ? (
        <p className="muted">{hasExe(file) ? t("install.blockedExe") : <T k="install.badFormat" v={{ ext: extOf(file) || "?" }} />}</p>
      ) : prep.t === "idle" ? (
        <div>
          <button className="btn primary big" onClick={() => check()}>
            {t("flow.check", { size: fmtBytes(file.size) })}
          </button>
          <p className="muted small">{t("flow.check.hint")}</p>
        </div>
      ) : prep.t === "busy" ? (
        <ul className="prechecks" role="status" aria-live="polite">
          <Check level="wait" title={t("flow.checking")}>
            <p className="muted">{t("flow.checking.hint", { size: fmtBytes(file.size) })}</p>
          </Check>
        </ul>
      ) : prep.t === "error" ? (
        <div className="confirm bad" role="alert">
          <p>
            <T k="analyzer.failed" v={{ msg: tc(prep.msg) }} />
          </p>
          <button className="btn" onClick={() => check()}>
            {t("common.retry")}
          </button>
        </div>
      ) : (
        <>
          {run.t === "idle" && have && !again && (
            <button className="link" onClick={() => setAgain(true)}>
              {t("flow.again")}
            </button>
          )}
          {run.t === "idle" && (!have || again) && (
            <>
              {verdict && (
                <p className={`verdict ${verdict.level}`} role="status">
                  {verdict.text}
                </p>
              )}
              {checks}

              {!map && compat !== "blocked" && !nothing && (
                <div className="dest">
                  <p className="dest-label">{t("install.dest.label")}</p>
                  <div className="dest-options" role="radiogroup" aria-label={t("install.dest.label")}>
                    {(["casual", "custom"] as const).map((o) => (
                      <button key={o} type="button" role="radio" aria-checked={dest === o} className="dest-opt" onClick={() => setPicked(o)}>
                        <span className="dest-title">{t(`install.dest.${o}`)}</span>
                        {recommended === o && <span className="dest-rec">{t("flow.recommended")}</span>}
                        <span className="dest-hint">{t(`install.dest.${o}.hint`)}</span>
                      </button>
                    ))}
                  </div>
                </div>
              )}
              {map && (
                <p className="note">
                  <T k="install.map.note" />
                </p>
              )}

              {canApply && (
                <label className="use apply-now">
                  <input type="checkbox" className="pick" checked={applyNow} onChange={(e) => setApplyNow(e.target.checked)} />
                  {t("flow.applyToo")}
                </label>
              )}
              <button className="btn primary big" disabled={!ready} onClick={() => void install()}>
                {have ? t("flow.again") : action}
              </button>
              {ready && <p className="muted small">{dest === "custom" ? t("import.hint.custom") : willApply ? t("flow.what.apply") : t("flow.what.casual")}</p>}
            </>
          )}
        </>
      )}

      {run.t === "running" && (
        <div className="flow" role="status" aria-live="polite">
          <ol className="flow-steps">
            {run.steps.map((s, i) => (
              <li key={s} className={i < run.at ? "done" : i === run.at ? "now" : ""}>
                <span className="precheck-mark" aria-hidden="true">
                  {i < run.at ? <Icon name="check" /> : i === run.at ? <Icon name="spinner" /> : i + 1}
                </span>
                {stepName[s]}
              </li>
            ))}
          </ol>
          {run.job && <JobBar job={run.job} />}
          <p className="muted small">{t("install.dontClose")}</p>
        </div>
      )}

      {run.t === "done" && (
        <div className="state-card good big" role="status">
          <b>
            <Icon name="check" /> {run.dest === "custom" ? t("flow.done.custom") : run.applied ? t("flow.done.applied") : t("flow.done.list")}
          </b>
          <p>
            {run.dest === "custom"
              ? gameRunning
                ? t("install.openNote")
                : t("flow.done.custom.next")
              : run.applied
                ? t("flow.done.applied.next")
                : t("flow.done.list.next")}
          </p>
          {run.out?.report.flags.includes("competitive_particles") && <p>{t("install.rgl")}</p>}
          <div className="row">
            {(run.dest === "custom" || run.applied) && playButton}
            {run.dest === "custom" && gameRunning && <RestartButton path={installRoot} />}
            {soundsButton}
            {run.dest === "casual" && (
              <button className="btn flat" onClick={casual.onGo}>
                {t("flow.seeCasual")}
              </button>
            )}
          </div>
        </div>
      )}

      {run.t === "failed" && (
        <div className="confirm bad" role="alert">
          {run.job ? <JobBar job={run.job} /> : <p>{tc(run.msg)}</p>}
          {sameFile.length > 0 && <p>{t("flow.failed.inList")}</p>}
          <button className="btn" onClick={() => setRun({ t: "idle" })}>
            {t("common.retry")}
          </button>
        </div>
      )}

      {launchError && (
        <p className="confirm bad" role="alert">
          {tc(launchError)}
        </p>
      )}

      <details className="file-details">
        <summary>{t("flow.details")}</summary>
        <p>
          <b>{file.file_name}</b> · {fmtBytes(file.size)}
        </p>
        <p>
          {t("ficha.antivirus")} <b>{file.av_result ?? t("ficha.noResult")}</b>
        </p>
        {file.md5 && (
          <p>
            <code>MD5 {file.md5}</code>
          </p>
        )}
        {d && (
          <AnalysisView
            key={file.id}
            a={d.analysis}
            icons={icons}
            hideSounds={soundsShownFor === file.id}
            loadAudio={(entry) => call<{ data_url: string }>("preview_audio", { kind, file_id: file.id, entry }).then((r) => r.data_url)}
          />
        )}
      </details>

      {picking && (
        <SoundPicker
          addon={picking}
          title={modName}
          busy={running}
          onClose={() => {
            setPicking(undefined);
            casual.onAdded();
            check(true);
          }}
        />
      )}
    </div>
  );
}
