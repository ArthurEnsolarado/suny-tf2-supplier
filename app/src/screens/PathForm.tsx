import { useState } from "react";
import { useI18n } from "../i18n";
import { Icon } from "../ui";

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

export function PathForm({ onUsePath }: { onUsePath: (p: string) => void }) {
  const { t } = useI18n();
  const [value, setValue] = useState("");
  return (
    <form
      className="row"
      onSubmit={(e) => {
        e.preventDefault();
        if (value.trim()) onUsePath(value.trim().replace(/^"|"$/g, ""));
      }}
    >
      <label className="field grow">
        <span>{t("path.label")}</span>
        <input className="blank" value={value} onChange={(e) => setValue(e.target.value)} placeholder="D:\SteamLibrary\steamapps\common\Team Fortress 2" spellCheck={false} />
      </label>
      <button className="btn" type="submit" disabled={!value.trim()}>
        <Icon name="folder" />
        {t("path.apply")}
      </button>
    </form>
  );
}
