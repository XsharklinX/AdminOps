//! Encontrar el cambio que lo rompió: la búsqueda por mitades se hace en la
//! interfaz (con los «Deshacer» y «Aplicar» de siempre); aquí solo se apunta el
//! resultado en el diario, para que el caso lo cuente en su resolución.

use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use tauri::State;

/// El título del diario, sin saltos ni textos larguísimos.
pub fn note_title(culprit: &str, total: usize, questions: usize) -> String {
    let name: String = culprit.chars().filter(|c| !c.is_control()).take(120).collect();
    let rest = total.saturating_sub(1);
    let pregunta = if questions == 1 { "1 pregunta".to_string() } else { format!("{questions} preguntas") };
    format!("Encontrado el cambio que lo rompía: «{name}» (entre {total} cambios, {pregunta}). Se deshizo; los otros {rest} siguen aplicados.")
}

#[tauri::command]
pub fn bisect_note(culprit: String, total: usize, questions: usize, tweaks: State<'_, TweakState>) -> Result<(), String> {
    if culprit.trim().is_empty() {
        return Err("Falta el nombre del cambio.".into());
    }
    tweaks.record(Op::Run, &note_title(&culprit, total, questions), &Ok::<(), String>(()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_titulo_cuenta_lo_que_paso() {
        let t = note_title("Suspensión selectiva USB", 8, 3);
        assert!(t.contains("«Suspensión selectiva USB»") && t.contains("entre 8 cambios, 3 preguntas") && t.contains("los otros 7"));
        assert!(note_title("a\nb", 2, 1).contains("«ab»"));
    }
}
