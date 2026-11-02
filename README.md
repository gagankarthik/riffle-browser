<p align="center">
  <a href="https://gagankarthik.github.io/riffle-browser/"><img src="website/logo.svg" alt="Riffle browser logo" width="112" height="112"></a>
</p>

<h1 align="center">Riffle: a fast, lightweight ad-blocking browser for Windows</h1>

<p align="center">
  <a href="https://github.com/gagankarthik/riffle-browser/releases/latest"><img src="https://img.shields.io/github/v/release/gagankarthik/riffle-browser?label=version" alt="Latest Riffle version"></a>
  <a href="https://github.com/gagankarthik/riffle-browser/actions/workflows/ci.yml"><img src="https://github.com/gagankarthik/riffle-browser/actions/workflows/ci.yml/badge.svg" alt="Test status"></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/gagankarthik/riffle-browser" alt="MIT license"></a>
  <img src="https://img.shields.io/badge/platform-Windows-0b6e8f" alt="Runs on Windows">
  <img src="https://img.shields.io/badge/written%20in-Rust-0b6e8f" alt="Written in Rust">
</p>

<p align="center">
  <a href="https://github.com/gagankarthik/riffle-browser/releases/latest/download/RiffleSetup.exe"><strong>Download for Windows</strong></a> |
  <a href="https://gagankarthik.github.io/riffle-browser/">Website</a> |
  <a href="https://github.com/gagankarthik/riffle-browser/releases">All releases</a>
</p>

**Riffle is a free, open-source web browser for Windows with a built-in ad blocker.** It stops ads, trackers, pop-ups and fake redirects, puts idle tabs to sleep to save memory, and needs no account. It's written in Rust, the installer is about 3.4 MB, and it uses the WebView2 engine built into Windows, so sites render as they do in Microsoft Edge. AI agents can also control it when you allow them to.

The name and logo come from a riffle, the quick, rippling stretch of a stream: pages should flow without anything in the way.

## Why use Riffle

- **Fewer ads, no extension needed.** Ad and tracker blocking is built in and on from the start.
- **Light on memory.** Only your most recent tabs stay loaded.
- **Private by default.** No accounts, sync or telemetry, and bookmarks and history stay on your own computer.
- **Ready for AI agents.** An opt-in agent mode works with Playwright and other Chrome DevTools Protocol tools.
- **Keeps itself current.** New versions install automatically unless you turn that off.

## Download and install

1. Download [`RiffleSetup.exe`](https://github.com/gagankarthik/riffle-browser/releases/latest/download/RiffleSetup.exe) from the [latest release](https://github.com/gagankarthik/riffle-browser/releases/latest).
2. Run it and select Install. No administrator rights are needed.
3. Open Riffle from the Start menu.

The installer is not code-signed yet, so Windows may warn about a new app: choose More info, then Run anyway. `SHA256SUMS.txt` in each release lets you check the file.

To update by hand, close Riffle and run a newer `RiffleSetup.exe`. Your bookmarks, history, logins and settings are kept.

## Features

- **Built-in ad blocker.** Uses Brave's open-source engine with EasyList, EasyPrivacy, uBlock's filter, badware, privacy and unbreak lists, and Peter Lowe's list, refreshed every few days into `%LOCALAPPDATA%\\Riffle`. It blocks ad requests, hides empty ad boxes, skips YouTube video ads, and runs uBlock's page-fixing scripts. Ads built into a page's own text or video, and sponsored posts, can still get through.
- **No trash redirects.** Pop-ups need a click on a real link, invisible links laid over a page are removed, and a click that sends you to an unrelated site is stopped. The shield in the address bar shows what was blocked, can open a stopped page anyway, and can turn blocking off for one site.
- **Tab sleeping.** Only the 3 most recently used tabs stay in memory, tabs idle for 5 minutes are released, tabs playing audio stay awake (changeable in Settings), and hidden tabs are asked to use less. Others are released and reload when selected, shown in grey italics. Unsaved form text and background audio in a sleeping tab are lost.
- **Safe address bar.** `javascript:` and `file:` text is searched, never run.
- **Pop-up limit.** Pop-ups open as tabs, at most one per second.
- **Automatic updates.** The installed copy asks GitHub for a newer release once a day, checks the download against its published SHA-256 hash, and starts the new version the next time you open it. Turn it off in Settings.
- **Private by default.** No accounts, sync or telemetry added by Riffle. Searches go to DuckDuckGo with its ads turned off.
- **Private windows.** Open in their own window with an explainer page. They keep no history, and their session is erased when the last one closes. The website and your network can still see the visit.
- **Playing tabs.** A tab playing audio shows a speaker icon.
- **Video fullscreen.** A site's fullscreen button fills the whole screen with the toolbar hidden. Esc or any browser shortcut comes back.
- **Music controls.** The music button beside the address bar lists what is playing, with play/pause and volume for each tab.
- **Own menus and dialogs.** The three dots menu, music dialog, confirm dialogs and Settings controls follow the theme, and work with the keyboard.
- **History.** Searchable, with an X to remove any single entry or a button to clear it all.
- **Downloads.** A small note under the toolbar reports each one. Ctrl+J or the toolbar button lists finished and failed downloads, and shows a file in its folder. Settings chooses the folder, or asks each time. Private windows stay off the list.
- **Per-site exceptions.** The shield can allow pop-ups and redirects, or turn blocking off, for one site. Settings lists every exception.
- **New tab page.** A plain page with the search bar in the centre, or your own page from Settings.
- **Bookmarks and history.** Stored as plain files in `%LOCALAPPDATA%\Riffle`, searchable from the menu.
- **Casting.** The menu opens the Windows Cast panel, which mirrors the screen to Miracast displays. Chromecast is not supported.
- **Customisation.** Settings has theme, accent colour, search engine (or your own URL), default browser, download folder, new tab page, restore tabs on startup, and how many tabs stay awake.
- **Address bar suggestions.** Bookmarks and history complete as you type, with search suggestions from your engine. Typed web addresses are never sent, and private windows ask no engine. Turn it off in Settings, Search engine.
- **Developer menu.** View page source, DevTools docked beside the page with a draggable divider, and the engine's task manager.

### Keyboard shortcuts

Esc leaves video fullscreen, Ctrl+Shift+N private window, Ctrl+Shift+T reopen closed tab, Ctrl+D bookmark, Ctrl+Shift+O bookmarks, Ctrl+H history, Ctrl+J downloads, Ctrl+P print, Ctrl+T new tab, Ctrl+W close, Ctrl+L address bar, Ctrl+Tab next tab, Ctrl+Shift+Tab previous tab, Ctrl+F find, Ctrl+plus and Ctrl+minus zoom, Ctrl+U page source, F12 or Ctrl+Shift+I DevTools.

## Performance and optimisation

Riffle has not been formally benchmarked yet, so this lists how it saves resources rather than speed claims.

- **Small download.** One executable of about 3.4 MB, built with link-time optimisation and size-focused compiler settings.
- **Shared engine.** It uses the WebView2 engine already on Windows instead of bundling its own copy of Chromium.
- **Tab sleeping.** Only the 3 most recently used tabs stay in memory, and tabs idle for 5 minutes are released.
- **Fewer requests.** Blocked ads and trackers are never downloaded.
- **Lower background use.** Hidden tabs are asked to use less memory, and only the visible tab is resized with the window.

## For AI agents

```
riffle-browser.exe --agent
```

This opens the Chrome DevTools Protocol on `127.0.0.1:9222`, which agent tools can connect to:

```
npx chrome-devtools-mcp --browserUrl http://127.0.0.1:9222
npx @playwright/mcp --cdp-endpoint http://127.0.0.1:9222
```

Agent mode lets any program on this computer control the browser, including sites you are logged in to. Use it only when needed. Close normal Riffle windows first, because both modes share one profile and cannot run together.

## Install options and uninstalling

Run `riffle-browser.exe --install`, choose Install Riffle in Settings, or rename the exe to `RiffleSetup.exe` and run it. A setup window offers a desktop shortcut and pinning to Start. It copies Riffle and `uninstall.exe` to `%LOCALAPPDATA%\Programs\Riffle`, adds a Start menu entry, lists it in Windows Apps and registers it as a browser, with no administrator rights. Windows does not let apps pin themselves to the taskbar, so the setup window explains the one manual step.

To remove it, use Windows Settings, Apps, `uninstall.exe`, or Uninstall Riffle in Settings. It asks whether to delete your bookmarks, history, logins and settings as well. `--uninstall --silent` keeps them. Files you downloaded are never touched.

## Build from source

Needs Windows and [Rust](https://rustup.rs).

```
cargo build --release
target\release\riffle-browser.exe [address]
```

Pushing a tag such as `v0.2.0` tests and builds the installer and publishes a release. Pushing changes under `website/` to `main` updates the website.

## FAQ

### Is Riffle free?

Yes. Riffle is free and open source under the [MIT license](LICENSE).

### Does Riffle block YouTube ads?

It skips YouTube video ads and blocks ad requests on other sites. Sponsored posts, and ads built into a page's own text or video, can still get through.

### Does Riffle work on Mac or Linux?

No. Riffle runs only on Windows, because it relies on the Windows WebView2 engine.

### Does Riffle collect my data?

Riffle adds no accounts, sync or telemetry. It does contact GitHub once a day to check for updates, which you can turn off in Settings, and it downloads filter list updates every few days.

### Can AI agents use Riffle?

Yes. Start it with `--agent` and connect a tool such as Playwright. See [For AI agents](#for-ai-agents).

## License

[MIT](LICENSE). The website's fonts are under the SIL Open Font License: see `website/fonts`.
