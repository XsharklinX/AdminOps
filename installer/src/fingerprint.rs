//! Huella de un adminops.exe, para saber si el instalado es el de este instalador.
//! Lo usan el programa y su `build.rs` (el mismo código en los dos lados).
//!
//! No vale la huella del archivo tal cual: al empaquetar, Tauri escribe dentro
//! del ejecutable con qué instalador va («…_VAR_NSS») y luego deja el de la
//! carpeta de compilación como estaba («…_VAR_UNK»). Son tres letras de
//! diferencia, y con ellas la comprobación fallaba siempre: el instalador decía
//! que no había podido actualizar cuando sí lo había hecho.

const MARK: &[u8] = b"__TAURI_BUNDLE_TYPE_VAR_";
/// Letras que siguen a la marca (NSS, MSI, UNK…).
const KIND_LEN: usize = 3;

pub fn fingerprint(bytes: &[u8]) -> String {
    use sha2::Digest;
    let mut data = bytes.to_vec();
    let mut from = 0;
    while let Some(pos) = data[from..].windows(MARK.len()).position(|w| w == MARK) {
        let at = from + pos + MARK.len();
        for b in data.iter_mut().skip(at).take(KIND_LEN) {
            *b = b'?';
        }
        from = at;
    }
    sha2::Sha256::digest(&data).iter().map(|x| format!("{x:02x}")).collect()
}
