//! Edicao de celulas de um notebook Jupyter (.ipynb, nbformat 4).
//!
//! Trabalha no JSON cru e mexe so na celula alvo: o resto do arquivo (metadata,
//! kernelspec, outras celulas) sai como entrou. Celula de codigo alterada perde
//! as saidas e o execution_count, porque ja nao correspondem ao codigo novo.

use anyhow::{anyhow, Result};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditMode {
    Replace,
    Insert,
    Delete,
}

impl EditMode {
    pub fn parse(raw: &str) -> Result<Self> {
        match raw {
            "replace" => Ok(Self::Replace),
            "insert" => Ok(Self::Insert),
            "delete" => Ok(Self::Delete),
            other => Err(anyhow!("edit_mode invalido '{other}' (use replace, insert ou delete)")),
        }
    }
}

pub struct NotebookEdit<'a> {
    pub mode: EditMode,
    /// Celula alvo (replace/delete) ou posicao onde a nova celula entra (insert;
    /// ausente = no fim).
    pub cell_index: Option<usize>,
    /// Alternativa a `cell_index` pra replace/delete: id da celula (nbformat 4.5).
    pub cell_id: Option<&'a str>,
    /// "code" ou "markdown".
    pub cell_type: &'a str,
    pub source: &'a str,
}

/// Aplica a edicao ao texto de um .ipynb. Devolve o novo texto e um resumo
/// curto pro modelo.
pub fn apply(raw: &str, edit: &NotebookEdit) -> Result<(String, String)> {
    let mut nb: Value =
        serde_json::from_str(raw).map_err(|e| anyhow!("ipynb invalido: {e}"))?;
    let cells = nb
        .get_mut("cells")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow!("notebook sem a lista 'cells'"))?;

    if !matches!(edit.cell_type, "code" | "markdown") {
        return Err(anyhow!("cell_type deve ser 'code' ou 'markdown'"));
    }

    let summary = match edit.mode {
        EditMode::Insert => {
            let pos = edit.cell_index.unwrap_or(cells.len());
            if pos > cells.len() {
                return Err(anyhow!(
                    "cell_index {pos} fora do intervalo (o notebook tem {} celulas)",
                    cells.len()
                ));
            }
            cells.insert(pos, new_cell(edit.cell_type, edit.source));
            format!("celula inserida na posicao {pos} ({})", edit.cell_type)
        }
        EditMode::Replace => {
            let idx = locate(cells, edit)?;
            let cell = &mut cells[idx];
            cell["source"] = source_lines(edit.source);
            cell["cell_type"] = json!(edit.cell_type);
            if edit.cell_type == "code" {
                cell["outputs"] = json!([]);
                cell["execution_count"] = Value::Null;
            } else if let Some(obj) = cell.as_object_mut() {
                obj.remove("outputs");
                obj.remove("execution_count");
            }
            format!("celula {idx} substituida ({})", edit.cell_type)
        }
        EditMode::Delete => {
            let idx = locate(cells, edit)?;
            cells.remove(idx);
            format!("celula {idx} removida")
        }
    };

    Ok((serialize_nb(&nb)?, summary))
}

fn locate(cells: &[Value], edit: &NotebookEdit) -> Result<usize> {
    if let Some(id) = edit.cell_id {
        return cells
            .iter()
            .position(|c| c["id"].as_str() == Some(id))
            .ok_or_else(|| anyhow!("nenhuma celula com id '{id}'"));
    }
    let idx = edit
        .cell_index
        .ok_or_else(|| anyhow!("informe cell_index ou cell_id para {:?}", edit.mode))?;
    if idx >= cells.len() {
        return Err(anyhow!(
            "cell_index {idx} fora do intervalo (o notebook tem {} celulas)",
            cells.len()
        ));
    }
    Ok(idx)
}

fn new_cell(cell_type: &str, source: &str) -> Value {
    let id = uuid::Uuid::new_v4().simple().to_string()[..8].to_string();
    if cell_type == "code" {
        json!({
            "cell_type": "code",
            "id": id,
            "metadata": {},
            "source": source_lines(source),
            "outputs": [],
            "execution_count": null
        })
    } else {
        json!({
            "cell_type": "markdown",
            "id": id,
            "metadata": {},
            "source": source_lines(source)
        })
    }
}

/// nbformat guarda `source` como lista de linhas, cada uma terminando em `\n`
/// exceto a ultima.
fn source_lines(text: &str) -> Value {
    Value::Array(
        text.split_inclusive('\n')
            .map(|line| Value::String(line.to_string()))
            .collect(),
    )
}

/// Indentacao de 1 espaco, como o Jupyter grava.
fn serialize_nb(nb: &Value) -> Result<String> {
    let mut buf = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b" ");
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    nb.serialize(&mut ser)?;
    let mut text = String::from_utf8(buf)?;
    text.push('\n');
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NB: &str = r##"{
 "cells": [
  {"cell_type": "markdown", "id": "aaa111", "metadata": {}, "source": ["# Titulo"]},
  {"cell_type": "code", "id": "bbb222", "metadata": {}, "source": ["x = 1\n", "print(x)"],
   "outputs": [{"output_type": "stream"}], "execution_count": 3}
 ],
 "metadata": {"kernelspec": {"name": "python3"}},
 "nbformat": 4,
 "nbformat_minor": 5
}"##;

    fn edit<'a>(mode: EditMode, idx: Option<usize>, id: Option<&'a str>, src: &'a str, ct: &'a str) -> NotebookEdit<'a> {
        NotebookEdit { mode, cell_index: idx, cell_id: id, cell_type: ct, source: src }
    }

    #[test]
    fn replace_code_cell_by_id_clears_outputs_and_keeps_metadata() {
        let (out, _) = apply(NB, &edit(EditMode::Replace, None, Some("bbb222"), "y = 2\n", "code")).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["cells"][1]["source"], json!(["y = 2\n"]));
        assert_eq!(v["cells"][1]["outputs"], json!([]));
        assert_eq!(v["cells"][1]["execution_count"], Value::Null);
        assert_eq!(v["metadata"]["kernelspec"]["name"], "python3");
        assert_eq!(v["nbformat"], 4);
    }

    #[test]
    fn insert_markdown_at_index_zero() {
        let (out, _) = apply(NB, &edit(EditMode::Insert, Some(0), None, "Intro", "markdown")).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["cells"].as_array().unwrap().len(), 3);
        assert_eq!(v["cells"][0]["source"], json!(["Intro"]));
        assert!(v["cells"][0].get("outputs").is_none());
    }

    #[test]
    fn delete_by_index_removes_cell() {
        let (out, summary) = apply(NB, &edit(EditMode::Delete, Some(0), None, "", "code")).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["cells"].as_array().unwrap().len(), 1);
        assert_eq!(v["cells"][0]["id"], "bbb222");
        assert!(summary.contains("removida"));
    }

    #[test]
    fn out_of_range_and_unknown_id_are_errors() {
        assert!(apply(NB, &edit(EditMode::Replace, Some(9), None, "x", "code")).is_err());
        assert!(apply(NB, &edit(EditMode::Delete, None, Some("nope"), "", "code")).is_err());
        assert!(apply(NB, &edit(EditMode::Insert, Some(5), None, "x", "code")).is_err());
        assert!(apply("{}", &edit(EditMode::Insert, None, None, "x", "code")).is_err());
    }

    #[test]
    fn source_lines_keep_newlines_like_nbformat() {
        assert_eq!(source_lines("a\nb"), json!(["a\n", "b"]));
        assert_eq!(source_lines(""), json!([]));
    }
}
