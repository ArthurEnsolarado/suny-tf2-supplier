import { useEffect, useState } from "react";
import { call } from "./api";
import type { ArtSet, Details, Page, Summary } from "./types";

/** Um destaque: vira slide de fundo e herói do Início. */
export interface Slide {
  id: string;
  /** Imagem grande (data URL do jogo ou URL do CDN do GameBanana). */
  src: string;
  title: string;
  /** Uma linha curta abaixo do título. */
  byline: string | null;
  category: string | null;
  likes: number;
  views: number;
  /** `game` = arte do próprio jogo; `gb` = item do GameBanana (dá para abrir no Catálogo). */
  source: "game" | "gb";
  gbId: number | null;
}

export interface Rails {
  mods: Summary[];
  sounds: Summary[];
}

interface Loaded {
  slides: Slide[];
  rails: Rails;
}

/** Só entra o que o GameBanana mostra sem esconder: nunca usamos conteúdo sensível como fundo. */
const isSafe = (r: Summary) => r.initial_visibility === "show" && !r.has_content_ratings && !!r.thumb;

let cache: Promise<Loaded> | undefined;

async function load(): Promise<Loaded> {
  const list = (kind: "Mod" | "Sound", sort: "MostLiked" | "MostDownloaded") =>
    call<Page<Summary>>("gb_list", { kind, sort, query: { page: 1, per_page: 24, category: null, search: null } }).then(
      (p) => p.records.filter(isSafe),
      () => [] as Summary[],
    );
  const [mods, sounds] = await Promise.all([list("Mod", "MostLiked"), list("Sound", "MostDownloaded")]);
  const heroes = mods.slice(0, 7);
  const details = await Promise.all(heroes.map((r) => call<Details>("gb_details", { kind: "Mod", id: r.id }).catch(() => null)));
  const slides: Slide[] = heroes.flatMap((r, i) => {
    const src = details[i]?.images[0]?.url ?? r.thumb;
    return src
      ? [
          {
            id: `gb-${r.id}`,
            src,
            title: r.name,
            byline: r.submitter?.name ?? null,
            category: r.root_category?.name ?? null,
            likes: r.like_count,
            views: r.view_count,
            source: "gb" as const,
            gbId: r.id,
          },
        ]
      : [];
  });
  return { slides, rails: { mods: mods.slice(7, 19), sounds: sounds.slice(0, 12) } };
}

/** Destaques do GameBanana, buscados uma vez por sessão. Sem rede, volta vazio e o app usa só a arte do jogo. */
export function useFeatured(art: ArtSet | null, gameTitle: string): { slides: Slide[]; rails: Rails; loading: boolean } {
  const [data, setData] = useState<Loaded | undefined>();
  useEffect(() => {
    let alive = true;
    cache ??= load();
    cache.then(
      (d) => alive && setData(d),
      () => {
        cache = undefined;
        if (alive) setData({ slides: [], rails: { mods: [], sounds: [] } });
      },
    );
    return () => {
      alive = false;
    };
  }, []);
  const game: Slide[] = art?.hero ? [{ id: "game", src: art.hero, title: gameTitle, byline: null, category: null, likes: 0, views: 0, source: "game", gbId: null }] : [];
  return { slides: [...game, ...(data?.slides ?? [])], rails: data?.rails ?? { mods: [], sounds: [] }, loading: !data };
}

/** Passa de slide em slide sozinho. Pausa com a aba escondida, com o mouse em cima do herói ou a pedido. */
export function useSlideshow(count: number, ms: number, paused: boolean): [number, (i: number) => void] {
  const [index, setIndex] = useState(0);
  useEffect(() => {
    if (count < 2 || paused) return;
    const id = setInterval(() => {
      if (!document.hidden) setIndex((i) => (i + 1) % count);
    }, ms);
    return () => clearInterval(id);
  }, [count, ms, paused, index]);
  return [count ? index % count : 0, setIndex];
}
