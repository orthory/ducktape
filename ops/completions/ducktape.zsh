#compdef ducktape
# ducktape zsh completion — hand-written; checked against the clap tree.
# The node-bin drift guard (cli.rs tests::completion_files_cover_the_verb_table)
# fails the build if a verb token or flag here drifts from the table.
#
# install: put this file on your $fpath as `_ducktape`, then `autoload -U _ducktape`.

_ducktape() {
    local families=(node user gateway fs service agent mcp help --help -h version --version -V)

    local node_verbs=(run key init invite admit join list status peers resident member help)
    local node_resident=(accept remove)
    local node_member=(promote remove leave status)
    local node_join=(requests state)
    local node_flags=(--config -n --network --sync-only --json --out --dir --name --listen --advertised --http --rpc --gateway --primary-coordinator --wireguard-listen --wireguard-advertised --invite-listen --wireguard-effect --role --ttl-days)

    local user_key=(init restore unlock reveal encrypt status)
    local user_cred=(add list remove grant revoke inspect seal)
    local user_verbs=(key sign-bind sign-unbind sign-possession sign-add-member sign-remove-member sign-gateway-route sign-frame sign-admin redeem-invite webauthn-challenge p256-payload cred account-init help)
    local user_flags=(--path --method --statement --possession --out --key --node -n --network --account-id --chain-id --new-key --new-kind --node-key --node-pub --target-key --nonce --seq --route-key --name --json --label --host --remote --attest --pccs-url --snp-product --snp-vcek --vendor --measurement --credentials --cred-kind --access-token --refresh-token)
    local gateway_verbs=(bind unbind list help)
    local gateway_flags=(--workspace -n --network --label --port)
    local fs_verbs=(ls cat stat history diff checkout status commit pin help)
    local fs_flags=(-n --network --json --node --message -m --no-rebase --snapshot --limit --prefix)
    local service_verbs=(run list enable disable status help)
    local service_flags=(--config --workspace -n --network --json --yes -y --enable --no-enable)
    local agent_verbs=(pty sched help)
    local agent_flags=(-n --network --node --host-node --cred --cpu --mem)

    if (( CURRENT == 2 )); then
        compadd -- $families
        return
    fi

    case ${words[2]} in
        node)
            case ${words[3]} in
                resident) compadd -- $node_resident $node_flags ;;
                member)   compadd -- $node_member $node_flags ;;
                join)     compadd -- $node_join $node_flags ;;
                run|key|init|invite|admit|list|status|peers) compadd -- $node_flags ;;
                *)        compadd -- $node_verbs ;;
            esac
            ;;
        user)
            case ${words[3]} in
                key)  compadd -- $user_key $user_flags ;;
                cred) compadd -- $user_cred $user_flags ;;
                *)    compadd -- $user_verbs $user_flags ;;
            esac
            ;;
        gateway) compadd -- $gateway_verbs $gateway_flags ;;
        fs)      compadd -- $fs_verbs $fs_flags ;;
        service) compadd -- $service_verbs $service_flags ;;
        agent)   compadd -- $agent_verbs $agent_flags ;;
    esac
}

_ducktape "$@"
