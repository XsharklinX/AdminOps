import { describe, expect, it } from "vitest";
import { safeSvg } from "./safeSvg";

const QR = `<?xml version="1.0" standalone="yes"?><svg xmlns="http://www.w3.org/2000/svg" version="1.1" width="228" height="228" viewBox="0 0 38 38" shape-rendering="crispEdges"><rect x="0" y="0" width="38" height="38" fill="#ffffff"/><path d="M4 4h1v1h-1z M6 4h2v1h-2z" fill="#111315"/></svg>`;

describe("SVG del QR", () => {
  it("acepta lo que genera AdminOps", () => {
    expect(safeSvg(QR)).toBe(QR);
    expect(safeSvg(`<svg width="1" height="1"><title>QR</title><path d="M0 0h1v1z"/></svg>`)).not.toBeNull();
  });

  it("rechaza todo lo que no sea rectángulos y trazos", () => {
    const bad = [
      `<svg><script>alert(1)</script></svg>`,
      `<svg onload="alert(1)"></svg>`,
      `<svg><rect width="1" height="1" onclick="x()"/></svg>`,
      `<svg><rect width="1" height="1" ONCLICK="x()"/></svg>`,
      `<svg><a href="https://evil.example"><rect width="1" height="1"/></a></svg>`,
      `<svg><image href="https://evil.example/x.png"/></svg>`,
      `<svg><foreignObject><iframe src="x"></iframe></foreignObject></svg>`,
      `<svg><style>*{display:none}</style></svg>`,
      `<svg><path d="M0 0" fill="url(javascript:alert(1))"/></svg>`,
      `<svg><rect width="1" height='1'/></svg>`,
      `<svg><rect width="1" height="1"/></svg><img src=x onerror=alert(1)>`,
      `<div><svg></svg></div>`,
      `<svg><title><b>x</b></title></svg>`,
      `<svg></svg><svg></svg>`,
      "",
      "texto",
    ];
    for (const s of bad) expect(safeSvg(s), s).toBeNull();
  });

  it("rechaza lo enorme", () => {
    expect(safeSvg(`<svg>${'<rect width="1" height="1"/>'.repeat(10_000)}</svg>`)).toBeNull();
  });
});
