// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The query language's conformance corpus, shared with the site's
//! TypeScript reimplementation (ADR 0052): every case in
//! `tests/fixtures/query-corpus.json` yields the same result here and in
//! `site/src/search/corpus.test.ts`.

use std::path::PathBuf;

use ntropy::note::Note;
use ntropy::query;

#[derive(serde::Deserialize)]
struct Corpus {
    notes: Vec<CorpusNote>,
    cases: Vec<Case>,
}

#[derive(serde::Deserialize)]
struct CorpusNote {
    id: String,
    frontmatter: serde_json::Value,
    body: String,
}

#[derive(serde::Deserialize)]
struct Case {
    query: String,
    #[serde(default)]
    expect: Vec<String>,
    #[serde(default)]
    error: bool,
}

fn corpus() -> Corpus {
    serde_json::from_str(include_str!("fixtures/query-corpus.json")).expect("the corpus parses")
}

/// A note from the corpus: the JSON frontmatter serialized as YAML in front
/// of the body, parsed the way a vault note is.
fn note(entry: &CorpusNote) -> Note {
    let yaml = serde_yaml_ng::to_string(&entry.frontmatter).expect("frontmatter serializes");
    let content = format!("---\n{yaml}---\n{}", entry.body);
    Note::parse(
        PathBuf::from(format!("/vault/all-notes/{}-note.md", entry.id)),
        &content,
        None,
    )
    .expect("the corpus note parses")
}

#[test]
fn every_corpus_case_holds() {
    let corpus = corpus();
    let notes: Vec<Note> = corpus.notes.iter().map(note).collect();
    let mut failures = Vec::new();
    for case in &corpus.cases {
        match query::compile(&case.query) {
            Err(error) => {
                if !case.error {
                    failures.push(format!("{:?}: unexpected error: {error}", case.query));
                }
            }
            Ok(prepared) => {
                if case.error {
                    failures.push(format!("{:?}: expected an error", case.query));
                    continue;
                }
                let mut got: Vec<String> = notes
                    .iter()
                    .filter(|note| prepared.matches(note))
                    .map(|note| note.id.to_string())
                    .collect();
                got.sort();
                let mut want = case.expect.clone();
                want.sort();
                if got != want {
                    failures.push(format!("{:?}: got {got:?}, want {want:?}", case.query));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} failing cases:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn the_corpus_covers_every_predicate_and_operator() {
    let corpus = corpus();
    let queries: Vec<&str> = corpus.cases.iter().map(|c| c.query.as_str()).collect();
    for needle in [
        "tag:", "text:", "status:", " and ", " or ", "not ", "(", "\"",
    ] {
        assert!(
            queries.iter().any(|q| q.contains(needle)),
            "no case exercises {needle:?}"
        );
    }
    assert!(corpus.cases.iter().any(|c| c.error), "no error case");
}
