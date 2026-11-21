# Direção visual

> **Revisão 3 (2026-11-21).** O visual de menu do TF2 (abas laranja, faixas em ângulo, botões creme, fonte do jogo) foi **trocado a pedido do usuário** por uma interface escura, calma e organizada, pensada para quem usa o app todo dia. A referência de organização foi o gerenciador de mods do Deadlock: menu lateral, telas bem separadas, nada gritando.

## Brief

| Item | Decisão |
|---|---|
| Público | Jogador de TF2 no PC. Abre o app para instalar um mod, conferir se o jogo está pronto e jogar. Não conhece `sv_pure`. |
| Ação principal | **Decidir "instalo esse mod ou não"** e ver, num relance, se o jogo está pronto. |
| Voz | Direta e curta. Diz o que acontece, não enfeita. Português do Brasil, com inglês e espanhol. |
| Modo | **Produto**: clareza, consistência e densidade média vencem. |
| Restrições | Tauri 2 + React, janela de desktop, tudo offline, nada da Valve no repositório. |
| Anti-referências | Visual de tema do jogo, gradiente roxo, vidro fosco, cartões dentro de cartões, tudo em caixa alta, cores saturadas. |

## Contrato de direção

```
TESE:        Uma bancada escura e calma: o conteúdo é o protagonista (capas, nomes, estados)
             e a interface fica quieta em volta dele.
MUNDO:       Ferramenta de oficina à noite: grafite quente, filetes finos, uma única luz laranja.
TIPOGRAFIA:  Fonte do sistema (Segoe UI Variable no Windows) para tudo: legível, nativa, sem
             download. Lilita One só no nome e no logo, é a assinatura. Títulos em peso 600,
             sem caixa alta; rótulos minúsculos de 11 a 12 px em caixa alta só para classificar.
COR:         Fundo cinza quente quase preto, superfícies em três degraus de luminosidade.
             UM acento: laranja dessaturado (botão principal, item ativo, foco, progresso).
             Estados em tons sóbrios (verde, âmbar, vermelho, azul) sempre com texto, não só cor.
DENSIDADE:   Média. Painéis com respiro, listas com filete entre linhas, cartões de tamanho fixo.
FORMA:       Cantos de 8 a 12 px, borda de 1 px, sem sombras (só na janela modal).
MOVIMENTO:   Só feedback de 120 ms em hover e a barra de progresso. Respeita reduzir movimento.
ASSINATURA:  A barrinha laranja no item ativo do menu e o nome do app em Lilita One.
PROIBIDO:    Texto inclinado, faixas em ângulo, caixa alta em título, sombra dura, arte da Valve.
```

## Estrutura

- **Menu lateral** com três grupos: Geral (Visão geral), Mods (Catálogo, Casual, Instalado) e Sistema (Configurações). Em janelas estreitas (até 1100 px) vira uma coluna só de ícones, com o nome no tooltip.
- **Conteúdo** com largura máxima de 1480 px, centralizado, com rolagem própria. O rodapé fica fixo.
- **Cada tela** abre com título e uma frase que diz para que serve. Seções em painéis; ações principais em laranja, as demais em cinza ou só com borda.

## Cores (OKLCH)

| Papel | Valor |
|---|---|
| Fundo | `oklch(0.175 0.008 60)` |
| Menu lateral | `oklch(0.145 0.008 60)` |
| Superfície 1, 2, 3 | `oklch(0.21 …)`, `oklch(0.25 …)`, `oklch(0.30 …)` (mesmo matiz 60) |
| Texto, apoio, discreto | `oklch(0.93 0.008 80)`, `oklch(0.73 0.012 75)`, `oklch(0.60 0.012 70)` |
| Acento | `oklch(0.74 0.12 55)` (texto sobre ele: `oklch(0.20 0.02 55)`) |
| OK, aviso, erro, info | `oklch(0.76 0.11 150)`, `oklch(0.80 0.11 80)`, `oklch(0.70 0.14 25)`, `oklch(0.74 0.08 240)` |

O texto de apoio tem contraste acima de 7:1 sobre as superfícies; o discreto serve só para rótulos e contagens.

## Arte do jogo

O app lê a capa e os ícones de classe da instalação do Steam do próprio usuário, só para reconhecer o jogo (capa no Início, ícones nas classes afetadas). Nada disso vai para o repositório, e as capturas de tela do README são tiradas com `SUNY_NO_GAME_ART=1`.
