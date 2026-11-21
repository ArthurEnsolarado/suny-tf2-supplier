import { useEffect, useRef, useState } from "react";
import { call, quitApp } from "../api";
import { fmtBytes } from "../format";
import { useI18n } from "../i18n";
import type { CasualJob, UpdateInfo } from "../types";
import { Badge } from "../ui";
import { useJob } from "../useJob";
import { JobBar } from "./JobBar";
import { Markdown } from "./Markdown";

interface Props {
  info: UpdateInfo;
  onClose: () => void;
  /** Esconde o aviso desta versão (continua visível nas Configurações). */
  onSkip: () => void;
}

/** Novidades da versão nova, com o botão de atualizar. O app fecha sozinho quando o instalador abre. */
export function UpdateDialog({ info, onClose, onSkip }: Props) {
  const { t, T, tc, lang } = useI18n();
  const ref = useRef<HTMLDivElement>(null);
  const [confirm, setConfirm] = useState(false);
  const [error, setError] = useState<string | undefined>();
  const { job, track, dismiss } = useJob((j: CasualJob) => {
    if (j.state === "done") setTimeout(() => void quitApp(), 1500); // o instalador já abriu: o app sai para ele poder trocar os arquivos
  });
  const busy = job?.state === "running";

  useEffect(() => {
    ref.current?.focus();
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && !busy && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, busy]);

  const inst = info.installer;
  const canAuto = !!inst && !!inst.sha256;
  const date = (iso: string | null) => (iso ? new Date(iso).toLocaleDateString(lang === "pt" ? "pt-BR" : lang === "es" ? "es-ES" : "en-US", { year: "numeric", month: "short", day: "numeric" }) : "");

  const start = () => {
    setError(undefined);
    setConfirm(false);
    call<CasualJob>("update_install").then(track, (e: unknown) => setError(e instanceof Error ? e.message : String(e)));
  };

  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && !busy && onClose()}>
      <div className="modal small" role="dialog" aria-modal="true" aria-labelledby="upd-title" tabIndex={-1} ref={ref}>
        <button className="btn flat modal-close" onClick={onClose} disabled={busy}>
          {t("common.close")}
        </button>
        <h2 id="upd-title" className="modal-title">
          {t("update.title", { version: info.latest ?? "" })}
        </h2>
        <p>
          <T k="update.youHave" v={{ current: info.current, latest: info.latest ?? "" }} />
        </p>

        {error && (
          <div className="confirm bad" role="alert">
            <p>{tc(error)}</p>
          </div>
        )}
        {job && <JobBar job={job} onDismiss={dismiss} />}

        {!busy && !(job?.state === "done") && (
          <div className="update-actions">
            {confirm ? (
              <div className="confirm">
                <p>
                  <T k="update.confirm" v={{ name: inst?.name ?? "", size: fmtBytes(inst?.size ?? 0) }} />
                </p>
                <div className="row">
                  <button className="btn primary" onClick={start}>
                    {t("update.confirmButton")}
                  </button>
                  <button className="btn flat" onClick={() => setConfirm(false)}>
                    {t("common.cancel")}
                  </button>
                </div>
              </div>
            ) : canAuto ? (
              <div className="row">
                <button className="btn primary" onClick={() => setConfirm(true)}>
                  {t("update.now")}
                </button>
                <button className="btn flat" onClick={onClose}>
                  {t("update.later")}
                </button>
                <button className="btn flat" onClick={onSkip}>
                  {t("update.skip")}
                </button>
              </div>
            ) : (
              <>
                <p className="muted">{inst ? t("update.noHash") : t("update.noInstaller")}</p>
                <div className="row">
                  <button className="btn primary" onClick={() => call("open_releases").catch(() => undefined)}>
                    {t("update.openSite")}
                  </button>
                  <button className="btn flat" onClick={onClose}>
                    {t("update.later")}
                  </button>
                </div>
              </>
            )}
          </div>
        )}
        {job?.state === "done" && (
          <p role="status">
            <Badge tone="ok" small>
              {t("update.opened")}
            </Badge>{" "}
            {t("update.closing")}
          </p>
        )}

        <h3>{t("update.changelog")}</h3>
        {info.releases.map((r) => (
          <section className="release" key={r.version}>
            <h4 className="release-head">
              {r.name}
              <span className="muted"> · v{r.version}{r.published ? ` · ${date(r.published)}` : ""}</span>
            </h4>
            {r.notes ? <Markdown text={r.notes} /> : <p className="muted">{t("update.noNotes")}</p>}
          </section>
        ))}
      </div>
    </div>
  );
}
