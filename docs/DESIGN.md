# Direção visual: "Sala de projeção"

> **Revisão 4 (design-lab).** Um redesenho do zero, feito como experimento numa branch própria. Troca a interface calma e plana da revisão 3 por uma interface imersiva: a arte dos mods é a luz do ambiente e a interface flutua em vidro por cima. A versão estável continua na `main`.

## Brief

| Item | Decisão |
|---|---|
| Público | Jogador de TF2 no PC. Abre o app para instalar um mod, conferir se o jogo está pronto e jogar. |
| Ação principal | **Descobrir um mod e instalar** sem esforço, com o botão de jogar sempre à mão. |
| Voz | Direta e curta. Diz o que acontece. Português do Brasil, com inglês e espanhol. |
| Modo | **Marca dentro de um produto**: o Início e o Catálogo são vitrine (ousadia alta); Instalado, Casual e Configurações são trabalho (calmos e densos). |
| Anti-referências | Gradiente roxo, vidro em tudo sem motivo, cartões iguais sem hierarquia, texto cinza ilegível sobre foto, movimento que atrapalha. |

## Contrato de direção

```
TESE:        O conteúdo é a luz do ambiente. A arte do mod em destaque preenche a janela,
             desfocada e em movimento lento; a interface é vidro por cima e some quando você
             precisa ler.
MUNDO:       Sala de projeção: escuro, uma fonte de luz que muda de slide, painel de comando
             flutuante.
TIPOGRAFIA:  Bricolage Grotesque (variável) em títulos, números e no nome do app: peso alto,
             espaçamento fechado, personalidade. Texto corrido na fonte do sistema
             (Segoe UI Variable): nativa, nítida, sem download. Rótulos pequenos em caixa alta
             só para classificar.
COR:         Base azul-tinta quase preta. UM acento âmbar (ações, item ativo, foco, progresso).
             A cor "viva" vem da arte do slide atual, não de decoração. Estados em verde, âmbar,
             vermelho e ciano, sempre acompanhados de texto.
DENSIDADE:   Variável. Início e Catálogo arejados e grandes; Instalado, Casual e Configurações
             em painéis de vidro mais densos.
FORMA:       Cantos grandes (20 a 30 px), filetes de 1 px com brilho no topo, vidro com blur.
MOVIMENTO:   Slide de fundo a cada 9 s com fade de 1,7 s e deriva lenta; entrada escalonada de
             cartões; pílula do menu que desliza; holofote que segue o mouse nos cartões;
             onda dos sons que dança ao passar o mouse. Tudo desliga com "reduzir movimento".
ASSINATURA:  O dock de vidro que se abre ao passar o mouse, com a pílula âmbar que desliza,
             e o herói do Início sincronizado com o fundo.
PROIBIDO:    Texto sobre imagem sem gradiente de leitura, vidro em cima de vidro sem blur,
             animação que repete sem motivo, conteúdo sensível como fundo.
```

## Estrutura

- **Dock** flutuante à esquerda (72 px, abre para 232 px ao passar o mouse ou receber foco). Cinco telas de mesma altura; a pílula âmbar desliza entre elas.
- **Barra do topo** de vidro, fixa: contexto da tela, estado do jogo (aberto ou fechado) e o botão **Jogar** sempre visível.
- **Fundo** em camadas: aurora (manchas de cor), arte do slide em blur com deriva, escurecimento, vinheta e grão de filme. No Início o fundo é nítido e claro; nas outras telas fica mais escuro e desfocado para a leitura.
- **Início:** herói com o destaque atual (título, autor, curtidas), faixa de estado do jogo em quatro blocos, prateleiras de mods e sons em alta, e os dados da instalação.
- **Catálogo:** cartões em 4:5 com a arte ocupando tudo e o texto sobre um gradiente de leitura. Sons sem imagem ganham uma forma de onda única por item.

## De onde vem a arte

Os slides são os mods mais curtidos do GameBanana (imagem grande da galeria) e, quando existe, a arte do jogo lida da instalação do Steam do próprio usuário. Tudo é buscado em tempo de execução e nunca entra no repositório. Itens que o GameBanana marca como sensíveis (`_sInitialVisibility` diferente de `show` ou com classificação de conteúdo) **nunca** viram fundo. Por isso as capturas de tela desta versão **não** vão para o repositório (mostrariam arte de autores e do jogo); ficam só na máquina de quem testa.

## Acessibilidade

- Contraste: o fundo recebe escurecimento e os painéis de vidro têm opacidade suficiente para o texto passar de 4,5:1 sobre qualquer slide.
- Foco visível em âmbar em tudo; o dock abre também por foco de teclado.
- `prefers-reduced-motion`: sem deriva, sem entrada escalonada, sem pulso; o fundo troca com um fade curto.
- O fundo e os efeitos são decorativos (`aria-hidden`); o carrossel do Início tem rótulo e cada ponto é um botão.
