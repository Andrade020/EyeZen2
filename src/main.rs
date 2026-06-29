#![windows_subsystem = "windows"]

use eframe::{egui, App, CreationContext, Frame, NativeOptions};
use egui::{Color32, RichText, Stroke, Vec2, ViewportCommand};
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::mpsc::{channel, Receiver},
    time::{Duration, Instant},
};
use tray_icon::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder,
};

// ─── Filtros ─────────────────────────────────────────────────────────────────
//
// Índices:
//   0-5   → Página principal (grid 3×2)
//   6     → Desativar (botão separado)
//   7-14  → Página "Outros" (grid 4×2)

enum FilterKind {
    Linear { r: f64, g: f64, b: f64 },
    ToneMap { r: f64, g: f64, b: f64, scale: f64, lift: f64 },
    /// Usa MagSetFullscreenColorEffect — sem restrição de ganho mínimo
    ColorMatrix { matrix: [f32; 25] },
}

struct FilterDef {
    name:  &'static str,
    emoji: &'static str,
    kind:  FilterKind,
    color: Color32,
}

fn all_filters() -> Vec<FilterDef> {
    vec![
        FilterDef { name: "Noturno Suave",  emoji: "\u{1F319}", kind: FilterKind::Linear { r: 0.95, g: 1.00, b: 0.85 }, color: Color32::from_rgb(93,  78, 55) },
        FilterDef { name: "Leitura Verde",  emoji: "\u{1F7E2}", kind: FilterKind::Linear { r: 0.88, g: 1.00, b: 0.88 }, color: Color32::from_rgb(46, 125, 50) },
        FilterDef { name: "Papel Creme",    emoji: "\u{1F7E1}", kind: FilterKind::Linear { r: 1.00, g: 0.98, b: 0.85 }, color: Color32::from_rgb(139,115, 85) },
        FilterDef { name: "Fique Acordado", emoji: "\u{2600}",  kind: FilterKind::Linear { r: 0.95, g: 1.00, b: 1.15 }, color: Color32::from_rgb(0,  119,190) },
        FilterDef { name: "Crepusculo",     emoji: "\u{1F49C}", kind: FilterKind::Linear { r: 1.00, g: 0.88, b: 1.05 }, color: Color32::from_rgb(106, 27,154) },
        // idx 5 = Modo Feliz (saturação +25% via ColorMatrix: vivid, cheerful)
        FilterDef { name: "Modo Feliz", emoji: "\u{2728}", kind: FilterKind::ColorMatrix { matrix: [
            // s=1.25: R'=sR+(1-s)L, G'=sG+(1-s)L, B'=sB+(1-s)L + 2% brightness offset
            1.19685, -0.05315, -0.05315, 0., 0.,
            -0.1788,  1.07120, -0.17880, 0., 0.,
            -0.01805,-0.01805,  1.23195, 0., 0.,
            0.,       0.,       0.,      1., 0.,
            0.02,     0.02,     0.02,    0., 1.,
        ]}, color: Color32::from_rgb(200, 160, 0) },
        // idx 6 = Desativar
        FilterDef { name: "Desativado",     emoji: "\u{23FA}",  kind: FilterKind::Linear { r: 1.00, g: 1.00, b: 1.00 }, color: Color32::from_rgb(60,  60, 70) },
        // idx 7-16 = Outros
        FilterDef { name: "Sepia Classico", emoji: "\u{1F3A8}", kind: FilterKind::Linear { r: 1.00, g: 0.95, b: 0.82 }, color: Color32::from_rgb(112, 66, 20) },
        FilterDef { name: "Fosforo Ambar",  emoji: "\u{1F7E0}", kind: FilterKind::ColorMatrix { matrix: [
            0.2126, 0.1318, 0.0, 0.0, 0.0,  0.7152, 0.4434, 0.0, 0.0, 0.0,
            0.0722, 0.0448, 0.0, 0.0, 0.0,  0.0, 0.0, 0.0, 1.0, 0.0,  0.0, 0.0, 0.0, 0.0, 1.0,
        ]}, color: Color32::from_rgb(180,100,  0) },
        FilterDef { name: "Astronomo",      emoji: "\u{1F52D}", kind: FilterKind::ColorMatrix { matrix: [
            0.8, 0.0, 0.0, 0.0, 0.0,  0.0, 0.08, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.02, 0.0, 0.0,  0.0, 0.0, 0.0, 1.0, 0.0,  0.0, 0.0, 0.0, 0.0, 1.0,
        ]}, color: Color32::from_rgb(140, 10, 10) },
        FilterDef { name: "Nevoa",          emoji: "\u{1F32B}", kind: FilterKind::ToneMap  { r: 1.00, g: 0.99, b: 0.96, scale: 0.74, lift: 0.13 }, color: Color32::from_rgb(80,  90,110) },
        FilterDef { name: "Curva S Suave",  emoji: "\u{3030}",  kind: FilterKind::ToneMap  { r: 1.00, g: 0.96, b: 0.87, scale: 0.86, lift: 0.07 }, color: Color32::from_rgb(100, 75, 45) },
        FilterDef { name: "Vaporwave",      emoji: "\u{1F49C}", kind: FilterKind::Linear { r: 1.08, g: 0.78, b: 1.05 }, color: Color32::from_rgb(160, 20,160) },
        FilterDef { name: "Vela",           emoji: "\u{1F56F}", kind: FilterKind::ColorMatrix { matrix: [
            1.0, 0.0, 0.0, 0.0, 0.0,  0.0, 0.54, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.12, 0.0, 0.0,  0.0, 0.0, 0.0, 1.0, 0.0,  0.0, 0.0, 0.0, 0.0, 1.0,
        ]}, color: Color32::from_rgb(180, 80, 10) },
        FilterDef { name: "Kodak Portra",   emoji: "\u{1F4F7}", kind: FilterKind::Linear { r: 1.06, g: 0.97, b: 0.87 }, color: Color32::from_rgb(130, 90, 50) },
        FilterDef { name: "Oceano",         emoji: "\u{1F30A}", kind: FilterKind::Linear { r: 0.90, g: 0.97, b: 1.08 }, color: Color32::from_rgb(2,   80,130) },
        FilterDef { name: "Modo Retro",     emoji: "\u{1F4FA}", kind: FilterKind::ToneMap { r: 1.02, g: 0.96, b: 0.72, scale: 0.85, lift: 0.08 }, color: Color32::from_rgb(150, 90, 20) },
    ]
}

const IDX_DESATIVAR:    usize = 6;
const IDX_OUTROS_START: usize = 7;
const TRANS_DURATION:   Duration = Duration::from_millis(1400);

// ─── Ramp helpers ────────────────────────────────────────────────────────────

fn neutral_ramp() -> [u16; 768] {
    let mut r = [0u16; 768];
    for i in 0..256usize { r[i] = (i as u16)*257; r[256+i] = (i as u16)*257; r[512+i] = (i as u16)*257; }
    r
}

fn compute_ramp(kind: &FilterKind, intensity: f64) -> [u16; 768] {
    let mut ramp = [0u16; 768];
    match kind {
        FilterKind::Linear { r, g, b } => {
            let (ri, gi, bi) = (1.0+(r-1.0)*intensity, 1.0+(g-1.0)*intensity, 1.0+(b-1.0)*intensity);
            for i in 0..256usize {
                ramp[i]     = (i as f64*257.0*ri).clamp(0.0,65535.0) as u16;
                ramp[256+i] = (i as f64*257.0*gi).clamp(0.0,65535.0) as u16;
                ramp[512+i] = (i as f64*257.0*bi).clamp(0.0,65535.0) as u16;
            }
        }
        FilterKind::ToneMap { r, g, b, scale, lift } => {
            let (ri,gi,bi) = (1.0+(r-1.0)*intensity, 1.0+(g-1.0)*intensity, 1.0+(b-1.0)*intensity);
            let (si, li)   = (1.0+(scale-1.0)*intensity, lift*intensity);
            for i in 0..256usize {
                let t = (i as f64/255.0*si+li).clamp(0.0,1.0);
                ramp[i]     = (t*ri*65535.0).clamp(0.0,65535.0) as u16;
                ramp[256+i] = (t*gi*65535.0).clamp(0.0,65535.0) as u16;
                ramp[512+i] = (t*bi*65535.0).clamp(0.0,65535.0) as u16;
            }
        }
        FilterKind::ColorMatrix { .. } => return neutral_ramp(),
    }
    ramp
}

fn compute_matrix_at(matrix: &[f32; 25], intensity: f64) -> [f32; 25] {
    let id: [f32; 25] = [1.,0.,0.,0.,0., 0.,1.,0.,0.,0., 0.,0.,1.,0.,0., 0.,0.,0.,1.,0., 0.,0.,0.,0.,1.];
    let mut m = [0f32; 25];
    for i in 0..25 { m[i] = id[i] + (matrix[i]-id[i]) * intensity as f32; }
    m
}

#[cfg(windows)]
fn set_gamma_ramp(ramp: &[u16; 768]) {
    use winapi::um::wingdi::SetDeviceGammaRamp;
    use winapi::um::winuser::{GetDC, ReleaseDC};
    unsafe {
        let hdc = GetDC(std::ptr::null_mut());
        if hdc.is_null() { return; }
        SetDeviceGammaRamp(hdc, ramp.as_ptr() as *mut _);
        ReleaseDC(std::ptr::null_mut(), hdc);
    }
}
#[cfg(not(windows))]
fn set_gamma_ramp(_: &[u16; 768]) {}

fn reset_gamma() { set_gamma_ramp(&neutral_ramp()); }

// ─── Magnification API ───────────────────────────────────────────────────────

#[cfg(windows)]
mod mag_api {
    use winapi::um::libloaderapi::{GetProcAddress, LoadLibraryA};
    #[repr(C)] struct ColorEffect { transform: [f32; 25] }
    const IDENTITY: ColorEffect = ColorEffect { transform: [1.,0.,0.,0.,0., 0.,1.,0.,0.,0., 0.,0.,1.,0.,0., 0.,0.,0.,1.,0., 0.,0.,0.,0.,1.] };
    const INV_GRAY: ColorEffect = ColorEffect { transform: [-0.2126,-0.2126,-0.2126,0.,0., -0.7152,-0.7152,-0.7152,0.,0., -0.0722,-0.0722,-0.0722,0.,0., 0.,0.,0.,1.,0., 1.,1.,1.,0.,1.] };
    type BoolFn   = unsafe extern "system" fn() -> i32;
    type EffectFn = unsafe extern "system" fn(*const ColorEffect) -> i32;
    unsafe fn proc(func: &[u8]) -> usize {
        let h = LoadLibraryA(b"magnification.dll\0".as_ptr() as _);
        if h.is_null() { return 0; }
        GetProcAddress(h, func.as_ptr() as _) as usize
    }
    pub fn init()   { unsafe { let a=proc(b"MagInitialize\0");   if a!=0 { let f:BoolFn=std::mem::transmute(a); f(); } } }
    fn set(e: &ColorEffect) { unsafe { let a=proc(b"MagSetFullscreenColorEffect\0"); if a!=0 { let f:EffectFn=std::mem::transmute(a); f(e); } } }
    pub fn apply_inv_gray()            { set(&INV_GRAY); }
    pub fn apply_matrix(m: &[f32;25]) { let e=ColorEffect{transform:*m}; set(&e); }
    pub fn reset()                     { set(&IDENTITY); }
    pub fn uninit() { unsafe { let a=proc(b"MagUninitialize\0"); if a!=0 { let f:BoolFn=std::mem::transmute(a); f(); } } }
}
#[cfg(not(windows))]
mod mag_api {
    pub fn init(){} pub fn apply_inv_gray(){} pub fn apply_matrix(_m:&[f32;25]){}
    pub fn reset(){} pub fn uninit(){}
}

// ─── Agendamento ─────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
struct ScheduleSlot {
    start_h:    u8,
    start_m:    u8,
    end_h:      u8,
    end_m:      u8,
    filter_idx: usize,
}

fn find_scheduled_filter(slots: &[ScheduleSlot], n: usize) -> Option<usize> {
    if slots.is_empty() { return None; }
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let mins_now = ((secs / 60) % (24 * 60)) as u32;
    for s in slots.iter().filter(|s| s.filter_idx < n) {
        let start = s.start_h as u32 * 60 + s.start_m as u32;
        let end   = s.end_h   as u32 * 60 + s.end_m   as u32;
        let active = if start <= end {
            mins_now >= start && mins_now < end
        } else {
            mins_now >= start || mins_now < end
        };
        if active { return Some(s.filter_idx); }
    }
    Some(IDX_DESATIVAR)
}

// ─── Filtro por app ──────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
struct AppRule {
    process_name: String,
    filter_idx:   usize,
}

fn get_foreground_process_name() -> Option<String> {
    use winapi::um::winuser::{GetForegroundWindow, GetWindowThreadProcessId};
    use winapi::um::processthreadsapi::OpenProcess;
    use winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION;
    use winapi::um::winbase::QueryFullProcessImageNameW;
    use winapi::um::handleapi::CloseHandle;
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() { return None; }
        let mut pid: winapi::shared::minwindef::DWORD = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 { return None; }
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() { return None; }
        let mut buf = [0u16; 260];
        let mut len: winapi::shared::minwindef::DWORD = 260;
        QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len);
        CloseHandle(handle);
        if len == 0 { return None; }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.split('\\').last().map(|s| s.to_lowercase())
    }
}

const PRESET_APPS: &[(&str, &str)] = &[
    ("Chrome",      "chrome.exe"),
    ("Firefox",     "firefox.exe"),
    ("Edge",        "msedge.exe"),
    ("Brave",       "brave.exe"),
    ("Opera",       "opera.exe"),
    ("VLC",         "vlc.exe"),
    ("MPV",         "mpv.exe"),
    ("Word",        "winword.exe"),
    ("Excel",       "excel.exe"),
    ("PowerPoint",  "powerpnt.exe"),
    ("Acrobat",     "acrobat.exe"),
    ("Foxit",       "foxitpdfreader.exe"),
    ("Obsidian",    "obsidian.exe"),
    ("VS Code",     "code.exe"),
    ("Notepad++",   "notepad++.exe"),
    ("Spotify",     "spotify.exe"),
];

// ─── Config ──────────────────────────────────────────────────────────────────

fn default_intensity() -> f64 { 1.0 }

#[derive(Serialize, Deserialize)]
struct Config {
    last_filter:        usize,
    inverted_grayscale: bool,
    #[serde(default = "default_intensity")]
    intensity:          f64,
    #[serde(default)]
    schedule_enabled:   bool,
    #[serde(default)]
    schedule_slots:     Vec<ScheduleSlot>,
    #[serde(default)]
    app_rules_enabled:  bool,
    #[serde(default)]
    app_rules:          Vec<AppRule>,
}

fn config_path() -> std::path::PathBuf {
    let mut p = std::env::current_exe().unwrap_or_default();
    p.set_file_name("eyezen_config.json");
    p
}

fn load_config(n: usize) -> Config {
    std::fs::read_to_string(config_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Config { last_filter: IDX_DESATIVAR, inverted_grayscale: false,
                            intensity: 1.0, schedule_enabled: false, schedule_slots: vec![],
                            app_rules_enabled: false, app_rules: vec![] })
        .pipe(|mut c| { c.last_filter = c.last_filter.min(n-1); c.intensity = c.intensity.clamp(0.0,1.5); c })
}

fn save_config(filter: usize, inverted: bool, intensity: f64, sched_on: bool, slots: &[ScheduleSlot], app_on: bool, app_rules: &[AppRule]) {
    let _ = std::fs::write(
        config_path(),
        serde_json::to_string(&Config {
            last_filter: filter, inverted_grayscale: inverted, intensity,
            schedule_enabled: sched_on, schedule_slots: slots.to_vec(),
            app_rules_enabled: app_on, app_rules: app_rules.to_vec(),
        }).unwrap_or_default(),
    );
}

trait Pipe: Sized { fn pipe<R>(self, f: impl FnOnce(Self) -> R) -> R { f(self) } }
impl<T> Pipe for T {}

// ─── Transição ───────────────────────────────────────────────────────────────

struct Transition { from: [u16;768], to: [u16;768], start: Instant }

impl Transition {
    fn sample(&self) -> Option<[u16;768]> {
        let raw_t = (self.start.elapsed().as_secs_f64() / TRANS_DURATION.as_secs_f64()).min(1.0);
        if raw_t >= 1.0 { return None; }
        let t = 1.0 - (1.0-raw_t).powi(3); // ease-out cubic
        let mut ramp = [0u16;768];
        for i in 0..768 {
            ramp[i] = (self.from[i] as f64 + (self.to[i] as f64 - self.from[i] as f64)*t).clamp(0.0,65535.0) as u16;
        }
        Some(ramp)
    }
}

// ─── Events ──────────────────────────────────────────────────────────────────

enum AppEvent { ApplyFilter(usize), ToggleInvertedGrayscale }

#[derive(PartialEq)]
enum Page { Main, Outros, Agenda, Apps }

// ─── Ícone bandeja ───────────────────────────────────────────────────────────

fn make_tray_icon(inverted: bool) -> tray_icon::Icon {
    let size = 32u32;
    let mut rgba = vec![0u8; (size*size*4) as usize];
    let cx = size as f32 / 2.0;
    for y in 0..size { for x in 0..size {
        let dist = ((x as f32-cx).powi(2)+(y as f32-cx).powi(2)).sqrt();
        let i = ((y*size+x)*4) as usize;
        if dist <= cx-1.0 {
            let t = ((cx-1.0-dist)/(cx-1.0)*0.5+0.5) as f32;
            rgba[i]   = if inverted {(220.0*t) as u8} else {0};
            rgba[i+1] = if inverted {(80.0*t)  as u8} else {(180.0*t) as u8};
            rgba[i+2] = if inverted {(220.0*t) as u8} else {(100.0*t) as u8};
            rgba[i+3] = 255;
        }
    }}
    tray_icon::Icon::from_rgba(rgba, size, size).expect("icon")
}

// ─── App ─────────────────────────────────────────────────────────────────────

struct EyeZenApp {
    filters:              Vec<FilterDef>,
    current:              usize,
    inverted_grayscale:   bool,
    uses_mag_filter:      bool,
    intensity:            f64,
    current_ramp:         [u16;768],
    transition:           Option<Transition>,
    schedule_enabled:     bool,
    schedule_slots:       Vec<ScheduleSlot>,
    sched_last_applied:   Option<usize>,
    last_sched_check:     Instant,
    app_rules_enabled:    bool,
    app_rules:            Vec<AppRule>,
    last_manual_filter:   usize,
    last_foreground:      String,
    last_app_check:       Instant,
    app_rule_active:      bool,
    apps_page_input:      String,
    page:                 Page,
    rx:                   Receiver<AppEvent>,
    _tray:                TrayIcon,
    tray_items:           Vec<CheckMenuItem>,
    tray_invert_item:     CheckMenuItem,
    exit_item_id:         tray_icon::menu::MenuId,
    show_item_id:         tray_icon::menu::MenuId,
    toast:                Option<(String, Instant)>,
}

impl EyeZenApp {
    fn new(cc: &CreationContext, rx: Receiver<AppEvent>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        if let Ok(b) = std::fs::read(r"C:\Windows\Fonts\seguiemj.ttf") {
            fonts.font_data.insert("emoji".into(), egui::FontData::from_owned(b));
            for fam in fonts.families.values_mut() { fam.push("emoji".into()); }
        }
        cc.egui_ctx.set_fonts(fonts);
        let mut vis = egui::Visuals::dark();
        vis.window_fill = Color32::from_rgb(22,22,28);
        vis.panel_fill  = Color32::from_rgb(22,22,28);
        cc.egui_ctx.set_visuals(vis);

        let filters  = all_filters();
        let cfg      = load_config(filters.len());
        let current  = cfg.last_filter;
        let inverted = cfg.inverted_grayscale;
        let intensity = cfg.intensity;

        let uses_mag = match &filters[current].kind {
            FilterKind::ColorMatrix { matrix } => { mag_api::apply_matrix(&compute_matrix_at(matrix, intensity)); true }
            _ => { set_gamma_ramp(&compute_ramp(&filters[current].kind, intensity)); false }
        };
        if inverted { mag_api::apply_inv_gray(); }

        let current_ramp = if uses_mag { neutral_ramp() } else { compute_ramp(&filters[current].kind, intensity) };

        // Tray
        let menu = Menu::new();
        let mut tray_items = Vec::new();
        for (i, f) in filters[..IDX_OUTROS_START].iter().enumerate() {
            let item = CheckMenuItem::new(f.name, true, i==current, None);
            let _ = menu.append(&item); tray_items.push(item);
        }
        let _ = menu.append(&PredefinedMenuItem::separator());
        for (i, f) in filters[IDX_OUTROS_START..].iter().enumerate() {
            let idx = IDX_OUTROS_START+i;
            let item = CheckMenuItem::new(f.name, true, idx==current, None);
            let _ = menu.append(&item); tray_items.push(item);
        }
        let _ = menu.append(&PredefinedMenuItem::separator());
        let tray_invert_item = CheckMenuItem::new("Cinza Invertido", true, inverted, None);
        let _ = menu.append(&tray_invert_item);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let show_item    = MenuItem::new("Abrir Painel", true, None); let show_item_id = show_item.id().clone(); let _ = menu.append(&show_item);
        let exit_item    = MenuItem::new("Sair",         true, None); let exit_item_id = exit_item.id().clone(); let _ = menu.append(&exit_item);

        let tray = TrayIconBuilder::new()
            .with_icon(make_tray_icon(inverted))
            .with_tooltip(tooltip_for(current, inverted, &filters))
            .with_menu(Box::new(menu))
            .build().expect("tray");

        // Schedule: inicia "como se já tivesse aplicado o filtro atual" para não sobrepor na abertura
        let sched_last = if cfg.schedule_enabled {
            find_scheduled_filter(&cfg.schedule_slots, filters.len())
        } else { None };

        Self {
            filters, current, inverted_grayscale: inverted,
            uses_mag_filter: uses_mag, intensity, current_ramp,
            transition: None,
            schedule_enabled: cfg.schedule_enabled, schedule_slots: cfg.schedule_slots,
            sched_last_applied: sched_last,
            last_sched_check: Instant::now() - Duration::from_secs(60),
            app_rules_enabled: cfg.app_rules_enabled, app_rules: cfg.app_rules,
            last_manual_filter: current, last_foreground: String::new(),
            last_app_check: Instant::now(), app_rule_active: false,
            apps_page_input: String::new(),
            page: Page::Main,
            rx, _tray: tray, tray_items, tray_invert_item,
            exit_item_id, show_item_id, toast: None,
        }
    }

    fn apply_filter(&mut self, idx: usize, ctx: &egui::Context) {
        if idx >= self.filters.len() { return; }
        self.intensity = 1.0;
        self.last_manual_filter = idx;
        self.app_rule_active = false;
        self.do_apply(idx, None, ctx);
    }

    // Aplicação automática por regra de app (não reseta intensidade, não muda last_manual_filter)
    fn apply_filter_auto(&mut self, idx: usize, app_label: &str, ctx: &egui::Context) {
        if idx >= self.filters.len() { return; }
        self.app_rule_active = true;
        self.do_apply(idx, Some(app_label), ctx);
    }

    fn do_apply(&mut self, idx: usize, auto_label: Option<&str>, ctx: &egui::Context) {
        if self.inverted_grayscale || self.uses_mag_filter { mag_api::reset(); }
        if self.inverted_grayscale {
            self.inverted_grayscale = false;
            self.tray_invert_item.set_checked(false);
            let _ = self._tray.set_icon(Some(make_tray_icon(false)));
        }
        let is_cm = matches!(self.filters[idx].kind, FilterKind::ColorMatrix{..});
        if is_cm {
            if let FilterKind::ColorMatrix { matrix } = &self.filters[idx].kind {
                reset_gamma(); mag_api::apply_matrix(&compute_matrix_at(matrix, self.intensity));
            }
            self.uses_mag_filter = true; self.transition = None; self.current_ramp = neutral_ramp();
        } else {
            let target = compute_ramp(&self.filters[idx].kind, self.intensity);
            let from   = self.transition.as_ref().and_then(|t| t.sample()).unwrap_or(self.current_ramp);
            self.transition = Some(Transition { from, to: target, start: Instant::now() });
            if self.uses_mag_filter { reset_gamma(); }
            self.uses_mag_filter = false;
        }
        for (i, item) in self.tray_items.iter().enumerate() { item.set_checked(i==idx); }
        let _ = self._tray.set_tooltip(Some(tooltip_for(idx, false, &self.filters)));
        let fname = self.filters[idx].name;
        let msg = match auto_label {
            Some(app) => format!("\u{1F5A5}  {} \u{2192} {}", app, fname),
            None      => format!("{}  {}", self.filters[idx].emoji, fname),
        };
        self.toast = Some((msg, Instant::now())); self.current = idx;
        self.persist();
        ctx.request_repaint();
    }

    fn reapply_intensity(&mut self) {
        self.transition = None;
        if self.inverted_grayscale { return; }
        if matches!(self.filters[self.current].kind, FilterKind::ColorMatrix{..}) {
            if let FilterKind::ColorMatrix { matrix } = &self.filters[self.current].kind {
                mag_api::apply_matrix(&compute_matrix_at(matrix, self.intensity));
            }
        } else {
            let ramp = compute_ramp(&self.filters[self.current].kind, self.intensity);
            set_gamma_ramp(&ramp); self.current_ramp = ramp;
        }
    }

    fn toggle_inv_gray(&mut self, ctx: &egui::Context) {
        self.inverted_grayscale = !self.inverted_grayscale;
        if self.inverted_grayscale {
            self.transition = None; reset_gamma(); self.uses_mag_filter = false;
            for (i, item) in self.tray_items.iter().enumerate() { item.set_checked(i==IDX_DESATIVAR); }
            self.current = IDX_DESATIVAR; self.current_ramp = neutral_ramp();
            mag_api::apply_inv_gray();
            let _ = self._tray.set_icon(Some(make_tray_icon(true)));
            let _ = self._tray.set_tooltip(Some(String::from("EyeZen – Cinza Invertido")));
            self.toast = Some(("Cinza Invertido ativado".into(), Instant::now()));
        } else {
            mag_api::reset();
            let _ = self._tray.set_icon(Some(make_tray_icon(false)));
            let _ = self._tray.set_tooltip(Some(tooltip_for(self.current, false, &self.filters)));
            self.toast = Some(("Cinza Invertido desativado".into(), Instant::now()));
        }
        self.tray_invert_item.set_checked(self.inverted_grayscale);
        save_config(self.current, self.inverted_grayscale, self.intensity, self.schedule_enabled, &self.schedule_slots, self.app_rules_enabled, &self.app_rules);
        ctx.request_repaint();
    }

    fn persist(&self) {
        save_config(self.current, self.inverted_grayscale, self.intensity, self.schedule_enabled, &self.schedule_slots, self.app_rules_enabled, &self.app_rules);
    }
}

fn tooltip_for(idx: usize, inv: bool, filters: &[FilterDef]) -> String {
    if inv { return "EyeZen – Cinza Invertido".into(); }
    format!("EyeZen – {}", filters.get(idx).map(|f| f.name).unwrap_or("?"))
}

// ─── Visuais ─────────────────────────────────────────────────────────────────

const BG:       Color32 = Color32::from_rgb(22,22,28);
const BTN_IDLE: Color32 = Color32::from_rgb(38,38,48);
const ACCENT:   Color32 = Color32::from_rgb(0,210,130);

fn active_stroke() -> Stroke { Stroke::new(2.0, ACCENT) }
fn idle_stroke()   -> Stroke { Stroke::new(1.0, Color32::from_gray(52)) }

fn filter_btn(ui: &mut egui::Ui, label: &str, active: bool, color: Color32, width: f32) -> bool {
    let fill   = if active { color } else { BTN_IDLE };
    let stroke = if active { active_stroke() } else { idle_stroke() };
    let text_c = if active { Color32::WHITE } else { Color32::from_gray(190) };
    ui.add(egui::Button::new(RichText::new(label).size(13.0).color(text_c))
        .fill(fill).stroke(stroke).min_size(Vec2::new(width,36.0))).clicked()
}

fn draw_intensity_slider(ui: &mut egui::Ui, intensity: f64, new_intensity: &mut Option<f64>) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Intensidade").size(11.0).color(Color32::from_gray(160)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(format!("{}%", (intensity * 100.0).round() as u32)).size(11.0).color(ACCENT));
        });
    });
    ui.add_space(2.0);

    let track_h = 28.0_f32;
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, track_h), egui::Sense::click_and_drag());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let r = track_h / 2.0;
        let fill_frac = (intensity as f32 / 1.5).clamp(0.0, 1.0);

        // Track background
        painter.rect_filled(rect, r, Color32::from_rgb(35, 38, 48));

        // Filled portion
        let fill_w = rect.width() * fill_frac;
        if fill_w > 0.0 {
            let fill_rect = egui::Rect::from_min_size(
                rect.min,
                Vec2::new(fill_w.max(r * 2.0).min(rect.width()), track_h),
            );
            painter.rect_filled(fill_rect, r, ACCENT);
        }

        // 100% marker (subtle vertical line at 2/3 of the bar)
        let mark_x = rect.left() + rect.width() * (1.0 / 1.5);
        painter.line_segment(
            [egui::Pos2::new(mark_x, rect.top() + 4.0), egui::Pos2::new(mark_x, rect.bottom() - 4.0)],
            Stroke::new(1.5, Color32::from_rgba_premultiplied(255, 255, 255, 60)),
        );

        // Thumb
        let thumb_x = (rect.left() + rect.width() * fill_frac).clamp(rect.left() + r, rect.right() - r);
        let is_active = resp.hovered() || resp.dragged();
        let thumb_r = if is_active { r + 2.0 } else { r };
        painter.circle_filled(egui::Pos2::new(thumb_x, rect.center().y), thumb_r, Color32::WHITE);
        if is_active {
            painter.circle_stroke(
                egui::Pos2::new(thumb_x, rect.center().y),
                thumb_r + 1.5,
                Stroke::new(1.5, Color32::from_rgba_premultiplied(255, 255, 255, 70)),
            );
        }
    }

    if resp.dragged() || resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let t = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            *new_intensity = Some(t as f64 * 1.5);
        }
    }
}

// ─── impl App ────────────────────────────────────────────────────────────────

impl App for EyeZenApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        // ── Avança transição ──
        if let Some(tr) = &self.transition {
            match tr.sample() {
                Some(ramp) => { set_gamma_ramp(&ramp); ctx.request_repaint_after(Duration::from_millis(16)); }
                None => { let to = self.transition.take().unwrap().to; set_gamma_ramp(&to); self.current_ramp = to; }
            }
        }

        // ── Verifica app em foco (a cada 500ms) ──
        if self.app_rules_enabled && self.last_app_check.elapsed() > Duration::from_millis(500) {
            self.last_app_check = Instant::now();
            if let Some(proc) = get_foreground_process_name() {
                if proc != self.last_foreground {
                    self.last_foreground = proc.clone();
                    let hit = self.app_rules.iter()
                        .find(|r| r.process_name.to_lowercase() == proc)
                        .map(|r| r.filter_idx);
                    if let Some(fi) = hit {
                        if fi < self.filters.len() && (fi != self.current || self.inverted_grayscale) {
                            let display = PRESET_APPS.iter()
                                .find(|(_, p)| *p == proc.as_str())
                                .map(|(n, _)| *n)
                                .unwrap_or(&proc[..proc.len().min(20)]);
                            let label = display.to_string();
                            self.apply_filter_auto(fi, &label, ctx);
                        }
                        self.app_rule_active = true;
                    } else if self.app_rule_active {
                        self.app_rule_active = false;
                        let restore = self.last_manual_filter;
                        if restore != self.current || self.inverted_grayscale {
                            self.apply_filter_auto(restore, "", ctx);
                            // limpa toast de "→" para não confundir
                            self.toast = None;
                        }
                    }
                }
            }
        }

        // ── Verifica agendamento (a cada 30s) ──
        if self.schedule_enabled && self.last_sched_check.elapsed() > Duration::from_secs(30) {
            self.last_sched_check = Instant::now();
            let target = find_scheduled_filter(&self.schedule_slots, self.filters.len());
            if target != self.sched_last_applied {
                self.sched_last_applied = target;
                if let Some(idx) = target {
                    if idx != self.current || self.inverted_grayscale {
                        self.apply_filter(idx, ctx);
                    }
                }
            }
        }

        // ── Hotkeys ──
        while let Ok(ev) = self.rx.try_recv() {
            match ev {
                AppEvent::ApplyFilter(i) => self.apply_filter(i, ctx),
                AppEvent::ToggleInvertedGrayscale => self.toggle_inv_gray(ctx),
            }
        }

        // ── Tray menu ──
        if let Ok(ev) = MenuEvent::receiver().try_recv() {
            if ev.id == self.exit_item_id { reset_gamma(); mag_api::reset(); ctx.send_viewport_cmd(ViewportCommand::Close); return; }
            if ev.id == self.show_item_id { ctx.send_viewport_cmd(ViewportCommand::Visible(true)); ctx.send_viewport_cmd(ViewportCommand::Focus); }
            if *self.tray_invert_item.id() == ev.id { self.toggle_inv_gray(ctx); }
            for (i, item) in self.tray_items.iter().enumerate() { if *item.id()==ev.id { self.apply_filter(i, ctx); break; } }
        }

        // ── Clique na bandeja ──
        if tray_icon::TrayIconEvent::receiver().try_recv().is_ok() {
            ctx.send_viewport_cmd(ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(ViewportCommand::Focus);
        }

        // ── Toast ──
        if let Some((_, t)) = &self.toast {
            if t.elapsed() > Duration::from_secs(2) { self.toast = None; }
            else { ctx.request_repaint_after(Duration::from_millis(100)); }
        }

        // ── UI ──
        let mut to_apply:    Option<usize> = None;
        let mut toggle_inv                 = false;
        let mut go_outros                  = false;
        let mut go_main                    = false;
        let mut go_agenda                  = false;
        let mut go_apps                    = false;
        let mut minimize                   = false;
        let mut quit                       = false;
        let mut new_intensity: Option<f64> = None;

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(BG).inner_margin(egui::Margin::same(16.0)))
            .show(ctx, |ui| {
                match self.page {
                    Page::Main   => draw_main(ui, self, &mut to_apply, &mut toggle_inv, &mut go_outros, &mut go_agenda, &mut go_apps, &mut minimize, &mut quit, &mut new_intensity),
                    Page::Outros => draw_outros(ui, self, &mut to_apply, &mut go_main, &mut minimize, &mut quit, &mut new_intensity),
                    Page::Agenda => draw_agenda(ui, self, &mut go_main),
                    Page::Apps   => draw_apps(ui, self, &mut go_main),
                }
            });

        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
        }

        if let Some(idx) = to_apply  { self.apply_filter(idx, ctx); }
        if toggle_inv                { self.toggle_inv_gray(ctx); }
        if go_outros                 { self.page = Page::Outros; }
        if go_main                   { self.page = Page::Main; }
        if go_agenda                 { self.page = Page::Agenda; }
        if go_apps                   { self.page = Page::Apps; }
        if minimize                  { ctx.send_viewport_cmd(ViewportCommand::Visible(false)); }
        if let Some(v) = new_intensity { self.intensity = v; self.reapply_intensity(); self.persist(); }
        if quit { reset_gamma(); mag_api::reset(); ctx.send_viewport_cmd(ViewportCommand::Close); }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        reset_gamma(); mag_api::reset(); mag_api::uninit();
    }
}

// ─── Página Principal ────────────────────────────────────────────────────────

fn draw_main(
    ui: &mut egui::Ui, app: &EyeZenApp,
    to_apply: &mut Option<usize>, toggle_inv: &mut bool,
    go_outros: &mut bool, go_agenda: &mut bool, go_apps: &mut bool,
    minimize: &mut bool, quit: &mut bool, new_intensity: &mut Option<f64>,
) {
    ui.set_min_width(420.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new("EyeZen").size(26.0).color(ACCENT).strong());
        ui.label(RichText::new("Filtros de Conforto Visual").size(11.0).color(Color32::from_gray(135)));
    });
    ui.add_space(8.0); ui.separator(); ui.add_space(6.0);

    ui.columns(2, |cols| {
        cols[0].vertical_centered(|ui| { ui.label(RichText::new("Conforto").size(11.0).color(Color32::from_gray(165))); });
        cols[1].vertical_centered(|ui| { ui.label(RichText::new("Especiais").size(11.0).color(Color32::from_gray(165))); });
    });
    ui.add_space(4.0);

    for row in 0..3usize {
        ui.columns(2, |cols| {
            for col in 0..2 {
                let idx = row + col*3;
                let f = &app.filters[idx]; let active = app.current==idx && !app.inverted_grayscale;
                let w = cols[col].available_width();
                if filter_btn(&mut cols[col], &format!("{}  {}", f.emoji, f.name), active, f.color, w) { *to_apply = Some(idx); }
            }
        });
        ui.add_space(3.0);
    }
    ui.add_space(6.0); ui.separator(); ui.add_space(6.0);

    // Linha 1: [Desativar] [Outros →]
    ui.columns(2, |cols| {
        let desa = &app.filters[IDX_DESATIVAR];
        let active_d = app.current==IDX_DESATIVAR && !app.inverted_grayscale;
        let w0 = cols[0].available_width();
        if filter_btn(&mut cols[0], "\u{23FA}  Desativar", active_d, desa.color, w0) { *to_apply = Some(IDX_DESATIVAR); }

        let outros_active = app.current >= IDX_OUTROS_START && !app.inverted_grayscale;
        let w1 = cols[1].available_width();
        if cols[1].add(
            egui::Button::new(RichText::new("Outros \u{27A1}").size(13.0)
                .color(if outros_active {Color32::WHITE} else {Color32::from_gray(200)}))
                .fill(if outros_active {Color32::from_rgb(90,40,130)} else {BTN_IDLE})
                .stroke(if outros_active {Stroke::new(2.0,Color32::from_rgb(180,130,255))} else {idle_stroke()})
                .min_size(Vec2::new(w1,36.0))
        ).clicked() { *go_outros = true; }
    });
    ui.add_space(3.0);
    // Linha 2: [⏰ Agenda] [🖥 Apps]
    ui.columns(2, |cols| {
        let sched_on = app.schedule_enabled;
        let w0 = cols[0].available_width();
        if cols[0].add(
            egui::Button::new(RichText::new("\u{23F0}  Agenda").size(12.0)
                .color(if sched_on {Color32::from_rgb(255,220,100)} else {Color32::from_gray(190)}))
                .fill(if sched_on {Color32::from_rgb(80,60,0)} else {BTN_IDLE})
                .stroke(if sched_on {Stroke::new(2.0,Color32::from_rgb(220,180,50))} else {idle_stroke()})
                .min_size(Vec2::new(w0,36.0))
        ).clicked() { *go_agenda = true; }

        let apps_on = app.app_rules_enabled;
        let w1 = cols[1].available_width();
        if cols[1].add(
            egui::Button::new(RichText::new("\u{1F5A5}  Apps").size(12.0)
                .color(if apps_on {Color32::from_rgb(100,200,255)} else {Color32::from_gray(190)}))
                .fill(if apps_on {Color32::from_rgb(0,50,80)} else {BTN_IDLE})
                .stroke(if apps_on {Stroke::new(2.0,Color32::from_rgb(80,170,255))} else {idle_stroke()})
                .min_size(Vec2::new(w1,36.0))
        ).clicked() { *go_apps = true; }
    });

    ui.add_space(6.0);
    // Cinza Invertido
    let inv = app.inverted_grayscale;
    let w = ui.available_width();
    if ui.add(egui::Button::new(
        if inv { RichText::new("\u{1F30B}  Cinza Invertido  \u{2022} ATIVO").size(13.0).color(Color32::from_rgb(210,180,255)) }
        else   { RichText::new("\u{1F30B}  Cinza Invertido").size(13.0).color(Color32::from_gray(185)) }
    ).fill(if inv {Color32::from_rgb(55,40,80)} else {BTN_IDLE})
     .stroke(if inv {Stroke::new(2.0,Color32::from_rgb(180,130,255))} else {idle_stroke()})
     .min_size(Vec2::new(w,36.0))).clicked() { *toggle_inv = true; }

    if !inv { draw_intensity_slider(ui, app.intensity, new_intensity); }

    ui.add_space(4.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new("Ctrl+Alt+1…6 · 0=Desativar · 9=Cinza Inv.").size(9.5).color(Color32::from_gray(95)));
    });
    draw_toast_and_bottom(ui, app, minimize, quit);
}

// ─── Página Outros ───────────────────────────────────────────────────────────

fn draw_outros(
    ui: &mut egui::Ui, app: &EyeZenApp,
    to_apply: &mut Option<usize>, go_main: &mut bool,
    minimize: &mut bool, quit: &mut bool, new_intensity: &mut Option<f64>,
) {
    ui.set_min_width(420.0);
    ui.horizontal(|ui| {
        if ui.add(egui::Button::new(RichText::new("\u{2190} Voltar").size(13.0).color(Color32::WHITE))
            .fill(Color32::from_rgb(20,70,55)).stroke(Stroke::new(1.5,ACCENT))).clicked() { *go_main = true; }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new("Outros Filtros").size(14.0).color(ACCENT).strong());
        });
    });
    ui.add_space(6.0); ui.separator(); ui.add_space(6.0);
    for row in 0..5usize {
        ui.columns(2, |cols| {
            for col in 0..2 {
                let idx = IDX_OUTROS_START + row + col*5;
                if idx >= app.filters.len() { return; }
                let f = &app.filters[idx]; let active = app.current==idx && !app.inverted_grayscale;
                let w = cols[col].available_width();
                if filter_btn(&mut cols[col], &format!("{}  {}", f.emoji, f.name), active, f.color, w) { *to_apply = Some(idx); }
            }
        });
        ui.add_space(3.0);
    }
    if !app.inverted_grayscale { draw_intensity_slider(ui, app.intensity, new_intensity); }
    draw_toast_and_bottom(ui, app, minimize, quit);
}

// ─── Página Agenda ───────────────────────────────────────────────────────────

fn draw_agenda(ui: &mut egui::Ui, app: &mut EyeZenApp, go_main: &mut bool) {
    ui.set_min_width(420.0);

    // Cabeçalho
    ui.horizontal(|ui| {
        if ui.add(egui::Button::new(RichText::new("\u{2190} Voltar").size(13.0).color(Color32::WHITE))
            .fill(Color32::from_rgb(20,70,55)).stroke(Stroke::new(1.5,ACCENT))).clicked() {
            app.persist(); *go_main = true;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new("\u{23F0}  Agenda").size(14.0).color(Color32::from_rgb(255,220,100)).strong());
        });
    });
    ui.add_space(6.0); ui.separator(); ui.add_space(6.0);

    // Toggle ativo/inativo
    ui.horizontal(|ui| {
        ui.label(RichText::new("Agendamento automático").size(13.0).color(Color32::from_gray(210)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = if app.schedule_enabled { RichText::new("ATIVO").size(12.0).color(ACCENT) }
                        else                     { RichText::new("inativo").size(12.0).color(Color32::from_gray(130)) };
            if ui.add(egui::Button::new(label)
                .fill(if app.schedule_enabled {Color32::from_rgb(0,60,40)} else {BTN_IDLE})
                .stroke(if app.schedule_enabled {Stroke::new(1.5,ACCENT)} else {idle_stroke()}))
                .clicked() { app.schedule_enabled = !app.schedule_enabled; }
        });
    });
    ui.add_space(4.0);
    ui.label(RichText::new("Define qual filtro aplicar em cada horário do dia.")
        .size(10.5).color(Color32::from_gray(120)));
    ui.add_space(8.0); ui.separator(); ui.add_space(6.0);

    // Lista de slots
    let mut to_delete: Option<usize> = None;
    egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
        for (i, slot) in app.schedule_slots.iter_mut().enumerate() {
            egui::Frame::none()
                .fill(Color32::from_rgb(18,30,24))
                .rounding(6.0)
                .inner_margin(egui::Margin::symmetric(6.0, 4.0))
                .show(ui, |ui| {
                    // Linha 1: intervalo [HH:MM → HH:MM]
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("De").size(11.0).color(Color32::from_gray(140)));
                        let mut sh = slot.start_h as i32;
                        if ui.add(egui::DragValue::new(&mut sh).range(0..=23).speed(0.15).suffix("h")).changed() {
                            slot.start_h = sh as u8;
                        }
                        let mut sm = slot.start_m as i32;
                        if ui.add(egui::DragValue::new(&mut sm).range(0..=59).speed(0.5).suffix("m")).changed() {
                            slot.start_m = sm as u8;
                        }
                        ui.label(RichText::new("\u{2192}").size(13.0).color(Color32::from_gray(160)));
                        let mut eh = slot.end_h as i32;
                        if ui.add(egui::DragValue::new(&mut eh).range(0..=23).speed(0.15).suffix("h")).changed() {
                            slot.end_h = eh as u8;
                        }
                        let mut em = slot.end_m as i32;
                        if ui.add(egui::DragValue::new(&mut em).range(0..=59).speed(0.5).suffix("m")).changed() {
                            slot.end_m = em as u8;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(egui::Button::new(RichText::new("\u{274C}").size(10.0))
                                .fill(Color32::from_rgb(70,20,20)).stroke(idle_stroke()).min_size(Vec2::new(20.0,20.0)))
                                .clicked() { to_delete = Some(i); }
                        });
                    });
                    // Linha 2: seleção de filtro
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Filtro:").size(11.0).color(Color32::from_gray(140)));
                        let fname = app.filters.get(slot.filter_idx).map(|f| f.name).unwrap_or("?");
                        let selected_text = if slot.filter_idx == IDX_DESATIVAR {
                            RichText::new(format!("\u{23FA} {}", fname)).size(12.0).color(Color32::from_rgb(255,220,60)).strong()
                        } else {
                            RichText::new(fname).size(12.0).color(Color32::from_gray(220))
                        };
                        egui::ComboBox::from_id_salt(format!("sched_{}", i))
                            .selected_text(selected_text)
                            .width(150.0)
                            .show_ui(ui, |ui| {
                                for (fi, f) in app.filters.iter().enumerate() {
                                    let label = if fi == IDX_DESATIVAR {
                                        RichText::new(format!("\u{23FA} {}", f.name)).color(Color32::from_rgb(255,220,60)).strong()
                                    } else {
                                        RichText::new(f.name).color(Color32::from_gray(220))
                                    };
                                    ui.selectable_value(&mut slot.filter_idx, fi, label);
                                }
                            });
                    });
                });
            ui.add_space(3.0);
        }
    });
    if let Some(i) = to_delete { app.schedule_slots.remove(i); }

    // Botão adicionar
    ui.add_space(6.0);
    let w = ui.available_width();
    if ui.add(egui::Button::new(RichText::new("+ Adicionar Intervalo").size(13.0).color(ACCENT))
        .fill(Color32::from_rgb(0,45,30)).stroke(Stroke::new(1.5,ACCENT))
        .min_size(Vec2::new(w,32.0))).clicked() {
        app.schedule_slots.push(ScheduleSlot { start_h: 20, start_m: 0, end_h: 23, end_m: 59, filter_idx: 0 });
    }

    // Dica
    ui.add_space(6.0); ui.separator(); ui.add_space(4.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new("Fora do intervalo configurado, o filtro é desativado automaticamente.")
            .size(10.0).color(Color32::from_gray(100)));
    });
}

// ─── Página Apps ─────────────────────────────────────────────────────────────

fn draw_apps(ui: &mut egui::Ui, app: &mut EyeZenApp, go_main: &mut bool) {
    ui.set_min_width(420.0);
    // Cabeçalho
    ui.horizontal(|ui| {
        if ui.add(egui::Button::new(RichText::new("\u{2190} Voltar").size(13.0).color(Color32::WHITE))
            .fill(Color32::from_rgb(20,70,55)).stroke(Stroke::new(1.5,ACCENT))).clicked() {
            app.persist(); *go_main = true;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new("\u{1F5A5}  Filtro por App").size(14.0).color(Color32::from_rgb(100,200,255)).strong());
        });
    });
    ui.add_space(6.0); ui.separator(); ui.add_space(6.0);

    // Toggle ativo/inativo
    ui.horizontal(|ui| {
        ui.label(RichText::new("Troca automática por app em foco").size(13.0).color(Color32::from_gray(210)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = if app.app_rules_enabled { RichText::new("ATIVO").size(12.0).color(Color32::from_rgb(100,200,255)) }
                        else                     { RichText::new("inativo").size(12.0).color(Color32::from_gray(130)) };
            if ui.add(egui::Button::new(label)
                .fill(if app.app_rules_enabled {Color32::from_rgb(0,50,80)} else {BTN_IDLE})
                .stroke(if app.app_rules_enabled {Stroke::new(1.5,Color32::from_rgb(80,170,255))} else {idle_stroke()}))
                .clicked() { app.app_rules_enabled = !app.app_rules_enabled; app.persist(); }
        });
    });
    ui.add_space(8.0); ui.separator(); ui.add_space(6.0);

    // Apps predefinidos
    ui.label(RichText::new("Adicionar preset:").size(11.0).color(Color32::from_gray(140)));
    ui.add_space(4.0);
    egui::ScrollArea::horizontal().id_salt("presets_scroll").max_height(36.0).show(ui, |ui| {
        ui.horizontal(|ui| {
            for (display, proc) in PRESET_APPS {
                let already = app.app_rules.iter().any(|r| r.process_name == *proc);
                let color = if already { Color32::from_gray(100) } else { Color32::from_gray(210) };
                let fill  = if already { Color32::from_rgb(30,30,40) } else { BTN_IDLE };
                if ui.add(egui::Button::new(RichText::new(*display).size(11.5).color(color))
                    .fill(fill).stroke(idle_stroke()))
                    .clicked() && !already {
                    app.app_rules.push(AppRule { process_name: proc.to_string(), filter_idx: IDX_DESATIVAR });
                    app.persist();
                }
            }
        });
    });
    ui.add_space(6.0);

    // Input manual
    ui.horizontal(|ui| {
        ui.label(RichText::new("Processo:").size(11.0).color(Color32::from_gray(140)));
        let resp = ui.add(egui::TextEdit::singleline(&mut app.apps_page_input)
            .hint_text("ex: notepad.exe")
            .desired_width(160.0));
        let submit = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui.add(egui::Button::new(RichText::new("+ Adicionar").size(11.5).color(ACCENT))
            .fill(Color32::from_rgb(0,45,30)).stroke(Stroke::new(1.0,ACCENT))).clicked() || submit {
            let name = app.apps_page_input.trim().to_lowercase();
            if !name.is_empty() && !app.app_rules.iter().any(|r| r.process_name == name) {
                app.app_rules.push(AppRule { process_name: name, filter_idx: IDX_DESATIVAR });
                app.apps_page_input.clear();
                app.persist();
            }
        }
    });
    ui.add_space(6.0); ui.separator(); ui.add_space(4.0);

    // Pré-extrai nomes para evitar conflito de borrow com iter_mut
    let filter_names: Vec<&'static str> = app.filters.iter().map(|f| f.name).collect();
    let filter_emojis: Vec<&'static str> = app.filters.iter().map(|f| f.emoji).collect();

    // Lista de regras
    let mut to_delete: Option<usize> = None;
    let mut need_persist = false;
    egui::ScrollArea::vertical().max_height(240.0).id_salt("apps_list").show(ui, |ui| {
        for (i, rule) in app.app_rules.iter_mut().enumerate() {
            egui::Frame::none()
                .fill(Color32::from_rgb(18,30,24))
                .rounding(6.0)
                .inner_margin(egui::Margin::symmetric(8.0, 5.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&rule.process_name).size(12.0).color(Color32::from_gray(220)).monospace());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(egui::Button::new(RichText::new("\u{274C}").size(10.0))
                                .fill(Color32::from_rgb(70,20,20)).stroke(idle_stroke())
                                .min_size(Vec2::new(20.0, 20.0))).clicked() { to_delete = Some(i); }
                            let fname = filter_names.get(rule.filter_idx).copied().unwrap_or("?");
                            let sel_text = if rule.filter_idx == IDX_DESATIVAR {
                                RichText::new(format!("\u{23FA} {}", fname)).color(Color32::from_rgb(255,220,60)).strong()
                            } else {
                                RichText::new(fname).color(Color32::from_gray(220))
                            };
                            egui::ComboBox::from_id_salt(format!("app_rule_{}", i))
                                .selected_text(sel_text)
                                .width(145.0)
                                .show_ui(ui, |ui| {
                                    for (fi, fname) in filter_names.iter().enumerate() {
                                        let emoji = filter_emojis.get(fi).copied().unwrap_or("");
                                        let label = if fi == IDX_DESATIVAR {
                                            RichText::new(format!("\u{23FA} {}", fname)).color(Color32::from_rgb(255,220,60)).strong()
                                        } else {
                                            RichText::new(format!("{}  {}", emoji, fname)).color(Color32::from_gray(220))
                                        };
                                        if ui.selectable_value(&mut rule.filter_idx, fi, label).changed() {
                                            need_persist = true;
                                        }
                                    }
                                });
                        });
                    });
                });
            ui.add_space(2.0);
        }
    });
    if let Some(i) = to_delete { app.app_rules.remove(i); need_persist = true; }
    if need_persist { app.persist(); }

    if app.app_rules.is_empty() {
        ui.add_space(8.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("Nenhuma regra configurada ainda.").size(11.0).color(Color32::from_gray(100)));
        });
    }
}

// ─── Toast + rodapé ──────────────────────────────────────────────────────────

fn draw_toast_and_bottom(ui: &mut egui::Ui, app: &EyeZenApp, minimize: &mut bool, quit: &mut bool) {
    ui.add_space(6.0); ui.separator(); ui.add_space(4.0);
    if let Some((msg, _)) = &app.toast {
        ui.vertical_centered(|ui| {
            egui::Frame::none().fill(Color32::from_rgb(0,80,45)).rounding(6.0)
                .inner_margin(egui::Margin::symmetric(10.0,4.0)).show(ui, |ui| {
                    ui.label(RichText::new(msg).size(12.0).color(Color32::from_rgb(180,255,200)));
                });
        });
        ui.add_space(4.0);
    }
    ui.columns(2, |cols| {
        let w0 = cols[0].available_width();
        if cols[0].add(egui::Button::new(RichText::new("Minimizar").size(12.0).color(Color32::from_gray(210)))
            .fill(Color32::from_rgb(28,68,135)).min_size(Vec2::new(w0,28.0))).clicked() { *minimize = true; }
        let w1 = cols[1].available_width();
        if cols[1].add(egui::Button::new(RichText::new("Sair").size(12.0).color(Color32::WHITE))
            .fill(Color32::from_rgb(145,22,22)).min_size(Vec2::new(w1,28.0))).clicked() { *quit = true; }
    });
}

// ─── Main ────────────────────────────────────────────────────────────────────

fn main() {
    mag_api::init();
    let (tx, rx) = channel::<AppEvent>();

    let hotkey_mgr = GlobalHotKeyManager::new().expect("hotkey manager");
    let mods = Modifiers::CONTROL | Modifiers::ALT;
    let filter_hk_defs: &[(Code, usize)] = &[
        (Code::Digit1,0),(Code::Digit2,1),(Code::Digit3,2),
        (Code::Digit4,3),(Code::Digit5,4),(Code::Digit6,5),(Code::Digit0,IDX_DESATIVAR),
    ];
    let mut filter_map: HashMap<u32,usize> = HashMap::new();
    for &(code, idx) in filter_hk_defs {
        let hk = HotKey::new(Some(mods), code);
        filter_map.insert(hk.id(), idx); let _ = hotkey_mgr.register(hk);
    }
    let hk9 = HotKey::new(Some(mods), Code::Digit9); let id9 = hk9.id(); let _ = hotkey_mgr.register(hk9);

    let tx2 = tx.clone();
    std::thread::spawn(move || {
        let hk_rx = GlobalHotKeyEvent::receiver().clone();
        loop {
            if let Ok(ev) = hk_rx.try_recv() {
                if ev.id == id9 { let _ = tx2.send(AppEvent::ToggleInvertedGrayscale); }
                else if let Some(&idx) = filter_map.get(&ev.id) { let _ = tx2.send(AppEvent::ApplyFilter(idx)); }
            }
            std::thread::sleep(Duration::from_millis(30));
        }
    });

    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("EyeZen – Filtros de Conforto Visual")
            .with_inner_size([440.0, 540.0])
            .with_resizable(false)
            .with_visible(false),
        ..Default::default()
    };

    eframe::run_native("EyeZen", options,
        Box::new(|cc| Ok(Box::new(EyeZenApp::new(cc, rx)) as Box<dyn App>))).expect("eframe");

    drop(hotkey_mgr);
}
