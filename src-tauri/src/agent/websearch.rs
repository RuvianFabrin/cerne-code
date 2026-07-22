use crate::search::{SearchProviderKind, SearchResultItem};
use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::net::IpAddr;
use std::path::Path;

const MIN_RESULTS: usize = 8;
const MAX_PAGES: u32 = 2;
const MERGE_TOP_N: usize = 12;
/// Cap num numero razoavel de queries por chamada — o modelo decide quantas
/// buscas fazer (ver `execute_tool` em `tools.rs`), mas sem limite isso vira
/// vetor de carga/abuso.
const MAX_QUERIES_PER_CALL: usize = 5;

/// Busca usando o provider configurado em Configurações → Busca na web
/// (`Auto` por padrão, sem precisar de chave nem instalar nada). Aceita uma
/// ou mais queries — o agente decide quantas fazer numa unica chamada (ver
/// `web_search` em `tools.rs`); cada uma roda em paralelo e o resultado
/// final concatena um bloco por query, com cabecalho so quando ha mais de
/// uma (pra nao poluir o caso comum de query unica).
pub async fn search_many(app_data_dir: &Path, queries: &[String]) -> Result<String> {
    let queries: Vec<&String> = queries.iter().take(MAX_QUERIES_PER_CALL).collect();
    if queries.len() == 1 {
        return search(app_data_dir, queries[0]).await;
    }
    let futures = queries
        .iter()
        .map(|q| search(app_data_dir, q))
        .collect::<Vec<_>>();
    let results = futures_util::future::join_all(futures).await;

    Ok(queries
        .iter()
        .zip(results)
        .map(|(q, r)| {
            let body = match r {
                Ok(text) => text,
                Err(e) => format!("(busca falhou: {e})"),
            };
            format!("### Resultados para \"{q}\"\n\n{body}")
        })
        .collect::<Vec<_>>()
        .join("\n\n---\n\n"))
}

/// Busca usando o provider configurado em Configurações → Busca na web
/// (`Auto`/multi-engine por padrão, sem precisar de chave nem instalar nada).
pub async fn search(app_data_dir: &Path, query: &str) -> Result<String> {
    let config = crate::search::load_config(app_data_dir);
    let results = match config.provider {
        SearchProviderKind::Auto => search_auto(query).await?,
        SearchProviderKind::Brave => {
            let key = crate::search::get_key(SearchProviderKind::Brave).ok_or_else(|| {
                anyhow!(
                    "busca via Brave selecionada mas sem chave de API configurada — adicione em Configurações → Busca na web"
                )
            })?;
            search_brave(query, &key).await?
        }
        SearchProviderKind::Tavily => {
            let key = crate::search::get_key(SearchProviderKind::Tavily).ok_or_else(|| {
                anyhow!(
                    "busca via Tavily selecionada mas sem chave de API configurada — adicione em Configurações → Busca na web"
                )
            })?;
            search_tavily(query, &key).await?
        }
        SearchProviderKind::Searxng => search_searxng(query, &config.searxng_url).await?,
    };
    Ok(format_results(&results))
}

/// Agregador multi-engine no estilo do SearXNG (estudei o código-fonte dele
/// — `searx/results.py`/`searx/engines/*.py` — pra copiar a ideia real, não só
/// a API JSON que o Cerne já chamava): várias fontes independentes e
/// keyless em paralelo (DuckDuckGo, a página pública do Brave, Mojeek),
/// merge por URL normalizada com pontuação por posição
/// (`1/posição`, somada entre engines que concordam) e corte no topo — assim
/// se uma fonte cair ou bloquear o scraping, as outras sustentam a busca em
/// vez do recurso inteiro falhar.
async fn search_auto(query: &str) -> Result<Vec<SearchResultItem>> {
    let (ddg, brave, mojeek) = tokio::join!(
        search_duckduckgo(query),
        search_brave_html(query),
        search_mojeek(query),
    );

    let mut engine_results: Vec<(&'static str, Vec<SearchResultItem>)> = Vec::new();
    for (name, result) in [("duckduckgo", ddg), ("brave", brave), ("mojeek", mojeek)] {
        match result {
            Ok(items) if !items.is_empty() => engine_results.push((name, items)),
            Ok(_) => {}
            Err(_e) => {} // uma fonte falhando nao derruba a busca inteira - so contribui menos
        }
    }

    if engine_results.is_empty() {
        return Err(anyhow!(
            "todas as fontes de busca falharam (DuckDuckGo, Brave, Mojeek) — tente de novo em instantes"
        ));
    }

    Ok(merge_engine_results(engine_results))
}

/// Combina resultados de varios engines por URL normalizada, somando
/// `1/(posicao+1)` de cada engine que trouxe aquela URL (mesma ideia do
/// `weight / position` do SearXNG) — resultado que aparece em mais de uma
/// fonte, ou bem rankeado numa so, sobe pro topo.
fn merge_engine_results(engine_results: Vec<(&'static str, Vec<SearchResultItem>)>) -> Vec<SearchResultItem> {
    struct Merged {
        item: SearchResultItem,
        score: f64,
    }

    let mut map: HashMap<String, Merged> = HashMap::new();
    for (_engine, items) in engine_results {
        for (i, item) in items.into_iter().enumerate() {
            let key = normalize_url(&item.url);
            let contribution = 1.0 / (i as f64 + 1.0);
            match map.get_mut(&key) {
                Some(existing) => {
                    existing.score += contribution;
                    if item.snippet.len() > existing.item.snippet.len() {
                        existing.item.snippet = item.snippet;
                    }
                    if item.title.len() > existing.item.title.len() {
                        existing.item.title = item.title;
                    }
                }
                None => {
                    map.insert(key, Merged { item, score: contribution });
                }
            }
        }
    }

    let mut merged: Vec<Merged> = map.into_values().collect();
    merged.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    merged.into_iter().take(MERGE_TOP_N).map(|m| m.item).collect()
}

/// Normaliza uma URL pra dedup entre engines: host sem `www.`, sem
/// barra final, sem fragmento, sem parametros de tracking comuns (o mesmo
/// artigo aparece com querystrings diferentes em cada engine).
fn normalize_url(u: &str) -> String {
    const TRACKING_PARAMS: &[&str] = &[
        "utm_source",
        "utm_medium",
        "utm_campaign",
        "utm_term",
        "utm_content",
        "ref",
        "fbclid",
        "gclid",
        "msclkid",
    ];
    let Ok(mut parsed) = url::Url::parse(u) else {
        return u.trim_end_matches('/').to_lowercase();
    };
    parsed.set_fragment(None);
    let kept: Vec<(String, String)> = parsed
        .query_pairs()
        .filter(|(k, _)| !TRACKING_PARAMS.contains(&k.as_ref()))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    if kept.is_empty() {
        parsed.set_query(None);
    } else {
        parsed.query_pairs_mut().clear().extend_pairs(&kept);
    }
    let host = parsed
        .host_str()
        .unwrap_or("")
        .strip_prefix("www.")
        .unwrap_or(parsed.host_str().unwrap_or(""))
        .to_lowercase();
    let path = parsed.path().trim_end_matches('/');
    format!("{host}{path}?{}", parsed.query().unwrap_or(""))
}

/// Roda uma busca de teste (query fixa) contra o provider indicado, sem
/// tocar no config salvo nem no keyring — usada pelo botão "Testar conexão"
/// na tela, com a chave/URL que o usuário acabou de digitar, antes de
/// efetivar. Devolve a contagem de resultados encontrados.
pub async fn test_provider(
    provider: SearchProviderKind,
    api_key: Option<&str>,
    searxng_url: Option<&str>,
) -> Result<usize> {
    let results = match provider {
        SearchProviderKind::Auto => search_auto("rust programming language").await?,
        SearchProviderKind::Brave => {
            let key = api_key.ok_or_else(|| anyhow!("informe a chave de API da Brave"))?;
            search_brave("rust programming language", key).await?
        }
        SearchProviderKind::Tavily => {
            let key = api_key.ok_or_else(|| anyhow!("informe a chave de API da Tavily"))?;
            search_tavily("rust programming language", key).await?
        }
        SearchProviderKind::Searxng => {
            let url = searxng_url.ok_or_else(|| anyhow!("informe a URL do SearXNG"))?;
            search_searxng("rust programming language", url).await?
        }
    };
    Ok(results.len())
}

fn format_results(results: &[SearchResultItem]) -> String {
    if results.is_empty() {
        return "Nenhum resultado encontrado.".to_string();
    }
    results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let snippet = if r.snippet.chars().count() > 300 {
                format!("{}...", r.snippet.chars().take(300).collect::<String>())
            } else {
                r.snippet.clone()
            };
            format!("{}. {} — {}\n   {}", i + 1, r.title, r.url, snippet)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Busca sem chave nem servidor local: HTML server-side do DuckDuckGo, sem
/// JS — não é uma API oficial, mas é o mesmo caminho keyless que outras
/// ferramentas de agente (ex. plugin `opencode-websearch_duckduckgo`) usam
/// como padrão sem exigir nenhuma conta.
async fn search_duckduckgo(query: &str) -> Result<Vec<SearchResultItem>> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://html.duckduckgo.com/html/")
        .query(&[("q", query)])
        .header("User-Agent", "Mozilla/5.0 (compatible; Cerne/0.1)")
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| anyhow!("falha ao buscar no DuckDuckGo: {e}"))?;

    if !resp.status().is_success() {
        return Err(anyhow!("DuckDuckGo respondeu {}", resp.status()));
    }
    let html = resp
        .text()
        .await
        .map_err(|e| anyhow!("resposta invalida do DuckDuckGo: {e}"))?;
    Ok(parse_duckduckgo_html(&html))
}

fn parse_duckduckgo_html(html: &str) -> Vec<SearchResultItem> {
    use scraper::{Html, Selector};

    let doc = Html::parse_document(html);
    let result_sel = Selector::parse("div.result").unwrap();
    let title_sel = Selector::parse("a.result__a").unwrap();
    let snippet_sel = Selector::parse(".result__snippet").unwrap();

    let mut out = Vec::new();
    for el in doc.select(&result_sel) {
        // Anuncios patrocinados carregam a classe `result--ad` e apontam pra
        // um redirect de clique (`y.js`) em vez de um `uddg` de verdade —
        // pulados aqui porque a URL final nao e diretamente util pro agente.
        if el
            .value()
            .attr("class")
            .unwrap_or("")
            .split_whitespace()
            .any(|c| c == "result--ad")
        {
            continue;
        }
        let Some(title_el) = el.select(&title_sel).next() else {
            continue;
        };
        let title = title_el.text().collect::<String>().trim().to_string();
        let href = title_el.value().attr("href").unwrap_or_default();
        let url = extract_ddg_target(href).unwrap_or_else(|| href.to_string());
        if url.contains("y.js?") {
            continue;
        }
        let snippet = el
            .select(&snippet_sel)
            .next()
            .map(|s| s.text().collect::<String>())
            .unwrap_or_default()
            .trim()
            .to_string();
        if title.is_empty() || url.is_empty() {
            continue;
        }
        out.push(SearchResultItem { title, url, snippet });
        if out.len() >= MIN_RESULTS.max(10) {
            break;
        }
    }
    out
}

/// O DuckDuckGo envolve todo link de resultado num redirect
/// (`//duckduckgo.com/l/?uddg=<url-encoded>`) — a URL de verdade fica no
/// parametro `uddg`.
fn extract_ddg_target(href: &str) -> Option<String> {
    let full = if let Some(stripped) = href.strip_prefix("//") {
        format!("https://{stripped}")
    } else {
        href.to_string()
    };
    let parsed = url::Url::parse(&full).ok()?;
    parsed
        .query_pairs()
        .find(|(k, _)| k == "uddg")
        .map(|(_, v)| v.to_string())
}

const DESKTOP_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

/// Segunda fonte keyless: a página pública de busca do Brave (não a API
/// paga — essa é `search_brave` mais abaixo). Seletores baseados no engine
/// `brave.py` do SearXNG (XPath `div[contains(@class,'snippet ')]`,
/// título em `.title`, corpo em `.content`) — aqui convertidos pro
/// equivalente em CSS que o `scraper` entende.
async fn search_brave_html(query: &str) -> Result<Vec<SearchResultItem>> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://search.brave.com/search")
        .query(&[("q", query), ("source", "web")])
        .header("User-Agent", DESKTOP_USER_AGENT)
        .header("Accept-Language", "en-US,en;q=0.9")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| anyhow!("falha ao buscar na pagina do Brave: {e}"))?;

    if !resp.status().is_success() {
        return Err(anyhow!("Brave (pagina publica) respondeu {}", resp.status()));
    }
    let html = resp
        .text()
        .await
        .map_err(|e| anyhow!("resposta invalida da pagina do Brave: {e}"))?;
    Ok(parse_brave_html(&html))
}

fn parse_brave_html(html: &str) -> Vec<SearchResultItem> {
    use scraper::{Html, Selector};

    let doc = Html::parse_document(html);
    let snippet_sel = Selector::parse("div.snippet").unwrap();
    let link_sel = Selector::parse("a").unwrap();
    let title_sel = Selector::parse(".title").unwrap();
    let content_sel = Selector::parse(".snippet-description, .content").unwrap();

    let mut out = Vec::new();
    for el in doc.select(&snippet_sel) {
        let Some(link) = el.select(&link_sel).next() else {
            continue;
        };
        let url = link.value().attr("href").unwrap_or_default().to_string();
        if !url.starts_with("http") {
            continue;
        }
        let title = el
            .select(&title_sel)
            .next()
            .map(|t| t.text().collect::<String>())
            .unwrap_or_default()
            .trim()
            .to_string();
        if title.is_empty() {
            continue;
        }
        let snippet = el
            .select(&content_sel)
            .next()
            .map(|s| s.text().collect::<String>())
            .unwrap_or_default()
            .trim()
            .to_string();
        out.push(SearchResultItem { title, url, snippet });
        if out.len() >= 10 {
            break;
        }
    }
    out
}

/// Terceira fonte keyless: Mojeek, um dos poucos motores com índice
/// próprio que ainda tolera bem scraping simples (usado pelo SearXNG via
/// `mojeek.py`) — bom desempate quando DuckDuckGo e Brave concordam menos.
async fn search_mojeek(query: &str) -> Result<Vec<SearchResultItem>> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://www.mojeek.com/search")
        .query(&[("q", query)])
        .header("User-Agent", DESKTOP_USER_AGENT)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| anyhow!("falha ao buscar no Mojeek: {e}"))?;

    if !resp.status().is_success() {
        return Err(anyhow!("Mojeek respondeu {}", resp.status()));
    }
    let html = resp
        .text()
        .await
        .map_err(|e| anyhow!("resposta invalida do Mojeek: {e}"))?;
    Ok(parse_mojeek_html(&html))
}

fn parse_mojeek_html(html: &str) -> Vec<SearchResultItem> {
    use scraper::{Html, Selector};

    let doc = Html::parse_document(html);
    let li_sel = Selector::parse("ul.results-standard > li").unwrap();
    let link_sel = Selector::parse("a.ob").unwrap();
    let title_sel = Selector::parse("h2 a").unwrap();
    let snippet_sel = Selector::parse("p.s").unwrap();

    let mut out = Vec::new();
    for li in doc.select(&li_sel) {
        let Some(link) = li.select(&link_sel).next() else {
            continue;
        };
        let url = link.value().attr("href").unwrap_or_default().to_string();
        let title = li
            .select(&title_sel)
            .next()
            .map(|t| t.text().collect::<String>())
            .unwrap_or_default()
            .trim()
            .to_string();
        if url.is_empty() || title.is_empty() {
            continue;
        }
        let snippet = li
            .select(&snippet_sel)
            .next()
            .map(|s| s.text().collect::<String>())
            .unwrap_or_default()
            .trim()
            .to_string();
        out.push(SearchResultItem { title, url, snippet });
        if out.len() >= 10 {
            break;
        }
    }
    out
}

#[derive(Debug, Deserialize)]
struct BraveResponse {
    #[serde(default)]
    web: BraveWeb,
}

#[derive(Debug, Deserialize, Default)]
struct BraveWeb {
    #[serde(default)]
    results: Vec<BraveResult>,
}

#[derive(Debug, Deserialize)]
struct BraveResult {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    description: String,
}

async fn search_brave(query: &str, api_key: &str) -> Result<Vec<SearchResultItem>> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.search.brave.com/res/v1/web/search")
        .query(&[("q", query), ("count", "10")])
        .header("X-Subscription-Token", api_key)
        .header("Accept", "application/json")
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| anyhow!("falha ao buscar na Brave Search API: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("Brave Search API respondeu {status}: {body}"));
    }
    let parsed: BraveResponse = resp
        .json()
        .await
        .map_err(|e| anyhow!("resposta invalida da Brave Search API: {e}"))?;

    Ok(parsed
        .web
        .results
        .into_iter()
        .map(|r| SearchResultItem {
            title: r.title,
            url: r.url,
            snippet: r.description,
        })
        .collect())
}

#[derive(Debug, serde::Serialize)]
struct TavilyRequest<'a> {
    query: &'a str,
    max_results: u32,
}

#[derive(Debug, Deserialize)]
struct TavilyResponse {
    #[serde(default)]
    results: Vec<TavilyResult>,
}

#[derive(Debug, Deserialize)]
struct TavilyResult {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    content: String,
}

async fn search_tavily(query: &str, api_key: &str) -> Result<Vec<SearchResultItem>> {
    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.tavily.com/search")
        .bearer_auth(api_key)
        .json(&TavilyRequest {
            query,
            max_results: 10,
        })
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| anyhow!("falha ao buscar na Tavily: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("Tavily respondeu {status}: {body}"));
    }
    let parsed: TavilyResponse = resp
        .json()
        .await
        .map_err(|e| anyhow!("resposta invalida da Tavily: {e}"))?;

    Ok(parsed
        .results
        .into_iter()
        .map(|r| SearchResultItem {
            title: r.title,
            url: r.url,
            snippet: r.content,
        })
        .collect())
}

#[derive(Debug, Deserialize)]
struct SearxResponse {
    #[serde(default)]
    results: Vec<SearxResult>,
}

#[derive(Debug, Deserialize)]
struct SearxResult {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    content: String,
}

/// Instância própria de SearXNG — comportamento de antes desta tela
/// existir, preservado pra quem já roda uma (ex: setup com
/// `docker run ... -p 8888:8080`, JSON API habilitada em settings.yml).
async fn search_searxng(query: &str, base_url: &str) -> Result<Vec<SearchResultItem>> {
    let client = reqwest::Client::new();
    let mut results: Vec<SearxResult> = Vec::new();
    let mut page = 1u32;

    while results.len() < MIN_RESULTS && page <= MAX_PAGES {
        let resp = client
            .get(format!("{base_url}/search"))
            .query(&[("q", query), ("format", "json"), ("pageno", &page.to_string())])
            .timeout(std::time::Duration::from_secs(20))
            .send()
            .await
            .map_err(|e| anyhow!("nao foi possivel contactar o SearXNG em {base_url} (esta rodando?): {e}"))?;

        if !resp.status().is_success() {
            return Err(anyhow!("SearXNG respondeu {}", resp.status()));
        }
        let parsed: SearxResponse = resp
            .json()
            .await
            .map_err(|e| anyhow!("resposta do SearXNG invalida: {e}"))?;
        if parsed.results.is_empty() {
            break;
        }
        results.extend(parsed.results);
        page += 1;
    }

    Ok(results
        .into_iter()
        .map(|r| SearchResultItem {
            title: r.title,
            url: r.url,
            snippet: r.content,
        })
        .collect())
}

/// Fetches a single page and returns its visible text, stripped of
/// scripts/styles/nav chrome. Guards against SSRF: only http/https, and the
/// resolved IP can't be loopback/private/link-local — the URL almost always
/// comes from search results or the model's own text, both effectively
/// untrusted input.
pub async fn fetch(url_str: &str) -> Result<String> {
    let parsed = validate_public_url(url_str).await?;

    let client = reqwest::Client::new();
    let resp = client
        .get(parsed.clone())
        .timeout(std::time::Duration::from_secs(15))
        .header("User-Agent", "Mozilla/5.0 (compatible; Cerne/0.1)")
        .send()
        .await
        .map_err(|e| anyhow!("falha ao buscar {url_str}: {e}"))?;

    if !resp.status().is_success() {
        return Err(anyhow!("{url_str} respondeu {}", resp.status()));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| anyhow!("falha lendo resposta de {url_str}: {e}"))?;
    // Cap before parsing — a malicious/huge page shouldn't tie up the parser.
    let capped = &bytes[..bytes.len().min(2_000_000)];
    let html = String::from_utf8_lossy(capped);

    let text = extract_text(&html);
    let truncated = if text.chars().count() > 8000 {
        format!(
            "{}\n... [truncado]",
            text.chars().take(8000).collect::<String>()
        )
    } else {
        text
    };
    Ok(truncated)
}

fn extract_text(html: &str) -> String {
    use scraper::{Html, Selector};

    let mut doc = Html::parse_document(html);
    // Detach non-content subtrees first so the later `.text()` walk never
    // sees them, instead of trying to filter text nodes by ancestor after
    // the fact.
    let skip = Selector::parse("script, style, nav, footer, noscript, svg, head").unwrap();
    let skip_ids: Vec<_> = doc.select(&skip).map(|el| el.id()).collect();
    for id in skip_ids {
        if let Some(mut node) = doc.tree.get_mut(id) {
            node.detach();
        }
    }

    let body_sel = Selector::parse("body").unwrap();
    let text = doc
        .select(&body_sel)
        .next()
        .map(|el| el.text().collect::<Vec<_>>().join(" "))
        .unwrap_or_default();

    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

async fn validate_public_url(url_str: &str) -> Result<url::Url> {
    let parsed = url::Url::parse(url_str).map_err(|e| anyhow!("URL invalida: {e}"))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(anyhow!("apenas URLs http/https sao permitidas"));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow!("URL sem host"))?
        .to_string();
    if host.eq_ignore_ascii_case("localhost") {
        return Err(anyhow!("acesso a localhost bloqueado"));
    }
    let port = parsed.port_or_known_default().unwrap_or(80);

    let addrs = tokio::net::lookup_host((host.as_str(), port))
        .await
        .map_err(|e| anyhow!("nao foi possivel resolver {host}: {e}"))?;

    let mut any = false;
    for addr in addrs {
        any = true;
        if is_blocked_ip(addr.ip()) {
            return Err(anyhow!(
                "{host} resolve para um endereco de rede interno/privado — bloqueado por seguranca"
            ));
        }
    }
    if !any {
        return Err(anyhow!("{host} nao resolveu para nenhum endereco"));
    }

    Ok(parsed)
}

fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_unspecified()
                || v4.is_documentation()
        }
        IpAddr::V6(v6) => {
            v6.is_loopback() || v6.is_unspecified() || (v6.segments()[0] & 0xfe00) == 0xfc00
            // fc00::/7 unique-local
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Hits a rede de verdade (DuckDuckGo por padrao) — run manually com
    // `cargo test -- --ignored --nocapture`, nao faz parte da suite default.
    #[tokio::test]
    #[ignore]
    async fn search_returns_results() {
        let dir = std::env::temp_dir().join(format!("cerne-websearch-test-{}", uuid::Uuid::new_v4()));
        let out = search(&dir, "rust programming language").await.unwrap();
        println!("{out}");
        assert!(out.contains("rust-lang.org") || out.contains("Rust"));
    }

    #[tokio::test]
    #[ignore]
    async fn search_many_runs_multiple_queries_in_parallel_with_headers() {
        let dir = std::env::temp_dir().join(format!("cerne-websearch-test-{}", uuid::Uuid::new_v4()));
        let queries = vec!["rust programming language".to_string(), "python programming language".to_string()];
        let out = search_many(&dir, &queries).await.unwrap();
        println!("{out}");
        assert!(out.contains("Resultados para \"rust programming language\""));
        assert!(out.contains("Resultados para \"python programming language\""));
    }

    #[test]
    fn parse_duckduckgo_html_extracts_title_url_and_unwraps_redirect() {
        let html = r#"<div class="result results_links results_links_deep web-result">
  <div class="result__body">
    <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fwww.rust%2Dlang.org%2F&amp;rut=abc">Rust Programming Language</a>
    <a class="result__snippet">A language empowering everyone to build reliable software.</a>
  </div>
</div>"#;
        let results = parse_duckduckgo_html(html);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust Programming Language");
        assert_eq!(results[0].url, "https://www.rust-lang.org/");
        assert!(results[0].snippet.contains("empowering"));
    }

    #[test]
    fn parse_duckduckgo_html_handles_no_results() {
        assert!(parse_duckduckgo_html("<html><body>sem resultados</body></html>").is_empty());
    }

    #[test]
    fn parse_brave_html_extracts_title_url_and_snippet() {
        let html = r#"<div class="snippet fdb" data-type="web">
    <a href="https://www.rust-lang.org/">
        <div class="title">Rust Programming Language</div>
    </a>
    <div class="content">A language empowering everyone to build reliable software.</div>
</div>"#;
        let results = parse_brave_html(html);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust Programming Language");
        assert_eq!(results[0].url, "https://www.rust-lang.org/");
        assert!(results[0].snippet.contains("empowering"));
    }

    #[test]
    fn parse_mojeek_html_extracts_title_url_and_snippet() {
        let html = r#"<ul class="results-standard">
  <li>
    <a class="ob" href="https://www.rust-lang.org/"></a>
    <h2><a href="https://www.rust-lang.org/">Rust Programming Language</a></h2>
    <p class="s">A language empowering everyone to build reliable software.</p>
  </li>
</ul>"#;
        let results = parse_mojeek_html(html);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust Programming Language");
        assert_eq!(results[0].url, "https://www.rust-lang.org/");
        assert!(results[0].snippet.contains("empowering"));
    }

    #[test]
    fn normalize_url_strips_www_trailing_slash_and_tracking_params() {
        assert_eq!(
            normalize_url("https://www.example.com/page/?utm_source=x&id=42"),
            normalize_url("http://example.com/page?id=42"),
        );
    }

    #[test]
    fn merge_engine_results_deduplicates_and_ranks_by_combined_score() {
        let item = |title: &str, url: &str| SearchResultItem {
            title: title.to_string(),
            url: url.to_string(),
            snippet: "s".to_string(),
        };
        // "rust-lang.org" aparece em 1o lugar no DDG e 2o no Brave - deve
        // ficar acima de algo que so uma fonte trouxe.
        let engine_results = vec![
            ("duckduckgo", vec![item("Rust", "https://www.rust-lang.org/"), item("Other", "https://example.com/other")]),
            ("brave", vec![item("Other2", "https://example.com/other2"), item("Rust lang", "https://rust-lang.org/")]),
        ];
        let merged = merge_engine_results(engine_results);
        assert_eq!(merged[0].url, "https://www.rust-lang.org/");
        assert_eq!(merged.len(), 3, "rust-lang.org duplicado deveria virar 1 resultado so");
    }

    #[tokio::test]
    #[ignore]
    async fn fetch_extracts_text() {
        let out = fetch("https://example.com").await.unwrap();
        println!("{out}");
        assert!(out.to_lowercase().contains("example"));
    }

    #[tokio::test]
    async fn fetch_blocks_private_targets() {
        for url in [
            "http://127.0.0.1:8888/",
            "http://localhost/",
            "http://192.168.1.1/",
        ] {
            let err = fetch(url).await.unwrap_err();
            assert!(err.to_string().contains("bloqueado") || err.to_string().contains("localhost"));
        }
    }
}
