import { useState } from "react";
import { call } from "../api";
import { inTauri } from "../api";
import { fmtBytes } from "../format";
import { LANGS, useI18n } from "../i18n";
import type { AppInfo, Detect, Inspect, UpdateInfo } from "../types";
import { Badge, ErrorState, useAsync } from "../ui";
import { ZOOM_MODES } from "../useUiZoom";
import { DetectPanel } from "./DetectPanel";
import { PathForm, PathText } from "./PathForm";

interface Props {
  inspect: Inspect | undefined;
  pathError: string | undefined;
  onUsePath: (p: string) => void;
  detect: Detect | undefined;
  detecting: boolean;
  onDetect: () => void;
  quickprecache: boolean;
  onQuickprecache: (v: boolean) => void;
  zoomMode: string;
  onZoomMode: (m: string) => void;
  update: { info: UpdateInfo | null; checking: boolean; error: string | undefined; check: () => void };
  autoUpdate: boolean;
  onAutoUpdate: (v: boolean) => void;
  onOpenUpdate: () => void;
}

export function Settings({ inspect, pathError, onUsePath, detect, detecting, onDetect, quickprecache, onQuickprecache, zoomMode, onZoomMode, update, autoUpdate, onAutoUpdate, onOpenUpdate }: Props) {
  const { t, T, tc, lang, setLang } = useI18n();
  const info = useAsync<AppInfo>(() => call<AppInfo>("app_info"), []);
  const [cleared, setCleared] = useState<string | undefined>();
  const [error, setError] = useState<string | undefined>();

  const clear = () => {
    setError(undefined);
    call<{ removed: number; freed_bytes: number }>("clear_cache").then(
      (r) => {
        setCleared(t("settings.cache.cleared", { n: r.removed, size: fmtBytes(r.freed_bytes) }));
        info.reload();
      },
      (e: unknown) => setError(e instanceof Error ? e.message : String(e)),
    );
  };
  const open = (which: "casual" | "cache") =>
    call("open_folder", { which }).catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)));

  const extractor = (id: string) => (id === "7zip" ? "7-Zip" : id === "winrar" ? "WinRAR" : t("settings.formats.tar"));
  const names = (list: string[]) => (list.length ? list.map(extractor).join(", ") : null);

  return (
    <>
      <div className="sheet-head">
        <div>
          <h1>{t("tab.settings")}</h1>
          <p>{t("settings.subtitle")}</p>
        </div>
      </div>

      {error && <ErrorState title={t("casual.error.title")} message={tc(error)} />}

      <div className="cols">
        <div>
          <section aria-labelledby="h-lang">
            <h2 id="h-lang">{t("settings.language")}</h2>
            <div className="toggle" role="group" aria-label={t("settings.language")}>
              {LANGS.map((l) => (
                <button key={l.id} type="button" aria-pressed={lang === l.id} onClick={() => setLang(l.id)}>
                  {l.label}
                </button>
              ))}
            </div>
            <p className="muted small">{t("settings.language.note")}</p>
          </section>

          <section aria-labelledby="h-size">
            <h2 id="h-size">{t("settings.size")}</h2>
            <label className="field">
              <span>{t("settings.size.label")}</span>
              <select className="blank" value={zoomMode} onChange={(e) => onZoomMode(e.target.value)} disabled={!inTauri}>
                {ZOOM_MODES.map((m) => (
                  <option key={m} value={m}>
                    {m === "auto" ? t("settings.size.auto") : `${Math.round(Number(m) * 100)}%`}
                  </option>
                ))}
              </select>
            </label>
            <p className="muted small">{inTauri ? t("settings.size.note") : t("settings.size.devOnly")}</p>
          </section>

          <section aria-labelledby="h-tf2">
            <h2 id="h-tf2">{t("settings.tf2")}</h2>
            {inspect ? (
              <dl className="form">
                <dt>{t("home.path")}</dt>
                <dd>
                  <PathText path={inspect.installation.root} />
                </dd>
                <dt>{t("home.build")}</dt>
                <dd>{inspect.installation.build_id ?? <span className="muted">{t("home.notInformed")}</span>}</dd>
                <dt>{t("home.detection")}</dt>
                <dd>{inspect.installation.source === "steam" ? "Steam" : t("home.manual")}</dd>
              </dl>
            ) : (
              <p className="muted">{t("settings.tf2.none")}</p>
            )}
            <PathForm onUsePath={onUsePath} />
            {pathError && <ErrorState title={t("home.invalidPath")} message={tc(pathError)} />}
            <h3>{t("settings.tf2.find")}</h3>
            <DetectPanel data={detect} searching={detecting} onSearch={onDetect} onUse={onUsePath} />
          </section>

          <section aria-labelledby="h-casual">
            <h2 id="h-casual">{t("settings.casual")}</h2>
            <label className="use opt">
              <input type="checkbox" className="pick" checked={quickprecache} onChange={(e) => onQuickprecache(e.target.checked)} />
              <span>
                <b>{t("settings.quickprecache")}</b>
                <br />
                <span className="muted">{t("settings.quickprecache.note")}</span>
              </span>
            </label>
          </section>
        </div>

        <div>
          <section aria-labelledby="h-formats">
            <h2 id="h-formats">{t("settings.formats")}</h2>
            <ul className="checks formats">
              <li>
                <span className="check-name">.zip</span>
                <span>{t("settings.formats.builtin")}</span>
                <Badge tone="ok" small>
                  {t("settings.formats.ok")}
                </Badge>
              </li>
              {info.data &&
                (["rar", "7z"] as const).map((f) => {
                  const found = names(info.data!.extractors[f]);
                  return (
                    <li key={f}>
                      <span className="check-name">.{f}</span>
                      <span>{found ? t("settings.formats.via", { name: found }) : t("settings.formats.missing")}</span>
                      {found ? (
                        <Badge tone="ok" small>
                          {t("settings.formats.ok")}
                        </Badge>
                      ) : (
                        <Badge tone="warn" small>
                          {t("settings.formats.none")}
                        </Badge>
                      )}
                    </li>
                  );
                })}
            </ul>
            <p className="muted small">
              <T k="settings.formats.note" />
            </p>
          </section>

          <section aria-labelledby="h-storage">
            <h2 id="h-storage">{t("settings.storage")}</h2>
            {info.data && (
              <dl className="form">
                <dt>{t("settings.storage.cache")}</dt>
                <dd>{fmtBytes(info.data.cache_bytes)}</dd>
                <dt>{t("settings.storage.casual")}</dt>
                <dd>{fmtBytes(info.data.casual_bytes)}</dd>
              </dl>
            )}
            <div className="row">
              <button className="btn flat" onClick={clear} disabled={!info.data?.cache_bytes}>
                {t("settings.cache.clear")}
              </button>
              <button className="btn flat" onClick={() => open("cache")}>
                {t("settings.cache.open")}
              </button>
              <button className="btn flat" onClick={() => open("casual")}>
                {t("settings.casual.open")}
              </button>
            </div>
            {cleared && (
              <p className="muted small" role="status">
                {cleared}
              </p>
            )}
            <p className="muted small">{t("settings.cache.note")}</p>
          </section>

          <section aria-labelledby="h-updates">
            <h2 id="h-updates">{t("settings.updates")}</h2>
            <p>
              {t("settings.updates.current")} <b>v{info.data?.version ?? update.info?.current ?? "…"}</b>
              {update.info && !update.info.available && update.info.latest && (
                <>
                  {" "}
                  <Badge tone="ok" small>
                    {t("settings.updates.upToDate")}
                  </Badge>
                </>
              )}
              {update.info?.available && (
                <>
                  {" "}
                  <Badge tone="info" small>
                    {t("settings.updates.available", { version: update.info.latest ?? "" })}
                  </Badge>
                </>
              )}
              {update.info && !update.info.latest && <span className="muted"> {t("settings.updates.none")}</span>}
            </p>
            {update.error && <p className="muted small" role="alert">{tc(update.error)}</p>}
            <div className="row">
              <button className="btn flat" onClick={update.check} disabled={update.checking}>
                {update.checking ? t("settings.updates.checking") : t("settings.updates.check")}
              </button>
              {update.info?.available && (
                <button className="btn primary" onClick={onOpenUpdate}>
                  {t("update.see")}
                </button>
              )}
            </div>
            <label className="use opt">
              <input type="checkbox" className="pick" checked={autoUpdate} onChange={(e) => onAutoUpdate(e.target.checked)} />
              <span>
                <b>{t("settings.updates.auto")}</b>
                <br />
                <span className="muted">{t("settings.updates.auto.note")}</span>
              </span>
            </label>
          </section>

          <section aria-labelledby="h-about">
            <h2 id="h-about">{t("settings.about")}</h2>
            <p>
              <b>suny&rsquo;s tf2 supplier</b> {info.data ? `v${info.data.version}` : ""}
            </p>
            <p className="muted small">
              <T k="settings.about.body" />
            </p>
          </section>
        </div>
      </div>
    </>
  );
}
