//! Model Context Protocol: conecta em servidores MCP externos configurados
//! pelo usuario e expoe as ferramentas deles pro agente, igual as ferramentas
//! embutidas (grep/edit_file/etc.) — so que rodando fora do processo do
//! Cerne, num servidor separado (o padrao real de "conectar ferramenta
//! externa" que Claude Code/Claude Desktop/etc. usam).
//!
//! Usa a crate `rmcp` (SDK oficial em Rust do modelcontextprotocol.io,
//! `github.com/modelcontextprotocol/rust-sdk`) direto — nao ha o que
//! vendorizar aqui, e o mesmo padrao das outras portas desta sessao (crate
//! real e mantida, nao reimplementacao caseira do protocolo).
//!
//! Transporte suportado: só stdio (`TokioChildProcess` — sobe o servidor MCP
//! como subprocesso e fala JSON-RPC pela stdin/stdout dele), que cobre a
//! grande maioria dos servidores MCP reais distribuidos hoje (`npx
//! @escopo/pacote`, `uvx pacote`, um binario local). SSE/HTTP streamable
//! ficam de fora por enquanto — soma reduzida ao caso de uso mais comum.
//!
//! Ferramentas de servidores MCP aparecem namespaced como
//! `mcp__{servidor}__{tool}` (mesma convencao que clientes MCP reais usam
//! pra nao colidir tool de servidores diferentes com nome igual).

use crate::models::{ToolFunctionSpec, ToolSpec};
use anyhow::{anyhow, Result};
use rmcp::model::{CallToolRequestParams, ContentBlock};
use rmcp::service::RunningService;
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use rmcp::{RoleClient, ServiceExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tokio::sync::Mutex;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

fn mcp_servers_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("mcp_servers.json")
}

fn legacy_toml_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("mcp_servers.toml")
}

/// Formato salvo em disco: `{ "mcpServers": { "nome": { command, args, env,
/// enabled } } }` — o mesmo formato de objeto-por-nome que a maioria dos
/// servidores MCP reais documenta no README (ex: config do Claude Desktop),
/// bem mais reconhecivel de colar/copiar do que uma lista TOML. `name` fica
/// de fora do struct salvo (e a CHAVE do objeto, nao um campo) mas continua
/// existindo em `McpServerConfig` internamente — o resto do Cerne (roteamento
/// de tool `mcp__{servidor}__{tool}`, UI) trabalha com a struct completa.
#[derive(Serialize, Deserialize)]
struct StoredServer {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: HashMap<String, String>,
    #[serde(default = "default_true")]
    enabled: bool,
}

#[derive(Serialize, Deserialize, Default)]
struct StoredConfig {
    #[serde(rename = "mcpServers", default)]
    mcp_servers: HashMap<String, StoredServer>,
}

/// Carrega os servidores MCP configurados; devolve lista vazia (nao erro) se
/// o arquivo ainda nao existe — estado normal antes do usuario configurar
/// o primeiro servidor. Faz migracao automatica e unica do formato antigo
/// (`mcp_servers.toml`, antes do formato mudar pro `.json` object-keyed mais
/// comum) na primeira leitura, se o `.json` ainda nao existir.
pub fn load_servers(app_data_dir: &Path) -> Result<Vec<McpServerConfig>> {
    let path = mcp_servers_path(app_data_dir);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return migrate_legacy_toml(app_data_dir),
    };
    let stored: StoredConfig =
        serde_json::from_str(&text).map_err(|e| anyhow!("mcp_servers.json invalido: {e}"))?;
    let mut servers: Vec<McpServerConfig> = stored
        .mcp_servers
        .into_iter()
        .map(|(name, s)| McpServerConfig {
            name,
            command: s.command,
            args: s.args,
            env: s.env,
            enabled: s.enabled,
        })
        .collect();
    servers.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(servers)
}

fn migrate_legacy_toml(app_data_dir: &Path) -> Result<Vec<McpServerConfig>> {
    let Ok(text) = std::fs::read_to_string(legacy_toml_path(app_data_dir)) else {
        return Ok(Vec::new());
    };
    #[derive(Deserialize, Default)]
    struct LegacyWrapper {
        #[serde(default)]
        server: Vec<McpServerConfig>,
    }
    let Ok(wrapper) = toml::from_str::<LegacyWrapper>(&text) else {
        return Ok(Vec::new());
    };
    if !wrapper.server.is_empty() {
        // Melhor esforco - se a escrita falhar, a config antiga ainda existe
        // e a migracao so tenta de novo na proxima vez.
        let _ = save_servers(app_data_dir, &wrapper.server);
    }
    Ok(wrapper.server)
}

pub fn save_servers(app_data_dir: &Path, servers: &[McpServerConfig]) -> Result<()> {
    let mcp_servers = servers
        .iter()
        .map(|s| {
            (
                s.name.clone(),
                StoredServer {
                    command: s.command.clone(),
                    args: s.args.clone(),
                    env: s.env.clone(),
                    enabled: s.enabled,
                },
            )
        })
        .collect();
    let stored = StoredConfig { mcp_servers };
    std::fs::create_dir_all(app_data_dir)?;
    let text = serde_json::to_string_pretty(&stored)?;
    std::fs::write(mcp_servers_path(app_data_dir), text)?;
    Ok(())
}

type McpClient = RunningService<RoleClient, ()>;

/// Monta o `Command` do processo do servidor MCP. No Windows, comandos do
/// ecossistema Node (`npx`, `npm`, etc.) sao na verdade shims `.cmd`, e
/// `Command::new` do Rust nao resolve isso sozinho (nao passa pelo
/// `PATHEXT` que o `cmd.exe` resolveria) — falha com "program not found"
/// mesmo com o `npx` funcionando normal no terminal. **Achado testando ao
/// vivo**: exatamente esse erro ao conectar num servidor MCP real via
/// `npx -y @modelcontextprotocol/server-everything`. Mesmo motivo pelo qual
/// Constrói o comando do servidor MCP usando o shell detectado pelo sistema.
/// No Windows, usa PowerShell (pwsh > powershell > cmd) com CREATE_NO_WINDOW.
/// Em outros SOs, roda o comando direto (MCP servers são executáveis).
fn build_command(command: &str) -> Command {
    #[cfg(windows)]
    {
        let shell = crate::agent::shell::detect_shell();
        let mut cmd = Command::new(&shell.executable);
        for arg in &shell.args_prefix {
            cmd.arg(arg);
        }
        cmd.arg(command);
        crate::agent::shell::apply_creation_flags(&mut cmd);
        cmd
    }
    #[cfg(not(windows))]
    {
        Command::new(command)
    }
}

/// Conexoes ja estabelecidas com servidores MCP, mantidas vivas entre
/// chamadas de ferramenta (reconectar a cada tool call seria lento — muitos
/// servidores MCP levam tempo real pra inicializar). Uma instancia vive em
/// `AppState`, compartilhada entre sessoes (igual `background_jobs`).
#[derive(Default)]
pub struct McpClients(Mutex<HashMap<String, McpClient>>);

impl McpClients {
    async fn ensure_connected(&self, server: &McpServerConfig) -> Result<()> {
        let mut clients = self.0.lock().await;
        if clients.contains_key(&server.name) {
            return Ok(());
        }
        let args = server.args.clone();
        let env = server.env.clone();
        let transport = TokioChildProcess::new(build_command(&server.command).configure(|cmd| {
            cmd.args(&args);
            for (key, value) in &env {
                cmd.env(key, value);
            }
        }))
        .map_err(|e| {
            anyhow!(
                "nao foi possivel iniciar o servidor MCP '{}': {e}",
                server.name
            )
        })?;
        let client = ()
            .serve(transport)
            .await
            .map_err(|e| anyhow!("falha ao conectar no servidor MCP '{}': {e}", server.name))?;
        clients.insert(server.name.clone(), client);
        Ok(())
    }

    /// Conecta (se ainda nao conectado) em todo servidor habilitado e
    /// devolve as tools deles, namespaced `mcp__{servidor}__{tool}`.
    /// Servidor que falha ao conectar/listar e so pulado (nao derruba os
    /// outros nem a sessao inteira) — mesma filosofia best-effort de outras
    /// integracoes externas do Cerne.
    pub async fn tool_specs(&self, servers: &[McpServerConfig]) -> Vec<ToolSpec> {
        let mut specs = Vec::new();
        for server in servers.iter().filter(|s| s.enabled) {
            if let Err(e) = self.ensure_connected(server).await {
                eprintln!("[mcp] {e}");
                continue;
            }
            let tools = {
                let clients = self.0.lock().await;
                let Some(client) = clients.get(&server.name) else {
                    continue;
                };
                client.list_all_tools().await
            };
            let Ok(tools) = tools else { continue };
            for tool in tools {
                specs.push(ToolSpec {
                    kind: "function".to_string(),
                    function: ToolFunctionSpec {
                        name: format!("mcp__{}__{}", server.name, tool.name),
                        description: tool.description.map(|d| d.to_string()).unwrap_or_default(),
                        parameters: serde_json::Value::Object((*tool.input_schema).clone()),
                    },
                });
            }
        }
        specs
    }

    /// Executa uma tool MCP pelo nome namespaced (`mcp__{servidor}__{tool}`).
    pub async fn call(
        &self,
        namespaced_name: &str,
        arguments: serde_json::Value,
    ) -> Result<String> {
        let (server_name, tool_name) = parse_namespaced_name(namespaced_name)?;
        let clients = self.0.lock().await;
        let client = clients
            .get(server_name)
            .ok_or_else(|| anyhow!("servidor MCP '{server_name}' nao conectado (chame tool_specs primeiro, ou verifique se esta habilitado)"))?;
        let args_map = arguments.as_object().cloned().unwrap_or_default();
        let result = client
            .call_tool(CallToolRequestParams::new(tool_name.to_string()).with_arguments(args_map))
            .await
            .map_err(|e| {
                anyhow!("erro chamando tool MCP '{tool_name}' no servidor '{server_name}': {e}")
            })?;
        Ok(extract_text(&result))
    }

    /// Encerra toda conexao MCP ativa — chamado ao fechar o app.
    pub async fn disconnect_all(&self) {
        let mut clients = self.0.lock().await;
        for (_, client) in clients.drain() {
            let _ = client.cancel().await;
        }
    }
}

const TEST_CONNECTION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// Testa uma configuracao de servidor MCP ANTES de salvar — sobe o processo,
/// faz o handshake MCP e lista as tools, tudo numa conexao descartavel que
/// nunca entra no pool compartilhado de `McpClients` (o de baixo e pra
/// servidor ja confirmado/habilitado; aqui o usuario ainda pode estar so
/// experimentando valores no formulario). Devolve os nomes das tools em caso
/// de sucesso, ou um erro com mensagem especifica o bastante pra apontar o
/// que checar (comando nao encontrado, handshake que nunca responde, etc.) —
/// mesma ideia do teste de conexao que o LM Studio faz antes de confirmar um
/// servidor MCP novo.
pub async fn test_connection(server: &McpServerConfig) -> Result<Vec<String>> {
    let args = server.args.clone();
    let env = server.env.clone();
    let transport = TokioChildProcess::new(build_command(&server.command).configure(|cmd| {
        cmd.args(&args);
        for (key, value) in &env {
            cmd.env(key, value);
        }
    }))
    .map_err(|e| {
        anyhow!(
            "nao foi possivel iniciar o comando '{}': {e} — confira se ele existe e esta no PATH",
            server.command
        )
    })?;

    let attempt = async move {
        let client = ()
            .serve(transport)
            .await
            .map_err(|e| anyhow!("processo iniciou, mas o handshake MCP falhou: {e} — confira os argumentos e se e mesmo um servidor MCP"))?;
        let tools = client
            .list_all_tools()
            .await
            .map_err(|e| anyhow!("conectou, mas falhou ao listar as tools: {e}"));
        let _ = client.cancel().await;
        tools
    };

    match tokio::time::timeout(TEST_CONNECTION_TIMEOUT, attempt).await {
        Ok(result) => result.map(|tools| tools.into_iter().map(|t| t.name.to_string()).collect()),
        Err(_) => Err(anyhow!(
            "tempo esgotado ({}s) esperando o servidor responder ao handshake — o processo pode ter travado, ou nunca vai responder",
            TEST_CONNECTION_TIMEOUT.as_secs()
        )),
    }
}

fn parse_namespaced_name(name: &str) -> Result<(&str, &str)> {
    let rest = name
        .strip_prefix("mcp__")
        .ok_or_else(|| anyhow!("nome de tool MCP invalido (esperava prefixo 'mcp__'): {name}"))?;
    rest.split_once("__").ok_or_else(|| {
        anyhow!("nome de tool MCP invalido (esperava 'mcp__servidor__tool'): {name}")
    })
}

fn extract_text(result: &rmcp::model::CallToolResult) -> String {
    let text = result
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    if result.is_error == Some(true) {
        format!("[erro reportado pelo servidor MCP] {text}")
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_namespaced_name_splits_server_and_tool() {
        assert_eq!(
            parse_namespaced_name("mcp__github__search_issues").unwrap(),
            ("github", "search_issues")
        );
    }

    #[test]
    fn parse_namespaced_name_handles_tool_names_with_double_underscore() {
        // split_once para no PRIMEIRO "__", entao o resto (incluindo mais
        // "__") vira o nome da tool inteiro - importante pra tools MCP cujo
        // proprio nome tem underscore duplo.
        assert_eq!(
            parse_namespaced_name("mcp__github__search__issues").unwrap(),
            ("github", "search__issues")
        );
    }

    #[test]
    fn parse_namespaced_name_errors_without_mcp_prefix() {
        assert!(parse_namespaced_name("grep").is_err());
    }

    #[test]
    fn parse_namespaced_name_errors_without_server_tool_separator() {
        assert!(parse_namespaced_name("mcp__onlyserver").is_err());
    }

    #[test]
    fn load_servers_returns_empty_when_file_missing() {
        let dir = std::env::temp_dir().join(format!("cerne-mcp-test-{}", uuid::Uuid::new_v4()));
        let servers = load_servers(&dir).unwrap();
        assert!(servers.is_empty());
    }

    #[test]
    fn save_then_load_roundtrips_server_config() {
        let dir = std::env::temp_dir().join(format!("cerne-mcp-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let servers = vec![McpServerConfig {
            name: "test-server".to_string(),
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-everything".to_string(),
            ],
            env: HashMap::new(),
            enabled: true,
        }];
        save_servers(&dir, &servers).unwrap();
        let loaded = load_servers(&dir).unwrap();
        assert_eq!(loaded, servers);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_servers_writes_the_common_mcp_servers_json_shape() {
        // Formato object-keyed por nome (`{"mcpServers": {"nome": {...}}}`) e
        // o que a maioria dos servidores MCP reais documenta pra colar direto
        // - confirma que e exatamente isso que sai no arquivo, nao uma lista.
        let dir = std::env::temp_dir().join(format!("cerne-mcp-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut env = HashMap::new();
        env.insert("TOKEN".to_string(), "abc".to_string());
        let servers = vec![McpServerConfig {
            name: "github".to_string(),
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-github".to_string(),
            ],
            env,
            enabled: true,
        }];
        save_servers(&dir, &servers).unwrap();

        let text = std::fs::read_to_string(mcp_servers_path(&dir)).unwrap();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(json["mcpServers"]["github"]["command"], "npx");
        assert_eq!(json["mcpServers"]["github"]["env"]["TOKEN"], "abc");
        assert!(
            json["mcpServers"].get("name").is_none(),
            "nome deve ser a chave, nao um campo dentro do objeto"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_servers_migrates_legacy_toml_once() {
        let dir = std::env::temp_dir().join(format!("cerne-mcp-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            legacy_toml_path(&dir),
            "[[server]]\nname = \"old-server\"\ncommand = \"npx\"\nargs = []\nenabled = true\n",
        )
        .unwrap();

        let loaded = load_servers(&dir).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "old-server");

        // Migracao deveria ter escrito o .json novo, entao uma segunda leitura
        // nao depende mais do .toml antigo continuar existindo.
        assert!(mcp_servers_path(&dir).exists());
        std::fs::remove_file(legacy_toml_path(&dir)).unwrap();
        let loaded_again = load_servers(&dir).unwrap();
        assert_eq!(loaded_again, loaded);

        std::fs::remove_dir_all(&dir).ok();
    }
}
