//! Tapar datos personales en un recorte de pantalla, con el OCR de Windows.
//!
//! El recorte de Windows deja la imagen en el portapapeles. Aquí se lee, se
//! reconoce el texto con `Windows.Media.Ocr` (el que trae Windows, sin nube y
//! sin descargar nada), y se tapan con un rectángulo las palabras que llevan
//! el nombre de usuario, la carpeta del perfil o el nombre del equipo (las
//! rutas `C:\Users\…` enteras). La imagen tapada vuelve al portapapeles, lista
//! para pegar en el ticket.
//!
//! El portapapeles se lee y se escribe con Win32 (funciona desde cualquier
//! hilo); WinRT solo se usa para el OCR.

use serde::Serialize;

/// Una imagen en memoria: BGRA, de arriba abajo, 4 bytes por píxel.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

/// Rectángulo en píxeles de la imagen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

// ---------- DIB (el formato de imagen del portapapeles) ----------

/// Lee un DIB (BITMAPINFOHEADER + píxeles) de 24 o 32 bits sin comprimir.
pub fn dib_to_image(dib: &[u8]) -> Option<Image> {
    let u32_at = |o: usize| dib.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let header = u32_at(0)? as usize;
    let width = i32::from_le_bytes(dib.get(4..8)?.try_into().ok()?);
    let height = i32::from_le_bytes(dib.get(8..12)?.try_into().ok()?);
    let bits = u16::from_le_bytes(dib.get(14..16)?.try_into().ok()?);
    let compression = u32_at(16)?;
    let colors = u32_at(32)? as usize;
    if width <= 0 || height == 0 || width > 16_384 || height.unsigned_abs() > 16_384 || !(bits == 24 || bits == 32) {
        return None;
    }
    // BI_RGB (0) o BI_BITFIELDS (3), que en 32 bits es el orden BGRA de siempre.
    if compression != 0 && compression != 3 {
        return None;
    }
    let masks = if compression == 3 && header == 40 { 12 } else { 0 };
    let offset = header + masks + colors * 4;
    let (w, h) = (width as usize, height.unsigned_abs() as usize);
    let bpp = usize::from(bits / 8);
    let stride = (w * bpp + 3) & !3;
    let data = dib.get(offset..offset + stride * h)?;
    let bottom_up = height > 0;
    let mut pixels = vec![0u8; w * h * 4];
    for row in 0..h {
        let src = &data[(if bottom_up { h - 1 - row } else { row }) * stride..][..w * bpp];
        let dst = &mut pixels[row * w * 4..][..w * 4];
        for x in 0..w {
            dst[x * 4..x * 4 + 3].copy_from_slice(&src[x * bpp..x * bpp + 3]);
            dst[x * 4 + 3] = 255;
        }
    }
    Some(Image { width: w, height: h, pixels })
}

/// Escribe un DIB de 32 bits de abajo arriba (el que entiende todo el mundo).
pub fn image_to_dib(img: &Image) -> Vec<u8> {
    let mut out = Vec::with_capacity(40 + img.pixels.len());
    out.extend(40u32.to_le_bytes());
    out.extend((img.width as i32).to_le_bytes());
    out.extend((img.height as i32).to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(32u16.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend((img.pixels.len() as u32).to_le_bytes());
    out.extend([0u8; 16]);
    for row in (0..img.height).rev() {
        out.extend_from_slice(&img.pixels[row * img.width * 4..][..img.width * 4]);
    }
    out
}

// ---------- Qué se tapa ----------

/// Palabras que delatan a la persona o al equipo en este Windows.
pub fn sensitive_terms() -> Vec<String> {
    let mut v: Vec<String> = ["USERNAME", "COMPUTERNAME", "USERDOMAIN"].iter().filter_map(|k| std::env::var(k).ok()).collect();
    if let Some(profile) = std::env::var_os("USERPROFILE").and_then(|p| std::path::Path::new(&p).file_name().map(|n| n.to_string_lossy().into_owned())) {
        v.push(profile);
    }
    let mut v: Vec<String> = v.into_iter().map(|s| s.to_lowercase()).filter(|s| s.chars().count() >= 3).collect();
    // «WORKGROUP» o el propio nombre de Windows no son datos de nadie.
    v.retain(|s| !["workgroup", "grupo_trabajo", "windows", "administrador", "administrator", "user", "usuario"].contains(&s.as_str()));
    v.sort();
    v.dedup();
    v
}

/// ¿Hay que tapar esta palabra? Rutas de perfil enteras, y cualquier palabra
/// que contenga el usuario, la carpeta del perfil o el nombre del equipo.
pub fn is_sensitive(word: &str, terms: &[String]) -> bool {
    let w = word.to_lowercase();
    let profile_path = ["\\users\\", "/users/", "\\usuarios\\", "\\documents and settings\\"].iter().any(|p| w.contains(p));
    profile_path || terms.iter().any(|t| w.contains(t.as_str()))
}

/// Pinta un rectángulo opaco (con un poco de margen) sobre cada área.
pub fn cover(img: &mut Image, areas: &[Area]) {
    const PAD: f32 = 2.0;
    for a in areas {
        let x0 = (a.x - PAD).max(0.0) as usize;
        let y0 = (a.y - PAD).max(0.0) as usize;
        let x1 = ((a.x + a.w + PAD).ceil() as usize).min(img.width);
        let y1 = ((a.y + a.h + PAD).ceil() as usize).min(img.height);
        for y in y0..y1 {
            for x in x0..x1 {
                let p = (y * img.width + x) * 4;
                img.pixels[p..p + 4].copy_from_slice(&[0x2A, 0x24, 0x22, 0xFF]);
            }
        }
    }
}

/// Reduce la imagen a la mitad (o más) hasta que quepa en `max` píxeles por
/// lado, que es lo que admite el OCR. Devuelve la imagen y el factor.
pub fn fit(img: &Image, max: usize) -> (Image, usize) {
    let mut f = 1;
    while img.width / f > max || img.height / f > max {
        f += 1;
    }
    if f == 1 {
        return (img.clone(), 1);
    }
    let (w, h) = (img.width / f, img.height / f);
    let mut pixels = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let s = ((y * f) * img.width + x * f) * 4;
            pixels[(y * w + x) * 4..][..4].copy_from_slice(&img.pixels[s..s + 4]);
        }
    }
    (Image { width: w, height: h, pixels }, f)
}

// ---------- OCR de Windows ----------

/// Palabras reconocidas con su rectángulo.
#[cfg(windows)]
fn recognize(img: &Image) -> Result<Vec<(String, Area)>, String> {
    use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::DataWriter;

    let fail = |e: windows::core::Error| format!("El OCR de Windows falló: {}", e.message());
    let engine = OcrEngine::TryCreateFromUserProfileLanguages().map_err(|_| "Windows no tiene instalado el reconocimiento de texto de ningún idioma de este usuario (Configuración → Hora e idioma → Idioma → Opciones del idioma).".to_string())?;
    let max = OcrEngine::MaxImageDimension().unwrap_or(2600) as usize;
    let (small, f) = fit(img, max);
    let writer = DataWriter::new().map_err(fail)?;
    writer.WriteBytes(&small.pixels).map_err(fail)?;
    let buffer = writer.DetachBuffer().map_err(fail)?;
    let bitmap = SoftwareBitmap::CreateCopyFromBuffer(&buffer, BitmapPixelFormat::Bgra8, small.width as i32, small.height as i32).map_err(fail)?;
    let result = engine.RecognizeAsync(&bitmap).map_err(fail)?.get().map_err(fail)?;
    let mut out = Vec::new();
    let lines = result.Lines().map_err(fail)?;
    for i in 0..lines.Size().map_err(fail)? {
        let words = lines.GetAt(i).and_then(|l| l.Words()).map_err(fail)?;
        for j in 0..words.Size().map_err(fail)? {
            let w = words.GetAt(j).map_err(fail)?;
            let r = w.BoundingRect().map_err(fail)?;
            let k = f as f32;
            out.push((w.Text().map_err(fail)?.to_string(), Area { x: r.X * k, y: r.Y * k, w: r.Width * k, h: r.Height * k }));
        }
    }
    Ok(out)
}

// ---------- Portapapeles ----------

const CF_DIB: u32 = 8;

#[cfg(windows)]
fn read_clipboard_dib() -> Option<Vec<u8>> {
    use windows_sys::Win32::System::DataExchange::{CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard};
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
    // SAFETY: se abre el portapapeles, se copia el bloque mientras está bloqueado y se cierra.
    unsafe {
        if IsClipboardFormatAvailable(CF_DIB) == 0 || OpenClipboard(std::ptr::null_mut()) == 0 {
            return None;
        }
        let h = GetClipboardData(CF_DIB);
        let out = if h.is_null() {
            None
        } else {
            let p = GlobalLock(h) as *const u8;
            let n = GlobalSize(h);
            let v = (!p.is_null()).then(|| std::slice::from_raw_parts(p, n).to_vec());
            GlobalUnlock(h);
            v
        };
        CloseClipboard();
        out
    }
}

#[cfg(windows)]
fn write_clipboard_dib(dib: &[u8]) -> Result<(), String> {
    use windows_sys::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    // SAFETY: el bloque se reserva con el tamaño exacto, se copia y se entrega
    // al portapapeles (que pasa a ser su dueño); si no lo acepta, se libera.
    unsafe {
        let h = GlobalAlloc(GMEM_MOVEABLE, dib.len());
        if h.is_null() {
            return Err("Sin memoria para el recorte.".into());
        }
        let p = GlobalLock(h) as *mut u8;
        if p.is_null() {
            GlobalFree(h);
            return Err("Sin memoria para el recorte.".into());
        }
        std::ptr::copy_nonoverlapping(dib.as_ptr(), p, dib.len());
        GlobalUnlock(h);
        // Otro programa puede tenerlo abierto un instante: se reintenta.
        let mut opened = false;
        for _ in 0..10 {
            if OpenClipboard(std::ptr::null_mut()) != 0 {
                opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        if !opened {
            GlobalFree(h);
            return Err("El portapapeles está ocupado por otro programa.".into());
        }
        EmptyClipboard();
        let ok = !SetClipboardData(CF_DIB, h).is_null();
        CloseClipboard();
        if !ok {
            GlobalFree(h);
            return Err("Windows no aceptó la imagen en el portapapeles.".into());
        }
        Ok(())
    }
}

#[cfg(windows)]
pub fn clipboard_sequence() -> u32 {
    // SAFETY: sin argumentos; solo lee un contador.
    unsafe { windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber() }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Redacted {
    /// Palabras tapadas.
    pub covered: usize,
    /// Palabras leídas en total (0: no había texto o no se pudo leer).
    pub words: usize,
    /// Si no se pudo, por qué (el recorte queda como estaba).
    pub error: String,
}

pub const NO_IMAGE: &str = "En el portapapeles no hay una imagen.";

/// Tapa los datos personales del recorte que hay en el portapapeles.
#[cfg(windows)]
pub fn redact_clipboard() -> Redacted {
    let fallo = |e: String| Redacted { covered: 0, words: 0, error: e };
    let Some(dib) = read_clipboard_dib() else { return fallo(NO_IMAGE.into()) };
    let Some(mut img) = dib_to_image(&dib) else { return fallo("La imagen del portapapeles tiene un formato que no se puede leer.".into()) };
    // WinRT en un hilo propio (multihilo): así no depende del hilo que llama.
    let copy = img.clone();
    let words = match std::thread::spawn(move || {
        // SAFETY: inicializa WinRT para este hilo, que acaba aquí mismo.
        let _ = unsafe { windows::Win32::System::WinRT::RoInitialize(windows::Win32::System::WinRT::RO_INIT_MULTITHREADED) };
        recognize(&copy)
    })
    .join()
    {
        Ok(Ok(w)) => w,
        Ok(Err(e)) => return fallo(e),
        Err(_) => return fallo("El OCR de Windows se detuvo inesperadamente.".into()),
    };
    let terms = sensitive_terms();
    let areas: Vec<Area> = words.iter().filter(|(t, _)| is_sensitive(t, &terms)).map(|(_, a)| *a).collect();
    if areas.is_empty() {
        return Redacted { covered: 0, words: words.len(), error: String::new() };
    }
    cover(&mut img, &areas);
    match write_clipboard_dib(&image_to_dib(&img)) {
        Ok(()) => Redacted { covered: areas.len(), words: words.len(), error: String::new() },
        Err(e) => fallo(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(w: usize, h: usize) -> Image {
        let mut pixels = Vec::new();
        for y in 0..h {
            for x in 0..w {
                pixels.extend([x as u8, y as u8, 7, 255]);
            }
        }
        Image { width: w, height: h, pixels }
    }

    /// Ida y vuelta por el formato del portapapeles, sin perder ni girar nada.
    #[test]
    fn dib_round_trip() {
        let a = img(5, 3);
        let dib = image_to_dib(&a);
        assert_eq!(dib.len(), 40 + 5 * 3 * 4);
        assert_eq!(dib_to_image(&dib).unwrap(), a);
    }

    /// 24 bits de arriba abajo, con relleno al final de cada fila.
    #[test]
    fn reads_24_bit_top_down() {
        let mut dib = Vec::new();
        dib.extend(40u32.to_le_bytes());
        dib.extend(2i32.to_le_bytes());
        dib.extend((-2i32).to_le_bytes());
        dib.extend(1u16.to_le_bytes());
        dib.extend(24u16.to_le_bytes());
        dib.extend([0u8; 24]);
        // Fila de 2 píxeles = 6 bytes + 2 de relleno.
        dib.extend([1, 2, 3, 4, 5, 6, 0, 0, 7, 8, 9, 10, 11, 12, 0, 0]);
        let i = dib_to_image(&dib).unwrap();
        assert_eq!((i.width, i.height), (2, 2));
        assert_eq!(&i.pixels[..8], &[1, 2, 3, 255, 4, 5, 6, 255]);
        assert_eq!(&i.pixels[8..], &[7, 8, 9, 255, 10, 11, 12, 255]);
        // Basura o formatos que no se leen: None, sin reventar.
        assert!(dib_to_image(&[0; 10]).is_none());
        let mut rle = dib.clone();
        rle[16] = 1;
        assert!(dib_to_image(&rle).is_none());
    }

    #[test]
    fn what_gets_covered() {
        let terms = vec!["otaku".to_string(), "desktop-7k2".to_string()];
        assert!(is_sensitive(r"C:\Users\otaku\Desktop", &terms));
        assert!(is_sensitive(r"C:\Users\maria\AppData", &terms), "cualquier ruta de perfil");
        assert!(is_sensitive("OTAKU", &terms));
        assert!(is_sensitive("DESKTOP-7K2\\admin", &terms));
        assert!(!is_sensitive("Impresora", &terms));
        assert!(!is_sensitive(r"C:\Windows\System32", &terms));
    }

    #[test]
    fn covering_stays_inside_the_image() {
        let mut i = img(10, 10);
        cover(&mut i, &[Area { x: 8.0, y: 8.0, w: 20.0, h: 20.0 }, Area { x: 0.0, y: 0.0, w: 1.0, h: 1.0 }]);
        assert_eq!(&i.pixels[(9 * 10 + 9) * 4..][..4], &[0x2A, 0x24, 0x22, 0xFF]);
        assert_eq!(&i.pixels[..4], &[0x2A, 0x24, 0x22, 0xFF]);
        assert_eq!(&i.pixels[(5 * 10 + 5) * 4..][..4], &[5, 5, 7, 255], "lo de en medio no se toca");
    }

    /// El OCR de Windows de verdad, con una imagen en blanco (no toca el portapapeles).
    #[cfg(windows)]
    #[test]
    #[ignore = "usa el OCR real de Windows"]
    fn windows_ocr_runs() {
        let blank = Image { width: 200, height: 60, pixels: vec![255; 200 * 60 * 4] };
        let words = std::thread::spawn(move || {
            let _ = unsafe { windows::Win32::System::WinRT::RoInitialize(windows::Win32::System::WinRT::RO_INIT_MULTITHREADED) };
            recognize(&blank)
        })
        .join()
        .unwrap()
        .unwrap();
        assert!(words.is_empty());
        println!("términos que se taparían aquí: {}", sensitive_terms().len());
    }

    #[test]
    fn big_images_are_scaled_for_ocr() {
        let (s, f) = fit(&img(100, 40), 30);
        assert_eq!((s.width, s.height, f), (25, 10, 4));
        assert_eq!(fit(&img(10, 10), 30).1, 1);
    }
}
