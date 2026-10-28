# Loaded into the launcher before the walk; also embedded in the driver's
# return command. No executable in the walk's store is needed for recovery.
rung_return() (
    set -eu
    rung_binary=$1 rung_store=$2 rung_client=$3 rung_path=$4 rung_number_file=$5
    # This path belongs to the launcher, so cancellation also precedes a
    # store that setup has not created yet. Import and unit launch check it.
    : > "$rung_number_file.returning"
    if [ -d "$rung_store" ]; then
        exec 9>"$rung_store/login.lock"
        flock -x 9
        : > "$rung_store/returning"
    fi
    if [ ! -e "$rung_path" ] && [ ! -L "$rung_path" ]; then
        if [ ! -s "$rung_number_file" ]; then
            # An import can move the record before its reply is observed.
            "$rung_binary" ctl "$rung_client" account list --json |
                jq -er '.accounts.party | map(select(.provider == 1 and .plan == true)) | if length == 1 then .[0].number else error("expected one held ChatGPT plan") end' > "$rung_number_file"
        fi
        IFS= read -r rung_account < "$rung_number_file"
        case $rung_account in ''|*[!0-9]*) exit 1 ;; esac
        "$rung_binary" ctl "$rung_client" account export "$rung_account" "$rung_path"
    fi
    [ "$(LC_ALL=C stat -c %F -- "$rung_path")" = 'regular file' ]
    [ "$(stat -c %a -- "$rung_path")" = 600 ]
    [ "$(stat -c %s -- "$rung_path")" -gt 0 ]
    stat -c 'LOGIN BACK: %F mode=%a bytes=%s owner=%u' -- "$rung_path"
)

rung_finish_account() {
    # A hook is a convenience, never the launcher's only way to give back.
    if [ -x "$2/give-back" ]; then
        timeout 60 "$2/give-back" || printf '%s\n' 'return hook failed; using launcher return' >&2
    fi
    timeout 60 sh -c "$rung_return_body
rung_return \"\$@\"" rung-return "$@"
}
