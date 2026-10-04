import { describe, expect, it } from "vitest";
import type { CatalogApp, CatalogView } from "./api";
import { catalogCounts, isVisible } from "./catalogView";
import toml from "../../src-tauri/tools/apps.toml?raw";

const app = (id: string, category: string, home = false): CatalogApp => ({ id, name: id, category, source: "winget", home });
const view = (over: Partial<CatalogView> = {}): CatalogView => ({ business: true, hiddenApps: [], hiddenCategories: [], ...over });

describe("qué enseña el catálogo", () => {
  const apps = [app("Chrome", "browser"), app("Telegram", "chat", true), app("Zoom", "chat"), app("Steam", "games", true)];

  it("la vista Empresa deja fuera lo de uso personal; «Todo» lo enseña", () => {
    expect(apps.filter((a) => isVisible(a, view())).map((a) => a.id)).toEqual(["Chrome", "Zoom"]);
    expect(apps.filter((a) => isVisible(a, view({ business: false })))).toHaveLength(4);
  });

  it("se pueden ocultar programas sueltos y categorías enteras", () => {
    const v = view({ business: false, hiddenApps: ["Zoom"], hiddenCategories: ["games"] });
    expect(apps.filter((a) => isVisible(a, v)).map((a) => a.id)).toEqual(["Chrome", "Telegram"]);
  });

  it("cuenta lo visible y lo oculto, y cuánto es solo por ser de uso personal", () => {
    expect(catalogCounts(apps, view())).toEqual({ visible: 2, hidden: 2, home: 2 });
    expect(catalogCounts(apps, view({ hiddenApps: ["Chrome", "Steam"] }))).toEqual({ visible: 1, hidden: 3, home: 1 });
    expect(catalogCounts(apps, view({ business: false }))).toEqual({ visible: 4, hidden: 0, home: 0 });
  });
});

describe("el catálogo de fábrica", () => {
  const blocks = toml.split("[[app]]").slice(1).map((b) => b.split("[[list]]")[0]);
  const field = (b: string, k: string) => new RegExp(`^${k} = "([^"]+)"`, "m").exec(b)?.[1] ?? "";
  const items = blocks.map((b) => ({ id: field(b, "id"), category: field(b, "category"), home: /^home = true/m.test(b) }));

  it("los juegos son siempre de uso personal, y lo de empresa nunca", () => {
    for (const a of items.filter((x) => x.category === "games")) expect(a.home, a.id).toBe(true);
    for (const a of items.filter((x) => ["vpn", "backup", "security", "runtime", "tech"].includes(x.category) && x.id !== "Microsoft.DirectX")) expect(a.home, a.id).toBe(false);
    for (const id of ["Telegram.TelegramDesktop", "Discord.Discord", "Spotify.Spotify", "qBittorrent.qBittorrent"]) expect(items.find((x) => x.id === id)?.home, id).toBe(true);
    for (const id of ["Google.Chrome", "Microsoft.Office", "Microsoft.Teams", "Zoom.Zoom", "AnyDesk.AnyDesk", "7zip.7zip"]) expect(items.find((x) => x.id === id)?.home, id).toBe(false);
  });

  it("la vista Empresa sigue teniendo de todo lo que hace falta en una oficina", () => {
    const business = items.filter((a) => !a.home);
    expect(business.length).toBeGreaterThan(100);
    for (const c of ["browser", "office", "chat", "remote", "vpn", "security", "backup"]) expect(business.some((a) => a.category === c), c).toBe(true);
  });
});
