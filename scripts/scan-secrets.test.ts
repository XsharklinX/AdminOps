import { describe, expect, it } from "vitest";
// @ts-expect-error módulo .mjs sin tipos
import { scan, skipped } from "./scan-secrets.mjs";

// Los valores de prueba se arman por partes para que este archivo no sea, él mismo, un hallazgo.
const ghp = "gh" + "p_" + "a".repeat(36);
const aws = "AK" + "IA" + "ABCDEFGHIJKLMNOP";
const pem = "-----BEGIN " + "RSA PRIVATE KEY-----";

describe("buscador de secretos", () => {
  it("encuentra lo que no debería subirse", () => {
    for (const line of [
      `const t = "${ghp}";`,
      `aws = ${aws}`,
      pem,
      `url = "https://admin:S3cretoLargo99@db.example.com/x"`,
      `const API_KEY = "Xk3j9Zq2LmN8pQr5TuV7wYb1";`,
      `password: 'AbCdEf0123456789GhIjKlMn'`,
      `Authorization: Bearer ${"ey" + "JhbGciOiJIUzI1NiJ9"}.${"ey" + "JzdWIiOiIxMjM0NTY3ODkwIn0"}.abcdefghij1234567890`,
    ]) expect(scan(line).length, line).toBeGreaterThan(0);
  });

  it("no da la alarma con lo normal", () => {
    for (const line of [
      `const password = usePassword();`,
      `password: "tu_contraseña_aquí_de_ejemplo_larga"`,
      `const token = "\${TOKEN_DE_LA_CI}";`,
      `api_key = "<pon aquí tu clave de api>"`,
      `https://github.com/XsharklinX/AdminOps/releases`,
      `fn secret_var(var: &str, secret: &str) -> String {`,
      `// el token se pide al usuario`,
      `const SEED_KEY = "adminops.prefs";`,
    ]) expect(scan(line), line).toEqual([]);
  });

  it("«secrets:ignore» salta la línea, y nunca devuelve el valor", () => {
    expect(scan(`const t = "${ghp}"; // secrets:ignore`)).toEqual([]);
    const hit = scan(`x = "${ghp}"`)[0];
    expect(JSON.stringify(hit)).not.toContain(ghp);
    expect(hit.line).toBe(1);
  });

  it("no mira binarios ni dependencias", () => {
    expect(skipped("package-lock.json")).toBe(true);
    expect(skipped("src/assets/logo.png")).toBe(true);
    expect(skipped("src/App.tsx")).toBe(false);
  });
});
