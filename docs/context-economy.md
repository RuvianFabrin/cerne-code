# Economia de contexto — núcleo Rust

Ative em **+ → Economia de contexto**. A preferência é por conversa.
**O usuário não precisa instalar Node nem baixar um servidor para ativar.**
SQLite/FTS5 e o motor JavaScript Boa são compilados no aplicativo.
Node continua sendo uma ferramenta de desenvolvimento do frontend.

## O que economiza contexto

- Analisa dados em código e retorna o resultado impresso, evitando enviar arquivos inteiros ao modelo.
- Guarda conteúdo completo em SQLite antes de reduzir saídas grandes de ferramentas.
- Divide Markdown, JSON e texto em seções pesquisáveis; mantém o texto bruto completo.
- Busca com Porter/BM25, trigramas, fusão RRF, proximidade e correção de digitação.
- Indexa arquivos/diretórios autorizados e atualiza arquivos alterados antes da busca.
- Busca páginas públicas, converte HTML, usa cache TTL e bloqueia endereços privados/redirecionamentos inseguros.
- Captura histórico, ferramentas, decisões, regras, erros e referências para retomada.

**Compacta automaticamente ao alcançar 80% da janela estimada**, quando o corte
reduz o tamanho e existem grupos antigos suficientes. Mantém os últimos grupos
completos de ferramentas e um snapshot com referências pesquisáveis.
**Compactar agora** usa esse mecanismo quando a opção está ativa.
Não há garantia de uma porcentagem fixa de economia ou de caber numa janela
menor que as instruções, ferramentas e mensagens recentes.

O histórico visível não é substituído pelo resumo. Saídas acima de 6.000 bytes
são arquivadas integralmente antes de enviar uma prévia de 800 caracteres e
uma referência ao modelo. Se arquivar falhar, a redução não ocorre.
As ferramentas ctx_* controlam suas próprias saídas, evitando arquivamento recursivo.

## Tarefas longas

Sem Long Horizon, não há teto fixo de 50 etapas por turno. A Economia de
contexto pode compactar a visão enviada ao modelo e a execução continua.
O botão Parar e a detecção de 20 chamadas idênticas consecutivas continuam
ativos; consultas de status próprias para acompanhamento têm exceções.

Os limites de etapas, chamadas ao modelo, tempo e tokens acumulados são
exclusivos do Long Horizon. A mensagem de pausa informa qual limite foi
atingido, em vez de listar motivos genéricos.

## Comandos de barra

São executados diretamente pelo Cerne, sem precisar consultar o modelo.
Aceitam também o prefixo `/context-mode:`, como `/context-mode:ctx-stats`.

| Comando | Uso |
|---|---|
| `/ctx-stats` | Métricas por ferramenta, bytes e estimativa de tokens |
| `/ctx-doctor` | Diagnóstico Rust/SQLite/FTS5 e integridade |
| `/ctx-index "caminho"` | Indexar arquivo ou diretório autorizado |
| `/ctx-search termos` | Recuperar trechos guardados |
| `/ctx-purge` | Explicar a limpeza sem apagar dados |
| `/ctx-purge --confirm` | Limpar o índice ativo desta conversa |
| `/ctx-insight` | Relatório local, sem envio a serviço externo |
| `/ctx-upgrade` | Explicar atualização pelo instalador do Cerne |

Parâmetros avançados também aceitam JSON, por exemplo:
`/ctx-index {"path":"src","maxFiles":100,"maxDepth":5}`.
A limpeza preserva o histórico e os bancos antigos; uma sincronização posterior
pode reconstruir conteúdo do histórico. Ela não apaga outras conversas.

## Persistência e migração

Banco nativo: `<app_data>/sessions/<id>/context-economy/native.sqlite3`.
Cada conversa tem bancos/eventos/métricas independentes. WAL, transações e
hashes permitem persistência e importação idempotente do histórico.

Na ativação, o Cerne importa os bancos antigos em **modo somente leitura**.
Os arquivos originais permanecem. Conteúdo de chunks antigos é importado
separadamente para preservar cada seção; não é possível recuperar bytes que
o mecanismo anterior nunca guardou. O histórico completo também é reindexado.
Desligar a opção preserva os dados e remove o corte da visão enviada ao modelo.

## Execução e diferenças do original

Esta é uma portabilidade dos mecanismos centrais, não uma reprodução literal
de todos os adaptadores, hooks e APIs Node do projeto original.

- JavaScript: Boa embutido, JSON, console, FILE_CONTENT, leitura/listagem/escrita básica de fs e child_process.execSync. Não há npm, módulos arbitrários, timers ou rede Node.
- O desempenho do Boa difere do Node. No teste de 1,2 MB, uma busca direta com expressão regular foi rápida; criar e filtrar 100 mil itens excedeu 30 segundos. Prefira buscas diretas em strings ou indexação/busca nativa; ajuste timeout quando a análise exigir mais trabalho.
- Shell: usa o shell detectado pelo Cerne. FILE_CONTENT é fornecido pelo ambiente; o limite do ambiente do sistema pode restringir arquivos grandes nessa linguagem.
- Python, Ruby, PHP, Perl, R, Elixir e Go: runtimes opcionais já instalados. Rust: rustc opcional, compilação e execução dentro do timeout. C#: dotnet-script opcional. TypeScript: Deno opcional, sem downloads remotos. Nenhum runtime é instalado automaticamente.
- FILE_CONTENT em runtimes opcionais usa arquivo temporário; Rust/Elixir também expõem file_content. Código de Rust/Go com arquivo é envolvido em main, como na referência.
- Execução JavaScript ocorre num processo do próprio Cerne. Timeout/cancelamento encerram a árvore de processos; timeout mantém a saída parcial já capturada. O worker transmite impressões ao processo pai durante a execução. A captura de saída tem limites. Isso não oferece isolamento de uma VM/container. Shell e runtimes externos mantêm as permissões do usuário.
- Execução em segundo plano usa shell e o gerenciador de tarefas existente do Cerne. Não reproduz o detach de todos os runtimes do servidor original.
- Estatísticas de tokens são estimativas. O Insight é local; o painel organizacional hospedado não está integrado. A atualização é pelo Cerne.
- A classificação de eventos foi adaptada ao vocabulário do Cerne e ao português. Conteúdo não classificado continua guardado e pesquisável; não se promete classificação idêntica à do extrator TypeScript.

Manual/Auto mantêm as aprovações do Cerne para ctx_execute, ctx_execute_file,
ctx_batch_execute e ctx_purge. YOLO segue o modo escolhido. Comandos de barra
locais não executam código; ctx-purge exige a confirmação explícita no comando.
Pastas extras autorizam leitura, enquanto o fs de JavaScript restringe escrita
ao projeto. Código shell/runtimes não é uma barreira de permissões do sistema.

O modo CLI externo tem seu próprio loop e não usa esta integração.
Com Long Horizon, esse modo mantém seus checkpoints; Economia de contexto
acrescenta ferramentas, captura e redução de saídas. A fila de tarefas continua
usando o mecanismo existente do Cerne.

## Créditos e licenças

Referência: **Context Mode 1.0.169**, commit
`8de5f73af9365d459bdfc564c045948cc3fc488b`, de **Mert Koseoğlu (mksglu)**,
copyright 2026 Mert Koseoglu, **Elastic License 2.0**.
[Repositório original](https://github.com/mksglu/context-mode).
A fonte original permanece em `src-tauri/resources/context-mode` para estudo.
O instalador distribui sua licença e proveniência, sem distribuir Node/servidor JS.
O port é derivado dessa referência e preserva sua atribuição/licença.

Boa (MIT/Unlicense), rusqlite (MIT), SQLite (domínio público) e SHA-2 (MIT/Apache-2.0)
têm avisos distribuídos em `context-economy-notices`. Créditos aparecem em **Sobre**.

## Verificação

- `npm run test:context-economy`: testes nativos específicos.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib --locked`: suíte Rust.
- `npm run build`: tipos e frontend.
- `npm run tauri -- build --bundles nsis`: instalador Windows.
- `node scripts/test-context-worker.mjs`: processo real do Cerne, com PATH sem Node.

O código TypeScript anterior e os scripts `*:reference` são apenas referência
de desenvolvimento, sem participação no build normal ou na ativação.
Windows é validado localmente; Linux/macOS exigem execução do CI.
