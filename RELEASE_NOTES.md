# Cerne Code v0.1.4

## Português (Brasil)

- **Imagem e vídeo diretos:** botões no composer enviam somente o prompt atual ao gerador escolhido, sem passar pelo LLM de texto. Prompts e resultados de mídia têm histórico separado.
- **APIs independentes:** imagem e vídeo podem usar provedores, servidores e modelos diferentes. Qwen Image pode continuar dedicado a imagens. OpenRouter reutiliza a conexão já configurada; conexões reconhecidas também podem ser reaproveitadas.
- **Adaptadores por serviço:** OpenRouter, Google Gemini/Imagen/Veo, Grok/xAI, fal.ai e APIs locais/compatíveis. OpenAI oferece geração/edição de imagens; a API Sora de vídeo foi encerrada. Consulta de modelos sem gerar mídia. fal.ai usa o ID do endpoint; edição fal.ai com anexos ainda não está disponível.
- **Resultados no chat:** miniaturas com ampliação, reprodução de vídeo e abertura dos arquivos. Vídeos são salvos na pasta escolhida; uma pasta gravável é exigida antes do envio à API.
- **Long Horizon e fila:** melhorias de checkpoint e memória compacta, acesso correto aos arquivos da sessão e proteção contra repetições sem progresso. Botões de salvar e iniciar fila padronizados.
- **Navegador:** Playwright MCP pré-configurado, opção de desativar, escolha Chrome/Edge e modal de configuração. Perfil separado ou conexão ao perfil existente por extensão para ler páginas, preencher campos, clicar e consultar o console.
- **Modelos locais:** ajustes no ciclo de iniciar/parar e na indicação de saúde do Strata. No Windows, seu adaptador pode abrir um console visível. Correção da ordem das mensagens de sistema para modelos como Bonsai 2.
- **Leitura:** Outfit como fonte padrão, fontes anteriores disponíveis, tamanho ajustado e Markdown mais proporcional em perguntas e respostas: títulos, listas, tabelas, código e espaçamento.

**Plataformas:** Windows, macOS universal (Apple Silicon e Intel) e Linux `full` e `compat`. Linux `compat` não inclui captura/automação nativa de desktop; Playwright e APIs de mídia são independentes desse recurso. Servidores/modelos locais precisam ser instalados e configurados em cada máquina. Builds automatizados não substituem testes em hardware real; reprodução de vídeo depende dos codecs do sistema. Nenhuma geração paga foi usada nos testes dos adaptadores.

## English

- **Direct images and videos:** composer buttons send only the current prompt to the selected generator, without the text LLM. Media prompts and results have a separate history.
- **Independent APIs:** images and videos can use different providers, servers and models. Qwen Image can remain dedicated to images. OpenRouter reuses its existing connection; recognized connections can also be reused.
- **Service-specific adapters:** OpenRouter, Google Gemini/Imagen/Veo, Grok/xAI, fal.ai and local/compatible APIs. OpenAI supports image generation/editing; the Sora video API has shut down. Fetch model catalogs without generating media. fal.ai uses endpoint IDs; its adapter does not yet support editing with attachments.
- **Results in chat:** thumbnails with enlargement, video playback and file opening. Videos are saved to the chosen folder; a writable folder is required before contacting the API.
- **Long Horizon and task queue:** improved checkpoints and compact memory, correct session-file access and protection against repetition without progress. Consistent save and start-queue buttons.
- **Browser:** preconfigured Playwright MCP, optional disabling, Chrome/Edge selection and a setup dialog. Use a separate profile or connect to an existing profile through the extension to read pages, fill forms, click and inspect console messages.
- **Local models:** improved Strata start/stop lifecycle and health indicators. On Windows, its adapter can open a visible console. Fixed system-message ordering for models such as Bonsai 2.
- **Readability:** Outfit as the default font, previous fonts still available, adjusted sizes and better Markdown proportions in user and assistant messages: headings, lists, tables, code and spacing.

**Platforms:** Windows, universal macOS (Apple Silicon and Intel), and Linux `full` and `compat`. Linux `compat` excludes native desktop capture/automation; Playwright and media APIs are independent of that feature. Local servers/models must be installed and configured on each machine. Automated builds do not replace real hardware testing; video playback depends on system codecs. No paid generations were used in adapter tests.

## Español

- **Imágenes y vídeos directos:** botones del composer envían únicamente el prompt actual al generador elegido, sin pasar por el LLM de texto. Los prompts y resultados tienen un historial separado.
- **APIs independientes:** imágenes y vídeos pueden usar proveedores, servidores y modelos distintos. Qwen Image puede seguir dedicado a imágenes. OpenRouter reutiliza la conexión existente; también se pueden reutilizar conexiones reconocidas.
- **Adaptadores por servicio:** OpenRouter, Google Gemini/Imagen/Veo, Grok/xAI, fal.ai y APIs locales/compatibles. OpenAI permite generar/editar imágenes; la API de vídeo Sora fue retirada. Consulta de modelos sin generar medios. fal.ai utiliza el ID del endpoint; su adaptador todavía no admite edición con adjuntos.
- **Resultados en el chat:** miniaturas ampliables, reproducción de vídeo y apertura de archivos. Los vídeos se guardan en la carpeta elegida; se exige una carpeta con permiso de escritura antes de contactar con la API.
- **Long Horizon y cola:** mejores checkpoints y memoria compacta, acceso correcto a los archivos de sesión y protección contra repeticiones sin progreso. Botones de guardar e iniciar cola uniformes.
- **Navegador:** Playwright MCP preconfigurado, opción de desactivarlo, selección Chrome/Edge y diálogo de configuración. Perfil separado o conexión al perfil existente mediante extensión para leer páginas, rellenar campos, hacer clic y consultar la consola.
- **Modelos locales:** mejoras en inicio/parada e indicadores de salud de Strata. En Windows, su adaptador puede abrir una consola visible. Corrección del orden de los mensajes de sistema para modelos como Bonsai 2.
- **Lectura:** Outfit como fuente predeterminada, fuentes anteriores disponibles, tamaños ajustados y Markdown más proporcional en preguntas y respuestas: títulos, listas, tablas, código y espaciado.

**Plataformas:** Windows, macOS universal (Apple Silicon e Intel) y Linux `full` y `compat`. Linux `compat` no incluye captura/automatización nativa del escritorio; Playwright y las APIs de medios son independientes de esa función. Los servidores/modelos locales deben instalarse y configurarse en cada equipo. Los builds automáticos no sustituyen pruebas en hardware real; la reproducción de vídeo depende de los códecs del sistema. No se realizaron generaciones de pago durante las pruebas de los adaptadores.

## 中文

- **直接生成图片和视频：** 输入框按钮只将当前提示词发送给所选生成器，不经过文本大模型。媒体提示词和结果使用独立历史记录。
- **独立的 API 配置：** 图片和视频可使用不同的服务商、服务器和模型。Qwen Image 可以继续专门用于图片。OpenRouter 复用已有连接，也支持复用可识别的其他连接。
- **服务商专用适配器：** 支持 OpenRouter、Google Gemini/Imagen/Veo、Grok/xAI、fal.ai 及本地/兼容 API。OpenAI 支持图片生成和编辑；Sora 视频 API 已停止服务。可查询模型列表而不生成媒体。fal.ai 使用端点 ID；其适配器暂不支持通过附件编辑图片。
- **聊天中的媒体结果：** 图片缩略图可放大查看，视频可直接播放，文件可打开。视频保存到选定文件夹；调用 API 前必须选择可写入的文件夹。
- **Long Horizon 和任务队列：** 改进检查点与精简记忆，修正会话文件访问，并防止无进展的重复执行。统一保存和启动队列按钮样式。
- **浏览器：** 预配置 Playwright MCP，可禁用，支持选择 Chrome/Edge，并提供设置对话框。可使用独立配置文件，或通过扩展连接已有配置文件，以读取网页、填写表单、点击及查看控制台消息。
- **本地模型：** 改进 Strata 的启动/停止流程及健康状态提示。Windows 上其适配器可打开可见控制台。修正 Bonsai 2 等模型的系统消息顺序。
- **阅读体验：** 默认字体改为 Outfit，保留原有字体选项，调整字号，并优化用户消息和模型回复的 Markdown 比例，包括标题、列表、表格、代码和间距。

**平台：** Windows、通用 macOS（Apple Silicon 和 Intel），以及 Linux 的 `full` 和 `compat` 版本。Linux `compat` 不包含原生桌面截图/自动化；Playwright 和媒体 API 不依赖该功能。每台设备都需要单独安装和配置本地服务器/模型。自动构建不能替代真实硬件测试；视频播放依赖系统编解码器。适配器测试未进行付费生成。
