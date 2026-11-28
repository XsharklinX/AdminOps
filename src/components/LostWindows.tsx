import { Check } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { lostWinApi, type LostWin } from "../lib/api";
import { useToast } from "./feedback";
import { Button, Card, Notice } from "./ui";

/** «Excel», «Excel y Word», «Excel, Word y 2 más»: los programas, sin repetir. */
export function appsText(list: LostWin[]): string {
  const apps = [...new Set(list.map((w) => w.app))];
  if (apps.length <= 2) return apps.join(" y ");
  return `${apps[0]}, ${apps[1]} y ${apps.length - 2} más`;
}

/**
 * Ventanas que quedaron fuera de la pantalla (un monitor que se quitó, otra resolución): las lista y las
 * trae al monitor principal. Va encima de los hallazgos de «Pantalla o monitor».
 */
export function LostWindows() {
  const [list, setList] = useState<LostWin[] | null>(null);
  const [brought, setBrought] = useState(false);
  const toast = useToast();

  const look = useCallback(async () => {
    try {
      setList(await lostWinApi.list());
      setBrought(false);
    } catch (e) {
      toast("error", String(e));
      setList((l) => l ?? []);
    }
  }, [toast]);
  useEffect(() => void look(), [look]);

  if (list === null) return null;

  const bring = async () => {
    try {
      await lostWinApi.bring(list.map((w) => w.hwnd));
      setBrought(true);
      setList([]);
    } catch (e) {
      toast("error", String(e));
    }
  };

  if (brought) {
    return (
      <Card className="col-span-12">
        <div className="flex flex-wrap items-center justify-between gap-3 text-sm">
          <span className="flex items-center gap-1.5 text-ok">
            <Check size={14} /> Traídas a la pantalla principal.
          </span>
          <Button kind="ghost" size="sm" onClick={look}>
            Volver a mirar
          </Button>
        </div>
      </Card>
    );
  }

  if (list.length > 0) {
    return (
      <Card className="col-span-12 border-warn/40">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <Notice tone="warn" className="min-w-0">
            {list.length === 1 ? "1 ventana está fuera de la pantalla" : `${list.length} ventanas están fuera de la pantalla`}: {appsText(list)}
          </Notice>
          <Button onClick={bring}>Traer las ventanas perdidas</Button>
        </div>
      </Card>
    );
  }

  return (
    <div className="col-span-12 flex items-center justify-between gap-3 px-1 text-[13px] text-mute">
      <span>Ninguna ventana fuera de la pantalla</span>
      <Button kind="ghost" size="sm" onClick={look}>
        Volver a mirar
      </Button>
    </div>
  );
}
