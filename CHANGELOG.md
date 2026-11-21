# Changelog

Every stable release shows up inside the app (Settings → Updates) with the notes from here.
Format: newest first. Versions follow `major.minor.patch`.

## 0.7.1

### Fixed
- **Downloads failing on slow or filtered networks** (university wifi, VPN / WARP): the app cut every request after 20 seconds, so any mod that took longer showed "network failure: error decoding response body". Now it only gives up when the connection sits idle for 30 seconds.
- Downloads that drop halfway now **resume from where they stopped** (up to 6 tries) instead of failing, and start over by themselves if the server doesn't support resuming. The final file is still checked against GameBanana's MD5.
- Network errors now say the real cause (timed out, connection reset...) instead of the generic "error decoding response body".

## 0.7.0

First public release. A lot of stuff, so here we go.

### Catalog and mods
- Catalog straight from GameBanana: mods and sounds, 5-column grid, infinite scroll, category tree with the class icons.
- Big mod page: image gallery (arrows, thumbnails, click to zoom), description, downloads / likes / views, dates, tags and every file.
- Installs `.zip`, `.rar` and `.7z`. `.rar` / `.7z` are opened with 7-Zip, WinRAR or Windows' own `tar`, whichever you have (nothing RAR-related is bundled).
- **Analyze before installing**: shows what a mod changes (hitsound, killsound, class voices, weapons...) with the class icon, and lets you listen to the sounds first.
- Installed mods are cards now, with the mod's picture.

### Casual
- New **Casual** tab: sets up cueki's Casual Preloader for you (official release, size and SHA-256 checked), keeps a list of casual mods, applies and reverts with a progress bar.
- Checks that TF2 launches with `+exec w/config.cfg` and tells you if it doesn't.

### The rest
- **Installer wizard** (pt-BR, English, Español), per-user install, no admin needed.
- **Updater**: tells you when a new stable version is out, shows what changed, and only installs after checking the published SHA-256.
- **Settings** tab: language, TF2 folder, QuickPrecache switch, file formats, cache, update options.
- English and Spanish translations.
- Play button now greys out while TF2 is open, and the game state refreshes by itself every few seconds.
- Class icons look the same everywhere now (before, some had a dark background and some looked like outlines).

### Important commits
- `8238e6d` license, credits, gitignore (what stays out of the repo)
- `5a7cd7a` core crate: Steam detection, GameBanana client, installer, analysis, Casual Preloader integration, updater
- `310e3cb` the bridge to the Casual Preloader
- `c1b0b08` the app: Tauri shell, React UI in en / es / pt, installer config, icons
- `4474b79` docs, changelog, CI and release workflows
- `166a0cc` readme

### Known stuff
- Mods with particles (`.pcf`) aren't supported in the Casual tab yet.
- Only Casual Preloader 2.3.1 is tested.
- The installer isn't code-signed yet, so Windows SmartScreen may complain.
