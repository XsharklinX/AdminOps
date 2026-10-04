import { describe, expect, it } from "vitest";
import { parsePrefix, sameNetwork, subnet } from "./subnet";
import { durationText, lasted, summaryText } from "../pages/NetWatch";

describe("calculadora de red", () => {
  it("una red de oficina /24", () => {
    const s = subnet("192.168.1.37/24")!;
    expect(s).toMatchObject({ network: "192.168.1.0", broadcast: "192.168.1.255", first: "192.168.1.1", last: "192.168.1.254", hosts: 254, mask: "255.255.255.0", private: true });
  });

  it("con la máscara escrita y redes más pequeñas", () => {
    expect(subnet("10.0.5.130", "255.255.255.192")).toMatchObject({ prefix: 26, network: "10.0.5.128", last: "10.0.5.190", hosts: 62 });
    expect(subnet("172.20.1.1/30")).toMatchObject({ hosts: 2, first: "172.20.1.1", last: "172.20.1.2" });
    expect(subnet("8.8.8.8/32")).toMatchObject({ hosts: 1, private: false });
  });

  it("rechaza lo que no es una IP o una máscara", () => {
    expect(subnet("192.168.1.300/24")).toBeNull();
    expect(subnet("hola")).toBeNull();
    expect(parsePrefix("255.0.255.0")).toBeNull();
    expect(parsePrefix("/33")).toBeNull();
    expect(parsePrefix("255.255.240.0")).toBe(20);
  });

  it("misma red o no", () => {
    const s = subnet("192.168.1.37/24")!;
    expect(sameNetwork(s, "192.168.1.200")).toBe(true);
    expect(sameNetwork(s, "192.168.2.5")).toBe(false);
    expect(sameNetwork(s, "x")).toBeNull();
  });
});

describe("vigilante de la conexión", () => {
  it("duraciones y resumen para el proveedor", () => {
    expect(durationText(42)).toBe("42 s");
    expect(durationText(125)).toBe("2 min 5 s");
    expect(durationText(3 * 3600 + 600)).toBe("3 h 10 min");
    expect(lasted({ start: 100, end: null, kind: "internet" }, 160)).toBe(60);
    const text = summaryText(
      { running: false, startedAt: 0, gateway: "192.168.1.1", checks: 100, failed: 4, points: [], outages: [{ start: 10, end: 70, kind: "internet" }, { start: 200, end: 230, kind: "router" }] },
      1000,
    );
    expect(text).toContain("2 cortes");
    expect(text).toContain("el más largo, 1 min 0 s");
    expect(text).toContain("Fallaba: el router.");
    expect(text).toContain("fallidas: 4 (4.0 %)");
  });
});
