//! Signed account/device operations for the native Home surface.

use serde_json::{Value, json};

use crate::backend::{
    AddMemberRequest, Backend, BindRequest, IdentityStatus, LinkAddress, LinkChallenge,
    LinkPending, LinkResponse, MemberKeyKind, PossessionRequest, RemoveMemberRequest, Workspace,
    decode_link_challenge, decode_link_response, encode_link_challenge, encode_link_response,
};
use crate::screens::user::{
    AccountKeyKind, Command, LinkChallengeView, LinkReplyPreview, LinkResponderReply,
    LinkResponderSession, LinkSession, PhoneCandidateView, PhoneEnrollmentView, ServiceEvent,
};
use crate::transport::NodeClient;

const MAX_IDENTITY_MESSAGE_BYTES: usize = 64 * 1024;
const MAX_LABEL_BYTES: usize = 64;

#[derive(Debug, Clone)]
pub(crate) struct AccountFacts {
    pub account_id: String,
    pub display_name: Option<String>,
    pub avatar: Option<String>,
    pub bio: Option<String>,
    pub nonce: u64,
    pub member_keys: Vec<(String, MemberKeyKind)>,
    pub nodes: Vec<String>,
}

pub async fn execute(
    backend: Option<Backend>,
    workspace: Option<Workspace>,
    node: Option<NodeClient>,
    command: Command,
) -> ServiceEvent {
    match command {
        Command::LinkDevice => ServiceEvent::LinkStarted(
            start_link(backend.as_ref(), workspace.as_ref(), node.as_ref()).await,
        ),
        Command::PollLink => ServiceEvent::LinkPolled(poll_link(backend.as_ref()).await),
        Command::ApproveLink {
            challenge,
            response,
        } => ServiceEvent::AccountActionFinished(
            approve_link(
                backend.as_ref(),
                workspace.as_ref(),
                node.as_ref(),
                challenge,
                response,
            )
            .await,
        ),
        Command::CancelLink => ServiceEvent::AccountActionFinished(cancel_link(backend).await),
        Command::ResolveLinkChallenge { input } => ServiceEvent::ResponderChallengeResolved(
            resolve_link_challenge(backend.as_ref(), input).await,
        ),
        Command::GenerateLinkResponse { session, label } => {
            ServiceEvent::ResponderResponseGenerated(
                generate_link_response(backend.as_ref(), session, label).await,
            )
        }
        Command::StartPhoneEnrollment => ServiceEvent::PhoneEnrollmentStarted(
            start_phone(backend.as_ref(), workspace.as_ref(), node.as_ref()).await,
        ),
        Command::PollPhoneEnrollment => {
            ServiceEvent::PhoneEnrollmentPolled(poll_phone(backend.as_ref()).await)
        }
        Command::ApprovePhoneEnrollment {
            enrollment,
            candidate,
            label,
        } => ServiceEvent::AccountActionFinished(
            approve_phone(
                backend.as_ref(),
                workspace.as_ref(),
                node.as_ref(),
                enrollment,
                candidate,
                label,
            )
            .await,
        ),
        Command::CancelPhoneEnrollment => {
            ServiceEvent::AccountActionFinished(cancel_phone(backend).await)
        }
        Command::RemoveMember(key) => ServiceEvent::AccountActionFinished(
            remove_member(backend.as_ref(), workspace.as_ref(), node.as_ref(), key).await,
        ),
        Command::UnbindNode(key) => ServiceEvent::AccountActionFinished(
            unbind_node(backend.as_ref(), workspace.as_ref(), node.as_ref(), key).await,
        ),
        Command::SetNodeLabel { key, label } => ServiceEvent::AccountActionFinished(
            set_node_label(workspace.as_ref(), node.as_ref(), key, label).await,
        ),
        Command::EnrollTouchId(password) => {
            ServiceEvent::AccountActionFinished(enroll_touch_id(backend.as_ref(), password).await)
        }
        Command::DisableTouchId => {
            ServiceEvent::AccountActionFinished(disable_touch_id(backend.as_ref()).await)
        }
        _ => ServiceEvent::AccountActionFinished(Err(
            "unsupported account operation reached the account service".into(),
        )),
    }
}

/// Bind the active node only after this device's member key already resolves
/// to an account. This is the safe post-link retry: a still-pending link never
/// founds a duplicate account with nonce zero.
pub async fn bind_member_node(
    backend: &Backend,
    workspace: &Workspace,
    node: &NodeClient,
    account: &Value,
) -> Result<bool, String> {
    let facts = parse_account(
        account
            .as_object()
            .ok_or_else(|| "node returned an invalid identity account".to_string())?,
    )?;
    if facts
        .nodes
        .iter()
        .any(|key| key.eq_ignore_ascii_case(&workspace.pubkey))
    {
        return Ok(false);
    }
    let message = backend
        .sign_bind(BindRequest {
            chain_id: workspace.chain_id.clone(),
            node_pubkey: workspace.pubkey.clone(),
            nonce: facts.nonce,
        })
        .await
        .map_err(actionable)?;
    submit_identity(Some(node), message).await.map(|()| true)
}

pub async fn complete_pending_bind(
    backend: &Backend,
    workspace: &Workspace,
    node: &NodeClient,
    member_key: &str,
    account: &Value,
) -> Result<bool, String> {
    let Some(pending) = backend.link_pending().await? else {
        return Ok(false);
    };
    let facts = parse_account(
        account
            .as_object()
            .ok_or_else(|| "node returned an invalid identity account".to_string())?,
    )?;
    if !pending_matches(&pending, workspace, member_key, &facts) {
        return Ok(false);
    }
    let changed = bind_member_node(backend, workspace, node, account).await?;
    backend.link_pending_clear().await?;
    Ok(changed)
}

/// Best-effort desktop-connect account binding, matching the legacy shell:
/// an existing node binding wins, an existing member account supplies its
/// current nonce, and a fresh key founds with nonce zero unless a device-link
/// response is still waiting to land.
pub async fn auto_bind_on_connect(
    backend: Backend,
    workspace: Workspace,
    node: NodeClient,
) -> Result<bool, String> {
    let identity = backend.identity_state().await?;
    if !matches!(
        identity.state,
        IdentityStatus::Plaintext | IdentityStatus::Unlocked
    ) {
        return Ok(false);
    }
    let member_key = identity
        .pubkey
        .ok_or_else(|| "readable identity has no public key".to_string())?;
    let member_bytes = decode_hex_exact(&member_key, 32, "local member key")?;
    let node_bytes = decode_hex_exact(&workspace.pubkey, 32, "active node key")?;
    let pending = backend.link_pending().await?;

    if let Some(bound) =
        query_account(&node, json!({ "of_node": { "node_key": node_bytes } })).await?
    {
        if pending
            .as_ref()
            .is_some_and(|pending| pending_matches(pending, &workspace, &member_key, &bound))
        {
            backend.link_pending_clear().await?;
        }
        crate::profile_service::reconcile_best_effort(&backend, &workspace, &node).await;
        return Ok(false);
    }

    let member = query_account(
        &node,
        json!({ "of_member": { "member_key": member_bytes } }),
    )
    .await?;
    if member.is_none()
        && pending.as_ref().is_some_and(|pending| {
            pending.chain_id == workspace.chain_id
                && pending.member_key.eq_ignore_ascii_case(&member_key)
        })
    {
        return Ok(false);
    }
    let nonce = member.as_ref().map_or(0, |account| account.nonce);
    let message = backend
        .sign_bind(BindRequest {
            chain_id: workspace.chain_id.clone(),
            node_pubkey: workspace.pubkey.clone(),
            nonce,
        })
        .await
        .map_err(actionable)?;
    submit_identity(Some(&node), message).await?;
    if let Some(account) = member.as_ref()
        && pending
            .as_ref()
            .is_some_and(|pending| pending_matches(pending, &workspace, &member_key, account))
    {
        backend.link_pending_clear().await?;
    }
    crate::profile_service::reconcile_best_effort(&backend, &workspace, &node).await;
    Ok(true)
}

pub(crate) async fn query_account(
    node: &NodeClient,
    query: Value,
) -> Result<Option<AccountFacts>, String> {
    let reply = node
        .query("identity", query)
        .await
        .map_err(|error| error.to_string())?;
    match reply.get("account") {
        Some(Value::Null) | None => Ok(None),
        Some(Value::Object(account)) => parse_account(account).map(Some),
        Some(_) => Err("node returned an invalid identity account".into()),
    }
}

fn pending_matches(
    pending: &LinkPending,
    workspace: &Workspace,
    member_key: &str,
    account: &AccountFacts,
) -> bool {
    pending.chain_id == workspace.chain_id
        && pending.member_key.eq_ignore_ascii_case(member_key)
        && pending.account_id.eq_ignore_ascii_case(&account.account_id)
}

async fn resolve_link_challenge(
    backend: Option<&Backend>,
    input: String,
) -> Result<LinkResponderSession, String> {
    let backend = require_backend(backend)?;
    let input = input.trim();
    if input.is_empty() || input.len() > 4 * 1024 {
        return Err("paste the link address or challenge code from your other device".into());
    }
    let (challenge, relay_url) = if input.starts_with("http://") {
        let address = LinkAddress::parse(input.to_string())?;
        let challenge = backend.link_fetch_challenge(address.clone()).await?;
        (challenge, Some(address.as_str().to_string()))
    } else {
        (decode_link_challenge(input)?, None)
    };
    Ok(LinkResponderSession {
        challenge: challenge_view(challenge.clone()),
        challenge_code: encode_link_challenge(&challenge)?,
        relay_url,
    })
}

async fn generate_link_response(
    backend: Option<&Backend>,
    session: LinkResponderSession,
    label: Option<String>,
) -> Result<LinkResponderReply, String> {
    let backend = require_backend(backend)?;
    let label = label
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if let Some(label) = label.as_deref() {
        validate_label(label)?;
    }
    let identity = backend.identity_state().await?;
    let pubkey = identity
        .pubkey
        .ok_or_else(|| "create or restore an identity before linking this device".to_string())?;
    decode_hex_exact(&pubkey, 32, "local identity key")?;
    let challenge = challenge_from_view(&session.challenge)?;
    if encode_link_challenge(&challenge)? != session.challenge_code {
        return Err("the link challenge changed — paste it again".into());
    }
    let possession = backend
        .sign_possession(PossessionRequest {
            chain_id: challenge.chain_id.clone(),
            account_id: challenge.account_id.clone(),
            nonce: challenge.nonce,
        })
        .await
        .map_err(actionable)?;
    let response = LinkResponse {
        pubkey: pubkey.clone(),
        kind: MemberKeyKind::Ed25519,
        possession,
        label,
    };
    let response_code = encode_link_response(&response)?;
    backend
        .link_pending_mark(LinkPending {
            chain_id: challenge.chain_id,
            account_id: challenge.account_id,
            member_key: pubkey.clone(),
        })
        .await?;
    let sent_automatically = if let Some(url) = session.relay_url {
        match backend
            .link_send_response(LinkAddress::parse(url)?, response)
            .await
        {
            Ok(()) => true,
            Err(error) => {
                tracing::debug!(
                    target: "ducktape::account",
                    event = "link_response_delivery_failed",
                    reason = "lan_reply_failed",
                    detail = %error,
                    "device link response is using the manual fallback"
                );
                false
            }
        }
    } else {
        false
    };
    Ok(LinkResponderReply {
        response_code,
        key: pubkey,
        sent_automatically,
    })
}

fn challenge_from_view(view: &LinkChallengeView) -> Result<LinkChallenge, String> {
    let challenge = LinkChallenge {
        chain_id: view.chain_id.clone(),
        account_id: view.account_id.clone(),
        nonce: view.nonce,
        name: view.name.clone(),
    };
    encode_link_challenge(&challenge)?;
    Ok(challenge)
}

async fn enroll_touch_id(backend: Option<&Backend>, password: String) -> Result<(), String> {
    require_backend(backend)?
        .touch_id_enroll(password)
        .await
        .map_err(actionable)
}

async fn disable_touch_id(backend: Option<&Backend>) -> Result<(), String> {
    require_backend(backend)?
        .touch_id_disable()
        .await
        .map_err(actionable)
}

async fn start_link(
    backend: Option<&Backend>,
    workspace: Option<&Workspace>,
    node: Option<&NodeClient>,
) -> Result<LinkSession, String> {
    let backend = require_backend(backend)?;
    let workspace = require_workspace(workspace)?;
    backend.phone_enrollment_cancel().await?;
    let facts = own_account(node, workspace).await?;
    let challenge = LinkChallenge {
        chain_id: workspace.chain_id.clone(),
        account_id: facts.account_id,
        nonce: facts.nonce,
        name: facts.display_name,
    };
    let challenge_code = encode_link_challenge(&challenge)?;
    let relay_url = match backend.link_relay_start(challenge.clone()).await {
        Ok(started) => Some(started.url.as_str().to_string()),
        Err(error) => {
            tracing::debug!(
                target: "ducktape::account",
                event = "link_relay_unavailable",
                reason = "lan_bind_failed",
                detail = %error,
                "device link is using the manual challenge path"
            );
            None
        }
    };
    Ok(LinkSession {
        challenge: challenge_view(challenge),
        challenge_code,
        relay_url,
    })
}

async fn poll_link(backend: Option<&Backend>) -> Result<Option<LinkReplyPreview>, String> {
    let response = require_backend(backend)?.link_relay_poll().await?;
    response
        .map(|response| {
            let code = encode_link_response(&response)?;
            Ok(LinkReplyPreview {
                response_code: code,
                key: response.pubkey,
                kind: account_kind(response.kind),
                label: response.label,
            })
        })
        .transpose()
}

async fn approve_link(
    backend: Option<&Backend>,
    workspace: Option<&Workspace>,
    node: Option<&NodeClient>,
    challenge: LinkChallengeView,
    response_code: String,
) -> Result<(), String> {
    let backend = require_backend(backend)?;
    let workspace = require_workspace(workspace)?;
    if workspace.chain_id != challenge.chain_id {
        return Err("the active network changed — restart the device link".into());
    }
    let response = decode_link_response(&response_code).map_err(|_| {
        "that doesn't look like a link response code — paste the code from the new device"
            .to_string()
    })?;
    let facts = own_account(node, workspace).await?;
    require_pinned_account(&facts, &challenge.account_id, challenge.nonce, "link code")?;
    let message = backend
        .sign_add_member(AddMemberRequest {
            chain_id: challenge.chain_id,
            account_id: challenge.account_id,
            new_pubkey: response.pubkey,
            new_kind: response.kind,
            nonce: challenge.nonce,
            possession: response.possession,
            label: response.label,
        })
        .await
        .map_err(actionable)?;
    submit_identity(node, message).await?;
    backend.link_relay_cancel().await
}

async fn cancel_link(backend: Option<Backend>) -> Result<(), String> {
    match backend {
        Some(backend) => backend.link_relay_cancel().await,
        None => Ok(()),
    }
}

async fn start_phone(
    backend: Option<&Backend>,
    workspace: Option<&Workspace>,
    node: Option<&NodeClient>,
) -> Result<PhoneEnrollmentView, String> {
    let backend = require_backend(backend)?;
    let workspace = require_workspace(workspace)?;
    backend.link_relay_cancel().await?;
    let facts = own_account(node, workspace).await?;
    let started = backend
        .phone_enrollment_start(
            workspace.chain_id.clone(),
            facts.account_id.clone(),
            facts.nonce,
        )
        .await?;
    Ok(PhoneEnrollmentView {
        url: started.url,
        chain_id: workspace.chain_id.clone(),
        account_id: facts.account_id,
        nonce: facts.nonce,
    })
}

async fn poll_phone(backend: Option<&Backend>) -> Result<Option<PhoneCandidateView>, String> {
    Ok(require_backend(backend)?
        .phone_enrollment_poll()
        .await?
        .map(|candidate| PhoneCandidateView {
            key: candidate.new_key,
            signature: candidate.signature,
        }))
}

async fn approve_phone(
    backend: Option<&Backend>,
    workspace: Option<&Workspace>,
    node: Option<&NodeClient>,
    enrollment: PhoneEnrollmentView,
    candidate: PhoneCandidateView,
    label: Option<String>,
) -> Result<(), String> {
    let backend = require_backend(backend)?;
    let workspace = require_workspace(workspace)?;
    if workspace.chain_id != enrollment.chain_id {
        return Err("the active network changed — restart phone enrollment".into());
    }
    validate_phone_candidate(&candidate)?;
    if let Some(label) = label.as_deref() {
        validate_label(label)?;
    }
    let facts = own_account(node, workspace).await?;
    require_pinned_account(
        &facts,
        &enrollment.account_id,
        enrollment.nonce,
        "enrollment QR",
    )?;
    let signature = decode_hex_exact(&candidate.signature, 64, "phone signature")?;
    let possession = json!({ "signature": { "sig": signature } }).to_string();
    let message = backend
        .sign_add_member(AddMemberRequest {
            chain_id: enrollment.chain_id,
            account_id: enrollment.account_id,
            new_pubkey: candidate.key,
            new_kind: MemberKeyKind::P256,
            nonce: enrollment.nonce,
            possession,
            label,
        })
        .await
        .map_err(actionable)?;
    submit_identity(node, message).await?;
    backend.phone_enrollment_cancel().await
}

async fn cancel_phone(backend: Option<Backend>) -> Result<(), String> {
    match backend {
        Some(backend) => backend.phone_enrollment_cancel().await,
        None => Ok(()),
    }
}

async fn remove_member(
    backend: Option<&Backend>,
    workspace: Option<&Workspace>,
    node: Option<&NodeClient>,
    target: String,
) -> Result<(), String> {
    let backend = require_backend(backend)?;
    let workspace = require_workspace(workspace)?;
    let facts = own_account(node, workspace).await?;
    let target = target.to_ascii_lowercase();
    if facts.member_keys.len() <= 1 {
        return Err("the last remaining account key cannot be removed".into());
    }
    if !facts.member_keys.iter().any(|(key, _)| key == &target) {
        return Err("that key is no longer a member of this account".into());
    }
    let message = backend
        .sign_remove_member(RemoveMemberRequest {
            chain_id: workspace.chain_id.clone(),
            account_id: facts.account_id,
            target_pubkey: target,
            nonce: facts.nonce,
        })
        .await
        .map_err(actionable)?;
    submit_identity(node, message).await
}

async fn unbind_node(
    backend: Option<&Backend>,
    workspace: Option<&Workspace>,
    node: Option<&NodeClient>,
    target: String,
) -> Result<(), String> {
    let backend = require_backend(backend)?;
    let workspace = require_workspace(workspace)?;
    let facts = own_account(node, workspace).await?;
    let target = target.to_ascii_lowercase();
    if !facts.nodes.contains(&target) {
        return Err("that node is no longer bound to this account on this network".into());
    }
    let message = backend
        .sign_unbind(BindRequest {
            chain_id: workspace.chain_id.clone(),
            node_pubkey: target,
            nonce: facts.nonce,
        })
        .await
        .map_err(actionable)?;
    submit_identity(node, message).await
}

async fn set_node_label(
    workspace: Option<&Workspace>,
    node: Option<&NodeClient>,
    target: String,
    label: Option<String>,
) -> Result<(), String> {
    let workspace = require_workspace(workspace)?;
    let client = node.ok_or_else(|| "enter a network to label account devices".to_string())?;
    let facts = own_account(Some(client), workspace).await?;
    let target = target.to_ascii_lowercase();
    if !facts.nodes.contains(&target) {
        return Err("that node is no longer bound to this account on this network".into());
    }
    let label = label
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if let Some(label) = label.as_deref() {
        validate_label(label)?;
    }
    client
        .submit(
            "identity",
            json!({
                "set_node_label": {
                    "node_key": decode_hex_exact(&target, 32, "bound node key")?,
                    "label": label,
                }
            }),
            Some(&workspace.pubkey),
        )
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

async fn own_account(
    node: Option<&NodeClient>,
    workspace: &Workspace,
) -> Result<AccountFacts, String> {
    let client = node.ok_or_else(|| "enter a network to manage account devices".to_string())?;
    let node_key = decode_hex_exact(&workspace.pubkey, 32, "active node key")?;
    let reply = client
        .query("identity", json!({ "of_node": { "node_key": node_key } }))
        .await
        .map_err(|error| error.to_string())?;
    let account = reply
        .get("account")
        .and_then(Value::as_object)
        .ok_or_else(|| "this node isn't linked to an account yet".to_string())?;
    parse_account(account)
}

fn parse_account(account: &serde_json::Map<String, Value>) -> Result<AccountFacts, String> {
    let account_id = wire_key(account.get("account_id"), &[32], "account id")?;
    let display_name = match account.get("display_name") {
        Some(Value::String(value)) if value.len() <= 64 && !value.chars().any(char::is_control) => {
            Some(value.clone())
        }
        Some(Value::Null) | None => None,
        _ => return Err("node returned an invalid account display name".into()),
    };
    let avatar = optional_text(account.get("avatar"), 512, "account avatar")?;
    let bio = optional_text(account.get("bio"), 280, "account bio")?;
    let nonce = account
        .get("nonce")
        .and_then(Value::as_u64)
        .ok_or_else(|| "node returned an invalid account nonce".to_string())?;
    let member_keys = account
        .get("member_keys")
        .and_then(Value::as_array)
        .ok_or_else(|| "node returned an invalid member-key list".to_string())?;
    if member_keys.is_empty() || member_keys.len() > 256 {
        return Err("node returned an invalid member-key list".into());
    }
    let member_keys = member_keys
        .iter()
        .map(|value| {
            let value = value
                .as_object()
                .ok_or_else(|| "node returned a malformed member key".to_string())?;
            let kind = match value.get("kind").and_then(Value::as_str) {
                Some("ed25519") => MemberKeyKind::Ed25519,
                Some("p256") => MemberKeyKind::P256,
                Some("webauthn_p256") => MemberKeyKind::WebauthnP256,
                _ => return Err("node returned an unsupported member-key kind".into()),
            };
            let lengths: &[usize] = match kind {
                MemberKeyKind::Ed25519 => &[32],
                MemberKeyKind::P256 | MemberKeyKind::WebauthnP256 => &[33, 65],
            };
            Ok((wire_key(value.get("pubkey"), lengths, "member key")?, kind))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let nodes = account
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| "node returned an invalid bound-node list".to_string())?;
    if nodes.len() > 256 {
        return Err("node returned too many bound nodes".into());
    }
    let nodes = nodes
        .iter()
        .map(|value| {
            let value = value
                .as_object()
                .ok_or_else(|| "node returned a malformed bound node".to_string())?;
            wire_key(value.get("node_key"), &[32], "bound node key")
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(AccountFacts {
        account_id,
        display_name,
        avatar,
        bio,
        nonce,
        member_keys,
        nodes,
    })
}

fn optional_text(value: Option<&Value>, max: usize, field: &str) -> Result<Option<String>, String> {
    match value {
        Some(Value::String(value)) if !value.is_empty() && value.len() <= max => {
            Ok(Some(value.clone()))
        }
        Some(Value::Null) | None => Ok(None),
        _ => Err(format!("node returned an invalid {field}")),
    }
}

async fn submit_identity(node: Option<&NodeClient>, message: String) -> Result<(), String> {
    let client = node.ok_or_else(|| "enter a network to manage account devices".to_string())?;
    let value = parse_signed_identity_message(&message)?;
    client
        .submit("identity", value, None)
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn parse_signed_identity_message(message: &str) -> Result<Value, String> {
    if message.is_empty() || message.len() > MAX_IDENTITY_MESSAGE_BYTES {
        return Err("the identity signer returned an invalid message size".into());
    }
    let value: Value = serde_json::from_str(message)
        .map_err(|_| "the identity signer returned malformed JSON".to_string())?;
    let object = value
        .as_object()
        .ok_or_else(|| "the identity signer returned a malformed message".to_string())?;
    if object.len() != 1
        || !object.keys().all(|key| {
            matches!(
                key.as_str(),
                "add_member_key" | "bind_node" | "remove_member_key" | "unbind_node"
            )
        })
    {
        return Err("the identity signer returned an unexpected operation".into());
    }
    Ok(value)
}

fn require_pinned_account(
    facts: &AccountFacts,
    account_id: &str,
    nonce: u64,
    ceremony: &str,
) -> Result<(), String> {
    if !facts.account_id.eq_ignore_ascii_case(account_id) || facts.nonce != nonce {
        Err(format!(
            "the account changed since this {ceremony} was made — restart the ceremony"
        ))
    } else {
        Ok(())
    }
}

fn challenge_view(challenge: LinkChallenge) -> LinkChallengeView {
    LinkChallengeView {
        chain_id: challenge.chain_id,
        account_id: challenge.account_id,
        nonce: challenge.nonce,
        name: challenge.name,
    }
}

fn account_kind(kind: MemberKeyKind) -> AccountKeyKind {
    match kind {
        MemberKeyKind::Ed25519 => AccountKeyKind::Ed25519,
        MemberKeyKind::P256 => AccountKeyKind::P256,
        MemberKeyKind::WebauthnP256 => AccountKeyKind::WebauthnP256,
    }
}

fn wire_key(value: Option<&Value>, lengths: &[usize], field: &str) -> Result<String, String> {
    let bytes = value
        .and_then(Value::as_array)
        .ok_or_else(|| format!("node returned an invalid {field}"))?;
    if !lengths.contains(&bytes.len()) {
        return Err(format!("node returned an invalid {field}"));
    }
    bytes
        .iter()
        .map(|value| {
            value
                .as_u64()
                .filter(|value| *value <= 255)
                .map(|value| value as u8)
        })
        .collect::<Option<Vec<_>>>()
        .map(|bytes| hex(&bytes))
        .ok_or_else(|| format!("node returned an invalid {field}"))
}

fn validate_phone_candidate(candidate: &PhoneCandidateView) -> Result<(), String> {
    let key = decode_hex_exact(&candidate.key, 33, "phone key")?;
    if !matches!(key.first(), Some(2 | 3)) {
        return Err("phone key is not a compressed P-256 key".into());
    }
    decode_hex_exact(&candidate.signature, 64, "phone signature").map(|_| ())
}

fn validate_label(value: &str) -> Result<(), String> {
    if value.len() > MAX_LABEL_BYTES || value.chars().any(char::is_control) {
        Err("key label is too long or contains controls".into())
    } else {
        Ok(())
    }
}

fn decode_hex_exact(value: &str, len: usize, field: &str) -> Result<Vec<u8>, String> {
    if value.len() != len * 2 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("{field} is not a {len}-byte hexadecimal value"));
    }
    (0..value.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&value[index..index + 2], 16).map_err(|_| format!("invalid {field}"))
        })
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn require_backend(backend: Option<&Backend>) -> Result<&Backend, String> {
    backend.ok_or_else(|| "desktop identity backend is unavailable".into())
}

fn require_workspace(workspace: Option<&Workspace>) -> Result<&Workspace, String> {
    workspace.ok_or_else(|| "enter a network to manage account devices".into())
}

fn actionable(error: String) -> String {
    if error == "identity-locked" {
        "your account is locked on this device — unlock it first, then retry".into()
    } else {
        error
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_wire_requires_exact_key_shapes() {
        let account = json!({
            "account_id": vec![1; 32],
            "display_name": "Kim",
            "nonce": 7,
            "member_keys": [{ "pubkey": vec![2; 32], "kind": "ed25519", "label": null, "added_at": 1 }],
            "nodes": [{ "node_key": vec![3; 32], "label": null }]
        });
        let parsed = parse_account(account.as_object().unwrap()).unwrap();
        assert_eq!(parsed.nonce, 7);
        assert_eq!(parsed.member_keys.len(), 1);
        let mut malformed = account;
        malformed["member_keys"][0]["pubkey"] = json!(vec![2; 31]);
        assert!(parse_account(malformed.as_object().unwrap()).is_err());
    }

    #[test]
    fn phone_candidate_requires_compressed_key_and_raw_signature() {
        let valid = PhoneCandidateView {
            key: format!("02{}", "11".repeat(32)),
            signature: "22".repeat(64),
        };
        assert!(validate_phone_candidate(&valid).is_ok());
        let mut invalid = valid;
        invalid.key.replace_range(..2, "04");
        assert!(validate_phone_candidate(&invalid).is_err());
    }

    #[test]
    fn only_expected_signed_identity_operations_can_be_submitted() {
        assert!(parse_signed_identity_message(r#"{"add_member_key":{"authorizer":{}}}"#).is_ok());
        assert!(parse_signed_identity_message(r#"{"bind_node":{"authorizer":{}}}"#).is_ok());
        assert!(
            parse_signed_identity_message(r#"{"remove_member_key":{"authorizer":{}}}"#).is_ok()
        );
        assert!(parse_signed_identity_message(r#"{"unbind_node":{"authorizer":{}}}"#).is_ok());
        assert!(
            parse_signed_identity_message(r#"{"set_account_name":{"display_name":"x"}}"#).is_err()
        );
        assert!(
            parse_signed_identity_message(r#"{"unbind_node":{},"remove_member_key":{}}"#).is_err()
        );
    }

    #[test]
    fn ceremonies_fail_closed_on_account_or_nonce_drift() {
        let facts = AccountFacts {
            account_id: "11".repeat(32),
            display_name: None,
            avatar: None,
            bio: None,
            nonce: 7,
            member_keys: vec![("22".repeat(32), MemberKeyKind::Ed25519)],
            nodes: vec![],
        };
        assert!(require_pinned_account(&facts, &"11".repeat(32), 7, "code").is_ok());
        assert!(require_pinned_account(&facts, &"33".repeat(32), 7, "code").is_err());
        assert!(require_pinned_account(&facts, &"11".repeat(32), 8, "code").is_err());
    }

    #[test]
    fn pending_bind_is_scoped_to_member_account_and_network() {
        let facts = AccountFacts {
            account_id: "11".repeat(32),
            display_name: None,
            avatar: None,
            bio: None,
            nonce: 7,
            member_keys: vec![("22".repeat(32), MemberKeyKind::Ed25519)],
            nodes: vec![],
        };
        let workspace = Workspace {
            id: "a".into(),
            name: "A".into(),
            chain_id: "chain-a".into(),
            pubkey: "33".repeat(32),
            founder: false,
            member: true,
            ports: crate::backend::WorkspacePorts {
                listen: 1,
                http: 2,
                rpc: 3,
                wireguard: None,
                invite: None,
            },
        };
        let mut pending = LinkPending {
            chain_id: "chain-a".into(),
            account_id: "11".repeat(32),
            member_key: "22".repeat(32),
        };
        assert!(pending_matches(
            &pending,
            &workspace,
            &"22".repeat(32),
            &facts
        ));
        pending.account_id = "44".repeat(32);
        assert!(!pending_matches(
            &pending,
            &workspace,
            &"22".repeat(32),
            &facts
        ));
    }
}
