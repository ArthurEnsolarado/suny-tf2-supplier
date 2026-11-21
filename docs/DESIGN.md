# Direção visual

> **Revisão 2 (2026-11-21).** A primeira versão (folha de pedido em papel) foi **rejeitada pelo usuário**: não parecia TF2 e parecia genérica. Esta versão segue as capturas de tela do menu do TF2 que o usuário mandou como referência, e usa a fonte **TF2 Build**.

## Brief

| Item | Decisão |
|---|---|
| Público | Jogador de TF2 no PC, em sessões curtas antes de jogar. Conhece hitsound, HUD e casual, não conhece `sv_pure`. |
| Ação principal | **Decidir "instalo ou não instalo esse mod"** e ver se o jogo está pronto. |
| Voz | Direta, seca, com um pouco de humor de mercenário. Português do Brasil. |
| Modo | **Produto**: clareza e densidade primeiro. |
| Restrições | Tauri 2 + React, janela de desktop (cerca de 1180×780), tudo offline, sem usar imagens, logo ou personagens da Valve. |
| Anti-referências | Painel escuro genérico com gradiente roxo, cards idênticos em grade, vidro fosco, tom de marketing, e **qualquer coisa que não pareça vir do mesmo jogo**. |

## Referências (as capturas do usuário)

Menu inicial novo, tela de loadout, menu clássico, placar, tela de "Mission Complete" e menu com perfil. O que elas têm em comum, e o que o app copia **como linguagem visual, não como arte**:

- **Barra de abas no topo**, com a aba ativa em amarelo-laranja e uma faixa laranja grossa embaixo. As inativas são marrons e mais baixas.
- **Painéis escuros** (marrom-cinza quase preto) com título em caixa alta e cantos pouco arredondados.
- **Botões creme** com texto escuro em caixa alta, ícone à esquerda.
- **Faixas em ângulo** (o placar azul e vermelho, a faixa vermelha do "Mission Complete").
- **Balão creme inclinado** para falas e avisos (o "I am full of sandwich").
- **Caixas com contorno laranja e título entalhado na borda** (os itens do loadout).
- **Fundo sépia** dessaturado e quente. Sem foto: usamos cor chapada.
- **Número grande** de destaque em creme com sombra dura (o "HEAVY").

## Contrato de direção

```
TESE:        O app tem que parecer uma tela do próprio TF2: quem abrir deve achar que o
             jogo ganhou uma aba "Fornecedor".
MUNDO:       Menu do TF2 (HUD/VGUI): painéis escuros, botões creme, abas laranja, faixas
             vermelhas e azuis em ângulo, fundo sépia.
TIPOGRAFIA:  TF2 Build (títulos, abas, botões, rótulos, números): a fonte pedida pelo usuário.
             Quicksand (texto corrido e dados): leve e arredondada, perto da fonte secundária
             que o jogo usa em subtítulos e nomes. Duas famílias, ambas embutidas.
COR:         Fundo sépia, painéis marrom-escuro, texto creme. Destaque: amarelo-laranja
             (abas, botão principal, títulos de tabela). Equipes: RED e BLU, só em faixas e
             selos. Verde só para "ok" (o ponto "Online" do jogo).
DENSIDADE:   Densa nas listas, arejada nos painéis de resumo.
FORMA:       Cantos de 3 a 4 px, sombra dura de 2 px, faixas em paralelogramo (clip-path).
MOVIMENTO:   Quase nenhum: hover de botão e uma entrada curta do selo. Desligado em
             `prefers-reduced-motion`.
ASSINATURA:  O selo de estado em paralelogramo (como a faixa do placar), com ícone e palavra.
PROIBIDO:    Gradiente decorativo, vidro/blur, card dentro de card, arte ou logo da Valve,
             ícone sem função.
```

## Decisões que o contrato obriga

- **Estado nunca depende só de cor:** todo selo tem palavra e símbolo.
- **Dados reais ou nada:** sem número de exemplo apresentado como real.
- **Português do Brasil**, mantendo os termos que a comunidade usa em inglês (hitsound, killsound, HUD, casual).
- **Sem modo claro/escuro alternativo:** o menu do TF2 é escuro, e o app também.
- **Rodapé fixo com os créditos** (Casual Preloader, GameBanana), para o crédito estar sempre visível.

## Fonte TF2 Build: licença

- Arquivo `tf2build.ttf` (57 KB, 255 glifos), baixado de https://www.dafont.com/pt/tf2-build.font, autor listado: **Andrea Wicklund**. Está em `app/src/assets/fonts/`.
- A página do dafont **não mostra uma licença** no texto, e o pacote não traz arquivo de licença. A fonte imita a do jogo, que é da Valve.
- **Antes de publicar o projeto ou o instalador, confirmar a licença** (ou trocar por uma fonte livre parecida). Está listado como pendência em `CREDITS.md`.

## Testes de originalidade (a rodar na entrega)

| Teste | Pergunta |
|---|---|
| Troca de logo | Trocando o nome, a tela ainda serviria para outro app? |
| Vizinho | O ModBoy ou outro gerenciador poderia usar esta tela sem mudar nada? |
| Convergência | Fonte, paleta e layout são o que um modelo escolheria por padrão para "gerenciador de mods"? |
| Assinatura | Existe um detalhe repetido e memorável? (o selo em ângulo) |
| **Coerência com o jogo** | Colocada ao lado das capturas de referência, a tela parece da mesma família? |

## O que evitar

Vidro fosco, neon, cards idênticos em grade infinita, ícones genéricos sem função, texto de marketing, `Oops!`. Se um elemento não ajuda a decidir "instalo ou não instalo esse mod", ele não entra.

## Resultado da revisão 2 (Início, Catálogo, Instalado)

Conferido no navegador com dados reais, comparando com as capturas de referência do usuário.

| Teste | Resultado |
|---|---|
| Troca de logo | Passa: rótulos, selos e textos falam de instalação do jogo, mods e preloader. |
| Vizinho | Passa: outros gerenciadores de mods não usam a linguagem visual do menu do TF2. |
| Convergência | Passa: não há gradiente, vidro nem paleta de painel genérico; a paleta vem das capturas. |
| Assinatura | Passa: o selo em paralelogramo aparece nas três telas. |
| Coerência com o jogo | Passa: abas laranja, faixa vermelha, painéis escuros, botões creme, balão creme e caixas com contorno laranja e título na borda. |

**Slopscan:** 0 itens. A primeira versão (papel e carimbos) foi substituída por esta.

**Contraste (OKLCH → sRGB):** texto creme sobre painel 13,8:1; texto cinza (muted) sobre painel 7,9:1; amarelo-laranja sobre painel 8,9:1; texto escuro sobre amarelo-laranja 9,6:1; creme sobre vermelho 4,9:1; creme sobre azul 5,5:1; creme sobre o fundo sépia 4,9:1; creme sobre a aba inativa 7,6:1. **Foram corrigidos três pares que estavam abaixo de 4,5:1** (cinza sobre o fundo sépia, texto da aba inativa e creme sobre azul).

**Ficou de fora de propósito:** seletor nativo de pasta, selo real de compatibilidade (a ficha mostra "Sem análise" e diz por quê), janela abaixo de 920 px.
**Risco:** a licença da fonte TF2 Build (ver `CREDITS.md`).

### Ajuste fino (revisão 2.1)

Comparado de novo com as capturas, quatro diferenças foram corrigidas:

- **Listas em barras arredondadas separadas**, como a lista de amigos e o placar, em vez de linhas de tabela.
- **Título do painel numa faixa mais clara** no topo (como PLAY e FRIENDS).
- **Fundo sépia com luz no centro** (gradiente leve, como o menu). O creme sobre o ponto mais claro tem 4,51:1, então esse é o limite: não clarear mais o centro.
- **Ícones nos botões** (buscar, pasta, atualizar), desenhados em SVG próprio.

Contraste do texto nas barras de lista: normal 11,7:1, hover 9,3:1, selecionada 8,3:1.

## Arte (banner, logo, capa e miniaturas) — revisão 2.2

Pedido do usuário: usar capa, banner e ícones do jogo, com fundos meio opacos, sem matar a vibe do app. Referência sugerida: o SteamGridDB (jogo 10602).

**Decisão: o projeto não distribui nenhuma arte da Valve nem do SteamGridDB.** As imagens do TF2 são da Valve, e as do SteamGridDB são de fãs, em geral feitas em cima da arte da Valve, sem licença clara por imagem. Colocá-las dentro do app ou do repositório seria redistribuir arte alheia. O app faz isto no lugar:

| Imagem | De onde vem | Direitos |
|---|---|---|
| Banner, logo, pôster do TF2 | **Lidos em tempo de execução da instalação do Steam do próprio usuário** (`appcache/librarycache/440/`). Se o usuário tiver capas personalizadas no Steam (por exemplo, escolhidas pelo SteamGridDB), as dele têm prioridade (`userdata/*/config/grid/440_*.png`). | Nada é copiado para o projeto. A imagem nunca sai do computador do usuário. |
| Miniaturas e imagens dos mods | CDN do próprio GameBanana (`images.gamebanana.com`), com o nome do autor na ficha. | O app só exibe; não baixa nem guarda. |

**Como o fundo ficou:** a arte vira um fundo fixo, em sépia, escurecido (brilho 40% mais uma camada escura de 50%), e os painéis ficam a 93% de opacidade, então a arte aparece atrás sem atrapalhar a leitura. No banner do Início há uma camada escura de 72% sobre a imagem. Os valores foram escolhidos pelo **pior caso** (um pixel branco da arte atrás do texto), que dá cerca de 5,3:1 a 5,7:1 de contraste para o texto creme.

**Se a arte não existir** (Steam sem cache, jogo sem capa), o app segue sem imagens: o banner volta à faixa vermelha simples e o fundo fica o sépia chapado.

**Segurança das imagens:** só aceitamos URLs que comecem com `https://images.gamebanana.com/`; qualquer outro host é descartado no núcleo, e o app desktop libera só esse domínio na política de segurança de conteúdo (CSP). Arquivos de arte do Steam acima de 6 MB são ignorados.

**Privacidade:** ao mostrar as miniaturas, o app faz requisições de imagem ao CDN do GameBanana (o mesmo que o site usa). Sem miniaturas, não há essas requisições.

**Opcional, não feito:** baixar capas direto do SteamGridDB exigiria uma chave de API e verificar a licença de cada imagem. Fica para depois, se o usuário quiser.

## Revisão 3 — feedback do usuário sobre o app aberto (2026-11-21)

O usuário abriu o app e anotou três telas. O que mudou:

| Feedback | Mudança |
|---|---|
| "Fundo e banner se repetem, tente diferenciar" | O fundo da janela usa a versão **desfocada** do banner que o próprio Steam guarda (`library_hero_blur`, 192×62); o banner do Início continua nítido. Sem a versão desfocada, o app borra o banner por CSS. |
| "'Como achei' usa 'path'... cara de IA" | Textos reescritos em tom seco, de interface de jogo: **Path**, **Build**, **Detecção**, **Aplicar**, **Atualizar**, "Estado do TF2 e do Casual Preloader". Sem frases em primeira pessoa ("o que o fornecedor encontrou"). |
| "Um grid 5×5 seria melhor que a lista de lado; modlauncher do Deadlock é um exemplo" | Catálogo em **grade 5×5** (25 por página) com **categorias numa barra lateral** e a ficha numa **janela (modal)**. Cards grandes: imagem 16:10, nome em até 2 linhas, autor, categoria e curtidas. Itens já instalados ganham a faixa **Instalado**. Sons, que não têm imagem, usam cards baixos com ícone. |
| "Instalado: o que devo fazer aqui? Confuso, sem indicações" | A tela abre com **Como usar esta tela** (3 passos + botão para o Catálogo) e se divide em seções: **Instalados pelo app** (com Remover e estado vazio com atalho), **Instalados à mão** (só leitura, com explicação) e **Sistema** (arquivos do preloader e do jogo). O Início ganhou o painel **Próximo passo** com o botão para o Catálogo. |
| Logo `stf2s_icon.png` | Usada como ícone do app (janela, barra de tarefas, instalador) e na barra do topo. |

**Acessibilidade da janela de ficha:** Esc fecha, o foco fica preso dentro enquanto aberta e volta ao card ao fechar (testado).

**Aprendido:** dentro de um flex, `display: -webkit-box` vira bloco e o limite de linhas deixa de valer; o texto do card usa um elemento interno para o `line-clamp`.

## Revisão 4 — segunda rodada de feedback (2026-11-21)

| Feedback | Mudança |
|---|---|
| "Paginação desnecessária; rolar e ir carregando é melhor" | **Rolagem infinita**: a lista cresce de 25 em 25 quando o fim chega perto da tela. Sem botões Anterior/Próxima. O rodapé da lista mostra "125 de 32.717" ou "Fim da lista". A aba Catálogo fica montada ao trocar de aba, então filtros e itens carregados não se perdem. |
| "Bug visual: cards muito longos, deveriam ter tamanho fixo" | A imagem do card saiu do fluxo (`position: absolute`), então uma imagem alta nunca estica o card nem a linha. Medido: 125 cards, todos com 240 px de altura, nenhuma linha com alturas diferentes. |
| "Bug visual: erro mal espaçado" | O painel de erro (e o de "nada encontrado") agora é um painel normal, com faixa de título, e fica alinhado com as categorias. |
| "Busca 'demoknight' quebrou com 'EOF while parsing'" | O GameBanana devolveu uma resposta vazia por um instante. O núcleo agora **tenta de novo sozinho** (3 tentativas) em resposta vazia, erro de servidor (5xx) ou limite (429). Erros 4xx não repetem. Se tudo falhar, a mensagem é "O GameBanana devolveu uma resposta vazia. Tente de novo em instantes". |
| "Falta Weapons, as normais" | As armas ficam dentro das **subcategorias** (Skins › The Sniper › Sniper Rifle; Skins › Multi Class › Shotgun). A barra lateral virou uma **árvore** que abre sob demanda, com o caminho mostrado acima da grade. Filtrar por uma categoria-pai inclui as filhas. |
| "Botão de jogar?" | Botão verde **Jogar TF2** na barra do topo (como o "Find a game" do menu do jogo). Abre o jogo pelo Steam, que aplica as opções de inicialização que o usuário já configurou. Desabilitado se o jogo já está aberto ou não há instalação. |
| "Usar ícones do próprio jogo" | O app lê os **ícones de classe** (`class_icons/filter_*`) do VPK do TF2 do usuário e os usa na árvore de categorias (Demoman, Engineer, Heavy...) e o emblema do jogo em "Todas". Mesma regra da capa: lidos em tempo de execução, nada é distribuído. |
| Logo | O ícone da janela e da barra de tarefas vem do arquivo da logo. O executável v0.2 ainda tinha o ícone antigo; do v0.3 em diante é o novo. |

**Descobertas técnicas**
- A API de categorias só devolve as filhas de uma categoria se o `_idGameRow` **não** for enviado junto.
- A busca por nome do GameBanana devolve **15 por página** mesmo pedindo 25; o app usa o tamanho real e confere pelo total.
- `innerText` aplica `text-transform`: ao testar texto em maiúsculas pelo CSS, comparar sem diferenciar caixa.

## Revisão 5 — "o que este mod muda" com o ícone da classe (2026-11-21)

Pedido do usuário: ao ver um som, mostrar algo como "este som afeta o Demoman" com a **foto dele num quadradinho**, usando o que já está instalado.

- **Botão "Analisar arquivo"** na ficha (só baixa quando clicado; nada é instalado). Mostra, para cada arquivo `.zip`:
  - o **selo de compatibilidade** (Casual OK / Parcial / Só offline / Bloqueado), agora de verdade, calculado contra os arquivos originais do TF2 do usuário;
  - **"Afeta:"** com o quadradinho do ícone da classe (o mesmo ícone do jogo que já alimenta a árvore de categorias) e o nome em caixa alta;
  - **"O que muda"**: um grupo por assunto (Hitsound, Killsound, Voz do Demoman, Armas do Soldier, Passos, Texturas do Spy, HUD...), cada um com o quadradinho, a contagem e "X substituem o jogo · Y novos";
  - **"Sons"**: lista com botão de tocar (▶/⏸) em cada áudio, o rótulo e a marca "substitui o jogo" ou "som novo". O áudio já baixado é reaproveitado ao tocar de novo.
- Sem classe (ex.: Hitsound), o quadradinho mostra o ícone de alto-falante. Sem o jogo instalado, a tela diz que não deu para comparar e **não inventa números**.
- O bloco "Compatibilidade: sem análise" que ficava no topo da ficha foi removido (passava a contradizer o resultado abaixo).

**Como funciona (sem tag no GameBanana):** os caminhos dentro do zip são traduzidos em rótulos por regras (`replaces.rs`) e comparados com o índice dos arquivos do jogo, lido dos `tf2_*_dir.vpk` do usuário (`vanilla.rs`, ~0,6 s para ~todo o jogo, guardado em memória e refeito sozinho quando o jogo atualiza). O rótulo é **estimado pelo caminho**; o "substitui o jogo" é confirmado contra o índice.

**Política de segurança do app desktop:** liberado `media-src data:` para a prévia de áudio (o som vai como `data:` URL; confirmado na janela real).

## Revisão 6 — aba Casual (2026-11-21)

- Nova aba **Casual** entre Catálogo e Instalado. Mesma linguagem dos painéis do Início: cabeçalho de painel escuro, tabela `ledger`, selos em ângulo com palavra + símbolo (nunca só cor).
- **Estado do mod no casual** como selo: *No jogo* (verde), *Entra na próxima* (azul), *Sai na próxima* (laranja), *Fora do jogo* (neutro).
- **Barra de progresso** (`.meter`): laranja do TF2 enquanto roda, verde ao terminar, vermelha se falhar; mostra a etapa atual, o tempo e, se falhar, o motivo e o *Registro da operação* (recolhido).
- Todo o texto antes de uma ação que mexe no jogo diz **o que muda** (arquivos tocados, cópia do `gameinfo.txt`, como desfazer). Aplicar e Reverter sempre pedem confirmação; Adicionar não pede (não toca no jogo).
- Na ficha, **Adicionar ao Casual** fica entre *Analisar arquivo* e *Instalar em custom*. Sem o Casual configurado, a ficha mostra um link para a aba em vez de um botão inútil.
