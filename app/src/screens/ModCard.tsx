import { useEffect, useState, type ReactNode } from "react";
import { call } from "../api";
import type { Details } from "../types";
import { Icon } from "../ui";

const cache = new Map<string, Promise<string | null>>();

/** Miniatura de um item do GameBanana (a primeira imagem da ficha). Guardada em memória: cada item é pedido uma vez. */
function loadThumb(kind: string, id: number): Promise<string | null> {
  const key = `${kind}:${id}`;
  let p = cache.get(key);
  if (!p) {
    p = call<Details>("gb_details", { kind, id }).then(
      (d) => d.images[0]?.thumb ?? null,
      () => {
        cache.delete(key); // falhou: tenta de novo na próxima vez
        return null;
      },
    );
    cache.set(key, p);
  }
  return p;
}

function useThumb(kind: string | null, id: number | null): string | null {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    setSrc(null);
    if (kind && id) loadThumb(kind, id).then((s) => alive && setSrc(s));
    return () => {
      alive = false;
    };
  }, [kind, id]);
  return src;
}

interface Props {
  /** `Mod` ou `Sound`; `null` para itens que não vêm do GameBanana. */
  kind: string | null;
  gbId: number | null;
  title: string;
  subtitle?: string;
  /** Linha pequena abaixo do autor (arquivos, tamanho...). */
  meta?: ReactNode;
  /** Selo no canto da imagem. */
  flag?: ReactNode;
  /** Botões e caixas do rodapé do card. */
  actions: ReactNode;
}

/** Card de mod já instalado ou na lista do casual. Mesmo desenho do catálogo, com os botões no rodapé. */
export function ModCard({ kind, gbId, title, subtitle, meta, flag, actions }: Props) {
  const thumb = useThumb(kind, gbId);
  return (
    <li className="mcard">
      <span className={`card-img ${thumb ? "" : "noimg"}`}>
        {thumb ? (
          <img src={thumb} alt="" loading="lazy" referrerPolicy="no-referrer" />
        ) : (
          <span className="card-empty" aria-hidden="true">
            <Icon name={kind === "Sound" ? "sound" : "box"} />
          </span>
        )}
        {flag && <span className="card-flag">{flag}</span>}
      </span>
      <span className="card-name">
        <span className="clamp">{title}</span>
      </span>
      <span className="card-author">{subtitle ?? " "}</span>
      <span className="card-meta">{meta ?? " "}</span>
      <div className="mcard-actions">{actions}</div>
    </li>
  );
}
