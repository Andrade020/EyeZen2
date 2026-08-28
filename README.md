# EyeZen

Filtros de cor para Windows que se adaptam ao que você está fazendo.

Gratuito · Portátil · 3.5 MB · Feito em Rust

---

## Início rápido

### Opção 1 — baixar o executável (recomendado)

1. Baixe **[EyeZen.exe](https://github.com/Andrade020/EyeZen2/releases/latest/download/EyeZen.exe)** na página de [Releases](https://github.com/Andrade020/EyeZen2/releases)
2. Execute o arquivo — não há instalador. O app inicia direto na **bandeja do sistema**; nenhuma janela aparece na tela
3. Clique no ícone na bandeja (ou clique com o botão direito → **Abrir Painel**) para abrir o painel de filtros
4. Escolha um filtro na lista, ou use os atalhos de teclado abaixo

### Opção 2 — compilar do código-fonte

Requer [Rust](https://rustup.rs) instalado.

```powershell
git clone https://github.com/Andrade020/EyeZen2
cd EyeZen2
cargo run --release
```

Ou, para apenas gerar o binário sem executar:

```powershell
cargo build --release
# Binário gerado em: target\release\EyeZen.exe
```

---

## O que é

EyeZen vai além do modo noturno padrão do Windows. Em vez de um único slider de temperatura de cor, ele oferece filtros compostos que ajustam saturação, contraste e balanço de cor de forma independente — incluindo efeitos que a API de gamma do Windows normalmente bloqueia.

> v2 é uma reescrita completa em Rust. A v1 original, em Python, está em [github.com/Andrade020/EyeZen](https://github.com/Andrade020/EyeZen).

---

## Funcionalidades

- 16 filtros preset (Noturno Suave, Papel Creme, Fósforo Âmbar, Modo Feliz, Modo Retrô, Astrônomo e outros)
- Slider de intensidade de 0 a 150%
- Troca automática de filtro por app em foco (Chrome, VS Code, Word, etc.)
- Agendamento por intervalo de horário
- Hotkeys globais (funcionam em qualquer janela, mesmo com o painel fechado)
- Transição suave entre filtros (1.4s ease-out)
- Ícone na bandeja do sistema
- Configuração salva automaticamente

---

## Como usar (primeira execução)

Ao abrir, o EyeZen **não mostra nenhuma janela** — ele carrega direto na bandeja do sistema (o ícone circular perto do relógio, às vezes escondido no menu de "ícones ocultos" `^`). A partir daí:

- **Clique no ícone da bandeja** para abrir o painel principal, com os filtros em grade
- **Clique com o botão direito** no ícone para o menu rápido: escolher filtro, alternar Cinza Invertido, "Abrir Painel" ou "Sair"
- No painel, a aba **Agenda** liga/desliga filtros por horário do dia, e a aba **Apps** associa um filtro a cada programa em foco

### Atalhos de teclado globais

| Atalho | Ação |
|---|---|
| `Ctrl+Alt+1` | Noturno Suave |
| `Ctrl+Alt+2` | Leitura Verde |
| `Ctrl+Alt+3` | Papel Creme |
| `Ctrl+Alt+4` | Fique Acordado |
| `Ctrl+Alt+5` | Crepúsculo |
| `Ctrl+Alt+6` | Modo Feliz |
| `Ctrl+Alt+0` | Desativar |
| `Ctrl+Alt+9` | Cinza Invertido |

(Os filtros da aba "Outros" não têm atalho dedicado — são acessados pelo painel.)

---

## Compatibilidade

Windows 10 e Windows 11. Testado em x64.

---

## Licença

MIT
