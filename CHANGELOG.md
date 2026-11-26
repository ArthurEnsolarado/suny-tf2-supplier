# Changelog

Every stable release shows up inside the app (Settings → Updates) with the notes from here.
Format: newest first. Versions follow `major.minor.patch`.

## Unreleased

### Added
- **Transparency: AI and your data.** *Settings → About* (and the readme) now say how AI was used to make the app, what it uses the internet for, what it reads and changes on your PC, and that it has no account, server, ads or telemetry.

### Fixed
- **Computers with more than one Steam account:** to know whether *+exec w/config.cfg* was already set, the app skipped an account that had no TF2 launch options and could read another account's. It then showed *Already there* and the Play button did not add the option, so the preload never ran and casual mods such as animations did not work. Only the account in use counts now.

## 0.13.0

### Changed
- **Installing from the catalog is now one panel and one button.** Opening a mod checks the file by itself (files above 25 MB wait for a click) and says in plain words whether it can be installed: antivirus, whether the paths work in casual, and which of your mods it would fight with, *before* you install. The destination comes pre-selected with the recommended one. In Casual, one button downloads, adds, ticks and applies the mod, showing each step; *Apply to the game too* can be turned off, and when the new mod brings the same files as a ticked one the button offers to swap them. When it ends there's a *Play* button. If you already have the mod, the page says so and offers what makes sense (apply, play, choose sounds). The separate *Analyze file* button and the extra confirmation for `custom` are gone.

### Added
- **Choose which sound plays.** A Casual mod that ships numbered variations (like the three shield impact sounds) has a *Choose sounds* button. For each group you can listen to every file and keep the random draw as the pack came, always play one of them (the app puts it in every slot of the draw), or leave that group with the original TF2 sound. The pack's original files are kept, so you can go back, and the game only changes when you apply again.

### Fixed
- **"Could not find EOCD" when opening a catalog file.** The file page can analyze a sound pack by itself when it opens, and a second request (Analyze, preview or Add to Casual) could read the download while it was still half written. Downloads are now assembled aside and only appear when complete, one request at a time for each file.

## 0.12.0

### Added
- **The app now sees mods that fight each other**, across the Casual list, mods in `custom` (installed by the app or by hand) and what you are about to install.
  - **Same files, different content: blocked.** In the Casual tab you choose which one stays; *Apply* waits until the conflict is solved. Installing to `custom` something that fights with `custom` or with a ticked Casual mod is refused, saying which one and which file, and nothing is written. A new Casual mod that fights with a ticked one enters the list unticked. The parts of a model (`.mdl`, `.vvd`, `.vtx`, `.phy`, `.ani`) count as one thing, so two mods can't be mixed inside one model.
  - **Different files, same object: a warning** (same weapon, cosmetic, character, two HUDs, or variations of one sound split between two mods). You decide. The object is estimated from file paths, so not every case is noticed.
  - Identical copies of a file don't count, and neither does the same GameBanana item present in both Casual and `custom`.

### Fixed
- **Windows are no longer covered by the top bar** (since 0.11.1 the title and the *Close* button of a window could end up under it).

## 0.11.3

### Changed
- **The Casual tab now says how to check that the preload ran:** when the game finishes loading, the console opens with a drawing of a cat and *casual particle preloader ACTIVE!!*.

### Fixed
- **The warnings about models no longer point to the wrong fix.** They suggested QuickPrecache and *Install to custom* for any model mod. QuickPrecache (in Casual Preloader 2.3.1) only picks up map objects such as props, flags, health kits, ammo boxes and MvM bots, chosen by file path, so it doesn't help class animations. The setting is now called *Preload map objects*.

## 0.11.2

### Fixed
- **Catalog mods that are just loose files can be installed.** Some old pages ship one or two files with no game folders and explain in the text where to put them (for example *tf\materials\models\weapons\c_items*), and "Add to Casual" answered "couldn't recognize the mod structure". The app now puts those files in the right folder before analyzing, installing or adding them: it uses the original game's files first and the folder the author wrote on the page second. Packs that already have the structure, carry a `.vpk` or have several variants are not changed.

## 0.11.1

### Fixed
- **The "Restart the game" confirmation can be clicked again.** Its box opens under the top bar, and the page below was painted over it, so the buttons did not react and the box overlapped the status badge on every screen. The top bar now sits above the page.

## 0.11.0

### Added
- **Add a mod you already have.** In the *Installed* tab, drop a `.zip`, `.rar`, `.7z`, `.vpk`, a folder or loose files (`.wav`, `.bsp`, ...) onto the window, or use *Choose files* / *Choose folder* / paste a path. The app works out the structure by itself and shows what it understood before it writes anything: the compatibility seal, which classes and parts of the game it changes, and the sounds to listen to.
  - Wrapping folders (`MyMod/tf/custom/mymod/...`) are peeled off, `.vpk` packages bring their `_000.vpk` data parts along, and junk like `Thumbs.db` is ignored.
  - Loose files get the right game folder: maps, configs and particles by extension; sounds, textures and models by comparing their name with the game's original files, **only when exactly one original path matches** (a name that exists in two places is left out and listed, never guessed).
  - It goes through the same install panel as the catalog: Casual or Offline/community (maps never to Casual), with the game open only new items are allowed, executables block everything, and nothing is overwritten.
  - Your original files are never changed. Installing the same mod again replaces the app's own copy; removing it from *Installed* deletes only what the app created.
- The Home screen no longer says the app only installs with TF2 closed (that stopped being true in 0.10.0).

## 0.10.1

### Fixed
- **Sound previews no longer load forever.** For `.zip` packs the app now reads only the file index and one sample straight from GameBanana (an 18 MB voice pack costs a few hundred KB instead of the whole download). Packs that still need the full download (`.rar`) stop after 2 minutes with a clear message instead of spinning.
- **The class stays selected** when you enter one of its subcategories (for example *Skins > The Scout > Animations*), and the category tree opens the class's branch so you can pick it. Picking a category from another branch clears the class, as it should. Changing the order never cleared it.
- **File page images are no longer cropped.** The original picture is shown whole inside a fixed box (some pictures bring their own black bars, which still show).
- On narrow windows the picture no longer overlaps the install area.

## 0.10.0

This version keeps the 0.8.1 look (the one testers liked) and adds what they asked for. The calm dark redesign that was tried in 0.9.0 and 0.9.1 is not part of it.

### Added
- **Class row in the Catalog**: the face of each class (read from your own game) works as a shortcut to that class's category, `Skins > The Scout` for mods and `Character Voice > Scout Voice` for sounds.
- **One place to install**: the file card now asks *Where will you play?* (Casual on Valve's servers, or Offline and community) and shows only the right button, with a short explanation of the difference. **Maps** never offer Casual: the Casual Preloader doesn't load maps.
- **Install with the game open**: new items can be copied to `custom` while TF2 is running, with a **Restart the game** button (asks first, because leaving a match counts as abandoning). Removing, reinstalling and applying the preloader still need the game closed.
- **Installed**: all the items of one mod are one card, with a window that lists them. Hand-made items with the same root name (`pack_01`, `pack_02`, ...) are grouped too.
- Preview errors on sound cards appear as a readable message instead of a tiny overlay.

### Fixed
- Searching by name no longer locks the order and the category. The search now goes through the same listing as the rest of the Catalog.
- The category tree only shows the expand arrow when the category really has children.
- Catalog and file pictures are sharper: cards use the 800 px version and the file page uses the original image.
- The file page fits the window (each column scrolls on its own), so the install button no longer sits below the fold.
- Casual: *Apply* and *So the game opens with the mods* sit side by side instead of leaving half the screen empty.
- The top bar no longer overflows (and cuts the build label) on windows around 1440 px wide.

## 0.8.1

### Fixed
- **Add to Casual** failed with "couldn't recognize the mod structure ... or the zip has more than one mod" on packs that carry several variants (for example *Scottish Shield + Captain America*: two models, two flags). Each variant now becomes its own item in the Casual list, named after its folder in the archive, and they come **unchecked**: they replace the same files, so you tick the one you want. Packs with more than 12 variants are still refused, with a message saying so.

## 0.8.0

### Added
- **Installed** now reads what each item in `tf/custom` actually changes (weapon models, HUD, sounds of a class, ...) and shows it as tags on the card. It works for mods you put there by hand, folders and `.vpk` alike, without touching anything.
- **File fights**: when two items bring the same file, only one can be used by the game. The Installed tab lists those fights, says which item usually wins (the first one in alphabetical order, the way the game reads `custom`) and which ones lose. Read only: to change who wins you rename the folder or the `.vpk`.

## 0.7.9

### Changed
- Installing over an older version no longer pushes you to uninstall first. The "already installed" page now offers **Update in place, without uninstalling (recommended)** and, as the other choice, **Uninstall and install from scratch**. Texts in Portuguese, English and Spanish.

## 0.7.8

### Changed
- The installer now has its own art: the logo on the side of the wizard, a header banner, and the orange and brown colours of the app, instead of the default blue.

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
