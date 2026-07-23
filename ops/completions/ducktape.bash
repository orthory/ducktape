# ducktape bash completion — hand-written; checked against the clap tree.
# The node-bin drift guard (cli.rs tests::completion_files_cover_the_verb_table)
# fails the build if a verb token or flag here drifts from the table.
#
# install: source this file, or drop it in /etc/bash_completion.d/.

_ducktape() {
    local cur prev
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"

    local families="node user gateway fs agent mcp help --help -h version --version -V"

    local node_verbs="run key init invite admit join list status peers resident member help"
    local node_resident="accept remove"
    local node_member="promote remove leave status"
    local node_join="requests state"
    local node_flags="--config -n --network --sync-only --json --out --dir --name --listen \
--advertised --http --rpc --gateway --primary-coordinator --wireguard-listen \
--wireguard-advertised --invite-listen --wireguard-effect --role --ttl-days"

    local user_key="init restore unlock reveal encrypt status"
    local user_cred="add list remove grant revoke"
    local user_verbs="key sign-bind sign-unbind sign-possession sign-add-member \
sign-remove-member sign-gateway-route sign-frame sign-admin redeem-invite \
webauthn-challenge p256-payload cred help"
    local user_flags="--path --method --statement --possession --out --key --node -n --network --account-id --chain-id --new-key --new-kind --node-key --node-pub --target-key --nonce --seq --route-key --json"
    local gateway_verbs="bind unbind list help"
    local gateway_flags="--workspace -n --network --label --port"
    local fs_verbs="ls cat stat history diff checkout status commit pin help"
    local fs_flags="-n --network --json --node --message -m --no-rebase --snapshot --limit --prefix"
    local agent_verbs="pty sched help"
    local agent_flags="-n --network --node --cred --cpu --mem"

    if [ "$COMP_CWORD" -eq 1 ]; then
        COMPREPLY=( $(compgen -W "$families" -- "$cur") )
        return
    fi

    case "${COMP_WORDS[1]}" in
        node)
            case "${COMP_WORDS[2]}" in
                resident) COMPREPLY=( $(compgen -W "$node_resident $node_flags" -- "$cur") ) ;;
                member)   COMPREPLY=( $(compgen -W "$node_member $node_flags" -- "$cur") ) ;;
                join)     COMPREPLY=( $(compgen -W "$node_join $node_flags" -- "$cur") ) ;;
                run|key|init|invite|admit|list|status|peers)
                          COMPREPLY=( $(compgen -W "$node_flags" -- "$cur") ) ;;
                *)        COMPREPLY=( $(compgen -W "$node_verbs" -- "$cur") ) ;;
            esac
            ;;
        user)
            case "${COMP_WORDS[2]}" in
                key)  COMPREPLY=( $(compgen -W "$user_key $user_flags" -- "$cur") ) ;;
                cred) COMPREPLY=( $(compgen -W "$user_cred $user_flags" -- "$cur") ) ;;
                *)    COMPREPLY=( $(compgen -W "$user_verbs $user_flags" -- "$cur") ) ;;
            esac
            ;;
        gateway) COMPREPLY=( $(compgen -W "$gateway_verbs $gateway_flags" -- "$cur") ) ;;
        fs)      COMPREPLY=( $(compgen -W "$fs_verbs $fs_flags" -- "$cur") ) ;;
        agent)   COMPREPLY=( $(compgen -W "$agent_verbs $agent_flags" -- "$cur") ) ;;
    esac
}

complete -F _ducktape ducktape
