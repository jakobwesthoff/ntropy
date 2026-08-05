# Shell integration

Optional shell functions that wrap the CLI, shipped as sourceable files under
`contrib/shell/`. They are not installed by any packaging step; a user sources
the file for their shell from their rc file. The command surface they build on
is in [cli.md](cli.md).

`contrib/shell/ntropy.sh` covers bash and zsh.

## `ncd`: jump to the active vault

A process cannot change its parent shell's working directory, so this cannot be
a subcommand: `ntropy cd` would change the directory of a child that is about to
exit. The work therefore splits across the two sides of that boundary. The
binary resolves the vault and reports it through
[`info --print`](cli.md#info); the shell function performs the `cd`.

    ncd() {
        local root
        root=$(ntropy info --print "$@") || return
        cd "$root" || return
    }

The path is captured in a variable rather than substituted directly into `cd`,
which preserves ntropy's exit status. An unresolvable vault leaves the shell
where it was, whereas `cd "$(...)"` would run `cd ""` and succeed.

Arguments are forwarded to `ntropy`, so the global flags pick a vault exactly as
they do for every other command, as in `ncd --vault ~/notes`.
