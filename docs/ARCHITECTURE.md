# Arquitetura

```
┌────────────────────────── App (Tauri 2) ──────────────────────────┐
│  React + TypeScript (UI estilo TF2)                                │
│        │  comandos tipados (invoke) / eventos de progresso         │
│  src-tauri  (camada fina: janela, deep-link, instância única)      │
│        │                                                           │
│  crates/suny-core  (toda a lógica, testável sem janela)            │
│    steam      → achar Steam, bibliotecas, TF2, buildid, gameinfo   │
│    gamebanana → cliente tipado da API apiv11 + cache + download    │
│    scanner    → abre zip/7z/rar, classifica conteúdo, compat       │
│    library    → mods instalados, manifestos, perfis (SQLite)       │
│    installer  → staging → cópia atômica → desfazer por manifesto   │
│    logs       → console.log (-condebug), sessões, suspeitos        │
│    system     → SO, RAM, espaço em disco, processo do jogo         │
│    preloader  → baixa/verifica o Casual Preloader, fala com a ponte│
└───────────────────────────────┬────────────────────────────────────┘
                                │ JSON por linha (stdout)
                     bridge/suny_bridge.py  (Python embutido do preloader)
                                │ importa
                     core.services.install.InstallService  (cueki)
```

## Regras de projeto

1. **`suny-core` não conhece Tauri.** Tudo que importa tem teste sem abrir janela.
2. **Nunca executar nada de dentro de um mod.** Mod é dado, não programa.
3. **Todo arquivo que instalamos fica num manifesto** (caminho + hash + mod de origem). Desinstalar = remover só o que está no manifesto.
4. **Nunca sobrescrever arquivo do usuário** (`cfg/`, HUD editado) sem backup.
5. **Toda decisão do app tem motivo legível** (selo de compatibilidade, suspeito de crash, conflito).
6. **Dois caminhos de instalação**, escolhidos por perfil:
   - **Custom direto**: copia para `tf/custom/<mod>/` (offline, servidores de comunidade, HUD, configs).
   - **Via Casual Preloader**: entrega os mods ao preloader (casual da Valve).
7. **Falha de instalação reverte.** O app guarda um backup do que vai tocar antes de começar.

## Instalação em `custom` (v0.2)

```
GameBanana ──download_to──▶ staging (temp do sistema, apagado depois)
                              │  scanner::analyze  (bloqueia .exe/.dll/.bat... e caminhos suspeitos)
                              ▼
                 extrai em custom/.suny/tmp-<slug>/   (limites por arquivo e total)
                              │  só então move
                              ▼
        custom/<slug>/…   e/ou   custom/<arquivo>.vpk     +   custom/.suny/<slug>.json (manifesto)
```

- `<slug>` = nome limpo + `(gbm<id>)` ou `(gbs<id>)`; ids de mod e de som podem coincidir.
- O manifesto guarda mod, arquivo, MD5, versão, **autor, licença**, itens criados e a lista de arquivos.
- Remover lê o manifesto, valida cada item (um único nome, sem `..`) e apaga só isso. Um manifesto adulterado não consegue apagar fora de `custom`.
- Reinstalar a mesma coisa troca a pasta antiga pela nova; se a nova falhar, a antiga fica.

## Análise de arquivo (v0.5)

```
clique em "Analisar" ──▶ download_to (cache temporário: %TEMP%\suny-tf2-supplier\analysis, até 24 h / 12 arquivos)
                            │
        vanilla::index_for ─┤  índice dos arquivos ORIGINAIS (lê os tf2_*_dir.vpk do usuário; em memória)
                            ▼
              analysis::analyze_zip  (lê só os nomes do zip; nada é extraído)
                 ├─ scanner::analyze     → selo de compatibilidade
                 ├─ replaces::group      → "Voz do Demoman", "Hitsound"... + classe + substitui/novo
                 └─ lista de áudios       → preview_audio lê UM áudio do zip em cache (data URL, até 12 MB)
```

O nome do zip em cache vem só de tipo + número; a interface nunca manda caminho de disco. A prévia só abre `.wav`, `.mp3` e `.ogg` que existam dentro do zip analisado.

## Casual dentro do app (v0.6)

```
interface (React) ── call("casual_*") ──▶ commands::dispatch
                                              │  tarefas lentas: start_job(...)  →  casual_job (a interface consulta a cada 0,7 s)
                                              ▼
casual.rs ── Layout (%LOCALAPPDATA%\suny-tf2-supplier\casual)
   ├─ preloader\casual-preloader\   release oficial v2.3.1 (python.exe embutido + bibliotecas via pip)
   │      └─ userdata\data\mods\addons\<Nome do mod (gbs123)>\   a LISTA do casual
   ├─ meta\<pasta>.json            de onde veio cada mod (GameBanana)
   ├─ state.json                    mods marcados + o que foi aplicado em cada instalação do jogo
   ├─ backups\<timestamp>\          cópia do gameinfo.txt antes de aplicar/reverter (guarda 5)
   └─ bridge\suny_bridge.py         a ponte, gravada a partir do executável (include_str!)
          ▲  JSON por linha (stdin/stdout)  ▼
   Bridge::spawn → python.exe suny_bridge.py → import_zip | install | uninstall | status
```

- **Uma tarefa por vez** (`JOB`): configurar, adicionar, aplicar e reverter nunca rodam juntas. Pânico dentro da tarefa vira "falhou". A barra usa o maior progresso já visto (o preloader às vezes volta de 80% para 78%).
- **Nome da pasta:** vem do app (`installer::slug_parts`: nome limpo + `gbs`/`gbm` + id). Zips no formato `tf/sound/...` viravam todos "tf" no importador do preloader e se sobrescreviam; a ponte agora acha a pasta de mod (a mais rasa que o preloader reconhece, até 5 níveis) e importa com o nome do app.
- **Segurança:** o zip passa por `precheck_zip` antes de chegar ao Python (executável, `..`, `:`, partículas, 1 GiB descompactado, 30 mil entradas). Nomes de mod são validados (`valid_addon_name`) antes de apagar qualquer pasta. O download do preloader é conferido por tamanho exato e SHA-256 e o zip é extraído com `enclosed_name`, sem links simbólicos e sem fluxos alternativos (`:`).
- **Sem janela:** o Python roda com `CREATE_NO_WINDOW`; a saída vai para a barra e o registro.

## Modelo de dados (resumo)

- `Installation { path, app_id=440, build_id, is_64bit, gameinfo_modified }`
- `Mod { id, source(GameBanana id | local), name, kind, version, files[], compat, enabled, order }`
- `Profile { name, mods[], mode(custom|casual), competitive_safe }`
- `Session { started_at, ended_at, mods[], exit(clean|suspect|crash), evidence[] }`
- `Suspect { mod_id, confidence(high|medium|low), reason, evidence_line }`

## Fluxo "instalar mod do GameBanana"

1. Busca/URL → `Mod/{id}/ProfilePage` → escolhe o arquivo (`_aFiles`).
2. Baixa para `staging/` com progresso, confere **MD5** e **antivírus do GameBanana**.
3. `scanner` lista o conteúdo → bloqueia executáveis → classifica → calcula selo de compat.
4. Mostra o resumo ao usuário (tipo, selo, conflitos, aviso de competitivo).
5. Instala conforme o perfil e grava o manifesto.
6. Se o perfil é casual: marca "precisa passar pelo preloader" e mostra o passo `+exec w/config.cfg` + mapa offline.

## Ponte do preloader (contrato)

Uma mensagem JSON por linha.

App → ponte (stdin): `{"cmd":"install","tf_path":"...","addons":["a","b"],"options":{...}}`
Ponte → app (stdout): `{"event":"progress","pct":42,"msg":"..."}`, `{"event":"done"}`, `{"event":"error","msg":"...","trace":"..."}`

A ponte é descartável: se uma versão nova do preloader quebrar a API interna, só `bridge/` muda.
