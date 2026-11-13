//! Modo Anything, fase 07: triagem, seleção de funções, ordem de estudo e geração do `tarefas.json`.
//! Tudo aqui é código. O modelo não escolhe funções nem ordem: ele só estuda o item que a fila entrega.

use super::ghidra::run_cli;
use super::workspace::{escrever_atomico, Workspace};
use crate::agent::task_queue;
use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Tempo máximo de uma chamada ao motor (a primeira análise de um alvo grande pode ser demorada).
const LIMITE_CHAMADA: Duration = Duration::from_secs(600);
/// Tamanho de página ao listar funções.
const PAGINA: u64 = 5000;
/// Quantos níveis de chamadas a seleção por objetivo acompanha a partir das sementes.
const PROFUNDIDADE: usize = 2;
/// Quantas palavras do objetivo viram busca de string.
const PALAVRAS_MAXIMAS: usize = 8;
/// Palavras muito curtas ou comuns não viram busca.
const PALAVRA_MINIMA: usize = 4;
/// Referências de string consideradas no máximo (um objetivo amplo pode achar milhares).
const REFERENCIAS_MAXIMAS: usize = 300;
/// Abaixo disto a busca por objetivo não é confiável: o código pede o modo completo.
const SEMENTES_MINIMAS: usize = 5;
const STOPWORDS: &[&str] = &[
    "como", "para", "pelo", "pela", "qual", "quais", "sobre", "esse", "essa", "isso", "isto", "uma", "umas", "uns", "with", "from", "that",
    "this", "what", "which", "does", "about", "how", "where", "when", "the", "and",
];

/// Uma função listada pelo motor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Funcao {
    pub endereco: String,
    pub nome: String,
    pub tamanho: u64,
}

/// O que a seleção devolve para a interface.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Resumo {
    pub modo: String,
    pub funcoes_no_alvo: usize,
    pub selecionadas: usize,
    pub tarefas_novas: usize,
    pub tarefas_no_arquivo: usize,
    pub primeiras: Vec<String>,
}

/// Contexto de uma série de chamadas ao motor: o executável, o projeto `.cany`, o alvo e os flags
/// (`--deep` ou `--clean-room`, vindos das opções efetivas).
pub struct Contexto<'a> {
    pub exe: &'a Path,
    pub projeto: PathBuf,
    pub alvo: &'a str,
    pub flags: Vec<String>,
}

/// Uma chamada ao motor. Devolve o envelope inteiro (`result`, `coverage`, ...). Falha com o
/// stderr se o motor sair com erro.
async fn chamar(ctx: &Contexto<'_>, ferramenta: &str, posicional: Option<&str>, opcoes: &[(&str, String)]) -> Result<Value> {
    let mut args: Vec<String> = ctx.flags.clone();
    args.push("--project".into());
    args.push(ctx.projeto.display().to_string());
    args.push(ferramenta.into());
    if let Some(p) = posicional {
        args.push(p.into());
    }
    for (k, v) in opcoes {
        args.push(format!("--{k}"));
        args.push(v.clone());
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = run_cli(ctx.exe, &refs, LIMITE_CHAMADA).await?;
    if !out.status.success() {
        let erro: String = String::from_utf8_lossy(&out.stderr).chars().rev().take(600).collect::<String>().chars().rev().collect();
        bail!("{ferramenta} falhou: {}", erro.trim());
    }
    serde_json::from_slice(&out.stdout).with_context(|| format!("{ferramenta} não devolveu JSON"))
}

/// Triagem do alvo: o que ele é (`kind`, `format`, arquitetura). Vai para `alvo.json`.
pub async fn triar(ctx: &Contexto<'_>) -> Result<Value> {
    let env = chamar(ctx, "core-target-triage", None, &[("path", ctx.alvo.to_string())]).await?;
    env.get("result").cloned().ok_or_else(|| anyhow!("a triagem não devolveu resultado"))
}

/// Todas as funções do alvo, em páginas, sem as que são só repasse (thunk) ou vazias.
pub async fn funcoes(ctx: &Contexto<'_>) -> Result<(usize, Vec<Funcao>)> {
    let mut offset: u64 = 0;
    let mut todas = Vec::new();
    let mut total = 0usize;
    loop {
        let env = chamar(ctx, "native-functions-list", Some(ctx.alvo), &[("limit", PAGINA.to_string()), ("offset", offset.to_string())]).await?;
        total = env["coverage"]["total"].as_u64().unwrap_or(total as u64) as usize;
        for f in env["result"]["functions"].as_array().into_iter().flatten() {
            let thunk = f.get("thunk_of").map(|v| !v.is_null()).unwrap_or(false);
            let tamanho = f["size"].as_u64().unwrap_or(0);
            if thunk || tamanho == 0 {
                continue;
            }
            todas.push(Funcao {
                endereco: f["address"].as_str().unwrap_or_default().to_string(),
                nome: f["name"].as_str().unwrap_or_default().to_string(),
                tamanho,
            });
        }
        match env["coverage"]["next_offset"].as_u64() {
            Some(n) if n > offset => offset = n,
            _ => break,
        }
    }
    Ok((total, todas))
}

/// Endereços das funções que `endereco` chama (sem imports e sem chamadas indiretas).
pub async fn chamadas(ctx: &Contexto<'_>, endereco: &str) -> Result<Vec<String>> {
    let env = chamar(ctx, "native-function-callees-list", Some(ctx.alvo), &[("function", endereco.to_string())]).await?;
    Ok(env["result"]["callees"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| !c["import"].as_bool().unwrap_or(false) && !c["indirect"].as_bool().unwrap_or(false))
        .filter_map(|c| c["target"].as_str().map(str::to_string))
        .collect())
}

/// Palavras do objetivo que viram busca de string: 4 letras ou mais, sem as comuns, sem repetir.
pub fn palavras_chave(objetivo: &str) -> Vec<String> {
    let mut vistas = BTreeSet::new();
    let mut out = Vec::new();
    for p in objetivo.split(|c: char| !c.is_alphanumeric()) {
        let p = p.to_lowercase();
        if p.chars().count() < PALAVRA_MINIMA || STOPWORDS.contains(&p.as_str()) {
            continue;
        }
        if vistas.insert(p.clone()) {
            out.push(p);
        }
        if out.len() == PALAVRAS_MAXIMAS {
            break;
        }
    }
    out
}

/// Sementes pelo objetivo: strings que contêm as palavras → quem as referencia → a função que contém
/// cada referência. Depois, `PROFUNDIDADE` níveis de chamadas a partir delas.
pub async fn selecionar_por_objetivo(ctx: &Contexto<'_>, objetivo: &str, teto: usize) -> Result<Vec<String>> {
    let palavras = palavras_chave(objetivo);
    if palavras.is_empty() {
        bail!("o objetivo não tem palavras para buscar; use o modo completo");
    }
    let mut origens: Vec<String> = Vec::new();
    let mut vistas = HashSet::new();
    'palavras: for p in &palavras {
        let env = chamar(ctx, "native-strings-find", Some(ctx.alvo), &[("pattern", p.clone()), ("limit", "200".into())]).await?;
        for m in env["result"]["matches"].as_array().into_iter().flatten() {
            for r in m["references"].as_array().into_iter().flatten() {
                if let Some(from) = r["from"].as_str() {
                    if vistas.insert(from.to_string()) {
                        origens.push(from.to_string());
                    }
                }
                if origens.len() >= REFERENCIAS_MAXIMAS {
                    break 'palavras;
                }
            }
        }
    }
    let mut sementes = BTreeSet::new();
    for from in &origens {
        let env = chamar(ctx, "native-function-find", Some(ctx.alvo), &[("address", from.clone())]).await?;
        if let Some(entrada) = env["result"]["entry"].as_str() {
            sementes.insert(entrada.to_string());
        }
    }
    if sementes.len() < SEMENTES_MINIMAS {
        bail!("a busca pelo objetivo achou só {} função(ões); use o modo completo", sementes.len());
    }
    let mut selecionadas: BTreeSet<String> = sementes.clone();
    let mut fronteira: Vec<String> = sementes.into_iter().collect();
    for _ in 0..PROFUNDIDADE {
        let mut proxima = Vec::new();
        for f in &fronteira {
            for c in chamadas(ctx, f).await? {
                if selecionadas.len() >= teto {
                    break;
                }
                if selecionadas.insert(c.clone()) {
                    proxima.push(c);
                }
            }
        }
        fronteira = proxima;
    }
    Ok(selecionadas.into_iter().collect())
}

/// Ordem de estudo: quem é chamado vem antes de quem chama (pós-ordem do grafo, só entre os nós
/// selecionados). Ciclos são quebrados onde a busca encontra a aresta de volta. Ordem estável: nós em
/// ordem de endereço.
pub fn ordenar(nodos: &[String], arestas: &HashMap<String, Vec<String>>) -> Vec<String> {
    let dentro: HashSet<&str> = nodos.iter().map(String::as_str).collect();
    let mut ordem_entrada: Vec<&String> = nodos.iter().collect();
    ordem_entrada.sort();
    let mut feitos: HashSet<String> = HashSet::new();
    let mut em_curso: HashSet<String> = HashSet::new();
    let mut saida = Vec::new();
    for raiz in ordem_entrada {
        if feitos.contains(raiz) {
            continue;
        }
        let mut pilha: Vec<(String, usize)> = vec![(raiz.clone(), 0)];
        em_curso.insert(raiz.clone());
        while let Some((no, i)) = pilha.last().cloned() {
            let filhos: &[String] = arestas.get(&no).map(Vec::as_slice).unwrap_or(&[]);
            let mut j = i;
            let mut prox = None;
            while j < filhos.len() {
                let c = &filhos[j];
                j += 1;
                if dentro.contains(c.as_str()) && !feitos.contains(c) && !em_curso.contains(c) {
                    prox = Some(c.clone());
                    break;
                }
            }
            if let Some(top) = pilha.last_mut() {
                top.1 = j;
            }
            match prox {
                Some(c) => {
                    em_curso.insert(c.clone());
                    pilha.push((c, 0));
                }
                None => {
                    pilha.pop();
                    em_curso.remove(&no);
                    feitos.insert(no.clone());
                    saida.push(no);
                }
            }
        }
    }
    saida
}

/// Um item de função para o `tarefas.json`. A descrição é provisória: a fase 09 põe o prompt definitivo.
pub fn tarefa_de_funcao(endereco: &str, nome: &str, chama: &[String], goal: &str) -> Value {
    json!({
        "id": format!("fn-{endereco}"),
        "status": task_queue::STATUS_FAZER,
        "titulo": format!("Estudar {nome} ({endereco})"),
        "descricao": format!(
            "Estude a função {endereco} ({nome}) para o objetivo: {goal}. Decompile e salve a ficha em funcoes/{endereco}.json."
        ),
        "tipo": "funcao",
        "endereco": endereco,
        "chama": chama,
        "saida": format!("funcoes/{endereco}.json"),
    })
}

/// Junta as tarefas novas às que já estão no arquivo. Item que já existe não é recriado nem muda de
/// status: o que está `pronto` ou `bloqueada` fica como está (a fase 08 mexe no status).
pub fn mesclar(existentes: Vec<Value>, novas: Vec<Value>) -> (Vec<Value>, usize) {
    let ids: HashSet<String> = existentes.iter().filter_map(|t| t["id"].as_str().map(str::to_string)).collect();
    let mut tarefas = existentes;
    let mut adicionadas = 0;
    for t in novas {
        let id = t["id"].as_str().unwrap_or_default().to_string();
        if !ids.contains(&id) {
            tarefas.push(t);
            adicionadas += 1;
        }
    }
    (tarefas, adicionadas)
}

/// Roda o plano inteiro: triagem, seleção, grafo, ordem e gravação. `modo` é `completo` ou `objetivo`.
pub async fn planejar(ctx: &Contexto<'_>, ws: &Workspace, modo: &str, teto: usize, goal: &str) -> Result<Resumo> {
    ws.ensure()?;
    let triagem = triar(ctx).await?;
    let (total, todas) = funcoes(ctx).await?;
    let selecionadas: Vec<String> = match modo {
        "completo" => {
            let mut v: Vec<String> = todas.iter().map(|f| f.endereco.clone()).collect();
            v.sort();
            v.truncate(teto);
            v
        }
        "objetivo" => selecionar_por_objetivo(ctx, goal, teto).await?,
        outro => bail!("modo desconhecido: {outro}"),
    };
    let nomes: HashMap<String, String> = todas.iter().map(|f| (f.endereco.clone(), f.nome.clone())).collect();
    let mut arestas: HashMap<String, Vec<String>> = HashMap::new();
    for e in &selecionadas {
        let filhos: Vec<String> = chamadas(ctx, e).await?.into_iter().filter(|c| selecionadas.contains(c)).collect();
        arestas.insert(e.clone(), filhos);
    }
    let ordem = ordenar(&selecionadas, &arestas);
    let novas: Vec<Value> = ordem
        .iter()
        .map(|e| {
            let nome = nomes.get(e).map(String::as_str).unwrap_or("sem nome");
            tarefa_de_funcao(e, nome, arestas.get(e).map(Vec::as_slice).unwrap_or(&[]), goal)
        })
        .collect();

    let caminho_tarefas = ws.root.join("tarefas.json");
    let existentes: Vec<Value> = if caminho_tarefas.is_file() {
        let texto = std::fs::read_to_string(&caminho_tarefas)?;
        task_queue::parse_tasks(&texto).map_err(|e| anyhow!("tarefas.json atual não confere: {e}"))?
    } else {
        Vec::new()
    };
    let (tarefas, adicionadas) = mesclar(existentes, novas);
    escrever_atomico(&caminho_tarefas, &task_queue::serialize_tasks(&tarefas))?;

    let alvo = json!({ "alvo": ctx.alvo, "triagem": triagem, "modo": modo, "teto": teto, "objetivo": goal, "funcoes_no_alvo": total });
    escrever_atomico(&ws.root.join("alvo.json"), &serde_json::to_string_pretty(&alvo)?)?;

    Ok(Resumo {
        modo: modo.to_string(),
        funcoes_no_alvo: total,
        selecionadas: selecionadas.len(),
        tarefas_novas: adicionadas,
        tarefas_no_arquivo: tarefas.len(),
        primeiras: ordem.into_iter().take(5).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arestas(pares: &[(&str, &[&str])]) -> HashMap<String, Vec<String>> {
        pares.iter().map(|(k, v)| (k.to_string(), v.iter().map(|s| s.to_string()).collect())).collect()
    }

    fn nodos(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn callees_come_before_callers() {
        // A chama B e C; B chama C. Ordem: C, B, A.
        let g = arestas(&[("0x1", &["0x2", "0x3"]), ("0x2", &["0x3"]), ("0x3", &[])]);
        assert_eq!(ordenar(&nodos(&["0x1", "0x2", "0x3"]), &g), nodos(&["0x3", "0x2", "0x1"]));
    }

    #[test]
    fn a_cycle_does_not_hang_and_every_node_appears_once() {
        let g = arestas(&[("0x1", &["0x2"]), ("0x2", &["0x3"]), ("0x3", &["0x1"])]);
        let o = ordenar(&nodos(&["0x1", "0x2", "0x3"]), &g);
        assert_eq!(o.len(), 3);
        let unicos: HashSet<_> = o.iter().collect();
        assert_eq!(unicos.len(), 3);
    }

    #[test]
    fn edges_leaving_the_selection_are_ignored() {
        let g = arestas(&[("0x1", &["0x9"])]);
        assert_eq!(ordenar(&nodos(&["0x1"]), &g), nodos(&["0x1"]));
    }

    #[test]
    fn keywords_skip_short_and_common_words_and_repeats() {
        let k = palavras_chave("Como o app salva a senha do usuário, salva senha!");
        assert_eq!(k, nodos(&["salva", "senha", "usuário"]));
        assert!(palavras_chave("como é isso").is_empty());
    }

    #[test]
    fn a_task_has_the_fields_the_queue_reads() {
        let t = tarefa_de_funcao("0x401000", "calcula_dano", &nodos(&["0x401200"]), "dano");
        assert_eq!(t["id"], "fn-0x401000");
        assert_eq!(t["status"], "fazer");
        assert_eq!(t["saida"], "funcoes/0x401000.json");
        assert!(task_queue::item_prompt_text(&t).is_ok());
    }

    #[test]
    fn merging_keeps_finished_items_and_adds_only_new_ids() {
        let antiga = json!({"id": "fn-0x1", "status": "pronto", "titulo": "antiga"});
        let nova_igual = json!({"id": "fn-0x1", "status": "fazer", "titulo": "nova"});
        let nova = json!({"id": "fn-0x2", "status": "fazer", "titulo": "nova"});
        let (tarefas, n) = mesclar(vec![antiga.clone()], vec![nova_igual, nova]);
        assert_eq!(n, 1);
        assert_eq!(tarefas.len(), 2);
        assert_eq!(tarefas[0], antiga, "o item pronto não muda");
    }
}

/// Teste de verdade, contra o motor instalado e um executável real. Não roda por padrão:
/// `ANYTHING_TEST_STORE=<pasta> ANYTHING_TEST_TARGET=<arquivo> ANYTHING_TEST_ROOT=<pasta> \
///  cargo test --lib planejamento_real -- --ignored --nocapture`
#[cfg(test)]
mod real {
    use super::*;

    #[tokio::test]
    #[ignore = "precisa do motor instalado e de um alvo; ver instruções acima"]
    async fn planejamento_real() {
        let store = std::path::PathBuf::from(std::env::var("ANYTHING_TEST_STORE").expect("ANYTHING_TEST_STORE"));
        let alvo = std::env::var("ANYTHING_TEST_TARGET").expect("ANYTHING_TEST_TARGET");
        let raiz = std::path::PathBuf::from(std::env::var("ANYTHING_TEST_ROOT").expect("ANYTHING_TEST_ROOT"));
        let (exe, _) = super::super::cli::find_cli(&store).expect("motor instalado");
        let ws = Workspace::new(&raiz);
        let ctx = Contexto { exe: &exe, projeto: super::super::session::project_dir(&raiz), alvo: &alvo, flags: vec!["--deep".into()] };
        let r = planejar(&ctx, &ws, "completo", 40, "user account name").await.unwrap();
        println!("COMPLETO: {}", serde_json::to_string_pretty(&r).unwrap());
        match planejar(&ctx, &ws, "objetivo", 60, "user account name windows security").await {
            Ok(r) => println!("OBJETIVO: {}", serde_json::to_string_pretty(&r).unwrap()),
            Err(e) => println!("OBJETIVO recusado (esperado com poucas sementes): {e}"),
        }
        let r2 = planejar(&ctx, &ws, "completo", 40, "user account name").await.unwrap();
        println!("REPETIDO: {}", serde_json::to_string_pretty(&r2).unwrap());
    }
}
