# Créditos

**suny's tf2 supplier** só existe por causa do trabalho de outras pessoas. Obrigado.

## Ferramentas usadas

| Projeto | Autor | Licença | Como usamos |
|---|---|---|---|
| [**Casual Preloader**](https://gamebanana.com/tools/19049) ([GitHub](https://github.com/cueki/casual-pre-loader)) | **cueki** e colaboradores | GPL-3.0 | Motor de instalação para mods no casual da Valve. Baixado do release oficial e chamado por uma ponte; não é copiado para dentro do nosso instalador. |
| [**valve-parsers**](https://github.com/cueki/valve-parsers) | cueki | ver repositório | Leitura/escrita de VPK e PCF (dependência do preloader). |
| **QuickPrecache** | GoopSwagger | MIT | Embutido no preloader. |
| [**studiomdl** (fork)](https://github.com/cueki/studiomdl) | cueki | ver repositório | Usado pelo preloader no Linux. |
| [**VPKEdit**](https://github.com/craftablescience/VPKEdit) | craftablescience | ver repositório | Citado pelo autor do preloader como base para estudar VPK/PCF. |
| [**GameBanana**](https://gamebanana.com) | equipe GameBanana | — | Fonte dos mods e sons, via API pública. Cada mod mostra o autor e leva ao link original. |
| **ModBoy** | GameBanana | — | Inspiração e referência do que melhorar. Não usamos o código. |
| [**TF2 Build**](https://www.dafont.com/pt/tf2-build.font) (fonte) | Andrea Wicklund (autoria listada no dafont); imita a fonte do jogo, que é da Valve | **Não informada** | Títulos, abas, botões e rótulos da interface. Arquivo `app/public/fonts/tf2build.ttf`, **fora do repositório** (o git ignora). Sem o arquivo, o app usa uma fonte reserva. As versões publicadas pelo GitHub Actions saem sem a fonte. **Ver pendência abaixo.** |
| [**Bricolage Grotesque**](https://fonts.google.com/specimen/Bricolage+Grotesque) (via `@fontsource-variable/bricolage-grotesque`) | The Bricolage Grotesque Project Authors (Mathieu Triay) | OFL-1.1 | Títulos e números da interface. |
| [**Lucide**](https://lucide.dev) (`lucide-react`) | Lucide Icons and Contributors (baseado no Feather, de Cole Bemis) | ISC | Todos os ícones da interface (abas, botões, selos, categorias). Texto da licença em `THIRD_PARTY_LICENSES/`, que também vai dentro do instalador. |
| Logo do app (`stf2s_icon.png`) | O usuário do projeto (criada por ele) | — | Ícone da janela e do instalador, e emblema na barra do topo. Arquivos gerados em `app/src-tauri/icons/`. |
| Arte do TF2 (banner, logo, capa) | Valve Corporation | Todos os direitos reservados | **Não incluída no projeto.** O app lê do Steam do usuário em tempo de execução. Capas personalizadas do usuário (por exemplo, do [SteamGridDB](https://www.steamgriddb.com)) também são lidas da pasta dele. |
| Ícones de classe do TF2 | Valve Corporation | Todos os direitos reservados | **Não incluídos no projeto.** O app decodifica, em tempo de execução, as texturas `class_icons/filter_*` do `tf2_textures_dir.vpk` do próprio TF2 do usuário. |
| Imagens dos mods | Autores de cada mod no GameBanana | Conforme cada mod | Exibidas a partir do CDN do GameBanana, com o autor indicado na ficha. Não são baixadas nem guardadas. |
| [Tauri](https://tauri.app), React, Vite, e as bibliotecas Rust listadas em `Cargo.toml` | seus autores | MIT/Apache-2.0 | Base do app. |

**Pendente antes da primeira versão pública:**
- Confirmar a licença exata de `valve-parsers`, `studiomdl` e `VPKEdit`.
- **Confirmar a licença da fonte TF2 Build.** A página do dafont não mostra licença e o pacote não traz arquivo de licença. Se não for permitida a redistribuição, trocar por uma fonte livre parecida ou fazer o app carregar a fonte da própria instalação do jogo.

## Autores de mods

O app **nunca** esconde quem fez o mod: nome do autor, link da página no GameBanana e licença (quando houver) aparecem na página do mod e no manifesto de instalação.

## Aviso

Team Fortress 2 é marca da Valve Corporation. Este projeto não é afiliado nem endossado pela Valve nem pelo GameBanana. Mods que alteram o visual podem ser proibidos em ligas competitivas (por exemplo, a RGL proíbe partículas customizadas). Use por sua conta e risco.


## Casual Preloader dentro do app (v0.6)

O app baixa o release oficial **v2.3.1** do Casual Preloader (cueki, GPL-3.0, https://gamebanana.com/tools/19049 e https://github.com/cueki/casual-pre-loader) para a pasta de dados do usuário, confere tamanho e SHA-256, e usa o código dele (importar mods, aplicar e reverter) pela ponte `bridge/suny_bridge.py`. Nada do preloader é redistribuído dentro do instalador do app. As bibliotecas Python dele (cappa, rich, PyGithub, valve-parsers etc.) são instaladas pelo pip a partir do PyPI, cada uma com a própria licença.
