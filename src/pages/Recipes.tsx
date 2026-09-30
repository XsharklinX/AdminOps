import {
  ArrowDown,
  ArrowUp,
  CheckCircle2,
  LaptopMinimalCheck,
  Circle,
  Copy,
  Loader2,
  Pencil,
  Play,
  Plus,
  SkipForward,
  Trash2,
  XCircle,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { Button, EmptyState, inputClass, Loading, Modal } from "../components/ui";
import {
  appsApi,
  diagApi,
  domainApi,
  libraryApi,
  profilesApi,
  sheetApi,
  systemApi,
  tweaksApi,
  usersApi,
  type AppCatalog,
  type ProfileView,
  type Recipe,
  type RecipeStep,
} from "../lib/api";
import { norm } from "../lib/contacts";

type Kind = RecipeStep["kind"];

const STEP_INFO: Record<Kind, { label: string; hint: string }> = {
  restorePoint: { label: "Punto de restauración", hint: "Para poder volver atrás si algo sale mal." },
  bloatware: { label: "Quitar bloatware", hint: "Quita las apps preinstaladas que AdminOps recomienda quitar." },
  installList: { label: "Instalar programas", hint: "Instala una de tus listas de programas (Programas → Instalar)." },
  profile: { label: "Aplicar un perfil", hint: "Un perfil de ajustes (privacidad, rendimiento, oficina…)." },
  tweaks: { label: "Aplicar ajustes sueltos", hint: "Ajustes o reparaciones concretas del catálogo." },
  user: { label: "Crear usuario local", hint: "La contraseña se pide al ejecutar (nunca se guarda)." },
  rename: { label: "Cambiar el nombre del equipo", hint: "Requiere reiniciar." },
  domain: { label: "Unir al dominio", hint: "Usuario y contraseña del dominio se piden al ejecutar. Requiere reiniciar." },
  diagnostics: { label: "Diagnóstico final", hint: "Para comprobar cómo queda el equipo." },
};

const NEW_STEP: Record<Kind, RecipeStep> = {
  restorePoint: { kind: "restorePoint" },
  bloatware: { kind: "bloatware" },
  installList: { kind: "installList", listId: "", listName: "" },
  profile: { kind: "profile", profileId: "", profileName: "" },
  tweaks: { kind: "tweaks", ids: [] },
  user: { kind: "user", name: "", fullName: "", admin: false },
  rename: { kind: "rename", newName: "" },
  domain: { kind: "domain", domain: "", ou: "" },
  diagnostics: { kind: "diagnostics" },
};

const EXAMPLE: Omit<Recipe, "id"> = {
  name: "Equipo nuevo de oficina",
  description: "Punto de restauración, sin bloatware, programas, ajustes de privacidad y diagnóstico final.",
  steps: [{ kind: "restorePoint" }, { kind: "bloatware" }, { kind: "installList", listId: "", listName: "" }, { kind: "diagnostics" }],
};

function describe(s: RecipeStep): string {
  switch (s.kind) {
    case "installList":
      return s.listName || "Sin elegir";
    case "profile":
      return s.profileName || "Sin elegir";
    case "tweaks":
      return `${s.ids.length} ajuste(s)`;
    case "user":
      return s.name ? `${s.name}${s.admin ? " (administrador)" : ""}` : "Sin nombre";
    case "rename":
      return s.newName || "Sin nombre";
    case "domain":
      return s.domain || "Sin dominio";
    default:
      return "";
  }
}

/** Plantillas de preparación: dejar un equipo listo de una vez (programas, bloatware, perfil, usuario, dominio…). */
export function Recipes({ isAdmin }: { isAdmin: boolean }) {
  const [list, setList] = useState<Recipe[] | null>(null);
  const [editing, setEditing] = useState<Recipe | null>(null);
  const [running, setRunning] = useState<Recipe | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(() => libraryApi.list("recipes").then(setList).catch((e) => toast("error", String(e))), [toast]);
  useEffect(() => {
    void load();
  }, [load]);

  const remove = async (r: Recipe) => {
    if (!(await confirm({ title: "Borrar plantilla", body: `Se borrará «${r.name}».`, confirmLabel: "Borrar", danger: true }))) return;
    await libraryApi.remove("recipes", r.id).catch((e) => toast("error", String(e)));
    void load();
  };
  const duplicate = async (r: Recipe) => {
    await libraryApi.save("recipes", { ...r, id: "", name: `${r.name} (copia)` }).catch((e) => toast("error", String(e)));
    void load();
  };

  return (
    <div className="mx-auto max-w-5xl space-y-4 p-6">
      <div className="flex items-center justify-between gap-3">
        <p className="text-xs text-dim">
          Una plantilla reúne todo lo que haces en un equipo nuevo (quitar bloatware, instalar programas, crear el usuario, unir al dominio…) para hacerlo de una
          vez y siempre igual. Viajan con AdminOps (en el USB con el portable).
        </p>
        <Button onClick={() => setEditing({ id: "", name: "", description: "", steps: [] })}>
          <Plus size={14} /> Nueva plantilla
        </Button>
      </div>
      {!isAdmin && <p className="rounded-lg border border-warn/40 bg-warn/5 px-3 py-2 text-sm text-warn">Para preparar un equipo con una plantilla hay que abrir AdminOps como administrador.</p>}
      {list === null ? (
        <Loading />
      ) : list.length === 0 ? (
        <EmptyState
          icon={<LaptopMinimalCheck size={22} />}
          title="Aún no hay plantillas"
          action={
            <Button onClick={() => setEditing({ id: "", ...EXAMPLE })}>
              <Plus size={14} /> Empezar con una de ejemplo
            </Button>
          }
        >
          Por ejemplo «Equipo de oficina»: quitar bloatware, instalar los programas de siempre, aplicar ajustes, crear el usuario y unirlo al dominio.
        </EmptyState>
      ) : (
        <div className="grid gap-3 md:grid-cols-2">
          {list.map((r) => (
            <div key={r.id} className="flex flex-col rounded-xl border border-line bg-panel p-4">
              <div className="flex items-start gap-2">
                <LaptopMinimalCheck size={16} className="mt-0.5 text-neon" />
                <div className="min-w-0 flex-1">
                  <p className="text-sm font-medium text-ink">{r.name}</p>
                  {r.description && <p className="text-xs text-dim">{r.description}</p>}
                </div>
              </div>
              <ol className="mt-3 flex-1 space-y-0.5 text-xs text-dim">
                {r.steps.map((s, i) => (
                  <li key={i}>
                    {i + 1}. {STEP_INFO[s.kind].label}
                    {describe(s) && <span className="text-mute"> · {describe(s)}</span>}
                  </li>
                ))}
              </ol>
              <div className="mt-3 flex items-center gap-2">
                <Button onClick={() => setRunning(r)} disabled={!isAdmin || !r.steps.length}>
                  <Play size={13} /> Ejecutar
                </Button>
                <Button kind="ghost" onClick={() => setEditing({ ...r, steps: [...r.steps] })}>
                  <Pencil size={13} /> Editar
                </Button>
                <button onClick={() => duplicate(r)} className="p-1 text-mute hover:text-ink" title="Duplicar">
                  <Copy size={13} />
                </button>
                <button onClick={() => remove(r)} className="ml-auto p-1 text-mute hover:text-bad" title="Borrar">
                  <Trash2 size={13} />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
      {editing && <RecipeEditor initial={editing} onClose={() => setEditing(null)} onSaved={load} />}
      {running && <RecipeRunner recipe={running} onClose={() => setRunning(null)} />}
      {dialog}
    </div>
  );
}

// ---------- Editor ----------

function RecipeEditor({ initial, onClose, onSaved }: { initial: Recipe; onClose: () => void; onSaved: () => void }) {
  const [r, setR] = useState(initial);
  const [catalog, setCatalog] = useState<AppCatalog | null>(null);
  const [profiles, setProfiles] = useState<ProfileView[]>([]);
  const [tweaks, setTweaks] = useState<{ id: string; name: string; category: string }[]>([]);
  const [tweakQuery, setTweakQuery] = useState("");
  const toast = useToast();

  useEffect(() => {
    appsApi.catalog().then(setCatalog).catch(() => {});
    profilesApi.list().then(setProfiles).catch(() => {});
    tweaksApi.index().then(setTweaks).catch(() => {});
  }, []);

  const setStep = (i: number, s: RecipeStep) => setR((x) => ({ ...x, steps: x.steps.map((y, j) => (j === i ? s : y)) }));
  const move = (i: number, d: number) =>
    setR((x) => {
      const steps = [...x.steps];
      const j = i + d;
      if (j < 0 || j >= steps.length) return x;
      [steps[i], steps[j]] = [steps[j], steps[i]];
      return { ...x, steps };
    });

  const lists = useMemo(
    () => [...(catalog?.lists ?? []).map((l) => ({ id: l.id, name: l.name })), ...(catalog?.presets ?? []).map((p) => ({ id: p.id, name: `${p.name} (predefinida)` }))],
    [catalog],
  );

  const save = async () => {
    if (!r.name.trim()) return toast("error", "Pon un nombre a la plantilla.");
    const incomplete = r.steps.find((s) => (s.kind === "installList" && !s.listId) || (s.kind === "profile" && !s.profileId) || (s.kind === "user" && !s.name.trim()) || (s.kind === "rename" && !s.newName.trim()) || (s.kind === "domain" && !s.domain.trim()));
    if (incomplete) return toast("error", `Completa el paso «${STEP_INFO[incomplete.kind].label}».`);
    try {
      await libraryApi.save("recipes", { ...r, name: r.name.trim() });
      toast("ok", "Plantilla guardada.");
      onSaved();
      onClose();
    } catch (e) {
      toast("error", String(e));
    }
  };

  const stepEditor = (s: RecipeStep, i: number) => {
    switch (s.kind) {
      case "installList":
        return (
          <select value={s.listId} onChange={(e) => setStep(i, { ...s, listId: e.target.value, listName: lists.find((l) => l.id === e.target.value)?.name ?? "" })} className={inputClass}>
            <option value="">— Elige una lista —</option>
            {lists.map((l) => (
              <option key={l.id} value={l.id}>
                {l.name}
              </option>
            ))}
          </select>
        );
      case "profile":
        return (
          <select value={s.profileId} onChange={(e) => setStep(i, { ...s, profileId: e.target.value, profileName: profiles.find((p) => p.id === e.target.value)?.name ?? "" })} className={inputClass}>
            <option value="">— Elige un perfil —</option>
            {profiles.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        );
      case "tweaks": {
        const q = norm(tweakQuery);
        return (
          <div>
            <div className="mb-1 flex flex-wrap gap-1">
              {s.ids.map((id) => (
                <span key={id} className="inline-flex items-center gap-1 rounded border border-line px-1.5 text-[11px] text-dim">
                  {tweaks.find((t) => t.id === id)?.name ?? id}
                  <button onClick={() => setStep(i, { ...s, ids: s.ids.filter((x) => x !== id) })} className="text-mute hover:text-bad">
                    ×
                  </button>
                </span>
              ))}
            </div>
            <input value={tweakQuery} onChange={(e) => setTweakQuery(e.target.value)} placeholder="Buscar ajuste para añadir…" className={inputClass} />
            {q && (
              <div className="mt-1 pane-sm overflow-y-auto rounded-md border border-line">
                {tweaks
                  .filter((t) => !s.ids.includes(t.id) && norm(t.name).includes(q))
                  .slice(0, 20)
                  .map((t) => (
                    <button
                      key={t.id}
                      onClick={() => {
                        setStep(i, { ...s, ids: [...s.ids, t.id] });
                        setTweakQuery("");
                      }}
                      className="block w-full px-2 py-1 text-left text-xs text-dim hover:bg-panel-2 hover:text-ink"
                    >
                      {t.name}
                    </button>
                  ))}
              </div>
            )}
          </div>
        );
      }
      case "user":
        return (
          <div className="grid grid-cols-3 gap-2">
            <input value={s.name} onChange={(e) => setStep(i, { ...s, name: e.target.value })} placeholder="Usuario" className={inputClass} />
            <input value={s.fullName} onChange={(e) => setStep(i, { ...s, fullName: e.target.value })} placeholder="Nombre completo" className={inputClass} />
            <label className="flex items-center gap-2 text-xs text-dim">
              <input type="checkbox" checked={s.admin} onChange={(e) => setStep(i, { ...s, admin: e.target.checked })} className="accent-[var(--color-neon)]" /> Administrador
            </label>
          </div>
        );
      case "rename":
        return <input value={s.newName} onChange={(e) => setStep(i, { ...s, newName: e.target.value })} placeholder="PC-CONTA-01 (admite {serie})" className={inputClass} />;
      case "domain":
        return (
          <div className="grid grid-cols-2 gap-2">
            <input value={s.domain} onChange={(e) => setStep(i, { ...s, domain: e.target.value })} placeholder="empresa.local" className={inputClass} />
            <input value={s.ou} onChange={(e) => setStep(i, { ...s, ou: e.target.value })} placeholder="OU (opcional)" className={inputClass} />
          </div>
        );
      default:
        return null;
    }
  };

  return (
    <Modal
      title={r.id ? "Editar plantilla" : "Nueva plantilla"}
      onClose={onClose}
      width="w-[760px] max-w-[95vw]"
      footer={
        <>
          <Button kind="ghost" onClick={onClose}>
            Cancelar
          </Button>
          <Button onClick={save}>Guardar</Button>
        </>
      }
    >
      <div className="space-y-3">
        <div className="grid grid-cols-2 gap-3">
          <input autoFocus value={r.name} onChange={(e) => setR({ ...r, name: e.target.value })} placeholder="Nombre de la plantilla (Equipo de oficina)" className={inputClass} />
          <input value={r.description} onChange={(e) => setR({ ...r, description: e.target.value })} placeholder="Descripción (opcional)" className={inputClass} />
        </div>
        {r.steps.map((s, i) => (
          <div key={i} className="rounded-lg border border-line p-3">
            <div className="mb-2 flex items-center gap-2">
              <span className="grid size-5 place-items-center rounded-full bg-neon/15 text-[11px] text-neon">{i + 1}</span>
              <span className="flex-1 text-sm text-ink">
                {STEP_INFO[s.kind].label} <span className="text-[11px] text-mute">· {STEP_INFO[s.kind].hint}</span>
              </span>
              <button onClick={() => move(i, -1)} className="p-1 text-mute hover:text-ink" title="Subir">
                <ArrowUp size={12} />
              </button>
              <button onClick={() => move(i, 1)} className="p-1 text-mute hover:text-ink" title="Bajar">
                <ArrowDown size={12} />
              </button>
              <button onClick={() => setR({ ...r, steps: r.steps.filter((_, j) => j !== i) })} className="p-1 text-mute hover:text-bad" title="Quitar">
                <Trash2 size={12} />
              </button>
            </div>
            {stepEditor(s, i)}
          </div>
        ))}
        <div>
          <p className="mb-1.5 text-[11px] text-mute">Añadir paso</p>
          <div className="flex flex-wrap gap-1.5">
            {(Object.keys(STEP_INFO) as Kind[]).map((k) => (
              <button key={k} onClick={() => setR({ ...r, steps: [...r.steps, structuredClone(NEW_STEP[k])] })} className="rounded-md border border-line px-2 py-1 text-xs text-dim hover:border-neon/50 hover:text-neon">
                + {STEP_INFO[k].label}
              </button>
            ))}
          </div>
        </div>
      </div>
    </Modal>
  );
}

// ---------- Ejecución ----------

type Status = "pending" | "running" | "ok" | "error" | "skipped";

function RecipeRunner({ recipe, onClose }: { recipe: Recipe; onClose: () => void }) {
  const needsUser = recipe.steps.some((s) => s.kind === "user");
  const needsDomain = recipe.steps.some((s) => s.kind === "domain");
  const [userPassword, setUserPassword] = useState("");
  const [domainUser, setDomainUser] = useState("");
  const [domainPassword, setDomainPassword] = useState("");
  const [stopOnError, setStopOnError] = useState(true);
  const [status, setStatus] = useState<Status[]>(recipe.steps.map(() => "pending"));
  const [messages, setMessages] = useState<string[]>(recipe.steps.map(() => ""));
  const [phase, setPhase] = useState<"ask" | "run" | "done">("ask");
  const toast = useToast();

  const set = (i: number, st: Status, msg = "") => {
    setStatus((x) => x.map((y, j) => (j === i ? st : y)));
    setMessages((x) => x.map((y, j) => (j === i ? msg : y)));
  };

  const run = async () => {
    if (needsUser && userPassword.length < 4) return toast("error", "Escribe la contraseña del usuario nuevo.");
    if (needsDomain && (!domainUser.trim() || !domainPassword)) return toast("error", "Escribe el usuario y la contraseña del dominio.");
    setPhase("run");
    // Si se une al dominio, el cambio de nombre va en la misma operación (dos reinicios pendientes chocan).
    const domainStep = recipe.steps.find((s) => s.kind === "domain");
    const renameStep = recipe.steps.find((s) => s.kind === "rename") as Extract<RecipeStep, { kind: "rename" }> | undefined;
    let failed = false;
    for (let i = 0; i < recipe.steps.length; i++) {
      const s = recipe.steps[i];
      if (failed && stopOnError) {
        set(i, "skipped", "No se hizo porque falló un paso anterior.");
        continue;
      }
      set(i, "running");
      try {
        const msg = await runStep(s, {
          userPassword,
          domainUser,
          domainPassword,
          renameInDomain: domainStep && renameStep ? renameStep.newName : "",
          skipRename: !!domainStep,
        });
        set(i, "ok", msg);
      } catch (e) {
        failed = true;
        set(i, "error", String(e));
      }
    }
    setUserPassword("");
    setDomainPassword("");
    setPhase("done");
  };

  const icon = (st: Status) =>
    ({
      pending: <Circle size={14} className="text-mute" />,
      running: <Loader2 size={14} className="animate-spin text-neon" />,
      ok: <CheckCircle2 size={14} className="text-ok" />,
      error: <XCircle size={14} className="text-bad" />,
      skipped: <SkipForward size={14} className="text-mute" />,
    })[st];

  return (
    <Modal
      title={`Preparar el equipo · ${recipe.name}`}
      onClose={phase === "run" ? () => {} : onClose}
      width="w-[640px] max-w-[95vw]"
      footer={
        phase === "ask" ? (
          <>
            <label className="mr-auto flex items-center gap-2 text-xs text-dim">
              <input type="checkbox" checked={stopOnError} onChange={(e) => setStopOnError(e.target.checked)} className="accent-[var(--color-neon)]" />
              Parar si un paso falla
            </label>
            <Button kind="ghost" onClick={onClose}>
              Cancelar
            </Button>
            <Button onClick={run}>
              <Play size={13} /> Empezar
            </Button>
          </>
        ) : (
          <Button onClick={onClose} disabled={phase === "run"}>
            {phase === "run" ? "Ejecutando…" : "Cerrar"}
          </Button>
        )
      }
    >
      {phase === "ask" && (needsUser || needsDomain) && (
        <div className="mb-4 grid gap-2 rounded-lg border border-line p-3">
          <p className="text-xs text-dim">Contraseñas para esta ejecución (no se guardan).</p>
          {needsUser && <input type="password" value={userPassword} onChange={(e) => setUserPassword(e.target.value)} placeholder="Contraseña del usuario nuevo" className={inputClass} />}
          {needsDomain && (
            <div className="grid grid-cols-2 gap-2">
              <input value={domainUser} onChange={(e) => setDomainUser(e.target.value)} placeholder="Usuario del dominio" className={inputClass} />
              <input type="password" value={domainPassword} onChange={(e) => setDomainPassword(e.target.value)} placeholder="Contraseña del dominio" className={inputClass} />
            </div>
          )}
        </div>
      )}
      <ol className="space-y-2">
        {recipe.steps.map((s, i) => (
          <li key={i} className="flex items-start gap-2.5">
            <span className="mt-0.5">{icon(status[i])}</span>
            <span className="min-w-0 flex-1">
              <span className="text-sm text-ink">
                {STEP_INFO[s.kind].label}
                {describe(s) && <span className="text-mute"> · {describe(s)}</span>}
              </span>
              {messages[i] && <span className={`block text-xs break-words ${status[i] === "error" ? "text-bad" : "text-dim"}`}>{messages[i]}</span>}
            </span>
          </li>
        ))}
      </ol>
      {phase === "done" && recipe.steps.some((s) => s.kind === "rename" || s.kind === "domain") && (
        <p className="mt-4 text-sm text-warn">Reinicia el equipo para completar el cambio de nombre o la unión al dominio.</p>
      )}
    </Modal>
  );
}

interface RunCtx {
  userPassword: string;
  domainUser: string;
  domainPassword: string;
  renameInDomain: string;
  skipRename: boolean;
}

async function withSerial(name: string): Promise<string> {
  if (!name.includes("{serie}")) return name;
  const sheet = await sheetApi.get();
  if (sheet.serial.startsWith("No disponible")) throw new Error("Este equipo no tiene número de serie: en el paso «Cambiar el nombre del equipo», quita {serie}.");
  const out = name.replace("{serie}", sheet.serial.replace(/[^A-Za-z0-9]/g, "").slice(-8));
  // Windows: 15 caracteres como mucho, letras, números y guiones.
  if (out.length > 15 || !/^[A-Za-z0-9-]+$/.test(out)) throw new Error(`«${out}» no es un nombre de equipo válido (máx. 15 letras, números o guiones).`);
  return out;
}

async function runStep(s: RecipeStep, ctx: RunCtx): Promise<string> {
  switch (s.kind) {
    case "restorePoint":
      await tweaksApi.createRestorePoint();
      return "Punto de restauración creado.";
    case "bloatware": {
      const apps = await systemApi.listApps();
      const remove = apps.filter((a) => a.installed && a.advice === "remove").map((a) => a.package);
      if (!remove.length) return "No había bloatware que quitar.";
      const r = await systemApi.removeApps(remove, true);
      const ok = r.results.filter((x) => x.ok).length;
      return `${ok} de ${remove.length} apps quitadas.`;
    }
    case "installList": {
      const catalog = await appsApi.catalog();
      const list = catalog.lists.find((l) => l.id === s.listId);
      const preset = catalog.presets.find((p) => p.id === s.listId);
      const apps = list?.apps ?? (preset ? catalog.apps.filter((a) => preset.apps.includes(a.id)) : []);
      if (!apps.length) throw new Error("La lista de programas ya no existe o está vacía.");
      const r = await appsApi.install(apps);
      const bad = r.filter((x) => !x.ok);
      if (bad.length === r.length) throw new Error(`No se instaló ninguno: ${bad[0]?.message ?? ""}`);
      return bad.length ? `${r.length - bad.length} instalados; fallaron: ${bad.map((b) => b.name).join(", ")}` : `${r.length} programas instalados.`;
    }
    case "profile": {
      const r = await profilesApi.apply(s.profileId, true);
      const failed = r.results.filter((x) => x.outcome === "failed");
      const done = r.results.filter((x) => x.outcome === "applied" || x.outcome === "ran").length;
      if (failed.length && !done) throw new Error(failed[0].message ?? "No se pudo aplicar el perfil.");
      return failed.length ? `${done} ajustes aplicados; fallaron ${failed.length}: ${failed.map((f) => f.name).join(", ")}` : `${done} ajustes aplicados.`;
    }
    case "tweaks": {
      let n = 0;
      for (const id of s.ids) {
        try {
          await tweaksApi.apply(id, true);
        } catch (e) {
          if (String(e).includes("tarea puntual")) await tweaksApi.run(id);
          else throw e;
        }
        n++;
      }
      return `${n} ajuste(s) aplicados.`;
    }
    case "user":
      await usersApi.create({ name: s.name.trim(), fullName: s.fullName.trim(), password: ctx.userPassword, admin: s.admin, passwordNeverExpires: false, mustChange: true });
      return `Usuario «${s.name}» creado (cambiará la contraseña al entrar).`;
    case "rename": {
      if (ctx.skipRename) return "Se hace junto con la unión al dominio.";
      const name = await withSerial(s.newName.trim());
      await domainApi.rename({ newName: name, user: "", password: "" });
      return `El equipo se llamará «${name}» al reiniciar.`;
    }
    case "domain": {
      const newName = ctx.renameInDomain ? await withSerial(ctx.renameInDomain) : "";
      await domainApi.join({ domain: s.domain.trim(), user: ctx.domainUser.trim(), password: ctx.domainPassword, ou: s.ou.trim(), newName });
      return `Unido a ${s.domain}${newName ? ` como «${newName}»` : ""}. Reinicia para completar.`;
    }
    case "diagnostics": {
      const d = await diagApi.run();
      const bad = d.findings.filter((f) => f.severity === "bad").length;
      const warn = d.findings.filter((f) => f.severity === "warn").length;
      return `${bad} problema(s), ${warn} aviso(s).`;
    }
  }
}
