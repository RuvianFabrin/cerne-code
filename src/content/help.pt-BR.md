# O que o Cerne Code sabe fazer

Este é o catálogo de ferramentas que o agente tem à disposição. Em cada turno, o modelo decide sozinho qual ferramenta chamar (ou nenhuma) com base no pedido — as colunas "Quando o agente decide usar" abaixo vêm direto da descrição que cada ferramenta recebe do próprio código, então refletem exatamente o que o modelo lê antes de escolher.

## Sempre disponíveis (com ou sem pasta de projeto)

| Ferramenta | O que faz | Quando o agente decide usar |
| --- | --- | --- |
| `web_search` | Busca na web e retorna título/URL/trecho dos resultados mais relevantes. Por padrão agrega DuckDuckGo + Brave + Mojeek em paralelo, sem exigir conta nem instalação, removendo duplicatas e rankeando pelo consenso entre as fontes (configurável em Configurações → Busca na web). Aceita uma ou mais queries por chamada. | Quando falta informação que não está no projeto nem no treino do modelo — versões recentes de uma lib, documentação externa, notícias. O agente decide sozinho quantas queries mandar numa chamada: uma para pedidos diretos, várias (frases diferentes, sinônimos) quando o pedido tem múltiplos ângulos ou a primeira busca não trouxe o suficiente. |
| `web_fetch` | Busca uma URL específica e retorna o texto visível da página, sem HTML/scripts. | Depois de um `web_search`, pra ler uma fonte inteira em vez de confiar só no trecho do resultado. |
| `load_skill` | Carrega o conteúdo completo de uma skill pelo nome exato, a partir do catálogo listado no início da conversa. | Quando o pedido do usuário casa com a descrição de uma skill cadastrada (ver seção Skills abaixo). |
| `ask` | Pausa o turno e pergunta algo específico ao usuário, com opções de múltipla escolha e/ou texto livre. | Quando uma decisão só o usuário pode tomar — escolher entre abordagens, confirmar uma ação arriscada, desambiguar um pedido — em vez de assumir e seguir. Usado com moderação, só quando o agente realmente travaria sem essa resposta. |

## Só com pasta de projeto aberta

| Ferramenta | O que faz | Quando o agente decide usar |
| --- | --- | --- |
| `read_file` | Lê o conteúdo de um arquivo do projeto (ou de uma pasta extra de leitura liberada pra sessão). | Sempre que precisa ver o conteúdo real de um arquivo antes de explicar, editar ou usar como referência. |
| `list_dir` | Lista arquivos e subpastas de um diretório. | Pra entender a estrutura do projeto antes de mexer em algo, ou achar onde um arquivo está. |
| `grep` | Busca um padrão (regex) no conteúdo dos arquivos. | Achar onde um texto, símbolo ou string aparece no projeto. |
| `ast_grep` | Busca estrutural de código (pela forma da AST, não texto solto) — `$VAR` casa um nó qualquer, `$$$ARGS` casa zero-ou-mais. | Preferida ao `grep` quando a busca é sobre estrutura de código (chamada de função, import, declaração) em vez de texto solto. |
| `run_command` | Roda um comando de shell no diretório do projeto. Com `background=true`, não espera o comando terminar (pra dev server, watch mode). | Rodar testes, build, lint, scripts do projeto; `background=true` especificamente pra processos que ficam rodando de propósito. |
| `check_background_output` | Lê o output acumulado e o status de um comando iniciado em segundo plano, sem pará-lo. | Conferir o progresso de um dev server ou processo longo já iniciado com `run_command(background=true)`. |
| `stop_background` | Encerra um comando em segundo plano (mata o processo). | Depois de confirmar que algo subiu certo, ou antes de subir uma versão nova no lugar da antiga. |
| `list_background` | Lista todo comando em segundo plano conhecido, rodando ou já encerrado. | Antes de iniciar um novo dev server, pra checar se já não tem um rodando de uma sessão anterior. |
| `write_file` | Cria ou sobrescreve um arquivo. A escrita vai pra uma pasta sandbox espelhada — o usuário precisa aceitar o diff na interface antes de aplicar no arquivo real. | Criar um arquivo novo ou substituir o conteúdo inteiro de um já existente. |
| `edit_file` | Edita um arquivo existente substituindo uma ocorrência exata de um trecho por outro. Também escreve na sandbox, sujeito a aceite. | Mudanças pontuais e localizadas num arquivo já existente. |
| `ast_edit` | Reescrita estrutural: toda ocorrência do padrão (mesma sintaxe do `ast_grep`) é trocada pelo template de reescrita. | Refactors — rename de chamada, mudar import — com mais segurança que `edit_file` porque opera na estrutura, não em texto exato. |
| `task` | Delega uma sub-tarefa bem definida pra um sub-agente descartável, que roda seu próprio loop de ferramentas e devolve só o relatório final. | Sub-tarefas que exigem várias chamadas de ferramenta cujo processo intermediário não importa pro usuário, só o resultado (ex: "ache todos os usos de X e resuma onde estão"). |
| `verify_completion` | Dispara um verificador independente e cético (não o próprio agente) pra reconferir com evidência real se uma tarefa foi mesmo concluída. Só tem ferramentas de leitura/execução, nunca de edição. | Antes de declarar sucesso numa tarefa complexa (vários arquivos, algo construído do zero) — não usado em pedidos simples de uma única chamada, onde o resultado já é obviamente verificável. |

## Ferramentas MCP (servidores externos)

Cada servidor configurado em Configurações → Servidores MCP entra automaticamente no catálogo do agente como `mcp__{servidor}__{ferramenta}`. O agente decide usá-las do mesmo jeito que as ferramentas nativas — pela descrição que o próprio servidor MCP expõe. Não aparecem numa tabela fixa aqui porque variam de instalação pra instalação, dependendo de quais servidores você configurou.

## Skills

Uma skill é um arquivo `SKILL.md` com instruções que o agente carrega sob demanda via `load_skill`, em vez de precisar reexplicar o mesmo processo em toda conversa. No início de cada sessão, o agente recebe só o catálogo (nome + descrição) de cada skill disponível — o corpo completo só é lido se o agente decidir chamar `load_skill(nome)`. Crie e edite skills em Configurações → Skills.

## Modo Manual vs Automático

Cada sessão tem um modo de execução, escolhido no seletor ao lado do botão "+" no composer:

- **Automático** (padrão): toda ferramenta roda direto, sem pausa. Um botão "Cancelar" na lista de tarefas lateral interrompe o turno inteiro a qualquer momento.
- **Manual**: toda chamada de ferramenta (exceto `ask`, que já é uma pausa) para o turno e pede aprovação explícita antes de rodar — útil quando você quer revisar cada ação antes dela acontecer, em vez de só poder cancelar depois.
