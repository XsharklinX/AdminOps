import { ExternalLink, FileCheck2, Loader2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useToast } from "../components/feedback";
import { Button, Card, ErrorState, Loading } from "../components/ui";
import { defappsApi, pendingRows, type DefaultsView } from "../lib/api/defapps";

/**
 * Programas predeterminados: qué abre cada tipo de archivo que importa en la oficina y cambiarlo
 * «como en la oficina». En dominio, con la directiva de Windows (se aplica al iniciar sesión, a todos
 * los usuarios); fuera, cada fila abre la página de Configuración de ese programa.
 */
export function DefaultApps({ isAdmin }: { isAdmin: boolean }) {
  const [v, setV] = useState<DefaultsView | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [newUsers, setNewUsers] = useState("");
  const toast = useToast();

  const load = useCallback(() => {
    defappsApi
      .view()
      .then((x) => {
        setV(x);
        setError("");
      })
      .catch((e) => setError(String(e)));
  }, []);
  useEffect(load, [load]);

  if (error) return <ErrorState page message={error} onRetry={load} />;
  if (!v) return <Loading page />;

  const pending = pendingRows(v);
  const setOffice = (id: string, name: string) => {
    setV({ ...v, rows: v.rows.map((r) => (r.id === id ? { ...r, office: name || null } : r)) });
    defappsApi.setOffice(id, name).catch((e) => toast("error", String(e)));
  };
  const apply = async () => {
    setBusy(true);
    try {
      const n = await defappsApi.apply();
      toast("ok", `Puesto (${n} tipos de archivo). Windows lo aplica al volver a iniciar sesión, a todos los usuarios de este equipo.`);
      load();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };
  const forNewUsers = async () => {
    setBusy(true);
    try {
      const n = await defappsApi.applyNewUsers();
      setNewUsers(`Guardado (${n} tipos de archivo): los usuarios que se creen en este equipo ya salen así.`);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };
  const open = (name: string, machine: boolean) => void defappsApi.openSettings(name, machine).catch((e) => toast("error", String(e)));

  return (
    <div className="mx-auto max-w-(--page-max) p-6">
      <Card title="Programas predeterminados" icon={<FileCheck2 size={14} />}>
        <p className="mb-3 text-sm text-dim">
          Qué abre cada tipo de archivo en este equipo. En «En la oficina» eliges una vez lo que se usa; queda guardado en AdminOps y vale para cualquier equipo.
        </p>
        <div className="overflow-hidden rounded-lg border border-line">
          <table className="w-full text-sm">
            <thead className="bg-panel-2/60 text-left text-[11px] uppercase tracking-wide text-mute">
              <tr>
                <th className="px-3 py-2 font-medium">Tipo</th>
                <th className="px-3 py-2 font-medium">Abre ahora con</th>
                <th className="px-3 py-2 font-medium">En la oficina</th>
                {!v.domain && <th className="px-3 py-2" />}
              </tr>
            </thead>
            <tbody className="divide-y divide-line/70">
              {v.rows.map((r) => {
                const differs = !!r.office && r.office !== r.current;
                const chosen = r.candidates.find((c) => c.name === r.office);
                return (
                  <tr key={r.id}>
                    <td className="px-3 py-2 text-ink">{r.label}</td>
                    <td className={`px-3 py-2 ${differs ? "text-warn" : "text-dim"}`}>{r.current || <span className="text-mute">lo de Windows</span>}</td>
                    <td className="px-3 py-2">
                      <select
                        aria-label={`${r.label}: en la oficina`}
                        value={r.office ?? ""}
                        onChange={(e) => setOffice(r.id, e.target.value)}
                        className="max-w-[16rem] rounded border border-line bg-void/60 px-1.5 py-1 text-xs text-ink outline-none"
                      >
                        <option value="">Sin elegir</option>
                        {r.office && !chosen && <option value={r.office}>{r.office} (no está en este equipo)</option>}
                        {r.candidates.map((c) => (
                          <option key={c.name} value={c.name}>
                            {c.name}
                          </option>
                        ))}
                      </select>
                    </td>
                    {!v.domain && (
                      <td className="px-3 py-2 text-right">
                        {differs && chosen && (
                          <button type="button" onClick={() => open(chosen.name, chosen.machine)} className="inline-flex items-center gap-1 text-[12px] text-neon hover:underline">
                            Cambiar <ExternalLink size={11} />
                          </button>
                        )}
                      </td>
                    )}
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
        {v.domain ? (
          <div className="mt-3 flex flex-wrap items-center gap-3">
            <Button disabled={!isAdmin || busy || pending.length === 0} onClick={apply}>
              {busy ? <Loader2 size={14} className="animate-spin" /> : null}
              Como en la oficina{pending.length ? ` · ${pending.length} ${pending.length === 1 ? "cambio" : "cambios"}` : ""}
            </Button>
            <span className="text-xs text-mute">Windows lo pone al volver a iniciar sesión, para todos los usuarios, y lo repone si una actualización lo cambia.</span>
            {v.policy && (
              <button type="button" className="text-xs text-mute hover:text-ink" onClick={() => void defappsApi.removePolicy().then(load, (e) => toast("error", String(e)))}>
                Quitar lo puesto por AdminOps
              </button>
            )}
            {!isAdmin && <span className="text-xs text-warn">Hace falta abrir AdminOps como administrador.</span>}
          </div>
        ) : (
          <p className="mt-3 text-xs text-mute">
            Este equipo no está en un dominio: Windows 11 no deja que otro programa cambie lo predeterminado. «Cambiar» abre la página de ese programa en Configuración; allí, «Establecer predeterminado» lo deja puesto con un clic.
          </p>
        )}
        {v.rows.some((r) => r.office && r.candidates.some((c) => c.name === r.office)) && (
          <div className="mt-3 flex flex-wrap items-center gap-3 border-t border-line pt-3">
            <Button size="sm" kind="secondary" disabled={!isAdmin || busy} onClick={forNewUsers}>
              Como en la oficina para los usuarios nuevos
            </Button>
            {newUsers ? <span className="text-xs text-ok">{newUsers}</span> : <span className="text-xs text-mute">Los usuarios que se creen después en este equipo ya salen con lo de la oficina, también fuera de un dominio.</span>}
          </div>
        )}
      </Card>
    </div>
  );
}
