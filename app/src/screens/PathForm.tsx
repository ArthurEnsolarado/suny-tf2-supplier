import { useEffect, useRef, useState } from "react";
import { call } from "../api";
import { useI18n } from "../i18n";
import { Badge, Icon } from "../ui";

/** Caminho do Windows que quebra de linha só depois de uma barra, nunca no meio de uma palavra. */
export function PathText({ path }: { path: string }) {
  const parts = path.split("\\");
  return (
    <>
      {parts.map((part, i) => (
        <span key={i}>
          {i > 0 && "\\"}
          {i > 0 && <wbr />}
          {part}
        </span>
      ))}
    </>
  );
}

interface Validation {
  ok: boolean;
  empty?: boolean;
  root?: string;
  build_id?: number | null;
  has_custom?: boolean;
  error?: string;
}

/**
 * Campo do caminho do TF2 com conferência ao vivo: enquanto você digita (ou cola, ou escolhe a pasta), o app diz
 * se a pasta serve, qual é o build e o que está errado quando não serve. Só aplica uma pasta que passou.
 */
export function PathForm({ onUsePath }: { onUsePath: (p: string) => void }) {
  const { t, tc } = useI18n();
  const [value, setValue] = useState("");
  const [state, setState] = useState<"empty" | "checking" | Validation>("empty");
  const [pickError, setPickError] = useState<string | undefined>();
  const latest = useRef(0);

  useEffect(() => {
    const id = ++latest.current;
    if (!value.trim()) {
      setState("empty");
      return;
    }
    setState("checking");
    const timer = setTimeout(() => {
      call<Validation>("validate_path", { path: value }).then(
        (v) => id === latest.current && setState(v),
        (e: unknown) => id === latest.current && setState({ ok: false, error: e instanceof Error ? e.message : String(e) }),
      );
    }, 350);
    return () => clearTimeout(timer);
  }, [value]);

  const browse = () => {
    setPickError(undefined);
    call<{ path: string | null }>("pick_folder").then(
      (r) => r.path && setValue(r.path),
      (e: unknown) => setPickError(e instanceof Error ? e.message : String(e)),
    );
  };

  const ok = typeof state === "object" && state.ok ? state : null;
  return (
    <form
      className="pathform"
      onSubmit={(e) => {
        e.preventDefault();
        if (ok?.root) onUsePath(ok.root);
      }}
    >
      <div className="row">
        <label className="field grow">
          <span>{t("path.label")}</span>
          <input className="blank" value={value} onChange={(e) => setValue(e.target.value)} placeholder="C:\Program Files (x86)\Steam\steamapps\common\Team Fortress 2" spellCheck={false} aria-describedby="path-status" />
        </label>
        <button className="btn flat" type="button" onClick={browse}>
          <Icon name="folder" />
          {t("path.browse")}
        </button>
        <button className="btn primary" type="submit" disabled={!ok}>
          <Icon name="check" />
          {t("path.apply")}
        </button>
      </div>
      <div id="path-status" className="path-status" role="status" aria-live="polite">
        {state === "empty" && <span className="muted">{t("path.hint")}</span>}
        {state === "checking" && (
          <span className="muted">
            <Icon name="spinner" /> {t("path.checking")}
          </span>
        )}
        {ok && (
          <>
            <Badge tone="ok" small>
              {t("path.ok")}
            </Badge>{" "}
            <span className="muted">
              {ok.build_id ? t("path.build", { build: ok.build_id }) : t("path.noBuild")}
              {ok.has_custom === false ? ` · ${t("path.noCustom")}` : ""}
            </span>
            {ok.root && <div className="muted small path-root"><PathText path={ok.root} /></div>}
          </>
        )}
        {typeof state === "object" && !state.ok && !state.empty && (
          <>
            <Badge tone="block" small>
              {t("path.bad")}
            </Badge>{" "}
            <span>{tc(state.error ?? "")}</span>
            <div className="muted small">{t("path.badHint")}</div>
          </>
        )}
        {pickError && <div className="muted small">{tc(pickError)}</div>}
      </div>
    </form>
  );
}
