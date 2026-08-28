# EyeZen v2 — Build Log

> Registro completo para continuidade por outra IA ou desenvolvedor.

---

## Status: BUILD RELEASE CONCLUÍDO ✅

**Data:** 2026-06-08  
**Binário:** `target/release/EyeZen.exe` — **3.54 MB** (vs 60-80 MB da versão Python)  
**Linguagem:** Rust 1.96.0  

---

## O que foi feito

### Problema original (Python/PyInstaller)
- Executável de 60-80 MB (Python runtime embutido via PyInstaller)
- Caminho hardcoded: `LOGO_PATH = r"C:\Users\LucasRafaeldeAndrade\..."` → falha em outros PCs
- Startup lento (~3-5s) por causa do Python runtime
- Interface básica sem indicação visual do filtro ativo
- Sem interceptação do botão X (fechava em vez de minimizar à bandeja)

### Solução (Rust)
- **3.54 MB** de binário autocontido, sem dependências externas
- Startup instantâneo (<100ms)
- Zero caminhos hardcoded — configuração salva junto ao .exe
- GUI com egui: botão ativo destaque (borda verde + cor diferente)
- X da janela → minimiza para bandeja (não fecha)

---

## Arquitetura

```
src/main.rs — arquivo único (~460 linhas)
├── all_filters()         → definições de 8 filtros (RGB gains + cor do botão)
├── set_gamma()           → chama SetDeviceGammaRamp via winapi (Win32 direto)
├── load_config/save_config → JSON em eyezen_config.json ao lado do .exe
├── EyeZenApp (eframe::App)
│   ├── new()             → cria bandeja (tray-icon), CheckMenuItems, aplica filtro salvo
│   ├── apply()           → aplica gamma + atualiza checkmarks da bandeja + toast
│   └── update()          → loop egui: processa eventos canal/bandeja/hotkeys, desenha UI
└── main()
    ├── GlobalHotKeyManager → registra Ctrl+Alt+1..8
    ├── thread background → poleia GlobalHotKeyEvent, manda para channel
    └── eframe::run_native → inicia com janela oculta (starts na bandeja)
```

---

## Dependências (Cargo.toml)

| Crate          | Versão  | Função                              |
|----------------|---------|-------------------------------------|
| eframe         | 0.29.1  | janela principal + loop de eventos  |
| egui           | 0.29.1  | widgets immediate-mode GUI          |
| tray-icon      | 0.18.0  | bandeja do sistema (Win32 Shell)    |
| global-hotkey  | 0.6.4   | Ctrl+Alt+1..8 globais               |
| serde          | 1.x     | serialização JSON                   |
| serde_json     | 1.x     | leitura/escrita eyezen_config.json  |
| winapi         | 0.3.9   | SetDeviceGammaRamp, GetDC           |

---

## Como compilar

```powershell
# Instalar Rust (uma vez)
winget install Rustlang.Rustup

# Compilar release
cd EyeZen-v2
cargo build --release

# Binário gerado:
# target/release/EyeZen.exe  (~3.5 MB, sem dependências)
```

---

## Filtros implementados

| # | Nome              | R     | G     | B     | Hotkey    |
|---|-------------------|-------|-------|-------|-----------|
| 1 | Noturno Suave     | 0.95  | 1.00  | 0.85  | Ctrl+Alt+1 |
| 2 | Leitura Verde     | 0.88  | 1.00  | 0.88  | Ctrl+Alt+2 |
| 3 | Papel Creme       | 1.00  | 0.98  | 0.85  | Ctrl+Alt+3 |
| 4 | Desativado        | 1.00  | 1.00  | 1.00  | Ctrl+Alt+4 |
| 5 | Fique Acordado    | 0.95  | 1.00  | 1.15  | Ctrl+Alt+5 |
| 6 | Dark Mode         | 0.70  | 0.70  | 0.70  | Ctrl+Alt+6 |
| 7 | Modo Kindle       | 0.85  | 0.88  | 0.80  | Ctrl+Alt+7 |
| 8 | Sépia Clássico    | 1.00  | 0.95  | 0.82  | Ctrl+Alt+8 |

---

## Melhorias pendentes (próxima sessão)

- [ ] **Ícone personalizado no .exe e na bandeja**: usar `winres` em build.rs para embeddar o .ico
  - Copiar `imagens/eyezen.ico` do repo original para `assets/eyezen.ico`
  - `cargo add winres --build`
  - `build.rs` com `winres::WindowsResource::new().set_icon("assets/eyezen.ico").compile()`
- [ ] **Startup automático com Windows**: escrever chave de registro em `HKCU\...\Run`
- [ ] **Slider de intensidade**: ajuste fino de 0–100% em vez de filtros fixos
- [ ] **Modo auto por horário**: ex. Noturno Suave após 20h, Desativado após 7h
- [ ] **UPX** para comprimir ainda mais (3.5 MB → ~1.5 MB):
  ```powershell
  winget install upx; upx --best target/release/EyeZen.exe
  ```
- [ ] **Criar instalador** com Inno Setup (igual ao plano original, mas agora com exe menor)

---

## Bugs conhecidos

- Tray icon usa círculo verde gerado dinamicamente — não usa o .ico customizado ainda
- `ShowWindow` e `Quit` no enum `AppEvent` não são usados via canal (só via tray direto)
- `on_exit` com assinatura glow pode conflitar se eframe mudar de renderer

---

## Estrutura de arquivos

```
EyeZen-v2/
├── src/
│   └── main.rs          ← código completo
├── Cargo.toml
├── Cargo.lock
├── .gitignore
└── BUILD_LOG.md         ← este arquivo
```
