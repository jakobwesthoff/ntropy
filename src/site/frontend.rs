// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The browser-side files the binary embeds, built from `site/` by Bun and
//! committed under `src/site/dist/` (ADR 0051, ADR 0053,
//! `docs/design/site-frontend.md`), embedded by the build script.
//!
//! `app.js` is the page script. `grammars.zst` holds every Shiki grammar
//! the curated languages need, their embedded languages included, as one
//! zstd-compressed JSON array; [`grammars`] inflates it, and the export
//! writes the grammars a site uses as plain scripts, one per grammar, each
//! appending itself to the registry the page script reads.

use std::collections::BTreeMap;
use std::io::Read;
use std::sync::OnceLock;

use super::embedded::{self, Asset};

/// Every file under `src/site/dist/`.
const DIST_FILES: &[Asset] = include!(concat!(env!("OUT_DIR"), "/site_dist_files.rs"));

/// The file name of the page script within the site's `assets/`.
pub const APP_SCRIPT: &str = "app.js";
/// The file name of the search script within the site's `assets/`
/// (ADR 0057); a standalone rendered note never loads it.
pub const SEARCH_SCRIPT: &str = "search.js";

/// The directory of grammar scripts within the site's `assets/`.
pub const GRAMMARS_DIR: &str = "grammars";

fn dist(path: &str) -> &'static Asset {
    embedded::find(DIST_FILES, path).expect("the frontend build writes every embedded file")
}

/// The search script, served as `assets/search.js`. Inflated once per
/// process.
pub fn search_js() -> &'static str {
    static SEARCH_JS: OnceLock<String> = OnceLock::new();
    SEARCH_JS.get_or_init(|| dist(SEARCH_SCRIPT).text())
}

/// The page script, served as `assets/app.js`. Inflated once per process.
pub fn app_js() -> &'static str {
    static APP_JS: OnceLock<String> = OnceLock::new();
    APP_JS.get_or_init(|| dist(APP_SCRIPT).text())
}

/// One TextMate grammar as Shiki ships it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grammar {
    pub name: String,
    pub aliases: Vec<String>,
    /// The grammars this one includes by name; every one of them is in the
    /// blob too.
    pub embedded: Vec<String>,
    /// The grammar object as JSON text, what the script registers.
    pub json: String,
}

/// Why the embedded blob could not be read: it is built into the binary,
/// so a failure means the committed build output is broken.
#[derive(Debug, thiserror::Error)]
pub enum GrammarError {
    #[error("while inflating the embedded grammars: {0}")]
    Inflate(std::io::Error),
    #[error("while parsing the embedded grammars: {0}")]
    Parse(serde_json::Error),
}

/// Every grammar in the embedded blob, by name.
pub fn grammars() -> Result<Grammars, GrammarError> {
    // The blob is a zstd frame of its own; the build script stores it raw
    // since compressing it again gains nothing.
    let blob = dist("grammars.zst").contents();
    let mut decoder = ruzstd::decoding::StreamingDecoder::new(&blob[..]).map_err(|e| {
        GrammarError::Inflate(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    })?;
    let mut json = Vec::new();
    decoder
        .read_to_end(&mut json)
        .map_err(GrammarError::Inflate)?;
    let values: Vec<serde_json::Value> =
        serde_json::from_slice(&json).map_err(GrammarError::Parse)?;

    let mut by_name = BTreeMap::new();
    for value in values {
        let strings = |key: &str| -> Vec<String> {
            value
                .get(key)
                .and_then(|v| v.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default()
        };
        let name = value
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let grammar = Grammar {
            aliases: strings("aliases"),
            embedded: strings("embeddedLangs"),
            json: value.to_string(),
            name: name.clone(),
        };
        by_name.insert(name, grammar);
    }
    Ok(Grammars { by_name })
}

/// The embedded grammars, resolvable by fence language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grammars {
    by_name: BTreeMap<String, Grammar>,
}

impl Grammars {
    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }

    pub fn get(&self, name: &str) -> Option<&Grammar> {
        self.by_name.get(name)
    }

    /// The grammar a fence language names: by grammar name first, then by
    /// alias, both case-insensitively. `None` for a language without a
    /// grammar.
    pub fn resolve(&self, language: &str) -> Option<&Grammar> {
        let wanted = language.to_lowercase();
        if let Some(grammar) = self.by_name.get(&wanted) {
            return Some(grammar);
        }
        self.by_name.values().find(|grammar| {
            grammar
                .aliases
                .iter()
                .any(|alias| alias.to_lowercase() == wanted)
        })
    }

    /// The grammars `names` need: themselves and, transitively, everything
    /// they embed, each once, by name. A name outside the blob is skipped.
    pub fn closure<'a>(&'a self, names: impl IntoIterator<Item = &'a str>) -> Vec<&'a Grammar> {
        let mut wanted: Vec<&str> = names.into_iter().collect();
        let mut seen: BTreeMap<&str, &Grammar> = BTreeMap::new();
        while let Some(name) = wanted.pop() {
            if seen.contains_key(name) {
                continue;
            }
            let Some(grammar) = self.by_name.get(name) else {
                continue;
            };
            seen.insert(&grammar.name, grammar);
            wanted.extend(grammar.embedded.iter().map(String::as_str));
        }
        seen.into_values().collect()
    }
}

impl Grammar {
    /// The file name of this grammar's script within `assets/grammars/`.
    pub fn file_name(&self) -> String {
        format!("{}.js", self.name)
    }

    /// The classic script that registers this grammar for the page script.
    /// `</` is escaped inside the JSON so no grammar text can close the
    /// script element that carries it.
    pub fn script(&self) -> String {
        let json = self.json.replace("</", "<\\/");
        format!(
            "(function(){{var w=window;w.__ntropyGrammars=w.__ntropyGrammars||[];w.__ntropyGrammars.push({json});}})();\n"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_blob_holds_the_curated_closure() {
        let grammars = grammars().expect("the embedded blob inflates and parses");
        let curated: Vec<String> = serde_json::from_str(include_str!("../../site/grammars.json"))
            .expect("the list parses");
        // A curated entry names Shiki's module, which can differ from the
        // grammar's own name (`dockerfile` is the grammar `docker`), so the
        // lookup goes through aliases like a fence language does.
        for name in &curated {
            assert!(
                grammars.resolve(name).is_some(),
                "curated grammar `{name}` missing"
            );
        }
        assert!(grammars.len() >= curated.len());
        // Every embedded language is present, so a closure never dangles.
        for grammar in grammars.by_name.values() {
            for embedded in &grammar.embedded {
                assert!(
                    grammars.get(embedded).is_some(),
                    "`{}` embeds `{embedded}`, which the blob lacks",
                    grammar.name
                );
            }
        }
    }

    #[test]
    fn languages_resolve_by_name_and_alias() {
        let grammars = grammars().expect("blob");
        assert_eq!(grammars.resolve("rust").expect("rust").name, "rust");
        assert_eq!(grammars.resolve("Rust").expect("rust").name, "rust");
        assert_eq!(grammars.resolve("ts").expect("alias").name, "typescript");
        assert_eq!(grammars.resolve("sh").expect("alias").name, "shellscript");
        assert!(grammars.resolve("no-such-language").is_none());
    }

    #[test]
    fn the_closure_includes_embedded_grammars_once() {
        let grammars = grammars().expect("blob");
        let closure = grammars.closure(["typst", "rust"]);
        let names: Vec<&str> = closure.iter().map(|g| g.name.as_str()).collect();
        assert!(names.contains(&"typst"));
        assert!(names.contains(&"rust"));
        // typst embeds c among many others.
        assert!(names.contains(&"c"), "{names:?}");
        let mut deduped = names.clone();
        deduped.dedup();
        assert_eq!(names, deduped);
        assert!(grammars.closure(["no-such-language"]).is_empty());
    }

    #[test]
    fn a_grammar_script_registers_the_grammar_and_cannot_close_a_script_tag() {
        let grammar = Grammar {
            name: "x".to_string(),
            aliases: Vec::new(),
            embedded: Vec::new(),
            json: "{\"name\":\"x\",\"p\":\"</script>\"}".to_string(),
        };
        assert_eq!(grammar.file_name(), "x.js");
        let script = grammar.script();
        assert!(script.starts_with("(function(){var w=window;"), "{script}");
        assert!(
            script.contains("push({\"name\":\"x\",\"p\":\"<\\/script>\"})"),
            "{script}"
        );
        assert!(!script.contains("</script>"), "{script}");
    }

    #[test]
    fn the_page_script_is_a_classic_script() {
        assert!(
            app_js().starts_with("(function(){"),
            "app.js must be an IIFE"
        );
        assert!(!app_js().contains("import "), "app.js must not be a module");
        assert_eq!(
            app_js(),
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/site/dist/app.js"))
                .expect("dist")
        );
    }
}
