// Mensagens que vêm do núcleo Rust (escritas em português) traduzidas para a tela.
//
// O núcleo continua sendo a fonte do texto em português. Aqui cada mensagem conhecida vira um modelo com
// `{lugares}` para os pedaços que mudam (nomes, números, mensagens do sistema). O que não for reconhecido
// aparece como o núcleo mandou: nunca some informação por falta de tradução.

export type CoreLang = "pt" | "en" | "es";

interface Rule {
  re: RegExp;
  names: string[];
  en: string;
  es: string;
}

const escapeRe = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/** `rule("não achei {x}", "not found {x}", "no encontré {x}")` */
function rule(pt: string, en: string, es: string): Rule {
  const names: string[] = [];
  const source = pt
    .split(/(\{\w+\})/)
    .map((part) => {
      const m = /^\{(\w+)\}$/.exec(part);
      if (!m) return escapeRe(part);
      names.push(m[1] ?? "");
      return "([\\s\\S]+?)";
    })
    .join("");
  return { re: new RegExp(`^${source}$`), names, en, es };
}

const RULES: Rule[] = [
  // ----- erros do núcleo (error.rs)
  rule("não achei o Steam neste computador: {x}", "couldn't find Steam on this computer: {x}", "no encontré Steam en este equipo: {x}"),
  rule("a pasta {p} não parece ser a do Team Fortress 2 (falta tf/gameinfo.txt)", "the folder {p} doesn't look like Team Fortress 2 (tf/gameinfo.txt is missing)", "la carpeta {p} no parece ser la de Team Fortress 2 (falta tf/gameinfo.txt)"),
  rule("erro de arquivo em {p}: {x}", "file error at {p}: {x}", "error de archivo en {p}: {x}"),
  rule("o GameBanana respondeu com erro ({c}): {x}", "GameBanana replied with an error ({c}): {x}", "GameBanana respondió con un error ({c}): {x}"),
  rule("falha de rede: {x}", "network failure: {x}", "fallo de red: {x}"),
  rule("resposta inesperada do GameBanana: {x}", "unexpected reply from GameBanana: {x}", "respuesta inesperada de GameBanana: {x}"),
  rule("arquivo de mod inválido: {x}", "invalid mod file: {x}", "archivo de mod no válido: {x}"),
  rule("o MD5 do arquivo não bate (esperado {a}, obtido {b})", "the file's MD5 doesn't match (expected {a}, got {b})", "el MD5 del archivo no coincide (esperado {a}, obtenido {b})"),
  rule("o app recusou este arquivo por segurança: {x}", "the app refused this file for safety: {x}", "la app rechazó este archivo por seguridad: {x}"),
  rule(
    "não achei nada para instalar neste arquivo: ele não tem pastas do jogo (models, sound, materials, resource...) nem .vpk",
    "nothing to install in this file: it has no game folders (models, sound, materials, resource...) and no .vpk",
    "no hay nada que instalar en este archivo: no tiene carpetas del juego (models, sound, materials, resource...) ni .vpk",
  ),
  rule("já existe '{n}' na pasta custom e não foi criado por este app. Nada foi alterado", "'{n}' already exists in the custom folder and wasn't created by this app. Nothing was changed", "ya existe '{n}' en la carpeta custom y no lo creó esta app. No se cambió nada"),
  rule("formato não suportado: {x}. O app abre .zip, .rar e .7z", "unsupported format: {x}. The app opens .zip, .rar and .7z", "formato no admitido: {x}. La app abre .zip, .rar y .7z"),
  rule(
    "para abrir arquivos {f} o app usa o 7-Zip, o WinRAR ou o tar do Windows, e nenhum foi encontrado neste computador. Instale o 7-Zip (gratuito, 7-zip.org) e tente de novo",
    "to open {f} files the app uses 7-Zip, WinRAR or Windows' tar, and none was found on this computer. Install 7-Zip (free, 7-zip.org) and try again",
    "para abrir archivos {f} la app usa 7-Zip, WinRAR o el tar de Windows, y no se encontró ninguno en este equipo. Instala 7-Zip (gratis, 7-zip.org) e inténtalo de nuevo",
  ),
  rule("o arquivo passa do limite de segurança de tamanho ({x})", "the file exceeds the safety size limit ({x})", "el archivo supera el límite de tamaño de seguridad ({x})"),
  rule("o Team Fortress 2 está aberto. Feche o jogo antes de instalar ou remover mods", "Team Fortress 2 is open. Close the game before installing or removing mods", "Team Fortress 2 está abierto. Cierra el juego antes de instalar o quitar mods"),
  rule("nome inválido: {x}", "invalid name: {x}", "nombre no válido: {x}"),
  rule("o GameBanana devolveu uma resposta vazia. Tente de novo em instantes", "GameBanana returned an empty reply. Try again in a moment", "GameBanana devolvió una respuesta vacía. Inténtalo de nuevo en unos instantes"),

  // ----- comandos
  rule("endereço não permitido", "address not allowed", "dirección no permitida"),
  rule("não consegui abrir o Steam: {x}", "couldn't open Steam: {x}", "no pude abrir Steam: {x}"),
  rule("argumentos inválidos: {x}", "invalid arguments: {x}", "argumentos no válidos: {x}"),
  rule("escolher pasta pela janela só existe no Windows", "the folder picker only exists on Windows", "elegir carpeta con la ventana solo existe en Windows"),
  rule("Este item não tem um arquivo pequeno o bastante para a prévia. Abra a ficha e use Analisar.", "This item has no file small enough for the preview. Open its page and use Analyze.", "Este elemento no tiene un archivo lo bastante pequeño para la vista previa. Abre su ficha y usa Analizar."),
  rule("Este pacote não tem áudio que dê para tocar (.wav, .mp3 ou .ogg de até 12 MB).", "This package has no playable audio (.wav, .mp3 or .ogg up to 12 MB).", "Este paquete no tiene audio reproducible (.wav, .mp3 u .ogg de hasta 12 MB)."),
  rule("comando desconhecido: {x}", "unknown command: {x}", "comando desconocido: {x}"),
  rule("esse arquivo não existe nesse item do GameBanana", "that file doesn't exist in this GameBanana item", "ese archivo no existe en este elemento de GameBanana"),
  rule("o GameBanana avisou que este arquivo contém executável", "GameBanana warned that this file contains an executable", "GameBanana avisó de que este archivo contiene un ejecutable"),
  rule("O TF2 já está aberto.", "TF2 is already open.", "TF2 ya está abierto."),
  rule("pasta não permitida", "folder not allowed", "carpeta no permitida"),
  rule("Analise o arquivo antes de ouvir os sons.", "Analyze the file before listening to the sounds.", "Analiza el archivo antes de escuchar los sonidos."),
  rule("Já existe uma operação em andamento: {t}. Espere terminar.", "An operation is already running: {t}. Wait for it to finish.", "Ya hay una operación en curso: {t}. Espera a que termine."),
  rule("Espere a operação em andamento terminar.", "Wait for the running operation to finish.", "Espera a que termine la operación en curso."),
  rule("Configure o Casual Preloader primeiro, na aba Casual.", "Set up the Casual Preloader first, in the Casual tab.", "Configura primero el Casual Preloader, en la pestaña Casual."),
  rule("Adicionar \"{n}\" ao casual", "Add \"{n}\" to Casual", "Añadir \"{n}\" a Casual"),
  rule("Configurar o Casual Preloader", "Set up the Casual Preloader", "Configurar el Casual Preloader"),
  rule("Baixar a atualização", "Download the update", "Descargar la actualización"),
  rule("Baixando a atualização… {a} de {b} MB", "Downloading the update… {a} of {b} MB", "Descargando la actualización… {a} de {b} MB"),
  rule("Baixando a atualização…", "Downloading the update…", "Descargando la actualización…"),
  rule("Abrindo o instalador…", "Opening the installer…", "Abriendo el instalador…"),
  rule("Não há atualização com instalador publicado.", "There's no update with a published installer.", "No hay ninguna actualización con instalador publicado."),
  rule("não consegui consultar as atualizações: {x}", "couldn't check for updates: {x}", "no pude consultar las actualizaciones: {x}"),
  rule("o GitHub respondeu {s} ao consultar as atualizações", "GitHub replied {s} when checking for updates", "GitHub respondió {s} al consultar las actualizaciones"),
  rule("resposta inesperada do GitHub: {x}", "unexpected reply from GitHub: {x}", "respuesta inesperada de GitHub: {x}"),
  rule("versão atual inválida: {x}", "invalid current version: {x}", "versión actual no válida: {x}"),
  rule("A release não publicou o SHA-256 do instalador, então o app não instala sozinho. Baixe pelo site do projeto.", "The release didn't publish the installer's SHA-256, so the app won't install it by itself. Get it from the project's site.", "La release no publicó el SHA-256 del instalador, así que la app no lo instala sola. Descárgalo desde el sitio del proyecto."),
  rule("o instalador tem um tamanho inesperado ({n} bytes). Recusado.", "the installer has an unexpected size ({n} bytes). Refused.", "el instalador tiene un tamaño inesperado ({n} bytes). Rechazado."),
  rule("não consegui baixar a atualização: {x}", "couldn't download the update: {x}", "no pude descargar la actualización: {x}"),
  rule("o download não bate com o SHA-256 publicado na release ({h}). Nada foi aberto.", "the download doesn't match the SHA-256 published with the release ({h}). Nothing was opened.", "la descarga no coincide con el SHA-256 publicado en la release ({h}). No se abrió nada."),
  rule("não consegui abrir o instalador: {x}", "couldn't open the installer: {x}", "no pude abrir el instalador: {x}"),
  rule("tar do Windows", "Windows tar", "tar de Windows"),
  rule("Aplicar mods no jogo", "Apply mods to the game", "Aplicar mods al juego"),
  rule("Reverter o jogo ao original", "Revert the game to original", "Revertir el juego al original"),

  // ----- instalador, scanner, análise, download
  rule("'{n}' não foi instalado por este app", "'{n}' wasn't installed by this app", "'{n}' no lo instaló esta app"),
  rule("manifesto de '{n}' não confere", "the manifest of '{n}' doesn't match", "el manifiesto de '{n}' no coincide"),
  rule("arquivos perigosos ou com caminho suspeito: {x}", "dangerous files or suspicious paths: {x}", "archivos peligrosos o con rutas sospechosas: {x}"),
  rule("um arquivo do mod passa de {n} bytes", "a file in the mod is over {n} bytes", "un archivo del mod supera {n} bytes"),
  rule("o mod descompactado passa de {n} bytes", "the unpacked mod is over {n} bytes", "el mod descomprimido supera {n} bytes"),
  rule("item inválido no manifesto: {x}", "invalid item in the manifest: {x}", "elemento no válido en el manifiesto: {x}"),
  rule("nome inválido no zip: {x}", "invalid name in the zip: {x}", "nombre no válido en el zip: {x}"),
  rule("nome de arquivo inválido no zip: {x}", "invalid file name in the zip: {x}", "nombre de archivo no válido en el zip: {x}"),
  rule("{a} arquivo(s) perigoso(s) ou com caminho inseguro", "{a} dangerous file(s) or unsafe path(s)", "{a} archivo(s) peligroso(s) o con ruta insegura"),
  rule("contém mapa (.bsp): conteúdo empacotado no mapa não passa pelo preloader", "contains a map (.bsp): content packed inside the map doesn't go through the preloader", "contiene un mapa (.bsp): el contenido empaquetado en el mapa no pasa por el preloader"),
  rule("sem modelos ou sons com nome próprio: nada que dependa do caminho original", "no models or sounds with their own names: nothing depends on the original path", "sin modelos ni sonidos con nombre propio: nada depende de la ruta original"),
  rule("{n} modelo(s)/som(ns) usam caminhos do TF2 original", "{n} model(s)/sound(s) use original TF2 paths", "{n} modelo(s)/sonido(s) usan rutas del TF2 original"),
  rule("{n} modelo(s)/som(ns) com caminho que não existe no TF2 original", "{n} model(s)/sound(s) with a path that doesn't exist in original TF2", "{n} modelo(s)/sonido(s) con una ruta que no existe en el TF2 original"),
  rule("{a} de {b} modelo(s)/som(ns) usam caminhos do TF2; o resto só funciona offline", "{a} of {b} model(s)/sound(s) use TF2 paths; the rest only works offline", "{a} de {b} modelo(s)/sonido(s) usan rutas del TF2; el resto solo funciona sin conexión"),
  rule("falta o índice de arquivos do TF2 para comparar os caminhos", "the TF2 file index is missing, so paths can't be compared", "falta el índice de archivos del TF2 para comparar las rutas"),
  rule("partículas customizadas: proibidas em ligas como a RGL", "custom particles: banned in leagues like RGL", "partículas personalizadas: prohibidas en ligas como RGL"),
  rule("só dá para ouvir arquivos .wav, .mp3 e .ogg", "only .wav, .mp3 and .ogg files can be played", "solo se pueden escuchar archivos .wav, .mp3 y .ogg"),
  rule("esse arquivo não está no zip", "that file isn't in the zip", "ese archivo no está en el zip"),
  rule("o áudio tem {a} MB; a prévia vai até {b} MB", "the audio is {a} MB; the preview goes up to {b} MB", "el audio pesa {a} MB; la vista previa llega hasta {b} MB"),
  rule("o áudio passou do limite durante a leitura", "the audio went over the limit while reading", "el audio superó el límite durante la lectura"),
  rule("endereço de download fora do GameBanana: {x}", "download address outside GameBanana: {x}", "dirección de descarga fuera de GameBanana: {x}"),
  rule("passou de {n} bytes durante o download", "went over {n} bytes during the download", "superó {n} bytes durante la descarga"),
  rule("O TF2 está aberto. Feche o jogo antes de instalar ou remover mods.", "TF2 is open. Close the game before installing or removing mods.", "TF2 está abierto. Cierra el juego antes de instalar o quitar mods."),
  rule("Pouco espaço livre no disco do jogo ({n} MB).", "Low free space on the game's disk ({n} MB).", "Poco espacio libre en el disco del juego ({n} MB)."),
  rule("Pouca memória ({n} MB). Mods de modelo e partícula podem pesar.", "Low memory ({n} MB). Model and particle mods can be heavy.", "Poca memoria ({n} MB). Los mods de modelos y partículas pueden pesar."),

  // ----- abrir .rar / .7z
  rule("não consegui iniciar o {p}: {x}", "couldn't start {p}: {x}", "no pude iniciar {p}: {x}"),
  rule("o {p} não conseguiu abrir o arquivo (código {c}). {x}", "{p} couldn't open the file (code {c}). {x}", "{p} no pudo abrir el archivo (código {c}). {x}"),
  rule("a extração passou de {n} s", "extraction took longer than {n} s", "la extracción superó {n} s"),
  rule("{a} MB e {b} arquivos descompactados", "{a} MB and {b} files unpacked", "{a} MB y {b} archivos descomprimidos"),
  rule("o arquivo tem um link simbólico ({x}), recusado", "the archive has a symbolic link ({x}), refused", "el archivo tiene un enlace simbólico ({x}), rechazado"),
  rule("o arquivo tem um link simbólico, recusado", "the archive has a symbolic link, refused", "el archivo tiene un enlace simbólico, rechazado"),
  rule("o arquivo tem um item especial (não é arquivo nem pasta), recusado", "the archive has a special item (neither file nor folder), refused", "el archivo tiene un elemento especial (ni archivo ni carpeta), rechazado"),
  rule("o arquivo está vazio", "the archive is empty", "el archivo está vacío"),
  rule("já é um .zip", "it's already a .zip", "ya es un .zip"),

  // ----- Casual
  rule("não consegui criar {p}: {x}", "couldn't create {p}: {x}", "no pude crear {p}: {x}"),
  rule("não consegui gravar {p}: {x}", "couldn't write {p}: {x}", "no pude escribir {p}: {x}"),
  rule("não consegui apagar {p}: {x}", "couldn't delete {p}: {x}", "no pude borrar {p}: {x}"),
  rule("o mod '{n}' não está na lista do casual", "the mod '{n}' isn't in the Casual list", "el mod '{n}' no está en la lista de Casual"),
  rule("a operação parou de forma inesperada: {x}", "the operation stopped unexpectedly: {x}", "la operación se detuvo de forma inesperada: {x}"),
  rule("Começando…", "Starting…", "Empezando…"),
  rule("Pronto", "Done", "Listo"),
  rule("O Casual Preloader ainda não está configurado no app.", "The Casual Preloader isn't set up in the app yet.", "El Casual Preloader aún no está configurado en la app."),
  rule("não consegui iniciar o Python do preloader: {x}", "couldn't start the preloader's Python: {x}", "no pude iniciar el Python del preloader: {x}"),
  rule("a ponte do preloader não iniciou. {x}", "the preloader bridge didn't start. {x}", "el puente del preloader no arrancó. {x}"),
  rule("Últimas linhas do Python: {x}", "Last Python lines: {x}", "Últimas líneas de Python: {x}"),
  rule("a ponte já foi fechada", "the bridge was already closed", "el puente ya se cerró"),
  rule("não consegui falar com a ponte: {x}", "couldn't talk to the bridge: {x}", "no pude comunicarme con el puente: {x}"),
  rule("a ponte do preloader fechou no meio da operação. {x}", "the preloader bridge closed mid-operation. {x}", "el puente del preloader se cerró a mitad de la operación. {x}"),
  rule("o preloader ficou 10 minutos sem responder e foi encerrado.", "the preloader didn't respond for 10 minutes and was stopped.", "el preloader estuvo 10 minutos sin responder y se cerró."),
  rule("a ponte respondeu algo inesperado: {x}", "the bridge replied something unexpected: {x}", "el puente respondió algo inesperado: {x}"),
  rule("o zip do preloader está corrompido: {x}", "the preloader zip is corrupted: {x}", "el zip del preloader está dañado: {x}"),
  rule("não consegui abrir o zip baixado: {x}", "couldn't open the downloaded zip: {x}", "no pude abrir el zip descargado: {x}"),
  rule("o zip do preloader tem arquivos demais ({n})", "the preloader zip has too many files ({n})", "el zip del preloader tiene demasiados archivos ({n})"),
  rule("caminho perigoso no zip: {x}", "dangerous path in the zip: {x}", "ruta peligrosa en el zip: {x}"),
  rule("o zip tem um link simbólico ({x}), recusado", "the zip has a symbolic link ({x}), refused", "el zip tiene un enlace simbólico ({x}), rechazado"),
  rule("o zip do preloader descompactaria acima do limite de segurança", "the preloader zip would unpack beyond the safety limit", "el zip del preloader se descomprimiría por encima del límite de seguridad"),
  rule("o arquivo {p} é maior do que o zip declara", "the file {p} is larger than the zip declares", "el archivo {p} es mayor de lo que declara el zip"),
  rule("não consegui baixar o Casual Preloader: {x}", "couldn't download the Casual Preloader: {x}", "no pude descargar el Casual Preloader: {x}"),
  rule(
    "o arquivo do preloader tem tamanho diferente do esperado ({a} em vez de {b} bytes). Recusado.",
    "the preloader file has a different size than expected ({a} instead of {b} bytes). Refused.",
    "el archivo del preloader tiene un tamaño distinto del esperado ({a} en vez de {b} bytes). Rechazado.",
  ),
  rule("a conexão caiu durante o download: {x}", "the connection dropped during the download: {x}", "se cortó la conexión durante la descarga: {x}"),
  rule("o download passou do tamanho esperado. Recusado.", "the download went over the expected size. Refused.", "la descarga superó el tamaño esperado. Rechazada."),
  rule("não consegui gravar o download: {x}", "couldn't save the download: {x}", "no pude guardar la descarga: {x}"),
  rule("Baixando o Casual Preloader… {a} de {b} MB", "Downloading the Casual Preloader… {a} of {b} MB", "Descargando el Casual Preloader… {a} de {b} MB"),
  rule("Baixando o Casual Preloader…", "Downloading the Casual Preloader…", "Descargando el Casual Preloader…"),
  rule(
    "o download não bate com a versão oficial fixada (SHA-256 {h}). Nada foi instalado.",
    "the download doesn't match the pinned official version (SHA-256 {h}). Nothing was installed.",
    "la descarga no coincide con la versión oficial fijada (SHA-256 {h}). No se instaló nada.",
  ),
  rule("o preloader não tem requirements.txt: {x}", "the preloader has no requirements.txt: {x}", "el preloader no tiene requirements.txt: {x}"),
  rule("não consegui iniciar o pip: {x}", "couldn't start pip: {x}", "no pude iniciar pip: {x}"),
  rule("Instalando bibliotecas do preloader… {x}", "Installing the preloader's libraries… {x}", "Instalando las bibliotecas del preloader… {x}"),
  rule("Instalando bibliotecas do preloader…", "Installing the preloader's libraries…", "Instalando las bibliotecas del preloader…"),
  rule(
    "o pip não conseguiu instalar as bibliotecas (código {c}). Precisa de internet. {x}",
    "pip couldn't install the libraries (code {c}). Internet is required. {x}",
    "pip no pudo instalar las bibliotecas (código {c}). Hace falta internet. {x}",
  ),
  rule("Extraindo…", "Extracting…", "Extrayendo…"),
  rule("não consegui preservar seus mods: {x}", "couldn't preserve your mods: {x}", "no pude conservar tus mods: {x}"),
  rule("não consegui finalizar a extração: {x}", "couldn't finish the extraction: {x}", "no pude terminar la extracción: {x}"),
  rule("O preloader foi extraído, mas faltam bibliotecas depois do pip. Veja o registro da operação.", "The preloader was extracted, but libraries are still missing after pip. See the operation log.", "El preloader se extrajo, pero faltan bibliotecas después de pip. Mira el registro de la operación."),
  rule("Conferindo a ponte com o preloader…", "Checking the bridge to the preloader…", "Comprobando el puente con el preloader…"),
  rule("tem arquivo executável ou perigoso ({x})", "has an executable or dangerous file ({x})", "tiene un archivo ejecutable o peligroso ({x})"),
  rule(
    "este mod tem partículas (.pcf). O app ainda não escolhe partículas para o casual: use por enquanto a janela do Casual Preloader",
    "this mod has particles (.pcf). The app can't pick particles for Casual yet: use the Casual Preloader window for now",
    "este mod tiene partículas (.pcf). La app aún no elige partículas para Casual: por ahora usa la ventana del Casual Preloader",
  ),
  rule("Conferindo o arquivo…", "Checking the file…", "Comprobando el archivo…"),
  rule("Importando no preloader…", "Importing into the preloader…", "Importando en el preloader…"),
  rule("este mod tem partículas (.pcf) dentro do .vpk. O app ainda não escolhe partículas para o casual: use por enquanto a janela do Casual Preloader", "this mod has particles (.pcf) inside the .vpk. The app can't pick particles for Casual yet: use the Casual Preloader window for now", "este mod tiene partículas (.pcf) dentro del .vpk. La app aún no elige partículas para Casual: por ahora usa la ventana del Casual Preloader"),
  rule("Não reconheci a estrutura do mod (esperava pastas como sound, models, materials... ou um .vpk) ou o zip tem mais de um mod.", "Couldn't recognize the mod structure (expected folders like sound, models, materials... or a .vpk) or the zip has more than one mod.", "No reconocí la estructura del mod (se esperaban carpetas como sound, models, materials... o un .vpk) o el zip tiene más de un mod."),
  rule("Estrutura de mod não reconhecida (esperava pastas como sound, models, materials...)", "Mod structure not recognized (expected folders like sound, models, materials...)", "Estructura de mod no reconocida (se esperaban carpetas como sound, models, materials...)"),
  rule(
    "Não reconheci a estrutura do mod (esperava pastas como sound, models, materials...) ou o zip tem mais de um mod.",
    "Couldn't recognize the mod structure (expected folders like sound, models, materials...) or the zip has more than one mod.",
    "No reconocí la estructura del mod (se esperaban carpetas como sound, models, materials...) o el zip tiene más de un mod.",
  ),
  rule("não consegui criar a pasta de backup: {x}", "couldn't create the backup folder: {x}", "no pude crear la carpeta de copia de seguridad: {x}"),
  rule("não consegui copiar {f} para o backup: {x}", "couldn't copy {f} to the backup: {x}", "no pude copiar {f} a la copia de seguridad: {x}"),
  rule("Escolha pelo menos um mod. Para deixar o jogo sem mods, use Reverter.", "Pick at least one mod. To leave the game without mods, use Revert.", "Elige al menos un mod. Para dejar el juego sin mods, usa Revertir."),
  rule("Guardando uma cópia do gameinfo.txt…", "Saving a copy of gameinfo.txt…", "Guardando una copia de gameinfo.txt…"),
  rule("Iniciando o preloader…", "Starting the preloader…", "Iniciando el preloader…"),
];

/** Rótulos de "o que muda" (replaces.rs). Exatos, ou com o nome da classe no fim/meio. */
const LABELS: Record<string, [string, string]> = {
  "Armas compartilhadas (pistola, shotgun, SMG)": ["Shared weapons (pistol, shotgun, SMG)", "Armas compartidas (pistola, escopeta, SMG)"],
  "Configurações (cfg)": ["Settings (cfg)", "Configuración (cfg)"],
  Cosméticos: ["Cosmetics", "Cosméticos"],
  Críticos: ["Crits", "Críticos"],
  "Dano e morte": ["Pain and death", "Daño y muerte"],
  Efeitos: ["Effects", "Efectos"],
  "Freeze cam": ["Freeze cam", "Freeze cam"],
  "HUD e interface": ["HUD and interface", "HUD e interfaz"],
  Hitsound: ["Hitsound", "Hitsound"],
  "Interface (imagens)": ["Interface (images)", "Interfaz (imágenes)"],
  Killsound: ["Killsound", "Killsound"],
  Killstreak: ["Killstreak", "Killstreak"],
  "Locutor (announcer)": ["Announcer", "Locutor (announcer)"],
  "Mann vs. Machine": ["Mann vs. Machine", "Mann vs. Machine"],
  Mapas: ["Maps", "Mapas"],
  "Modelos de armas": ["Weapon models", "Modelos de armas"],
  "Modelos de personagens": ["Character models", "Modelos de personajes"],
  Modelos: ["Models", "Modelos"],
  "Música do menu": ["Menu music", "Música del menú"],
  Música: ["Music", "Música"],
  Outros: ["Other", "Otros"],
  Partículas: ["Particles", "Partículas"],
  Passos: ["Footsteps", "Pasos"],
  "Provocações (música)": ["Taunts (music)", "Burlas (música)"],
  "Provocações (voz)": ["Taunts (voice)", "Burlas (voz)"],
  "Scripts de som (game_sounds)": ["Sound scripts (game_sounds)", "Scripts de sonido (game_sounds)"],
  "Scripts do jogo": ["Game scripts", "Scripts del juego"],
  Skybox: ["Skybox", "Skybox"],
  "Sons (outros)": ["Sounds (other)", "Sonidos (otros)"],
  "Sons de armas": ["Weapon sounds", "Sonidos de armas"],
  "Sons de interface": ["Interface sounds", "Sonidos de interfaz"],
  "Sons de itens": ["Item sounds", "Sonidos de objetos"],
  "Sons diversos": ["Misc sounds", "Sonidos varios"],
  "Sons do jogador": ["Player sounds", "Sonidos del jugador"],
  "Texturas de armas": ["Weapon textures", "Texturas de armas"],
  "Texturas de cosméticos": ["Cosmetic textures", "Texturas de cosméticos"],
  "Texturas de personagens": ["Character textures", "Texturas de personajes"],
  Texturas: ["Textures", "Texturas"],
  "Vozes (outras)": ["Voices (other)", "Voces (otras)"],
  "Vozes do Mann vs. Machine": ["Mann vs. Machine voices", "Voces de Mann vs. Machine"],
};

/** Rótulos com o nome da classe: "Voz do Demoman" → "Demoman voice". */
const CLASS_LABELS: [RegExp, string, string][] = [
  [/^Armas do (.+)$/, "$1 weapons", "Armas del $1"],
  [/^Voz do (.+)$/, "$1 voice", "Voz del $1"],
  [/^Provocações \(voz\) do (.+)$/, "$1 taunts (voice)", "Burlas (voz) del $1"],
  [/^Texturas do (.+)$/, "$1 textures", "Texturas del $1"],
  [/^Modelo do (.+)$/, "$1 model", "Modelo del $1"],
];

export function translateCore(msg: string, lang: CoreLang): string {
  if (lang === "pt" || !msg) return msg;
  const i = lang === "en" ? 0 : 1;

  const label = LABELS[msg];
  if (label) return label[i];
  for (const [re, en, es] of CLASS_LABELS) {
    if (re.test(msg)) return msg.replace(re, i === 0 ? en : es);
  }

  for (const r of RULES) {
    const m = r.re.exec(msg);
    if (!m) continue;
    const template = lang === "en" ? r.en : r.es;
    // os pedaços capturados também podem ser mensagens conhecidas ("o app recusou: tem arquivo executável...")
    return template.replace(/\{(\w+)\}/g, (_, name: string) => {
      const idx = r.names.indexOf(name);
      return idx >= 0 ? translateCore(m[idx + 1] ?? "", lang) : `{${name}}`;
    });
  }
  return msg;
}
