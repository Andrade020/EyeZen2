# EyeZen

Filtros de cor para Windows que se adaptam ao que você está fazendo.

Gratuito · Portátil · 3.5 MB · Feito em Rust

---

## Download

[**⬇ Baixar EyeZen.exe**](https://github.com/Andrade020/EyeZen2/releases/latest/download/EyeZen.exe)

Não requer instalação. Só abrir e usar.

---

## O que é

EyeZen vai além do modo noturno padrão do Windows. Em vez de um único slider de temperatura de cor, ele oferece filtros compostos que ajustam saturação, contraste e balanço de cor de forma independente — incluindo efeitos que a API de gamma do Windows normalmente bloqueia.

---

## Funcionalidades

- 16 filtros preset (Noturno Suave, Papel Creme, Fósforo Âmbar, Modo Feliz, Modo Retrô, Astrônomo e outros)
- Slider de intensidade de 0 a 150%
- Troca automática de filtro por app em foco (Chrome, VS Code, Word, etc.)
- Agendamento por intervalo de horário
- Hotkeys globais Ctrl+Alt+1–9
- Transição suave entre filtros (1.4s ease-out)
- Ícone na bandeja do sistema
- Configuração salva automaticamente

---

## Como usar

1. Baixe o `EyeZen.exe` acima
2. Abra o arquivo — ele vai para a bandeja do sistema
3. Clique no ícone para abrir o painel
4. Escolha um filtro ou configure a automação por app na aba **Apps**

---

## Compilar do código

Requer [Rust](https://rustup.rs) instalado.

```powershell
cargo build --release
# Binário gerado em: target/release/EyeZen.exe
```

---

## Compatibilidade

Windows 10 e Windows 11. Testado em x64.

---

## Licença

MIT
