//! Diccionario de códigos de error sin conexión: 0x80070005, 0x800f081f,
//! 0xc000021a, STOP 0x133… Qué es, por qué suele pasar y qué hacer, sin
//! Internet (cuando más hace falta). Lo usan Windows Update, los pantallazos
//! azules y Ctrl+K.

use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ErrorInfo {
    /// «0x80070005»
    pub code: String,
    /// Nombre corto («Acceso denegado», «DPC_WATCHDOG_VIOLATION»).
    pub name: String,
    /// windows · update · stop · office · network · activation
    pub area: &'static str,
    pub what: &'static str,
    pub why: &'static str,
    pub todo: &'static str,
    /// Pantalla de AdminOps donde se arregla («repair», «space»…), si la hay.
    pub page: &'static str,
}

type Row = (u32, &'static str, &'static str, &'static str, &'static str, &'static str, &'static str);

/// (código, nombre, área, qué es, por qué, qué hacer, pantalla)
const CODES: &[Row] = &[
    // ---- Windows (HRESULT de Win32) ----
    (0x8007_0005, "Acceso denegado", "windows", "Windows no deja escribir o leer donde se intenta.", "Falta de permisos de administrador, un antivirus que bloquea la carpeta o permisos de archivo cambiados.", "Ejecuta como administrador. Si sigue, revisa el antivirus y los permisos de la carpeta.", ""),
    (0x8007_0002, "No se encuentra el archivo", "windows", "Falta un archivo que el proceso necesita.", "Archivo borrado o movido, instalación incompleta o una caché de actualizaciones dañada.", "Vuelve a descargar o reinstalar. En Windows Update: reparar Windows Update.", "repair"),
    (0x8007_0003, "No se encuentra la ruta", "windows", "La carpeta indicada no existe.", "Ruta mal escrita, unidad desconectada o carpeta borrada.", "Comprueba que la unidad está conectada y que la carpeta existe.", ""),
    (0x8007_0020, "Archivo en uso", "windows", "Otro programa tiene el archivo abierto.", "Un programa, el antivirus o el indexador están usando el archivo.", "Cierra el programa que lo usa o reinicia y vuelve a intentarlo.", "processes"),
    (0x8007_0070, "Disco lleno", "windows", "No queda espacio en el disco.", "Poco espacio libre para la operación.", "Libera espacio (temporales, papelera, actualizaciones antiguas).", "space"),
    (0x8007_000E, "Memoria insuficiente", "windows", "No hay memoria suficiente para terminar.", "Poca RAM libre o el archivo de paginación desactivado.", "Cierra programas, reinicia y comprueba el archivo de paginación.", "processes"),
    (0x8007_0057, "Parámetro incorrecto", "windows", "Una orden recibió un valor que no admite.", "Configuración dañada, sistema de archivos con errores o un driver antiguo.", "Comprueba el disco (Discos → Reparar) y actualiza el driver implicado.", "space"),
    (0x8007_045D, "Error de E/S del dispositivo", "windows", "El disco o el USB no responde bien al leer o escribir.", "Disco con sectores dañados, cable o puerto defectuoso.", "Mira la salud del disco en Discos y cambia el cable o el puerto.", "space"),
    (0x8007_0017, "Error de datos (CRC)", "windows", "Los datos leídos no cuadran con su comprobación.", "Sectores dañados en el disco o un medio (DVD, USB) defectuoso.", "Copia lo que puedas y revisa la salud del disco.", "space"),
    (0x8007_04C7, "Operación cancelada", "windows", "El usuario o un programa canceló la operación.", "Se cerró una ventana, se canceló un diálogo o se apagó el equipo.", "Vuelve a intentarlo sin cerrar nada.", ""),
    (0x8007_05B4, "Tiempo de espera agotado", "windows", "La operación tardó demasiado.", "Equipo saturado, red lenta o un servicio colgado.", "Reinicia y vuelve a intentarlo; si se repite, revisa el servicio implicado.", ""),
    (0x8007_06BA, "Servidor RPC no disponible", "network", "El equipo de destino no contesta.", "Equipo apagado, sin red o con el cortafuegos cerrado.", "Comprueba que responde (ping) y que el cortafuegos permite la administración remota.", "nettools"),
    (0x8007_0035, "No se encuentra la ruta de red", "network", "No se llega a ese equipo o carpeta compartida.", "Nombre mal escrito, equipo apagado, SMB desactivado o el descubrimiento de red apagado.", "Prueba con la IP en vez del nombre y revisa que la carpeta sigue compartida.", "shares"),
    (0x8007_04CF, "Red inaccesible", "network", "No hay camino hasta el destino.", "Sin conexión, sin puerta de enlace o una VPN que corta el tráfico.", "Repara la red y comprueba la VPN.", "network"),
    (0x8007_0422, "Servicio deshabilitado", "windows", "El servicio necesario está deshabilitado.", "Un «optimizador» o alguien lo desactivó.", "Vuelve a ponerlo en Manual o Automático en Servicios.", "services"),
    (0x8007_041D, "El servicio no respondió a tiempo", "windows", "Un servicio no arrancó en el tiempo esperado.", "Servicio dañado, dependencia rota o equipo muy cargado.", "Reinicia el equipo; si sigue, repara el sistema (SFC y DISM).", "repair"),
    (0x8007_0643, "Error grave durante la instalación", "update", "La instalación falló a mitad.", "Componente .NET dañado, falta de espacio o un antivirus que bloquea.", "Repara .NET y Windows Update, libera espacio y vuelve a intentarlo.", "repair"),
    (0x8007_0652, "Ya hay otra instalación en curso", "update", "Windows Installer está ocupado con otra instalación.", "Otra instalación o actualización está en marcha.", "Espera a que termine o reinicia y vuelve a intentarlo.", ""),
    (0x8007_1A90, "Operación de transacciones no admitida", "update", "El registro de transacciones del sistema de archivos está dañado.", "Cierre inesperado durante una actualización.", "Reinicia y repara el sistema (SFC y DISM).", "repair"),
    (0x8007_000D, "Datos no válidos", "update", "Un archivo de la actualización está dañado.", "Descarga dañada o almacén de componentes con errores.", "Repara Windows Update y el almacén de componentes (DISM).", "repair"),
    (0x8007_0BC2, "Reinicio pendiente", "update", "Hace falta reiniciar para seguir.", "Una actualización anterior espera un reinicio.", "Reinicia el equipo y vuelve a buscar actualizaciones.", ""),
    (0x8007_0BC9, "Reinicio pendiente para terminar", "update", "La operación necesita reiniciar para completarse.", "Hay cambios del sistema a medias.", "Reinicia y vuelve a intentarlo.", ""),
    (0x8007_2EE7, "No se resuelve el nombre del servidor", "network", "No se encuentra el servidor de actualizaciones.", "DNS que falla, proxy o sin Internet.", "Repara la red y comprueba el proxy.", "network"),
    (0x8007_2EFD, "No se puede conectar con el servidor", "network", "No se llega al servidor.", "Cortafuegos, proxy o sin Internet.", "Comprueba Internet, el proxy y el cortafuegos.", "network"),
    (0x8007_2EFE, "Conexión interrumpida", "network", "La conexión con el servidor se cortó.", "Red inestable, proxy o antivirus que inspecciona el tráfico.", "Vuelve a intentarlo con otra red o sin el proxy.", "network"),
    (0x8007_2F8F, "Error de seguridad en la conexión", "network", "Falló la conexión segura (TLS).", "La hora del equipo está mal o falta un certificado raíz.", "Sincroniza la hora y vuelve a intentarlo.", "repair"),
    // ---- Windows Update / servicio ----
    (0x800F_081F, "Faltan archivos de origen", "update", "No se encuentran los archivos para reparar o instalar un componente.", "Almacén de componentes dañado o falta el origen (.NET 3.5, idioma…).", "Repara el almacén de componentes (DISM /RestoreHealth) con Internet y vuelve a intentarlo.", "repair"),
    (0x800F_0906, "No se pudo descargar el origen", "update", "No se descargaron los archivos de un componente.", "Sin Internet, WSUS de empresa que no lo tiene o directiva que lo impide.", "Comprueba Internet; en empresa, pide al administrador del WSUS.", "network"),
    (0x800F_0922, "Falló la instalación de la actualización", "update", "No se pudo terminar de instalar.", "Partición de recuperación o reservada sin espacio, o conexión con el servidor cortada (VPN).", "Libera espacio, desconecta la VPN y vuelve a intentarlo.", "space"),
    (0x800F_0988, "Limpieza de componentes pendiente", "update", "El almacén de componentes necesita una limpieza.", "Restos de actualizaciones anteriores.", "Ejecuta la limpieza de componentes (DISM /StartComponentCleanup) y reinicia.", "repair"),
    (0x800F_0831, "Falta un paquete anterior", "update", "Falta la actualización de la que depende esta.", "Una actualización previa no se instaló bien.", "Instala la última actualización de la pila de servicio y repara el almacén (DISM).", "repair"),
    (0x8007_3712, "Almacén de componentes dañado", "update", "Falta o está dañado un archivo del almacén de componentes.", "Corrupción tras un apagado inesperado o un disco con errores.", "Repara el almacén (DISM /RestoreHealth) y después SFC.", "repair"),
    (0x8007_0490, "Elemento no encontrado", "update", "Falta un elemento del almacén de componentes o del registro.", "Almacén dañado o una actualización a medias.", "Repara el almacén (DISM) y Windows Update.", "repair"),
    (0x8024_402C, "No se puede conectar con Windows Update", "update", "No se llega a los servidores de actualizaciones.", "Proxy, DNS o cortafuegos.", "Repara la red y quita el proxy si sobra.", "network"),
    (0x8024_4022, "Servicio de actualizaciones no disponible", "update", "El servidor de actualizaciones no responde.", "Problema temporal del servidor o un WSUS de empresa caído.", "Vuelve a intentarlo más tarde.", ""),
    (0x8024_0438, "Sin conexión con el servicio", "update", "Windows Update no consigue hablar con el servicio.", "Proxy, directivas que bloquean Windows Update o sin Internet.", "Comprueba Internet y las directivas de Windows Update.", "network"),
    (0x8024_A105, "Error de descarga de Windows Update", "update", "La descarga no se completó.", "Caché de actualizaciones dañada o red inestable.", "Repara Windows Update (borra su caché) y vuelve a intentarlo.", "repair"),
    (0x8024_2006, "Metadatos de la actualización dañados", "update", "La información de la actualización está dañada.", "Caché de Windows Update dañada.", "Repara Windows Update.", "repair"),
    (0x8024_0034, "La descarga falló", "update", "No se pudo descargar la actualización.", "Red inestable o caché dañada.", "Repara Windows Update y vuelve a intentarlo.", "repair"),
    (0x8024_001E, "Operación interrumpida", "update", "Windows Update se detuvo a mitad.", "Apagado, servicio parado o red cortada.", "Vuelve a buscar actualizaciones con el equipo enchufado.", ""),
    (0x8024_0FFF, "Error inesperado de Windows Update", "update", "Windows Update falló sin dar un motivo concreto.", "Normalmente, caché o componentes dañados.", "Repara Windows Update; si sigue, DISM y SFC.", "repair"),
    (0x8009_2004, "Firma o paquete no encontrado", "update", "No se encuentra la firma del paquete.", "Paquete descargado dañado o una actualización retirada.", "Repara Windows Update y vuelve a buscar.", "repair"),
    (0x8009_2002, "Error de firma", "update", "La firma de la actualización no es válida.", "Descarga dañada o fecha del equipo incorrecta.", "Sincroniza la hora y repara Windows Update.", "repair"),
    (0xC190_0101, "Error de driver al actualizar Windows", "update", "La actualización a una versión nueva se deshizo por un driver.", "Un driver (sobre todo de almacenamiento o de la gráfica) incompatible, o poco espacio.", "Actualiza drivers, desconecta USB que no hagan falta y libera espacio.", "space"),
    (0xC190_0208, "Programa incompatible", "update", "Hay un programa que impide actualizar Windows.", "Antivirus antiguo u otro programa marcado como incompatible.", "Desinstala o actualiza el programa que indica el asistente.", "uninstall"),
    (0xC190_0223, "No se pudo descargar la actualización", "update", "La descarga de la nueva versión falló.", "Bloqueo de red (proxy, DNS filtrado) o VPN.", "Prueba con otra red, sin VPN.", "network"),
    (0xC190_0204, "Edición no compatible", "update", "La edición o el idioma no permiten esta actualización.", "Imagen distinta de la instalada o idioma diferente.", "Usa el asistente oficial con la misma edición e idioma.", ""),
    (0xC190_020E, "Poco espacio para la actualización", "update", "No hay sitio para la nueva versión.", "Menos de 20 GB libres en el disco del sistema.", "Libera espacio o usa un USB externo para la actualización.", "space"),
    // ---- Arranque y sistema (NTSTATUS) ----
    (0xC000_021A, "Fallo de un proceso crítico", "stop", "Windows se paró porque falló un proceso esencial (Winlogon o CSRSS).", "Archivos de sistema dañados, una actualización a medias o un programa de seguridad que interfiere.", "Arranca en modo seguro y repara (SFC/DISM) o vuelve a un punto de restauración.", "repair"),
    (0xC000_000F, "Falta un archivo de arranque", "stop", "No se encuentra un archivo necesario para arrancar.", "Datos de arranque (BCD) dañados.", "Repara el arranque desde Discos → Particiones o con un USB de instalación.", "space"),
    (0xC000_0225, "Error en los datos de arranque", "stop", "El equipo no encuentra el sistema para arrancar.", "BCD dañado o partición EFI borrada.", "Repara el arranque.", "space"),
    (0xC000_0005, "Infracción de acceso", "windows", "Un programa intentó usar memoria que no era suya y se cerró.", "Un fallo del programa, un complemento o memoria RAM defectuosa.", "Actualiza o reinstala el programa; si pasa con muchos, prueba la memoria.", "hardware"),
    (0xC000_0142, "La aplicación no se inició", "windows", "Un programa no pudo arrancar.", "Una DLL dañada, falta de recursos del escritorio o .NET/Visual C++ dañados.", "Reinstala el programa y los Visual C++ Redistributables.", "install"),
    (0xC000_007B, "Imagen no válida", "windows", "Un programa intenta cargar una DLL de la arquitectura equivocada (32/64 bits) o dañada.", "Visual C++ o DirectX dañados o mezclados.", "Reinstala los Visual C++ Redistributables (x86 y x64).", "install"),
    // ---- Pantallazos (STOP) ----
    (0x0000_0133, "DPC_WATCHDOG_VIOLATION", "stop", "Un driver tardó demasiado en responder.", "Driver de almacenamiento (SSD) o de red antiguo; firmware del SSD.", "Actualiza el driver del controlador de almacenamiento y el firmware del SSD.", "boots"),
    (0x0000_00D1, "DRIVER_IRQL_NOT_LESS_OR_EQUAL", "stop", "Un driver accedió a memoria que no debía.", "Driver defectuoso, casi siempre de red, Wi-Fi o antivirus.", "Mira qué driver señala el volcado y actualízalo o vuelve al anterior.", "boots"),
    (0x0000_000A, "IRQL_NOT_LESS_OR_EQUAL", "stop", "Un driver o el sistema accedió a memoria con un nivel de prioridad no permitido.", "Driver defectuoso o memoria RAM con fallos.", "Revisa el driver señalado; si cambia cada vez, prueba la memoria.", "boots"),
    (0x0000_001E, "KMODE_EXCEPTION_NOT_HANDLED", "stop", "Un driver provocó un error que nadie atendió.", "Driver incompatible o defectuoso.", "Actualiza o desinstala el driver señalado.", "boots"),
    (0x0000_003B, "SYSTEM_SERVICE_EXCEPTION", "stop", "Fallo al pasar del modo usuario al del sistema.", "Driver de gráfica, antivirus o archivos de sistema dañados.", "Actualiza la gráfica, revisa el antivirus y repara el sistema.", "boots"),
    (0x0000_0050, "PAGE_FAULT_IN_NONPAGED_AREA", "stop", "Se pidió memoria que no existía.", "RAM defectuosa, driver dañado o disco con errores.", "Prueba la memoria y revisa el driver señalado y el disco.", "boots"),
    (0x0000_007E, "SYSTEM_THREAD_EXCEPTION_NOT_HANDLED", "stop", "Un hilo del sistema falló sin control.", "Driver incompatible, a menudo tras actualizar Windows.", "Vuelve al driver anterior del señalado o actualízalo.", "boots"),
    (0x0000_00EF, "CRITICAL_PROCESS_DIED", "stop", "Murió un proceso esencial de Windows.", "Archivos de sistema dañados o disco con errores.", "Repara el sistema (SFC/DISM) y revisa el disco.", "repair"),
    (0x0000_007A, "KERNEL_DATA_INPAGE_ERROR", "stop", "No se pudo leer de disco una parte de la memoria.", "Disco que falla, cable SATA o archivo de paginación dañado.", "Revisa la salud del disco y su cable.", "space"),
    (0x0000_0124, "WHEA_UNCORRECTABLE_ERROR", "stop", "El propio hardware informó de un error grave.", "Calor, overclock, fuente de alimentación o procesador/RAM con fallos.", "Comprueba temperaturas, quita overclocks y prueba la memoria.", "hardware"),
    (0x0000_0116, "VIDEO_TDR_FAILURE", "stop", "El driver de la gráfica no se recuperó tras colgarse.", "Driver de la gráfica, calor o gráfica defectuosa.", "Instala el driver de la gráfica limpio y vigila la temperatura.", "boots"),
    (0x0000_009F, "DRIVER_POWER_STATE_FAILURE", "stop", "Un driver no supo entrar o salir del modo de ahorro.", "Driver de red, USB o gráfica al suspender o hibernar.", "Actualiza el driver señalado y desactiva el ahorro de energía del dispositivo.", "boots"),
    (0x0000_00C2, "BAD_POOL_CALLER", "stop", "Un driver usó mal la memoria del sistema.", "Driver defectuoso, antivirus antiguo.", "Actualiza o quita el driver señalado.", "boots"),
    (0x0000_0019, "BAD_POOL_HEADER", "stop", "La memoria del sistema quedó dañada.", "Driver defectuoso o RAM con fallos.", "Revisa el driver y prueba la memoria.", "boots"),
    (0x0000_001A, "MEMORY_MANAGEMENT", "stop", "Error grave en la gestión de la memoria.", "RAM defectuosa o driver dañado.", "Prueba la memoria (Diagnóstico de memoria de Windows).", "hardware"),
    (0x0000_00F4, "CRITICAL_OBJECT_TERMINATION", "stop", "Se cerró un proceso o hilo crítico.", "Disco que falla o cable defectuoso.", "Revisa la salud del disco y su conexión.", "space"),
    (0x0000_007F, "UNEXPECTED_KERNEL_MODE_TRAP", "stop", "El procesador detectó un error que el sistema no esperaba.", "Hardware (RAM, calor) o driver.", "Comprueba temperaturas y prueba la memoria.", "hardware"),
    (0x0000_00BE, "ATTEMPTED_WRITE_TO_READONLY_MEMORY", "stop", "Un driver intentó escribir en memoria de solo lectura.", "Driver defectuoso.", "Actualiza o quita el driver señalado.", "boots"),
    (0x0000_0139, "KERNEL_SECURITY_CHECK_FAILURE", "stop", "El sistema detectó datos dañados en una estructura crítica.", "Driver incompatible o RAM con fallos.", "Actualiza drivers y prueba la memoria.", "boots"),
    (0x0000_007B, "INACCESSIBLE_BOOT_DEVICE", "stop", "Windows no puede leer el disco desde el que arranca.", "Cambio del modo SATA en la BIOS (AHCI/RAID), driver de almacenamiento o disco dañado.", "Revisa el modo SATA en la BIOS y la salud del disco; si fue tras actualizar, vuelve atrás desde el arranque avanzado.", "space"),
    (0x0000_0154, "UNEXPECTED_STORE_EXCEPTION", "stop", "Error en el almacén de memoria comprimida.", "Disco o SSD con problemas, o el antivirus.", "Revisa la salud del disco y actualiza el firmware del SSD.", "space"),
    // ---- Office y activación ----
    (0xC004_F074, "No se encontró el servidor de activación (KMS)", "activation", "Windows u Office no encuentran el servidor de licencias de la empresa.", "Fuera de la red de la empresa, DNS que no resuelve el servidor KMS o una activación no oficial.", "Conéctate a la red de la empresa o usa una licencia OEM/retail.", ""),
    (0xC004_C003, "Clave bloqueada", "activation", "El servidor de activación rechazó la clave.", "Clave usada en demasiados equipos o no válida.", "Usa una clave válida para esa edición.", ""),
    (0xC004_F213, "No se encontró licencia para este hardware", "activation", "Windows no encuentra una licencia ligada a este equipo.", "Cambio de placa base o reinstalación con otra edición.", "Usa el solucionador de activación o una clave para esta edición.", ""),
    (0x8007_232B, "No se encuentra el servidor de activación por DNS", "activation", "La activación por volumen no encuentra su servidor.", "Clave genérica de volumen sin servidor KMS al que llegar.", "Introduce la clave correcta (Configuración → Activación).", ""),
    (0x8007_0426, "Servicio de licencias no iniciado", "activation", "El servicio de licencias de Windows no está en marcha.", "Servicio deshabilitado por un «optimizador».", "Vuelve a poner el servicio de protección de software en Automático.", "services"),
];

/// Errores con nombre propio en Ctrl+K («DPC_WATCHDOG_VIOLATION»).
fn by_name(q: &str) -> Option<&'static Row> {
    let q = q.trim().to_ascii_uppercase().replace(' ', "_");
    CODES.iter().find(|r| r.1.eq_ignore_ascii_case(&q))
}

/// Interpreta lo escrito como código: «0x80070005», «80070005», «-2147024891», «STOP 0x133», «0x0000007B».
pub fn parse_code(q: &str) -> Option<u32> {
    let t = q.trim().to_ascii_lowercase();
    let t = t.trim_start_matches("stop").trim_start_matches(':').trim();
    if let Some(h) = t.strip_prefix("0x") {
        return u32::from_str_radix(h.trim_end_matches(|c: char| !c.is_ascii_hexdigit()), 16).ok();
    }
    if let Some(neg) = t.strip_prefix('-') {
        return neg.parse::<i64>().ok().map(|n| (-n) as i32 as u32);
    }
    if t.len() == 8 && t.chars().all(|c| c.is_ascii_hexdigit()) {
        return u32::from_str_radix(t, 16).ok();
    }
    t.parse::<u64>().ok().filter(|n| *n > 0xFFFF).map(|n| n as u32)
}

fn info(r: &Row) -> ErrorInfo {
    let code = if r.0 <= 0xFFFF { format!("0x{:X}", r.0) } else { format!("0x{:08X}", r.0) };
    ErrorInfo { code, name: r.1.into(), area: r.2, what: r.3, why: r.4, todo: r.5, page: r.6 }
}

/// Explica un código, o None si no está en el diccionario.
pub fn explain(code: u32) -> Option<ErrorInfo> {
    CODES.iter().find(|r| r.0 == code).map(info).or_else(|| {
        // Un HRESULT de Win32 desconocido (0x8007xxxx): al menos se dice el error de Windows de dentro.
        (code & 0xFFFF_0000 == 0x8007_0000).then(|| {
            let win32 = code & 0xFFFF;
            ErrorInfo {
                code: format!("0x{code:08X}"),
                name: format!("Error de Windows {win32}"),
                area: "windows",
                what: "Un error de Windows envuelto en un código de 32 bits.",
                why: "El número de dentro es el error original de Windows.",
                todo: "Busca el error con su número en Ctrl+K o en la documentación de Microsoft.",
                page: "",
            }
        })
    })
}

/// Busca por código o por nombre (para Ctrl+K y los pantallazos).
pub fn lookup(query: &str) -> Vec<ErrorInfo> {
    if let Some(r) = by_name(query) {
        return vec![info(r)];
    }
    parse_code(query).and_then(explain).into_iter().collect()
}

#[tauri::command]
pub fn error_lookup(query: String) -> Vec<ErrorInfo> {
    lookup(&query)
}

/// Todo el diccionario (Ctrl+K lo carga una vez para buscar sin preguntar a cada tecla).
#[tauri::command]
pub fn error_codes() -> Vec<ErrorInfo> {
    CODES.iter().map(info).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entiende_los_codigos_como_se_escriban() {
        assert_eq!(parse_code("0x80070005"), Some(0x8007_0005));
        assert_eq!(parse_code("80070005"), Some(0x8007_0005));
        assert_eq!(parse_code("-2147024891"), Some(0x8007_0005));
        assert_eq!(parse_code("STOP 0x133"), Some(0x133));
        assert_eq!(parse_code("0x0000007B"), Some(0x7B));
        assert_eq!(parse_code("hola"), None);
    }

    #[test]
    fn explica_los_conocidos() {
        let e = lookup("0x800f081f");
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].area, "update");
        assert_eq!(e[0].code, "0x800F081F");
        assert_eq!(lookup("dpc watchdog violation")[0].code, "0x133");
        assert_eq!(lookup("0xc000021a")[0].area, "stop");
    }

    #[test]
    fn win32_desconocido_dice_algo() {
        let e = explain(0x8007_1234).unwrap();
        assert!(e.name.contains("4660"));
        assert!(explain(0x1234_5678).is_none());
    }

    #[test]
    fn sin_repetidos_y_con_texto() {
        for (i, a) in CODES.iter().enumerate() {
            assert!(!a.3.is_empty() && !a.4.is_empty() && !a.5.is_empty(), "{}", a.1);
            assert!(CODES.iter().skip(i + 1).all(|b| b.0 != a.0), "repetido 0x{:X}", a.0);
        }
    }
}
