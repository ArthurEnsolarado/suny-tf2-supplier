import { useState } from "react";
import { call } from "../api";
import { useI18n } from "../i18n";
import { Icon } from "../ui";

/** Fecha o TF2 e abre de novo, para um mod instalado com o jogo aberto passar a valer. Pede confirmação porque, se o
 *  jogador estiver numa partida, sair conta como abandono. */
export function RestartButton({ path, compact }: { path: string | undefined; compact?: boolean }) {
  const { t, T, tc } = useI18n();
  const [phase, setPhase] = useState<"idle" | "confirm" | "busy" | "done">("idle");
  const [error, setError] = useState<string | undefined>();

  const run = () => {
    setPhase("busy");
    setError(undefined);
    call("restart_game", { path: path ?? null }).then(
      () => {
        setPhase("done");
        setTimeout(() => setPhase("idle"), 6000);
      },
      (e: unknown) => {
        setPhase("idle");
        setError(e instanceof Error ? e.message : String(e));
      },
    );
  };

  if (phase === "confirm") {
    return (
      <div className="confirm restart-confirm" role="alertdialog" aria-label={t("restart.button")}>
        <p>
          <T k="restart.confirm" />
        </p>
        <div className="row">
          <button className="btn primary mini" onClick={run}>
            {t("restart.yes")}
          </button>
          <button className="btn flat mini" onClick={() => setPhase("idle")}>
            {t("common.cancel")}
          </button>
        </div>
      </div>
    );
  }
  return (
    <span className="restart">
      <button className={`btn flat ${compact ? "mini" : ""}`} onClick={() => setPhase("confirm")} disabled={phase === "busy" || phase === "done"}>
        <Icon name={phase === "busy" ? "spinner" : phase === "done" ? "check" : "refresh"} />
        {phase === "busy" ? t("restart.busy") : phase === "done" ? t("restart.done") : t("restart.button")}
      </button>
      {error && (
        <span className="play-error" role="alert">
          {t("restart.failed", { msg: tc(error) })}
        </span>
      )}
    </span>
  );
}
