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
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tray_icon::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder,
};

const LOGO_RGBA: &[u8] = include_bytes!("../assets/logo_96.rgba");

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
    /// Rotação de matiz contínua, recalculada a cada quadro — Psicodélico
    HueCycle,
    /// Matriz de cor aleatória sorteada uma vez, ao selecionar — Tubbies
    Chaotic,
    /// Alterna claro/escuro numa frequência fixa — Sandman. Só pode ser
    /// ativado através do fluxo de confirmação em `draw_beta` (nunca direto
    /// por agenda ou regra de app — ver os `continue` em IDX_BETA_START).
    Strobe,
    /// Overlay transparente com ruído estático — Chiado (efeito TV antiga).
    /// Não mexe na cor da tela, sobrepõe uma segunda janela por cima.
    Noise,
    /// Curva de tom tipo ToneMap + overlay transparente de grão de papel,
    /// gerado uma vez (estático, não redesenha a cada quadro como o Chiado).
    PaperGrain { r: f64, g: f64, b: f64, scale: f64, lift: f64 },
    /// Overlay escuro com um "buraco" claro seguindo o cursor — Lanterna.
    Spotlight,
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
        // idx 17-23 = Versão de Teste (experimental)
        FilterDef { name: "Psicodelico", emoji: "\u{1F300}", kind: FilterKind::HueCycle, color: Color32::from_rgb(180, 0, 210) },
        FilterDef { name: "Tubbies",     emoji: "\u{1F9F8}", kind: FilterKind::Chaotic,  color: Color32::from_rgb(255, 130, 0) },
        FilterDef { name: "Sandman",     emoji: "\u{1F4A4}", kind: FilterKind::Strobe,   color: Color32::from_rgb(210, 20, 60) },
        FilterDef { name: "Chiado",      emoji: "\u{1F4FC}", kind: FilterKind::Noise,    color: Color32::from_rgb(90, 90, 100) },
        FilterDef { name: "Papel", emoji: "\u{1F4C4}", kind: FilterKind::PaperGrain {
            r: 1.00, g: 0.97, b: 0.90, scale: 0.88, lift: 0.10,
        }, color: Color32::from_rgb(190, 175, 145) },
        // Ultradark: escala bem abaixo do piso de ~50% que o gamma ramp aceita,
        // mas ainda enxergável (15%, não 2% — isso ficou escuro demais na prática).
        // `compute_matrix_at` sempre garante um piso mínimo de brilho por canal —
        // nunca chega a preto absoluto, por mais que suba a intensidade.
        FilterDef { name: "Ultradark", emoji: "\u{1F311}", kind: FilterKind::ColorMatrix { matrix: [
            0.15,0.,0.,0.,0., 0.,0.15,0.,0.,0., 0.,0.,0.15,0.,0., 0.,0.,0.,1.,0., 0.,0.,0.,0.,1.,
        ]}, color: Color32::from_rgb(25, 25, 32) },
        FilterDef { name: "Lanterna", emoji: "\u{1F526}", kind: FilterKind::Spotlight, color: Color32::from_rgb(40, 40, 30) },
    ]
}

const IDX_DESATIVAR:    usize = 6;
const IDX_OUTROS_START: usize = 7;
const IDX_BETA_START:   usize = 17;
const IDX_SANDMAN:      usize = 19;
const TRANS_DURATION:   Duration = Duration::from_millis(1400);
const MIN_WIN_H: f32 = 420.0;
const MAX_WIN_H: f32 = 760.0;

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
        FilterKind::ToneMap { r, g, b, scale, lift } | FilterKind::PaperGrain { r, g, b, scale, lift } => {
            let (ri,gi,bi) = (1.0+(r-1.0)*intensity, 1.0+(g-1.0)*intensity, 1.0+(b-1.0)*intensity);
            let (si, li)   = (1.0+(scale-1.0)*intensity, lift*intensity);
            for i in 0..256usize {
                let t = (i as f64/255.0*si+li).clamp(0.0,1.0);
                ramp[i]     = (t*ri*65535.0).clamp(0.0,65535.0) as u16;
                ramp[256+i] = (t*gi*65535.0).clamp(0.0,65535.0) as u16;
                ramp[512+i] = (t*bi*65535.0).clamp(0.0,65535.0) as u16;
            }
        }
        FilterKind::ColorMatrix { .. } | FilterKind::HueCycle | FilterKind::Chaotic
            | FilterKind::Strobe | FilterKind::Noise | FilterKind::Spotlight => return neutral_ramp(),
    }
    ramp
}

fn compute_matrix_at(matrix: &[f32; 25], intensity: f64) -> [f32; 25] {
    let id: [f32; 25] = [1.,0.,0.,0.,0., 0.,1.,0.,0.,0., 0.,0.,1.,0.,0., 0.,0.,0.,1.,0., 0.,0.,0.,0.,1.];
    let mut m = [0f32; 25];
    for i in 0..25 { m[i] = id[i] + (matrix[i]-id[i]) * intensity as f32; }
    // Piso de segurança: o ganho R/G/B nunca fica negativo (o que inverteria o
    // canal) nem chega a zero absoluto — nenhum filtro, nem em 150% de
    // intensidade, apaga a tela por completo (ver Ultradark).
    for &i in &[0usize, 6, 12] { m[i] = m[i].max(0.05); }
    m
}

// ─── Filtros experimentais (Psicodélico / Tubbies / Sandman) ─────────────────
//
// A Magnification API só aceita uma matriz de cor 5×5 aplicada uniformemente
// à tela inteira — não há acesso à posição do pixel, então não dá pra desenhar
// padrões espaciais (fractais de verdade). O que dá pra fazer: Psicodélico
// recalcula uma rotação de matiz a cada quadro; Tubbies sorteia uma mistura
// caótica de canais uma única vez, ao selecionar; Sandman alterna entre a
// tela normal e um flash escuro numa frequência fixa.

/// Matriz de rotação de matiz (preserva luminância), ângulo em radianos.
fn hue_rotate_matrix(theta: f32) -> [f32; 25] {
    let (s, c) = theta.sin_cos();
    let a00 = 0.213 + c*0.787 - s*0.213;
    let a01 = 0.715 - c*0.715 - s*0.715;
    let a02 = 0.072 - c*0.072 + s*0.928;
    let a10 = 0.213 - c*0.213 + s*0.143;
    let a11 = 0.715 + c*0.285 + s*0.140;
    let a12 = 0.072 - c*0.072 - s*0.283;
    let a20 = 0.213 - c*0.213 - s*0.787;
    let a21 = 0.715 - c*0.715 + s*0.715;
    let a22 = 0.072 + c*0.928 + s*0.072;
    let mut m = [0f32; 25];
    m[0*5+0]=a00; m[0*5+1]=a10; m[0*5+2]=a20;
    m[1*5+0]=a01; m[1*5+1]=a11; m[1*5+2]=a21;
    m[2*5+0]=a02; m[2*5+1]=a12; m[2*5+2]=a22;
    m[3*5+3]=1.0; m[4*5+4]=1.0;
    m
}

fn xorshift(mut x: u64) -> u64 {
    if x == 0 { x = 0x9E3779B97F4A7C15; }
    x ^= x << 13; x ^= x >> 7; x ^= x << 17; x
}

fn next_unit(seed: &mut u64) -> f32 {
    *seed = xorshift(*seed);
    ((*seed >> 40) as f32) / (16777216.0) // 24 bits -> [0,1)
}

/// Mistura aleatória e pesada dos 3 canais entre si — cada saída (R/G/B) vira
/// uma combinação caótica das 3 entradas, com coeficientes que podem passar de
/// 1.0 ou ser negativos (estoura/inverte cores de propósito). Sorteada uma vez
/// por seleção, não a cada quadro.
fn chaotic_matrix(seed: &mut u64) -> [f32; 25] {
    let mut m = [0f32; 25];
    for out in 0..3 {
        for inp in 0..3 { m[inp*5 + out] = -1.3 + next_unit(seed) * 2.9; }
        m[4*5 + out] = -0.12 + next_unit(seed) * 0.24;
    }
    m[3*5+3] = 1.0; m[4*5+4] = 1.0;
    m
}

/// Fundo do ciclo do estroboscópio (Sandman). `peak` 0..1 controla o quão escuro
/// fica: 0 é só uma leve escurecida, 1 é preto quase total.
fn strobe_dark_matrix(peak: f32) -> [f32; 25] {
    let k = (1.0 - peak.clamp(0.0, 1.0) * 0.95).max(0.05);
    [k,0.,0.,0.,0., 0.,k,0.,0.,0., 0.,0.,k,0.,0., 0.,0.,0.,1.,0., 0.,0.,0.,0.,1.]
}

fn lerp_matrix(a: &[f32; 25], b: &[f32; 25], t: f32) -> [f32; 25] {
    let mut m = [0f32; 25];
    for i in 0..25 { m[i] = a[i] + (b[i]-a[i]) * t; }
    m
}

/// Curva de brilho do Sandman ao longo de um ciclo (fase 0..1). `smoothness` 0..1:
/// perto de 0 é um flash rápido e estreito, perto de 1 é uma respiração suave
/// (seno puro) entre claro e escuro — sem corte abrupto.
fn strobe_pulse(phase: f32, smoothness: f32) -> f32 {
    let sine = 0.5 - 0.5 * (phase * std::f32::consts::TAU).cos(); // 0..1, suave
    let exponent = 1.0 + (1.0 - smoothness.clamp(0.0, 1.0)) * 24.0;
    sine.max(0.0).powf(exponent)
}

// ─── Overlay (Chiado / Papel) ─────────────────────────────────────────────────
//
// Nem gamma ramp nem a Magnification API têm noção de posição de pixel, então
// pra desenhar ruído/grão de verdade a única saída é uma segunda janela por
// cima da tela. Primeira tentativa usou o sistema de viewport do egui
// (`show_viewport_immediate`/`_deferred`) — falhou de duas formas diferentes:
// o clique não atravessava (tela travada, sem dar pra interagir com nada), e
// depois de corrigir isso, a transparência de verdade nunca funcionou (a
// janela aparecia sólida/branca, cobrindo a tela com uma imagem parada) —
// o backend OpenGL (glow) do eframe não compõe alpha de verdade com o DWM
// pra viewports secundárias nesse setup.
//
// Solução: abandonar o egui pra essa parte e usar uma janela nativa do Windows
// (`WS_EX_LAYERED`) atualizada via `UpdateLayeredWindow` — é o mecanismo GDI
// clássico de transparência real por pixel, independente de OpenGL/DWM, usado
// por overlays de verdade (ex.: launchers, HUDs). Roda numa thread própria,
// dona da janela e do laço de mensagens do Windows.

const NOISE_W: usize = 320;
const NOISE_H: usize = 180;
const PAPER_MAX_DIM: i32 = 1600;

fn gen_noise_rgba(seed: &mut u64) -> Vec<u8> {
    let mut buf = vec![0u8; NOISE_W * NOISE_H * 4];
    for px in buf.chunks_exact_mut(4) {
        *seed = xorshift(*seed);
        let v = (*seed >> 24) as u8;
        px[0] = v; px[1] = v; px[2] = v; px[3] = 55;
    }
    buf
}

fn gen_paper_grain_rgba(w: usize, h: usize, seed: &mut u64) -> Vec<u8> {
    let mut buf = vec![0u8; w * h * 4];
    for px in buf.chunks_exact_mut(4) {
        *seed = xorshift(*seed);
        let v = (*seed >> 24) as u8;
        px[0] = 235; px[1] = 222; px[2] = 195;
        px[3] = (v as u16 * 22 / 255) as u8;
    }
    buf
}

/// Amplia por vizinho-mais-próximo de `sw`×`sh` pra `dw`×`dh` (RGBA straight-alpha),
/// e já converte pra BGRA com alpha pré-multiplicado — formato que `UpdateLayeredWindow`
/// exige (`ULW_ALPHA`).
fn rgba_to_bgra_premul_scaled(src: &[u8], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<u8> {
    let mut out = vec![0u8; dw * dh * 4];
    for y in 0..dh {
        let sy = (y * sh / dh).min(sh.saturating_sub(1));
        for x in 0..dw {
            let sx = (x * sw / dw).min(sw.saturating_sub(1));
            let si = (sy * sw + sx) * 4;
            let di = (y * dw + x) * 4;
            let (r, g, b, a) = (src[si] as u16, src[si+1] as u16, src[si+2] as u16, src[si+3] as u16);
            out[di]   = (b * a / 255) as u8;
            out[di+1] = (g * a / 255) as u8;
            out[di+2] = (r * a / 255) as u8;
            out[di+3] = a as u8;
        }
    }
    out
}

#[cfg(windows)]
fn virtual_screen_rect() -> (i32, i32, i32, i32) {
    use winapi::um::winuser::{GetSystemMetrics, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN};
    unsafe {
        (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN),
         GetSystemMetrics(SM_CXVIRTUALSCREEN), GetSystemMetrics(SM_CYVIRTUALSCREEN))
    }
}
#[cfg(not(windows))]
fn virtual_screen_rect() -> (i32, i32, i32, i32) { (0, 0, 1920, 1080) }

#[cfg(windows)]
fn cursor_pos() -> (i32, i32) {
    use winapi::um::winuser::GetCursorPos;
    use winapi::shared::windef::POINT;
    unsafe {
        let mut p: POINT = std::mem::zeroed();
        if GetCursorPos(&mut p) != 0 { (p.x, p.y) } else { (0, 0) }
    }
}
#[cfg(not(windows))]
fn cursor_pos() -> (i32, i32) { (0, 0) }

/// Máscara escura com um "buraco" claro ao redor do cursor (borda suave entre
/// `inner` e `outer`, em pixels). BGRA pré-multiplicado — como a cor é sempre
/// preta, o pré-multiplicado é sempre 0 nos três canais, só o alpha varia.
fn gen_spotlight_bgra(cx: f32, cy: f32, w: i32, h: i32, inner: f32, outer: f32) -> Vec<u8> {
    let (w, h) = (w.max(1) as usize, h.max(1) as usize);
    let mut buf = vec![0u8; w * h * 4];
    const DARK_ALPHA: f32 = 210.0;
    for y in 0..h {
        let dy = y as f32 - cy;
        for x in 0..w {
            let dx = x as f32 - cx;
            let dist = (dx*dx + dy*dy).sqrt();
            let t = ((dist - inner) / (outer - inner).max(1.0)).clamp(0.0, 1.0);
            buf[(y*w+x)*4 + 3] = (t * DARK_ALPHA) as u8;
        }
    }
    buf
}

enum OverlayMsg {
    /// Mostra (ou atualiza) a janela com este retângulo + bitmap BGRA pré-multiplicado.
    Show { x: i32, y: i32, w: i32, h: i32, bgra: Vec<u8> },
    Hide,
    Quit,
}

struct OverlayHandle {
    tx: std::sync::mpsc::Sender<OverlayMsg>,
}

impl OverlayHandle {
    fn show(&self, x: i32, y: i32, w: i32, h: i32, bgra: Vec<u8>) {
        let _ = self.tx.send(OverlayMsg::Show { x, y, w, h, bgra });
    }
    fn hide(&self) { let _ = self.tx.send(OverlayMsg::Hide); }
    fn quit(&self) { let _ = self.tx.send(OverlayMsg::Quit); }
}

#[cfg(windows)]
mod overlay_win32 {
    use super::OverlayMsg;
    use std::sync::mpsc::Receiver;
    use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};
    use winapi::shared::windef::{HWND, POINT, SIZE};
    use winapi::um::libloaderapi::GetModuleHandleW;
    use winapi::um::wingdi::{
        CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject,
        BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, AC_SRC_OVER, AC_SRC_ALPHA, BLENDFUNCTION,
    };
    use winapi::um::winuser::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetDC, PeekMessageW,
        RegisterClassW, ReleaseDC, SetWindowPos, ShowWindow, TranslateMessage, UpdateLayeredWindow,
        CW_USEDEFAULT, HWND_TOPMOST, MSG, PM_REMOVE, SWP_NOACTIVATE, SW_HIDE, ULW_ALPHA, WNDCLASSW,
        WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
    };

    unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    pub fn run(rx: Receiver<OverlayMsg>) {
        unsafe {
            let class_name: Vec<u16> = "EyeZenOverlayClass\0".encode_utf16().collect();
            let hinstance = GetModuleHandleW(std::ptr::null());
            let wc = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinstance,
                hIcon: std::ptr::null_mut(),
                hCursor: std::ptr::null_mut(),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: class_name.as_ptr(),
            };
            RegisterClassW(&wc);

            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class_name.as_ptr(),
                std::ptr::null(),
                WS_POPUP,
                CW_USEDEFAULT, CW_USEDEFAULT, 1, 1,
                std::ptr::null_mut(), std::ptr::null_mut(), hinstance, std::ptr::null_mut(),
            );
            if hwnd.is_null() { return; }

            loop {
                let mut msg: MSG = std::mem::zeroed();
                while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                match rx.recv_timeout(std::time::Duration::from_millis(30)) {
                    Ok(OverlayMsg::Show { x, y, w, h, bgra }) => blit(hwnd, x, y, w, h, &bgra),
                    Ok(OverlayMsg::Hide) => { ShowWindow(hwnd, SW_HIDE); }
                    Ok(OverlayMsg::Quit) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        DestroyWindow(hwnd);
                        break;
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
        }
    }

    unsafe fn blit(hwnd: HWND, x: i32, y: i32, w: i32, h: i32, bgra: &[u8]) {
        if w <= 0 || h <= 0 || bgra.len() < (w as usize * h as usize * 4) { return; }

        SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE);

        let hdc_screen = GetDC(std::ptr::null_mut());
        let hdc_mem = CreateCompatibleDC(hdc_screen);

        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h, // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: 0, biXPelsPerMeter: 0, biYPelsPerMeter: 0, biClrUsed: 0, biClrImportant: 0,
        };

        let mut bits_ptr: *mut winapi::ctypes::c_void = std::ptr::null_mut();
        let hbitmap = CreateDIBSection(hdc_mem, &bmi, DIB_RGB_COLORS, &mut bits_ptr, std::ptr::null_mut(), 0);
        if !hbitmap.is_null() && !bits_ptr.is_null() {
            std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits_ptr as *mut u8, bgra.len());
            let old = SelectObject(hdc_mem, hbitmap as _);

            let mut size = SIZE { cx: w, cy: h };
            let mut src_pt = POINT { x: 0, y: 0 };
            let mut blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA };
            UpdateLayeredWindow(hwnd, hdc_screen, std::ptr::null_mut(), &mut size, hdc_mem, &mut src_pt, 0, &mut blend, ULW_ALPHA);
            ShowWindow(hwnd, 8 /*SW_SHOWNA*/);

            SelectObject(hdc_mem, old);
            DeleteObject(hbitmap as _);
        }
        DeleteDC(hdc_mem);
        ReleaseDC(std::ptr::null_mut(), hdc_screen);
    }
}
#[cfg(not(windows))]
mod overlay_win32 {
    use super::OverlayMsg;
    use std::sync::mpsc::Receiver;
    pub fn run(_rx: Receiver<OverlayMsg>) {}
}

fn spawn_overlay() -> OverlayHandle {
    let (tx, rx) = std::sync::mpsc::channel::<OverlayMsg>();
    std::thread::spawn(move || overlay_win32::run(rx));
    OverlayHandle { tx }
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
enum Page { Main, Outros, Agenda, Apps, Beta }

// ─── Sandman (estroboscópio) ──────────────────────────────────────────────────
//
// Nunca ativa direto. Selecionar mostra um aviso; só depois de SANDMAN_WARN_SECS
// segundos o botão de confirmar libera. Fica ativo até o usuário parar — clicando
// em "Parar agora", em qualquer outro filtro, ou Ctrl+Alt+0. Frequência, suavidade
// e o quão escuro fica o fundo (pico) são ajustáveis em tempo real, inclusive já
// ativo. Frequência baixa (ex.: 1 ciclo a cada 5-7s) funciona como lembrete de
// piscar — nada a ver com estroboscópio de verdade; o risco cresce junto com
// frequência + pico + falta de suavidade, não é fixo em todo o intervalo.
const SANDMAN_WARN_SECS:     u64 = 3;
const SANDMAN_FREQ_MIN:      f64 = 0.1;
const SANDMAN_FREQ_MAX:      f64 = 14.0;
const SANDMAN_FREQ_DEFAULT:  f64 = 5.0;
const SANDMAN_SMOOTH_DEFAULT: f64 = 0.6;
const SANDMAN_PEAK_DEFAULT:  f64 = 1.0;

#[derive(Clone, Copy, PartialEq)]
enum SandmanState { Idle, Warning(Instant), Active(Instant) }

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
    anim_start:           Instant,
    chaotic_matrix:       [f32; 25],
    rng_state:            u64,
    sandman_state:        SandmanState,
    sandman_freq_hz:      f64,
    sandman_smoothness:   f64,
    sandman_peak:         f64,
    overlay:              OverlayHandle,
    paper_bgra:           Option<(Vec<u8>, i32, i32)>,
    overlay_last_send:       Instant,
    spotlight_radius:     f64,
    last_spotlight_pos:   (i32, i32),
    win_h:                f32,
    page:                 Page,
    logo_texture:         egui::TextureHandle,
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

        let logo_texture = cc.egui_ctx.load_texture(
            "logo",
            egui::ColorImage::from_rgba_unmultiplied([96, 96], LOGO_RGBA),
            egui::TextureOptions::LINEAR,
        );

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
        // Filtros da "Versão de Teste" (idx >= IDX_BETA_START) ficam de fora do menu da bandeja
        // de propósito — o Sandman só pode ser ativado pelo fluxo de confirmação na aba Beta.
        for (i, f) in filters[IDX_OUTROS_START..IDX_BETA_START].iter().enumerate() {
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
            anim_start: Instant::now(),
            chaotic_matrix: [1.,0.,0.,0.,0., 0.,1.,0.,0.,0., 0.,0.,1.,0.,0., 0.,0.,0.,1.,0., 0.,0.,0.,0.,1.],
            rng_state: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0x2545F4914F6CDD1D),
            sandman_state: SandmanState::Idle,
            sandman_freq_hz: SANDMAN_FREQ_DEFAULT,
            sandman_smoothness: SANDMAN_SMOOTH_DEFAULT,
            sandman_peak: SANDMAN_PEAK_DEFAULT,
            overlay: spawn_overlay(),
            paper_bgra: None,
            overlay_last_send: Instant::now(),
            spotlight_radius: 160.0,
            last_spotlight_pos: (i32::MIN, i32::MIN),
            win_h: 540.0,
            page: Page::Main,
            logo_texture,
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
        // Usado pelo Psicodélico (fase da rotação) e pelo timeout de segurança
        // do Chiado/Papel — marca "desde quando o filtro atual está selecionado".
        self.anim_start = Instant::now();
        if self.inverted_grayscale || self.uses_mag_filter { mag_api::reset(); }
        if self.inverted_grayscale {
            self.inverted_grayscale = false;
            self.tray_invert_item.set_checked(false);
            let _ = self._tray.set_icon(Some(make_tray_icon(false)));
        }
        match &self.filters[idx].kind {
            FilterKind::ColorMatrix { matrix } => {
                reset_gamma(); mag_api::apply_matrix(&compute_matrix_at(matrix, self.intensity));
                self.uses_mag_filter = true; self.transition = None; self.current_ramp = neutral_ramp();
            }
            FilterKind::HueCycle => {
                reset_gamma();
                self.uses_mag_filter = true; self.transition = None; self.current_ramp = neutral_ramp();
            }
            FilterKind::Chaotic => {
                self.rng_state = xorshift(self.rng_state);
                self.chaotic_matrix = chaotic_matrix(&mut self.rng_state);
                reset_gamma(); mag_api::apply_matrix(&compute_matrix_at(&self.chaotic_matrix, self.intensity));
                self.uses_mag_filter = true; self.transition = None; self.current_ramp = neutral_ramp();
            }
            FilterKind::Strobe => {
                reset_gamma();
                self.sandman_state = SandmanState::Active(Instant::now());
                self.uses_mag_filter = true; self.transition = None; self.current_ramp = neutral_ramp();
            }
            _ => {
                let target = compute_ramp(&self.filters[idx].kind, self.intensity);
                let from   = self.transition.as_ref().and_then(|t| t.sample()).unwrap_or(self.current_ramp);
                self.transition = Some(Transition { from, to: target, start: Instant::now() });
                if self.uses_mag_filter { reset_gamma(); }
                self.uses_mag_filter = false;
            }
        }
        // Sai do Chiado/Papel pra qualquer outro filtro: fecha a janela-overlay.
        if !matches!(self.filters[idx].kind, FilterKind::Noise | FilterKind::PaperGrain{..} | FilterKind::Spotlight) {
            self.overlay.hide();
            self.paper_bgra = None;
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
        match &self.filters[self.current].kind {
            FilterKind::ColorMatrix { matrix } => {
                mag_api::apply_matrix(&compute_matrix_at(matrix, self.intensity));
            }
            FilterKind::Chaotic => {
                mag_api::apply_matrix(&compute_matrix_at(&self.chaotic_matrix, self.intensity));
            }
            // A matriz é recalculada todo quadro em `update`, que já lê `self.intensity` atual.
            FilterKind::HueCycle | FilterKind::Strobe => {}
            _ => {
                let ramp = compute_ramp(&self.filters[self.current].kind, self.intensity);
                set_gamma_ramp(&ramp); self.current_ramp = ramp;
            }
        }
    }

    /// Chiado: gera ruído novo (baixa resolução) e manda pra thread do overlay
    /// a cada ~120ms — ela amplia e converte pra BGRA pré-multiplicado e faz o
    /// blit via `UpdateLayeredWindow` (transparência real, fora do egui).
    fn render_noise_overlay(&mut self, ctx: &egui::Context) {
        if self.overlay_last_send.elapsed() > Duration::from_millis(120) {
            self.overlay_last_send = Instant::now();
            let (vx, vy, vw, vh) = virtual_screen_rect();
            self.rng_state = xorshift(self.rng_state);
            let small = gen_noise_rgba(&mut self.rng_state);
            let bgra = rgba_to_bgra_premul_scaled(&small, NOISE_W, NOISE_H, vw as usize, vh as usize);
            self.overlay.show(vx, vy, vw, vh, bgra);
        }
        ctx.request_repaint_after(Duration::from_millis(60));
    }

    /// Papel: gera o grão uma vez (na primeira ativação), guarda o BGRA já
    /// convertido, e só reenvia esse mesmo buffer pra thread do overlay depois
    /// — sem regenerar nada, bem mais barato que o Chiado.
    fn render_paper_overlay(&mut self, ctx: &egui::Context) {
        let (vx, vy, vw, vh) = virtual_screen_rect();
        if self.paper_bgra.is_none() {
            let scale = (PAPER_MAX_DIM as f32 / (vw.max(vh) as f32)).min(1.0);
            let tw = ((vw as f32 * scale) as usize).max(64);
            let th = ((vh as f32 * scale) as usize).max(64);
            self.rng_state = xorshift(self.rng_state);
            let small = gen_paper_grain_rgba(tw, th, &mut self.rng_state);
            let bgra = rgba_to_bgra_premul_scaled(&small, tw, th, vw as usize, vh as usize);
            self.paper_bgra = Some((bgra, vw, vh));
            self.overlay.show(vx, vy, vw, vh, self.paper_bgra.as_ref().unwrap().0.clone());
        }
        // Textura estática — só reafirma a janela de vez em quando, sem reconverter nada.
        if self.overlay_last_send.elapsed() > Duration::from_secs(2) {
            self.overlay_last_send = Instant::now();
            if let Some((bgra, w, h)) = &self.paper_bgra {
                self.overlay.show(vx, vy, *w, *h, bgra.clone());
            }
        }
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    /// Lanterna: escurece a tela toda com um "buraco" claro seguindo o cursor.
    /// Só refaz a máscara se o mouse realmente se moveu desde o último envio —
    /// parado, custa zero; em movimento, atualiza a ~25fps.
    fn render_spotlight_overlay(&mut self, ctx: &egui::Context) {
        let (cx, cy) = cursor_pos();
        let moved = (cx - self.last_spotlight_pos.0).abs() > 1 || (cy - self.last_spotlight_pos.1).abs() > 1;
        if moved && self.overlay_last_send.elapsed() > Duration::from_millis(40) {
            self.overlay_last_send = Instant::now();
            self.last_spotlight_pos = (cx, cy);
            let (vx, vy, vw, vh) = virtual_screen_rect();
            let inner = self.spotlight_radius as f32;
            let outer = inner + 90.0;
            let bgra = gen_spotlight_bgra((cx - vx) as f32, (cy - vy) as f32, vw, vh, inner, outer);
            self.overlay.show(vx, vy, vw, vh, bgra);
        }
        ctx.request_repaint_after(Duration::from_millis(30));
    }

    fn toggle_inv_gray(&mut self, ctx: &egui::Context) {
        self.inverted_grayscale = !self.inverted_grayscale;
        if self.inverted_grayscale {
            self.transition = None; reset_gamma(); self.uses_mag_filter = false;
            self.overlay.hide(); self.paper_bgra = None;
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

fn shade(c: Color32, factor: f32) -> Color32 {
    Color32::from_rgb(
        (c.r() as f32 * factor).clamp(0.0, 255.0) as u8,
        (c.g() as f32 * factor).clamp(0.0, 255.0) as u8,
        (c.b() as f32 * factor).clamp(0.0, 255.0) as u8,
    )
}

/// Botão com relevo (sombra + bisel), fica "apertado" enquanto pressionado.
fn bevel_btn(ui: &mut egui::Ui, label: &str, base: Color32, width: f32, height: f32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let pressed = resp.is_pointer_button_down_on();
        let hovered = resp.hovered();
        let r = 7.0;

        if !pressed {
            painter.rect_filled(rect.translate(Vec2::new(0.0, 3.0)), r, Color32::from_rgba_premultiplied(0,0,0,90));
        }
        let btn_rect = rect.translate(Vec2::new(0.0, if pressed { 2.0 } else { 0.0 }));
        let fill = if pressed { shade(base, 0.75) } else if hovered { shade(base, 1.15) } else { base };
        painter.rect_filled(btn_rect, r, fill);

        if !pressed {
            painter.line_segment(
                [btn_rect.left_top()+Vec2::new(r,1.2), btn_rect.right_top()+Vec2::new(-r,1.2)],
                Stroke::new(1.2, Color32::from_rgba_premultiplied(255,255,255,70)),
            );
            painter.line_segment(
                [btn_rect.left_bottom()+Vec2::new(r,-1.5), btn_rect.right_bottom()+Vec2::new(-r,-1.5)],
                Stroke::new(1.5, Color32::from_rgba_premultiplied(0,0,0,110)),
            );
        }
        painter.rect_stroke(btn_rect, r, Stroke::new(1.0, Color32::from_rgba_premultiplied(0,0,0,140)));
        painter.text(btn_rect.center(), egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(12.5), Color32::WHITE);
    }
    resp.clicked()
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

        // ── Psicodélico — rotação de matiz recalculada a cada quadro ──
        if !self.inverted_grayscale {
            if let FilterKind::HueCycle = &self.filters[self.current].kind {
                let t = self.anim_start.elapsed().as_secs_f32();
                mag_api::apply_matrix(&compute_matrix_at(&hue_rotate_matrix(t * 1.3), self.intensity));
                ctx.request_repaint_after(Duration::from_millis(16));
            }
        }

        // ── Chiado — overlay de ruído estático (fica ligado até o usuário desligar) ──
        if !self.inverted_grayscale && matches!(self.filters[self.current].kind, FilterKind::Noise) {
            self.render_noise_overlay(ctx);
        }

        // ── Papel — overlay de grão estático (fica ligado até o usuário desligar) ──
        if !self.inverted_grayscale && matches!(self.filters[self.current].kind, FilterKind::PaperGrain{..}) {
            self.render_paper_overlay(ctx);
        }

        // ── Lanterna — máscara escura seguindo o cursor ──
        if !self.inverted_grayscale && matches!(self.filters[self.current].kind, FilterKind::Spotlight) {
            self.render_spotlight_overlay(ctx);
        }

        // ── Sandman — oscila claro/escuro até o usuário parar ──
        if !self.inverted_grayscale {
            if let FilterKind::Strobe = &self.filters[self.current].kind {
                if let SandmanState::Active(started) = self.sandman_state {
                    const IDENTITY: [f32;25] = [1.,0.,0.,0.,0., 0.,1.,0.,0.,0., 0.,0.,1.,0.,0., 0.,0.,0.,1.,0., 0.,0.,0.,0.,1.];
                    let phase = (started.elapsed().as_secs_f32() * self.sandman_freq_hz as f32).fract();
                    let t = strobe_pulse(phase, self.sandman_smoothness as f32);
                    let dark = strobe_dark_matrix(self.sandman_peak as f32);
                    let base = lerp_matrix(&IDENTITY, &dark, t);
                    mag_api::apply_matrix(&base);
                    ctx.request_repaint_after(Duration::from_millis(8));
                }
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
        let mut go_beta                    = false;
        let mut sandman_warn               = false;
        let mut sandman_cancel             = false;
        let mut new_sandman_freq:   Option<f64> = None;
        let mut new_sandman_smooth: Option<f64> = None;
        let mut new_sandman_peak:   Option<f64> = None;
        let mut new_spotlight_radius: Option<f64> = None;
        let mut minimize                   = false;
        let mut quit                       = false;
        let mut new_intensity: Option<f64> = None;

        let mut content_h: f32 = 0.0;
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(BG).inner_margin(egui::Margin::same(16.0)))
            .show(ctx, |ui| {
                let scroll = egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
                    match self.page {
                        Page::Main   => draw_main(ui, self, &mut to_apply, &mut toggle_inv, &mut go_outros, &mut go_agenda, &mut go_apps, &mut minimize, &mut quit, &mut new_intensity),
                        Page::Outros => draw_outros(ui, self, &mut to_apply, &mut go_main, &mut go_beta, &mut minimize, &mut quit, &mut new_intensity),
                        Page::Agenda => draw_agenda(ui, self, &mut go_main),
                        Page::Apps   => draw_apps(ui, self, &mut go_main),
                        Page::Beta   => draw_beta(ui, self, &mut to_apply, &mut go_outros, &mut sandman_warn, &mut sandman_cancel, &mut new_sandman_freq, &mut new_sandman_smooth, &mut new_sandman_peak, &mut new_spotlight_radius, &mut new_intensity, &mut minimize, &mut quit),
                    }
                });
                content_h = scroll.content_size.y;
            });

        // Janela sem redimensionamento manual: cresce pra caber o conteúdo da
        // página mais alta já vista (com teto — além disso a ScrollArea cobre o
        // resto), mas nunca encolhe ao trocar de página — uma vez que cresceu,
        // fica desse tamanho.
        let target_h = (content_h + 34.0).clamp(MIN_WIN_H, MAX_WIN_H);
        if target_h > self.win_h + 1.0 {
            self.win_h = target_h;
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(egui::vec2(440.0, target_h)));
        }

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
        if go_beta                   { self.page = Page::Beta; }
        if sandman_warn              { self.sandman_state = SandmanState::Warning(Instant::now()); }
        if sandman_cancel            { self.sandman_state = SandmanState::Idle; }
        if let Some(v) = new_sandman_freq   { self.sandman_freq_hz = v; }
        if let Some(v) = new_sandman_smooth { self.sandman_smoothness = v; }
        if let Some(v) = new_sandman_peak   { self.sandman_peak = v; }
        if let Some(v) = new_spotlight_radius { self.spotlight_radius = v; }
        if minimize                  { ctx.send_viewport_cmd(ViewportCommand::Visible(false)); }
        if let Some(v) = new_intensity { self.intensity = v; self.reapply_intensity(); self.persist(); }
        if quit { reset_gamma(); mag_api::reset(); ctx.send_viewport_cmd(ViewportCommand::Close); }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        reset_gamma(); mag_api::reset(); mag_api::uninit();
        self.overlay.quit();
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
        ui.add(egui::Image::new((app.logo_texture.id(), Vec2::new(44.0, 44.0))));
        ui.add_space(2.0);
        ui.label(RichText::new("EyeZen").size(26.0).color(Color32::WHITE).strong());
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
    to_apply: &mut Option<usize>, go_main: &mut bool, go_beta: &mut bool,
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

    ui.add_space(6.0);
    if ui.add(egui::Button::new(RichText::new("\u{1F9EA}  Vers\u{e3}o de Teste").size(12.5).color(Color32::from_rgb(230,200,255)))
        .fill(Color32::from_rgb(60,20,80)).stroke(Stroke::new(1.0, Color32::from_rgb(150,80,190)))
        .min_size(Vec2::new(ui.available_width(), 30.0))).clicked() { *go_beta = true; }

    draw_toast_and_bottom(ui, app, minimize, quit);
}

// ─── Página Versão de Teste ──────────────────────────────────────────────────
//
// Filtros experimentais: a matriz de cor é recalculada a cada quadro em vez de
// uma vez só, então custam mais CPU/GPU que os demais. Ficam de fora das
// páginas normais de propósito.

fn draw_beta(
    ui: &mut egui::Ui, app: &EyeZenApp,
    to_apply: &mut Option<usize>, go_outros: &mut bool,
    sandman_warn: &mut bool, sandman_cancel: &mut bool,
    new_sandman_freq: &mut Option<f64>, new_sandman_smooth: &mut Option<f64>, new_sandman_peak: &mut Option<f64>,
    new_spotlight_radius: &mut Option<f64>,
    new_intensity: &mut Option<f64>,
    minimize: &mut bool, quit: &mut bool,
) {
    ui.set_min_width(420.0);
    ui.horizontal(|ui| {
        if ui.add(egui::Button::new(RichText::new("\u{2190} Voltar").size(13.0).color(Color32::WHITE))
            .fill(Color32::from_rgb(20,70,55)).stroke(Stroke::new(1.5,ACCENT))).clicked() {
            *go_outros = true;
            if matches!(app.sandman_state, SandmanState::Warning(_)) { *sandman_cancel = true; }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new("\u{1F9EA}  Vers\u{e3}o de Teste").size(14.0).color(Color32::from_rgb(230,200,255)).strong());
        });
    });
    ui.add_space(6.0); ui.separator(); ui.add_space(6.0);

    ui.label(RichText::new("Custam mais CPU: a cor da tela é recalculada continuamente, não uma vez só.")
        .size(10.5).color(Color32::from_gray(140)));
    ui.add_space(6.0);

    // Sandman (IDX_SANDMAN) fica de fora dessa grade — tem fluxo próprio mais abaixo.
    const GRID: [usize; 6] = [
        IDX_BETA_START, IDX_BETA_START+1, IDX_BETA_START+3, IDX_BETA_START+4, IDX_BETA_START+5, IDX_BETA_START+6,
    ];
    for row in 0..3usize {
        ui.columns(2, |cols| {
            for col in 0..2 {
                let idx = GRID[row*2 + col];
                if idx >= app.filters.len() { continue; }
                let f = &app.filters[idx]; let active = app.current==idx && !app.inverted_grayscale;
                let w = cols[col].available_width();
                if filter_btn(&mut cols[col], &format!("{}  {}", f.emoji, f.name), active, f.color, w) { *to_apply = Some(idx); }
            }
        });
        ui.add_space(3.0);
    }
    ui.label(RichText::new("Tubbies sorteia uma mistura de cores nova a cada clique. Papel, Chiado e Lanterna usam uma janela-overlay por cima da tela, não a cor da tela em si. Ultradark nunca chega a preto absoluto.")
        .size(9.5).color(Color32::from_gray(120)));
    ui.label(RichText::new("Ficam ligados até você desligar. Se o clique não passar: Ctrl+Alt+0 desliga na hora, independe de clique.")
        .size(9.5).color(Color32::from_rgb(210,160,100)));

    let spotlight_active = app.current == IDX_BETA_START + 6 && !app.inverted_grayscale;
    if spotlight_active {
        ui.add_space(4.0);
        ui.spacing_mut().slider_width = 250.0;
        let mut radius = app.spotlight_radius;
        if ui.add(egui::Slider::new(&mut radius, 40.0..=500.0).logarithmic(true).text("Raio da lanterna")).changed() {
            *new_spotlight_radius = Some(radius);
        }
    }

    if !app.inverted_grayscale { draw_intensity_slider(ui, app.intensity, new_intensity); }

    ui.add_space(10.0); ui.separator(); ui.add_space(8.0);

    let sandman_running = app.current == IDX_SANDMAN && !app.inverted_grayscale
        && matches!(app.sandman_state, SandmanState::Active(_));

    // Frequência, suavidade e pico ficam sempre visíveis — dá pra ajustar antes
    // de ativar ou em tempo real enquanto o Sandman já está piscando. Log-scale
    // e barra larga pra favorecer precisão nos valores baixos (é lá que ficam os
    // ajustes mais úteis: lembrete de piscar, "tubo de imagem" etc.).
    ui.spacing_mut().slider_width = 250.0;

    let mut freq = app.sandman_freq_hz;
    if ui.add(egui::Slider::new(&mut freq, SANDMAN_FREQ_MIN..=SANDMAN_FREQ_MAX)
        .logarithmic(true).text("Frequência (Hz)")).changed() {
        *new_sandman_freq = Some(freq);
    }
    ui.label(RichText::new(format!("1 ciclo a cada {:.1}s \u{2014} abaixo de ~0.3 Hz funciona mais como lembrete de piscar do que como estroboscópio.", 1.0 / freq.max(0.01)))
        .size(9.0).color(Color32::from_gray(120)));
    ui.add_space(4.0);
    let mut smooth = app.sandman_smoothness;
    if ui.add(egui::Slider::new(&mut smooth, 0.0..=1.0)
        .logarithmic(true).smallest_positive(0.01).text("Suavidade")).changed() {
        *new_sandman_smooth = Some(smooth);
    }
    let mut peak = app.sandman_peak;
    if ui.add(egui::Slider::new(&mut peak, 0.0..=1.0)
        .logarithmic(true).smallest_positive(0.01).text("Pico (escurid\u{e3}o)")).changed() {
        *new_sandman_peak = Some(peak);
    }
    ui.label(RichText::new("Suavidade alta = respiração de luz suave, sem corte abrupto. Pico baixo = só uma leve escurecida, sem chegar ao preto.")
        .size(9.0).color(Color32::from_gray(120)));
    ui.add_space(6.0);

    match app.sandman_state {
        SandmanState::Active(_) if sandman_running => {
            egui::Frame::none().fill(Color32::from_rgb(60,10,20)).rounding(6.0)
                .inner_margin(egui::Margin::same(10.0)).show(ui, |ui| {
                    ui.label(RichText::new("\u{1F4A4} Sandman ativo — fica ligado até você parar")
                        .size(12.0).color(Color32::from_rgb(255,150,150)).strong());
                    ui.add_space(4.0);
                    if ui.add(egui::Button::new(RichText::new("Parar agora").size(12.0).color(Color32::WHITE))
                        .fill(Color32::from_rgb(120,20,20)).min_size(Vec2::new(ui.available_width(), 28.0))).clicked() {
                        *to_apply = Some(IDX_DESATIVAR);
                        *sandman_cancel = true;
                    }
                });
        }
        SandmanState::Warning(started) => {
            let remain = (SANDMAN_WARN_SECS as f32 - started.elapsed().as_secs_f32()).ceil().max(0.0) as u64;
            let ready = remain == 0;
            egui::Frame::none().fill(Color32::from_rgb(50,35,10)).rounding(6.0)
                .inner_margin(egui::Margin::same(10.0)).show(ui, |ui| {
                    ui.label(RichText::new("\u{26A0}  Antes de ativar o Sandman")
                        .size(12.5).color(Color32::from_rgb(255,210,120)).strong());
                    ui.add_space(3.0);
                    ui.label(RichText::new(
                        "Tela piscando entre ~3 e 60 Hz é gatilho documentado de crise em epilepsia \
                         fotossensível. Não use durante chamada de vídeo, gravação ou compartilhamento \
                         de tela, e não ative se você (ou alguém perto de você) tiver histórico de \
                         epilepsia ou sensibilidade à luz. Fica ligado até você parar — clicando em \
                         \"Parar agora\", em qualquer outro filtro, ou Ctrl+Alt+0.")
                        .size(10.0).color(Color32::from_gray(180)));
                    ui.add_space(6.0);
                    ui.columns(2, |cols| {
                        let w0 = cols[0].available_width();
                        if cols[0].add(egui::Button::new(RichText::new("Cancelar").size(12.0).color(Color32::from_gray(210)))
                            .fill(Color32::from_rgb(50,50,58)).min_size(Vec2::new(w0,28.0))).clicked() {
                            *sandman_cancel = true;
                        }
                        let w1 = cols[1].available_width();
                        let label = if ready { "Ativar mesmo assim".to_string() } else { format!("Aguarde {}s\u{2026}", remain) };
                        let btn = egui::Button::new(RichText::new(label).size(12.0).color(if ready {Color32::WHITE} else {Color32::from_gray(140)}))
                            .fill(if ready {Color32::from_rgb(150,20,20)} else {Color32::from_rgb(45,45,50)})
                            .min_size(Vec2::new(w1,28.0));
                        if cols[1].add_enabled(ready, btn).clicked() { *to_apply = Some(IDX_SANDMAN); }
                    });
                });
            ui.ctx().request_repaint_after(Duration::from_millis(200));
        }
        _ => {
            if ui.add(egui::Button::new(RichText::new("\u{1F4A4}  Sandman (estrobosc\u{f3}pio)").size(12.5).color(Color32::from_rgb(255,190,190)))
                .fill(Color32::from_rgb(70,15,20)).stroke(Stroke::new(1.0, Color32::from_rgb(150,40,40)))
                .min_size(Vec2::new(ui.available_width(), 30.0))).clicked() { *sandman_warn = true; }
            ui.add_space(3.0);
            ui.label(RichText::new("Pisca a tela — mostra um aviso antes de ativar de verdade.")
                .size(9.5).color(Color32::from_gray(120)));
        }
    }

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
                                    if fi >= IDX_BETA_START { continue; } // efeitos experimentais não entram na agenda
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
                                        if fi >= IDX_BETA_START { continue; } // efeitos experimentais não entram em regras automáticas
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
        if bevel_btn(&mut cols[0], "Minimizar", Color32::from_rgb(32,78,150), w0, 30.0) { *minimize = true; }
        let w1 = cols[1].available_width();
        if bevel_btn(&mut cols[1], "Sair", Color32::from_rgb(160,26,26), w1, 30.0) { *quit = true; }
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
