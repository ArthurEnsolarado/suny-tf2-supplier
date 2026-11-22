import { useEffect, useRef, useState } from "react";

interface Layer {
  key: number;
  src: string;
  /** Direção do movimento lento (efeito Ken Burns), sorteada por camada. */
  dx: number;
  dy: number;
}

/** Fundo da janela: a arte do slide atual, desfocada, escurecida e em movimento lento, com troca em fade.
 *  Sem imagem, sobra só a aurora (manchas de cor suaves). Tudo é decorativo (`aria-hidden`). */
export function Backdrop({ src, mode }: { src: string | undefined; mode: "home" | "calm" }) {
  const [layers, setLayers] = useState<Layer[]>([]);
  const counter = useRef(0);

  useEffect(() => {
    if (!src) return;
    let alive = true;
    const add = () => {
      if (!alive) return;
      setLayers((ls) => {
        if (ls.at(-1)?.src === src) return ls;
        const n = ++counter.current;
        return [...ls.slice(-1), { key: n, src, dx: (n % 2 ? 1 : -1) * (1 + (n % 3)), dy: (n % 3) - 1 }];
      });
    };
    // espera a imagem decodificar para a troca nunca piscar
    const img = new Image();
    img.src = src;
    if (img.decode) img.decode().then(add, add);
    else img.onload = add;
    return () => {
      alive = false;
    };
  }, [src]);

  // a camada antiga sai depois do fade
  useEffect(() => {
    if (layers.length < 2) return;
    const id = setTimeout(() => setLayers((ls) => ls.slice(-1)), 1900);
    return () => clearTimeout(id);
  }, [layers]);

  return (
    <div className="backdrop" data-mode={mode} aria-hidden="true">
      <div className="aurora">
        <span />
        <span />
        <span />
      </div>
      {layers.map((l, i) => (
        <div
          key={l.key}
          className={`bd-layer ${i === layers.length - 1 ? "in" : "out"}`}
          style={{ backgroundImage: `url("${l.src}")`, ["--dx" as string]: `${l.dx}%`, ["--dy" as string]: `${l.dy}%` }}
        />
      ))}
      <div className="bd-scrim" />
      <div className="bd-vignette" />
      <div className="bd-grain" />
    </div>
  );
}
