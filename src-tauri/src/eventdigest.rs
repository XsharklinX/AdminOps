//! El Visor de eventos, traducido y agrupado: los errores de los últimos días
//! juntos por causa (no evento a evento), ordenados por los que de verdad
//! importan, con una frase de qué significan y qué hacer. Los ruidosos
//! conocidos (los que Windows registra siempre) se apartan.

use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct RawEvent {
    pub log: String,
    pub provider: String,
    pub id: u32,
    /// 1 crítico · 2 error · 3 aviso
    pub level: u32,
    pub t: u64,
    pub message: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EventGroup {
    pub provider: String,
    pub id: u32,
    pub log: String,
    pub count: usize,
    pub last: u64,
    /// matter (importa) · watch (vigilar) · noise (ruido)
    pub weight: &'static str,
    pub title: String,
    pub meaning: String,
    pub todo: String,
    /// Pantalla de AdminOps donde seguir.
    pub page: &'static str,
    /// Primera línea del mensaje original, por si se quiere el detalle.
    pub sample: String,
}

/// (proveedor contiene, id, peso, título, significado, qué hacer, pantalla). id 0: cualquiera de ese proveedor.
type Known = (&'static str, u32, &'static str, &'static str, &'static str, &'static str, &'static str);

const KNOWN: &[Known] = &[
    // Lo que importa
    ("disk", 7, "matter", "Sectores dañados en un disco", "El disco no pudo leer o escribir un bloque: tiene sectores defectuosos.", "Copia de seguridad ya y revisa la salud del disco.", "space"),
    ("disk", 11, "matter", "Error del controlador del disco", "Falló la comunicación con el disco.", "Revisa el cable y el puerto; mira la salud del disco.", "space"),
    ("disk", 51, "matter", "Error al escribir en el disco (paginación)", "Windows no pudo escribir en el disco.", "Revisa la salud del disco y su conexión.", "space"),
    ("disk", 153, "matter", "Operaciones de disco reintentadas", "El disco tardó en responder y Windows tuvo que repetir lecturas.", "Señal temprana de un disco que falla o de un cable flojo.", "space"),
    ("ntfs", 55, "matter", "Sistema de archivos dañado", "La estructura de archivos de un volumen tiene errores.", "Repara el volumen (chkdsk) desde Discos.", "space"),
    ("ntfs", 98, "matter", "El volumen necesita reparación", "Windows marcó un volumen para revisarlo.", "Repara el volumen desde Discos.", "space"),
    ("volmgr", 161, "matter", "No se pudo crear el volcado de memoria", "El equipo se colgó y no quedó volcado para saber por qué.", "Revisa el espacio libre y el archivo de paginación.", "boots"),
    ("kernel-power", 41, "matter", "Apagado inesperado", "El equipo se reinició sin apagarse bien: corte de luz, cuelgue o pantallazo.", "Mira Arranques y cuelgues: si coincide con pantallazos, ahí está el driver.", "boots"),
    ("whea-logger", 0, "matter", "El hardware informó de un error", "El procesador, la memoria o el bus PCIe avisaron de un fallo.", "Comprueba temperaturas y la memoria; si se repite, puede ser una pieza.", "hardware"),
    ("bugcheck", 1001, "matter", "Pantallazo azul", "Windows se paró por un error grave.", "Arranques y cuelgues dice qué driver lo provocó.", "boots"),
    ("wer-systemerrorreporting", 1001, "matter", "Pantallazo azul", "Windows se paró por un error grave.", "Arranques y cuelgues dice qué driver lo provocó.", "boots"),
    ("eventlog", 6008, "matter", "El apagado anterior fue inesperado", "La última vez el equipo no se apagó bien.", "Si pasa a menudo, mira Arranques y cuelgues.", "boots"),
    ("tcpip", 4199, "matter", "Conflicto de dirección IP", "Otro aparato de la red tiene la misma IP que este equipo.", "Pon la IP en automático o reserva una distinta en el router.", "network"),
    ("windowsupdateclient", 20, "matter", "Falló una actualización", "Windows Update no pudo instalar una actualización.", "Solucionar problemas → Windows Update dice cuál y por qué.", "troubleshoot"),
    ("bitlocker", 0, "watch", "Aviso de BitLocker", "Algo relacionado con el cifrado del disco.", "Comprueba que tienes la clave de recuperación guardada.", "security"),
    // Vigilar
    ("service control manager", 7000, "watch", "Un servicio no arrancó", "Un servicio configurado para arrancar falló.", "Si es de un programa desinstalado, se puede deshabilitar.", "services"),
    ("service control manager", 7009, "watch", "Un servicio tardó demasiado en arrancar", "Un servicio no contestó a tiempo al arrancar.", "Suele ser un equipo lento o un servicio pesado; si se repite, revísalo.", "services"),
    ("service control manager", 7031, "watch", "Un servicio se cerró de golpe", "Un servicio se detuvo inesperadamente y Windows lo reinició.", "Si es siempre el mismo, actualiza o reinstala su programa.", "services"),
    ("service control manager", 7034, "watch", "Un servicio se cerró de golpe", "Un servicio terminó inesperadamente.", "Si es siempre el mismo, actualiza o reinstala su programa.", "services"),
    ("application error", 1000, "watch", "Un programa se cerró solo", "Un programa falló y Windows lo cerró.", "Si es siempre el mismo, actualízalo o repáralo.", "apps"),
    ("application hang", 1002, "watch", "Un programa dejó de responder", "Un programa se colgó y se cerró.", "Si se repite, mira si coincide con poco espacio o memoria.", "processes"),
    (".net runtime", 1026, "watch", "Un programa .NET falló", "Un programa hecho con .NET se cerró por un error.", "Actualiza el programa o repara .NET.", "apps"),
    ("time-service", 0, "watch", "La hora no se sincroniza", "Windows no consigue poner la hora en hora.", "Sincroniza la hora: una hora mal rompe Office, Teams y webs seguras.", "repair"),
    ("dns client", 1014, "watch", "Fallos de DNS", "No se resolvieron algunos nombres de servidores a tiempo.", "Si van lentas las webs, prueba Reparar la red o cambiar de DNS.", "network"),
    ("netlogon", 5719, "watch", "No se encontró el dominio", "El equipo no pudo hablar con el servidor del dominio.", "Normal fuera de la oficina; en ella, revisa la red y el DNS.", "domain"),
    ("grouppolicy", 1129, "watch", "Directivas del dominio sin aplicar", "No se pudieron aplicar las directivas por falta de red con el dominio.", "Revisa la conexión con el servidor del dominio.", "domain"),
    ("e1i", 27, "watch", "La red por cable se desconecta", "El adaptador de red perdió el enlace.", "Revisa el cable, el puerto del switch y el ahorro de energía del adaptador.", "network"),
    ("netwtw", 0, "watch", "Avisos del driver Wi-Fi Intel", "El driver de la Wi-Fi registró problemas.", "Si la Wi-Fi se corta, actualiza su driver.", "network"),
    ("schannel", 36887, "watch", "Conexiones seguras rechazadas", "Un servidor rechazó una conexión cifrada.", "Si falla una web o el correo, revisa la hora del equipo.", "network"),
    // Ruido conocido
    ("distributedcom", 10016, "noise", "Permisos de DCOM", "Windows lo registra constantemente; no indica un problema.", "Nada.", ""),
    ("distributedcom", 10010, "noise", "Un servidor DCOM tardó en registrarse", "Casi siempre inofensivo.", "Nada salvo que coincida con un fallo concreto.", ""),
    ("kernel-eventtracing", 0, "noise", "Sesión de seguimiento interna", "Registro interno de diagnóstico de Windows.", "Nada.", ""),
    ("perflib", 0, "noise", "Contadores de rendimiento", "Un programa registró mal sus contadores de rendimiento.", "Nada.", ""),
    ("esent", 0, "noise", "Base de datos interna de Windows", "Avisos internos del indexador o de componentes de Windows.", "Nada salvo que el buscador no funcione.", ""),
    ("user device registration", 0, "noise", "Registro del dispositivo en Entra ID", "Habitual en equipos que no están en Entra ID.", "Nada.", ""),
    ("winmgmt", 10, "noise", "Filtro WMI antiguo", "Restos de un programa que registró un filtro WMI.", "Nada.", ""),
    ("security-spp", 0, "noise", "Servicio de licencias", "Avisos rutinarios de la protección de software.", "Nada salvo que Windows diga que no está activado.", ""),
    ("devicesetupmanager", 0, "noise", "Instalación de dispositivos", "Windows no encontró metadatos de un dispositivo en Internet.", "Nada.", ""),
    ("defrag", 0, "noise", "Optimización de unidades", "Aviso de la optimización programada.", "Nada.", ""),
    ("wmi", 0, "noise", "WMI", "Aviso interno de WMI.", "Nada.", ""),
];

fn classify(provider: &str, id: u32) -> Option<&'static Known> {
    let p = provider.to_ascii_lowercase();
    KNOWN.iter().find(|k| p.contains(k.0) && k.1 == id).or_else(|| KNOWN.iter().find(|k| p.contains(k.0) && k.1 == 0))
}

/// Agrupa por causa y ordena: lo que importa primero, luego por número de veces.
pub fn digest(events: &[RawEvent]) -> Vec<EventGroup> {
    let mut groups: Vec<EventGroup> = Vec::new();
    for e in events {
        match groups.iter_mut().find(|g| g.provider == e.provider && g.id == e.id) {
            Some(g) => {
                g.count += 1;
                if e.t > g.last {
                    g.last = e.t;
                }
            }
            None => {
                let first_line = e.message.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim().chars().take(200).collect::<String>();
                let (weight, title, meaning, todo, page) = match classify(&e.provider, e.id) {
                    Some(k) => (k.2, k.3.to_string(), k.4.to_string(), k.5.to_string(), k.6),
                    None => (if e.level <= 1 { "matter" } else if e.level == 2 { "watch" } else { "noise" }, format!("{} ({})", e.provider, e.id), first_line.clone(), "Si coincide con el problema que buscas, copia el detalle y búscalo.".to_string(), ""),
                };
                groups.push(EventGroup { provider: e.provider.clone(), id: e.id, log: e.log.clone(), count: 1, last: e.t, weight, title, meaning, todo, page, sample: first_line });
            }
        }
    }
    let rank = |w: &str| match w {
        "matter" => 0,
        "watch" => 1,
        _ => 2,
    };
    groups.sort_by(|a, b| rank(a.weight).cmp(&rank(b.weight)).then(b.count.cmp(&a.count)));
    groups
}

const SCRIPT: &str = r#"
$since = (Get-Date).AddDays(-$days)
$ev = foreach ($log in 'System', 'Application') {
  Get-WinEvent -FilterHashtable @{ LogName = $log; Level = 1, 2, 3; StartTime = $since } -MaxEvents 4000 -ErrorAction SilentlyContinue | ForEach-Object {
    [pscustomobject]@{ log = $log; provider = "$($_.ProviderName)"; id = [int]$_.Id; level = [int]$_.Level; t = [int64](($_.TimeCreated.ToUniversalTime() - [datetime]'1970-01-01').TotalSeconds); message = "$(($_.Message -split "`n")[0])" }
  }
}
ConvertTo-Json -InputObject @($ev) -Depth 3 -Compress
"#;

#[tauri::command(async)]
pub fn event_digest(days: u32) -> Result<Vec<EventGroup>, String> {
    let days = days.clamp(1, 30);
    let script = format!("$days = {days}\n{SCRIPT}");
    let out = crate::pspool::query(&script, Some(Duration::from_secs(120)), "Visor de eventos agrupado")?;
    let events: Vec<RawEvent> = if out.trim().is_empty() { vec![] } else { serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada: {e}"))? };
    Ok(digest(&events))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(provider: &str, id: u32, level: u32, t: u64) -> RawEvent {
        RawEvent { log: "System".into(), provider: provider.into(), id, level, t, message: "Mensaje\nsegunda línea".into() }
    }

    #[test]
    fn agrupa_y_ordena_por_lo_que_importa() {
        let mut v: Vec<RawEvent> = (0..340).map(|i| e("Microsoft-Windows-DistributedCOM", 10016, 3, i)).collect();
        v.extend((0..3).map(|i| e("disk", 7, 2, 1000 + i)));
        v.extend((0..12).map(|i| e("Service Control Manager", 7031, 2, 2000 + i)));
        let g = digest(&v);
        assert_eq!(g[0].title, "Sectores dañados en un disco");
        assert_eq!(g[0].count, 3);
        assert_eq!(g[0].last, 1002);
        assert_eq!(g[1].weight, "watch");
        assert_eq!(g[2].weight, "noise");
        assert_eq!(g[2].count, 340);
    }

    #[test]
    fn lo_desconocido_va_por_su_gravedad() {
        let g = digest(&[e("Programa Raro", 77, 1, 5), e("Otro", 3, 3, 6)]);
        assert_eq!(g[0].weight, "matter");
        assert_eq!(g[0].sample, "Mensaje");
        assert_eq!(g[1].weight, "noise");
    }

    #[test]
    fn whea_por_proveedor() {
        assert_eq!(classify("Microsoft-Windows-WHEA-Logger", 17).unwrap().2, "matter");
    }

    #[test]
    fn script_parses() {
        let e = crate::ps::parse_errors(SCRIPT);
        assert!(e.is_empty(), "{e}");
    }
}
