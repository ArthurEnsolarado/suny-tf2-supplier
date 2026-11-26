import { useState } from "react";
import { call } from "../api";
import { useI18n } from "../i18n";
import type { Clash, InstalledAddon } from "../types";
import { useAsync } from "../ui";

interface Props {
  versions: InstalledAddon[];
  /** Dá para aplicar agora (TF2 fechado, pasta do jogo escolhida). */
  canApply: boolean;
  applyWanted: boolean;
  onUse: (chosen: string[], apply: boolean) => void;
}

/** O autor mandou várias versões no mesmo arquivo: a pessoa vê o nome de cada uma e escolhe. */
export function VersionPicker({ versions, canApply, applyWanted, onUse }: Props) {
  const { t } = useI18n();
  const [chosen, setChosen] = useState<string[]>([]);
  // quais versões não podem ficar juntas (quase sempre trocam os mesmos arquivos)
  const key = versions.map((v) => v.name).join("|");
  const found = useAsync<{ clashes: Clash[] } | null>(() => call<{ clashes: Clash[] }>("conflicts", { path: null }).catch(() => null), [key]);
  const fight = (a: string, b: string) =>
    (found.data?.clashes ?? []).some((c) => c.level === "block" && c.a.place === "casual" && c.b.place === "casual" && ((c.a.id === a && c.b.id === b) || (c.a.id === b && c.b.id === a)));

  // marcar uma tira as que trocam os mesmos arquivos; as independentes podem ficar juntas
  const toggle = (name: string) => setChosen((now) => (now.includes(name) ? now.filter((n) => n !== name) : [...now.filter((n) => !fight(n, name)), name]));
  const apply = canApply && applyWanted;

  return (
    <div className="state-card" role="group" aria-labelledby="h-versions">
      <b id="h-versions">{t("flow.versions.title", { n: versions.length })}</b>
      <p className="muted small">{t("flow.versions.body")}</p>
      <div className="version-list">
        {versions.map((v) => (
          <label key={v.name} className="version-opt">
            <input type="checkbox" checked={chosen.includes(v.name)} disabled={found.loading} onChange={() => toggle(v.name)} />
            <span>{v.variant ?? v.name}</span>
          </label>
        ))}
      </div>
      <button className="btn primary big" disabled={chosen.length === 0 || found.loading} onClick={() => onUse(chosen, apply)}>
        {apply ? t("flow.versions.apply") : t("flow.versions.use")}
      </button>
    </div>
  );
}
