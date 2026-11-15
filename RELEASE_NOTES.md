# Cerne Code v0.1.6

## Português (Brasil)

- **Modo Anything:** estudo de programas, DLLs, Java, Android, WebAssembly e Electron com o motor craft-anything e o Ghidra. Painel na sessão, lista de fontes (pastas e arquivos), varredura que reconhece vários formatos, estudo em lote e instalação guiada do motor, do Ghidra e do Java (o motor já vem no instalador do Windows).
- **Fila validada por código:** cada ficha de função é conferida antes de ser aceita; módulos agrupam funções avulsas por endereço e recebem nome descritivo; a síntese gera módulos, relatório e entrega final. A entrega agora é dividida por módulo, a escrita de cada item fica restrita à sua pasta e os ids de evidência citados são conferidos contra o armazém de evidências.
- **Prompts editáveis:** o prompt da entrega final pode ser editado nas Configurações e por sessão, com botão Redefinir. Os prompts de função, módulo e relatório ficam em arquivos.
- **Modelos locais:** o servidor só sobe ao enviar a mensagem, com comando de servidor por sessão. O pacote de ferramentas nativas do motor volta a ser ligado automaticamente quando faltar, evitando chamadas de ferramenta devolvidas como texto.
- **Computer use por grade numerada:** novas ferramentas `grid_screenshot`, `grid_click` e `grid_move` desenham uma grade numerada no print; o modelo escolhe a célula e o código calcula o ponto na tela, com refinamento por célula. Útil para modelos com pouca precisão em coordenadas.
- **Agente e fila:** modo Agente do CLI retoma a sessão do próprio CLI; fila de tarefas sem Long Horizon com Ctx fixo em blocos, sub-agentes em segundo plano e memória da sessão.
- **Ferramentas novas:** documentos Word/PDF/PowerPoint com runtime Node embutido, glob, busca de skills, anexo de arquivo no chat, `report_findings`, `notebook_edit`, `show_widget` e `html_to_pdf`; overlay do Computer Use e perfis de navegador por sessão.

**Plataformas:** Windows, macOS universal (Apple Silicon e Intel) e Linux `full` e `compat`. Linux `compat` não inclui captura/automação nativa de desktop. Servidores/modelos locais precisam ser instalados e configurados em cada máquina. Builds automatizados não substituem testes em hardware real; a grade de computer use e a entrega por módulo do Anything ainda precisam de mais testes de uso.

## English

- **Anything mode:** study programs, DLLs, Java, Android, WebAssembly and Electron apps with the craft-anything engine and Ghidra. Session panel, source list (folders and files), a scanner that recognizes many formats, batch study, and guided setup of the engine, Ghidra and Java (the engine now ships in the Windows installer).
- **Code-validated queue:** each function card is checked before it is accepted; modules group loose functions by address and get descriptive names; synthesis produces modules, a report and a final delivery. The delivery is now split per module, each item may only write inside its own folder, and cited evidence ids are checked against the evidence store.
- **Editable prompts:** the final-delivery prompt can be edited in Settings and per session, with a Reset button. Function, module and report prompts live in files.
- **Local models:** the server only starts when a message is sent, with a per-session server command. The engine's native toolset is switched on automatically when missing, preventing tool calls returned as plain text.
- **Numbered-grid computer use:** new `grid_screenshot`, `grid_click` and `grid_move` tools overlay a numbered grid on the screenshot; the model picks a cell and the code computes the screen point, with per-cell refinement. Helpful for models that are imprecise with coordinates.
- **Agent and queue:** the CLI Agent mode resumes the CLI's own session; task queue without Long Horizon, with fixed-context blocks, background sub-agents and session memory.
- **New tools:** Word/PDF/PowerPoint documents with a bundled Node runtime, glob, skill search, file attachments in chat, `report_findings`, `notebook_edit`, `show_widget` and `html_to_pdf`; Computer Use overlay and per-session browser profiles.

**Platforms:** Windows, universal macOS (Apple Silicon and Intel), and Linux `full` and `compat`. Linux `compat` excludes native desktop capture/automation. Local servers/models must be installed and configured on each machine. Automated builds do not replace real hardware testing; the grid computer use and Anything's per-module delivery still need more real-world testing.

## Español

- **Modo Anything:** estudio de programas, DLL, Java, Android, WebAssembly y Electron con el motor craft-anything y Ghidra. Panel en la sesión, lista de fuentes (carpetas y archivos), escáner que reconoce muchos formatos, estudio por lotes e instalación guiada del motor, Ghidra y Java (el motor ya viene en el instalador de Windows).
- **Cola validada por código:** cada ficha de función se comprueba antes de aceptarse; los módulos agrupan funciones sueltas por dirección y reciben un nombre descriptivo; la síntesis genera módulos, informe y entrega final. La entrega ahora se divide por módulo, cada elemento solo puede escribir en su carpeta y los ids de evidencia citados se comprueban contra el almacén de evidencias.
- **Prompts editables:** el prompt de la entrega final se edita en Configuración y por sesión, con botón Restablecer. Los prompts de función, módulo e informe están en archivos.
- **Modelos locales:** el servidor solo arranca al enviar el mensaje, con comando de servidor por sesión. El paquete de herramientas nativas del motor se activa automáticamente si falta, evitando llamadas de herramienta devueltas como texto.
- **Computer use con cuadrícula numerada:** las nuevas herramientas `grid_screenshot`, `grid_click` y `grid_move` dibujan una cuadrícula numerada en la captura; el modelo elige la celda y el código calcula el punto en pantalla, con refinamiento por celda. Útil para modelos poco precisos con coordenadas.
- **Agente y cola:** el modo Agente del CLI retoma la sesión del propio CLI; cola de tareas sin Long Horizon con Ctx fijo en bloques, sub-agentes en segundo plano y memoria de sesión.
- **Herramientas nuevas:** documentos Word/PDF/PowerPoint con runtime Node integrado, glob, búsqueda de skills, adjuntos en el chat, `report_findings`, `notebook_edit`, `show_widget` y `html_to_pdf`; overlay de Computer Use y perfiles de navegador por sesión.

**Plataformas:** Windows, macOS universal (Apple Silicon e Intel) y Linux `full` y `compat`. Linux `compat` no incluye captura/automatización nativa del escritorio. Los servidores/modelos locales deben instalarse y configurarse en cada equipo. Los builds automáticos no sustituyen pruebas en hardware real; la cuadrícula de computer use y la entrega por módulo de Anything aún necesitan más pruebas de uso.

## 中文

- **Anything 模式：** 借助 craft-anything 引擎和 Ghidra 研究程序、DLL、Java、Android、WebAssembly 和 Electron 应用。会话面板、来源列表（文件夹和文件）、可识别多种格式的扫描器、批量研究，以及引擎、Ghidra 和 Java 的引导式安装（Windows 安装包已内置引擎）。
- **由代码校验的队列：** 每张函数卡片在接受前都会被检查；模块按地址归并零散函数并获得描述性名称；综合阶段生成模块、报告和最终交付。交付现按模块拆分，每个条目只能写入自己的文件夹，所引用的证据 ID 会与证据库核对。
- **可编辑的提示词：** 最终交付提示词可在设置中和按会话编辑，并带有“重置”按钮。函数、模块和报告提示词以文件形式存放。
- **本地模型：** 服务器仅在发送消息时启动，并支持按会话设置服务器命令。引擎的原生工具包缺失时会自动开启，避免工具调用被当作纯文本返回。
- **编号网格的计算机操作：** 新增 `grid_screenshot`、`grid_click` 和 `grid_move` 工具，在截图上叠加编号网格；模型选择格子，代码计算屏幕坐标，并支持按格子细化。适合坐标精度较低的模型。
- **代理与队列：** CLI 代理模式可恢复 CLI 自身的会话；不使用 Long Horizon 的任务队列，支持分块固定上下文、后台子代理和会话记忆。
- **新工具：** 内置 Node 运行时的 Word/PDF/PowerPoint 文档、glob、技能搜索、聊天文件附件、`report_findings`、`notebook_edit`、`show_widget` 和 `html_to_pdf`；Computer Use 叠加层以及按会话的浏览器配置文件。

**平台：** Windows、通用 macOS（Apple Silicon 和 Intel），以及 Linux 的 `full` 和 `compat` 版本。Linux `compat` 不包含原生桌面截图/自动化。每台设备都需要单独安装和配置本地服务器/模型。自动构建不能替代真实硬件测试；网格式计算机操作和 Anything 的按模块交付仍需更多实际使用测试。
