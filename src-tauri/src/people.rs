//! Personas: la ficha de alguien del dominio de la empresa.
//!
//! La mayoría de los tickets empiezan con una persona («a María no le entra el
//! correo»), no con un equipo. Aquí se busca por nombre, usuario, correo o
//! extensión, y se ve y se hace lo de todos los días sin abrir la consola de
//! Active Directory: si la cuenta está bloqueada o caducada, desbloquearla,
//! darle una contraseña temporal, y las contraseñas de su equipo (LAPS y
//! recuperación de BitLocker).
//!
//! **Con los permisos del propio técnico.** Se habla con el dominio por ADSI,
//! que viene con Windows (no hace falta instalar las herramientas RSAT), con la
//! cuenta que ha iniciado sesión: lo mismo que puede hacer en la consola de
//! Microsoft, y nada más. Si no tiene permiso para algo, el dominio lo rechaza y
//! se le dice.
//!
//! Lo que cambia una cuenta queda en el diario y lo bloquea el modo auditoría.
//! Consultar una contraseña LAPS o una clave de BitLocker no cambia nada, pero
//! también queda en el diario (qué equipo, nunca la contraseña).

use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::State;

// ---------- Entrada ----------

/// Escapa un valor para meterlo en un filtro LDAP (RFC 4515). Sin esto, un
/// nombre con paréntesis o un asterisco cambiaría la búsqueda.
pub fn ldap_escape(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for c in v.chars() {
        match c {
            '\\' => out.push_str("\\5c"),
            '*' => out.push_str("\\2a"),
            '(' => out.push_str("\\28"),
            ')' => out.push_str("\\29"),
            '\0' => out.push_str("\\00"),
            _ => out.push(c),
        }
    }
    out
}

/// Lo que se puede buscar: de 2 a 64 caracteres, sin controles.
fn clean_query(q: &str) -> Result<String, String> {
    let q = q.trim();
    if q.chars().count() < 2 {
        return Err("Escribe al menos dos letras del nombre, el usuario, el correo o la extensión.".into());
    }
    if q.chars().count() > 64 || q.chars().any(|c| c.is_control()) {
        return Err("La búsqueda no es válida.".into());
    }
    Ok(q.to_string())
}

/// Nombre de usuario del dominio (sAMAccountName): lo que se admite como identificador.
fn clean_sam(sam: &str) -> Result<String, String> {
    let s = sam.trim();
    let ok = !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_alphanumeric() || "._-$".contains(c));
    ok.then(|| s.to_string()).ok_or_else(|| "Ese usuario no es válido.".to_string())
}

/// Nombre de equipo (NetBIOS o el primer tramo del nombre completo).
fn clean_computer(name: &str) -> Result<String, String> {
    let n = name.trim().split('.').next().unwrap_or("").to_uppercase();
    let ok = !n.is_empty() && n.len() <= 15 && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    ok.then_some(n).ok_or_else(|| "Escribe el nombre del equipo (hasta 15 letras, números o guiones).".to_string())
}

/// ID de una clave de recuperación de BitLocker: lo que enseña la pantalla azul
/// del equipo (los 8 primeros caracteres bastan).
fn clean_key_id(id: &str) -> Result<String, String> {
    let k: String = id.trim().trim_matches(|c| c == '{' || c == '}').to_uppercase();
    let ok = k.len() >= 8 && k.len() <= 36 && k.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
    ok.then_some(k).ok_or_else(|| "El ID de la clave son los caracteres que enseña la pantalla de recuperación, por ejemplo 1A2B3C4D.".to_string())
}

// ---------- Scripts ----------

/// Cabecera común: el dominio y una función para pasar fechas del dominio
/// (FILETIME) a segundos Unix. 0 es «nunca» o «no se sabe».
const COMMON: &str = r#"
try { $rootDse = [adsi]'LDAP://RootDSE'; $base = "$($rootDse.defaultNamingContext)" } catch { $base = '' }
if (-not $base) { throw 'SIN_DOMINIO' }
function U($x) {
  try { $v = [int64]"$x" } catch { return 0 }
  if ($v -le 0 -or $v -ge 9223372036854775807) { return 0 }
  return [int64](([DateTime]::FromFileTimeUtc($v) - [DateTime]::new(1970, 1, 1, 0, 0, 0, 'Utc')).TotalSeconds)
}
function P($r, $n) { $v = $r.Properties[$n.ToLower()]; if ($v -and $v.Count) { return $v[0] } else { return $null } }
function Cn($dn) { if ($dn -match '^CN=((?:\\,|[^,])+)') { return ($matches[1] -replace '\\,', ',') } else { return "$dn" } }
"#;

const SEARCH_SCRIPT: &str = r#"
$s = New-Object DirectoryServices.DirectorySearcher([adsi]"LDAP://$base")
$s.Filter = "(&(objectCategory=person)(objectClass=user)(|(sAMAccountName=*$q*)(displayName=*$q*)(mail=*$q*)(telephoneNumber=*$q*)(ipPhone=*$q*)(mobile=*$q*)))"
$s.SizeLimit = 25
foreach ($p in 'sAMAccountName','displayName','department','title','mail','telephoneNumber','ipPhone','userAccountControl','lockoutTime') { [void]$s.PropertiesToLoad.Add($p) }
$r = @($s.FindAll() | ForEach-Object {
  $uac = [int](P $_ 'userAccountControl')
  [pscustomobject]@{
    sam = "$(P $_ 'sAMAccountName')"; name = "$(P $_ 'displayName')"
    department = "$(P $_ 'department')"; title = "$(P $_ 'title')"; mail = "$(P $_ 'mail')"
    phone = "$(P $_ 'telephoneNumber')"; extension = "$(P $_ 'ipPhone')"
    disabled = [bool]($uac -band 2); lockedHint = (U (P $_ 'lockoutTime')) -gt 0
  }
})
ConvertTo-Json -InputObject $r -Compress
"#;

const DETAILS_SCRIPT: &str = r#"
$s = New-Object DirectoryServices.DirectorySearcher([adsi]"LDAP://$base")
$s.Filter = "(&(objectCategory=person)(objectClass=user)(sAMAccountName=$sam))"
foreach ($p in 'sAMAccountName','userPrincipalName','displayName','department','title','mail','telephoneNumber','ipPhone','mobile','physicalDeliveryOfficeName','manager','memberOf','userAccountControl','pwdLastSet','lastLogonTimestamp','whenCreated') { [void]$s.PropertiesToLoad.Add($p) }
$u = $s.FindOne()
if (-not $u) { throw 'NO_EXISTE' }
# Los atributos calculados (bloqueo real y caducidad de la contraseña) solo se
# devuelven buscando sobre el propio objeto.
$b = New-Object DirectoryServices.DirectorySearcher([adsi]$u.Path)
$b.SearchScope = 'Base'; $b.Filter = '(objectClass=*)'
[void]$b.PropertiesToLoad.Add('msDS-User-Account-Control-Computed'); [void]$b.PropertiesToLoad.Add('msDS-UserPasswordExpiryTimeComputed')
$c = $b.FindOne()
$computed = [int64](P $c 'msDS-User-Account-Control-Computed')
$uac = [int](P $u 'userAccountControl')
[pscustomobject]@{
  sam = "$(P $u 'sAMAccountName')"; upn = "$(P $u 'userPrincipalName')"; name = "$(P $u 'displayName')"
  department = "$(P $u 'department')"; title = "$(P $u 'title')"; office = "$(P $u 'physicalDeliveryOfficeName')"
  mail = "$(P $u 'mail')"; phone = "$(P $u 'telephoneNumber')"; extension = "$(P $u 'ipPhone')"; mobile = "$(P $u 'mobile')"
  manager = (Cn "$(P $u 'manager')")
  groups = @($u.Properties['memberof'] | ForEach-Object { Cn "$_" } | Sort-Object)
  disabled = [bool]($uac -band 2)
  locked = [bool]($computed -band 0x10)
  passwordExpired = [bool]($computed -band 0x800000)
  passwordNeverExpires = [bool]($uac -band 0x10000)
  passwordLastSet = (U (P $u 'pwdLastSet'))
  passwordExpires = (U (P $c 'msDS-UserPasswordExpiryTimeComputed'))
  lastLogon = (U (P $u 'lastLogonTimestamp'))
} | ConvertTo-Json -Depth 3 -Compress
"#;

/// Busca el objeto del usuario y deja su ruta en `$path`.
const FIND_USER: &str = r#"
$s = New-Object DirectoryServices.DirectorySearcher([adsi]"LDAP://$base")
$s.Filter = "(&(objectCategory=person)(objectClass=user)(sAMAccountName=$sam))"
$u = $s.FindOne()
if (-not $u) { throw 'NO_EXISTE' }
$e = [adsi]$u.Path
"#;

const UNLOCK_SCRIPT: &str = r#"
$e.Properties['lockoutTime'].Value = 0
$e.CommitChanges()
'ok'
"#;

const RESET_SCRIPT: &str = r#"
$plain = [Runtime.InteropServices.Marshal]::PtrToStringBSTR([Runtime.InteropServices.Marshal]::SecureStringToBSTR($pw))
$e.Invoke('SetPassword', $plain)
$plain = $null
if ($mustChange) { $e.Properties['pwdLastSet'].Value = 0 }
if ($unlock) { $e.Properties['lockoutTime'].Value = 0 }
$e.CommitChanges()
'ok'
"#;

/// Busca el equipo en el dominio y deja su ruta en `$compPath`.
const FIND_COMPUTER: &str = r#"
$s = New-Object DirectoryServices.DirectorySearcher([adsi]"LDAP://$base")
$s.Filter = "(&(objectCategory=computer)(sAMAccountName=$computer`$))"
$comp = $s.FindOne()
if (-not $comp) { throw 'EQUIPO_NO_EXISTE' }
$compPath = $comp.Path
"#;

const LAPS_SCRIPT: &str = r#"
$out = [ordered]@{ found = $false; account = ''; password = ''; expires = 0; source = '' }
# Windows LAPS moderno: el cmdlet sabe descifrar la contraseña si va cifrada.
if (Get-Command Get-LapsADPassword -ErrorAction SilentlyContinue) {
  try {
    $l = Get-LapsADPassword -Identity $computer -AsPlainText -ErrorAction Stop
    if ($l -and $l.Password) {
      $out.found = $true; $out.account = "$($l.Account)"; $out.password = "$($l.Password)"; $out.source = 'Windows LAPS'
      if ($l.ExpirationTimestamp) { $out.expires = [int64]([DateTimeOffset]$l.ExpirationTimestamp).ToUnixTimeSeconds() }
    }
  } catch {}
}
if (-not $out.found) {
  $b = New-Object DirectoryServices.DirectorySearcher([adsi]$compPath)
  $b.SearchScope = 'Base'; $b.Filter = '(objectClass=*)'
  foreach ($p in 'msLAPS-Password','msLAPS-PasswordExpirationTime','ms-Mcs-AdmPwd','ms-Mcs-AdmPwdExpirationTime') { [void]$b.PropertiesToLoad.Add($p) }
  $r = $b.FindOne()
  $j = P $r 'msLAPS-Password'
  if ($j) {
    try { $o = "$j" | ConvertFrom-Json; $out.found = $true; $out.account = "$($o.n)"; $out.password = "$($o.p)"; $out.source = 'Windows LAPS' } catch {}
    $out.expires = (U (P $r 'msLAPS-PasswordExpirationTime'))
  } elseif (P $r 'ms-Mcs-AdmPwd') {
    $out.found = $true; $out.account = 'Administrador local'; $out.password = "$(P $r 'ms-Mcs-AdmPwd')"; $out.source = 'LAPS (versión anterior)'
    $out.expires = (U (P $r 'ms-Mcs-AdmPwdExpirationTime'))
  }
}
[pscustomobject]$out | ConvertTo-Json -Compress
"#;

const BITLOCKER_BY_COMPUTER: &str = r#"
$b = New-Object DirectoryServices.DirectorySearcher([adsi]$compPath)
$b.SearchScope = 'OneLevel'; $b.Filter = '(objectClass=msFVE-RecoveryInformation)'
foreach ($p in 'name','msFVE-RecoveryPassword','whenCreated') { [void]$b.PropertiesToLoad.Add($p) }
$r = @($b.FindAll() | ForEach-Object {
  [pscustomobject]@{ computer = $computer; name = "$(P $_ 'name')"; password = "$(P $_ 'msFVE-RecoveryPassword')"; created = [int64]([DateTimeOffset](P $_ 'whenCreated')).ToUnixTimeSeconds() }
})
ConvertTo-Json -InputObject $r -Compress
"#;

const BITLOCKER_BY_ID: &str = r#"
$s = New-Object DirectoryServices.DirectorySearcher([adsi]"LDAP://$base")
$s.Filter = "(&(objectClass=msFVE-RecoveryInformation)(name=*{$keyId*))"
$s.SizeLimit = 10
foreach ($p in 'name','msFVE-RecoveryPassword','whenCreated','distinguishedName') { [void]$s.PropertiesToLoad.Add($p) }
$r = @($s.FindAll() | ForEach-Object {
  $dn = "$(P $_ 'distinguishedName')"
  $parent = if ($dn -match '^CN=(?:\\,|[^,])+,CN=((?:\\,|[^,])+)') { $matches[1] } else { '' }
  [pscustomobject]@{ computer = $parent; name = "$(P $_ 'name')"; password = "$(P $_ 'msFVE-RecoveryPassword')"; created = [int64]([DateTimeOffset](P $_ 'whenCreated')).ToUnixTimeSeconds() }
})
ConvertTo-Json -InputObject $r -Compress
"#;

// ---------- Datos ----------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PersonHit {
    pub sam: String,
    pub name: String,
    pub department: String,
    pub title: String,
    pub mail: String,
    pub phone: String,
    pub extension: String,
    pub disabled: bool,
    /// Tuvo un bloqueo (puede haber caducado ya; el estado real está en la ficha).
    pub locked_hint: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Person {
    pub sam: String,
    /// Usuario de Microsoft 365 (normalmente el correo): el que usa Graph.
    pub upn: String,
    pub name: String,
    pub department: String,
    pub title: String,
    pub office: String,
    pub mail: String,
    pub phone: String,
    pub extension: String,
    pub mobile: String,
    pub manager: String,
    pub groups: Vec<String>,
    pub disabled: bool,
    pub locked: bool,
    pub password_expired: bool,
    pub password_never_expires: bool,
    /// Segundos Unix; 0 = no se sabe.
    pub password_last_set: u64,
    /// Segundos Unix; 0 = no caduca o no se sabe.
    pub password_expires: u64,
    /// Último inicio de sesión según el dominio (se actualiza cada ~2 semanas).
    pub last_logon: u64,
    /// Equipos donde tiene la sesión abierta, según la última comprobación de Puestos.
    #[serde(default)]
    pub signed_in_on: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LapsPassword {
    pub found: bool,
    pub account: String,
    pub password: String,
    pub expires: u64,
    pub source: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct RecoveryKey {
    pub computer: String,
    /// Nombre del objeto: fecha de creación y el ID de la clave entre llaves.
    pub name: String,
    pub password: String,
    pub created: u64,
    /// El ID de la clave, para compararlo con el de la pantalla de recuperación.
    #[serde(default)]
    pub key_id: String,
}

impl RecoveryKey {
    /// El ID de la clave (lo que enseña la pantalla de recuperación).
    pub fn key_id(&self) -> String {
        self.name.split('{').nth(1).and_then(|s| s.split('}').next()).unwrap_or("").to_uppercase()
    }
}

// ---------- Ejecución ----------

/// Traduce los errores del dominio a algo que el técnico pueda contar.
fn friendly(e: String) -> String {
    let l = e.to_lowercase();
    if e.contains("SIN_DOMINIO") {
        "Este equipo no está en un dominio, o no llega a él ahora (¿falta la VPN?). La ficha de la persona necesita el dominio de la empresa.".into()
    } else if e.contains("NO_EXISTE") && !e.contains("EQUIPO_NO_EXISTE") {
        "Esa persona ya no está en el dominio.".into()
    } else if e.contains("EQUIPO_NO_EXISTE") {
        "Ese equipo no está en el dominio. Revisa el nombre.".into()
    } else if l.contains("access is denied") || l.contains("acceso denegado") || l.contains("0x80070005") || l.contains("insufficient") {
        "El dominio no te deja hacer esto con tu cuenta: hace falta permiso de soporte sobre esa cuenta o equipo.".into()
    } else if l.contains("password does not meet") || l.contains("no cumple") || l.contains("0x800708c5") {
        "La contraseña no cumple las reglas del dominio (longitud, complejidad o historial). Prueba a generar otra.".into()
    } else if l.contains("server is not operational") || l.contains("servidor no está operativo") || l.contains("0x8007203a") {
        "No se llega a ningún controlador de dominio. Revisa la red o la VPN.".into()
    } else {
        e
    }
}

/// Consulta al dominio por el grupo de PowerShell. El registro solo ve `detail`.
fn ad(script: &str, detail: &str) -> Result<String, String> {
    crate::pspool::query(script, Some(Duration::from_secs(45)), detail).map_err(friendly)
}

fn parse<T: for<'de> Deserialize<'de> + Default>(out: &str) -> Result<T, String> {
    let t = out.trim();
    if t.is_empty() || t == "null" {
        return Ok(T::default());
    }
    serde_json::from_str(t).map_err(|e| format!("Respuesta inesperada del dominio: {e}"))
}

/// Contraseña temporal fácil de dictar por teléfono y que cumple la complejidad
/// habitual del dominio: `Norte-Pino-4827!`. Sin letras que se confundan al
/// leerlas (no hay «l» ni «0/O» en las palabras).
pub fn temp_password() -> String {
    use aes_gcm::aead::rand_core::RngCore;
    use aes_gcm::aead::OsRng;
    const WORDS: &[&str] = &[
        "Norte", "Pino", "Faro", "Monte", "Brisa", "Cielo", "Roble", "Puerto", "Nube", "Trigo", "Coral", "Arena", "Cumbre", "Sierra", "Valle", "Rio", "Marea", "Tierra",
        "Bosque", "Aurora",
    ];
    const SYMBOLS: &[char] = &['!', '#', '%', '+'];
    let r = || OsRng.next_u32() as usize;
    let a = WORDS[r() % WORDS.len()];
    let mut b = WORDS[r() % WORDS.len()];
    while b == a {
        b = WORDS[r() % WORDS.len()];
    }
    format!("{a}-{b}-{:04}{}", 1000 + r() % 9000, SYMBOLS[r() % SYMBOLS.len()])
}

// ---------- Comandos ----------

/// Busca personas en el dominio por nombre, usuario, correo o extensión.
#[tauri::command(async)]
pub fn search_people(query: String) -> Result<Vec<PersonHit>, String> {
    let q = clean_query(&query)?;
    let script = format!("{}{COMMON}{SEARCH_SCRIPT}", crate::ps::text_var("q", &ldap_escape(&q)));
    let mut hits: Vec<PersonHit> = parse(&ad(&script, "Personas: buscar en el dominio")?)?;
    hits.retain(|h| !h.sam.is_empty());
    hits.sort_by_key(|h| (h.disabled, h.name.to_lowercase()));
    Ok(hits)
}

/// La ficha de una persona.
#[tauri::command(async)]
pub fn person_details(sam: String) -> Result<Person, String> {
    let sam = clean_sam(&sam)?;
    let script = format!("{}{COMMON}{DETAILS_SCRIPT}", crate::ps::text_var("sam", &ldap_escape(&sam)));
    let mut p: Person = parse(&ad(&script, &format!("Personas: ficha de {sam}"))?)?;
    p.signed_in_on = crate::stations::signed_in_on(&sam);
    Ok(p)
}

/// Desbloquea la cuenta del dominio.
#[tauri::command(async)]
pub fn unlock_account(sam: String, tweaks: State<'_, TweakState>) -> Result<(), String> {
    let sam = clean_sam(&sam)?;
    let script = format!("{}{COMMON}{FIND_USER}{UNLOCK_SCRIPT}", crate::ps::text_var("sam", &ldap_escape(&sam)));
    let r = ad(&script, &format!("Personas: desbloquear {sam}")).map(|_| ());
    tweaks.record(Op::Run, &format!("Dominio: desbloquear la cuenta de {sam}"), &r);
    r
}

/// Da a la cuenta una contraseña temporal nueva y la devuelve para dictarla.
/// Por defecto obliga a cambiarla al entrar y desbloquea la cuenta a la vez,
/// que es casi siempre lo que se quiere cuando alguien la ha olvidado.
#[tauri::command(async)]
pub fn reset_domain_password(sam: String, must_change: bool, unlock: bool, tweaks: State<'_, TweakState>) -> Result<String, String> {
    let sam = clean_sam(&sam)?;
    let password = temp_password();
    let script = format!(
        "{}{}$mustChange = ${must_change}\n$unlock = ${unlock}\n{COMMON}{FIND_USER}{RESET_SCRIPT}",
        crate::ps::text_var("sam", &ldap_escape(&sam)),
        crate::ps::secret_var("pw", &password),
    );
    let r = ad(&script, &format!("Personas: restablecer la contraseña de {sam}")).map(|_| ());
    // El diario nunca guarda la contraseña: solo que se cambió.
    tweaks.record(Op::Run, &format!("Dominio: contraseña temporal para {sam}{}", if must_change { " (cambio obligatorio)" } else { "" }), &r);
    r.map(|()| password)
}

/// La contraseña LAPS del administrador local de un equipo.
#[tauri::command(async)]
pub fn laps_password(computer: String, tweaks: State<'_, TweakState>) -> Result<LapsPassword, String> {
    let computer = clean_computer(&computer)?;
    let script = format!("{}{COMMON}{FIND_COMPUTER}{LAPS_SCRIPT}", crate::ps::text_var("computer", &ldap_escape(&computer)));
    let r: Result<LapsPassword, String> = ad(&script, &format!("Personas: LAPS de {computer}")).and_then(|o| parse(&o));
    // Se apunta la consulta (no la contraseña), la encuentre o no.
    tweaks.record(Op::Run, &format!("Dominio: consultada la contraseña LAPS de {computer}"), &r.as_ref().map(|_| ()).map_err(Clone::clone));
    let l = r?;
    if !l.found {
        return Err(format!("{computer} no tiene contraseña LAPS guardada en el dominio, o tu cuenta no tiene permiso para verla."));
    }
    Ok(l)
}

/// Claves de recuperación de BitLocker guardadas en el dominio: las de un
/// equipo, o la que corresponde al ID que enseña la pantalla de recuperación.
#[tauri::command(async)]
pub fn bitlocker_recovery(computer: Option<String>, key_id: Option<String>, tweaks: State<'_, TweakState>) -> Result<Vec<RecoveryKey>, String> {
    let (script, what) = match (computer.filter(|c| !c.trim().is_empty()), key_id.filter(|k| !k.trim().is_empty())) {
        (_, Some(id)) => {
            let id = clean_key_id(&id)?;
            (format!("{}{COMMON}{BITLOCKER_BY_ID}", crate::ps::text_var("keyId", &ldap_escape(&id))), format!("la clave con ID {id}"))
        }
        (Some(c), None) => {
            let c = clean_computer(&c)?;
            (format!("{}{COMMON}{FIND_COMPUTER}{BITLOCKER_BY_COMPUTER}", crate::ps::text_var("computer", &ldap_escape(&c))), format!("las claves de {c}"))
        }
        (None, None) => return Err("Escribe el nombre del equipo o el ID de la clave.".into()),
    };
    let r: Result<Vec<RecoveryKey>, String> = ad(&script, "Personas: claves de BitLocker").and_then(|o| parse(&o));
    tweaks.record(Op::Run, &format!("Dominio: consultadas {what} de BitLocker"), &r.as_ref().map(|_| ()).map_err(Clone::clone));
    let mut keys = r?;
    keys.retain(|k| !k.password.is_empty());
    for k in &mut keys {
        k.key_id = k.key_id();
    }
    keys.sort_by_key(|k| std::cmp::Reverse(k.created));
    if keys.is_empty() {
        return Err(format!("No hay {what} en el dominio, o tu cuenta no tiene permiso para verlas."));
    }
    Ok(keys)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lo que escribe el técnico nunca cambia el sentido de la búsqueda.
    #[test]
    fn ldap_values_are_escaped() {
        assert_eq!(ldap_escape("maria.perez"), "maria.perez");
        assert_eq!(ldap_escape("*)(uid=*"), "\\2a\\29\\28uid=\\2a");
        assert_eq!(ldap_escape("a\\b"), "a\\5cb");
        assert_eq!(ldap_escape("Pérez (Conta)"), "Pérez \\28Conta\\29");
    }

    #[test]
    fn inputs_are_validated() {
        assert!(clean_query("m").is_err());
        assert_eq!(clean_query("  maría  ").unwrap(), "maría");
        assert!(clean_sam("maria.perez").is_ok());
        assert!(clean_sam("maria perez").is_err());
        assert!(clean_sam("x' ; Remove-Item").is_err());
        assert_eq!(clean_computer("pc-conta-03.empresa.local").unwrap(), "PC-CONTA-03");
        assert!(clean_computer("NOMBRE-DEMASIADO-LARGO-PARA-NETBIOS").is_err());
        assert_eq!(clean_key_id("{1a2b3c4d}").unwrap(), "1A2B3C4D");
        assert!(clean_key_id("123").is_err());
        assert!(clean_key_id("ZZZZZZZZ").is_err());
    }

    /// La temporal se puede dictar y cumple la complejidad del dominio:
    /// mayúsculas, minúsculas, números y un símbolo, y al menos 12 caracteres.
    #[test]
    fn temporary_passwords_are_strong_and_readable() {
        for _ in 0..200 {
            let p = temp_password();
            assert!(p.len() >= 12, "{p}");
            assert!(p.chars().any(|c| c.is_ascii_uppercase()), "{p}");
            assert!(p.chars().any(|c| c.is_ascii_lowercase()), "{p}");
            assert!(p.chars().any(|c| c.is_ascii_digit()), "{p}");
            assert!(p.chars().any(|c| "!#%+".contains(c)), "{p}");
            let partes: Vec<&str> = p.split('-').collect();
            assert_eq!(partes.len(), 3, "{p}");
            assert_ne!(partes[0], partes[1], "las dos palabras son distintas: {p}");
        }
    }

    #[test]
    fn recovery_key_id_comes_from_the_object_name() {
        let k = RecoveryKey { name: "2024-01-15T10:22:33-05:00{1a2b3c4d-1111-2222-3333-444455556666}".into(), ..Default::default() };
        assert_eq!(k.key_id(), "1A2B3C4D-1111-2222-3333-444455556666");
    }

    #[test]
    fn domain_errors_are_explained() {
        assert!(friendly("SIN_DOMINIO".into()).contains("no está en un dominio"));
        assert!(friendly("Exception: NO_EXISTE".into()).contains("ya no está"));
        assert!(friendly("EQUIPO_NO_EXISTE".into()).contains("equipo no está"));
        assert!(friendly("Access is denied. (0x80070005)".into()).contains("no te deja"));
        assert!(friendly("The server is not operational".into()).contains("controlador de dominio"));
    }

    /// Las respuestas vacías del dominio no son un error: es que no hay nada.
    #[test]
    fn empty_answers_parse_to_defaults() {
        let v: Vec<PersonHit> = parse("").unwrap();
        assert!(v.is_empty());
        let p: Person = parse(r#"{"sam":"maria.perez","locked":true,"groups":["Contabilidad"],"passwordExpires":1760000000}"#).unwrap();
        assert!(p.locked && p.groups == vec!["Contabilidad"] && p.password_expires == 1_760_000_000);
    }

    #[test]
    fn embedded_scripts_parse() {
        let scripts = [
            ("COMMON", COMMON),
            ("SEARCH", SEARCH_SCRIPT),
            ("DETAILS", DETAILS_SCRIPT),
            ("FIND_USER", FIND_USER),
            ("UNLOCK", UNLOCK_SCRIPT),
            ("RESET", RESET_SCRIPT),
            ("FIND_COMPUTER", FIND_COMPUTER),
            ("LAPS", LAPS_SCRIPT),
            ("BITLOCKER_COMPUTER", BITLOCKER_BY_COMPUTER),
            ("BITLOCKER_ID", BITLOCKER_BY_ID),
        ];
        for (name, s) in scripts {
            let errors = crate::ps::parse_errors(s);
            assert!(errors.is_empty(), "{name}: {errors}");
        }
    }
}
