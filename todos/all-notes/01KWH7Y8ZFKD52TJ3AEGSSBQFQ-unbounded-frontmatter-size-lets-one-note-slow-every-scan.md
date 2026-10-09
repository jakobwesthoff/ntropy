---
title: "Unbounded frontmatter size lets one note slow every scan"
kind: bug
component: note
origin: review
tags: [security]
---
# Unbounded frontmatter size lets one note slow every scan

Found in the 2026-07-02 security review of the note parser. A note whose
frontmatter is a deeply nested flow collection, such as `[[[...]]]`, costs
quadratic CPU time in the YAML library's load phase. Nothing caps how large
a frontmatter block or a note file may be. The review rated this a local
denial of service. It is reachable only after a vault the user did not
author is adopted, for example a cloned repository, a synced folder or an
extracted archive. The fix is a one-line byte cap, so it is cheap to do.

## Problem

Read in the code and checked empirically in a sandbox during the 2026-07-02
security review, against `serde_yaml_ng` 0.10.0 on `unsafe-libyaml` 0.2.11.

The path runs as follows. `parse_block` in `src/note/frontmatter.rs` (around
line 111) deserializes the block into an owned `Value` with
`serde_yaml_ng::from_str`. Untrusted input reaches it through `load_note` in
`src/scan.rs` (around line 134), which reads the whole file with
`std::fs::read_to_string` and no size cap. Then `Note::parse` in
`src/note/mod.rs` runs `frontmatter::split` and `parse_block`. The scanner
uses the parallel walker from `ignore` and waits until every `load_note`
call returns.

### The library already mitigates the classic YAML attacks

The two attacks the finding named do not work as assumed. The pinned library
inherits two guards from the deserializer, both in the deserialize phase.

- Alias expansion is bounded by a repetition limit that scales with the
  source event count, so expansion is linear, never exponential. An alias
  bomb of 686 bytes, which would expand to 10^12 leaves without the limit,
  returned an error in 8 ms and 27 MB.
- Nesting depth is capped at 128. A flow sequence at depth 127 parses, and
  depth 128 returns a recursion limit error. Depths of 200, 1,000 and 10,000,
  flow maps and 12.5 MB block inputs all returned errors. None crashed.

### The real vector: quadratic CPU in the load phase

The library parses in two phases. The load phase pulls the entire event
stream from libyaml into memory before anything else. The guards above run
only in the later deserialize phase, so the load phase has no time or depth
guard.

For flow collections (`[` and `{`), libyaml's tokenizer is quadratic in input
size, and that cost is paid before the depth guard can fire. The error column
stays at 131 while runtime grows with `n`, so the guard trips at the same
shallow point every time and the time is spent upstream. Block-nested input
is not quadratic: 5,000 levels in 12.5 MB took 35 ms.

| shape | n | input | elapsed | peak RSS |
|---|---|---|---|---|
| flow seq | 10 000 | 20 KB | 152 ms | 8 MB |
| flow seq | 40 000 | 80 KB | 2 032 ms | 19 MB |
| flow seq | 80 000 | 160 KB | 8 128 ms | 35 MB |
| flow seq | 100 000 | 200 KB | timeout after 5 s | n/a |
| flow map | 40 000 | 200 KB | 4 025 ms | 25 MB |

Doubling `n` roughly quadruples the time. Memory stays low, so the attack
exhausts CPU, not memory. Extrapolating, a block of about 2 MB pins one core
for about 20 minutes before it returns an error. Nothing in `load_note` or
`parse_block` stops such a file from being accepted.

### Residual alias memory

The alias ceiling is `events.len() * 100`, and each permitted jump can build
a leaf subtree. A crafted 3.2 KB block reached about 1.96 GB of transient
RSS before tripping the limit. That is roughly 600,000 times amplification,
but it is bounded by input size, not exponential. A byte cap on the block
bounds it, because the ceiling scales with source size. This corrects an
earlier claim that a byte cap cannot help against alias bombs.

### Reachability

The scan is stateless and reparses every note on each invocation (ADR 0002),
and it waits for all files. One crafted note therefore adds its full parse
time to every scanning command (search, view sync, reconcile, LSP hover and
completion) until the note is removed. A few hundred KB costs seconds, and a
few MB costs minutes, on every command. The parallel walker parses up to one
bomb file per worker thread at once, and extra bombs queue behind them.

The error path does contain the result. Each failure becomes a warning, so
one bad file never breaks the scan (ADR 0019). The containment covers the
result, though, not the CPU time spent reaching the error, and that time is
the damage.

## Impact

Local denial of service only. Nothing is lost, corrupted or executed. Each
parse eventually returns an error and memory stays bounded. The review rated
the severity low and the fix priority high relative to its effort, because
the quadratic case is cheap and deterministic to trigger and degrades every
command. It does not reach the critical level, so no impact is set.

## Suggested fix

Rank 1: cap the frontmatter block's byte length in `parse_block`, before
`from_str`. The cap bounds the quadratic flow cost, the alias memory
amplification and any oversized flat input. Because the flow cost is
quadratic in bytes, the cap value matters. At 8 KiB the worst case is about
25 ms, at 16 KiB about 100 ms (recommended) and at 64 KiB about 1.5 s. A real
note header is well under 4 KB, so 16 KiB is generous. Add a
`FrontmatterError::TooLarge { bytes, limit }` variant so the scanner records
the failure as a warning under ADR 0019.

```rust
const MAX_FRONTMATTER_BYTES: usize = 16 * 1024;

pub fn parse_block(block: &str) -> Result<Frontmatter, FrontmatterError> {
    if block.len() > MAX_FRONTMATTER_BYTES {
        return Err(FrontmatterError::TooLarge { bytes: block.len(), limit: MAX_FRONTMATTER_BYTES });
    }
    let value: Value = serde_yaml_ng::from_str(block)?;
    // ...
}
```

Rank 2, required alongside rank 1: cap the file size in `load_note` before
`read_to_string`. The block cap cannot cover this. `read_to_string` allocates
a multi-GB `.md` file in full before any frontmatter split happens. Check
`metadata().len()`, which the code already reads nearby for the mtime, or use
a length-limited reader, and reject files over a generous bound. Bodies are
held in memory for `text:` search (ADR 0030), so a few MB is a reasonable
limit. Consider making it configurable. Record the rejection as a
`ScanWarning`.

Rank 3, do not rely on it: reject anchors and aliases. The library already
bounds alias expansion, and the byte cap bounds the residual memory, so this
is defense in depth at best. It cannot be done cleanly. The `libyaml` module
of `serde_yaml_ng` is private and offers no anchor or alias hook, and by the
time a `Value` exists the aliases are already expanded. A textual scan for `&`
and `*` gives false positives on quoted scalars such as `title: "R&D budget"`,
and separating anchor syntax from quoted characters needs a quote-aware
tokenizer, which means parsing again. The only robust route is an event-level
check on `unsafe-libyaml` for alias events, which is a large `unsafe` surface
for little gain. Do not block on it.

Rank 4: recursion depth is already capped at 128. No action, because the
quadratic cost sits upstream of that guard.

Rank 5: streaming or a bounded representation. Rejected. The owned `Value` is
required for `field:value` matching (ADR 0005), and its size is bounded once
the block is capped.

Rank 6: leave it as wontfix. Rejected. The fix is trivial, and the quadratic
case is a deterministic single-file footgun that degrades every command. If
it is chosen anyway, document the quadratic flow nesting behaviour, not the
already mitigated billion laughs case.

## Tests

Add these deterministic, fast tests to the `#[cfg(test)]` module of
`src/note/frontmatter.rs`:

```rust
#[test]
fn parse_alias_bomb_returns_err_not_hang() {
    // Tiny source that would expand to 10^6 leaves if unbounded; the library's
    // repetition limit returns Err in well under a millisecond.
    let mut b = String::from("title: T\nl0: &l0 [\"x\",\"x\",\"x\",\"x\",\"x\"]\n");
    for lvl in 1..=6 {
        b.push_str(&format!("l{lvl}: &l{lvl} [*l{p},*l{p},*l{p},*l{p},*l{p}]\n", p = lvl - 1));
    }
    b.push_str("top: *l6\n");
    assert!(parse_block(&b).is_err()); // repetition limit exceeded
}

#[test]
fn parse_deeply_nested_flow_does_not_stack_overflow() {
    // Depth 300 flow seq (~605 bytes, under the byte cap): the recursion guard
    // returns Err at depth 128; must not abort the process.
    let block = format!("title: T\nk: {}1{}\n", "[".repeat(300), "]".repeat(300));
    assert!(parse_block(&block).is_err()); // recursion limit exceeded
}

#[test]
fn parse_rejects_oversized_block_before_yaml() {
    let block = "x".repeat(MAX_FRONTMATTER_BYTES + 1);
    assert!(matches!(parse_block(&block), Err(FrontmatterError::TooLarge { .. })));
}
```

Add a scanner test as well: an oversized `.md` becomes a `ScanWarning` and is
not read whole. Use a file just over the cap, never a multi-GB one. Do not add
a quadratic-input test (n = 100,000) to the suite. It is slow by construction
and adds nothing over the cap test.

## Further notes

- The two caps guard different failures at two sites, and neither replaces
  the other. The file size cap in `load_note` must come before the read,
  because it cannot be applied after it. The block byte cap in `parse_block`
  comes after `split`, because the block size is known only after the split.
- Any write-up should name the quadratic flow nesting cost as the risk. The
  billion laughs and stack overflow cases are mitigated by the library.
- Once the block is capped, `Frontmatter.mapping` retention is not a DoS
  factor.
- The sandbox harness lived in the session scratchpad. Nothing was written
  into the repository.

## Done when

- A frontmatter block over the byte cap returns `FrontmatterError::TooLarge`
  before `from_str` and is recorded as a `ScanWarning` (ADR 0019). A test
  verifies it.
- A deeply nested flow block and an alias bomb both return `Err` in bounded
  time without a stack overflow. Tests verify it.
- An oversized note file becomes a `ScanWarning` and is not read whole. A
  scanner test verifies it.
- Legitimate frontmatter (title, tags, arbitrary scalar fields) parses
  unchanged, and the existing `frontmatter.rs` tests stay green.
