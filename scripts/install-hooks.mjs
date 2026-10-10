// Activa los avisos de .githooks en este clon (se ejecuta solo con «npm install»).
// Fuera de un repositorio de git (un .zip descargado, la CI sin .git) no hace nada.
import { execFileSync } from "node:child_process";

try {
  execFileSync("git", ["rev-parse", "--git-dir"], { stdio: "ignore" });
  execFileSync("git", ["config", "core.hooksPath", ".githooks"], { stdio: "ignore" });
} catch {
  /* sin git: no hay nada que activar */
}
