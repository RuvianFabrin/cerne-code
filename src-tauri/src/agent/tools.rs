use super::ast_tools;
use super::websearch;
use crate::models::{ToolFunctionSpec, ToolSpec};
use crate::sandbox;
use anyhow::{anyhow, Result};
use grep::regex::RegexMatcher;
use grep::searcher::sinks::UTF8;
use grep::searcher::{BinaryDetection, SearcherBuilder};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

/// Available in every session, project folder or not.
pub fn always_tool_specs() -> Vec<ToolSpec> {
    vec![
        spec(
            "web_search",
            "Busca na web e retorna titulo/URL/trecho dos resultados mais relevantes, agregando varias fontes independentes em paralelo (sem depender de nenhuma conta ou instalacao) e removendo duplicatas. Aceita uma ou mais queries por chamada - voce decide quantas: uma so basta na maioria dos casos, mas se o pedido tiver varios angulos, ou a primeira busca claramente nao trouxe o que precisa, mande queries adicionais (frases diferentes, sinonimos, termos mais especificos) na MESMA chamada em vez de repetir chamadas uma de cada vez.",
            json!({
                "type": "object",
                "properties": {
                    "queries": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Uma ou mais queries de busca."
                    }
                },
                "required": ["queries"]
            }),
        ),
        spec(
            "web_fetch",
            "Busca uma URL especifica e retorna o texto visivel da pagina (sem HTML/scripts). Use depois de web_search para ler uma fonte inteira.",
            json!({
                "type": "object",
                "properties": { "url": { "type": "string" } },
                "required": ["url"]
            }),
        ),
        spec(
            "load_skill",
            "Carrega o conteudo completo de uma skill pelo nome exato, a partir do catalogo de skills disponiveis listado no inicio desta conversa.",
            json!({
                "type": "object",
                "properties": { "name": { "type": "string" } },
                "required": ["name"]
            }),
        ),
        spec(
            "ask",
            "Pausa o turno e pergunta algo especifico ao usuario, com opcoes de multipla escolha e/ou texto livre, antes de continuar - use quando precisar de uma decisao que so o usuario pode tomar (escolher entre abordagens, confirmar uma acao arriscada, desambiguar algo) em vez de assumir e seguir. Espera a resposta antes de prosseguir, entao use com moderacao - so quando realmente travar sem essa decisao.",
            json!({
                "type": "object",
                "properties": {
                    "question": { "type": "string" },
                    "options": { "type": "array", "items": { "type": "string" }, "description": "Opcoes de multipla escolha (opcional). Sem isso, o usuario so responde com texto livre." }
                },
                "required": ["question"]
            }),
        ),
        spec(
            "todo_list",
            "Cria ou atualiza uma lista de tarefas visivel no chat do usuario. Use pra planejar trabalho complexo (3+ passos), mostrar progresso, e manter o usuario informado. Cada chamada SUBSTITUI a lista inteira — mande todos os itens (nao so os mudados). Status: pending (a fazer), in_progress (fazendo agora, no maximo 1), completed (concluido). Nao use pra tarefas simples de 1 passo.",
            json!({
                "type": "object",
                "properties": {
                    "todos": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "content": { "type": "string", "description": "Descricao da tarefa" },
                                "status": { "type": "string", "enum": ["pending", "in_progress", "completed"] }
                            },
                            "required": ["content", "status"]
                        }
                    }
                },
                "required": ["todos"]
            }),
        ),
    ]
}

/// Only available when the session has a project folder attached.
pub fn project_tool_specs() -> Vec<ToolSpec> {
    vec![
        spec(
            "read_file",
            "Le o conteudo de um arquivo. Caminho relativo e resolvido dentro do projeto; caminho absoluto funciona para QUALQUER pasta do sistema (ex: F:\\outro-repo\\src\\main.rs) — use para consultar codigo de outros repositorios ou documentacao externa. Use offset+limit pra ler so um trecho de arquivos grandes (economiza tokens e memoria) — o retorno inclui o total de linhas pra voce saber se precisa continuar lendo.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Caminho relativo a raiz do projeto, ou caminho absoluto de qualquer pasta do sistema" },
                    "offset": { "type": "integer", "description": "Linha inicial (0-based). Omita pra comecar do inicio." },
                    "limit": { "type": "integer", "description": "Maximo de linhas a retornar. Omita pra ler ate o fim (ou ate o teto de seguranca)." }
                },
                "required": ["path"]
            }),
        ),
        spec(
            "list_dir",
            "Lista arquivos e subpastas de um diretorio. Caminho relativo e resolvido dentro do projeto; caminho absoluto funciona para qualquer pasta do sistema.",
            json!({
                "type": "object",
                "properties": { "path": { "type": "string", "description": "Caminho relativo a raiz do projeto (vazio = raiz), ou caminho absoluto de qualquer pasta" } },
                "required": []
            }),
        ),
        spec(
            "grep",
            "Busca um padrao (regex) no conteudo dos arquivos. Caminho relativo busca dentro do projeto; caminho absoluto busca em qualquer pasta do sistema.",
            json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string" },
                    "path": { "type": "string", "description": "Subpasta relativa ao projeto, ou caminho absoluto de qualquer pasta (opcional)" }
                },
                "required": ["pattern"]
            }),
        ),
        spec(
            "run_command",
            "Roda um comando de shell no diretorio do projeto. Por padrao e sincrono e retorna stdout/stderr so quando o comando termina - NUNCA use isso pra dev server, watch mode, ou qualquer processo que fica rodando de proposito (o comando trava ate alguem matar o processo, e a chamada nunca retorna). Pra esses casos, passe background=true: retorna na hora com um id, sem esperar terminar; use check_background_output(id) pra ver o progresso depois e stop_background(id) pra encerrar.",
            json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string" },
                    "background": { "type": "boolean", "description": "true pra nao esperar o comando terminar (dev server, watch mode, build longo). Default false (sincrono)." }
                },
                "required": ["command"]
            }),
        ),
        spec(
            "check_background_output",
            "Le o output acumulado (stdout+stderr) e o status atual (rodando ou encerrado com que codigo) de um comando iniciado com run_command(background=true), sem para-lo.",
            json!({
                "type": "object",
                "properties": { "id": { "type": "string", "description": "id devolvido por run_command(background=true)" } },
                "required": ["id"]
            }),
        ),
        spec(
            "stop_background",
            "Encerra um comando em segundo plano iniciado com run_command(background=true) (mata o processo). Use quando nao precisar mais dele - por exemplo, depois de confirmar que um dev server subiu certo, ou antes de subir uma versao nova no lugar da antiga.",
            json!({
                "type": "object",
                "properties": { "id": { "type": "string", "description": "id devolvido por run_command(background=true)" } },
                "required": ["id"]
            }),
        ),
        spec(
            "list_background",
            "Lista todo comando em segundo plano conhecido (rodando ou ja encerrado), com id, status e o comando original. Use antes de iniciar um novo dev server pra checar se ja nao tem um rodando de uma sessao anterior.",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),
        spec(
            "write_file",
            "Cria ou sobrescreve um arquivo. A escrita vai para uma pasta sandbox espelhada; o usuario precisa aceitar o diff na interface antes de aplicar no arquivo real.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"]
            }),
        ),
        spec(
            "edit_file",
            "Edita um arquivo existente substituindo uma ocorrencia exata de old_str por new_str. old_str deve aparecer exatamente uma vez no arquivo. Escreve na sandbox, precisa ser aceito na interface.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "old_str": { "type": "string" },
                    "new_str": { "type": "string" }
                },
                "required": ["path", "old_str", "new_str"]
            }),
        ),
        spec(
            "ast_grep",
            "Busca ESTRUTURAL de codigo (nao textual): o padrao casa pela forma da AST, ignorando espaco/quebra de linha/formatacao. Use $VAR pra casar um nodo qualquer e $$$ARGS pra casar zero-ou-mais nodos (ex: 'console.log($$$ARGS)' acha qualquer chamada de console.log independente da quantidade de argumentos). Prefira isto a grep quando a busca for sobre estrutura de codigo (chamada de funcao, import, declaracao) em vez de texto solto.",
            json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string" },
                    "language": { "type": "string", "description": "bash, c, cpp, csharp, css, dart, elixir, go, haskell, hcl, html, java, javascript, json, kotlin, lua, markdown, nix, php, python, ruby, rust, scala, solidity, swift, typescript, tsx ou yaml" },
                    "path": { "type": "string", "description": "Subpasta onde buscar (opcional, vazio = raiz do projeto)" }
                },
                "required": ["pattern", "language"]
            }),
        ),
        spec(
            "ast_edit",
            "Reescrita ESTRUTURAL de um arquivo: toda ocorrencia do padrao (mesma sintaxe do ast_grep, $VAR/$$$ARGS) e trocada pelo template de reescrita, que pode reusar os mesmos nomes de variavel capturados. Mais seguro que edit_file pra refactor (rename de chamada, mudar import, etc.) porque opera na estrutura, nao em texto exato. Escreve na sandbox, precisa ser aceito na interface.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "pattern": { "type": "string" },
                    "rewrite": { "type": "string" },
                    "language": { "type": "string", "description": "bash, c, cpp, csharp, css, dart, elixir, go, haskell, hcl, html, java, javascript, json, kotlin, lua, markdown, nix, php, python, ruby, rust, scala, solidity, swift, typescript, tsx ou yaml" }
                },
                "required": ["path", "pattern", "rewrite", "language"]
            }),
        ),
        spec(
            "task",
            "Delega uma sub-tarefa bem definida e limitada pra um sub-agente descartavel: ele roda seu proprio loop de ferramentas (ate concluir ou atingir um limite de passos) usando o mesmo provider/modelo/projeto desta sessao, e devolve so o RELATORIO FINAL - os passos intermediarios dele nao aparecem nesta conversa. Use quando uma sub-tarefa precisa de varias chamadas de ferramenta (ler varios arquivos, investigar, editar) mas cujo processo intermediario nao importa pro usuario, so o resultado - por exemplo 'ache todos os usos de X no projeto e resuma onde estao' ou 'implemente a funcao Y no arquivo Z seguindo o padrao existente'. O sub-agente NAO tem acesso a esta ferramenta (nao pode delegar pra outro sub-agente - sem recursao).",
            json!({
                "type": "object",
                "properties": {
                    "description": { "type": "string", "description": "Resumo curto da sub-tarefa (aparece no painel de tarefas)" },
                    "prompt": { "type": "string", "description": "Instrucao completa e autocontida pro sub-agente - ele nao ve o historico desta conversa, so o que for escrito aqui" }
                },
                "required": ["description", "prompt"]
            }),
        ),
        spec(
            "verify_completion",
            "Dispara um verificador independente e CETICO (nao voce mesmo) pra reconferir com evidencia real se uma tarefa complexa/de varios passos foi REALMENTE concluida, antes de voce declarar sucesso pro usuario. O verificador so tem ferramentas de leitura/busca/execucao (read_file, list_dir, grep, ast_grep, run_command pra rodar teste/build/lint) - NAO pode editar nada - e assume REFUTADO por padrao ate achar evidencia concreta (rodar teste/build de verdade, nao so ler codigo). Devolve um veredito comecando com APROVADO ou REFUTADO + a evidencia. Use isso antes de declarar concluida uma tarefa complexa (varios arquivos, criar algo do zero) - NAO para um pedido simples de uma unica chamada de ferramenta, onde o resultado ja e obviamente verificavel sem esse passo extra.",
            json!({
                "type": "object",
                "properties": {
                    "task_summary": { "type": "string", "description": "O que foi pedido originalmente e o que voce fez pra resolver" },
                    "how_to_verify": { "type": "string", "description": "Como confirmar de verdade - que comando rodar (ex: 'cargo test', 'npm run build') ou o que conferir no codigo" }
                },
                "required": ["task_summary", "how_to_verify"]
            }),
        ),
        spec(
            "create_excel",
            "Cria um arquivo Excel (.xlsx) com uma ou mais abas, headers formatados, dados, largura de colunas e auto-filtro. Escreve direto no disco (nao usa sandbox). Use quando o usuario pedir para criar planilhas.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Caminho relativo do arquivo .xlsx a criar" },
                    "sheets": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string", "description": "Nome da aba" },
                                "headers": { "type": "array", "items": { "type": "string" }, "description": "Nomes das colunas (linha de header com negrito)" },
                                "rows": { "type": "array", "items": { "type": "array", "items": { "type": "string" } }, "description": "Linhas de dados" },
                                "column_widths": { "type": "array", "items": { "type": "number" }, "description": "Largura de cada coluna (opcional)" },
                                "freeze_header": { "type": "boolean", "description": "Congelar linha de header (default: true)" },
                                "auto_filter": { "type": "boolean", "description": "Adicionar auto-filtro nos headers (default: true)" }
                            },
                            "required": ["name", "headers", "rows"]
                        }
                    }
                },
                "required": ["path", "sheets"]
            }),
        ),
        spec(
            "create_word",
            "Cria um documento Word (.docx) com titulos, paragrafos, tabelas e listas formatadas. Escreve direto no disco (nao usa sandbox). Use quando o usuario pedir para criar documentos Word.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Caminho relativo do arquivo .docx a criar" },
                    "elements": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "type": { "type": "string", "enum": ["heading", "paragraph", "table", "list"], "description": "Tipo do elemento" },
                                "level": { "type": "integer", "description": "Nivel do titulo (1-3, so pra heading)" },
                                "text": { "type": "string", "description": "Texto do paragrafo ou titulo" },
                                "bold": { "type": "boolean", "description": "Texto em negrito (so pra paragraph)" },
                                "headers": { "type": "array", "items": { "type": "string" }, "description": "Headers da tabela (so pra table)" },
                                "rows": { "type": "array", "items": { "type": "array", "items": { "type": "string" } }, "description": "Linhas da tabela (so pra table)" },
                                "items": { "type": "array", "items": { "type": "string" }, "description": "Itens da lista (so pra list)" }
                            },
                            "required": ["type"]
                        }
                    }
                },
                "required": ["path", "elements"]
            }),
        ),
        spec(
            "create_pdf",
            "Cria um documento PDF com titulos, paragrafos e tabelas. Escreve direto no disco (nao usa sandbox). Use quando o usuario pedir para criar relatorios ou documentos PDF.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Caminho relativo do arquivo .pdf a criar" },
                    "title": { "type": "string", "description": "Titulo do documento (opcional)" },
                    "elements": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "type": { "type": "string", "enum": ["heading", "paragraph", "table"], "description": "Tipo do elemento" },
                                "text": { "type": "string", "description": "Texto do paragrafo ou titulo" },
                                "headers": { "type": "array", "items": { "type": "string" }, "description": "Headers da tabela (so pra table)" },
                                "rows": { "type": "array", "items": { "type": "array", "items": { "type": "string" } }, "description": "Linhas da tabela (so pra table)" }
                            },
                            "required": ["type"]
                        }
                    }
                },
                "required": ["path", "elements"]
            }),
        ),
    ]
}

fn spec(name: &str, description: &str, parameters: Value) -> ToolSpec {
    ToolSpec {
        kind: "function".to_string(),
        function: ToolFunctionSpec {
            name: name.to_string(),
            description: description.to_string(),
            parameters,
        },
    }
}

/// Resolve um caminho (relativo ou absoluto) restrito a `project_root` — usado
/// pelas ferramentas de ESCRITA (`write_file`/`edit_file`/`ast_edit`). Nunca
/// aceita pasta extra de leitura: a sandbox so espelha o `project_root`,
/// entao escrever fora dele nao tem onde ficar "pendente de aceite".
fn resolve_path(project_root: &Path, rel: &str) -> Result<PathBuf> {
    resolve_within(project_root, &[], rel)
}

/// Como `resolve_path`, mas para LEITURA aceita QUALQUER caminho absoluto no
/// sistema (o usuario pode pedir pra ler `F:\outro-repo\src\main.rs` sem
/// configurar pasta extra). Caminho relativo continua resolvendo dentro do
/// projeto. Restricao de escrita continua em `resolve_path` (so project_root).
fn resolve_read_path(project_root: &Path, _extra_roots: &[String], rel: &str) -> Result<PathBuf> {
    let trimmed = rel.trim();
    if trimmed.is_empty() {
        return Ok(project_root.to_path_buf());
    }
    let candidate = PathBuf::from(trimmed);
    if candidate.is_absolute() {
        return Ok(candidate);
    }
    Ok(project_root.join(trimmed.trim_start_matches(['/', '\\'])))
}

fn resolve_within(project_root: &Path, extra_roots: &[String], rel: &str) -> Result<PathBuf> {
    let candidate = project_root.join(rel.trim_start_matches(['/', '\\']));
    let root = project_root
        .canonicalize()
        .map_err(|e| anyhow!("raiz do projeto invalida: {e}"))?;
    let mut allowed_roots = vec![root];
    for extra in extra_roots {
        if let Ok(canon) = Path::new(extra).canonicalize() {
            allowed_roots.push(canon);
        }
    }
    // Path may not exist yet (e.g. new file) — canonicalize the deepest existing ancestor.
    let mut check = candidate.clone();
    while !check.exists() {
        match check.parent() {
            Some(p) => check = p.to_path_buf(),
            None => break,
        }
    }
    let canon_check = check.canonicalize().unwrap_or(check);
    if !allowed_roots
        .iter()
        .any(|allowed| canon_check.starts_with(allowed))
    {
        let extra_note = if extra_roots.is_empty() {
            String::new()
        } else {
            format!(
                " nem das pastas extras de leitura configuradas ({})",
                extra_roots.join(", ")
            )
        };
        return Err(anyhow!(
            "caminho fora da raiz do projeto{extra_note}: {rel}"
        ));
    }
    Ok(candidate)
}

/// Le um arquivo do projeto detectando a codificacao real dos bytes (ver
/// `crate::encoding`) em vez de assumir UTF-8 e falhar em arquivo legado
/// (Windows-1252/ISO) ou UTF-16 — cobre `read_file`/`edit_file`/`ast_edit`.
fn read_project_file(path: &Path, rel: &str) -> Result<String> {
    let bytes = std::fs::read(path).map_err(|e| anyhow!("nao foi possivel ler {rel}: {e}"))?;
    Ok(crate::encoding::decode(&bytes).0)
}

/// Grep real sobre as crates do ripgrep (grep-regex/grep-searcher), nao regex
/// linha-a-linha manual: suporta a sintaxe completa da crate `regex`
/// (lookaround-free mas com classes Unicode, `\b`, etc.) e detecta/pula
/// arquivos binarios automaticamente via a mesma heuristica do ripgrep.
///
/// Le e decodifica cada arquivo via `crate::encoding` (em vez de deixar o
/// `search_path` ler os bytes crus do disco direto) antes de buscar — sem
/// isso, um arquivo em Windows-1252/ISO/UTF-16 nunca batia com um padrao
/// acentuado (o modelo manda o padrao em UTF-8, e o byte 0xE9 cru de um "e"
/// acentuado em Windows-1252 nunca é igual aos 2 bytes UTF-8 de "é").
/// Decodificar primeiro e buscar no texto ja em UTF-8 (`search_slice`, nao
/// `search_path`) resolve isso pelo mesmo motivo que resolveu no
/// `edit_file`/`read_file`.
fn grep_search(pattern: &str, search_root: &Path, project_root: &Path) -> Result<Vec<String>> {
    let matcher = RegexMatcher::new(pattern).map_err(|e| anyhow!("regex invalida: {e}"))?;
    let mut searcher = SearcherBuilder::new()
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .line_number(true)
        .build();
    let mut matches = Vec::new();
    'walk: for path in super::walk_cache::files_under(search_root) {
        let rel_path = path
            .strip_prefix(project_root)
            .unwrap_or(&path)
            .display()
            .to_string();
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let (content, _) = crate::encoding::decode(&bytes);
        let result = searcher.search_slice(
            &matcher,
            content.as_bytes(),
            UTF8(|line_number, line| {
                matches.push(format!("{rel_path}:{line_number}: {}", line.trim()));
                Ok(matches.len() < 200)
            }),
        );
        if result.is_err() {
            // Binario (NUL detectado pelo binary_detection): ja tratado, so pula.
            continue;
        }
        if matches.len() >= 200 {
            break 'walk;
        }
    }
    Ok(matches)
}

/// Cascata de fallback do `edit_file` quando `old_str` nao bate byte-a-byte,
/// do mais estrito pro mais especulativo — inspirada na cascata de 8 passos
/// do modo `replace` do oh-my-pi (`packages/coding-agent/src/edit/modes/
/// replace.ts::seekSequence`, lido em `C:\Users\ru\oh-my-pi-src`), mas com 3
/// niveis em vez de 8 (trim/comentario/unicode/prefixo/substring/fuzzy por
/// similaridade/character-level): (1) espaco/indentacao nas pontas de cada
/// linha, (2) tambem normaliza aspas tipograficas/travessao/reticencias
/// unicode, (3) similaridade de texto (Levenshtein) como ultimo recurso. Cada
/// nivel so roda se o anterior nao achou nada (0 janelas); se um nivel achar
/// mais de uma janela, para ali e reporta ambiguidade em vez de tentar o
/// proximo nivel (mais ambiguidade, nao menos, ao afrouxar o criterio). A
/// escrita ainda cai na sandbox, entao o pior caso de um reindent errado e
/// um diff estranho pro usuario revisar, nao um arquivo corrompido.
enum FuzzyEditOutcome {
    /// Uma unica janela bateu — indice de linha (0-based) onde ela comeca.
    Unique(usize),
    /// Mais de uma janela bateu neste nivel — motivo (pra mensagem de erro) e quantas.
    Ambiguous(&'static str, usize),
    /// Nenhuma janela bateu em nivel nenhum — similaridade da mais proxima, se houver.
    NotFound(Option<f64>),
}

fn find_edit_window(original: &str, old_str: &str) -> FuzzyEditOutcome {
    match find_trimmed_line_windows(original, old_str).as_slice() {
        [] => {}
        &[start] => return FuzzyEditOutcome::Unique(start),
        matches => {
            return FuzzyEditOutcome::Ambiguous(
                "espacos/indentacao no inicio e fim de cada linha",
                matches.len(),
            )
        }
    }
    match find_unicode_normalized_windows(original, old_str).as_slice() {
        [] => {}
        &[start] => return FuzzyEditOutcome::Unique(start),
        matches => {
            return FuzzyEditOutcome::Ambiguous(
                "aspas tipograficas/travessao/reticencias diferentes",
                matches.len(),
            )
        }
    }
    let scores = fuzzy_window_scores(original, old_str);
    match scores.above_threshold {
        0 => FuzzyEditOutcome::NotFound(scores.best_score),
        1 => FuzzyEditOutcome::Unique(
            scores
                .best_index
                .expect("above_threshold=1 implica best_index preenchido"),
        ),
        n => FuzzyEditOutcome::Ambiguous("similaridade de texto (fuzzy)", n),
    }
}

/// Nivel 1: janelas de linhas cujo conteudo, ignorando espaco em branco nas
/// pontas de cada linha, e identico ao padrao — cobre o caso comum de
/// indentacao/trailing-whitespace diferente do que o modelo "lembra" do
/// arquivo. Retorna os indices de linha (0-based) onde cada janela comeca.
fn find_trimmed_line_windows(content: &str, pattern: &str) -> Vec<usize> {
    find_windows_by(content, pattern, |l| l.trim().to_string())
}

/// Nivel 2: como o nivel 1, mas tambem normaliza pontuacao tipografica
/// unicode (aspas curvas, travessao/meia-risca, reticencias) pro equivalente
/// ASCII — cobre o caso de um modelo "embelezar" texto ao citar de volta um
/// trecho lido antes (comum quando o texto passou por renderizacao
/// markdown).
fn find_unicode_normalized_windows(content: &str, pattern: &str) -> Vec<usize> {
    find_windows_by(content, pattern, |l| normalize_unicode(l.trim()))
}

fn find_windows_by(content: &str, pattern: &str, normalize: impl Fn(&str) -> String) -> Vec<usize> {
    let content_lines: Vec<&str> = content.lines().collect();
    let pattern_lines: Vec<&str> = pattern.lines().collect();
    if pattern_lines.is_empty() || pattern_lines.len() > content_lines.len() {
        return Vec::new();
    }
    let pattern_norm: Vec<String> = pattern_lines.iter().map(|l| normalize(l)).collect();
    (0..=content_lines.len() - pattern_lines.len())
        .filter(|&start| {
            (0..pattern_lines.len()).all(|i| normalize(content_lines[start + i]) == pattern_norm[i])
        })
        .collect()
}

/// Substitui pontuacao tipografica unicode comum pelo equivalente ASCII:
/// aspas curvas simples/duplas, travessao/meia-risca, espaco sem quebra,
/// reticencias.
fn normalize_unicode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\u{2018}' | '\u{2019}' | '\u{201B}' => out.push('\''),
            '\u{201C}' | '\u{201D}' | '\u{201F}' => out.push('"'),
            '\u{2013}' | '\u{2014}' | '\u{2015}' => out.push('-'),
            '\u{00A0}' => out.push(' '),
            '\u{2026}' => out.push_str("..."),
            other => out.push(other),
        }
    }
    out
}

/// Similaridade minima (0-1) pra uma janela ser aceita no nivel 3 (fuzzy).
const FUZZY_MATCH_THRESHOLD: f64 = 0.95;

struct FuzzyWindowScores {
    best_index: Option<usize>,
    best_score: Option<f64>,
    above_threshold: usize,
}

/// Nivel 3 (ultimo recurso): pontua toda janela de linhas por similaridade
/// media (Levenshtein por linha, jah normalizada como o nivel 2) contra o
/// padrao, e conta quantas passam de [`FUZZY_MATCH_THRESHOLD`]. So e seguro
/// aceitar quando exatamente 1 janela passa do limiar — 0 quer dizer "nao
/// achei nada parecido o bastante" e mais de 1 quer dizer "ambiguo,
/// pode acertar o trecho errado".
fn fuzzy_window_scores(content: &str, pattern: &str) -> FuzzyWindowScores {
    let content_lines: Vec<&str> = content.lines().collect();
    let pattern_lines: Vec<&str> = pattern.lines().collect();
    if pattern_lines.is_empty() || pattern_lines.len() > content_lines.len() {
        return FuzzyWindowScores {
            best_index: None,
            best_score: None,
            above_threshold: 0,
        };
    }
    let pattern_norm: Vec<String> = pattern_lines
        .iter()
        .map(|l| normalize_unicode(l.trim()))
        .collect();
    let mut best_index = None;
    let mut best_score = None;
    let mut above_threshold = 0;
    for start in 0..=(content_lines.len() - pattern_lines.len()) {
        let score = (0..pattern_lines.len())
            .map(|i| {
                text_similarity(
                    &normalize_unicode(content_lines[start + i].trim()),
                    &pattern_norm[i],
                )
            })
            .sum::<f64>()
            / pattern_lines.len() as f64;
        if score >= FUZZY_MATCH_THRESHOLD {
            above_threshold += 1;
        }
        if best_score.is_none_or(|best| score > best) {
            best_score = Some(score);
            best_index = Some(start);
        }
    }
    FuzzyWindowScores {
        best_index,
        best_score,
        above_threshold,
    }
}

/// Distancia de Levenshtein (numero minimo de insercoes/remocoes/trocas de
/// caractere pra transformar `a` em `b`) — implementacao direta, sem crate:
/// a mesma ideia de ~15 linhas que o proprio oh-my-pi usa a mao
/// (`replace.ts::levenshteinDistance`), nao ha crate publicada padrao pra
/// isso que valha a pena trazer so por essa funcao.
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        curr[0] = i;
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

/// Similaridade normalizada (0 a 1, 1 = identico) derivada da distancia de Levenshtein.
fn text_similarity(a: &str, b: &str) -> f64 {
    let max_len = a.chars().count().max(b.chars().count());
    if max_len == 0 {
        return 1.0;
    }
    1.0 - (levenshtein_distance(a, b) as f64 / max_len as f64)
}

/// Substitui a janela de linhas que comeca em `start` (do tamanho de
/// `old_str`, achada por [`find_trimmed_line_windows`]) pelo `new_str`
/// reindentado ao nivel do bloco real do arquivo.
fn replace_line_window(content: &str, old_str: &str, new_str: &str, start: usize) -> String {
    let content_lines: Vec<&str> = content.lines().collect();
    let pattern_line_count = old_str.lines().count();
    let matched_block = content_lines[start..start + pattern_line_count].join("\n");
    let adjusted_new = reindent_replacement(old_str, &matched_block, new_str);

    let mut new_lines: Vec<&str> = content_lines[..start].to_vec();
    let adjusted_lines: Vec<&str> = adjusted_new.lines().collect();
    new_lines.extend(adjusted_lines);
    new_lines.extend(&content_lines[start + pattern_line_count..]);

    let mut joined = new_lines.join("\n");
    if content.ends_with('\n') {
        joined.push('\n');
    }
    joined
}

/// Reaplica a indentacao do bloco de verdade no texto de troca: calcula a
/// diferenca entre a indentacao da primeira linha do `old_str` que o modelo
/// mandou e a indentacao real do bloco que bateu no arquivo, e desloca cada
/// linha do `new_str` por essa mesma diferenca. Se a indentacao for
/// incompativel (por exemplo tabs de um lado e espacos do outro), devolve
/// `new_str` sem alteracao em vez de arriscar um deslocamento errado.
fn reindent_replacement(old_str: &str, matched_block: &str, new_str: &str) -> String {
    let old_indent = leading_whitespace(old_str.lines().next().unwrap_or(""));
    let matched_indent = leading_whitespace(matched_block.lines().next().unwrap_or(""));
    if old_indent == matched_indent {
        return new_str.to_string();
    }
    if let Some(extra) = matched_indent.strip_prefix(old_indent) {
        return new_str
            .lines()
            .map(|line| {
                if line.trim().is_empty() {
                    line.to_string()
                } else {
                    format!("{extra}{line}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    if old_indent.strip_prefix(matched_indent).is_some() {
        let remove = old_indent.len() - matched_indent.len();
        return new_str
            .lines()
            .map(|line| {
                if line.len() >= remove
                    && line.as_bytes()[..remove]
                        .iter()
                        .all(u8::is_ascii_whitespace)
                {
                    line[remove..].to_string()
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    new_str.to_string()
}

fn leading_whitespace(line: &str) -> &str {
    let trimmed_len = line.len() - line.trim_start().len();
    &line[..trimmed_len]
}

/// Result of executing a tool: the text observation to feed back to the
/// model, plus an optional pending-edit record when the tool wrote to the
/// sandbox (so the caller can persist/emit it for the diff-review UI).
pub struct ToolOutcome {
    pub observation: String,
    /// (target_path, sandbox_path, diff, is_new_file, already_applied)
    pub pending_edit: Option<(String, String, String, bool, bool)>,
}

fn ok(observation: impl Into<String>) -> ToolOutcome {
    ToolOutcome {
        observation: observation.into(),
        pending_edit: None,
    }
}

pub async fn execute_tool(
    name: &str,
    args: &Value,
    project_root: Option<&Path>,
    extra_read_paths: &[String],
    background_jobs: &super::background::BackgroundJobs,
    mcp_clients: &crate::mcp::McpClients,
    app_data_dir: &Path,
    execution_mode: &crate::models::ExecutionMode,
) -> Result<ToolOutcome> {
    match name {
        "web_search" => {
            let queries: Vec<String> = args["queries"]
                .as_array()
                .ok_or_else(|| anyhow!("queries obrigatorio (array de strings)"))?
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .filter(|s| !s.trim().is_empty())
                .collect();
            if queries.is_empty() {
                return Err(anyhow!("queries obrigatorio (array de strings, pelo menos uma)"));
            }
            Ok(ok(websearch::search_many(app_data_dir, &queries).await?))
        }
        "web_fetch" => {
            let url = args["url"]
                .as_str()
                .ok_or_else(|| anyhow!("url obrigatorio"))?;
            Ok(ok(websearch::fetch(url).await?))
        }
        _ if name.starts_with("mcp__") => Ok(ok(mcp_clients.call(name, args.clone()).await?)),
        _ => {
            let project_root = project_root.ok_or_else(|| {
                anyhow!("esta ferramenta precisa de uma pasta de projeto associada a sessao")
            })?;
            let mut extended_paths = extra_read_paths.to_vec();
            extended_paths.push(app_data_dir.to_string_lossy().to_string());
            execute_project_tool(name, args, project_root, &extended_paths, background_jobs, execution_mode).await
        }
    }
}

async fn execute_project_tool(
    name: &str,
    args: &Value,
    project_root: &Path,
    extra_read_paths: &[String],
    background_jobs: &super::background::BackgroundJobs,
    execution_mode: &crate::models::ExecutionMode,
) -> Result<ToolOutcome> {
    match name {
        "read_file" => {
            let rel = args["path"]
                .as_str()
                .ok_or_else(|| anyhow!("path obrigatorio"))?;
            let path = resolve_read_path(project_root, extra_read_paths, rel)?;
            let content = read_project_file(&path, rel)?;
            let offset = args["offset"].as_u64().map(|n| n as usize);
            let limit = args["limit"].as_u64().map(|n| n as usize);
            let all_lines: Vec<&str> = content.lines().collect();
            let total_lines = all_lines.len();
            let start = offset.unwrap_or(0).min(total_lines);
            let default_limit = 2000;
            let end = (start + limit.unwrap_or(default_limit)).min(total_lines);
            let slice = all_lines[start..end].join("\n");
            let header = if offset.is_some() || limit.is_some() {
                format!("[linhas {}-{} de {total_lines}]\n", start + 1, end)
            } else if total_lines > default_limit {
                format!("[mostrando linhas 1-{end} de {total_lines} — use offset+limit pra ler o resto]\n")
            } else {
                String::new()
            };
            Ok(ok(format!("{header}{slice}")))
        }
        "list_dir" => {
            let rel = args["path"].as_str().unwrap_or("");
            let path = resolve_read_path(project_root, extra_read_paths, rel)?;
            let mut entries = Vec::new();
            for entry in std::fs::read_dir(&path)? {
                let entry = entry?;
                let ty = if entry.file_type()?.is_dir() {
                    "dir"
                } else {
                    "file"
                };
                entries.push(format!("{ty}\t{}", entry.file_name().to_string_lossy()));
            }
            entries.sort();
            Ok(ok(entries.join("\n")))
        }
        "grep" => {
            let pattern = args["pattern"]
                .as_str()
                .ok_or_else(|| anyhow!("pattern obrigatorio"))?;
            let rel = args["path"].as_str().unwrap_or("");
            let search_root = resolve_read_path(project_root, extra_read_paths, rel)?;
            let matches = grep_search(pattern, &search_root, project_root)?;
            if matches.is_empty() {
                Ok(ok("nenhuma ocorrencia encontrada"))
            } else {
                Ok(ok(matches.join("\n")))
            }
        }
        "run_command" => {
            let command = args["command"]
                .as_str()
                .ok_or_else(|| anyhow!("command obrigatorio"))?;
            if args["background"].as_bool().unwrap_or(false) {
                let id = background_jobs.start(project_root, command)?;
                return Ok(ok(format!(
                    "Comando iniciado em segundo plano com id {id} (nao esperou terminar). Use \
                     check_background_output({{\"id\": \"{id}\"}}) pra ver o progresso, e \
                     stop_background({{\"id\": \"{id}\"}}) quando nao precisar mais dele."
                )));
            }
            let output = Command::new("cmd")
                .arg("/C")
                .arg(command)
                .current_dir(project_root)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let result = format!(
                "exit_code: {}\nstdout:\n{}\nstderr:\n{}",
                output.status.code().unwrap_or(-1),
                truncate(&stdout, 8000),
                truncate(&stderr, 4000)
            );
            // Comando arbitrario pode ter criado/apagado arquivos reais;
            // descarta o cache de travessia pra proxima busca ver o estado atual.
            super::walk_cache::invalidate(project_root);
            Ok(ok(result))
        }
        "check_background_output" => {
            let id = args["id"]
                .as_str()
                .ok_or_else(|| anyhow!("id obrigatorio"))?;
            let output = background_jobs.read_output(id)?;
            // So invalida se o processo ja encerrou - enquanto ainda esta
            // rodando, o TTL de 1s do walk_cache ja da conta sozinho, e
            // invalidar a cada poll seria trabalho a toa pro modelo checar
            // progresso repetidamente.
            if output.contains("status: encerrado") {
                super::walk_cache::invalidate(project_root);
            }
            Ok(ok(output))
        }
        "stop_background" => {
            let id = args["id"]
                .as_str()
                .ok_or_else(|| anyhow!("id obrigatorio"))?;
            let result = background_jobs.stop(id).await?;
            super::walk_cache::invalidate(project_root);
            Ok(ok(result))
        }
        "list_background" => Ok(ok(background_jobs.list())),
        "create_excel" => {
            let rel = args["path"]
                .as_str()
                .ok_or_else(|| anyhow!("path obrigatorio"))?;
            let target = resolve_path(project_root, rel)?;
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut workbook = rust_xlsxwriter::Workbook::new();
            let sheets = args["sheets"]
                .as_array()
                .ok_or_else(|| anyhow!("sheets obrigatorio (array)"))?;
            for (si, sheet_def) in sheets.iter().enumerate() {
                let sheet_name = sheet_def["name"].as_str().unwrap_or("Sheet1");
                let worksheet = if si == 0 {
                    workbook.worksheet_from_index(0)?
                } else {
                    workbook.add_worksheet()
                };
                worksheet.set_name(sheet_name)?;
                let headers = sheet_def["headers"]
                    .as_array()
                    .ok_or_else(|| anyhow!("sheet '{sheet_name}': headers obrigatorio"))?;
                let bold = rust_xlsxwriter::Format::new().set_bold();
                for (ci, h) in headers.iter().enumerate() {
                    worksheet.write_with_format(0, ci as u16, h.as_str().unwrap_or(""), &bold)?;
                }
                let freeze = sheet_def["freeze_header"].as_bool().unwrap_or(true);
                if freeze {
                    worksheet.set_freeze_panes(1, 0)?;
                }
                let auto_filter = sheet_def["auto_filter"].as_bool().unwrap_or(true);
                if let Some(rows) = sheet_def["rows"].as_array() {
                    for (ri, row) in rows.iter().enumerate() {
                        if let Some(cells) = row.as_array() {
                            for (ci, cell) in cells.iter().enumerate() {
                                worksheet.write(ri as u32 + 1, ci as u16, cell.as_str().unwrap_or(""))?;
                            }
                        }
                    }
                    if auto_filter && !headers.is_empty() && !rows.is_empty() {
                        let last_col = (headers.len() - 1) as u16;
                        let last_row = rows.len() as u32;
                        worksheet.autofilter(0, 0, last_row, last_col)?;
                    }
                }
                if let Some(widths) = sheet_def["column_widths"].as_array() {
                    for (ci, w) in widths.iter().enumerate() {
                        if let Some(width) = w.as_f64() {
                            worksheet.set_column_width(ci as u16, width)?;
                        }
                    }
                }
            }
            workbook.save(&target)?;
            Ok(ok(format!("Arquivo Excel criado: {}", target.display())))
        }
        "create_word" => {
            use docx_rust::document::{Paragraph, Table, TableCell, TableRow};
            use docx_rust::Docx;

            let rel = args["path"]
                .as_str()
                .ok_or_else(|| anyhow!("path obrigatorio"))?;
            let target = resolve_path(project_root, rel)?;
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let elements = args["elements"]
                .as_array()
                .ok_or_else(|| anyhow!("elements obrigatorio (array)"))?;

            let mut docx = Docx::default();
            for el in elements {
                match el["type"].as_str().unwrap_or("") {
                    "heading" => {
                        let text = el["text"].as_str().unwrap_or("");
                        docx.document.push(Paragraph::default().push_text(text));
                    }
                    "paragraph" => {
                        let text = el["text"].as_str().unwrap_or("");
                        docx.document.push(Paragraph::default().push_text(text));
                    }
                    "table" => {
                        let mut table = Table::default();
                        if let Some(headers) = el["headers"].as_array() {
                            let mut row = TableRow::default();
                            for h in headers {
                                row = row.push_cell(TableCell::paragraph(
                                    Paragraph::default().push_text(h.as_str().unwrap_or("")),
                                ));
                            }
                            table = table.push_row(row);
                        }
                        if let Some(rows) = el["rows"].as_array() {
                            for r in rows {
                                let mut row = TableRow::default();
                                if let Some(cells) = r.as_array() {
                                    for c in cells {
                                        row = row.push_cell(TableCell::paragraph(
                                            Paragraph::default().push_text(c.as_str().unwrap_or("")),
                                        ));
                                    }
                                }
                                table = table.push_row(row);
                            }
                        }
                        docx.document.push(table);
                    }
                    "list" => {
                        if let Some(items) = el["items"].as_array() {
                            for item in items {
                                let text = item.as_str().unwrap_or("");
                                docx.document.push(Paragraph::default().push_text(format!("• {text}")));
                            }
                        }
                    }
                    _ => {}
                }
            }
            docx.write_file(&target)
                .map_err(|e| anyhow!("falha ao salvar docx: {e}"))?;
            Ok(ok(format!("Documento Word criado: {}", target.display())))
        }
        "create_pdf" => {
            use printpdf::*;
            use std::io::BufWriter;

            let rel = args["path"]
                .as_str()
                .ok_or_else(|| anyhow!("path obrigatorio"))?;
            let target = resolve_path(project_root, rel)?;
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let elements = args["elements"]
                .as_array()
                .ok_or_else(|| anyhow!("elements obrigatorio (array)"))?;

            let (doc, page1, layer1) = PdfDocument::new(
                args["title"].as_str().unwrap_or("Documento"),
                Mm(210.0),
                Mm(297.0),
                "Page 1",
            );
            let layer = doc.get_page(page1).get_layer(layer1);
            let font = doc.add_builtin_font(BuiltinFont::Helvetica)?;
            let font_bold = doc.add_builtin_font(BuiltinFont::HelveticaBold)?;

            let mut y: f64 = 277.0;
            let left_margin = 20.0;
            let line_height = 6.0;

            for el in elements {
                if y < 20.0 {
                    break;
                }
                match el["type"].as_str().unwrap_or("") {
                    "heading" => {
                        let text = el["text"].as_str().unwrap_or("");
                        layer.use_text(text, 16.0, Mm(left_margin), Mm(y), &font_bold);
                        y -= line_height * 1.8;
                    }
                    "paragraph" => {
                        let text = el["text"].as_str().unwrap_or("");
                        for line in text.lines() {
                            if y < 20.0 { break; }
                            layer.use_text(line, 11.0, Mm(left_margin), Mm(y), &font);
                            y -= line_height;
                        }
                        y -= line_height * 0.5;
                    }
                    "table" => {
                        let headers: Vec<&str> = el["headers"]
                            .as_array()
                            .map(|hs| hs.iter().map(|h| h.as_str().unwrap_or("")).collect())
                            .unwrap_or_default();
                        let rows: Vec<Vec<&str>> = el["rows"]
                            .as_array()
                            .map(|rs| {
                                rs.iter()
                                    .map(|r| {
                                        r.as_array()
                                            .map(|cs| cs.iter().map(|c| c.as_str().unwrap_or("")).collect())
                                            .unwrap_or_default()
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        let col_count = headers.len().max(rows.first().map(|r| r.len()).unwrap_or(0));
                        if col_count > 0 {
                            let col_width = 170.0 / col_count as f64;
                            if !headers.is_empty() {
                                for (ci, h) in headers.iter().enumerate() {
                                    layer.use_text(*h, 10.0, Mm(left_margin + ci as f64 * col_width), Mm(y), &font_bold);
                                }
                                y -= line_height * 1.5;
                            }
                            for row in &rows {
                                if y < 20.0 { break; }
                                for (ci, cell) in row.iter().enumerate() {
                                    layer.use_text(*cell, 10.0, Mm(left_margin + ci as f64 * col_width), Mm(y), &font);
                                }
                                y -= line_height;
                            }
                            y -= line_height * 0.5;
                        }
                    }
                    _ => {}
                }
            }

            let file = std::fs::File::create(&target)?;
            doc.save(&mut BufWriter::new(file))
                .map_err(|e| anyhow!("falha ao salvar pdf: {e}"))?;
            Ok(ok(format!("Documento PDF criado: {}", target.display())))
        }
        "write_file" => {
            let rel = args["path"]
                .as_str()
                .ok_or_else(|| anyhow!("path obrigatorio"))?;
            let content = args["content"]
                .as_str()
                .ok_or_else(|| anyhow!("content obrigatorio"))?;
            let target = resolve_path(project_root, rel)?;
            if *execution_mode == crate::models::ExecutionMode::Yolo {
                let (diff, is_new_file) = sandbox::write_direct(&target, content)?;
                Ok(ToolOutcome {
                    observation: format!("Arquivo escrito diretamente. Diff:\n{diff}"),
                    pending_edit: Some((
                        target.to_string_lossy().to_string(),
                        String::new(),
                        diff,
                        is_new_file,
                        true,
                    )),
                })
            } else {
                let (diff, is_new_file) = sandbox::write_sandboxed(project_root, &target, content)?;
                let sandbox_path = sandbox::to_sandbox_path(project_root, &target)?;
                Ok(ToolOutcome {
                    observation: format!(
                        "Alteracao escrita na sandbox (ainda NAO aplicada ao arquivo real). Diff:\n{diff}"
                    ),
                    pending_edit: Some((
                        target.to_string_lossy().to_string(),
                        sandbox_path.to_string_lossy().to_string(),
                        diff,
                        is_new_file,
                        false,
                    )),
                })
            }
        }
        "edit_file" => {
            let rel = args["path"]
                .as_str()
                .ok_or_else(|| anyhow!("path obrigatorio"))?;
            let old_str = args["old_str"]
                .as_str()
                .ok_or_else(|| anyhow!("old_str obrigatorio"))?;
            let new_str = args["new_str"]
                .as_str()
                .ok_or_else(|| anyhow!("new_str obrigatorio"))?;
            let target = resolve_path(project_root, rel)?;
            let original = sandbox::read_current_content(project_root, &target)?;
            let occurrences = original.matches(old_str).count();
            let new_content = match occurrences {
                1 => original.replacen(old_str, new_str, 1),
                0 => match find_edit_window(&original, old_str) {
                    FuzzyEditOutcome::Unique(start) => {
                        replace_line_window(&original, old_str, new_str, start)
                    }
                    FuzzyEditOutcome::Ambiguous(reason, n) => {
                        return Ok(ok(format!(
                            "old_str bate em {n} trechos diferentes de {rel} (ignorando {reason}). Ajuste o trecho para ser unico."
                        )));
                    }
                    FuzzyEditOutcome::NotFound(best_score) => {
                        let hint = match best_score {
                            Some(score) if score > 0.0 => format!(
                                " O trecho mais parecido encontrado tem {:.0}% de similaridade — confira se o old_str esta certo.",
                                score * 100.0
                            ),
                            _ => String::new(),
                        };
                        return Ok(ok(format!(
                            "old_str nao encontrado em {rel}, mesmo com fallback fuzzy (espaco/indentacao, aspas tipograficas/travessao, similaridade de texto).{hint}"
                        )));
                    }
                },
                n => {
                    return Ok(ok(format!(
                        "old_str encontrado {n} vezes em {rel} (precisa ser exatamente 1). Ajuste o trecho para ser unico."
                    )));
                }
            };
            if *execution_mode == crate::models::ExecutionMode::Yolo {
                let (diff, is_new_file) = sandbox::write_direct(&target, &new_content)?;
                Ok(ToolOutcome {
                    observation: format!("Arquivo editado diretamente. Diff:\n{diff}"),
                    pending_edit: Some((
                        target.to_string_lossy().to_string(),
                        String::new(),
                        diff,
                        is_new_file,
                        true,
                    )),
                })
            } else {
                let (diff, is_new_file) =
                    sandbox::write_sandboxed(project_root, &target, &new_content)?;
                let sandbox_path = sandbox::to_sandbox_path(project_root, &target)?;
                Ok(ToolOutcome {
                    observation: format!(
                        "Alteracao escrita na sandbox (ainda NAO aplicada ao arquivo real). Diff:\n{diff}"
                    ),
                    pending_edit: Some((
                        target.to_string_lossy().to_string(),
                        sandbox_path.to_string_lossy().to_string(),
                        diff,
                        is_new_file,
                        false,
                    )),
                })
            }
        }
        "ast_grep" => {
            let pattern = args["pattern"]
                .as_str()
                .ok_or_else(|| anyhow!("pattern obrigatorio"))?;
            let lang = args["language"]
                .as_str()
                .ok_or_else(|| anyhow!("language obrigatorio"))?;
            let subpath = args["path"].as_str().unwrap_or("");
            let search_root = resolve_read_path(project_root, extra_read_paths, subpath)?;
            Ok(ok(ast_tools::search(
                &search_root,
                project_root,
                pattern,
                lang,
            )?))
        }
        "ast_edit" => {
            let rel = args["path"]
                .as_str()
                .ok_or_else(|| anyhow!("path obrigatorio"))?;
            let pattern = args["pattern"]
                .as_str()
                .ok_or_else(|| anyhow!("pattern obrigatorio"))?;
            let rewrite = args["rewrite"]
                .as_str()
                .ok_or_else(|| anyhow!("rewrite obrigatorio"))?;
            let lang = args["language"]
                .as_str()
                .ok_or_else(|| anyhow!("language obrigatorio"))?;
            let target = resolve_path(project_root, rel)?;
            let original = sandbox::read_current_content(project_root, &target)?;
            let new_content = match ast_tools::rewrite_file(&original, pattern, rewrite, lang) {
                Ok(c) => c,
                Err(e) => return Ok(ok(format!("nao foi possivel reescrever: {e}"))),
            };
            if *execution_mode == crate::models::ExecutionMode::Yolo {
                let (diff, is_new_file) = sandbox::write_direct(&target, &new_content)?;
                Ok(ToolOutcome {
                    observation: format!("Alteracao estrutural aplicada diretamente. Diff:\n{diff}"),
                    pending_edit: Some((
                        target.to_string_lossy().to_string(),
                        String::new(),
                        diff,
                        is_new_file,
                        true,
                    )),
                })
            } else {
                let (diff, is_new_file) =
                    sandbox::write_sandboxed(project_root, &target, &new_content)?;
                let sandbox_path = sandbox::to_sandbox_path(project_root, &target)?;
                Ok(ToolOutcome {
                    observation: format!(
                        "Alteracao estrutural escrita na sandbox (ainda NAO aplicada ao arquivo real). Diff:\n{diff}"
                    ),
                    pending_edit: Some((
                        target.to_string_lossy().to_string(),
                        sandbox_path.to_string_lossy().to_string(),
                        diff,
                        is_new_file,
                        false,
                    )),
                })
            }
        }
        // "task" (subagente) e tratado a parte em agent::mod::run_turn, igual
        // "load_skill" - precisa de app/estado/provider que essa funcao nao
        // tem, entao nunca chega aqui de verdade.
        other => Err(anyhow!("ferramenta desconhecida: {other}")),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}...[truncado]", &s[..max])
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn scratch_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cerne-grep-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn grep_search_finds_matches_with_line_numbers() {
        let dir = scratch_dir();
        fs::write(dir.join("a.rs"), "fn main() {\n    let x = 1;\n}\n").unwrap();
        fs::write(dir.join("b.txt"), "nothing here\n").unwrap();
        let matches = grep_search(r"let \w+", &dir, &dir).unwrap();
        assert_eq!(matches.len(), 1);
        assert!(matches[0].starts_with("a.rs:2:"));
        assert!(matches[0].contains("let x = 1;"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn grep_search_skips_binary_files() {
        let dir = scratch_dir();
        fs::write(dir.join("bin.dat"), [0x00u8, 0x01, b'a', b'b', b'c']).unwrap();
        fs::write(dir.join("text.txt"), "abc match here\n").unwrap();
        let matches = grep_search("abc", &dir, &dir).unwrap();
        assert_eq!(matches.len(), 1);
        assert!(matches[0].starts_with("text.txt:"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn grep_search_finds_accented_pattern_in_windows_1252_file() {
        let dir = scratch_dir();
        // "descricao = café" em Windows-1252 puro: 'é' e o byte 0xE9 sozinho,
        // nao os 2 bytes UTF-8 (0xC3 0xA9) que o padrao abaixo usa.
        fs::write(dir.join("config.ini"), [b'c', b'a', b'f', 0xE9, b'\n']).unwrap();
        let matches = grep_search("café", &dir, &dir).unwrap();
        assert_eq!(
            matches.len(),
            1,
            "deveria achar 'cafe' com acento mesmo com o arquivo em Windows-1252 cru"
        );
        assert!(matches[0].starts_with("config.ini:1:"));
    }

    #[test]
    fn grep_search_no_matches_returns_empty() {
        let dir = scratch_dir();
        fs::write(dir.join("a.txt"), "hello world\n").unwrap();
        let matches = grep_search("zzz_not_found", &dir, &dir).unwrap();
        assert!(matches.is_empty());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn find_trimmed_line_windows_ignores_indentation_drift() {
        let content = "fn main() {\n        let x = 1;\n}\n";
        // old_str do modelo com indentacao diferente (4 espacos) da do arquivo (8 espacos).
        let old_str = "    let x = 1;";
        let windows = find_trimmed_line_windows(content, old_str);
        assert_eq!(windows, vec![1]);
    }

    #[test]
    fn find_trimmed_line_windows_empty_when_content_really_differs() {
        let content = "fn main() {\n    let x = 1;\n}\n";
        let windows = find_trimmed_line_windows(content, "    let y = 2;");
        assert!(windows.is_empty());
    }

    #[test]
    fn find_trimmed_line_windows_reports_all_ambiguous_matches() {
        let content = "  let x = 1;\n  let x = 1;\n";
        let windows = find_trimmed_line_windows(content, "let x = 1;");
        assert_eq!(windows, vec![0, 1]);
    }

    #[test]
    fn reindent_replacement_adds_missing_indent() {
        let old_str = "  let x = 1;";
        let matched_block = "      let x = 1;";
        let new_str = "  let x = 2;";
        assert_eq!(
            reindent_replacement(old_str, matched_block, new_str),
            "      let x = 2;"
        );
    }

    #[test]
    fn reindent_replacement_removes_extra_indent() {
        let old_str = "      let x = 1;";
        let matched_block = "  let x = 1;";
        let new_str = "      let x = 2;";
        assert_eq!(
            reindent_replacement(old_str, matched_block, new_str),
            "  let x = 2;"
        );
    }

    #[test]
    fn reindent_replacement_leaves_new_str_unchanged_when_indent_matches() {
        let old_str = "  let x = 1;";
        let matched_block = "  let x = 1;";
        let new_str = "  let x = 2;";
        assert_eq!(
            reindent_replacement(old_str, matched_block, new_str),
            new_str
        );
    }

    #[test]
    fn find_unicode_normalized_windows_matches_curly_quotes_and_dash() {
        let content = "let title = \u{201C}hello\u{201D};\nlet range = 1\u{2013}10;\n";
        let pattern = "let title = \"hello\";"; // aspas retas, modelo "esqueceu" que o arquivo usa curvas
        let windows = find_unicode_normalized_windows(content, pattern);
        assert_eq!(windows, vec![0]);
        // Ja resolvido no nivel 1 (trim), entao nivel 2 nao devia ser nem chamado nesse caso -
        // mas testado isoladamente confirma que a normalizacao em si funciona.
    }

    #[test]
    fn text_similarity_of_identical_strings_is_one() {
        assert_eq!(text_similarity("abc", "abc"), 1.0);
    }

    #[test]
    fn text_similarity_drops_with_edit_distance() {
        let close = text_similarity("let x = 1;", "let x = 2;");
        let far = text_similarity("let x = 1;", "totally different content here");
        assert!(
            close > 0.8,
            "uma troca de 1 char deveria ficar bem similar: {close}"
        );
        assert!(
            far < 0.5,
            "strings bem diferentes deveriam ficar pouco similares: {far}"
        );
    }

    #[test]
    fn fuzzy_window_scores_accepts_single_high_similarity_window() {
        let content = "fn main() {\n    let result = compute_total(a, b);\n}\n";
        // Typo plausivel de um modelo mais fraco: "compute_totals" em vez de "compute_total".
        let pattern = "let result = compute_totals(a, b);";
        let scores = fuzzy_window_scores(content, pattern);
        assert_eq!(scores.above_threshold, 1);
        assert_eq!(scores.best_index, Some(1));
    }

    #[test]
    fn fuzzy_window_scores_rejects_when_nothing_similar_enough() {
        let content = "fn main() {}\n";
        let scores = fuzzy_window_scores(content, "let totally_unrelated_thing = 42;");
        assert_eq!(scores.above_threshold, 0);
    }

    #[tokio::test]
    async fn edit_file_falls_back_to_unicode_normalized_match() {
        let dir = scratch_dir();
        // Arquivo real com aspas tipograficas (comum apos passar por
        // renderizacao markdown/editor "esperto").
        fs::write(dir.join("a.txt"), "let title = \u{201C}hello\u{201D};\n").unwrap();
        let args = json!({
            "path": "a.txt",
            "old_str": "let title = \"hello\";",
            "new_str": "let title = \"bye\";",
        });
        let outcome = execute_project_tool(
            "edit_file",
            &args,
            &dir,
            &[],
            &crate::agent::background::BackgroundJobs::default(),
            &crate::models::ExecutionMode::Auto,
        )
        .await
        .unwrap();
        assert!(
            outcome.pending_edit.is_some(),
            "esperava sucesso, recebeu: {}",
            outcome.observation
        );
        let (_, sandbox_path, _, _, _) = outcome.pending_edit.unwrap();
        let sandboxed = fs::read_to_string(sandbox_path).unwrap();
        assert!(sandboxed.contains("bye"));
        fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn edit_file_falls_back_to_fuzzy_similarity_match() {
        let dir = scratch_dir();
        fs::write(
            dir.join("a.rs"),
            "fn main() {\n    let result = compute_total(a, b);\n}\n",
        )
        .unwrap();
        let args = json!({
            "path": "a.rs",
            "old_str": "let result = compute_totals(a, b);", // typo: "totals" em vez de "total"
            "new_str": "let result = compute_total(a, b) * 2;",
        });
        let outcome = execute_project_tool(
            "edit_file",
            &args,
            &dir,
            &[],
            &crate::agent::background::BackgroundJobs::default(),
            &crate::models::ExecutionMode::Auto,
        )
        .await
        .unwrap();
        assert!(
            outcome.pending_edit.is_some(),
            "esperava sucesso via fuzzy, recebeu: {}",
            outcome.observation
        );
        let (_, sandbox_path, _, _, _) = outcome.pending_edit.unwrap();
        let sandboxed = fs::read_to_string(sandbox_path).unwrap();
        assert!(sandboxed.contains("compute_total(a, b) * 2;"));
        fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn edit_file_not_found_message_includes_closest_similarity_hint() {
        let dir = scratch_dir();
        fs::write(dir.join("a.rs"), "fn main() {}\n").unwrap();
        let args = json!({
            "path": "a.rs",
            "old_str": "let totally_unrelated_thing_not_in_file = 42;",
            "new_str": "x",
        });
        let outcome = execute_project_tool(
            "edit_file",
            &args,
            &dir,
            &[],
            &crate::agent::background::BackgroundJobs::default(),
            &crate::models::ExecutionMode::Auto,
        )
        .await
        .unwrap();
        assert!(outcome.pending_edit.is_none());
        assert!(outcome.observation.contains("nao encontrado"));
        fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn edit_file_falls_back_to_trimmed_match_on_indentation_drift() {
        let dir = scratch_dir();
        fs::write(dir.join("a.rs"), "fn main() {\n        let x = 1;\n}\n").unwrap();
        let args = json!({
            "path": "a.rs",
            "old_str": "    let x = 1;",
            "new_str": "    let x = 2;",
        });
        let outcome = execute_project_tool(
            "edit_file",
            &args,
            &dir,
            &[],
            &crate::agent::background::BackgroundJobs::default(),
            &crate::models::ExecutionMode::Auto,
        )
        .await
        .unwrap();
        assert!(
            outcome.observation.contains("Diff"),
            "esperava sucesso com diff, recebeu: {}",
            outcome.observation
        );
        assert!(outcome.pending_edit.is_some());
        let (_, sandbox_path, _, _, _) = outcome.pending_edit.unwrap();
        let sandboxed = fs::read_to_string(sandbox_path).unwrap();
        assert!(
            sandboxed.contains("        let x = 2;"),
            "deveria reindentar pro nivel real do bloco: {sandboxed}"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn edit_file_errors_when_old_str_not_found_even_fuzzy() {
        let dir = scratch_dir();
        fs::write(dir.join("a.rs"), "fn main() {}\n").unwrap();
        let args = json!({
            "path": "a.rs",
            "old_str": "let totally_missing = 1;",
            "new_str": "let x = 2;",
        });
        let outcome = execute_project_tool(
            "edit_file",
            &args,
            &dir,
            &[],
            &crate::agent::background::BackgroundJobs::default(),
            &crate::models::ExecutionMode::Auto,
        )
        .await
        .unwrap();
        assert!(
            outcome.observation.contains("nao encontrado"),
            "esperava erro claro, recebeu: {}",
            outcome.observation
        );
        assert!(outcome.pending_edit.is_none());
        fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn edit_file_preserves_utf16le_encoding_on_write() {
        let dir = scratch_dir();
        let mut original_bytes = vec![0xFFu8, 0xFE]; // BOM UTF-16LE
        original_bytes.extend(
            "let x = café;\n"
                .encode_utf16()
                .flat_map(|u| u.to_le_bytes()),
        );
        fs::write(dir.join("a.txt"), &original_bytes).unwrap();

        let args = json!({
            "path": "a.txt",
            "old_str": "café",
            "new_str": "cha",
        });
        let outcome = execute_project_tool(
            "edit_file",
            &args,
            &dir,
            &[],
            &crate::agent::background::BackgroundJobs::default(),
            &crate::models::ExecutionMode::Auto,
        )
        .await
        .unwrap();
        assert!(
            outcome.pending_edit.is_some(),
            "esperava sucesso, recebeu: {}",
            outcome.observation
        );
        let (_, sandbox_path, _, _, _) = outcome.pending_edit.unwrap();

        let written_bytes = fs::read(&sandbox_path).unwrap();
        assert_eq!(
            &written_bytes[..2],
            &[0xFF, 0xFE],
            "deveria manter o BOM UTF-16LE, nao virar UTF-8"
        );
        let (decoded, _) = crate::encoding::decode(&written_bytes);
        assert_eq!(decoded, "let x = cha;\n");
        fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn edit_file_preserves_windows_1252_encoding_on_write() {
        let dir = scratch_dir();
        // "café" em Windows-1252: 'c','a','f',0xE9 (nao decodifica como UTF-8 valido).
        fs::write(dir.join("a.txt"), [b'c', b'a', b'f', 0xE9, b'\n']).unwrap();

        let args = json!({
            "path": "a.txt",
            "old_str": "caf",
            "new_str": "bar",
        });
        let outcome = execute_project_tool(
            "edit_file",
            &args,
            &dir,
            &[],
            &crate::agent::background::BackgroundJobs::default(),
            &crate::models::ExecutionMode::Auto,
        )
        .await
        .unwrap();
        assert!(
            outcome.pending_edit.is_some(),
            "esperava sucesso, recebeu: {}",
            outcome.observation
        );
        let (_, sandbox_path, _, _, _) = outcome.pending_edit.unwrap();

        let written_bytes = fs::read(&sandbox_path).unwrap();
        assert_eq!(
            written_bytes,
            [b'b', b'a', b'r', 0xE9, b'\n'],
            "deveria continuar em Windows-1252 (byte 0xE9 cru), nao converter pra UTF-8"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn run_command_background_roundtrip_via_execute_project_tool() {
        let dir = scratch_dir();
        let background_jobs = crate::agent::background::BackgroundJobs::default();

        let start_args = json!({ "command": "echo from-tool-dispatch", "background": true });
        let outcome = execute_project_tool("run_command", &start_args, &dir, &[], &background_jobs, &crate::models::ExecutionMode::Auto)
            .await
            .unwrap();
        assert!(
            outcome.observation.contains("segundo plano"),
            "esperava confirmacao de inicio, recebeu: {}",
            outcome.observation
        );

        // Extrai o id da mensagem (formato: "...id {uuid} (nao esperou...").
        let id = outcome
            .observation
            .split("id ")
            .nth(1)
            .and_then(|s| s.split_whitespace().next())
            .expect("mensagem deveria conter o id")
            .to_string();

        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        let check_args = json!({ "id": id });
        let checked = execute_project_tool(
            "check_background_output",
            &check_args,
            &dir,
            &[],
            &background_jobs,
            &crate::models::ExecutionMode::Auto,
        )
        .await
        .unwrap();
        assert!(
            checked.observation.contains("from-tool-dispatch"),
            "esperava ver o output: {}",
            checked.observation
        );

        let listed =
            execute_project_tool("list_background", &json!({}), &dir, &[], &background_jobs, &crate::models::ExecutionMode::Auto)
                .await
                .unwrap();
        assert!(listed.observation.contains(&id));

        let stopped =
            execute_project_tool("stop_background", &check_args, &dir, &[], &background_jobs, &crate::models::ExecutionMode::Auto)
                .await
                .unwrap();
        assert!(stopped.observation.contains("encerrado"));

        fs::remove_dir_all(&dir).ok();
    }

    /// Regressao de um bug real encontrado testando o sub-agente ao vivo:
    /// pedi pra adicionar docstring em 3 funcoes do mesmo arquivo, e so a
    /// ultima sobreviveu depois de aceitar as 3 - cada `edit_file` lia o
    /// arquivo REAL (que nunca muda ate o aceite), entao a segunda edicao
    /// nao via a primeira. Corrigido em `sandbox::read_current_content`
    /// (prefere a sandbox, se ja existir uma edicao anterior pendente).
    #[tokio::test]
    async fn edit_file_chains_on_top_of_a_previous_unaccepted_edit_to_the_same_file() {
        let dir = scratch_dir();
        let background_jobs = crate::agent::background::BackgroundJobs::default();
        fs::write(
            dir.join("utils.py"),
            "def add(a, b):\n    return a + b\n\n\ndef subtract(a, b):\n    return a - b\n",
        )
        .unwrap();

        let first = json!({
            "path": "utils.py",
            "old_str": "def add(a, b):\n    return a + b",
            "new_str": "def add(a, b):\n    \"\"\"Soma dois numeros.\"\"\"\n    return a + b",
        });
        let outcome1 = execute_project_tool("edit_file", &first, &dir, &[], &background_jobs, &crate::models::ExecutionMode::Auto)
            .await
            .unwrap();
        assert!(
            outcome1.pending_edit.is_some(),
            "primeira edicao deveria ter sucesso: {}",
            outcome1.observation
        );
        let (_, sandbox_path_1, _, _, _) = outcome1.pending_edit.unwrap();

        let second = json!({
            "path": "utils.py",
            "old_str": "def subtract(a, b):\n    return a - b",
            "new_str": "def subtract(a, b):\n    \"\"\"Subtrai dois numeros.\"\"\"\n    return a - b",
        });
        let outcome2 = execute_project_tool("edit_file", &second, &dir, &[], &background_jobs, &crate::models::ExecutionMode::Auto)
            .await
            .unwrap();
        assert!(
            outcome2.pending_edit.is_some(),
            "segunda edicao deveria ter sucesso: {}",
            outcome2.observation
        );
        let (_, sandbox_path_2, _, _, _) = outcome2.pending_edit.unwrap();

        // Invariante que torna a correcao suficiente mesmo sem colapsar as
        // entradas de pending-edit na UI: `to_sandbox_path` e deterministico
        // por arquivo, entao as duas edicoes apontam pro MESMO arquivo de
        // sandbox — aceitar qualquer uma das 2 entradas mostradas na
        // interface aplica o mesmo conteudo cumulativo mais recente.
        assert_eq!(
            sandbox_path_1, sandbox_path_2,
            "edicoes no mesmo arquivo deveriam compartilhar o mesmo caminho de sandbox"
        );

        // A sandbox depois da 2a edicao deveria ter AS DUAS docstrings, nao
        // so a da 2a - senao aceitar essa (a mais recente) perderia a 1a.
        let final_sandboxed = fs::read_to_string(sandbox_path_2).unwrap();
        assert!(
            final_sandboxed.contains("Soma dois numeros"),
            "deveria manter a docstring da 1a edicao: {final_sandboxed}"
        );
        assert!(
            final_sandboxed.contains("Subtrai dois numeros"),
            "deveria ter a docstring da 2a edicao: {final_sandboxed}"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn execute_tool_routes_mcp_prefixed_names_to_mcp_clients() {
        // Sem servidor MCP de verdade conectado, a chamada deveria falhar
        // com um erro claro vindo do McpClients (nao "ferramenta
        // desconhecida") - confirma que o roteamento por prefixo funciona,
        // mesmo sem exercitar uma conexao real (isso e testado ao vivo).
        let background_jobs = crate::agent::background::BackgroundJobs::default();
        let mcp_clients = crate::mcp::McpClients::default();
        let result = execute_tool(
            "mcp__github__search_issues",
            &json!({}),
            None,
            &[],
            &background_jobs,
            &mcp_clients,
            Path::new("."),
            &crate::models::ExecutionMode::Auto,
        )
        .await;
        match result {
            Err(e) => assert!(
                e.to_string().contains("nao conectado"),
                "esperava erro de servidor MCP nao conectado, recebeu: {e}"
            ),
            Ok(_) => panic!("esperava erro (servidor MCP nao conectado), recebeu sucesso"),
        }
    }

    #[tokio::test]
    async fn read_file_accepts_absolute_path_inside_configured_extra_root() {
        let project = scratch_dir();
        let extra = scratch_dir();
        fs::write(extra.join("notes.txt"), "conteudo de fora do projeto").unwrap();
        let background_jobs = crate::agent::background::BackgroundJobs::default();

        let extra_path = extra.join("notes.txt").to_string_lossy().to_string();
        let args = json!({ "path": extra_path });
        let outcome = execute_project_tool(
            "read_file",
            &args,
            &project,
            &[extra.to_string_lossy().to_string()],
            &background_jobs,
            &crate::models::ExecutionMode::Auto,
        )
        .await
        .unwrap();
        assert!(outcome.observation.contains("conteudo de fora do projeto"));

        fs::remove_dir_all(&project).ok();
        fs::remove_dir_all(&extra).ok();
    }

    #[tokio::test]
    async fn read_file_accepts_any_absolute_path() {
        let project = scratch_dir();
        let outsider = scratch_dir();
        fs::write(outsider.join("secret.txt"), "conteudo de fora").unwrap();
        let background_jobs = crate::agent::background::BackgroundJobs::default();

        let outsider_path = outsider.join("secret.txt").to_string_lossy().to_string();
        let args = json!({ "path": outsider_path });
        let result =
            execute_project_tool("read_file", &args, &project, &[], &background_jobs, &crate::models::ExecutionMode::Auto).await;
        assert!(
            result.is_ok(),
            "read_file deve aceitar qualquer caminho absoluto, recebeu erro: {:?}",
            result.err()
        );
        assert!(result.unwrap().observation.contains("conteudo de fora"));

        fs::remove_dir_all(&project).ok();
        fs::remove_dir_all(&outsider).ok();
    }

    #[tokio::test]
    async fn list_dir_and_grep_accept_absolute_path_inside_extra_root() {
        let project = scratch_dir();
        let extra = scratch_dir();
        fs::write(extra.join("a.txt"), "hello world\n").unwrap();
        fs::create_dir_all(extra.join("sub")).unwrap();
        let background_jobs = crate::agent::background::BackgroundJobs::default();
        let extra_roots = vec![extra.to_string_lossy().to_string()];

        let list_args = json!({ "path": extra.to_string_lossy().to_string() });
        let listed = execute_project_tool(
            "list_dir",
            &list_args,
            &project,
            &extra_roots,
            &background_jobs,
            &crate::models::ExecutionMode::Auto,
        )
        .await
        .unwrap();
        assert!(listed.observation.contains("a.txt"));
        assert!(listed.observation.contains("sub"));

        let grep_args = json!({ "pattern": "hello", "path": extra.to_string_lossy().to_string() });
        let grepped =
            execute_project_tool("grep", &grep_args, &project, &extra_roots, &background_jobs, &crate::models::ExecutionMode::Auto)
                .await
                .unwrap();
        assert!(grepped.observation.contains("a.txt"));

        fs::remove_dir_all(&project).ok();
        fs::remove_dir_all(&extra).ok();
    }

    #[tokio::test]
    async fn write_file_rejects_absolute_path_outside_project_root_even_with_extra_read_paths_configured(
    ) {
        // Ferramenta de ESCRITA nunca deve aceitar uma pasta extra de
        // leitura como destino - a sandbox so espelha o project_root, entao
        // nao ha onde deixar uma edicao "pendente de aceite" fora dele.
        let project = scratch_dir();
        let extra = scratch_dir();
        let background_jobs = crate::agent::background::BackgroundJobs::default();
        let extra_roots = vec![extra.to_string_lossy().to_string()];

        let target = extra.join("novo.txt").to_string_lossy().to_string();
        let args = json!({ "path": target, "content": "nao deveria escrever aqui" });
        let result = execute_project_tool(
            "write_file",
            &args,
            &project,
            &extra_roots,
            &background_jobs,
            &crate::models::ExecutionMode::Auto,
        )
        .await;
        assert!(
            result.is_err(),
            "write_file nao deveria aceitar caminho absoluto de uma pasta extra de leitura"
        );

        fs::remove_dir_all(&project).ok();
        fs::remove_dir_all(&extra).ok();
    }
}
