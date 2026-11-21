import type { ArtSet, Inspect } from "../types";
import { fmtMb } from "../format";
import { useI18n } from "../i18n";
import type { Detect } from "../types";
import { Badge, Box, ErrorState, Icon } from "../ui";
import { DetectPanel } from "./DetectPanel";
import { PathForm, PathText } from "./PathForm";
import { PlayButton, type PlayState } from "./PlayButton";

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
}

export function Home({ inspect, error, loading, steamError, detecting, onUsePath, onRefresh, onGoCatalog, onGoSettings, art, play, detect, searching, onSearch }: Props) {
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

  return (
    <>
      <div className={`sheet-head ${art?.hero ? "with-hero" : ""}`} style={art?.hero ? { backgroundImage: `url(${art.hero})` } : undefined}>
        <div>
          <h1>{t("home.title")}</h1>
          <p>{t("home.subtitle")}</p>
        </div>
        <div className="head-side">
          {art?.logo && <img className="hero-logo" src={art.logo} alt="Team Fortress 2" />}
          <PlayButton play={play} />
        </div>
      </div>

      <div className="cols">
        <div>
          <section aria-labelledby="h-install">
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
        </div>

        <div>
          <section aria-labelledby="h-next">
            <h2 id="h-next">{t("home.next")}</h2>
            {sys.game_running ? (
              <p>{t("home.next.open")}</p>
            ) : (
              <>
                <p>{t("home.next.ready")}</p>
                <button className="btn primary" onClick={onGoCatalog}>
                  {t("home.next.catalog")}
                </button>
              </>
            )}
          </section>

          <section aria-labelledby="h-status">
            <h2 id="h-status">{t("home.status")}</h2>
            <ul className="checks">
              <li>
                <Box ok={!sys.game_running} />
                <span className="check-name">{t("home.check.game")}</span>
                <span>{sys.game_running ? t("home.check.game.open") : t("home.check.game.closed")}</span>
                {sys.game_running ? (
                  <Badge tone="warn" small>
                    {t("status.open")}
                  </Badge>
                ) : (
                  <Badge tone="ok" small>
                    {t("status.closed")}
                  </Badge>
                )}
              </li>
              <li>
                <Box ok={!lowDisk} />
                <span className="check-name">{t("home.check.disk")}</span>
                <span>
                  {sys.free_disk_mb === null ? (
                    <span className="muted">{t("home.check.disk.unknown")}</span>
                  ) : (
                    <>
                      {t("home.check.disk.free", { size: fmtMb(sys.free_disk_mb) })}
                      {lowDisk && ` ${t("home.check.disk.low")}`}
                    </>
                  )}
                </span>
                <span />
              </li>
              <li>
                <Box ok={!lowRam} />
                <span className="check-name">{t("home.check.ram")}</span>
                <span>
                  {t("home.check.ram.total", { size: fmtMb(sys.total_ram_mb) })}
                  {lowRam && ` ${t("home.check.ram.low")}`}
                </span>
                <span />
              </li>
              <li>
                <Box ok={!inspect.preloader_leftover} />
                <span className="check-name">Casual Preloader</span>
                <span>
                  {inspect.preloader_applied && t("home.check.pre.applied")}
                  {inspect.preloader_leftover && (inspect.preloader.gameinfo_modified ? t("home.check.pre.leftover1") : t("home.check.pre.leftover2"))}
                  {!inspect.preloader_applied && !inspect.preloader_leftover && t("home.check.pre.clean")}
                </span>
                {inspect.preloader_applied ? (
                  <Badge tone="ok" small>
                    {t("home.pre.applied")}
                  </Badge>
                ) : inspect.preloader_leftover ? (
                  <Badge tone="warn" small>
                    {t("home.pre.leftover")}
                  </Badge>
                ) : (
                  <Badge tone="neutral" small>
                    {t("home.pre.clean")}
                  </Badge>
                )}
              </li>
            </ul>

            {inspect.preloader_applied && (
              <p className="note">
                <T k="home.firstBoot" />
              </p>
            )}
          </section>
        </div>
      </div>
    </>
  );
}
