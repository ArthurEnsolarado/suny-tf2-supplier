# Pesquisa — suny's tf2 supplier

> Pesquisa feita em 2026-11-21. Cada afirmação tem uma fonte e um nível de confiança:
> **[verificado]** = li na fonte ou testei na API; **[comunidade]** = relato de fórum/guia, pode estar desatualizado; **[a verificar]** = precisa de teste na máquina antes de codar em cima.

## 1. Resumo para decidir

1. **O pre-loader (GameBanana tool 19049) se chama _Casual Preloader_**, do **cueki**. É Python + PyQt6, licença **GPL-3.0**, versão mais nova **v2.3.1** (GitHub, 22/09/2026). A página do GameBanana ainda mostra v2.3.0.
2. **Ele não tem modo "headless" (sem janela)**. A linha de comando só tem `gui`, `print_sourcemods` e `reset`. Mas o código tem uma API Python limpa (`InstallService.install(...)`), então dá para chamá-lo por uma ponte, sem reescrever o que ele faz.
3. **Ele altera arquivos do jogo**: faz patch no `tf2_misc_dir.vpk` (ou equivalente), muda o `gameinfo.txt` e exige `+exec w/config.cfg` nas opções de inicialização. Por isso precisamos de **backup, restauração e detecção de "jogo modificado"** desde o dia 1.
4. **O GameBanana não tem campo "casual compatible" na API**. Na amostra que testei, `_aTags` veio vazio. A compatibilidade tem que ser **calculada por nós**, comparando os caminhos dos arquivos do mod com os caminhos do TF2 original.
5. **Áudio é um tipo separado no GameBanana** (`Sound`, 19.766 itens para TF2), com 50+ categorias (Hitsound, Killsound, Taunts, Announcer...). Não é só "Mod".
6. **Stack recomendada: Tauri 2 (Rust) + React + TypeScript**, com a ponte Python usando o Python embutido do próprio preloader. Detalhes na seção 6.

## 2. Como o TF2 carrega arquivos (e o que está certo no que você ouviu)

Você ouviu que "arquivos da Valve vêm primeiro". **Isso é meia-verdade.**

- **Em partida offline / servidor com `sv_pure 0`**: a pasta `tf/custom/*` é montada _antes_ dos VPKs da Valve no `gameinfo.txt`, então o custom ganha. **[a verificar no `gameinfo.txt` instalado — a gente vai ler esse arquivo em runtime, não depender da memória]**
- **Em servidor com `sv_pure` ligado (casual da Valve usa `sv_pure 1`)**: o servidor manda uma _whitelist_ de caminhos permitidos. O cliente confere o que tem na memória e, se achar um caminho fora da lista, **recarrega o arquivo original da Valve**. É aí que parece que "a Valve vem primeiro". **[verificado — explicação do cueki em `doc/docs/how_it_works.md`]**
- Níveis de `sv_pure`: 0 permite tudo; 1 permite só coisas na whitelist (ex.: skins de jogador); 2 não permite nada. **[comunidade]**

### O que o pre-loader faz para contornar isso

Segundo o autor, são **cinco** técnicas combinadas, todas dependendo de **"pré-carregar" num mapa offline antes de entrar no casual**: **[verificado]**

1. **Mudança no `gameinfo.txt`**: uma linha que desliga boa parte da checagem (libera materiais, modelos, animações, alguns elementos de interface e lightwarps).
2. **MD5 do VPK não é checado**: o jogo confere o _tamanho_ do arquivo dentro do VPK, mas não o hash. Então ele reescreve arquivos do VPK mantendo o tamanho (completa com bytes de enchimento).
3. **Persistência de materiais de VGUI**.
4. **Encadeamento de dependência de modelos**.
5. **Exclusão de diretórios aninhados**.

Consequências que o app precisa tratar:

| Fato | Impacto no app |
|---|---|
| Precisa carregar um mapa offline (`itemtest`) depois de instalar | O app deve gerar o config e avisar o usuário; opcionalmente iniciar o jogo já com os launch options certos |
| Atualização do TF2 desfaz o patch | Detectar mudança de build do jogo e mostrar "reaplicar mods" |
| O jogo precisa estar fechado | Detectar o processo `tf_win64.exe`/`hl2.exe` antes de instalar |
| A Valve pode corrigir isso a qualquer momento (o próprio autor avisa) | Avisar o usuário; nunca prometer que funciona |
| Instalação falha pelo meio pode deixar o jogo quebrado | O preloader já tem rotina de restauração; o app faz o seu backup extra e mostra "verificar arquivos no Steam" como último recurso |

### O que funciona / não funciona no preloader **[verificado, FAQ e troubleshooting oficiais]**

- **Funciona**: partículas, modelos, animações, elementos de VGUI, lightwarps, skyboxes, warpaints (até 14/06/2025) e sons do jogo (exceto os da lista negra).
- **Não funciona**: decals (previstos), conteúdo empacotado dentro de BSP (mapas de comunidade), sons da lista negra, configs (têm que ficar em `custom`), "no hats bgum" (existe uma versão compatível: GameBanana tool 20969), nomes customizados de materiais/modelos (só caminhos compatíveis com casual).
- **Regra de ouro dele**: os mods ficam **dentro do preloader**, **não** em `custom`. Só HUD e configs podem ficar em `custom`.
- **Ordem de carga**: animações por último, se houver modelo de arma customizado.
- **Se o jogo crashar do nada, o autor diz que quase sempre é um dos mods**, não o preloader. Isso é a base do nosso "log de crash por mod" (seção 5).

### Aviso ético/prático que o app deve mostrar

- **VAC**: o autor afirma que não há risco de VAC, porque tudo depende de pré-carregar offline e nada mexe no VAC. **[verificado, é a afirmação dele; o app não deve prometer segurança, só citar a fonte]**
- **RGL (liga competitiva)**: **partículas customizadas dão ban de 1 ano** na RGL. Texturas planas são permitidas. **[verificado, FAQ do preloader]** O app terá um aviso claro e um modo "perfil competitivo" que desmarca partículas e modelos.
- Alguns mods dão vantagem visual (ex.: chama transparente). O app não deve esconder isso; o rótulo "vantagem visual" fica visível.

## 3. Pre-loader: licença, versões e como integrar

- Repositório: `cueki/casual-pre-loader` (o antigo `sv-pure-stuff/casual-pre-loader` está parado em jul/2025). Licença **GPL-3.0**. 159 estrelas. Último push em 16/11/2026. **[verificado, API do GitHub]**
- Release v2.3.1 tem **`casual-preloader-v2.3.1.zip` (~19 MB)**, que inclui `python.exe` embutido (Python 3.12) e as dependências. O GameBanana publica o MD5 de cada arquivo (v2.3.0: `ce35bcec9bb60c227180a99bb9112b9d`). **[verificado]**
- Dependências Python: `PyQt6`, `cappa`, `more-itertools`, `packaging`, `platformdirs`, `PyGithub`, `rich`, `valve-parsers` (do próprio cueki). **[verificado]**
- API que queremos chamar: `core.services.install.InstallService.install(tf_path, selected_addons, on_progress, apply_particle_selections, disable_paint_colors, show_console_on_startup, fix_mdl_paths, skip_quickprecache, sourcemod)` e `uninstall(tf_path, sourcemod)`. Mods ficam em `<userdata>/mods/addons/<nome>/` e cada um pode ter um `mod.json` (`type`, `addon_name`, `description`, `contents`). **[verificado]**
- O preloader também embute o **QuickPrecache** (MIT, © 2024 GoopSwagger) e usa um fork do `studiomdl` (submódulo do cueki). **[verificado]** → vai para o `CREDITS.md`.

### Três formas de integrar (e a escolhida)

| Opção | Prós | Contras |
|---|---|---|
| **A. Só abrir a janela do preloader** | Trivial | Experiência dividida em dois programas; não é "melhor que o ModBoy" |
| **B. Reescrever tudo (PCF, VPK, sons) em Rust** | Sem Python | Meses de trabalho, bugs sutis ("erros que só aparecem meses depois", palavras do autor), e a gente teria de acompanhar cada mudança do jogo |
| **C. Ponte Python que chama o `InstallService` do preloader (escolhida)** | Reaproveita o que já funciona e é mantido; o projeto vira GPL-3 e dá os créditos | Depende de API interna; precisa fixar versão e testar a cada release |

**Como a ponte funciona (decisão C):**
1. O app baixa o zip oficial do preloader (GitHub release, conferindo o tamanho e o MD5 quando existir) para a pasta de dados do app. Nada do preloader é redistribuído dentro do nosso instalador.
2. Roda `bridge/suny_bridge.py` com o `python.exe` embutido do preloader, falando JSON por linha (stdout) com o app Rust.
3. A ponte importa `core.services.install`, mas **antes** ajusta `sys.argv` e usa o `userdata` portátil, porque o `core.config` do preloader lê a linha de comando ao ser importado. **[verificado no spike #1, §8.1]**

**Licença:** como a ponte importa código GPL-3.0 no mesmo processo, **o projeto inteiro é GPL-3.0-or-later**. Isso também resolve a atribuição.

## 4. GameBanana — API e download

Base testada: `https://gamebanana.com/apiv11/...` (sem chave, responde JSON, atrás do Cloudflare). **[verificado, chamadas reais]**

| Necessidade | Endpoint | Observação |
|---|---|---|
| Jogo TF2 | `Game/297/ProfilePage` | ID do TF2 = **297** |
| Listar mods | `Mod/Index?_aFilters[Generic_Game]=297&_sSort=Generic_MostDownloaded&_nPerpage=N` | **32.714 mods**; `_nPerpage` máx. **50** |
| Filtrar por categoria | `...&_aFilters[Generic_Category]=1090` | funciona (Effects → 1.358) |
| Escolher campos | `_csvProperties=_idRow,_sName,...` | reduz muito o tamanho |
| Detalhe e arquivos | `Mod/{id}/ProfilePage` | `_aFiles[]` traz `_sFile`, `_nFilesize`, `_sDownloadUrl`, **`_sMd5Checksum`**, `_sAvResult`, `_aAnalysisWarnings` |
| Áudio | `Sound/Index?_aFilters[Generic_Game]=297` | **19.766** itens |
| Categorias de áudio | `Sound/Categories?_idGameRow=297&_nPerpage=50` | devolve **no máximo 50** e **ignora `_nPage`** (testado: as páginas 1, 2 e 3 vêm idênticas). A lista a_to_z vai de "Achievement" a "Vote Sounds", então provavelmente está completa, mas **não consegui confirmar** (Hitsound 381, Killsound 2630, Taunts 2295...) |
| Categorias de mods | `Mod/Categories?_idGameRow=297&_nPerpage=50` | Raiz: Skins 7951, Maps 5371, GUIs 1644, Effects 1090, Game files 2774, Textures 587, Castaways 163, Prefabs 396, Decal Tool 17411, Serverside Weapons 39068 |
| Busca | `Util/Search/Results?_sSearchString=...&_idGameRow=297&_sModelName=Mod` | busca por nome, não por tag |
| Download | `https://gamebanana.com/dl/{fileId}` | MD5 dá para conferir depois de baixar |

**Pontos importantes:**

- **Não existe tag "casual compatible"** na API. Quem faz a "casual compatibility" é a convenção dos autores (caminhos iguais aos do TF2) e o `mod.json` que o preloader gera ao "exportar como VPK". Então o app **analisa o conteúdo** (seção 5.3).
- O GameBanana já faz análise antivírus e dá avisos (`contains_exe`, `nested_archive`). O app deve **mostrar esses avisos** e bloquear automaticamente arquivos com `.exe`/`.dll`/`.bat` dentro de mods de conteúdo. **[verificado: o próprio preloader aparece com `contains_exe`, é normal para ferramenta, não para skin]**
- Mande um `User-Agent` honesto (`suny-tf2-supplier/<versão> (+repositório)`), use cache local e respeite o limite. Não achei limite documentado; **[a verificar]** o comportamento em volume. O cabeçalho de resposta não expõe limite.
- **O ModBoy** é o instalador oficial de 1 clique do GameBanana (pacote winget `Gamebanana.Modboy`, v3.11, instalador Inno Setup). **[comunidade]** Relatos de falha: não abre depois do clique no site; erro `failed to execute crucial install command` ao apontar a pasta do TF2. **[comunidade, 1 thread]** Isso mostra onde ganhar: detecção da instalação correta, mensagens de erro úteis, desfazer limpo.
- Protocolo de 1 clique (botão "Install" no site): o GameBanana usa um esquema de URL registrado pelo ModBoy. **[a verificar]** como replicar com `tauri-plugin-deep-link`. Se não der para registrar o mesmo esquema sem conflito, o app oferece "colar link do GameBanana" e a busca interna.

## 5. Funcionalidades — o que a pesquisa diz sobre cada uma

### 5.1 Achar o TF2 (ou escolher a instalação)
- Crate **`steamlocate` 2.1.1** acha Steam, bibliotecas e jogos (usa `libraryfolders.vdf` + `appmanifest_440.acf`). AppID do TF2 = **440**. **[verificado: existe e está atual]**
- O app lista **todas** as instalações encontradas (várias bibliotecas, Linux/Proton) e deixa escolher; também aceita pasta manual. Valida pela presença de `tf/gameinfo.txt`.
- Também lê o `appmanifest` para saber o **buildid** do jogo (detectar update que desfez o patch).
- Linux: o preloader exige `studiomdl.exe` da versão Windows e usa Proton; fora do escopo da v1, mas a arquitetura não deve impedir.

### 5.2 Instalar áudio
- Áudio normal vai em `tf/custom/<mod>/sound/...` (arquivo solto ou VPK). Sons substituem os do jogo pelo mesmo caminho. **[comunidade/guia Steam sobre sons customizados]**
- **Hitsound/killsound** são o caso mais popular e dependem de `cfg` e `scripts/game_sounds_*.txt`. O app deve instalar o cfg junto e avisar sobre `tf_dingalingaling_*`.
- No casual, **o preloader também sabe lidar com "game sounds" (menos a lista negra)** — então áudio vai por dois caminhos: **custom direto** (offline/comunidade) ou **via preloader** (casual). O app decide pelo perfil.
- Arquivo `sound.cache` do jogo pode precisar ser regenerado/ignorado. **[a verificar]**

### 5.3 Compatibilidade (o "negócio ao redor disso")
Classificação calculada por nós, em camadas:
1. **Estrutural**: lista todos os caminhos do zip/rar/7z. Compara com o **índice de arquivos originais do TF2** (lido dos `*_dir.vpk` da instalação; o preloader já tem `valve-parsers` para isso). Caminho igual ao original → **"casual-compatible"**; caminho novo → **"só offline/comunidade"**; mistura → **"parcial"**, mostrando quais arquivos quebram.
2. **Tipo de conteúdo**: partícula (`.pcf`), modelo (`.mdl`), material (`.vmt/.vtf`), som, HUD (`resource/`, `scripts/`), mapa (`.bsp`), config (`.cfg`).
3. **Regras do preloader** (seção 2): BSP empacotado, decals e configs → avisar.
4. **Conflitos entre mods**: dois mods com o mesmo arquivo → mostrar quem ganha pela ordem de carga (o preloader já faz isso, e o nosso mostra a mesma coisa antes de instalar).
5. **Segurança**: executáveis, DLLs, scripts → bloqueia.

Resultado exibido como selo: `CASUAL OK`, `PARCIAL`, `SÓ OFFLINE`, `PRECISA REVISÃO`, com o motivo clicável.

### 5.4 Log detalhado de qual mod causou o crash
**O TF2 não gera relatório amigável.** O que dá para usar:
- **`-condebug`** nas opções de inicialização grava `tf/console.log`. **[comunidade]** Alternativa por cfg: `con_logfile`. Sem isso, não há log por padrão.
- **Minidump (`.mdmp`)**: não achei o caminho oficial no Windows. **[a verificar — Spike #3]**
- Mensagens úteis no console: `Error loading ...`, `Model ... not found`, `Could not load sound`, `Material ... not found`, erros de VPK.

**Estratégia do app (sem inventar o que não existe):**
1. Ligamos `-condebug` e lemos o `console.log` durante e depois do jogo.
2. Cada mod instalado tem um **manifesto** (lista de arquivos). Quando o log cita um caminho, o app descobre **a qual mod ele pertence**.
3. Antes de iniciar, o app grava uma **sessão** (mods ativos + hora). Se o processo do jogo morrer sem encerrar normal (e o log para de repente), a sessão vira **"crash suspeito"**, com os últimos arquivos carregados e os mods ligados a eles.
4. **Bisseção assistida**: "desligar metade dos mods e tentar de novo" para achar o culpado quando o log não ajuda.
5. Tudo guardado por sessão, exportável em um arquivo para pedir ajuda.
O app **deve dizer a confiança** de cada suspeita (ex.: "o log cita o arquivo do mod X" = alta; "X é o último mod ligado" = baixa).

### 5.5 Compatibilidade da máquina
- **`sysinfo` 0.37 (as versões 0.39+ exigem Rust 1.95)**: SO, RAM, CPU, processos (detectar o jogo aberto). GPU e DirectX: **[a verificar]** melhor via DXGI no Windows (crate `windows`) ou `wgpu`.
- Pontos de checagem úteis: TF2 tem executável de 64 bits (`tf_win64.exe`, **[a verificar]** nome e versão em uso), espaço livre em disco, WebView2 presente, permissão de escrita na pasta do jogo (Program Files), OneDrive sincronizando a pasta (o preloader avisa para não ficar em OneDrive).

## 6. Stack: linguagem, bibliotecas e por quê

### Decisão: **Tauri 2 + Rust + React + TypeScript**

| Critério | Tauri 2 (Rust+Web) | Electron | .NET (WPF/Avalonia) | Python + PyQt |
|---|---|---|---|---|
| Tamanho do instalador | ~5–10 MB | ~100 MB+ | ~20–70 MB | ~50–100 MB |
| Liberdade de design (estilo TF2) | **Total (CSS/SVG/fontes)** | Total | Boa, mais trabalhosa | Limitada (QSS) |
| Mexer em disco/Steam/processos | **Ótimo (Rust)** | Médio (Node) | Ótimo | Bom |
| Segurança (baixa mods da internet) | Boa (permissões por capability) | Média | Boa | Média |
| Fala com o preloader (Python) | Sidecar por JSON | Sidecar | Sidecar | Mesmo processo |
| Windows 11 já tem WebView2 | Sim | — | — | — |

Tauri vence por design (CSS puro para o visual TF2), tamanho e robustez do código que mexe em arquivos. **Atenção:** a linha `tauri` do crates.io mostra `3.0.0-alpha`, mas o CLI estável no npm é **2.12.6**. **Usamos Tauri 2.x estável** e travamos a versão.

### Bibliotecas escolhidas

**Rust (núcleo)**
| Crate | Para quê |
|---|---|
| `steamlocate` | Achar Steam e o TF2 |
| `keyvalues-parser` | Ler `appmanifest`/`gameinfo.txt`/VDF |
| `reqwest` (rustls) + `tokio` | Cliente do GameBanana, download com progresso |
| `serde` / `serde_json` | JSON da API e da ponte |
| `zip` + suporte a 7z/rar | Abrir arquivos de mod (GameBanana tem zip, rar e 7z) |
| `md-5` / `sha2` | Conferir MD5 do GameBanana |
| `rusqlite` | Biblioteca de mods, perfis, sessões e histórico de crash |
| `sysinfo` | Máquina e processo do jogo |
| `tracing` + `tracing-appender` | Logs do próprio app |
| `thiserror` | Erros tipados com mensagens úteis |
| `tauri-plugin-single-instance`, `tauri-plugin-deep-link`, `tauri-plugin-dialog` | Janela única, link do site, escolher pasta |

**Front-end**
React 19 + TypeScript + Vite 8, `zustand` (estado) e `@tanstack/react-query` (cache da API). Versões atuais do npm conferidas em 21/11/2026. Sem biblioteca de componentes pronta, de propósito: o visual é próprio (ver `DESIGN.md`).

**Ponte**: Python 3.12 do próprio preloader, sem dependências extras.

## 7. Riscos (ordenados)

1. **API interna do preloader muda** → fixar versão testada, tabela de compatibilidade, testes de contrato rodando a ponte contra cada release nova.
2. **Valve corrige as técnicas** → o app só orquestra; avisa de forma honesta; modo "custom puro" continua funcionando.
3. **Instalação interrompida deixa o jogo inconsistente** → backup antes, restauração, botão "reverter tudo", e instrução de verificar arquivos no Steam.
4. **Mod malicioso** (executável dentro de zip) → bloquear tipos de arquivo, mostrar avisos do GameBanana, nunca executar nada de dentro de mod.
5. **Limites/mudanças da API do GameBanana** → cache, `_csvProperties`, backoff, camada de adaptação com testes gravados.
6. **Perder o save/config do usuário** → nunca sobrescrever `cfg/` do usuário sem backup; manifesto por mod para desinstalar só o que instalamos.
7. **Ban em liga competitiva** → aviso e perfil competitivo.

## 8. Spikes — o que testar antes de construir o resto

| # | Pergunta | Como validar | Critério de sucesso |
|---|---|---|---|
| 1 | Dá para importar `core.services.install` sem abrir a janela e sem ler `sys.argv` errado? | Baixar v2.3.1, rodar um script curto com o Python embutido | Instala 1 mod de teste num TF2 de cópia — **✅ FEITO, ver §8.1** |
| 2 | O preloader respeita um `userdata` isolado? | Rodar com `--portable` / pasta própria | Nada escrito fora da pasta do app — **✅ FEITO** (modo portátil grava só em `<preloader>/userdata`) |
| 3 | Onde o TF2 grava minidump no Windows? | Forçar crash com mod inválido, procurar `.mdmp` | Caminho e formato confirmados |
| 4 | Como é o protocolo de 1 clique do GameBanana? | Ler o registro após instalar o ModBoy / inspecionar o botão | Esquema de URL e formato do parâmetro |
| 5 | Limite de requisições do GameBanana | Teste de carga leve com backoff | Número seguro de requisições por minuto |
| 6 | Formatos de arquivo em mods do TF2 (zip/rar/7z) | Amostrar 200 mods populares | % por formato → decide qual lib de rar/7z |

### 8.1 Resultado do spike #1 — a ponte funciona (2026-11-21)

**Teste:** release oficial v2.3.1 (19.065.394 bytes, igual ao tamanho publicado no GitHub; SHA-256 `e911e6d5…f397d3`), extraído numa pasta própria. Cópia mínima do TF2 (3,1 GB: `gameinfo.txt`, todos os `*_dir.vpk`, `tf2_misc_*.vpk`, `cfg`, `scripts`) numa pasta isolada. A instalação real do jogo foi só **lida** e conferida depois: nada mudou.

**O que funcionou, sem abrir janela, pelo `bridge/suny_bridge.py`:**
- `hello`: importa o preloader e devolve a versão (`2.3.1`) e as pastas de dados.
- `status`: detecta se o `gameinfo.txt` está modificado (o preloader comenta a linha `type multiplayer_only`).
- `install` de um mod de teste (um `hitsound.wav`): eventos de progresso de 0 a 100%; o som e os cfgs (`cfg/w/config.cfg`, `cfg/w/kitty.cfg`) foram parar no VPK `custom/_casual_preloader_*.vpk` (11 arquivos), e o `gameinfo.txt` ficou modificado. Levou ~22 s.
- `uninstall`: voltou o `gameinfo.txt` e limpou o `custom`. **Os `tf2_misc_*.vpk` ficaram byte a byte iguais aos de antes da instalação** (mesmo hash antes e depois). Isso é a garantia mais importante para o botão "reverter tudo".

**O que descobri e muda o projeto:**
1. **O Python embutido vem sem bibliotecas.** O `RUNME.bat` do preloader roda `pip install -r requirements.txt` a cada abertura. O nosso app precisa fazer essa instalação na primeira vez (sem o PyQt6, que o núcleo não usa) e guardar o resultado. Isso exige internet na configuração.
2. **A ponte precisa chamar `prepare_runtime_environment()` antes de instalar ou remover**, como a janela faz ao abrir. Sem isso o `install` falha com "Vanilla particle staging folder is missing files". Já está na ponte.
3. **`core.config` lê `sys.argv` na importação.** A ponte fixa `--no-update --no-migrate` antes de importar. Funciona.
4. **O `Sourcemods.DEFAULT` é um apelido do `TF2`** no código do preloader; a ponte passa `Sourcemods.TF2` explicitamente.
5. **O `progress` pode voltar atrás** (de 80% para 78% no meio da instalação). A barra do app deve usar o maior valor já visto.
6. **A instalação real do usuário já tinha o preloader aplicado** (linha comentada no `gameinfo.txt` e a pasta `zz_run_preloader_again_if_you_add_a_custom_hud_...` dentro de `custom`). O app precisa **detectar um preloader já instalado** e oferecer assumir o controle (ler o que está instalado) em vez de assumir que o jogo está limpo. O `status` da ponte já responde a primeira parte.
7. A instalação com `skip_quickprecache=true` não precisou de `bin/` nem de `studiomdl`. O caminho com QuickPrecache (modelos) **não foi testado**.

**O que este teste NÃO prova:**
- Só um mod de áudio. **Partículas, modelos, animações, HUD e skybox não foram testados**, então os problemas deles ainda podem aparecer.
- ~~Não foi aberto o jogo~~ → **testado depois, ver §8.2**.
- A compatibilidade com versões futuras do preloader não é garantida (API interna). A ponte foi testada só com a **v2.3.1**.

### 8.2 Resultado em jogo — mod de áudio funcionando no casual (2026-11-21)

**Teste:** o mod `Pink Charge` (18 sons do Demoman, um deles com 3 min 5 s) foi instalado pela ponte na instalação real, com o jogo fechado e backup verificado por hash antes. O usuário abriu o TF2 com `+exec w/config.cfg -condebug` e jogou uma partida casual.

**Resultado: o áudio tocou no casual** (confirmado pelo usuário, que jogou normalmente com outras pessoas).

**O que o teste mostrou e o app precisa tratar:**
1. **O primeiro boot demorou ~4 min 30 s** (00:46:21 até ~00:50:50). Durante esse tempo o Windows mostrou "tf_win64.exe não está respondendo". **Não era travamento**: o processo gastava CPU (33 s de CPU em 30 s) e o `console.log` crescia. O usuário fechou o jogo na primeira tentativa e o Windows registrou "Application Hang" (id 1002), que parece uma falha, mas foi o fechamento forçado. **Regra para o app:** avisar antes de abrir ("o primeiro boot com mods leva vários minutos; se aparecer 'não está respondendo', clique em Aguardar") e mostrar progresso.
2. **O `-condebug` funciona** e o `tf/console.log` dá para ler enquanto o jogo roda. O log mostra o fim do preload (`casual particle preloader ACTIVE!!` seguido de `Client Disconnect`). Isso serve de **marcador de "preload concluído"** para o app.
3. **Dá para distinguir "carregando" de "travado" de fora**, só lendo: `Responding` do processo, CPU consumida em uma janela e crescimento do log. Essa é a base da tela de sessão (§5.4). **Ainda não vimos um travamento real**, então o limite entre "lento" e "travado" continua sem calibrar.
4. **O log tem muito ruído do próprio preloader** (`Missing Vgui material vgui/../...`, `env_cubemap ...`, `proxy "clamp" not found`). O analisador de logs precisa ignorar esses padrões, senão toda sessão parecerá cheia de erros.
5. **Aviso de lentidão:** o tempo de boot a cada abertura (não só a primeira) ainda não foi medido.

**Ainda não testado em jogo:** partículas, modelos, animações, HUD, skybox e o caminho com QuickPrecache. A prova vale só para áudio de arma.

## 9. Fontes

- Casual Preloader no GameBanana: https://gamebanana.com/tools/19049 (API: `apiv11/Tool/19049/ProfilePage`)
- Repositório: https://github.com/cueki/casual-pre-loader (README, `doc/docs/how_it_works.md`, `faq.md`, `troubleshooting.md`, `tutorial.md`, `core/config.py`, `core/services/install.py`, `core/settings.py`, `core/services/addons.py`)
- Tutorial oficial: https://gamebanana.com/tuts/18436
- Guia Steam sobre mods no casual: https://steamcommunity.com/sharedfiles/filedetails/?id=3315213612
- Modern Casual Preloader (VScript, mais antigo): https://gamebanana.com/wips/79779
- Discussões sobre preload/sv_pure: https://www.teamfortress.tv/49650/preloading-custom-viewmodels-to-work-on-sv-pure-2
- Docs Tauri sidecar: https://v2.tauri.app/develop/sidecar
- Regras RGL (mods): https://docs.rgl.gg/rules/global/1003/#examples-of-not-allowed-mods
- ModBoy (listagem winget): https://wingetly.io/apps/gamebanana/modboy · thread Steam: https://steamcommunity.com/app/440/discussions/0/3768987637134361887
- Crates e npm: versões conferidas por `cargo search` / `npm view` em 21/11/2026.


## Casual contra custom (pesquisa de 2026-11-22)

Fonte: a documentação oficial do Casual Preloader (`cueki.github.io/casual-pre-loader`: *how it works*, *tutorial*, *faq*, *troubleshooting*). A página `Sv_pure` da wiki da Valve respondeu 403 e não foi lida.

- **Por que `custom` não vale no casual:** os servidores da Valve mandam uma lista de caminhos permitidos (`sv_pure`) e o cliente recarrega o padrão do jogo para o que não passa. O Preloader usa 5 desvios dessa verificação (linha do `gameinfo.txt`, hash de MD5 do VPK sem checagem, persistência de materiais por VGUI, cadeia de dependências de modelos, diretórios aninhados) e por isso modifica o jogo.
- **"Todos os seus mods devem ficar no Preloader, não em custom"**, com duas exceções: HUD (pode ficar em `custom`, o Preloader faz o mesmo) e configs.
- **Funciona no Preloader:** partículas, modelos, animações, VGUI, lightwarps, skyboxes, warpaints e sons do jogo (menos os da lista negra). **Não funciona:** decalques, **mapas** (conteúdo empacotado em `.bsp`), sons da lista negra e nomes de material/modelo fora dos caminhos do TF2.
- **Animações:** funcionam, e o pacote de animações vai por último na ordem de carga.
- **Ordem de carga:** em conflito, o número maior vence. A ordem alfabética de `custom` (onde o primeiro vence, usada na aba Instalado) e a ordem de carga do Preloader são coisas diferentes.
- **`+exec w/config.cfg`** é obrigatório. O primeiro carregamento passa por um mapa offline.
- **Depois de atualizar o TF2**, rodar o Preloader de novo.
- **Risco:** partículas customizadas podem dar banimento em ligas competitivas (a FAQ cita 1 ano). O app já recusa partículas dentro de `.vpk`.
- **API do GameBanana (conferida):** `_aFilters[Generic_Name]=contains,<texto>` funciona no `Index` junto com ordem e categoria; as categorias trazem `_nCategoryCount` (quantas filhas); não existe filtro por tag.
- **Retratos das classes:** `materials/vgui/class_portraits/<classe>.vtf` (256 px, também `_blue` e `_grey`) em `tf2_textures_dir.vpk`.
