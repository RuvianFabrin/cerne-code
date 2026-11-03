//! Bounded native HTTP fetch with DNS pinning and redirect SSRF checks.
use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};
use url::Url;

pub fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_unspecified()
                || ip.is_multicast()
                || ip.is_broadcast()
                || a == 0
                || a >= 240
                || (a == 100 && (64..=127).contains(&b))
                || (a == 192 && b == 0 && (c == 0 || c == 2))
                || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113))
        }
        IpAddr::V6(ip) => {
            if let Some(v4) = ip.to_ipv4_mapped() {
                return public_ip(IpAddr::V4(v4));
            }
            let s = ip.segments();
            !ip.is_loopback()
                && !ip.is_unspecified()
                && !ip.is_multicast()
                && (s[0] & 0xe000) == 0x2000
                && !(s[0] == 0x2001 && (s[1] == 0xdb8 || s[1] == 0 || s[1] == 2))
        }
    }
}
pub async fn addresses(url: &Url) -> Result<Vec<SocketAddr>> {
    if !["https", "http"].contains(&url.scheme())
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(anyhow!("Somente URLs HTTP(S) publicas sem credenciais"));
    }
    let host = url.host_str().ok_or_else(|| anyhow!("URL sem host"))?;
    if host.eq_ignore_ascii_case("localhost")
        || host.ends_with(".localhost")
        || host.ends_with(".local")
    {
        return Err(anyhow!("SSRF: host local bloqueado"));
    }
    let host_for_dns = host.trim_matches(['[', ']']);
    let addresses: Vec<_> = tokio::time::timeout(
        Duration::from_secs(10),
        tokio::net::lookup_host((host_for_dns, url.port_or_known_default().unwrap_or(443))),
    )
    .await??
    .collect();
    if addresses.is_empty() || addresses.iter().any(|a| !public_ip(a.ip())) {
        return Err(anyhow!(
            "SSRF: endereco privado, reservado ou local bloqueado"
        ));
    }
    Ok(addresses)
}

pub async fn page(value: &str) -> Result<(String, String)> {
    let mut url = Url::parse(value)?;
    for _ in 0..=5 {
        let addresses = addresses(&url).await?;
        let host = url.host_str().unwrap().to_string();
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(30))
            .no_proxy()
            .resolve_to_addrs(&host, &addresses)
            .user_agent("CerneCode/0.1.5 ContextEconomy Rust")
            .build()?;
        let response = client.get(url.clone()).send().await?;
        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .ok_or_else(|| anyhow!("Redirecionamento sem destino"))?
                .to_str()?;
            url = url.join(location)?;
            continue;
        }
        let response = response.error_for_status()?;
        if response
            .content_length()
            .is_some_and(|n| n > super::execute::MAX_BYTES as u64)
        {
            return Err(anyhow!("Pagina excede 50 MB"));
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("")
            .to_string();
        let mut bytes = vec![];
        let mut stream = response.bytes_stream();
        while let Some(piece) = stream.next().await {
            let piece = piece?;
            if bytes.len() + piece.len() > super::execute::MAX_BYTES {
                return Err(anyhow!("Pagina excede 50 MB"));
            }
            bytes.extend_from_slice(&piece);
        }
        let text = crate::encoding::decode(&bytes).0;
        let text = if content_type.contains("html") || text.trim_start().starts_with("<!DOCTYPE") {
            html_to_markdown(&text)
        } else {
            text
        };
        return Ok((text, url.to_string()));
    }
    Err(anyhow!("Excesso de redirecionamentos"))
}

pub fn html_to_markdown(text: &str) -> String {
    // Readable headings, links, lists, code fences and table rows. No scripts.
    let document = scraper::Html::parse_document(text);
    // CSS selectors keep the implementation on the existing scraper dependency.
    let body = scraper::Selector::parse("body").unwrap();
    let root = document
        .select(&body)
        .next()
        .unwrap_or_else(|| document.root_element());
    let mut result = String::new();
    fn walk_element(element: scraper::ElementRef<'_>, out: &mut String) {
        let tag = element.value().name();
        if ["script", "style", "noscript", "svg", "nav", "footer"].contains(&tag) {
            return;
        }
        let heading = match tag {
            "h1" => Some("# "),
            "h2" => Some("## "),
            "h3" => Some("### "),
            "h4" => Some("#### "),
            "h5" => Some("##### "),
            "h6" => Some("###### "),
            _ => None,
        };
        if let Some(heading) = heading {
            out.push_str("\n\n");
            out.push_str(heading);
        }
        if tag == "pre" {
            out.push_str("\n\n```\n");
        }
        if tag == "li" {
            out.push_str("\n- ");
        }
        if tag == "br" {
            out.push('\n');
        }
        if tag == "td" || tag == "th" {
            out.push_str(" | ");
        }
        if tag == "a" {
            out.push('[');
        }
        if tag == "code"
            && element
                .parent()
                .and_then(scraper::ElementRef::wrap)
                .is_none_or(|p| p.value().name() != "pre")
        {
            out.push('`');
        }
        for child in element.children() {
            if let Some(child) = scraper::ElementRef::wrap(child) {
                walk_element(child, out);
            } else if let scraper::node::Node::Text(text) = child.value() {
                out.push_str(text);
            }
        }
        if tag == "code"
            && element
                .parent()
                .and_then(scraper::ElementRef::wrap)
                .is_none_or(|p| p.value().name() != "pre")
        {
            out.push('`');
        }
        if tag == "a" {
            out.push_str("](");
            out.push_str(element.value().attr("href").unwrap_or(""));
            out.push(')');
        }
        if tag == "pre" {
            out.push_str("\n```\n\n");
        }
        if heading.is_some() || ["p", "div", "section", "article", "tr", "ul", "ol"].contains(&tag)
        {
            out.push_str("\n\n");
        }
    }
    walk_element(root, &mut result);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_and_mapped_addresses_are_blocked() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "169.254.169.254",
            "100.64.0.1",
            "::1",
            "::ffff:127.0.0.1",
            "fc00::1",
            "2001:db8::1",
        ] {
            assert!(!public_ip(ip.parse().unwrap()), "{ip}");
        }
        assert!(public_ip("1.1.1.1".parse().unwrap()));
    }
    #[tokio::test]
    async fn local_fetch_is_rejected_before_http() {
        assert!(page("http://127.0.0.1/private")
            .await
            .unwrap_err()
            .to_string()
            .contains("SSRF"));
    }
    #[test]
    fn html_keeps_code_and_omits_scripts() {
        let result = html_to_markdown("<h1>Title</h1><pre><code>fn main(){}</code></pre><script>SECRET</script><a href='/docs'>Guide</a>");
        assert!(result.contains("# Title"));
        assert!(result.contains("fn main(){}"));
        assert!(!result.contains("SECRET"));
        assert!(result.contains("[Guide](/docs)"));
    }
}
