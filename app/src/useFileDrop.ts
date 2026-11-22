import { useEffect, useRef, useState } from "react";
import { inTauri } from "./api";

/**
 * Escuta arquivos e pastas soltos na janela do app (só no app de verdade: o navegador não entrega o caminho do disco).
 * Devolve `true` enquanto algo está sendo arrastado sobre a janela, para a tela mostrar o aviso de soltar.
 */
export function useFileDrop(onDrop: (paths: string[]) => void): boolean {
  const [over, setOver] = useState(false);
  const cb = useRef(onDrop);
  cb.current = onDrop;

  useEffect(() => {
    if (!inTauri) return;
    let off: (() => void) | undefined;
    let gone = false;
    import("@tauri-apps/api/webview")
      .then(({ getCurrentWebview }) =>
        getCurrentWebview().onDragDropEvent((e) => {
          const p = e.payload;
          if (p.type === "enter" || p.type === "over") setOver(true);
          else if (p.type === "drop") {
            setOver(false);
            if (p.paths.length > 0) cb.current(p.paths);
          } else setOver(false);
        }),
      )
      .then((f) => {
        if (gone) f();
        else off = f;
      })
      .catch(() => undefined);
    return () => {
      gone = true;
      off?.();
    };
  }, []);

  return over;
}
