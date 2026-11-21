# Roadmap

Em ordem. Cada etapa só começa quando a anterior está testada.

## 0. Fundação — feito
- Pesquisa e decisões (`RESEARCH.md`)
- `suny-core`: steam, gamebanana, scanner, logs, system (22 testes + 1 teste de rede)

## 1. Spikes de risco (antes de construir mais)
Ver tabela completa em `RESEARCH.md` §8. Os que bloqueiam o resto:
1. ✅ **Ponte do preloader**: funciona de ponta a ponta para mod de áudio, **com o som tocando no casual** (`RESEARCH.md` §8.1 e §8.2). Falta testar partículas, modelos, animações e QuickPrecache.
2. **Onde o TF2 grava minidump** e como o `-condebug` se comporta.
3. **Protocolo de 1 clique do GameBanana**.

## 1.5 Instalar em `custom` — feito (v0.2)
- ✅ Download seguro (só `gamebanana.com`, MD5, limite de 1 GiB, apaga o parcial se falhar)
- ✅ Instalador com manifesto em `custom/.suny/`, só remove o que criou, nunca sobrescreve o que não é dele, recusa com o jogo aberto, bloqueia executáveis e caminhos suspeitos, limites contra zip bomb
- ✅ Botões **Instalar em custom** (na ficha) e **Remover** (em Instalado), sempre com confirmação
- Limites conhecidos: só `.zip`; sem barra de progresso (a tela mostra "Baixando e instalando…"); `.rar` e `.7z` ainda não; sem selo de compatibilidade (falta o índice de arquivos do TF2)

## 1.6 Catálogo v2 — feito (v0.4)
- ✅ Rolagem infinita, cards de tamanho fixo, árvore de categorias (inclui armas), tentativa automática quando o GameBanana responde vazio
- ✅ Botão Jogar TF2 (abre pelo Steam) e ícones de classe lidos do jogo (VPK + VTF → PNG)

## 1.7 Prévia de sons e "o que isto substitui" — feito (v0.5)
Hoje o GameBanana **não tem tag** dizendo qual som/classe um pacote troca. Dá para **derivar**, e é o diferencial para áudio:
1. Ao abrir a ficha de um som, o botão **Analisar** baixa o `.zip` (só com o clique) e lista os caminhos **sem extrair**.
2. Os caminhos são comparados com os sons originais do TF2 (`sound/ui/hitsound.wav`, `sound/vo/demoman_*`, `sound/weapons/...`) e viram texto legível: "Hitsound", "Voz do Demoman (12 arquivos)", "Espada do Demoman".
3. **Prévia de áudio:** tocar os `.wav` de dentro do zip na própria ficha, antes de instalar.
4. Também é o que alimenta o selo de compatibilidade com o casual (mesmo índice de arquivos originais).
**Feito:** `vanilla.rs` (índice dos arquivos originais), `replaces.rs` (caminho → rótulo + classe), `analysis.rs` (análise do zip e prévia), comandos `analyze_file` e `preview_audio`, e a tela de análise com o ícone da classe.
**Limites conhecidos:** o rótulo é por regra de caminho (ex.: armas compartilhadas viram "Armas compartilhadas"); áudio acima de 12 MB não toca na prévia (tem a mensagem); só `.zip`; a análise baixa o arquivo para uma pasta temporária (cache de até 24 h, 12 arquivos, 300 MB por arquivo).

## 1.8 Casual pelo app — feito (v0.6)
Aba **Casual**: configurar o Casual Preloader, montar a lista de mods do casual, aplicar no jogo e reverter.
- **Configurar:** baixa a versão **fixada** (v2.3.1, 19.065.394 bytes, SHA-256 `e911e6d5…f397d3`) do GitHub de cueki, recusa se tamanho ou hash não baterem, extrai com proteção contra caminho perigoso e instala as bibliotecas Python pelo pip (sem a parte gráfica, PyQt6). Tudo em `%LOCALAPPDATA%\suny-tf2-supplier\casual`.
- **Adicionar:** botão *Adicionar ao Casual* na ficha. O app confere o zip (sem executável, sem caminho perigoso, sem partículas) e entrega ao importador do próprio preloader, com o **nome da pasta escolhido pelo app** (`Nome do mod (gbs123)`). A lista só muda; o jogo não é tocado.
- **Aplicar / Reverter:** só com o TF2 fechado, com confirmação, cópia do `gameinfo.txt` antes, barra de progresso (nunca anda para trás) e registro da operação se falhar.
- **Aviso das opções de inicialização:** lê (só leitura) o `localconfig.vdf` do Steam e avisa se o TF2 não abre com `+exec w/config.cfg`.
**Limites conhecidos:** mods com partículas (`.pcf`) são recusados (falta a tela de escolher partículas); só `.zip`; só preloader v2.3.1 testado; QuickPrecache (modelos) fica desligado e não foi testado; o app ainda não percebe sozinho que o jogo atualizou (a tela só lembra de aplicar de novo); download real do GitHub e `pip` real com internet ainda não foram exercitados fora dos testes locais.

## 1.9 v0.7: `.rar`/`.7z`, configurações, idiomas, atualizador e instalador — feito
- **`.rar` e `.7z`:** `archive.rs` extrai com o programa que existir (7-Zip, WinRAR/UnRAR ou o `tar` do Windows), com limite de tamanho/quantidade vigiado durante a extração, recusa de links simbólicos, e entrega um `.zip` que segue o caminho de segurança de sempre. Nada de RAR vai dentro do app (licença do UnRAR não combina com a GPL).
- **Estado do jogo ao vivo:** `game_state` (a cada 3 s). `hl2.exe` só conta como TF2 se estiver na pasta do jogo ou rodar com `-game tf`. O botão Jogar fica indisponível com o jogo aberto.
- **Ícones de classe uniformes:** o fundo opaco vira transparência (`to_glyph`).
- **Cards** em Instalado e na lista do Casual; **ficha grande** com galeria, descrição, números e tags.
- **Idiomas:** português, inglês e espanhol (`app/src/i18n`). Mensagens do núcleo são traduzidas na tela por modelos (`core.ts`); o que não é reconhecido aparece em português.
- **Configurações:** idioma, instalação do TF2, QuickPrecache, formatos, cache, atualizações.
- **Atualizador** (`update.rs`): só releases estáveis do repositório; junta o changelog; baixa o instalador e confere o SHA-256 publicado (recusa sem hash).
- **Instalador:** assistente NSIS (pt-BR, English, Español), instalação por usuário. Releases pelo GitHub Actions (`.github/workflows/release.yml`) ao enviar uma tag `vX.Y.Z`.
**Limites:** o instalador não é assinado (SmartScreen avisa); o hash vem do mesmo GitHub da release (integridade, não autenticidade: assinatura de código é o próximo passo); `.rar` real compactado só foi testado com arquivos montados à mão.

## 2. Núcleo que falta
- `library`: SQLite com mods instalados, manifesto de arquivos, perfis, ordem de carga
- **Casual:** escolher partículas, detectar update do jogo sozinho e reaplicar, perfil competitivo (sem partículas/modelos)
- Barra de progresso do download (eventos do Tauri) e suporte a `.rar`/`.7z`
- Índice de arquivos originais do TF2 (ler os `*_dir.vpk`) para o selo de compatibilidade ficar completo
- Suporte a `.7z` e `.rar` no scanner (decidir a lib depois de amostrar 200 mods)
- Sessões de jogo: ligar `-condebug`, detectar fim anormal, bisseção assistida

## 3. App (Tauri 2 + React)
- Tela de instalações do TF2 (lista + escolher pasta)
- Navegar no GameBanana (mods e sons, por categoria, com selo de compatibilidade)
- Biblioteca: ativar/desativar, ordem de carga, conflitos
- Perfis: custom direto, casual, competitivo (sem partículas/modelos)
- Tela de crash: sessão, suspeitos com confiança, exportar relatório
- Créditos e aviso de liga competitiva visíveis

## 4. Antes de publicar
- Confirmar licenças pendentes em `CREDITS.md`
- Instalador assinado, atualização automática
- Testar em TF2 limpo, com e sem OneDrive, Windows 10 e 11
