// «Falta MSVCP140.dll»: de qué paquete oficial de Microsoft es cada DLL que suele faltar y cómo se instala con winget.
// Nunca se descarga la DLL suelta: las webs de «descargar DLL» son de las fuentes de malware más habituales y una DLL
// sin su paquete casi nunca arregla nada. Se instalan las dos versiones (32 y 64 bits): un programa de 32 bits necesita
// la de 32 aunque Windows sea de 64.

import type { CatalogApp } from "./api/install";

export interface DllPackage {
  /** Nombre que se enseña («Visual C++ 2015–2022»). */
  name: string;
  /** Las DLL que trae (en minúsculas, sin «.dll»). */
  dlls: string[];
  /** Lo que se pone en la cola de Instalar (winget). */
  apps: CatalogApp[];
  /** Si no hay paquete que instalar (`apps` vacío), lo que hay que hacer. */
  hint?: string;
}

const vc = (year: string, ids: string[]): CatalogApp[] => ids.map((id) => ({ id, name: `Visual C++ ${year} (${/x86/.test(id) ? "32" : "64"} bits)`, category: "Componentes", source: "winget" }));

export const DLL_PACKAGES: DllPackage[] = [
  {
    name: "Visual C++ 2015–2022",
    dlls: ["msvcp140", "msvcp140_1", "msvcp140_2", "vcruntime140", "vcruntime140_1", "concrt140", "vccorlib140", "mfc140", "mfc140u", "ucrtbase", "api-ms-win-crt-runtime-l1-1-0", "api-ms-win-crt-heap-l1-1-0", "api-ms-win-crt-string-l1-1-0", "api-ms-win-crt-stdio-l1-1-0", "api-ms-win-crt-math-l1-1-0", "api-ms-win-crt-locale-l1-1-0"],
    apps: vc("2015–2022", ["Microsoft.VCRedist.2015+.x64", "Microsoft.VCRedist.2015+.x86"]),
  },
  { name: "Visual C++ 2013", dlls: ["msvcp120", "msvcr120", "mfc120", "mfc120u"], apps: vc("2013", ["Microsoft.VCRedist.2013.x64", "Microsoft.VCRedist.2013.x86"]) },
  { name: "Visual C++ 2012", dlls: ["msvcp110", "msvcr110", "mfc110", "mfc110u"], apps: vc("2012", ["Microsoft.VCRedist.2012.x64", "Microsoft.VCRedist.2012.x86"]) },
  { name: "Visual C++ 2010", dlls: ["msvcp100", "msvcr100", "mfc100", "mfc100u"], apps: vc("2010", ["Microsoft.VCRedist.2010.x64", "Microsoft.VCRedist.2010.x86"]) },
  { name: "Visual C++ 2008", dlls: ["msvcp90", "msvcr90", "mfc90"], apps: vc("2008", ["Microsoft.VCRedist.2008.x64", "Microsoft.VCRedist.2008.x86"]) },
  {
    name: "DirectX (programas y juegos)",
    dlls: ["d3dx9_43", "d3dx9_42", "d3dx9_41", "d3dx9_39", "d3dx10_43", "d3dx11_43", "d3dcompiler_43", "xinput1_3", "xinput1_4", "xaudio2_7", "x3daudio1_7", "xapofx1_5"],
    apps: [{ id: "Microsoft.DirectX", name: "DirectX (runtime de juegos)", category: "Componentes", source: "winget" }],
  },
  // .NET Framework viene con Windows: no hay paquete de winget que valga (el «Developer Pack» es para programar). Si falta
  // mscoree.dll, lo habitual es que haga falta la 3.5, que se activa en las características de Windows.
  { name: ".NET Framework", dlls: ["mscoree", "mscorlib", "clr"], apps: [], hint: "Activa .NET Framework 3.5 en Características de Windows" },
];

/** El paquete de una DLL («MSVCP140.dll», «msvcp140», «vcruntime140_1.dll»). */
export function packageOfDll(text: string): DllPackage | null {
  const n = text.trim().toLowerCase().replace(/\.dll$/, "");
  if (!n) return null;
  return DLL_PACKAGES.find((p) => p.dlls.includes(n)) ?? null;
}
