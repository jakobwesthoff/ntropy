# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.

# shellcheck shell=bash
#
# ntropy shell integration for bash and zsh. Source it from your rc file:
#
#     source /path/to/contrib/shell/ntropy.sh

# Change directory to the active ntropy vault.
#
# A process cannot change its parent shell's working directory, so the lookup
# and the `cd` are split between the binary and this function: `ntropy info
# --print` applies the usual vault resolution rules and prints the resulting
# path, which is always absolute.
#
# Arguments are forwarded to `ntropy`, so the global flags select a vault the
# same way they do everywhere else, as in `ncd --vault ~/notes`.
ncd() {
    # Capturing the path in a variable rather than inlining the substitution
    # into `cd` keeps ntropy's exit status: an unresolvable vault must leave
    # the shell where it was, and `cd ""` would succeed.
    local root
    root=$(ntropy info --print "$@") || return
    cd "$root" || return
}
