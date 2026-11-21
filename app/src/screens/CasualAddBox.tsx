import { useState } from "react";
import { call } from "../api";
import { useI18n } from "../i18n";
import type { CasualJob, FileInfo, Kind } from "../types";
import { Badge } from "../ui";
import { useJob } from "../useJob";
import { SUPPORTED } from "./InstallBox";
import { JobBar } from "./JobBar";

interface Props {
  kind: Kind;
  modId: number;
  file: FileInfo;
  /** O Casual Preloader já está configurado no app? */
  ready: boolean;
  /** Ainda não carregou o estado do Casual. */
  unknown: boolean;
  onGoCasual: () => void;
  /** Depois de adicionar, a lista da aba Casual precisa ser relida. */
  onAdded: () => void;
}

/** Botão "Adicionar ao Casual": baixa o arquivo e o coloca na lista do preloader. Não altera o jogo. */
export function CasualAddBox({ kind, modId, file, ready, unknown, onGoCasual, onAdded }: Props) {
  const { t, T, tc } = useI18n();
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState<string | undefined>();
  const [added, setAdded] = useState<string[] | undefined>();
  const [hasModels, setHasModels] = useState(false);
  const [variants, setVariants] = useState(false);
  const { job, track, dismiss } = useJob((j: CasualJob) => {
    onAdded();
    if (j.state === "done") {
      const r = j.result as { added?: string[]; already_there?: boolean; has_models?: boolean; variants?: boolean } | null;
      setAdded(r?.added ?? []);
      setHasModels(!!r?.has_models);
      setVariants(!!r?.variants);
    }
  });

  const exe = !!file.analysis_warnings && "contains_exe" in file.analysis_warnings;
  const ext = file.file_name.toLowerCase().split(".").pop() ?? "";
  if (exe || !SUPPORTED.includes(ext) || unknown) return null;

  if (!ready) {
    return (
      <p className="muted">
        {t("add.needSetup")}{" "}
        <button className="link" onClick={onGoCasual}>
          {t("add.openCasual")}
        </button>
      </p>
    );
  }

  const run = () => {
    setStarting(true);
    setError(undefined);
    setAdded(undefined);
    call<CasualJob>("casual_add", { kind, id: modId, file_id: file.id }).then(
      (j) => {
        setStarting(false);
        track(j);
      },
      (e: unknown) => {
        setStarting(false);
        setError(e instanceof Error ? e.message : String(e));
      },
    );
  };

  const running = job?.state === "running";
  return (
    <div className="casual-add">
      <button className="btn primary" onClick={run} disabled={starting || running}>
        {starting ? t("add.downloading") : t("add.button")}
      </button>
      {error && (
        <div className="confirm bad" role="alert">
          <p>
            <T k="add.failed" v={{ msg: tc(error) }} />
          </p>
        </div>
      )}
      {job && job.state !== "done" && <JobBar job={job} onDismiss={dismiss} />}
      {job?.state === "done" && added && (
        <div className="confirm good" role="status">
          <p>
            <Badge tone="ok" small>
              {t("add.inList")}
            </Badge>{" "}
            {added.length === 0 ? t("add.alreadyThere") : variants ? t("add.variants", { n: added.length }) : t("add.marked", { names: added.join(", ") })}
          </p>
          {variants && (
            <ul className="muted small">
              {added.map((n) => (
                <li key={n}>{n}</li>
              ))}
            </ul>
          )}
          {hasModels && (
            <p>
              <Badge tone="warn" small>
                {t("casual.tag.models")}
              </Badge>{" "}
              <T k="add.hasModels" />
            </p>
          )}
          <p className="muted">
            {t("add.gameUnchanged")}{" "}
            <button className="link" onClick={onGoCasual}>
              {t("add.openToApply")}
            </button>
          </p>
        </div>
      )}
    </div>
  );
}
