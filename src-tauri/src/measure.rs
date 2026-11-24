//! «Cada arreglo, con su antes y su después».
//!
//! Una acción que cambia el equipo puede declarar qué mide (`espacio`, `cola`,
//! `ping`, `inicio`, `memoria`, `servicio`). Se lee antes, se ejecuta y se lee
//! después; la diferencia sale en el aviso y se guarda en el diario. **La medida
//! nunca hace fallar la acción**: si no se puede leer, el aviso es el de siempre.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Lo medido antes y después de una acción. Se guarda en el diario (campo opcional).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Measure {
    /// Qué se midió: «C:», «Cola», «Ping al router», «Programas al iniciar»…
    pub label: String,
    pub before: f64,
    pub after: f64,
    /// «GB libres», «trabajos», «ms», «», «GB», «s» o «estado».
    pub unit: String,
}

/// Número con coma decimal y sin ceros de más: `3,1`, `24,8`, `0`, `340`.
pub fn fmt_num(v: f64) -> String {
    let r = (v * 10.0).round() / 10.0;
    if (r - r.round()).abs() < 1e-9 {
        return format!("{}", r.round() as i64);
    }
    format!("{r:.1}").replace('.', ",")
}

/// «1 min 40 s», «38 s».
pub fn fmt_secs(v: f64) -> String {
    let s = v.round().max(0.0) as u64;
    if s >= 60 {
        format!("{} min {} s", s / 60, s % 60)
    } else {
        format!("{s} s")
    }
}

fn state_word(v: f64) -> &'static str {
    if v >= 0.5 {
        "en marcha"
    } else {
        "detenido"
    }
}

impl Measure {
    /// Un lado de la comparación; `last` es el de después (lleva la unidad completa).
    fn side(&self, v: f64, last: bool) -> String {
        match self.unit.as_str() {
            "GB libres" => format!("{} GB{}", fmt_num(v), if last { " libres" } else { "" }),
            "GB" => format!("{} GB", fmt_num(v)),
            "trabajos" => {
                let n = v.round() as i64;
                if last && n == 0 {
                    "0".into()
                } else if n == 1 {
                    "1 trabajo".into()
                } else if last {
                    fmt_num(v)
                } else {
                    format!("{} trabajos", fmt_num(v))
                }
            }
            "ms" => format!("{} ms", fmt_num(v)),
            "s" => fmt_secs(v),
            "estado" => state_word(v).into(),
            "" => fmt_num(v),
            u => format!("{} {u}", fmt_num(v)),
        }
    }

    /// «3,1 GB → 24,8 GB libres» (sin la etiqueta: cabe a la derecha de una fila).
    pub fn short(&self) -> String {
        format!("{} → {}", self.side(self.before, false), self.side(self.after, true))
    }

    /// «C: 3,1 GB → 24,8 GB libres», «Cola: 7 trabajos → 0».
    pub fn text(&self) -> String {
        if self.label.ends_with(':') {
            format!("{} {}", self.label, self.short())
        } else {
            format!("{}: {}", self.label, self.short())
        }
    }
}

/// El aviso de éxito: la diferencia en vez de «Hecho». Si la acción ya dijo algo
/// concreto («Liberados 12,3 MB»), se conserva y se le añade la medida.
pub fn with_message(message: &str, m: Option<&Measure>) -> String {
    let Some(m) = m else { return message.to_string() };
    let plain = message.trim().trim_end_matches('.').to_lowercase();
    if matches!(plain.as_str(), "" | "hecho" | "completado" | "listo" | "ok") {
        m.text()
    } else {
        format!("{} · {}", message.trim(), m.text())
    }
}

// ---------- Lectura ----------

/// Qué medir: `espacio`, `cola`, `cola:<impresora>`, `ping`, `ping:<host>`,
/// `inicio`, `memoria`, `servicio:<nombre>`.
fn split(spec: &str) -> (&str, Option<&str>) {
    match spec.split_once(':') {
        Some((k, a)) if !a.is_empty() => (k, Some(a)),
        Some((k, _)) => (k, None),
        None => (spec, None),
    }
}

/// Las medidas que existen (para validar lo que declara el catálogo).
pub fn is_known(spec: &str) -> bool {
    let (kind, arg) = split(spec);
    match kind {
        "espacio" | "cola" | "ping" | "inicio" | "memoria" => true,
        "servicio" => arg.is_some(),
        _ => false,
    }
}

fn system_free_gb() -> Option<f64> {
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()).to_uppercase();
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let d = disks.iter().find(|d| d.mount_point().to_string_lossy().to_uppercase().starts_with(&drive))?;
    Some(d.available_space() as f64 / 1_073_741_824.0)
}

fn free_memory_gb() -> Option<f64> {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    let v = sys.available_memory();
    (v > 0).then(|| v as f64 / 1_073_741_824.0)
}

/// Trabajos de impresión en la carpeta de la cola (`spool\PRINTERS`): cada trabajo
/// deja un `.SPL`. Es instantáneo, a diferencia de preguntar impresora por impresora.
/// `None` si no se puede leer la carpeta (sin permisos).
fn spool_jobs() -> Option<f64> {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    let dir = std::path::Path::new(&root).join(r"System32\spool\PRINTERS");
    let n = std::fs::read_dir(dir).ok()?.flatten().filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("spl"))).count();
    Some(n as f64)
}

/// Cuántos segundos como mucho se espera a medir la cola: medir nunca retrasa el arreglo.
const QUEUE_TIMEOUT_SECS: u64 = 5;

fn print_jobs(printer: Option<&str>) -> Option<f64> {
    let script = match printer {
        Some(p) => format!("{}@(Get-PrintJob -PrinterName $name -ErrorAction Stop).Count", crate::ps::text_var("name", p)),
        None => return spool_jobs(),
    };
    let out = crate::ps::powershell_opts(&script, crate::ps::Opts { timeout: Some(Duration::from_secs(QUEUE_TIMEOUT_SECS)), task: None }).ok()?;
    out.lines().last()?.trim().parse::<f64>().ok()
}

fn ping_ms(host: Option<&str>) -> Option<f64> {
    let target = match host {
        Some(h) => crate::ps::text_var("target", h),
        None => "$target = (Get-NetRoute -DestinationPrefix '0.0.0.0/0' -ErrorAction Stop | Sort-Object RouteMetric | Select-Object -First 1).NextHop\n".into(),
    };
    let script = format!(
        "{target}$ping = New-Object System.Net.NetworkInformation.Ping\n\
         $t = @(); foreach ($i in 1..2) {{ $r = $ping.Send($target, 1000); if ($r.Status -eq 'Success') {{ $t += $r.RoundtripTime }} }}\n\
         if ($t.Count -eq 0) {{ throw 'sin respuesta' }}\n\
         [math]::Round(($t | Measure-Object -Average).Average, 0)"
    );
    let out = crate::ps::powershell_opts(&script, crate::ps::Opts { timeout: Some(Duration::from_secs(8)), task: None }).ok()?;
    out.lines().last()?.trim().replace(',', ".").parse::<f64>().ok()
}

/// Lee una medida. `None` si no se pudo (nunca es un error).
pub fn read(spec: &str) -> Option<f64> {
    let (kind, arg) = split(spec);
    match kind {
        "espacio" => system_free_gb(),
        "memoria" => free_memory_gb(),
        "cola" => print_jobs(arg),
        "ping" => ping_ms(arg),
        "inicio" => crate::tweaks::startup::enabled_count().ok().map(|n| n as f64),
        "servicio" => {
            let name = arg?;
            crate::tweaks::service::exists(name).then(|| if crate::tweaks::service::is_running(name) { 1.0 } else { 0.0 })
        }
        _ => None,
    }
}

/// Etiqueta y unidad de cada medida.
fn describe(spec: &str) -> Option<(String, &'static str)> {
    let (kind, arg) = split(spec);
    Some(match kind {
        "espacio" => (format!("{}:", std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()).trim_end_matches(':')), "GB libres"),
        "cola" => ("Cola".into(), "trabajos"),
        "ping" => (if arg.is_some() { "Ping al servidor".into() } else { "Ping al router".into() }, "ms"),
        "inicio" => ("Programas al iniciar".into(), ""),
        "memoria" => ("Memoria libre".into(), "GB"),
        "servicio" => (format!("Servicio {}", arg?), "estado"),
        _ => return None,
    })
}

/// Lo que se lee antes de ejecutar la acción.
pub struct Before {
    spec: String,
    value: Option<f64>,
}

pub fn before(spec: Option<&str>) -> Option<Before> {
    let spec = spec?;
    if !is_known(spec) {
        return None;
    }
    Some(Before { spec: spec.to_string(), value: read(spec) })
}

impl Before {
    /// Lee de nuevo y devuelve la comparación, si se pudo medir las dos veces.
    pub fn finish(self) -> Option<Measure> {
        let b = self.value?;
        let mut a = read(&self.spec);
        // Tras renovar la red, el router tarda un momento en volver a contestar.
        // Pocos reintentos: la tarjeta de «Reparar la red» promete esperar «hasta medio minuto» en total.
        if a.is_none() && split(&self.spec).0 == "ping" {
            for _ in 0..2 {
                std::thread::sleep(Duration::from_secs(2));
                a = read(&self.spec);
                if a.is_some() {
                    break;
                }
            }
        }
        let (label, unit) = describe(&self.spec)?;
        // Un servicio que estaba en marcha y sigue en marcha no cuenta nada.
        if split(&self.spec).0 == "servicio" && a == Some(b) {
            return None;
        }
        Some(Measure { label, before: b, after: a?, unit: unit.into() })
    }
}

/// Ejecuta `run` midiendo antes y después. Devuelve su resultado y la medida.
pub fn around<T>(spec: Option<&str>, run: impl FnOnce() -> T) -> (T, Option<Measure>) {
    let pre = before(spec);
    let out = run();
    (out, pre.and_then(Before::finish))
}

// ---------- Comandos ----------

/// Lee una medida suelta (para las pantallas que ejecutan varias cosas seguidas).
#[tauri::command(async)]
pub fn measure_read(spec: String) -> Option<f64> {
    if !is_known(&spec) {
        return None;
    }
    read(&spec)
}

/// El texto de una comparación («C: 3,1 GB → 24,8 GB libres»), con el formato en español.
#[tauri::command]
pub fn measure_text(measure: Measure) -> String {
    measure.text()
}

/// Apunta en el diario una medida suelta (el arranque, que solo se puede medir tras reiniciar).
#[tauri::command]
pub fn measure_record(title: String, measure: Measure, state: tauri::State<'_, crate::tweaks::TweakState>) {
    let title: String = title.chars().filter(|c| !c.is_control()).take(200).collect();
    state.record_measure_note(&title, measure);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(label: &str, before: f64, after: f64, unit: &str) -> Measure {
        Measure { label: label.into(), before, after, unit: unit.into() }
    }

    #[test]
    fn numeros_en_espanol() {
        assert_eq!(fmt_num(3.1), "3,1");
        assert_eq!(fmt_num(24.8), "24,8");
        assert_eq!(fmt_num(340.0), "340");
        assert_eq!(fmt_num(0.0), "0");
        assert_eq!(fmt_num(1.2), "1,2");
        assert_eq!(fmt_num(3.96), "4");
        assert_eq!(fmt_num(1234.56), "1234,6");
    }

    #[test]
    fn frases_de_la_maqueta() {
        assert_eq!(m("C:", 3.1, 24.8, "GB libres").text(), "C: 3,1 GB → 24,8 GB libres");
        assert_eq!(m("Cola", 7.0, 0.0, "trabajos").text(), "Cola: 7 trabajos → 0");
        assert_eq!(m("Ping al router", 340.0, 4.0, "ms").text(), "Ping al router: 340 ms → 4 ms");
        assert_eq!(m("Programas al iniciar", 14.0, 8.0, "").text(), "Programas al iniciar: 14 → 8");
        assert_eq!(m("Memoria libre", 1.2, 3.9, "GB").text(), "Memoria libre: 1,2 GB → 3,9 GB");
        assert_eq!(m("Arranque", 100.0, 38.0, "s").text(), "Arranque: 1 min 40 s → 38 s");
        assert_eq!(m("Servicio Spooler", 0.0, 1.0, "estado").text(), "Servicio Spooler: detenido → en marcha");
    }

    #[test]
    fn cola_con_un_solo_trabajo() {
        assert_eq!(m("Cola", 1.0, 0.0, "trabajos").short(), "1 trabajo → 0");
    }

    #[test]
    fn el_corto_no_lleva_etiqueta() {
        assert_eq!(m("C:", 3.1, 24.8, "GB libres").short(), "3,1 GB → 24,8 GB libres");
    }

    #[test]
    fn el_aviso_cambia_hecho_por_la_diferencia() {
        let x = m("C:", 3.1, 24.8, "GB libres");
        assert_eq!(with_message("Hecho.", Some(&x)), "C: 3,1 GB → 24,8 GB libres");
        assert_eq!(with_message("", Some(&x)), "C: 3,1 GB → 24,8 GB libres");
        assert_eq!(with_message("Liberados 12,3 MB", Some(&x)), "Liberados 12,3 MB · C: 3,1 GB → 24,8 GB libres");
        // Sin medida, el aviso de siempre.
        assert_eq!(with_message("Hecho.", None), "Hecho.");
    }

    #[test]
    fn solo_se_aceptan_medidas_conocidas() {
        for ok in ["espacio", "cola", "cola:HP", "ping", "ping:10.0.0.1", "inicio", "memoria", "servicio:Spooler"] {
            assert!(is_known(ok), "{ok}");
        }
        for bad in ["servicio", "temperatura", ""] {
            assert!(!is_known(bad), "{bad}");
        }
    }

    #[test]
    fn el_diario_viejo_se_lee_sin_medida() {
        // El campo `measure` es opcional: un diario de antes sigue leyéndose igual.
        let viejo = r#"[{"id":1,"timestamp":1790000000,"op":"run","tweakId":"cleanup.user-temp","title":"Limpiar","ok":true}]"#;
        let v: Vec<crate::tweaks::journal::Entry> = serde_json::from_str(viejo).unwrap();
        assert_eq!(v.len(), 1);
        assert!(v[0].measure.is_none());
        // Y con medida, va y vuelve.
        let mut e = v[0].clone();
        e.measure = Some(m("C:", 3.1, 24.8, "GB libres"));
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("\"measure\""));
        let otra: crate::tweaks::journal::Entry = serde_json::from_str(&json).unwrap();
        assert_eq!(otra.measure, e.measure);
    }

    #[test]
    fn lecturas_reales_no_fallan_la_accion() {
        // Memoria y espacio se leen siempre; el resto puede dar None sin romper nada.
        assert!(read("memoria").is_some());
        assert!(read("espacio").is_some());
        assert!(read("servicio:ServicioQueNoExiste").is_none());
        assert!(read("inventado").is_none());
    }
}
