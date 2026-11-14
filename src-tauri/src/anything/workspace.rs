//! Modo Anything, fase 06: a pasta `.anything` do projeto e o formato das fichas. Tudo que o modo
//! produz fica dentro do projeto do usuário. O código valida cada ficha; quem grava é o modelo, com a
//! ferramenta de escrita que o Cerne já tem, e a validação roda depois (fase 08).

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

const BASES: &[&str] = &["observado", "derivado", "inferido"];
const CONFIANCAS: &[&str] = &["alta", "media", "baixa"];
/// Limite do resumo de uma linha, que vai para o índice e para o prompt de quem chama.
pub const RESUMO_MAXIMO: usize = 160;
/// Campos que toda ficha precisa ter, nesta ordem (a ordem vira a ordem das mensagens de erro).
const OBRIGATORIOS: &[&str] = &["endereco", "nome_original", "nome_sugerido", "resumo", "o_que_faz", "base", "confianca"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Constante {
    pub valor: String,
    pub significado: String,
}

/// Ficha de uma função. `chama` e `chamada_por` vêm do motor (o código preenche, não o modelo).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Ficha {
    pub endereco: String,
    pub nome_original: String,
    pub nome_sugerido: String,
    pub resumo: String,
    pub o_que_faz: String,
    #[serde(default)]
    pub constantes: Vec<Constante>,
    #[serde(default)]
    pub chama: Vec<String>,
    #[serde(default)]
    pub chamada_por: Vec<String>,
    pub base: String,
    pub confianca: String,
    #[serde(default)]
    pub evidencias: Vec<String>,
    #[serde(default)]
    pub pendencias: Vec<String>,
}

/// Uma linha do índice (`indice.json`): o que um item de síntese ou de outra função lê sem abrir a ficha.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinhaIndice {
    pub endereco: String,
    pub nome_sugerido: String,
    pub resumo: String,
    pub confianca: String,
}

/// Valida o JSON de uma ficha e devolve todos os problemas de uma vez, em frases curtas que podem
/// voltar ao modelo. Vazio = ficha válida.
pub fn validar(json: &Value) -> Vec<String> {
    let mut erros = Vec::new();
    for campo in OBRIGATORIOS {
        match json.get(campo) {
            None => erros.push(format!("falta o campo {campo}")),
            Some(Value::String(s)) if s.trim().is_empty() => erros.push(format!("o campo {campo} está vazio")),
            _ => {}
        }
    }
    if let Some(e) = json.get("endereco").and_then(Value::as_str) {
        let hex = e.strip_prefix("0x").unwrap_or("");
        if hex.is_empty() || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            erros.push(format!("endereco precisa ser 0x seguido de hexadecimal, veio '{e}'"));
        }
    }
    if let Some(r) = json.get("resumo").and_then(Value::as_str) {
        if r.chars().count() > RESUMO_MAXIMO {
            erros.push(format!("resumo passa de {RESUMO_MAXIMO} caracteres ({} agora)", r.chars().count()));
        }
    }
    if let Some(b) = json.get("base").and_then(Value::as_str) {
        if !BASES.contains(&b) {
            erros.push(format!("base precisa ser observado, derivado ou inferido, veio '{b}'"));
        }
    }
    if let Some(c) = json.get("confianca").and_then(Value::as_str) {
        if !CONFIANCAS.contains(&c) {
            erros.push(format!("confianca precisa ser alta, media ou baixa, veio '{c}'"));
        }
    }
    if erros.is_empty() {
        if let Err(e) = serde_json::from_value::<Ficha>(json.clone()) {
            erros.push(format!("formato da ficha não confere: {e}"));
        }
    }
    erros
}

/// Valida e converte. Erro = lista de problemas (a mesma de [`validar`]).
pub fn parse_ficha(texto: &str) -> std::result::Result<Ficha, Vec<String>> {
    let json: Value = serde_json::from_str(texto).map_err(|e| vec![format!("não é JSON: {e}")])?;
    let erros = validar(&json);
    if !erros.is_empty() {
        return Err(erros);
    }
    serde_json::from_value(json).map_err(|e| vec![format!("formato da ficha não confere: {e}")])
}

/// Pasta `.anything` de um projeto.
pub fn pasta_do_modo(project_root: &Path) -> PathBuf {
    project_root.join(".anything")
}

/// A pasta de trabalho do modo: `funcoes/` (uma ficha por função) e `indice.json`.
pub struct Workspace {
    pub root: PathBuf,
}

impl Workspace {
    pub fn new(project_root: &Path) -> Self {
        Self { root: pasta_do_modo(project_root) }
    }

    /// Cria o layout se faltar. Não apaga nada.
    pub fn ensure(&self) -> Result<()> {
        std::fs::create_dir_all(self.funcoes_dir()).with_context(|| format!("não consegui criar {}", self.funcoes_dir().display()))?;
        std::fs::create_dir_all(self.root.join("modulos"))?;
        Ok(())
    }

    pub fn funcoes_dir(&self) -> PathBuf {
        self.root.join("funcoes")
    }

    pub fn ficha_path(&self, endereco: &str) -> PathBuf {
        self.funcoes_dir().join(format!("{endereco}.json"))
    }

    pub fn indice_path(&self) -> PathBuf {
        self.root.join("indice.json")
    }

    /// Grava a ficha (atômico: arquivo temporário e troca) e atualiza a linha dela no índice.
    pub fn gravar_ficha(&self, f: &Ficha) -> Result<()> {
        self.ensure()?;
        escrever_atomico(&self.ficha_path(&f.endereco), &serde_json::to_string_pretty(f)?)?;
        let mut indice = self.ler_indice()?;
        indice.retain(|l| l.endereco != f.endereco);
        indice.push(LinhaIndice {
            endereco: f.endereco.clone(),
            nome_sugerido: f.nome_sugerido.clone(),
            resumo: f.resumo.clone(),
            confianca: f.confianca.clone(),
        });
        indice.sort_by(|a, b| a.endereco.cmp(&b.endereco));
        escrever_atomico(&self.indice_path(), &serde_json::to_string_pretty(&indice)?)
    }

    /// Preenche `chama` na ficha de `endereco` e põe `endereco` em `chamada_por` de cada função chamada que
    /// já tem ficha. É o código quem liga as fichas (o modelo deixa esses campos vazios), com o grafo do plano.
    pub fn completar_ligacoes(&self, endereco: &str, chama: &[String]) -> Result<()> {
        let Some(mut ficha) = self.ler_ficha(endereco)? else { return Ok(()) };
        ficha.chama = chama.to_vec();
        self.gravar_ficha(&ficha)?;
        for c in chama {
            // Ficha vizinha inválida (item bloqueado que deixou o arquivo no disco) não pode derrubar a fila:
            // fica sem a ligação até ser corrigida.
            if let Some(mut chamada) = self.ler_ficha(c).ok().flatten() {
                if !chamada.chamada_por.iter().any(|x| x == endereco) {
                    chamada.chamada_por.push(endereco.to_string());
                    self.gravar_ficha(&chamada)?;
                }
            }
        }
        Ok(())
    }

    pub fn ler_ficha(&self, endereco: &str) -> Result<Option<Ficha>> {
        let p = self.ficha_path(endereco);
        if !p.is_file() {
            return Ok(None);
        }
        let texto = std::fs::read_to_string(&p)?;
        parse_ficha(&texto).map(Some).map_err(|e| anyhow!("ficha {endereco} inválida: {}", e.join("; ")))
    }

    pub fn ler_indice(&self) -> Result<Vec<LinhaIndice>> {
        let p = self.indice_path();
        if !p.is_file() {
            return Ok(Vec::new());
        }
        Ok(serde_json::from_str(&std::fs::read_to_string(&p)?)?)
    }

    /// Uma linha por endereço pedido, só o resumo: é o que um item recebe das funções que ela chama.
    /// Endereço sem ficha vira "ainda sem ficha", para o prompt dizer o que falta.
    pub fn resumos(&self, enderecos: &[String]) -> Result<Vec<String>> {
        let indice = self.ler_indice()?;
        Ok(enderecos
            .iter()
            .map(|e| match indice.iter().find(|l| &l.endereco == e) {
                Some(l) => format!("{} ({}): {}", l.endereco, l.nome_sugerido, l.resumo),
                None => format!("{e}: ainda sem ficha"),
            })
            .collect())
    }
}

/// Escreve `conteudo` em `destino` por um arquivo temporário ao lado, depois troca. Se falhar no
/// meio, o arquivo antigo continua inteiro.
pub fn escrever_atomico(destino: &Path, conteudo: &str) -> Result<()> {
    if let Some(pai) = destino.parent() {
        std::fs::create_dir_all(pai)?;
    }
    let tmp = destino.with_extension("tmp");
    std::fs::write(&tmp, conteudo).with_context(|| format!("não consegui gravar {}", tmp.display()))?;
    std::fs::rename(&tmp, destino).with_context(|| format!("não consegui trocar {}", destino.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ficha_ok() -> Value {
        serde_json::json!({
            "endereco": "0x401000",
            "nome_original": "FUN_00401000",
            "nome_sugerido": "calcula_dano",
            "resumo": "Calcula o dano final com a armadura.",
            "o_que_faz": "Lê o dano base e aplica o multiplicador.",
            "constantes": [{"valor": "0.75", "significado": "multiplicador de armadura"}],
            "chama": ["0x401200"],
            "chamada_por": [],
            "base": "observado",
            "confianca": "alta",
            "evidencias": ["ev_1"],
            "pendencias": []
        })
    }

    fn tmp(nome: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("anything-ws-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_good_ficha_has_no_errors() {
        assert!(validar(&ficha_ok()).is_empty());
        assert!(parse_ficha(&ficha_ok().to_string()).is_ok());
    }

    #[test]
    fn every_problem_is_listed_at_once() {
        let mut j = ficha_ok();
        j.as_object_mut().unwrap().remove("resumo");
        j["base"] = "chutado".into();
        j["confianca"] = "altissima".into();
        j["endereco"] = "401000".into();
        let erros = validar(&j);
        assert!(erros.iter().any(|e| e.contains("falta o campo resumo")));
        assert!(erros.iter().any(|e| e.contains("base precisa")));
        assert!(erros.iter().any(|e| e.contains("confianca precisa")));
        assert!(erros.iter().any(|e| e.contains("endereco precisa")));
        assert!(erros.len() >= 4);
    }

    #[test]
    fn a_long_summary_is_refused_with_the_count() {
        let mut j = ficha_ok();
        j["resumo"] = "x".repeat(RESUMO_MAXIMO + 5).into();
        let erros = validar(&j);
        assert_eq!(erros.len(), 1);
        assert!(erros[0].contains(&format!("({} agora)", RESUMO_MAXIMO + 5)));
    }

    #[test]
    fn invalid_json_is_reported_not_panicked() {
        assert!(parse_ficha("{ isto não é json").unwrap_err()[0].contains("não é JSON"));
    }

    #[test]
    fn saving_a_ficha_updates_the_index_and_reads_back() {
        let d = tmp("grava");
        let ws = Workspace::new(&d);
        let f = parse_ficha(&ficha_ok().to_string()).unwrap();
        ws.gravar_ficha(&f).unwrap();
        assert_eq!(ws.ler_ficha("0x401000").unwrap(), Some(f.clone()));
        // Gravar de novo troca a linha, não duplica.
        ws.gravar_ficha(&f).unwrap();
        assert_eq!(ws.ler_indice().unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn summaries_mark_functions_without_a_ficha() {
        let d = tmp("resumos");
        let ws = Workspace::new(&d);
        ws.gravar_ficha(&parse_ficha(&ficha_ok().to_string()).unwrap()).unwrap();
        let r = ws.resumos(&["0x401000".into(), "0x999999".into()]).unwrap();
        assert!(r[0].contains("calcula_dano"));
        assert_eq!(r[1], "0x999999: ainda sem ficha");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_index_stays_sorted_by_address() {
        let d = tmp("ordem");
        let ws = Workspace::new(&d);
        for e in ["0x500000", "0x100000", "0x300000"] {
            let mut j = ficha_ok();
            j["endereco"] = e.into();
            ws.gravar_ficha(&parse_ficha(&j.to_string()).unwrap()).unwrap();
        }
        let enderecos: Vec<String> = ws.ler_indice().unwrap().into_iter().map(|l| l.endereco).collect();
        assert_eq!(enderecos, ["0x100000", "0x300000", "0x500000"]);
        let _ = std::fs::remove_dir_all(&d);
    }
}

/// Um caminho de saída que vem do `tarefas.json` (que pode ser editado à mão) só vale se for relativo,
/// sem `..` nem prefixo de unidade, e ficar em `funcoes/`, `modulos/` ou ser `relatorio.md`.
pub fn saida_segura(saida: &str) -> bool {
    use std::path::Component;
    let p = Path::new(saida);
    if saida.is_empty() || p.is_absolute() {
        return false;
    }
    if !p.components().all(|c| matches!(c, Component::Normal(_))) {
        return false;
    }
    saida == "relatorio.md" || saida.starts_with("funcoes/") || saida.starts_with("modulos/") || saida.starts_with("entrega/")
}

#[cfg(test)]
mod saida_tests {
    use super::*;

    #[test]
    fn only_paths_inside_the_workspace_are_accepted() {
        assert!(saida_segura("funcoes/0x401000.json"));
        assert!(saida_segura("modulos/modulo_01.md"));
        assert!(saida_segura("relatorio.md"));
        assert!(saida_segura("entrega/LEIAME.md"));
        assert!(!saida_segura("entrega/../fora.md"));
        assert!(!saida_segura("../fora.json"));
        assert!(!saida_segura("funcoes/../../fora.json"));
        assert!(!saida_segura("/etc/passwd"));
        assert!(!saida_segura("C:/Windows/x.json"));
        assert!(!saida_segura("alvo.json"));
        assert!(!saida_segura(""));
    }
}

#[cfg(test)]
mod ligacoes_tests {
    use super::*;

    fn ficha(endereco: &str) -> Ficha {
        Ficha {
            endereco: endereco.into(),
            nome_original: "sub".into(),
            nome_sugerido: "f".into(),
            resumo: "faz algo".into(),
            o_que_faz: "faz".into(),
            constantes: vec![],
            chama: vec![],
            chamada_por: vec![],
            base: "observado".into(),
            confianca: "alta".into(),
            evidencias: vec![],
            pendencias: vec![],
        }
    }

    #[test]
    fn the_code_links_callers_and_callees_between_fichas() {
        let d = std::env::temp_dir().join(format!("anything-lig-{}", uuid::Uuid::new_v4()));
        let ws = Workspace { root: d.clone() };
        ws.gravar_ficha(&ficha("0x10")).unwrap(); // folha, estudada primeiro
        ws.gravar_ficha(&ficha("0x20")).unwrap(); // chama a folha
        ws.completar_ligacoes("0x20", &["0x10".to_string(), "0x99".to_string()]).unwrap();
        // De novo: não duplica.
        ws.completar_ligacoes("0x20", &["0x10".to_string(), "0x99".to_string()]).unwrap();
        let chamadora = ws.ler_ficha("0x20").unwrap().unwrap();
        assert_eq!(chamadora.chama, ["0x10", "0x99"], "chama vem do plano, mesmo sem ficha do 0x99");
        let folha = ws.ler_ficha("0x10").unwrap().unwrap();
        assert_eq!(folha.chamada_por, ["0x20"]);
        // Função sem ficha: nada acontece, sem erro.
        ws.completar_ligacoes("0x77", &["0x10".to_string()]).unwrap();
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn an_invalid_neighbour_ficha_does_not_stop_the_linking() {
        let d = std::env::temp_dir().join(format!("anything-lig-inv-{}", uuid::Uuid::new_v4()));
        let ws = Workspace { root: d.clone() };
        ws.gravar_ficha(&ficha("0x20")).unwrap();
        // Ficha de item bloqueado que ficou no disco com um campo fora do padrão.
        std::fs::create_dir_all(ws.ficha_path("0x10").parent().unwrap()).unwrap();
        std::fs::write(ws.ficha_path("0x10"), "{\"endereco\":\"0x10\",\"base\":\"texto livre\"}").unwrap();
        ws.completar_ligacoes("0x20", &["0x10".to_string()]).unwrap();
        assert_eq!(ws.ler_ficha("0x20").unwrap().unwrap().chama, ["0x10"]);
        let _ = std::fs::remove_dir_all(&d);
    }
}
