#![cfg_attr(not(test), windows_subsystem = "windows")]

use serde_json::{Value, json};
use tao::dpi::{LogicalSize, PhysicalPosition};
use tao::event::{ElementState, Event, MouseButton, WindowEvent};
use tao::keyboard::KeyCode;
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget};
use tao::window::{CursorIcon, Fullscreen, Icon, ResizeDirection, Window, WindowBuilder};
use tao::platform::windows::{WindowBuilderExtWindows, WindowExtWindows};
use url::{Host, Url};
use wry::dpi::{LogicalPosition, LogicalSize as ViewSize};
use adblock::Engine;
use adblock::lists::{FilterSet, ParseOptions};
use std::cell::RefCell;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::{WebResourceRequestedEventHandler, take_pwstr};
use webview2_com::{AcceleratorKeyPressedEventHandler, AddScriptToExecuteOnDocumentCreatedCompletedHandler, BrowserExtensionEnableCompletedHandler, BrowserExtensionRemoveCompletedHandler, CallDevToolsProtocolMethodCompletedHandler, ContainsFullScreenElementChangedEventHandler, GetProcessExtendedInfosCompletedHandler, NavigationCompletedEventHandler, NavigationStartingEventHandler, ProcessFailedEventHandler, ProfileAddBrowserExtensionCompletedHandler, ProfileGetBrowserExtensionsCompletedHandler};
use windows::Win32::Foundation::{COLORREF, CloseHandle, HWND};
use windows::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender};
use windows::Win32::System::Com::{CLSCTX_ALL, CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::Foundation::{LPARAM, RECT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{CreateWindowExW, EnumWindows, FindWindowExW, GWL_EXSTYLE, GWL_STYLE, GetWindowLongPtrW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, HWND_TOP, IsWindow, IsWindowVisible, PostMessageW, SW_HIDE, SW_SHOWNOACTIVATE, SWP_FRAMECHANGED, SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, LWA_ALPHA, SetLayeredWindowAttributes, WS_EX_LAYERED, CHILDID_SELF, EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_SHOW, OBJID_WINDOW, WINEVENT_OUTOFCONTEXT, SetParent, SetWindowLongPtrW, SetWindowPos, ShowWindow, WINDOW_EX_STYLE, WM_CLOSE, WS_CAPTION, WS_CHILD, WS_CLIPCHILDREN, WS_EX_NOACTIVATE, WS_POPUP, WS_THICKFRAME};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook};
use windows::Win32::UI::Shell::{FOLDERID_Desktop, FOLDERID_Downloads, FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS, FileOpenDialog, FileSaveDialog, IFileDialog, IShellItem, KF_FLAG_DEFAULT, SHCreateItemFromParsingName, SHCreateMemStream, SHGetKnownFolderPath, SIGDN_FILESYSPATH};
use windows::core::{BOOL, Interface, PCWSTR, PWSTR, w};
use wry::{MemoryUsageLevel, NewWindowResponse, PageLoadEvent, Rect, WebContext, WebView, WebViewBuilder, WebViewBuilderExtWindows, WebViewExtWindows};

const WINDOW_COLOR: (u8, u8, u8, u8) = (0x8a, 0x95, 0x9c, 0xff); // only ever seen as the DevTools divider
const RELEASES: &str = "https://github.com/gagankarthik/riffle-browser/releases/download";
const LATEST_RELEASE: &str = "https://api.github.com/repos/gagankarthik/riffle-browser/releases/latest";
const UPDATE_GAP_SECS: u64 = 86_400; // how often GitHub is asked
const UPDATE_WAKE_SECS: u64 = 6 * 3600; // so a browser left open for days still updates
const TOOLBAR_HEIGHT: f64 = 81.0;
const BOOKMARKS_BAR_HEIGHT: f64 = 32.0;
const NAV_ROW_BOTTOM: f64 = 80.0; // tab row plus address row in toolbar.html
const MENU_BUTTON_RIGHT: f64 = 8.0; // distance from the menu button's right edge to the window's
const DEFAULT_LIVE_TABS: u64 = 3; // the rest sleep and free their memory until selected
const SEARCH_URL: &str = "https://duckduckgo.com/?k1=-1&q="; // k1=-1 turns off DuckDuckGo's own ads
const AGENT_PORT: u16 = 9222;
const POPUP_MIN_GAP_MS: u128 = 1000;
const SCRIPTLETS_JS: &str = include_str!("scriptlets.js");
const TOOLBAR_HTML: &str = include_str!("toolbar.html");
const PANEL_HTML: &str = include_str!("panel.html");
const ICONS_HTML: &str = include_str!("icons.html");
const MUSIC_HTML: &str = include_str!("music.html");
const MENU_HTML: &str = include_str!("menu.html");
const SHIELD_HTML: &str = include_str!("shield.html");
const BOOKMARK_HTML: &str = include_str!("bookmark.html");
const ZOOM_HTML: &str = include_str!("zoom.html");
const TAB_MENU_HTML: &str = include_str!("tabmenu.html");
const MARKS_HTML: &str = include_str!("marks.html");
const TAB_SEARCH_HTML: &str = include_str!("tabsearch.html");
const ERROR_HTML: &str = include_str!("error.html");
const SLOW_HTML: &str = include_str!("slow.html");
const SLOW_SECS: u64 = 12; // a page with nothing to show after this long gets a note
const SLOW_NOTE: (f64, f64) = (380.0, 120.0);
const WELCOME_HTML: &str = include_str!("welcome.html");
const WELCOME_URL: &str = "riffle:welcome";
const WELCOME_FOR: &str = "0.4"; // change when welcome.html lists new features, and it shows once more
const READER_JS: &str = include_str!("reader.js");
const TAB_MENU_SIZE: (f64, f64) = (220.0, 208.0);
const TAB_SEARCH_SIZE: (f64, f64) = (480.0, 380.0);
const MARKS_WIDTH: f64 = 280.0;
const IMPORT_LIMIT: usize = 5000;
const ZOOM_POPUP: (f64, f64) = (248.0, 52.0);
const ZOOM_LEVELS: &[f64] = &[0.25, 0.33, 0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0, 5.0]; // the steps Chrome and Edge use
const BOOKMARK_WIDTH: f64 = 340.0;
const DOWNLOAD_ICON_SECS: u64 = 300; // how long the toolbar icon stays after a download ends. The menu always has Downloads
const TOOLBAR_BUTTON: f64 = 36.0; // each icon's width and gap
const ADDRESS_ICON: f64 = 28.0; // each icon inside the address bar
const SHIELD_WIDTH: f64 = 320.0;
const MUSIC_WIDTH: f64 = 320.0;
const IDLE_SLEEP_SECS: u64 = 300; // background tabs untouched this long release their memory
const IDLE_CHECK_SECS: u64 = 60;
const MEDIA_PICK: &str = "const m=[...document.querySelectorAll('video,audio')],el=m.find(x=>!x.paused)||m[0];";
const NEW_TAB_HTML: &str = include_str!("newtab.html");
const INSTALL_HTML: &str = include_str!("install.html");
const SUGGEST_HTML: &str = include_str!("suggest.html");
const DOWNLOAD_HTML: &str = include_str!("download.html");
const DOWNLOAD_BUBBLE: (f64, f64) = (340.0, 104.0);
const DEVTOOLS_CLOSE: f64 = 32.0; // the close button laid over the right end of DevTools' own tab row, where Chrome and Edge put theirs
const DEVTOOLS_HTML: &str = include_str!("devtools.html");
const DEVTOOLS_ARMED_SECS: u64 = 3; // after asking for DevTools, how long a new engine window is taken to be it
const DEVTOOLS_SETTLE_SECS: u64 = 2; // docked this long, a DevTools window has stayed
const DEVTOOLS_RETRIES: u32 = 2;
const DEVTOOLS_HIDE_SECS: u64 = 4; // a hidden window not docked by now comes back, so DevTools can never be lost
const DEVTOOLS_SCAN_MS: u64 = 1000; // only finds DevTools opened from the page's own menu, and this wakes the browser
const DEVTOOLS_SHARE: f64 = 0.45; // of the window width, like a browser's docked panel
const DEVTOOLS_DIVIDER: f64 = 6.0; // the bare window showing between page and DevTools, dragged to resize
const DEVTOOLS_BURST_MS: u64 = 15; // right after opening, so the floating window shows for as little as possible
const SUGGEST_MAX_LOCAL: usize = 4;
const SUGGEST_MAX_SEARCH: usize = 5;
const HISTORY_LIMIT: usize = 5000;
const DOWNLOADS_KEPT: usize = 200;
const HISTORY_COMPACT_BYTES: u64 = 2_000_000;
const DEFAULT_ACCENT: &str = "#0b6e8f";

// The same public lists uBlock Origin and Brave build on. Downloaded rather than bundled, so they stay
// current without a new Riffle release. BLOCKED_HOSTS covers the moment before they have loaded.
const FILTER_LISTS: &[(&str, &str)] = &[
    ("easylist.txt", "https://easylist.to/easylist/easylist.txt"),
    ("easyprivacy.txt", "https://easylist.to/easylist/easyprivacy.txt"),
    ("ublock.txt", "https://ublockorigin.github.io/uAssets/filters/filters.min.txt"),
    ("ublock-badware.txt", "https://ublockorigin.github.io/uAssets/filters/badware.min.txt"),
    ("ublock-privacy.txt", "https://ublockorigin.github.io/uAssets/filters/privacy.min.txt"),
    ("ublock-unbreak.txt", "https://ublockorigin.github.io/uAssets/filters/unbreak.min.txt"),
    ("ublock-quick-fixes.txt", "https://ublockorigin.github.io/uAssets/filters/quick-fixes.min.txt"),
    ("peter-lowe.txt", "https://pgl.yoyo.org/adservers/serverlist.php?hostformat=adblockplus&showintro=0&mimetype=plaintext"),
];
// The small scripts uBlock's rules call on, for ads and anti-adblock walls that blocking requests cannot stop.
const SCRIPTLETS: (&str, &str) = ("scriptlets.json", "https://raw.githubusercontent.com/brave/adblock-resources/master/dist/resources.json");
const MENU_TRUST_SECS: u64 = 15;
const TRUST_SECS: u64 = 2; // a click explains a navigation or pop-up this soon after it
const VOUCH_SECS: u64 = 5; // what the user asked for directly, including the redirects on the way there
const FILTER_MAX_AGE_SECS: u64 = 4 * 24 * 60 * 60; // the lists themselves ask to be refetched every 4 days
const MIN_FILTER_LIST_BYTES: usize = 20_000;

static BLOCKED_NOW: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0); // not yet saved to stats.json
static ENGINE: OnceLock<Engine> = OnceLock::new();
static SEED_ENGINE: OnceLock<Engine> = OnceLock::new();
// $popup rules, which name the sites ads redirect to. The engine has no such option and would drop them.
static POPUP_ENGINE: OnceLock<Engine> = OnceLock::new();
static ALLOWED_SITES: Mutex<Vec<String>> = Mutex::new(Vec::new());
static POPUP_SITES: Mutex<Vec<String>> = Mutex::new(Vec::new());
static DOWNLOAD_PREFS: Mutex<(String, bool)> = Mutex::new((String::new(), false)); // (folder, ask each time), read when a download starts

// The three dots menu: (label, shortcut, icon, action). An empty label is a divider.
const MENU: &[(&str, &str, &str, &str)] = &[
    ("New tab", "Ctrl+T", "plus", "new_tab"),
    ("New window", "Ctrl+N", "window", "new_window"),
    ("New private window", "Ctrl+Shift+N", "private", "private_tab"),
    ("Reopen closed tab", "Ctrl+Shift+T", "reopen", "reopen_tab"),
    ("", "", "", ""),
    ("Bookmarks", "Ctrl+Shift+O", "star", "bookmarks"),
    ("History", "Ctrl+H", "clock", "history"),
    ("Downloads", "Ctrl+J", "download", "downloads"),
    ("", "", "", ""),
    ("Zoom", "", "search", "zoom"),
    ("Ad blocker", "", "shield", "ad_blocker"),
    ("Music and volume", "", "music", "music_dialog"),
    ("Picture in picture", "Ctrl+Shift+P", "pip", "picture_in_picture"),
    ("Extensions", "", "extension", "extensions"),
    ("Search tabs", "Ctrl+Shift+A", "tabs", "tab_search"),
    ("Reader mode", "", "book", "reader"),
    ("", "", "", ""),
    ("Cast screen", "Win+K", "cast", "cast"),
    ("Print", "Ctrl+P", "print", "print"),
    ("", "", "", ""),
    ("Developer", "", "code", "developer"),
    ("Settings", "", "settings", "settings"),
];
const DEVELOPER_MENU: &[(&str, &str, &str, &str)] = &[
    ("Back", "", "back", "menu_back"),
    ("", "", "", ""),
    ("View page source", "Ctrl+U", "code", "view_source"),
    ("Developer tools", "F12", "tools", "dev_tools"),
    ("Task manager", "", "tasks", "task_manager"),
    ("", "", "", ""),
    ("Close", "", "close", "menu_close"),
];
const MENU_WIDTH: f64 = 280.0;

const BLOCKED_HOSTS: &[&str] = &[
    "doubleclick.net", "googlesyndication.com", "googleadservices.com", "google-analytics.com",
    "googletagmanager.com", "googletagservices.com", "adservice.google.com", "adnxs.com", "taboola.com",
    "outbrain.com", "criteo.com", "criteo.net", "amazon-adsystem.com", "scorecardresearch.com",
    "quantserve.com", "rubiconproject.com", "pubmatic.com", "openx.net", "casalemedia.com", "moatads.com",
    "adsrvr.org", "yieldmo.com", "sharethrough.com", "bidswitch.net", "smartadserver.com", "33across.com",
    "hotjar.com", "mouseflow.com", "clarity.ms", "connect.facebook.net", "ads-twitter.com",
    "ads.linkedin.com", "ads.yahoo.com", "popads.net", "propellerads.com", "adcolony.com", "media.net",
    "mgid.com", "revcontent.com", "zedo.com", "2mdn.net", "adform.net", "adition.com", "adroll.com",
    "adsafeprotected.com", "advertising.com", "appnexus.com", "bluekai.com", "contextweb.com", "demdex.net",
    "doubleverify.com", "exelator.com", "eyeota.net", "gumgum.com", "indexww.com", "krxd.net", "liadm.com",
    "lijit.com", "mathtag.com", "mediavine.com", "adthrive.com", "nativo.com", "omtrdc.net",
    "pippio.com", "rlcdn.com", "rfihub.com", "semasio.net", "sonobi.com", "spotxchange.com", "spotx.tv",
    "teads.tv", "tapad.com", "tribalfusion.com", "turn.com", "undertone.com", "yieldlab.net", "yieldoptimizer.com",
    "zemanta.com", "adzerk.net", "buysellads.com", "carbonads.net", "exoclick.com", "juicyads.com", "trafficjunky.net",
    "adcash.com", "hilltopads.net", "adsterra.com", "admob.com", "applovin.com",
    "vungle.com", "inmobi.com", "smaato.net", "mopub.com", "chartboost.com", "app-measurement.com",
    "mixpanel.com", "fullstory.com", "crazyegg.com", "nr-data.net", "chartbeat.com", "chartbeat.net", "comscore.com",
    "an.facebook.com", "analytics.twitter.com", "analytics.tiktok.com", "ads.tiktok.com",
    "ads.reddit.com", "ads.pinterest.com", "bat.bing.com", "ads.microsoft.com",
];

// Collapses the empty boxes left where a blocked ad would have been, and skips YouTube's video ads,
// which share a host with the videos so the host list cannot stop them.
// shortcut: depends on YouTube's class names, update these selectors when YouTube ads reappear.
const AD_SCRIPT: &str = r#"
(() => {
  const yt = /(^|\.)youtube\.com$/.test(location.hostname);
  const css = () => {
    if (!document.documentElement) return new MutationObserver((_, o) => { if (document.documentElement) { o.disconnect(); css(); } }).observe(document, {childList: true});
    const s = document.createElement('style');
    s.textContent = 'ins.adsbygoogle,iframe[src*="doubleclick"],iframe[src*="googlesyndication"],[id^="google_ads_"],[id^="div-gpt-ad"],[data-ad-slot],[data-ad-client]{display:none!important}'
      + (yt ? 'ytd-ad-slot-renderer,ytd-in-feed-ad-layout-renderer,ytd-banner-promo-renderer,ytd-promoted-sparkles-web-renderer,#masthead-ad,#player-ads,.ytp-ad-overlay-container{display:none!important}' : '');
    (document.head || document.documentElement).appendChild(s);
  };
  css();
  // Tells the browser whether each click was on a real link, so pop-ups and redirects it did not explain can be stopped.
  // shortcut: a page aiming at Riffle could post click:1 itself. Would need WebView2's own gesture flag, which wry hides.
  const link = e => e.target.closest?.('a[href]');
  addEventListener('click', e => {
    if (!e.isTrusted) return;
    const a = link(e);
    if (a && !window.__riffleAllow) {
      const site = host => host.split('.').slice(-2).join('.');
      const look = getComputedStyle(a);
      const box = a.getBoundingClientRect();
      const huge = box.width * box.height > innerWidth * innerHeight * .5;
      const bare = !a.textContent.trim() && !a.querySelector('img,svg,video') && look.backgroundImage === 'none';
      if ((parseFloat(look.opacity) < .1 || (huge && bare)) && site(a.host) !== site(location.host)) {
        // An invisible link laid over the page to catch clicks: remove it so the next click reaches the page.
        e.preventDefault();
        e.stopImmediatePropagation();
        a.remove();
        window.ipc.postMessage('click:0');
        return;
      }
    }
    const real = e.target.closest?.('a[href],button,input[type=submit],input[type=button],input[type=image],[role=button],[role=link]');
    window.ipc.postMessage(real && window === top ? 'click:1' : 'click:0');
  }, true);
  addEventListener('submit', e => { if (e.isTrusted && window === top) window.ipc.postMessage('click:1'); }, true);
  addEventListener('auxclick', e => { if (e.isTrusted && e.button === 1 && link(e) && window === top) window.ipc.postMessage('click:1'); }, true);
  addEventListener('contextmenu', e => { if (e.isTrusted && link(e) && window === top) window.ipc.postMessage('click:menu'); }, true);
  if (window !== top) return;
  // Reports class names and ids so the filter lists can hide matching ad boxes, including ones added later.
  const seen = new Set();
  let timer = 0, first = true, added = [];
  const report = () => {
    timer = 0;
    const classes = [], ids = [];
    const note = el => {
      if (el.id && !seen.has('#' + el.id)) { seen.add('#' + el.id); ids.push(el.id); }
      for (const c of el.classList) if (!seen.has('.' + c)) { seen.add('.' + c); classes.push(c); }
    };
    // Only what was added, so a busy page does not rescan its whole document.
    for (const root of first ? [document.documentElement] : added) {
      if (root.nodeType !== 1) continue;
      note(root);
      root.querySelectorAll('[class],[id]').forEach(note);
    }
    added = [];
    if (first || classes.length || ids.length) window.ipc.postMessage('ads:' + JSON.stringify({first, classes, ids}));
    first = false;
  };
  const later = records => {
    for (const r of records) for (const n of r.addedNodes) added.push(n);
    if (!timer) timer = setTimeout(report, 400);
  };
  addEventListener('DOMContentLoaded', () => { report(); new MutationObserver(later).observe(document.documentElement, {childList: true, subtree: true}); });
  if (!yt) return;
  setInterval(() => {
    const player = document.querySelector('.html5-video-player.ad-showing');
    if (!player) return;
    const video = player.querySelector('video');
    if (video && isFinite(video.duration)) video.currentTime = video.duration;
    player.querySelector('.ytp-skip-ad-button,.ytp-ad-skip-button,.ytp-ad-skip-button-modern')?.click();
  }, 500);
})();
"#;

// uBlock's rules that plain CSS cannot express, such as hiding a box because of the words in it.
const PROCEDURAL_SCRIPT: &str = r#"
(rules => {
  const has = (el, arg) => {
    const re = arg.match(/^\/(.*)\/([a-z]*)$/);
    return re ? new RegExp(re[1], re[2]).test(el.textContent) : el.textContent.includes(arg);
  };
  const up = (el, arg) => {
    if (!/^\d+$/.test(arg)) return el.parentElement?.closest(arg);
    for (let i = +arg; el && i; i--) el = el.parentElement;
    return el;
  };
  const pick = stages => {
    let els = stages[0].type === 'css-selector' ? [...document.querySelectorAll(stages[0].arg)] : [];
    for (const {type, arg} of stages.slice(1)) {
      if (type === 'has-text') els = els.filter(el => has(el, arg));
      else if (type === 'min-text-length') els = els.filter(el => el.textContent.length >= +arg);
      else if (type === 'upward') els = els.map(el => up(el, arg)).filter(Boolean);
      else if (type === 'css-selector') els = els.flatMap(el => [...el.querySelectorAll(':scope ' + arg)]);
      else return []; // a step this does not understand must not hide the wrong thing
    }
    return els;
  };
  const act = (el, action) => {
    if (!action) el.style.setProperty('display', 'none', 'important');
    else if (action.type === 'remove') el.remove();
    else if (action.type === 'style') el.style.cssText += ';' + action.arg;
    else if (action.type === 'remove-attr') el.removeAttribute(action.arg);
    else if (action.type === 'remove-class') el.classList.remove(action.arg);
  };
  const run = () => {
    timer = 0;
    for (const rule of rules) {
      try { for (const el of pick(rule.selector)) act(el, rule.action); } catch {}
    }
  };
  let timer = 0;
  addEventListener('DOMContentLoaded', () => {
    run();
    new MutationObserver(() => { if (!timer) timer = setTimeout(run, 1000); }).observe(document.documentElement, {childList: true, subtree: true});
  });
})
"#;

// Forwards browser shortcuts from Riffle's own pages. Web pages never get this: see native_keys.
const KEY_SCRIPT: &str = r#"
if (window === top) addEventListener('keydown', e => {
  if (e.key === 'F12') { e.preventDefault(); return window.ipc.postMessage('dev_tools'); }
  if (!e.ctrlKey || e.altKey) return;
  const name = {A: 'tab_search', '+': 'zoom_in', '=': 'zoom_in', '-': 'zoom_out', _: 'zoom_out', 0: 'zoom_reset', t: 'new_tab', w: 'close_tab', l: 'focus_address', d: 'bookmark', h: 'history', j: 'downloads', T: 'reopen_tab', N: 'private_tab', O: 'bookmarks', I: 'dev_tools', u: 'view_source', Tab: e.shiftKey ? 'prev_tab' : 'next_tab'}[e.key];
  if (name) { e.preventDefault(); window.ipc.postMessage(name); }
}, true);
"#;

// Tells the browser what this page is playing, for the music dialog and so a playing tab never sleeps.
// shortcut: top frame only, so players inside iframes do not appear. Would need per-frame scripting.
const MEDIA_SCRIPT: &str = r#"
if (window === top) addEventListener('DOMContentLoaded', () => {
  const link = [...document.querySelectorAll('link[rel~="icon"]')].pop();
  if (link?.href) window.ipc.postMessage('icon:' + link.href);
});
if (window === top) addEventListener('load', () => setTimeout(() => {
  const paragraphs = [...document.querySelectorAll('article p, main p, [role=main] p, p')].map(p => p.textContent.trim().length).filter(n => n > 80);
  if (paragraphs.length >= 4 && paragraphs.reduce((a, b) => a + b, 0) > 1400) window.ipc.postMessage('article');
}, 800));
if (window === top) for (const name of ['play', 'pause', 'ended', 'volumechange']) addEventListener(name, () => {
  const m = [...document.querySelectorAll('video,audio')], el = m.find(x => !x.paused) || m[0];
  if (el) window.ipc.postMessage('media:' + JSON.stringify({playing: !el.paused, volume: el.muted ? 0 : el.volume, video: !!document.querySelector('video')}));
}, true);
"#;

// What this tab's navigation checks need, shared with handlers that must answer synchronously.
struct Guard {
    top_url: String, // the page showing now
    nav_url: String, // the navigation last allowed, which may never replace that page
    vouched_until: Instant,
    last_click: Option<(Instant, bool)>, // (trusted until, on a real link or button)
    blocked: u32,
    stopped: Option<String>, // the last pop-up or redirect stopped, which the shield offers to open
}

impl Guard {
    fn new() -> Rc<RefCell<Guard>> {
        Rc::new(RefCell::new(Guard { top_url: String::new(), nav_url: String::new(), vouched_until: Instant::now(), last_click: None, blocked: 0, stopped: None }))
    }
    fn vouch(&mut self) {
        self.vouched_until = Instant::now() + Duration::from_secs(VOUCH_SECS);
    }
    fn vouched(&self) -> bool {
        Instant::now() < self.vouched_until
    }
    fn recent_click(&self) -> Option<bool> {
        self.last_click.filter(|(until, _)| Instant::now() < *until).map(|(_, real)| real)
    }
}

enum Msg {
    DownloadStarted(u64, String, String),
    DownloadDone(u64, String, Option<String>, bool),
    IdleCheck,
    Key(u64, &'static str),
    Refresh,
    Toolbar(String),
    Shortcut(u64, String),
    Title(u64, String),
    Load(u64, bool, String),
    Popup(u64, String),
    Suggestions(u64, Vec<String>),
    DevtoolsScan,
    Fullscreen(u64, bool),
    UpdateReady(String),
    TabCrashed(u64),
    NavStarted(u64),
    NavFinished(u64, COREWEBVIEW2_WEB_ERROR_STATUS, bool),
    SlowCheck(u64, u64),
    Extensions(Vec<(String, String, bool)>),
    ExtensionAdded(bool),
    UpdateChecked(Option<Option<String>>),
    Processes(Vec<(u32, COREWEBVIEW2_PROCESS_KIND, Vec<String>)>),
}

struct Tab {
    id: u64,
    title: String,
    url: String,
    loading: bool,
    last_used: u64,
    private: bool,
    downloads: u32, // unfinished, which end if the tab sleeps
    icon: Option<String>, // the address the page declares for its icon
    media: Option<(bool, f64)>, // (playing, volume), set once the page has reported any
    last_seen: std::time::Instant,
    guard: Rc<RefCell<Guard>>,
    view: Option<WebView>, // None = asleep
    devtools: Option<isize>, // its DevTools window, docked inside the browser window
    zoom: f64, // kept here so it survives the tab sleeping
    pinned: bool, // stays left and awake, and comes back next time
    has_video: bool,
    readable: bool, // the page looks like an article, so reader mode can help
    crashed_url: Option<String>, // the page to go back to while an error page shows
    nav_seq: Option<u64>, // the navigation under way that has not finished yet
}

// A real floating window, so Windows gives it rounded corners and a shadow, and it can pass the browser's edges.
struct Popup {
    view: WebView, // declared first so it is dropped before its window
    window: Window,
}

impl std::ops::Deref for Popup {
    type Target = WebView;
    fn deref(&self) -> &WebView {
        &self.view
    }
}

impl Popup {
    fn place(&self, parent: &Window, area: Rect) {
        let scale = parent.scale_factor();
        let origin = parent.inner_position().unwrap_or_default();
        let at = area.position.to_physical::<i32>(scale);
        let size = area.size.to_logical::<f64>(scale);
        self.window.set_outer_position(PhysicalPosition::new(origin.x + at.x, origin.y + at.y));
        self.window.set_inner_size(LogicalSize::new(size.width, size.height));
        let _ = self.view.set_bounds(Rect { position: LogicalPosition::new(0.0, 0.0).into(), size: area.size });
    }
}

struct Browser {
    window: Window,
    context: WebContext,
    ui_context: Option<WebContext>, // agent mode only: keeps Riffle's own interface out of the agent's reach
    args: String,
    proxy: EventLoopProxy<Msg>,
    toolbar: Option<WebView>,
    tabs: Vec<Tab>,
    active: u64,
    fullscreen: bool, // the selected tab's video or element fills the screen
    update_ready: Option<String>, // the version that starts next time
    update_status: &'static str, // what the last check from Settings found
    next_id: u64,
    clock: u64,
    last_popup: std::time::Instant,
    data_dir: PathBuf,
    settings: Value,
    private_window: bool, // every tab in this window is private
    moved_window: bool, // made from a tab moved out of another window, which stays the one that restores and saves tabs
    adblock: bool, // fixed at startup, because web views must all share one set of browser args
    bookmarks: Vec<Value>,
    downloads: Vec<Value>,
    closed: Vec<String>,
    last_visit: (String, String),
    panel: Option<WebView>,
    panel_page: String,
    music: Option<Popup>,
    music_closed: std::time::Instant,
    menu: Option<Popup>,
    menu_closed: std::time::Instant,
    menu_developer: bool, // showing the Developer list
    devtools_share: f64,
    devtools_host: Option<isize>, // clips every tab's DevTools down to its content
    devtools_wanted: Option<(u64, u32, Instant)>, // (tab, retries used, since) after asking, until it has stayed open
    devtools_header: Option<WebView>, // Chromium's own close button is cut off with its title bar
    devtools_frame: (i32, i32, i32, i32), // the title bar and edges to clip: left, top, right, bottom
    divider_drag: bool,
    download_bubble: Option<Popup>,
    download_shown: Value,
    suggest: Option<Popup>,
    suggest_items: Vec<Value>,
    suggest_query: String,
    suggest_seq: u64, // answers to anything older are stale
    suggest_box: (f64, f64), // the address box's left edge and width
    shield: Option<Popup>,
    shield_closed: std::time::Instant,
    bookmark_dialog: Option<Popup>,
    bookmark_height: f64, // measured by the dialog, which grows to show an error
    zoom_popup: Option<Popup>,
    site_zoom: Value, // host to level, for sites not at 100%
    tab_menu: Option<(Popup, u64)>,
    marks_menu: Option<(Popup, usize)>, // the bookmarks that do not fit on the bar, from this index on
    tab_search: Option<Popup>,
    last_download: Option<Instant>,
    slow_note: Option<Popup>,
    nav_counter: u64,
    import_status: String,
    extension_status: String,
    bookmark_closed: std::time::Instant,
    bookmark_url: String, // the bookmark that dialog is editing, kept in case the tab moves on
    bookmark_added: bool, // just now, rather than opened to edit
    shield_height: f64, // measured by the dialog, since its text varies
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let has = |flag: &str| args.iter().any(|a| a == flag);
    // The file name works like a flag, so a copy named RiffleSetup.exe is an installer and uninstall.exe an uninstaller.
    let stem = std::env::current_exe().ok().and_then(|e| e.file_stem().map(|s| s.to_string_lossy().to_lowercase())).unwrap_or_default();
    if has("--install") || stem.contains("setup") {
        if let Err(e) = run_setup(false) {
            show_error(&format!("Could not start Riffle Setup: {e}"));
        }
        return;
    }
    if has("--uninstall") || stem == "uninstall" {
        // Never removes a folder this copy does not own.
        if !is_installed() {
            if !has("--silent") {
                show_error("This copy of Riffle is not the installed one, so there is nothing to remove.");
            }
        } else if has("--silent") {
            let _ = uninstall_app(false, |_, _| ()); // no dialogs at all, since nobody may be there to answer one
        } else if let Err(e) = run_setup(true) {
            show_error(&format!("Could not uninstall Riffle: {e}"));
        }
        return;
    }
    if let Err(e) = run() {
        show_error(&e.to_string());
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let agent_mode = std::env::args().any(|a| a == "--agent");
    let private_window = std::env::args().any(|a| a == "--private");
    let event_loop = EventLoopBuilder::<Msg>::with_user_event().build();
    let window = WindowBuilder::new()
        .with_title(if private_window { "Riffle Private" } else { "Riffle" })
        .with_decorations(false) // the tab row replaces the title bar
        .with_maximized(true) // fills whatever screen it opens on, and Windows keeps it clear of the taskbar
        .with_inner_size(LogicalSize::new(1200.0, 800.0)) // what the restore button returns to
        .with_min_inner_size(LogicalSize::new(360.0, 300.0))
        .with_window_icon(Some(Icon::from_rgba(logo_rgba(), LOGO_SIZE, LOGO_SIZE)?))
        .build(&event_loop)?;

    window.set_background_color(Some(WINDOW_COLOR));
    let data_dir = std::env::var_os("LOCALAPPDATA")
        .map(|d| std::path::PathBuf::from(d).join("Riffle"))
        .ok_or("LOCALAPPDATA is not set")?;
    std::fs::create_dir_all(&data_dir)?;
    let settings = sanitize_settings(&load_json(&data_dir.join("settings.json")));
    let adblock = settings["adblock"] == true;
    let agent_setting = settings["agent_mode"] == true;
    set_allowed_sites(&settings);
    if !private_window {
        compact_history(&data_dir.join("history.jsonl"));
    }
    if adblock {
        let _ = SEED_ENGINE.set(seed_engine());
        load_filters(data_dir.clone(), !private_window); // two windows must not download over each other
    }
    let site_zoom = if private_window { json!({}) } else { load_json(&data_dir.join("zoom.json")) };
    let mut browser = Browser {
        window,
        context: WebContext::new(Some(data_dir.clone())),
        // A second engine process, so it costs memory only while agent mode is on.
        ui_context: ((agent_mode || agent_setting) && std::env::var_os("RIFFLE_AGENT_SEE_UI").is_none()).then(|| WebContext::new(Some(data_dir.join("agent-ui")))),
        args: browser_args(agent_mode || agent_setting),
        proxy: event_loop.create_proxy(),
        toolbar: None,
        tabs: Vec::new(),
        active: 0,
        fullscreen: false,
        update_ready: None,
        update_status: "",
        next_id: 1,
        clock: 0,
        last_popup: std::time::Instant::now(),
        bookmarks: load_json(&data_dir.join("bookmarks.json")).as_array().cloned().unwrap_or_default(),
        downloads: load_downloads(&data_dir.join("downloads.json")),
        data_dir,
        settings,
        private_window,
        moved_window: std::env::args().any(|a| a == "--moved"),
        adblock,
        closed: Vec::new(),
        last_visit: Default::default(),
        panel: None,
        panel_page: String::new(),
        music: None,
        music_closed: std::time::Instant::now(),
        menu: None,
        menu_closed: std::time::Instant::now(),
        menu_developer: false,
        devtools_share: DEVTOOLS_SHARE,
        devtools_host: None,
        devtools_wanted: None,
        devtools_header: None,
        devtools_frame: (0, 0, 0, 0),
        divider_drag: false,
        download_bubble: None,
        download_shown: Value::Null,
        suggest: None,
        suggest_items: Vec::new(),
        suggest_query: String::new(),
        suggest_seq: 0,
        suggest_box: (0.0, 0.0),
        shield: None,
        shield_closed: std::time::Instant::now(),
        bookmark_dialog: None,
        bookmark_height: 220.0,
        zoom_popup: None,
        site_zoom,
        tab_menu: None,
        marks_menu: None,
        tab_search: None,
        last_download: None,
        slow_note: None,
        nav_counter: 0,
        import_status: String::new(),
        extension_status: String::new(),
        bookmark_closed: std::time::Instant::now(),
        bookmark_url: String::new(),
        bookmark_added: false,
        shield_height: 200.0,
    };
    browser.build_toolbar()?;
    let start = std::env::args().skip(1).find(|a| !a.starts_with("--")).and_then(|a| address_to_url(&a, browser.search()));
    browser.restore_session();
    match start {
        Some(url) => browser.open_tab(&url, false),
        None => match browser.tabs.last().map(|t| t.id) {
            Some(last) => browser.select(last),
            None => browser.open_tab(&browser.new_tab_url(), false),
        },
    }

    if !private_window {
        keep_updated(browser.data_dir.clone(), event_loop.create_proxy());
    }
    // Once ever, and not for an agent that has no one to read it.
    let welcomed = browser.data_dir.join("welcomed");
    if !private_window && !agent_mode && !browser.moved_window && std::fs::read_to_string(&welcomed).ok().as_deref() != Some(WELCOME_FOR) {
        let _ = std::fs::write(welcomed, WELCOME_FOR);
        browser.open_tab(WELCOME_URL, false);
    }

    let idle_proxy = event_loop.create_proxy();
    std::thread::spawn(move || {
        while idle_proxy.send_event(Msg::IdleCheck).is_ok() {
            std::thread::sleep(std::time::Duration::from_secs(IDLE_CHECK_SECS));
        }
    });

    let scan_proxy = event_loop.create_proxy();
    std::thread::spawn(move || {
        while scan_proxy.send_event(Msg::DevtoolsScan).is_ok() {
            std::thread::sleep(Duration::from_millis(DEVTOOLS_SCAN_MS));
        }
    });

    event_loop.run(move |event, target, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent { window_id, .. } if window_id != browser.window.id() => {}
            Event::WindowEvent { event: WindowEvent::Moved(_), .. } => {
                browser.music = None;
                browser.menu = None;
                browser.shield = None;
                browser.bookmark_dialog = None;
                browser.zoom_popup = None;
                browser.tab_menu = None;
                browser.marks_menu = None;
                browser.tab_search = None;
                browser.slow_note = None;
                browser.suggest = None;
                browser.download_bubble = None;
            }
            Event::WindowEvent { event: WindowEvent::CursorMoved { position, .. }, .. } => browser.move_divider(position.x, position.y),
            Event::WindowEvent { event: WindowEvent::MouseInput { state, button: MouseButton::Left, .. }, .. } => {
                browser.divider_drag = state == ElementState::Pressed && browser.window.cursor_position().is_ok_and(|p| {
                    let origin = browser.window.inner_position().unwrap_or_default();
                    browser.on_divider(p.x - origin.x as f64, p.y - origin.y as f64)
                });
            }
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                browser.save_session();
                *control_flow = ControlFlow::Exit;
            }
            // The other window (normal or private) may have changed them.
            // When the window itself holds the focus, such as after Alt+Tab, Esc lands here instead of in the page.
            Event::WindowEvent { event: WindowEvent::KeyboardInput { event, .. }, .. } => {
                if event.state == ElementState::Pressed && event.physical_key == KeyCode::Escape {
                    browser.leave_fullscreen();
                }
                // SAFETY: GetKeyState takes a plain value and keeps nothing.
                if event.state == ElementState::Pressed && !event.repeat && unsafe { GetKeyState(0x11) } < 0 {
                    let typed = event.key_without_modifiers();
                    let how = match (typed.to_text(), event.physical_key) {
                        (Some("=" | "+"), _) | (_, KeyCode::NumpadAdd) => "in",
                        (Some("-" | "_"), _) | (_, KeyCode::NumpadSubtract) => "out",
                        (Some("0"), _) | (_, KeyCode::Numpad0) => "reset",
                        _ => "",
                    };
                    if !how.is_empty() {
                        browser.zoom_by_key(how, target);
                    }
                }
            }
            Event::WindowEvent { event: WindowEvent::Focused(true), .. } => {
                if browser.fullscreen {
                    if let Some(view) = browser.active_view() {
                        let _ = view.focus();
                    }
                }
                browser.reload_bookmarks();
                browser.reload_settings();
                browser.sync_toolbar();
                if browser.panel_page == "settings" {
                    browser.render_panel(); // back from Windows Settings, the default browser may have changed
                }
            }
            Event::WindowEvent { event: WindowEvent::Resized(_), .. } => {
                browser.layout();
                browser.sync_toolbar(); // the maximize button changes to restore
            }
            Event::UserEvent(msg) => {
                if browser.handle(msg, target) {
                    browser.save_session();
                    *control_flow = ControlFlow::Exit;
                }
            }
            _ => {}
        }
    });
}

/// For the toolbar, dialogs and pages. In agent mode they run without the debugging port, so agents see only real tabs.
fn ui_view_builder<'a>(ui_context: &'a mut Option<WebContext>, context: &'a mut WebContext, args: &str) -> WebViewBuilder<'a> {
    match ui_context {
        Some(own) => WebViewBuilder::new_with_web_context(own).with_additional_browser_args(browser_args(false)),
        None => WebViewBuilder::new_with_web_context(context).with_additional_browser_args(args),
    }
}

fn browser_args(agent_mode: bool) -> String {
    // The first flags are wry's defaults, which setting custom args would otherwise drop.
    let mut args = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection".to_string();
    if agent_mode {
        // Off by default: this port gives local programs full control of the browser, including logged-in sites.
        args.push_str(&format!(" --remote-debugging-address=127.0.0.1 --remote-debugging-port={AGENT_PORT}"));
    }
    args
}

impl Browser {
    fn show_bookmarks_bar(&self) -> bool {
        self.settings["bookmarks_bar"] == true
    }

    fn bounds(&self, toolbar: bool) -> Rect {
        let size = self.window.inner_size().to_logical::<f64>(self.window.scale_factor());
        let top = if self.fullscreen { 0.0 } else { TOOLBAR_HEIGHT + if self.show_bookmarks_bar() { BOOKMARKS_BAR_HEIGHT } else { 0.0 } };
        let (y, height) = if toolbar { (0.0, top) } else { (top, (size.height - top).max(0.0)) };
        let docked = !toolbar && !self.fullscreen && self.tabs.iter().any(|t| t.id == self.active && t.devtools.is_some());
        let width = if docked { (size.width * (1.0 - self.devtools_share) - DEVTOOLS_DIVIDER).max(0.0) } else { size.width };
        Rect { position: LogicalPosition::new(0.0, y).into(), size: ViewSize::new(width, height).into() }
    }

    fn build_toolbar(&mut self) -> wry::Result<()> {
        let proxy = self.proxy.clone();
        let bounds = self.bounds(true);
        let view = ui_view_builder(&mut self.ui_context, &mut self.context, &self.args)
            .with_browser_extensions_enabled(true)
            .with_bounds(bounds)
            .with_html(TOOLBAR_HTML.replace("<!--ICONS-->", ICONS_HTML))
            .with_initialization_script(KEY_SCRIPT)
            .with_drag_drop_handler(|_| true) // a dropped link must not replace trusted UI with a web page
            .with_navigation_handler(is_built_in_page)
            .with_ipc_handler(move |req| drop(proxy.send_event(Msg::Toolbar(req.body().clone()))))
            .build_as_child(&self.window)?;
        self.toolbar = Some(view);
        Ok(())
    }

    fn music_items(&self) -> Vec<Value> {
        self.tabs
            .iter()
            .filter(|t| t.view.is_some())
            .filter_map(|t| {
                let site = Url::parse(&t.url).ok().and_then(|u| u.host_str().map(|h| h.trim_start_matches("www.").to_string())).unwrap_or_default();
                t.media.map(|(playing, volume)| json!({"id": t.id, "title": t.title, "site": site, "playing": playing, "volume": volume, "video": t.has_video}))
            })
            .collect()
    }

    fn music_bounds(&self) -> Rect {
        let size = self.window.inner_size().to_logical::<f64>(self.window.scale_factor());
        let rows = self.music_items().len();
        let system = if system_volume().is_some() { 97.0 } else { 0.0 }; // that row and its divider in music.html
        let wanted = system + if rows == 0 { 72.0 } else { rows as f64 * 117.0 };
        let height = wanted.min(400.0).min((size.height - NAV_ROW_BOTTOM).max(0.0));
        let width = MUSIC_WIDTH.min(size.width);
        let x = (size.width - self.toolbar_button_right("music") - width).max(0.0);
        Rect { position: LogicalPosition::new(x, NAV_ROW_BOTTOM).into(), size: ViewSize::new(width, height).into() }
    }

    fn build_popup(&mut self, target: &EventLoopWindowTarget<Msg>, html: &str, title: &str, area: Rect, focus: bool) -> Option<Popup> {
        let size = area.size.to_logical::<f64>(self.window.scale_factor());
        let built = WindowBuilder::new()
            .with_title(title)
            .with_decorations(false)
            .with_undecorated_shadow(true)
            .with_resizable(false)
            .with_skip_taskbar(true)
            .with_owner_window(self.window.hwnd() as _)
            .with_inner_size(LogicalSize::new(size.width, size.height))
            .with_visible(false)
            .build(target);
        let window = match built {
            Ok(window) => window,
            Err(e) => {
                show_error(&format!("Could not open this: {e}"));
                return None;
            }
        };
        round_corners(window.hwnd() as _);
        let proxy = self.proxy.clone();
        let built = ui_view_builder(&mut self.ui_context, &mut self.context, &self.args)
            .with_browser_extensions_enabled(true)
            .with_focused(focus) // on by default, which would pull typing out of the address bar
            .with_initialization_script(KEY_SCRIPT)
            .with_bounds(Rect { position: LogicalPosition::new(0.0, 0.0).into(), size: area.size })
            .with_html(html.replace("<!--ICONS-->", ICONS_HTML))
            .with_drag_drop_handler(|_| true)
            .with_navigation_handler(is_built_in_page)
            .with_ipc_handler(move |req| drop(proxy.send_event(Msg::Toolbar(req.body().clone()))))
            .build_as_child(&window);
        match built {
            Ok(view) => {
                let popup = Popup { view, window };
                popup.place(&self.window, area);
                if focus {
                    popup.window.set_visible(true);
                    popup.window.set_focus();
                    let _ = popup.view.focus();
                } else {
                    let hwnd = HWND(popup.window.hwnd() as _);
                    // SAFETY: a window this thread just made. Typing must stay in the address bar while this shows.
                    unsafe {
                        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, GetWindowLongPtrW(hwnd, GWL_EXSTYLE) | WS_EX_NOACTIVATE.0 as isize);
                        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                    }
                }
                Some(popup)
            }
            Err(e) => {
                show_error(&format!("Could not open this: {e}"));
                None
            }
        }
    }

    // The backup for early_scriptlets, in case the engine had not finished loading when the page was asked for.
    // shortcut: top frame only, so ads inside iframes keep their own scripts. Would need per-frame scripting.
    fn inject_scriptlets(&self, id: u64) {
        let (true, Some(engine)) = (self.adblock, ENGINE.get()) else { return };
        let Some(tab) = self.tabs.iter().find(|t| t.id == id) else { return };
        let Some(view) = &tab.view else { return };
        if site_is_allowed(&tab.url) {
            let _ = view.evaluate_script("window.__riffleAllow = true");
        } else if is_web(&tab.url) {
            let script = early_script(engine, &tab.url);
            if !script.is_empty() {
                let _ = view.evaluate_script(&script); // does nothing if early_scriptlets got there first
            }
        }
    }

    fn shield_bounds(&self) -> Rect {
        let size = self.window.inner_size().to_logical::<f64>(self.window.scale_factor());
        let width = SHIELD_WIDTH.min(size.width);
        let height = self.shield_height.min((size.height - NAV_ROW_BOTTOM).max(0.0));
        Rect { position: LogicalPosition::new((size.width - self.toolbar_button_right("shield") - width).max(0.0), NAV_ROW_BOTTOM).into(), size: ViewSize::new(width, height).into() }
    }

    fn toggle_shield(&mut self, target: &EventLoopWindowTarget<Msg>) {
        if self.shield.take().is_some() || self.shield_closed.elapsed().as_millis() < 300 {
            return;
        }
        self.shield = self.build_popup(target, SHIELD_HTML, "Riffle shield", self.shield_bounds(), true);
    }

    fn render_shield(&self) {
        let Some(shield) = &self.shield else { return };
        let Some(tab) = self.tabs.iter().find(|t| t.id == self.active) else { return };
        shield.place(&self.window, self.shield_bounds());
        let guard = tab.guard.borrow();
        let site = Url::parse(&tab.url).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default();
        let state = json!({"blocked": guard.blocked, "target": guard.stopped, "site": site, "on": !site_is_allowed(&tab.url), "popups": popups_are_allowed(&tab.url), "month": if self.private_window { 0 } else { blocked_this_month(&self.data_dir) }, "theme": self.settings["theme"], "private": tab.private});
        let _ = shield.evaluate_script(&format!("render({state})"));
    }

    fn toggle_music(&mut self, target: &EventLoopWindowTarget<Msg>) {
        if self.music.take().is_some() || self.music_closed.elapsed().as_millis() < 300 {
            return; // the click that blurred the dialog closed it, and must not reopen it
        }
        self.music = self.build_popup(target, MUSIC_HTML, "Riffle music", self.music_bounds(), true);
    }

    fn suggest_bounds(&self) -> Rect {
        let size = self.window.inner_size().to_logical::<f64>(self.window.scale_factor());
        let height = self.suggest_items.len() as f64 * 32.0 + 8.0; // rows and padding in suggest.html
        Rect { position: LogicalPosition::new(self.suggest_box.0, NAV_ROW_BOTTOM).into(), size: ViewSize::new(self.suggest_box.1, height.min((size.height - NAV_ROW_BOTTOM).max(0.0))).into() }
    }

    fn close_suggestions(&mut self) {
        self.suggest_seq += 1;
        self.suggest = None;
    }

    // History and bookmarks answer at once. Search words follow from the engine, unless this is private or an address.
    fn start_suggestions(&mut self, query: &str, target: &EventLoopWindowTarget<Msg>) {
        self.suggest_seq += 1;
        self.suggest_query = query.to_string();
        let history = parse_history(&read_lossy(&self.history_path()).unwrap_or_default());
        self.suggest_items = local_suggestions(query, &self.bookmarks, &history);
        self.show_suggestions(target);
        let private = self.tabs.iter().any(|t| t.id == self.active && t.private);
        if self.settings["search_suggest"] != true || private {
            return;
        }
        let Some(url) = suggest_url(self.search(), query) else { return };
        let (proxy, seq) = (self.proxy.clone(), self.suggest_seq);
        std::thread::spawn(move || drop(proxy.send_event(Msg::Suggestions(seq, fetch_suggestions(&url)))));
    }

    fn show_suggestions(&mut self, target: &EventLoopWindowTarget<Msg>) {
        if let Some(toolbar) = &self.toolbar {
            let _ = toolbar.evaluate_script(&format!("gotSuggestions({},{})", json!(self.suggest_query), json!(self.suggest_items)));
        }
        if self.suggest_items.is_empty() {
            self.suggest = None;
        } else if self.suggest.is_none() {
            self.suggest = self.build_popup(target, SUGGEST_HTML, "Riffle suggestions", self.suggest_bounds(), false);
        } else {
            self.render_suggestions();
        }
    }

    fn render_suggestions(&self) {
        let Some(suggest) = &self.suggest else { return };
        suggest.place(&self.window, self.suggest_bounds());
        let private = self.tabs.iter().any(|t| t.id == self.active && t.private);
        let _ = suggest.evaluate_script(&format!("render({})", json!({"items": self.suggest_items, "theme": self.settings["theme"], "private": private, "accent": self.settings["accent"]})));
    }

    fn menu_list(&self) -> &'static [(&'static str, &'static str, &'static str, &'static str)] {
        if self.menu_developer { DEVELOPER_MENU } else { MENU }
    }

    fn menu_bounds(&self) -> Rect {
        let size = self.window.inner_size().to_logical::<f64>(self.window.scale_factor());
        let list = self.menu_list();
        let dividers = list.iter().filter(|m| m.0.is_empty()).count() as f64;
        let rows = list.len() as f64 - dividers;
        let wanted = rows * 32.0 + dividers * 9.0 + 16.0; // rows, dividers and padding in menu.html
        let height = wanted.min((size.height - NAV_ROW_BOTTOM).max(0.0));
        let width = MENU_WIDTH.min(size.width);
        Rect { position: LogicalPosition::new((size.width - MENU_BUTTON_RIGHT - width).max(0.0), NAV_ROW_BOTTOM).into(), size: ViewSize::new(width, height).into() }
    }

    fn toggle_menu(&mut self, target: &EventLoopWindowTarget<Msg>) {
        if self.menu.take().is_some() || self.menu_closed.elapsed().as_millis() < 300 {
            return;
        }
        self.menu_developer = false;
        self.menu = self.build_popup(target, MENU_HTML, "Riffle menu", self.menu_bounds(), true);
    }

    fn render_menu(&self) {
        let Some(menu) = &self.menu else { return };
        let items: Vec<Value> = self
            .menu_list()
            .iter()
            .map(|(label, keys, icon, action)| json!({"label": label, "keys": keys, "icon": icon, "action": action, "disabled": *action == "reopen_tab" && self.closed.is_empty()}))
            .collect();
        let private = self.tabs.iter().any(|t| t.id == self.active && t.private);
        menu.place(&self.window, self.menu_bounds());
        let _ = menu.evaluate_script(&format!("render({})", json!({"items": items, "theme": self.settings["theme"], "private": private, "sub": self.menu_developer, "zoom": self.zoom_state(), "accent": self.settings["accent"]})));
    }

    fn pick_menu_item(&mut self, action: &str) {
        if action == "developer" || action == "menu_back" {
            self.menu_developer = action == "developer";
            return self.render_menu();
        }
        self.menu = None;
        self.menu_closed = std::time::Instant::now();
        match action {
            "cast" => cast_screen(),
            "print" => {
                if let Some(view) = self.active_view() {
                    let _ = view.print();
                }
            }
            _ if MENU.iter().chain(DEVELOPER_MENU).any(|m| m.3 == action) => drop(self.shortcut(action)),
            _ => {}
        }
    }

    fn render_music(&self) {
        let Some(music) = &self.music else { return };
        music.place(&self.window, self.music_bounds());
        let private = self.tabs.iter().any(|t| t.id == self.active && t.private);
        let system = system_volume().map(|(volume, muted)| json!({"volume": volume, "muted": muted}));
        let state = json!({"items": self.music_items(), "system": system, "theme": self.settings["theme"], "accent": self.settings["accent"], "private": private});
        let _ = music.evaluate_script(&format!("render({state})"));
    }

    fn sleep_idle_tabs(&mut self) {
        if self.settings["idle_sleep"] != true {
            return;
        }
        let limit = std::time::Duration::from_secs(IDLE_SLEEP_SECS);
        let mut changed = false;
        let active = self.active;
        for tab in self.tabs.iter_mut().filter(|t| t.id != active && t.view.is_some()) {
            if !must_stay_awake(tab) && tab.last_seen.elapsed() > limit {
                tab.view = None;
                tab.media = None;
                tab.loading = false;
                changed = true;
            }
        }
        if changed {
            self.sync_toolbar();
        }
    }

    fn open_panel(&mut self, page: &str) {
        self.download_bubble = None;
        self.music = None;
        self.menu = None;
        self.shield = None;
        self.bookmark_dialog = None;
        self.zoom_popup = None;
        self.tab_menu = None;
        self.marks_menu = None;
        self.tab_search = None;
        self.slow_note = None;
        self.panel_page = page.to_string();
        if self.panel.is_some() {
            self.render_panel();
            return;
        }
        let proxy = self.proxy.clone();
        let bounds = self.bounds(false);
        let built = ui_view_builder(&mut self.ui_context, &mut self.context, &self.args)
            .with_browser_extensions_enabled(true)
            .with_bounds(bounds)
            .with_html(PANEL_HTML.replace("<!--ICONS-->", ICONS_HTML))
            .with_initialization_script(KEY_SCRIPT)
            .with_drag_drop_handler(|_| true)
            .with_navigation_handler(is_built_in_page)
            .with_ipc_handler(move |req| drop(proxy.send_event(Msg::Toolbar(req.body().clone()))))
            .build_as_child(&self.window);
        match built {
            Ok(view) => {
                let _ = view.focus();
                self.panel = Some(view);
                if let Some(active) = self.active_view() {
                    let _ = active.set_visible(false);
                }
            }
            Err(e) => show_error(&format!("Could not open this page: {e}")),
        }
    }

    fn close_panel(&mut self) {
        if self.panel.take().is_some() {
            if let Some(active) = self.active_view() {
                let _ = active.set_visible(true);
                let _ = active.focus();
            }
        }
    }

    // Riffle's own list, because the engine's task manager window names itself after WebView2 and cannot be rebranded.
    fn request_processes(&self) {
        let Some(toolbar) = &self.toolbar else { return };
        let proxy = self.proxy.clone();
        // SAFETY: COM calls on the UI thread that owns this web view; returned strings are taken once by take_pwstr.
        unsafe {
            let handler = GetProcessExtendedInfosCompletedHandler::create(Box::new(move |_, list| {
                let Some(list) = list else { return Ok(()) };
                let mut count = 0;
                list.Count(&mut count)?;
                let mut found = Vec::new();
                for i in 0..count {
                    let item = list.GetValueAtIndex(i)?;
                    let info = item.ProcessInfo()?;
                    let (mut pid, mut kind) = (0, COREWEBVIEW2_PROCESS_KIND_UTILITY);
                    info.ProcessId(&mut pid)?;
                    info.Kind(&mut kind)?;
                    let frame_list = item.AssociatedFrameInfos()?; // named, as the iterator needs it to stay alive
                    let frames = frame_list.GetIterator()?;
                    let mut sources = Vec::new();
                    let mut more = BOOL(0);
                    frames.HasCurrent(&mut more)?;
                    while more.as_bool() {
                        let mut raw = PWSTR::null();
                        let frame = frames.GetCurrent()?;
                        frame.Source(&mut raw)?;
                        sources.push(take_pwstr(raw));
                        frames.MoveNext(&mut more)?;
                    }
                    found.push((pid as u32, kind, sources));
                }
                drop(proxy.send_event(Msg::Processes(found)));
                Ok(())
            }));
            let _ = self.engine_view().unwrap_or(toolbar).environment().cast::<ICoreWebView2Environment13>().map(|e| e.GetProcessExtendedInfos(&handler));
        }
    }

    fn render_processes(&self, list: Vec<(u32, COREWEBVIEW2_PROCESS_KIND, Vec<String>)>) {
        let Some(panel) = self.panel.as_ref().filter(|_| self.panel_page == "tasks") else { return };
        let mut rows = vec![json!({"name": "Riffle", "kind": "App", "bytes": process_memory(std::process::id())})];
        for (pid, kind, sources) in list {
            let (name, kind) = match kind {
                COREWEBVIEW2_PROCESS_KIND_BROWSER => ("Browser engine".to_string(), "Engine"),
                COREWEBVIEW2_PROCESS_KIND_GPU => ("Graphics".to_string(), "Engine"),
                COREWEBVIEW2_PROCESS_KIND_RENDERER => {
                    let titles: Vec<&str> = self.tabs.iter().filter(|t| sources.contains(&t.url)).map(|t| t.title.as_str()).collect();
                    let web = sources.iter().find(|s| is_web(s)).and_then(|s| site_of(s));
                    match (titles.is_empty(), web) {
                        (false, _) => (titles.join(", "), "Tab"),
                        (true, Some(site)) => (site, "Tab"),
                        (true, None) => ("Riffle interface".to_string(), "App"),
                    }
                }
                _ => ("Background service".to_string(), "Engine"),
            };
            rows.push(json!({"name": name, "kind": kind, "bytes": process_memory(pid)}));
        }
        rows.sort_by_key(|r| std::cmp::Reverse(r["bytes"].as_u64()));
        for tab in self.tabs.iter().filter(|t| t.view.is_none()) {
            rows.push(json!({"name": tab.title, "kind": "Asleep", "bytes": 0}));
        }
        let _ = panel.evaluate_script(&format!("renderTasks({})", Value::Array(rows)));
    }

    /// A web view in the engine that runs the tabs, which in agent mode is not the toolbar's.
    fn engine_view(&self) -> Option<&WebView> {
        if self.ui_context.is_some() { self.tabs.iter().find_map(|t| t.view.as_ref()) } else { self.toolbar.as_ref() }
    }

    fn extension_profile(&self) -> Option<ICoreWebView2Profile7> {
        // SAFETY: COM calls on the UI thread that owns this web view.
        unsafe { self.engine_view()?.webview().cast::<ICoreWebView2_13>().ok()?.Profile().ok()?.cast().ok() }
    }

    fn request_extensions(&self) {
        let Some(profile) = self.extension_profile() else { return };
        let proxy = self.proxy.clone();
        // SAFETY: COM calls on the UI thread; returned strings are taken once by take_pwstr.
        unsafe {
            let handler = ProfileGetBrowserExtensionsCompletedHandler::create(Box::new(move |_, list| {
                let Some(list) = list else { return Ok(()) };
                let mut count = 0;
                list.Count(&mut count)?;
                let mut rows = Vec::new();
                for i in 0..count {
                    let item = list.GetValueAtIndex(i)?;
                    let (mut id, mut name, mut on) = (PWSTR::null(), PWSTR::null(), BOOL(0));
                    item.Id(&mut id)?;
                    item.Name(&mut name)?;
                    item.IsEnabled(&mut on)?;
                    let name = take_pwstr(name);
                    // Parts of the engine, such as its PDF viewer, which removing would break.
                    if !name.starts_with("Microsoft ") {
                        rows.push((take_pwstr(id), name, on.as_bool()));
                    }
                }
                drop(proxy.send_event(Msg::Extensions(rows)));
                Ok(())
            }));
            let _ = profile.GetBrowserExtensions(&handler);
        }
    }

    fn add_extension(&self, folder: &Path) {
        let Some(profile) = self.extension_profile() else { return };
        let proxy = self.proxy.clone();
        let path = windows::core::HSTRING::from(folder.as_os_str());
        // SAFETY: a COM call on the UI thread, with a string that outlives the call.
        unsafe {
            let handler = ProfileAddBrowserExtensionCompletedHandler::create(Box::new(move |result, _| {
                drop(proxy.send_event(Msg::ExtensionAdded(result.is_ok())));
                Ok(())
            }));
            if profile.AddBrowserExtension(&path, &handler).is_err() {
                drop(self.proxy.send_event(Msg::ExtensionAdded(false)));
            }
        }
    }

    fn change_extension(&self, wanted: String, remove: bool, on: bool) {
        let Some(profile) = self.extension_profile() else { return };
        let proxy = self.proxy.clone();
        // SAFETY: COM calls on the UI thread; returned strings are taken once by take_pwstr.
        unsafe {
            let handler = ProfileGetBrowserExtensionsCompletedHandler::create(Box::new(move |_, list| {
                let Some(list) = list else { return Ok(()) };
                let mut count = 0;
                list.Count(&mut count)?;
                for i in 0..count {
                    let item = list.GetValueAtIndex(i)?;
                    let mut id = PWSTR::null();
                    item.Id(&mut id)?;
                    if take_pwstr(id) == wanted {
                        let done = proxy.clone();
                        if remove {
                            item.Remove(&BrowserExtensionRemoveCompletedHandler::create(Box::new(move |_| {
                                drop(done.send_event(Msg::Toolbar(json!({"cmd": "ext_refresh"}).to_string())));
                                Ok(())
                            })))?;
                        } else {
                            item.Enable(on, &BrowserExtensionEnableCompletedHandler::create(Box::new(move |_| {
                                drop(done.send_event(Msg::Toolbar(json!({"cmd": "ext_refresh"}).to_string())));
                                Ok(())
                            })))?;
                        }
                    }
                }
                Ok(())
            }));
            let _ = profile.GetBrowserExtensions(&handler);
        }
    }

    fn render_panel(&self) {
        let Some(panel) = &self.panel else { return };
        let mut history = parse_history(&read_lossy(&self.history_path()).unwrap_or_default());
        history.reverse();
        let state = json!({"page": self.panel_page, "bookmarks": self.bookmarks, "history": history, "settings": self.settings, "downloads": self.downloads.iter().rev().collect::<Vec<_>>(), "installed": is_installed(), "version": env!("CARGO_PKG_VERSION"), "update_ready": self.update_ready, "update_status": self.update_status, "import_status": self.import_status, "engine_version": wry::webview_version().ok(), "default_browser": self.panel_page == "settings" && is_default_browser()});
        let _ = panel.evaluate_script(&format!("render({state})"));
    }

    fn history_path(&self) -> PathBuf {
        self.data_dir.join("history.jsonl")
    }

    fn search(&self) -> &str {
        self.settings["search"].as_str().unwrap_or(SEARCH_URL)
    }

    fn new_tab_url(&self) -> String {
        match self.settings["home"].as_str() {
            Some(home) if !home.is_empty() => home.to_string(),
            _ => "about:blank".to_string(),
        }
    }

    fn restore_session(&mut self) {
        if self.private_window || self.moved_window {
            return;
        }
        let restore = self.settings["restore"] == true;
        let saved = load_json(&self.data_dir.join("session.json"));
        for entry in saved.as_array().into_iter().flatten() {
            let pinned = entry["pinned"] == true;
            if !restore && !pinned {
                continue; // pinned tabs always come back
            }
            let Some(url) = entry["url"].as_str().filter(|u| is_web(u)) else { continue };
            let title = entry["title"].as_str().unwrap_or(url).to_string();
            let id = self.next_id;
            self.next_id += 1;
            // Left asleep: only the selected tab loads, so restoring many tabs stays fast.
            self.tabs.push(Tab { id, title, url: url.to_string(), loading: false, last_used: 0, private: false, downloads: 0, icon: None, media: None, last_seen: std::time::Instant::now(), guard: Guard::new(), view: None, devtools: None, zoom: 1.0, pinned, has_video: false, readable: false, crashed_url: None, nav_seq: None });
        }
    }

    fn save_session(&self) {
        if self.private_window {
            return; // private tabs are never reopened, and the normal window's session stays untouched
        }
        save_blocked_count(&self.data_dir);
        if self.moved_window {
            return;
        }
        let path = self.data_dir.join("session.json");
        let restore = self.settings["restore"] == true;
        let tabs: Vec<Value> = self.tabs.iter().filter(|t| !t.private && is_web(&t.url) && (restore || t.pinned)).map(|t| json!({"url": t.url, "title": t.title, "pinned": t.pinned})).collect();
        if tabs.is_empty() {
            let _ = std::fs::remove_file(path);
        } else {
            save_json(&path, &json!(tabs));
        }
    }

    fn vouch_active(&self) {
        if let Some(tab) = self.tabs.iter().find(|t| t.id == self.active) {
            tab.guard.borrow_mut().vouch();
        }
    }

    // The other window may have changed these. A private window keeps its own, which include exceptions never saved.
    fn reload_settings(&mut self) {
        if !self.private_window {
            self.settings = sanitize_settings(&load_json(&self.data_dir.join("settings.json")));
            set_allowed_sites(&self.settings);
        }
    }

    // What goes to disk: a private window's site exceptions must stay out of it.
    fn save_settings(&self) {
        let mut saved = self.settings.clone();
        if self.private_window {
            let disk = sanitize_settings(&load_json(&self.data_dir.join("settings.json")));
            saved["allow_sites"] = disk["allow_sites"].clone();
            saved["allow_popups"] = disk["allow_popups"].clone();
        }
        save_json(&self.data_dir.join("settings.json"), &saved);
    }

    fn reload_downloads(&mut self) {
        self.downloads = load_json(&self.data_dir.join("downloads.json")).as_array().cloned().unwrap_or_default();
    }

    fn reload_bookmarks(&mut self) {
        self.bookmarks = load_json(&self.data_dir.join("bookmarks.json")).as_array().cloned().unwrap_or_default();
    }

    // A separate process, like other browsers' private windows, sharing the same profile and browser args.
    // A web view belongs to its window, so the page reopens there: logins stay, while a playing video or unsaved typing does not.
    fn move_tab_to_new_window(&mut self, id: u64) -> bool {
        let Some(tab) = self.tabs.iter().find(|t| t.id == id).filter(|_| self.tabs.len() > 1) else { return false };
        let opened = std::env::current_exe().and_then(|exe| {
            let mut command = std::process::Command::new(exe);
            command.arg("--moved");
            if tab.private {
                command.arg("--private");
            }
            if self.args.contains("--remote-debugging-port") {
                command.arg("--agent");
            }
            if is_web(&tab.url) {
                command.arg(&tab.url);
            }
            command.spawn()
        });
        match opened {
            Ok(_) => self.close_tab(id),
            Err(e) => {
                show_error(&format!("Could not open a new window: {e}"));
                false
            }
        }
    }

    // Like a dragged-out tab's window: it doesn't take over the saved session.
    fn open_new_window(&self) {
        let opened = std::env::current_exe().and_then(|exe| {
            let mut command = std::process::Command::new(exe);
            command.arg("--moved");
            if self.args.contains("--remote-debugging-port") {
                command.arg("--agent");
            }
            command.spawn()
        });
        if let Err(e) = opened {
            show_error(&format!("Could not open a new window: {e}"));
        }
    }

    fn open_private_window(&self) {
        let opened = std::env::current_exe().and_then(|exe| {
            let mut command = std::process::Command::new(exe);
            command.arg("--private");
            if self.args.contains("--remote-debugging-port") {
                command.arg("--agent");
            }
            command.spawn()
        });
        if let Err(e) = opened {
            show_error(&format!("Could not open a private window: {e}"));
        }
    }

    fn save_downloads(&self) {
        save_json(&self.data_dir.join("downloads.json"), &json!(self.downloads));
        self.render_panel();
    }

    fn toggle_bookmark(&mut self) {
        self.reload_bookmarks();
        let Some(tab) = self.tabs.iter().find(|t| t.id == self.active) else { return };
        if !is_web(&tab.url) {
            return;
        }
        // The dialog is where it gets renamed or removed, so the star never removes anything by itself.
        let url = tab.url.clone();
        let added = !self.bookmarks.iter().any(|b| b["url"] == url.as_str());
        if added {
            let icon = if tab.private { None } else { tab.icon.clone() };
            let title = if tab.title.trim().is_empty() { url.as_str() } else { tab.title.as_str() };
            self.bookmarks.push(json!({"url": url, "title": title, "icon": icon}));
            self.save_bookmarks();
            self.layout();
            self.sync_toolbar();
        }
        self.bookmark_url = url;
        self.bookmark_added = added;
        // Through the event loop, because this can start from a key press, which has no window target to hand.
        drop(self.proxy.send_event(Msg::Toolbar(json!({"cmd": "bookmark_dialog"}).to_string())));
    }

    fn zoom_bounds(&self) -> Rect {
        let size = self.window.inner_size().to_logical::<f64>(self.window.scale_factor());
        let width = ZOOM_POPUP.0.min(size.width);
        Rect { position: LogicalPosition::new((size.width - self.address_end_right() - self.address_icons_before_zoom() - width).max(0.0), NAV_ROW_BOTTOM).into(), size: ViewSize::new(width, ZOOM_POPUP.1).into() }
    }

    /// Whether the blocker has something to say about this page: it stopped something, or it is turned off here.
    fn show_shield_icon(&self) -> bool {
        self.adblock && self.icon_choice("shield", self.tabs.iter().find(|t| t.id == self.active).is_some_and(|t| {
            let guard = t.guard.borrow();
            is_web(&t.url) && (guard.blocked > 0 || guard.stopped.is_some() || site_is_allowed(&t.url))
        }))
    }

    /// What the user chose in Settings, with "needed" deciding for "only when needed".
    fn icon_choice(&self, name: &str, needed: bool) -> bool {
        match self.settings["toolbar"][name].as_str().unwrap_or("show") {
            "hide" => false,
            "auto" => needed,
            _ => true,
        }
    }

    fn show_reader_icon(&self) -> bool {
        self.tabs.iter().find(|t| t.id == self.active).is_some_and(|t| is_web(&t.url) && self.icon_choice("reader", t.readable))
    }

    // From the right: menu, music, private, shield, downloads, extensions.
    fn toolbar_button_shown(&self, name: &str) -> bool {
        match name {
            "menu" => true,
            "shield" => self.show_shield_icon(),
            "downloads" => self.show_download_icon(),
            "extensions" => !self.private_window && self.icon_choice("extensions", true),
            other => self.icon_choice(other, true),
        }
    }

    fn toolbar_button_right(&self, name: &str) -> f64 {
        let before = ["menu", "music", "private", "shield", "downloads", "extensions"].iter().take_while(|n| **n != name).filter(|n| self.toolbar_button_shown(n)).count();
        8.0 + before as f64 * TOOLBAR_BUTTON
    }

    fn address_end_right(&self) -> f64 {
        self.toolbar_button_right("") + 4.0 - TOOLBAR_BUTTON * 0.0 - 2.0
    }

    fn address_icons_before_zoom(&self) -> f64 {
        (if self.icon_choice("star", true) { ADDRESS_ICON } else { 0.0 }) + (if self.show_reader_icon() { ADDRESS_ICON } else { 0.0 })
    }

    fn show_download_icon(&self) -> bool {
        self.icon_choice("downloads", self.tabs.iter().any(|t| t.downloads > 0) || self.last_download.is_some_and(|at| at.elapsed().as_secs() < DOWNLOAD_ICON_SECS))
    }

    fn active_zoom(&self) -> f64 {
        self.tabs.iter().find(|t| t.id == self.active).map_or(1.0, |t| t.zoom)
    }

    fn change_zoom(&mut self, how: &str) {
        let Some(tab) = self.tabs.iter_mut().find(|t| t.id == self.active) else { return };
        tab.zoom = next_zoom(tab.zoom, how);
        let (zoom, host) = (tab.zoom, Url::parse(&tab.url).ok().and_then(|u| u.host_str().map(str::to_string)));
        if let Some(view) = &tab.view {
            set_view_zoom(view, zoom);
        }
        // The whole site follows, as in Chrome and Edge. A private window remembers nothing.
        if let Some(host) = host {
            for other in self.tabs.iter_mut().filter(|t| Url::parse(&t.url).ok().is_some_and(|u| u.host_str() == Some(&host))) {
                other.zoom = zoom;
                if let Some(view) = &other.view {
                    set_view_zoom(view, zoom);
                }
            }
            if !self.private_window {
                if let Some(map) = self.site_zoom.as_object_mut() {
                    if zoom == 1.0 { map.remove(&host); } else { map.insert(host, json!(zoom)); }
                } else {
                    self.site_zoom = json!({host: zoom});
                }
                save_json(&self.data_dir.join("zoom.json"), &self.site_zoom);
            }
        }
        self.render_zoom();
        self.sync_toolbar();
    }

    fn zoom_by_key(&mut self, how: &str, target: &EventLoopWindowTarget<Msg>) {
        self.change_zoom(how);
        if self.zoom_popup.is_none() && !self.fullscreen {
            self.zoom_popup = self.build_popup(target, ZOOM_HTML, "Riffle zoom", self.zoom_bounds(), false);
        }
    }

    fn zoom_state(&self) -> Value {
        let zoom = self.active_zoom();
        json!({"percent": (zoom * 100.0).round() as u32, "can_in": next_zoom(zoom, "in") != zoom, "can_out": next_zoom(zoom, "out") != zoom, "theme": self.settings["theme"], "private": self.tabs.iter().any(|t| t.id == self.active && t.private)})
    }

    fn render_zoom(&self) {
        if let Some(popup) = &self.zoom_popup {
            let _ = popup.evaluate_script(&format!("render({})", self.zoom_state()));
        }
        if self.menu.is_some() {
            self.render_menu();
        }
    }

    fn bookmark_bounds(&self) -> Rect {
        let size = self.window.inner_size().to_logical::<f64>(self.window.scale_factor());
        let width = BOOKMARK_WIDTH.min(size.width);
        Rect { position: LogicalPosition::new((size.width - self.address_end_right() - width).max(0.0), NAV_ROW_BOTTOM).into(), size: ViewSize::new(width, self.bookmark_height).into() }
    }

    fn save_bookmarks(&self) {
        save_json(&self.data_dir.join("bookmarks.json"), &json!(self.bookmarks));
    }

    // Picks up the current address too, because single-page sites change it without a page load.
    // shortcut: a site that changes neither title nor page is missed, enough for history in practice.
    fn note_visit(&mut self, id: u64) {
        let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) else { return };
        // These are built-in pages, which the engine sees as a blank address. The tab keeps what it really is.
        if tab.url == WELCOME_URL || tab.crashed_url.is_some() {
            return;
        }
        if let Some(Ok(url)) = tab.view.as_ref().map(|v| v.url()) {
            if !url.is_empty() {
                tab.url = url;
            }
        }
        let visit = (tab.url.clone(), tab.title.clone());
        if tab.private || !is_web(&tab.url) || self.last_visit == visit {
            return;
        }
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let line = json!({"url": visit.0, "title": visit.1, "time": time});
        self.last_visit = visit;
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(self.history_path()) {
            let _ = writeln!(file, "{line}"); // history is best effort: never interrupt browsing for it
        }
    }

    fn set_fullscreen(&mut self, on: bool) {
        if self.fullscreen == on {
            return;
        }
        self.fullscreen = on;
        self.music = None;
        self.menu = None;
        self.shield = None;
        self.bookmark_dialog = None;
        self.zoom_popup = None;
        self.tab_menu = None;
        self.marks_menu = None;
        self.tab_search = None;
        self.slow_note = None;
        self.suggest = None;
        self.download_bubble = None;
        if let Some(toolbar) = &self.toolbar {
            let _ = toolbar.set_visible(!on);
        }
        // Black first, so no grey edge shows around the video while the window grows.
        self.window.set_background_color(Some(if on { (0, 0, 0, 0xff) } else { WINDOW_COLOR }));
        self.window.set_fullscreen(on.then_some(Fullscreen::Borderless(None)));
        self.layout();
        if let Some(view) = self.active_view() {
            let _ = view.focus(); // so Esc reaches the page
        }
    }

    // For when the browser itself moves on, such as to another tab: the page has to be told too.
    fn leave_fullscreen(&mut self) {
        if self.fullscreen {
            if let Some(view) = self.active_view() {
                let _ = view.evaluate_script("document.exitFullscreen()");
            }
            self.set_fullscreen(false);
        }
    }

    fn shortcut(&mut self, name: &str) -> bool {
        self.leave_fullscreen(); // each of these needs the browser's own window back
        match name {
            "new_tab" => self.open_tab(&self.new_tab_url(), false),
            "new_window" => self.open_new_window(),
            "private_tab" => self.open_private_window(),
            "reopen_tab" => {
                if let Some(url) = self.closed.pop() {
                    self.open_tab(&url, false);
                }
            }
            "close_tab" => return self.close_tab(self.active),
            "focus_address" => self.focus_address(),
            "next_tab" => self.step_tab(true),
            "prev_tab" => self.step_tab(false),
            "bookmark" => self.toggle_bookmark(),
            "view_source" => {
                if let Some(tab) = self.tabs.iter().find(|t| t.id == self.active && is_web(&t.url)) {
                    let (url, private) = (format!("view-source:{}", tab.url), tab.private);
                    self.open_tab(&url, private);
                }
            }
            "dev_tools" => {
                self.close_panel();
                self.devtools_wanted = None;
                let mut opened = None;
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == self.active) {
                    match tab.devtools.take() {
                        // SAFETY: only asks the window to close, and fails harmlessly if it already has.
                        Some(hwnd) => close_docked_devtools(HWND(hwnd as _)),
                        None => {
                            if let Some(view) = &tab.view {
                                arm_devtools_watch();
                                view.open_devtools();
                                opened = Some(tab.id);
                                let proxy = self.proxy.clone();
                                std::thread::spawn(move || {
                                    for _ in 0..100 {
                                        drop(proxy.send_event(Msg::DevtoolsScan));
                                        std::thread::sleep(Duration::from_millis(DEVTOOLS_BURST_MS));
                                    }
                                });
                            }
                        }
                    }
                }
                self.devtools_wanted = opened.map(|id| (id, 0, Instant::now()));
                self.sync_devtools_header();
                self.layout();
            }
            "task_manager" => self.open_panel("tasks"),
            "extensions" => self.open_panel("extensions"),
            "picture_in_picture" => {
                if let Some(view) = self.active_view() {
                    toggle_picture_in_picture(view);
                }
            }
            "ad_blocker" => drop(self.proxy.send_event(Msg::Toolbar(json!({"cmd": "shield"}).to_string()))),
            "music_dialog" => drop(self.proxy.send_event(Msg::Toolbar(json!({"cmd": "music"}).to_string()))),
            "tab_search" => drop(self.proxy.send_event(Msg::Toolbar(json!({"cmd": "tab_search"}).to_string()))),
            "reader" => {
                if let Some(view) = self.active_view() {
                    let _ = view.evaluate_script(READER_JS);
                }
            }
            "bookmarks" | "history" | "downloads" | "settings" => self.open_panel(name),
            _ => {}
        }
        false
    }

    // The page reports its class names and ids, and gets back the filter lists' rules for hiding ads.
    fn hide_ads(&self, id: u64, report: &str) {
        let (Some(engine), Ok(report)) = (ENGINE.get(), serde_json::from_str::<Value>(report)) else { return };
        let Some(view) = self.tabs.iter().find(|t| t.id == id).and_then(|t| t.view.as_ref()) else { return };
        let Ok(url) = view.url() else { return }; // from the browser, never the page's own word for it
        if !is_web(&url) || site_is_allowed(&url) {
            return;
        }
        let names = |key: &str| report[key].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect::<Vec<_>>();
        let css = ad_css(engine, &url, report["first"] == true, &names("classes"), &names("ids"));
        if !css.is_empty() {
            let _ = view.evaluate_script(&format!("document.documentElement.appendChild(Object.assign(document.createElement('style'),{{textContent:{}}}))", json!(css)));
        }
    }

    fn build_view(&mut self, id: u64, url: &str, private: bool, guard: Rc<RefCell<Guard>>) -> wry::Result<WebView> {
        let (shortcut, title, load, popup) = (self.proxy.clone(), self.proxy.clone(), self.proxy.clone(), self.proxy.clone());
        let bounds = self.bounds(false);
        guard.borrow_mut().vouch(); // this tab opening its address is always what was asked for
        let (nav_guard, click_guard, popup_guard, request_guard) = (guard.clone(), guard.clone(), guard.clone(), guard);
        let (nav_refresh, popup_refresh) = (self.proxy.clone(), self.proxy.clone());
        let filtering = self.adblock;
        let (started, finished) = (self.proxy.clone(), self.proxy.clone());
        let owner = self.window.hwnd() as isize;
        let nav_started = self.proxy.clone();
        let mut builder = WebViewBuilder::new_with_web_context(&mut self.context)
            .with_additional_browser_args(self.args.clone())
            .with_browser_extensions_enabled(true)
            .with_bounds(bounds)
            .with_incognito(private)
            .with_devtools(true)
            // Private downloads still save the file, but stay off the list.
            .with_download_started_handler(move |url, path| {
                if !choose_download_path(owner, path) {
                    return false;
                }
                if !private {
                    drop(started.send_event(Msg::DownloadStarted(id, url, path.display().to_string())));
                }
                true
            })
            .with_download_completed_handler(move |url, path, ok| {
                if !private {
                    drop(finished.send_event(Msg::DownloadDone(id, url, path.map(|p| p.display().to_string()), ok)));
                }
            })
            .with_navigation_handler(move |url| {
                let mut guard = nav_guard.borrow_mut();
                if !filtering || !is_web(&url) || navigation_allowed(&guard, &url) {
                    if guard.stopped.as_deref() == Some(url.as_str()) {
                        guard.stopped = None;
                    }
                    guard.nav_url = url;
                    drop(nav_started.send_event(Msg::NavStarted(id)));
                    return true;
                }
                guard.blocked += 1;
                guard.stopped = Some(url);
                drop(nav_refresh.send_event(Msg::Refresh));
                false
            })
            .with_initialization_script(MEDIA_SCRIPT);
        builder = if url == WELCOME_URL {
            builder.with_html(
                WELCOME_HTML
                    .replace("__THEME__", self.settings["theme"].as_str().unwrap_or("system"))
                    .replace("__ACCENT__", self.settings["accent"].as_str().unwrap_or(DEFAULT_ACCENT))
                    .replace("__VERSION__", env!("CARGO_PKG_VERSION")),
            )
        } else if url == "about:blank" {
            // Theme and accent are sanitized settings, so they are safe to place in the page.
            builder.with_html(
                NEW_TAB_HTML
                    .replace("__THEME__", self.settings["theme"].as_str().unwrap_or("system"))
                    .replace("__ACCENT__", self.settings["accent"].as_str().unwrap_or(DEFAULT_ACCENT))
                    .replace("__PRIVATE__", if private { "true" } else { "false" })
                    // Escaped, because titles come from web pages and this lands inside a script block.
                    .replace("__MARKS__", &if private { "[]".to_string() } else { json!(self.bookmarks.iter().take(8).map(|b| json!({"title": b["title"], "url": b["url"]})).collect::<Vec<_>>()).to_string().replace('<', "\\u003c") })
                    .replace("__BLOCKED__", &if private { 0 } else { blocked_this_month(&self.data_dir) }.to_string()),
            )
        } else {
            builder.with_url(url)
        };
        if self.adblock {
            builder = builder.with_initialization_script(AD_SCRIPT);
        }
        if self.settings["theme"] == "dark" {
            builder = builder.with_background_color((0x2d, 0x2d, 0x2d, 0xff)); // no white flash before a page paints
        }
        let adblock = self.adblock;
        let keys = self.proxy.clone();
        let watch = self.proxy.clone();
        let request_refresh = self.proxy.clone();
        let crashes = self.proxy.clone();
        let nav_finished = self.proxy.clone();
        let fullscreen = self.proxy.clone();
        builder
            .with_ipc_handler(move |req| {
                // Answered here rather than through the event loop, so it arrives before the navigation it explains.
                match req.body().as_str() {
                    "click:1" => click_guard.borrow_mut().last_click = Some((Instant::now() + Duration::from_secs(TRUST_SECS), true)),
                    "click:0" => click_guard.borrow_mut().last_click = Some((Instant::now() + Duration::from_secs(TRUST_SECS), false)),
                    // Choosing from the right-click menu takes longer than a click.
                    "click:menu" => click_guard.borrow_mut().last_click = Some((Instant::now() + Duration::from_secs(MENU_TRUST_SECS), true)),
                    body => drop(shortcut.send_event(Msg::Shortcut(id, body.to_string()))),
                }
            })
            .with_document_title_changed_handler(move |t| drop(title.send_event(Msg::Title(id, t))))
            .with_on_page_load_handler(move |event, url| {
                drop(load.send_event(Msg::Load(id, matches!(event, PageLoadEvent::Started), url)))
            })
            .with_new_window_req_handler(move |url, _| {
                let mut guard = popup_guard.borrow_mut();
                if !filtering || popup_allowed(&guard, &url) {
                    drop(popup.send_event(Msg::Popup(id, url)));
                } else {
                    guard.blocked += 1;
                    guard.stopped = Some(url);
                    drop(popup_refresh.send_event(Msg::Refresh));
                }
                NewWindowResponse::Deny
            })
            .build_as_child(&self.window)
            .inspect(|view| {
                native_keys(view, id, keys);
                report_crashes(view, id, crashes);
                report_navigation_results(view, id, nav_finished);
                watch_devtools_windows(view, watch);
                report_fullscreen(view, id, fullscreen);
                if adblock {
                    block_requests(view, request_guard, request_refresh);
                    early_scriptlets(view);
                }
            })
    }

    fn open_tab(&mut self, url: &str, private: bool) {
        let private = private || self.private_window;
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.push(Tab {
            id,
            title: if url == WELCOME_URL { "Welcome to Riffle".to_string() } else { "New tab".to_string() },
            url: url.to_string(),
            loading: false,
            last_used: 0,
            private,
            downloads: 0,
            icon: None,
            media: None,
            last_seen: std::time::Instant::now(),
            guard: Guard::new(),
            view: None,
            devtools: None,
            zoom: 1.0,
            pinned: false,
            has_video: false,
            readable: false,
            crashed_url: None,
            nav_seq: None,
        });
        self.select(id);
    }

    fn select(&mut self, id: u64) {
        let Some(index) = self.tabs.iter().position(|t| t.id == id) else { return };
        self.leave_fullscreen();
        self.panel = None;
        self.music = None;
        self.menu = None;
        self.shield = None;
        self.bookmark_dialog = None;
        self.zoom_popup = None;
        self.tab_menu = None;
        self.marks_menu = None;
        self.tab_search = None;
        self.slow_note = None;
        let previous = self.active;
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == previous) {
            tab.last_seen = std::time::Instant::now();
        }
        if self.tabs[index].view.is_none() {
            let url = self.tabs[index].url.clone();
            match self.build_view(id, &url, self.tabs[index].private, self.tabs[index].guard.clone()) {
                Ok(view) => {
                    set_view_zoom(&view, self.tabs[index].zoom);
                    self.tabs[index].view = Some(view);
                }
                Err(e) => {
                    show_error(&format!("Could not open this tab: {e}"));
                    return;
                }
            }
        }
        self.active = id;
        self.clock += 1;
        self.tabs[index].last_used = self.clock;
        for tab in &self.tabs {
            if let Some(view) = &tab.view {
                let _ = view.set_visible(tab.id == id);
                let _ = view.set_memory_usage_level(if tab.id == id { MemoryUsageLevel::Normal } else { MemoryUsageLevel::Low });
            }
        }
        let live: Vec<(u64, u64)> = self.tabs.iter().filter(|t| t.view.is_some() && !must_stay_awake(t)).map(|t| (t.id, t.last_used)).collect();
        for sleeper in tabs_to_sleep(&live, id, self.settings["max_live_tabs"].as_u64().unwrap_or(DEFAULT_LIVE_TABS) as usize) {
            if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == sleeper) {
                tab.view = None;
                tab.media = None;
                tab.loading = false;
            }
        }
        self.layout();
        self.sync_toolbar();
        if let Some(view) = self.active_view() {
            let _ = view.focus(); // so shortcuts keep working, and a new tab's search box takes the typing
        }
    }

    fn close_tab(&mut self, id: u64) -> bool {
        let Some(index) = self.tabs.iter().position(|t| t.id == id) else { return false };
        let closed = self.tabs.remove(index);
        if !closed.private && is_web(&closed.url) {
            self.closed.push(closed.url);
        }
        if self.tabs.is_empty() {
            return true;
        }
        if self.active == id {
            let next = self.tabs[index.min(self.tabs.len() - 1)].id;
            self.select(next);
        } else {
            self.sync_toolbar();
        }
        false
    }

    fn step_tab(&mut self, forward: bool) {
        let Some(index) = self.tabs.iter().position(|t| t.id == self.active) else { return };
        let count = self.tabs.len();
        let next = if forward { (index + 1) % count } else { (index + count - 1) % count };
        self.select(self.tabs[next].id);
    }

    fn active_view(&self) -> Option<&WebView> {
        self.tabs.iter().find(|t| t.id == self.active)?.view.as_ref()
    }

    fn layout(&self) {
        if let Some(toolbar) = &self.toolbar {
            let _ = toolbar.set_bounds(self.bounds(true));
        }
        // Only the visible view: hidden tabs get their size when selected, which keeps resizing smooth.
        let bounds = self.bounds(false);
        if let Some(view) = self.active_view() {
            let _ = view.set_bounds(bounds);
        }
        if let Some(panel) = &self.panel {
            let _ = panel.set_bounds(bounds);
        }
        self.place_devtools();
        if let Some(music) = &self.music {
            music.place(&self.window, self.music_bounds());
        }
        if let Some(menu) = &self.menu {
            menu.place(&self.window, self.menu_bounds());
        }
        if let Some(shield) = &self.shield {
            shield.place(&self.window, self.shield_bounds());
        }
        if let Some(dialog) = &self.bookmark_dialog {
            dialog.place(&self.window, self.bookmark_bounds());
        }
        if let Some(popup) = &self.zoom_popup {
            popup.place(&self.window, self.zoom_bounds());
        }
    }

    // Beside the page for the selected tab, hidden for the others. Chromium paints its own title bar and buttons, so
    // removing the window frame leaves them. Hanging past the host's edges is what cuts them off.
    fn place_devtools(&self) {
        let Some(host) = self.devtools_host else { return };
        let scale = self.window.scale_factor();
        let size = self.window.inner_size();
        let bar = self.bounds(true).size.to_logical::<f64>(scale).height;
        let top = (bar * scale) as i32;
        let width = (size.width as f64 * self.devtools_share) as i32;
        let height = size.height as i32 - top;
        let (left, over, right, under) = self.devtools_frame;
        let mut shown = false;
        for tab in &self.tabs {
            let Some(hwnd) = tab.devtools else { continue };
            let hwnd = HWND(hwnd as _);
            // SAFETY: Windows checks the handle, and a window closed meanwhile only makes these calls fail.
            unsafe {
                if tab.id == self.active && !self.fullscreen {
                    shown = true;
                    let _ = SetWindowPos(hwnd, Some(HWND_TOP), -left, -over, width + left + right, height + over + under, SWP_SHOWWINDOW | SWP_NOACTIVATE);
                } else {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
            }
        }
        if let Some(strip) = &self.devtools_header {
            let _ = strip.set_visible(shown);
            let logical = size.to_logical::<f64>(scale);
            let _ = strip.set_bounds(Rect { position: LogicalPosition::new(logical.width - DEVTOOLS_CLOSE, bar).into(), size: ViewSize::new(DEVTOOLS_CLOSE, DEVTOOLS_CLOSE).into() });
        }
        // SAFETY: as above, for a window this thread made.
        let _ = unsafe { SetWindowPos(HWND(host as _), Some(HWND_TOP), size.width as i32 - width, top, width, height, SWP_NOACTIVATE | if shown { SWP_SHOWWINDOW } else { SWP_HIDEWINDOW }) };
        if let Some(strip) = &self.devtools_header {
            // SAFETY: a window this thread made. Above the DevTools host, or it could not be seen or clicked.
            let _ = unsafe { SetWindowPos(strip.hwnd(), Some(HWND_TOP), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
        }
    }

    fn on_divider(&self, x: f64, y: f64) -> bool {
        if self.fullscreen || !self.tabs.iter().any(|t| t.id == self.active && t.devtools.is_some()) {
            return false;
        }
        let scale = self.window.scale_factor();
        let width = self.window.inner_size().width as f64;
        let right = width * (1.0 - self.devtools_share);
        y >= self.bounds(true).size.to_logical::<f64>(scale).height * scale && x <= right && x >= right - DEVTOOLS_DIVIDER * scale
    }

    fn move_divider(&mut self, x: f64, y: f64) {
        if self.divider_drag {
            let width = self.window.inner_size().width as f64;
            self.devtools_share = devtools_share_at(x, width);
            self.layout();
        } else {
            self.window.set_cursor_icon(if self.on_divider(x, y) { CursorIcon::ColResize } else { CursorIcon::Default });
        }
    }

    // Chrome's small note under the toolbar, which leaves the page in view and typing where it was.
    fn show_download_bubble(&mut self, target: &EventLoopWindowTarget<Msg>, entry: Value) {
        let size = self.window.inner_size().to_logical::<f64>(self.window.scale_factor());
        let width = DOWNLOAD_BUBBLE.0.min(size.width);
        let area = Rect { position: LogicalPosition::new((size.width - MENU_BUTTON_RIGHT - width).max(0.0), NAV_ROW_BOTTOM).into(), size: ViewSize::new(width, DOWNLOAD_BUBBLE.1).into() };
        self.download_shown = entry;
        if self.download_bubble.is_some() {
            self.render_download_bubble();
        } else {
            self.download_bubble = self.build_popup(target, DOWNLOAD_HTML, "Riffle download", area, false);
        }
    }

    fn render_download_bubble(&self) {
        let Some(bubble) = &self.download_bubble else { return };
        let private = self.tabs.iter().any(|t| t.id == self.active && t.private);
        let _ = bubble.evaluate_script(&format!("render({})", json!({"file": self.download_shown["file"], "path": self.download_shown["path"], "state": self.download_shown["state"], "theme": self.settings["theme"], "accent": self.settings["accent"], "private": private})));
    }

    // WebView2 can only open DevTools as its own window, so that window is moved inside this one as soon as it appears.
    // shortcut: goes by the engine process and the address in the title, so two windows showing one address could swap. Upgrade if WebView2 gains docking.
    // The engine sometimes drops a DevTools window just as it appears, so one that was asked for is asked for again.
    fn keep_wanted_devtools(&mut self) {
        let Some((id, retries, since)) = self.devtools_wanted else { return };
        let Some(tab) = self.tabs.iter().find(|t| t.id == id) else {
            self.devtools_wanted = None;
            return;
        };
        if tab.devtools.is_some() {
            if since.elapsed().as_secs() >= DEVTOOLS_SETTLE_SECS {
                self.devtools_wanted = None;
            }
            return;
        }
        let (seen, alive) = DEVTOOLS_WATCH.with(|w| {
            let watch = w.borrow();
            // SAFETY: IsWindow accepts any value.
            (watch.seen, watch.hidden.is_some_and(|(hwnd, _)| unsafe { IsWindow(Some(HWND(hwnd as _))) }.as_bool()))
        });
        if !seen || alive {
            return;
        }
        if retries >= DEVTOOLS_RETRIES {
            self.devtools_wanted = None;
        } else if let Some(view) = &tab.view {
            arm_devtools_watch();
            view.open_devtools();
            self.devtools_wanted = Some((id, retries + 1, Instant::now()));
        }
    }

    fn scan_devtools(&mut self) {
        restore_lost_devtools();
        let mut changed = false;
        for tab in &mut self.tabs {
            // SAFETY: IsWindow accepts any value. This also catches DevTools closing itself.
            if tab.devtools.is_some_and(|h| !unsafe { IsWindow(Some(HWND(h as _))) }.as_bool()) {
                tab.devtools = None;
                changed = true;
            }
        }
        let mut process = 0u32;
        let mut address = String::new();
        if let Some(view) = self.active_view() {
            address = view.url().unwrap_or_default();
            // SAFETY: COM call on the UI thread writing into a local value.
            let _ = unsafe { view.webview().BrowserProcessId(&mut process) };
        }
        let free = self.tabs.iter().any(|t| t.id == self.active && t.devtools.is_none());
        if let (true, true, Some(found)) = (free, process != 0, floating_devtools(process, &address)) {
            if self.devtools_host.is_none() {
                // SAFETY: a plain child of this thread's own window, which takes it along when it closes.
                let made = unsafe { CreateWindowExW(WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR::null(), WS_CHILD | WS_CLIPCHILDREN, 0, 0, 0, 0, Some(HWND(self.window.hwnd() as _)), None, None, None) };
                self.devtools_host = made.ok().map(|host| host.0 as isize);
            }
            // Without a host DevTools just stays a floating window, which still works.
            if let Some(host) = self.devtools_host {
                // SAFETY: a window checked just now. Removing the frame and making it a child is what moves it inside.
                unsafe {
                    let style = GetWindowLongPtrW(found, GWL_STYLE) & !((WS_POPUP.0 | WS_CAPTION.0 | WS_THICKFRAME.0) as isize);
                    SetWindowLongPtrW(found, GWL_STYLE, style | WS_CHILD.0 as isize);
                    let _ = SetParent(found, Some(HWND(host as _)));
                    let _ = SetWindowPos(found, Some(HWND_TOP), 0, 0, 0, 0, SWP_FRAMECHANGED);
                }
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == self.active) {
                    tab.devtools = Some(found.0 as isize);
                }
                set_see_through(found, false);
                DEVTOOLS_WATCH.with(|w| {
                    let mut watch = w.borrow_mut();
                    watch.armed_until = None;
                    watch.hidden = None;
                });
                changed = true;
            }
        }
        // Measured rather than fixed, as these differ with display scale. The content appears a moment after the window.
        let shown = self.tabs.iter().find(|t| t.id == self.active).and_then(|t| t.devtools);
        if let Some(frame) = shown.and_then(measure_devtools_frame) {
            if frame != self.devtools_frame {
                self.devtools_frame = frame;
                changed = true;
            }
        }
        if changed {
            self.sync_devtools_header();
            self.layout();
        }
        self.keep_wanted_devtools();
    }

    // Only exists while DevTools does, so it costs nothing the rest of the time.
    fn sync_devtools_header(&mut self) {
        let any = self.tabs.iter().any(|t| t.devtools.is_some());
        if !any {
            self.devtools_header = None;
        } else if self.devtools_header.is_none() {
            let proxy = self.proxy.clone();
            let dark = self.settings["theme"] == "dark" || self.private_window;
            let html = DEVTOOLS_HTML.replace("<!--ICONS-->", ICONS_HTML).replace("__CLASS__", if dark { "dark" } else if self.settings["theme"] == "light" { "light" } else { "" });
            self.devtools_header = ui_view_builder(&mut self.ui_context, &mut self.context, &self.args)
            .with_browser_extensions_enabled(true)
                .with_focused(false)
                .with_html(html)
                .with_drag_drop_handler(|_| true)
                .with_navigation_handler(is_built_in_page)
                .with_ipc_handler(move |req| drop(proxy.send_event(Msg::Toolbar(req.body().clone()))))
                .build_as_child(&self.window)
                .ok();
        }
    }

    fn focus_address(&self) {
        if let Some(toolbar) = &self.toolbar {
            let _ = toolbar.focus();
            let _ = toolbar.evaluate_script("focusAddress()");
        }
    }

    fn sync_toolbar(&self) {
        let Some(toolbar) = &self.toolbar else { return };
        let active = self.tabs.iter().find(|t| t.id == self.active);
        let tabs: Vec<Value> = self
            .tabs
            .iter()
            .map(|t| json!({"id": t.id, "title": t.title, "asleep": t.view.is_none(), "active": t.id == self.active, "pinned": t.pinned, "private": t.private, "url": t.url, "icon": t.icon, "loading": t.loading, "playing": t.media.is_some_and(|(playing, _)| playing)}))
            .collect();
        let history_state = active.and_then(|t| t.view.as_ref()).map_or((false, false), can_go);
        let state = json!({
            "tabs": tabs,
            "url": active.map_or("", |t| if is_web(&t.url) || t.url.starts_with("blob:") { &t.url } else { "" }),
            "loading": active.is_some_and(|t| t.loading),
            "maximized": self.window.is_maximized(),
            "private": active.is_some_and(|t| t.private),
            "zoom": active.map_or(100, |t| (t.zoom * 100.0).round() as u32),
            "showDownloads": self.show_download_icon(),
            "showShield": self.show_shield_icon(),
            "showReader": self.show_reader_icon(),
            "showExtensions": self.toolbar_button_shown("extensions"),
            "showPrivate": self.icon_choice("private", true),
            "showMusic": self.icon_choice("music", true),
            "showStar": self.icon_choice("star", true),
            "downloading": self.tabs.iter().map(|t| t.downloads).sum::<u32>(),
            "bookmarked": active.is_some_and(|t| self.bookmarks.iter().any(|b| b["url"] == t.url.as_str())),
            "bar": self.show_bookmarks_bar(),
            "bookmarks": if self.show_bookmarks_bar() { json!(self.bookmarks) } else { json!([]) },
            "adblock": self.adblock,
            "blocked": active.map_or(0, |t| t.guard.borrow().blocked),
            "pinnedCount": self.tabs.iter().filter(|t| t.pinned).count(),
            "stopped": active.is_some_and(|t| t.guard.borrow().stopped.is_some()),
            "allowed": active.is_some_and(|t| site_is_allowed(&t.url)),
            "playing": self.tabs.iter().any(|t| t.media.is_some_and(|(playing, _)| playing)),
            "canBack": history_state.0,
            "canForward": history_state.1,
            "theme": self.settings["theme"],
            "accent": self.settings["accent"],
        });
        let _ = toolbar.evaluate_script(&format!("render({state})"));
        let name = if self.private_window { "Riffle Private" } else { "Riffle" };
        let title = active.map_or(name.to_string(), |t| format!("{} - {name}", t.title));
        self.window.set_title(&title);
    }

    /// Returns true when the browser should exit.
    fn handle(&mut self, msg: Msg, target: &EventLoopWindowTarget<Msg>) -> bool {
        match msg {
            Msg::Toolbar(raw) => return self.toolbar_command(&raw, target),
            Msg::DownloadStarted(id, url, path) => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
                    tab.downloads += 1;
                }
                self.reload_downloads();
                let file = Path::new(&path).file_name().map_or(url.clone(), |f| f.to_string_lossy().to_string());
                self.downloads.push(json!({"url": url, "path": path, "file": file, "state": "downloading", "time": now_secs()}));
                let excess = self.downloads.len().saturating_sub(DOWNLOADS_KEPT);
                self.downloads.drain(..excess);
                self.save_downloads();
                self.sync_toolbar();
                if let (true, true, Some(entry)) = (self.settings["download_show"] == true, self.panel.is_none() && !self.fullscreen, self.downloads.last().cloned()) {
                    self.show_download_bubble(target, entry);
                }
            }
            Msg::DownloadDone(id, url, path, ok) => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
                    tab.downloads = tab.downloads.saturating_sub(1);
                }
                self.last_download = Some(Instant::now());
                self.reload_downloads();
                if let Some(entry) = self.downloads.iter_mut().rev().find(|d| d["url"] == url.as_str() && d["state"] == "downloading") {
                    entry["state"] = json!(if ok { "done" } else { "failed" });
                    if let Some(path) = path {
                        entry["file"] = json!(Path::new(&path).file_name().map(|f| f.to_string_lossy()));
                        entry["path"] = json!(path);
                    }
                }
                self.save_downloads();
                self.sync_toolbar();
                let finished = self.downloads.iter().rev().find(|d| d["url"] == url.as_str()).cloned();
                if let (true, true, Some(entry)) = (self.settings["download_show"] == true, self.panel.is_none() && !self.fullscreen, finished) {
                    self.show_download_bubble(target, entry);
                }
            }
            Msg::Suggestions(seq, words) => {
                if seq == self.suggest_seq {
                    self.suggest_items.extend(words.into_iter().take(SUGGEST_MAX_SEARCH).map(|text| json!({"text": text})));
                    self.show_suggestions(target);
                }
            }
            Msg::DevtoolsScan => self.scan_devtools(),
            Msg::TabCrashed(id) => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id && t.crashed_url.is_none()) {
                    tab.crashed_url = Some(tab.url.clone());
                    tab.title = "This page stopped working".to_string();
                    tab.media = None;
                    tab.loading = false;
                    let site = tab.url.clone();
                    if let Some(view) = &tab.view {
                        let _ = view.load_html(&error_page(error_page_look(COREWEBVIEW2_WEB_ERROR_STATUS_UNEXPECTED_ERROR), "This page stopped working", "Your other tabs are fine. Anything you had typed into this page may be lost.", &[], &site, "Reload page", false));
                    }
                }
                self.sync_toolbar();
            }
            Msg::NavStarted(id) => {
                self.nav_counter += 1;
                let seq = self.nav_counter;
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
                    tab.nav_seq = Some(seq);
                }
                if id == self.active {
                    self.slow_note = None;
                }
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_secs(SLOW_SECS));
                    drop(proxy.send_event(Msg::SlowCheck(id, seq)));
                });
            }
            Msg::SlowCheck(id, seq) => {
                // Still the same navigation, on the tab in view. Not over a video in fullscreen.
                if id == self.active && !self.fullscreen && self.slow_note.is_none() && self.tabs.iter().any(|t| t.id == id && t.nav_seq == Some(seq)) {
                    let size = self.window.inner_size().to_logical::<f64>(self.window.scale_factor());
                    let width = SLOW_NOTE.0.min(size.width);
                    let top = TOOLBAR_HEIGHT + if self.show_bookmarks_bar() { BOOKMARKS_BAR_HEIGHT } else { 0.0 } + 8.0; // clear of the bars, over the page
                    let area = Rect { position: LogicalPosition::new((size.width - width) / 2.0, top).into(), size: ViewSize::new(width, SLOW_NOTE.1).into() };
                    self.slow_note = self.build_popup(target, SLOW_HTML, "Riffle slow page", area, false);
                }
            }
            Msg::NavFinished(id, status, ok) => {
                if id == self.active {
                    self.slow_note = None;
                }
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
                    tab.nav_seq = None;
                    if let Some((title, detail, tips)) = error_page_text(status).filter(|_| !ok && tab.crashed_url.is_none()) {
                        let wanted = tab.guard.borrow().nav_url.clone();
                        let url = if is_web(&wanted) { wanted } else { tab.url.clone() };
                        tab.url = url.clone();
                        tab.title = title.to_string();
                        tab.crashed_url = Some(url.clone());
                        tab.loading = false;
                        let retry_online = status == COREWEBVIEW2_WEB_ERROR_STATUS_DISCONNECTED;
                        let site = Url::parse(&url).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or(url);
                        if let Some(view) = &tab.view {
                            let _ = view.load_html(&error_page(error_page_look(status), title, detail, tips, &site, "Try again", retry_online));
                        }
                    }
                }
                self.sync_toolbar();
            }
            Msg::Extensions(list) => {
                if let Some(panel) = self.panel.as_ref().filter(|_| self.panel_page == "extensions") {
                    let rows: Vec<Value> = list.iter().map(|(id, name, on)| json!({"id": id, "name": name, "on": on})).collect();
                    let _ = panel.evaluate_script(&format!("renderExtensions({})", json!({"rows": rows, "status": self.extension_status})));
                }
            }
            Msg::ExtensionAdded(ok) => {
                self.extension_status = if ok { "Extension added".to_string() } else { "That folder is not a working extension. Choose the folder holding its manifest.json".to_string() };
                self.request_extensions();
            }
            Msg::UpdateReady(version) => {
                self.update_ready = Some(version);
                if self.panel_page == "settings" {
                    self.render_panel();
                }
            }
            Msg::UpdateChecked(found) => {
                self.update_status = match &found {
                    Some(None) => "current",
                    Some(Some(_)) => "",
                    None => "failed",
                };
                if let Some(Some(version)) = found {
                    self.update_ready = Some(version);
                }
                if self.panel_page == "settings" {
                    self.render_panel();
                }
            }
            Msg::Processes(list) => self.render_processes(list),
            Msg::Fullscreen(id, on) => {
                if id == self.active {
                    self.set_fullscreen(on);
                }
            }
            Msg::IdleCheck => {
                self.sleep_idle_tabs();
                if !self.private_window {
                    save_blocked_count(&self.data_dir);
                }
                self.sync_toolbar(); // the download icon leaves in time
            }
            Msg::Refresh => {
                self.sync_toolbar();
                self.render_shield();
            }
            Msg::Shortcut(id, name) => {
                if let Some(icon) = name.strip_prefix("icon:") {
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id).filter(|_| icon.len() < 2048 && is_web(icon)) {
                        tab.icon = Some(icon.to_string());
                        self.sync_toolbar();
                    }
                    return false;
                }
                if let Some(report) = name.strip_prefix("media:") {
                    if let (Ok(report), Some(tab)) = (serde_json::from_str::<Value>(report), self.tabs.iter_mut().find(|t| t.id == id)) {
                        tab.media = Some((report["playing"] == true, report["volume"].as_f64().unwrap_or(1.0).clamp(0.0, 1.0)));
                        tab.has_video = report["video"] == true;
                    }
                    self.render_music();
                    self.sync_toolbar();
                    return false;
                }
                if let Some(report) = name.strip_prefix("ads:") {
                    self.hide_ads(id, report);
                    return false;
                }
                // Only from the welcome tab itself: a web page in another tab, or this one later, gets nothing from these.
                if let Some(action) = name.strip_prefix("welcome:").filter(|_| self.tabs.iter().any(|t| t.id == id && t.url == WELCOME_URL)) {
                    match action {
                        "chrome" | "edge" => {
                            let command = json!({"cmd": "import_bookmarks", "value": action}).to_string();
                            self.toolbar_command(&command, target);
                            if let Some(view) = self.tabs.iter().find(|t| t.id == id).and_then(|t| t.view.as_ref()) {
                                let _ = view.evaluate_script(&format!("imported({})", json!(self.import_status)));
                            }
                        }
                        "default" => {
                            let command = if is_installed() { "make_default" } else { "install_app" };
                            return self.toolbar_command(&json!({"cmd": command}).to_string(), target);
                        }
                        "close" => {
                            if self.tabs.len() > 1 {
                                return self.close_tab(id);
                            }
                            self.open_tab(&self.new_tab_url(), false);
                            return self.close_tab(id);
                        }
                        _ => {}
                    }
                    return false;
                }
                if name == "article" {
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
                        tab.readable = true;
                    }
                    self.sync_toolbar();
                    return false;
                }
                if name == "crash_reload" {
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
                        if let Some(url) = tab.crashed_url.take() {
                            tab.guard.borrow_mut().vouch();
                            if let Some(view) = &tab.view {
                                let _ = view.load_url(&url);
                            }
                        }
                    }
                    return false;
                }
                if id != self.active {
                    return false;
                }
                // The new tab page's search box. Safe from any page: it only moves that page's own tab,
                // and address_to_url never yields anything but a web address or a search.
                if let Some(text) = name.strip_prefix("go:") {
                    // shortcut: any page can post this, so it can vouch for its own next address. Only the new tab page should.
                    if let Some(tab) = self.tabs.iter().find(|t| t.id == self.active && !is_web(&t.url)) {
                        tab.guard.borrow_mut().vouch();
                    }
                    if let (Some(url), Some(view)) = (address_to_url(text, self.search()), self.active_view()) {
                        let _ = view.load_url(&url);
                    }
                    return false;
                }
                // Anything else from a page is ignored: keyboard shortcuts arrive as Msg::Key from the engine.
            }
            Msg::Key(id, name) => {
                if id == self.active {
                    if let Some(how) = name.strip_prefix("zoom_") {
                        self.zoom_by_key(how, target);
                        return false;
                    }
                    return self.shortcut(name);
                }
            }
            Msg::Title(id, title) => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
                    tab.title = if title.trim().is_empty() { tab.url.clone() } else { title };
                }
                self.note_visit(id);
                self.sync_toolbar();
            }
            Msg::Load(id, loading, url) => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
                    tab.loading = loading;
                    if loading {
                        // The page really changed now. Downloads and mailto links reach the navigation check but never here.
                        let mut guard = tab.guard.borrow_mut();
                        if !same_site(&guard.top_url, &url) {
                            guard.blocked = 0;
                            guard.stopped = None;
                        }
                        guard.top_url = url.clone();
                        tab.media = None; // a reload stops whatever was playing
                        tab.readable = false;
                    } else {
                        tab.guard.borrow_mut().last_click = None; // a click explains its own page, not what that page does next
                    }
                    if (tab.url == WELCOME_URL || tab.crashed_url.is_some()) && !is_web(&url) {
                        self.sync_toolbar(); // still that built-in page, which the engine sees as a blank address
                        return false;
                    }
                    if is_web(&url) {
                        tab.crashed_url = None; // the user moved on from an error page
                    }
                    if tab.url != url {
                        tab.title = url.clone(); // until the new page reports its title
                        tab.media = None;
                        if !same_site(&tab.url, &url) {
                            tab.icon = None;
                        }
                    }
                    tab.url = url;
                }
                if loading {
                    let site = self.tabs.iter().find(|t| t.id == id).and_then(|t| Url::parse(&t.url).ok()).and_then(|u| u.host_str().map(str::to_string));
                    let zoom = site.and_then(|host| self.site_zoom[&host].as_f64()).unwrap_or(1.0);
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
                        tab.zoom = zoom;
                        if let Some(view) = &tab.view {
                            set_view_zoom(view, zoom);
                        }
                    }
                    self.inject_scriptlets(id);
                } else {
                    self.note_visit(id);
                }
                self.sync_toolbar();
            }
            Msg::Popup(id, url) => {
                // A link from a private tab stays private.
                let private = self.tabs.iter().any(|t| t.id == id && t.private);
                let ok = self.adblock || self.last_popup.elapsed().as_millis() >= POPUP_MIN_GAP_MS; // with blocking on, each already needs its own click
                if ok && Url::parse(&url).is_ok_and(|u| matches!(u.scheme(), "http" | "https")) {
                    self.last_popup = std::time::Instant::now();
                    self.open_tab(&url, private);
                }
            }
        }
        false
    }

    fn toolbar_command(&mut self, raw: &str, target: &EventLoopWindowTarget<Msg>) -> bool {
        if let Some(how) = raw.strip_prefix("zoom_") {
            self.zoom_by_key(how, target);
            return false;
        }
        let Ok(command) = serde_json::from_str::<Value>(raw) else { return self.shortcut(raw) };
        let id = command["id"].as_u64();
        let script = match command["cmd"].as_str().unwrap_or("") {
            "ready" => {
                self.sync_toolbar();
                return false;
            }
            "new" => {
                self.open_tab(&self.new_tab_url(), false);
                return false;
            }
            "shield" => {
                self.toggle_shield(target);
                return false;
            }
            "shield_ready" => {
                self.render_shield();
                return false;
            }
            "shield_size" => {
                self.shield_height = command["value"].as_f64().unwrap_or(200.0).clamp(60.0, 480.0);
                if let Some(shield) = &self.shield {
                    shield.place(&self.window, self.shield_bounds());
                }
                return false;
            }
            "shield_close" => {
                if self.shield.take().is_some() {
                    self.shield_closed = std::time::Instant::now();
                }
                return false;
            }
            "shield_open" => {
                let stopped = self.tabs.iter().find(|t| t.id == self.active).and_then(|t| t.guard.borrow_mut().stopped.take());
                self.shield = None;
                if let Some(url) = stopped.filter(|u| is_web(u)) {
                    let private = self.tabs.iter().any(|t| t.id == self.active && t.private);
                    self.open_tab(&url, private);
                }
                return false;
            }
            "shield_site" | "shield_popups" => {
                let site_toggle = command["cmd"] == "shield_site";
                self.reload_settings();
                let host = self.tabs.iter().find(|t| t.id == self.active).and_then(|t| Url::parse(&t.url).ok()).and_then(|u| u.host_str().map(str::to_string));
                if let Some(host) = host {
                    let key = if site_toggle { "allow_sites" } else { "allow_popups" };
                    // The site switch reads "block here", the pop-up switch "allow here", so their lists run opposite ways.
                    let listed = (command["value"] == true) != site_toggle;
                    let mut sites: Vec<Value> = self.settings[key].as_array().cloned().unwrap_or_default();
                    sites.retain(|s| s != host.as_str());
                    if listed {
                        sites.push(json!(host));
                    }
                    self.settings[key] = json!(sites);
                    self.settings = sanitize_settings(&self.settings);
                    set_allowed_sites(&self.settings);
                    self.save_settings();
                    self.render_panel();
                    if site_toggle {
                        self.shield = None;
                        if let Some(view) = self.active_view() {
                            let _ = view.reload();
                        }
                    }
                }
                return false;
            }
            "menu" => {
                self.toggle_menu(target);
                return false;
            }
            "suggest" => {
                self.suggest_box = (command["x"].as_f64().unwrap_or(0.0), command["w"].as_f64().unwrap_or(MENU_WIDTH));
                self.start_suggestions(command["value"].as_str().unwrap_or(""), target);
                return false;
            }
            "download_ready" => {
                self.render_download_bubble();
                return false;
            }
            "download_close" => {
                self.download_bubble = None;
                return false;
            }
            "suggest_ready" => {
                self.render_suggestions();
                return false;
            }
            "suggest_mark" => {
                if let Some(suggest) = &self.suggest {
                    let _ = suggest.evaluate_script(&format!("mark({})", command["value"].as_i64().unwrap_or(-1)));
                }
                return false;
            }
            "suggest_close" => {
                self.close_suggestions();
                return false;
            }
            "menu_ready" => {
                self.render_menu();
                return false;
            }
            "menu_close" => {
                if self.menu.take().is_some() {
                    self.menu_closed = std::time::Instant::now();
                }
                return false;
            }
            "zoom" => {
                self.change_zoom(command["value"].as_str().unwrap_or(""));
                return false;
            }
            "tab_move" => {
                if let Some(from) = self.tabs.iter().position(|t| Some(t.id) == id) {
                    let pinned = self.tabs.iter().filter(|t| t.pinned).count();
                    let to = tab_position(command["value"].as_u64().unwrap_or(from as u64) as usize, pinned, self.tabs.len(), self.tabs[from].pinned);
                    let tab = self.tabs.remove(from);
                    self.tabs.insert(to, tab);
                    self.save_session();
                }
                self.sync_toolbar();
                return false;
            }
            "marks_more" => {
                self.marks_menu = None;
                let from = command["from"].as_u64().unwrap_or(0) as usize;
                let count = self.bookmarks.len().saturating_sub(from);
                if count > 0 {
                    let size = self.window.inner_size().to_logical::<f64>(self.window.scale_factor());
                    let height = (count as f64 * 32.0 + 16.0).min(416.0);
                    let x = (command["x"].as_f64().unwrap_or(0.0) - MARKS_WIDTH).clamp(0.0, (size.width - MARKS_WIDTH).max(0.0));
                    let area = Rect { position: LogicalPosition::new(x, TOOLBAR_HEIGHT + BOOKMARKS_BAR_HEIGHT).into(), size: ViewSize::new(MARKS_WIDTH, height).into() };
                    if let Some(popup) = self.build_popup(target, MARKS_HTML, "Riffle more bookmarks", area, true) {
                        self.marks_menu = Some((popup, from));
                    }
                }
                return false;
            }
            "marks_ready" => {
                if let Some((popup, from)) = &self.marks_menu {
                    let marks = self.bookmarks.get(*from..).unwrap_or_default();
                    let _ = popup.evaluate_script(&format!("render({})", json!({"marks": marks, "theme": self.settings["theme"], "private": self.private_window})));
                }
                return false;
            }
            "marks_close" => {
                self.marks_menu = None;
                return false;
            }
            "marks_pick" => {
                self.marks_menu = None;
                drop(self.proxy.send_event(Msg::Toolbar(json!({"cmd": "go", "value": command["value"]}).to_string())));
                return false;
            }
            "bookmark_move" => {
                let (from, to) = (command["from"].as_u64().unwrap_or(u64::MAX) as usize, command["to"].as_u64().unwrap_or(u64::MAX) as usize);
                if from < self.bookmarks.len() && to < self.bookmarks.len() && from != to {
                    let moved = self.bookmarks.remove(from);
                    self.bookmarks.insert(to, moved);
                    self.save_bookmarks();
                    self.sync_toolbar();
                }
                return false;
            }
            "tab_menu" => {
                self.tab_menu = None;
                self.marks_menu = None;
                if let Some(tab) = id.filter(|i| self.tabs.iter().any(|t| t.id == *i)) {
                    let size = self.window.inner_size().to_logical::<f64>(self.window.scale_factor());
                    let x = command["x"].as_f64().unwrap_or(0.0).clamp(0.0, (size.width - TAB_MENU_SIZE.0).max(0.0));
                    let area = Rect { position: LogicalPosition::new(x, 36.0).into(), size: ViewSize::new(TAB_MENU_SIZE.0, TAB_MENU_SIZE.1).into() };
                    if let Some(popup) = self.build_popup(target, TAB_MENU_HTML, "Riffle tab menu", area, true) {
                        self.tab_menu = Some((popup, tab));
                    }
                }
                return false;
            }
            "tab_detach" => return id.is_some_and(|id| self.move_tab_to_new_window(id)),
            "tab_menu_ready" => {
                if let Some((popup, tab)) = &self.tab_menu {
                    let pinned = self.tabs.iter().any(|t| t.id == *tab && t.pinned);
                    let _ = popup.evaluate_script(&format!("render({})", json!({"pinned": pinned, "alone": self.tabs.len() < 2, "theme": self.settings["theme"], "private": self.private_window})));
                }
                return false;
            }
            "tab_menu_close" => {
                self.tab_menu = None;
                self.marks_menu = None;
                return false;
            }
            "tab_pick" => {
                let Some((_, tab)) = self.tab_menu.take() else { return false };
                match command["value"].as_str().unwrap_or("") {
                    "pin" => {
                        if let Some(from) = self.tabs.iter().position(|t| t.id == tab) {
                            let mut moved = self.tabs.remove(from);
                            moved.pinned = !moved.pinned;
                            let group = self.tabs.iter().filter(|t| t.pinned).count();
                            self.tabs.insert(group, moved);
                            self.save_session();
                            self.sync_toolbar();
                        }
                    }
                    "window" => return self.move_tab_to_new_window(tab),
                    "start" | "end" => {
                        if let Some(from) = self.tabs.iter().position(|t| t.id == tab) {
                            let pinned = self.tabs.iter().filter(|t| t.pinned).count();
                            let wanted = if command["value"] == "start" { 0 } else { self.tabs.len() };
                            let to = tab_position(wanted, pinned, self.tabs.len(), self.tabs[from].pinned);
                            let moved = self.tabs.remove(from);
                            self.tabs.insert(to, moved);
                            self.save_session();
                            self.sync_toolbar();
                        }
                    }
                    "close" => return self.close_tab(tab),
                    "others" => {
                        let others: Vec<u64> = self.tabs.iter().filter(|t| t.id != tab && !t.pinned).map(|t| t.id).collect();
                        self.select(tab);
                        for other in others {
                            self.close_tab(other);
                        }
                    }
                    _ => {}
                }
                return false;
            }
            "tab_search" => {
                if self.tab_search.take().is_none() {
                    let size = self.window.inner_size().to_logical::<f64>(self.window.scale_factor());
                    let width = TAB_SEARCH_SIZE.0.min(size.width);
                    let area = Rect { position: LogicalPosition::new(((size.width - width) / 2.0).max(0.0), NAV_ROW_BOTTOM).into(), size: ViewSize::new(width, TAB_SEARCH_SIZE.1.min((size.height - NAV_ROW_BOTTOM).max(120.0))).into() };
                    self.tab_search = self.build_popup(target, TAB_SEARCH_HTML, "Riffle tab search", area, true);
                }
                return false;
            }
            "tab_search_ready" => {
                let tabs: Vec<Value> = self.tabs.iter().map(|t| json!({"id": t.id, "title": t.title, "url": t.url, "asleep": t.view.is_none()})).collect();
                if let Some(popup) = &self.tab_search {
                    let _ = popup.evaluate_script(&format!("render({})", json!({"tabs": tabs, "theme": self.settings["theme"], "private": self.private_window})));
                }
                return false;
            }
            "tab_search_close" => {
                self.tab_search = None;
        self.slow_note = None;
                return false;
            }
            "tab_search_pick" => {
                self.tab_search = None;
        self.slow_note = None;
                id.map(|id| self.select(id));
                return false;
            }
            "media_pip" => {
                let Some(view) = id.and_then(|id| self.tabs.iter().find(|t| t.id == id)).and_then(|t| t.view.as_ref()) else { return false };
                toggle_picture_in_picture(view);
                self.music = None;
                return false;
            }
            "import_bookmarks" => {
                let (name, path) = match command["value"].as_str().unwrap_or("") {
                    "edge" => ("Edge", local_dir("Microsoft\\Edge\\User Data\\Default\\Bookmarks")),
                    _ => ("Chrome", local_dir("Google\\Chrome\\User Data\\Default\\Bookmarks")),
                };
                self.reload_bookmarks();
                self.import_status = match path.and_then(|p| std::fs::read_to_string(p).ok()) {
                    None => format!("No {name} bookmarks were found on this PC"),
                    Some(text) => {
                        let added = merge_bookmarks(&mut self.bookmarks, parse_browser_bookmarks(&text));
                        self.save_bookmarks();
                        self.layout();
                        self.sync_toolbar();
                        format!("Added {added} {} from {name}", if added == 1 { "bookmark" } else { "bookmarks" })
                    }
                };
                self.render_panel();
                return false;
            }
            "ext_refresh" => {
                self.request_extensions();
                return false;
            }
            "ext_add" => {
                self.extension_status.clear();
                if let Some(folder) = pick_path(self.window.hwnd() as isize, true, "", "") {
                    self.add_extension(&folder);
                }
                return false;
            }
            "ext_toggle" | "ext_remove" => {
                let (wanted, remove, on) = (command["id"].as_str().unwrap_or("").to_string(), command["cmd"] == "ext_remove", command["value"] == true);
                self.change_extension(wanted, remove, on);
                return false;
            }
            "slow_ready" => {
                if let Some(note) = &self.slow_note {
                    let _ = note.evaluate_script(&format!("render({})", json!({"theme": self.settings["theme"], "private": self.private_window})));
                }
                return false;
            }
            "slow_close" => {
                self.slow_note = None;
                return false;
            }
            "slow_stop" => {
                self.slow_note = None;
                if let Some(view) = self.active_view() {
                    let _ = view.evaluate_script("window.stop()");
                }
                return false;
            }
            "zoom_toggle" => {
                if self.zoom_popup.take().is_none() {
                    self.zoom_popup = self.build_popup(target, ZOOM_HTML, "Riffle zoom", self.zoom_bounds(), false);
                }
                return false;
            }
            "zoom_ready" => {
                self.render_zoom();
                return false;
            }
            "zoom_close" => {
                self.zoom_popup = None;
        self.tab_menu = None;
        self.marks_menu = None;
        self.tab_search = None;
        self.slow_note = None;
                return false;
            }
            "menu_pick" => {
                self.pick_menu_item(command["value"].as_str().unwrap_or(""));
                return false;
            }
            "bookmark" => {
                if self.bookmark_dialog.is_none() && self.bookmark_closed.elapsed().as_millis() >= 300 {
                    self.toggle_bookmark();
                }
                return false;
            }
            "bookmark_dialog" => {
                if self.bookmark_dialog.is_none() {
                    self.bookmark_dialog = self.build_popup(target, BOOKMARK_HTML, "Riffle bookmark", self.bookmark_bounds(), true);
                }
                return false;
            }
            "bookmark_size" => {
                self.bookmark_height = command["value"].as_f64().unwrap_or(220.0).clamp(120.0, 340.0);
                if let Some(dialog) = &self.bookmark_dialog {
                    dialog.place(&self.window, self.bookmark_bounds());
                }
                return false;
            }
            "bookmark_ready" => {
                let title = self.bookmarks.iter().find(|b| b["url"] == self.bookmark_url.as_str()).map(|b| b["title"].clone());
                let state = json!({"title": title, "url": self.bookmark_url, "added": self.bookmark_added, "theme": self.settings["theme"], "private": self.private_window});
                if let Some(dialog) = &self.bookmark_dialog {
                    let _ = dialog.evaluate_script(&format!("render({state})"));
                }
                return false;
            }
            "bookmark_save" | "bookmark_remove" | "bookmark_close" => {
                self.reload_bookmarks();
                let url = self.bookmark_url.clone();
                if command["cmd"] == "bookmark_remove" {
                    self.bookmarks.retain(|b| b["url"] != url.as_str());
                } else if command["cmd"] == "bookmark_save" {
                    // A click elsewhere saves what it can. Done on a bad address stays open to say so.
                    let new_url = web_address(command["url"].as_str().unwrap_or(""));
                    if new_url.is_none() && command["strict"] == true {
                        if let Some(dialog) = &self.bookmark_dialog {
                            let _ = dialog.evaluate_script("badLink()");
                        }
                        return false;
                    }
                    edit_bookmark(&mut self.bookmarks, &url, command["value"].as_str().unwrap_or(""), new_url.as_deref());
                }
                self.save_bookmarks();
                self.bookmark_dialog = None;
                self.bookmark_closed = std::time::Instant::now();
                self.layout();
                self.sync_toolbar();
                if let Some(view) = self.active_view() {
                    let _ = view.focus();
                }
                return false;
            }
            "music" => {
                self.toggle_music(target);
                return false;
            }
            "system_volume" => {
                if let Some(level) = command["value"].as_f64() {
                    set_system_volume(level);
                }
                return false;
            }
            "system_mute" => {
                set_system_mute(command["value"] == true);
                self.render_music();
                return false;
            }
            "music_ready" => {
                self.render_music();
                return false;
            }
            "music_close" => {
                if self.music.take().is_some() {
                    self.music_closed = std::time::Instant::now();
                }
                return false;
            }
            "media_toggle" | "media_volume" | "media_mute" => {
                let Some(view) = id.and_then(|id| self.tabs.iter().find(|t| t.id == id)).and_then(|t| t.view.as_ref()) else { return false };
                let action = match (command["value"].as_f64(), command["value"].as_bool()) {
                    (Some(volume), _) => format!("if(el){{el.muted=false;el.volume={}}}", volume.clamp(0.0, 1.0)),
                    // The player's own mute, which keeps its volume for unmuting even if this dialog has closed.
                    (_, Some(true)) => "if(el){el.muted=true}".to_string(),
                    (_, Some(false)) => "if(el){el.muted=false;if(!el.volume)el.volume=1}".to_string(),
                    _ => "if(el){el.paused?el.play():el.pause()}".to_string(),
                };
                let _ = view.evaluate_script(&format!("(()=>{{{MEDIA_PICK}{action}}})()"));
                return false;
            }
            "panel" => {
                self.open_panel(command["value"].as_str().unwrap_or("settings"));
                return false;
            }
            "panel_ready" => {
                self.render_panel();
                return false;
            }
            "close_panel" => {
                self.close_panel();
                return false;
            }
            "put_bookmark" => {
                self.reload_bookmarks();
                match command["url"].as_str().and_then(web_address) {
                    Some(url) => {
                        put_bookmark(&mut self.bookmarks, command["old"].as_str(), command["title"].as_str().unwrap_or(""), &url);
                        self.save_bookmarks();
                        self.render_panel();
                        self.sync_toolbar();
                    }
                    None => {
                        if let Some(panel) = &self.panel {
                            let _ = panel.evaluate_script("showFormError('Enter a web address, such as example.com')");
                        }
                    }
                }
                return false;
            }
            "move_bookmark" => {
                self.reload_bookmarks();
                move_bookmark(&mut self.bookmarks, command["value"].as_str().unwrap_or(""), command["up"] == true);
                self.save_bookmarks();
                self.render_panel();
                self.sync_toolbar();
                return false;
            }
            "show_download" => {
                // Only a path on the list, and only ever shown in its folder, never run.
                if let Some(path) = command["value"].as_str().filter(|p| self.downloads.iter().any(|d| d["path"] == *p)) {
                    let _ = std::process::Command::new(system32("..\\explorer.exe")).raw_arg(format!("/select,\"{path}\"")).spawn();
                }
                return false;
            }
            "remove_download" => {
                self.reload_downloads();
                self.downloads.retain(|d| d["path"] != command["value"]);
                self.save_downloads();
                return false;
            }
            "clear_downloads" => {
                self.reload_downloads();
                self.downloads.retain(|d| d["state"] == "downloading");
                self.save_downloads();
                return false;
            }
            "install_app" => {
                if let Err(e) = std::env::current_exe().and_then(|exe| std::process::Command::new(exe).arg("--install").spawn()) {
                    show_error(&format!("Could not start Riffle Setup: {e}"));
                }
                return false;
            }
            "make_default" => {
                if is_installed() {
                    let _ = std::process::Command::new(system32("..\\explorer.exe")).arg("ms-settings:defaultapps?registeredAppUser=Riffle").spawn();
                }
                return false;
            }
            "choose_download_dir" => {
                let current = self.settings["download_dir"].as_str().unwrap_or("").to_string();
                if let Some(dir) = pick_path(self.window.hwnd() as isize, true, &current, "") {
                    self.settings["download_dir"] = json!(dir.to_string_lossy());
                    set_allowed_sites(&self.settings);
                    self.save_settings();
                    self.render_panel();
                }
                return false;
            }
            "uninstall_app" => {
                self.save_session();
                if let Ok(exe) = std::env::current_exe() {
                    let _ = std::process::Command::new(exe).arg("--uninstall").spawn();
                }
                return false;
            }
            "remove_bookmark" => {
                self.reload_bookmarks();
                self.bookmarks.retain(|b| b["url"] != command["value"]);
                self.save_bookmarks();
                self.render_panel();
                self.layout();
                self.sync_toolbar();
                return false;
            }
            "remove_history" => {
                let path = self.history_path();
                let Some(text) = read_lossy(&path) else { return false };
                let kept = remove_history_entry(&text, &command["value"], &command["time"]);
                write_replacing(&path, &kept).unwrap_or_else(|e| show_error(&format!("Could not save history: {e}")));
                self.last_visit = Default::default();
                self.render_panel();
                return false;
            }
            "clear_history" => {
                let _ = std::fs::remove_file(self.history_path());
                self.last_visit = Default::default();
                self.render_panel();
                return false;
            }
            "clear_data" => {
                let _ = std::fs::remove_file(self.history_path());
                self.last_visit = Default::default();
                self.closed.clear();
                self.downloads.clear();
                self.save_downloads();
                // The panel, not the active tab: a private tab would clear only its own session.
                if let Some(panel) = &self.panel {
                    let _ = panel.clear_all_browsing_data();
                }
                self.render_panel();
                return false;
            }
            "settings" => {
                self.settings = sanitize_settings(&command["value"]);
                set_allowed_sites(&self.settings);
                self.save_settings();
                if let Some(panel) = &self.panel {
                    let _ = panel.evaluate_script(&format!("showSettings({})", self.settings));
                }
                self.layout();
                self.sync_toolbar();
                return false;
            }
            "select" => {
                id.map(|id| self.select(id));
                return false;
            }
            "close" => return id.is_some_and(|id| self.close_tab(id)),
            "close_window" => return true,
            "minimize" => {
                self.window.set_minimized(true);
                return false;
            }
            "maximize" => {
                self.window.set_maximized(!self.window.is_maximized());
                return false;
            }
            "drag" => {
                let _ = self.window.drag_window();
                return false;
            }
            "check_updates" => {
                if let Some(dir) = install_dir().filter(|_| is_installed() && self.update_ready.is_none()) {
                    self.update_status = "checking";
                    self.render_panel();
                    let proxy = self.proxy.clone();
                    std::thread::spawn(move || drop(proxy.send_event(Msg::UpdateChecked(update_once(&dir)))));
                }
                return false;
            }
            "about_link" => {
                let url = match command["link"].as_str().unwrap_or("") {
                    "website" => "https://gagankarthik.github.io/riffle-browser/",
                    "notes" => "https://github.com/gagankarthik/riffle-browser/releases",
                    "source" => "https://github.com/gagankarthik/riffle-browser",
                    "license" => "https://github.com/gagankarthik/riffle-browser/blob/main/LICENSE",
                    _ => return false,
                };
                self.close_panel();
                self.open_tab(url, false);
                return false;
            }
            "tasks_refresh" => {
                self.request_processes();
                return false;
            }
            "resize_top" => {
                let _ = self.window.drag_resize_window(ResizeDirection::North);
                return false;
            }
            "go" => {
                self.close_suggestions();
                if let Some(tab) = self.tabs.iter().find(|t| t.id == self.active) {
                    tab.guard.borrow_mut().vouch();
                }
                if let Some(url) = command["value"].as_str().and_then(|v| address_to_url(v, self.search())) {
                    self.close_panel();
                    if let Some(view) = self.active_view() {
                        let _ = view.load_url(&url);
                        let _ = view.focus();
                    }
                }
                return false;
            }
            "back" => {
                self.vouch_active();
                self.close_panel();
                "history.back()"
            }
            "forward" => {
                self.vouch_active();
                self.close_panel();
                "history.forward()"
            }
            "stop" => "window.stop()",
            "reload" => {
                self.close_panel();
                if let Some(view) = self.active_view() {
                    let _ = view.reload();
                }
                return false;
            }
            _ => return false,
        };
        if let Some(view) = self.active_view() {
            let _ = view.evaluate_script(script);
        }
        false
    }
}

/// Least recently used tabs to drop so at most `max_live` stay in memory. Never the active tab.
fn tabs_to_sleep(live: &[(u64, u64)], active: u64, max_live: usize) -> Vec<u64> {
    let mut others: Vec<(u64, u64)> = live.iter().copied().filter(|(id, _)| *id != active).collect();
    others.sort_by_key(|(_, used)| *used);
    let excess = (others.len() + 1).saturating_sub(max_live);
    others.into_iter().take(excess).map(|(id, _)| id).collect()
}

/// Turns address bar text into a URL. Anything that is not clearly a web address becomes a search,
/// so javascript: and file: text can never run or open.
fn address_to_url(input: &str, search: &str) -> Option<String> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }
    let lower = input.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        if let Ok(url) = Url::parse(input) {
            return Some(url.to_string());
        }
    } else if !input.contains(char::is_whitespace) && !input.contains("://") {
        if let Ok(url) = Url::parse(&format!("https://{input}")) {
            match url.host() {
                Some(Host::Domain("localhost")) => return Some(format!("http://{input}")),
                Some(Host::Domain(domain)) => {
                    let tld = domain.rsplit('.').next().unwrap_or("");
                    if domain.contains('.') && tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic()) {
                        return Some(url.to_string());
                    }
                }
                Some(Host::Ipv4(ip)) if input.split(['/', ':']).next() == Some(ip.to_string().as_str()) => {
                    return Some(format!("http://{input}"));
                }
                _ => {}
            }
        }
    }
    let query: String = url::form_urlencoded::byte_serialize(input.as_bytes()).collect();
    Some(format!("{search}{query}"))
}

/// Runs on its own thread, so startup never waits for a download or for parsing the lists.
fn load_filters(data_dir: PathBuf, download: bool) {
    std::thread::spawn(move || {
        let refresh = |file: &str, url: &str, valid: fn(&str) -> bool| {
            let path = data_dir.join(file);
            let age = std::fs::metadata(&path).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok());
            if download && age.is_none_or(|a| a.as_secs() > FILTER_MAX_AGE_SECS) {
                download_file(url, &path, valid); // on failure the older copy, if any, keeps working
            }
        };
        for (file, url) in FILTER_LISTS {
            refresh(file, url, is_filter_list);
        }
        refresh(SCRIPTLETS.0, SCRIPTLETS.1, |t| t.starts_with('['));
        let newest = FILTER_LISTS.iter().filter_map(|(file, _)| std::fs::metadata(data_dir.join(file)).and_then(|m| m.modified()).ok()).max();
        let cached = cached_engine(&data_dir, "engine.dat", newest).zip(cached_engine(&data_dir, "popup.dat", newest));
        let (mut engine, popups) = match cached {
            Some(both) => both,
            None => {
                let lists: Vec<String> = FILTER_LISTS.iter().filter_map(|(file, _)| std::fs::read_to_string(data_dir.join(file)).ok()).collect();
                if lists.is_empty() {
                    return;
                }
                let popups = build_engine(vec![popup_rules(&lists)]);
                let engine = build_engine(lists);
                save_engine_cache(&data_dir, &engine, &popups);
                (engine, popups)
            }
        };
        let _ = POPUP_ENGINE.set(popups);
        // A missing or unreadable file only means no scriptlets: blocking itself still works.
        let scripts = std::fs::read_to_string(data_dir.join(SCRIPTLETS.0)).ok().and_then(|t| serde_json::from_str::<Vec<adblock::resources::Resource>>(&t).ok());
        engine.use_resources(built_in_resources().into_iter().chain(scripts.unwrap_or_default()));
        let _ = ENGINE.set(engine);
    });
}

/// The page scripts and harmless stand-ins that filter rules ask for by name. Built in, so they can never be missing.
fn built_in_resources() -> Vec<adblock::resources::Resource> {
    use adblock::resources::{MimeType, Resource, ResourceType};
    let resource = |names: &[&str], kind: ResourceType, body: &[u8]| Resource {
        name: names[0].to_string(),
        aliases: names[1..].iter().map(|n| n.to_string()).collect(),
        kind,
        content: base64(body),
        dependencies: Vec::new(),
        permission: Default::default(),
    };
    let text = SCRIPTLETS_JS.replace("\r\n", "\n");
    let mut sections = text.split("\n/// ");
    sections.next(); // the explanation at the top
    let prelude = sections.next().and_then(|s| s.split_once('\n')).map_or("", |s| s.1);
    let mut out: Vec<Resource> = sections
        .filter_map(|section| section.split_once('\n'))
        .map(|(names, body)| {
            let names: Vec<&str> = names.split_whitespace().collect();
            resource(&names, ResourceType::Template, format!("(()=>{{{prelude}\n{body}\n}})();").as_bytes())
        })
        .collect();
    let gif = decode_base64("R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7").unwrap_or_default();
    let (js, none) = (MimeType::ApplicationJavascript, MimeType::TextPlain);
    out.push(resource(&["noop.js", "noopjs"], ResourceType::Mime(js.clone()), b"(function(){})();"));
    out.push(resource(&["noop.txt", "nooptext"], ResourceType::Mime(none), b""));
    out.push(resource(&["noop.json", "noopjson"], ResourceType::Mime(MimeType::ApplicationJson), b"{}"));
    out.push(resource(&["1x1.gif", "1x1-transparent.gif"], ResourceType::Mime(MimeType::ImageGif), &gif));
    out.push(resource(&["googlesyndication_adsbygoogle.js", "googlesyndication.com/adsbygoogle.js"], ResourceType::Mime(js), b"window.adsbygoogle={loaded:true,push(){}};"));
    out
}

/// The compiled rules from last time, which load far faster than parsing every list, and use less memory doing it.
/// Only if nothing has changed since: a newer list or a new Riffle version means parsing again.
fn cached_engine(dir: &Path, file: &str, newest_list: Option<std::time::SystemTime>) -> Option<Engine> {
    if std::fs::read_to_string(dir.join("engine.version")).ok()? != env!("CARGO_PKG_VERSION") {
        return None;
    }
    let path = dir.join(file);
    if std::fs::metadata(&path).and_then(|m| m.modified()).ok()? < newest_list? {
        return None;
    }
    let mut engine = build_engine(Vec::new());
    engine.deserialize(&std::fs::read(path).ok()?).ok()?; // a damaged file is rejected, and the lists are parsed instead
    Some(engine)
}

fn save_engine_cache(dir: &Path, engine: &Engine, popups: &Engine) {
    for (file, data) in [("engine.dat", engine.serialize()), ("popup.dat", popups.serialize())] {
        let temp = dir.join(format!("{file}.{}", std::process::id()));
        if std::fs::write(&temp, data).is_err() || std::fs::rename(&temp, dir.join(file)).is_err() {
            let _ = std::fs::remove_file(temp);
            return;
        }
    }
    let _ = std::fs::write(dir.join("engine.version"), env!("CARGO_PKG_VERSION"));
}

/// The $popup rules from every list, with that option removed so the engine accepts them.
fn popup_rules(lists: &[String]) -> String {
    let mut rules = String::new();
    for line in lists.iter().flat_map(|l| l.lines()) {
        let Some((pattern, options)) = line.rsplit_once('$') else { continue };
        let is_popup = |o: &&str| matches!(*o, "popup" | "popunder");
        if line.starts_with('!') || !options.split(',').any(|o| is_popup(&o)) {
            continue;
        }
        let rest: Vec<&str> = options.split(',').filter(|o| !is_popup(o)).collect();
        rules.push_str(pattern);
        if !rest.is_empty() {
            rules.push('$');
            rules.push_str(&rest.join(","));
        }
        rules.push('\n');
    }
    rules
}

fn set_allowed_sites(settings: &Value) {
    let read = |key: &str| settings[key].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect::<Vec<_>>();
    if let (Ok(mut sites), Ok(mut popups)) = (ALLOWED_SITES.lock(), POPUP_SITES.lock()) {
        *sites = read("allow_sites");
        *popups = read("allow_popups");
    }
    if let Ok(mut prefs) = DOWNLOAD_PREFS.lock() {
        *prefs = (settings["download_dir"].as_str().unwrap_or("").to_string(), settings["download_ask"] == true);
    }
}

fn host_listed(list: &Mutex<Vec<String>>, url: &str) -> bool {
    let Some(host) = Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_string)) else { return false };
    list.lock().is_ok_and(|sites| sites.iter().any(|s| host == *s || host.ends_with(&format!(".{s}"))))
}

/// Sites where the user switched blocking off, with their subdomains.
fn site_is_allowed(url: &str) -> bool {
    host_listed(&ALLOWED_SITES, url)
}

/// Sites the user lets open pop-ups and redirects, with their subdomains.
fn popups_are_allowed(url: &str) -> bool {
    host_listed(&POPUP_SITES, url)
}

/// shortcut: guesses the registrable part of a host from its shape instead of the public suffix list, so
/// a.github.io and b.github.io count as one site. Enough for spotting redirects to unrelated sites.
fn site_of(url: &str) -> Option<String> {
    let host = Url::parse(url).ok()?.host_str()?.to_string();
    let labels: Vec<&str> = host.split('.').collect();
    let two_part_suffix = labels.len() >= 3 && labels[labels.len() - 1].len() == 2 && ["co", "com", "org", "net", "ac", "gov", "edu"].contains(&labels[labels.len() - 2]);
    Some(labels[labels.len().saturating_sub(if two_part_suffix { 3 } else { 2 })..].join("."))
}

fn same_site(a: &str, b: &str) -> bool {
    site_of(a).is_some() && site_of(a) == site_of(b)
}

fn trash_rule(url: &str, source: &str) -> bool {
    POPUP_ENGINE.get().is_some_and(|e| is_blocked(e, url, source, "subdocument"))
}

fn ad_rule(url: &str, source: &str) -> bool {
    ENGINE.get().is_some_and(|e| is_blocked(e, url, source, "subdocument"))
}

/// Should this top-level navigation go ahead?
fn navigation_allowed(guard: &Guard, url: &str) -> bool {
    let source = &guard.top_url;
    if guard.vouched() || same_site(source, url) || site_is_allowed(source) || site_is_allowed(url) || popups_are_allowed(source) {
        return true;
    }
    navigation_verdict(trash_rule(url, source), ad_rule(url, source), guard.recent_click())
}

/// The redirects this stops: to a known trash site, to a known ad server unless a real link was clicked, and
/// the signature of click hijacking, a click on something that is not a link followed by a jump to another site.
fn navigation_verdict(trash: bool, ad: bool, click: Option<bool>) -> bool {
    !trash && (click == Some(true) || (!ad && click != Some(false)))
}

/// A pop-up needs a real link click behind it, or to stay on the same site. Anything else is held for the shield.
fn popup_allowed(guard: &Guard, url: &str) -> bool {
    let source = &guard.top_url;
    if site_is_allowed(source) || site_is_allowed(url) || popups_are_allowed(source) {
        return true;
    }
    popup_verdict(trash_rule(url, source), same_site(source, url), guard.recent_click())
}

fn popup_verdict(trash: bool, same_site: bool, click: Option<bool>) -> bool {
    !trash && (click == Some(true) || (same_site && click.is_some()))
}

// curl.exe ships with Windows. The full path stops a planted curl.exe next to Riffle from running.
fn download_file(url: &str, path: &Path, valid: fn(&str) -> bool) {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let curl = PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into())).join("System32\\curl.exe");
    let temp = path.with_extension("download");
    let fetched = std::process::Command::new(curl).args(["-fsSL", "--max-time", "120", "-o"]).arg(&temp).arg(url).creation_flags(CREATE_NO_WINDOW).status();
    if fetched.is_ok_and(|s| s.success()) && std::fs::read_to_string(&temp).is_ok_and(|t| valid(&t)) {
        let _ = std::fs::rename(&temp, path);
    } else {
        let _ = std::fs::remove_file(&temp);
    }
}

/// A captive portal's login page must not replace a working list.
fn is_filter_list(text: &str) -> bool {
    (text.starts_with('!') || text.starts_with("[Adblock Plus")) && text.len() >= MIN_FILTER_LIST_BYTES
}

fn build_engine(lists: Vec<String>) -> Engine {
    let mut filters = FilterSet::new(false); // false: no debug info, which would cost memory for every rule
    for list in lists {
        filters.add_filter_list(list, ParseOptions::default());
    }
    Engine::new_with_filter_set(filters)
}

fn seed_engine() -> Engine {
    build_engine(vec![BLOCKED_HOSTS.iter().map(|h| format!("||{h}^\n")).collect()])
}

fn is_blocked(engine: &Engine, url: &str, source: &str, kind: &str) -> bool {
    adblock::request::Request::new(url, source, kind, "GET").is_ok_and(|r| engine.check_network_request(&r).should_block())
}

/// One rule per selector, because a single selector the engine cannot parse would void a combined rule.
fn ad_css(engine: &Engine, url: &str, first: bool, classes: &[String], ids: &[String]) -> String {
    let found = engine.url_cosmetic_resources(url);
    let mut selectors: Vec<String> = if first { found.hide_selectors.iter().cloned().collect() } else { Vec::new() };
    if !found.generichide {
        selectors.extend(engine.hidden_class_id_selectors(classes.iter().map(String::as_str), ids.iter().map(String::as_str), &found.exceptions));
    }
    selectors.iter().map(|s| format!("{s}{{display:none!important}}\n")).collect()
}

fn resource_kind(context: COREWEBVIEW2_WEB_RESOURCE_CONTEXT) -> &'static str {
    match context {
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT => "subdocument",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_STYLESHEET => "stylesheet",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_IMAGE => "image",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_MEDIA => "media",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FONT => "font",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_SCRIPT => "script",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_XML_HTTP_REQUEST | COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FETCH => "xhr",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_WEBSOCKET => "websocket",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_PING => "ping",
        _ => "other",
    }
}

/// What this site's rules want done to the page, safe to run more than once.
fn early_script(engine: &Engine, url: &str) -> String {
    let found = engine.url_cosmetic_resources(url);
    if found.hide_selectors.is_empty() && found.procedural_actions.is_empty() && found.injected_script.is_empty() {
        return String::new();
    }
    let css: String = found.hide_selectors.iter().map(|s| format!("{s}{{display:none!important}}\n")).collect();
    let rules: Vec<Value> = found.procedural_actions.iter().filter_map(|r| serde_json::from_str(r).ok()).collect();
    let procedural = if rules.is_empty() { String::new() } else { format!("{PROCEDURAL_SCRIPT}({});", Value::Array(rules)) };
    format!(
        "if(window===top&&!window.__riffleRules){{window.__riffleRules=document.scripts.length+1;
(c=>{{const a=()=>{{if(!document.documentElement)return new MutationObserver((_,o)=>{{if(document.documentElement){{o.disconnect();a()}}}}).observe(document,{{childList:true}});const s=document.createElement('style');s.textContent=c;document.documentElement.appendChild(s)}};if(c)a()}})({});
{procedural}
try{{{}}}catch(e){{}}
}}",
        json!(css),
        found.injected_script
    )
}

// Registered as navigation starts, so the page's own scripts, including anti-adblock checks, run after these.
fn early_scriptlets(view: &WebView) {
    let current: Rc<RefCell<Option<String>>> = Rc::default();
    // SAFETY: COM calls on the UI thread that owns this web view; strings outlive the calls that read them.
    unsafe {
        let handler = NavigationStartingEventHandler::create(Box::new(move |sender, args| {
            let (Some(sender), Some(args)) = (sender, args) else { return Ok(()) };
            if let Some(old) = current.borrow_mut().take() {
                sender.RemoveScriptToExecuteOnDocumentCreated(&windows::core::HSTRING::from(old))?;
            }
            let Some(engine) = ENGINE.get() else { return Ok(()) };
            let mut raw = PWSTR::null();
            args.Uri(&mut raw)?;
            let url = take_pwstr(raw);
            if !is_web(&url) || site_is_allowed(&url) {
                return Ok(());
            }
            let script = early_script(engine, &url);
            if !script.is_empty() {
                let keep = current.clone();
                let done = AddScriptToExecuteOnDocumentCreatedCompletedHandler::create(Box::new(move |_, id| {
                    *keep.borrow_mut() = Some(id);
                    Ok(())
                }));
                sender.AddScriptToExecuteOnDocumentCreated(&windows::core::HSTRING::from(script), &done)?;
            }
            Ok(())
        }));
        let mut token = Default::default();
        let _ = view.webview().add_NavigationStarting(&handler, &mut token);
    }
}

/// What to do with a request: None lets it through, Some(None) blocks it, Some(Some(data)) answers with a harmless stand-in.
// The small seed list is only for before the full lists load: after that it must not override their exceptions.
fn request_verdict(url: &str, source: &str, kind: &str) -> Option<Option<String>> {
    let engine = ENGINE.get().or(SEED_ENGINE.get())?;
    let request = adblock::request::Request::new(url, source, kind, "GET").ok()?;
    let result = engine.check_network_request(&request);
    result.should_block().then_some(result.redirect)
}

/// A stand-in's bytes and type, from the data address the engine gives.
fn parse_stand_in(data: &str) -> Option<(String, Vec<u8>)> {
    let (kind, body) = data.strip_prefix("data:")?.split_once(";base64,")?;
    Some((kind.to_string(), decode_base64(body)?))
}

fn decode_base64(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut bits, mut count) = (0u32, 0);
    for byte in text.bytes().filter(|b| *b != b'=') {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        bits = bits << 6 | value as u32;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
        }
    }
    Some(out)
}

// Every request from the page passes through here. A blocked one gets an empty 403 and never reaches the network.
fn block_requests(view: &WebView, guard: Rc<RefCell<Guard>>, refresh: EventLoopProxy<Msg>) {
    let webview = view.webview();
    let environment = view.environment();
    // SAFETY: COM calls on the UI thread that owns this web view; returned strings are taken once by take_pwstr.
    unsafe {
        let _ = webview.AddWebResourceRequestedFilter(w!("*"), COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL);
        let mut token = Default::default();
        let handler = WebResourceRequestedEventHandler::create(Box::new(move |sender, args| {
            let (Some(args), Some(sender)) = (args, sender) else { return Ok(()) };
            let mut raw = PWSTR::null();
            args.Request()?.Uri(&mut raw)?;
            let url = take_pwstr(raw);
            let mut context = COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL;
            args.ResourceContext(&mut context)?;
            // A navigation just stopped must not reach the site's server at all, even as a bare request.
            if context == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT && guard.borrow().stopped.as_deref() == Some(url.as_str()) {
                let response = environment.CreateWebResourceResponse(None::<&windows::Win32::System::Com::IStream>, 403, w!("Blocked"), PCWSTR::null())?;
                args.SetResponse(&response)?;
                return Ok(());
            }
            // The page the user asked for is never blocked, only what it embeds.
            if context == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT && guard.borrow().nav_url == url {
                return Ok(());
            }
            let mut raw = PWSTR::null();
            sender.Source(&mut raw)?;
            let source = take_pwstr(raw);
            if site_is_allowed(&source) {
                return Ok(());
            }
            if let Some(stand_in) = request_verdict(&url, &source, resource_kind(context)) {
                let count = {
                    let mut guard = guard.borrow_mut();
                    guard.blocked += 1;
                    guard.blocked
                };
                if count == 1 || count % 10 == 0 {
                    drop(refresh.send_event(Msg::Refresh));
                }
                BLOCKED_NOW.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                // A site that checks its ad script loaded gets a harmless copy, so it neither breaks nor accuses the visitor.
                let response = match stand_in.as_deref().and_then(parse_stand_in) {
                    Some((kind, body)) => {
                        let stream = SHCreateMemStream(Some(&body));
                        let headers = windows::core::HSTRING::from(format!("Content-Type: {kind}\nAccess-Control-Allow-Origin: *"));
                        environment.CreateWebResourceResponse(stream.as_ref(), 200, w!("OK"), &headers)?
                    }
                    None => environment.CreateWebResourceResponse(None::<&windows::Win32::System::Com::IStream>, 403, w!("Blocked"), PCWSTR::null())?,
                };
                args.SetResponse(&response)?;
            }
            Ok(())
        }));
        let _ = webview.add_WebResourceRequested(&handler, &mut token);
    }
}

fn key_shortcut(key: u32, shift: bool) -> Option<&'static str> {
    // The plus and minus keys, on the main row and the number pad. Shift is how some keyboards reach plus.
    match key {
        0xBB | 0x6B => return Some("zoom_in"),
        0xBD | 0x6D => return Some("zoom_out"),
        0x30 | 0x60 if !shift => return Some("zoom_reset"),
        _ => {}
    }
    Some(match (key as u8 as char, shift) {
        ('T', false) => "new_tab",
        ('T', true) => "reopen_tab",
        ('W', false) => "close_tab",
        ('L', false) => "focus_address",
        ('D', false) => "bookmark",
        ('H', false) => "history",
        ('J', false) => "downloads",
        ('N', false) => "new_window",
        ('N', true) => "private_tab",
        ('U', false) => "view_source",
        ('I' | 'J' | 'C', true) => "dev_tools",
        ('O', true) => "bookmarks",
        ('A', true) => "tab_search",
        ('P', true) => "picture_in_picture",
        ('\t', false) => "next_tab",
        ('\t', true) => "prev_tab",
        _ => return None,
    })
}

// Real key presses as the engine reports them. A page cannot fake these or block them.
fn native_keys(view: &WebView, id: u64, proxy: EventLoopProxy<Msg>) {
    const VK_SHIFT: i32 = 0x10;
    const VK_CONTROL: i32 = 0x11;
    const VK_MENU: i32 = 0x12;
    let down = |key: i32| unsafe { GetKeyState(key) } < 0; // SAFETY: takes a plain value and keeps nothing
    // SAFETY: COM calls on the UI thread that owns this web view, writing into local values.
    unsafe {
        let engine = view.webview();
        let handler = AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else { return Ok(()) };
            let (mut kind, mut key, mut status) = (COREWEBVIEW2_KEY_EVENT_KIND_KEY_UP, 0u32, COREWEBVIEW2_PHYSICAL_KEY_STATUS::default());
            args.KeyEventKind(&mut kind)?;
            args.VirtualKey(&mut key)?;
            args.PhysicalKeyStatus(&mut status)?;
            if kind == COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN && !status.WasKeyDown.as_bool() {
                const VK_F12: u32 = 0x7B;
                const VK_ESCAPE: u32 = 0x1B;
                let mut full = BOOL(0);
                let name = if key == VK_ESCAPE {
                    // Only while fullscreen: everywhere else Esc belongs to the page.
                    engine.ContainsFullScreenElement(&mut full)?;
                    full.as_bool().then_some("exit_fullscreen")
                } else if key == VK_F12 { Some("dev_tools") } else if down(VK_CONTROL) && !down(VK_MENU) { key_shortcut(key, down(VK_SHIFT)) } else { None };
                if let Some(name) = name {
                    args.SetHandled(true)?;
                    drop(proxy.send_event(Msg::Key(id, name)));
                }
            }
            Ok(())
        }));
        let mut token = 0;
        let _ = view.controller().add_AcceleratorKeyPressed(&handler, &mut token);
    }
}

// The engine reports this for any element in any frame, so every site's own fullscreen button works.
fn report_fullscreen(view: &WebView, id: u64, proxy: EventLoopProxy<Msg>) {
    // SAFETY: COM calls on the UI thread that owns this web view, writing into local values.
    unsafe {
        let handler = ContainsFullScreenElementChangedEventHandler::create(Box::new(move |sender, _| {
            let Some(sender) = sender else { return Ok(()) };
            let mut on = BOOL(0);
            sender.ContainsFullScreenElement(&mut on)?;
            drop(proxy.send_event(Msg::Fullscreen(id, on.as_bool())));
            Ok(())
        }));
        let mut token = 0;
        let _ = view.webview().add_ContainsFullScreenElementChanged(&handler, &mut token);
    }
}

/// Memory only this process holds, the figure Windows Task Manager shows by default.
fn process_memory(pid: u32) -> usize {
    // SAFETY: Windows checks the id and the handle, and the counters are a local value of the size given.
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return 0 };
        let mut counters = PROCESS_MEMORY_COUNTERS_EX::default();
        let size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
        let ok = K32GetProcessMemoryInfo(handle, &mut counters as *mut _ as *mut PROCESS_MEMORY_COUNTERS, size).as_bool();
        let _ = CloseHandle(handle);
        if ok { counters.PrivateUsage } else { 0 }
    }
}

/// The next step up or down from any level, staying within what the engine supports.
fn next_zoom(current: f64, how: &str) -> f64 {
    let (first, last) = (ZOOM_LEVELS[0], ZOOM_LEVELS[ZOOM_LEVELS.len() - 1]);
    match how {
        "in" => ZOOM_LEVELS.iter().copied().find(|z| *z > current + 0.001).unwrap_or(last),
        "out" => ZOOM_LEVELS.iter().rev().copied().find(|z| *z < current - 0.001).unwrap_or(first),
        "reset" => 1.0,
        _ => current,
    }
}

fn set_view_zoom(view: &WebView, zoom: f64) {
    // SAFETY: a COM call on the UI thread that owns this web view.
    let _ = unsafe { view.controller().SetZoomFactor(zoom) };
}

/// The speakers or headphones Windows is using now, looked up each time because the user can switch them.
fn system_audio() -> Option<IAudioEndpointVolume> {
    // SAFETY: COM calls on the UI thread, where COM is already initialized for the web views.
    unsafe {
        let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER).ok()?;
        devices.GetDefaultAudioEndpoint(eRender, eConsole).ok()?.Activate(CLSCTX_ALL, None).ok()
    }
}

/// (level from 0 to 1, muted). None when the computer has no audio output.
fn system_volume() -> Option<(f64, bool)> {
    let audio = system_audio()?;
    // SAFETY: plain queries on a COM object owned here.
    unsafe { Some((audio.GetMasterVolumeLevelScalar().ok()? as f64, audio.GetMute().ok()?.as_bool())) }
}

fn set_system_volume(level: f64) {
    if let Some(audio) = system_audio() {
        // SAFETY: a COM call on an object owned here. The range is what the API accepts.
        unsafe {
            let _ = audio.SetMasterVolumeLevelScalar(level.clamp(0.0, 1.0) as f32, std::ptr::null());
            if level > 0.0 {
                let _ = audio.SetMute(false, std::ptr::null()); // moving the slider up should be heard
            }
        }
    }
}

fn set_system_mute(muted: bool) {
    if let Some(audio) = system_audio() {
        // SAFETY: a COM call on an object owned here.
        let _ = unsafe { audio.SetMute(muted, std::ptr::null()) };
    }
}

/// Year and month, such as 2026-11, from seconds since 1970. Howard Hinnant's civil-from-days.
fn month_key(secs: u64) -> String {
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{}-{month:02}", yoe + era * 400 + i64::from(month <= 2))
}

/// Only counts by month, so nothing here says what was visited. Adds to the file, because other windows write to it too.
fn save_blocked_count(dir: &Path) {
    let new = BLOCKED_NOW.swap(0, std::sync::atomic::Ordering::Relaxed);
    if new > 0 {
        let path = dir.join("stats.json");
        let mut stats = load_json(&path);
        if !stats.is_object() {
            stats = json!({});
        }
        let key = month_key(now_secs());
        stats[&key] = json!(stats[&key].as_u64().unwrap_or(0) + new);
        save_json(&path, &stats);
    }
}

fn blocked_this_month(dir: &Path) -> u64 {
    load_json(&dir.join("stats.json"))[month_key(now_secs())].as_u64().unwrap_or(0) + BLOCKED_NOW.load(std::sync::atomic::Ordering::Relaxed)
}

/// Where a tab may sit: pinned tabs keep to the left group, the rest keep to the right of it.
fn tab_position(wanted: usize, pinned_tabs: usize, total: usize, is_pinned: bool) -> usize {
    if is_pinned { wanted.min(pinned_tabs.saturating_sub(1)) } else { wanted.clamp(pinned_tabs, total.saturating_sub(1)) }
}

/// Every web bookmark in a Chrome or Edge Bookmarks file, as (title, address).
fn parse_browser_bookmarks(text: &str) -> Vec<(String, String)> {
    fn walk(node: &Value, out: &mut Vec<(String, String)>) {
        if out.len() >= IMPORT_LIMIT {
            return;
        }
        if node["type"] == "url" {
            if let Some(url) = node["url"].as_str().filter(|u| is_web(u)) {
                out.push((node["name"].as_str().unwrap_or(url).to_string(), url.to_string()));
            }
        }
        for child in node["children"].as_array().into_iter().flatten() {
            walk(child, out);
        }
    }
    let parsed: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let mut out = Vec::new();
    if let Some(roots) = parsed["roots"].as_object() {
        for root in roots.values() {
            walk(root, &mut out);
        }
    }
    out
}

/// Adds what is not already saved and returns how many that was. Existing names are left alone.
fn merge_bookmarks(bookmarks: &mut Vec<Value>, found: Vec<(String, String)>) -> usize {
    let before = bookmarks.len();
    for (title, url) in found {
        if !bookmarks.iter().any(|b| b["url"] == url.as_str()) {
            let title = if title.trim().is_empty() { url.clone() } else { title };
            bookmarks.push(json!({"url": url, "title": title}));
        }
    }
    bookmarks.len() - before
}

/// (page title, what happened, what to try) for a failed page. None for what is not a failure to explain, such as
/// the user stopping it, a download, or a site asking for a password.
fn error_page_text(status: COREWEBVIEW2_WEB_ERROR_STATUS) -> Option<(&'static str, &'static str, &'static [&'static str])> {
    const CHECK_CONNECTION: &[&str] = &["Check your Wi-Fi or network cable", "Try again in a moment"];
    Some(match status {
        COREWEBVIEW2_WEB_ERROR_STATUS_DISCONNECTED => ("No internet connection", "Riffle can't reach the internet right now. This page will come back by itself once you're connected.", CHECK_CONNECTION),
        COREWEBVIEW2_WEB_ERROR_STATUS_TIMEOUT => ("This site took too long to respond", "Your connection may be slow, or the site may be busy.", CHECK_CONNECTION),
        COREWEBVIEW2_WEB_ERROR_STATUS_HOST_NAME_NOT_RESOLVED => ("This site can't be found", "The address may be misspelled, or the site may no longer exist.", &["Check the address for typing mistakes", "Check your internet connection"]),
        COREWEBVIEW2_WEB_ERROR_STATUS_CANNOT_CONNECT | COREWEBVIEW2_WEB_ERROR_STATUS_SERVER_UNREACHABLE => ("Can't connect to this site", "The site isn't accepting connections right now. It may be down, or your network may be blocking it.", &["Try again in a moment", "Check your internet connection"]),
        COREWEBVIEW2_WEB_ERROR_STATUS_CONNECTION_RESET => ("The connection was interrupted", "The connection to this site dropped while the page was loading.", CHECK_CONNECTION),
        COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_EXPIRED | COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_COMMON_NAME_IS_INCORRECT | COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_IS_INVALID | COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_REVOKED | COREWEBVIEW2_WEB_ERROR_STATUS_CLIENT_CERTIFICATE_CONTAINS_ERRORS => ("This site's connection isn't secure", "Riffle stopped loading this page because its security certificate isn't valid. Someone could be trying to read what you send, so don't enter passwords or card details here.", &["Check your computer's date and time are right"]),
        COREWEBVIEW2_WEB_ERROR_STATUS_REDIRECT_FAILED => ("This page isn't working", "The site sent Riffle round in a loop.", &["Try again in a moment"]),
        COREWEBVIEW2_WEB_ERROR_STATUS_ERROR_HTTP_INVALID_SERVER_RESPONSE | COREWEBVIEW2_WEB_ERROR_STATUS_UNEXPECTED_ERROR => ("Something went wrong", "This site sent back something Riffle couldn't use.", &["Try again in a moment"]),
        COREWEBVIEW2_WEB_ERROR_STATUS_UNKNOWN => ("Something went wrong", "Riffle couldn't load this page.", &["Try again in a moment", "Check your internet connection"]),
        _ => return None,
    })
}

/// The icon drawing and tone for each failure: info, warning or danger.
fn error_page_look(status: COREWEBVIEW2_WEB_ERROR_STATUS) -> (&'static str, &'static str) {
    const WIFI_OFF: &str = r#"<path d="M2 2l20 20M8.500 16.500a5 5 0 0 1 7 0M2 8.800a15 15 0 0 1 4.200-2.700M10.700 5.100A15 15 0 0 1 22 8.800M5 12.900a10 10 0 0 1 5.200-2.800M18.500 12.900c-.6-.6-1.300-1.100-2-1.500M12 20h.01"/>"#;
    const CLOCK: &str = r#"<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>"#;
    const SEARCH: &str = r#"<circle cx="11" cy="11" r="7"/><path d="m20 20-4-4"/>"#;
    const CLOUD_OFF: &str = r#"<path d="M2 2l20 20M5.800 5.800A7 7 0 0 0 7 19h10c.7 0 1.300-.1 1.900-.4M21.500 16a4.500 4.500 0 0 0-4-6.900h-.8A7 7 0 0 0 10 5.100"/>"#;
    const SHIELD_ALERT: &str = r#"<path d="M12 3 5 6v5c0 4.500 3 8 7 10 4-2 7-5.500 7-10V6zM12 8v4M12 16h.01"/>"#;
    const WARNING: &str = r#"<path d="M12 4 3 20h18zM12 10v4M12 17h.01"/>"#;
    match status {
        COREWEBVIEW2_WEB_ERROR_STATUS_DISCONNECTED | COREWEBVIEW2_WEB_ERROR_STATUS_CONNECTION_RESET => (WIFI_OFF, "info"),
        COREWEBVIEW2_WEB_ERROR_STATUS_TIMEOUT => (CLOCK, "warning"),
        COREWEBVIEW2_WEB_ERROR_STATUS_HOST_NAME_NOT_RESOLVED => (SEARCH, "info"),
        COREWEBVIEW2_WEB_ERROR_STATUS_CANNOT_CONNECT | COREWEBVIEW2_WEB_ERROR_STATUS_SERVER_UNREACHABLE => (CLOUD_OFF, "warning"),
        COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_EXPIRED | COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_COMMON_NAME_IS_INCORRECT | COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_IS_INVALID | COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_REVOKED | COREWEBVIEW2_WEB_ERROR_STATUS_CLIENT_CERTIFICATE_CONTAINS_ERRORS => (SHIELD_ALERT, "danger"),
        _ => (WARNING, "warning"),
    }
}

fn html_text(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn error_page(look: (&str, &str), title: &str, detail: &str, tips: &[&str], site: &str, action: &str, retry_online: bool) -> String {
    let tips: String = tips.iter().map(|tip| format!("<li>{}</li>", html_text(tip))).collect();
    ERROR_HTML
        .replace("__ICON__", look.0)
        .replace("__TONE__", look.1)
        .replace("__TITLE__", &html_text(title))
        .replace("__DETAIL__", &html_text(detail))
        .replace("__TIPS__", &tips)
        .replace("__SITE__", &html_text(site))
        .replace("__ACTION__", &html_text(action))
        .replace("__RETRY_ONLINE__", if retry_online { "true" } else { "false" })
}

fn report_navigation_results(view: &WebView, id: u64, proxy: EventLoopProxy<Msg>) {
    // SAFETY: COM calls on the UI thread that owns this web view, writing into local values.
    unsafe {
        let handler = NavigationCompletedEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else { return Ok(()) };
            let (mut ok, mut status) = (BOOL(0), COREWEBVIEW2_WEB_ERROR_STATUS_UNKNOWN);
            args.IsSuccess(&mut ok)?;
            args.WebErrorStatus(&mut status)?;
            drop(proxy.send_event(Msg::NavFinished(id, status, ok.as_bool())));
            Ok(())
        }));
        let mut token = Default::default();
        let _ = view.webview().add_NavigationCompleted(&handler, &mut token);
    }
}

fn report_crashes(view: &WebView, id: u64, proxy: EventLoopProxy<Msg>) {
    // SAFETY: COM calls on the UI thread that owns this web view, writing into a local value.
    unsafe {
        let handler = ProcessFailedEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else { return Ok(()) };
            let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED;
            args.ProcessFailedKind(&mut kind)?;
            if kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED || kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_UNRESPONSIVE {
                drop(proxy.send_event(Msg::TabCrashed(id)));
            }
            Ok(())
        }));
        let mut token = Default::default();
        let _ = view.webview().add_ProcessFailed(&handler, &mut token);
    }
}

/// Floats the page's playing video, or brings it back. Through DevTools, because a browser only allows this after
/// a user gesture, and the gesture happened in Riffle rather than in the page.
fn toggle_picture_in_picture(view: &WebView) {
    let script = "(()=>{if(document.pictureInPictureElement)return document.exitPictureInPicture();const v=[...document.querySelectorAll('video')].filter(x=>x.readyState>0),el=v.find(x=>!x.paused)||v[0];if(!el)return;el.disablePictureInPicture=false;return el.requestPictureInPicture()})()";
    let params = windows::core::HSTRING::from(json!({"expression": script, "userGesture": true, "awaitPromise": true}).to_string());
    // SAFETY: a COM call on the UI thread, with strings that outlive the call.
    let _ = unsafe { view.webview().CallDevToolsProtocolMethod(w!("Runtime.evaluate"), &params, &CallDevToolsProtocolMethodCompletedHandler::create(Box::new(|_, _| Ok(())))) };
}

/// Tabs playing audio through any route, including players the page script cannot see.
fn plays_audio(view: &WebView) -> bool {
    let mut playing = BOOL(0);
    // SAFETY: COM call on the UI thread writing into a local value.
    view.webview().cast::<ICoreWebView2_8>().is_ok_and(|w| unsafe { w.IsDocumentPlayingAudio(&mut playing) }.is_ok() && playing.as_bool())
}

/// Private tabs never sleep: dropping the last one would erase that private session's logins.
fn must_stay_awake(tab: &Tab) -> bool {
    tab.private || tab.pinned || tab.devtools.is_some() || tab.downloads > 0 || tab.media.is_some_and(|(playing, _)| playing) || tab.view.as_ref().is_some_and(plays_audio)
}

/// Whether a DevTools window's title names this address. Long titles are cut, so a start of it is enough.
fn devtools_title_matches(title: &str, address: &str) -> bool {
    let bare = address.trim_start_matches("https://").trim_start_matches("http://");
    title.strip_prefix("DevTools - ").is_some_and(|shown| !shown.is_empty() && bare.starts_with(shown.trim_end_matches("...")))
}

fn devtools_share_at(x: f64, width: f64) -> f64 {
    (1.0 - x / width.max(1.0)).clamp(0.2, 0.8) // neither side can be dragged out of reach
}

fn frame_around(window: RECT, content: RECT) -> (i32, i32, i32, i32) {
    (content.left - window.left, content.top - window.top, window.right - content.right, window.bottom - content.bottom)
}

/// How far a DevTools window's content sits inside it, from the child window that holds the content.
fn measure_devtools_frame(hwnd: isize) -> Option<(i32, i32, i32, i32)> {
    let (mut window, mut content) = (RECT::default(), RECT::default());
    // SAFETY: Windows checks the handles, and these only write into local values.
    unsafe {
        let hwnd = HWND(hwnd as _);
        let inner = FindWindowExW(Some(hwnd), None, w!("Chrome_RenderWidgetHostHWND"), PCWSTR::null()).ok()?;
        GetWindowRect(hwnd, &mut window).ok()?;
        GetWindowRect(inner, &mut content).ok()?;
    }
    Some(frame_around(window, content))
}

#[derive(Default)]
struct DevtoolsWatch {
    process: u32,
    proxy: Option<EventLoopProxy<Msg>>,
    armed_until: Option<Instant>,
    hidden: Option<(isize, Instant)>,
    seen: bool, // the window asked for has appeared
}

fn arm_devtools_watch() {
    DEVTOOLS_WATCH.with(|w| {
        let mut watch = w.borrow_mut();
        watch.armed_until = Some(Instant::now() + Duration::from_secs(DEVTOOLS_ARMED_SECS));
        watch.hidden = None;
        watch.seen = false;
    });
}

thread_local! {
    static DEVTOOLS_WATCH: RefCell<DevtoolsWatch> = RefCell::default();
}

// WebView2 can only open DevTools as its own window. Windows reports it here the moment it shows, so it turns
// fully transparent before it is seen, and is docked as soon as the next scan runs. Not moved off screen: the
// engine stops painting a window it thinks nobody can see, and DevTools then stays blank after docking.
unsafe extern "system" fn engine_window_shown(_: HWINEVENTHOOK, event: u32, hwnd: HWND, object: i32, child: i32, _: u32, _: u32) {
    if object != OBJID_WINDOW.0 || child != CHILDID_SELF as i32 {
        return;
    }
    let mut text = [0u16; 64];
    // SAFETY: plain queries on a window handle that Windows checks, writing into local values.
    let (top_level, title) = unsafe {
        let top_level = GetWindowLongPtrW(hwnd, GWL_STYLE) & WS_CHILD.0 as isize == 0 && IsWindowVisible(hwnd).as_bool();
        let length = GetWindowTextW(hwnd, &mut text);
        (top_level, String::from_utf16_lossy(&text[..length.max(0) as usize]))
    };
    if !top_level {
        return;
    }
    DEVTOOLS_WATCH.with(|w| {
        let mut watch = w.borrow_mut();
        let armed = event == EVENT_OBJECT_SHOW && title.is_empty() && watch.armed_until.is_some_and(|until| Instant::now() < until);
        if !armed && !title.starts_with("DevTools") {
            return;
        }
        set_see_through(hwnd, true);
        watch.hidden = Some((hwnd.0 as isize, Instant::now()));
        watch.seen = true;
        if let Some(proxy) = &watch.proxy {
            drop(proxy.send_event(Msg::DevtoolsScan));
        }
    });
}

/// Undoes the docking first, unseen. Closing it while still a child of Riffle's window could make the engine
/// drop the next DevTools window it opens.
fn close_docked_devtools(hwnd: HWND) {
    set_see_through(hwnd, true);
    // SAFETY: Windows checks the handle, and only asks the window to close. A window gone by now just makes these fail.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) & !(WS_CHILD.0 as isize);
        SetWindowLongPtrW(hwnd, GWL_STYLE, style | WS_POPUP.0 as isize);
        let _ = SetParent(hwnd, None);
        let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
    }
}

fn set_see_through(hwnd: HWND, on: bool) {
    // SAFETY: Windows checks the handle, and a window gone by now just makes these fail.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        if on {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_LAYERED.0 as isize);
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 0, LWA_ALPHA);
        } else if style & WS_EX_LAYERED.0 as isize != 0 {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style & !(WS_EX_LAYERED.0 as isize));
        }
    }
}

fn watch_devtools_windows(view: &WebView, proxy: EventLoopProxy<Msg>) {
    let mut process = 0;
    // SAFETY: a COM call on the UI thread writing into a local value.
    let _ = unsafe { view.webview().BrowserProcessId(&mut process) };
    DEVTOOLS_WATCH.with(|w| {
        let mut watch = w.borrow_mut();
        if process == 0 || watch.process == process {
            return;
        }
        watch.process = process;
        watch.proxy = Some(proxy);
        // SAFETY: the callback is a plain function that lives as long as the program, and these hooks do too.
        unsafe {
            for event in [EVENT_OBJECT_SHOW, EVENT_OBJECT_NAMECHANGE] {
                SetWinEventHook(event, event, None, Some(engine_window_shown), process, 0, WINEVENT_OUTOFCONTEXT);
            }
        }
    });
}

/// If something hidden never got docked, such as an address that does not match its title, it returns where it can be used.
fn restore_lost_devtools() {
    DEVTOOLS_WATCH.with(|w| {
        let mut watch = w.borrow_mut();
        let Some((hwnd, at)) = watch.hidden else { return };
        if at.elapsed().as_secs() >= DEVTOOLS_HIDE_SECS {
            watch.hidden = None;
            set_see_through(HWND(hwnd as _), false);
        }
    });
}

/// A visible top-level DevTools window of this engine process and this tab's address, not yet docked.
fn floating_devtools(process: u32, address: &str) -> Option<HWND> {
    unsafe extern "system" fn check(hwnd: HWND, data: LPARAM) -> BOOL {
        // SAFETY: data is the pair below, alive for the whole EnumWindows call.
        let (process, address, found) = unsafe { &mut *(data.0 as *mut (u32, &str, Option<HWND>)) };
        let (mut owner, mut text) = (0u32, [0u16; 512]);
        // SAFETY: plain queries writing into local values.
        let title = unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut owner));
            if owner != *process || !IsWindowVisible(hwnd).as_bool() {
                return BOOL(1);
            }
            let length = GetWindowTextW(hwnd, &mut text);
            String::from_utf16_lossy(&text[..length.max(0) as usize])
        };
        if devtools_title_matches(&title, address) {
            *found = Some(hwnd);
            return BOOL(0);
        }
        BOOL(1)
    }
    let mut search = (process, address, None);
    // SAFETY: the callback only uses this pair, and EnumWindows returns before it goes away. It reports failure when stopped early.
    let _ = unsafe { EnumWindows(Some(check), LPARAM(&mut search as *mut _ as isize)) };
    search.2
}

fn can_go(view: &WebView) -> (bool, bool) {
    let webview = view.webview();
    let (mut back, mut forward) = (BOOL(0), BOOL(0));
    // SAFETY: COM calls on the UI thread writing into the two local values.
    unsafe {
        let _ = webview.CanGoBack(&mut back);
        let _ = webview.CanGoForward(&mut forward);
    }
    (back.as_bool(), forward.as_bool())
}

fn is_web(url: &str) -> bool {
    Url::parse(url).is_ok_and(|u| matches!(u.scheme(), "http" | "https"))
}

// Keeps the toolbar and panel on their built-in page, since they can run browser commands.
fn is_built_in_page(url: String) -> bool {
    url.starts_with("about:") || url.starts_with("data:")
}

/// What the user typed as an address, or None if it would only have become a search.
fn web_address(input: &str) -> Option<String> {
    address_to_url(input, SEARCH_URL).filter(|u| !u.starts_with(SEARCH_URL))
}

/// Adds a bookmark, or changes the one at `old`. An address already bookmarked is replaced, never doubled.
/// Changes the name and link, keeping the icon while the site stays the same. A blank name keeps the old one.
fn edit_bookmark(bookmarks: &mut Vec<Value>, url: &str, title: &str, new_url: Option<&str>) {
    let Some(at) = bookmarks.iter().position(|b| b["url"] == url) else { return };
    if !title.trim().is_empty() {
        bookmarks[at]["title"] = json!(title.trim());
    }
    let Some(new_url) = new_url.filter(|n| *n != url) else { return };
    if !same_site(url, new_url) {
        bookmarks[at]["icon"] = Value::Null;
    }
    bookmarks[at]["url"] = json!(new_url);
    let mut index = 0;
    bookmarks.retain(|b| {
        index += 1;
        index - 1 == at || b["url"] != new_url // an older bookmark of that address gives way
    });
}

fn put_bookmark(bookmarks: &mut Vec<Value>, old: Option<&str>, title: &str, url: &str) {
    let title = if title.trim().is_empty() { url } else { title.trim() };
    let entry = json!({"url": url, "title": title});
    let at = old.and_then(|old| bookmarks.iter().position(|b| b["url"] == old));
    if let Some(at) = at {
        bookmarks[at] = entry;
        let keep = &bookmarks[at] as *const Value;
        bookmarks.retain(|b| std::ptr::eq(b, keep) || b["url"] != url);
    } else {
        bookmarks.retain(|b| b["url"] != url);
        bookmarks.push(entry);
    }
}

fn move_bookmark(bookmarks: &mut [Value], url: &str, up: bool) {
    let Some(at) = bookmarks.iter().position(|b| b["url"] == url) else { return };
    let to = if up { at.checked_sub(1) } else { Some(at + 1).filter(|t| *t < bookmarks.len()) };
    if let Some(to) = to {
        bookmarks.swap(at, to);
    }
}

fn site_list(raw: &Value) -> Vec<String> {
    let valid = |s: &&str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    raw.as_array().into_iter().flatten().filter_map(Value::as_str).filter(valid).map(str::to_string).collect()
}

/// A download cut off by closing Riffle can never finish, so it must not read as still running.
fn load_downloads(path: &Path) -> Vec<Value> {
    let mut list = load_json(path).as_array().cloned().unwrap_or_default();
    for entry in list.iter_mut().filter(|d| d["state"] == "downloading") {
        entry["state"] = json!("interrupted");
    }
    list
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn system32(tool: &str) -> PathBuf {
    PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into())).join("System32").join(tool)
}

fn local_dir(rest: &str) -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join(rest))
}

const INSTALL_POINTER: &str = "install-location.txt"; // in the data folder, for a folder chosen during setup

fn install_dir() -> Option<PathBuf> {
    let chosen = local_dir("Riffle").and_then(|d| std::fs::read_to_string(d.join(INSTALL_POINTER)).ok()).map(|text| PathBuf::from(text.trim())).filter(|p| p.is_absolute());
    chosen.or_else(|| local_dir("Programs\\Riffle"))
}

/// Riffle always gets a folder of its own, because uninstalling deletes the whole folder.
fn riffle_folder(chosen: &Path) -> PathBuf {
    if chosen.file_name().is_some_and(|n| n.eq_ignore_ascii_case("Riffle")) { chosen.to_path_buf() } else { chosen.join("Riffle") }
}

/// Why this folder can't take Riffle. Refuses one holding other files, which uninstalling would delete.
fn folder_problem(dir: &Path) -> Option<&'static str> {
    if !dir.is_absolute() {
        return Some("Choose a full folder path.");
    }
    let mut files = std::fs::read_dir(dir).ok()?.flatten();
    (!dir.join("riffle-browser.exe").exists() && files.next().is_some()).then_some("That Riffle folder already has other files in it. Choose a different location.")
}

fn start_menu_link() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("Microsoft\\Windows\\Start Menu\\Programs\\Riffle.lnk"))
}

fn is_installed() -> bool {
    let here = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.to_string_lossy().to_lowercase()));
    here.is_some() && here == install_dir().map(|d| d.to_string_lossy().to_lowercase())
}

/// A PowerShell string that holds any path safely.
fn ps_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for part in bytes.chunks(3) {
        let n = (part[0] as u32) << 16 | (*part.get(1).unwrap_or(&0) as u32) << 8 | *part.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            out.push(if i <= part.len() { TABLE[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

// Encoded, so quotes and non-English letters in a Windows user name reach PowerShell intact.
fn powershell(script: &str) -> std::io::Result<std::process::Child> {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let encoded = base64(&script.encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<u8>>());
    std::process::Command::new(system32("WindowsPowerShell\\v1.0\\powershell.exe"))
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", &encoded])
        .current_dir(std::env::temp_dir()) // so it never holds the folder it deletes
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
}

const UNINSTALL_KEY: &str = "HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Riffle";
const PROG_KEY: &str = "HKCU:\\Software\\Classes\\RiffleHTML";
const CLIENT_KEY: &str = "HKCU:\\Software\\Clients\\StartMenuInternet\\Riffle";

fn desktop_link() -> Option<PathBuf> {
    // The real Desktop folder, which OneDrive and users can move away from the profile.
    known_folder(&FOLDERID_Desktop).map(|d| PathBuf::from(d).join("Riffle.lnk"))
}

fn install_script(exe: &str, dir: &str, link: &str, desktop: Option<&str>, pin_start: bool) -> String {
    let (q_exe, q_dir, q_link) = (ps_quote(exe), ps_quote(dir), ps_quote(link));
    let uninstaller = Path::new(dir).join("uninstall.exe").to_string_lossy().to_string();
    let (uninstall, quiet) = (ps_quote(&format!("\"{uninstaller}\"")), ps_quote(&format!("\"{uninstaller}\" --silent")));
    let open = ps_quote(&format!("\"{exe}\" \"%1\""));
    let launch = ps_quote(&format!("\"{exe}\""));
    let version = env!("CARGO_PKG_VERSION");
    let desktop_part = desktop.map_or(String::new(), |d| format!("$s=(New-Object -ComObject WScript.Shell).CreateShortcut({}); $s.TargetPath={q_exe}; $s.Description='Riffle - lightweight web browser with ad blocking'; $s.Save()\n", ps_quote(d)));
    // Best effort: Windows may refuse, and Riffle is still in the Start menu's list of apps.
    let start_part = if pin_start { format!("try {{ (New-Object -ComObject Shell.Application).Namespace((Split-Path {q_link})).ParseName((Split-Path {q_link} -Leaf)).InvokeVerb('startpin') }} catch {{}}\n") } else { String::new() };
    format!(
        "$ErrorActionPreference='Stop'
New-Item -ItemType Directory -Force (Split-Path {q_link}) | Out-Null
$s=(New-Object -ComObject WScript.Shell).CreateShortcut({q_link}); $s.TargetPath={q_exe}; $s.Description='Riffle - lightweight web browser with ad blocking'; $s.Save()
{desktop_part}New-Item '{UNINSTALL_KEY}' -Force | Out-Null
$v=@{{DisplayName='Riffle';DisplayIcon={q_exe};DisplayVersion='{version}';Publisher='Riffle';InstallLocation={q_dir};UninstallString={uninstall};QuietUninstallString={quiet}}}
foreach($k in $v.Keys){{New-ItemProperty '{UNINSTALL_KEY}' -Name $k -Value $v[$k] -PropertyType String -Force | Out-Null}}
foreach($k in 'NoModify','NoRepair'){{New-ItemProperty '{UNINSTALL_KEY}' -Name $k -Value 1 -PropertyType DWord -Force | Out-Null}}
New-ItemProperty '{UNINSTALL_KEY}' -Name EstimatedSize -Value ([int]((Get-Item -LiteralPath {q_exe}).Length/1024)) -PropertyType DWord -Force | Out-Null
function Put($path,$value){{ New-Item $path -Force | Out-Null; if($value){{ Set-Item $path -Value $value }} }}
Put '{PROG_KEY}' 'Riffle Web Document'
Put '{PROG_KEY}\\DefaultIcon' {q_exe}
Put '{PROG_KEY}\\shell\\open\\command' {open}
Put '{CLIENT_KEY}' 'Riffle'
Put '{CLIENT_KEY}\\DefaultIcon' {q_exe}
Put '{CLIENT_KEY}\\shell\\open\\command' {launch}
Put '{CLIENT_KEY}\\Capabilities' $null
foreach($k in @{{ApplicationName='Riffle';ApplicationDescription='Lightweight web browser with ad blocking';ApplicationIcon={q_exe}}}.GetEnumerator()){{New-ItemProperty '{CLIENT_KEY}\\Capabilities' -Name $k.Key -Value $k.Value -PropertyType String -Force | Out-Null}}
Put '{CLIENT_KEY}\\Capabilities\\URLAssociations' $null
foreach($k in 'http','https'){{New-ItemProperty '{CLIENT_KEY}\\Capabilities\\URLAssociations' -Name $k -Value 'RiffleHTML' -PropertyType String -Force | Out-Null}}
if(-not(Test-Path 'HKCU:\\Software\\RegisteredApplications')){{New-Item 'HKCU:\\Software\\RegisteredApplications' | Out-Null}}
New-ItemProperty 'HKCU:\\Software\\RegisteredApplications' -Name Riffle -Value 'Software\\Clients\\StartMenuInternet\\Riffle\\Capabilities' -PropertyType String -Force | Out-Null
{start_part}"
    )
}

/// Closes Riffle and removes everything but its own folder. Never stops `keep`, the process running this uninstall.
fn uninstall_script(exe: &str, link: &str, desktop: &str, data: Option<&str>, keep: u32) -> String {
    let (q_exe, q_link, q_desktop) = (ps_quote(exe), ps_quote(link), ps_quote(desktop));
    let q_old = ps_quote(&Path::new(exe).with_extension("old").to_string_lossy()); // a window still open since an update
    let wipe = data.map_or(String::new(), |d| format!("\nforeach($i in 1..15){{ Remove-Item -LiteralPath {0} -Recurse -Force -ErrorAction SilentlyContinue; if(-not(Test-Path -LiteralPath {0})){{break}}; Start-Sleep 1 }}", ps_quote(d)));
    format!(
        "Get-Process riffle-browser -ErrorAction SilentlyContinue | Where-Object {{ $_.Path -eq {q_exe} -and $_.Id -ne {keep} }} | Stop-Process -Force
Get-Process riffle-browser -ErrorAction SilentlyContinue | Where-Object {{ $_.Path -eq {q_old} }} | Stop-Process -Force
Start-Sleep 2
try {{ (New-Object -ComObject Shell.Application).Namespace((Split-Path {q_link})).ParseName((Split-Path {q_link} -Leaf)).InvokeVerb('startunpin') }} catch {{}}
Remove-Item '{UNINSTALL_KEY}','{PROG_KEY}','{CLIENT_KEY}' -Recurse -Force -ErrorAction SilentlyContinue
Remove-ItemProperty 'HKCU:\\Software\\RegisteredApplications' -Name Riffle -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath {q_link},{q_desktop} -Force -ErrorAction SilentlyContinue{wipe}"
    )
}

/// Runs after this process exits, because a running program cannot delete itself.
fn remove_folder_script(dir: &str, waiting_for: u32) -> String {
    let q_dir = ps_quote(dir);
    format!(
        "Wait-Process -Id {waiting_for} -ErrorAction SilentlyContinue
foreach($i in 1..15){{ Remove-Item -LiteralPath {q_dir} -Recurse -Force -ErrorAction SilentlyContinue; if(-not(Test-Path -LiteralPath {q_dir})){{break}}; Start-Sleep 1 }}"
    )
}

fn taskbar_pinned() -> bool {
    std::env::var_os("APPDATA").is_some_and(|d| PathBuf::from(d).join("Microsoft\\Internet Explorer\\Quick Launch\\User Pinned\\TaskBar\\Riffle.lnk").exists())
}

/// "v1.2.3" as numbers. Anything else, such as a test release's "1.2.3-rc1", is never installed.
fn parse_version(tag: &str) -> Option<(u64, u64, u64)> {
    let mut parts = tag.strip_prefix('v').unwrap_or(tag).split('.').map(|p| p.parse::<u64>().ok());
    let version = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(version)
}

fn is_newer(tag: &str, current: &str) -> bool {
    matches!((parse_version(tag), parse_version(current)), (Some(new), Some(old)) if new > old)
}

/// The hash SHA256SUMS.txt lists for a file.
fn listed_hash(sums: &str, file: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, name) = line.split_once(char::is_whitespace)?;
        (name.trim().trim_start_matches('*') == file && is_sha256(hash)).then(|| hash.to_lowercase())
    })
}

fn is_sha256(text: &str) -> bool {
    text.len() == 64 && text.chars().all(|c| c.is_ascii_hexdigit())
}

fn file_hash(path: &Path) -> Option<String> {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new(system32("certutil.exe")).arg("-hashfile").arg(path).arg("SHA256").creation_flags(CREATE_NO_WINDOW).output().ok()?;
    String::from_utf8_lossy(&out.stdout).lines().map(|l| l.replace(' ', "")).find(|l| is_sha256(l)).map(|l| l.to_lowercase())
}

fn fetch_text(url: &str) -> Option<String> {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new(system32("curl.exe")).args(["-fsSL", "--proto", "=https", "--max-time", "20", url]).creation_flags(CREATE_NO_WINDOW).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).to_string())
}

/// Puts dir's riffle-browser.new in place. Windows allows renaming a running program, so open windows carry on
/// with the old version and the next start gets the new one. Any failure leaves the old version working.
fn swap_in_update(dir: &Path) -> std::io::Result<()> {
    let (exe, new, old) = (dir.join("riffle-browser.exe"), dir.join("riffle-browser.new"), dir.join("riffle-browser.old"));
    let (uninstaller, old_uninstaller) = (dir.join("uninstall.exe"), dir.join("uninstall.old"));
    let _ = std::fs::remove_file(&old);
    let _ = std::fs::remove_file(&old_uninstaller);
    std::fs::rename(&exe, &old)?;
    if let Err(e) = std::fs::rename(&new, &exe) {
        std::fs::rename(&old, &exe)?;
        return Err(e);
    }
    // uninstall.exe is the same file under a second name, so it must follow.
    let _ = std::fs::rename(&uninstaller, &old_uninstaller);
    std::fs::hard_link(&exe, &uninstaller).or_else(|_| std::fs::copy(&exe, &uninstaller).map(drop))
}

/// Checks GitHub once and installs anything newer. None if the check failed, Some(None) if already current,
/// and Some(Some(version)) if that version now starts next time.
fn update_once(dir: &Path) -> Option<Option<String>> {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    static BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if BUSY.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return None; // the daily check and the Settings button must not download over each other
    }
    let found = (|| {
    let release: Value = serde_json::from_str(&fetch_text(LATEST_RELEASE)?).ok()?;
    let tag = release["tag_name"].as_str()?;
    // Numbers only from here on, so the tag is safe inside an address.
    if !is_newer(tag, env!("CARGO_PKG_VERSION")) {
        return Some(None);
    }
    let expected = listed_hash(&fetch_text(&format!("{RELEASES}/{tag}/SHA256SUMS.txt"))?, "RiffleSetup.exe")?;
    let new = dir.join("riffle-browser.new");
    let fetched = std::process::Command::new(system32("curl.exe")).args(["-fsSL", "--proto", "=https", "--max-time", "600", "-o"]).arg(&new).arg(format!("{RELEASES}/{tag}/RiffleSetup.exe")).creation_flags(CREATE_NO_WINDOW).status();
    if !fetched.is_ok_and(|s| s.success()) || file_hash(&new).as_deref() != Some(expected.as_str()) || swap_in_update(dir).is_err() {
        let _ = std::fs::remove_file(&new);
        return None;
    }
    let version = tag.trim_start_matches('v').to_string();
    let _ = std::process::Command::new(system32("reg.exe")).args(["add", &UNINSTALL_KEY.replace("HKCU:", "HKCU"), "/v", "DisplayVersion", "/d", &version, "/f"]).creation_flags(CREATE_NO_WINDOW).status();
    Some(Some(version))
    })();
    BUSY.store(false, std::sync::atomic::Ordering::SeqCst);
    found
}

/// Only the installed copy updates itself: a copy someone built or carried elsewhere is theirs to manage.
fn keep_updated(data_dir: PathBuf, proxy: EventLoopProxy<Msg>) {
    let Some(dir) = install_dir().filter(|_| is_installed()) else { return };
    std::thread::spawn(move || {
        for leftover in ["riffle-browser.old", "uninstall.old", "riffle-browser.new"] {
            let _ = std::fs::remove_file(dir.join(leftover)); // fails harmlessly while an old window is open
        }
        let stamp = data_dir.join("update-check");
        loop {
            let settings = sanitize_settings(&load_json(&data_dir.join("settings.json"))); // another window may have changed it
            let age = std::fs::metadata(&stamp).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok());
            if settings["auto_update"] == true && age.is_none_or(|a| a.as_secs() >= UPDATE_GAP_SECS) {
                let _ = std::fs::write(&stamp, "");
                if let Some(Some(version)) = update_once(&dir) {
                    drop(proxy.send_event(Msg::UpdateReady(version)));
                    return;
                }
            }
            std::thread::sleep(Duration::from_secs(UPDATE_WAKE_SECS));
        }
    });
}

/// Copies Riffle to the user's own programs folder and lists it in Windows. Needs no administrator rights.
fn install_app(chosen: &Path, desktop: bool, pin_start: bool, step: impl Fn(u32, &str)) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = riffle_folder(chosen);
    if let Some(problem) = folder_problem(&dir) {
        return Err(problem.into());
    }
    let link = start_menu_link().ok_or("APPDATA is not set")?;
    let target = dir.join("riffle-browser.exe");
    step(15, "Copying files");
    let here = std::env::current_exe()?.parent().map(|p| p.to_string_lossy().to_lowercase());
    if here != Some(dir.to_string_lossy().to_lowercase()) {
        std::fs::create_dir_all(&dir).map_err(|e| if e.kind() == std::io::ErrorKind::PermissionDenied { "Riffle can't write to that folder without administrator rights. Go back and choose a different one.".to_string() } else { e.to_string() })?;
        std::fs::copy(std::env::current_exe()?, &target).map_err(|e| format!("{e}. Close any installed Riffle window and try again"))?;
    }
    step(45, "Adding the uninstaller");
    // The same file under a second name, which takes no extra disk space. The name makes it uninstall.
    let uninstaller = dir.join("uninstall.exe");
    let _ = std::fs::remove_file(&uninstaller);
    std::fs::hard_link(&target, &uninstaller).or_else(|_| std::fs::copy(&target, &uninstaller).map(drop))?;
    step(65, "Adding shortcuts and listing Riffle in Windows");
    let desktop = if desktop { Some(desktop_link().ok_or("The desktop folder was not found")?) } else { None };
    let desktop_text = desktop.as_ref().map(|d| d.to_string_lossy().to_string());
    let script = install_script(&target.to_string_lossy(), &dir.to_string_lossy(), &link.to_string_lossy(), desktop_text.as_deref(), pin_start);
    let done = powershell(&script)?.wait()?;
    if !done.success() {
        return Err("Windows would not add the shortcuts or the Apps listing".into());
    }
    let data = local_dir("Riffle").ok_or("LOCALAPPDATA is not set")?;
    std::fs::create_dir_all(&data)?;
    std::fs::write(data.join(INSTALL_POINTER), dir.to_string_lossy().as_bytes())?;
    step(100, "Finishing");
    Ok(target)
}

enum SetupMsg {
    Script(String),
    Pick(String),
    Quit,
}

// The setup window, for installing or removing. In-process rather than a bundled installer tool, since Riffle's one exe already holds everything it copies.
fn run_setup(removing: bool) -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoopBuilder::<SetupMsg>::with_user_event().build();
    let window = WindowBuilder::new()
        .with_title(if removing { "Uninstall Riffle" } else { "Riffle Setup" })
        .with_visible(false) // shown once its page is ready, so there's no blank flash
        .with_inner_size(LogicalSize::new(520.0, 320.0))
        .with_resizable(false)
        .with_maximizable(false)
        .with_window_icon(Some(Icon::from_rgba(logo_rgba(), LOGO_SIZE, LOGO_SIZE)?))
        .build(&event_loop)?;
    // Its own profile: browser windows hold theirs with different engine flags, and it stores nothing worth keeping.
    let mut context = WebContext::new(Some(std::env::temp_dir().join("RiffleSetup")));
    let proxy = event_loop.create_proxy();
    let current = install_dir().ok_or("LOCALAPPDATA is not set")?;
    let existing = current.join("riffle-browser.exe").exists();
    let place = current.to_string_lossy().to_string();
    let view = WebViewBuilder::new_with_web_context(&mut context)
        .with_html(INSTALL_HTML)
        .with_drag_drop_handler(|_| true)
        .with_navigation_handler(is_built_in_page)
        .with_ipc_handler(move |req| {
            let command: Value = serde_json::from_str(req.body()).unwrap_or_default();
            let name = command["cmd"].as_str().unwrap_or("").to_string();
            match name.as_str() {
                "ready" => drop(proxy.send_event(SetupMsg::Script(format!("setup({},{removing},{existing},{})", json!(place), json!(env!("CARGO_PKG_VERSION")))))),
                "quit" => drop(proxy.send_event(SetupMsg::Quit)),
                "pick" => drop(proxy.send_event(SetupMsg::Pick(command["place"].as_str().unwrap_or("").to_string()))),
                "finish" => {
                    if command["launch"] == true {
                        if let Some(dir) = install_dir() {
                            let _ = std::process::Command::new(dir.join("riffle-browser.exe")).spawn();
                        }
                    }
                    drop(proxy.send_event(SetupMsg::Quit));
                }
                "uninstall" => {
                    let proxy = proxy.clone();
                    std::thread::spawn(move || {
                        let progress = proxy.clone();
                        let step = move |percent: u32, text: &str| drop(progress.send_event(SetupMsg::Script(format!("progress({percent},{})", json!(text)))));
                        let script = match uninstall_app(command["wipe"] == true, step) {
                            Ok(()) => "removed()".to_string(),
                            Err(e) => format!("failed({})", json!(e.to_string())),
                        };
                        drop(proxy.send_event(SetupMsg::Script(script)));
                    });
                }
                "install" => {
                    let proxy = proxy.clone();
                    std::thread::spawn(move || {
                        let progress = proxy.clone();
                        let step = move |percent: u32, text: &str| drop(progress.send_event(SetupMsg::Script(format!("progress({percent},{})", json!(text)))));
                        // shortcut: Windows has no public taskbar pin for unpackaged apps, so the wizard explains the manual step.
                        let script = match install_app(Path::new(command["place"].as_str().unwrap_or("")), command["desktop"] == true, command["start"] == true, step) {
                            Ok(_) => format!("finished({})", taskbar_pinned()),
                            Err(e) => format!("failed({})", json!(e.to_string())),
                        };
                        drop(proxy.send_event(SetupMsg::Script(script)));
                    });
                }
                _ => {}
            }
        })
        .build(&window)?;
    event_loop.run(move |event, _, control_flow| {
        let _alive = (&context, &window);
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } | Event::UserEvent(SetupMsg::Quit) => *control_flow = ControlFlow::Exit,
            Event::UserEvent(SetupMsg::Pick(place)) => {
                let start = Path::new(&place).parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
                if let Some(folder) = pick_path(window.hwnd() as isize, true, &start, "").map(|p| riffle_folder(&p)) {
                    let _ = view.evaluate_script(&format!("chosen({},{})", json!(folder.to_string_lossy()), json!(folder_problem(&folder).unwrap_or(""))));
                }
            }
            Event::UserEvent(SetupMsg::Script(script)) => {
                let _ = view.evaluate_script(&script);
                window.set_visible(true);
            }
            _ => {}
        }
    });
}

fn uninstall_app(wipe: bool, step: impl Fn(u32, &str)) -> Result<(), Box<dyn std::error::Error>> {
    let dir = install_dir().ok_or("LOCALAPPDATA is not set")?;
    let link = start_menu_link().ok_or("APPDATA is not set")?;
    let data = local_dir("Riffle").ok_or("LOCALAPPDATA is not set")?;
    let (exe, desktop, data_text) = (dir.join("riffle-browser.exe"), desktop_link().unwrap_or_default(), data.to_string_lossy());
    step(20, "Closing Riffle");
    let script = uninstall_script(&exe.to_string_lossy(), &link.to_string_lossy(), &desktop.to_string_lossy(), wipe.then_some(&*data_text), std::process::id());
    step(50, if wipe { "Removing shortcuts, Windows entries and your data" } else { "Removing shortcuts and Windows entries" });
    powershell(&script)?.wait()?;
    let _ = std::fs::remove_file(data.join(INSTALL_POINTER)); // the next install starts from the default again
    step(85, "Removing program files");
    powershell(&remove_folder_script(&dir.to_string_lossy(), std::process::id()))?;
    step(100, "Finishing");
    Ok(())
}

/// Up to a few pages from bookmarks and history, those whose address starts with the text first. The first of
/// those carries `complete`, the address the bar can finish typing.
fn local_suggestions(query: &str, bookmarks: &[Value], history: &[Value]) -> Vec<Value> {
    let needle = query.trim().to_lowercase();
    let short = |url: &str| url.trim_start_matches("https://").trim_start_matches("http://").trim_start_matches("www.").trim_end_matches('/').to_string();
    let mut seen = std::collections::HashSet::new();
    let mut found: Vec<(bool, Value)> = Vec::new();
    for entry in bookmarks.iter().chain(history.iter().rev()) {
        let (Some(url), title) = (entry["url"].as_str().filter(|u| is_web(u)), entry["title"].as_str().unwrap_or("")) else { continue };
        let bare = short(url);
        let starts = bare.to_lowercase().starts_with(&needle);
        if needle.is_empty() || !(starts || bare.to_lowercase().contains(&needle) || title.to_lowercase().contains(&needle)) || !seen.insert(url.to_string()) {
            continue;
        }
        let mut item = json!({"text": if title.is_empty() { &bare } else { title }, "url": url});
        if starts {
            item["complete"] = json!(bare);
        }
        found.push((starts, item));
    }
    found.sort_by_key(|(starts, _)| !starts); // stable, so the most recent stays first within each group
    found.into_iter().take(SUGGEST_MAX_LOCAL).map(|(_, item)| item).collect()
}

/// Where the chosen engine offers suggestions. None for an address, which must not be sent anywhere while being
/// typed, and for engines with no known service, whose text must not go to a different company instead.
fn suggest_url(search: &str, query: &str) -> Option<String> {
    if web_address(query).is_some() {
        return None;
    }
    let (base, key) = match Url::parse(search).ok()?.host_str()? {
        "duckduckgo.com" => ("https://duckduckgo.com/ac/?type=list", "q"),
        "www.google.com" => ("https://suggestqueries.google.com/complete/search?client=firefox", "q"),
        "www.bing.com" => ("https://api.bing.com/osjson.aspx", "query"),
        "search.brave.com" => ("https://search.brave.com/api/suggest", "q"),
        _ => return None,
    };
    let mut url = Url::parse(base).ok()?;
    url.query_pairs_mut().append_pair(key, query);
    Some(url.into())
}

/// The OpenSearch answer all these services give: ["typed text", ["suggestion", ...]].
fn parse_suggestions(body: &str) -> Vec<String> {
    let value: Value = serde_json::from_str(body).unwrap_or_default();
    value[1].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect()
}

fn fetch_suggestions(url: &str) -> Vec<String> {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let answer = std::process::Command::new(system32("curl.exe")).args(["-fsSL", "--max-time", "3", url]).creation_flags(CREATE_NO_WINDOW).output();
    answer.map_or(Vec::new(), |o| parse_suggestions(&String::from_utf8_lossy(&o.stdout)))
}

fn is_default_browser() -> bool {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // Newer Windows 11 builds keep the choice under UserChoiceLatest.
    ["UserChoice", "UserChoiceLatest"].iter().any(|key| {
        std::process::Command::new(system32("reg.exe"))
            .args(["query", &format!("HKCU\\Software\\Microsoft\\Windows\\Shell\\Associations\\UrlAssociations\\https\\{key}"), "/s"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains("RiffleHTML"))
    })
}

fn known_folder(id: &windows::core::GUID) -> Option<String> {
    // SAFETY: takes a valid folder id and returns a string that take_pwstr frees.
    unsafe { SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None) }.ok().map(take_pwstr)
}

fn default_download_dir() -> String {
    known_folder(&FOLDERID_Downloads).unwrap_or_default()
}

/// The native save or folder dialog, opened in `dir`. None if cancelled.
fn pick_path(owner: isize, folder: bool, dir: &str, name: &str) -> Option<PathBuf> {
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let (dir, name) = (wide(dir), wide(name));
    // SAFETY: COM calls on the UI thread, with strings that outlive each call.
    unsafe {
        let dialog: IFileDialog = CoCreateInstance(if folder { &FileOpenDialog } else { &FileSaveDialog }, None, CLSCTX_INPROC_SERVER).ok()?;
        if folder {
            dialog.SetOptions(dialog.GetOptions().ok()? | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM).ok()?;
        } else {
            dialog.SetFileName(PCWSTR(name.as_ptr())).ok()?;
        }
        if let Ok(item) = SHCreateItemFromParsingName::<_, _, IShellItem>(PCWSTR(dir.as_ptr()), None) {
            let _ = dialog.SetFolder(&item);
        }
        dialog.Show(Some(HWND(owner as _))).ok()?;
        Some(PathBuf::from(take_pwstr(dialog.GetResult().ok()?.GetDisplayName(SIGDN_FILESYSPATH).ok()?)))
    }
}

/// `name` in `dir`, numbered like "a (1).zip" if taken, because saving there would replace the existing file.
fn unique_path(dir: &Path, name: &std::ffi::OsStr) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let (stem, ext) = (Path::new(name).file_stem().unwrap_or(name).to_string_lossy().to_string(), Path::new(name).extension().map(|e| e.to_string_lossy().to_string()));
    (1u32..).map(|n| dir.join(ext.as_ref().map_or(format!("{stem} ({n})"), |e| format!("{stem} ({n}).{e}")))).find(|p| !p.exists()).unwrap_or(first)
}

/// Applies the download settings to the path the engine suggests. False cancels the download.
fn choose_download_path(owner: isize, path: &mut PathBuf) -> bool {
    let (dir, ask) = DOWNLOAD_PREFS.lock().map(|p| p.clone()).unwrap_or_default();
    let Some(name) = path.file_name().map(|n| n.to_os_string()) else { return true };
    if ask {
        return match pick_path(owner, false, &dir, &name.to_string_lossy()) {
            Some(chosen) => {
                *path = chosen; // the dialog has already asked before replacing a file
                true
            }
            None => false,
        };
    }
    // A folder that cannot be made, such as an unplugged drive, falls back to the engine's own choice.
    if !dir.is_empty() && std::fs::create_dir_all(&dir).is_ok() {
        // The engine fails a download whose path mixes / and \, and a settings file can hold either.
        *path = unique_path(Path::new(&dir.replace('/', "\\")), &name);
    }
    true
}

/// Every setting comes back valid, whether it came from the panel or a hand-edited file.
// Each icon beside the address bar: its name, its default, and whether "only when needed" means anything for it.
const TOOLBAR_ICONS: &[(&str, &str, bool)] = &[("extensions", "show", false), ("downloads", "auto", true), ("shield", "auto", true), ("private", "show", false), ("music", "show", false), ("reader", "auto", true), ("star", "show", false)];

fn sanitize_toolbar(raw: &Value) -> Value {
    let mut out = serde_json::Map::new();
    for (name, default, has_auto) in TOOLBAR_ICONS {
        let choice = raw[*name].as_str().filter(|c| *c == "show" || *c == "hide" || (*c == "auto" && *has_auto)).unwrap_or(default);
        out.insert(name.to_string(), json!(choice));
    }
    Value::Object(out)
}

fn sanitize_settings(raw: &Value) -> Value {
    let theme = raw["theme"].as_str().filter(|t| ["system", "light", "dark"].contains(t)).unwrap_or("system");
    let accent = raw["accent"].as_str().filter(|a| a.len() == 7 && a.starts_with('#') && a.is_char_boundary(1) && a[1..].chars().all(|c| c.is_ascii_hexdigit())).unwrap_or(DEFAULT_ACCENT);
    let search = raw["search"].as_str().filter(|s| Url::parse(s).is_ok_and(|u| u.scheme() == "https")).unwrap_or(SEARCH_URL);
    // A home page that is not an address would turn into a search, so it is dropped instead.
    let home = raw["home"].as_str().and_then(web_address).unwrap_or_default();
    json!({
        "theme": theme,
        "accent": accent,
        "search": search,
        "home": home,
        "restore": raw["restore"] == true,
        "bookmarks_bar": raw["bookmarks_bar"] != false,
        "idle_sleep": raw["idle_sleep"] != false,
        "allow_sites": site_list(&raw["allow_sites"]),
        "allow_popups": site_list(&raw["allow_popups"]),
        "adblock": raw["adblock"] != false,
        "download_dir": raw["download_dir"].as_str().filter(|d| Path::new(d).is_absolute()).map_or_else(default_download_dir, str::to_string),
        "search_suggest": raw["search_suggest"] != false,
        "agent_mode": raw["agent_mode"] == true,
        "toolbar": sanitize_toolbar(&raw["toolbar"]),
        "auto_update": raw["auto_update"] != false,
        "download_ask": raw["download_ask"] == true,
        "download_show": raw["download_show"] != false,
        "max_live_tabs": raw["max_live_tabs"].as_u64().unwrap_or(DEFAULT_LIVE_TABS).clamp(1, 32),
    })
}

/// Oldest first. Skips lines a crash cut short, and keeps only the final title of each visit.
fn parse_history(text: &str) -> Vec<Value> {
    let mut entries: Vec<Value> = Vec::new();
    for entry in text.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()) {
        if entries.last().is_some_and(|last| last["url"] == entry["url"]) {
            entries.pop();
        }
        entries.push(entry);
    }
    let excess = entries.len().saturating_sub(HISTORY_LIMIT);
    entries.drain(..excess);
    entries
}

/// A crash can cut a line inside a letter, which must not make the whole file unreadable.
fn read_lossy(path: &Path) -> Option<String> {
    std::fs::read(path).ok().map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

/// The history file without one visit, matched on address and time so other visits to that page stay.
fn remove_history_entry(text: &str, url: &Value, time: &Value) -> String {
    parse_history(text).iter().filter(|e| e["url"] != *url || e["time"] != *time).map(|e| format!("{e}
")).collect()
}

fn compact_history(path: &Path) {
    if std::fs::metadata(path).is_ok_and(|m| m.len() > HISTORY_COMPACT_BYTES) {
        let Some(text) = read_lossy(path) else { return };
        let lines: String = parse_history(&text).iter().map(|e| format!("{e}\n")).collect();
        write_replacing(path, &lines).unwrap_or_else(|e| show_error(&format!("Could not save history: {e}")));
    }
}

fn load_json(path: &Path) -> Value {
    let Ok(text) = std::fs::read_to_string(path) else { return Value::Null };
    serde_json::from_str(&text).unwrap_or_else(|_| {
        let _ = std::fs::rename(path, path.with_extension("corrupt")); // keep it, the next save would overwrite it
        Value::Null
    })
}

fn save_json(path: &Path, value: &Value) {
    write_replacing(path, &value.to_string()).unwrap_or_else(|e| show_error(&format!("Could not save {}: {e}", path.display())));
}

// Through a temporary file, so a crash mid-write cannot destroy the existing file.
fn write_replacing(path: &Path, text: &str) -> std::io::Result<()> {
    let temp = path.with_extension(format!("{}.tmp", std::process::id())); // a normal and a private window can save at once
    std::fs::write(&temp, text)?;
    std::fs::rename(temp, path)
}

#[link(name = "user32")]
unsafe extern "system" {
    fn keybd_event(key: u8, scan: u8, flags: u32, extra: usize);
    fn GetKeyState(key: i32) -> i16;
    fn MessageBoxW(hwnd: *mut core::ffi::c_void, text: *const u16, caption: *const u16, kind: u32) -> i32;
}

#[link(name = "dwmapi")]
unsafe extern "system" {
    fn DwmSetWindowAttribute(hwnd: *mut core::ffi::c_void, attribute: u32, value: *const i32, size: u32) -> i32;
}

// Windows 11 rounds these on request. Windows 10 ignores the call and keeps square corners.
fn round_corners(hwnd: *mut core::ffi::c_void) {
    const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    const DWMWCP_ROUND: i32 = 2;
    // SAFETY: the value outlives the call and its size is passed with it.
    unsafe { DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &DWMWCP_ROUND, 4) };
}

// Opens Windows' own Cast panel (Win+K). WebView2 has no tab casting API to call.
fn cast_screen() {
    const VK_LWIN: u8 = 0x5B;
    const VK_K: u8 = 0x4B;
    const KEYUP: u32 = 2;
    // SAFETY: keybd_event takes plain values and holds on to nothing.
    unsafe {
        keybd_event(VK_LWIN, 0, 0, 0);
        keybd_event(VK_K, 0, 0, 0);
        keybd_event(VK_K, 0, KEYUP, 0);
        keybd_event(VK_LWIN, 0, KEYUP, 0);
    }
}

const LOGO_SIZE: u32 = 32;

// The logo from newtab.html drawn as pixels, because a window icon cannot be an SVG: a riffle is the quick,
// rippling stretch of a stream. Each pixel is sampled at 4x4 points to smooth the curves.
fn logo_rgba() -> Vec<u8> {
    let covers = |x: f64, y: f64| -> (bool, bool) {
        let disc = (x - 16.0).powi(2) + (y - 16.0).powi(2) <= 225.0;
        let wave = (7.0..=25.0).contains(&x)
            && [11.0, 16.0, 21.0].iter().any(|base| (y - (base - 2.4 * ((x - 7.0) * std::f64::consts::PI / 9.0).sin())).abs() <= 1.3);
        (disc, wave)
    };
    let mut rgba = Vec::with_capacity((LOGO_SIZE * LOGO_SIZE * 4) as usize);
    for py in 0..LOGO_SIZE {
        for px in 0..LOGO_SIZE {
            let (mut disc, mut wave) = (0u32, 0u32);
            for sy in 0..4 {
                for sx in 0..4 {
                    let (d, w) = covers(px as f64 + (sx as f64 + 0.5) / 4.0, py as f64 + (sy as f64 + 0.5) / 4.0);
                    disc += u32::from(d);
                    wave += u32::from(d && w);
                }
            }
            let blend = |teal: u32| ((teal * (disc - wave) + 255 * wave) / disc.max(1)) as u8;
            rgba.extend_from_slice(&[blend(0x0b), blend(0x6e), blend(0x8f), (disc * 255 / 16) as u8]);
        }
    }
    rgba
}

// This app has no console, so failures such as a missing WebView2 runtime need a visible message.
fn show_error(message: &str) {
    message_box(message, 0x10);
}

fn message_box(message: &str, kind: u32) -> i32 {
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let (text, caption) = (wide(message), wide("Riffle"));
    // SAFETY: both pointers are NUL-terminated buffers that outlive the call.
    unsafe { MessageBoxW(core::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), kind) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_bar_understands_urls_and_searches() {
        assert_eq!(address_to_url("example.com", SEARCH_URL).unwrap(), "https://example.com/");
        assert_eq!(address_to_url("https://a.org/x?y=1", SEARCH_URL).unwrap(), "https://a.org/x?y=1");
        assert_eq!(address_to_url("localhost:3000", SEARCH_URL).unwrap(), "http://localhost:3000");
        assert_eq!(address_to_url("192.168.1.1/admin", SEARCH_URL).unwrap(), "http://192.168.1.1/admin");
        assert_eq!(address_to_url("   ", SEARCH_URL), None);
        for search in ["best kettle", "kettle", "3.14", "a.b c"] {
            assert!(address_to_url(search, SEARCH_URL).unwrap().starts_with(SEARCH_URL), "{search}");
        }
    }

    #[test]
    fn unsafe_schemes_only_ever_become_searches() {
        for bad in ["javascript:alert(1)", "javascript:alert(1.2)", "file:///c:/secret.txt", "data:text/html,x", "ms-settings:"] {
            assert!(address_to_url(bad, SEARCH_URL).unwrap().starts_with(SEARCH_URL), "{bad}");
        }
    }

    #[test]
    fn oldest_background_tabs_sleep_first_and_active_never() {
        let live = [(1, 1), (2, 5), (3, 3), (4, 4), (5, 2), (6, 6)];
        assert_eq!(tabs_to_sleep(&live, 1, 4), [5, 3]);
        assert!(tabs_to_sleep(&live[..4], 2, 4).is_empty());
        assert_eq!(tabs_to_sleep(&[(1, 1), (2, 2)], 1, 1), [2]);
    }

    #[test]
    fn agent_port_is_opt_in_and_local_only() {
        assert!(!browser_args(false).contains("remote-debugging")); // what Riffle's own interface gets in agent mode
        let args = browser_args(true);
        assert!(args.contains("--remote-debugging-port=9222") && args.contains("--remote-debugging-address=127.0.0.1"));
    }

    #[test]
    fn built_in_host_list_covers_subdomains_only_of_those_hosts() {
        let engine = seed_engine();
        assert!(is_blocked(&engine, "https://stats.doubleclick.net/x", "https://site.test/", "script"));
        assert!(!is_blocked(&engine, "https://notdoubleclick.net/x", "https://site.test/", "script"));
    }

    #[test]
    fn only_real_shortcut_keys_map() {
        assert_eq!(key_shortcut(0x54, false), Some("new_tab"));
        assert_eq!(key_shortcut(0x54, true), Some("reopen_tab"));
        assert_eq!(key_shortcut(0x09, true), Some("prev_tab"));
        assert_eq!(key_shortcut(0x4E, false), Some("new_window"));
        assert_eq!(key_shortcut(0x47, false), None); // plain Ctrl+G
        assert_eq!(key_shortcut(0x41, false), None); // Ctrl+A stays the page's
        assert_eq!(key_shortcut(0xBB, true), Some("zoom_in"));
        assert_eq!(key_shortcut(0x6B, false), Some("zoom_in"));
        assert_eq!(key_shortcut(0xBD, false), Some("zoom_out"));
        assert_eq!(key_shortcut(0x60, false), Some("zoom_reset"));
        assert_eq!(key_shortcut(0x30, true), None);
    }

    #[test]
    fn a_cut_off_letter_leaves_the_rest_of_history_readable() {
        let path = std::env::temp_dir().join(format!("riffle-lossy-{}.jsonl", std::process::id()));
        std::fs::write(&path, b"{\"url\":\"a\",\"time\":1}\n{\"url\":\"b\",\"title\":\"\xC3").unwrap();
        let text = read_lossy(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(parse_history(&text).len(), 1);
    }

    #[test]
    fn searches_use_the_chosen_engine() {
        assert_eq!(address_to_url("a b", "https://s.test/?q=").unwrap(), "https://s.test/?q=a+b");
    }

    #[test]
    fn bad_settings_fall_back_and_good_ones_stay() {
        let bad = sanitize_settings(&json!({"theme": "x", "accent": "red;}", "search": "javascript:alert(1)//", "home": "file:///c:/x", "adblock": "no", "max_live_tabs": 999}));
        assert_eq!(bad, json!({"theme": "system", "accent": DEFAULT_ACCENT, "search": SEARCH_URL, "home": "", "restore": false, "bookmarks_bar": true, "idle_sleep": true, "allow_sites": [], "allow_popups": [], "adblock": true, "max_live_tabs": 32, "auto_update": true, "agent_mode": false, "toolbar": sanitize_toolbar(&Value::Null), "search_suggest": true, "download_dir": default_download_dir(), "download_ask": false, "download_show": true}));
        let good = json!({"theme": "dark", "accent": "#112233", "search": "https://s.test/?q=", "home": "https://a.org/", "restore": true, "bookmarks_bar": false, "idle_sleep": false, "allow_sites": ["a.org"], "allow_popups": ["b.org"], "adblock": false, "max_live_tabs": 2, "auto_update": false, "agent_mode": true, "toolbar": {"extensions": "hide", "downloads": "show", "shield": "hide", "private": "show", "music": "hide", "reader": "auto", "star": "show"}, "search_suggest": false, "download_dir": "C:/x", "download_ask": true, "download_show": false});
        assert_eq!(sanitize_settings(&good), good);
        assert_eq!(sanitize_settings(&Value::Null)["adblock"], true);
    }

    #[test]
    fn history_skips_broken_lines_and_keeps_the_final_title() {
        let text = "{\"url\":\"a\",\"title\":\"old\"}\n{broken\n{\"url\":\"a\",\"title\":\"new\"}\n{\"url\":\"b\",\"title\":\"b\"}\n";
        let parsed = parse_history(text);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0]["title"], "new");
    }

    #[test]
    fn saving_replaces_the_file_and_corrupt_files_are_kept() {
        let dir = std::env::temp_dir().join(format!("riffle-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("x.json");
        save_json(&path, &json!([1]));
        assert_eq!(load_json(&path), json!([1]));
        std::fs::write(&path, "{broken").unwrap();
        assert_eq!(load_json(&path), Value::Null);
        assert!(path.with_extension("corrupt").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn filter_lists_block_by_path_and_type_and_allow_exceptions() {
        let list = "||ads.test^\n/banner-ad.$image\n@@||ads.test/ok.js\n";
        let engine = build_engine(vec![list.to_string()]);
        let source = "https://site.test/";
        assert!(is_blocked(&engine, "https://ads.test/a.js", source, "script"));
        assert!(is_blocked(&engine, "https://cdn.test/banner-ad.png", source, "image"));
        assert!(!is_blocked(&engine, "https://cdn.test/banner-ad.png", source, "script"));
        assert!(!is_blocked(&engine, "https://ads.test/ok.js", source, "script"));
        assert!(is_blocked(&engine, "https://ads.test/popunder", source, "document"));
        assert!(!is_blocked(&engine, "https://site.test/app.js", source, "script"));
    }

    #[test]
    fn filter_lists_hide_ad_boxes_by_site_and_by_name() {
        let engine = build_engine(vec!["site.test##.sitead\n##.adbox\nok.test#@#.adbox\n".to_string()]);
        let css = ad_css(&engine, "https://site.test/", true, &["adbox".to_string(), "story".to_string()], &[]);
        assert!(css.contains(".sitead{display:none!important}") && css.contains(".adbox{display:none!important}"));
        assert!(!css.contains("story"));
        assert!(!ad_css(&engine, "https://ok.test/", false, &["adbox".to_string()], &[]).contains(".adbox"));
    }

    #[test]
    fn a_login_page_cannot_replace_a_filter_list() {
        assert!(!is_filter_list("<html>please sign in to the wifi</html>"));
        assert!(!is_filter_list("[Adblock Plus 2.0]\nshort"));
        assert!(is_filter_list(&format!("! Title: uBlock\n{}", "x".repeat(MIN_FILTER_LIST_BYTES))));
        assert!(is_filter_list(&format!("[Adblock Plus 2.0]\n{}", "x".repeat(MIN_FILTER_LIST_BYTES))));
    }

    #[test]
    fn custom_bookmarks_add_edit_and_reorder() {
        let urls = |b: &[Value]| b.iter().map(|x| x["url"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        let mut b = Vec::new();
        put_bookmark(&mut b, None, " Mail ", "https://a.test/");
        put_bookmark(&mut b, None, "", "https://b.test/");
        assert_eq!(b[0]["title"], "Mail");
        assert_eq!(b[1]["title"], "https://b.test/");
        put_bookmark(&mut b, None, "Again", "https://a.test/");
        assert_eq!(urls(&b), ["https://b.test/", "https://a.test/"]);
        put_bookmark(&mut b, Some("https://b.test/"), "Moved", "https://a.test/");
        assert_eq!(urls(&b), ["https://a.test/"]);
        put_bookmark(&mut b, None, "", "https://c.test/");
        move_bookmark(&mut b, "https://c.test/", true);
        assert_eq!(urls(&b), ["https://c.test/", "https://a.test/"]);
        move_bookmark(&mut b, "https://c.test/", true);
        move_bookmark(&mut b, "https://a.test/", false);
        assert_eq!(urls(&b), ["https://c.test/", "https://a.test/"]);
        assert_eq!(web_address("example.com").unwrap(), "https://example.com/");
        assert!(web_address("javascript:alert(1)").is_none() && web_address("two words").is_none());
    }

    #[test]
    fn menu_actions_are_all_handled_and_dividers_are_empty() {
        for (label, _, icon, action) in MENU.iter().chain(DEVELOPER_MENU) {
            assert_eq!(label.is_empty(), action.is_empty());
            assert_eq!(label.is_empty(), icon.is_empty());
            assert!(action.is_empty() || ICONS_HTML.contains(&format!("id=\"i-{icon}\"")), "{icon}");
        }
    }

    #[test]
    fn popup_rules_are_rescued_and_only_those() {
        let lists = vec!["! a $popup comment\n||trash.test^$popup\n||other.test^$popup,third-party\n||bait.test^$popunder\n||normal.test^$script\n##.ad\n".to_string()];
        assert_eq!(popup_rules(&lists), "||trash.test^\n||other.test^$third-party\n||bait.test^\n");
        let engine = build_engine(vec![popup_rules(&lists)]);
        assert!(is_blocked(&engine, "https://trash.test/x", "https://site.test/", "subdocument"));
        assert!(is_blocked(&engine, "https://other.test/x", "https://site.test/", "subdocument"));
        assert!(!is_blocked(&engine, "https://site2.test/", "https://site.test/", "subdocument"));
    }

    #[test]
    fn redirects_need_a_real_link_or_no_click_at_all() {
        // (trash, ad, click) -> allowed
        assert!(!navigation_verdict(true, false, Some(true)), "known trash is stopped even from a link");
        assert!(navigation_verdict(false, true, Some(true)), "a real link to an ad server is the user's choice");
        assert!(!navigation_verdict(false, true, None), "a page sending itself to an ad server");
        assert!(!navigation_verdict(false, false, Some(false)), "click hijack to an unknown site");
        assert!(navigation_verdict(false, false, Some(true)), "ordinary link");
        assert!(navigation_verdict(false, false, None), "login redirects with no click");
    }

    #[test]
    fn popups_need_a_real_link_or_a_click_on_the_same_site() {
        assert!(popup_verdict(false, false, Some(true)), "target=_blank link");
        assert!(popup_verdict(false, true, Some(false)), "same-site button");
        assert!(!popup_verdict(false, false, Some(false)), "click anywhere opens another site");
        assert!(!popup_verdict(false, true, None), "no click at all");
        assert!(!popup_verdict(false, false, None), "no click at all");
        assert!(!popup_verdict(true, false, Some(true)), "known trash");
    }

    #[test]
    fn same_site_groups_subdomains_only() {
        assert!(same_site("https://www.bbc.com/a", "https://news.bbc.com/b"));
        assert!(same_site("https://a.bbc.co.uk/", "https://b.bbc.co.uk/"));
        assert!(!same_site("https://bbc.co.uk/", "https://itv.co.uk/"));
        assert!(!same_site("https://site.com/", "https://trash.net/"));
        assert!(!same_site("about:blank", "about:blank"));
        assert!(same_site("https://www.gmx.de/", "https://auth.gmx.de/"));
    }

    #[test]
    fn a_chosen_folder_is_never_one_uninstalling_could_empty() {
        assert_eq!(riffle_folder(Path::new("D:\\Apps")), Path::new("D:\\Apps\\Riffle"));
        assert_eq!(riffle_folder(Path::new("D:\\")), Path::new("D:\\Riffle"));
        assert_eq!(riffle_folder(Path::new("D:\\riffle")), Path::new("D:\\riffle"));
        let base = std::env::temp_dir().join(format!("riffle-folder-test-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        assert_eq!(folder_problem(&base.join("new")), None);
        assert_eq!(folder_problem(&base), None);
        std::fs::write(base.join("holiday.jpg"), b"x").unwrap();
        assert!(folder_problem(&base).is_some());
        std::fs::write(base.join("riffle-browser.exe"), b"x").unwrap();
        assert_eq!(folder_problem(&base), None); // an earlier Riffle, being updated
        std::fs::remove_dir_all(&base).unwrap();
        assert!(folder_problem(Path::new("Riffle")).is_some());
    }

    #[test]
    fn install_scripts_survive_awkward_paths() {
        assert_eq!(ps_quote("C:\\O'Neil\\x"), "'C:\\O''Neil\\x'");
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
        let keep = uninstall_script("C:\\O'N\\r.exe", "C:\\l.lnk", "C:\\d.lnk", None, 7);
        assert!(keep.contains("'C:\\O''N\\r.exe' -and $_.Id -ne 7"), "{keep}");
        assert!(keep.contains("'C:\\d.lnk'") && !keep.contains("foreach"), "{keep}");
        assert!(uninstall_script("e", "l", "k", Some("data"), 7).contains("-LiteralPath 'data'"));
        assert!(remove_folder_script("C:\\O'N", 7).contains("Wait-Process -Id 7"));
        let installed = install_script("C:\\a b\\r.exe", "C:\\a b", "l", Some("C:\\d.lnk"), true);
        assert!(installed.contains("'\"C:\\a b\\uninstall.exe\"'"), "{installed}");
        assert!(installed.contains("'\"C:\\a b\\r.exe\" \"%1\"'"), "{installed}");
        assert!(!install_script("r", "d", "l", None, false).contains("startpin"));
    }

    #[test]
    fn stand_ins_decode_and_compiled_rules_survive_a_reload() {
        for text in [&b"Man"[..], b"Ma", b"M", b"", b"(function(){})();"] {
            assert_eq!(decode_base64(&base64(text)).unwrap(), text);
        }
        assert_eq!(parse_stand_in("data:application/javascript;base64,TWFu"), Some(("application/javascript".to_string(), b"Man".to_vec())));
        assert_eq!(parse_stand_in("https://evil.test/x.js"), None);
        assert_eq!(decode_base64("not base64!"), None);
        let dir = std::env::temp_dir().join(format!("riffle-cache-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let engine = build_engine(vec!["||ads.test^".to_string()]);
        save_engine_cache(&dir, &engine, &engine);
        let now = std::time::SystemTime::now() - Duration::from_secs(60);
        let loaded = cached_engine(&dir, "engine.dat", Some(now)).unwrap();
        assert!(is_blocked(&loaded, "https://ads.test/a.js", "https://site.test/", "script"));
        assert!(cached_engine(&dir, "engine.dat", Some(now + Duration::from_secs(3600))).is_none()); // a newer list wins
        std::fs::write(dir.join("engine.dat"), "damaged").unwrap();
        assert!(cached_engine(&dir, "engine.dat", Some(now)).is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn site_rules_reach_the_page_including_word_based_ones() {
        let engine = build_engine(vec!["site.test##.box\nsite.test##div:has-text(Sponsored)\n".to_string()]);
        let script = early_script(&engine, "https://site.test/");
        assert!(script.contains(".box{display:none") && script.contains("has-text") && script.contains("Sponsored"), "{script}");
        assert_eq!(early_script(&engine, "https://other.test/"), "");
    }

    #[test]
    fn rules_find_their_built_in_scripts_and_stand_ins() {
        let names = ["set, ads.enabled, false", "aopr, a", "acs, a, b", "nowoif", "nostif, a, 100", "rmnt, script, a", "aeld, click, a", "no-fetch-if, a", "aopw, a", "rpnt, script, a, b", "no-xhr-if, a", "aost, a, b", "nano-sib", "nosiif, a", "noeval-if, a", "nano-stb", "json-prune, a", "ra, onclick", "trusted-rpfr, a, b", "trusted-replace-xhr-response, a, b"];
        let mut rules: String = names.iter().map(|n| format!("site.test##+js({n})\n")).collect();
        rules.push_str("||ads.test/x.js$script,redirect=noopjs\n");
        let mut engine = build_engine(vec![rules]);
        engine.use_resources(built_in_resources());
        let script = engine.url_cosmetic_resources("https://site.test/").injected_script;
        assert_eq!(script.matches("(()=>{").count(), names.len(), "every rule above must find its script");
        assert!(script.contains("ads.enabled"), "{script}");
        let request = adblock::request::Request::new("https://ads.test/x.js", "https://site.test/", "script", "GET").unwrap();
        let stand_in = engine.check_network_request(&request).redirect.unwrap();
        assert_eq!(parse_stand_in(&stand_in).unwrap().0, "application/javascript");
    }

    #[test]
    fn editing_a_bookmark_keeps_its_icon_and_never_blanks_it() {
        let mut marks = vec![json!({"url": "https://a.test/", "title": "Old", "icon": "https://a.test/i.png"}), json!({"url": "https://b.test/", "title": "B"})];
        edit_bookmark(&mut marks, "https://a.test/", "  New  ", Some("https://a.test/page"));
        assert_eq!(marks[0], json!({"url": "https://a.test/page", "title": "New", "icon": "https://a.test/i.png"}));
        edit_bookmark(&mut marks, "https://a.test/page", "   ", None);
        assert_eq!(marks[0]["title"], "New");
        edit_bookmark(&mut marks, "https://none.test/", "X", None);
        assert_eq!(marks.len(), 2);
        edit_bookmark(&mut marks, "https://a.test/page", "Moved", Some("https://b.test/"));
        assert_eq!(marks, vec![json!({"url": "https://b.test/", "title": "Moved", "icon": null})]);
    }

    #[test]
    fn zoom_steps_stop_at_the_ends_and_reset() {
        assert_eq!(next_zoom(1.0, "in"), 1.1);
        assert_eq!(next_zoom(1.0, "out"), 0.9);
        assert_eq!(next_zoom(5.0, "in"), 5.0);
        assert_eq!(next_zoom(0.25, "out"), 0.25);
        assert_eq!(next_zoom(3.0, "reset"), 1.0);
        assert_eq!(next_zoom(1.0, "nonsense"), 1.0);
    }

    #[test]
    fn failures_get_plain_words_and_nothing_else_gets_a_page() {
        assert_eq!(error_page_text(COREWEBVIEW2_WEB_ERROR_STATUS_DISCONNECTED).unwrap().0, "No internet connection");
        assert_eq!(error_page_text(COREWEBVIEW2_WEB_ERROR_STATUS_HOST_NAME_NOT_RESOLVED).unwrap().0, "This site can't be found");
        assert!(error_page_text(COREWEBVIEW2_WEB_ERROR_STATUS_OPERATION_CANCELED).is_none()); // the user pressed stop
        assert!(error_page_text(COREWEBVIEW2_WEB_ERROR_STATUS_CONNECTION_ABORTED).is_none()); // downloads end like this
        assert!(error_page_text(COREWEBVIEW2_WEB_ERROR_STATUS_VALID_AUTHENTICATION_CREDENTIALS_REQUIRED).is_none());
        let page = error_page(error_page_look(COREWEBVIEW2_WEB_ERROR_STATUS_TIMEOUT), "A<b>", "x", &["t&u"], "<site>", "Go", false);
        assert!(page.contains("<circle") && page.contains("data-tone=\"warning\""));
        assert_eq!(error_page_look(COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_EXPIRED).1, "danger");
        assert!(page.contains("A&lt;b&gt;") && page.contains("<li>t&amp;u</li>") && page.contains("&lt;site&gt;") && !page.contains("__"), "{page}");
    }

    #[test]
    fn toolbar_choices_stay_valid() {
        let picked = sanitize_toolbar(&json!({"star": "hide", "private": "auto", "shield": "show", "downloads": "nonsense"}));
        assert_eq!(picked["star"], "hide");
        assert_eq!(picked["private"], "show"); // "when needed" means nothing for this one
        assert_eq!(picked["shield"], "show");
        assert_eq!(picked["downloads"], "auto");
    }

    #[test]
    fn months_come_out_right() {
        assert_eq!(month_key(0), "1970-01");
        assert_eq!(month_key(1_793_646_873), "2026-11");
        assert_eq!(month_key(1_709_164_800), "2024-02"); // 29 February 2024
        assert_eq!(month_key(1_735_689_599), "2024-12");
        assert_eq!(month_key(1_735_689_600), "2025-01");
    }

    #[test]
    fn pinned_tabs_keep_their_group() {
        assert_eq!(tab_position(5, 2, 6, true), 1);
        assert_eq!(tab_position(0, 2, 6, true), 0);
        assert_eq!(tab_position(0, 2, 6, false), 2);
        assert_eq!(tab_position(9, 2, 6, false), 5);
        assert_eq!(tab_position(3, 0, 4, false), 3);
    }

    #[test]
    fn import_takes_web_bookmarks_once_and_keeps_existing_names() {
        let file = json!({"roots": {"bookmark_bar": {"type": "folder", "children": [
            {"type": "url", "name": "A", "url": "https://a.test/"},
            {"type": "url", "name": "Sneaky", "url": "javascript:alert(1)"},
            {"type": "folder", "children": [{"type": "url", "name": "B", "url": "https://b.test/"}]},
        ]}, "other": {"type": "folder", "children": [{"type": "url", "name": "A again", "url": "https://a.test/"}]}}}).to_string();
        let found = parse_browser_bookmarks(&file);
        assert_eq!(found.len(), 3);
        let mut marks = vec![json!({"url": "https://b.test/", "title": "Mine"})];
        assert_eq!(merge_bookmarks(&mut marks, found), 1);
        assert_eq!(marks[0]["title"], "Mine");
        assert_eq!(marks.len(), 2);
        assert!(parse_browser_bookmarks("not json").is_empty());
    }

    #[test]
    fn only_a_newer_plain_version_counts() {
        assert!(is_newer("v0.2.0", "0.1.9") && is_newer("0.10.0", "0.9.0") && is_newer("v1.0.0", "0.9.9"));
        for tag in ["v0.1.0", "v0.0.9", "v9.0.0-rc1", "v9.0", "v9.0.0.0", "../evil", ""] {
            assert!(!is_newer(tag, "0.1.0"), "{tag}");
        }
    }

    #[test]
    fn checksum_list_gives_only_a_real_hash_for_that_file() {
        let hash = "A".repeat(64);
        assert_eq!(listed_hash(&format!("{hash}  RiffleSetup.exe\n"), "RiffleSetup.exe"), Some("a".repeat(64)));
        assert_eq!(listed_hash(&format!("{hash} *RiffleSetup.exe\r\n"), "RiffleSetup.exe"), Some("a".repeat(64)));
        assert_eq!(listed_hash(&format!("{hash}  Other.exe"), "RiffleSetup.exe"), None);
        assert_eq!(listed_hash("abc  RiffleSetup.exe", "RiffleSetup.exe"), None);
    }

    #[test]
    fn an_update_replaces_both_names_and_a_failed_one_keeps_the_old_version() {
        let dir = std::env::temp_dir().join(format!("riffle-swap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("riffle-browser.exe"), "old").unwrap();
        std::fs::hard_link(dir.join("riffle-browser.exe"), dir.join("uninstall.exe")).unwrap();
        assert!(swap_in_update(&dir).is_err()); // nothing downloaded
        assert_eq!(std::fs::read_to_string(dir.join("riffle-browser.exe")).unwrap(), "old");
        std::fs::write(dir.join("riffle-browser.new"), "new").unwrap();
        swap_in_update(&dir).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("riffle-browser.exe")).unwrap(), "new");
        assert_eq!(std::fs::read_to_string(dir.join("uninstall.exe")).unwrap(), "new");
        assert!(!dir.join("riffle-browser.new").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn downloads_never_replace_an_existing_file() {
        let dir = std::env::temp_dir().join(format!("riffle-unique-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let name = std::ffi::OsStr::new("a.tar.gz");
        assert_eq!(unique_path(&dir, name), dir.join("a.tar.gz"));
        std::fs::write(dir.join("a.tar.gz"), "").unwrap();
        assert_eq!(unique_path(&dir, name), dir.join("a.tar (1).gz"));
        std::fs::write(dir.join("a.tar (1).gz"), "").unwrap();
        assert_eq!(unique_path(&dir, name), dir.join("a.tar (2).gz"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn suggestions_rank_address_matches_and_keep_addresses_private() {
        let marks = [json!({"url": "https://news.test/you", "title": "Daily"})];
        let history = [json!({"url": "javascript:alert(1)", "title": "you"}), json!({"url": "https://www.youtube.com/", "title": "Video"}), json!({"url": "https://www.youtube.com/", "title": "Video"})];
        let found = local_suggestions("YOU", &marks, &history);
        assert_eq!(found.len(), 2);
        assert_eq!((found[0]["url"].as_str(), found[0]["complete"].as_str()), (Some("https://www.youtube.com/"), Some("youtube.com")));
        assert!(found[1]["complete"].is_null());
        assert!(local_suggestions("  ", &marks, &history).is_empty());
        assert_eq!(suggest_url(SEARCH_URL, "a b&c").unwrap(), "https://duckduckgo.com/ac/?type=list&q=a+b%26c");
        assert_eq!(suggest_url(SEARCH_URL, "secret.example.com/page"), None);
        assert_eq!(suggest_url("https://www.startpage.com/do/search?q=", "cats"), None);
        assert_eq!(parse_suggestions(r#"["c",["cat",1,"car"]]"#), ["cat", "car"]);
        assert!(parse_suggestions("<html>").is_empty());
    }

    #[test]
    fn devtools_docks_only_for_its_own_address_and_divider_stays_reachable() {
        let rect = |left, top, right, bottom| RECT { left, top, right, bottom };
        assert_eq!(frame_around(rect(720, 166, 1260, 853), rect(727, 196, 1253, 846)), (7, 30, 7, 7));
        assert!(devtools_title_matches("DevTools - example.com/", "https://example.com/"));
        assert!(devtools_title_matches("DevTools - about:blank", "about:blank"));
        assert!(devtools_title_matches("DevTools - example.com/lo...", "https://example.com/long/path"));
        assert!(!devtools_title_matches("DevTools - other.test/", "https://example.com/"));
        assert!(!devtools_title_matches("DevTools - ", "https://example.com/"));
        assert!(!devtools_title_matches("My DevTools - example.com/", "https://example.com/"));
        assert_eq!(devtools_share_at(500.0, 1000.0), 0.5);
        assert_eq!((devtools_share_at(0.0, 1000.0), devtools_share_at(1000.0, 1000.0)), (0.8, 0.2));
    }

    #[test]
    fn download_settings_fall_back_safely() {
        let s = sanitize_settings(&json!({"download_dir": "relative\\path"}));
        assert_eq!(s["download_dir"].as_str().unwrap(), default_download_dir());
        assert_eq!((s["download_ask"].as_bool(), s["download_show"].as_bool()), (Some(false), Some(true)));
        assert_eq!(sanitize_settings(&json!({"download_dir": "D:\\x"}))["download_dir"], "D:\\x");
    }

    #[test]
    fn unfinished_downloads_read_as_interrupted() {
        let path = std::env::temp_dir().join(format!("riffle-dl-{}.json", std::process::id()));
        std::fs::write(&path, r#"[{"state":"downloading"},{"state":"done"}]"#).unwrap();
        let list = load_downloads(&path);
        std::fs::remove_file(path).unwrap();
        assert_eq!((list[0]["state"].as_str(), list[1]["state"].as_str()), (Some("interrupted"), Some("done")));
    }

    #[test]
    fn one_history_entry_goes_and_other_visits_stay() {
        let text = "{\"url\":\"a\",\"time\":1}
{\"url\":\"b\",\"time\":2}
{\"url\":\"a\",\"time\":3}
";
        let kept = parse_history(&remove_history_entry(text, &json!("a"), &json!(1)));
        assert_eq!(kept.len(), 2);
        assert_eq!((kept[0]["url"].as_str(), kept[1]["time"].as_u64()), (Some("b"), Some(3)));
    }

    #[test]
    fn window_icon_is_valid() {
        assert!(Icon::from_rgba(logo_rgba(), LOGO_SIZE, LOGO_SIZE).is_ok());
    }
}
