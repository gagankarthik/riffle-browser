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
use webview2_com::{AcceleratorKeyPressedEventHandler, ContainsFullScreenElementChangedEventHandler};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::Foundation::{LPARAM, RECT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{CreateWindowExW, EnumWindows, FindWindowExW, GWL_EXSTYLE, GWL_STYLE, GetWindowLongPtrW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, HWND_TOP, IsWindow, IsWindowVisible, PostMessageW, SW_HIDE, SW_SHOWNOACTIVATE, SWP_FRAMECHANGED, SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetParent, SetWindowLongPtrW, SetWindowPos, ShowWindow, WINDOW_EX_STYLE, WM_CLOSE, WS_CAPTION, WS_CHILD, WS_CLIPCHILDREN, WS_EX_NOACTIVATE, WS_POPUP, WS_THICKFRAME};
use windows::Win32::UI::Shell::{FOLDERID_Desktop, FOLDERID_Downloads, FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS, FileOpenDialog, FileSaveDialog, IFileDialog, IShellItem, KF_FLAG_DEFAULT, SHCreateItemFromParsingName, SHGetKnownFolderPath, SIGDN_FILESYSPATH};
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
const MUSIC_BUTTON_RIGHT: f64 = 44.0; // distance from the music button's right edge to the window's
const DEFAULT_LIVE_TABS: u64 = 3; // the rest sleep and free their memory until selected
const SEARCH_URL: &str = "https://duckduckgo.com/?k1=-1&q="; // k1=-1 turns off DuckDuckGo's own ads
const AGENT_PORT: u16 = 9222;
const POPUP_MIN_GAP_MS: u128 = 1000;
const TOOLBAR_HTML: &str = include_str!("toolbar.html");
const PANEL_HTML: &str = include_str!("panel.html");
const ICONS_HTML: &str = include_str!("icons.html");
const MUSIC_HTML: &str = include_str!("music.html");
const MENU_HTML: &str = include_str!("menu.html");
const SHIELD_HTML: &str = include_str!("shield.html");
const SHIELD_WIDTH: f64 = 320.0;
const SHIELD_BUTTON_RIGHT: f64 = 116.0; // distance from the shield button's right edge to the window's
const MUSIC_WIDTH: f64 = 320.0;
const IDLE_SLEEP_SECS: u64 = 300; // background tabs untouched this long release their memory
const IDLE_CHECK_SECS: u64 = 60;
const MEDIA_PICK: &str = "const m=[...document.querySelectorAll('video,audio')],el=m.find(x=>!x.paused)||m[0];";
const NEW_TAB_HTML: &str = include_str!("newtab.html");
const INSTALL_HTML: &str = include_str!("install.html");
const SUGGEST_HTML: &str = include_str!("suggest.html");
const DOWNLOAD_HTML: &str = include_str!("download.html");
const DOWNLOAD_BUBBLE: (f64, f64) = (340.0, 104.0);
const DEVTOOLS_SCAN_MS: u64 = 250;
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
    ("peter-lowe.txt", "https://pgl.yoyo.org/adservers/serverlist.php?hostformat=adblockplus&showintro=0&mimetype=plaintext"),
];
// The small scripts uBlock's rules call on, for ads and anti-adblock walls that blocking requests cannot stop.
const SCRIPTLETS: (&str, &str) = ("scriptlets.json", "https://raw.githubusercontent.com/brave/adblock-resources/master/dist/resources.json");
const MENU_TRUST_SECS: u64 = 15;
const TRUST_SECS: u64 = 2; // a click explains a navigation or pop-up this soon after it
const VOUCH_SECS: u64 = 5; // what the user asked for directly, including the redirects on the way there
const FILTER_MAX_AGE_SECS: u64 = 4 * 24 * 60 * 60; // the lists themselves ask to be refetched every 4 days
const MIN_FILTER_LIST_BYTES: usize = 20_000;

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
    ("New private window", "Ctrl+Shift+N", "private", "private_tab"),
    ("Reopen closed tab", "Ctrl+Shift+T", "reopen", "reopen_tab"),
    ("", "", "", ""),
    ("Bookmarks", "Ctrl+Shift+O", "star", "bookmarks"),
    ("History", "Ctrl+H", "clock", "history"),
    ("Downloads", "Ctrl+J", "download", "downloads"),
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
    const s = document.createElement('style');
    s.textContent = 'ins.adsbygoogle,iframe[src*="doubleclick"],iframe[src*="googlesyndication"],[id^="google_ads_"],[id^="div-gpt-ad"],[data-ad-slot],[data-ad-client]{display:none!important}'
      + (yt ? 'ytd-ad-slot-renderer,ytd-in-feed-ad-layout-renderer,ytd-banner-promo-renderer,ytd-promoted-sparkles-web-renderer,#masthead-ad,#player-ads,.ytp-ad-overlay-container{display:none!important}' : '');
    (document.head || document.documentElement).appendChild(s);
  };
  if (document.documentElement) css(); else addEventListener('DOMContentLoaded', css);
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
  let timer = 0, first = true;
  const report = () => {
    timer = 0;
    const classes = [], ids = [];
    for (const el of document.querySelectorAll('[class],[id]')) {
      if (el.id && !seen.has('#' + el.id)) { seen.add('#' + el.id); ids.push(el.id); }
      for (const c of el.classList) if (!seen.has('.' + c)) { seen.add('.' + c); classes.push(c); }
    }
    if (first || classes.length || ids.length) window.ipc.postMessage('ads:' + JSON.stringify({first, classes, ids}));
    first = false;
  };
  const later = () => { if (!timer) timer = setTimeout(report, 400); };
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

// Forwards browser shortcuts from Riffle's own pages. Web pages never get this: see native_keys.
const KEY_SCRIPT: &str = r#"
if (window === top) addEventListener('keydown', e => {
  if (e.key === 'F12') { e.preventDefault(); return window.ipc.postMessage('dev_tools'); }
  if (!e.ctrlKey || e.altKey) return;
  const name = {t: 'new_tab', w: 'close_tab', l: 'focus_address', d: 'bookmark', h: 'history', j: 'downloads', T: 'reopen_tab', N: 'private_tab', O: 'bookmarks', I: 'dev_tools', u: 'view_source', Tab: e.shiftKey ? 'prev_tab' : 'next_tab'}[e.key];
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
if (window === top) for (const name of ['play', 'pause', 'ended', 'volumechange']) addEventListener(name, () => {
  const m = [...document.querySelectorAll('video,audio')], el = m.find(x => !x.paused) || m[0];
  if (el) window.ipc.postMessage('media:' + JSON.stringify({playing: !el.paused, volume: el.muted ? 0 : el.volume}));
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
    args: String,
    proxy: EventLoopProxy<Msg>,
    toolbar: Option<WebView>,
    tabs: Vec<Tab>,
    active: u64,
    fullscreen: bool, // the selected tab's video or element fills the screen
    update_ready: Option<String>, // the version that starts next time
    next_id: u64,
    clock: u64,
    last_popup: std::time::Instant,
    data_dir: PathBuf,
    settings: Value,
    private_window: bool, // every tab in this window is private
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
    set_allowed_sites(&settings);
    if !private_window {
        compact_history(&data_dir.join("history.jsonl"));
    }
    if adblock {
        let _ = SEED_ENGINE.set(seed_engine());
        load_filters(data_dir.clone(), !private_window); // two windows must not download over each other
    }
    let mut browser = Browser {
        window,
        context: WebContext::new(Some(data_dir.clone())),
        args: browser_args(agent_mode),
        proxy: event_loop.create_proxy(),
        toolbar: None,
        tabs: Vec::new(),
        active: 0,
        fullscreen: false,
        update_ready: None,
        next_id: 1,
        clock: 0,
        last_popup: std::time::Instant::now(),
        bookmarks: load_json(&data_dir.join("bookmarks.json")).as_array().cloned().unwrap_or_default(),
        downloads: load_downloads(&data_dir.join("downloads.json")),
        data_dir,
        settings,
        private_window,
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
                if event.physical_key == KeyCode::Escape && event.state == ElementState::Pressed {
                    browser.leave_fullscreen();
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
        let view = WebViewBuilder::new_with_web_context(&mut self.context)
            .with_additional_browser_args(self.args.clone())
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
                t.media.map(|(playing, volume)| json!({"id": t.id, "title": t.title, "site": site, "playing": playing, "volume": volume}))
            })
            .collect()
    }

    fn music_bounds(&self) -> Rect {
        let size = self.window.inner_size().to_logical::<f64>(self.window.scale_factor());
        let rows = self.music_items().len();
        let wanted = if rows == 0 { 72.0 } else { rows as f64 * 117.0 };
        let height = wanted.min(400.0).min((size.height - NAV_ROW_BOTTOM).max(0.0));
        let width = MUSIC_WIDTH.min(size.width);
        let x = (size.width - MUSIC_BUTTON_RIGHT - width).max(0.0);
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
        let built = WebViewBuilder::new_with_web_context(&mut self.context)
            .with_additional_browser_args(self.args.clone())
            .with_focused(focus) // on by default, which would pull typing out of the address bar
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

    // shortcut: runs as the page starts rather than before it, and in the top frame only, so a few anti-adblock
    // scripts can get in first. Upgrade by adding it through AddScriptToExecuteOnDocumentCreated per navigation.
    fn inject_scriptlets(&self, id: u64) {
        let (true, Some(engine)) = (self.adblock, ENGINE.get()) else { return };
        let Some(tab) = self.tabs.iter().find(|t| t.id == id) else { return };
        let Some(view) = &tab.view else { return };
        if site_is_allowed(&tab.url) {
            let _ = view.evaluate_script("window.__riffleAllow = true");
        } else if is_web(&tab.url) {
            let script = engine.url_cosmetic_resources(&tab.url).injected_script;
            if !script.is_empty() {
                let _ = view.evaluate_script(&script);
            }
        }
    }

    fn shield_bounds(&self) -> Rect {
        let size = self.window.inner_size().to_logical::<f64>(self.window.scale_factor());
        let width = SHIELD_WIDTH.min(size.width);
        let height = self.shield_height.min((size.height - NAV_ROW_BOTTOM).max(0.0));
        Rect { position: LogicalPosition::new((size.width - SHIELD_BUTTON_RIGHT - width).max(0.0), NAV_ROW_BOTTOM).into(), size: ViewSize::new(width, height).into() }
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
        let state = json!({"blocked": guard.blocked, "target": guard.stopped, "site": site, "on": !site_is_allowed(&tab.url), "popups": popups_are_allowed(&tab.url), "theme": self.settings["theme"], "private": tab.private});
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
        let _ = menu.evaluate_script(&format!("render({})", json!({"items": items, "theme": self.settings["theme"], "private": private, "sub": self.menu_developer, "accent": self.settings["accent"]})));
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
        let state = json!({"items": self.music_items(), "theme": self.settings["theme"], "accent": self.settings["accent"], "private": private});
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
        self.panel_page = page.to_string();
        if self.panel.is_some() {
            self.render_panel();
            return;
        }
        let proxy = self.proxy.clone();
        let bounds = self.bounds(false);
        let built = WebViewBuilder::new_with_web_context(&mut self.context)
            .with_additional_browser_args(self.args.clone())
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

    fn render_panel(&self) {
        let Some(panel) = &self.panel else { return };
        let mut history = parse_history(&read_lossy(&self.history_path()).unwrap_or_default());
        history.reverse();
        let state = json!({"page": self.panel_page, "bookmarks": self.bookmarks, "history": history, "settings": self.settings, "downloads": self.downloads.iter().rev().collect::<Vec<_>>(), "installed": is_installed(), "version": env!("CARGO_PKG_VERSION"), "update_ready": self.update_ready, "default_browser": self.panel_page == "settings" && is_default_browser()});
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
        if self.settings["restore"] != true || self.private_window {
            return;
        }
        let saved = load_json(&self.data_dir.join("session.json"));
        for entry in saved.as_array().into_iter().flatten() {
            let Some(url) = entry["url"].as_str().filter(|u| is_web(u)) else { continue };
            let title = entry["title"].as_str().unwrap_or(url).to_string();
            let id = self.next_id;
            self.next_id += 1;
            // Left asleep: only the selected tab loads, so restoring many tabs stays fast.
            self.tabs.push(Tab { id, title, url: url.to_string(), loading: false, last_used: 0, private: false, downloads: 0, icon: None, media: None, last_seen: std::time::Instant::now(), guard: Guard::new(), view: None, devtools: None });
        }
    }

    fn save_session(&self) {
        if self.private_window {
            return; // private tabs are never reopened, and the normal window's session stays untouched
        }
        let path = self.data_dir.join("session.json");
        if self.settings["restore"] != true {
            let _ = std::fs::remove_file(path);
            return;
        }
        let tabs: Vec<Value> = self.tabs.iter().filter(|t| !t.private && is_web(&t.url)).map(|t| json!({"url": t.url, "title": t.title})).collect();
        save_json(&path, &json!(tabs));
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
    fn open_private_window(&self) {
        let opened = std::env::current_exe().and_then(|exe| {
            let mut command = std::process::Command::new(exe);
            command.arg("--private");
            if std::env::args().any(|a| a == "--agent") {
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
        let url = tab.url.clone();
        let before = self.bookmarks.len();
        self.bookmarks.retain(|b| b["url"] != url.as_str());
        if self.bookmarks.len() == before {
            let icon = if tab.private { None } else { tab.icon.clone() };
            self.bookmarks.push(json!({"url": url, "title": tab.title, "icon": icon}));
        }
        self.save_bookmarks();
        self.layout();
        self.sync_toolbar();
    }

    fn save_bookmarks(&self) {
        save_json(&self.data_dir.join("bookmarks.json"), &json!(self.bookmarks));
    }

    // Picks up the current address too, because single-page sites change it without a page load.
    // shortcut: a site that changes neither title nor page is missed, enough for history in practice.
    fn note_visit(&mut self, id: u64) {
        let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) else { return };
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
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == self.active) {
                    match tab.devtools.take() {
                        // SAFETY: only asks the window to close, and fails harmlessly if it already has.
                        Some(hwnd) => drop(unsafe { PostMessageW(Some(HWND(hwnd as _)), WM_CLOSE, WPARAM(0), LPARAM(0)) }),
                        None => {
                            if let Some(view) = &tab.view {
                                view.open_devtools();
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
                self.layout();
            }
            "task_manager" => {
                if let Some(view) = self.active_view() {
                    // SAFETY: COM call on the UI thread that owns this web view.
                    let _ = view.webview().cast::<ICoreWebView2_6>().map(|w| unsafe { w.OpenTaskManagerWindow() });
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
        let mut builder = WebViewBuilder::new_with_web_context(&mut self.context)
            .with_additional_browser_args(self.args.clone())
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
                    return true;
                }
                guard.blocked += 1;
                guard.stopped = Some(url);
                drop(nav_refresh.send_event(Msg::Refresh));
                false
            })
            .with_initialization_script(MEDIA_SCRIPT);
        builder = if url == "about:blank" {
            // Theme and accent are sanitized settings, so they are safe to place in the page.
            builder.with_html(
                NEW_TAB_HTML
                    .replace("__THEME__", self.settings["theme"].as_str().unwrap_or("system"))
                    .replace("__ACCENT__", self.settings["accent"].as_str().unwrap_or(DEFAULT_ACCENT))
                    .replace("__PRIVATE__", if private { "true" } else { "false" }),
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
                report_fullscreen(view, id, fullscreen);
                if adblock {
                    block_requests(view, request_guard);
                }
            })
    }

    fn open_tab(&mut self, url: &str, private: bool) {
        let private = private || self.private_window;
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.push(Tab {
            id,
            title: "New tab".to_string(),
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
        let previous = self.active;
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == previous) {
            tab.last_seen = std::time::Instant::now();
        }
        if self.tabs[index].view.is_none() {
            let url = self.tabs[index].url.clone();
            match self.build_view(id, &url, self.tabs[index].private, self.tabs[index].guard.clone()) {
                Ok(view) => self.tabs[index].view = Some(view),
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
    }

    // Beside the page for the selected tab, hidden for the others. Chromium paints its own title bar and buttons, so
    // removing the window frame leaves them. Hanging past the host's edges is what cuts them off.
    fn place_devtools(&self) {
        let Some(host) = self.devtools_host else { return };
        let scale = self.window.scale_factor();
        let size = self.window.inner_size();
        let top = (self.bounds(true).size.to_logical::<f64>(scale).height * scale) as i32;
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
        // SAFETY: as above, for a window this thread made.
        let _ = unsafe { SetWindowPos(HWND(host as _), Some(HWND_TOP), size.width as i32 - width, top, width, height, SWP_NOACTIVATE | if shown { SWP_SHOWWINDOW } else { SWP_HIDEWINDOW }) };
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
    fn scan_devtools(&mut self) {
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
            self.layout();
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
            .map(|t| json!({"id": t.id, "title": t.title, "asleep": t.view.is_none(), "active": t.id == self.active, "private": t.private, "url": t.url, "icon": t.icon, "loading": t.loading, "playing": t.media.is_some_and(|(playing, _)| playing)}))
            .collect();
        let history_state = active.and_then(|t| t.view.as_ref()).map_or((false, false), can_go);
        let state = json!({
            "tabs": tabs,
            "url": active.map_or("", |t| if is_web(&t.url) || t.url.starts_with("blob:") { &t.url } else { "" }),
            "loading": active.is_some_and(|t| t.loading),
            "maximized": self.window.is_maximized(),
            "private": active.is_some_and(|t| t.private),
            "bookmarked": active.is_some_and(|t| self.bookmarks.iter().any(|b| b["url"] == t.url.as_str())),
            "bar": self.show_bookmarks_bar(),
            "bookmarks": if self.show_bookmarks_bar() { json!(self.bookmarks) } else { json!([]) },
            "adblock": self.adblock,
            "blocked": active.map_or(0, |t| t.guard.borrow().blocked),
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
            }
            Msg::DownloadDone(id, url, path, ok) => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
                    tab.downloads = tab.downloads.saturating_sub(1);
                }
                self.reload_downloads();
                if let Some(entry) = self.downloads.iter_mut().rev().find(|d| d["url"] == url.as_str() && d["state"] == "downloading") {
                    entry["state"] = json!(if ok { "done" } else { "failed" });
                    if let Some(path) = path {
                        entry["file"] = json!(Path::new(&path).file_name().map(|f| f.to_string_lossy()));
                        entry["path"] = json!(path);
                    }
                }
                self.save_downloads();
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
            Msg::UpdateReady(version) => {
                self.update_ready = Some(version);
                if self.panel_page == "settings" {
                    self.render_panel();
                }
            }
            Msg::Fullscreen(id, on) => {
                if id == self.active {
                    self.set_fullscreen(on);
                }
            }
            Msg::IdleCheck => self.sleep_idle_tabs(),
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
                    }
                    self.render_music();
                    self.sync_toolbar();
                    return false;
                }
                if let Some(report) = name.strip_prefix("ads:") {
                    self.hide_ads(id, report);
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
                    } else {
                        tab.guard.borrow_mut().last_click = None; // a click explains its own page, not what that page does next
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
            "menu_pick" => {
                self.pick_menu_item(command["value"].as_str().unwrap_or(""));
                return false;
            }
            "bookmark" => {
                self.toggle_bookmark();
                return false;
            }
            "music" => {
                self.toggle_music(target);
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
        let fetch = |file: &str, url: &str, valid: fn(&str) -> bool| {
            let path = data_dir.join(file);
            let age = std::fs::metadata(&path).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok());
            if download && age.is_none_or(|a| a.as_secs() > FILTER_MAX_AGE_SECS) {
                download_file(url, &path, valid); // on failure the older copy, if any, keeps working
            }
            std::fs::read_to_string(path).ok()
        };
        let lists: Vec<String> = FILTER_LISTS.iter().filter_map(|(file, url)| fetch(file, url, is_filter_list)).collect();
        if lists.is_empty() {
            return;
        }
        let _ = POPUP_ENGINE.set(build_engine(vec![popup_rules(&lists)]));
        let mut engine = build_engine(lists);
        // A missing or unreadable file only means no scriptlets: blocking itself still works.
        let scripts = fetch(SCRIPTLETS.0, SCRIPTLETS.1, |t| t.starts_with('[')).and_then(|t| serde_json::from_str::<Vec<adblock::resources::Resource>>(&t).ok());
        engine.use_resources(scripts.unwrap_or_default());
        let _ = ENGINE.set(engine);
    });
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

// Every request from the page passes through here. A blocked one gets an empty 403 and never reaches the network.
fn block_requests(view: &WebView, guard: Rc<RefCell<Guard>>) {
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
            if !site_is_allowed(&source) && [SEED_ENGINE.get(), ENGINE.get()].into_iter().flatten().any(|e| is_blocked(e, &url, &source, resource_kind(context))) {
                guard.borrow_mut().blocked += 1;
                let response = environment.CreateWebResourceResponse(None::<&windows::Win32::System::Com::IStream>, 403, w!("Blocked"), PCWSTR::null())?;
                args.SetResponse(&response)?;
            }
            Ok(())
        }));
        let _ = webview.add_WebResourceRequested(&handler, &mut token);
    }
}

fn key_shortcut(key: u32, shift: bool) -> Option<&'static str> {
    Some(match (key as u8 as char, shift) {
        ('T', false) => "new_tab",
        ('T', true) => "reopen_tab",
        ('W', false) => "close_tab",
        ('L', false) => "focus_address",
        ('D', false) => "bookmark",
        ('H', false) => "history",
        ('J', false) => "downloads",
        ('N', true) => "private_tab",
        ('U', false) => "view_source",
        ('I' | 'J' | 'C', true) => "dev_tools",
        ('O', true) => "bookmarks",
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

/// Tabs playing audio through any route, including players the page script cannot see.
fn plays_audio(view: &WebView) -> bool {
    let mut playing = BOOL(0);
    // SAFETY: COM call on the UI thread writing into a local value.
    view.webview().cast::<ICoreWebView2_8>().is_ok_and(|w| unsafe { w.IsDocumentPlayingAudio(&mut playing) }.is_ok() && playing.as_bool())
}

/// Private tabs never sleep: dropping the last one would erase that private session's logins.
fn must_stay_awake(tab: &Tab) -> bool {
    tab.private || tab.devtools.is_some() || tab.downloads > 0 || tab.media.is_some_and(|(playing, _)| playing) || tab.view.as_ref().is_some_and(plays_audio)
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

fn install_dir() -> Option<PathBuf> {
    local_dir("Programs\\Riffle")
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
    let desktop_part = desktop.map_or(String::new(), |d| format!("$s=(New-Object -ComObject WScript.Shell).CreateShortcut({}); $s.TargetPath={q_exe}; $s.Description='Riffle - fast, ad-free web browser'; $s.Save()\n", ps_quote(d)));
    // Best effort: Windows may refuse, and Riffle is still in the Start menu's list of apps.
    let start_part = if pin_start { format!("try {{ (New-Object -ComObject Shell.Application).Namespace((Split-Path {q_link})).ParseName((Split-Path {q_link} -Leaf)).InvokeVerb('startpin') }} catch {{}}\n") } else { String::new() };
    format!(
        "$ErrorActionPreference='Stop'
New-Item -ItemType Directory -Force (Split-Path {q_link}) | Out-Null
$s=(New-Object -ComObject WScript.Shell).CreateShortcut({q_link}); $s.TargetPath={q_exe}; $s.Description='Riffle - fast, ad-free web browser'; $s.Save()
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
foreach($k in @{{ApplicationName='Riffle';ApplicationDescription='Fast, ad-free web browser';ApplicationIcon={q_exe}}}.GetEnumerator()){{New-ItemProperty '{CLIENT_KEY}\\Capabilities' -Name $k.Key -Value $k.Value -PropertyType String -Force | Out-Null}}
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

/// Checks GitHub once and installs anything newer, returning its version.
fn update_once(dir: &Path) -> Option<String> {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let release: Value = serde_json::from_str(&fetch_text(LATEST_RELEASE)?).ok()?;
    let tag = release["tag_name"].as_str()?;
    // Numbers only from here on, so the tag is safe inside an address.
    if !is_newer(tag, env!("CARGO_PKG_VERSION")) {
        return None;
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
    Some(version)
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
                if let Some(version) = update_once(&dir) {
                    drop(proxy.send_event(Msg::UpdateReady(version)));
                    return;
                }
            }
            std::thread::sleep(Duration::from_secs(UPDATE_WAKE_SECS));
        }
    });
}

/// Copies Riffle to the user's own programs folder and lists it in Windows. Needs no administrator rights.
fn install_app(desktop: bool, pin_start: bool, step: impl Fn(u32, &str)) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = install_dir().ok_or("LOCALAPPDATA is not set")?;
    let link = start_menu_link().ok_or("APPDATA is not set")?;
    let target = dir.join("riffle-browser.exe");
    step(15, "Copying files");
    if !is_installed() {
        std::fs::create_dir_all(&dir)?;
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
    step(100, "Finishing");
    Ok(target)
}

enum SetupMsg {
    Script(String),
    Quit,
}

// The setup window, for installing or removing. In-process rather than a bundled installer tool, since Riffle's one exe already holds everything it copies.
fn run_setup(removing: bool) -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoopBuilder::<SetupMsg>::with_user_event().build();
    let window = WindowBuilder::new()
        .with_title(if removing { "Uninstall Riffle" } else { "Riffle Setup" })
        .with_inner_size(LogicalSize::new(520.0, 400.0))
        .with_resizable(false)
        .with_maximizable(false)
        .with_window_icon(Some(Icon::from_rgba(logo_rgba(), LOGO_SIZE, LOGO_SIZE)?))
        .build(&event_loop)?;
    // Its own profile: browser windows hold theirs with different engine flags, and it stores nothing worth keeping.
    let mut context = WebContext::new(Some(std::env::temp_dir().join("RiffleSetup")));
    let proxy = event_loop.create_proxy();
    let place = install_dir().ok_or("LOCALAPPDATA is not set")?.to_string_lossy().to_string();
    let view = WebViewBuilder::new_with_web_context(&mut context)
        .with_html(INSTALL_HTML)
        .with_drag_drop_handler(|_| true)
        .with_navigation_handler(is_built_in_page)
        .with_ipc_handler(move |req| {
            let command: Value = serde_json::from_str(req.body()).unwrap_or_default();
            let name = command["cmd"].as_str().unwrap_or("").to_string();
            match name.as_str() {
                "ready" => drop(proxy.send_event(SetupMsg::Script(format!("setup({},{removing})", json!(place))))),
                "quit" => drop(proxy.send_event(SetupMsg::Quit)),
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
                        let script = match install_app(command["desktop"] == true, command["start"] == true, step) {
                            Ok(_) => format!("finished({})", command["taskbar"] != true || taskbar_pinned()),
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
            Event::UserEvent(SetupMsg::Script(script)) => drop(view.evaluate_script(&script)),
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
        *path = unique_path(Path::new(&dir), &name);
    }
    true
}

/// Every setting comes back valid, whether it came from the panel or a hand-edited file.
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
        assert!(!browser_args(false).contains("remote-debugging"));
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
        assert_eq!(key_shortcut(0x4E, false), None); // plain Ctrl+N
        assert_eq!(key_shortcut(0x41, false), None); // Ctrl+A stays the page's
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
        assert_eq!(bad, json!({"theme": "system", "accent": DEFAULT_ACCENT, "search": SEARCH_URL, "home": "", "restore": false, "bookmarks_bar": true, "idle_sleep": true, "allow_sites": [], "allow_popups": [], "adblock": true, "max_live_tabs": 32, "auto_update": true, "search_suggest": true, "download_dir": default_download_dir(), "download_ask": false, "download_show": true}));
        let good = json!({"theme": "dark", "accent": "#112233", "search": "https://s.test/?q=", "home": "https://a.org/", "restore": true, "bookmarks_bar": false, "idle_sleep": false, "allow_sites": ["a.org"], "allow_popups": ["b.org"], "adblock": false, "max_live_tabs": 2, "auto_update": false, "search_suggest": false, "download_dir": "C:/x", "download_ask": true, "download_show": false});
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
