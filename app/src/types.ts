// Espelho dos tipos do núcleo Rust (crates/suny-core). Mantenha em sincronia.

export type Kind = "Mod" | "Sound";
export type Sort = "MostDownloaded" | "MostLiked" | "Newest";

export interface Installation {
  root: string;
  tf_dir: string;
  build_id: number | null;
  source: "steam" | "manual" | "scan";
}

export interface Detect {
  installations: Installation[];
  steam_error: string | null;
  steam_dir: string | null;
  /** Pastas comuns olhadas além do Steam, e se o jogo estava lá. */
  tried: { path: string; found: boolean }[];
}

export interface Snapshot {
  os: string;
  total_ram_mb: number;
  free_disk_mb: number | null;
  game_running: boolean;
  warnings: string[];
}

export type Role = "preloader" | "preloader_backup" | "workshop" | "mod";

/** Dados de um mod instalado por este app (vêm do manifesto). */
export interface ManagedInfo {
  slug: string;
  gb_id: number;
  kind: string;
  name: string;
  version: string | null;
  submitter: string | null;
  profile_url: string;
  installed_at: number;
}

export interface CustomEntry {
  name: string;
  role: Role;
  is_folder: boolean;
  size_bytes: number;
  file_count: number;
  managed: ManagedInfo | null;
}

export interface InstallOutcome {
  manifest: { slug: string; items: string[]; files: { path: string; size: number }[]; skipped: string[] };
  report: { compat: string; reasons: string[]; flags: string[] };
}

export interface Inspect {
  installation: Installation;
  preloader: { gameinfo_modified: boolean; vpk_present: boolean };
  preloader_applied: boolean;
  preloader_leftover: boolean;
  custom: CustomEntry[];
  system: Snapshot;
}

export interface Page<T> {
  total: number;
  is_complete: boolean;
  records: T[];
}

export interface Submitter {
  id: number;
  name: string;
  profile_url: string;
}

export interface Category {
  id: number;
  name: string;
  item_count: number;
}

/** Arte do jogo lida da instalação do Steam do usuário (data URLs). */
export interface ArtSet {
  hero: string | null;
  hero_blur: string | null;
  logo: string | null;
  header: string | null;
  portrait: string | null;
  custom: boolean;
}

export interface Summary {
  id: number;
  thumb: string | null;
  name: string;
  profile_url: string;
  submitter: Submitter | null;
  root_category: Category | null;
  like_count: number;
  view_count: number;
  /** `show` ou `hide` (conteúdo sensível). */
  initial_visibility?: string | null;
  has_content_ratings?: boolean;
}

export interface FileInfo {
  id: number;
  file_name: string;
  size: number;
  download_url: string;
  md5: string | null;
  av_result: string | null;
  analysis_warnings: Record<string, string[]> | null;
}

export interface Details {
  id: number;
  name: string;
  profile_url: string;
  submitter: Submitter | null;
  category: Category | null;
  files: FileInfo[];
  license: string | null;
  version: string | null;
  is_obsolete: boolean;
  /** Até 12 imagens: a grande (visualizador) e a miniatura (tira de baixo). */
  images: GalleryImage[];
  description: string | null;
  like_count: number;
  view_count: number;
  download_count: number;
  date_added: number | null;
  date_modified: number | null;
  tags: string[];
}

export interface GalleryImage {
  url: string;
  thumb: string;
}

/** Ícones lidos do próprio jogo (data URLs PNG). Chaves: all, scout, soldier, pyro, demoman, heavy, engineer, medic, sniper, spy. */
export interface GameIcons {
  icons: Record<string, string>;
}

export interface AudioItem {
  entry: string;
  name: string;
  size: number;
  label: string;
  class: string | null;
  /** true = substitui um som do jogo; false = som novo; null = não deu para comparar. */
  vanilla: boolean | null;
}

export interface ChangeGroup {
  label: string;
  class: string | null;
  kind: string;
  total: number;
  replaces: number | null;
  added: number | null;
  examples: string[];
}

/** O que um item de `custom` muda (lido do próprio conteúdo) e com quem ele disputa arquivos. */
export interface ItemContents {
  name: string;
  files: number;
  truncated: boolean;
  groups: ChangeGroup[];
  classes: string[];
  wins: number;
  loses: number;
  rivals: string[];
}

export interface ConflictSet {
  items: string[];
  winner: string;
  count: number;
  examples: string[];
}

export interface CustomReport {
  items: ItemContents[];
  conflicts: ConflictSet[];
  conflict_files: number;
  unreadable: string[];
}

export interface Analysis {
  file_count: number;
  game_files: number;
  report: { compat: string; reasons: string[]; flags: string[]; blocked: string[]; unmatched: string[] };
  groups: ChangeGroup[];
  classes: string[];
  audio: AudioItem[];
  audio_total: number;
  vanilla_known: boolean;
}

/* ---------- Casual Preloader dentro do app ---------- */

export interface CasualSource {
  kind: string;
  gb_id: number;
  file_id: number;
  file_name: string;
  name: string;
  version: string | null;
  submitter: string | null;
  profile_url: string;
  md5: string | null;
  added_at: number;
}

export interface CasualAddon {
  name: string;
  kind: string;
  file_count: number;
  size_bytes: number;
  enabled: boolean;
  source: CasualSource | null;
  has_models: boolean;
  also_in_custom: boolean;
}

export interface CasualJob {
  id: number;
  kind: "setup" | "add" | "apply" | "revert" | string;
  title: string;
  state: "running" | "done" | "failed";
  pct: number;
  msg: string;
  log: string[];
  error: string | null;
  result: unknown;
  elapsed_s: number;
}

export interface CasualStatus {
  runtime: "missing" | "needs_deps" | "ready";
  version: string | null;
  pinned_version: string;
  root: string;
  addons: CasualAddon[];
  /** O que o app aplicou nesta instalação do jogo (pode estar desatualizado se o jogo foi restaurado por fora). */
  applied_here: { addons: string[]; at: number; preloader_version: string } | null;
  /** O jogo está com o preloader de verdade (gameinfo e VPK). */
  game_applied: boolean | null;
  launch_options: string | null;
  launch_options_ok: boolean | null;
  job: CasualJob | null;
}

export interface AppInfo {
  version: string;
  casual_dir: string;
  cache_dir: string;
  cache_bytes: number;
  casual_bytes: number;
  extractors: { rar: string[]; "7z": string[] };  // ids: 7zip, winrar, tar
}

export interface ReleaseNote {
  version: string;
  name: string;
  notes: string;
  published: string | null;
  url: string;
}

export interface UpdateInfo {
  current: string;
  latest: string | null;
  available: boolean;
  releases: ReleaseNote[];
  installer: { name: string; size: number; url: string; sha256: string | null } | null;
}
