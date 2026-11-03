//! Native implementations of Context Mode's tools and slash commands.
use super::{chunk, execute, fetch, search, store::Store};
use crate::models::{Session, ToolFunctionSpec, ToolSpec};
use anyhow::{anyhow, Result};
use futures_util::{stream, StreamExt};
use rusqlite::params;
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    time::Instant,
};

pub fn specs(id: &str) -> Result<Vec<ToolSpec>> {
    let reference: Vec<Value> = serde_json::from_str(include_str!("reference-tools.json"))?;
    reference.into_iter().map(|tool| {
        let name = tool["name"].as_str().unwrap();
        let description = match name {
            "ctx_execute" => "Execute codigo em Rust/Boa para analisar dados sem despeja-los no contexto. JavaScript embutido inclui console, JSON, FILE_CONTENT, fs basico e child_process.execSync; nao e Node/npm. Shell usa o shell do sistema. Linguagens opcionais usam ferramentas instaladas. Saida completa e arquivada; intent seleciona trechos. Exige aprovacao conforme o modo do Cerne.",
            "ctx_execute_file" => "Analise um arquivo autorizado sem coloca-lo inteiro no contexto. FILE_CONTENT e fornecido ao JavaScript embutido e runtimes opcionais; Rust/Elixir tambem usam file_content. Apenas o que o codigo imprime e retornado; resultados grandes sao indexados antes da reducao.",
            "ctx_index" => "Indexe texto, JSON, Markdown, arquivo ou diretorio autorizado em SQLite FTS5. Preserve conteudo bruto completo, cercas de codigo, hierarquia e hashes. Diretorios respeitam filtros, gitignore, profundidade e limite de arquivos. Retorna metadados, nao o documento inteiro.",
            "ctx_search" => "Busque conteudo persistido com FTS5 Porter/BM25 + trigramas, RRF, proximidade e correcao de digitacao. queries permite agrupar consultas. Filtre por source e contentType; sort timeline e cronologico dentro desta conversa. Arquivos alterados sao atualizados na busca apos revalidacao de acesso; indisponiveis sao sinalizados. Trechos sao centrados nos termos. Limite de 8 chamadas por minuto; a partir de 3 chamadas, 1 resultado por consulta. Use lotes.",
            "ctx_fetch_and_index" => "Busque pagina HTTP(S) publica com DNS fixado, protecao SSRF e validacao de redirecionamentos. Converta HTML para Markdown, indexe integralmente e devolva metadados. Cache TTL em segundos; force ignora cache. requests agrupa URLs; concurrency limita paralelismo.",
            "ctx_batch_execute" => "Execute comandos de shell em lote com timeout e concorrencia limitada. Arquive stdout/stderr e pesquise queries somente nas fontes deste lote; query_scope global permite consultar todas as fontes desta conversa.",
            "ctx_stats" => "Estatisticas locais reais por ferramenta: chamadas, bytes arquivados/retornados/evitados, tokens estimados, erros, tempo de execucao e categorias de memoria.",
            "ctx_doctor" => "Diagnostico local do mecanismo Rust, SQLite, FTS5/trigramas, integridade do banco e migracao. Nao depende de Node, npm ou hooks de outros editores.",
            "ctx_purge" => "Limpe o indice ativo e os eventos SOMENTE desta conversa. confirm true e obrigatorio. Historico e copias antigas de migracao sao preservados; a proxima sincronizacao pode reconstruir o indice a partir do historico. Exige aprovacao conforme o modo do Cerne.",
            "ctx_insight" => "Relatorio local de economia, ferramentas e memoria. Funciona offline. O dashboard organizacional externo do Context Mode nao esta integrado e nenhum dado e enviado a ele.",
            _ => return Err(anyhow!("Ferramenta de referencia desconhecida: {name}")),
        };
        let mut parameters=tool["inputSchema"].clone();
        if name=="ctx_execute" || name=="ctx_execute_file" {
            parameters["properties"]["language"]["description"]=json!("JavaScript Boa embutido; shell do sistema; outros runtimes opcionais instalados. TypeScript usa Deno, C# usa dotnet-script. Nao instala dependencias.");
            parameters["properties"]["timeout"]["description"]=json!("Tempo limite total em ms, incluindo compilacao. Padrao 30000, intervalo 100 a 300000. Encerra a arvore de processos.");
        }
        if name=="ctx_execute" {
            parameters["properties"]["background"]["description"]=json!("Segundo plano somente para language shell, pelo gerenciador do Cerne. Use check_background_output / stop_background.");
            parameters["properties"]["cwd"]["description"]=json!("Diretorio autorizado para esta execucao; padrao e o projeto.");
        }
        if name=="ctx_purge" {
            parameters["properties"]["scope"]=json!({"type":"string","enum":["session"],"default":"session","description":"Somente o indice ativo desta conversa; historico e bancos antigos sao preservados."});
        }
        Ok(ToolSpec{kind:"function".into(),function:ToolFunctionSpec{name:super::tool_name(id,name),description:description.into(),parameters}})
    }).collect()
}

pub async fn database<T: Send + 'static>(
    data: &Path,
    id: &str,
    operation: impl FnOnce(&mut Store) -> Result<T> + Send + 'static,
) -> Result<T> {
    let data = data.to_path_buf();
    let id = id.to_string();
    tokio::task::spawn_blocking(move || operation(&mut Store::open(&data, &id)?)).await?
}
fn strings(value: &Value) -> Result<Vec<String>> {
    let owned = if let Some(text) = value.as_str() {
        if text.trim_start().starts_with('[') {
            serde_json::from_str(text)?
        } else {
            return Ok(vec![text.into()]);
        }
    } else {
        value.clone()
    };
    Ok(owned
        .as_array()
        .ok_or_else(|| anyhow!("Esperado uma lista"))?
        .iter()
        .map(|s| {
            s.as_str()
                .map(str::to_string)
                .ok_or_else(|| anyhow!("Esperado texto na lista"))
        })
        .collect::<Result<_>>()?)
}
fn boolean(value: &Value, default: bool) -> Result<bool> {
    if value.is_null() {
        return Ok(default);
    }
    value
        .as_bool()
        .or_else(|| match value.as_str() {
            Some("true") => Some(true),
            Some("false") => Some(false),
            _ => None,
        })
        .ok_or_else(|| anyhow!("Esperado true ou false"))
}
fn project(data: &Path, session: &Session) -> Result<PathBuf> {
    let path = session
        .project_root
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            data.join("sessions")
                .join(&session.id)
                .join("context-economy/workspace")
        });
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

fn index_paths(
    store: &mut Store,
    project: &Path,
    roots: &[PathBuf],
    args: &Value,
) -> Result<String> {
    if let Some(content) = args["content"].as_str() {
        if content.len() > execute::MAX_BYTES {
            return Err(anyhow!("Conteudo excede 50 MB"));
        }
        let source = args["source"].as_str().unwrap_or("indexed-content");
        let chunks = store.index(content, source, None)?;
        return Ok(format!(
            "Indexed {chunks} sections ({} bytes completos), source: {source}. Use ctx_search.",
            content.len()
        ));
    }
    let path = args["path"]
        .as_str()
        .ok_or_else(|| anyhow!("Informe content ou path"))?;
    let path = execute::resolve_read(project, roots, path)?;
    let source = args["source"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| path.to_string_lossy().into_owned());
    if path.is_file() {
        let content = execute::read_text(&path)?;
        let chunks = store.index(&content, &source, Some(&path.to_string_lossy()))?;
        return Ok(format!(
            "Indexed {chunks} sections, {} bytes completos, source: {source}",
            content.len()
        ));
    }
    if !path.is_dir() {
        return Err(anyhow!("Caminho nao e arquivo ou diretorio"));
    }
    let max_files = args["maxFiles"].as_u64().unwrap_or(200).clamp(1, 1000) as usize;
    let max_depth = args["maxDepth"].as_u64().unwrap_or(5).min(20) as usize;
    let respect = boolean(&args["respectGitignore"], true)?;
    let follow = boolean(&args["followSymlinks"], false)?;
    let mut overrides = ignore::overrides::OverrideBuilder::new(&path);
    for pattern in args
        .get("include")
        .map(strings)
        .transpose()?
        .unwrap_or_default()
    {
        overrides.add(&pattern)?;
    }
    for pattern in args
        .get("exclude")
        .map(strings)
        .transpose()?
        .unwrap_or_default()
    {
        overrides.add(&format!("!{pattern}"))?;
    }
    let extensions = args.get("extensions").map(strings).transpose()?.unwrap_or_else(||"md mdx txt json yaml yml toml ts tsx js jsx vue py rs go sh csv sql html css java c cpp h cs rb php".split_whitespace().map(str::to_owned).collect());
    let mut walker = ignore::WalkBuilder::new(&path);
    walker
        .hidden(false)
        .max_depth(Some(max_depth + 1))
        .follow_links(follow)
        .git_ignore(respect)
        .git_global(respect)
        .git_exclude(respect)
        .parents(respect)
        .overrides(overrides.build()?)
        .filter_entry(|entry| {
            ![
                "node_modules",
                ".git",
                "dist",
                "build",
                "target",
                "coverage",
                ".venv",
                "__pycache__",
                ".next",
            ]
            .iter()
            .any(|s| entry.file_name() == std::ffi::OsStr::new(s))
        });
    let mut files = 0;
    let mut chunks = 0;
    let mut bytes = 0;
    let mut skipped = 0;
    for entry in walker.build() {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        if !extensions.is_empty()
            && !entry.path().extension().is_some_and(|ext| {
                extensions.iter().any(|s| {
                    ext.to_string_lossy()
                        .eq_ignore_ascii_case(s.trim_start_matches('.'))
                })
            })
        {
            continue;
        }
        if files >= max_files {
            break;
        }
        let canonical = entry.path().canonicalize()?;
        if !canonical.starts_with(&path) {
            skipped += 1;
            continue;
        }
        let text = match execute::read_text(&canonical) {
            Ok(text) => text,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        let label = format!("{source}:{}", canonical.strip_prefix(&path)?.display());
        chunks += store.index(&text, &label, Some(&canonical.to_string_lossy()))?;
        bytes += text.len();
        files += 1;
    }
    Ok(format!("Indexed {files} files, {chunks} sections, {bytes} bytes completos; skipped {skipped}; maxFiles {max_files}; source: {source}"))
}

fn search_result(store: &mut Store, args: &Value, throttle: bool) -> Result<String> {
    let queries = args
        .get("queries")
        .or(args.get("query"))
        .map(strings)
        .transpose()?
        .unwrap_or_default();
    if queries.is_empty() || queries.len() > 20 {
        return Err(anyhow!("Informe 1 a 20 queries em uma chamada"));
    }
    let mut limit = args["limit"].as_u64().unwrap_or(3).clamp(1, 20) as usize;
    let mut count = 0;
    if throttle {
        store
            .db
            .execute("DELETE FROM search_calls WHERE at<unixepoch()-60", [])?;
        count = store
            .db
            .query_row::<i64, _, _>("SELECT count(*) FROM search_calls", [], |r| r.get(0))?
            + 1;
        if count > 8 {
            return Err(anyhow!("Limite de busca: 8 chamadas por minuto. Agrupe queries; aguarde a janela de 60 segundos."));
        }
        store
            .db
            .execute("INSERT INTO search_calls(at) VALUES (unixepoch())", [])?;
        if count >= 3 {
            limit = 1;
        }
    }
    let source = args["source"].as_str();
    let kind = args["contentType"].as_str();
    if kind.is_some_and(|s| s != "code" && s != "prose") {
        return Err(anyhow!("contentType deve ser code ou prose"));
    }
    let timeline = args["sort"].as_str() == Some("timeline");
    let mut output = String::new();
    for query in queries {
        output.push_str(&format!("\n## {}\n", chunk::prefix(&query, 300)));
        let hits = store.search(&query, source, kind, limit, timeline)?;
        if hits.is_empty() {
            output.push_str("No results. Indexe o conteudo antes de pesquisar.\n");
        }
        for hit in hits {
            let budget = (12_000usize.saturating_sub(output.len())).min(2200);
            if budget < 300 {
                output
                    .push_str("Orcamento atingido; refine a consulta para obter outros trechos.\n");
                break;
            }
            output.push_str(&format!(
                "### {} [{}] {} {}\n{}\n",
                chunk::prefix(&hit.title, 200),
                chunk::prefix(&hit.source, 300),
                hit.layer,
                if hit.stale {
                    "[ARQUIVO ALTERADO: reindexe]"
                } else {
                    ""
                },
                search::snippet(&hit.content, &query, budget.saturating_sub(550))
            ));
        }
        if output.len() > 11_700 {
            break;
        }
    }
    if throttle {
        output.push_str(&format!("\nThrottle: call #{count}/8 em 60s."));
    }
    Ok(output)
}

pub async fn execute_tool(
    data: &Path,
    session: &Session,
    name: &str,
    args: &Value,
    background: Option<&crate::agent::background::BackgroundJobs>,
) -> Result<String> {
    let short = name
        .strip_prefix(&format!("mcp__{}__", super::server_name(&session.id)))
        .unwrap_or(name);
    if short.contains("__") || !short.starts_with("ctx_") {
        return Err(anyhow!("Ferramenta fora desta conversa"));
    }
    let started = Instant::now();
    let mut avoided = 0;
    let mut execution_error = false;
    let roots: Vec<_> = session
        .extra_read_paths
        .iter()
        .map(|p| PathBuf::from(&p.path))
        .collect();
    let project = project(data, session)?;
    let owned = args.clone();
    let result: Result<String> = async { match short {
        "ctx_index" => database(data,&session.id,move |store|index_paths(store,&project,&roots,&owned)).await,
        "ctx_search" => {
            if let Some(filter) = args["project"].as_str() {
                if Path::new(filter).canonicalize().ok() != project.canonicalize().ok() { return Ok("No results: projeto diferente desta conversa.".into()); }
            }
            database(data,&session.id,move |store| {
                let paths = {
                    let mut statement = store.db.prepare("SELECT label,path,hash FROM sources WHERE path IS NOT NULL")?;
                    let rows = statement.query_map([],|r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
                    rows
                };
                for (label,path,saved_hash) in paths {
                    // Re-check authorization on every refresh, including changed symlinks.
                    if let Ok(canonical) = execute::resolve_read(&project,&roots,&path) {
                        if let Ok(text) = execute::read_text(&canonical) {
                            if super::store::hash(&text) != saved_hash {
                                store.index(&text,&label,Some(&canonical.to_string_lossy()))?;
                            }
                        }
                    }
                }
                search_result(store,&owned,true)
            }).await
        }
        "ctx_execute" | "ctx_execute_file" => {
            let language = args["language"].as_str().ok_or_else(||anyhow!("language obrigatorio"))?;
            let code = args["code"].as_str().ok_or_else(||anyhow!("code obrigatorio"))?;
            if code.len() > 2*1024*1024 { return Err(anyhow!("Codigo excede 2 MB")); }
            let cwd = args["cwd"].as_str().map(|p|execute::resolve_read(&project,&roots,p)).transpose()?.unwrap_or(project.clone());
            if boolean(&args["background"],false)? {
                if language != "shell" {return Err(anyhow!("Execucao em segundo plano nativa usa language shell; use run_command para outros runtimes."));}
                let jobs = background.ok_or_else(||anyhow!("Gerenciador de processos indisponivel"))?;
                let id = jobs.start(&cwd,code,data,&session.id)?;
                return Ok(format!("Comando em segundo plano: {id}. Use check_background_output / stop_background."));
            }
            let file = if short == "ctx_execute_file" {
                let path = execute::resolve_read(&project,&roots,args["path"].as_str().ok_or_else(||anyhow!("path obrigatorio"))?)?;
                Some(execute::read_text(&path)?)
            } else {None};
            let outcome = execute::run(language,code,&cwd,&roots,file,args["timeout"].as_u64().unwrap_or(30_000)).await?;
            execution_error = outcome.exit_code != 0 || outcome.timed_out;
            let raw = format!("# stdout\n{}\n# stderr\n{}\nexit_code={} timed_out={}",outcome.stdout,outcome.stderr,outcome.exit_code,outcome.timed_out);
            let source = format!("execution-{}",uuid::Uuid::new_v4());
            let stored = raw.clone();let label = source.clone();
            database(data,&session.id,move |s|s.index(&stored,&label,None)).await?;
            let result = if raw.len() > 5000 {
                let intent = args["intent"].as_str().unwrap_or("error result");
                let query = json!({"queries":[intent],"source":source,"limit":3});
                let detail = database(data,&session.id,move |s|search_result(s,&query,false)).await?;
                format!("Saida completa arquivada: {} bytes, source {source}. exit={}\n{detail}",raw.len(),outcome.exit_code)
            } else {raw.clone()};
            avoided = raw.len().saturating_sub(result.len()); Ok(result)
        }
        "ctx_batch_execute" => {
            let commands = if let Some(value) = args["commands"].as_str() {serde_json::from_str(value)?}else{args["commands"].clone()};
            let commands = commands.as_array().ok_or_else(||anyhow!("commands obrigatorio"))?.clone();
            if commands.len() > 50 || commands.is_empty() {return Err(anyhow!("Informe 1 a 50 comandos"));}
            let timeout = args["timeout"].as_u64().unwrap_or(30_000);
            let concurrency = args["concurrency"].as_u64().unwrap_or(3).clamp(1,8) as usize;
            let cwd = args["cwd"].as_str().map(|p|execute::resolve_read(&project,&roots,p)).transpose()?.unwrap_or(project);
            let mut outputs: Vec<(usize,String)> = stream::iter(commands.into_iter().enumerate().map(|(i,command)| {
                let cwd = cwd.clone();async move {
                    let code = command.as_str().or(command["command"].as_str()).ok_or_else(||anyhow!("command ausente"))?;
                    let label = command["label"].as_str().unwrap_or("command");
                    let result = execute::run("shell",code,&cwd,&[],None,timeout).await;
                    Ok::<_,anyhow::Error>((i,match result {Ok(r)=>format!("# {label}\n{}\n{}\nexit={} timeout={}",r.stdout,r.stderr,r.exit_code,r.timed_out),Err(e)=>format!("# {label}\nERROR: {e}")}))
                }
            })).buffer_unordered(concurrency).collect::<Vec<_>>().await.into_iter().collect::<Result<_>>()?;
            outputs.sort_by_key(|(i,_)|*i);
            let raw = outputs.into_iter().map(|(_,s)|s).collect::<Vec<_>>().join("\n\n");
            if raw.len() > execute::MAX_BYTES {return Err(anyhow!("Saida total do lote excede 50 MB"));}
            let source = format!("batch-{}",uuid::Uuid::new_v4());let stored = raw.clone();let label = source.clone();
            database(data,&session.id,move |s|s.index(&stored,&label,None)).await?;
            let mut query = args.clone();query["source"] = if args["query_scope"].as_str()==Some("global"){Value::Null}else{json!(source)};
            let result = if args.get("queries").is_some() { database(data,&session.id,move |s|search_result(s,&query,false)).await? }else{format!("Lote arquivado: {} bytes; source {source}. Use ctx_search.",raw.len())};
            avoided = raw.len().saturating_sub(result.len());Ok(result)
        }
        "ctx_fetch_and_index" => {
            let requests = if let Some(requests) = args.get("requests") {requests.as_array().ok_or_else(||anyhow!("requests deve ser lista"))?.clone()}else{vec![args.clone()]};
            if requests.is_empty() || requests.len()>20 {return Err(anyhow!("Informe 1 a 20 URLs"));}
            let concurrency = args["concurrency"].as_u64().unwrap_or(3).clamp(1,8) as usize;
            let force = boolean(&args["force"],false)?;
            let ttl = args["ttl"].as_u64().unwrap_or(86400).min(604800);
            let outcomes = stream::iter(requests.into_iter().map(|request|{
                let data=data.to_path_buf();let id=session.id.clone();async move {
                    let url = request["url"].as_str().ok_or_else(||anyhow!("url obrigatoria"))?.to_string();
                    let cached_url=url.clone();
                    let cached = database(&data,&id,move |s| {
                        Ok(s.db.query_row::<String,_,_>("SELECT source FROM fetch_cache WHERE url=? AND expires>unixepoch()",[cached_url],|r|r.get(0)).ok())
                    }).await?;
                    if !force {if let Some(source)=cached {return Ok(format!("Cache valido: {url}; source {source}"));}}
                    let (text,final_url)=fetch::page(&url).await?;
                    let source=request["source"].as_str().unwrap_or(&final_url).to_string();let bytes=text.len();
                    let label=source.clone();
                    let count=database(&data,&id,move |s|{
                        let count=s.index(&text,&label,None)?;
                        s.db.execute("INSERT INTO fetch_cache(url,source,expires) VALUES (?,?,unixepoch()+?) ON CONFLICT(url) DO UPDATE SET source=excluded.source,expires=excluded.expires",params![url,label,ttl as i64])?;Ok(count)
                    }).await?;
                    Ok::<_,anyhow::Error>(format!("Fetched and indexed {count} sections ({bytes} bytes completos). source: {source}"))
                }
            })).buffer_unordered(concurrency).collect::<Vec<_>>().await;
            let mut result=vec![];
            for outcome in outcomes {result.push(match outcome{Ok(s)=>s,Err(e)=>format!("Fetch error: {e}")});}
            Ok(result.join("\n"))
        }
        "ctx_stats" | "ctx_insight" => {
            let model=session.model.clone();
            database(data,&session.id,move |store|{
                let mut stats=store.stats()?;
                let mut statement=store.db.prepare("SELECT raw FROM sources")?;
                let mut rows=statement.query([])?;
                let mut tokens=0usize;
                while let Some(row)=rows.next()? {
                    let raw:String=row.get(0)?;
                    tokens=tokens.saturating_add(crate::context::estimate_tokens_for(&model,&raw) as usize);
                }
                stats["stored_tokens_estimate"]=json!(tokens);
                stats["token_estimate_model"]=json!(model);
                stats["privacy"]=json!("Relatorio local; nenhum dado enviado ao dashboard externo");
                Ok(serde_json::to_string_pretty(&stats)?)
            }).await
        }
        "ctx_doctor" => database(data,&session.id,move |store| {
            let integrity:String=store.db.query_row("PRAGMA quick_check",[],|r|r.get(0))?;
            let sqlite:String=store.db.query_row("SELECT sqlite_version()",[],|r|r.get(0))?;
            Ok(format!("Rust nativo: OK\nSQLite incluido: {sqlite}\nFTS5 Porter/unicode61 + trigram: OK\nIntegridade: {integrity}\nNode/npm/MCP subprocesso: nao utilizados\nBanco: {}\nMigracao: {:?}\nJavaScript: Boa embutido; APIs Node limitadas e documentadas\nLinguagens extras: opcionais do projeto",store.root.join("native.sqlite3").display(),store.meta("legacy_imported")?))
        }).await,
        "ctx_purge" => {
            if args["scope"].as_str().is_some_and(|scope|scope!="session"){return Err(anyhow!("Limpeza suporta apenas scope session"));}
            if args["sessionId"].as_str().is_some_and(|id|id!=session.id){return Err(anyhow!("Limpeza de outra conversa nao permitida"));}
            if !boolean(&args["confirm"],false)? {return Ok("Para limpar o indice ativo desta conversa, confirme com confirm: true. O historico e as copias antigas de migracao permanecem.".into());}
            database(data,&session.id,|store| {
                let tx=store.db.transaction()?;
                tx.execute_batch("DELETE FROM chunks; DELETE FROM chunks_trigram; DELETE FROM sources; DELETE FROM vocabulary; DELETE FROM records; DELETE FROM events; DELETE FROM calls; DELETE FROM fetch_cache; DELETE FROM search_calls; DELETE FROM meta WHERE key!='legacy_imported';")?;tx.commit()?;
                let view=store.root.join("view.json");if view.exists(){std::fs::remove_file(view)?;}
                Ok("Indice ativo, eventos e metricas desta conversa limpos. Historico e copias antigas preservados.".into())
            }).await
        }
        _ => Err(anyhow!("Ferramenta nativa desconhecida: {short}")),
    }}.await;
    let metric_name = short.to_string();
    let bytes = args.to_string().len();
    let elapsed = started.elapsed().as_millis();
    let (out, error) = match &result {
        Ok(s) => (s.len(), execution_error),
        Err(e) => (e.to_string().len(), true),
    };
    database(data, &session.id, move |s| {
        s.record_call(&metric_name, bytes, out, avoided, elapsed, error)
    })
    .await?;
    result
}

pub fn slash(text: &str) -> Option<(&str, &str)> {
    let body = text.strip_prefix('/')?;
    let (name, args) = body.split_once(char::is_whitespace).unwrap_or((body, ""));
    let name = name.strip_prefix("context-mode:").unwrap_or(name);
    if [
        "ctx-stats",
        "ctx-doctor",
        "ctx-index",
        "ctx-search",
        "ctx-purge",
        "ctx-insight",
        "ctx-upgrade",
    ]
    .contains(&name)
    {
        Some((name, args.trim()))
    } else {
        None
    }
}
pub async fn command(data: &Path, session: &Session, text: &str) -> Result<String> {
    let (name, arguments) =
        slash(text).ok_or_else(|| anyhow!("Comando de Economia de contexto desconhecido"))?;
    if name == "ctx-upgrade" {
        return Ok("Economia de contexto agora e compilada em Rust dentro do Cerne. Para atualiza-la, instale uma nova versao do Cerne Code. Nao ha download de Node, npm ou substituicao de codigo no projeto.".into());
    }
    let args = if arguments.starts_with('{') {
        serde_json::from_str(arguments)?
    } else {
        match name {
            "ctx-index" => json!({"path":arguments.trim_matches(['\"','\''])}),
            "ctx-search" => json!({"queries":[arguments]}),
            "ctx-purge" => json!({"confirm":arguments=="--confirm"}),
            _ => json!({}),
        }
    };
    execute_tool(data, session, &name.replace('-', "_"), &args, None).await
}
