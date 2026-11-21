"""Ponte entre o suny's tf2 supplier e o Casual Preloader (cueki, GPL-3.0).

Roda com o Python embutido do release oficial do preloader. Fala JSON, uma
mensagem por linha: comandos em stdin, eventos em stdout. Tudo que não for
protocolo (prints, logs) vai para stderr, para não quebrar o parser do app.

Comandos:
  {"cmd": "hello"}
  {"cmd": "status",    "tf_path": "..."}
  {"cmd": "import_zip", "zip": "...", "name": "pasta"}   (cria addons/<name>; responde "imported")
  {"cmd": "install",   "tf_path": "...", "addons": ["nome"], "options": {...}}
  {"cmd": "uninstall", "tf_path": "..."}

`tf_path` é a pasta `tf/` do jogo (não a raiz).
"""

import json
import logging
import sys
import traceback
from pathlib import Path

PROTOCOL = 1

_proto = sys.stdout
sys.stdout = sys.stderr  # qualquer print solto não pode sujar o protocolo


def emit(**event) -> None:
    _proto.write(json.dumps(event, ensure_ascii=False) + "\n")
    _proto.flush()


def _bootstrap_preloader() -> None:
    """O `core.config` do preloader lê a linha de comando ao ser importado.
    Entregamos argumentos fixos: sem auto-update e sem migração de dados antigos."""
    sys.argv = [sys.argv[0], "--no-update", "--no-migrate"]
    logging.basicConfig(level=logging.INFO, stream=sys.stderr, format="%(levelname)s %(name)s: %(message)s")


_prepared = False


def _prepare_once() -> None:
    """Faz o que a janela do preloader faz ao abrir: copia o `backup/` do release
    para os dados do usuário e prepara a pasta temporária com as partículas originais.
    Sem isso, install/uninstall falham com "staging folder is missing files"."""
    global _prepared
    if _prepared:
        return
    from core.backup_manager import prepare_runtime_environment

    error = prepare_runtime_environment()
    if error:
        raise RuntimeError(error)
    _prepared = True


def _options(raw: dict) -> dict:
    return {
        "disable_paint_colors": bool(raw.get("disable_paint_colors", False)),
        "show_console_on_startup": bool(raw.get("show_console_on_startup", True)),
        "fix_mdl_paths": bool(raw.get("fix_mdl_paths", True)),
        "skip_quickprecache": bool(raw.get("skip_quickprecache", False)),
    }


def _find_mod_root(importer, root: Path):
    """Acha a pasta que o preloader reconhece como mod (a mais rasa, até 5 níveis). Zips costumam vir
    embalados: `Nome/tf/sound/...`, `Nome/README.txt` + `Nome/tf/...`. Dois mods no mesmo nível = ambíguo (None)."""
    level = [root]
    for _ in range(5):
        valid = [d for d in level if importer.validator.validate_folder(d).is_valid]
        if len(valid) == 1:
            return valid[0]
        if len(valid) > 1:
            return None
        level = [child for d in level for child in sorted(d.iterdir()) if child.is_dir()]
    return None


def _import_vpks(importer, root: Path, name: str, progress):
    """Importa os `.vpk` achados em `root` como mods do casual, com o nome escolhido pelo app.
    Devolve None se não há `.vpk`. Um `.vpk` com partículas (.pcf) é recusado (o app ainda não escolhe partículas)."""
    import re
    import shutil

    from core.config import config
    from valve_parsers import VPKFile

    vpks = sorted(root.rglob("*.vpk"))
    mains = [p for p in vpks if p.stem.lower().endswith("_dir")] or [p for p in vpks if not re.search(r"_\d{3}$", p.stem)]
    if not mains:
        return None
    for vpk in mains:
        if VPKFile(vpk).find_files("*.pcf"):
            return False, (
                "este mod tem partículas (.pcf) dentro do .vpk. O app ainda não escolhe partículas para o casual: "
                "use por enquanto a janela do Casual Preloader"
            )
    names = []
    for vpk in mains:
        ok, message = importer.process_vpk_file(vpk, progress_callback=progress)
        if not ok:
            return ok, message
        stem = vpk.stem
        if (stem[-3:].isdigit() and len(stem) > 3 and stem[-4] == "_") or stem[-4:] == "_dir":
            stem = stem[:-4]
        src = config.addons_dir / stem
        target = config.addons_dir / (name if len(mains) == 1 else f"{name} ({stem})")
        if src != target and src.exists():
            if target.exists():
                shutil.rmtree(target)
            shutil.move(str(src), str(target))
        names.append(target.name)
    return True, f"Successfully processed {len(names)} VPK file(s)"


def handle(msg: dict) -> None:
    cmd = msg.get("cmd")

    if cmd == "hello":
        from core.config import config
        from core.version import VERSION

        emit(
            event="hello",
            protocol=PROTOCOL,
            preloader_version=VERSION,
            addons_dir=str(config.addons_dir),
            userdata=str(config.project_dir),
        )
        return

    _prepare_once()

    from core.constants import Sourcemods
    from core.services.install import InstallService

    tf_path = Path(msg["tf_path"]) if "tf_path" in msg else None  # `import_zip` não usa o jogo
    service = InstallService()

    if cmd == "status":
        emit(event="status", modified=InstallService.is_modified(tf_path))
    elif cmd == "import_zip":
        # Usa o importador do próprio preloader: ele valida a estrutura e cria a pasta em `addons`.
        import tempfile

        from core.services.importer import ImportService
        from core.util.zip import extract

        progress = lambda pct, text: emit(event="progress", pct=pct, msg=text)
        importer = ImportService()
        name = msg.get("name")
        if not name:
            ok, message = importer.process_zip_file(Path(msg["zip"]), progress_callback=progress)
        else:
            # O nome da pasta vem do app (nome do mod + id do GameBanana). Sem isso, um zip no formato
            # `tf/sound/...` viraria sempre "tf", e dois mods diferentes se sobrescreveriam.
            with tempfile.TemporaryDirectory() as tmp:
                extract(Path(msg["zip"]), Path(tmp))
                folder = _find_mod_root(importer, Path(tmp))
                if folder is not None:
                    ok, message = importer.process_folder(folder, override_name=name, progress_callback=progress)
                else:
                    # Mods empacotados num .vpk (muito comum no GameBanana): o preloader sabe extrair o .vpk.
                    result = _import_vpks(importer, Path(tmp), name, progress)
                    ok, message = result if result is not None else (
                        False,
                        "Não reconheci a estrutura do mod (esperava pastas como sound, models, materials... ou um .vpk) "
                        "ou o zip tem mais de um mod.",
                    )
        emit(event="imported", ok=bool(ok), message=str(message))
    elif cmd == "install":
        service.install(
            tf_path=tf_path,
            selected_addons=list(msg.get("addons", [])),
            on_progress=lambda pct, text: emit(event="progress", pct=pct, msg=text),
            sourcemod=Sourcemods.TF2,
            **_options(msg.get("options", {})),
        )
        emit(event="done", modified=InstallService.is_modified(tf_path))
    elif cmd == "uninstall":
        service.uninstall(
            tf_path=tf_path,
            on_progress=lambda pct, text: emit(event="progress", pct=pct, msg=text),
            sourcemod=Sourcemods.TF2,
        )
        emit(event="done", modified=InstallService.is_modified(tf_path))
    else:
        emit(event="error", msg=f"comando desconhecido: {cmd!r}")


def main() -> int:
    _bootstrap_preloader()
    emit(event="ready", protocol=PROTOCOL)
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            handle(json.loads(line))
        except Exception as exc:  # a ponte nunca morre por erro do preloader
            emit(event="error", msg=str(exc), kind=type(exc).__name__, trace=traceback.format_exc())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
