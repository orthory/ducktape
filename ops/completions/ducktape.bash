# ducktape bash completion — hand-written; checked against the clap tree.
# The node-bin drift guard (cli.rs tests::completion_files_match_the_clap_tree)
# fails the build if a verb token or flag here drifts from the grammar.
#
# install: source this file, or drop it in /etc/bash_completion.d/.

_ducktape() {
    local cur prev
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"

    local families="node user wallet gateway fs service agent mcp help --help -h --version -V"

    local node_verbs="run key init invite admit join list status peers resident member work help"
    local node_resident="accept remove"
    local node_member="promote remove leave status"
    local node_work="list admit revoke"
    local node_join="requests state"
    local node_flags="--config -n --network --sync-only --json --out --dir --name --listen --advertised --http --rpc --gateway --primary-coordinator --wireguard-listen --wireguard-advertised --invite-listen --ttl-days"

    local user_key="init restore unlock reveal status"
    local user_cred="add list remove grant revoke inspect seal"
    local user_verbs="key sign-bind sign-unbind sign-possession sign-add-member sign-remove-member sign-gateway-route sign-frame sign-admin webauthn-challenge p256-payload cred account-init help"
    local user_flags="--path --method --statement --possession --out --key --node -n --network --account-id --chain-id --new-key --new-kind --node-key --node-pub --target-key --nonce --name --json --label --host --remote --attest --pccs-url --snp-product --snp-vcek --vendor --measurement --credentials --cred-kind --access-token --refresh-token"
    local wallet_verbs="new import list use help"
    local wallet_flags="--json"
    local gateway_verbs="bind unbind list help"
    local gateway_flags="--workspace -n --network --label --port"
    local fs_verbs="ls cat stat history diff checkout status commit pin help"
    local fs_flags="-n --network --json --node --message --no-rebase --snapshot --limit --prefix"
    local service_verbs="run list enable disable status help"
    local service_flags="--config --workspace -n --network --json --yes -y --enable --no-enable"
    # every service verb takes a KIND now, `list`/`status` included.
    local service_kinds="compute agent airlock"
    local agent_verbs="pty sched install help"
    local agent_flags="-n --network --node --host-node --cred --cpu --mem"

    if [ "$COMP_CWORD" -eq 1 ]; then
        COMPREPLY=( $(compgen -W "$families" -- "$cur") )
        return
    fi

    case "${COMP_WORDS[1]}" in
        node)
            case "${COMP_WORDS[2]}" in
                resident) COMPREPLY=( $(compgen -W "$node_resident $node_flags" -- "$cur") ) ;;
                member)   COMPREPLY=( $(compgen -W "$node_member $node_flags" -- "$cur") ) ;;
                work)     COMPREPLY=( $(compgen -W "$node_work $node_flags" -- "$cur") ) ;;
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
        wallet)  COMPREPLY=( $(compgen -W "$wallet_verbs $wallet_flags" -- "$cur") ) ;;
        gateway) COMPREPLY=( $(compgen -W "$gateway_verbs $gateway_flags" -- "$cur") ) ;;
        fs)      COMPREPLY=( $(compgen -W "$fs_verbs $fs_flags" -- "$cur") ) ;;
        service)
            case "${COMP_WORDS[2]}" in
                run|list|enable|disable|status)
                          COMPREPLY=( $(compgen -W "$service_kinds $service_flags" -- "$cur") ) ;;
                *)        COMPREPLY=( $(compgen -W "$service_verbs $service_flags" -- "$cur") ) ;;
            esac
            ;;
        agent)   COMPREPLY=( $(compgen -W "$agent_verbs $agent_flags" -- "$cur") ) ;;
    esac
}

complete -F _ducktape ducktape
