# Riffle Browser

A fast, lightweight Windows web browser with a built-in ad blocker, written in Rust. The name and logo come from a riffle, the quick, rippling stretch of a stream: pages should flow without anything in the way. It uses the WebView2 engine built into Windows, so sites render as they do in Edge.

[Website](https://gagankarthik.github.io/riffle-browser/) | [Download for Windows](https://github.com/gagankarthik/riffle-browser/releases/latest/download/RiffleSetup.exe) | [All releases](https://github.com/gagankarthik/riffle-browser/releases)

## Download

Download `RiffleSetup.exe` from the [latest release](https://github.com/gagankarthik/riffle-browser/releases/latest) and run it. No administrator rights are needed. The installer is not code-signed yet, so Windows may warn about a new app: choose More info, then Run anyway. `SHA256SUMS.txt` in each release lets you check the file.

## Build from source

Needs Windows and [Rust](https://rustup.rs).

```
cargo build --release
target\release\riffle-browser.exe [address]
```

Pushing a tag such as `v0.1.0` builds the installer and publishes a release. Pushing changes under `website/` to `main` updates the website.

## Install and uninstall

Run `riffle-browser.exe --install`, choose Install Riffle in Settings, or rename the exe to `RiffleSetup.exe` and run it. A setup window offers a desktop shortcut and pinning to Start. It copies Riffle and `uninstall.exe` to `%LOCALAPPDATA%\Programs\Riffle`, adds a Start menu entry, lists it in Windows Apps and registers it as a browser, with no administrator rights. Windows does not let apps pin themselves to the taskbar, so the setup window explains the one manual step.

To remove it, use Windows Settings, Apps, `uninstall.exe`, or Uninstall Riffle in Settings. It asks whether to delete your bookmarks, history, logins and settings as well. `--uninstall --silent` keeps them. Files you downloaded are never touched.

## Features

- **Built-in ad blocker.** Uses Brave's open-source engine with EasyList, EasyPrivacy, uBlock's filter, badware, privacy and unbreak lists, and Peter Lowe's list, refreshed every few days into `%LOCALAPPDATA%\\Riffle`. It blocks ad requests, hides empty ad boxes, skips YouTube video ads, and runs uBlock's page-fixing scripts. Ads built into a page's own text or video, and sponsored posts, can still get through.
- **No trash redirects.** Pop-ups need a click on a real link, invisible links laid over a page are removed, and a click that sends you to an unrelated site is stopped. The shield in the address bar shows what was blocked, can open a stopped page anyway, and can turn blocking off for one site.
- **Tab sleeping.** Only the 3 most recently used tabs stay in memory, tabs idle for 5 minutes are released, tabs playing audio stay awake (changeable in Settings), and hidden tabs are asked to use less. Others are released and reload when selected, shown in grey italics. Unsaved form text and background audio in a sleeping tab are lost.
- **Safe address bar.** `javascript:` and `file:` text is searched, never run.
- **Pop-up limit.** Pop-ups open as tabs, at most one per second.
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

Shortcuts: Ctrl+Shift+N private window, Ctrl+Shift+T reopen closed tab, Ctrl+D bookmark, Ctrl+Shift+O bookmarks, Ctrl+H history, Ctrl+J downloads, Ctrl+P print, Ctrl+T new tab, Ctrl+W close, Ctrl+L address bar, Ctrl+Tab next tab, Ctrl+Shift+Tab previous tab, Ctrl+F find, Ctrl+plus and Ctrl+minus zoom, Ctrl+U page source, F12 or Ctrl+Shift+I DevTools.

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

## License

[MIT](LICENSE). The website's fonts are under the SIL Open Font License: see `website/fonts`.
