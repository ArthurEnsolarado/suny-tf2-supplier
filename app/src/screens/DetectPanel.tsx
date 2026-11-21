import { useI18n } from "../i18n";
import type { Detect } from "../types";
import { Badge, Icon } from "../ui";
import { PathText } from "./PathForm";

interface Props {
  data: Detect | undefined;
  searching: boolean;
  onSearch: () => void;
  /** Usar uma das instalações achadas. */
  onUse: (root: string) => void;
}

/** O que a procura automática fez: o que achou, o que o Steam disse e as pastas olhadas. Nunca fica "não achei" sem explicação. */
export function DetectPanel({ data, searching, onSearch, onUse }: Props) {
  const { t, tc } = useI18n();
  const tried = data?.tried ?? [];
  const looked = tried.filter((p) => p.found).length;
  return (
    <div className="detect">
      <button className="btn flat" onClick={onSearch} disabled={searching}>
        <Icon name={searching ? "spinner" : "refresh"} />
        {searching ? t("detect.searching") : t("detect.search")}
      </button>

      {data && (
        <div className="detect-result" role="status" aria-live="polite">
          {data.installations.length > 0 ? (
            <ul className="detect-list">
              {data.installations.map((i) => (
                <li key={i.root}>
                  <div>
                    <Badge tone="ok" small>
                      {t("detect.found")}
                    </Badge>{" "}
                    <span className="muted">
                      {i.source === "steam" ? t("detect.source.steam") : t("detect.source.scan")}
                      {i.build_id ? ` · ${t("path.build", { build: i.build_id })}` : ""}
                    </span>
                    <div className="path-root">
                      <PathText path={i.root} />
                    </div>
                  </div>
                  <button className="btn primary mini" onClick={() => onUse(i.root)}>
                    {t("detect.use")}
                  </button>
                </li>
              ))}
            </ul>
          ) : (
            <p>
              <Badge tone="warn" small>
                {t("detect.none")}
              </Badge>
            </p>
          )}
          <details>
            <summary>{t("detect.details")}</summary>
            <ul className="detect-detail">
              <li>
                {data.steam_dir ? (
                  <>
                    {t("detect.steamAt")} <code>{data.steam_dir}</code>
                  </>
                ) : (
                  <>
                    {t("detect.steamNone")}
                    {data.steam_error ? ` (${tc(data.steam_error)})` : ""}
                  </>
                )}
              </li>
              <li>{t("detect.tried", { n: tried.length, found: looked })}</li>
              {tried.slice(0, 40).map((p) => (
                <li key={p.path} className={p.found ? "hit" : "miss"}>
                  <span aria-hidden="true">{p.found ? "✓" : "·"}</span> <code>{p.path}</code>
                </li>
              ))}
            </ul>
          </details>
        </div>
      )}
    </div>
  );
}
