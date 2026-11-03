//! Context Mode search: Porter + trigram, RRF, proximity and typo correction.
use super::{chunk, store::Store};
use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

const STOPWORDS: &str = "the and for are but not you all can had her was one our out has his how its may new now old see way who did get got let say she too use will with this that from they been have many some them than each make like just over such take into year your good could would about which their there other after should through also more most only very when what then these those being does done both same still while where here were much update updates updated deps dev tests test add added fix fixed run running using";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hit {
    pub title: String,
    pub content: String,
    pub source: String,
    pub kind: String,
    pub at: String,
    pub rank: f64,
    pub rowid: i64,
    pub stale: bool,
    pub layer: String,
}

pub fn terms(query: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let words: Vec<_> = query
        .split(|c: char| c.is_whitespace() || "'\"(){}[]*:^~".contains(c))
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .filter(|w| !["and", "or", "not", "near"].contains(&w.as_str()) && seen.insert(w.clone()))
        .collect();
    let meaningful: Vec<_> = words
        .iter()
        .filter(|w| !STOPWORDS.split_whitespace().any(|s| s == *w))
        .cloned()
        .collect();
    if meaningful.is_empty() {
        words
    } else {
        meaningful
    }
}

fn quoted(words: &[String], trigram: bool) -> String {
    words
        .iter()
        .filter(|w| !trigram || w.chars().count() >= 3)
        .take(32)
        .map(|w| format!("\"{}\"", w.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" OR ")
}

fn distance(a: &str, b: &str) -> usize {
    let a: Vec<_> = a.chars().collect();
    let b: Vec<_> = b.chars().collect();
    let mut prev: Vec<_> = (0..=b.len()).collect();
    for (i, c) in a.iter().enumerate() {
        let mut current = vec![i + 1];
        for (j, d) in b.iter().enumerate() {
            current.push(if c == d {
                prev[j]
            } else {
                1 + prev[j].min(prev[j + 1]).min(current[j])
            });
        }
        prev = current;
    }
    prev[b.len()]
}

impl Store {
    fn matcher(
        &self,
        table: &str,
        query: &str,
        source: Option<&str>,
        kind: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Hit>> {
        if query.is_empty() {
            return Ok(vec![]);
        }
        debug_assert!(table == "chunks" || table == "chunks_trigram");
        let sql = format!("SELECT f.rowid,f.title,f.content,s.label,f.content_type,s.at,bm25({table},5.0,1.0) FROM {table} f JOIN sources s ON s.id=f.source_id WHERE {table} MATCH ?1 AND (?2 IS NULL OR instr(s.label,?2)>0) AND (?3 IS NULL OR f.content_type=?3) ORDER BY bm25({table},5.0,1.0) LIMIT ?4");
        let mut statement = self.db.prepare(&sql)?;
        let hits = statement
            .query_map(params![query, source, kind, limit as i64], |r| {
                Ok(Hit {
                    rowid: r.get(0)?,
                    title: r.get(1)?,
                    content: r.get(2)?,
                    source: r.get(3)?,
                    kind: r.get(4)?,
                    at: r.get(5)?,
                    rank: r.get(6)?,
                    stale: false,
                    layer: "rrf".into(),
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(hits)
    }
    fn fusion(
        &self,
        words: &[String],
        source: Option<&str>,
        kind: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Hit>> {
        let porter = self.matcher("chunks", &quoted(words, false), source, kind, limit)?;
        let trigram = self.matcher("chunks_trigram", &quoted(words, true), source, kind, limit)?;
        let mut fusion: HashMap<i64, (Hit, f64)> = HashMap::new();
        for list in [porter, trigram] {
            for (i, hit) in list.into_iter().enumerate() {
                let score = 1.0 / (60.0 + i as f64 + 1.0);
                fusion
                    .entry(hit.rowid)
                    .and_modify(|(_, s)| *s += score)
                    .or_insert((hit, score));
            }
        }
        let mut hits: Vec<_> = fusion
            .into_values()
            .map(|(mut hit, score)| {
                hit.rank = -score;
                hit
            })
            .collect();
        hits.sort_by(|a, b| {
            a.rank
                .total_cmp(&b.rank)
                .then_with(|| a.rowid.cmp(&b.rowid))
        });
        hits.truncate(limit);
        Ok(hits)
    }
    pub fn search(
        &self,
        query: &str,
        source: Option<&str>,
        kind: Option<&str>,
        limit: usize,
        timeline: bool,
    ) -> Result<Vec<Hit>> {
        let mut words = terms(query);
        let mut hits = self.fusion(&words, source, kind, limit)?;
        if hits.is_empty() {
            let mut statement = self.db.prepare("SELECT word FROM vocabulary WHERE length(word) BETWEEN ? AND ? AND substr(word,1,1)=? LIMIT 5000")?;
            for word in &mut words {
                if word.chars().count() < 3 || word.len() > 120 {
                    continue;
                }
                let length = word.chars().count();
                let max = if length <= 4 {
                    1
                } else if length <= 12 {
                    2
                } else {
                    3
                };
                let candidates: Vec<String> = statement
                    .query_map(
                        params![
                            length.saturating_sub(max) as i64,
                            (length + max) as i64,
                            word.chars().next().unwrap().to_string()
                        ],
                        |r| r.get(0),
                    )?
                    .collect::<rusqlite::Result<_>>()?;
                if let Some((candidate, _)) = candidates
                    .into_iter()
                    .map(|c| {
                        let d = distance(word, &c);
                        (c, d)
                    })
                    .filter(|(_, d)| *d <= max)
                    .min_by_key(|(_, d)| *d)
                {
                    *word = candidate;
                }
            }
            hits = self.fusion(&words, source, kind, limit)?;
            for hit in &mut hits {
                hit.layer = "rrf-fuzzy".into();
            }
        }
        if timeline {
            hits.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.rowid.cmp(&b.rowid)));
        } else {
            hits.sort_by(|a, b| {
                boost(b, &words)
                    .total_cmp(&boost(a, &words))
                    .then_with(|| a.rank.total_cmp(&b.rank))
            });
        }
        for hit in &mut hits {
            // Flag changes without following an arbitrary stored path or losing old data.
            let path: Option<String> = self.db.query_row(
                "SELECT path FROM sources WHERE label=?",
                [&hit.source],
                |r| r.get(0),
            )?;
            if let Some(path) = path {
                hit.stale = std::path::Path::new(&path)
                    .canonicalize()
                    .ok()
                    .filter(|canonical| canonical == std::path::Path::new(&path))
                    .and_then(|canonical| super::execute::read_text(&canonical).ok())
                    .map(|text| {
                        let actual = super::store::hash(&text);
                        self.db
                            .query_row::<String, _, _>(
                                "SELECT hash FROM sources WHERE label=?",
                                [&hit.source],
                                |r| r.get(0),
                            )
                            .map(|saved| saved != actual)
                            .unwrap_or(true)
                    })
                    .unwrap_or(true);
            }
        }
        Ok(hits)
    }
}

fn positions(content: &str, term: &str) -> Vec<usize> {
    content
        .match_indices(term)
        .take(2000)
        .map(|(n, _)| n)
        .collect()
}
fn boost(hit: &Hit, terms: &[String]) -> f64 {
    if terms.is_empty() {
        return 0.0;
    }
    let title = hit.title.to_lowercase();
    let weight = if hit.kind == "code" { 0.6 } else { 0.3 };
    let title_boost = weight * terms.iter().filter(|t| title.contains(t.as_str())).count() as f64
        / terms.len() as f64;
    if terms.len() < 2 {
        return title_boost;
    }
    let content = hit.content.to_lowercase();
    let positions: Vec<_> = terms.iter().map(|t| positions(&content, t)).collect();
    if positions.iter().any(Vec::is_empty) {
        return title_boost;
    }
    let mut pointers = vec![0; terms.len()];
    let mut span = usize::MAX;
    loop {
        let values: Vec<_> = positions
            .iter()
            .enumerate()
            .map(|(i, p)| p[pointers[i]])
            .collect();
        let (i, min) = values.iter().enumerate().min_by_key(|(_, n)| **n).unwrap();
        span = span.min(values.iter().max().unwrap() - min);
        pointers[i] += 1;
        if pointers[i] >= positions[i].len() {
            break;
        }
    }
    let pairs: usize = positions
        .windows(2)
        .enumerate()
        .map(|(i, p)| {
            p[0].iter()
                .filter(|&&a| p[1].iter().any(|&b| b > a && b - a <= terms[i].len() + 3))
                .count()
        })
        .sum();
    title_boost
        + 1.0 / (1.0 + span as f64 / content.len().max(1) as f64)
        + 0.5 * (pairs as f64 / 4.0).min(1.0)
}

pub fn snippet(text: &str, query: &str, budget: usize) -> String {
    if text.len() <= budget {
        return text.into();
    }
    let lower = text.to_lowercase();
    let first = terms(query)
        .iter()
        .filter_map(|t| lower.find(t))
        .min()
        .unwrap_or(0);
    // Case folding may change byte offsets; boundaries are checked against original.
    let mut start = first.saturating_sub(budget / 3).min(text.len());
    while !text.is_char_boundary(start) {
        start -= 1;
    }
    let body = chunk::prefix(&text[start..], budget.saturating_sub(32));
    format!(
        "{}{}{}",
        if start > 0 { "… " } else { "" },
        body,
        if start + body.len() < text.len() {
            " … [trecho]"
        } else {
            ""
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typo_distance_unicode() {
        assert_eq!(distance("ação", "açao"), 1);
        assert_eq!(distance("compilador", "compildor"), 1);
    }
    #[test]
    fn query_operators_cannot_inject_fts() {
        assert_eq!(quoted(&terms("\"foo\" OR foo NEAR()"), false), "\"foo\"");
    }
    #[test]
    fn snippet_finds_end_of_large_log() {
        let text = format!("{}\nRARACERNE falha", "normal\n".repeat(2000));
        assert!(snippet(&text, "RARACERNE", 300).contains("RARACERNE"));
    }
}
