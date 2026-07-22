//! Extração de texto de anexos do composer (`+` na UI). Cobre só os formatos
//! que dão pra extrair texto de forma confiável com crate publicada de
//! verdade — imagem/áudio/vídeo ficam de fora por enquanto (dependem de
//! suporte multimodal real por provider, que ainda precisa ser pesquisado
//! caso a caso antes de prometer algo na UI que o provider/modelo configurado
//! não entrega).
use anyhow::{anyhow, Result};
use calamine::{open_workbook_auto, Data, Reader};
use std::path::Path;

/// Mesmo limite usado por `tools::read_file` — teto de sanidade contra um
/// anexo patologicamente grande (um dump de log de centenas de MB) travar a
/// UI, não um limite "normal" de uso. Documentos reais grandes (um PDF de
/// ~300 mil caracteres, por exemplo) precisam passar inteiros — é o próprio
/// `context_length` da sessão (e a compactação automática baseada nele,
/// ver `agent::maybe_compact`) que já cuida de não estourar o contexto do
/// modelo, então truncar aqui embaixo de propósito só cortava documento
/// real sem necessidade.
pub const MAX_ATTACHMENT_CHARS: usize = 500_000;

pub fn extract_text(path: &Path) -> Result<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let text = match ext.as_str() {
        "pdf" => pdf_extract::extract_text(path)
            .map_err(|e| anyhow!("falha ao extrair texto do PDF: {e}"))?,
        "docx" => extract_docx(path)?,
        "xlsx" | "xlsm" | "xls" | "ods" => extract_spreadsheet(path)?,
        _ => {
            let bytes = std::fs::read(path).map_err(|e| anyhow!("falha ao ler arquivo: {e}"))?;
            let (content, _) = crate::encoding::decode(&bytes);
            content
        }
    };
    Ok(truncate(&text))
}

pub fn truncate(text: &str) -> String {
    let total = text.chars().count();
    if total > MAX_ATTACHMENT_CHARS {
        let head: String = text.chars().take(MAX_ATTACHMENT_CHARS).collect();
        format!("{head}\n... [truncado, {total} caracteres no total]")
    } else {
        text.to_string()
    }
}

fn extract_spreadsheet(path: &Path) -> Result<String> {
    let mut workbook =
        open_workbook_auto(path).map_err(|e| anyhow!("falha ao abrir planilha: {e}"))?;
    let mut out = String::new();
    for sheet_name in workbook.sheet_names() {
        let range = workbook
            .worksheet_range(&sheet_name)
            .map_err(|e| anyhow!("falha ao ler aba '{sheet_name}': {e}"))?;
        out.push_str(&format!("## {sheet_name}\n"));
        for row in range.rows() {
            let cells: Vec<String> = row.iter().map(format_cell).collect();
            out.push_str(&cells.join("\t"));
            out.push('\n');
        }
        out.push('\n');
    }
    Ok(out)
}

fn format_cell(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        other => other.to_string(),
    }
}

fn extract_docx(path: &Path) -> Result<String> {
    use docx_rust::document::{
        BodyContent, Paragraph, ParagraphContent, Run, RunContent, TableCell, TableCellContent,
        TableRowContent,
    };
    use docx_rust::DocxFile;

    let docx_file = DocxFile::from_file(path).map_err(|e| anyhow!("falha ao abrir docx: {e}"))?;
    let docx = docx_file
        .parse()
        .map_err(|e| anyhow!("falha ao interpretar docx: {e}"))?;

    fn push_run(run: &Run, out: &mut String) {
        for content in &run.content {
            match content {
                RunContent::Text(t) => out.push_str(&t.text),
                RunContent::Break(_) | RunContent::CarriageReturn(_) => out.push('\n'),
                RunContent::Tab(_) => out.push('\t'),
                _ => {}
            }
        }
    }

    fn push_paragraph(p: &Paragraph, out: &mut String) {
        for content in &p.content {
            match content {
                ParagraphContent::Run(r) => push_run(r, out),
                ParagraphContent::Link(link) => {
                    if let Some(r) = &link.content {
                        push_run(r, out);
                    }
                }
                _ => {}
            }
        }
        out.push('\n');
    }

    fn push_table_cell(cell: &TableCell, out: &mut String) {
        for content in &cell.content {
            match content {
                TableCellContent::Paragraph(p) => push_paragraph(p, out),
            }
        }
    }

    fn push_body_content(content: &BodyContent, out: &mut String) {
        match content {
            BodyContent::Paragraph(p) => push_paragraph(p, out),
            BodyContent::Run(r) => push_run(r, out),
            BodyContent::TableCell(tc) => push_table_cell(tc, out),
            BodyContent::Table(t) => {
                for row in &t.rows {
                    for cell in &row.cells {
                        match cell {
                            TableRowContent::TableCell(tc) => push_table_cell(tc, out),
                            TableRowContent::SDT(sdt) => {
                                if let Some(sdt_content) = &sdt.content {
                                    for c in &sdt_content.content {
                                        push_body_content(c, out);
                                    }
                                }
                            }
                        }
                    }
                    out.push('\n');
                }
            }
            BodyContent::Sdt(sdt) => {
                if let Some(sdt_content) = &sdt.content {
                    for c in &sdt_content.content {
                        push_body_content(c, out);
                    }
                }
            }
            BodyContent::SectionProperty(_) => {}
        }
    }

    let mut out = String::new();
    for content in &docx.document.body.content {
        push_body_content(content, &mut out);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn scratch_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("cerne-attach-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Reproduz o bug real relatado: PDF de ~300 mil caracteres cortado em
    /// 20 mil. Aponta pro arquivo real do usuário — roda manual com
    /// `cargo test -- --ignored --nocapture extract_real_large_pdf`, não faz
    /// parte da suite default (arquivo não existe no CI/outras máquinas).
    #[test]
    #[ignore]
    fn extract_real_large_pdf_is_not_truncated() {
        let path = std::path::Path::new(r"D:\SCA_elton\Versão Final SCA 0707.pdf");
        let out = extract_text(path).unwrap();
        println!("total de caracteres extraidos: {}", out.chars().count());
        println!("primeiros 300 chars: {}", &out.chars().take(300).collect::<String>());
        println!(
            "ultimos 300 chars: {}",
            &out.chars().rev().take(300).collect::<String>().chars().rev().collect::<String>()
        );
        assert!(
            !out.contains("[truncado"),
            "documento real ainda esta sendo truncado - MAX_ATTACHMENT_CHARS baixo demais pro tamanho real"
        );
        assert!(out.chars().count() > 20_000, "extraiu menos texto do que o limite antigo - algo quebrou na extracao");
    }

    #[test]
    fn extract_text_reads_plain_text_files_directly() {
        let dir = scratch_dir();
        let path = dir.join("notes.md");
        fs::write(&path, "# Titulo\n\nConteudo em markdown.\n").unwrap();
        let out = extract_text(&path).unwrap();
        assert!(out.contains("Conteudo em markdown"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn extract_text_truncates_very_long_files() {
        let dir = scratch_dir();
        let path = dir.join("big.txt");
        let big = "a".repeat(MAX_ATTACHMENT_CHARS + 500);
        fs::write(&path, &big).unwrap();
        let out = extract_text(&path).unwrap();
        assert!(out.contains("truncado"));
        assert!(out.len() < big.len());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn extract_text_reads_docx_paragraphs_and_tables() {
        use docx_rust::document::{Paragraph, Table, TableCell, TableRow};
        use docx_rust::Docx;

        let dir = scratch_dir();
        let path = dir.join("doc.docx");

        let mut docx = Docx::default();
        docx.document
            .push(Paragraph::default().push_text("Paragrafo de teste"));
        let row = TableRow::default().push_cell(TableCell::paragraph(
            Paragraph::default().push_text("celula A"),
        ));
        docx.document.push(Table::default().push_row(row));
        docx.write_file(&path).unwrap();

        let out = extract_text(&path).unwrap();
        assert!(
            out.contains("Paragrafo de teste"),
            "esperava o paragrafo, recebeu: {out}"
        );
        assert!(
            out.contains("celula A"),
            "esperava o conteudo da tabela, recebeu: {out}"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn extract_text_reads_xlsx_cell_values() {
        let dir = scratch_dir();
        let path = dir.join("planilha.xlsx");
        write_minimal_xlsx(&path, "ola,mundo\n1,2\n");
        let out = extract_text(&path).unwrap();
        assert!(
            out.contains("ola"),
            "esperava conteudo da celula, recebeu: {out}"
        );
        fs::remove_dir_all(&dir).ok();
    }

    /// Gera um .xlsx minimo de verdade (via `rust_xlsxwriter` seria ideal, mas
    /// pra nao adicionar mais uma dependencia so pro teste, monta o xlsx
    /// manualmente com uma unica aba com os valores de `csv_like` - suficiente
    /// pra exercitar `open_workbook_auto`/`worksheet_range` de ponta a ponta).
    fn write_minimal_xlsx(path: &Path, csv_like: &str) {
        use std::io::Write;
        use zip::write::SimpleFileOptions;

        let file = fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts = SimpleFileOptions::default();

        zip.start_file("[Content_Types].xml", opts).unwrap();
        zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#).unwrap();

        zip.start_file("_rels/.rels", opts).unwrap();
        zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#).unwrap();

        zip.start_file("xl/_rels/workbook.xml.rels", opts).unwrap();
        zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#).unwrap();

        zip.start_file("xl/workbook.xml", opts).unwrap();
        zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheets><sheet name="Sheet1" sheetId="1" r:id="rId1" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"/></sheets></workbook>"#).unwrap();

        let mut sheet_xml = String::from(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>"#,
        );
        for (r, line) in csv_like.lines().enumerate() {
            sheet_xml.push_str(&format!(r#"<row r="{}">"#, r + 1));
            for (c, value) in line.split(',').enumerate() {
                let col = (b'A' + c as u8) as char;
                if value.parse::<f64>().is_ok() {
                    sheet_xml.push_str(&format!(r#"<c r="{col}{}"><v>{value}</v></c>"#, r + 1));
                } else {
                    sheet_xml.push_str(&format!(
                        r#"<c r="{col}{}" t="inlineStr"><is><t>{value}</t></is></c>"#,
                        r + 1
                    ));
                }
            }
            sheet_xml.push_str("</row>");
        }
        sheet_xml.push_str("</sheetData></worksheet>");

        zip.start_file("xl/worksheets/sheet1.xml", opts).unwrap();
        zip.write_all(sheet_xml.as_bytes()).unwrap();

        zip.finish().unwrap();
    }
}
