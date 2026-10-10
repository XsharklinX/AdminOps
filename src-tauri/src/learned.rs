//! Soluciones que aprenden de lo que funcionó: al cerrar un caso o terminar una
//! solución paso a paso, se apunta qué lo arregló. Con el tiempo, cada síntoma
//! ordena las soluciones por las que de verdad funcionaron en tus clientes («la
//! impresora no imprime: el 70 % de las veces era la cola»). Los datos son del
//! técnico y viajan con él (carpeta de datos compartida, también en el pendrive).

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub at: u64,
    /// Síntoma («printer») o solución («solution:outlook-pide-contrasena»).
    pub topic: String,
    /// Lo que lo arregló, en palabras del técnico o de la acción de AdminOps.
    pub fix: String,
    #[serde(default)]
    pub client: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Stat {
    pub fix: String,
    pub count: usize,
    pub pct: u32,
}

/// Lo que funcionó para un tema, lo más frecuente primero. `client`: solo los de ese cliente (vacío: todos).
pub fn stats(records: &[Record], topic: &str, client: &str) -> Vec<Stat> {
    let mine: Vec<&Record> = records.iter().filter(|r| r.topic == topic && (client.is_empty() || r.client == client)).collect();
    let total = mine.len();
    let mut by: Vec<(String, usize)> = Vec::new();
    for r in &mine {
        let key = r.fix.trim();
        match by.iter_mut().find(|(f, _)| f.eq_ignore_ascii_case(key)) {
            Some(e) => e.1 += 1,
            None => by.push((key.to_string(), 1)),
        }
    }
    by.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    by.into_iter().map(|(fix, count)| Stat { pct: (count * 100 / total.max(1)) as u32, fix, count }).collect()
}

fn file(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::shared_data_dir(app).join("learned.json")
}

#[tauri::command]
pub fn learned_record(app: tauri::AppHandle, topic: String, fix: String, client: String) -> Result<(), String> {
    let (topic, fix) = (topic.trim().to_string(), fix.trim().chars().take(200).collect::<String>());
    if topic.is_empty() || fix.is_empty() {
        return Err("Falta el problema o lo que lo arregló.".into());
    }
    let mut all: Vec<Record> = crate::paths::read_json(&file(&app));
    let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    all.push(Record { at, topic, fix, client: client.trim().to_string() });
    let keep = all.len().saturating_sub(5000);
    all.drain(..keep);
    crate::paths::write_json(&file(&app), &all)
}

#[tauri::command]
pub fn learned_stats(app: tauri::AppHandle, topic: String, client: String) -> Vec<Stat> {
    stats(&crate::paths::read_json::<Vec<Record>>(&file(&app)), &topic, &client)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(topic: &str, fix: &str, client: &str) -> Record {
        Record { at: 0, topic: topic.into(), fix: fix.into(), client: client.into() }
    }

    #[test]
    fn ordena_por_lo_que_funciono() {
        let mut v: Vec<Record> = (0..7).map(|_| r("printer", "Vaciar la cola de impresión", "A")).collect();
        v.extend((0..2).map(|_| r("printer", "Reinstalar el driver", "B")));
        v.push(r("printer", "vaciar la cola de impresión ", "B"));
        v.push(r("internet", "Reiniciar el router", "A"));
        let s = stats(&v, "printer", "");
        assert_eq!(s[0].fix, "Vaciar la cola de impresión");
        assert_eq!(s[0].count, 8);
        assert_eq!(s[0].pct, 80);
        assert_eq!(s[1].pct, 20);
        assert_eq!(stats(&v, "printer", "B").len(), 2);
        assert!(stats(&v, "audio", "").is_empty());
    }
}
