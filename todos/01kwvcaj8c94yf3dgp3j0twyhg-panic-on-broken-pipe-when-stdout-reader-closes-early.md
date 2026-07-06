# Panic on broken pipe when the stdout reader closes early

`ntropy info | head -2` (binary 1.3.0) panics once `head` closes the pipe:

    thread 'main' panicked at library/std/src/io/stdio.rs:1165:9:
    failed printing to stdout: Broken pipe (os error 32)

Rust's default `println!` machinery panics on `EPIPE` instead of exiting
quietly the way Unix CLI tools conventionally do. Any command with more output
than the reader consumes is affected, not just `info`.

Fix direction: treat `EPIPE` on stdout as a normal early exit (e.g. reset
`SIGPIPE` to `SIG_DFL` at startup, or route output through a writer that maps
`BrokenPipe` errors to a clean exit).

Per the bug-fix workflow, start with a failing test reproducing the panic
(pipe a command into a reader that closes immediately).
