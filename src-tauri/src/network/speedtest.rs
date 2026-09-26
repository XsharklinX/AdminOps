//! Test de velocidad contra la red de Cloudflare (los mismos puntos que usa
//! speed.cloudflare.com).
//!
//! Metodología, pensada para ser comparable con las webs de speedtest:
//! - **Latencia y jitter**: 20 peticiones vacías; se resta el tiempo de proceso
//!   del servidor (`server-timing`) para quedarse solo con la red. Latencia =
//!   mediana; jitter = media de la diferencia entre mediciones consecutivas.
//! - **Bajada / subida**: varias conexiones TCP en paralelo (HTTP/1.1, una por
//!   conexión) durante un tiempo fijo. Se descartan los primeros segundos (el
//!   "arranque lento" de TCP) y la velocidad es el promedio del tramo estable.
//! - **Latencia con carga**: se mide mientras la línea está saturada; si sube
//!   mucho respecto a la de reposo hay "bufferbloat" (tirones en videollamadas y juegos).

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::Emitter;

const BASE: &str = "https://speed.cloudflare.com";
const PHASE: Duration = Duration::from_secs(10);
const WARMUP: Duration = Duration::from_secs(2);
const DOWN_STREAMS: usize = 6;
const UP_STREAMS: usize = 4;
const DOWN_CHUNK: u64 = 25_000_000;
const UP_CHUNK: usize = 8_000_000;
const MAX_HISTORY: usize = 50;

static CANCEL: AtomicBool = AtomicBool::new(false);
static RUNNING: AtomicBool = AtomicBool::new(false);

const CANCELLED: &str = "Test cancelado.";

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SpeedResult {
    pub timestamp: u64,
    /// Centro de datos de Cloudflare que atendió el test (p. ej. "MIA · Miami").
    pub server: String,
    pub isp: Option<String>,
    pub ip: Option<String>,
    pub latency_ms: f64,
    pub jitter_ms: f64,
    pub download_mbps: f64,
    pub upload_mbps: f64,
    pub download_latency_ms: Option<f64>,
    pub upload_latency_ms: Option<f64>,
    /// Datos transferidos (MB), para saber cuánto consumió el test.
    pub transferred_mb: f64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    /// meta | latency | download | upload | done
    phase: &'static str,
    /// Velocidad instantánea (último segundo) en Mbps.
    mbps: f64,
    /// Avance de la fase actual, 0..1.
    progress: f64,
    latency_ms: Option<f64>,
}

/// Receptor del progreso: (fase, Mbps instantáneos, avance 0..1, latencia).
type OnProgress<'a> = &'a (dyn Fn(&'static str, f64, f64, Option<f64>) + Sync);

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(concat!("AdminOps/", env!("CARGO_PKG_VERSION")))
        // HTTP/1.1: cada flujo en su propia conexión TCP, como los speedtests web.
        .http1_only()
        .connect_timeout(Duration::from_secs(10))
        .pool_max_idle_per_host(16)
        .build()
        .map_err(|e| e.to_string())
}

fn check_cancel() -> Result<(), String> {
    if CANCEL.load(Ordering::Relaxed) { Err(CANCELLED.into()) } else { Ok(()) }
}

/// Tiempo de proceso del servidor: suma de todos los `dur=` de todas las
/// cabeceras `server-timing` (Cloudflare envía varias: edge + worker).
fn server_time(headers: &reqwest::header::HeaderMap) -> f64 {
    headers
        .get_all("server-timing")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .filter_map(|metric| metric.split(';').find_map(|p| p.trim().strip_prefix("dur=")))
        .filter_map(|d| d.trim().parse::<f64>().ok())
        .sum()
}

/// Centro de datos, país e IP pública (`/cdn-cgi/trace`: líneas `clave=valor`).
async fn meta(client: &reqwest::Client) -> Meta {
    let Ok(resp) = client.get(format!("{BASE}/cdn-cgi/trace")).timeout(Duration::from_secs(8)).send().await else {
        return Meta::default();
    };
    let text = resp.text().await.unwrap_or_default();
    let get = |k: &str| {
        text.lines().find_map(|l| l.strip_prefix(k).and_then(|v| v.strip_prefix('='))).map(str::to_string).filter(|v| !v.is_empty())
    };
    Meta { client_ip: get("ip"), colo: get("colo"), country: get("loc") }
}

/// Latencia de red de una petición vacía, sin el tiempo de proceso del servidor.
async fn ping(client: &reqwest::Client) -> Result<f64, String> {
    let start = Instant::now();
    let resp = client
        .get(format!("{BASE}/__down?bytes=0"))
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let total = start.elapsed().as_secs_f64() * 1000.0;
    let server = server_time(resp.headers());
    let _ = resp.bytes().await;
    Ok((total - server).max(0.1))
}

fn median(v: &mut [f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let m = v.len() / 2;
    if v.len().is_multiple_of(2) { (v[m - 1] + v[m]) / 2.0 } else { v[m] }
}

fn jitter(samples: &[f64]) -> f64 {
    if samples.len() < 2 {
        return 0.0;
    }
    samples.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f64>() / (samples.len() - 1) as f64
}

#[derive(Default)]
struct Meta {
    client_ip: Option<String>,
    /// Código IATA del centro de datos (p. ej. "MIA").
    colo: Option<String>,
    country: Option<String>,
}

/// Mide el caudal de una fase (bajada o subida) con `streams` conexiones en paralelo.
/// Devuelve (Mbps del tramo estable, bytes totales, latencias con carga).
async fn throughput(
    on: OnProgress<'_>,
    client: &reqwest::Client,
    phase: &'static str,
    streams: usize,
) -> Result<(f64, u64, Vec<f64>), String> {
    let bytes = Arc::new(AtomicU64::new(0));
    let start = Instant::now();
    let deadline = start + PHASE;
    let upload_data = if phase == "upload" { Some(random_block(UP_CHUNK)) } else { None };

    let mut workers = Vec::new();
    for _ in 0..streams {
        let (client, bytes, data) = (client.clone(), bytes.clone(), upload_data.clone());
        workers.push(tauri::async_runtime::spawn(async move {
            while Instant::now() < deadline && !CANCEL.load(Ordering::Relaxed) {
                let r = match &data {
                    None => download_once(&client, &bytes, deadline).await,
                    Some(data) => upload_once(&client, &bytes, deadline, data.clone()).await,
                };
                if r.is_err() {
                    // Un flujo que falla (p. ej. corte puntual) no invalida el test.
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
            }
        }));
    }

    // Latencia con la línea cargada, en paralelo.
    let loaded = {
        let client = client.clone();
        tauri::async_runtime::spawn(async move {
            let mut v = Vec::new();
            tokio::time::sleep(WARMUP).await;
            while Instant::now() + Duration::from_millis(500) < deadline && !CANCEL.load(Ordering::Relaxed) {
                if let Ok(ms) = ping(&client).await {
                    v.push(ms);
                }
                tokio::time::sleep(Duration::from_millis(400)).await;
            }
            v
        })
    };

    // Muestreo cada 250 ms: velocidad instantánea (ventana de 1 s) para la UI
    // y marca de bytes al terminar el calentamiento.
    let mut samples: Vec<(Duration, u64)> = vec![(Duration::ZERO, 0)];
    let mut at_warmup: Option<(Duration, u64)> = None;
    while Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(250)).await;
        check_cancel()?;
        let now = (start.elapsed(), bytes.load(Ordering::Relaxed));
        samples.push(now);
        if at_warmup.is_none() && now.0 >= WARMUP {
            at_warmup = Some(now);
        }
        let back = samples.iter().rev().find(|s| now.0 - s.0 >= Duration::from_secs(1)).copied().unwrap_or(samples[0]);
        let secs = (now.0 - back.0).as_secs_f64().max(0.001);
        let mbps = (now.1 - back.1) as f64 * 8.0 / secs / 1e6;
        on(phase, mbps, (now.0.as_secs_f64() / PHASE.as_secs_f64()).min(1.0), None);
    }
    let end = (start.elapsed(), bytes.load(Ordering::Relaxed));
    for w in workers {
        let _ = w.await;
    }
    let loaded = loaded.await.unwrap_or_default();
    check_cancel()?;

    let (t0, b0) = at_warmup.unwrap_or((Duration::ZERO, 0));
    let secs = (end.0 - t0).as_secs_f64().max(0.001);
    let mbps = (end.1.saturating_sub(b0)) as f64 * 8.0 / secs / 1e6;
    if end.1 == 0 {
        return Err(format!("No se pudo transferir ningún dato en la {}.", if phase == "download" { "bajada" } else { "subida" }));
    }
    Ok((mbps, end.1, loaded))
}

async fn download_once(client: &reqwest::Client, bytes: &AtomicU64, deadline: Instant) -> Result<(), String> {
    let resp = client.get(format!("{BASE}/__down?bytes={DOWN_CHUNK}")).send().await.map_err(|e| e.to_string())?;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        bytes.fetch_add(chunk.len() as u64, Ordering::Relaxed);
        if Instant::now() >= deadline || CANCEL.load(Ordering::Relaxed) {
            break;
        }
    }
    Ok(())
}

async fn upload_once(client: &reqwest::Client, bytes: &Arc<AtomicU64>, deadline: Instant, data: bytes::Bytes) -> Result<(), String> {
    const PIECE: usize = 64 * 1024;
    let counter = bytes.clone();
    // El cuerpo se genera por trozos y se cuenta a medida que se envía; se corta
    // al llegar al límite de tiempo.
    let body = futures_util::stream::iter((0..data.len()).step_by(PIECE).map(move |i| {
        let piece = data.slice(i..(i + PIECE).min(data.len()));
        (piece, Instant::now() < deadline && !CANCEL.load(Ordering::Relaxed))
    }))
    .take_while(|(_, go)| std::future::ready(*go))
    .map(move |(piece, _)| {
        counter.fetch_add(piece.len() as u64, Ordering::Relaxed);
        Ok::<_, std::io::Error>(piece)
    });
    client
        .post(format!("{BASE}/__up"))
        .header("content-type", "application/octet-stream")
        .body(reqwest::Body::wrap_stream(body))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Datos pseudoaleatorios (no comprimibles) para la subida.
fn random_block(len: usize) -> bytes::Bytes {
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15 ^ std::process::id() as u64;
    let mut v = Vec::with_capacity(len);
    while v.len() < len {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        v.extend_from_slice(&x.to_le_bytes());
    }
    v.truncate(len);
    bytes::Bytes::from(v)
}

fn history_path(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::machine_data_dir(app).join("speedtests.json")
}

pub fn history(app: &tauri::AppHandle) -> Vec<SpeedResult> {
    std::fs::read_to_string(history_path(app)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save(app: &tauri::AppHandle, r: &SpeedResult) {
    let mut h = history(app);
    h.insert(0, r.clone());
    h.truncate(MAX_HISTORY);
    let path = history_path(app);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(&h) {
        let _ = std::fs::write(path, json);
    }
}

async fn run(on: OnProgress<'_>) -> Result<SpeedResult, String> {
    let client = client()?;
    on("meta", 0.0, 0.0, None);
    let meta = meta(&client).await;
    // La primera petición abre la conexión y no cuenta para la latencia.
    if let Err(e) = ping(&client).await {
        return Err(format!("Sin conexión con el servidor de pruebas: {e}"));
    }

    let mut lat = Vec::new();
    for i in 0..20 {
        check_cancel()?;
        if let Ok(ms) = ping(&client).await {
            lat.push(ms);
            on("latency", 0.0, (i + 1) as f64 / 20.0, Some(ms));
        }
    }
    if lat.is_empty() {
        return Err("El servidor de pruebas no responde.".into());
    }
    let jitter_ms = jitter(&lat);
    let latency_ms = median(&mut lat.clone());

    let (download_mbps, down_bytes, mut down_lat) = throughput(on, &client, "download", DOWN_STREAMS).await?;
    let (upload_mbps, up_bytes, mut up_lat) = throughput(on, &client, "upload", UP_STREAMS).await?;

    let server = match (&meta.colo, &meta.country) {
        (Some(c), Some(cc)) => format!("Cloudflare {c} ({cc})"),
        (Some(c), None) => format!("Cloudflare {c}"),
        _ => "Cloudflare".into(),
    };
    let result = SpeedResult {
        timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
        server,
        isp: None,
        ip: meta.client_ip,
        latency_ms,
        jitter_ms,
        download_mbps,
        upload_mbps,
        download_latency_ms: (!down_lat.is_empty()).then(|| median(&mut down_lat)),
        upload_latency_ms: (!up_lat.is_empty()).then(|| median(&mut up_lat)),
        transferred_mb: (down_bytes + up_bytes) as f64 / 1e6,
    };
    on("done", 0.0, 1.0, None);
    Ok(result)
}

#[tauri::command]
pub async fn run_speedtest(app: tauri::AppHandle) -> Result<SpeedResult, String> {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Err("Ya hay un test de velocidad en curso.".into());
    }
    CANCEL.store(false, Ordering::SeqCst);
    log::info!("Test de velocidad iniciado");
    let emitter = app.clone();
    let on = move |phase: &'static str, mbps: f64, progress: f64, latency_ms: Option<f64>| {
        let _ = emitter.emit("speedtest-progress", Progress { phase, mbps, progress, latency_ms });
    };
    let result = run(&on).await;
    RUNNING.store(false, Ordering::SeqCst);
    match &result {
        Ok(r) => {
            log::info!(
                "Test de velocidad: ↓ {:.1} Mbps ↑ {:.1} Mbps, latencia {:.1} ms ({})",
                r.download_mbps, r.upload_mbps, r.latency_ms, r.server
            );
            save(&app, r);
        }
        Err(e) => log::warn!("Test de velocidad falló: {e}"),
    }
    result
}

#[tauri::command]
pub fn cancel_speedtest() {
    CANCEL.store(true, Ordering::SeqCst);
}

#[tauri::command]
pub fn list_speedtests(app: tauri::AppHandle) -> Vec<SpeedResult> {
    history(&app)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_every_server_timing_duration() {
        let mut h = reqwest::header::HeaderMap::new();
        h.append("server-timing", "cfSpeedEdge;dur=4, cfSpeedWorker;dur=22".parse().unwrap());
        h.append("server-timing", "cfL4;desc=\"?proto=TCP&rtt=35054\"".parse().unwrap());
        assert_eq!(server_time(&h), 26.0);
    }

    #[test]
    fn stats() {
        assert_eq!(median(&mut [5.0, 1.0, 3.0]), 3.0);
        assert_eq!(median(&mut [4.0, 1.0, 3.0, 2.0]), 2.5);
        assert_eq!(jitter(&[10.0, 12.0, 11.0]), 1.5);
    }

    /// Test real contra Cloudflare (usa ~cientos de MB): `cargo test speedtest_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn speedtest_real() {
        let last = std::sync::Mutex::new(String::new());
        let on = |phase: &'static str, mbps: f64, _p: f64, _l: Option<f64>| {
            let mut l = last.lock().unwrap();
            if *l != phase {
                println!("fase: {phase}");
                *l = phase.to_string();
            }
            let _ = mbps;
        };
        let r = tauri::async_runtime::block_on(run(&on)).unwrap();
        println!("{r:#?}");
        assert!(r.download_mbps > 0.0 && r.upload_mbps > 0.0);
    }

    #[test]
    fn random_block_is_not_trivially_compressible() {
        let b = random_block(4096);
        assert_eq!(b.len(), 4096);
        let distinct: std::collections::HashSet<u8> = b.iter().copied().collect();
        assert!(distinct.len() > 200);
    }
}
