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

/// `reqwest::Client::new()` sozinho NÃO tem timeout nenhum — se o provider
/// (scraping de DuckDuckGo/Brave/Mojeek, ou qualquer API) travar a conexão
/// sem responder, a chamada fica pendurada PRA SEMPRE, o turno nunca
/// termina e a bolinha de "processando" pisca sem parar. Bug real
/// encontrado testando ao vivo, 2026-08-20: duas sessões diferentes
/// travaram juntas em "Buscou na web" por minutos, sem timeout nenhum pra
/// desistir. Todo `reqwest::Client` deste arquivo usa este helper agora.
fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .unwrap_or_default()
}

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
        SearchProviderKind::Serper => {
            let key = crate::search::get_key(SearchProviderKind::Serper).ok_or_else(|| {
                anyhow!(
                    "busca via Serper selecionada mas sem chave de API configurada — adicione em Configurações → Busca na web"
                )
            })?;
            search_serper(query, &key).await?
        }
        SearchProviderKind::Exa => {
            let key = crate::search::get_key(SearchProviderKind::Exa).ok_or_else(|| {
                anyhow!(
                    "busca via Exa selecionada mas sem chave de API configurada — adicione em Configurações → Busca na web"
                )
            })?;
            search_exa(query, &key).await?
        }
        SearchProviderKind::GoogleCse => {
            let key = crate::search::get_key(SearchProviderKind::GoogleCse).ok_or_else(|| {
                anyhow!(
                    "busca via Google Custom Search selecionada mas sem chave de API configurada — adicione em Configurações → Busca na web"
                )
            })?;
            if config.google_cse_id.trim().is_empty() {
                return Err(anyhow!(
                    "busca via Google Custom Search selecionada mas sem Search Engine ID (cx) configurado — adicione em Configurações → Busca na web"
                ));
            }
            search_google_cse(query, &key, &config.google_cse_id).await?
        }
        SearchProviderKind::Bing => {
            let key = crate::search::get_key(SearchProviderKind::Bing).ok_or_else(|| {
                anyhow!(
                    "busca via Bing/Azure selecionada mas sem chave de API configurada — adicione em Configurações → Busca na web"
                )
            })?;
            search_bing(query, &key, &config.bing_endpoint).await?
        }
    };
    Ok(format_results(&results))
}

/// Limite de chamadas concorrentes a `search_auto` no app inteiro — as 3
/// fontes que ele usa (DuckDuckGo, pagina publica do Brave, Mojeek) sao
/// scraping sem API oficial, entao rajadas de requisicoes simultaneas
/// aumentam muito a chance de bloqueio anti-bot. Antes só existia uma
/// chamada de busca por vez; com `task` em paralelo (Fase A4) e sub-agente
/// tendo acesso a `web_search` (Fase A6/T45), um turno agora pode disparar
/// o `web_search` do agente principal AO MESMO TEMPO que 2+ sub-agentes
/// também chamam `web_search` — cada chamada já dispara 3 requisições em
/// paralelo (uma por fonte) e pode ter várias queries na mesma chamada
/// (`search_many`, mais 3x cada), multiplicando rápido a carga simultânea
/// nas mesmas 3 fontes gratuitas. O semáforo enfileira o excesso em vez de
/// disparar tudo de uma vez — não impede a concorrência real de `task`,
/// só a parte de busca na web despachada por eles.
fn search_auto_semaphore() -> &'static tokio::sync::Semaphore {
    static SEM: std::sync::OnceLock<tokio::sync::Semaphore> = std::sync::OnceLock::new();
    SEM.get_or_init(|| tokio::sync::Semaphore::new(2))
}

/// Atraso antes da 2a tentativa de uma fonte que falhou — só pra dar um
/// respiro numa falha transiente de rede/rate-limit momentâneo, não uma
/// espera longa.
const RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(400);

/// Roda `f` uma vez; se falhar (erro de rede/HTTP, não "0 resultados" — isso
/// já é um resultado válido, só fraco), espera `RETRY_DELAY` e tenta mais
/// uma vez antes de desistir. Devolve o erro da 1a tentativa se as duas
/// falharem (geralmente mais informativo que o da 2a, que costuma ser o
/// mesmo bloqueio batendo de novo).
async fn with_retry<F, Fut>(f: F) -> Result<Vec<SearchResultItem>>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<Vec<SearchResultItem>>>,
{
    match f().await {
        Ok(items) => Ok(items),
        Err(first_err) => {
            tokio::time::sleep(RETRY_DELAY).await;
            f().await.map_err(|_| first_err)
        }
    }
}

/// Agregador multi-engine keyless (DuckDuckGo, a página pública do Brave,
/// Mojeek) — cadeia de fallback com retry (estudei como o projeto Odysseus
/// estrutura busca: tenta 1 provider por vez com retry, só escala pro
/// próximo em falha/vazio, em vez de disparar tudo de uma vez) em vez do
/// fan-out sempre-3-em-paralelo de antes. **DuckDuckGo primeiro, sozinho**:
/// na maioria das buscas ele já traz resultado suficiente sozinho, e um só
/// request reduz MUITO a chance de bloqueio anti-bot comparado a martelar
/// as 3 fontes toda hora — só escala pra Brave+Mojeek em paralelo (e faz o
/// merge multi-engine com pontuação por posição, mesma ideia do SearXNG de
/// antes) quando o DuckDuckGo vier fraco/vazio/com erro mesmo após retry.
async fn search_auto(query: &str) -> Result<Vec<SearchResultItem>> {
    let _permit = search_auto_semaphore()
        .acquire()
        .await
        .expect("search_auto_semaphore nunca é fechado");

    let ddg = with_retry(|| search_duckduckgo(query)).await;
    if let Ok(items) = &ddg {
        if items.len() >= MIN_RESULTS {
            return Ok(merge_engine_results(vec![("duckduckgo", items.clone())]));
        }
    }

    // DuckDuckGo sozinho não bastou (fraco, vazio ou falhou mesmo com
    // retry) — escala pras outras 2 fontes em paralelo. O resultado do DDG
    // (mesmo que fraco) ainda entra no merge se tiver trazido algo.
    let (brave, mojeek) = tokio::join!(
        with_retry(|| search_brave_html(query)),
        with_retry(|| search_mojeek(query)),
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
    google_cse_id: Option<&str>,
    bing_endpoint: Option<&str>,
) -> Result<usize> {
    const TEST_QUERY: &str = "rust programming language";
    let results = match provider {
        SearchProviderKind::Auto => search_auto(TEST_QUERY).await?,
        SearchProviderKind::Brave => {
            let key = api_key.ok_or_else(|| anyhow!("informe a chave de API da Brave"))?;
            search_brave(TEST_QUERY, key).await?
        }
        SearchProviderKind::Tavily => {
            let key = api_key.ok_or_else(|| anyhow!("informe a chave de API da Tavily"))?;
            search_tavily(TEST_QUERY, key).await?
        }
        SearchProviderKind::Searxng => {
            let url = searxng_url.ok_or_else(|| anyhow!("informe a URL do SearXNG"))?;
            search_searxng(TEST_QUERY, url).await?
        }
        SearchProviderKind::Serper => {
            let key = api_key.ok_or_else(|| anyhow!("informe a chave de API da Serper"))?;
            search_serper(TEST_QUERY, key).await?
        }
        SearchProviderKind::Exa => {
            let key = api_key.ok_or_else(|| anyhow!("informe a chave de API da Exa"))?;
            search_exa(TEST_QUERY, key).await?
        }
        SearchProviderKind::GoogleCse => {
            let key = api_key.ok_or_else(|| anyhow!("informe a chave de API do Google"))?;
            let cx = google_cse_id.ok_or_else(|| anyhow!("informe o Search Engine ID (cx)"))?;
            search_google_cse(TEST_QUERY, key, cx).await?
        }
        SearchProviderKind::Bing => {
            let key = api_key.ok_or_else(|| anyhow!("informe a chave de API da Bing/Azure"))?;
            let endpoint = bing_endpoint.unwrap_or("https://api.bing.microsoft.com/v7.0/search");
            search_bing(TEST_QUERY, key, endpoint).await?
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
    let client = http_client();
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
    let client = http_client();
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
    let client = http_client();
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
    let client = http_client();
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
    let client = http_client();
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
    let client = http_client();
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

#[derive(Debug, serde::Serialize)]
struct SerperRequest<'a> {
    q: &'a str,
}

#[derive(Debug, Deserialize)]
struct SerperResponse {
    #[serde(default)]
    organic: Vec<SerperResult>,
}

#[derive(Debug, Deserialize)]
struct SerperResult {
    #[serde(default)]
    title: String,
    #[serde(default)]
    link: String,
    #[serde(default)]
    snippet: String,
}

/// Resultados do Google via API (google.serper.dev) — nao e a API oficial do
/// Google, mas retorna o mesmo indice, com um plano gratuito generoso e
/// integracao bem mais simples que o Custom Search oficial (sem precisar
/// configurar um "motor de busca programavel" separado).
async fn search_serper(query: &str, api_key: &str) -> Result<Vec<SearchResultItem>> {
    let client = http_client();
    let resp = client
        .post("https://google.serper.dev/search")
        .header("X-API-KEY", api_key)
        .header("Content-Type", "application/json")
        .json(&SerperRequest { q: query })
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| anyhow!("falha ao buscar na Serper: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("Serper respondeu {status}: {body}"));
    }
    let parsed: SerperResponse = resp
        .json()
        .await
        .map_err(|e| anyhow!("resposta invalida da Serper: {e}"))?;

    Ok(parsed
        .organic
        .into_iter()
        .map(|r| SearchResultItem {
            title: r.title,
            url: r.link,
            snippet: r.snippet,
        })
        .collect())
}

#[derive(Debug, serde::Serialize)]
struct ExaRequest<'a> {
    query: &'a str,
    #[serde(rename = "numResults")]
    num_results: u32,
    contents: ExaContents,
}

#[derive(Debug, serde::Serialize)]
struct ExaContents {
    text: ExaTextOpts,
}

#[derive(Debug, serde::Serialize)]
struct ExaTextOpts {
    #[serde(rename = "maxCharacters")]
    max_characters: u32,
}

#[derive(Debug, Deserialize)]
struct ExaResponse {
    #[serde(default)]
    results: Vec<ExaResult>,
}

#[derive(Debug, Deserialize)]
struct ExaResult {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    url: String,
    #[serde(default)]
    text: Option<String>,
}

/// Busca "neural"/semantica da Exa (api.exa.ai) — indice voltado pra achar
/// paginas por significado em vez de so por palavra-chave, costuma trazer
/// fontes tecnicas/de nicho melhor que um motor generico. `contents.text`
/// pedido explicitamente porque a resposta padrao da Exa nao inclui trecho
/// nenhum, so titulo/URL/metadados.
async fn search_exa(query: &str, api_key: &str) -> Result<Vec<SearchResultItem>> {
    let client = http_client();
    let resp = client
        .post("https://api.exa.ai/search")
        .header("x-api-key", api_key)
        .json(&ExaRequest {
            query,
            num_results: 10,
            contents: ExaContents {
                text: ExaTextOpts { max_characters: 300 },
            },
        })
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| anyhow!("falha ao buscar na Exa: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("Exa respondeu {status}: {body}"));
    }
    let parsed: ExaResponse = resp
        .json()
        .await
        .map_err(|e| anyhow!("resposta invalida da Exa: {e}"))?;

    Ok(parsed
        .results
        .into_iter()
        .map(|r| SearchResultItem {
            title: r.title.unwrap_or_default(),
            url: r.url,
            snippet: r.text.unwrap_or_default(),
        })
        .collect())
}

#[derive(Debug, Deserialize)]
struct GoogleCseResponse {
    #[serde(default)]
    items: Vec<GoogleCseResult>,
}

#[derive(Debug, Deserialize)]
struct GoogleCseResult {
    #[serde(default)]
    title: String,
    #[serde(default)]
    link: String,
    #[serde(default)]
    snippet: String,
}

/// API oficial do Google (Programmable Search Engine / Custom Search JSON
/// API) — exige uma chave de API E um Search Engine ID ("cx") criado no
/// console do Google (console.cloud.google.com + programmablesearchengine.google.com),
/// mais burocratico de configurar que Serper/Brave/Tavily mas e a fonte
/// oficial pra quem ja tem isso montado.
async fn search_google_cse(query: &str, api_key: &str, cx: &str) -> Result<Vec<SearchResultItem>> {
    let client = http_client();
    let resp = client
        .get("https://www.googleapis.com/customsearch/v1")
        .query(&[("key", api_key), ("cx", cx), ("q", query)])
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| anyhow!("falha ao buscar no Google Custom Search: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("Google Custom Search respondeu {status}: {body}"));
    }
    let parsed: GoogleCseResponse = resp
        .json()
        .await
        .map_err(|e| anyhow!("resposta invalida do Google Custom Search: {e}"))?;

    Ok(parsed
        .items
        .into_iter()
        .map(|r| SearchResultItem {
            title: r.title,
            url: r.link,
            snippet: r.snippet,
        })
        .collect())
}

#[derive(Debug, Deserialize)]
struct BingResponse {
    #[serde(default)]
    #[serde(rename = "webPages")]
    web_pages: Option<BingWebPages>,
}

#[derive(Debug, Deserialize)]
struct BingWebPages {
    #[serde(default)]
    value: Vec<BingResult>,
}

#[derive(Debug, Deserialize)]
struct BingResult {
    #[serde(default)]
    name: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    snippet: String,
}

/// Bing Web Search via Azure AI Search (`Ocp-Apim-Subscription-Key`) —
/// `endpoint` e configuravel porque recursos do Azure ganham um endpoint
/// proprio por regiao/recurso, diferente do dominio fixo da API classica.
async fn search_bing(query: &str, api_key: &str, endpoint: &str) -> Result<Vec<SearchResultItem>> {
    let client = http_client();
    let resp = client
        .get(endpoint)
        .header("Ocp-Apim-Subscription-Key", api_key)
        .query(&[("q", query), ("count", "10")])
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| anyhow!("falha ao buscar na Bing/Azure Search: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("Bing/Azure Search respondeu {status}: {body}"));
    }
    let parsed: BingResponse = resp
        .json()
        .await
        .map_err(|e| anyhow!("resposta invalida da Bing/Azure Search: {e}"))?;

    Ok(parsed
        .web_pages
        .map(|wp| wp.value)
        .unwrap_or_default()
        .into_iter()
        .map(|r| SearchResultItem {
            title: r.name,
            url: r.url,
            snippet: r.snippet,
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

    let client = http_client();
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

pub(crate) async fn validate_public_url(url_str: &str) -> Result<url::Url> {
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
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn with_retry_succeeds_on_second_attempt_after_first_error() {
        let attempts = AtomicUsize::new(0);
        let result = with_retry(|| {
            let n = attempts.fetch_add(1, Ordering::SeqCst);
            async move {
                if n == 0 {
                    Err(anyhow!("falha transiente simulada"))
                } else {
                    Ok(vec![SearchResultItem {
                        title: "ok".to_string(),
                        url: "https://example.com".to_string(),
                        snippet: "s".to_string(),
                    }])
                }
            }
        })
        .await;
        assert!(result.is_ok());
        assert_eq!(attempts.load(Ordering::SeqCst), 2, "deveria ter tentado 2 vezes");
    }

    #[tokio::test]
    async fn with_retry_gives_up_after_two_failures_returning_first_error() {
        let attempts = AtomicUsize::new(0);
        let result = with_retry(|| {
            attempts.fetch_add(1, Ordering::SeqCst);
            async move { Err::<Vec<SearchResultItem>, _>(anyhow!("falha permanente")) }
        })
        .await;
        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 2, "deveria ter tentado exatamente 2 vezes, sem 3a tentativa");
    }

    #[tokio::test]
    async fn with_retry_does_not_retry_on_ok_even_if_empty() {
        // Resultado vazio (Ok(vec![])) e um resultado valido, so fraco - nao
        // e a mesma coisa que falha de rede, entao nao deveria disparar retry.
        let attempts = AtomicUsize::new(0);
        let result = with_retry(|| {
            attempts.fetch_add(1, Ordering::SeqCst);
            async move { Ok::<Vec<SearchResultItem>, anyhow::Error>(vec![]) }
        })
        .await;
        assert!(result.unwrap().is_empty());
        assert_eq!(attempts.load(Ordering::SeqCst), 1, "Ok vazio nao deveria disparar retry");
    }

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
