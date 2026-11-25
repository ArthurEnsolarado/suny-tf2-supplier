import { useEffect, useState } from "react";
import { call } from "../api";
import { useI18n } from "../i18n";
import type { CasualJob, GameIcons, InstallOutcome, LocalInspect } from "../types";
import { Badge, Icon } from "../ui";
import { useJob } from "../useJob";
import { AnalysisView } from "./AnalysisView";
import { JobBar } from "./JobBar";
import { Modal } from "./Modal";
import { RestartButton } from "./RestartButton";

export interface ImportCasual {
  ready: boolean;
  unknown: boolean;
  onGo: () => void;
  onAdded: () => void;
}

interface Props {
  paths: string[];
  installRoot: string | undefined;
  gameRunning: boolean;
  icons: GameIcons["icons"];
  casual: ImportCasual;
  onClose: () => void;
  onChanged: () => void;
}

type Load = { t: "busy" } | { t: "ok"; d: LocalInspect } | { t: "error"; msg: string };
type Run = { t: "idle" } | { t: "busy" } | { t: "custom"; out: InstallOutcome } | { t: "casual" } | { t: "error"; msg: string };

const lastName = (p: string) => p.split(/[\\/]/).filter(Boolean).pop() ?? p;

/**
 * "Já tenho este mod": o usuário só entrega o arquivo, a pasta ou o .vpk, e o app entende a estrutura, arruma como um mod
 * do catálogo e instala. Primeiro mostra o que entendeu (nada é escrito no jogo até o botão de instalar).
 */
export function ImportMod({ paths, installRoot, gameRunning, icons, casual, onClose, onChanged }: Props) {
  const { t, T, tc } = useI18n();
  const [load, setLoad] = useState<Load>({ t: "busy" });
  const [name, setName] = useState("");
  const [dest, setDest] = useState<"casual" | "custom">("custom");
  const [run, setRun] = useState<Run>({ t: "idle" });
  const [added, setAdded] = useState<string[] | undefined>();
  const [blockedBy, setBlockedBy] = useState<string[]>([]);
  const { job, track, dismiss } = useJob((j: CasualJob) => {
    casual.onAdded();
    if (j.state === "done") {
      const r = j.result as { added?: string[]; blocked_by?: string[] } | null;
      setBlockedBy(r?.blocked_by ?? []);
      setAdded(r?.added ?? []);
    }
  });

  useEffect(() => {
    let alive = true;
    setLoad({ t: "busy" });
    call<LocalInspect>("local_inspect", { paths, path: installRoot ?? null }).then(
      (d) => {
        if (!alive) return;
        setLoad({ t: "ok", d });
        setName(d.staged.suggested_name);
        const map = d.analysis.report.flags.includes("map_content");
        const offline = d.analysis.report.compat === "offline_only";
        setDest(!map && !offline && casual.ready ? "casual" : "custom");
      },
      (e: unknown) => alive && setLoad({ t: "error", msg: e instanceof Error ? e.message : String(e) }),
    );
    return () => {
      alive = false;
    };
    // só relê quando o usuário escolhe outra coisa
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [paths.join("\n"), installRoot]);

  const d = load.t === "ok" ? load.d : undefined;
  const isMap = !!d?.analysis.report.flags.includes("map_content");
  const blocked = d?.analysis.report.compat === "blocked";
  const nothing = !!d && d.analysis.game_files === 0 && d.analysis.vpk_files === 0;
  const jobRunning = job?.state === "running";
  const finished = run.t === "custom" || (job?.state === "done" && !!added);
  const busy = run.t === "busy" || jobRunning;

  const install = () => {
    if (!d) return;
    setRun({ t: "busy" });
    if (dest === "custom") {
      call<InstallOutcome>("local_install", { paths, path: installRoot ?? null, name, dest }).then(
        (out) => {
          setRun({ t: "custom", out });
          onChanged();
        },
        (e: unknown) => setRun({ t: "error", msg: e instanceof Error ? e.message : String(e) }),
      );
    } else {
      call<CasualJob>("local_install", { paths, path: installRoot ?? null, name, dest }).then(
        (j) => {
          setRun({ t: "casual" });
          track(j);
        },
        (e: unknown) => setRun({ t: "error", msg: e instanceof Error ? e.message : String(e) }),
      );
    }
  };

  const options: { id: "casual" | "custom"; title: string; hint: string }[] = [
    { id: "casual", title: t("install.dest.casual"), hint: t("install.dest.casual.hint") },
    { id: "custom", title: t("install.dest.custom"), hint: t("install.dest.custom.hint") },
  ];
  const canInstall = !!d && !blocked && !nothing && name.trim().length > 0 && !busy && !finished && (dest === "custom" ? !!installRoot : casual.ready);

  return (
    <Modal onClose={onClose} labelledBy="import-title">
      <button className="btn flat modal-close" onClick={onClose}>
        {t("common.close")}
      </button>
      <h2 id="import-title" className="modal-title">
        {t("import.title")}
      </h2>

      {load.t === "busy" && (
        <p role="status" aria-live="polite" className="muted">
          <Icon name="spinner" /> {t("import.reading", { name: lastName(paths[0] ?? "") })}
        </p>
      )}

      {load.t === "error" && (
        <div className="confirm bad" role="alert">
          <p>
            <T k="import.failed" v={{ msg: tc(load.msg) }} />
          </p>
          <button className="btn flat" onClick={onClose}>
            {t("common.close")}
          </button>
        </div>
      )}

      {d && (
        <div className="modal-body import-body">
          <div className="modal-media">
            <p className="muted import-source">
              <T k={`import.source.${d.staged.source}` as "import.source.zip"} v={{ name: d.staged.file_name }} />
            </p>

            {(d.staged.moved.length > 0 || d.staged.unplaced.length > 0) && (
              <div className="organized">
                <h3>{t("import.organized")}</h3>
                {d.staged.moved.length > 0 && (
                  <>
                    <p className="muted small">{t("import.organized.note", { n: d.staged.moved.length })}</p>
                    <ul className="moved">
                      {d.staged.moved.slice(0, 12).map((m) => (
                        <li key={m.from}>
                          <code>{m.from}</code> <span aria-hidden="true">→</span> <code>{m.to}</code>
                        </li>
                      ))}
                    </ul>
                    {d.staged.moved.length > 12 && <p className="muted small">{t("import.organized.more", { n: d.staged.moved.length - 12 })}</p>}
                  </>
                )}
                {d.staged.unplaced.length > 0 && (
                  <div className="note">
                    <p>
                      <T k="import.unplaced" v={{ n: d.staged.unplaced.length }} />
                    </p>
                    <ul className="moved">
                      {d.staged.unplaced.slice(0, 8).map((u) => (
                        <li key={u}>
                          <code>{u}</code>
                        </li>
                      ))}
                    </ul>
                    {d.staged.unplaced.length > 8 && <p className="muted small">{t("import.organized.more", { n: d.staged.unplaced.length - 8 })}</p>}
                  </div>
                )}
              </div>
            )}

            <h3>{t("import.understood")}</h3>
            {nothing ? (
              <p>
                <Badge tone="block" small>
                  {t("import.nothing.badge")}
                </Badge>{" "}
                <T k="import.nothing" />
              </p>
            ) : (
              <AnalysisView
                a={d.analysis}
                icons={icons}
                loadAudio={(entry) => call<{ data_url: string }>("local_preview_audio", { id: d.staged.id, entry }).then((r) => r.data_url)}
              />
            )}
          </div>

          <div className="modal-side">
            <label className="field import-name">
              <span>{t("import.name")}</span>
              <input className="blank" value={name} maxLength={80} onChange={(e) => setName(e.target.value)} disabled={busy || finished} />
            </label>
            <p className="muted small">{t("import.name.hint")}</p>

            {!blocked && !nothing && (
              <div className="dest">
                <p className="dest-label">{t("install.dest.label")}</p>
                {isMap ? (
                  <p className="note">
                    <T k="install.map.note" />
                  </p>
                ) : (
                  <div className="dest-options" role="radiogroup" aria-label={t("install.dest.label")}>
                    {options.map((o) => (
                      <button key={o.id} type="button" role="radio" aria-checked={dest === o.id} className="dest-opt" disabled={busy || finished} onClick={() => setDest(o.id)}>
                        <span className="dest-title">{o.title}</span>
                        <span className="dest-hint">{o.hint}</span>
                      </button>
                    ))}
                  </div>
                )}
                {!isMap && d.analysis.report.compat === "offline_only" && dest === "casual" && (
                  <p className="note">
                    <T k="import.offlineNote" />
                  </p>
                )}
              </div>
            )}

            {blocked && (
              <div className="confirm bad" role="alert">
                <p>
                  <T k="import.blocked" />
                </p>
              </div>
            )}

            {!blocked && !nothing && dest === "custom" && !installRoot && <p className="muted">{t("install.needInstall")}</p>}
            {!blocked && !nothing && dest === "casual" && !casual.ready && !casual.unknown && (
              <p className="muted">
                {t("add.needSetup")}{" "}
                <button className="link" onClick={casual.onGo}>
                  {t("add.openCasual")}
                </button>
              </p>
            )}

            {!blocked && !nothing && !finished && (
              <p>
                <button className="btn primary" disabled={!canInstall} onClick={install}>
                  {busy ? t("import.installing") : dest === "casual" ? t("add.button") : t("install.button")}
                </button>
                <span className="muted small install-hint">{dest === "casual" ? t("import.hint.casual") : gameRunning ? t("install.openNote") : t("import.hint.custom")}</span>
              </p>
            )}

            {run.t === "error" && (
              <div className="confirm bad" role="alert">
                <p>
                  <T k="install.failed" v={{ msg: tc(run.msg) }} />
                </p>
                <button className="btn flat" onClick={() => setRun({ t: "idle" })}>
                  {t("common.retry")}
                </button>
              </div>
            )}

            {run.t === "custom" && (
              <div className="confirm good" role="status">
                <p>
                  <Badge tone="ok" small>
                    {t("card.installed")}
                  </Badge>
                </p>
                <p>
                  <T k="install.done" v={{ n: run.out.manifest.files.length, slug: run.out.manifest.slug }} />
                  {run.out.manifest.skipped.length > 0 && <> {t("install.skipped", { n: run.out.manifest.skipped.length })}</>}
                </p>
                {run.out.report.flags.includes("competitive_particles") && (
                  <p>
                    <Badge tone="warn" small>
                      {t("flag.particles")}
                    </Badge>{" "}
                    {t("install.rgl")}
                  </p>
                )}
                {gameRunning && (
                  <>
                    <p className="muted">{t("install.openNote")}</p>
                    <RestartButton path={installRoot} />
                  </>
                )}
                <button className="btn flat" onClick={onClose}>
                  {t("common.close")}
                </button>
              </div>
            )}

            {run.t === "casual" && job && job.state !== "done" && <JobBar job={job} onDismiss={() => { dismiss(); setRun({ t: "idle" }); }} />}
            {job?.state === "done" && added && (
              <div className="confirm good" role="status">
                <p>
                  <Badge tone="ok" small>
                    {t("add.inList")}
                  </Badge>{" "}
                  {added.length === 0 ? t("add.alreadyThere") : blockedBy.length > 0 ? t("add.blockedBy", { names: blockedBy.join(", ") }) : t("add.marked", { names: added.join(", ") })}
                </p>
                <p className="muted">
                  {t("add.gameUnchanged")}{" "}
                  <button className="link" onClick={() => { onClose(); casual.onGo(); }}>
                    {t("add.openToApply")}
                  </button>
                </p>
                <button className="btn flat" onClick={onClose}>
                  {t("common.close")}
                </button>
              </div>
            )}
          </div>
        </div>
      )}
    </Modal>
  );
}
