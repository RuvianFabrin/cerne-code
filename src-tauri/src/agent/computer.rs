use crate::models::{ToolFunctionSpec, ToolSpec};
#[cfg(not(feature = "computer_use"))]
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
#[cfg(not(feature = "computer_use"))]
use std::path::Path;

pub struct ComputerOutcome {
    pub text: String,
    pub screenshot_base64: Option<String>,
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

pub fn tool_specs() -> Vec<ToolSpec> {
    vec![
        spec(
            "computer_use_screenshot",
            "Captura screenshot do MONITOR PRIMARIO apenas. Retorna a imagem + metadata (resolucao, offset, total de monitores). SEMPRE use esta tool ANTES de qualquer click/type para ver o estado atual. Se a aplicacao alvo nao estiver visivel no monitor primario, peça ao usuario para move-la. REQUER modelo com visao.",
            json!({
                "type": "object",
                "properties": {
                    "window_title": { "type": "string", "description": "Titulo parcial da janela (opcional, vazio = tela inteira)" }
                },
                "required": []
            }),
        ),
        spec(
            "computer_use_click",
            "Clica em coordenadas do MONITOR PRIMARIO (pixels relativos ao screenshot). SEMPRE chame computer_use_screenshot ANTES. As coordenadas sao relativas ao canto superior esquerdo do monitor primario (0,0). O resultado inclui screenshot pos-clique. REQUER modelo com visao.",
            json!({
                "type": "object",
                "properties": {
                    "x": { "type": "integer", "description": "Coordenada X em pixels de tela" },
                    "y": { "type": "integer", "description": "Coordenada Y em pixels de tela" },
                    "button": { "type": "string", "enum": ["left", "right", "middle"], "description": "Botao do mouse (default: left)" },
                    "window_title": { "type": "string", "description": "Titulo parcial da janela alvo (opcional). Se informado, ela e trazida pro primeiro plano ANTES do clique - use sempre que a aplicacao alvo nao for a que o usuario estava usando (ex: o Cerne Code) no momento do pedido, senao o clique cai na janela errada." }
                },
                "required": ["x", "y"]
            }),
        ),
        spec(
            "computer_use_type_text",
            "Digita texto via teclado no elemento focado. Foque o campo desejado ANTES, com computer_use_click (precisa de visao) ou computer_use_click_element (via AX-tree, nao precisa de visao). Max 500 chars por chamada. Nao requer visao (desde que o foco tenha sido feito por outra via).",
            json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "Texto a digitar (max 500 chars)" },
                    "window_title": { "type": "string", "description": "Titulo parcial da janela alvo (opcional). Se informado, ela e trazida pro primeiro plano ANTES de digitar - use sempre que a aplicacao alvo nao for a que o usuario estava usando (ex: o Cerne Code) no momento do pedido." }
                },
                "required": ["text"]
            }),
        ),
        spec(
            "computer_use_press_key",
            "Pressiona uma tecla ou combinacao (ex: ctrl+c, alt+tab, enter). Combinacoes destrutivas (alt+f4, ctrl+shift+esc) sao bloqueadas. Nao requer visao.",
            json!({
                "type": "object",
                "properties": {
                    "key": { "type": "string", "description": "Tecla: return, tab, escape, up, down, left, right, space, delete, home, end, pageup, pagedown, f1-f12, ou letra/digito" },
                    "modifiers": { "type": "array", "items": { "type": "string" }, "description": "Modificadores: ctrl, shift, alt, win" },
                    "window_title": { "type": "string", "description": "Titulo parcial da janela alvo (opcional). Se informado, ela e trazida pro primeiro plano ANTES de pressionar a tecla." }
                },
                "required": ["key"]
            }),
        ),
        spec(
            "computer_use_list_windows",
            "Lista janelas visiveis com PID, titulo e geometria (x,y,width,height). Use para descobrir o PID/titulo antes de screenshot ou click. Nao requer visao.",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),
        spec(
            "computer_use_focus_window",
            "Traz uma janela (pelo titulo parcial, igual computer_use_list_windows mostra) para primeiro plano, restaurando-a se estiver minimizada. Use ISSO antes de click/type/scroll sempre que a aplicacao alvo nao for a que ja esta em primeiro plano - sem focar primeiro, o clique/digitacao vai pra janela errada (a que o usuario estiver usando no momento). Depois de focar, chame computer_use_screenshot pra confirmar o estado antes de interagir. Nao requer visao.",
            json!({
                "type": "object",
                "properties": {
                    "window_title": { "type": "string", "description": "Titulo parcial da janela (case-insensitive), ex: 'Notepad' ou 'Cerne Code'" }
                },
                "required": ["window_title"]
            }),
        ),
        spec(
            "computer_use_scroll",
            "Rola a tela ou janela focada. Use computer_use_click ou computer_use_focus_window ANTES para garantir que a janela certa esta focada. Nao requer visao.",
            json!({
                "type": "object",
                "properties": {
                    "direction": { "type": "string", "enum": ["up", "down", "left", "right"] },
                    "amount": { "type": "integer", "description": "Quantidade de linhas/scrolls (default: 3)" },
                    "window_title": { "type": "string", "description": "Titulo parcial da janela alvo (opcional). Se informado, ela e trazida pro primeiro plano ANTES de rolar." }
                },
                "required": ["direction"]
            }),
        ),
        spec(
            "computer_use_authorize",
            "Autoriza o computer_use a interagir com uma aplicacao (pelo nome do executavel/app, ex: chrome.exe no Windows, chrome no Linux, ou \"Google Chrome\" no macOS). Use ANTES de click/type/key/scroll. Sempre confirme com o usuario via ask antes de autorizar.",
            json!({
                "type": "object",
                "properties": {
                    "exe_name": { "type": "string", "description": "Nome do executavel/app: com .exe no Windows (ex: chrome.exe, code.exe), sem extensao no Linux (ex: chrome, code), nome de exibicao no macOS (ex: \"Google Chrome\") - use o nome exato que apareceu no erro 'APLICACAO NAO AUTORIZADA'" }
                },
                "required": ["exe_name"]
            }),
        ),
        spec(
            "computer_use_browser_execute",
            "Interage com paginas web via CDP (Chrome DevTools Protocol). Funciona com Chrome/Edge/Brave que tenham --remote-debugging-port ativado. Acoes: execute_javascript, click_element (CSS), get_text, query_dom. Nao requer visao nem autorizacao de janela.",
            json!({
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["execute_javascript", "click_element", "get_text", "query_dom"] },
                    "javascript": { "type": "string", "description": "JS a executar (para execute_javascript)" },
                    "css_selector": { "type": "string", "description": "Seletor CSS (para click_element / query_dom)" },
                    "port": { "type": "integer", "description": "Porta CDP (default: 9222)" }
                },
                "required": ["action"]
            }),
        ),
        spec(
            "computer_use_get_window_state",
            "Le a arvore de acessibilidade de uma janela (UI Automation no Windows, AT-SPI2 no Linux - GTK/Qt tem suporte nativo, Electron/Chrome pode ser parcial). Retorna elementos interativos com [element_index N] para usar em computer_use_click_element. Mais confiavel que coordenadas pixel. Sem suporte no macOS ainda.",
            json!({
                "type": "object",
                "properties": {
                    "pid": { "type": "integer", "description": "PID do processo" }
                },
                "required": ["pid"]
            }),
        ),
        spec(
            "computer_use_click_element",
            "Clica em um elemento da arvore de acessibilidade pelo element_index (obtido via computer_use_get_window_state). Mais confiavel que coordenadas pixel. Sem suporte no macOS ainda.",
            json!({
                "type": "object",
                "properties": {
                    "pid": { "type": "integer", "description": "PID do processo" },
                    "element_index": { "type": "integer", "description": "Indice do elemento (de get_window_state)" }
                },
                "required": ["pid", "element_index"]
            }),
        ),
    ]
}

/// Ferramentas que exigem TER VISTO a tela em pixel pra funcionar - screenshot
/// obviamente, e click porque as coordenadas x,y vem de olhar o screenshot.
/// Todo o resto continua disponivel sem visao:
/// list_windows/focus_window/authorize/browser_execute/AX-tree (get_window_state,
/// click_element) sao baseadas em texto/estrutura; type_text/press_key/scroll
/// so agem sobre o elemento que ja estiver focado (via computer_use_click_element,
/// por exemplo) e nao precisam saber onde nada esta na tela - um modelo sem
/// visao consegue automatizar 100% via arvore de acessibilidade: list_windows
/// -> focus_window -> get_window_state -> click_element -> type_text/press_key.
pub const VISION_REQUIRED_TOOLS: &[&str] = &["computer_use_screenshot", "computer_use_click"];

pub fn requires_vision(tool_name: &str) -> bool {
    VISION_REQUIRED_TOOLS.contains(&tool_name)
}

// A implementacao de verdade (xcap/enigo/ashpd/atspi) fica em
// `computer_engine.rs`, atras da feature `computer_use` - permite compilar
// o Cerne Code no Linux SEM essas dependencias (pra distros com PipeWire
// antigo demais pro crate atual, ex. Ubuntu 22.04 - ver
// release-build.yml/release.yml, job "linux (compat)"), mantendo as tools
// anunciadas normalmente pro LLM: quando chamadas nessa build, devolvem uma
// mensagem clara em vez do build simplesmente falhar. Ver README, secao
// "Como instalar" > Linux, pra explicacao pro usuario final.
#[cfg(feature = "computer_use")]
#[path = "computer_engine.rs"]
mod engine;

#[cfg(feature = "computer_use")]
pub use engine::execute;

#[cfg(not(feature = "computer_use"))]
pub async fn execute(name: &str, _args: &Value, _app_data_dir: &Path) -> Result<ComputerOutcome> {
    Err(anyhow!(
        "computer_use ({name}) nao esta disponivel nesta build do Cerne Code - foi compilada \
         sem suporte a captura de tela / automacao de UI (variante 'compat' do Linux, pra rodar \
         em distros com PipeWire mais antigo que o exigido). Use a build 'full' se precisar \
         deste recurso - ver a secao Linux do README pra saber a diferenca entre as duas."
    ))
}
