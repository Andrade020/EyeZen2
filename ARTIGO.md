# Olhos cansados? Fiz um app em Rust que ajusta sua tela para cada momento do dia.

Ele é gratuito e vai muito além do modo noturno: tem filtro pra olho seco, pra descanso visual e até um modo pra espantar o sono. E o melhor: muda automaticamente conforme o app que você está usando.

---

## Introdução

Tudo começou com um problema banal: ficar em frente ao computador depois das 22h com a tela no brilho padrão dói. Não é uma descoberta nova. Pesquisas sobre luz azul e ritmo circadiano existem desde os anos 2000. O f.lux foi lançado em 2009. O Windows 10 incluiu o "Night Light" em 2017.

O problema está resolvido, então.

Não está.

Quando você começa a usar essas ferramentas com atenção, percebe algo: elas resolvem um caso muito específico, que é o caso mais simples. Temperatura de cor mais quente à noite. Tudo bem. Mas o conforto visual é um problema com mais dimensões do que isso.

Existe o programador que trabalha no dark mode e precisa de um ajuste diferente do designer que trabalha com cores. Existe a diferença entre ler PDF às três da tarde num dia nublado versus assistir a um vídeo no escuro. Existe o monitor barato com backlight irregular versus o painel OLED com contraste nativo. Existe o usuário com sensibilidade à luz que não consegue usar o modo noturno padrão porque ele distorce demais as cores para o trabalho dele.

Nenhuma dessas ferramentas trata essas diferenças. Elas oferecem um slider chamado "temperatura" e uma opção de horário.

A hipótese que guiou o EyeZen desde o início foi simples: filtros de cor deveriam ser compostos, configuráveis por contexto, e rápidos de alternar. O usuário deveria poder ter um perfil para leitura noturna, outro para edição de imagens, outro para reuniões em vídeo, e mudar entre eles sem abrir nenhum painel.

A hipótese estava certa. Mas o caminho para implementar isso foi cheio de paredes onde não esperávamos.

[IMAGEM 1: screenshot da versão inicial do EyeZen em Python, mostrando a interface simples com poucos botões]

---

## O mercado parece resolvido. Não está.

O f.lux é o ponto de referência desta categoria. Funciona bem, tem comunidade, tem anos de refinamento. Mas olhando para ele como produto, algumas escolhas ficam datadas.

A interface principal é um painel modal que você abre quando quer ajustar algo. Não existe conceito de "perfil por aplicativo". A curva de temperatura ao longo do dia é o único eixo de configuração. O controle de intensidade existe, mas enterrado. A transição entre estados é automática baseada em horário e localização, o que é ótimo para o caso geral mas remove agência do usuário para casos específicos.

O Windows Night Light é ainda mais limitado. É uma implementação de referência, não um produto. Dois sliders, um toggle, um horário. Funciona para quem quer algo sem instalar nada, mas não vai além disso.

Existem alternativas pagas como o Iris que tentam ir mais longe, com dezenas de parâmetros e modos predefinidos. O problema delas é diferente: há tanta configuração exposta que o usuário médio abre o app, fica perdido, e vai embora. A complexidade não está organizada de forma que faça sentido intuitivo.

Existe uma armadilha comum nesses aplicativos que ninguém discute abertamente: eles foram construídos assumindo que o conforto visual é um problema de temperatura de cor. Isso está tecnicamente errado.

Temperatura de cor descreve a composição espectral da luz, mas a percepção de conforto visual envolve contraste, saturação, brilho relativo entre elementos da interface, fadiga por cintilação, e a proporção entre luminância da tela e do ambiente. Um filtro que só move a temperatura está atacando uma variável de um sistema com oito variáveis.

[COMPARAÇÃO VISUAL: lado a lado de f.lux, Windows Night Light e EyeZen mostrando as diferenças de interface e opções disponíveis]

O EyeZen começou tentando resolver isso com matrizes de cor completas, que permitem manipular cada canal RGB de forma independente e com relações entre canais. Mas antes de chegar nisso, houve uma primeira versão que não usava nada disso.

---

## O que tentamos primeiro e por que não funcionou

A primeira versão do EyeZen foi escrita em Python. Faz sentido: prototipagem rápida, biblioteca tkinter disponível, código de 300 linhas, rodava no dia seguinte. O problema apareceu quando chegou a hora de distribuir.

O PyInstaller empacota o runtime inteiro do Python dentro do executável. O resultado foi um `.exe` de 67 megabytes. Para um aplicativo que mora na bandeja do sistema e aplica um filtro de cor, isso é difícil de justificar. Além disso, o caminho para a imagem do ícone estava hardcoded como `C:\Users\LucasRafaeldeAndrade\...`. Funcionava na máquina de desenvolvimento. Em qualquer outra, crashava na inicialização.

A lição aqui é velha e todo mundo sabe mas ignora no calor do protótipo: nunca construa dependência de caminho absoluto. O problema é que quando você está construindo algo para uso próprio, o custo parece zero porque funciona. O custo aparece quando você tenta distribuir.

A interface da versão Python também sofria de um problema de arquitetura: era uma janela tkinter que ficava aberta. O usuário tinha que minimizar manualmente. Não havia integração com a bandeja do sistema. Não havia hotkeys globais. Era um aplicativo de desktop tradicional disfarçado de ferramenta de sistema.

Depois de alguns meses usando aquela versão, ficou claro que o modelo de interação estava errado. Ferramentas de sistema não devem ser janelas que ficam abertas. Elas vivem na bandeja, esperam um atalho de teclado, ou são esquecidas completamente depois da configuração inicial. O f.lux você configura uma vez e não abre de novo por semanas. Era isso que precisávamos.

A decisão foi reescrever em Rust.

[IMAGEM 2: comparação de tamanho de arquivo, Python 67MB vs Rust 3.54MB]

A escolha de Rust não foi por performance de CPU. Filtros de cor não são computacionalmente intensivos. A escolha foi por:

- Executável autocontido sem runtime externo
- Startup instantâneo contra 3-5 segundos do Python empacotado
- Acesso direto às APIs do Windows sem camadas intermediárias
- Binário de 3.54MB contra os 67MB anteriores

O stack escolhido foi eframe com egui para a interface gráfica, tray-icon para a bandeja do sistema, e global-hotkey para os atalhos de teclado. Acesso às APIs do Windows via bindings diretos através do crate winapi.

---

## As escolhas de design

### Por que immediate mode GUI

O egui usa o modelo de interface immediate mode, onde a interface inteira é redesenhada a cada frame em vez de manter uma árvore de objetos na memória. Isso parece ineficiente à primeira vista.

Na prática, para um aplicativo como esse, é a escolha certa. A interface tem menos de 20 widgets interativos. O estado da aplicação é simples: qual filtro está ativo, qual é a intensidade, qual é a página atual. Com immediate mode, esse estado é a única fonte de verdade. Não há sincronização entre estado de aplicação e estado de interface. Não há listeners, callbacks, ou property bindings. O código fica linear e fácil de entender.

A desvantagem real do egui é que o visual padrão é funcional mas não polido. Passamos algum tempo customizando cores, bordas, espaçamentos e estilos dos botões para chegar a algo que não parecesse um protótipo.

### A decisão de tela que mais importou: a bandeja como ponto de entrada

A janela principal abre invisível. O aplicativo começa na bandeja do sistema. Clicar no ícone mostra a janela. Fechar a janela a esconde de volta para a bandeja.

Isso parece óbvio, mas exige uma linha específica no código que intercepta o evento de fechamento e cancela a operação padrão:

```rust
if ctx.input(|i| i.viewport().close_requested()) {
    ctx.send_viewport_cmd(ViewportCommand::CancelClose);
    ctx.send_viewport_cmd(ViewportCommand::Visible(false));
}
```

Sem essa interceptação, o aplicativo fecha e os filtros param de funcionar. O usuário fica sem saber o que aconteceu porque a tela voltou ao normal.

Esse padrão é o que diferencia uma ferramenta de sistema de um aplicativo de desktop. É a razão pela qual você pode instalar o f.lux e esquecer que ele existe.

### A arquitetura dos filtros

A abordagem inicial era simples: cada filtro era uma tripla de ganhos RGB. Para fazer o Noturno Suave, multiplica a curva de vermelhos por 0.95, a de verdes por 1.0, a de azuis por 0.85. Aplica via `SetDeviceGammaRamp`. Pronto.

Isso funciona para filtros de ajuste leve. Mas havia filtros que precisavam de efeitos mais intensos, como o modo para astrofotografia que precisa de quase zero de azul e verde para preservar a adaptação do olho ao escuro, ou o filtro que simula a aparência de um monitor fósforo âmbar dos anos 80.

Para o filtro de fósforo âmbar, a ideia era simples: converte tudo para escala de cinza e tinge de laranja. Matematicamente, isso significa que cada canal de saída é uma combinação ponderada de todos os canais de entrada, algo que a tripla RGB não consegue expressar.

[IMAGEM 3: exemplo visual do filtro Fósforo Âmbar aplicado a uma interface de código]

Precisávamos de uma matriz de transformação de cor.

O Windows oferece isso via Magnification API, uma API do sistema de acessibilidade que originalmente foi feita para aumentar regiões da tela para usuários com baixa visão. Como efeito colateral, ela aceita uma matriz 5x5 que transforma os valores RGBA de cada pixel antes de renderizar na tela.

A matriz funciona assim: para cada pixel, o valor de saída de cada canal é a soma ponderada de todos os canais de entrada mais um offset. A quinta linha da matriz representa deslocamentos de cor independentes da entrada. O layout é:

```
transform[input_channel * 5 + output_channel]
```

O coeficiente `transform[0*5 + 0]` multiplica o canal vermelho da entrada pelo canal vermelho da saída. O coeficiente `transform[1*5 + 0]` multiplica o canal verde da entrada para o canal vermelho da saída. Isso permite transformações impossíveis com ganhos simples, como o cross-channel mixing necessário para o sépia ou o fósforo âmbar.

Criamos então três tipos de filtro:

- `Linear`: ganhos RGB simples via `SetDeviceGammaRamp`
- `ToneMap`: curva S com parâmetros de escala e levantamento de sombras, também via `SetDeviceGammaRamp`
- `ColorMatrix`: matriz 5x5 via Magnification API

### A transição suave

Quando o usuário muda de filtro, a tela não pula abruptamente de um estado para outro. Há uma animação de 1,4 segundos com easing cúbico.

A implementação armazena a curva gamma de origem e de destino, e a cada frame computa um valor interpolado:

```rust
let t = 1.0 - (1.0 - raw_t).powi(3); // ease-out cúbico
```

O expoente 3 produz uma aceleração inicial seguida de desaceleração no final, que é o perfil perceptualmente mais natural para transições de cor. Uma transição linear parece mecânica; uma transição cúbica parece que a tela está se "acomodando" no novo estado.

[GIF: transição suave entre dois filtros de cor, mostrando o easing]

---

## A parede que não esperávamos: o driver de display

Este foi o problema mais curioso e mais difícil de diagnosticar.

Alguns filtros pareciam não funcionar. O Modo Vela, que cria uma aparência quente e com pouco azul, simplesmente não mudava nada na tela. O código estava correto. Os valores eram passados corretamente para a API. E a tela ficava igual.

O `SetDeviceGammaRamp` falha silenciosamente quando o ganho de qualquer canal fica abaixo de aproximadamente 0.5 em drivers modernos de Windows. Não retorna erro. Não lança exceção. Apenas não faz nada.

Esta é uma limitação de driver, não de API. A documentação da Microsoft menciona brevemente que valores fora do intervalo "suportado" podem ser rejeitados, mas não especifica qual é esse intervalo, e o intervalo muda conforme o driver e o hardware.

O Modo Vela precisava de azul em 0.12. O filtro Fósforo Âmbar precisava de azul em 0.04. O filtro Astrônomo precisava de verde em 0.08. Todos abaixo do limite do driver.

A solução foi usar a Magnification API para esses filtros. Ela não tem a mesma restrição. A desvantagem é que a Magnification API é global e sobrescreve qualquer transformação de escala existente, o que em monitores com HiDPI pode causar comportamentos inesperados. A solução foi carregar a biblioteca dinamicamente e checar se as funções existem antes de chamar:

```rust
let mag_lib = LoadLibraryA(b"Magnification.dll\0".as_ptr() as _);
```

Isso tornou o código mais resiliente a versões antigas do Windows onde a Magnification API pode não estar disponível.

[IMAGEM 4: diagrama mostrando quando cada API é usada - SetDeviceGammaRamp vs Magnification API]

---

## O que aprendemos sobre percepção de cor

Existem conceitos sobre percepção de cor que não aparecem nas documentações de API mas que são centrais para entender por que certos filtros funcionam e outros não.

O olho humano não é linear. Dobrar a intensidade de luz não parece o dobro de brilho. A percepção segue uma curva aproximadamente logarítmica. É por isso que a gamma ramp do Windows usa 256 pontos de 0 a 65535 em vez de uma escala linear: os pontos são distribuídos para compensar a resposta não-linear do sistema visual humano.

Essa não-linearidade tem implicações práticas. Um filtro que reduz o azul de 255 para 200 parece muito menos intenso que um filtro que reduz de 100 para 50, mesmo que a diferença numérica seja a mesma. A percepção está na razão, não na diferença absoluta.

Existe também o fenômeno da adaptação cromática. Depois de alguns minutos olhando para uma tela com filtro vermelho intenso, o branco começa a parecer branco de verdade novamente. O sistema visual ajusta sua referência de "neutro" dinamicamente. Isso significa que filtros muito agressivos perdem eficácia ao longo do tempo, o que é uma das razões para o slider de intensidade existir: o usuário pode começar mais suave e aumentar progressivamente.

A saturação é outra dimensão negligenciada. A maioria dos filtros de conforto visual trabalha exclusivamente com temperatura de cor, que é um ajuste de equilíbrio entre vermelho, verde e azul. Mas a saturação, que é a "pureza" das cores ou o quanto elas se desviam do cinza, também afeta o cansaço visual. Interfaces com saturação muito alta exigem mais esforço cognitivo para processar.

É por isso que o Modo Feliz do EyeZen usa uma matriz de saturação aumentada: não para conforto, mas para o caso de uso oposto, quando o usuário quer maior vivacidade e presença das cores, por exemplo em trabalho criativo ou como contra-ponto ao ambiente.

O Modo Retrô usa um ToneMap com levantamento de sombras (lift) combinado com redução de azul. Isso cria a aparência de um monitor CRT vintage não só pela temperatura mais quente, mas pela compressão tonal que imita os phosphors de uma tela de tubo.

[COMPARAÇÃO VISUAL: as mesmas cores vista com diferentes filtros, incluindo temperatura, saturação e tonemap]

---

## O problema que ninguém comenta: o contexto muda tudo

Existe uma falácia implícita em todos os aplicativos de filtro de cor do mercado: que o conforto visual ótimo é uma constante.

Não é.

O que você precisa quando lê um PDF técnico às 14h num dia ensolarado é diferente do que precisa quando assiste a um filme às 23h num quarto escuro. O que você precisa quando edita código é diferente do que precisa quando revisa um documento com formatação rica.

A maioria dos aplicativos trata isso com o horário como único eixo de variação. Isso é uma aproximação grosseira que funciona para o caso médio e falha para casos específicos.

A abordagem do EyeZen é diferente: o sistema detecta qual processo está em foco no sistema operacional e aplica o filtro configurado para aquele contexto.

```rust
fn get_foreground_process_name() -> Option<String> {
    unsafe {
        let hwnd = GetForegroundWindow();
        let mut pid: DWORD = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        // ...
        path.split('\\').last().map(|s| s.to_lowercase())
    }
}
```

A chamada custa microssegundos e é feita a cada 500 milissegundos. O custo computacional é negligenciável. O resultado é que quando você abre o Chrome o filtro muda, quando você minimiza e volta ao editor de código o filtro muda de volta.

Isso não existe em nenhum dos concorrentes principais. O f.lux não tem esse conceito. O Windows Night Light não tem. O Iris tem "perfis", mas eles são manuais, não automáticos por contexto.

A automação por contexto é onde a categoria vai evoluir nos próximos anos. As interfaces de conforto visual estão onde os gerenciadores de janelas estavam nos anos 90: funcionais, mas sem consciência de contexto.

---

## Comparando abordagens

### f.lux

Pontos fortes: maturidade, comunidade, integração com o ciclo circadiano, algoritmo de cor bem calibrado, funciona em Windows, Mac e Linux.

Pontos fracos: não tem filtros preset que o usuário possa customizar além da temperatura, não tem transição manual entre perfis, sem automação por aplicativo.

Faz sentido quando: você quer algo que funcione sem configuração e que se ajuste automaticamente com base em horário e localização.

O EyeZen faz mais sentido quando: você quer controle granular, filtros específicos para tipos de trabalho, ou automação por contexto de aplicativo.

### Windows Night Light

Pontos fortes: já instalado, zero fricção de setup, integrado com configurações do sistema.

Pontos fracos: dois parâmetros, sem programabilidade, sem perfis, sem hotkeys.

Faz sentido quando: o usuário quer simplesmente "menos azul à noite" sem instalar nada.

### Iris (e similares pagos)

Pontos fortes: mais parâmetros, mais modos, suporte premium.

Pontos fracos: interface sobrecarregada, custo recorrente, complexidade que não se converte em clareza de uso.

Faz sentido quando: o usuário é um profissional de saúde visual ou tem necessidades muito específicas e tempo para configurar.

[IMAGEM 5: tabela de comparação de funcionalidades entre EyeZen, f.lux, Windows Night Light e Iris]

---

## Decisões controversas

Algumas escolhas do EyeZen são deliberadamente diferentes do que a maioria faria.

A primeira é o slider de intensidade de 0 a 150% em vez de 0 a 100%. A escolha convencional é limitar em 100% porque acima disso o efeito "começa a parecer exagerado". Mas exagerado para quem? Para o usuário médio, talvez. Para alguém com hipersensibilidade à luz ou usando o app em ambiente muito escuro, 100% pode ser o mínimo necessário. Colocar o hard cap em 100% é uma decisão paternalista. Colocar em 150% e deixar o usuário explorar é respeitar a diversidade de casos de uso.

A segunda é a decisão de não ter um "modo automático por localização" como o f.lux. Implementar isso exigiria GPS ou entrada manual de cidade, um servidor de horários ou cálculos astronômicos. O custo de manutenção para um projeto de código pequeno não compensa. E mais importante: o horário como único eixo de automação é a hipótese que estamos questionando. Adicionar automação por horário enquanto construímos automação por contexto seria validar uma premissa que consideramos incompleta.

A terceira é a escolha de não ter uma versão para Mac ou Linux. O EyeZen usa extensivamente APIs do Windows que não têm equivalente direto em outros sistemas. O `SetDeviceGammaRamp` existe no Linux via xrandr, mas com comportamento diferente. A Magnification API é exclusiva do Windows. Portar o app significaria reescrever a camada de efeitos inteira para cada plataforma. Por enquanto, melhor fazer uma coisa bem do que três coisas medianamente.

Crítica válida: a automação por processo em foco pode ter comportamento estranho em ambientes com múltiplos monitores. O `GetForegroundWindow` retorna a janela ativa, que pode estar num monitor diferente do monitor onde o filtro está sendo aplicado. A Magnification API aplica globalmente, não por monitor. Essa limitação é real e ainda não resolvemos.

---

## O que vem pela frente

A próxima fronteira técnica é aplicação de filtros por monitor individual. O Windows permite aplicar gamma ramps separadas para cada monitor via handles de device context específicos. A Magnification API, infelizmente, é global. Para filtros que precisam da Magnification API (matrizes de cor), aplicação por monitor exigiria uma abordagem diferente, talvez um driver de display em modo kernel.

Drivers em modo kernel no Windows exigem assinatura digital cara e um processo de certificação da Microsoft. Não é viável para um projeto independente no estágio atual. Mas é para onde a categoria vai se mover quando os jogadores maiores precisarem diferenciar.

Outra área de investigação é a calibração adaptativa. Em vez de o usuário escolher um filtro manualmente, o sistema poderia medir a luminância ambiente e ajustar automaticamente. Webcams modernas conseguem estimar a luminância do ambiente. O problema é que exige permissão de câmera, o que cria fricção de instalação e possível resistência do usuário por razões de privacidade.

Exists também a questão de acessibilidade formal. Daltonismo afeta aproximadamente 8% dos homens e 0.5% das mulheres. Existem matrizes de transformação de cor projetadas para cada tipo de daltonismo (protanopia, deuteranopia, tritanopia) que podem aumentar o contraste percebido para usuários afetados. Implementar isso de forma correta e calibrada é um trabalho de pesquisa, não apenas de engenharia.

[IMAGEM 6: protótipo de interface para seleção de perfil de daltonismo]

A pergunta sem resposta mais interessante é sobre efeitos de longo prazo. Há evidências razoáveis de que redução de luz azul melhora qualidade de sono quando a exposição é à noite. Mas os efeitos de filtros de cor durante o dia no trabalho são menos documentados. Seria possível medir, através de telemetria opt-in, se usuários que configuram determinados filtros para leitura reportam menos fadiga? Essa é a pesquisa que ninguém fez.

---

## Conclusão

Software que parece simples é geralmente o mais difícil de fazer bem.

Um aplicativo de filtro de cor deveria ser trivial. Pega os pixels, multiplica por alguns coeficientes, exibe. Qualquer programador iniciante consegue uma prova de conceito em uma tarde.

O problema é que o espaço entre "prova de conceito" e "ferramenta que funciona de verdade para pessoas reais" está cheio de surpresas. Um driver de display que silenciosamente ignora seus valores. Uma API de acessibilidade que acidentalmente faz o que você precisa mas foi feita para outra coisa. Um modelo de interação que parece óbvio mas que afasta o usuário. Um executável de 67MB que ninguém vai baixar.

Cada uma dessas surpresas exige uma decisão: aceitar a limitação, encontrar um workaround, ou repensar a abordagem. As decisões se acumulam. O resultado final é muito mais específico, muito mais carregado de contexto e de tentativas fracassadas, do que qualquer especificação ou design inicial conseguiria capturar.

Isso é verdade para qualquer software de infraestrutura de usuário, que é a categoria onde ferramentas de sistema como essa se encaixam. É o tipo de software que funciona em segundo plano, que a maioria das pessoas nunca vai notar quando está funcionando, mas que vai sentir imediatamente quando para.

O texto do commit que mais sintetiza o projeto inteiro não é o commit de features. É um commit de 2 linhas que corrige o método `save` conflitando com o trait `eframe::App::save`. Ninguém vai ler esse commit. Mas sem ele, o app crashava ao salvar configurações.

A maioria do trabalho real em software é assim.

---

*EyeZen é um projeto em desenvolvimento ativo. O código é Rust, a interface é egui, e o binário tem 3.54 MB. Se você tiver interesse em contribuir ou acompanhar o desenvolvimento, os comentários estão abertos.*
