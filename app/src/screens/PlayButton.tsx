import { useI18n } from "../i18n";
import { Icon } from "../ui";

export interface PlayState {
  /** O TF2 está aberto agora. */
  running: boolean;
  /** O Steam foi mandado abrir o jogo e ainda não apareceu. */
  launching: boolean;
  canPlay: boolean;
  error?: string;
  onPlay: () => void;
}

/** Botão de jogar, no cartão do Início. Fica indisponível enquanto o jogo abre e enquanto ele está aberto. */
export function PlayButton({ play, compact }: { play: PlayState; compact?: boolean }) {
  const { t, tc } = useI18n();
  const { running, launching, canPlay, error, onPlay } = play;
  return (
    <div className="play-wrap">
      <button
        className={`btn play ${compact ? "" : "big"} ${running ? "is-open" : ""}`}
        onClick={onPlay}
        disabled={!canPlay}
        aria-busy={launching}
        title={running ? t("play.title.open") : launching ? t("play.title.launching") : t("play.title.ready")}
      >
        <Icon name={running ? "check" : launching ? "spinner" : "play"} filled={!running && !launching} />
        {running ? t("play.open") : launching ? t("play.launching") : t("play.button")}
      </button>
      {error && (
        <span className="play-error" role="alert">
          {tc(error)}
        </span>
      )}
    </div>
  );
}
