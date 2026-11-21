import { useI18n } from "../i18n";
import type { CasualJob } from "../types";

/** Andamento de uma tarefa do Casual: barra, o que está fazendo agora e, se falhou, o motivo e o registro. */
export function JobBar({ job, onDismiss }: { job: CasualJob; onDismiss?: () => void }) {
  const { t, tc } = useI18n();
  const running = job.state === "running";
  const time = (s: number) => (s < 60 ? t("time.s", { n: s }) : t("time.ms", { m: Math.floor(s / 60), s: String(s % 60).padStart(2, "0") }));
  const title = tc(job.title);
  return (
    <div className={`job ${job.state}`} role="status" aria-live="polite">
      <div className="job-head">
        <b>{title}</b>
        <span className="muted">
          {running ? `${job.pct}% · ${time(job.elapsed_s)}` : job.state === "done" ? t("job.doneIn", { time: time(job.elapsed_s) }) : t("job.failed")}
        </span>
      </div>
      <div className="meter" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={job.pct} aria-label={title}>
        <span style={{ width: `${job.state === "failed" ? Math.max(job.pct, 4) : job.pct}%` }} />
      </div>
      {running && <p className="job-msg">{tc(job.msg)}</p>}
      {job.state === "failed" && (
        <>
          <p className="job-msg bad" role="alert">
            {tc(job.error ?? "")}
          </p>
          {job.log.length > 1 && (
            <details>
              <summary>{t("job.log")}</summary>
              <pre className="job-log">{job.log.map(tc).join("\n")}</pre>
            </details>
          )}
        </>
      )}
      {!running && onDismiss && (
        <button className="btn flat mini" onClick={onDismiss}>
          {t("job.dismiss")}
        </button>
      )}
    </div>
  );
}
