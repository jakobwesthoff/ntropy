# ntropy demo - single terminal
# Run with: ./record.sh   (it seeds the vault this tape expects)

Output demo.webm
Output demo.mp4

Set Shell "zsh"
Set FontFamily "VictorMono NFM SemiBold"
Set FontSize 26
Set Width 1700
Set Height 1000
# ntropy palette: brand teal lives in the cyan slot, so the picker's current
# row, the cursor, and the `tree` view's symlinks all render in the accent.
# Matches stay yellow; directories blue; everything reads on a near-black base.
Set Theme { "name": "ntropy", "black": "#0f0f12", "red": "#ef6487", "green": "#3fb950", "yellow": "#fdd877", "blue": "#60a5fa", "magenta": "#a78bfa", "cyan": "#2dd4bf", "white": "#e5e5e5", "brightBlack": "#6b7280", "brightRed": "#f78fa7", "brightGreen": "#56d364", "brightYellow": "#fde68a", "brightBlue": "#83b3fb", "brightMagenta": "#c4b5fd", "brightCyan": "#5eead4", "brightWhite": "#ffffff", "background": "#0f0f12", "foreground": "#e5e5e5", "selection": "#1c1c22", "cursor": "#2dd4bf" }
Set Padding 20

# Hidden setup - drop into the prepared demo vault with a clean green prompt.
# ntropy resolves the vault by walking up to the .ntropy directory, so simply
# being inside it is enough - no --vault needed for the rest of the demo.
Set TypingSpeed 0
Hide
Type "cd /tmp/ntropy-demo"
Enter
Sleep 100ms
Type "PROMPT='%F{#2dd4bf}> %f'"
Enter
Sleep 100ms
Type "clear"
Enter
Sleep 300ms
Show
Set TypingSpeed 35ms

# == VISIBLE RECORDING STARTS ==

Sleep 500ms

# Create a new note. ntropy stamps it from a template and hands it to $EDITOR.
# Pause after typing so the command is readable before it runs.
Type `ntropy new "The load-bearing print statement"`
Sleep 1.3s
Enter
Sleep 1.5s

# --- inside Neovim ---------------------------------------------------------
# Jump to the `tags: []` line and add two tags between the brackets.
Type "3G"
Sleep 600ms
Type "f["
Sleep 400ms
Type "a"
Type "rust, learning"
Sleep 800ms
Escape
Sleep 500ms

# Jump to the end and write the note body: a blank line, then one sentence.
Type "G"
Sleep 300ms
Type "o"
Enter
Type "The Heisenbug vanishes the instant I delete the debug print, so the print is now load-bearing production infrastructure."
Sleep 900ms
Escape
Sleep 500ms

# Save and quit; ntropy reconciles the new note and returns to the shell.
Type ":wq"
Enter
Sleep 1.5s
# ---------------------------------------------------------------------------

# Find it again with the query language: a tag the new note shares with several
# others, minus the reading pile. `not` excludes; more than one match opens the
# interactive fuzzy picker, bottom-anchored like a shell prompt.
Type "ntropy search tag:rust and not tag:reading"
Sleep 1.5s
Enter
Sleep 2s

# Filter live to the note we just wrote, then open it to confirm it stuck.
Type "load"
Sleep 1.5s
Enter
Sleep 2.5s
Type ":q"
Enter
Sleep 1.5s

# Browsing is the second half of the story. Turn a frontmatter field into a
# materialized view: a real directory of symlinks grouped by `status`.
Type "ntropy view add by-status --field status"
Sleep 1.5s
Enter
Sleep 2s

# The view is just files. `tree` shows it: a folder per status value, each
# leaf a symlink back to the canonical note in all-notes/.
Type "tree by-status"
Sleep 1.3s
Enter
Sleep 4s
