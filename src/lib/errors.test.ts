import { describe, expect, it, vi } from "vitest";
import { humanError, onInternalError, withoutUserPaths } from "./errors";

describe("sin nombres de usuario", () => {
  it("quita la carpeta del usuario de cualquier mensaje", () => {
    expect(withoutUserPaths("No se pudo abrir C:\\Users\\ana\\Desktop\\x.txt")).toBe("No se pudo abrir Carpeta personal\\Desktop\\x.txt");
    expect(withoutUserPaths("'file:///C:/Users/admin/Downloads/a.dll'")).toBe("'Carpeta personal/Downloads/a.dll'");
    expect(humanError("x", "Fallo en D:\\Usuarios\\pepe\\datos")).toBe("Fallo en Carpeta personal\\datos");
    expect(withoutUserPaths("C:\\Program Files\\AdminOps")).toBe("C:\\Program Files\\AdminOps");
  });
});

describe("errores comprensibles", () => {
  it("traduce los errores típicos del sistema", () => {
    expect(humanError("x", "Access is denied. (os error 5)")).toMatch(/administrador/);
    expect(humanError("x", "The process cannot access the file because it is being used by another process. (os error 32)")).toMatch(/en uso/);
    expect(humanError("x", "error sending request for url (https://x)")).toMatch(/Internet/);
    expect(humanError("x", "The RPC server is unavailable. (0x800706BA)")).toMatch(/equipo remoto/);
  });

  it("deja igual los mensajes que ya son claros", () => {
    expect(humanError("x", "Requiere ejecutar AdminOps como administrador.")).toBe("Requiere ejecutar AdminOps como administrador.");
  });

  it("los fallos internos se anotan y se explican sin jerga", () => {
    const log = vi.fn();
    onInternalError(log);
    const msg = humanError("save_contact", "invalid args `contact` for command `save_contact`: invalid type: floating point `1.5`, expected u64");
    expect(msg).toMatch(/Error interno de AdminOps/);
    expect(msg).not.toMatch(/u64/);
    expect(log).toHaveBeenCalledWith(expect.stringContaining("save_contact"));
  });
});
