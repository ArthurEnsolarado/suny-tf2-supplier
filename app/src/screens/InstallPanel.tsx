import { useState } from "react";
import { useI18n } from "../i18n";
import type { FileInfo, Kind } from "../types";
import { CasualAddBox } from "./CasualAddBox";
import { InstallBox, SUPPORTED } from "./InstallBox";

export interface CasualLink {
  ready: boolean;
  unknown: boolean;
  onGo: () => void;
  onAdded: () => void;
}

interface Props {
  installRoot: string | undefined;
  kind: Kind;
  modId: number;
  file: FileInfo;
  gameRunning: boolean;
  casual: CasualLink;
  /** Mapa (.bsp): o Casual Preloader não carrega mapas, então só faz sentido em custom. */
  isMap: boolean;
  onChanged: () => void;
}

/** Um só lugar para instalar: o usuário diz ONDE vai jogar e o app escolhe o caminho certo. */
export function InstallPanel({ installRoot, kind, modId, file, gameRunning, casual, isMap, onChanged }: Props) {
  const { t, T } = useI18n();
  const exe = !!file.analysis_warnings && "contains_exe" in file.analysis_warnings;
  const ext = file.file_name.toLowerCase().split(".").pop() ?? "";
  const supported = !exe && SUPPORTED.includes(ext);
  const [dest, setDest] = useState<"casual" | "custom">(!isMap && casual.ready ? "casual" : "custom");

  // Formato ou arquivo que o app não instala: a caixa de custom explica o motivo.
  if (!supported) {
    return <InstallBox installRoot={installRoot} kind={kind} modId={modId} file={file} gameRunning={gameRunning} onChanged={onChanged} />;
  }

  const options: { id: "casual" | "custom"; title: string; hint: string }[] = [
    { id: "casual", title: t("install.dest.casual"), hint: t("install.dest.casual.hint") },
    { id: "custom", title: t("install.dest.custom"), hint: t("install.dest.custom.hint") },
  ];

  return (
    <div className="dest">
      <p className="dest-label">{t("install.dest.label")}</p>
      {isMap ? (
        <p className="note">
          <T k="install.map.note" />
        </p>
      ) : (
        <div className="dest-options" role="radiogroup" aria-label={t("install.dest.label")}>
          {options.map((o) => (
            <button key={o.id} type="button" role="radio" aria-checked={dest === o.id} className="dest-opt" onClick={() => setDest(o.id)}>
              <span className="dest-title">{o.title}</span>
              <span className="dest-hint">{o.hint}</span>
            </button>
          ))}
        </div>
      )}
      {!isMap && (
        <details className="dest-diff">
          <summary>{t("install.diff.summary")}</summary>
          <ul>
            <li>
              <T k="install.diff.1" />
            </li>
            <li>
              <T k="install.diff.2" />
            </li>
            <li>
              <T k="install.diff.3" />
            </li>
            <li>
              <T k="install.diff.4" />
            </li>
          </ul>
        </details>
      )}
      {dest === "casual" && !isMap ? (
        <CasualAddBox key={`c${file.id}`} installRoot={installRoot} kind={kind} modId={modId} file={file} ready={casual.ready} unknown={casual.unknown} onGoCasual={casual.onGo} onAdded={casual.onAdded} />
      ) : (
        <InstallBox key={`i${file.id}`} installRoot={installRoot} kind={kind} modId={modId} file={file} gameRunning={gameRunning} onChanged={onChanged} />
      )}
    </div>
  );
}
