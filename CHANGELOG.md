# Changelog

Every stable release shows up inside the app (Settings → Updates) with the notes from here.
Format: newest first. Versions follow `major.minor.patch`.

## 0.7.7

### Fixed
- Casual models (weapons, cosmetics) showing the normal model: the preloader only makes them appear when TF2 starts with `+exec w/config.cfg`, and the **Play button now adds that option by itself** when the preloader is applied and your Steam launch options don't have it. Your own launch options are kept. Starting TF2 from the Steam library still needs the option set in Steam.

## 0.7.6

### Added
- **Warning for mods with models in the Casual list.** Found the hard way: when a model mod (like The Eyecrafter) is in the Casual list, the preloader packs its model inside `_casual_preloader_*.vpk`, which is read *before* `custom` and overrides the copy in `custom`. In Valve's casual the model then needs pre-loading (QuickPrecache, off by default and untested) or you see the normal model again. The app now tags those mods with **Models**, tags mods installed in both places with **Also in custom**, and explains it right on the Casual page and when you add one.

### Fixed
- Category names no longer overlap their counters in the catalog sidebar.

## 0.7.5

### Fixed
- The app told you that mods installed to `custom` never show up in Valve's casual. That was wrong as a blanket statement: in a real casual match a `custom` mod (The Eyecrafter) did show up while the Casual Preloader was applied to the game. The texts now say what is actually known: it can show up when the preloader is applied, but **Add to Casual** is still the supported way for casual.

## 0.7.4

### Fixed
- **Mods packed in a `.vpk` now work in the Casual tab.** Many GameBanana mods (like The Eyecrafter) ship a single `.vpk`; "Add to Casual" used to answer "couldn't recognize the mod structure". The app now hands the `.vpk` to the preloader's own VPK importer. VPKs with particles are refused with a clear message.
- The analysis now reads **inside** `.vpk` files, so "What changes" lists the real models and textures (before it said "no game files in this package").
- The window no longer overflows around 1500 px wide (the tab icons pushed the top bar out of the window).

### Changed
- **Add to Casual** is the main (orange) button on a mod's page; **Install to custom** is secondary and now says right there what it is for (offline play and community servers).
- **The interface adapts to any screen.** Content has a maximum width and stays centered (ultrawide screens no longer stretch everything), the catalog grid picks its number of columns from the width, and on big screens (2K, 4K, ultrawide) the app scales itself up. Settings → Interface size lets you pick a fixed size instead. Checked from 920 x 600 up to 2560 x 720.

## 0.7.3

### Added
- **Listen before you install, right in the catalog**: every sound card has a play button. It downloads the first file of the pack, plays the first sound and says how many more are inside. In a sound's page, the sounds are listed with play buttons as soon as it opens (no need to press Analyze first).
- **Game path checker**: type, paste or pick the folder and the app tells you right away if it works (build number included) or exactly what's wrong. It accepts the `Team Fortress 2` folder, `tf`, `custom`, `hl2.exe`, the Steam library folder, and quoted paths.
- **Better game detection**: besides Steam, the app now looks in the usual folders (`Program Files (x86)\Steam`, `SteamLibrary` on every drive...). **Find the game** (Home and Settings) shows what was found, what Steam said and every folder checked, so it never just says "not found". There's also a **Choose folder** button.
- **Play TF2 moved into the Home card**, where "Ready" used to be. Still greyed out while the game is open.
- Real icons: the whole app uses the Lucide set now (tabs, buttons, badges, categories, gallery arrows).

### Fixed
- The language switch was cut off at the right edge in Settings.
- Buttons in Settings (Updates, Storage) were glued to the text and to each other.
- Disabled outline buttons looked like plain text.
- The "Categories" title was drawn under the list while searching.
- The `tar do Windows` name showed up untranslated in English and Spanish.
- A sound's page had a huge empty block and stretched to the bottom of the window. It fits its content now.

## 0.7.2

### Changed
- **Titles, tabs and buttons now use Lilita One**, a free font that looks a lot like the TF2 menu (before, builds without TF2 Build fell back to plain Arial). It ships inside the installer, together with its license.

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
