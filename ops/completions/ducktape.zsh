#compdef ducktape
# ducktape zsh completion — hand-generated from bin/node/src/cli.rs NODE_VERBS.
# The node-bin drift guard (cli.rs tests::completion_files_cover_the_verb_table)
# fails the build if a verb token or flag here drifts from the table.
#
# install: put this file on your $fpath as `_ducktape`, then `autoload -U _ducktape`.

_ducktape() {
    local families=(node user gateway fs mcp help --help -h version --version -V)

    local node_verbs=(run key init invite admit join list resident member upgrade help)
    local node_resident=(accept remove)
    local node_member=(promote remove leave status)
    local node_join=(requests state)
    local node_upgrade=(status)
    local node_flags=(--config -n --network --sync-only --json --out --dir --name --listen
        --advertised --http --rpc --gateway --primary-coordinator --wireguard-listen
        --wireguard-advertised --invite-listen --wireguard-effect --role --ttl-days)

    local user_verbs=(key sign-bind sign-unbind sign-possession sign-add-member
        sign-remove-member sign-gateway-route sign-frame sign-admin redeem-invite
        webauthn-challenge p256-payload help)
    local user_flags=(--out --key --node -n --network)
    local gateway_verbs=(bind unbind list help)
    local gateway_flags=(--workspace -n --network --label --port)
    local fs_verbs=(ls cat stat history diff checkout status commit pin help)
    local fs_flags=(-n --network --json --node --message -m --no-rebase --snapshot)

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
                upgrade)  compadd -- $node_upgrade $node_flags ;;
                run|key|init|invite|admit|list) compadd -- $node_flags ;;
                *)        compadd -- $node_verbs ;;
            esac
            ;;
        user)    compadd -- $user_verbs $user_flags ;;
        gateway) compadd -- $gateway_verbs $gateway_flags ;;
        fs)      compadd -- $fs_verbs $fs_flags ;;
    esac
}

_ducktape "$@"
