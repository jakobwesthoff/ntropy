# SEC-1: YAML frontmatter parser DoS — quadratic flow-nesting CPU + giant-file OOM

> **STATUS: TRIAGED (code review + Fable empirical triage, 2026-07-02).**
> Fable ran a bounded empirical investigation against `serde_yaml_ng` 0.10.0
> (source analysis + memory/time sweep in a sandbox). **The finding's original
> framing is overturned:** the "billion laughs" alias bomb and deep-nesting
> stack overflow are **both already mitigated by the pinned library**. The real,
> unmitigated vector is a **quadratic-time CPU blowup in libyaml's load phase for
> deeply-nested flow collections** (`[[[…]]]` / `{{{…}}}`), plus a separate
> giant-file OOM in the scanner. Severity is LOW (local, DoS-only, SEC-4-gated)
> but the fix is a one-line byte cap, so fix-priority-relative-to-effort is high.

## Where / call path

- `parse_block` (`src/note/frontmatter.rs:111-136`) does
  `let value: Value = serde_yaml_ng::from_str(block)?;` (line 112), deserializing
  into the fully-owned `Value` tree (retained on `Frontmatter.mapping` for
  generic `field:value` matching).
- Untrusted-input path: `scan::scan_notes_dir` → `load_note`
  (`src/scan.rs:134-138`, reads the whole file via `std::fs::read_to_string`
  with **no size cap**) → `Note::parse` (`src/note/mod.rs:68-103`) →
  `frontmatter::split` → `parse_block`.
- The scanner uses `ignore`'s **parallel** walker (`src/scan.rs:69-73`) and
  blocks until every file's `load_note` returns (`scan.rs:69-109`).

## What the library ALREADY mitigates (empirically confirmed)

`serde_yaml_ng` 0.10.0 (Antoine Catton's maintained fork of the archived
`serde_yaml`, on `unsafe-libyaml` 0.2.11) inherited dtolnay's two DoS guards,
both in the **deserialize** phase:

- **Alias repetition limit (billion-laughs guard).** Every alias resolution goes
  through `jump`, bounded at `de.rs:478-481`:
  `if *self.jumpcount > self.document.events.len() * 100 { return Err(RepetitionLimitExceeded) }`.
  Total expansions are **linear in source event count, never exponential**.
  - Empirical: an alias bomb of `levels=12, width=10` (would expand to 10¹²
    leaves unbounded) returns `Err` in **8 ms / 27 MB from a 686-byte input**.
    `levels=2,width=10` (159 B) parses OK; `levels≥4` trips the limit. Never
    exponential.
- **Recursion depth limit (128).** `recursion_check` (`de.rs:632-645`, set to 128
  at `de.rs:112`) wraps `visit_sequence`/`visit_mapping` (`de.rs:538,555`).
  - Empirical: flow-seq depth 127 → OK, depth 128 → `Err "recursion limit
    exceeded"`. Depth 200/1 000/10 000, flow-map, and 12.5 MB block inputs: all
    `Err`, **no SIGSEGV/SIGABRT/stack overflow in any configuration**. The
    finding's "deep nesting stack-overflows" is **refuted**.

So the two mechanisms the finding named do not behave as assumed: both are
caught, in bounded time (alias) or cleanly at depth 128 (nesting).

## The REAL vector: O(n²) CPU in libyaml's flow-collection load phase

serde_yaml parses in two phases: **phase 1 (load)** eagerly pulls the entire
event stream from libyaml into `document.events` before anything else
(`loader.rs:60-117`); **phase 2 (deserialize)** walks events into a `Value` and
is where both guards live. **The guards are phase-2 only; phase 1 has no time or
depth guard.**

For deeply-nested **flow** collections (`[`/`{`), libyaml's phase-1 tokenizer is
**quadratic in input size**, and that cost is paid in full *before* the depth-128
guard can fire. Proof: the error column stays fixed at 131 while runtime grows
with `n` (the guard trips at the same shallow point every time; the time is spent
upstream in phase 1). Empirical sweep (release build, sandboxed):

| shape | n | input | elapsed | peak RSS |
|---|---|---|---|---|
| flow seq | 10 000 | 20 KB | 152 ms | 8 MB |
| flow seq | 40 000 | 80 KB | 2 032 ms | 19 MB |
| flow seq | 80 000 | 160 KB | 8 128 ms | 35 MB |
| flow seq | 100 000 | 200 KB | **TIMEOUT >5 s** | — |
| flow map | 40 000 | 200 KB | 4 025 ms | 25 MB |

Doubling `n` ~quadruples time → **O(n²)**, memory stays low (CPU exhaustion, not
memory). **Block-nested input is NOT quadratic** (n=5000 block, 12.5 MB, 35 ms) —
the blowup is specific to flow context. Extrapolating: a ~2 MB frontmatter block
pins one core for **~20 minutes** and still returns `Err` at the end.
`load_note`/`parse_block` have no size cap, so such a file is fully accepted.

### Residual alias case is memory, and a byte cap bounds it
The alias repetition ceiling is `events.len() * 100`, and each permitted jump can
materialize a leaf subtree, so a crafted **3.2 KB** block reached **~1.96 GB**
transient RSS before tripping (~600 000× amplification, but **bounded by input
size**, not exponential). A byte cap on the block *does* bound this, because the
ceiling scales with source size. (This corrects the preliminary's note that a
byte cap can't help alias bombs: it can here, because the library already caps
expansion linearly in source size.)

## Reachability / blast radius

The scan is stateless and re-parses every note on every invocation (ADR 0002),
and `scan_notes_dir` blocks until all `load_note` calls return. So a **single**
crafted `[[[…]]]` note adds its full quadratic parse time to **every** scanning
command (search, view sync, reconcile, LSP hover/complete) until removed. A
few-hundred-KB file → seconds; low-MB → minutes, recurring on every command.
Parallel walker: one file degrades every command regardless; K bomb files
(K ≤ worker threads) parse concurrently (wall-time ≈ slowest file) but burn K
cores; more than that queue. The walker neither amplifies nor mitigates.

## ADR 0019 nuance

The net catches all three cases as `Err`/warning (no crash), so "one bad file
never breaks the scan" holds. But it catches the **result**; it does **not** bound
the **CPU-time** spent reaching the `Err` for the quadratic flow case. That time
is the damage.

## Severity: LOW (fix-priority-relative-to-effort: HIGH)

Local, DoS-only, SEC-4-gated (requires adopting an untrusted vault — cloned
repo / synced folder / extracted archive). No data loss, corruption, or RCE;
self-limits (returns `Err` eventually, memory bounded for the quadratic case).
Rated LOW. Worth fixing anyway because the fix is a one-line byte cap and the
quadratic footgun is cheap and deterministic to trigger and degrades every
command.

## Fix — ranked

**Rank 1 (primary): cap the frontmatter block byte length in `parse_block`
before `from_str`.** Not merely partial — it directly bounds the quadratic
flow-nesting CPU DoS (the only unmitigated vector), the residual alias memory
amplification (ceiling scales with source size), and any absurd flat input.
Because flow cost is O(bytes²), choose the cap deliberately: 8 KiB → ~25 ms
worst case; **16 KiB → ~100 ms (recommended)**; 64 KiB → ~1.5 s. A real note
header is well under 4 KB, so 16 KiB is generous. Add a
`FrontmatterError::TooLarge { bytes, limit }` variant so the scanner records it
as a warning per ADR 0019.

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

**Rank 2 (independent, also needed): file-size cap in `load_note` before
`read_to_string`.** A separate DoS the block cap does not cover: `read_to_string`
(`scan.rs:135`) allocates an entire multi-GB `.md` into memory before any
frontmatter split. Check `metadata().len()` (a stat is already done nearby at
`scan.rs:136` for mtime) or use a length-limited reader; reject over a generous
bound. Bodies are held in memory for `text:` search (ADR 0030), so allow a few
MB (consider making it configurable). Closes an OOM independent of YAML. Record
it as a `ScanWarning`.

**Rank 3 (de-prioritize; do NOT rely on): reject anchors/aliases.** The library
already bounds alias expansion, and the byte cap bounds the memory residual, so
this is defense-in-depth at best. It also cannot be done cleanly:
`serde_yaml_ng`'s `libyaml` module is **private** (`lib.rs:176`) and exposes **no
anchor/alias hook or "no-aliases" mode** (public entry points are
`from_str`/`from_slice`/`from_reader`/`Deserializer`, `lib.rs:165`); by the time
you hold a `Value`, aliases are already expanded. A textual pre-scan for `&`/`*`
false-positives on legitimate quoted scalars (`title: "R&D budget"`,
`status: "*important*"`) — reliably distinguishing anchor syntax from quoted `&`/
`*` needs a quote-aware tokenizer, i.e. re-parsing. The only robust route is an
event-level check against `unsafe-libyaml` (reject `YAML_ALIAS_EVENT`), a large
`unsafe` surface for marginal benefit. Do not block on it.

**Rank 4: recursion depth — already 128, no action.** Not the bottleneck (the
quadratic is in phase 1, upstream of this guard).

**Rank 5: streaming/bounded representation — reject** (over-engineering; the
owned `Value`/`Mapping` is required for `field:value` matching per ADR 0005, and
its size is bounded once the block is capped; `Frontmatter.mapping` retention is
not a DoS multiplier once capped).

**Rank 6: wontfix — reject** (trivial fix vs. a deterministic single-file
quadratic footgun that degrades every command). If chosen anyway, document the
quadratic flow-nesting behavior, not the already-mitigated billion-laughs.

## Tests to add (deterministic, fast — no multi-GB allocation)

In `src/note/frontmatter.rs` `#[cfg(test)]`:

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

Plus a scanner test: an oversized `.md` becomes a `ScanWarning` rather than being
read whole (size just over the cap, not multi-GB). Do **not** add a
quadratic-input test (n=100 000) to the suite — slow by construction, adds
nothing over the cap test.

## Anything else

- Two **independent** caps at two sites, addressing different DoS classes,
  neither substituting for the other: file-size cap in `load_note` *before*
  `read_to_string` (giant-file OOM — impossible to do after the read); block
  byte-length cap in `parse_block` *after* `split` (block size only known
  post-split; bounds the YAML quadratic + alias memory).
- Correct the queue framing in any write-up: billion-laughs and stack overflow
  are mitigated by the library; center the quadratic **flow**-nesting CPU cost.
- `Frontmatter.mapping` retention is not a DoS factor once the block is capped.
- Sandbox harness lived under the session scratchpad (`.../scratchpad/yamltest/`);
  nothing was written into the tracked repo.

## Acceptance

- A frontmatter block over the byte cap returns `FrontmatterError::TooLarge`
  before `from_str`, recorded as a `ScanWarning` (ADR 0019); verified by test.
- A deeply-nested flow block and an alias bomb both return `Err` in bounded
  time without stack overflow; verified by tests.
- An oversized note file becomes a `ScanWarning` rather than being read whole;
  verified by a scanner test.
- Legitimate frontmatter (title, tags, arbitrary scalar fields) parses
  unchanged; existing `frontmatter.rs` tests stay green.
