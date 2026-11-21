import { useEffect, useRef } from "react";
import { inTauri } from "./api";

export const ZOOM_MODES = ["auto", "0.9", "1", "1.1", "1.25", "1.5", "1.75", "2"] as const;

/** Largura/altura (em pixels CSS a 100%) para a qual a interface foi desenhada. Telas maiores ganham zoom. */
const DESIGN_W = 1480;
const DESIGN_H = 720;
const MAX_AUTO = 2.25;
const MIN_W = 920;
const MIN_H = 600;

/** Zoom automático pelo tamanho da janela: o app foi desenhado para ~1480x720; em telas grandes (2K, 4K, ultrawide)
 *  ele cresce em vez de ficar pequeno e esticado. Só no app (no navegador de desenvolvimento não faz nada). */
export function useUiZoom(mode: string) {
  const applied = useRef(1);
  useEffect(() => {
    if (!inTauri) return;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const apply = async () => {
      let z: number;
      if (mode === "auto") {
        // pixels CSS "a 100%": independe do zoom que já aplicamos, então não há vai e vem
        const baseW = window.innerWidth * applied.current;
        const baseH = window.innerHeight * applied.current;
        z = Math.min(MAX_AUTO, Math.max(1, Math.min(baseW / DESIGN_W, baseH / DESIGN_H)));
      } else {
        z = Number(mode) || 1;
      }
      // Nunca deixa a tela com menos de 920x600 pixels CSS (o mínimo que o layout suporta), mesmo no zoom fixo.
      const cap = Math.max(1, Math.min((window.innerWidth * applied.current) / MIN_W, (window.innerHeight * applied.current) / MIN_H));
      z = Math.floor(Math.min(z, cap) * 20 + 1e-6) / 20;
      if (Math.abs(z - applied.current) < 0.001) return;
      const before = applied.current;
      applied.current = z;
      try {
        const { getCurrentWebview } = await import("@tauri-apps/api/webview");
        await getCurrentWebview().setZoom(z);
      } catch {
        applied.current = before; // sem permissão ou sem suporte: segue como está
      }
    };
    const onResize = () => {
      clearTimeout(timer);
      timer = setTimeout(() => void apply(), 250);
    };
    void apply();
    window.addEventListener("resize", onResize);
    return () => {
      clearTimeout(timer);
      window.removeEventListener("resize", onResize);
    };
  }, [mode]);
}
