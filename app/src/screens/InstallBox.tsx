import { useState } from "react";
import { call } from "../api";
import { useI18n } from "../i18n";
import type { FileInfo, InstallOutcome, Kind } from "../types";
import { Badge } from "../ui";

type Phase =
  | { t: "idle" }
  | { t: "confirm" }
  | { t: "busy" }
  | { t: "done"; out: InstallOutcome }
  | { t: "error"; msg: string };

interface Props {
  /** Pasta raiz do TF2 escolhida. Sem ela, não dá para instalar. */
  installRoot: string | undefined;
  kind: Kind;
  modId: number;
  file: FileInfo;
  /** O TF2 está aberto agora (instalar só com ele fechado). */
  gameRunning: boolean;
  /** Chamado depois de uma instalação, para a lista de instalados se atualizar. */
  onChanged: () => void;
}

export const SUPPORTED = ["zip", "rar", "7z"];

/** Instalar um arquivo do GameBanana em tf/custom. Sempre pede confirmação antes de escrever no jogo. */
export function InstallBox({ installRoot, kind, modId, file, gameRunning, onChanged }: Props) {
  const { t, T, tc } = useI18n();
  const [phase, setPhase] = useState<Phase>({ t: "idle" });

  const exe = !!file.analysis_warnings && "contains_exe" in file.analysis_warnings;
  const ext = file.file_name.toLowerCase().split(".").pop() ?? "";

  if (exe) {
    return <p className="muted">{t("install.blockedExe")}</p>;
  }
  if (!SUPPORTED.includes(ext)) {
    return (
      <p className="muted">
        <T k="install.badFormat" v={{ ext: ext || "?" }} />
      </p>
    );
  }
  if (!installRoot) {
    return <p className="muted">{t("install.needInstall")}</p>;
  }

  const run = () => {
    setPhase({ t: "busy" });
    call<InstallOutcome>("install_mod", { path: installRoot, kind, id: modId, file_id: file.id }).then(
      (out) => {
        setPhase({ t: "done", out });
        onChanged();
      },
      (e: unknown) => setPhase({ t: "error", msg: e instanceof Error ? e.message : String(e) }),
    );
  };

  if (phase.t === "idle") {
    return (
      <p>
        <button className="btn" onClick={() => setPhase({ t: "confirm" })} disabled={gameRunning} title={gameRunning ? t("install.closeGame") : undefined}>
          {t("install.button")}
        </button>
        <span className="muted small install-hint">{gameRunning ? t("install.closeGame") : t("install.hint")}</span>
      </p>
    );
  }

  if (phase.t === "confirm") {
    return (
      <div className="confirm">
        <p>
          <T k="install.confirm" />
        </p>
        <div className="row">
          <button className="btn primary" onClick={run} disabled={gameRunning}>
            {t("install.confirmButton")}
          </button>
          <button className="btn flat" onClick={() => setPhase({ t: "idle" })}>
            {t("common.cancel")}
          </button>
        </div>
      </div>
    );
  }

  if (phase.t === "busy") {
    return (
      <p role="status" aria-live="polite">
        <button className="btn" disabled>
          {t("install.busy")}
        </button>
        <span className="muted"> {t("install.dontClose")}</span>
      </p>
    );
  }

  if (phase.t === "error") {
    return (
      <div className="confirm bad" role="alert">
        <p>
          <T k="install.failed" v={{ msg: tc(phase.msg) }} />
        </p>
        <button className="btn flat" onClick={() => setPhase({ t: "idle" })}>
          {t("common.retry")}
        </button>
      </div>
    );
  }

  const { manifest, report } = phase.out;
  return (
    <div className="confirm good" role="status">
      <p>
        <Badge tone="ok" small>
          {t("card.installed")}
        </Badge>
      </p>
      <p>
        <T k="install.done" v={{ n: manifest.files.length, slug: manifest.slug }} />
        {manifest.skipped.length > 0 && <> {t("install.skipped", { n: manifest.skipped.length })}</>}
      </p>
      {report.flags.includes("competitive_particles") && (
        <p>
          <Badge tone="warn" small>
            {t("flag.particles")}
          </Badge>{" "}
          {t("install.rgl")}
        </p>
      )}
      <p className="muted">
        <T k="install.notCasual" />
      </p>
    </div>
  );
}
