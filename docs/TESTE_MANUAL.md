# Roteiro de teste manual (v0.7)

Executável: `G:\suny_tf2_supplier\release\suny-tf2-supplier-v0.6.exe` (interface embutida, não precisa de servidor).
Feche o app que estiver aberto antes de abrir esse: o Windows não deixa trocar um programa em uso. Os `.exe` antigos podem ser apagados.
Para gerar de novo: `npm --prefix app run build` e `cargo build --release -p suny-tf2-supplier --features custom-protocol`, depois copiar de `C:\KattyTarget\release\`.

**Antes de instalar ou remover mods:** feche o TF2. O app recusa com o jogo aberto.

## 1. Barra do topo e Início
- [ ] O ícone da janela e da barra de tarefas é a **sua logo** (e não o carimbo antigo). Se ainda aparecer o antigo, é cache do Windows: feche o app, desafixe e abra de novo.
- [ ] O botão verde **Jogar TF2** está ao lado do Build. **Clique só quando quiser abrir o jogo**: ele abre o TF2 pelo Steam, com as suas opções de inicialização. Fica cinza se o jogo já está aberto.
- [ ] Banner nítido no topo e fundo desfocado (não repetem a imagem). Path, Build, Detecção e Sistema. Painel **Próximo passo** com o botão do Catálogo.

## 2. Catálogo
- [ ] **Rolagem infinita**: ao rolar, mais cards aparecem sozinhos; o rodapé mostra "125 de 32.717". Não há botões de página.
- [ ] **Todos os cards têm o mesmo tamanho**, mesmo com imagens altas (ex.: busque `demoman`).
- [ ] A busca `demoknight` funciona. Se o GameBanana falhar, o app tenta de novo sozinho e só então mostra o erro, num painel alinhado.
- [ ] **Categorias em árvore**: clique na seta de **Skins** para abrir; as classes aparecem com os **ícones do jogo** (Scout, Demoman...). Abra uma classe para ver as **armas** (ex.: Skins › Sniper › Sniper Rifle). O caminho aparece acima da grade.
- [ ] Mods / Sons, busca e ordem funcionam. Clicar num card abre a ficha em janela (Esc fecha, o foco volta ao card).
- [ ] Item `.rar`/`.7z` mostra o aviso de formato não suportado. Itens já instalados pelo app têm a faixa verde **Instalado**.
- [ ] Trocar para Início e voltar ao Catálogo **mantém** os filtros e os itens já carregados.

## 3. Analisar arquivo (novo na v0.5)
Abra a ficha de um som, por exemplo **POOT Hitsound** (aba Sons, categoria Hitsound), e clique em **Analisar arquivo**.

- [ ] Aparece o selo **Casual OK** e a lista **O que muda** ("Hitsound · 1 arquivo · 1 substitui o jogo · 0 novos").
- [ ] Em **Sons**, o botão **▶** toca o `hitsound.wav` (é curtinho, 0,2 s). Clicar de novo toca outra vez na hora.
- [ ] Para ver o **ícone da classe**, abra um pacote de voz ou de armas de uma classe (ex.: busque `demoman` em Sons): aparece **"Afeta: [carinha do Demoman] DEMOMAN"** e o quadradinho dele em cada som daquela classe.
- [ ] Um pacote com `.exe` não oferece análise e mostra o bloqueio.
- [ ] Sem o TF2 encontrado, a análise avisa que não dá para comparar com o jogo (sem inventar números).

## 4. Casual (novo na v0.6)
Abra a aba **Casual**.

- [ ] Se aparecer **Configurar o Casual Preloader**: clique, leia o aviso e confirme. Ele baixa 19 MB do GitHub e instala as bibliotecas pelo pip (precisa de internet, leva alguns minutos). A barra mostra o andamento. **Se falhar, copie o texto vermelho e o *Registro da operação*.**
- [ ] Com o Casual pronto, abra um som no Catálogo (ex.: **POOT Hitsound**) e clique em **Adicionar ao Casual**. Aparece "Na lista do casual" e o jogo **não muda**.
- [ ] Na aba Casual o mod aparece com a caixa **Usar** marcada e o estado **Entra na próxima**. **Tirar** pede confirmação e só tira da lista.
- [ ] Com o **TF2 aberto**, *Aplicar no jogo* fica bloqueado com o aviso "TF2 aberto".
- [ ] Feche o TF2. **Aplicar no jogo** pede confirmação, mostra a barra (uns 30 s para áudio) e termina em "Pronto". O selo do topo vira **Aplicado no jogo** e o estado do mod vira **No jogo**.
- [ ] **Reverter ao original** tira o preloader do jogo (selo volta para "Jogo sem preloader").
- [ ] "Para o jogo abrir com os mods" mostra se `+exec w/config.cfg` já está nas opções de inicialização do Steam.
- [ ] Um mod com **partículas** é recusado com uma mensagem clara.
- Lembrete: o primeiro boot do jogo depois de aplicar demora vários minutos; clique em *Aguardar* se o Windows disser que não está respondendo.

## 5. Instalar e remover em custom
Mod sugerido: **POOT Hitsound** (Jotunn, 19 KB): aba **Sons**, categoria **Hitsound**, abrir o card.

- [ ] **Instalar em custom** mostra a confirmação antes de qualquer coisa.
- [ ] Confirmando, aparece "Instalado" e a pasta `POOT Hitsound (gbs16166)` em `tf/custom`, com `sound/ui/hitsound.wav`.
- [ ] O card ganha a faixa **Instalado**.

## 6. Instalado
- [ ] **Como usar esta tela** + botão "Ir ao Catálogo".
- [ ] **Instalados pelo app (1)** lista o POOT com o botão **Remover**; **Instalados à mão** lista o `Pink Charge` sem botão.
- [ ] **Remover** pede confirmação e apaga só a pasta do POOT.

Isso mexe na sua instalação real, mas só adiciona (e depois remove) uma pasta nova dentro de `custom`.
Funciona offline e em servidores de comunidade. No casual da Valve pode aparecer se o Casual Preloader estiver aplicado no jogo (visto num teste real com o Eyecrafter), mas o caminho suportado para o casual é *Adicionar ao Casual*.

## 7. O que me contar se algo estranho aparecer
- Uma captura de tela da janela, o texto exato de qualquer erro e o que você clicou antes.


## 8. Novidades da v0.7
- [ ] **Jogar TF2** fica cinza e vira "TF2 aberto" enquanto o jogo está aberto, e volta sozinho uns segundos depois de você fechar o jogo.
- [ ] Início: a linha "Estado do jogo" diz "aberto agora" ou "fechado" (com selo), sem ambiguidade.
- [ ] Configurações: trocar o idioma (Português / English / Español) muda a interface inteira na hora.
- [ ] Catálogo: abra um mod. A ficha é grande, tem setas e miniaturas nas imagens, clicar na imagem amplia, a descrição aparece e dá para ler mais.
- [ ] Um mod em **.rar** mostra os botões de analisar / adicionar ao Casual / instalar. Em Configurações, a linha .rar diz com qual programa abre.
- [ ] Instalado e Casual mostram **cards** com a imagem do mod.
- [ ] Os ícones das classes (Demoman, Heavy, Pyro...) têm todos o mesmo estilo.
- [ ] Configurações → Atualizações: *Verificar agora* diz se há versão nova. Com uma release estável nova no GitHub, aparece o aviso azul no topo, *Ver novidades* mostra o changelog e *Atualizar agora* baixa, confere o SHA-256 e abre o instalador.
- [ ] Instalador (`...-setup.exe`): abre o assistente, deixa escolher o idioma, instala sem pedir administrador e cria o atalho no menu Iniciar. Desinstalar pelo Windows remove tudo.
