use anyhow::{anyhow, Result};
use ast_grep_core::tree_sitter::LanguageExt;
use ast_grep_core::Pattern;
use ast_grep_language::SupportLang;
use ignore::WalkBuilder;
use std::path::Path;
use std::str::FromStr;

/// Todas as 28 linguagens que `ast-grep-language` 0.44 traz prontas (o
/// crate.io real, nao o fork estendido do oh-my-pi com ~57 gramaticas
/// vendorizadas) — ver `SupportLang::all_langs()`. Antes o Cerne so
/// habilitava 12 dessas 28 a mao; agora usa `SupportLang::file_types()` (do
/// proprio crate) pra filtrar arquivo por extensao, entao cobre as 28 sem
/// precisar duplicar a lista de extensao por linguagem.
const SUPPORTED_LANGS: &str = "bash, c, cpp, csharp, css, dart, elixir, go, haskell, hcl, html, \
java, javascript, json, kotlin, lua, markdown, nix, php, python, ruby, rust, scala, solidity, \
swift, typescript, tsx, yaml";

fn parse_lang(lang: &str) -> Result<SupportLang> {
    SupportLang::from_str(lang)
        .map_err(|_| anyhow!("linguagem desconhecida: '{lang}'. Use uma de: {SUPPORTED_LANGS}"))
}

/// Structural search: same idea as `grep` but matching AST shape, not text.
/// `pattern` uses ast-grep syntax (`$VAR` for one node, `$$$ARGS` for zero
/// or more) — e.g. `console.log($$$ARGS)` matches any call regardless of
/// argument count/formatting/whitespace.
///
/// `search_root` ja vem resolvido (dentro do projeto ou de uma pasta extra
/// de leitura permitida, ver `tools::resolve_read_path`) — `project_root`
/// so e usado aqui pra tentar exibir o caminho de cada match relativo a ele
/// (cai pro caminho absoluto se `search_root` estiver fora do projeto).
pub fn search(
    search_root: &Path,
    project_root: &Path,
    pattern: &str,
    lang: &str,
) -> Result<String> {
    let sg_lang = parse_lang(lang)?;

    let mut matches = Vec::new();
    'walk: for entry in WalkBuilder::new(search_root)
        .types(sg_lang.file_types())
        .build()
    {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let Ok(bytes) = std::fs::read(entry.path()) else {
            continue;
        };
        let (content, _) = crate::encoding::decode(&bytes);

        let root = sg_lang.ast_grep(&content);
        let compiled = Pattern::new(pattern, sg_lang);
        for m in root.root().find_all(compiled) {
            let rel = entry
                .path()
                .strip_prefix(project_root)
                .unwrap_or(entry.path());
            let start_line = content[..m.range().start].matches('\n').count() + 1;
            let text = m.text();
            let one_line = text.replace('\n', "  ");
            let snippet = if one_line.chars().count() > 200 {
                format!("{}...", one_line.chars().take(200).collect::<String>())
            } else {
                one_line
            };
            matches.push(format!("{}:{}: {}", rel.display(), start_line, snippet));
            if matches.len() >= 100 {
                break 'walk;
            }
        }
    }

    if matches.is_empty() {
        Ok("nenhuma ocorrencia estrutural encontrada".to_string())
    } else {
        Ok(matches.join("\n"))
    }
}

/// Structural rewrite of a single file: every match of `pattern` is
/// replaced using `rewrite` (same `$VAR`/`$$$ARGS` names, ast-grep expands
/// them). Returns the full new file content — caller is responsible for
/// routing it through the sandbox, same as `write_file`/`edit_file`.
pub fn rewrite_file(content: &str, pattern: &str, rewrite: &str, lang: &str) -> Result<String> {
    let sg_lang = parse_lang(lang)?;
    let mut root = sg_lang.ast_grep(content);

    // `Root::replace` only rewrites the first match per call, not every
    // occurrence — loop until none are left. Capped so a rewrite template
    // that (accidentally or not) still matches its own pattern can't spin
    // forever instead of erroring.
    let mut total = 0;
    loop {
        let changed = root
            .replace(pattern, rewrite)
            .map_err(|e| anyhow!("padrao ast invalido ou erro na reescrita: {e}"))?;
        if !changed {
            break;
        }
        total += 1;
        if total >= 1000 {
            return Err(anyhow!(
                "reescrita nao convergiu depois de 1000 substituicoes — o template de reescrita provavelmente ainda casa com o proprio padrao"
            ));
        }
    }

    if total == 0 {
        return Err(anyhow!(
            "nenhuma ocorrencia do padrao encontrada no arquivo"
        ));
    }
    Ok(root.generate())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn scratch_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("cerne-ast-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn rewrite_file_replaces_matching_calls() {
        let src = "fn main() {\n    println!(\"hello {}\", name);\n    println!(\"bye\");\n}\n";
        let out = rewrite_file(src, "println!($$$ARGS)", "log::info!($$$ARGS)", "rust").unwrap();
        assert!(out.contains("log::info!(\"hello {}\", name)"));
        assert!(out.contains("log::info!(\"bye\")"));
        assert!(!out.contains("println!"));
    }

    #[test]
    fn rewrite_file_errors_when_pattern_not_found() {
        let src = "fn main() {}\n";
        let err = rewrite_file(src, "println!($$$ARGS)", "x", "rust").unwrap_err();
        assert!(err.to_string().contains("nenhuma ocorrencia"));
    }

    #[test]
    fn rewrite_file_errors_on_unknown_language() {
        let err = rewrite_file("x", "y", "z", "not-a-real-lang").unwrap_err();
        assert!(err.to_string().contains("linguagem desconhecida"));
    }

    #[test]
    fn search_finds_structural_matches_ignoring_formatting() {
        let dir = scratch_dir();
        fs::write(
            dir.join("a.ts"),
            "console.log('one');\nconsole.log(\n  'two',\n  'three'\n);\n// console.log('commented out') not real code here\nconsole.error('nope');\n",
        )
        .unwrap();

        let out = search(&dir, &dir, "console.log($$$ARGS)", "typescript").unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2, "expected 2 matches, got: {out}");
        assert!(lines[0].contains("a.ts:1"));
        assert!(lines[1].contains("a.ts:2")); // multi-line call starts on line 2

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn search_respects_subpath_and_extension_filter() {
        let dir = scratch_dir();
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("root.py"), "print('at root')\n").unwrap();
        fs::write(dir.join("sub/nested.py"), "print('nested')\n").unwrap();
        fs::write(dir.join("sub/nested.txt"), "print('not python, ignored')\n").unwrap();

        let out = search(&dir.join("sub"), &dir, "print($$$ARGS)", "python").unwrap();
        assert!(out.contains("nested.py"));
        assert!(!out.contains("root.py"));
        assert!(!out.contains("nested.txt"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn search_supports_languages_beyond_the_original_twelve() {
        // C e C# nao estavam habilitados antes de usar SupportLang::file_types()
        // direto do crate - confirma que as 28 linguagens do ast-grep-language
        // 0.44 funcionam, nao so as 12 que o Cerne mapeava a mao. Padrao leva
        // ";" no final porque C/C# (diferente de JS/Python) exigem o statement
        // completo pra casar uma chamada de funcao solta - particularidade do
        // ast-grep pra linguagem baseada em statement, nao bug do Cerne.
        let dir = scratch_dir();
        fs::write(
            dir.join("a.c"),
            "int main() {\n  printf(\"hi\");\n  return 0;\n}\n",
        )
        .unwrap();
        let out = search(&dir, &dir, "printf($$$ARGS);", "c").unwrap();
        assert!(
            out.contains("a.c:2"),
            "esperava achar printf em C, recebeu: {out}"
        );

        fs::write(
            dir.join("b.cs"),
            "class P { void M() { Console.WriteLine(\"hi\"); } }\n",
        )
        .unwrap();
        let out = search(&dir, &dir, "Console.WriteLine($$$ARGS);", "csharp").unwrap();
        assert!(
            out.contains("b.cs:1"),
            "esperava achar Console.WriteLine em C#, recebeu: {out}"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn search_reports_no_matches_cleanly() {
        let dir = scratch_dir();
        fs::write(dir.join("a.rs"), "fn main() {}\n").unwrap();
        let out = search(&dir, &dir, "println!($$$ARGS)", "rust").unwrap();
        assert_eq!(out, "nenhuma ocorrencia estrutural encontrada");
        fs::remove_dir_all(&dir).ok();
    }
}
