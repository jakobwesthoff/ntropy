# Security review queue

Raw security-relevant observations collected during the 2026-07-02 codebase
review. Each entry is triaged and processed at the end of the review (final
unit). Entries here are *candidates*, not confirmed vulnerabilities.

## Queue

_Empty — all candidates processed on 2026-07-02 into individual, self-contained,
Fable-triaged todos (severity order, highest first):_

1. **SEC-3** → `01kwh7y8zfkd52tj3aegssbqfr-sec3-view-name-path-traversal-destructive.md`
   (HIGH — destructive out-of-vault deletion via unvalidated view name).
2. **SEC-5** → `01kwh7y8zfkd52tj3aegssbqft-sec5-terminal-escape-injection-untrusted-fields.md`
   (Medium — terminal escape injection from untrusted note fields).
3. **SEC-4** → `01kwh7y8zfkd52tj3aegssbqfs-sec4-walkup-vault-trust-model.md`
   (Low residual — walk-up trust model; recommend document-don't-gate ADR).
4. **SEC-1** → `01kwh7y8zfkd52tj3aegssbqfq-sec1-yaml-frontmatter-dos-alias-recursion.md`
   (Low — quadratic YAML flow-nesting CPU DoS + giant-file OOM; alias/nesting
   already mitigated by the library).
5. **SEC-2** → `01kwh5hhwgqwmr1eghhtmed79t-filename-parse-panics-on-multibyte-names.md`
   (Medium — filename parse panic → livelock; existing todo rewritten).
6. **SEC-6** → `01kwh7y8zfkd52tj3aegssbqfv-sec6-ci-supply-chain-action-pinning.md`
   (Low-Medium hygiene — CI action pinning policy).

Each todo is preliminary-grade: extensive, self-contained, and ready for
implementation without re-analysis. Empirical PoCs are specified as regression
tests inside each.
