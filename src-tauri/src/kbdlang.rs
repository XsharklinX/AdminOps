//! «El teclado escribe raro» (Estado del equipo → Teclado e idioma).
//!
//! Lee los idiomas de teclado del usuario (`Get-WinUserLanguageList`), cuál está activo ahora
//! (`GetKeyboardLayout` del hilo en primer plano) y el atajo que cambia de idioma sin avisar
//! (`HKCU\Keyboard Layout\Toggle`). El arreglo, «dejar solo Español y quitar el atajo», guarda
//! antes la lista completa y los tres valores del atajo en el diario para poder deshacerlo.
//!
//! La prueba de «ñ @ ´» vive en la interfaz: solo compara lo que se escribe.

use crate::tweaks::journal::Backup;
use crate::tweaks::model::{RegData, RegKind};
use crate::tweaks::registry;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::State;

pub const TOGGLE_KEY: &str = r"HKCU\Keyboard Layout\Toggle";
/// Los tres valores que deciden cómo se cambia de idioma o de distribución.
pub const TOGGLE_VALUES: [&str; 3] = ["Hotkey", "Language Hotkey", "Layout Hotkey"];
const TITLE: &str = "Teclado: dejar solo Español y quitar el atajo de cambio";
/// Donde Windows guarda los idiomas del usuario (en su colmena, aunque AdminOps esté abierto con otra cuenta).
const PROFILE_KEY: &str = r"HKCU\Control Panel\International\User Profile";

/// Lo que se dice cuando AdminOps está abierto con otra cuenta de administrador: la lista de idiomas
/// solo se cambia desde la sesión de su dueño.
pub const OTHER_USER_SENTENCE: &str = "Con AdminOps abierto como otra cuenta no se puede cambiar el idioma de este usuario: ábrelo con su cuenta.";

/// Un idioma de la lista del usuario con sus métodos de entrada (`0C0A:0000040A`): lo que
/// hace falta para volver a ponerlo igual.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LanguageEntry {
    pub tag: String,
    pub tips: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct RawLanguage {
    tag: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    tips: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct RawState {
    #[serde(default)]
    languages: Vec<RawLanguage>,
    #[serde(default)]
    region: String,
    /// El idioma de visualización de Windows (`en-US`).
    #[serde(default)]
    ui: String,
}

/// Una fila de la pantalla: una distribución de un idioma.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InputRow {
    /// Etiqueta del idioma (`es-DO`).
    pub tag: String,
    /// «Español (República Dominicana)».
    pub language: String,
    /// «Latinoamericano», «Inglés (EE. UU.)»…
    pub layout: String,
    /// Identificador de la distribución (`0000080A`), si lo hay.
    pub klid: Option<String>,
    /// El identificador del idioma que usa Windows (0x0C0A).
    pub lang_id: u32,
    /// Es la distribución que está en uso ahora mismo.
    pub active: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Shortcut {
    /// `1` Alt+Mayús, `2` Ctrl+Mayús, `3` ninguno, `4` acento grave.
    pub code: String,
    pub label: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KbdState {
    pub rows: Vec<InputRow>,
    /// Cuántos idiomas hay en la lista (no filas).
    pub language_count: usize,
    /// País de Windows (`DO`, `ES`…).
    pub region: String,
    pub shortcut: Shortcut,
    /// Hay algún español instalado: sin él no se ofrece dejar solo Español.
    pub has_spanish: bool,
    /// AdminOps está abierto con otra cuenta: los idiomas que se ven son los del cliente y no se pueden cambiar.
    pub other_user: bool,
    /// Si es el idioma de Windows y no hay paquete en español, se queda en la lista sin teclado (su etiqueta).
    pub kept: Option<String>,
    /// Qué se dice en la confirmación: quitar ese idioma puede cambiar el idioma de Windows.
    pub warning: Option<String>,
    /// Por qué un idioma se queda en la lista sin teclado.
    pub note: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KbdChange {
    pub message: String,
    /// La entrada del diario, para «Deshacer».
    pub entry_id: u64,
}

// ---------- Puro, con pruebas ----------

/// «0C0A:0000040A» → (0x0C0A, Some("0000040A")). Los métodos de entrada de un editor (IME) llevan
/// una guía en vez del identificador y no tienen distribución propia.
pub fn parse_tip(tip: &str) -> Option<(u32, Option<String>)> {
    let (lang, rest) = tip.split_once(':')?;
    let id = u32::from_str_radix(lang.trim(), 16).ok().filter(|&n| n <= 0xFFFF)?;
    let rest = rest.trim();
    let klid = (rest.len() == 8 && rest.chars().all(|c| c.is_ascii_hexdigit())).then(|| rest.to_ascii_uppercase());
    Some((id, klid))
}

/// El nombre de una distribución, en español.
pub fn layout_name(klid: &str) -> Option<&'static str> {
    Some(match klid {
        "0000040A" => "Español",
        "0001040A" => "Español (variante)",
        "0000080A" => "Latinoamericano",
        "00000409" => "Inglés (EE. UU.)",
        "00020409" => "Inglés (EE. UU., internacional)",
        "00010409" => "Inglés (Dvorak)",
        "00000809" => "Inglés (Reino Unido)",
        "00001009" => "Inglés (Canadá)",
        "0000040C" => "Francés",
        "00000407" => "Alemán",
        "00000410" => "Italiano",
        "00000416" => "Portugués (Brasil, ABNT)",
        "00000816" => "Portugués",
        "00000403" => "Catalán",
        _ => return None,
    })
}

/// La posición del primer idioma español de la lista (`es` o `es-XX`).
pub fn pick_spanish(tags: &[String]) -> Option<usize> {
    tags.iter().position(|t| {
        let t = t.trim().to_ascii_lowercase();
        t == "es" || t.starts_with("es-")
    })
}

/// El atajo de cambio que vale: el del idioma, y si no el general; sin nada, Alt+Mayús (el de Windows).
/// Si el de idioma está quitado pero el de distribución (`Layout Hotkey`) sigue puesto, vale ese: también
/// cambia el teclado sin avisar.
pub fn effective_code(hotkey: Option<&str>, language_hotkey: Option<&str>, layout_hotkey: Option<&str>) -> String {
    let pick = |v: Option<&str>| v.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let language = pick(language_hotkey).or_else(|| pick(hotkey)).unwrap_or_else(|| "1".into());
    if language == "3" {
        if let Some(layout) = pick(layout_hotkey).filter(|l| l != "3") {
            return layout;
        }
    }
    language
}

pub fn shortcut_label(code: &str) -> &'static str {
    match code {
        "1" => "Alt+Mayús",
        "2" => "Ctrl+Mayús",
        "3" => "Sin atajo",
        "4" => "Acento grave (`)",
        _ => "Otro atajo",
    }
}

/// ¿Es una distribución española (`0000040A`, `0000080A`, `0001040A`…)? Todas acaban en `0A`: el idioma 0x0A.
pub fn is_spanish_klid(klid: &str) -> bool {
    klid.len() == 8 && klid.chars().all(|c| c.is_ascii_hexdigit()) && klid.to_ascii_uppercase().ends_with("0A")
}

/// De los métodos de entrada del español, solo la primera distribución española: se quita la inglesa
/// que Windows deja dentro de `es-XX`. Si no hay ninguna española, se dejan como están.
pub fn trim_spanish_tips(tips: &[String]) -> Vec<String> {
    let first = tips.iter().find(|t| parse_tip(t).and_then(|(_, k)| k).is_some_and(|k| is_spanish_klid(&k)));
    match first {
        Some(t) => vec![t.clone()],
        None => tips.to_vec(),
    }
}

/// Qué pasa con el idioma de Windows al dejar solo Español.
#[derive(Debug, PartialEq, Default)]
pub struct OnlySpanishPlan {
    /// Idiomas que se quedan en la lista sin teclado (el de Windows si no hay paquete en español).
    pub keep: Vec<String>,
    /// Quitar el idioma de Windows puede cambiarlo a Español (hay paquete): se avisa al confirmar.
    pub warning: Option<String>,
}

/// `ui`: idioma de visualización de Windows; `spanish_pack`: hay un paquete de idioma en español instalado.
pub fn plan_only_spanish(tags: &[String], ui: &str, spanish_pack: bool) -> OnlySpanishPlan {
    let ui = ui.trim();
    let spanish = pick_spanish(tags);
    let Some(i) = tags.iter().position(|t| !ui.is_empty() && t.trim().eq_ignore_ascii_case(ui)) else { return OnlySpanishPlan::default() };
    if Some(i) == spanish || ui.to_ascii_lowercase().starts_with("es") {
        return OnlySpanishPlan::default();
    }
    if spanish_pack {
        OnlySpanishPlan { keep: Vec::new(), warning: Some(format!("Windows está en el idioma «{}»: al quitarlo de la lista puede pasarse a Español. ¿Continuar?", tags[i].trim())) }
    } else {
        OnlySpanishPlan { keep: vec![tags[i].trim().to_string()], warning: None }
    }
}

/// Lo que se dice cuando un idioma se queda sin teclado por ser el de Windows.
fn kept_note(tag: &str) -> String {
    format!("«{tag}» es el idioma de Windows y no hay paquete en español: se queda en la lista, sin teclado, para no cambiar el idioma de Windows.")
}

/// Las filas (una por distribución) de los idiomas leídos. `active`: el identificador del idioma que escribe ahora.
fn build_rows(languages: &[RawLanguage], active: u32) -> Vec<InputRow> {
    let mut rows = Vec::new();
    let mut active_used = false;
    for l in languages {
        for tip in &l.tips {
            let Some((lang_id, klid)) = parse_tip(tip) else { continue };
            let layout = match &klid {
                Some(k) => layout_name(k).map(str::to_string).or_else(|| registry::read_string(&format!(r"HKLM\SYSTEM\CurrentControlSet\Control\Keyboard Layouts\{k}"), "Layout Text")).unwrap_or_else(|| format!("Distribución {k}")),
                None => "Método de entrada".to_string(),
            };
            // Con dos distribuciones del mismo idioma solo se marca la primera.
            let is_active = !active_used && lang_id == active;
            active_used |= is_active;
            rows.push(InputRow { tag: l.tag.clone(), language: if l.name.is_empty() { l.tag.clone() } else { l.name.clone() }, layout, klid, lang_id, active: is_active });
        }
    }
    rows
}

// ---------- Scripts ----------

/// Lee los idiomas de teclado del usuario y su país.
pub const READ_SCRIPT: &str = r#"# Los idiomas de teclado del usuario y sus métodos de entrada
$idiomas = @(foreach ($l in (Get-WinUserLanguageList)) { [pscustomobject]@{ tag = $l.LanguageTag; name = $l.Autonym; tips = @($l.InputMethodTips) } })
# El país de Windows
$pais = try { [System.Globalization.RegionInfo]::CurrentRegion.TwoLetterISORegionName } catch { '' }
# El idioma de visualización de Windows
$ui = try { [System.Globalization.CultureInfo]::CurrentUICulture.Name } catch { '' }
ConvertTo-Json -Compress -Depth 5 -InputObject ([pscustomobject]@{ languages = $idiomas; region = $pais; ui = $ui })
"#;

/// Deja solo el primer idioma español de la lista, con su primera distribución española (`$tipsEspanol`).
/// `$conservar`: los idiomas que se quedan en la lista sin teclado (el de Windows si no hay paquete en español).
const ONLY_SPANISH_BODY: &str = r#"# Los idiomas de teclado del usuario
$lista = Get-WinUserLanguageList
# El primer idioma español de la lista
$espanol = @($lista) | Where-Object { $_.LanguageTag -eq 'es' -or $_.LanguageTag -like 'es-*' } | Select-Object -First 1
if (-not $espanol) { throw 'No hay ningún idioma español instalado.' }
# Del español solo se queda su primera distribución española (se quita la inglesa que lleve dentro)
if (@($tipsEspanol).Count) { $espanol.InputMethodTips.Clear(); foreach ($t in $tipsEspanol) { [void]$espanol.InputMethodTips.Add($t) } }
# Se quitan los demás idiomas; los de $conservar se quedan sin teclado
foreach ($otro in @($lista)) {
    if ([object]::ReferenceEquals($otro, $espanol)) { continue }
    if ($conservar -contains $otro.LanguageTag) { $otro.InputMethodTips.Clear() } else { [void]$lista.Remove($otro) }
}
Set-WinUserLanguageList -LanguageList $lista -Force
"#;

/// El script completo con los idiomas que se conservan sin teclado.
pub fn only_spanish_script(keep: &[String], spanish_tips: &[String]) -> String {
    let list = |v: &[String], ok: fn(&str) -> bool| v.iter().filter(|t| ok(t)).map(|t| crate::ps::ps_literal(t)).collect::<Vec<_>>().join(", ");
    format!("$conservar = @({})\n$tipsEspanol = @({})\n{ONLY_SPANISH_BODY}", list(keep, tag_ok), list(spanish_tips, tip_ok))
}

fn tag_ok(t: &str) -> bool {
    !t.is_empty() && t.len() <= 35 && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn tip_ok(t: &str) -> bool {
    !t.is_empty() && t.len() <= 100 && t.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '{' | '}' | '-'))
}

/// El script que vuelve a poner una lista de idiomas tal como estaba.
pub fn restore_script(list: &[LanguageEntry]) -> Result<String, String> {
    if list.is_empty() {
        return Err("La copia de los idiomas está vacía.".into());
    }
    let mut items = Vec::new();
    for l in list {
        if !tag_ok(&l.tag) || !l.tips.iter().all(|t| tip_ok(t)) {
            return Err("La copia de los idiomas no es válida.".into());
        }
        let tips: Vec<String> = l.tips.iter().map(|t| crate::ps::ps_literal(t)).collect();
        items.push(format!("    @{{ Tag = {}; Tips = @({}) }}", crate::ps::ps_literal(&l.tag), tips.join(", ")));
    }
    Ok(format!(
        "# Volver a poner los idiomas de teclado como estaban\n$datos = @(\n{}\n)\n$lista = $null\nforeach ($d in $datos) {{\n    $nuevo = New-WinUserLanguageList -Language $d.Tag\n    $nuevo[0].InputMethodTips.Clear()\n    foreach ($t in $d.Tips) {{ [void]$nuevo[0].InputMethodTips.Add($t) }}\n    if ($null -eq $lista) {{ $lista = $nuevo }} else {{ $lista.Add($nuevo[0]) }}\n}}\nSet-WinUserLanguageList -LanguageList $lista -Force\n",
        items.join(",\n")
    ))
}

// ---------- Leer Windows ----------

fn read_raw_state() -> Result<RawState, String> {
    // Con otra cuenta, los idiomas se leen de la colmena del cliente (no los de quien abrió AdminOps).
    if crate::target_user::hkcu_redirect().is_some() {
        return Ok(read_hive_state());
    }
    let out = crate::ps::powershell_opts(READ_SCRIPT, crate::ps::Opts { timeout: Some(Duration::from_secs(30)), task: None })?;
    serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada de Windows: {e}"))
}

/// Los textos de un REG_MULTI_SZ (UTF-16LE, separados y terminados por nulos).
pub fn multi_sz(bytes: &[u8]) -> Vec<String> {
    let words: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    words.split(|&w| w == 0).filter(|part| !part.is_empty()).map(String::from_utf16_lossy).collect()
}

/// Los idiomas del cliente leídos del registro: `Control Panel\International\User Profile` (lista y sus
/// métodos de entrada) y `Geo` (país). Con `hkcu_redirect()` activo el registro ya apunta a su colmena.
fn read_hive_state() -> RawState {
    let tags: Vec<String> = registry::read_raw(PROFILE_KEY, "Languages").filter(|r| r.vtype == 7).map(|r| multi_sz(&r.bytes)).unwrap_or_default().into_iter().filter(|t| tag_ok(t)).collect();
    let names = native_names(&tags);
    let languages = tags
        .into_iter()
        .map(|tag| {
            let tips: Vec<String> = registry::values(&format!(r"{PROFILE_KEY}\{tag}")).into_iter().map(|(n, _)| n).filter(|n| parse_tip(n).is_some()).collect();
            let name = names.get(&tag).cloned().unwrap_or_default();
            RawLanguage { tag, name, tips }
        })
        .collect();
    let region = registry::read_string(r"HKCU\Control Panel\International\Geo", "Name").unwrap_or_default();
    RawState { languages, region, ui: String::new() }
}

/// Cómo se llama cada idioma en su propia lengua («Español (República Dominicana)»), por el .NET de Windows.
fn native_names(tags: &[String]) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    if tags.is_empty() {
        return out;
    }
    let script = format!(
        "{}$r = @{{}}\nforeach ($t in ($lista -split '\\|')) {{ $r[$t] = try {{ [System.Globalization.CultureInfo]::GetCultureInfo($t).NativeName }} catch {{ '' }} }}\nConvertTo-Json -Compress -InputObject $r\n",
        crate::ps::text_var("lista", &tags.join("|"))
    );
    if let Ok(text) = crate::ps::powershell_opts(&script, crate::ps::Opts { timeout: Some(Duration::from_secs(20)), task: None }) {
        if let Ok(map) = serde_json::from_str::<std::collections::HashMap<String, String>>(text.trim()) {
            out = map.into_iter().filter(|(_, v)| !v.is_empty()).collect();
        }
    }
    out
}

/// ¿Hay un paquete de idioma de visualización en español instalado?
fn spanish_pack_installed() -> bool {
    registry::subkeys(r"HKLM\SYSTEM\CurrentControlSet\Control\MUI\UILanguages").iter().any(|k| {
        let k = k.to_ascii_lowercase();
        k == "es" || k.starts_with("es-")
    })
}

/// Le dice a Windows que el atajo de cambio de idioma ha cambiado (sin esto tarda hasta el próximo inicio de sesión).
#[cfg(windows)]
pub fn apply_toggle_change() {
    use windows_sys::Win32::UI::WindowsAndMessaging::SystemParametersInfoW;
    const SPI_SETLANGTOGGLE: u32 = 0x005B;
    // SAFETY: sin puntero ni datos: solo recarga el atajo desde el registro.
    unsafe {
        SystemParametersInfoW(SPI_SETLANGTOGGLE, 0, std::ptr::null_mut(), 0);
    }
}

#[cfg(not(windows))]
pub fn apply_toggle_change() {}

/// El identificador del idioma con el que escribe ahora la ventana en primer plano.
#[cfg(windows)]
fn active_lang_id() -> u32 {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayout;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    unsafe {
        let hwnd = GetForegroundWindow();
        let thread = if hwnd.is_null() { 0 } else { GetWindowThreadProcessId(hwnd, std::ptr::null_mut()) };
        (GetKeyboardLayout(thread) as usize & 0xFFFF) as u32
    }
}

#[cfg(not(windows))]
fn active_lang_id() -> u32 {
    0
}

pub fn state() -> Result<KbdState, String> {
    let raw = read_raw_state()?;
    let rows = build_rows(&raw.languages, active_lang_id());
    let tags: Vec<String> = raw.languages.iter().map(|l| l.tag.clone()).collect();
    let code = effective_code(
        registry::read_string(TOGGLE_KEY, "Hotkey").as_deref(),
        registry::read_string(TOGGLE_KEY, "Language Hotkey").as_deref(),
        registry::read_string(TOGGLE_KEY, "Layout Hotkey").as_deref(),
    );
    let other_user = crate::target_user::hkcu_redirect().is_some();
    let plan = plan_only_spanish(&tags, &raw.ui, spanish_pack_installed());
    Ok(KbdState {
        rows,
        language_count: raw.languages.len(),
        region: raw.region.trim().to_ascii_uppercase(),
        shortcut: Shortcut { label: shortcut_label(&code).to_string(), code },
        has_spanish: pick_spanish(&tags).is_some(),
        other_user,
        kept: plan.keep.first().cloned(),
        note: plan.keep.first().map(|t| kept_note(t)),
        warning: plan.warning,
    })
}

#[tauri::command(async)]
pub fn kbd_state() -> Result<KbdState, String> {
    state()
}

/// Vuelve a poner la lista de idiomas (lo usa «Deshacer» del historial).
pub fn restore_languages(list: &[LanguageEntry]) -> Result<(), String> {
    if crate::target_user::hkcu_redirect().is_some() {
        return Err(OTHER_USER_SENTENCE.into());
    }
    let script = restore_script(list)?;
    crate::ps::powershell_opts(&script, crate::ps::Opts { timeout: Some(Duration::from_secs(60)), task: None }).map(|_| ())
}

/// «Dejar solo Español y quitar el atajo». Guarda antes la lista y el atajo para poder deshacerlo.
#[tauri::command(async)]
pub fn kbd_only_spanish(state: State<'_, TweakState>) -> Result<KbdChange, String> {
    if crate::target_user::hkcu_redirect().is_some() {
        return Err(OTHER_USER_SENTENCE.into());
    }
    let raw = read_raw_state()?;
    let tags: Vec<String> = raw.languages.iter().map(|l| l.tag.clone()).collect();
    if pick_spanish(&tags).is_none() {
        return Err("No hay ningún idioma español instalado.".into());
    }
    let plan = plan_only_spanish(&tags, &raw.ui, spanish_pack_installed());
    // Del español solo se queda su primera distribución española.
    let spanish_tips = pick_spanish(&tags).map(|i| trim_spanish_tips(&raw.languages[i].tips)).unwrap_or_default();
    let before: Vec<LanguageEntry> = raw.languages.iter().map(|l| LanguageEntry { tag: l.tag.clone(), tips: l.tips.clone() }).collect();
    let mut backups = vec![Backup::Languages { list: before.clone() }];
    for name in TOGGLE_VALUES {
        backups.push(Backup::Registry { path: TOGGLE_KEY.into(), name: name.into(), previous: registry::read_raw(TOGGLE_KEY, name) });
    }
    let result = (|| -> Result<(), String> {
        crate::ps::powershell_opts(&only_spanish_script(&plan.keep, &spanish_tips), crate::ps::Opts { timeout: Some(Duration::from_secs(60)), task: None })?;
        for name in TOGGLE_VALUES {
            if let Err(e) = registry::write(TOGGLE_KEY, name, RegKind::String, &RegData::Str("3".into())) {
                // Que no quede a medias: se devuelven los idiomas.
                let _ = restore_languages(&before);
                return Err(e);
            }
        }
        // Que Windows use ya el atajo nuevo, sin esperar a otro inicio de sesión.
        apply_toggle_change();
        Ok(())
    })();
    let entry_id = state.log_change(TITLE, backups, &result);
    result.map(|_| KbdChange { message: "Solo Español · atajo quitado".into(), entry_id })
}

/// El comando equivalente de una acción del teclado (`only-spanish`).
pub fn action_script(action: &str) -> Result<(&'static str, String), String> {
    match action {
        "only-spanish" => {
            let mut s = String::from("# Dejar solo el idioma español y quitar el atajo que cambia de idioma\n\n");
            s.push_str("# Idiomas que se quedan sin teclado en vez de quitarse (si el idioma de Windows no tiene paquete en español, AdminOps lo deja ahí)\n");
            s.push_str(&only_spanish_script(&[], &[]));
            s.push_str("\n# Quitar el atajo de cambio de idioma (3 = ninguno). AdminOps lo escribe en el registro del usuario.\n");
            s.push_str("$clave = 'HKCU:\\Keyboard Layout\\Toggle'\nif (-not (Test-Path $clave)) { New-Item -Path $clave -Force | Out-Null }\n");
            s.push_str("foreach ($n in 'Hotkey', 'Language Hotkey', 'Layout Hotkey') { Set-ItemProperty -Path $clave -Name $n -Value '3' -Type String }\n");
            s.push_str("# Que Windows use ya el atajo nuevo (AdminOps llama a SystemParametersInfo con SPI_SETLANGTOGGLE)\n");
            s.push_str("Add-Type -Namespace AdminOps -Name Kbd -MemberDefinition '[DllImport(\"user32.dll\")] public static extern bool SystemParametersInfo(uint a, uint b, IntPtr c, uint d);'\n");
            s.push_str("[void][AdminOps.Kbd]::SystemParametersInfo(0x5B, 0, [IntPtr]::Zero, 0)\n");
            Ok(("Dejar solo Español y quitar el atajo", s))
        }
        _ => Err(format!("Acción desconocida: kbd.{action}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_metodo_de_entrada_se_separa_en_idioma_y_distribucion() {
        assert_eq!(parse_tip("0C0A:0000040A"), Some((0x0C0A, Some("0000040A".into()))));
        assert_eq!(parse_tip("0409:00020409"), Some((0x0409, Some("00020409".into()))));
        // Un editor de métodos de entrada lleva guías: sin distribución propia.
        assert_eq!(parse_tip("0411:{03B5835F-F03C-411B-9CE2-AA23E1171E36}{A76C93D9-5523-4E90-AAFA-4DB112F9AC76}"), Some((0x0411, None)));
        assert_eq!(parse_tip("sin dos puntos"), None);
        assert_eq!(parse_tip("ZZZZ:0000040A"), None);
        assert_eq!(parse_tip("12345:0000040A"), None);
    }

    #[test]
    fn las_distribuciones_conocidas_salen_en_espanol() {
        assert_eq!(layout_name("0000040A"), Some("Español"));
        assert_eq!(layout_name("0000080A"), Some("Latinoamericano"));
        assert_eq!(layout_name("00000409"), Some("Inglés (EE. UU.)"));
        assert_eq!(layout_name("DEADBEEF"), None);
    }

    #[test]
    fn se_elige_el_primer_espanol() {
        let t = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(pick_spanish(&t(&["en-US", "es-DO", "es-ES"])), Some(1));
        assert_eq!(pick_spanish(&t(&["en-US", "es"])), Some(1));
        assert_eq!(pick_spanish(&t(&["es-419"])), Some(0));
        assert_eq!(pick_spanish(&t(&["en-US", "fr-FR"])), None);
        // «est» (estonio) empieza por «es» pero no es español.
        assert_eq!(pick_spanish(&t(&["et-EE", "est"])), None);
        assert_eq!(pick_spanish(&[]), None);
    }

    #[test]
    fn el_atajo_vale_el_del_idioma_y_si_no_el_general() {
        assert_eq!(effective_code(None, None, None), "1");
        assert_eq!(effective_code(Some("2"), None, None), "2");
        assert_eq!(effective_code(Some("1"), Some("3"), None), "3");
        assert_eq!(effective_code(Some(" "), Some(""), None), "1");
        // Con el de idioma quitado, el de distribución sigue cambiando el teclado.
        assert_eq!(effective_code(Some("3"), Some("3"), Some("2")), "2");
        assert_eq!(effective_code(Some("1"), Some("3"), Some("1")), "1");
        assert_eq!(effective_code(Some("3"), Some("3"), Some("3")), "3");
        assert_eq!(effective_code(Some("1"), Some("1"), Some("2")), "1");
        assert_eq!(shortcut_label("1"), "Alt+Mayús");
        assert_eq!(shortcut_label("2"), "Ctrl+Mayús");
        assert_eq!(shortcut_label("3"), "Sin atajo");
        assert_eq!(shortcut_label("4"), "Acento grave (`)");
        assert_eq!(shortcut_label("9"), "Otro atajo");
    }

    #[test]
    fn las_filas_marcan_solo_la_primera_distribucion_activa() {
        let langs = vec![
            RawLanguage { tag: "es-DO".into(), name: "Español (República Dominicana)".into(), tips: vec!["1C0A:0000080A".into(), "1C0A:0000040A".into()] },
            RawLanguage { tag: "en-US".into(), name: "".into(), tips: vec!["0409:00000409".into()] },
        ];
        let rows = build_rows(&langs, 0x1C0A);
        assert_eq!(rows.len(), 3);
        assert_eq!((rows[0].layout.as_str(), rows[0].active), ("Latinoamericano", true));
        assert_eq!((rows[1].layout.as_str(), rows[1].active), ("Español", false));
        assert_eq!((rows[2].language.as_str(), rows[2].active), ("en-US", false));
        assert!(build_rows(&langs, 0x0000).iter().all(|r| !r.active));
    }

    #[test]
    fn el_script_de_restaurar_lleva_cada_idioma_y_se_niega_a_lo_raro() {
        let list = vec![
            LanguageEntry { tag: "es-DO".into(), tips: vec!["1C0A:0000080A".into()] },
            LanguageEntry { tag: "en-US".into(), tips: vec!["0409:00000409".into()] },
        ];
        let s = restore_script(&list).unwrap();
        assert!(s.contains("Tag = 'es-DO'") && s.contains("'0409:00000409'"), "{s}");
        assert!(restore_script(&[]).is_err());
        assert!(restore_script(&[LanguageEntry { tag: "es'; calc".into(), tips: vec![] }]).is_err());
        assert!(restore_script(&[LanguageEntry { tag: "es-ES".into(), tips: vec!["x'; calc".into()] }]).is_err());
    }

    #[test]
    fn del_espanol_solo_se_queda_su_primera_distribucion_espanola() {
        let t = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        // La inglesa dentro de es-DO se quita; la primera española se queda.
        assert_eq!(trim_spanish_tips(&t(&["1C0A:00000409", "1C0A:0000080A", "1C0A:0000040A"])), t(&["1C0A:0000080A"]));
        assert_eq!(trim_spanish_tips(&t(&["0C0A:0000040A", "0C0A:00000409"])), t(&["0C0A:0000040A"]));
        assert_eq!(trim_spanish_tips(&t(&["0C0A:0001040A"])), t(&["0C0A:0001040A"]));
        // Sin ninguna española, se deja como estaba.
        assert_eq!(trim_spanish_tips(&t(&["0C0A:00000409"])), t(&["0C0A:00000409"]));
        assert!(trim_spanish_tips(&[]).is_empty());
        assert!(is_spanish_klid("0000040A") && is_spanish_klid("0000080a") && !is_spanish_klid("00000409") && !is_spanish_klid("0A"));
    }

    #[test]
    fn si_windows_esta_en_ingles_no_se_le_quita_el_idioma_sin_avisar() {
        let t = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let tags = t(&["es-DO", "en-US"]);
        // Con paquete en español: se avisa en la confirmación y se quita.
        let p = plan_only_spanish(&tags, "en-US", true);
        assert!(p.keep.is_empty() && p.warning.as_deref().is_some_and(|w| w.contains("en-US") && w.contains("Español")), "{p:?}");
        // Sin paquete: en-US se queda en la lista, sin teclado.
        assert_eq!(plan_only_spanish(&tags, "en-US", false), OnlySpanishPlan { keep: t(&["en-US"]), warning: None });
        assert_eq!(plan_only_spanish(&tags, "EN-us", false).keep, t(&["en-US"]));
        // Windows en español, otro idioma de pantalla o sin dato: nada especial.
        assert_eq!(plan_only_spanish(&tags, "es-DO", false), OnlySpanishPlan::default());
        assert_eq!(plan_only_spanish(&tags, "fr-FR", false), OnlySpanishPlan::default());
        assert_eq!(plan_only_spanish(&tags, "", true), OnlySpanishPlan::default());
    }

    #[test]
    fn el_script_de_dejar_solo_espanol_lleva_lo_que_se_conserva() {
        let s = only_spanish_script(&["en-US".to_string(), "x'; calc".to_string()], &["1C0A:0000080A".to_string(), "mal'; calc".to_string()]);
        assert!(s.starts_with("$conservar = @('en-US')\n$tipsEspanol = @('1C0A:0000080A')\n"), "{s}");
        assert!(only_spanish_script(&[], &[]).starts_with("$conservar = @()\n$tipsEspanol = @()\n"));
    }

    #[test]
    fn un_reg_multi_sz_se_parte_en_sus_textos() {
        let mut b: Vec<u8> = Vec::new();
        for t in ["es-DO", "en-US"] {
            b.extend(t.encode_utf16().flat_map(|u| u.to_le_bytes()));
            b.extend([0, 0]);
        }
        b.extend([0, 0]);
        assert_eq!(multi_sz(&b), vec!["es-DO".to_string(), "en-US".to_string()]);
        assert!(multi_sz(&[]).is_empty());
        assert!(multi_sz(&[0, 0, 0, 0]).is_empty());
    }

    #[test]
    fn todos_los_scripts_son_powershell_valido() {
        let list = vec![LanguageEntry { tag: "es-DO".into(), tips: vec!["1C0A:0000080A".into(), "0C0A:0000040A".into()] }, LanguageEntry { tag: "en-US".into(), tips: vec![] }];
        for (name, script) in [
            ("leer", READ_SCRIPT.to_string()),
            ("dejar solo español", only_spanish_script(&[], &[])),
            ("dejar solo español conservando", only_spanish_script(&["en-US".to_string()], &["1C0A:0000080A".to_string()])),
            ("restaurar", restore_script(&list).unwrap()),
            ("comando", action_script("only-spanish").unwrap().1),
        ] {
            let errors = crate::ps::parse_errors(&script);
            assert!(errors.trim().is_empty(), "{name}: {errors}");
        }
        assert!(action_script("otra").is_err());
    }

    /// Equipo real (solo lee): `cargo test --lib teclado_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn teclado_real() {
        let s = state().expect("estado");
        println!("Idiomas: {} · país {} · atajo {} ({}) · español instalado: {}", s.language_count, s.region, s.shortcut.label, s.shortcut.code, s.has_spanish);
        println!("Otra cuenta: {} · se queda sin teclado: {:?} · aviso al confirmar: {:?}", s.other_user, s.kept, s.warning);
        for r in &s.rows {
            println!("  {} · {} · {} ({:?}) · idioma 0x{:04X} · activo {}", r.tag, r.language, r.layout, r.klid, r.lang_id, r.active);
        }
        assert!(s.language_count > 0);
    }
}
