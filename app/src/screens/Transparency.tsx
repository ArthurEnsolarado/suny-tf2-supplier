import { useI18n } from "../i18n";

type Key =
  | `privacy.ai.${"1" | "2" | "3"}`
  | `privacy.net.${"1" | "2" | "3"}`
  | `privacy.read.${"1" | "2" | "3" | "4" | "5"}`
  | `privacy.write.${"1" | "2" | "3"}`;

/** Como o app foi feito e o que ele faz com os dados. O mesmo texto está no readme: mudou o acesso, muda os dois. */
export function Transparency() {
  const { t, T } = useI18n();
  const list = (keys: Key[]) => (
    <ul>
      {keys.map((k) => (
        <li key={k}>
          <T k={k} />
        </li>
      ))}
    </ul>
  );

  return (
    <details className="file-details transparency">
      <summary>{t("privacy.title")}</summary>
      <h3>{t("privacy.ai.title")}</h3>
      {list(["privacy.ai.1", "privacy.ai.2", "privacy.ai.3"])}
      <h3>{t("privacy.data.title")}</h3>
      <p>
        <T k="privacy.data.intro" />
      </p>
      <p>
        <b>{t("privacy.net.title")}</b>
      </p>
      {list(["privacy.net.1", "privacy.net.2", "privacy.net.3"])}
      <p>{t("privacy.net.note")}</p>
      <p>
        <b>{t("privacy.read.title")}</b>
      </p>
      {list(["privacy.read.1", "privacy.read.2", "privacy.read.3", "privacy.read.4", "privacy.read.5"])}
      <p>
        <b>{t("privacy.write.title")}</b>
      </p>
      {list(["privacy.write.1", "privacy.write.2", "privacy.write.3"])}
      <p>{t("privacy.preloader")}</p>
      <h3>{t("privacy.pledge.title")}</h3>
      <p>{t("privacy.pledge")}</p>
    </details>
  );
}
