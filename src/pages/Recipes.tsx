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
  RotateCw,
  SkipForward,
  Trash2,
  XCircle,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { Button, EmptyState, inputClass, Intro, Loading, Modal } from "../components/ui";
import { api, logQuietly, appsApi, diagApi, domainApi, libraryApi, planApi, profilesApi, sheetApi, systemApi, tweaksApi, usersApi, type AppCatalog, type PlanState, type PlanStatus, type PlanWait, type ProfileView, type Recipe, type RecipeStep } from "../lib/api";
import { checkedMessage, fit, passwordsNeeded, pauseNeeded, resumeBanner, shouldResume, takeResumeRequest } from "../lib/planResume";
import { riskyTweaks, withTweakIds } from "../lib/recipeEdit";
import { RISK } from "../lib/risk";
import { norm } from "../lib/contacts";
import { NeedsAdmin } from "../components/AdminBanner";

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
  reboot: { label: "Reiniciar el equipo", hint: "El plan sigue solo al volver: AdminOps se abre y continúa." },
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
  reboot: { kind: "reboot" },
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
      return s.names?.length && s.names.length === s.ids.length ? s.names.join(", ") : `${s.ids.length} ajuste(s)`;
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
  const [running, setRunning] = useState<{ recipe: Recipe; resume?: PlanState } | null>(null);
  // Un plan guardado que no se retomó solo (no hubo reinicio): aviso discreto, sin navegar solo.
  const [pending, setPending] = useState<PlanState | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(() => libraryApi.list("recipes").then(setList).catch((e) => toast("error", String(e))), [toast]);
  useEffect(() => {
    void load();
  }, [load]);

  // Si AdminOps se abrió tras reiniciar, se retoma el plan donde se quedó; si hay un plan
  // guardado pero no hubo reinicio, solo se avisa.
  const refreshPending = useCallback(
    (resumeNow: boolean) =>
      planApi
        .load()
        .then((l) => {
          if (!l) return setPending(null);
          if (resumeNow) {
            setPending(null);
            setRunning({ recipe: l.state.recipe as Recipe, resume: l.state });
          } else setPending(shouldResume(l) ? null : l.state);
        })
        .catch(logQuietly("Recipes")),
    [],
  );
  useEffect(() => {
    void refreshPending(takeResumeRequest());
  }, [refreshPending]);

  const stopPending = async () => {
    await planApi.clear().catch(logQuietly("Recipes"));
    setPending(null);
  };
  const rebootPending = async () => {
    if (!pending) return;
    const name = (pending.recipe as Recipe | undefined)?.name ?? "";
    const go = await confirm({
      title: "Reiniciar y seguir",
      body: `El equipo se reiniciará en unos segundos${name ? ` y el plan «${name}» seguirá` : " y el plan seguirá"} al volver.`,
      confirmLabel: "Reiniciar",
    });
    if (!go) return;
    try {
      const task = await planApi.reboot(pending);
      if (!task) toast("info", "Al volver, abre AdminOps y el plan seguirá.");
    } catch (e) {
      toast("error", String(e));
    }
  };

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
    <div className="mx-auto max-w-(--page-max) space-y-4 p-6">
      <div className="flex items-center justify-between gap-3">
        <Intro id="recipes:recipes">
          Una plantilla reúne todo lo que haces en un equipo nuevo (quitar bloatware, instalar programas, crear el usuario, unir al dominio…) para hacerlo de una
          vez y siempre igual. Viajan con AdminOps (en el USB con el portable).
        </Intro>
        <Button onClick={() => setEditing({ id: "", name: "", description: "", steps: [] })}>
          <Plus size={14} /> Nueva plantilla
        </Button>
      </div>
      {!isAdmin && <NeedsAdmin>Preparar un equipo con una plantilla requiere administrador.</NeedsAdmin>}
      {pending && !running && (
        <div className="flex flex-wrap items-center gap-2 rounded-lg border border-warn/30 bg-warn/5 px-3 py-2 text-xs text-warn" role="status">
          <span className="min-w-0 flex-1">
            Hay un plan sin terminar{(pending.recipe as Recipe | undefined)?.name ? `: «${(pending.recipe as Recipe).name}»` : ""}, esperando un reinicio.
          </span>
          <Button size="sm" onClick={() => void rebootPending()} disabled={!isAdmin}>
            <RotateCw size={13} /> Reiniciar y seguir
          </Button>
          <Button kind="ghost" size="sm" onClick={() => void stopPending()}>
            Parar el plan
          </Button>
        </div>
      )}
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
            <div key={r.id} className="flex flex-col rounded-2xl border border-line/70 bg-panel shadow-elev-1 p-4">
              <div className="flex items-start gap-2">
                <LaptopMinimalCheck size={16} className="mt-0.5 text-neon" />
                <div className="min-w-0 flex-1">
                  <p className="flex items-center gap-2 text-sm font-medium text-ink">
                    {r.name}
                    {r.isNew && !r.uses && <span className="rounded-full border border-neon/40 bg-neon/10 px-2 py-px text-[11px] font-medium text-neon">Nueva</span>}
                  </p>
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
                <Button onClick={() => setRunning({ recipe: r })} disabled={!isAdmin || !r.steps.length}>
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
      {running && (
        <RecipeRunner
          recipe={running.recipe}
          resume={running.resume}
          isAdmin={isAdmin}
          onUsed={() => void load()}
          onClose={() => {
            setRunning(null);
            void refreshPending(false);
          }}
        />
      )}
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
    appsApi.catalog().then(setCatalog).catch(logQuietly("Recipes"));
    profilesApi.list().then(setProfiles).catch(logQuietly("Recipes"));
    tweaksApi.index().then(setTweaks).catch(logQuietly("Recipes"));
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
                  <button onClick={() => setStep(i, withTweakIds(s, s.ids.filter((x) => x !== id), tweaks))} className="text-mute hover:text-bad">
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
                        setStep(i, withTweakIds(s, [...s.ids, t.id], tweaks));
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
      width="w-[760px]"
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

type Status = PlanStatus;

type Phase = "ask" | "run" | "check" | "reboot" | "stuck" | "done";

function RecipeRunner({ recipe, resume, isAdmin, onUsed, onClose }: { recipe: Recipe; resume?: PlanState | null; isAdmin: boolean; onUsed: () => void; onClose: () => void }) {
  const total = recipe.steps.length;
  // Al retomar tras un reinicio, las contraseñas solo se piden para los pasos que quedan.
  const [from, setFrom] = useState(0);
  const need = passwordsNeeded(recipe.steps, from);
  const [userPassword, setUserPassword] = useState("");
  const [domainUser, setDomainUser] = useState("");
  const [domainPassword, setDomainPassword] = useState("");
  const [stopOnError, setStopOnError] = useState(resume?.stopOnError ?? true);
  const [status, setStatus] = useState<Status[]>(() => fit<Status>(resume?.statuses, total, "pending"));
  const [messages, setMessages] = useState<string[]>(() => fit(resume?.messages, total, ""));
  const [phase, setPhase] = useState<Phase>(resume ? "check" : "ask");
  const [waiting, setWaiting] = useState<{ step: number; wait: PlanWait } | null>(resume ? { step: resume.waitingStep, wait: resume.wait } : null);
  const [stuckText, setStuckText] = useState("");
  const [busyReboot, setBusyReboot] = useState(false);
  // ¿Quedó creada la tarea que abre AdminOps al volver? Si no, el técnico lo abre a mano.
  const [autoResume, setAutoResume] = useState(true);
  const started = useRef(false);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const sync = (st: Status[], msgs: string[]) => {
    setStatus([...st]);
    setMessages([...msgs]);
  };

  /** Lo que se guarda en disco para seguir tras el reinicio. */
  const snapshot = (st: Status[], msgs: string[], step: number, wait: PlanWait): PlanState => ({
    recipe,
    statuses: st,
    messages: msgs,
    stopOnError,
    waitingStep: step,
    wait,
    hostBefore: "",
    bootTime: 0,
    savedAt: 0,
  });

  /** Ejecuta los pasos desde `start`. Si uno pide reiniciar, para y deja el plan guardado. */
  const runFrom = async (start: number, st: Status[], msgs: string[]) => {
    // Si se une al dominio, el cambio de nombre va en la misma operación (dos reinicios pendientes chocan).
    const domainStep = recipe.steps.find((s) => s.kind === "domain");
    const renameStep = recipe.steps.find((s) => s.kind === "rename") as Extract<RecipeStep, { kind: "rename" }> | undefined;
    setPhase("run");
    let failed = false;
    for (let i = start; i < total; i++) {
      const s = recipe.steps[i];
      if (failed && stopOnError) {
        st[i] = "skipped";
        msgs[i] = "No se hizo porque falló un paso anterior.";
        sync(st, msgs);
        continue;
      }
      st[i] = "running";
      msgs[i] = "";
      sync(st, msgs);
      const ctx: RunCtx = {
        userPassword,
        domainUser,
        domainPassword,
        renameInDomain: domainStep && renameStep ? renameStep.newName : "",
        skipRename: !!domainStep,
        wait: null,
      };
      try {
        msgs[i] = await runStep(s, ctx);
        st[i] = "ok";
      } catch (e) {
        failed = true;
        st[i] = "error";
        msgs[i] = String(e);
      }
      sync(st, msgs);
      if (st[i] === "ok" && ctx.wait && pauseNeeded(ctx.wait, i, total)) {
        // Hay que reiniciar para seguir: el plan se guarda y se espera al técnico.
        const wait = ctx.wait;
        setWaiting({ step: i, wait });
        const task = await planApi.save(snapshot(st, msgs, i, wait)).catch((e) => {
          logQuietly("Recipes")(e);
          return false;
        });
        setAutoResume(task === true);
        setPhase("reboot");
        return;
      }
    }
    setUserPassword("");
    setDomainPassword("");
    setPhase("done");
    // El plan terminó: ya no hay nada que retomar.
    void planApi.clear().catch(logQuietly("Recipes"));
  };

  /** Antes de empezar, los ajustes de riesgo medio o alto se listan y se pide confirmar. */
  const confirmRisky = async (): Promise<boolean> => {
    if (!recipe.steps.some((s) => s.kind === "tweaks")) return true;
    let risky;
    try {
      risky = riskyTweaks(recipe.steps, await tweaksApi.index());
    } catch (e) {
      toast("error", `No se pudo comprobar el riesgo de los ajustes: ${String(e)}`);
      return false;
    }
    if (!risky.length) return true;
    return confirm({
      title: "Esta plantilla incluye ajustes de riesgo",
      body: (
        <ul className="space-y-1.5">
          {risky.map((t) => (
            <li key={t.id}>
              <span className="text-ink">{t.name}</span>{" "}
              <span className={`rounded border px-1.5 py-px text-[11px] font-medium ${RISK[t.risk].cls}`}>{RISK[t.risk].label}</span>
              {t.note && <span className="block text-xs text-mute">{t.note}</span>}
            </li>
          ))}
        </ul>
      ),
      confirmLabel: "Ejecutar",
      danger: risky.some((t) => t.risk === "high"),
    });
  };

  const run = async () => {
    if (need.user && userPassword.length < 4) return toast("error", "Escribe la contraseña del usuario nuevo.");
    if (need.domain && (!domainUser.trim() || !domainPassword)) return toast("error", "Escribe el usuario y la contraseña del dominio.");
    if (from === 0) {
      if (!(await confirmRisky())) return;
      if (recipe.id) void libraryApi.touch("recipes", recipe.id).then(onUsed, logQuietly("Recipes"));
      await runFrom(0, recipe.steps.map(() => "pending" as Status), recipe.steps.map(() => ""));
    } else {
      await runFrom(from, [...status], [...messages]);
    }
  };

  /** Al volver del reinicio: comprueba el paso que lo pedía y, si quedó bien, sigue con el siguiente. */
  const checkAndContinue = async () => {
    if (!resume) return;
    setPhase("check");
    const r = await planApi.check(resume).catch((e) => ({ ok: false, message: String(e) }));
    const st = fit<Status>(resume.statuses, total, "pending");
    const msgs = fit(resume.messages, total, "");
    const w = resume.waitingStep;
    if (!r.ok) {
      st[w] = "error";
      msgs[w] = r.message;
      sync(st, msgs);
      setStuckText(r.message);
      setPhase("stuck");
      return;
    }
    st[w] = "ok";
    msgs[w] = checkedMessage(r.message);
    sync(st, msgs);
    setFrom(w + 1);
    const rest = passwordsNeeded(recipe.steps, w + 1);
    // Al abrirse, la app aún puede no saber si es administrador: se pregunta de verdad antes de decidir.
    const admin = await api.isAdmin().catch(() => isAdmin);
    if (rest.user || rest.domain || !admin) {
      // Las contraseñas no se guardan: se vuelven a pedir. Sin administrador tampoco se puede seguir solo.
      setPhase("ask");
      return;
    }
    await runFrom(w + 1, st, msgs);
  };

  // Al abrirse con un plan retomado, se comprueba una sola vez.
  useEffect(() => {
    if (!resume || started.current) return;
    started.current = true;
    void checkAndContinue();
  });

  const rebootNow = async () => {
    if (!waiting) return;
    setBusyReboot(true);
    try {
      await planApi.reboot(snapshot(status, messages, waiting.step, waiting.wait));
    } catch (e) {
      toast("error", String(e));
      setBusyReboot(false);
    }
  };

  const stopPlan = async () => {
    await planApi.clear().catch(logQuietly("Recipes"));
    onClose();
  };

  /**
   * Cerrar el diálogo (X, Esc o «Cancelar») con el plan guardado a medias no lo deja colgado:
   * se pregunta si parar el plan. «Cancelar» en esa pregunta deja el diálogo como estaba.
   */
  const leave = async () => {
    const saved = phase === "reboot" || phase === "stuck" || (phase === "ask" && from > 0);
    if (!saved) return onClose();
    const stop = await confirm({
      title: "¿Parar el plan?",
      body: "El plan está a medias. Si cierras ahora se para y se borra lo guardado: no seguirá después de reiniciar.",
      confirmLabel: "Parar el plan",
      danger: true,
    });
    if (stop) await stopPlan();
  };

  const icon = (st: Status) =>
    ({
      pending: <Circle size={14} className="text-mute" />,
      running: <Loader2 size={14} className="animate-spin text-neon" />,
      ok: <CheckCircle2 size={14} className="text-ok" />,
      error: <XCircle size={14} className="text-bad" />,
      skipped: <SkipForward size={14} className="text-mute" />,
    })[st];

  const busyPhase = phase === "run" || phase === "check";

  return (
    <>
      <Modal
        title={`Preparar el equipo · ${recipe.name}`}
        onClose={busyPhase || busyReboot ? () => {} : () => void leave()}
        width="w-[640px]"
        footer={
          phase === "ask" ? (
            <>
              {from === 0 && (
                <label className="mr-auto flex items-center gap-2 text-xs text-dim">
                  <input type="checkbox" checked={stopOnError} onChange={(e) => setStopOnError(e.target.checked)} className="accent-[var(--color-neon)]" />
                  Parar si un paso falla
                </label>
              )}
              {from > 0 && (
                <Button kind="ghost" onClick={() => void stopPlan()}>
                  Parar el plan
                </Button>
              )}
              <Button kind="ghost" onClick={() => void leave()}>
                Cancelar
              </Button>
              <Button onClick={run} disabled={from > 0 && !isAdmin}>
                <Play size={13} /> {from > 0 ? "Seguir" : "Empezar"}
              </Button>
            </>
          ) : phase === "reboot" ? (
            <>
              <Button kind="ghost" onClick={() => void stopPlan()} disabled={busyReboot}>
                Parar el plan
              </Button>
              <Button onClick={() => void rebootNow()} disabled={busyReboot}>
                {busyReboot ? <Loader2 size={13} className="animate-spin" /> : <RotateCw size={13} />} Reiniciar y seguir
              </Button>
            </>
          ) : phase === "stuck" ? (
            <>
              <Button kind="ghost" onClick={() => void stopPlan()}>
                Parar el plan
              </Button>
              <Button onClick={() => void checkAndContinue()}>Reintentar</Button>
            </>
          ) : (
            <Button onClick={onClose} disabled={busyPhase}>
              {busyPhase ? "Ejecutando…" : "Cerrar"}
            </Button>
          )
        }
      >
        {resume && (
          <p className="mb-4 rounded-lg border border-neon/40 bg-neon/10 px-3 py-2 text-xs font-medium text-neon" role="status">
            {resumeBanner(resume.waitingStep, total)}
          </p>
        )}
        {phase === "ask" && from > 0 && !isAdmin && <NeedsAdmin className="mb-4">Para seguir con el plan, abre AdminOps como administrador.</NeedsAdmin>}
        {phase === "ask" && (need.user || need.domain) && (
          <div className="mb-4 grid gap-2 rounded-lg border border-line p-3">
            <p className="text-xs text-dim">Contraseñas para esta ejecución (no se guardan).</p>
            {need.user && <input type="password" value={userPassword} onChange={(e) => setUserPassword(e.target.value)} placeholder="Contraseña del usuario nuevo" className={inputClass} />}
            {need.domain && (
              <div className="grid grid-cols-2 gap-2">
                <input value={domainUser} onChange={(e) => setDomainUser(e.target.value)} placeholder="Usuario del dominio" className={inputClass} />
                <input type="password" value={domainPassword} onChange={(e) => setDomainPassword(e.target.value)} placeholder="Contraseña del dominio" className={inputClass} />
              </div>
            )}
          </div>
        )}
        {phase === "reboot" && (
          <p className="mb-4 rounded-lg border border-warn/30 bg-warn/5 px-3 py-2 text-xs text-warn" role="status">
            {autoResume ? "Hay que reiniciar para seguir. AdminOps se abrirá solo y continuará." : "Hay que reiniciar para seguir. Al volver, abre AdminOps y el plan seguirá."}
          </p>
        )}
        {phase === "stuck" && (
          <p className="mb-4 rounded-lg border border-bad/40 bg-bad/5 px-3 py-2 text-xs text-bad" role="alert">
            {stuckText}
          </p>
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
      {dialog}
    </>
  );
}

interface RunCtx {
  userPassword: string;
  domainUser: string;
  domainPassword: string;
  renameInDomain: string;
  skipRename: boolean;
  /** Lo que el paso pide comprobar tras reiniciar (lo rellena `runStep`). */
  wait: PlanWait | null;
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
      ctx.wait = { kind: "rename", expect: name, name: "" };
      return `El equipo se llamará «${name}» al reiniciar.`;
    }
    case "domain": {
      const newName = ctx.renameInDomain ? await withSerial(ctx.renameInDomain) : "";
      await domainApi.join({ domain: s.domain.trim(), user: ctx.domainUser.trim(), password: ctx.domainPassword, ou: s.ou.trim(), newName });
      ctx.wait = { kind: "domain", expect: s.domain.trim(), name: newName };
      return `Unido a ${s.domain}${newName ? ` como «${newName}»` : ""}. Reinicia para completar.`;
    }
    case "reboot":
      // El reinicio en sí lo hace el botón «Reiniciar y seguir»: aquí solo se apunta qué comprobar al volver.
      ctx.wait = { kind: "reboot", expect: "", name: "" };
      return "El equipo se reiniciará para seguir con el plan.";
    case "diagnostics": {
      const d = await diagApi.run();
      const bad = d.findings.filter((f) => f.severity === "bad").length;
      const warn = d.findings.filter((f) => f.severity === "warn").length;
      return `${bad} problema(s), ${warn} aviso(s).`;
    }
  }
}
