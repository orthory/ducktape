#compdef ducktape
# ducktape zsh completion — hand-written; checked against the clap tree.
# The node-bin drift guard (cli.rs tests::completion_files_match_the_clap_tree)
# fails the build if a verb token or flag here drifts from the grammar.
#
# install: put this file on your $fpath as `_ducktape`, then `autoload -U _ducktape`.

_ducktape() {
    local families=(node user account wallet gateway fs service agent mcp help --help -h --version -V)

    local node_verbs=(run key init invite admit join list status peers resident member work sandbox help)
    local node_resident=(accept remove)
    local node_member=(promote remove leave status)
    local node_work=(list admit revoke)
    local node_join=(requests state)
    local node_flags=(--config -n --network --sync-only --json --yes --out --dir --name --listen --advertised --http --rpc --gateway --primary-coordinator --wireguard-listen --wireguard-advertised --invite-listen --ttl-days)

    local user_key=(init restore unlock reveal status)
    local user_cred=(add list remove grant revoke inspect seal)
    local user_verbs=(key sign-gateway-route sign-frame sign-admin sign-caller cred help)
    local user_flags=(--path --method --statement --out --key --node -n --network --node-key --publisher-node --account --route --name --json --host --remote --attest --pccs-url --snp-product --snp-vcek --vendor --measurement --credentials --cred-kind --access-token --refresh-token)
    local account_verbs=(create show key set-name set-profile help)
    local account_key=(list approve add join remove)
    local account_flags=(--node -n --network --key --name --number --pubkey --scheme --label --ticket --avatar --bio)
    local wallet_verbs=(new import list use help)
    local wallet_flags=(--json)
    local gateway_verbs=(bind unbind list help)
    local gateway_flags=(--workspace -n --network --label --port)
    local fs_verbs=(ls cat stat history diff checkout status commit pin help)
    local fs_flags=(-n --network --json --node --message --no-rebase --snapshot --limit --prefix)
    local service_verbs=(run list enable disable status help)
    local service_flags=(--config --workspace -n --network --json --yes -y --enable --no-enable)
    # every service verb takes a KIND now, `list`/`status` included.
    local service_kinds=(compute agent airlock)
    local agent_verbs=(pty sched install help)
    local agent_flags=(-n --network --node --key --host-node --cred --cpu --mem)

    if (( CURRENT == 2 )); then
        compadd -- $families
        return
    fi

    case ${words[2]} in
        node)
            case ${words[3]} in
                resident) compadd -- $node_resident $node_flags ;;
                member)   compadd -- $node_member $node_flags ;;
                work)     compadd -- $node_work $node_flags ;;
                join)     compadd -- $node_join $node_flags ;;
                run|key|init|invite|admit|list|status|peers|sandbox) compadd -- $node_flags ;;
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
        account)
            case ${words[3]} in
                key) compadd -- $account_key $account_flags ;;
                *)   compadd -- $account_verbs $account_flags ;;
            esac
            ;;
        wallet)  compadd -- $wallet_verbs $wallet_flags ;;
        gateway) compadd -- $gateway_verbs $gateway_flags ;;
        fs)      compadd -- $fs_verbs $fs_flags ;;
        service)
            case $words[3] in
                run|list|enable|disable|status) compadd -- $service_kinds $service_flags ;;
                *) compadd -- $service_verbs $service_flags ;;
            esac
            ;;
        agent)   compadd -- $agent_verbs $agent_flags ;;
    esac
}

_ducktape "$@"
