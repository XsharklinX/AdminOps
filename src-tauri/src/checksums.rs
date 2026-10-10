//! Comprobar lo que se descarga: la huella SHA-256 del archivo tiene que ser la
//! que se publicó. Hay dos fuentes independientes: la que GitHub calcula al subir
//! cada archivo («digest») y el `SHA256SUMS.txt` que se publica junto a la versión.
//! Si hay las dos, tienen que decir lo mismo y coincidir con lo descargado.

/// ¿Es una huella SHA-256 válida (64 cifras hexadecimales)?
pub fn is_sha256(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// La huella de `file` en un `SHA256SUMS.txt` (formato `huella  archivo` o `huella *archivo`).
pub fn hash_for(sums: &str, file: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, name) = line.trim().split_once(char::is_whitespace)?;
        let name = name.trim().trim_start_matches('*');
        (name == file && is_sha256(hash)).then(|| hash.to_ascii_lowercase())
    })
}

/// La huella de un «digest» de GitHub (`sha256:…`).
pub fn from_digest(digest: &str) -> Option<String> {
    digest.strip_prefix("sha256:").filter(|h| is_sha256(h)).map(str::to_ascii_lowercase)
}

#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Coincide con todas las huellas publicadas (`sources`: cuántas había, 1 o 2).
    Verified { sources: usize },
    /// No coincide con alguna, o las publicadas no coinciden entre sí.
    Mismatch,
    /// No hay ninguna huella publicada con la que comparar.
    Unavailable,
}

pub fn verdict(actual: &str, digest: Option<&str>, sums: Option<&str>) -> Verdict {
    let published: Vec<String> = [digest.and_then(from_digest), sums.map(str::to_ascii_lowercase).filter(|h| is_sha256(h))].into_iter().flatten().collect();
    // Una huella publicada con mal formato no cuenta como «sin huella»: es sospechosa.
    let given = usize::from(digest.is_some()) + usize::from(sums.is_some());
    if published.len() != given {
        return Verdict::Mismatch;
    }
    if published.is_empty() {
        return Verdict::Unavailable;
    }
    let actual = actual.to_ascii_lowercase();
    if published.iter().all(|h| *h == actual) {
        Verdict::Verified { sources: published.len() }
    } else {
        Verdict::Mismatch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "9f2c1d4e5b6a79880123456789abcdef0123456789abcdef0123456789abe41a";
    const B: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    #[test]
    fn reads_the_published_list() {
        let sums = format!("{A}  AdminOps-2.0.0-Setup.exe\n{B} *AdminOps-2.0.0-portable.zip\n\nbasura\n{} otro.exe\n", &A[..10]);
        assert_eq!(hash_for(&sums, "AdminOps-2.0.0-Setup.exe").as_deref(), Some(A));
        assert_eq!(hash_for(&sums, "AdminOps-2.0.0-portable.zip").as_deref(), Some(B));
        assert_eq!(hash_for(&sums, "otro.exe"), None, "una huella corta no vale");
        assert_eq!(hash_for(&sums, "AdminOps-2.0.0-Setup"), None, "el nombre es exacto");
        assert_eq!(hash_for("", "x"), None);
        assert_eq!(hash_for(&sums.to_uppercase(), "AdminOps-2.0.0-Setup.exe"), None, "los nombres distinguen mayúsculas");
    }

    #[test]
    fn digest_must_be_sha256() {
        assert_eq!(from_digest(&format!("sha256:{}", A.to_uppercase())).as_deref(), Some(A));
        assert_eq!(from_digest("sha1:abc"), None);
        assert_eq!(from_digest("sha256:xyz"), None);
        assert_eq!(from_digest(A), None);
    }

    #[test]
    fn verdicts() {
        let d = format!("sha256:{A}");
        assert_eq!(verdict(A, Some(&d), Some(A)), Verdict::Verified { sources: 2 });
        assert_eq!(verdict(&A.to_uppercase(), Some(&d), None), Verdict::Verified { sources: 1 });
        assert_eq!(verdict(A, None, Some(A)), Verdict::Verified { sources: 1 });
        assert_eq!(verdict(A, None, None), Verdict::Unavailable);
        // Lo descargado no es lo publicado.
        assert_eq!(verdict(B, Some(&d), None), Verdict::Mismatch);
        assert_eq!(verdict(B, Some(&d), Some(A)), Verdict::Mismatch);
        // Las dos huellas publicadas no coinciden entre sí: algo no cuadra aunque una coincida.
        assert_eq!(verdict(A, Some(&d), Some(B)), Verdict::Mismatch);
        // Una huella publicada con mal formato es sospechosa, no «sin huella».
        assert_eq!(verdict(A, Some("sha256:corta"), None), Verdict::Mismatch);
        assert_eq!(verdict(A, None, Some("corta")), Verdict::Mismatch);
    }
}
