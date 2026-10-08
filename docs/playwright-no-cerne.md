# Navegador para tarefas no Cerne

Playwright MCP 0.0.83 fica pré-configurado como servidor `playwright`.
O usuário pode desativá-lo em Configurações → Servidores MCP, ou somente
na conversa pelo menu + → MCP. Remover ou desativar é respeitado ao reabrir.

Em Configurações → Servidores MCP → Playwright → Configurar, escolha
Google Chrome ou Microsoft Edge e o modo de conexão. O navegador escolhido e Node.js 18+ precisam
estar instalados. O pacote oficial é obtido pelo npm na primeira conexão;
a partir daí é reutilizado do cache. Não exige extensão neste modo.

O navegador abre visível quando uma ferramenta precisar dele. Seu perfil
separado guarda logins em `browser-profiles/chrome` ou `browser-profiles/msedge`
na pasta de dados do Cerne. Faça login manualmente nessa janela quando necessário.
Trocar de navegador ou desativar globalmente encerra a conexão antiga.

Ferramentas: abrir/selecionar abas, ler os elementos da página, preencher
formulários, clicar e consultar mensagens e erros do console após a conexão.
O modelo não precisa de visão para trabalhar com a estrutura da página.
O usuário pode acompanhar as ações na janela do navegador.

Não existe garantia de superar bloqueios de automação. O Cerne recebe
instrução para pausar e pedir ajuda diante de CAPTCHA ou bloqueio, evitando
repetir a tentativa indefinidamente. Este recurso não inclui disfarce de
fingerprint, rotação de proxies ou resolução automática de CAPTCHA.

Validação: teste de integração com página HTTP local, servidor MCP real,
preenchimento por referência de elemento, clique, alteração observável da
página e captura de console. O perfil de teste contém espaços no caminho.
Também foram testadas migração sem sobrescrever MCPs existentes e persistência
da escolha de desativar/remover. Não valida compatibilidade com todos os sites.


Resultado: teste real passou no Chrome e no Edge. Suíte Rust: 330 passaram,
0 falhas, 5 ignorados (inclui o teste real, executado separadamente nos dois
navegadores). Build Vue/TypeScript concluído. O cadastro local foi salvo sem
alterar o MCP existente, com backup antes da inclusão.

## Perfil existente (7 de outubro)

No botão Configurar, escolha **Meu navegador e logins (extensão)**.
Instale a Playwright Extension no navegador e no perfil que quer controlar.
O botão Abrir página da extensão abre o navegador escolhido. Mantenha uma
aba normal aberta; clique Testar conexão e autorize o pedido da extensão.
O campo opcional aceita a pasta do perfil (Default, Profile 1), não um caminho.
A extensão usa o perfil existente sem copiar cookies ou alterar seus arquivos.
Ao trocar a configuração ou desabilitar o MCP, Cerne desconecta sem enviar
browser_close ao seu navegador pessoal.

O modo Perfil separado continua disponível e não precisa de extensão.
Salvar preserva a escolha anterior de habilitar/desabilitar o MCP.
A instalação e autorização da extensão são manuais. O teste da conexão só
confirma sucesso após listar as abas pelo navegador real, não apenas após
iniciar o processo MCP. O modo com extensão não foi validado ao vivo neste
PC porque a extensão ainda não está instalada/autorizada.

Referência oficial: https://playwright.dev/mcp/configuration/browser-extension
