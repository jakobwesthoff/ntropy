# README Scripting section still describes tab-separated output

The README's Scripting section documents the plain `search` output as a
tab-separated table (`ID<TAB>DATE<TAB>TITLE<TAB>TAGS<TAB>PATH`, "awk and cut
work on it directly").

Since v1.3.0 (ADR 0033, CHANGELOG v1.3.0), plain tables are space-aligned:
columns padded to their widest cell in Unicode display width, last column
unpadded, separated by runs of two or more spaces. Verified against the 1.3.0
binary: `search -n` emits spaces, no tabs. `docs/design/cli.md` already
describes the new format; only the README lags.

Update the Scripting section (and any other README mention of tab-separated
output) to the aligned format, including revised `awk`/`cut` guidance
(`awk -F'  +'` or first-column extraction; `tail -n +2` still drops the
header).
