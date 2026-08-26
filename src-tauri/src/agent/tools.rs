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
            "read_skill_details",
            "Le a descricao COMPLETA (sem corte) de uma skill do catalogo pelo nome exato - use quando a descricao curta listada no catalogo terminar cortada ('...') e nao for suficiente pra decidir se a skill e relevante. Diferente de load_skill: isso NAO carrega as instrucoes da skill pra voce seguir, so mostra a descricao inteira pra ajudar a decidir SE vale chamar load_skill.",
            json!({
                "type": "object",
                "properties": { "name": { "type": "string" } },
                "required": ["name"]
            }),
        ),
        spec(
            "improve_skill",
            "Reescreve o SKILL.md de uma skill existente do catálogo (frontmatter + corpo inteiros - SUBSTITUI tudo, não é um patch). Use quando perceber, USANDO a skill numa tarefa real, que as instruções dela estão desatualizadas, incompletas, ou levaram a um erro que valeria documentar pra da próxima vez sair certo de primeira - não use pra reescrever uma skill que nunca chegou a carregar/seguir nesta conversa. SEMPRE chame load_skill(name) primeiro pra ter o conteúdo completo atual (frontmatter incluso) antes de decidir o que mudar, senão o campo 'description' ou outro metadado pode se perder na reescrita.",
            json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Nome exato da skill no catálogo" },
                    "new_content": { "type": "string", "description": "Conteúdo COMPLETO do novo SKILL.md (frontmatter '---\\nname: ...\\ndescription: ...\\n---' + corpo)" }
                },
                "required": ["name", "new_content"]
            }),
        ),
        spec(
            "remember",
            "Registra um fato DURÁVEL em MEMORY.md, que vai aparecer no system prompt de TODA sessão futura (não só esta) - use pra preferências do usuário ('sempre responde em portugues'), convenções do projeto que vão continuar valendo, ou decisões já tomadas que valem a pena lembrar sem precisar reexplicar depois. NÃO use pra informação temporária/específica desta conversa - só o que genuinamente vale lembrar pra sempre. Só acrescenta (nunca edita nem apaga o que já tinha) - o usuário pode editar o arquivo direto se quiser corrigir algo.",
            json!({
                "type": "object",
                "properties": {
                    "fact": { "type": "string", "description": "O fato em uma frase curta e autocontida" }
                },
                "required": ["fact"]
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
        spec(
            "create_python_tool",
            "Cria uma ferramenta Python reutilizavel que fica disponivel em QUALQUER sessao futura (nao so esta), nao apenas na sessao atual - use quando um pedido precisar de uma capacidade que provavelmente vale reusar depois (ex: 'converta X pra Y', 'gere um grafico assim', 'valide um CPF'), em vez de so rodar codigo Python avulso uma vez via run_command. Apos criada, a ferramenta aparece automaticamente no catalogo de skills (nome 'python-tool-<nome>') com instrucoes prontas de como chamar. O script roda via `uv run`, que instala as dependencias listadas automaticamente num ambiente Python efemero - nao precisa (nem deve) gerenciar venv na mao. Falha se ja existir uma ferramenta com esse nome (use update_python_tool pra editar).",
            json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Nome curto da ferramenta (vira slug, ex: 'validador-de-cpf')" },
                    "description": { "type": "string", "description": "Quando usar essa ferramenta - aparece no catalogo de skills, e o que outras sessoes vao ler pra decidir se ela e relevante" },
                    "script": { "type": "string", "description": "Codigo Python completo (SEM cabecalho de metadata - isso e gerado automaticamente a partir de 'dependencies'). Deve ler argumentos de sys.argv e imprimir o resultado em stdout, pra ser chamado via run_command." },
                    "dependencies": { "type": "array", "items": { "type": "string" }, "description": "Pacotes pip necessarios, ex: ['requests', 'pillow'] (opcional, vazio = so biblioteca padrao do Python)" }
                },
                "required": ["name", "description", "script"]
            }),
        ),
        spec(
            "update_python_tool",
            "Atualiza uma ferramenta Python ja criada (por create_python_tool, nesta sessao ou em outra) - use pra corrigir um bug ou mudar o comportamento de uma ferramenta que ja existe, em vez de criar uma nova do zero com outro nome. Mesmos parametros de create_python_tool. Falha se a ferramenta nao existir.",
            json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Nome exato (slug) da ferramenta ja existente" },
                    "description": { "type": "string" },
                    "script": { "type": "string" },
                    "dependencies": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["name", "description", "script"]
            }),
        ),
    ]
}

/// Only available when the session has a project folder attached.
pub fn project_tool_specs() -> Vec<ToolSpec> {
    vec![
        spec(
            "read_file",
            // Exemplo neutro de SO (Tarefa 4.2 do port): mostra os dois
            // formatos em vez de ensinar so o Windows.
            "Le o conteudo de um arquivo. Caminho relativo e resolvido dentro do projeto; caminho absoluto funciona para QUALQUER pasta do sistema (ex: /pasta/outro-repo/src/main.rs no Unix, C:\\pasta\\outro-repo\\src\\main.rs no Windows) — use para consultar codigo de outros repositorios ou documentacao externa. Use offset+limit pra ler so um trecho de arquivos grandes (economiza tokens e memoria) — o retorno inclui o total de linhas pra voce saber se precisa continuar lendo.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Caminho ABSOLUTO (preferido) de qualquer pasta do sistema; caminho relativo resolve na raiz do projeto" },
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
                "properties": { "path": { "type": "string", "description": "Caminho ABSOLUTO (preferido) de qualquer pasta; vazio = raiz do projeto" } },
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
                    "path": { "type": "string", "description": "Caminho ABSOLUTO (preferido) ou subpasta relativa ao projeto (opcional)" }
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
            "check_dependencies_osv",
            "Confere as dependencias DIRETAS declaradas no manifesto do projeto (package.json, Cargo.toml e/ou requirements.txt, na raiz) contra a base publica de vulnerabilidades conhecidas do OSV.dev (osv.dev) - sem precisar de conta ou chave. Devolve um resumo do que foi encontrado, com id e descricao curta de cada vulnerabilidade. Nao resolve dependencias transitivas (so o que esta escrito no manifesto) nem le lockfile - use quando o usuario pedir pra checar seguranca/vulnerabilidade das dependencias do projeto.",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),
        spec(
            "write_file",
            "Cria ou sobrescreve um arquivo. Conforme o modo de execucao da sessao, aplica direto no arquivo real ou fica pendente numa sandbox esperando o usuario aceitar na interface - o texto devolvido por CADA chamada informa qual dos dois aconteceu.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Caminho ABSOLUTO (preferido) do arquivo; caminho relativo resolve na raiz do projeto" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"]
            }),
        ),
        spec(
            "edit_file",
            "Edita um arquivo existente substituindo uma ocorrencia exata de old_str por new_str. old_str deve aparecer exatamente uma vez no arquivo. Conforme o modo de execucao da sessao, aplica direto no arquivo real ou fica pendente numa sandbox esperando o usuario aceitar na interface - o texto devolvido por CADA chamada informa qual dos dois aconteceu.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Caminho ABSOLUTO (preferido) do arquivo; caminho relativo resolve na raiz do projeto" },
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
            "Reescrita ESTRUTURAL de um arquivo: toda ocorrencia do padrao (mesma sintaxe do ast_grep, $VAR/$$$ARGS) e trocada pelo template de reescrita, que pode reusar os mesmos nomes de variavel capturados. Mais seguro que edit_file pra refactor (rename de chamada, mudar import, etc.) porque opera na estrutura, nao em texto exato. Conforme o modo de execucao da sessao, aplica direto no arquivo real ou fica pendente numa sandbox esperando o usuario aceitar na interface - o texto devolvido por CADA chamada informa qual dos dois aconteceu.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Caminho ABSOLUTO (preferido) do arquivo; caminho relativo resolve na raiz do projeto" },
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
            "run_pipeline",
            "Dispara um pipeline DETERMINISTICO Dev -> QA -> Analista pra uma tarefa complexa que merece o rigor completo de implementacao + validacao tecnica + validacao de requisito, em vez de voce mesmo implementar e verificar. Diferente de task/verify_completion (que voce chama quando decide), aqui a SEQUENCIA e fixa: um Dev implementa, um QA confirma que funciona tecnicamente (roda teste/build de verdade), um Analista confirma que atende o pedido original (nao so que funciona) - se QA ou Analista refutar, volta pro Dev automaticamente com as pendencias, ate max_rounds. Voce so decide QUANDO usar isso (pedido grande, varios arquivos, criar algo do zero) - o que acontece depois de comecar nao e escolha sua. Use pra pedidos que voce mesmo trataria com task + verify_completion em sequencia de qualquer forma, mas quer o ciclo completo automatico incluindo validacao de requisito. NAO use pra pedido simples que uma unica chamada de ferramenta ja resolve.",
            json!({
                "type": "object",
                "properties": {
                    "requirement": { "type": "string", "description": "O requisito completo e autocontido - o Dev/QA/Analista nao veem o historico desta conversa, so o que for escrito aqui" },
                    "max_rounds": { "type": "integer", "description": "Quantas rodadas dev->qa->analista tentar antes de desistir e devolver o que ficou pendente (default 3)" }
                },
                "required": ["requirement"]
            }),
        ),
        spec(
            "create_excel",
            "Cria um arquivo Excel (.xlsx) com uma ou mais abas, headers formatados, dados, largura de colunas e auto-filtro. Escreve direto no disco (nao usa sandbox). Use quando o usuario pedir para criar planilhas.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Caminho ABSOLUTO (preferido) do arquivo .xlsx a criar" },
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
                    "path": { "type": "string", "description": "Caminho ABSOLUTO (preferido) do arquivo .docx a criar" },
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
                    "path": { "type": "string", "description": "Caminho ABSOLUTO (preferido) do arquivo .pdf a criar" },
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
        spec(
            "create_pptx",
            "Cria uma apresentacao PowerPoint (.pptx) com um slide por item da lista. Cada slide tem titulo opcional e uma lista de elementos empilhados verticalmente: paragrafo (com negrito/italico/tamanho opcionais), lista de topicos (bullets), tabela, ou IMAGEM (le um arquivo png/jpg/jpeg/gif/bmp do disco e embute no slide, mantendo a proporcao original se largura/altura nao forem informadas). Escreve direto no disco (nao usa sandbox). Use quando o usuario pedir para criar apresentacoes/slides.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Caminho ABSOLUTO (preferido) do arquivo .pptx a criar" },
                    "slides": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "title": { "type": "string", "description": "Titulo do slide (opcional)" },
                                "elements": {
                                    "type": "array",
                                    "description": "Conteudo do slide, empilhado de cima pra baixo na ordem da lista",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "type": { "type": "string", "enum": ["paragraph", "bullets", "table", "image"], "description": "Tipo do elemento" },
                                            "text": { "type": "string", "description": "Texto (so pra paragraph)" },
                                            "bold": { "type": "boolean", "description": "Negrito (so pra paragraph)" },
                                            "italic": { "type": "boolean", "description": "Italico (so pra paragraph)" },
                                            "size": { "type": "integer", "description": "Tamanho da fonte em pontos (paragraph/bullets, default 18)" },
                                            "items": { "type": "array", "items": { "type": "string" }, "description": "Topicos (so pra bullets)" },
                                            "headers": { "type": "array", "items": { "type": "string" }, "description": "Headers da tabela (so pra table)" },
                                            "rows": { "type": "array", "items": { "type": "array", "items": { "type": "string" } }, "description": "Linhas da tabela (so pra table)" },
                                            "path": { "type": "string", "description": "Caminho (absoluto ou relativo ao projeto) da imagem png/jpg/jpeg/gif/bmp no disco (so pra image)" },
                                            "width_in": { "type": "number", "description": "Largura da imagem em polegadas (opcional — sem isso usa a altura informada + proporcao original, ou um tamanho default)" },
                                            "height_in": { "type": "number", "description": "Altura da imagem em polegadas (opcional, mesma logica de width_in)" }
                                        },
                                        "required": ["type"]
                                    }
                                }
                            }
                        }
                    }
                },
                "required": ["path", "slides"]
            }),
        ),
    ]
}

/// Fase G do roteiro: sessões paralelas orquestradas — diferente de `task`
/// (sub-agente efêmero, sempre síncrono do ponto de vista do turno que
/// chamou), aqui o LLM principal cria uma `Session` de verdade que roda
/// DESACOPLADA do turno atual (mesmo padrão de `send_message`, sem esperar)
/// e confere o progresso sob demanda depois. Fora de `always_tool_specs()`/
/// `project_tool_specs()` de propósito — só entra no toolset de sessões que
/// NÃO são elas mesmas orquestradas (`session.parent_session_id.is_none()`
/// em `agent/mod.rs`), guarda de profundidade de nível único, mesmo
/// espírito do guard que `task` já tem pra não recursar.
pub fn orchestration_tool_specs() -> Vec<ToolSpec> {
    vec![
        spec(
            "start_agent_session",
            "Cria uma SESSAO completa e independente que roda em paralelo, sem bloquear seu turno atual - diferente de task (que espera terminar), esta dispara e volta na hora com o id da sessao; voce confere o progresso depois via check_agent_session quando quiser. Use pra trabalho grande e genuinamente paralelo que nao precisa do resultado imediatamente (ex: 'monte o frontend Angular' enquanto voce continua com outra coisa, ou dispara varias de uma vez: uma pro frontend, outra pro backend, uma terceira pra conferir integracao). A sessao criada usa o mesmo provider/modelo/modo de execucao desta sessao. IMPORTANTE: se project_root ficar vazio (nem passado aqui, nem existente nesta sessao), a sessao criada NAO tera nenhuma ferramenta de arquivo/comando (read_file, write_file, run_command etc) - so use sem project_root pra tarefas que so precisam de busca na web ou raciocinio, nunca pra algo que crie/edite arquivos ou rode comandos. NAO use pra algo que precisa do resultado antes de continuar (isso e task ou fazer voce mesmo).",
            json!({
                "type": "object",
                "properties": {
                    "description": { "type": "string", "description": "Descricao curta (vira o titulo da sessao na lista)" },
                    "prompt": { "type": "string", "description": "A tarefa completa e autocontida - a sessao criada nao ve o historico desta conversa, so o que for escrito aqui" },
                    "project_root": { "type": "string", "description": "Pasta de projeto pra essa sessao - default herda a pasta desta sessao, se houver. Passe explicitamente sempre que a tarefa envolver arquivos ou comandos, senao a sessao criada fica sem essas ferramentas." }
                },
                "required": ["description", "prompt"]
            }),
        ),
        spec(
            "check_agent_session",
            "Confere o status de uma sessao orquestrada criada com start_agent_session. Se ainda estiver rodando, devolve isso mais uma sugestao de quanto esperar antes de checar de novo (baseado em quanto a primeira resposta dela levou) - nao fique chamando isso em loop apertado, va fazendo outra coisa entre uma checagem e outra. Se ja tiver terminado, devolve a ultima resposta do assistente daquela sessao.",
            json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" }
                },
                "required": ["session_id"]
            }),
        ),
        spec(
            "list_agent_sessions",
            "Lista as sessoes orquestradas que voce criou nesta conversa (via start_agent_session), com status de cada uma - use pra nao perder o fio de quais ja disparou.",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),
        spec(
            "stop_agent_session",
            "Aborta uma sessao orquestrada em andamento (criada com start_agent_session) - encerra o turno dela na hora, sem esperar nenhum checkpoint.",
            json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" }
                },
                "required": ["session_id"]
            }),
        ),
    ]
}

/// Aviso anexado ao resultado de `create_python_tool`/`update_python_tool`
/// quando `uv` não está no PATH desta máquina — sem isso, um usuário só com
/// o Cerne Code instalado (sem `uv`) só ia descobrir o problema quando a
/// ferramenta falhasse com "comando não encontrado", sem entender o motivo
/// (pedido do usuário testando ao vivo, 2026-08-17). String vazia quando
/// `uv` existe, pra não poluir a resposta no caso comum.
fn uv_missing_warning() -> String {
    if super::shell::command_exists("uv") {
        String::new()
    } else {
        " AVISO: o comando 'uv' nao foi encontrado no PATH desta maquina - a ferramenta foi \
         criada normalmente, mas nao vai funcionar ate o usuario instalar o uv \
         (https://docs.astral.sh/uv/getting-started/installation/). Avise o usuario disso na \
         sua resposta."
            .to_string()
    }
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

/// Escapa texto pra entrar cru dentro de um elemento/atributo XML — usado
/// por `create_pptx`, que monta o OOXML na mao (sem lib de apresentacao,
/// diferente de `create_excel`/`create_word`/`create_pdf` que tem crate
/// dedicada). Ordem importa: `&` primeiro, senao escaparia os `&` que os
/// outros replace's acabaram de inserir.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Resolve um caminho (relativo ou absoluto) pras ferramentas de ESCRITA
/// (`write_file`/`edit_file`/`ast_edit`/`create_excel`/`create_word`/
/// `create_pdf`). Caminho relativo sempre resolve dentro do `project_root`.
/// Caminho absoluto fora do projeto so e aceito em modo Auto ou YOLO — em
/// Manual, cada tool call ja pausa pedindo aprovacao explicita, mas ainda
/// assim mantemos a mesma restricao de sempre por consistencia, ja que
/// ninguem pediu pra mudar esse modo. `to_sandbox_path` (sandbox.rs) sabe
/// espelhar caminhos externos num subdiretorio `_external` da sandbox.
fn resolve_path(
    project_root: &Path,
    rel: &str,
    execution_mode: &crate::models::ExecutionMode,
    writable_extra_roots: &[String],
) -> Result<PathBuf> {
    let allow_external = *execution_mode != crate::models::ExecutionMode::Manual;
    // Pastas extras com modo ReadWrite também são permitidas para escrita,
    // mesmo em modo Manual (o usuário já autorizou explicitamente ao adicionar).
    if !writable_extra_roots.is_empty() {
        if let Ok(result) = resolve_within(project_root, writable_extra_roots, rel, true) {
            return Ok(result);
        }
    }
    resolve_within(project_root, &[], rel, allow_external)
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

fn resolve_within(
    project_root: &Path,
    extra_roots: &[String],
    rel: &str,
    allow_external: bool,
) -> Result<PathBuf> {
    let trimmed = rel.trim();
    // Caminho absoluto deve ser preservado COMO ESTA. Nao remover os
    // separadores iniciais: em POSIX, tirar a `/` inicial transforma um
    // caminho absoluto em RELATIVO e o `project_root.join()` o colocaria
    // DENTRO do projeto — o write sairia dentro da raiz quando deveria ir
    // pra pasta externa (bug que so aparecia fora do Windows, onde absolutos
    // tem drive e o join() os substitui de qualquer forma).
    let candidate = if Path::new(trimmed).is_absolute() {
        PathBuf::from(trimmed)
    } else {
        project_root.join(trimmed)
    };
    if allow_external && Path::new(trimmed).is_absolute() {
        return Ok(candidate);
    }
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
    extra_folders: &[crate::models::FolderEntry],
    background_jobs: &super::background::BackgroundJobs,
    mcp_clients: &crate::mcp::McpClients,
    app_data_dir: &Path,
    execution_mode: &crate::models::ExecutionMode,
    session_id: &str,
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
        // Ferramentas Python (T17) sao GLOBAIS (nao presas a um projeto,
        // diferente de write_file/edit_file) - ficam disponiveis pra
        // qualquer sessao futura via a skill companheira que
        // create_python_tool/update_python_tool geram, entao nao precisam
        // de project_root pra rodar.
        "create_python_tool" => {
            let py_name = args["name"].as_str().ok_or_else(|| anyhow!("name obrigatorio"))?;
            let description = args["description"]
                .as_str()
                .ok_or_else(|| anyhow!("description obrigatorio"))?;
            let script = args["script"]
                .as_str()
                .ok_or_else(|| anyhow!("script obrigatorio"))?;
            let dependencies: Vec<String> = args["dependencies"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            let tool = crate::python_tools::create_python_tool(
                app_data_dir,
                py_name,
                description,
                script,
                dependencies,
            )?;
            Ok(ok(format!(
                "Ferramenta Python '{}' criada e disponivel como skill 'python-tool-{}' em qualquer sessao futura. Pra chamar: uv run \"{}\" [args].{}",
                tool.name, tool.name, tool.tool_path, uv_missing_warning()
            )))
        }
        "update_python_tool" => {
            let py_name = args["name"].as_str().ok_or_else(|| anyhow!("name obrigatorio"))?;
            let description = args["description"]
                .as_str()
                .ok_or_else(|| anyhow!("description obrigatorio"))?;
            let script = args["script"]
                .as_str()
                .ok_or_else(|| anyhow!("script obrigatorio"))?;
            let dependencies: Vec<String> = args["dependencies"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            let tool = crate::python_tools::update_python_tool(
                app_data_dir,
                py_name,
                description,
                script,
                dependencies,
            )?;
            Ok(ok(format!(
                "Ferramenta Python '{}' atualizada. Pra chamar: uv run \"{}\" [args].{}",
                tool.name, tool.tool_path, uv_missing_warning()
            )))
        }
        _ if name.starts_with("mcp__") => Ok(ok(mcp_clients.call(name, args.clone()).await?)),
        _ => {
            let project_root = project_root.ok_or_else(|| {
                anyhow!("esta ferramenta precisa de uma pasta de projeto associada a sessao")
            })?;
            let read_paths = crate::models::FolderEntry::paths(extra_folders);
            let writable_paths = crate::models::FolderEntry::writable_paths(extra_folders);
            let mut extended_read = read_paths;
            extended_read.push(app_data_dir.to_string_lossy().to_string());
            execute_project_tool(
                name,
                args,
                project_root,
                &extended_read,
                &writable_paths,
                background_jobs,
                execution_mode,
                app_data_dir,
                session_id,
            )
            .await
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn execute_project_tool(
    name: &str,
    args: &Value,
    project_root: &Path,
    extra_read_paths: &[String],
    writable_extra_roots: &[String],
    background_jobs: &super::background::BackgroundJobs,
    execution_mode: &crate::models::ExecutionMode,
    app_data_dir: &Path,
    session_id: &str,
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
                let id = background_jobs.start(project_root, command, app_data_dir, session_id)?;
                return Ok(ok(format!(
                    "Comando iniciado em segundo plano com id {id} (nao esperou terminar). Use \
                     check_background_output({{\"id\": \"{id}\"}}) pra ver o progresso, e \
                     stop_background({{\"id\": \"{id}\"}}) quando nao precisar mais dele."
                )));
            }
            const RUN_COMMAND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);
            let mut cmd = super::shell::build_shell_command(command);
            // Unix: grupo de processo proprio — no timeout, `kill_on_drop`
            // so mata o shell; sem o grupo, os filhos dele ficariam orfaos
            // (mesma classe do bug do background.rs::stop). Com o grupo,
            // kill_pid_tree_blocking alcanca todos.
            super::shell::apply_process_group(&mut cmd);
            cmd.current_dir(project_root)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
            let child = cmd.spawn()?;
            // Captura o PID ANTES: `wait_with_output()` consome o Child, e se
            // o timeout vencer, o future e dropado junto com ele (kill_on_drop
            // mata so o shell). O PID capturado permite matar a ARVORE depois.
            let child_pid = child.id();
            let result =
                match tokio::time::timeout(RUN_COMMAND_TIMEOUT, child.wait_with_output()).await {
                    Ok(output) => {
                        let output = output?;
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        format!(
                            "exit_code: {}\nstdout:\n{}\nstderr:\n{}",
                            output.status.code().unwrap_or(-1),
                            truncate(&stdout, 8000),
                            truncate(&stderr, 4000)
                        )
                    }
                    Err(_) => {
                        // Timeout: mata a ARVORE explicitamente. No Unix,
                        // kill_on_drop so mataria o shell (/bin/sh -c) e
                        // deixaria o comando real orfao rodando; no Windows,
                        // taskkill /T /F garante o mesmo. A mensagem
                        // consumida pelo LLM nao muda.
                        #[cfg(windows)]
                        if let Some(pid) = child_pid {
                            let mut taskkill = tokio::process::Command::new("taskkill");
                            taskkill.args(["/PID", &pid.to_string(), "/T", "/F"]);
                            super::shell::apply_creation_flags(&mut taskkill);
                            let _ = taskkill.output().await;
                        }
                        #[cfg(not(windows))]
                        if let Some(pid) = child_pid {
                            let _ = tokio::task::spawn_blocking(move || {
                                super::shell::kill_pid_tree_blocking(pid);
                            })
                            .await;
                        }
                        format!(
                            "comando expirou apos {}s e foi encerrado (nao terminou sozinho). Se for um \
                             servidor ou processo de longa duracao que deveria continuar rodando, use \
                             {{\"background\": true}} em vez de esperar ele terminar.",
                            RUN_COMMAND_TIMEOUT.as_secs()
                        )
                    }
                };
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
        "check_dependencies_osv" => {
            let result = crate::osv::check_project(project_root).await?;
            Ok(ok(result))
        }
        "create_excel" => {
            let rel = args["path"]
                .as_str()
                .ok_or_else(|| anyhow!("path obrigatorio"))?;
            let target = resolve_path(project_root, rel, execution_mode, writable_extra_roots)?;
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
            let target = resolve_path(project_root, rel, execution_mode, writable_extra_roots)?;
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
            let target = resolve_path(project_root, rel, execution_mode, writable_extra_roots)?;
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
        "create_pptx" => {
            let rel = args["path"]
                .as_str()
                .ok_or_else(|| anyhow!("path obrigatorio"))?;
            let target = resolve_path(project_root, rel, execution_mode, writable_extra_roots)?;
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let slides = args["slides"]
                .as_array()
                .ok_or_else(|| anyhow!("slides obrigatorio (array)"))?;
            if slides.is_empty() {
                return Err(anyhow!("slides precisa ter pelo menos 1 item"));
            }
            write_pptx(&target, slides, project_root)?;
            Ok(ok(format!(
                "Apresentacao PowerPoint criada com {} slide(s): {}",
                slides.len(),
                target.display()
            )))
        }
        "write_file" => {
            let rel = args["path"]
                .as_str()
                .ok_or_else(|| anyhow!("path obrigatorio"))?;
            let content = args["content"]
                .as_str()
                .ok_or_else(|| anyhow!("content obrigatorio"))?;
            let target = resolve_path(project_root, rel, execution_mode, writable_extra_roots)?;
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
            let target = resolve_path(project_root, rel, execution_mode, writable_extra_roots)?;
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
            let target = resolve_path(project_root, rel, execution_mode, writable_extra_roots)?;
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
        // "load_skill"/"read_skill_details" - precisa de app/estado/provider
        // que essa funcao nao tem, entao nunca chega aqui de verdade.
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

// ---------------------------------------------------------------------
// create_pptx — gera um .pptx (Office Open XML) na mao, sem lib de
// apresentacao dedicada (nao existe uma crate PPTX madura pro ecossistema
// Rust, diferente de xlsx/docx/pdf que ja tem `rust_xlsxwriter`/`docx-rust`/
// `printpdf`). Um .pptx e so um ZIP com um conjunto de partes XML descritas
// pelo schema OOXML PresentationML — a crate `zip` (ja usada em
// `attachments.rs` pra ler xlsx de anexo) e suficiente pra escrever.
//
// Estrutura minima gerada: 1 slideMaster + 1 slideLayout "blank" (boilerplate
// fixo), 1 theme (cores/fontes do Cerne), N slides (1 por item da lista),
// cada um com suas proprias shapes posicionadas explicitamente via `a:xfrm`
// (nao depende de placeholder herdado do layout, que exigiria um layout mais
// elaborado por tipo de slide). Imagens viram partes `ppt/media/imageN.*` +
// relationship no slide + elemento `<p:pic>`.
// ---------------------------------------------------------------------

const PPTX_SLIDE_W_EMU: i64 = 12_192_000; // 13.333in — widescreen 16:9
const PPTX_SLIDE_H_EMU: i64 = 6_858_000; // 7.5in
const PPTX_MARGIN_EMU: i64 = 685_800; // 0.75in
const PPTX_CONTENT_W_EMU: i64 = PPTX_SLIDE_W_EMU - 2 * PPTX_MARGIN_EMU;
const PPTX_TITLE_Y_EMU: i64 = 274_638;
const PPTX_TITLE_H_EMU: i64 = 1_143_000;
const PPTX_EMU_PER_INCH: f64 = 914_400.0;
const PPTX_EMU_PER_PX_96DPI: f64 = 9_525.0; // 914400 / 96, conversao padrao usada por python-pptx e afins

struct PptxImagePart {
    /// Nome do arquivo dentro de `ppt/media/`, ex. "image3.png".
    file_name: String,
    content_type: String,
    bytes: Vec<u8>,
}

fn pptx_ext_content_type(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        _ => "image/png",
    }
}

/// Le a imagem do disco (mesma resolucao de caminho de leitura usada por
/// `read_file` — aceita absoluto de qualquer lugar, relativo resolve dentro
/// do projeto) e devolve os bytes + dimensoes em pixels (via crate `image`,
/// ja dependencia do projeto pra screenshots do `computer_use`). Se o
/// formato nao for decodificavel (arquivo corrompido, formato exotico),
/// cai num fallback 800x600 em vez de falhar a apresentacao inteira por
/// causa de UMA imagem ruim — o usuario ainda ve o slide, só com a imagem
/// num tamanho generico.
fn pptx_read_image(project_root: &Path, path_str: &str) -> Result<(Vec<u8>, String, u32, u32)> {
    let resolved = resolve_read_path(project_root, &[], path_str)?;
    let bytes = std::fs::read(&resolved)
        .map_err(|e| anyhow!("nao foi possivel ler imagem '{path_str}': {e}"))?;
    let ext = resolved
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();
    let (width_px, height_px) = image::load_from_memory(&bytes)
        .map(|img| (img.width(), img.height()))
        .unwrap_or((800, 600));
    Ok((bytes, ext, width_px, height_px))
}

/// Monta o XML de UM slide + a lista de imagens que ele referencia (media
/// parts a incluir no zip + relationships a declarar no `.rels` do slide).
/// `media_counter` e compartilhado entre slides pra numerar `imageN.ext`
/// sem colisao na apresentacao inteira.
fn build_pptx_slide(
    slide: &Value,
    project_root: &Path,
    media_counter: &mut u32,
) -> Result<(String, String, Vec<PptxImagePart>)> {
    let mut shapes = String::new();
    let mut media = Vec::new();
    // rId1 do slide sempre aponta pro slideLayout (ver build_pptx_slide_rels) —
    // relationships de imagem comecam em rId2.
    let mut next_rel_id: u32 = 2;
    let mut shape_id: u32 = 2; // id 1 e do grupo raiz (nvGrpSpPr), shapes comecam em 2

    let mut cur_y = PPTX_MARGIN_EMU;

    if let Some(title) = slide["title"].as_str().filter(|t| !t.trim().is_empty()) {
        shapes.push_str(&format!(
            r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="Title"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="pt-BR" sz="3200" b="1"/><a:t>{text}</a:t></a:r></a:p></p:txBody></p:sp>"#,
            id = shape_id,
            x = PPTX_MARGIN_EMU,
            y = PPTX_TITLE_Y_EMU,
            cx = PPTX_CONTENT_W_EMU,
            cy = PPTX_TITLE_H_EMU,
            text = xml_escape(title),
        ));
        shape_id += 1;
        cur_y = PPTX_TITLE_Y_EMU + PPTX_TITLE_H_EMU + 100_000;
    }

    let elements = slide["elements"].as_array().cloned().unwrap_or_default();
    for el in &elements {
        let remaining_h = (PPTX_SLIDE_H_EMU - PPTX_MARGIN_EMU - cur_y).max(0);
        if remaining_h <= 0 {
            break; // slide cheio — resto do conteudo nao cabe, evita sobrepor
        }
        match el["type"].as_str().unwrap_or("") {
            "paragraph" => {
                let text = el["text"].as_str().unwrap_or("");
                let size_pt = el["size"].as_i64().unwrap_or(18).clamp(6, 96);
                let bold = if el["bold"].as_bool().unwrap_or(false) { " b=\"1\"" } else { "" };
                let italic = if el["italic"].as_bool().unwrap_or(false) { " i=\"1\"" } else { "" };
                let lines = text.lines().count().max(1) as i64;
                let h = (size_pt * 12_700 * 16 / 10) * lines; // ~1.6x line-height, sz esta em pontos*100
                shapes.push_str(&format!(
                    r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="TextBox"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr wrap="square"><a:normAutofit/></a:bodyPr><a:lstStyle/><a:p><a:r><a:rPr lang="pt-BR" sz="{sz}"{bold}{italic}/><a:t>{text}</a:t></a:r></a:p></p:txBody></p:sp>"#,
                    id = shape_id,
                    x = PPTX_MARGIN_EMU,
                    y = cur_y,
                    cx = PPTX_CONTENT_W_EMU,
                    cy = h.min(remaining_h),
                    sz = size_pt * 100,
                    bold = bold,
                    italic = italic,
                    text = xml_escape(text),
                ));
                shape_id += 1;
                cur_y += h.min(remaining_h) + 50_000;
            }
            "bullets" => {
                let items: Vec<&str> = el["items"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
                    .unwrap_or_default();
                if items.is_empty() {
                    continue;
                }
                let size_pt = el["size"].as_i64().unwrap_or(18).clamp(6, 96);
                let line_h = size_pt * 12_700 * 16 / 10;
                let h = (line_h * items.len() as i64).min(remaining_h);
                let mut paras = String::new();
                for item in &items {
                    paras.push_str(&format!(
                        r#"<a:p><a:pPr marL="285750" indent="-285750"><a:buFont typeface="Arial"/><a:buChar char="&#8226;"/></a:pPr><a:r><a:rPr lang="pt-BR" sz="{sz}"/><a:t>{text}</a:t></a:r></a:p>"#,
                        sz = size_pt * 100,
                        text = xml_escape(item),
                    ));
                }
                shapes.push_str(&format!(
                    r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="Bullets"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr wrap="square"><a:normAutofit/></a:bodyPr><a:lstStyle/>{paras}</p:txBody></p:sp>"#,
                    id = shape_id,
                    x = PPTX_MARGIN_EMU,
                    y = cur_y,
                    cx = PPTX_CONTENT_W_EMU,
                    cy = h,
                    paras = paras,
                ));
                shape_id += 1;
                cur_y += h + 50_000;
            }
            "table" => {
                let headers: Vec<&str> = el["headers"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
                    .unwrap_or_default();
                let rows: Vec<Vec<&str>> = el["rows"]
                    .as_array()
                    .map(|rs| {
                        rs.iter()
                            .map(|r| {
                                r.as_array()
                                    .map(|cs| cs.iter().filter_map(|c| c.as_str()).collect())
                                    .unwrap_or_default()
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let col_count = headers.len().max(rows.first().map(|r| r.len()).unwrap_or(0));
                if col_count == 0 {
                    continue;
                }
                let row_h: i64 = 370_840;
                let total_rows = 1 + rows.len() as i64; // header + dados
                let h = (row_h * total_rows).min(remaining_h);
                let col_w = PPTX_CONTENT_W_EMU / col_count as i64;
                let mut grid = String::new();
                for _ in 0..col_count {
                    grid.push_str(&format!(r#"<a:gridCol w="{col_w}"/>"#));
                }
                let mut tr_xml = String::new();
                let cell = |text: &str, bold: bool| -> String {
                    format!(
                        r#"<a:tc><a:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="pt-BR" sz="1400"{b}/><a:t>{t}</a:t></a:r></a:p></a:txBody><a:tcPr/></a:tc>"#,
                        b = if bold { " b=\"1\"" } else { "" },
                        t = xml_escape(text),
                    )
                };
                if !headers.is_empty() {
                    let cells: String = headers.iter().map(|h| cell(h, true)).collect();
                    tr_xml.push_str(&format!(r#"<a:tr h="{row_h}">{cells}</a:tr>"#));
                }
                for row in &rows {
                    let cells: String = row.iter().map(|c| cell(c, false)).collect();
                    tr_xml.push_str(&format!(r#"<a:tr h="{row_h}">{cells}</a:tr>"#));
                }
                shapes.push_str(&format!(
                    r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="{id}" name="Table"/><p:cNvGraphicFramePr><a:graphicFrameLocks noGrp="1"/></p:cNvGraphicFramePr><p:nvPr/></p:nvGraphicFramePr><p:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></p:xfrm><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table"><a:tbl><a:tblPr firstRow="1" bandRow="1"/><a:tblGrid>{grid}</a:tblGrid>{rows_xml}</a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#,
                    id = shape_id,
                    x = PPTX_MARGIN_EMU,
                    y = cur_y,
                    cx = PPTX_CONTENT_W_EMU,
                    cy = h,
                    grid = grid,
                    rows_xml = tr_xml,
                ));
                shape_id += 1;
                cur_y += h + 50_000;
            }
            "image" => {
                let Some(path_str) = el["path"].as_str() else { continue };
                let (bytes, ext, width_px, height_px) = pptx_read_image(project_root, path_str)?;
                *media_counter += 1;
                let file_name = format!("image{}.{}", media_counter, ext);
                let content_type = pptx_ext_content_type(&ext).to_string();

                // Tamanho: usa width_in/height_in se informados; senao deriva
                // da proporcao real da imagem (px -> EMU a 96 DPI), limitado
                // pra caber na largura de conteudo e no espaco vertical
                // restante do slide — nunca estoura o slide.
                let natural_cx = (width_px as f64 * PPTX_EMU_PER_PX_96DPI) as i64;
                let natural_cy = (height_px as f64 * PPTX_EMU_PER_PX_96DPI) as i64;
                let (mut cx, mut cy) = match (el["width_in"].as_f64(), el["height_in"].as_f64()) {
                    (Some(w), Some(h)) => (
                        (w * PPTX_EMU_PER_INCH) as i64,
                        (h * PPTX_EMU_PER_INCH) as i64,
                    ),
                    (Some(w), None) => {
                        let cx = (w * PPTX_EMU_PER_INCH) as i64;
                        let cy = if natural_cx > 0 { cx * natural_cy / natural_cx } else { cx };
                        (cx, cy)
                    }
                    (None, Some(h)) => {
                        let cy = (h * PPTX_EMU_PER_INCH) as i64;
                        let cx = if natural_cy > 0 { cy * natural_cx / natural_cy } else { cy };
                        (cx, cy)
                    }
                    (None, None) => (natural_cx.max(1), natural_cy.max(1)),
                };
                // Encolhe mantendo proporcao se nao couber na largura de
                // conteudo ou no espaco vertical restante do slide.
                if cx > PPTX_CONTENT_W_EMU {
                    let scale = PPTX_CONTENT_W_EMU as f64 / cx as f64;
                    cx = PPTX_CONTENT_W_EMU;
                    cy = (cy as f64 * scale) as i64;
                }
                if cy > remaining_h {
                    let scale = remaining_h as f64 / cy.max(1) as f64;
                    cy = remaining_h;
                    cx = (cx as f64 * scale) as i64;
                }

                let rel_id = next_rel_id;
                next_rel_id += 1;
                shapes.push_str(&format!(
                    r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="Picture"/><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="rId{rel_id}"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#,
                    id = shape_id,
                    rel_id = rel_id,
                    x = PPTX_MARGIN_EMU,
                    y = cur_y,
                    cx = cx,
                    cy = cy,
                ));
                shape_id += 1;
                cur_y += cy + 50_000;
                media.push((rel_id, PptxImagePart { file_name, content_type, bytes }));
            }
            _ => {}
        }
    }

    // media colhida acima carrega o rel_id junto pra montar o .rels do slide
    // (rId1 = layout, rId de imagem = o que foi atribuido na hora de montar
    // a shape) — separa em duas listas paralelas antes de devolver.
    let mut rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>"#
    );
    let mut media_parts = Vec::new();
    for (rel_id, part) in media {
        rels.push_str(&format!(
            r#"<Relationship Id="rId{rel_id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/{file}"/>"#,
            rel_id = rel_id,
            file = part.file_name,
        ));
        media_parts.push(part);
    }
    rels.push_str("</Relationships>");

    let slide_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>{shapes}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#
    );

    Ok((slide_xml, rels, media_parts))
}

fn write_pptx(target: &Path, slides: &[Value], project_root: &Path) -> Result<()> {
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    let mut content_types = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>"#,
    );
    let mut media_extensions_seen: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    content_types.push_str(
        r#"<Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/><Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/><Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/><Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/><Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>"#,
    );

    let mut media_counter: u32 = 0;
    let mut slide_entries: Vec<(String, String, Vec<PptxImagePart>)> = Vec::new(); // (slide_xml, rels_xml, media)
    for slide in slides {
        let (slide_xml, rels_xml, media) = build_pptx_slide(slide, project_root, &mut media_counter)?;
        for part in &media {
            let ext = part
                .file_name
                .rsplit('.')
                .next()
                .unwrap_or("png")
                .to_lowercase();
            media_extensions_seen.insert(ext, part.content_type.clone());
        }
        slide_entries.push((slide_xml, rels_xml, media));
    }
    for (ext, content_type) in &media_extensions_seen {
        content_types.push_str(&format!(
            r#"<Default Extension="{ext}" ContentType="{content_type}"/>"#,
        ));
    }
    for i in 1..=slide_entries.len() {
        content_types.push_str(&format!(
            r#"<Override PartName="/ppt/slides/slide{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#
        ));
    }
    content_types.push_str("</Types>");

    let package_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/></Relationships>"#;

    let mut sld_id_lst = String::new();
    for i in 0..slide_entries.len() {
        sld_id_lst.push_str(&format!(
            r#"<p:sldId id="{id}" r:id="rId{rid}"/>"#,
            id = 256 + i,
            rid = i + 1, // rId1..N nas presentation rels sao os slides (rIdM1 e o master, ver abaixo)
        ));
    }
    let presentation_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rIdM1"/></p:sldMasterIdLst><p:sldIdLst>{sld_id_lst}</p:sldIdLst><p:sldSz cx="{w}" cy="{h}" type="screen16x9"/><p:notesSz cx="6858000" cy="9144000"/></p:presentation>"#,
        sld_id_lst = sld_id_lst,
        w = PPTX_SLIDE_W_EMU,
        h = PPTX_SLIDE_H_EMU,
    );

    let mut presentation_rels = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    for i in 0..slide_entries.len() {
        presentation_rels.push_str(&format!(
            r#"<Relationship Id="rId{rid}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{n}.xml"/>"#,
            rid = i + 1,
            n = i + 1,
        ));
    }
    presentation_rels.push_str(
        r#"<Relationship Id="rIdM1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/></Relationships>"#,
    );

    let slide_master_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:bg><p:bgPr><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill><a:effectLst/></p:bgPr></p:bg><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/><p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst></p:sldMaster>"#;

    let slide_master_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/></Relationships>"#;

    let slide_layout_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="blank" preserve="1"><p:cSld name="Blank"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#;

    let slide_layout_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#;

    // Tema simplificado mas schema-valido: cores/fontes do Cerne, com os 3
    // niveis de fill/line/effect que o fmtScheme exige (mesmo padrao
    // reduzido usado por outros geradores minimos de pptx).
    let theme_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Cerne"><a:themeElements><a:clrScheme name="Cerne"><a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1><a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="1F1F1F"/></a:dk2><a:lt2><a:srgbClr val="EEEEEE"/></a:lt2><a:accent1><a:srgbClr val="6366F1"/></a:accent1><a:accent2><a:srgbClr val="8B5CF6"/></a:accent2><a:accent3><a:srgbClr val="EC4899"/></a:accent3><a:accent4><a:srgbClr val="F59E0B"/></a:accent4><a:accent5><a:srgbClr val="10B981"/></a:accent5><a:accent6><a:srgbClr val="3B82F6"/></a:accent6><a:hlink><a:srgbClr val="0563C1"/></a:hlink><a:folHlink><a:srgbClr val="954F72"/></a:folHlink></a:clrScheme><a:fontScheme name="Cerne"><a:majorFont><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme><a:fmtScheme name="Cerne"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="12700"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="19050"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>"#;

    let core_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:creator>Cerne Code</dc:creator></cp:coreProperties>"#
    );
    let app_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><Application>Cerne Code</Application><Slides>{n}</Slides></Properties>"#,
        n = slide_entries.len(),
    );

    let file = std::fs::File::create(target)
        .map_err(|e| anyhow!("nao foi possivel criar {}: {e}", target.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default();

    zip.start_file("[Content_Types].xml", opts)?;
    zip.write_all(content_types.as_bytes())?;

    zip.start_file("_rels/.rels", opts)?;
    zip.write_all(package_rels.as_bytes())?;

    zip.start_file("docProps/core.xml", opts)?;
    zip.write_all(core_xml.as_bytes())?;
    zip.start_file("docProps/app.xml", opts)?;
    zip.write_all(app_xml.as_bytes())?;

    zip.start_file("ppt/presentation.xml", opts)?;
    zip.write_all(presentation_xml.as_bytes())?;
    zip.start_file("ppt/_rels/presentation.xml.rels", opts)?;
    zip.write_all(presentation_rels.as_bytes())?;

    zip.start_file("ppt/slideMasters/slideMaster1.xml", opts)?;
    zip.write_all(slide_master_xml.as_bytes())?;
    zip.start_file("ppt/slideMasters/_rels/slideMaster1.xml.rels", opts)?;
    zip.write_all(slide_master_rels.as_bytes())?;

    zip.start_file("ppt/slideLayouts/slideLayout1.xml", opts)?;
    zip.write_all(slide_layout_xml.as_bytes())?;
    zip.start_file("ppt/slideLayouts/_rels/slideLayout1.xml.rels", opts)?;
    zip.write_all(slide_layout_rels.as_bytes())?;

    zip.start_file("ppt/theme/theme1.xml", opts)?;
    zip.write_all(theme_xml.as_bytes())?;

    for (i, (slide_xml, rels_xml, media)) in slide_entries.into_iter().enumerate() {
        let n = i + 1;
        zip.start_file(format!("ppt/slides/slide{n}.xml"), opts)?;
        zip.write_all(slide_xml.as_bytes())?;
        zip.start_file(format!("ppt/slides/_rels/slide{n}.xml.rels"), opts)?;
        zip.write_all(rels_xml.as_bytes())?;
        for part in media {
            zip.start_file(format!("ppt/media/{}", part.file_name), opts)?;
            zip.write_all(&part.bytes)?;
        }
    }

    zip.finish()?;
    Ok(())
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
    fn orchestration_tool_specs_has_the_four_session_tools() {
        // Fase G: só as 4 ferramentas de sessão orquestrada, nada mais —
        // guarda de profundidade (não deixa `task`/`run_pipeline` vazarem
        // pra cá) é responsabilidade de `agent/mod.rs` filtrar isso pra
        // sessões orquestradas, não desta função.
        let specs = orchestration_tool_specs();
        let names: Vec<&str> = specs.iter().map(|s| s.function.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "start_agent_session",
                "check_agent_session",
                "list_agent_sessions",
                "stop_agent_session",
            ]
        );
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
        let outcome = execute_project_tool("edit_file", &args, &dir, &[], &[], &crate::agent::background::BackgroundJobs::default(), &crate::models::ExecutionMode::Auto, &dir, "test-session")
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
        let outcome = execute_project_tool("edit_file", &args, &dir, &[], &[], &crate::agent::background::BackgroundJobs::default(), &crate::models::ExecutionMode::Auto, &dir, "test-session")
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
        let outcome = execute_project_tool("edit_file", &args, &dir, &[], &[], &crate::agent::background::BackgroundJobs::default(), &crate::models::ExecutionMode::Auto, &dir, "test-session")
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
        let outcome = execute_project_tool("edit_file", &args, &dir, &[], &[], &crate::agent::background::BackgroundJobs::default(), &crate::models::ExecutionMode::Auto, &dir, "test-session")
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
        let outcome = execute_project_tool("edit_file", &args, &dir, &[], &[], &crate::agent::background::BackgroundJobs::default(), &crate::models::ExecutionMode::Auto, &dir, "test-session")
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
        let outcome = execute_project_tool("edit_file", &args, &dir, &[], &[], &crate::agent::background::BackgroundJobs::default(), &crate::models::ExecutionMode::Auto, &dir, "test-session")
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
        let outcome = execute_project_tool("edit_file", &args, &dir, &[], &[], &crate::agent::background::BackgroundJobs::default(), &crate::models::ExecutionMode::Auto, &dir, "test-session")
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
        let outcome = execute_project_tool("run_command", &start_args, &dir, &[], &[], &background_jobs, &crate::models::ExecutionMode::Auto, &dir, "test-session")
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
        let checked = execute_project_tool("check_background_output", &check_args, &dir, &[], &[], &background_jobs, &crate::models::ExecutionMode::Auto, &dir, "test-session")
        .await
        .unwrap();
        assert!(
            checked.observation.contains("from-tool-dispatch"),
            "esperava ver o output: {}",
            checked.observation
        );

        let listed =
            execute_project_tool("list_background", &json!({}), &dir, &[], &[], &background_jobs, &crate::models::ExecutionMode::Auto, &dir, "test-session")
                .await
                .unwrap();
        assert!(listed.observation.contains(&id));

        let stopped =
            execute_project_tool("stop_background", &check_args, &dir, &[], &[], &background_jobs, &crate::models::ExecutionMode::Auto, &dir, "test-session")
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
        let outcome1 = execute_project_tool("edit_file", &first, &dir, &[], &[], &background_jobs, &crate::models::ExecutionMode::Auto, &dir, "test-session")
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
        let outcome2 = execute_project_tool("edit_file", &second, &dir, &[], &[], &background_jobs, &crate::models::ExecutionMode::Auto, &dir, "test-session")
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
            "test-session",
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
            &[],
            &background_jobs,
            &crate::models::ExecutionMode::Auto,
            &project,
            "test-session",
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
            execute_project_tool("read_file", &args, &project, &[], &[], &background_jobs, &crate::models::ExecutionMode::Auto, &project, "test-session").await;
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
            &[],
            &background_jobs,
            &crate::models::ExecutionMode::Auto,
            &project,
            "test-session",
        )
        .await
        .unwrap();
        assert!(listed.observation.contains("a.txt"));
        assert!(listed.observation.contains("sub"));

        let grep_args = json!({ "pattern": "hello", "path": extra.to_string_lossy().to_string() });
        let grepped =
            execute_project_tool("grep", &grep_args, &project, &extra_roots, &[], &background_jobs, &crate::models::ExecutionMode::Auto, &project, "test-session")
                .await
                .unwrap();
        assert!(grepped.observation.contains("a.txt"));

        fs::remove_dir_all(&project).ok();
        fs::remove_dir_all(&extra).ok();
    }

    #[tokio::test]
    async fn write_file_rejects_absolute_path_outside_project_root_in_manual_mode() {
        // Em modo Manual cada tool call ja pausa pedindo aprovacao, mas por
        // consistencia com o comportamento historico continuamos restritos
        // ao project_root nesse modo especifico.
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
            &[],
            &background_jobs,
            &crate::models::ExecutionMode::Manual,
            &project,
            "test-session",
        )
        .await;
        assert!(
            result.is_err(),
            "write_file em modo Manual nao deveria aceitar caminho absoluto fora do projeto"
        );

        fs::remove_dir_all(&project).ok();
        fs::remove_dir_all(&extra).ok();
    }

    #[tokio::test]
    async fn write_file_allows_absolute_path_outside_project_root_in_auto_and_yolo_mode() {
        // Auto e YOLO liberam escrever num caminho absoluto fora do projeto
        // (ex: usuario aponta uma pasta externa no composer) - a sandbox
        // espelha em `_external` (Auto) ou escreve direto (YOLO).
        for mode in [
            crate::models::ExecutionMode::Auto,
            crate::models::ExecutionMode::Yolo,
        ] {
            let project = scratch_dir();
            let extra = scratch_dir();
            let background_jobs = crate::agent::background::BackgroundJobs::default();

            let target = extra.join("novo.txt");
            let args =
                json!({ "path": target.to_string_lossy().to_string(), "content": "conteudo externo" });
            let result = execute_project_tool("write_file", &args, &project, &[], &[], &background_jobs, &mode, &project, "test-session")
            .await
            .unwrap_or_else(|e| panic!("write_file deveria aceitar caminho externo em modo {mode:?}: {e}"));

            if mode == crate::models::ExecutionMode::Yolo {
                assert!(
                    target.exists(),
                    "YOLO deveria ter escrito direto no arquivo externo"
                );
                assert_eq!(fs::read_to_string(&target).unwrap(), "conteudo externo");
            } else {
                assert!(
                    result.observation.contains("sandbox"),
                    "Auto deveria escrever a edicao pendente na sandbox, nao direto no externo"
                );
                assert!(!target.exists(), "Auto nao deveria escrever direto no arquivo externo");
            }

            fs::remove_dir_all(&project).ok();
            fs::remove_dir_all(&extra).ok();
        }
    }

    /// T16 — pptx com titulo, paragrafo, bullets, tabela e uma imagem real
    /// (PNG minusculo gerado em memoria via a crate `image`) — confere que o
    /// zip resultante tem as partes obrigatorias do OOXML e que os bytes da
    /// imagem embutida batem com o arquivo original, byte a byte.
    #[test]
    fn create_pptx_end_to_end_with_image() {
        let dir = scratch_dir();
        let img_path = dir.join("logo.png");
        let img = image::RgbaImage::from_pixel(4, 2, image::Rgba([255, 0, 0, 255]));
        image::DynamicImage::ImageRgba8(img)
            .save(&img_path)
            .unwrap();
        let original_png_bytes = fs::read(&img_path).unwrap();

        let out_path = dir.join("apresentacao.pptx");
        let slides = vec![
            json!({
                "title": "Slide 1",
                "elements": [
                    { "type": "paragraph", "text": "Um paragrafo em negrito.", "bold": true, "size": 20 },
                    { "type": "bullets", "items": ["Topico A", "Topico B", "Topico C"] },
                ]
            }),
            json!({
                "title": "Slide 2 — tabela e imagem",
                "elements": [
                    { "type": "table", "headers": ["Coluna 1", "Coluna 2"], "rows": [["a", "b"], ["c", "d"]] },
                    { "type": "image", "path": img_path.to_string_lossy().to_string() },
                ]
            }),
        ];
        write_pptx(&out_path, &slides, &dir).unwrap();

        let file = fs::File::open(&out_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();

        for expected in [
            "[Content_Types].xml",
            "_rels/.rels",
            "ppt/presentation.xml",
            "ppt/_rels/presentation.xml.rels",
            "ppt/slideMasters/slideMaster1.xml",
            "ppt/slideLayouts/slideLayout1.xml",
            "ppt/theme/theme1.xml",
            "ppt/slides/slide1.xml",
            "ppt/slides/slide2.xml",
            "ppt/slides/_rels/slide1.xml.rels",
            "ppt/slides/_rels/slide2.xml.rels",
            "ppt/media/image1.png",
        ] {
            assert!(names.contains(&expected.to_string()), "esperava a parte '{expected}' no zip, achou: {names:?}");
        }

        let mut content_types = String::new();
        std::io::Read::read_to_string(&mut archive.by_name("[Content_Types].xml").unwrap(), &mut content_types).unwrap();
        assert!(content_types.contains(r#"Extension="png""#));
        assert!(content_types.contains("slide1.xml"));
        assert!(content_types.contains("slide2.xml"));

        let mut slide1 = String::new();
        std::io::Read::read_to_string(&mut archive.by_name("ppt/slides/slide1.xml").unwrap(), &mut slide1).unwrap();
        assert!(slide1.contains("Slide 1"));
        assert!(slide1.contains("Um paragrafo em negrito."));
        assert!(slide1.contains("Topico A"));
        assert!(slide1.contains("Topico B"));

        let mut slide2 = String::new();
        std::io::Read::read_to_string(&mut archive.by_name("ppt/slides/slide2.xml").unwrap(), &mut slide2).unwrap();
        assert!(slide2.contains("Coluna 1"));
        assert!(slide2.contains(r#"<a:t>a</a:t>"#));
        assert!(slide2.contains("<p:pic>"), "slide 2 deveria ter a imagem embutida: {slide2}");

        let mut embedded_png = Vec::new();
        std::io::Read::read_to_end(&mut archive.by_name("ppt/media/image1.png").unwrap(), &mut embedded_png).unwrap();
        assert_eq!(embedded_png, original_png_bytes, "imagem embutida deveria ser identica ao arquivo original");

        fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn create_pptx_rejects_empty_slides_via_execute_project_tool() {
        let dir = scratch_dir();
        let background_jobs = crate::agent::background::BackgroundJobs::default();
        let args = json!({ "path": "out.pptx", "slides": [] });
        let result = execute_project_tool(
            "create_pptx",
            &args,
            &dir,
            &[],
            &[],
            &background_jobs,
            &crate::models::ExecutionMode::Auto,
            &dir,
            "test-session",
        )
        .await;
        assert!(result.is_err(), "slides vazio deveria ser rejeitado");
        fs::remove_dir_all(&dir).ok();
    }
}
