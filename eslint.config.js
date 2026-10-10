// Reglas de código de la interfaz.
//
// El objetivo no es el estilo (de eso se encarga el formato del editor), sino la
// clase de fallo que TypeScript no ve. En concreto `react-hooks/exhaustive-deps`,
// que es la regla que habría avisado del portal que se recolocaba en cada
// recarga de la lista: el tipo de error que solo aparece en ejecución y en el
// equipo de un cliente.
//
// Cuando un efecto tiene que ignorar una dependencia a propósito, se apaga la
// regla en esa línea **con el motivo escrito**. Silenciarla sin explicar por qué
// es peor que no tenerla.
import js from "@eslint/js";
import globals from "globals";
import reactHooks from "eslint-plugin-react-hooks";
import tseslint from "typescript-eslint";

export default tseslint.config(
  { ignores: ["dist", "src-tauri", "node_modules", "release", "installer", "docs", "public", "eslint.config.js", "tests/ui/out", "tests/sandbox"] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["src/**/*.{ts,tsx}"],
    languageOptions: {
      ecmaVersion: 2022,
      globals: { ...globals.browser, ...globals.es2021 },
      parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname },
    },
    plugins: { "react-hooks": reactHooks },
    rules: {
      // Lo que de verdad se quiere vigilar.
      "react-hooks/rules-of-hooks": "error",
      // `useLiveEffect` recibe sus dependencias igual que `useEffect`: se revisan igual.
      "react-hooks/exhaustive-deps": ["error", { additionalHooks: "(useLiveEffect)" }],

      // El resto del paquete son reglas pensadas para el compilador de React, que
      // este proyecto no usa: dan por malos patrones correctos aquí (guardar en el
      // estado desde un efecto al cargar datos, `Date.now()` dentro de un `onClick`).
      // Se apagan a conciencia, no por comodidad.
      "react-hooks/set-state-in-effect": "off",
      "react-hooks/purity": "off",
      "react-hooks/refs": "off",
      "react-hooks/immutability": "off",
      "react-hooks/preserve-manual-memoization": "off",

      // Una promesa sin `await` ni `.catch()` se pierde en silencio. Queda como
      // aviso mientras se limpian las que vienen de antes (ver ROADMAP.md).
      "@typescript-eslint/no-floating-promises": "warn",
      // Los argumentos que empiezan por «_» son a propósito.
      "@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_", varsIgnorePattern: "^_" }],
    },
  },
  {
    // Las pruebas de la interfaz en marcha: un script de Node que maneja un navegador.
    files: ["tests/ui/**/*.{mjs,ts}"],
    languageOptions: { globals: { ...globals.node, ...globals.browser } },
  },
  {
    // Las pruebas pueden ser más laxas con los tipos.
    files: ["src/**/*.test.ts"],
    rules: { "@typescript-eslint/no-explicit-any": "off" },
  },
);
