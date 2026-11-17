// ¿Es el perfil o el equipo? Crea un usuario LOCAL temporal (nunca del dominio)
// con una contraseña de un solo uso fácil de dictar. Se entra con él y se
// prueba: si con él funciona, el problema está en el perfil de la persona; si no,
// en el equipo. Se borra con su carpeta al pulsar el botón o, si se olvida, a las 24 h.
import { Loader2, ShieldAlert, Trash2, UserPlus, UserRound } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useConfirm, useToast } from "../feedback";
import { Card, smallBtn, softBtn } from "../ui";
import { diagnoseApi, type TestUser } from "../../lib/api";
import { Copyable } from "../Copyable";

/** «23 h 59 min», «12 min», «ya toca borrarlo». */
export function timeLeft(expires: number, now = Date.now()): string {
  const s = Math.floor(expires - now / 1000);
  if (s <= 0) return "ya toca borrarlo";
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  return h > 0 ? `${h} h ${String(m).padStart(2, "0")} min` : `${Math.max(1, m)} min`;
}

// La contraseña solo existe aquí mientras AdminOps esté abierta: no se guarda en ningún sitio.
let shown: { name: string; password: string } | null = null;

export function TestUserCard({ isAdmin }: { isAdmin: boolean }) {
  const toast = useToast();
  const { confirm, dialog } = useConfirm();
  const [user, setUser] = useState<TestUser | null | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<"create" | "delete" | null>(null);
  const [, tick] = useState(0);

  const load = useCallback(() => {
    diagnoseApi.testUser().then(
      (u) => {
        setUser(u);
        setError(null);
      },
      (e) => {
        setUser(null);
        setError(String(e));
      },
    );
  }, []);

  useEffect(() => {
    load();
    const t = window.setInterval(() => tick((n) => n + 1), 30_000);
    return () => window.clearInterval(t);
  }, [load]);

  const create = async () => {
    setBusy("create");
    try {
      const c = await diagnoseApi.createTestUser();
      shown = { name: c.name, password: c.password };
      toast("ok", `Usuario de prueba «${c.name}» creado.`);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
      load();
    }
  };

  const remove = async () => {
    const ok = await confirm({
      title: "¿Borrar el usuario de prueba?",
      danger: true,
      confirmLabel: "Borrar el de prueba",
      body: <p>Se borra la cuenta con su carpeta de perfil. No toca el perfil de ninguna otra persona.</p>,
    });
    if (!ok) return;
    setBusy("delete");
    try {
      await diagnoseApi.deleteTestUser();
      shown = null;
      toast("ok", "Usuario de prueba borrado.");
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
      load();
    }
  };

  const password = user && shown?.name === user.name ? shown.password : null;

  return (
    <Card id="testuser" title="¿Es el perfil o el equipo?" icon={<UserRound size={14} />} className="col-span-12">
      <p className="mb-3 text-xs text-dim">
        Crea un usuario local de prueba, entra con él y prueba lo que falla. Si con él funciona, el problema está en el perfil de la persona; si no, en el equipo. Es una cuenta de este equipo (no del dominio), con una contraseña de un solo uso; se borra sola a las 24 h.
      </p>
      {error && <p className="mb-2 text-sm text-bad">{error}</p>}

      {user === undefined ? (
        <p className="flex items-center gap-2 text-sm text-mute">
          <Loader2 size={14} className="animate-spin" /> Mirando…
        </p>
      ) : user ? (
        <div className="space-y-3">
          <div className="flex flex-wrap items-center gap-x-6 gap-y-2 rounded-lg border border-line bg-void/40 px-3 py-2.5">
            <div>
              <div className="text-[11px] text-mute">Usuario</div>
              <Copyable value={user.name} className="font-mono text-sm text-ink">
                {user.name}
              </Copyable>
            </div>
            <div>
              <div className="text-[11px] text-mute">Contraseña de un solo uso</div>
              {password ? (
                <Copyable value={password} className="font-mono text-sm text-ink">
                  {password}
                </Copyable>
              ) : (
                <span className="text-xs text-dim">Solo se enseña al crearlo.</span>
              )}
            </div>
            <div>
              <div className="text-[11px] text-mute">Se borra en</div>
              <span className="text-sm text-ink">{timeLeft(user.expires)}</span>
            </div>
            <button onClick={() => void remove()} disabled={!isAdmin || busy !== null || user.signedIn} className={`${smallBtn} ml-auto`} title={user.signedIn ? "Tiene la sesión abierta: ciérrala primero" : "Borra la cuenta con su carpeta"}>
              {busy === "delete" ? <Loader2 size={12} className="animate-spin" /> : <Trash2 size={12} />} Borrar el de prueba
            </button>
          </div>
          <ol className="list-decimal space-y-0.5 pl-5 text-xs text-dim">
            <li>Cierra la sesión de la persona (Win + L) y entra con el usuario de prueba.</li>
            <li>Haz lo que falla. ¿Funciona con él? Entonces es su perfil, no el equipo.</li>
            <li>Vuelve a entrar como tú y pulsa «Borrar el de prueba».</li>
          </ol>
        </div>
      ) : (
        <div className="flex flex-wrap items-center gap-3">
          <button onClick={() => void create()} disabled={!isAdmin || busy !== null} className={softBtn}>
            {busy === "create" ? <Loader2 size={12} className="animate-spin" /> : <UserPlus size={12} />} Crear usuario de prueba
          </button>
          {!isAdmin && (
            <span className="flex items-center gap-1 text-xs text-warn">
              <ShieldAlert size={12} /> Requiere ejecutar AdminOps como administrador.
            </span>
          )}
        </div>
      )}
      {dialog}
    </Card>
  );
}
