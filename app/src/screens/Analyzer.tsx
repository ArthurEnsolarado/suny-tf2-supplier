import { useEffect, useState } from "react";
import { call } from "../api";
import { useI18n } from "../i18n";
import type { Analysis, FileInfo, GameIcons, Kind } from "../types";
import { Icon } from "../ui";
import { AnalysisView } from "./AnalysisView";
import { SUPPORTED } from "./InstallBox";

type Phase = { t: "idle" } | { t: "busy" } | { t: "done"; a: Analysis } | { t: "error"; msg: string };

interface Props {
  installRoot: string | undefined;
  kind: Kind;
  modId: number;
  file: FileInfo;
  icons: GameIcons["icons"];
  /** Analisa sozinho ao abrir (usado na ficha de sons, para já poder ouvir). Só para arquivos pequenos. */
  auto?: boolean;
  /** `sounds`: mostra só a lista de sons com ▶ (sem selo, "o que muda" etc.). */
  view?: "full" | "sounds";
  /** Esconde a lista de sons (ela já aparece em outro lugar da ficha). */
  hideSounds?: boolean;
}

/** Arquivos até este tamanho são analisados sozinhos na ficha de um som. */
export const AUTO_ANALYZE_MAX = 25 << 20;

/** Baixa o arquivo (só quando o usuário clica) e mostra o que ele muda, a compatibilidade e deixa ouvir os sons. Não instala nada. */
export function Analyzer({ installRoot, kind, modId, file, icons, auto, view = "full", hideSounds }: Props) {
  const { t, T, tc } = useI18n();
  const [phase, setPhase] = useState<Phase>({ t: "idle" });

  const exe = !!file.analysis_warnings && "contains_exe" in file.analysis_warnings;
  const ext = file.file_name.toLowerCase().split(".").pop() ?? "";
  const eligible = !exe && SUPPORTED.includes(ext);

  // Ficha de som: já analisa ao abrir (arquivo pequeno), para dar para ouvir sem clicar em nada.
  useEffect(() => {
    if (!auto || !eligible || file.size > AUTO_ANALYZE_MAX) return;
    let alive = true;
    setPhase({ t: "busy" });
    call<Analysis>("analyze_file", { path: installRoot ?? null, kind, id: modId, file_id: file.id }).then(
      (a) => alive && setPhase({ t: "done", a }),
      (e: unknown) => alive && setPhase({ t: "error", msg: e instanceof Error ? e.message : String(e) }),
    );
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [auto, file.id]);

  if (!eligible) return null; // o botão de instalar já explica esses casos
  const soundsOnly = view === "sounds";

  const analyze = () => {
    setPhase({ t: "busy" });
    call<Analysis>("analyze_file", { path: installRoot ?? null, kind, id: modId, file_id: file.id }).then(
      (a) => setPhase({ t: "done", a }),
      (e: unknown) => setPhase({ t: "error", msg: e instanceof Error ? e.message : String(e) }),
    );
  };

  if (phase.t === "idle" && soundsOnly) {
    return (
      <div className="analysis">
        <p className="muted">{t("analyzer.tooBigAuto")}</p>
      </div>
    );
  }
  if (phase.t === "idle") {
    return (
      <div className="analysis">
        <button className="btn" onClick={analyze}>
          {t("analyzer.button")}
        </button>
        <p className="muted small">{t("analyzer.hint")}</p>
      </div>
    );
  }
  if (phase.t === "busy" && soundsOnly) {
    return (
      <div className="analysis" role="status" aria-live="polite">
        <p className="muted">
          <Icon name="spinner" /> {t("analyzer.loadingSounds")}
        </p>
      </div>
    );
  }
  if (phase.t === "busy") {
    return (
      <div className="analysis" role="status" aria-live="polite">
        <button className="btn" disabled>
          {t("analyzer.busy")}
        </button>
        <p className="muted small">{t("analyzer.busyHint")}</p>
      </div>
    );
  }
  if (phase.t === "error") {
    return (
      <div className="analysis">
        <div className="confirm bad" role="alert">
          <p>
            <T k="analyzer.failed" v={{ msg: tc(phase.msg) }} />
          </p>
          <button className="btn flat" onClick={() => setPhase({ t: "idle" })}>
            {t("common.retry")}
          </button>
        </div>
      </div>
    );
  }

  return (
    <AnalysisView
      a={phase.a}
      icons={icons}
      view={view}
      hideSounds={hideSounds}
      loadAudio={(entry) => call<{ data_url: string }>("preview_audio", { kind, file_id: file.id, entry }).then((r) => r.data_url)}
    />
  );
}
