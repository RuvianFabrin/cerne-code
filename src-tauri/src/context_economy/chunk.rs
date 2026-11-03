//! Port of Context Mode's sectioning. Elastic-2.0; see bundled LICENSE.
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct Chunk {
    pub title: String,
    pub content: String,
    pub code: bool,
}

pub fn prefix(text: &str, bytes: usize) -> &str {
    let mut end = text.len().min(bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn bounded(text: &str, title: &str, code: bool) -> Vec<Chunk> {
    if text.len() <= 4096 || code {
        return vec![Chunk {
            title: title.into(),
            content: text.into(),
            code,
        }];
    }
    let mut chunks = vec![];
    let mut rest = text;
    while !rest.is_empty() {
        let mut part = prefix(rest, 4096);
        if part.len() < rest.len() {
            if let Some(boundary) = part.rfind(['\n', ' ']) {
                if boundary > part.len() / 2 {
                    part = &part[..boundary + 1];
                }
            }
        }
        chunks.push(Chunk {
            title: format!("{title} ({})", chunks.len() + 1),
            content: part.into(),
            code,
        });
        rest = &rest[part.len()..];
    }
    chunks
}

pub fn markdown(text: &str) -> Vec<Chunk> {
    let mut result = vec![];
    let mut stack: Vec<(usize, String)> = vec![];
    let mut body = String::new();
    let mut fence: Option<String> = None;
    let flush = |body: &mut String, stack: &[(usize, String)], result: &mut Vec<Chunk>| {
        if !body.trim().is_empty() {
            let title = stack
                .iter()
                .map(|(_, s)| s.as_str())
                .collect::<Vec<_>>()
                .join(" > ");
            result.extend(bounded(
                body.trim(),
                if title.is_empty() { "(root)" } else { &title },
                body.contains("```") || body.contains("~~~"),
            ));
        }
        body.clear();
    };
    for line in text.split('\n') {
        if let Some(open) = &fence {
            body.push_str(line);
            body.push('\n');
            if line.trim() == open {
                fence = None;
            }
            continue;
        }
        if line.starts_with("```") || line.starts_with("~~~") {
            let first = line.chars().next().unwrap();
            fence = Some(line.chars().take_while(|&c| c == first).collect());
        } else {
            let level = line.chars().take_while(|&c| c == '#').count();
            if (1..=4).contains(&level) && line.as_bytes().get(level) == Some(&b' ') {
                flush(&mut body, &stack, &mut result);
                while stack.last().is_some_and(|(n, _)| *n >= level) {
                    stack.pop();
                }
                stack.push((level, line[level..].trim().into()));
            } else if line.trim().len() >= 3
                && ['-', '_', '*']
                    .iter()
                    .any(|c| line.trim().chars().all(|x| x == *c))
            {
                flush(&mut body, &stack, &mut result);
                continue;
            }
        }
        body.push_str(line);
        body.push('\n');
    }
    flush(&mut body, &stack, &mut result);
    result
}

pub fn plain(text: &str) -> Vec<Chunk> {
    let paragraphs: Vec<_> = text
        .split("\n\n")
        .filter(|s| !s.trim().is_empty())
        .collect();
    if (3..=200).contains(&paragraphs.len()) && paragraphs.iter().all(|s| s.len() <= 5000) {
        return paragraphs
            .into_iter()
            .flat_map(|p| {
                bounded(
                    p,
                    &p.lines()
                        .next()
                        .unwrap_or("")
                        .chars()
                        .take(80)
                        .collect::<String>(),
                    false,
                )
            })
            .collect();
    }
    let lines: Vec<_> = text.split('\n').collect();
    lines
        .chunks(20)
        .flat_map(|lines| {
            let body = lines.join("\n");
            let title = lines
                .first()
                .unwrap_or(&"")
                .chars()
                .take(80)
                .collect::<String>();
            bounded(
                &body,
                if title.is_empty() { "Lines" } else { &title },
                false,
            )
        })
        .collect()
}

fn walk_json(value: &Value, path: &str, result: &mut Vec<Chunk>) {
    let serialized = serde_json::to_string_pretty(value).unwrap_or_default();
    match value {
        Value::Object(map)
            if map.values().any(|v| v.is_object() || v.is_array()) || serialized.len() > 4096 =>
        {
            for (key, value) in map {
                walk_json(value, &format!("{path} > {key}"), result);
            }
        }
        Value::Array(values) if serialized.len() > 4096 => {
            for (i, value) in values.iter().enumerate() {
                walk_json(value, &format!("{path}[{i}]"), result);
            }
        }
        _ => result.extend(bounded(&serialized, path, false)),
    }
}

pub fn split(text: &str) -> Vec<Chunk> {
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        if value.is_object() || value.is_array() {
            let mut result = vec![];
            walk_json(&value, "(root)", &mut result);
            return result;
        }
    }
    if text
        .lines()
        .any(|s| s.starts_with('#') || s.starts_with("```"))
    {
        markdown(text)
    } else {
        plain(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn headings_in_fences_are_not_sections() {
        let chunks =
            markdown("# API\n```rust\n# not a heading\nfn main() {}\n```\n## Usage\nexemplo");
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].code);
        assert_eq!(chunks[1].title, "API > Usage");
    }
    #[test]
    fn long_unicode_lines_are_lossless() {
        let text = "中文🙂ação".repeat(2000);
        let chunks = plain(&text);
        assert!(chunks.iter().all(|c| c.content.len() <= 4096));
        assert_eq!(
            chunks
                .iter()
                .map(|c| c.content.as_str())
                .collect::<String>(),
            text
        );
    }
    #[test]
    fn nested_json_has_searchable_paths() {
        let chunks = split(r#"{"api":{"port":42},"rules":["keep"]}"#);
        assert!(chunks.iter().any(|c| c.title.contains("api")));
    }
}
