import { useCallback, useEffect, useRef, useState } from "react";
import { call } from "./api";

interface Preview {
  url: string;
  name: string;
  total: number;
}

export interface SoundPreviewState {
  id: number;
  state: "loading" | "playing" | "error";
  /** Quando deu erro: o motivo (texto do núcleo, a tela traduz). */
  msg?: string;
  name?: string;
  total?: number;
}

/**
 * Prévia de som direto do catálogo: um único `<audio>`, um som por vez. O núcleo baixa o primeiro arquivo do item
 * (cache de análise), acha o primeiro áudio tocável e devolve como data URL. Os últimos sons ficam em memória.
 */
export function useSoundPreview() {
  const audio = useRef<HTMLAudioElement | null>(null);
  const cache = useRef(new Map<number, Preview>());
  const [cur, setCur] = useState<SoundPreviewState | null>(null);
  const token = useRef(0);

  useEffect(() => {
    const el = audio.current;
    return () => el?.pause();
  }, []);

  const stop = useCallback(() => {
    token.current++;
    audio.current?.pause();
    setCur(null);
  }, []);

  const toggle = useCallback(
    async (id: number) => {
      if (cur?.id === id && (cur.state === "playing" || cur.state === "loading")) {
        stop();
        return;
      }
      const mine = ++token.current;
      audio.current?.pause();
      setCur({ id, state: "loading" });
      try {
        let p = cache.current.get(id);
        if (!p) {
          // O núcleo já tem limite de tempo; este é só o cinto de segurança para a tela nunca girar para sempre.
          const r = await Promise.race([
            call<{ data_url: string; name: string; total: number }>("sound_preview", { kind: "Sound", id }),
            new Promise<never>((_, rej) => setTimeout(() => rej(new Error("A prévia demorou demais. Abra a ficha e use Analisar, ou tente de novo.")), 135_000)),
          ]);
          p = { url: r.data_url, name: r.name, total: r.total };
          cache.current.set(id, p);
          if (cache.current.size > 12) cache.current.delete(cache.current.keys().next().value as number);
        }
        if (mine !== token.current || !audio.current) return; // clicou em outro enquanto carregava
        audio.current.src = p.url;
        await audio.current.play();
        if (mine === token.current) setCur({ id, state: "playing", name: p.name, total: p.total });
      } catch (e) {
        if (mine === token.current) setCur({ id, state: "error", msg: e instanceof Error ? e.message : String(e) });
      }
    },
    [cur, stop],
  );

  return {
    cur,
    toggle,
    stop,
    /** Coloque uma vez na tela: é o elemento que toca. */
    audioRef: audio,
    onEnded: () => setCur(null),
  };
}
