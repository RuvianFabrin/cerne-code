# What Cerne Code can do

This is the catalog of tools the agent has available. On every turn, the model decides on its own which tool to call (or none) based on the request — the "When the agent decides to use it" columns below come straight from the description each tool receives from the code itself, so they reflect exactly what the model reads before choosing.

## Always available (with or without a project folder)

| Tool | What it does | When the agent decides to use it |
| --- | --- | --- |
| `web_search` | Searches the web and returns title/URL/snippet of the most relevant results. By default it aggregates DuckDuckGo + Brave + Mojeek in parallel, with no account or install required, removing duplicates and ranking by consensus across sources (configurable in Settings → Web search). Accepts one or more queries per call. | When information is missing that isn't in the project or the model's training — recent versions of a library, external docs, news. The agent decides on its own how many queries to send in one call: one for direct requests, several (different phrasings, synonyms) when the request has multiple angles or the first search didn't bring back enough. |
| `web_fetch` | Fetches a specific URL and returns the page's visible text, without HTML/scripts. | After a `web_search`, to read a whole source instead of relying only on the result snippet. |
| `load_skill` | Loads the full content of a skill by its exact name, from the catalog listed at the start of the conversation. | When the user's request matches the description of a registered skill (see the Skills section below). |
| `ask` | Pauses the turn and asks the user something specific, with multiple-choice options and/or free text. | When only the user can make a decision — choosing between approaches, confirming a risky action, disambiguating a request — instead of assuming and moving on. Used sparingly, only when the agent would genuinely be stuck without that answer. |

## Only with a project folder open

| Tool | What it does | When the agent decides to use it |
| --- | --- | --- |
| `read_file` | Reads the content of a project file (or an extra read folder granted to the session). | Whenever it needs to see a file's real content before explaining, editing, or using it as reference. |
| `list_dir` | Lists files and subfolders of a directory. | To understand the project structure before touching something, or to find where a file is. |
| `grep` | Searches for a pattern (regex) in file contents. | To find where a text, symbol, or string appears in the project. |
| `ast_grep` | Structural code search (by AST shape, not loose text) — `$VAR` matches any node, `$$$ARGS` matches zero-or-more. | Preferred over `grep` when the search is about code structure (function call, import, declaration) rather than loose text. |
| `run_command` | Runs a shell command in the project directory. With `background=true`, it doesn't wait for the command to finish (for dev servers, watch mode). | Running tests, build, lint, project scripts; `background=true` specifically for processes meant to keep running. |
| `check_background_output` | Reads the accumulated output and status of a command started in the background, without stopping it. | Checking the progress of a dev server or long-running process already started with `run_command(background=true)`. |
| `stop_background` | Stops a background command (kills the process). | After confirming something started correctly, or before starting a new version in place of the old one. |
| `list_background` | Lists every known background command, running or already finished. | Before starting a new dev server, to check whether one from a previous session isn't already running. |
| `write_file` | Creates or overwrites a file. The write goes to a mirrored sandbox folder — the user needs to accept the diff in the interface before it's applied to the real file. | Creating a new file or replacing the entire content of an existing one. |
| `edit_file` | Edits an existing file by replacing one exact occurrence of a snippet with another. Also writes to the sandbox, subject to acceptance. | Small, localized changes to an already existing file. |
| `ast_edit` | Structural rewrite: every occurrence of the pattern (same syntax as `ast_grep`) is replaced by the rewrite template. | Refactors — renaming a call, changing an import — more safely than `edit_file` because it operates on structure, not exact text. |
| `task` | Delegates a well-defined subtask to a disposable sub-agent, which runs its own tool loop and returns only the final report. | Subtasks that require several tool calls whose intermediate process doesn't matter to the user, only the result (e.g. "find every use of X and summarize where they are"). |
| `verify_completion` | Triggers an independent, skeptical verifier (not the agent itself) to double-check with real evidence whether a task was actually completed. It only has read/execution tools, never edit ones. | Before declaring success on a complex task (several files, something built from scratch) — not used for simple single-call requests, where the result is already obviously verifiable. |

## MCP tools (external servers)

Every server configured in Settings → MCP Servers is automatically added to the agent's catalog as `mcp__{server}__{tool}`. The agent decides to use them the same way as native tools — based on the description the MCP server itself exposes. They're not listed in a fixed table here because they vary from install to install, depending on which servers you've configured.

## Skills

A skill is a `SKILL.md` file with instructions that the agent loads on demand via `load_skill`, instead of needing to re-explain the same process in every conversation. At the start of each session, the agent only receives the catalog (name + description) of each available skill — the full body is only read if the agent decides to call `load_skill(name)`. Create and edit skills in Settings → Skills.

## Manual vs Automatic mode

Every session has an execution mode, chosen in the selector next to the "+" button in the composer:

- **Automatic** (default): every tool runs immediately, without pausing. A "Cancel" button in the side task list interrupts the whole turn at any time.
- **Manual**: every tool call (except `ask`, which is already a pause) stops the turn and asks for explicit approval before running — useful when you want to review each action before it happens, instead of only being able to cancel afterward.
