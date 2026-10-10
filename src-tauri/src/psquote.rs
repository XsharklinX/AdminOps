//! Texto del usuario (o de lo que haya en el equipo: nombres de programas, rutas,
//! valores del registro) dentro de un script de PowerShell. Es la única puerta:
//! todo lo que se mete entre comillas simples pasa por aquí.
//!
//! PowerShell trata como comilla simple no solo `'` sino también `‘ ’ ‚ ‛`: un
//! nombre como `Bob’s App` cerraba la cadena a mitad y lo que seguía se ejecutaba
//! como código. Por eso cada una de esas comillas se duplica, y dentro de una
//! cadena entre comillas simples no se interpreta nada más (`$()`, `"`, `` ` ``,
//! saltos de línea…).

/// Las cinco comillas que PowerShell toma por «comilla simple».
pub const QUOTES: [char; 5] = ['\'', '\u{2018}', '\u{2019}', '\u{201A}', '\u{201B}'];

pub fn is_quote(c: char) -> bool {
    QUOTES.contains(&c)
}

/// El texto listo para ir dentro de comillas simples (sin ponerlas): se duplican
/// las comillas y se quitan los NUL.
pub fn ps_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    for c in value.chars().filter(|c| *c != '\0') {
        if is_quote(c) {
            out.push(c);
        }
        out.push(c);
    }
    out
}

/// Texto cualquiera como cadena literal de PowerShell, con sus comillas.
pub fn ps_literal(value: &str) -> String {
    format!("'{}'", ps_escape(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lo que PowerShell lee de una cadena entre comillas simples: la cadena
    /// termina en la primera comilla que no va duplicada. `None` si no termina
    /// exactamente al final (es decir, si se pudiera salir de ella).
    fn read_literal(lit: &str) -> Option<String> {
        let mut chars = lit.chars().peekable();
        if !is_quote(chars.next()?) {
            return None;
        }
        let mut out = String::new();
        loop {
            let c = chars.next()?;
            if is_quote(c) {
                match chars.peek() {
                    Some(n) if is_quote(*n) => out.push(chars.next().unwrap()),
                    // Una comilla suelta cierra la cadena: tiene que ser la última.
                    _ => return chars.next().is_none().then_some(out),
                }
            } else {
                out.push(c);
            }
        }
    }

    const HOSTILE: &[&str] = &[
        "",
        "plano",
        "O'Brien",
        "Bob’s App",
        "‘cerrada’",
        "‚baja‛",
        "'; calc.exe; '",
        "’; Remove-Item C:\\ -Recurse; ‘",
        "$(calc)",
        "`$(whoami)`",
        "\"; whoami; \"",
        "línea1\nlínea2\r\n'@",
        "'@\nWrite-Host hola\n@'",
        "nul\0dentro",
        "''''",
        "’’’’",
        "a'b’c‘d‚e‛f",
        "C:\\Users\\Ana\\Programas (x86)\\Mi ‘App’\\a.exe",
        "-NoProfile -Command calc",
        "{ calc }",
        "$env:USERNAME;$PSVersionTable",
        "💥 emoji 日本語 ñ",
    ];

    #[test]
    fn hostile_text_cannot_leave_its_quotes() {
        for s in HOSTILE {
            let lit = ps_literal(s);
            let expected: String = s.chars().filter(|c| *c != '\0').collect();
            assert_eq!(read_literal(&lit).as_deref(), Some(expected.as_str()), "se salió de las comillas con {s:?} → {lit}");
        }
    }

    /// Todas las combinaciones de comillas, otros símbolos y letras, hasta 5 caracteres.
    #[test]
    fn every_short_combination_round_trips() {
        let alphabet = ['\'', '’', '‘', '‚', '‛', '"', '`', '$', ';', '\n', 'a'];
        let mut count = 0;
        fn walk(prefix: &mut String, depth: usize, alphabet: &[char], count: &mut usize) {
            let lit = ps_literal(prefix);
            assert_eq!(read_literal(&lit).as_deref(), Some(prefix.as_str()), "falla con {prefix:?}");
            *count += 1;
            if depth == 0 {
                return;
            }
            for c in alphabet {
                prefix.push(*c);
                walk(prefix, depth - 1, alphabet, count);
                prefix.pop();
            }
        }
        walk(&mut String::new(), 5, &alphabet, &mut count);
        assert!(count > 100_000);
    }

    #[test]
    fn plain_text_is_untouched_and_the_old_escape_was_not_enough() {
        assert_eq!(ps_escape("hola"), "hola");
        assert_eq!(ps_literal("O'Brien"), "'O''Brien'");
        // Escapar solo la comilla ASCII (lo que se hacía antes) dejaba salir de la cadena.
        let old = format!("'{}'", "Bob’s'; calc; '".replace('\'', "''"));
        assert_eq!(read_literal(&old), None);
    }

    /// Nadie vuelve a escapar a mano: la única función que duplica comillas es esta.
    #[test]
    fn nobody_escapes_quotes_by_hand() {
        fn walk(dir: &std::path::Path, hits: &mut Vec<String>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, hits);
                } else if p.extension().is_some_and(|x| x == "rs") && p.file_name().is_some_and(|n| n != "psquote.rs") {
                    let text = std::fs::read_to_string(&p).unwrap();
                    for (i, line) in text.lines().enumerate() {
                        if line.contains("replace('\\'', \"''\")") || line.contains("replace(\"'\", \"''\")") {
                            hits.push(format!("{}:{}", p.display(), i + 1));
                        }
                    }
                }
            }
        }
        let mut hits = Vec::new();
        walk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut hits);
        assert!(hits.is_empty(), "escapan comillas a mano (usa crate::psquote::ps_escape): {hits:?}");
    }
}
