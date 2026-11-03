# CLI como agente ou provedor

Na conversa, selecione um provedor CLI e abra **+ → Modo do CLI**.

- **Agente do CLI**: comportamento anterior. O CLI controla suas ferramentas e contexto; o Cerne recebe a resposta final. Conversas existentes mantêm esse modo.
- **Provedor do Cerne**: Claude, Codex ou Pi fornecem uma resposta por chamada. O Cerne envia a visão atual do histórico e suas ferramentas, valida o JSON e executa as chamadas pelo fluxo normal, com permissões, eventos, edições pendentes e botão Parar.

Cada inferência inicia um processo/sessão descartável. Não há resume/continue do CLI. Os resultados retornam ao histórico do Cerne antes da próxima inferência. Long Horizon, fila de tarefas, persona, skills e Economia de contexto usam o mesmo fluxo dos provedores normais. O Cerne não usa HTTP para consultar o modelo nesse modo.

## Codex

Reutiliza a autenticação existente do CLI. A lista e a janela de contexto vêm de `$CODEX_HOME/models_cache.json` (ou `~/.codex/models_cache.json`). É necessário abrir o Codex autenticado uma vez para carregar o catálogo. A lista local pode estar desatualizada; o serviço ainda pode negar um modelo.

Cada chamada usa `exec --ephemeral --ignore-user-config --ignore-rules`, pasta temporária, sandbox somente leitura e saída JSON com schema. Um catálogo temporário derivado do catálogo local desativa shell, apply_patch, ferramentas experimentais e code mode. Recursos de agentes, apps, plugins, imagens como ferramenta, espera, goals e shell snapshot também ficam desativados. A saída de eventos é auditada para recusar ferramentas nativas inesperadas. O catálogo original e a configuração global do Codex não são modificados.

Imagens anexadas PNG/JPEG/WebP são materializadas na pasta temporária e enviadas por `-i`. Não se baixam URLs de imagens. Até 32 imagens e 40 MB por chamada. A localização de cada imagem é indicada na mensagem de origem.

## Claude e Pi

Claude usa ferramentas nativas vazias, MCP vazio exclusivo, resposta estruturada e nenhuma persistência de sessão. `--safe-mode` ignora customizações locais sem desativar o login por assinatura. O histórico completo vai em um registro `stream-json`, com blocos de texto e imagens PNG/JPEG/WebP em base64. A saída também usa `stream-json`, exigido pelo CLI 2.1.302: o Cerne extrai o resultado final e soma os tokens de entrada, criação e leitura de cache. Até 32 imagens, 5 MB por imagem e 10 MB no registro completo; os limites podem exigir compactar a conversa antes de continuar. Imagens são numeradas na ordem das mensagens de origem. Erros de autenticação enviados em stdout agora aparecem na mensagem de erro do Cerne.

Pi usa JSON, nenhuma ferramenta, extensão, skill, arquivo de contexto ou template externo, sem salvar sessão. Esse adaptador continua aceitando texto.

No modo **Agente do CLI**, Claude e Codex também recebem as imagens da mensagem atual. Claude usa os mesmos blocos multimodais; Codex recebe arquivos temporários por `-i`, removidos ao concluir. Nos demais CLIs, um anexo gera erro explícito em vez de ser ignorado.

O Claude precisa estar autenticado com uma conta que tenha acesso ao Claude Code. A conta gratuita do chat Claude, sozinha, não dá esse acesso. O Cerne não cadastra chaves, não compra créditos e não altera planos. Referência: https://code.claude.com/docs/en/authentication

O Pi consulta os modelos disponíveis com `--list-models` em modo offline, sem iniciar inferência. Os identificadores de Claude têm uma lista sugerida. O seletor de modelo dos CLIs permite digitar um identificador manualmente, inclusive `provider/model` no Pi. A disponibilidade e a autenticação são determinadas pelo próprio CLI. Gemini e Qwen continuam disponíveis como agentes, ainda sem adaptador de provedor.

Os argumentos extras das configurações só são usados no modo agente: no modo provedor, poderiam reativar ferramentas ou retomar sessões e por isso são ignorados. O caminho do executável continua configurável.

## Detecção automática ao iniciar

Ao abrir, o Cerne procura Claude, Codex, Pi, Gemini e Qwen no PATH e em diretórios padrão de instalação. No Windows isso inclui `~/.local/bin`, npm, NVM e a instalação do aplicativo Codex. Isso cobre o caso de PATH antigo herdado pelo aplicativo aberto. O caminho explícito salvo em Configurações continua tendo prioridade, inclusive se estiver incorreto. Nenhum CLI é instalado, autenticado ou chamado para inferência nessa verificação.

## Limites e validação

- Até 5 minutos por inferência; não é um limite de duração da tarefa inteira.
- Captura limitada de stdout/stderr; timeout/cancelamento encerra a árvore de processos e remove arquivos temporários.
- Nome de ferramenta precisa existir no catálogo daquela fase; argumentos precisam ser objeto JSON com os campos obrigatórios. Os handlers nativos fazem a validação específica antes de agir.
- Um lote pode pedir até 32 ferramentas. IDs são gerados pelo Cerne.
- O CLI pode acrescentar instruções ou overhead próprios. O modo provedor não equivale exatamente ao acesso direto por API, nem garante o mesmo consumo ou desempenho.
- Claude/Pi não fornecem streaming de texto parcial nessa integração. O estado de processamento continua visível enquanto a chamada ocorre.

Teste local opcional com duas chamadas reais ao Codex e busca AST nativa:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib live_codex_requests_native_ast_then_continues_in_a_fresh_session -- --ignored --nocapture
```

Esse teste precisa de autenticação e pode consumir a franquia normal do CLI.

## Validação nesta máquina

- 404 testes automatizados passaram, sem falhas; 10 testes externos/diagnósticos ficaram ignorados na execução comum.
- Os testes reais de Codex passaram: AST nativa + continuação em nova sessão e análise de imagem anexada. A análise da imagem foi revalidada nesta alteração nos dois modos: Provedor do Cerne e Agente do CLI.
- Consulta offline do Pi retornou 17 modelos. Não foi feita inferência autenticada com Pi nem Claude nesta máquina.
- A detecção automática encontrou Claude, Codex e Pi nesta máquina, com caminhos absolutos, e marcou Gemini e Qwen como ausentes.
- Claude 2.1.302 aceitou a entrada multimodal com saída stream-json, mas retornou falha de autenticação, sem tokens e com custo reportado zero. A interpretação visual pelo modelo permanece sem validação por falta de acesso autenticado.
- O caminho do Codex instalado foi salvo no Cerne, com backup do config.json. Foram criadas conversas separadas “Teste Codex — Agente do CLI” e “Teste Codex — Provedor do Cerne”, com projetos em `C:\cerne-cli-tests\agent` e `C:\cerne-cli-tests\provider`. A segunda já tem Economia de contexto ligada; Long Horizon pode ser ativado pelo usuário.
- Não foi realizada instalação interativa nem navegação na interface do novo build. Instale com o Cerne fechado e reabra para carregar a configuração e as conversas de teste.

## Pastas da sessão

Todas as pastas indicadas pelo usuário compõem o workspace, sem pasta principal ou prioridade. O contexto informa os caminhos absolutos e as permissões de leitura/escrita de cada pasta. Quando há pastas anexadas, o campo legado `project_root` não é apresentado como outro alvo ao modelo; ele só é usado como fallback de compatibilidade quando a lista está vazia.

O painel continua com uma lista de pastas e seus controles de permissões. Os controles de pasta principal introduzidos durante o diagnóstico foram removidos conforme a preferência do usuário.

A lista atual prevalece sobre caminhos antigos no histórico. `list_dir` continua informando o caminho consultado, a quantidade de itens e a confirmação de pasta vazia.
