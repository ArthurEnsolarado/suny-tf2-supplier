import type { ArtSet, Detect, Inspect, Summary } from "../types";
import type { Rails, Slide } from "../featured";
import { fmtInt, fmtMb } from "../format";
import { useI18n } from "../i18n";
import { Badge, ErrorState, Icon, Wave } from "../ui";
import { DetectPanel } from "./DetectPanel";
import { PathForm, PathText } from "./PathForm";
import type { PlayState } from "./PlayButton";

const MIN_DISK_MB = 2048;
const MIN_RAM_MB = 4096;

interface Props {
  inspect: Inspect | undefined;
  error: string | undefined;
  loading: boolean;
  steamError: string | null;
  detecting: boolean;
  onUsePath: (path: string) => void;
  onRefresh: () => void;
  onGoCatalog: () => void;
  onGoSettings: () => void;
  art: ArtSet | null;
  play: PlayState;
  detect: Detect | undefined;
  searching: boolean;
  onSearch: () => void;
  featured: { slides: Slide[]; rails: Rails; loading: boolean };
  slideIndex: number;
  onSlide: (i: number) => void;
  onHeroHover: (hover: boolean) => void;
  onOpenItem: (kind: "Mod" | "Sound", id: number) => void;
}

/** Faixa horizontal de cartões (mods ou sons em alta). Rola com o dedo, o mouse ou o teclado. */
function Rail({ title, kind, items, onOpen }: { title: string; kind: "Mod" | "Sound"; items: Summary[]; onOpen: (kind: "Mod" | "Sound", id: number) => void }) {
  const { t } = useI18n();
  if (items.length === 0) return null;
  return (
    <section className="rail" aria-label={title}>
      <h2>{title}</h2>
      <ul className="rail-track">
        {items.map((r, i) => (
          <li key={r.id} style={{ ["--i" as string]: i }}>
            <button className="rail-card glow" onClick={() => onOpen(kind, r.id)} aria-label={t("browse.open", { name: r.name })}>
              <span className="rail-img">
                {r.thumb ? <img src={r.thumb} alt="" loading="lazy" referrerPolicy="no-referrer" /> : kind === "Sound" ? <Wave seed={r.id} /> : <Icon name="box" />}
              </span>
              <span className="rail-name">{r.name}</span>
              <span className="rail-meta">
                {r.submitter?.name ?? "—"} · ♥ {fmtInt(r.like_count)}
              </span>
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}

export function Home({
  inspect,
  error,
  loading,
  steamError,
  detecting,
  onUsePath,
  onRefresh,
  onGoCatalog,
  onGoSettings,
  art,
  detect,
  searching,
  onSearch,
  featured,
  slideIndex,
  onSlide,
  onHeroHover,
  onOpenItem,
}: Props) {
  const { t, T, tc } = useI18n();

  if (detecting || (loading && !inspect)) {
    return (
      <div className="sheet-head">
        <div>
          <h1>{t("home.detecting")}</h1>
          <p>{t("home.detecting.body")}</p>
        </div>
      </div>
    );
  }

  if (!inspect) {
    return (
      <>
        <div className="sheet-head">
          <div>
            <h1>{t("home.notFound")}</h1>
            <p>
              {steamError ? t("home.steamMissing", { err: tc(steamError) }) : t("home.noTf2")} {t("home.giveFolder")}
            </p>
          </div>
          <Badge tone="warn">{t("home.noInstall")}</Badge>
        </div>
        {error && <ErrorState title={t("home.invalidPath")} message={tc(error)} />}
        <section aria-labelledby="h-find">
          <h2 id="h-find">{t("home.find")}</h2>
          <DetectPanel data={detect} searching={searching} onSearch={onSearch} onUse={onUsePath} />
        </section>
        <section aria-labelledby="h-manual-path">
          <h2 id="h-manual-path">{t("home.manualPath")}</h2>
          <PathForm onUsePath={onUsePath} />
        </section>
      </>
    );
  }

  const { installation: inst, system: sys } = inspect;
  const lowDisk = sys.free_disk_mb !== null && sys.free_disk_mb < MIN_DISK_MB;
  const lowRam = sys.total_ram_mb > 0 && sys.total_ram_mb < MIN_RAM_MB;
  const slides = featured.slides;
  const slide = slides[slideIndex];
  const isGb = slide?.source === "gb";

  return (
    <>
      <section className="hero" onPointerEnter={() => onHeroHover(true)} onPointerLeave={() => onHeroHover(false)} aria-roledescription="carousel" aria-label={t("home.slides")}>
        <div className="hero-copy" key={slide?.id ?? "none"}>
          <span className="eyebrow">{isGb ? t("home.featured") : t("home.yourGame")}</span>
          <h1 data-len={(slide?.title.length ?? 0) > 44 ? "xl" : (slide?.title.length ?? 0) > 26 ? "lg" : "md"}>{slide?.title ?? "Team Fortress 2"}</h1>
          {isGb ? (
            <p className="hero-by">
              {slide.byline && <span>{t("common.by", { who: slide.byline })}</span>}
              {slide.category && <span className="tag">{slide.category}</span>}
              <span>♥ {fmtInt(slide.likes)}</span>
              <span>
                <Icon name="download" /> {fmtInt(slide.views)}
              </span>
            </p>
          ) : (
            <p className="hero-by">
              <span>{t("home.subtitle")}</span>
            </p>
          )}
          <div className="hero-actions">
            {isGb && slide.gbId !== null ? (
              <button className="btn primary" onClick={() => onOpenItem("Mod", slide.gbId!)}>
                {t("home.viewInCatalog")}
              </button>
            ) : (
              <button className="btn primary" onClick={onGoCatalog}>
                {t("home.next.catalog")}
              </button>
            )}
            {isGb && (
              <button className="btn flat" onClick={onGoCatalog}>
                {t("home.next.catalog")}
              </button>
            )}
          </div>
        </div>
        {slides.length > 1 && (
          <div className="dots" role="tablist" aria-label={t("home.slides")}>
            {slides.map((s, i) => (
              <button key={s.id} role="tab" aria-selected={i === slideIndex} aria-label={s.title} className={`dot ${i === slideIndex ? "on" : ""}`} onClick={() => onSlide(i)}>
                <i />
              </button>
            ))}
          </div>
        )}
      </section>

      <section className="tiles" aria-label={t("home.status")}>
        <div className="tile glow">
          <span className="tile-label">{t("home.check.game")}</span>
          <span className="tile-value">{sys.game_running ? t("status.open") : t("status.closed")}</span>
          <span className="tile-sub">{sys.game_running ? t("home.check.game.open") : t("home.check.game.closed")}</span>
        </div>
        <div className="tile glow">
          <span className="tile-label">Casual Preloader</span>
          <span className="tile-value">{inspect.preloader_applied ? t("home.pre.applied") : inspect.preloader_leftover ? t("home.pre.leftover") : t("home.pre.clean")}</span>
          <span className="tile-sub">
            {inspect.preloader_applied && t("home.check.pre.applied")}
            {inspect.preloader_leftover && (inspect.preloader.gameinfo_modified ? t("home.check.pre.leftover1") : t("home.check.pre.leftover2"))}
            {!inspect.preloader_applied && !inspect.preloader_leftover && t("home.check.pre.clean")}
          </span>
        </div>
        <div className="tile glow">
          <span className="tile-label">{t("home.check.disk")}</span>
          <span className="tile-value">{sys.free_disk_mb === null ? "—" : fmtMb(sys.free_disk_mb)}</span>
          <span className="tile-sub">{lowDisk ? t("home.check.disk.low") : t("home.check.disk.free", { size: sys.free_disk_mb === null ? "—" : fmtMb(sys.free_disk_mb) })}</span>
        </div>
        <div className="tile glow">
          <span className="tile-label">{t("home.check.ram")}</span>
          <span className="tile-value">{fmtMb(sys.total_ram_mb)}</span>
          <span className="tile-sub">{lowRam ? t("home.check.ram.low") : t("home.check.ram.total", { size: fmtMb(sys.total_ram_mb) })}</span>
        </div>
      </section>

      {inspect.preloader_applied && (
        <p className="note">
          <T k="home.firstBoot" />
        </p>
      )}

      <Rail title={t("home.rail.mods")} kind="Mod" items={featured.rails.mods} onOpen={onOpenItem} />
      <Rail title={t("home.rail.sounds")} kind="Sound" items={featured.rails.sounds} onOpen={onOpenItem} />
      {featured.loading && (
        <p className="muted small" role="status">
          {t("home.loadingFeatured")}
        </p>
      )}

      <section aria-labelledby="h-install" className="install-panel">
        <h2 id="h-install">{t("home.install")}</h2>
        <div className={art?.portrait ? "install-card" : undefined}>
          {art?.portrait && <img className="poster" src={art.portrait} alt={t("home.cover")} />}
          <dl className="form">
            <dt>{t("home.path")}</dt>
            <dd>
              <PathText path={inst.root} />
            </dd>
            <dt>{t("home.build")}</dt>
            <dd>{inst.build_id ?? <span className="muted">{t("home.notInformed")}</span>}</dd>
            <dt>{t("home.detection")}</dt>
            <dd>{inst.source === "steam" ? "Steam" : inst.source === "scan" ? t("home.scan") : t("home.manual")}</dd>
            <dt>{t("home.system")}</dt>
            <dd>{sys.os}</dd>
          </dl>
        </div>
        <p>
          <button className="btn flat" onClick={onRefresh}>
            <Icon name="refresh" />
            {t("common.refresh")}
          </button>{" "}
          <button className="link" onClick={onGoSettings}>
            {t("home.changeInstall")}
          </button>
        </p>
        {error && <ErrorState title={t("home.invalidPath")} message={tc(error)} />}
      </section>
    </>
  );
}
