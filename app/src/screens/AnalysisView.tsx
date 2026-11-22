import { useEffect, useRef, useState } from "react";
import { useI18n } from "../i18n";
import { fmtBytes, fmtInt } from "../format";
import type { Analysis, GameIcons } from "../types";
import { Badge, Icon, type Tone } from "../ui";

/** Chave do ícone → nome para exibir. */
const CLASS_NAMES: Record<string, string> = {
  scout: "Scout",
  soldier: "Soldier",
  pyro: "Pyro",
  demoman: "Demoman",
  heavy: "Heavy",
  engineer: "Engineer",
  medic: "Medic",
  sniper: "Sniper",
  spy: "Spy",
};

export const COMPAT: Record<string, { tone: Tone; label: "compat.casual_ok" | "compat.partial" | "compat.offline_only" | "compat.needs_index" | "compat.blocked" }> = {
  casual_ok: { tone: "ok", label: "compat.casual_ok" },
  partial: { tone: "warn", label: "compat.partial" },
  offline_only: { tone: "warn", label: "compat.offline_only" },
  needs_index: { tone: "neutral", label: "compat.needs_index" },
  blocked: { tone: "block", label: "compat.blocked" },
};

/** Quadradinho com o ícone da classe (lido do próprio jogo) ou, sem classe, o ícone de som. */
function Square({ cls, icons, fallback = "sound" }: { cls: string | null; icons: GameIcons["icons"]; fallback?: "sound" | "box" }) {
  const src = cls ? icons[cls] : undefined;
  return (
    <span className="sq" aria-hidden="true">
      {src ? <img src={src} alt="" /> : <Icon name={fallback} />}
    </span>
  );
}

interface Props {
  a: Analysis;
  icons: GameIcons["icons"];
  /** Devolve o áudio de uma entrada do zip como data URL (cada tela sabe de onde o zip veio). */
  loadAudio: (entry: string) => Promise<string>;
  /** `sounds`: mostra só a lista de sons com ▶ (sem selo, "o que muda" etc.). */
  view?: "full" | "sounds";
  /** Esconde a lista de sons (ela já aparece em outro lugar da ficha). */
  hideSounds?: boolean;
}

/** O resultado de uma análise: selo, o que muda, classes afetadas e os sons para ouvir. Serve ao catálogo e ao mod local. */
export function AnalysisView({ a, icons, loadAudio, view = "full", hideSounds }: Props) {
  const { t, tc } = useI18n();
  const [playing, setPlaying] = useState<{ entry: string; state: "loading" | "playing" } | undefined>();
  const [audioError, setAudioError] = useState<string | undefined>();
  const audio = useRef<HTMLAudioElement>(null);
  const urls = useRef(new Map<string, string>());
  const soundsOnly = view === "sounds";

  // fechar a ficha para o som
  useEffect(() => {
    const el = audio.current;
    return () => el?.pause();
  }, []);

  const toggle = async (entry: string) => {
    const el = audio.current;
    if (!el) return;
    if (playing?.entry === entry && playing.state === "playing") {
      el.pause();
      setPlaying(undefined);
      return;
    }
    setAudioError(undefined);
    setPlaying({ entry, state: "loading" });
    try {
      let url = urls.current.get(entry);
      if (!url) {
        url = await loadAudio(entry);
        urls.current.set(entry, url);
      }
      el.src = url;
      await el.play();
      setPlaying({ entry, state: "playing" });
    } catch (e) {
      setPlaying(undefined);
      setAudioError(e instanceof Error ? tc(e.message) : t("analyzer.cantPlay"));
    }
  };

  const compat = COMPAT[a.report.compat];
  return (
    <div className="analysis">
      <audio ref={audio} onEnded={() => setPlaying(undefined)} onError={() => setPlaying(undefined)} />

      {!soundsOnly && (
        <>
          <p>
            <Badge tone={compat?.tone ?? "neutral"} small>
              {compat ? t(compat.label) : a.report.compat}
            </Badge>
          </p>
          <ul className="reasons">
            {a.report.reasons.map((r) => (
              <li key={r}>{tc(r)}</li>
            ))}
          </ul>
          {!a.vanilla_known && <p className="muted small">{t("analyzer.noVanilla")}</p>}

          {a.classes.length > 0 && (
            <div className="affects">
              <b>{t("analyzer.affects")}</b>
              {a.classes.map((c) => (
                <span className="cls" key={c}>
                  <Square cls={c} icons={icons} />
                  {CLASS_NAMES[c] ?? c}
                </span>
              ))}
            </div>
          )}

          <h4>{t("analyzer.changes")}</h4>
          {a.groups.length === 0 ? (
            <p className="muted">{t("analyzer.noGameFiles")}</p>
          ) : (
            <ul className="changes">
              {a.groups.map((g) => (
                <li key={g.label}>
                  <Square cls={g.class} icons={icons} fallback={g.kind === "sound" ? "sound" : "box"} />
                  <span className="ch-main">
                    <span>
                      <b>{tc(g.label)}</b> <span className="muted">· {t("files", { n: g.total })}</span>
                    </span>
                    <span className="ch-sub">
                      {g.replaces !== null && g.added !== null
                        ? `${t("analyzer.replaces", { n: g.replaces })} · ${t("analyzer.added", { n: g.added })}`
                        : g.examples.join(", ")}
                    </span>
                  </span>
                </li>
              ))}
            </ul>
          )}
        </>
      )}

      {a.audio.length === 0 && soundsOnly && <p className="muted">{t("analyzer.noAudio")}</p>}
      {a.audio.length > 0 && !hideSounds && (
        <>
          <h4>
            {t("analyzer.sounds", { n: a.audio_total })}
            {a.audio_total > a.audio.length ? ` · ${t("analyzer.showing", { shown: fmtInt(a.audio.length) })}` : ""}
          </h4>
          <ul className="sounds">
            {a.audio.map((s) => {
              const active = playing?.entry === s.entry;
              return (
                <li key={s.entry}>
                  <Square cls={s.class} icons={icons} />
                  <span className="ch-main">
                    <span className="snd-name">{s.name}</span>
                    <span className="ch-sub">
                      {tc(s.label)} · {fmtBytes(s.size)}
                      {s.vanilla === true && <span className="tag ok"> {t("analyzer.replacesGame")}</span>}
                      {s.vanilla === false && <span className="tag new"> {t("analyzer.newSound")}</span>}
                    </span>
                  </span>
                  <button
                    className="btn mini"
                    onClick={() => toggle(s.entry)}
                    aria-label={`${active && playing?.state === "playing" ? t("analyzer.stop") : t("analyzer.listen")} ${s.name}`}
                    disabled={active && playing?.state === "loading"}
                  >
                    {active && playing?.state === "loading" ? "…" : active ? <Icon name="pause" /> : <Icon name="play" filled />}
                  </button>
                </li>
              );
            })}
          </ul>
          {audioError && (
            <p className="muted small" role="alert">
              {audioError}
            </p>
          )}
        </>
      )}
    </div>
  );
}
