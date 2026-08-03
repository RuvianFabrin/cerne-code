# Qué sabe hacer Cerne Code

Este es el catálogo de herramientas que el agente tiene disponibles. En cada turno, el modelo decide por sí solo qué herramienta llamar (o ninguna) según el pedido — las columnas "Cuándo el agente decide usarla" de abajo vienen directo de la descripción que cada herramienta recibe del propio código, así que reflejan exactamente lo que el modelo lee antes de elegir.

## Siempre disponibles (con o sin carpeta de proyecto)

| Herramienta | Qué hace | Cuándo el agente decide usarla |
| --- | --- | --- |
| `web_search` | Busca en la web y devuelve título/URL/fragmento de los resultados más relevantes. Por defecto agrega DuckDuckGo + Brave + Mojeek en paralelo, sin requerir cuenta ni instalación, eliminando duplicados y ordenando por consenso entre las fuentes (configurable en Configuración → Búsqueda web). Acepta una o más consultas por llamada. | Cuando falta información que no está en el proyecto ni en el entrenamiento del modelo — versiones recientes de una librería, documentación externa, noticias. El agente decide por sí solo cuántas consultas enviar en una llamada: una para pedidos directos, varias (frases distintas, sinónimos) cuando el pedido tiene múltiples ángulos o la primera búsqueda no trajo suficiente. |
| `web_fetch` | Busca una URL específica y devuelve el texto visible de la página, sin HTML/scripts. | Después de un `web_search`, para leer una fuente completa en vez de confiar solo en el fragmento del resultado. |
| `load_skill` | Carga el contenido completo de una skill por su nombre exacto, desde el catálogo listado al inicio de la conversación. | Cuando el pedido del usuario coincide con la descripción de una skill registrada (ver la sección Skills más abajo). |
| `ask` | Pausa el turno y le pregunta algo específico al usuario, con opciones de selección múltiple y/o texto libre. | Cuando solo el usuario puede tomar una decisión — elegir entre enfoques, confirmar una acción riesgosa, desambiguar un pedido — en vez de asumir y continuar. Se usa con moderación, solo cuando el agente realmente se quedaría trabado sin esa respuesta. |

## Solo con una carpeta de proyecto abierta

| Herramienta | Qué hace | Cuándo el agente decide usarla |
| --- | --- | --- |
| `read_file` | Lee el contenido de un archivo del proyecto (o de una carpeta extra de lectura habilitada para la sesión). | Siempre que necesita ver el contenido real de un archivo antes de explicar, editar o usarlo como referencia. |
| `list_dir` | Lista archivos y subcarpetas de un directorio. | Para entender la estructura del proyecto antes de tocar algo, o encontrar dónde está un archivo. |
| `grep` | Busca un patrón (regex) en el contenido de los archivos. | Encontrar dónde aparece un texto, símbolo o cadena en el proyecto. |
| `ast_grep` | Búsqueda estructural de código (por la forma del AST, no texto suelto) — `$VAR` coincide con cualquier nodo, `$$$ARGS` coincide con cero o más. | Preferida sobre `grep` cuando la búsqueda es sobre estructura de código (llamada de función, import, declaración) en vez de texto suelto. |
| `run_command` | Ejecuta un comando de shell en el directorio del proyecto. Con `background=true`, no espera a que el comando termine (para dev server, watch mode). | Ejecutar tests, build, lint, scripts del proyecto; `background=true` específicamente para procesos que quedan corriendo a propósito. |
| `check_background_output` | Lee la salida acumulada y el estado de un comando iniciado en segundo plano, sin detenerlo. | Revisar el progreso de un dev server o proceso largo ya iniciado con `run_command(background=true)`. |
| `stop_background` | Detiene un comando en segundo plano (mata el proceso). | Después de confirmar que algo arrancó bien, o antes de levantar una versión nueva en lugar de la anterior. |
| `list_background` | Lista todo comando en segundo plano conocido, corriendo o ya finalizado. | Antes de iniciar un nuevo dev server, para verificar que no haya uno ya corriendo de una sesión anterior. |
| `write_file` | Crea o sobrescribe un archivo. La escritura va a una carpeta sandbox espejada — el usuario debe aceptar el diff en la interfaz antes de aplicarlo al archivo real. | Crear un archivo nuevo o reemplazar el contenido completo de uno ya existente. |
| `edit_file` | Edita un archivo existente reemplazando una ocurrencia exacta de un fragmento por otro. También escribe en la sandbox, sujeto a aceptación. | Cambios puntuales y localizados en un archivo ya existente. |
| `ast_edit` | Reescritura estructural: toda ocurrencia del patrón (misma sintaxis que `ast_grep`) se reemplaza por la plantilla de reescritura. | Refactors — renombrar una llamada, cambiar un import — con más seguridad que `edit_file` porque opera sobre la estructura, no sobre texto exacto. |
| `task` | Delega una subtarea bien definida a un sub-agente descartable, que ejecuta su propio ciclo de herramientas y devuelve solo el informe final. | Subtareas que requieren varias llamadas a herramientas cuyo proceso intermedio no le importa al usuario, solo el resultado (ej: "encuentra todos los usos de X y resume dónde están"). |
| `verify_completion` | Dispara un verificador independiente y escéptico (no el propio agente) para reconfirmar con evidencia real si una tarea realmente se completó. Solo tiene herramientas de lectura/ejecución, nunca de edición. | Antes de declarar éxito en una tarea compleja (varios archivos, algo construido desde cero) — no se usa en pedidos simples de una sola llamada, donde el resultado ya es obviamente verificable. |

## Herramientas MCP (servidores externos)

Cada servidor configurado en Configuración → Servidores MCP se agrega automáticamente al catálogo del agente como `mcp__{servidor}__{herramienta}`. El agente decide usarlas igual que las herramientas nativas — según la descripción que el propio servidor MCP expone. No aparecen en una tabla fija aquí porque varían de instalación a instalación, según qué servidores hayas configurado.

## Skills

Una skill es un archivo `SKILL.md` con instrucciones que el agente carga bajo demanda vía `load_skill`, en vez de tener que reexplicar el mismo proceso en cada conversación. Al inicio de cada sesión, el agente recibe solo el catálogo (nombre + descripción) de cada skill disponible — el cuerpo completo solo se lee si el agente decide llamar a `load_skill(nombre)`. Crea y edita skills en Configuración → Skills.

## Modo Manual vs Automático

Cada sesión tiene un modo de ejecución, elegido en el selector junto al botón "+" del composer:

- **Automático** (por defecto): toda herramienta se ejecuta directo, sin pausa. Un botón "Cancelar" en la lista de tareas lateral interrumpe todo el turno en cualquier momento.
- **Manual**: toda llamada a herramienta (excepto `ask`, que ya es una pausa) detiene el turno y pide aprobación explícita antes de ejecutarse — útil cuando querés revisar cada acción antes de que ocurra, en vez de solo poder cancelar después.
