//! Documentos de escritório além do que `attachments` lê direto: `.odt`
//! (LibreOffice texto), `.doc` (convertido pelo LibreOffice, se estiver
//! instalado), imagens embutidas em docx/odt/pdf (pra mandar ao modelo) e
//! edição de texto de um `.docx` existente.
//!
//! A edição mexe só em `word/document.xml` e regrava o zip copiando as outras
//! partes byte a byte — assim imagens, estilos e cabeçalhos ficam como estavam.
use anyhow::{anyhow, Result};
use std::collections::HashSet;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Teto de imagens por documento: um manual com centenas de figuras não deve
/// lotar o contexto do modelo.
pub const MAX_EMBEDDED_IMAGES: usize = 12;
/// Abaixo disso é ícone ou marcador de lista, não figura que ajude o modelo.
const MIN_IMAGE_BYTES: usize = 2 * 1024;

pub struct EmbeddedImage {
    pub mime: &'static str,
    pub bytes: Vec<u8>,
}

pub fn extract_odt_text(path: &Path) -> Result<String> {
    let xml = read_zip_entry_string(path, "content.xml")?;
    Ok(odt_xml_to_text(&xml))
}

pub fn extract_doc_text(path: &Path) -> Result<String> {
    with_converted_docx(path, crate::attachments::extract_text)
}

/// Imagens embutidas do documento, na ordem em que aparecem no arquivo.
pub fn embedded_images(path: &Path) -> Result<Vec<EmbeddedImage>> {
    let ext = extension_lower(path);
    let images = match ext.as_str() {
        "docx" => zip_images(path, "word/media/")?,
        "odt" => zip_images(path, "Pictures/")?,
        "pdf" => pdf_images(path)?,
        "doc" => with_converted_docx(path, |docx| zip_images(docx, "word/media/"))?,
        _ => Vec::new(),
    };
    Ok(images)
}

/// Troca `old` por `new` no `word/document.xml` de um `.docx`. Só funciona
/// quando o trecho aparece exatamente uma vez no documento inteiro (mesmo
/// critério do `edit_file`), e dentro de um único parágrafo.
pub fn edit_docx_text(path: &Path, old: &str, new: &str) -> Result<usize> {
    if old.is_empty() {
        return Err(anyhow!("old_str vazio"));
    }
    let mut entries = read_zip_entries(path)?;
    let (_, data) = entries
        .iter_mut()
        .find(|(name, _)| name == "word/document.xml")
        .ok_or_else(|| anyhow!("word/document.xml não encontrado — o arquivo não parece um .docx válido"))?;
    let mut xml = String::from_utf8(std::mem::take(data))
        .map_err(|_| anyhow!("word/document.xml não está em UTF-8"))?;

    match count_matches(&xml, old) {
        0 => return Err(anyhow!("trecho não encontrado no documento (o texto precisa estar num único parágrafo)")),
        1 => {}
        n => return Err(anyhow!("trecho aparece {n} vezes no documento — use um trecho único")),
    }
    apply_once(&mut xml, old, new);

    let (_, data) = entries
        .iter_mut()
        .find(|(name, _)| name == "word/document.xml")
        .ok_or_else(|| anyhow!("word/document.xml sumiu durante a edição"))?;
    *data = xml.into_bytes();
    write_zip_entries(path, &entries)?;
    Ok(1)
}

fn extension_lower(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase()
}

fn mime_for(name: &str) -> Option<&'static str> {
    match extension_lower(Path::new(name)).as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

// ---------- ODT ----------

/// Texto do `content.xml` do ODT: parágrafos e títulos viram quebras de linha,
/// `text:s`/`text:tab`/`text:line-break` viram espaço, tabulação e quebra.
fn odt_xml_to_text(xml: &str) -> String {
    let body = xml.find("<office:body").map_or(xml, |i| &xml[i..]);
    let mut out = String::new();
    let mut i = 0;
    while i < body.len() {
        let lt = match body[i..].find('<') {
            Some(rel) => i + rel,
            None => {
                out.push_str(&unescape(&body[i..]));
                break;
            }
        };
        out.push_str(&unescape(&body[i..lt]));
        let Some(gt_rel) = body[lt..].find('>') else { break };
        let gt = lt + gt_rel;
        let tag = &body[lt + 1..gt];
        let fechamento = tag.starts_with('/');
        let nome = tag
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or("");
        match (fechamento, nome) {
            (true, "text:p") | (true, "text:h") => out.push('\n'),
            (false, "text:p") | (false, "text:h") if tag.ends_with('/') => out.push('\n'),
            (false, "text:s") => {
                let n = attr_number(tag, "text:c").unwrap_or(1);
                out.push_str(&" ".repeat(n));
            }
            (false, "text:tab") => out.push('\t'),
            (false, "text:line-break") => out.push('\n'),
            _ => {}
        }
        i = gt + 1;
    }
    out
}

fn attr_number(tag: &str, attr: &str) -> Option<usize> {
    let pat = format!("{attr}=\"");
    let inicio = tag.find(&pat)? + pat.len();
    let digitos: String = tag[inicio..].chars().take_while(|c| c.is_ascii_digit()).collect();
    digitos.parse().ok()
}

// ---------- LibreOffice (.doc) ----------

/// Converte `path` para `.docx` numa pasta temporária, roda `f` sobre o
/// `.docx` e apaga a pasta. Exige LibreOffice instalado (`soffice`).
fn with_converted_docx<T>(path: &Path, f: impl FnOnce(&Path) -> Result<T>) -> Result<T> {
    let soffice = libreoffice_binary()
        .ok_or_else(|| anyhow!(LIBREOFFICE_AUSENTE))?;
    let dir = std::env::temp_dir().join(format!("cerne-office-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir)?;
    let resultado = (|| {
        let saida = Command::new(&soffice)
            .args(["--headless", "--convert-to", "docx", "--outdir"])
            .arg(&dir)
            .arg(path)
            .output()
            .map_err(|e| anyhow!("não foi possível rodar o LibreOffice: {e}"))?;
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("documento");
        let convertido = dir.join(format!("{stem}.docx"));
        if !convertido.exists() {
            let erro = String::from_utf8_lossy(&saida.stderr);
            return Err(anyhow!("o LibreOffice não conseguiu converter o arquivo: {erro}"));
        }
        f(&convertido)
    })();
    std::fs::remove_dir_all(&dir).ok();
    resultado
}

/// Mensagem que vai pro modelo quando o LibreOffice falta: ele deve avisar o
/// usuário e pedir a instalação, não tentar contornar.
pub const LIBREOFFICE_AUSENTE: &str = "Este arquivo precisa do LibreOffice para ser lido, e ele não foi encontrado nesta máquina. Avise o usuário que é preciso instalar o LibreOffice (https://www.libreoffice.org/download) e peça para ele instalar; não tente outro caminho para ler este arquivo.";

/// Diz se o LibreOffice (`soffice`) está disponível — usado pela ferramenta
/// `check_libreoffice` pra o modelo verificar antes de prometer uma conversão.
pub fn libreoffice_disponivel() -> bool {
    libreoffice_binary().is_some()
}

fn libreoffice_binary() -> Option<PathBuf> {
    let conhecidos = [
        PathBuf::from(r"C:\Program Files\LibreOffice\program\soffice.exe"),
        PathBuf::from(r"C:\Program Files (x86)\LibreOffice\program\soffice.exe"),
        PathBuf::from("/usr/bin/soffice"),
        PathBuf::from("/Applications/LibreOffice.app/Contents/MacOS/soffice"),
    ];
    if let Some(p) = conhecidos.into_iter().find(|p| p.exists()) {
        return Some(p);
    }
    // Sem caminho conhecido: aceita se `soffice` estiver no PATH.
    Command::new("soffice")
        .arg("--version")
        .output()
        .ok()
        .map(|_| PathBuf::from("soffice"))
}

// ---------- Imagens ----------

fn zip_images(path: &Path, prefixo: &str) -> Result<Vec<EmbeddedImage>> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut candidatos: Vec<(String, usize)> = Vec::new();
    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        let name = entry.name().to_string();
        if name.starts_with(prefixo) && mime_for(&name).is_some() {
            candidatos.push((name, i));
        }
    }
    candidatos.sort();
    let mut out = Vec::new();
    for (name, i) in candidatos {
        if out.len() >= MAX_EMBEDDED_IMAGES {
            break;
        }
        let mut entry = archive.by_index(i)?;
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        if bytes.len() < MIN_IMAGE_BYTES {
            continue;
        }
        let mime = mime_for(&name).unwrap_or("image/png");
        out.push(EmbeddedImage { mime, bytes });
    }
    Ok(out)
}

fn pdf_images(path: &Path) -> Result<Vec<EmbeddedImage>> {
    let doc = lopdf::Document::load(path).map_err(|e| anyhow!("falha ao abrir PDF para ler imagens: {e}"))?;
    let mut vistos = HashSet::new();
    let mut out = Vec::new();
    for page_id in doc.get_pages().values() {
        let Ok(imagens) = doc.get_page_images(*page_id) else { continue };
        for img in imagens {
            if out.len() >= MAX_EMBEDDED_IMAGES {
                return Ok(out);
            }
            if !vistos.insert(img.id) {
                continue;
            }
            let filtros = img.filters.clone().unwrap_or_default();
            let bytes_e_mime = if filtros.iter().any(|f| f == "DCTDecode") {
                // JPEG já vem pronto no stream.
                Some((img.content.to_vec(), "image/jpeg"))
            } else if filtros.iter().all(|f| f == "FlateDecode") && img.bits_per_component == Some(8) {
                let cs = img.color_space.clone().unwrap_or_default();
                let raw = doc
                    .get_object(img.id)
                    .ok()
                    .and_then(|o| o.as_stream().ok())
                    .and_then(|s| s.decompressed_content().ok());
                raw.and_then(|raw| raw_to_png(img.width as u32, img.height as u32, &cs, raw))
                    .map(|png| (png, "image/png"))
            } else {
                None
            };
            let Some((bytes, mime)) = bytes_e_mime else { continue };
            if bytes.len() < MIN_IMAGE_BYTES {
                continue;
            }
            out.push(EmbeddedImage { mime, bytes });
        }
    }
    Ok(out)
}

/// Pixels crus (RGB ou tons de cinza, 8 bits) viram PNG.
fn raw_to_png(width: u32, height: u32, color_space: &str, raw: Vec<u8>) -> Option<Vec<u8>> {
    let imagem = match color_space {
        "DeviceRGB" => image::DynamicImage::ImageRgb8(image::RgbImage::from_raw(width, height, raw)?),
        "DeviceGray" => image::DynamicImage::ImageLuma8(image::GrayImage::from_raw(width, height, raw)?),
        _ => return None,
    };
    let mut out = Cursor::new(Vec::new());
    imagem.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

// ---------- Edição de .docx (XML de document.xml) ----------

/// Intervalo do conteúdo de um `<w:t>` no XML, já sem escapes.
struct Span {
    start: usize,
    end: usize,
    text: String,
}

fn count_matches(xml: &str, old: &str) -> usize {
    paragraph_ranges(xml)
        .into_iter()
        .map(|(s, e)| full_text(&text_spans(xml, s, e)).matches(old).count())
        .sum()
}

fn apply_once(xml: &mut String, old: &str, new: &str) {
    for (s, e) in paragraph_ranges(xml) {
        let spans = text_spans(xml, s, e);
        if full_text(&spans).contains(old) {
            apply_in_paragraph(xml, &spans, old, new);
            return;
        }
    }
}

fn apply_in_paragraph(xml: &mut String, spans: &[Span], old: &str, new: &str) {
    let full = full_text(spans);
    let Some(ms) = full.find(old) else { return };
    let me = ms + old.len();
    let mut novos: Vec<String> = spans.iter().map(|s| s.text.clone()).collect();
    let mut offset = 0;
    let mut primeiro = true;
    for (i, span) in spans.iter().enumerate() {
        let ss = offset;
        let se = offset + span.text.len();
        offset = se;
        let a = ms.max(ss);
        let b = me.min(se);
        if a < b {
            // O texto novo entra no primeiro trecho atingido; os outros perdem a parte casada.
            let troca = if primeiro { new } else { "" };
            primeiro = false;
            novos[i].replace_range(a - ss..b - ss, troca);
        }
    }
    for (i, span) in spans.iter().enumerate().rev() {
        if novos[i] != span.text {
            xml.replace_range(span.start..span.end, &escape(&novos[i]));
        }
    }
}

fn full_text(spans: &[Span]) -> String {
    spans.iter().map(|s| s.text.as_str()).collect()
}

fn paragraph_ranges(xml: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(start) = find_open(xml, pos, "w:p") {
        let Some(gt_rel) = xml[start..].find('>') else { break };
        let gt = start + gt_rel;
        if xml[..gt].ends_with('/') {
            pos = gt + 1;
            continue;
        }
        let Some(close_rel) = xml[gt..].find("</w:p>") else { break };
        let end = gt + close_rel + "</w:p>".len();
        out.push((start, end));
        pos = end;
    }
    out
}

fn text_spans(xml: &str, s: usize, e: usize) -> Vec<Span> {
    let mut out = Vec::new();
    let mut pos = s;
    while let Some(start) = find_open(&xml[..e], pos, "w:t") {
        let Some(gt_rel) = xml[start..e].find('>') else { break };
        let gt = start + gt_rel;
        if xml[..gt].ends_with('/') {
            pos = gt + 1;
            continue;
        }
        let inicio = gt + 1;
        let Some(close_rel) = xml[inicio..e].find("</w:t>") else { break };
        let fim = inicio + close_rel;
        out.push(Span {
            start: inicio,
            end: fim,
            text: unescape(&xml[inicio..fim]),
        });
        pos = fim + "</w:t>".len();
    }
    out
}

/// Acha `<nome` seguido de `>`, espaço ou `/` (assim `<w:t` não casa com `<w:tab`).
fn find_open(xml: &str, from: usize, nome: &str) -> Option<usize> {
    let pat = format!("<{nome}");
    let mut i = from;
    while let Some(rel) = xml[i..].find(&pat) {
        let at = i + rel;
        let proximo = xml[at + pat.len()..].chars().next();
        if matches!(proximo, Some('>') | Some(' ') | Some('/')) {
            return Some(at);
        }
        i = at + pat.len();
    }
    None
}

fn unescape(raw: &str) -> String {
    raw.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

// ---------- Zip ----------

fn read_zip_entry_string(path: &Path, name: &str) -> Result<String> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut entry = archive
        .by_name(name)
        .map_err(|_| anyhow!("{name} não encontrado dentro de {}", path.display()))?;
    let mut text = String::new();
    entry.read_to_string(&mut text)?;
    Ok(text)
}

fn read_zip_entries(path: &Path) -> Result<Vec<(String, Vec<u8>)>> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut out = Vec::with_capacity(archive.len());
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();
        let mut data = Vec::new();
        entry.read_to_end(&mut data)?;
        out.push((name, data));
    }
    Ok(out)
}

/// Regrava o zip inteiro num arquivo temporário e troca pelo original — se der
/// erro no meio, o `.docx` original continua intacto.
fn write_zip_entries(path: &Path, entries: &[(String, Vec<u8>)]) -> Result<()> {
    let tmp = path.with_file_name(format!(
        "{}.tmp-edicao",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("documento.docx")
    ));
    {
        let file = std::fs::File::create(&tmp)?;
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default();
        for (name, data) in entries {
            if name.ends_with('/') {
                zip.add_directory(name.as_str(), opts)?;
            } else {
                zip.start_file(name.as_str(), opts)?;
                zip.write_all(data)?;
            }
        }
        zip.finish()?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn scratch_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cerne-office-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn odt_texto_com_titulo_espacos_tab_e_entidades() {
        let xml = r#"<?xml version="1.0"?><office:document-content xmlns:office="o" xmlns:text="t"><office:body><office:text><text:h text:outline-level="1">Título</text:h><text:p>Olá <text:s text:c="2"/>mundo &amp; cia<text:tab/>fim</text:p><text:p/></office:text></office:body></office:document-content>"#;
        // "Olá " tem um espaço no próprio texto, mais os 2 de text:s.
        assert_eq!(odt_xml_to_text(xml), "Título\nOlá   mundo & cia\tfim\n\n");
    }

    #[test]
    fn odt_lido_de_zip_real() {
        let dir = scratch_dir();
        let path = dir.join("texto.odt");
        {
            let file = fs::File::create(&path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let opts = zip::write::SimpleFileOptions::default();
            zip.start_file("content.xml", opts).unwrap();
            zip.write_all(b"<office:body><office:text><text:p>Conteudo odt</text:p></office:text></office:body>").unwrap();
            zip.finish().unwrap();
        }
        assert!(extract_odt_text(&path).unwrap().contains("Conteudo odt"));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn edita_trecho_que_atravessa_runs_e_preserva_outras_partes() {
        let dir = scratch_dir();
        let path = dir.join("doc.docx");
        {
            use docx_rust::document::Paragraph;
            use docx_rust::Docx;
            let mut docx = Docx::default();
            // Dois runs: "Olá " e "mundo". O trecho "Olá mundo" atravessa os dois.
            docx.document.push(Paragraph::default().push_text("Olá ").push_text("mundo"));
            docx.write_file(&path).unwrap();
        }
        // Acrescenta uma parte extra pra provar que ela sobrevive à regravação.
        let mut entries = read_zip_entries(&path).unwrap();
        entries.push(("word/media/image1.png".to_string(), vec![1u8; 4096]));
        write_zip_entries(&path, &entries).unwrap();

        assert_eq!(edit_docx_text(&path, "Olá mundo", "Oi mundo").unwrap(), 1);

        let depois = read_zip_entries(&path).unwrap();
        assert!(depois.iter().any(|(n, d)| n == "word/media/image1.png" && d.len() == 4096));
        let texto = attachments_text(&path);
        assert!(texto.contains("Oi mundo"), "texto depois da edição: {texto}");
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn edicao_recusa_trecho_ambiguo_ou_ausente() {
        let dir = scratch_dir();
        let path = dir.join("doc.docx");
        {
            use docx_rust::document::Paragraph;
            use docx_rust::Docx;
            let mut docx = Docx::default();
            docx.document.push(Paragraph::default().push_text("A & B"));
            docx.document.push(Paragraph::default().push_text("A & B de novo"));
            docx.write_file(&path).unwrap();
        }
        assert!(edit_docx_text(&path, "A & B", "X").is_err(), "trecho repetido deveria ser recusado");
        assert!(edit_docx_text(&path, "não existe", "X").is_err());
        assert!(attachments_text(&path).contains("A & B"), "arquivo não deveria ter sido alterado");
        fs::remove_dir_all(dir).ok();
    }

    /// PNG de ruído 64x48 — grande o bastante pra passar do piso de 2 KB.
    fn png_de_teste() -> Vec<u8> {
        let img = image::RgbImage::from_fn(64, 48, |x, y| image::Rgb([(x * 3) as u8, (y * 5) as u8, ((x ^ y) & 0xff) as u8]));
        let mut out = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img).write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn docx_lista_imagem_de_word_media() {
        let dir = scratch_dir();
        let path = dir.join("com-imagem.docx");
        {
            use docx_rust::document::Paragraph;
            use docx_rust::Docx;
            let mut docx = Docx::default();
            docx.document.push(Paragraph::default().push_text("Texto com figura"));
            docx.write_file(&path).unwrap();
        }
        let mut entries = read_zip_entries(&path).unwrap();
        entries.push(("word/media/image1.png".to_string(), png_de_teste()));
        write_zip_entries(&path, &entries).unwrap();

        let imagens = embedded_images(&path).unwrap();
        assert_eq!(imagens.len(), 1);
        assert_eq!(imagens[0].mime, "image/png");
        assert_eq!(image::load_from_memory(&imagens[0].bytes).unwrap().width(), 64);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn pdf_lista_imagem_rgb_comprimida_como_png() {
        use lopdf::{dictionary, Document, Object, Stream};
        let dir = scratch_dir();
        let path = dir.join("com-imagem.pdf");
        let (w, h) = (64i64, 48i64);
        let pixels: Vec<u8> = (0..(w * h * 3) as usize).map(|i| (i % 251) as u8).collect();

        let mut doc = Document::with_version("1.5");
        let mut imagem = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => w,
                "Height" => h,
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8,
            },
            pixels,
        );
        imagem.compress().unwrap();
        let imagem_id = doc.add_object(imagem);
        let conteudo_id = doc.add_object(Stream::new(dictionary! {}, Vec::new()));
        let pages_id = doc.new_object_id();
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 200.into(), 200.into()],
            "Contents" => conteudo_id,
            "Resources" => dictionary! { "XObject" => dictionary! { "Im1" => imagem_id } },
        });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page_id.into()], "Count" => 1 }),
        );
        let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog_id);
        doc.save(&path).unwrap();

        let imagens = embedded_images(&path).unwrap();
        assert_eq!(imagens.len(), 1, "o PDF tem uma imagem");
        assert_eq!(imagens[0].mime, "image/png");
        let decodificada = image::load_from_memory(&imagens[0].bytes).unwrap();
        assert_eq!((decodificada.width(), decodificada.height()), (64, 48));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn edita_docx_com_imagem_sem_perder_a_imagem() {
        let dir = scratch_dir();
        let path = dir.join("doc-com-imagem.docx");
        {
            use docx_rust::document::Paragraph;
            use docx_rust::Docx;
            let mut docx = Docx::default();
            docx.document.push(Paragraph::default().push_text("Versão 1"));
            docx.write_file(&path).unwrap();
        }
        let mut entries = read_zip_entries(&path).unwrap();
        entries.push(("word/media/image1.png".to_string(), png_de_teste()));
        write_zip_entries(&path, &entries).unwrap();

        assert_eq!(edit_docx_text(&path, "Versão 1", "Versão 2").unwrap(), 1);

        assert_eq!(embedded_images(&path).unwrap().len(), 1, "a imagem tem que continuar");
        assert!(attachments_text(&path).contains("Versão 2"));
        fs::remove_dir_all(dir).ok();
    }

    fn attachments_text(path: &Path) -> String {
        crate::attachments::extract_text(path).unwrap()
    }
}
