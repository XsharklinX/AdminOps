//! Suspensión y pantalla según el uso: cuándo se apaga la pantalla, cuándo se suspende y qué hace la
//! tapa, enchufado y con batería, dicho en una frase; y perfiles de un clic («Oficina»,
//! «Presentaciones», «No se apaga nunca», «Portátil en base») o a medida.
//!
//! Se lee y se cambia el plan de energía activo con la API de Windows (powrprof), no con `powercfg`:
//! su salida cambia con el idioma. Los cambios valen para todos los usuarios del equipo (el plan es
//! del equipo) y necesitan administrador.

use serde::{Deserialize, Serialize};

/// Minutos; 0 = nunca.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Side {
    pub screen_min: u32,
    pub sleep_min: u32,
    /// Qué hace la tapa: 0 nada, 1 suspender, 2 hibernar, 3 apagar.
    pub lid: u32,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PowerState {
    pub ac: Side,
    pub dc: Side,
    /// Tiene tapa (portátil).
    pub lid_present: bool,
    pub battery: bool,
    /// El perfil de AdminOps que coincide con lo que hay, si alguno.
    pub profile: Option<String>,
    pub admin: bool,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: &'static str,
    pub ac: Side,
    pub dc: Side,
}

/// Los perfiles de un clic. `lid` en DC (con batería) siempre suspende: un portátil cerrado en la
/// mochila no debe seguir encendido.
pub fn profiles() -> Vec<Profile> {
    vec![
        Profile { id: "office", ac: Side { screen_min: 10, sleep_min: 30, lid: 1 }, dc: Side { screen_min: 5, sleep_min: 15, lid: 1 } },
        Profile { id: "present", ac: Side { screen_min: 0, sleep_min: 0, lid: 1 }, dc: Side { screen_min: 0, sleep_min: 0, lid: 1 } },
        Profile { id: "always", ac: Side { screen_min: 15, sleep_min: 0, lid: 0 }, dc: Side { screen_min: 10, sleep_min: 30, lid: 1 } },
        Profile { id: "dock", ac: Side { screen_min: 10, sleep_min: 30, lid: 0 }, dc: Side { screen_min: 5, sleep_min: 15, lid: 1 } },
    ]
}

/// ¿Coincide lo que hay con un perfil? Sin tapa, la tapa no cuenta.
pub fn match_profile(ac: Side, dc: Side, lid_present: bool) -> Option<String> {
    let eq = |a: Side, b: Side| a.screen_min == b.screen_min && a.sleep_min == b.sleep_min && (!lid_present || a.lid == b.lid);
    // «Portátil en base» y «Oficina» solo se distinguen por la tapa.
    profiles().into_iter().filter(|p| lid_present || p.id != "dock").find(|p| eq(p.ac, ac) && eq(p.dc, dc)).map(|p| p.id.to_string())
}

pub fn valid_side(s: &Side) -> bool {
    s.screen_min <= 24 * 60 && s.sleep_min <= 24 * 60 && s.lid <= 3
}

fn hibernate_file() -> Option<std::path::PathBuf> {
    crate::paths::shared_data_dir_early().map(|d| d.join("energia-hibernar.json"))
}

/// La hibernación (segundos, enchufado y con batería) que había antes de «nunca se suspende».
fn saved_hibernate() -> Option<(u32, u32)> {
    hibernate_file().and_then(|f| std::fs::read(f).ok()).and_then(|b| serde_json::from_slice(&b).ok())
}

fn save_hibernate(v: (u32, u32)) {
    if let Some(f) = hibernate_file() {
        let _ = crate::paths::write_json(&f, &v);
    }
}

#[cfg(windows)]
mod win {
    use super::Side;
    use windows_sys::core::GUID;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::System::Power::{GetPwrCapabilities, PowerGetActiveScheme, PowerReadACValueIndex, PowerReadDCValueIndex, PowerSetActiveScheme, PowerWriteACValueIndex, PowerWriteDCValueIndex, SYSTEM_POWER_CAPABILITIES};

    const fn g(d1: u32, d2: u16, d3: u16, d4: [u8; 8]) -> GUID {
        GUID { data1: d1, data2: d2, data3: d3, data4: d4 }
    }
    const SUB_VIDEO: GUID = g(0x7516b95f, 0xf776, 0x4464, [0x8c, 0x53, 0x06, 0x16, 0x7f, 0x40, 0xcc, 0x99]);
    const VIDEOIDLE: GUID = g(0x3c0bc021, 0xc8a8, 0x4e07, [0xa9, 0x73, 0x6b, 0x14, 0xcb, 0xcb, 0x2b, 0x7e]);
    const SUB_SLEEP: GUID = g(0x238c9fa8, 0x0aad, 0x41ed, [0x83, 0xf4, 0x97, 0xbe, 0x24, 0x2c, 0x8f, 0x20]);
    const STANDBYIDLE: GUID = g(0x29f6c1db, 0x86da, 0x48c5, [0x9f, 0xdb, 0xf2, 0xb6, 0x7b, 0x1f, 0x44, 0xda]);
    const HIBERNATEIDLE: GUID = g(0x9d7815a6, 0x7ee4, 0x497e, [0x88, 0x88, 0x51, 0x5a, 0x05, 0xf0, 0x23, 0x64]);
    const SUB_BUTTONS: GUID = g(0x4f971e89, 0xeebd, 0x4455, [0xa8, 0xde, 0x9e, 0x59, 0x04, 0x0e, 0x73, 0x47]);
    const LIDACTION: GUID = g(0x5ca83367, 0x6e45, 0x459f, [0xa2, 0x7b, 0x47, 0x6b, 0x1d, 0x01, 0xc9, 0x36]);

    struct Scheme(*mut GUID);
    impl Drop for Scheme {
        fn drop(&mut self) {
            // SAFETY: lo reservó PowerGetActiveScheme y se libera una sola vez.
            unsafe { LocalFree(self.0.cast()) };
        }
    }

    fn active() -> Result<Scheme, String> {
        let mut p: *mut GUID = std::ptr::null_mut();
        // SAFETY: `p` recibe un GUID que se libera con LocalFree (Scheme).
        let r = unsafe { PowerGetActiveScheme(std::ptr::null_mut(), &mut p) };
        if r != 0 || p.is_null() {
            return Err(format!("No se pudo leer el plan de energía (error {r})."));
        }
        Ok(Scheme(p))
    }

    fn read(s: &Scheme, sub: &GUID, set: &GUID) -> (u32, u32) {
        let (mut ac, mut dc) = (0u32, 0u32);
        // SAFETY: punteros a GUID constantes y a enteros propios.
        unsafe {
            PowerReadACValueIndex(std::ptr::null_mut(), s.0, sub, set, &mut ac);
            PowerReadDCValueIndex(std::ptr::null_mut(), s.0, sub, set, &mut dc);
        }
        (ac, dc)
    }

    fn write(s: &Scheme, sub: &GUID, set: &GUID, ac: u32, dc: u32) -> Result<(), String> {
        // SAFETY: como en `read`.
        let (a, d) = unsafe { (PowerWriteACValueIndex(std::ptr::null_mut(), s.0, sub, set, ac), PowerWriteDCValueIndex(std::ptr::null_mut(), s.0, sub, set, dc)) };
        if a != 0 || d != 0 {
            return Err(if a == 5 || d == 5 { "Hace falta abrir AdminOps como administrador para cambiar la energía.".into() } else { format!("Windows no dejó cambiar la energía (error {}).", if a != 0 { a } else { d }) });
        }
        Ok(())
    }

    pub fn caps() -> (bool, bool) {
        let mut c: SYSTEM_POWER_CAPABILITIES = unsafe { std::mem::zeroed() };
        // SAFETY: estructura propia.
        if unsafe { GetPwrCapabilities(&mut c) } {
            (c.LidPresent, c.SystemBatteriesPresent)
        } else {
            (false, false)
        }
    }

    pub fn get() -> Result<(Side, Side), String> {
        let s = active()?;
        let (vac, vdc) = read(&s, &SUB_VIDEO, &VIDEOIDLE);
        let (sac, sdc) = read(&s, &SUB_SLEEP, &STANDBYIDLE);
        let (lac, ldc) = read(&s, &SUB_BUTTONS, &LIDACTION);
        // Segundos → minutos (redondeando hacia arriba: 90 s se dice «2 min», no «1»).
        let m = |secs: u32| secs.div_ceil(60);
        Ok((Side { screen_min: m(vac), sleep_min: m(sac), lid: lac }, Side { screen_min: m(vdc), sleep_min: m(sdc), lid: ldc }))
    }

    pub fn set(ac: Side, dc: Side, lid_present: bool) -> Result<(), String> {
        let s = active()?;
        write(&s, &SUB_VIDEO, &VIDEOIDLE, ac.screen_min * 60, dc.screen_min * 60)?;
        write(&s, &SUB_SLEEP, &STANDBYIDLE, ac.sleep_min * 60, dc.sleep_min * 60)?;
        // «Nunca se suspende» también es «nunca hiberna sola»: si no, se apagaría igual a las horas. Lo que
        // había se guarda, y al volver a un perfil que sí se suspende se recupera.
        let (hac, hdc) = read(&s, &SUB_SLEEP, &HIBERNATEIDLE);
        let saved = super::saved_hibernate();
        let pick = |never: bool, now: u32, saved: Option<u32>| if never { 0 } else if now == 0 { saved.unwrap_or(0) } else { now };
        let (nac, ndc) = (pick(ac.sleep_min == 0, hac, saved.map(|x| x.0)), pick(dc.sleep_min == 0, hdc, saved.map(|x| x.1)));
        if (ac.sleep_min == 0 && hac != 0) || (dc.sleep_min == 0 && hdc != 0) {
            super::save_hibernate((if hac != 0 { hac } else { saved.map_or(0, |x| x.0) }, if hdc != 0 { hdc } else { saved.map_or(0, |x| x.1) }));
        }
        if (nac, ndc) != (hac, hdc) {
            write(&s, &SUB_SLEEP, &HIBERNATEIDLE, nac, ndc)?;
        }
        if lid_present {
            write(&s, &SUB_BUTTONS, &LIDACTION, ac.lid, dc.lid)?;
        }
        // Vuelve a activar el mismo plan: así Windows aplica los valores al momento.
        // SAFETY: GUID del plan activo, aún vivo.
        let r = unsafe { PowerSetActiveScheme(std::ptr::null_mut(), s.0) };
        if r != 0 {
            return Err(format!("Windows no aplicó el plan de energía (error {r})."));
        }
        Ok(())
    }
}

#[tauri::command]
pub async fn power_state() -> Result<PowerState, String> {
    tauri::async_runtime::spawn_blocking(|| {
        #[cfg(windows)]
        {
            let (lid_present, battery) = win::caps();
            let (ac, dc) = win::get()?;
            Ok(PowerState { ac, dc, lid_present, battery, profile: match_profile(ac, dc, lid_present), admin: crate::elevation::is_elevated() })
        }
        #[cfg(not(windows))]
        Err("Solo en Windows.".to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn power_profiles() -> Vec<Profile> {
    profiles()
}

/// Pone los valores (de un perfil o a medida) en el plan activo.
#[tauri::command]
pub async fn power_set(ac: Side, dc: Side) -> Result<PowerState, String> {
    if !valid_side(&ac) || !valid_side(&dc) {
        return Err("Valores de energía no válidos.".into());
    }
    #[cfg(windows)]
    {
        let (lid_present, _) = win::caps();
        tauri::async_runtime::spawn_blocking(move || win::set(ac, dc, lid_present)).await.map_err(|e| e.to_string())??;
        log::info!("Energía: enchufado {ac:?}, batería {dc:?}");
    }
    power_state().await
}

/// Qué despertó al equipo la última vez y qué lo puede despertar (`powercfg`, texto de Windows tal cual).
#[tauri::command]
pub async fn power_wake() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let run = |args: &[&str]| crate::ps::exec("powercfg", args).unwrap_or_else(|e| e);
        let last = run(&["/lastwake"]);
        let timers = run(&["/waketimers"]);
        let armed = run(&["/devicequery", "wake_armed"]);
        Ok(format!("{}\n\n{}\n\nDispositivos que pueden despertarlo:\n{}", last.trim(), timers.trim(), armed.trim()))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconoce_los_perfiles() {
        let p = profiles();
        let office = p.iter().find(|x| x.id == "office").unwrap();
        assert_eq!(match_profile(office.ac, office.dc, true).as_deref(), Some("office"));
        let dock = p.iter().find(|x| x.id == "dock").unwrap();
        assert_eq!(match_profile(dock.ac, dock.dc, true).as_deref(), Some("dock"));
        // Sin tapa, «en base» es igual que «oficina»: se dice «oficina».
        assert_eq!(match_profile(dock.ac, dock.dc, false).as_deref(), Some("office"));
        assert_eq!(match_profile(Side { screen_min: 3, sleep_min: 7, lid: 1 }, office.dc, true), None);
    }

    #[test]
    fn limites() {
        assert!(valid_side(&Side { screen_min: 0, sleep_min: 0, lid: 3 }));
        assert!(!valid_side(&Side { screen_min: 0, sleep_min: 0, lid: 4 }));
        assert!(!valid_side(&Side { screen_min: 2000, sleep_min: 0, lid: 0 }));
    }
}
