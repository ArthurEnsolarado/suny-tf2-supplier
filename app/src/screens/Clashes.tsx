import { useI18n } from "../i18n";
import type { Clash, ClashSide } from "../types";
import { Badge } from "../ui";

/** O lado está em jogo? Tudo de custom está; do casual, só o que está marcado. */
export const sideActive = (s: ClashSide, checked: ReadonlySet<string>) => s.place === "custom" || (s.place === "casual" && checked.has(s.id));

/** As brigas que valem para o casual com estas marcas: os dois lados em jogo e ao menos um do casual. */
export function casualClashes(clashes: Clash[], checked: ReadonlySet<string>) {
  return clashes.filter((c) => (c.a.place === "casual" || c.b.place === "casual") && sideActive(c.a, checked) && sideActive(c.b, checked));
}

export const clashKey = (c: Clash) => `${c.a.place}:${c.a.id}|${c.b.place}:${c.b.id}`;

/** Lista de brigas: quem com quem, e por quê (arquivos iguais quando barra; o objeto em comum quando só avisa). */
export function ClashList({ clashes }: { clashes: Clash[] }) {
  const { t } = useI18n();
  const who = (s: ClashSide) => (
    <>
      <b>{s.name}</b>
      {s.place === "custom" && <span className="muted"> ({t("clash.inCustom")})</span>}
    </>
  );
  return (
    <ul className="conflicts">
      {clashes.map((c) => (
        <li key={clashKey(c)}>
          <div>
            <Badge tone={c.level === "block" ? "block" : "warn"} small>
              {c.level === "block" ? t("files", { n: c.files }) : t("clash.warn.badge")}
            </Badge>{" "}
            {who(c.a)} <span className="muted">{t("clash.and")}</span> {who(c.b)}
          </div>
          {c.level === "block" ? (
            <div className="muted small">
              <code>{c.examples.join("  ·  ")}</code>
            </div>
          ) : (
            <div className="muted small">{t("clash.same", { what: c.objects.map((o) => t(`clash.object.${o.kind}` as "clash.object.hud", { name: o.name })).join(", ") })}</div>
          )}
        </li>
      ))}
    </ul>
  );
}
