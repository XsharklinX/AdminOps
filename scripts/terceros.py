# -*- coding: utf-8 -*-
"""Lista de componentes de terceros de AdminOps, con su licencia.

La pide IT antes de aprobar un programa. Sale de los manifiestos reales
(`cargo metadata` y node_modules), así que está al día si se regenera:

    python scripts/terceros.py

Escribe docs/TERCEROS.md.
"""
import io
import json
import os
import subprocess

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def rust():
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--offline"],
        cwd=os.path.join(ROOT, "src-tauri"), capture_output=True, text=True, encoding="utf-8", check=True,
    ).stdout
    meta = json.loads(out)
    # Solo lo que de verdad entra en AdminOps (no lo de otras plataformas ni las pruebas).
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    root = meta["resolve"]["root"]
    seen, stack = set(), [root]
    while stack:
        nid = stack.pop()
        if nid in seen:
            continue
        seen.add(nid)
        for d in nodes[nid]["deps"]:
            kinds = d.get("dep_kinds", [])
            if any(k.get("kind") in (None, "normal", "build") and not (k.get("target") or "").startswith(("cfg(target_os = \"macos\"", "cfg(target_os = \"linux\"", "cfg(target_os = \"android\"", "cfg(target_os = \"ios\"")) for k in kinds):
                stack.append(d["pkg"])
    pkgs = [p for p in meta["packages"] if p["id"] in seen and p["id"] != root]
    return sorted({(p["name"], p["version"], p.get("license") or "ver el paquete") for p in pkgs})


def npm():
    pkg = json.load(io.open(os.path.join(ROOT, "package.json"), encoding="utf-8"))
    out, stack, seen = set(), list(pkg.get("dependencies", {}).keys()), set()
    while stack:
        name = stack.pop()
        if name in seen:
            continue
        seen.add(name)
        path = os.path.join(ROOT, "node_modules", *name.split("/"), "package.json")
        if not os.path.exists(path):
            continue
        p = json.load(io.open(path, encoding="utf-8"))
        lic = p.get("license")
        if isinstance(lic, dict):
            lic = lic.get("type")
        out.add((name, p.get("version", "?"), lic or "ver el paquete"))
        stack.extend(p.get("dependencies", {}).keys())
    return sorted(out)


def table(rows):
    lines = ["| Componente | Versión | Licencia |", "|---|---|---|"]
    lines += [f"| {n} | {v} | {l} |" for n, v, l in rows]
    return "\n".join(lines)


def main():
    r, n = rust(), npm()
    licenses = {}
    for _, _, l in r + n:
        licenses[l] = licenses.get(l, 0) + 1
    resumen = ", ".join(f"{l} ({c})" for l, c in sorted(licenses.items(), key=lambda x: -x[1]))
    md = f"""# Componentes de terceros de AdminOps

Generado con `python scripts/terceros.py` a partir de los manifiestos del proyecto
(`cargo metadata` y `node_modules`). Regenerarlo antes de cada versión.

**{len(r)} componentes de Rust y {len(n)} de la interfaz.** Licencias: {resumen}.

Además, AdminOps incluye **LibreHardwareMonitor** (MPL-2.0) para leer temperaturas; su aviso de
licencias completo va en `lhm/THIRD-PARTY-NOTICES.txt` junto al programa y se abre desde
Ajustes → Acerca de. La tipografía **IBM Plex Sans** (SIL Open Font License 1.1) va dentro de la
interfaz y de los informes PDF; su licencia está en `src-tauri/assets/fonts/OFL-IBM-Plex.txt`.

Ninguna es GPL ni AGPL: ninguna obliga a publicar el código de AdminOps.

## Interfaz (JavaScript)

{table(n)}

## Programa (Rust)

{table(r)}
"""
    io.open(os.path.join(ROOT, "docs", "TERCEROS.md"), "w", encoding="utf-8").write(md)
    gpl = [x for x in r + n if "GPL" in (x[2] or "") and "LGPL" not in (x[2] or "")]
    print(f"docs/TERCEROS.md: {len(r)} de Rust, {len(n)} de la interfaz. GPL: {gpl or 'ninguno'}")


if __name__ == "__main__":
    main()
