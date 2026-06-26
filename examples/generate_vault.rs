// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Deterministic benchmark-corpus generator.
//!
//! This example fabricates a vault full of realistic notes so the benchmark
//! harness (`scripts/benchmark.sh`) has a representative, *reproducible* corpus
//! to measure against. It is intentionally not part of the shipped binary: it
//! exists only to feed benchmarks and ad-hoc profiling.
//!
//! Two properties matter for a trustworthy benchmark and drive the design:
//!
//! 1. **Reproducibility.** The same `--seed` and `--notes` must yield a
//!    byte-identical corpus on any machine and at any time, so numbers compare
//!    across runs and across ntropy versions. We therefore avoid the `rand`
//!    crate (whose stream is not guaranteed stable across versions) in favour
//!    of a tiny inlined SplitMix64 generator, and we anchor all timestamps to a
//!    fixed epoch constant rather than the wall clock.
//! 2. **Canonical validity.** Every file must be a note ntropy actually
//!    accepts, so we reuse the real [`ntropy::text::slug::slugify`] for the
//!    filename slug and the already-present `ulid` crate for identities. A
//!    corpus full of files ntropy silently skips would measure nothing.
//!
//! The generator writes notes directly into `all-notes/` (far faster than
//! thousands of `ntropy new` invocations, each of which would rebuild views)
//! and prints a JSON *manifest* describing the corpus: a sample id, a few
//! representative queries, and the exact hit counts for a common and a rare
//! search term. The harness consumes the manifest so it never has to hardcode
//! assumptions about the generated content.

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ntropy::text::slug;
use serde_json::json;
use ulid::Ulid;

// =========================================================
// Deterministic randomness: SplitMix64
// =========================================================

// We need a stream of pseudo-random numbers that is identical forever, so the
// generated corpus is reproducible across machines and crate-version bumps.
// SplitMix64 is a single-`u64`-state generator that is trivial to implement,
// has good statistical quality for this purpose, and (unlike `rand`'s adapters)
// has a fixed, well-known algorithm we control here in full.
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A uniformly distributed value in `0..bound` (bound must be non-zero).
    fn below(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }

    /// Pick a random element from a non-empty slice.
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }

    /// Return `true` with probability `numerator/denominator`.
    fn chance(&mut self, numerator: u32, denominator: u32) -> bool {
        (self.next_u64() % denominator as u64) < numerator as u64
    }
}

// =========================================================
// Content vocabulary
// =========================================================

// A small, hand-picked vocabulary that produces note-shaped prose. The goal is
// not literary quality but realistic *shape and size*: word lengths, sentence
// rhythm, and frontmatter variety that exercise the scan, YAML parse, and regex
// body search the way a real vault would.

const ADJECTIVES: &[&str] = &[
    "quarterly",
    "initial",
    "draft",
    "final",
    "rough",
    "detailed",
    "quick",
    "annual",
    "weekly",
    "internal",
    "shared",
    "personal",
    "technical",
    "strategic",
    "tactical",
    "experimental",
    "archived",
    "pending",
    "urgent",
    "minor",
];

const NOUNS: &[&str] = &[
    "review",
    "meeting",
    "plan",
    "retrospective",
    "design",
    "proposal",
    "report",
    "summary",
    "outline",
    "checklist",
    "spike",
    "investigation",
    "decision",
    "roadmap",
    "postmortem",
    "experiment",
    "interview",
    "onboarding",
    "budget",
    "audit",
];

const TOPICS: &[&str] = &[
    "authentication",
    "billing",
    "caching",
    "deployment",
    "indexing",
    "migration",
    "observability",
    "scheduling",
    "search",
    "storage",
    "tooling",
    "testing",
    "rendering",
    "parsing",
    "networking",
    "scaling",
    "security",
    "logging",
    "backup",
    "frontend",
];

const VERBS: &[&str] = &[
    "investigate",
    "implement",
    "measure",
    "document",
    "refactor",
    "review",
    "ship",
    "validate",
    "benchmark",
    "profile",
    "design",
    "evaluate",
    "monitor",
    "deprecate",
    "migrate",
    "simplify",
    "harden",
    "instrument",
];

const SENTENCE_WORDS: &[&str] = &[
    "the",
    "system",
    "approach",
    "currently",
    "handles",
    "every",
    "request",
    "through",
    "a",
    "single",
    "pass",
    "over",
    "the",
    "data",
    "which",
    "keeps",
    "the",
    "model",
    "simple",
    "and",
    "predictable",
    "for",
    "the",
    "common",
    "case",
    "we",
    "should",
    "consider",
    "whether",
    "the",
    "added",
    "complexity",
    "pays",
    "for",
    "itself",
    "before",
    "committing",
    "to",
    "it",
    "in",
    "production",
    "the",
    "numbers",
    "from",
    "the",
    "last",
    "run",
    "suggest",
    "there",
    "is",
    "headroom",
    "but",
    "the",
    "tail",
    "latency",
    "remains",
    "a",
    "concern",
    "worth",
    "tracking",
    "across",
    "releases",
    "over",
    "time",
];

// Tags follow the slash-hierarchy convention (ADR 0006). A spread of depths and
// shared prefixes lets the harness exercise both shallow (`tag:programming`)
// and deep (`tag:programming/rust`) segment matches.
const TAGS: &[&str] = &[
    "programming/rust",
    "programming/python",
    "programming/cli",
    "programming/web",
    "area/work",
    "area/home",
    "area/health",
    "area/finance",
    "project/alpha",
    "project/beta",
    "project/gamma",
    "meta/reference",
    "meta/idea",
    "meta/journal",
    "reading/article",
    "reading/book",
];

const STATUSES: &[&str] = &["todo", "in-progress", "done", "blocked", "archived"];
const PRIORITIES: &[&str] = &["low", "medium", "high"];

// The "codename" generator: the familiar `adjective-adjective-animal` scheme
// (think `invisible-pompous-camel`) used as memorable release/project names. It
// gives every note a high-cardinality `codename` field, which is realistic
// frontmatter and a useful extra axis for `field:value` lookups.
const CODENAME_ADJECTIVES: &[&str] = &[
    "invisible",
    "pompous",
    "sleepy",
    "brave",
    "curious",
    "grumpy",
    "nimble",
    "stoic",
    "vivid",
    "wary",
    "jolly",
    "feral",
    "gentle",
    "rowdy",
    "somber",
    "zealous",
    "quaint",
    "lucid",
    "rustic",
    "fierce",
    "mellow",
    "plucky",
    "snug",
    "witty",
];

const ANIMALS: &[&str] = &[
    "camel", "otter", "lynx", "heron", "badger", "marmot", "ferret", "gecko", "ibis", "tapir",
    "narwhal", "quokka", "puffin", "mantis", "raven", "stoat", "walrus", "yak", "panther", "newt",
];

// The two sentinel terms the harness searches for. `COMMON_TERM` is injected
// into most bodies (a near-worst-case for a regex match that scans almost
// everything), `RARE_TERM` into a tiny fraction (the early-out, few-hits case).
// Both are deliberately outside the natural vocabulary so their hit counts are
// exact and controllable.
const COMMON_TERM: &str = "roadmap";
const RARE_TERM: &str = "xyzzy";

// A fixed timestamp anchor (2026-06-01T00:00:00Z in milliseconds). All note
// identities are stamped at or before this instant so `created` dates are
// realistic, spread across the prior year, and identical on every run.
const ANCHOR_MS: u64 = 1_780_272_000_000;
const YEAR_MS: u64 = 365 * 24 * 60 * 60 * 1000;

// =========================================================
// Note fabrication
// =========================================================

/// Everything we need to remember about a generated note: enough to write the
/// file, link to it from later notes, and tally manifest statistics.
struct GeneratedNote {
    ulid: Ulid,
    slug: String,
    title: String,
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Build an `adjective-adjective-animal` codename like
/// `invisible-pompous-camel`. The two adjectives may repeat; that is fine for a
/// codename and keeps the draw cheap.
fn make_codename(rng: &mut SplitMix64) -> String {
    format!(
        "{}-{}-{}",
        rng.pick(CODENAME_ADJECTIVES),
        rng.pick(CODENAME_ADJECTIVES),
        rng.pick(ANIMALS)
    )
}

/// Build a human-looking title like "Quarterly review of billing".
fn make_title(rng: &mut SplitMix64) -> String {
    let adjective = rng.pick(ADJECTIVES);
    let noun = rng.pick(NOUNS);
    let topic = rng.pick(TOPICS);
    format!("{} {} of {}", capitalize(adjective), noun, topic)
}

/// Assemble one sentence of `len` words from the filler vocabulary, capitalized
/// and terminated, optionally splicing a sentinel term in at a word boundary.
fn make_sentence(rng: &mut SplitMix64, len: usize, inject: Option<&str>) -> String {
    let mut words: Vec<String> = (0..len)
        .map(|_| rng.pick(SENTENCE_WORDS).to_string())
        .collect();
    if let Some(term) = inject {
        let at = rng.below(words.len());
        words[at] = term.to_string();
    }
    let mut sentence = words.join(" ");
    sentence = capitalize(&sentence);
    sentence.push('.');
    sentence
}

/// Render the Markdown body for a note. Returns the body text and whether it
/// contains each sentinel term, so the caller can tally exact manifest counts.
fn make_body(
    rng: &mut SplitMix64,
    title: &str,
    earlier: &[GeneratedNote],
    inject_common: bool,
    inject_rare: bool,
) -> String {
    let mut body = String::new();
    let _ = writeln!(body, "# {}\n", title);

    // An opening action line gives the note a verb-led intent, the way a real
    // task or research note usually starts.
    let _ = writeln!(
        body,
        "We need to {} the {} path and capture the findings here.\n",
        rng.pick(VERBS),
        rng.pick(TOPICS)
    );

    // Two to five paragraphs of filler prose. The sentinel terms are spliced
    // into the first paragraph when requested so they always land in the body.
    let paragraphs = 2 + rng.below(4);
    for index in 0..paragraphs {
        let sentences = 3 + rng.below(4);
        let mut paragraph = String::new();
        for sentence_index in 0..sentences {
            let inject = if index == 0 && sentence_index == 0 {
                match (inject_common, inject_rare) {
                    (_, true) => Some(RARE_TERM),
                    (true, false) => Some(COMMON_TERM),
                    _ => None,
                }
            } else {
                None
            };
            if !paragraph.is_empty() {
                paragraph.push(' ');
            }
            let length = 8 + rng.below(10);
            paragraph.push_str(&make_sentence(rng, length, inject));
        }
        let _ = writeln!(body, "{}\n", paragraph);
    }

    // Roughly a third of notes carry a checklist, which adds list syntax and
    // shorter lines to the corpus.
    if rng.chance(1, 3) {
        let _ = writeln!(body, "## Action items\n");
        for _ in 0..(2 + rng.below(4)) {
            let _ = writeln!(body, "- {} the {} layer", rng.pick(VERBS), rng.pick(TOPICS));
        }
        body.push('\n');
    }

    // Occasionally a fenced code block, to make some bodies markedly larger and
    // to include lines a naive search must still scan past.
    if rng.chance(1, 5) {
        let _ = writeln!(body, "```rust");
        let _ = writeln!(body, "fn {}() {{", rng.pick(VERBS));
        let _ = writeln!(
            body,
            "    // {} the {} before returning",
            rng.pick(VERBS),
            rng.pick(TOPICS)
        );
        let _ = writeln!(body, "    todo!()");
        let _ = writeln!(body, "}}");
        let _ = writeln!(body, "```\n");
    }

    // Link to an earlier note as a standard Markdown link (ADR 0028) so the
    // corpus contains real inter-note references.
    if !earlier.is_empty() && rng.chance(1, 2) {
        let target = rng.pick(earlier);
        let _ = writeln!(
            body,
            "See also [{}]({}-{}.md).",
            target.title, target.ulid, target.slug
        );
    }

    body
}

// =========================================================
// Generation driver
// =========================================================

fn generate(vault: &Path, count: usize, seed: u64) -> std::io::Result<serde_json::Value> {
    let all_notes = vault.join("all-notes");
    fs::create_dir_all(&all_notes)?;

    let mut rng = SplitMix64::new(seed);
    let mut generated: Vec<GeneratedNote> = Vec::with_capacity(count);

    // Manifest tallies: exact hit counts make the harness's search benchmarks
    // self-describing rather than guesswork.
    let mut common_hits = 0usize;
    let mut rare_hits = 0usize;
    let mut tag_shallow_hits = 0usize;
    let mut tag_deep_hits = 0usize;
    let mut field_hits = 0usize;

    const TAG_SHALLOW: &str = "programming";
    const TAG_DEEP: &str = "programming/rust";
    const FIELD_NAME: &str = "status";
    const FIELD_VALUE: &str = "done";

    for index in 0..count {
        // Spread identities across the year preceding the anchor. Distinct
        // random components keep ids unique even when timestamps collide.
        let offset_ms = (rng.next_u64() % YEAR_MS).min(ANCHOR_MS);
        let ms = ANCHOR_MS - offset_ms;
        let random = rng.next_u64() as u128 | ((rng.next_u64() as u128) << 64);
        let ulid = Ulid::from_parts(ms, random);

        let title = make_title(&mut rng);
        let note_slug = slug::slugify(&title);

        // Tags: zero to four, drawn without worrying about duplicates beyond a
        // cheap check. A meaningful fraction of notes stay untagged to exercise
        // the "no value, skipped from views" path.
        let tag_count = match rng.below(10) {
            0..=1 => 0,
            2..=4 => 1,
            5..=6 => 2,
            7..=8 => 3,
            _ => 4,
        };
        let mut tags: Vec<String> = Vec::with_capacity(tag_count);
        for _ in 0..tag_count {
            let candidate = rng.pick(TAGS).to_string();
            if !tags.contains(&candidate) {
                tags.push(candidate);
            }
        }
        if tags.iter().any(|t| t.starts_with(TAG_SHALLOW)) {
            tag_shallow_hits += 1;
        }
        if tags.iter().any(|t| t == TAG_DEEP) {
            tag_deep_hits += 1;
        }

        // Most notes carry a status; a subset also a priority. Leaving some
        // notes without these fields exercises the absent-field branch of
        // `field:value` evaluation.
        let mut frontmatter = String::new();
        frontmatter.push_str("---\n");
        let _ = writeln!(frontmatter, "title: {}", yaml_scalar(&title));
        if tags.is_empty() {
            frontmatter.push_str("tags: []\n");
        } else {
            let rendered: Vec<String> = tags.iter().map(|t| t.to_string()).collect();
            let _ = writeln!(frontmatter, "tags: [{}]", rendered.join(", "));
        }
        if rng.chance(4, 5) {
            let status = *rng.pick(STATUSES);
            let _ = writeln!(frontmatter, "{}: {}", FIELD_NAME, status);
            if status == FIELD_VALUE {
                field_hits += 1;
            }
        }
        if rng.chance(1, 2) {
            let _ = writeln!(frontmatter, "priority: {}", rng.pick(PRIORITIES));
        }
        let _ = writeln!(frontmatter, "codename: {}", make_codename(&mut rng));
        frontmatter.push_str("---\n");

        // Inject the common term into most bodies and the rare term into a thin
        // slice, then record the actual outcome for the manifest.
        let inject_common = rng.chance(7, 10);
        let inject_rare = rng.chance(1, 80);
        if inject_common && !inject_rare {
            common_hits += 1;
        }
        if inject_rare {
            rare_hits += 1;
        }

        let body = make_body(&mut rng, &title, &generated, inject_common, inject_rare);

        let filename = format!("{}-{}.md", ulid, note_slug);
        fs::write(
            all_notes.join(&filename),
            format!("{}{}", frontmatter, body),
        )?;

        generated.push(GeneratedNote {
            ulid,
            slug: note_slug,
            title,
        });

        if (index + 1) % 1000 == 0 {
            eprintln!("  generated {} / {} notes", index + 1, count);
        }
    }

    // A sample id from the middle of the corpus for the `edit`/`delete`
    // single-note benchmarks. The middle avoids any accidental edge effects at
    // the chronological extremes.
    let sample = &generated[generated.len() / 2];

    Ok(json!({
        "vault": vault.display().to_string(),
        "notes": count,
        "seed": seed,
        "sample_id": sample.ulid.to_string(),
        "sample_slug": sample.slug,
        "tag_shallow": TAG_SHALLOW,
        "tag_shallow_hits": tag_shallow_hits,
        "tag_deep": TAG_DEEP,
        "tag_deep_hits": tag_deep_hits,
        "field_query": format!("{}:{}", FIELD_NAME, FIELD_VALUE),
        "field_hits": field_hits,
        "text_common": COMMON_TERM,
        "text_common_hits": common_hits,
        "text_rare": RARE_TERM,
        "text_rare_hits": rare_hits,
    }))
}

/// Quote a YAML scalar only when it needs it. Our titles never contain YAML
/// metacharacters, but quoting defensively keeps the frontmatter valid even if
/// the vocabulary grows.
fn yaml_scalar(value: &str) -> String {
    if value.contains([':', '#', '\'', '"', '[', ']', '{', '}']) {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

// =========================================================
// Argument handling
// =========================================================

struct Args {
    vault: PathBuf,
    notes: usize,
    seed: u64,
    manifest: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut vault: Option<PathBuf> = None;
    let mut notes: usize = 3000;
    let mut seed: u64 = 0x1234_5678;
    let mut manifest: Option<PathBuf> = None;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--vault" => {
                vault = Some(PathBuf::from(
                    args.next().ok_or("--vault requires a value")?,
                ))
            }
            "--notes" => {
                notes = args
                    .next()
                    .ok_or("--notes requires a value")?
                    .parse()
                    .map_err(|_| "--notes must be a positive integer")?
            }
            "--seed" => {
                seed = args
                    .next()
                    .ok_or("--seed requires a value")?
                    .parse()
                    .map_err(|_| "--seed must be an unsigned integer")?
            }
            "--manifest" => {
                manifest = Some(PathBuf::from(
                    args.next().ok_or("--manifest requires a value")?,
                ))
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(Args {
        vault: vault.ok_or("--vault is required")?,
        notes,
        seed,
        manifest,
    })
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("error: {message}");
            eprintln!(
                "usage: generate_vault --vault <PATH> [--notes <N>] [--seed <N>] [--manifest <PATH>]"
            );
            return ExitCode::FAILURE;
        }
    };

    eprintln!(
        "generating {} notes into {} (seed {})",
        args.notes,
        args.vault.display(),
        args.seed
    );

    let manifest = match generate(&args.vault, args.notes, args.seed) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!("error: failed to generate vault: {error}");
            return ExitCode::FAILURE;
        }
    };

    let rendered = serde_json::to_string_pretty(&manifest).expect("manifest serializes");
    match args.manifest {
        Some(path) => {
            if let Err(error) = fs::write(&path, rendered) {
                eprintln!(
                    "error: failed to write manifest to {}: {error}",
                    path.display()
                );
                return ExitCode::FAILURE;
            }
        }
        None => println!("{rendered}"),
    }

    ExitCode::SUCCESS
}
