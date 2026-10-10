import { describe, expect, it, vi } from "vitest";
import { humanError, isUnexpected, onInternalError, withoutUserPaths } from "./errors";

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

import { explainError, supportDetails } from "./errors";

describe("errores que ayudan", () => {
  it("explica el porqué y propone qué hacer", () => {
    const e = explainError("Windows denegó el acceso. Prueba a abrir AdminOps como administrador.");
    expect(e.fix).toBe("admin");
    expect(e.tries.length).toBeGreaterThan(0);
    expect(explainError("Algo raro 0x1234").why).toMatch(/no reconoce/);
  });
  it("los detalles para soporte no llevan datos personales", () => {
    const t = supportDetails("Fallo en C:\\Users\\ana\\x con ana@empresa.com en \\\\SRV01\\datos");
    expect(t).not.toMatch(/ana/);
    expect(t).not.toMatch(/SRV01/);
    expect(t).toMatch(/\[correo\]/);
  });
});

describe("qué va al registro de errores de la app", () => {
  it("anota los fallos de verdad", () => {
    expect(isUnexpected("thread 'main' panicked at src/x.rs:10")).toBe(true);
    expect(isUnexpected("No se pudo leer el disco: HRESULT 0x80041003")).toBe(true);
    expect(isUnexpected(new Error("Unhandled exception in module"))).toBe(true);
    expect(isUnexpected("El comando tardó demasiado: tiempo agotado")).toBe(true);
  });
  it("no anota lo que es una condición normal", () => {
    expect(isUnexpected("Acceso denegado (0x80070005)")).toBe(false);
    expect(isUnexpected("os error 5: access is denied")).toBe(false);
    expect(isUnexpected("os error 112: no hay espacio suficiente")).toBe(false);
    expect(isUnexpected("No hay conexión: error sending request")).toBe(false);
    expect(isUnexpected("Escribe entre 2 y 60 caracteres.")).toBe(false);
    // Los internos ya se anotan por su lado: no se duplican.
    expect(isUnexpected("invalid args `x` for command y")).toBe(false);
  });
});
