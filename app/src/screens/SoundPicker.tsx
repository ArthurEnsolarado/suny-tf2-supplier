import { useEffect, useRef, useState } from "react";
import { call } from "../api";
import { fmtBytes } from "../format";
import { useI18n } from "../i18n";
import type { SoundFamily } from "../types";
import { Icon } from "../ui";
import { Modal } from "./Modal";

/** Grupos que o app sabe explicar (nomes do script de sons do jogo). Os outros aparecem pelo nome do arquivo. */
const KNOWN: Record<string, "chargeWorld" | "chargeFlesh" | "chargeRange" | "chargeWindup" | "swordSwing"> = {
  "weapons/demo_charge_hit_world": "chargeWorld",
  "weapons/demo_charge_hit_flesh": "chargeFlesh",
  "weapons/demo_charge_hit_flesh_range": "chargeRange",
  "weapons/demo_charge_windup": "chargeWindup",
  "weapons/demo_sword_swing": "swordSwing",
};

interface Props {
  /** Nome do mod na lista do casual (identifica) e o nome que a pessoa conhece. */
  addon: string;
  title: string;
  busy: boolean;
  onClose: () => void;
}

/** Escolher qual som do pacote toca em cada grupo que o jogo sorteia. */
export function SoundPicker({ addon, title, busy, onClose }: Props) {
  const { t, T, tc } = useI18n();
  const [families, setFamilies] = useState<SoundFamily[] | undefined>();
  const [error, setError] = useState<string | undefined>();
  const [saving, setSaving] = useState(false);
  const [playing, setPlaying] = useState<string | undefined>();
  const audio = useRef<HTMLAudioElement | undefined>(undefined);
  const wanted = useRef<string | undefined>(undefined);
  const fail = (e: unknown) => setError(e instanceof Error ? e.message : String(e));

  const stop = () => {
    wanted.current = undefined;
    audio.current?.pause();
    audio.current = undefined;
    setPlaying(undefined);
  };

  useEffect(() => {
    call<{ families: SoundFamily[] }>("casual_sounds", { name: addon }).then((r) => setFamilies(r.families), fail);
    return () => {
      wanted.current = undefined;
      audio.current?.pause();
    };
  }, [addon]);

  const play = (file: string) => {
    if (playing === file) return stop();
    stop();
    wanted.current = file;
    setPlaying(file);
    call<{ data_url: string }>("casual_sound_play", { name: addon, file }).then(
      (r) => {
        if (wanted.current !== file) return; // pediram outro enquanto este carregava
        const a = new Audio(r.data_url);
        audio.current = a;
        a.onended = () => setPlaying(undefined);
        a.play().catch(() => setPlaying(undefined));
      },
      (e: unknown) => {
        setPlaying(undefined);
        fail(e);
      },
    );
  };

  const choose = (family: string, choice: string) => {
    setSaving(true);
    setError(undefined);
    call<{ families: SoundFamily[] }>("casual_sound_choose", { name: addon, family, choice })
      .then((r) => setFamilies(r.families), fail)
      .finally(() => setSaving(false));
  };

  const option = (f: SoundFamily, value: string, label: string) => (
    <label className="pick-option">
      <input type="radio" name={f.key} checked={f.choice === value} disabled={busy || saving} onChange={() => choose(f.key, value)} />
      <span>{label}</span>
    </label>
  );

  return (
    <Modal onClose={onClose} labelledBy="picker-title">
      <button className="btn flat modal-close" onClick={onClose}>
        {t("common.close")}
      </button>
      <h2 id="picker-title" className="modal-title">
        {t("picker.title", { name: title })}
      </h2>
      <div className="picker">
        <p className="muted">
          <T k="picker.body" />
        </p>
        {error && (
          <div className="confirm bad" role="alert">
            <p>{tc(error)}</p>
          </div>
        )}
        {!families && !error && (
          <p className="muted" role="status">
            <Icon name="spinner" /> {t("picker.reading")}
          </p>
        )}
        {families?.map((f) => {
          const known = KNOWN[f.name];
          return (
            <fieldset key={f.key} className="pick-family">
              <legend>{known ? t(`picker.family.${known}`) : f.name}</legend>
              {known && <p className="muted small">{f.name}</p>}
              {option(f, "random", t("picker.random", { n: f.options.length }))}
              {f.options.map((o) => (
                <div key={o.file} className="pick-row">
                  {option(f, o.file, `${t("picker.always", { name: o.name })} · ${fmtBytes(o.size)}`)}
                  <button className="btn mini" onClick={() => play(o.file)} aria-label={t(playing === o.file ? "picker.stop" : "picker.play", { name: o.name })}>
                    <Icon name={playing === o.file ? "pause" : "play"} filled />
                  </button>
                </div>
              ))}
              {option(f, "original", t("picker.original"))}
            </fieldset>
          );
        })}
      </div>
    </Modal>
  );
}
