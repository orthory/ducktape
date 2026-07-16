//! Native account onboarding.
//!
//! This module deliberately has no backend dependency. [`update`] returns a
//! typed [`Command`] for the host to execute; the host feeds the result back as
//! a [`ServiceEvent`]. This keeps account custody and transport outside the UI.

use iced::widget::{
    Column, Space, TextInput, button, column, container, row, text, text_input,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Shadow, Vector};
use zeroize::Zeroize as _;

use crate::icons::{self, Icon};
use crate::theme::{
    self, MONO, Palette, RADIUS_LG, RADIUS_MD, RADIUS_SM, SANS, SANS_MEDIUM, SANS_SEMIBOLD,
};

const CARD_WIDTH: f32 = 440.0;
const MIN_PASSWORD_LEN: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityKind {
    Absent,
    Plaintext,
    Locked,
    Unlocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdentityReport {
    pub kind: IdentityKind,
    pub mnemonic_confirmed: bool,
    pub touch_id_available: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryMode {
    Create,
    TouchId,
    Restore,
    LinkDevice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Loading,
    LoadError,
    Create,
    TouchIdCreate,
    RecoveryPhrase,
    ConfirmRecovery,
    Restore,
    LinkPassword,
    LinkChallenge,
    LinkResponse,
    SecureLegacy,
    SecureLegacyPassword,
    RevealLegacy,
    ResumeRecovery,
    Unlock,
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountAction {
    Unlock,
    Secure,
    RevealRecovery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecoveryPurpose {
    Create,
    Resume,
    Legacy,
}

#[derive(Clone, PartialEq, Eq)]
pub struct CreatedIdentity {
    pub mnemonic: String,
}

impl CreatedIdentity {
    fn take_mnemonic(mut self) -> String {
        std::mem::take(&mut self.mnemonic)
    }
}

impl std::fmt::Debug for CreatedIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("CreatedIdentity { mnemonic: [REDACTED] }")
    }
}

impl Drop for CreatedIdentity {
    fn drop(&mut self) {
        self.mnemonic.zeroize();
    }
}

/// A secret while it is in flight from the UI reducer to the native backend.
///
/// Taking the inner string transfers the one allocation to the backend's own
/// zeroizing wrapper. Dropping a canceled command scrubs it here instead.
#[derive(Clone, PartialEq, Eq)]
pub struct CommandSecret(String);

impl CommandSecret {
    pub(crate) fn into_string(mut self) -> String {
        std::mem::take(&mut self.0)
    }
}

impl From<String> for CommandSecret {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for CommandSecret {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl std::ops::Deref for CommandSecret {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Debug for CommandSecret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("CommandSecret([REDACTED])")
    }
}

impl Drop for CommandSecret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct LinkReply {
    pub response: String,
    pub account_name: Option<String>,
    pub device_key: Option<String>,
    pub sent_automatically: bool,
}

impl std::fmt::Debug for LinkReply {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LinkReply")
            .field("response", &"[REDACTED]")
            .field("account_name", &self.account_name)
            .field("device_key", &self.device_key)
            .field("sent_automatically", &self.sent_automatically)
            .finish()
    }
}

impl Drop for LinkReply {
    fn drop(&mut self) {
        self.response.zeroize();
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum Command {
    LoadIdentity,
    CreateIdentity {
        password: CommandSecret,
        display_name: Option<String>,
    },
    CreateIdentityWithTouchId {
        display_name: Option<String>,
    },
    ConfirmMnemonic,
    RestoreIdentity {
        mnemonic: CommandSecret,
        password: CommandSecret,
    },
    PrepareLinkIdentity {
        password: CommandSecret,
    },
    GenerateLinkResponse {
        challenge: CommandSecret,
        device_label: Option<String>,
    },
    UnlockIdentity {
        password: CommandSecret,
    },
    UnlockWithTouchId,
    EnrollTouchIdSession,
    EncryptLegacy {
        password: CommandSecret,
    },
    RevealMnemonic {
        password: CommandSecret,
    },
    CopyText(CommandSecret),
    GateCompleted,
    GateSkipped,
}

impl std::fmt::Debug for Command {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::LoadIdentity => "LoadIdentity",
            Self::CreateIdentity { .. } => "CreateIdentity([REDACTED])",
            Self::CreateIdentityWithTouchId { .. } => "CreateIdentityWithTouchId",
            Self::ConfirmMnemonic => "ConfirmMnemonic",
            Self::RestoreIdentity { .. } => "RestoreIdentity([REDACTED])",
            Self::PrepareLinkIdentity { .. } => "PrepareLinkIdentity([REDACTED])",
            Self::GenerateLinkResponse { .. } => "GenerateLinkResponse([REDACTED])",
            Self::UnlockIdentity { .. } => "UnlockIdentity([REDACTED])",
            Self::UnlockWithTouchId => "UnlockWithTouchId",
            Self::EnrollTouchIdSession => "EnrollTouchIdSession",
            Self::EncryptLegacy { .. } => "EncryptLegacy([REDACTED])",
            Self::RevealMnemonic { .. } => "RevealMnemonic([REDACTED])",
            Self::CopyText(_) => "CopyText([REDACTED])",
            Self::GateCompleted => "GateCompleted",
            Self::GateSkipped => "GateSkipped",
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ServiceEvent {
    IdentityLoaded(Result<IdentityReport, String>),
    IdentityCreated(Result<CreatedIdentity, String>),
    MnemonicConfirmed(Result<(), String>),
    IdentityRestored(Result<(), String>),
    LinkIdentityPrepared(Result<(), String>),
    LinkResponseGenerated(Result<LinkReply, String>),
    IdentityUnlocked(Result<(), String>),
    TouchIdUnlocked(Result<(), String>),
    TouchIdEnrolled(Result<(), String>),
    LegacyEncrypted(Result<(), String>),
    MnemonicRevealed(Result<String, String>),
    TextCopied(Result<(), String>),
}

impl std::fmt::Debug for ServiceEvent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::IdentityLoaded(Ok(_)) => "IdentityLoaded(Ok)",
            Self::IdentityLoaded(Err(_)) => "IdentityLoaded(Err)",
            Self::IdentityCreated(Ok(_)) => "IdentityCreated(Ok([REDACTED]))",
            Self::IdentityCreated(Err(_)) => "IdentityCreated(Err)",
            Self::MnemonicConfirmed(Ok(_)) => "MnemonicConfirmed(Ok)",
            Self::MnemonicConfirmed(Err(_)) => "MnemonicConfirmed(Err)",
            Self::IdentityRestored(Ok(_)) => "IdentityRestored(Ok)",
            Self::IdentityRestored(Err(_)) => "IdentityRestored(Err)",
            Self::LinkIdentityPrepared(Ok(_)) => "LinkIdentityPrepared(Ok)",
            Self::LinkIdentityPrepared(Err(_)) => "LinkIdentityPrepared(Err)",
            Self::LinkResponseGenerated(Ok(_)) => "LinkResponseGenerated(Ok([REDACTED]))",
            Self::LinkResponseGenerated(Err(_)) => "LinkResponseGenerated(Err)",
            Self::IdentityUnlocked(Ok(_)) => "IdentityUnlocked(Ok)",
            Self::IdentityUnlocked(Err(_)) => "IdentityUnlocked(Err)",
            Self::TouchIdUnlocked(Ok(_)) => "TouchIdUnlocked(Ok)",
            Self::TouchIdUnlocked(Err(_)) => "TouchIdUnlocked(Err)",
            Self::TouchIdEnrolled(Ok(_)) => "TouchIdEnrolled(Ok)",
            Self::TouchIdEnrolled(Err(_)) => "TouchIdEnrolled(Err)",
            Self::LegacyEncrypted(Ok(_)) => "LegacyEncrypted(Ok)",
            Self::LegacyEncrypted(Err(_)) => "LegacyEncrypted(Err)",
            Self::MnemonicRevealed(Ok(_)) => "MnemonicRevealed(Ok([REDACTED]))",
            Self::MnemonicRevealed(Err(_)) => "MnemonicRevealed(Err)",
            Self::TextCopied(Ok(_)) => "TextCopied(Ok)",
            Self::TextCopied(Err(_)) => "TextCopied(Err)",
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum Message {
    SelectMode(EntryMode),
    DisplayNameChanged(String),
    PasswordChanged(String),
    ConfirmPasswordChanged(String),
    RestoreWordsChanged(String),
    ConfirmWordChanged(usize, String),
    LinkChallengeChanged(String),
    DeviceLabelChanged(String),
    Submit,
    ContinueRecovery,
    SecureLegacy,
    CopyRecovery,
    CopyLinkResponse,
    UseTouchId,
    FinishLater,
    Skip,
    Retry,
    Service(ServiceEvent),
}

impl std::fmt::Debug for Message {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::SelectMode(_) => "SelectMode",
            Self::DisplayNameChanged(_) => "DisplayNameChanged",
            Self::PasswordChanged(_) => "PasswordChanged([REDACTED])",
            Self::ConfirmPasswordChanged(_) => "ConfirmPasswordChanged([REDACTED])",
            Self::RestoreWordsChanged(_) => "RestoreWordsChanged([REDACTED])",
            Self::ConfirmWordChanged(_, _) => "ConfirmWordChanged([REDACTED])",
            Self::LinkChallengeChanged(_) => "LinkChallengeChanged([REDACTED])",
            Self::DeviceLabelChanged(_) => "DeviceLabelChanged",
            Self::Submit => "Submit",
            Self::ContinueRecovery => "ContinueRecovery",
            Self::SecureLegacy => "SecureLegacy",
            Self::CopyRecovery => "CopyRecovery",
            Self::CopyLinkResponse => "CopyLinkResponse",
            Self::UseTouchId => "UseTouchId",
            Self::FinishLater => "FinishLater",
            Self::Skip => "Skip",
            Self::Retry => "Retry",
            Self::Service(_) => "Service([REDACTED])",
        })
    }
}

#[derive(Clone)]
pub struct State {
    pub stage: Stage,
    pub mode: EntryMode,
    pub busy: bool,
    pub error: Option<String>,
    pub display_name: String,
    pub password: String,
    pub confirm_password: String,
    pub restore_words: String,
    pub link_challenge: String,
    pub device_label: String,
    pub mnemonic: String,
    pub link_reply: Option<LinkReply>,
    pub copied: bool,
    pub touch_id_available: bool,
    first_run: bool,
    recovery_purpose: RecoveryPurpose,
    confirm_indices: [usize; 3],
    confirm_answers: [String; 3],
}

impl std::fmt::Debug for State {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("State")
            .field("stage", &self.stage)
            .field("mode", &self.mode)
            .field("busy", &self.busy)
            .field("error", &self.error.as_ref().map(|_| "[PRESENT]"))
            .field("display_name", &self.display_name)
            .field("password", &"[REDACTED]")
            .field("confirm_password", &"[REDACTED]")
            .field("restore_words", &"[REDACTED]")
            .field("link_challenge", &"[REDACTED]")
            .field("device_label", &self.device_label)
            .field("mnemonic", &"[REDACTED]")
            .field("link_reply", &self.link_reply)
            .field("copied", &self.copied)
            .field("touch_id_available", &self.touch_id_available)
            .field("first_run", &self.first_run)
            .finish_non_exhaustive()
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            stage: Stage::Loading,
            mode: EntryMode::Create,
            busy: true,
            error: None,
            display_name: String::new(),
            password: String::new(),
            confirm_password: String::new(),
            restore_words: String::new(),
            link_challenge: String::new(),
            device_label: String::new(),
            mnemonic: String::new(),
            link_reply: None,
            copied: false,
            touch_id_available: false,
            first_run: false,
            recovery_purpose: RecoveryPurpose::Create,
            confirm_indices: [2, 11, 20],
            confirm_answers: std::array::from_fn(|_| String::new()),
        }
    }
}

impl State {
    pub fn new() -> (Self, Command) {
        (Self::default(), Command::LoadIdentity)
    }

    pub const fn is_ready(&self) -> bool {
        matches!(self.stage, Stage::Ready)
    }

    pub const fn shows_first_run_steps(&self) -> bool {
        self.first_run
    }
}

pub fn begin_account_action(state: &mut State, action: AccountAction) {
    clear_secrets(state);
    state.first_run = false;
    state.busy = false;
    state.error = None;
    state.stage = match action {
        AccountAction::Unlock => Stage::Unlock,
        AccountAction::Secure => Stage::SecureLegacyPassword,
        AccountAction::RevealRecovery => Stage::RevealLegacy,
    };
}

impl Drop for State {
    fn drop(&mut self) {
        clear_secrets(self);
    }
}

pub fn update(state: &mut State, message: Message) -> Option<Command> {
    match message {
        Message::SelectMode(mode) => {
            clear_secrets(state);
            state.mode = mode;
            state.stage = match mode {
                EntryMode::Create => Stage::Create,
                EntryMode::TouchId => Stage::TouchIdCreate,
                EntryMode::Restore => Stage::Restore,
                EntryMode::LinkDevice => Stage::LinkPassword,
            };
            state.busy = false;
            state.error = None;
            None
        }
        Message::DisplayNameChanged(value) => {
            state.display_name = value;
            state.error = None;
            None
        }
        Message::PasswordChanged(value) => {
            replace_secret(&mut state.password, value);
            state.error = None;
            None
        }
        Message::ConfirmPasswordChanged(value) => {
            replace_secret(&mut state.confirm_password, value);
            state.error = None;
            None
        }
        Message::RestoreWordsChanged(value) => {
            replace_secret(&mut state.restore_words, value);
            state.error = None;
            None
        }
        Message::LinkChallengeChanged(value) => {
            replace_secret(&mut state.link_challenge, value);
            state.error = None;
            None
        }
        Message::DeviceLabelChanged(value) => {
            state.device_label = value;
            state.error = None;
            None
        }
        Message::ConfirmWordChanged(slot, value) => {
            if let Some(answer) = state.confirm_answers.get_mut(slot) {
                replace_secret(answer, value);
                state.error = None;
            }
            None
        }
        Message::Submit => submit(state),
        Message::ContinueRecovery => match state.recovery_purpose {
            RecoveryPurpose::Legacy => finish(state, Command::GateCompleted),
            RecoveryPurpose::Create | RecoveryPurpose::Resume => {
                state.stage = Stage::ConfirmRecovery;
                state.error = None;
                None
            }
        },
        Message::SecureLegacy => {
            state.stage = Stage::SecureLegacyPassword;
            state.error = None;
            None
        }
        Message::CopyRecovery => {
            state.copied = false;
            Some(Command::CopyText(state.mnemonic.clone().into()))
        }
        Message::CopyLinkResponse => state.link_reply.as_ref().map(|reply| {
            state.copied = false;
            Command::CopyText(reply.response.clone().into())
        }),
        Message::UseTouchId => {
            state.busy = true;
            state.error = None;
            Some(Command::UnlockWithTouchId)
        }
        Message::FinishLater => finish(state, Command::GateCompleted),
        Message::Skip => finish(state, Command::GateSkipped),
        Message::Retry => {
            clear_secrets(state);
            state.stage = Stage::Loading;
            state.busy = true;
            state.error = None;
            Some(Command::LoadIdentity)
        }
        Message::Service(event) => service_event(state, event),
    }
}

fn submit(state: &mut State) -> Option<Command> {
    if state.busy {
        return None;
    }

    match state.stage {
        Stage::Create => {
            validate_set_password(state)?;
            state.busy = true;
            let command = Command::CreateIdentity {
                password: take_secret(&mut state.password).into(),
                display_name: nonempty(&state.display_name),
            };
            state.confirm_password.zeroize();
            Some(command)
        }
        Stage::TouchIdCreate => {
            state.busy = true;
            state.recovery_purpose = RecoveryPurpose::Create;
            Some(Command::CreateIdentityWithTouchId {
                display_name: nonempty(&state.display_name),
            })
        }
        Stage::Restore => {
            validate_set_password(state)?;
            let words = normalized_words(&state.restore_words);
            if words.split_whitespace().count() != 24 {
                state.error = Some(format!(
                    "enter all 24 words (got {})",
                    words.split_whitespace().count()
                ));
                return None;
            }
            state.busy = true;
            state.restore_words.zeroize();
            let command = Command::RestoreIdentity {
                mnemonic: words.into(),
                password: take_secret(&mut state.password).into(),
            };
            state.confirm_password.zeroize();
            Some(command)
        }
        Stage::LinkPassword => {
            validate_set_password(state)?;
            state.busy = true;
            let command = Command::PrepareLinkIdentity {
                password: take_secret(&mut state.password).into(),
            };
            state.confirm_password.zeroize();
            Some(command)
        }
        Stage::LinkChallenge => {
            if state.link_challenge.trim().is_empty() {
                state.error = Some(
                    "paste the link code or type the http:// address from your other device".into(),
                );
                return None;
            }
            state.busy = true;
            let command = Command::GenerateLinkResponse {
                challenge: state.link_challenge.trim().to_owned().into(),
                device_label: nonempty(&state.device_label),
            };
            state.link_challenge.zeroize();
            Some(command)
        }
        Stage::ConfirmRecovery => {
            let words: Vec<_> = state.mnemonic.split_whitespace().collect();
            for (slot, index) in state.confirm_indices.into_iter().enumerate() {
                let expected = words.get(index).copied().unwrap_or_default();
                if !state.confirm_answers[slot]
                    .trim()
                    .eq_ignore_ascii_case(expected)
                {
                    state.error = Some(format!("word #{} doesn't match — try again", index + 1));
                    return None;
                }
            }
            state.busy = true;
            Some(Command::ConfirmMnemonic)
        }
        Stage::Unlock => {
            if state.password.is_empty() {
                state.error = Some("enter your password".into());
                return None;
            }
            state.busy = true;
            Some(Command::UnlockIdentity {
                password: take_secret(&mut state.password).into(),
            })
        }
        Stage::ResumeRecovery => {
            if state.password.is_empty() {
                state.error = Some("enter your password".into());
                return None;
            }
            state.busy = true;
            state.recovery_purpose = RecoveryPurpose::Resume;
            Some(Command::RevealMnemonic {
                password: take_secret(&mut state.password).into(),
            })
        }
        Stage::SecureLegacyPassword => {
            validate_set_password(state)?;
            state.busy = true;
            let command = Command::EncryptLegacy {
                password: take_secret(&mut state.password).into(),
            };
            state.confirm_password.zeroize();
            Some(command)
        }
        Stage::RevealLegacy => {
            state.busy = true;
            state.recovery_purpose = RecoveryPurpose::Legacy;
            Some(Command::RevealMnemonic {
                password: take_secret(&mut state.password).into(),
            })
        }
        Stage::LoadError => update(state, Message::Retry),
        _ => None,
    }
}

fn service_event(state: &mut State, event: ServiceEvent) -> Option<Command> {
    state.busy = false;
    match event {
        ServiceEvent::IdentityLoaded(Ok(report)) => {
            state.touch_id_available = report.touch_id_available;
            state.first_run = report.kind == IdentityKind::Absent;
            state.stage = match (report.kind, report.mnemonic_confirmed) {
                (IdentityKind::Absent, _) => Stage::Create,
                (IdentityKind::Plaintext, _) => Stage::SecureLegacy,
                (IdentityKind::Locked | IdentityKind::Unlocked, false) => Stage::ResumeRecovery,
                (IdentityKind::Locked, true) => Stage::Unlock,
                (IdentityKind::Unlocked, true) => Stage::Ready,
            };
            state.error = None;
            None
        }
        ServiceEvent::IdentityLoaded(Err(error)) => {
            state.stage = Stage::LoadError;
            state.error = Some(error);
            None
        }
        ServiceEvent::IdentityCreated(Ok(created)) => {
            state.password.zeroize();
            state.confirm_password.zeroize();
            state.restore_words.zeroize();
            install_mnemonic(state, created.take_mnemonic(), RecoveryPurpose::Create);
            None
        }
        ServiceEvent::MnemonicConfirmed(Ok(())) => {
            if state.mode == EntryMode::TouchId {
                state.busy = true;
                Some(Command::EnrollTouchIdSession)
            } else {
                finish(state, Command::GateCompleted)
            }
        }
        ServiceEvent::IdentityRestored(Ok(()))
        | ServiceEvent::IdentityUnlocked(Ok(()))
        | ServiceEvent::TouchIdUnlocked(Ok(())) => finish(state, Command::GateCompleted),
        ServiceEvent::TouchIdEnrolled(Ok(())) => finish(state, Command::GateCompleted),
        ServiceEvent::TouchIdEnrolled(Err(error)) => {
            // Do NOT finish(): a swallowed enrollment failure would falsely
            // report onboarding success while leaving the account with neither
            // a working Touch ID credential nor a password. Stay on-screen with
            // a visible error; the proceed button re-issues enrollment.
            state.error = Some(
                if error == "touchid-canceled" || error.starts_with("touchid-unavailable") {
                    "Touch ID setup didn't finish. Your recovery phrase is your only way back \
                     in — keep it safe, then retry Touch ID."
                        .into()
                } else {
                    error
                },
            );
            None
        }
        ServiceEvent::LinkIdentityPrepared(Ok(())) => {
            state.password.zeroize();
            state.confirm_password.zeroize();
            state.stage = Stage::LinkChallenge;
            state.error = None;
            None
        }
        ServiceEvent::LinkResponseGenerated(Ok(reply)) => {
            state.link_challenge.zeroize();
            state.link_reply = Some(reply);
            state.stage = Stage::LinkResponse;
            state.error = None;
            None
        }
        ServiceEvent::LegacyEncrypted(Ok(())) => {
            state.stage = Stage::RevealLegacy;
            state.error = None;
            None
        }
        ServiceEvent::MnemonicRevealed(Ok(mnemonic)) => {
            state.password.zeroize();
            state.confirm_password.zeroize();
            let purpose = state.recovery_purpose;
            install_mnemonic(state, mnemonic, purpose);
            None
        }
        ServiceEvent::TextCopied(Ok(())) => {
            state.copied = true;
            None
        }
        ServiceEvent::IdentityCreated(Err(error))
        | ServiceEvent::MnemonicConfirmed(Err(error))
        | ServiceEvent::IdentityRestored(Err(error))
        | ServiceEvent::LinkIdentityPrepared(Err(error))
        | ServiceEvent::LinkResponseGenerated(Err(error))
        | ServiceEvent::IdentityUnlocked(Err(error))
        | ServiceEvent::TouchIdUnlocked(Err(error))
        | ServiceEvent::LegacyEncrypted(Err(error))
        | ServiceEvent::MnemonicRevealed(Err(error))
        | ServiceEvent::TextCopied(Err(error)) => {
            if error == "touchid-canceled" {
                state.error = None;
            } else if error == "touchid-unavailable" {
                state.error = Some(
                    "Touch ID is unavailable — unlock with your password or recovery phrase (Restore) instead."
                        .into(),
                );
            } else {
                state.error = Some(error);
            }
            None
        }
    }
}

fn validate_set_password(state: &mut State) -> Option<()> {
    if state.password.len() < MIN_PASSWORD_LEN {
        state.error = Some(format!(
            "password must be at least {MIN_PASSWORD_LEN} characters"
        ));
        return None;
    }
    if state.password != state.confirm_password {
        state.error = Some("passwords do not match".into());
        return None;
    }
    state.error = None;
    Some(())
}

fn install_mnemonic(state: &mut State, mnemonic: String, purpose: RecoveryPurpose) {
    replace_secret(&mut state.mnemonic, mnemonic);
    state.recovery_purpose = purpose;
    for answer in &mut state.confirm_answers {
        answer.zeroize();
    }
    state.confirm_answers = std::array::from_fn(|_| String::new());
    state.confirm_indices = confirm_indices(state.mnemonic.split_whitespace().count());
    state.stage = Stage::RecoveryPhrase;
    state.error = None;
}

fn confirm_indices(word_count: usize) -> [usize; 3] {
    if word_count < 3 {
        return [0, 0, 0];
    }
    [word_count / 8, word_count / 2 - 1, word_count * 7 / 8 - 1]
}

fn normalized_words(input: &str) -> String {
    input
        .split_whitespace()
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

fn nonempty(input: &str) -> Option<String> {
    let trimmed = input.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn finish(state: &mut State, command: Command) -> Option<Command> {
    state.busy = false;
    state.error = None;
    state.stage = Stage::Ready;
    clear_secrets(state);
    Some(command)
}

fn clear_secrets(state: &mut State) {
    state.password.zeroize();
    state.confirm_password.zeroize();
    state.restore_words.zeroize();
    state.link_challenge.zeroize();
    state.mnemonic.zeroize();
    state.link_reply.take();
    for answer in &mut state.confirm_answers {
        answer.zeroize();
    }
}

fn replace_secret(slot: &mut String, value: String) {
    slot.zeroize();
    *slot = value;
}

fn take_secret(slot: &mut String) -> String {
    std::mem::take(slot)
}

pub fn view(state: &State, mode: theme::Mode) -> Element<'_, Message> {
    let p = *theme::palette(mode);
    let content = match state.stage {
        Stage::Loading => gate_card(
            "Loading your account",
            Some("Reading the account key stored on this device…"),
            column![text("Please wait").font(MONO).size(11).color(p.muted)],
            p,
        ),
        Stage::LoadError => gate_card(
            "Couldn't read your account key",
            state.error.as_deref(),
            column![primary("Retry", Some(Message::Retry), p)],
            p,
        ),
        Stage::Create => create_view(state, p),
        Stage::TouchIdCreate => touch_id_create_view(state, p),
        Stage::RecoveryPhrase => recovery_view(state, p),
        Stage::ConfirmRecovery => confirm_recovery_view(state, p),
        Stage::Restore => restore_view(state, p),
        Stage::LinkPassword => link_password_view(state, p),
        Stage::LinkChallenge => link_challenge_view(state, p),
        Stage::LinkResponse => link_response_view(state, p),
        Stage::SecureLegacy => secure_legacy_view(p),
        Stage::SecureLegacyPassword => secure_legacy_password_view(state, p),
        Stage::RevealLegacy => reveal_legacy_view(state, p),
        Stage::ResumeRecovery => resume_view(state, p),
        Stage::Unlock => unlock_view(state, p),
        Stage::Ready => Space::new().width(Length::Fill).height(Length::Fill).into(),
    };

    let body = if state.first_run && !state.is_ready() {
        column![step_rail(p), content]
            .align_x(Alignment::Center)
            .spacing(18)
    } else {
        column![content].align_x(Alignment::Center)
    };

    container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .padding(24)
        .style(move |_| container::Style {
            background: Some(Background::Color(p.paper)),
            text_color: Some(p.ink),
            ..Default::default()
        })
        .into()
}

fn create_view(state: &State, p: Palette) -> Element<'_, Message> {
    let mut credentials =
        password_fields(state, "Password (min 8 characters)", "Confirm password", p);
    if let Some(error) = state.error.as_deref() {
        credentials = credentials.push(text(error).font(MONO).size(11.5).color(p.red));
    }
    credentials = credentials.push(primary(
        if state.busy {
            "Creating…"
        } else {
            "Create account"
        },
        (!state.busy).then_some(Message::Submit),
        p,
    ));
    gate_card(
        "Create your account",
        Some(
            "One account for all your devices and workspaces. Set a password to protect it on this device — your 24-word recovery phrase comes next.",
        ),
        column![
            mode_tabs(state, p),
            field(
                "Your name (optional)",
                &state.display_name,
                Message::DisplayNameChanged,
                false,
                p
            ),
            credentials,
        ]
        .spacing(10),
        p,
    )
}

fn touch_id_create_view(state: &State, p: Palette) -> Element<'_, Message> {
    gate_card(
        "Use Touch ID",
        Some(
            "Unlock this Mac with Touch ID — no password to remember. You'll still get a 24-word recovery phrase: it's the only other way back into your account, so save it.",
        ),
        column![
            mode_tabs(state, p),
            field(
                "Your name (optional)",
                &state.display_name,
                Message::DisplayNameChanged,
                false,
                p
            ),
            error_line(state, p),
            primary(
                if state.busy {
                    "Creating…"
                } else {
                    "Continue with Touch ID"
                },
                (!state.busy).then_some(Message::Submit),
                p
            ),
        ]
        .spacing(10),
        p,
    )
}

fn restore_view(state: &State, p: Palette) -> Element<'_, Message> {
    gate_card(
        "Restore your account",
        Some("Enter your 24-word recovery phrase and set a new password for this device."),
        column![
            mode_tabs(state, p),
            field(
                "24-word recovery phrase, separated by spaces",
                &state.restore_words,
                Message::RestoreWordsChanged,
                false,
                p
            )
            .font(MONO),
            password_fields(state, "New password", "Confirm new password", p),
            error_line(state, p),
            primary(
                if state.busy {
                    "Restoring…"
                } else {
                    "Restore account"
                },
                (!state.busy).then_some(Message::Submit),
                p
            ),
        ]
        .spacing(10),
        p,
    )
}

fn link_password_view(state: &State, p: Palette) -> Element<'_, Message> {
    gate_card(
        "Link this device",
        Some(
            "Your account lives on another device. Set a password for this device's own key — you'll approve the link from your other device next.",
        ),
        column![
            mode_tabs(state, p),
            password_fields(state, "Password (min 8 characters)", "Confirm password", p),
            error_line(state, p),
            primary(
                if state.busy {
                    "Creating…"
                } else {
                    "Create this device's key"
                },
                (!state.busy).then_some(Message::Submit),
                p
            ),
        ]
        .spacing(10),
        p,
    )
}

fn recovery_view(state: &State, p: Palette) -> Element<'_, Message> {
    let (title, subtitle, continue_label) = match state.recovery_purpose {
        RecoveryPurpose::Create => (
            "Save your recovery phrase",
            "These 24 words ARE your account — anyone holding them can restore it anywhere. Write them down in order; they're shown only once.",
            "I've saved it — continue",
        ),
        RecoveryPurpose::Resume => (
            "Your recovery phrase",
            "Write these 24 words down in order and keep them somewhere safe.",
            "Continue",
        ),
        RecoveryPurpose::Legacy => (
            "View your recovery phrase",
            "You can write down your 24-word recovery phrase now, or do this later from the Account view.",
            "Done",
        ),
    };
    let words: Vec<_> = state.mnemonic.split_whitespace().collect();
    let mut grid = Column::new().spacing(6);
    for (row_index, chunk) in words.chunks(3).enumerate() {
        let mut line = row![].spacing(6);
        for (column_index, word) in chunk.iter().enumerate() {
            let index = row_index * 3 + column_index + 1;
            line = line.push(word_cell(index, word, p));
        }
        grid = grid.push(line);
    }
    gate_card(
        title,
        Some(subtitle),
        column![
            grid,
            secondary(
                if state.copied {
                    "Copied"
                } else {
                    "Copy to clipboard"
                },
                Some(Message::CopyRecovery),
                p
            ),
            primary(continue_label, Some(Message::ContinueRecovery), p),
        ]
        .spacing(12),
        p,
    )
}

fn confirm_recovery_view(state: &State, p: Palette) -> Element<'_, Message> {
    let mut fields = Column::new().spacing(10);
    for (slot, index) in state.confirm_indices.iter().copied().enumerate() {
        fields = fields.push(field(
            &format!("Word #{}", index + 1),
            &state.confirm_answers[slot],
            move |value| Message::ConfirmWordChanged(slot, value),
            false,
            p,
        ));
    }
    gate_card(
        "Confirm your recovery phrase",
        Some("Enter the requested words to prove you saved them."),
        fields.push(error_line(state, p)).push(primary(
            if state.busy {
                "Confirming…"
            } else {
                "Confirm"
            },
            (!state.busy).then_some(Message::Submit),
            p,
        )),
        p,
    )
}

fn link_challenge_view(state: &State, p: Palette) -> Element<'_, Message> {
    gate_card(
        "Approve from your other device",
        Some(
            "On your other device, open Account → Link a device, then type the address under its QR here — or swap the two codes by hand. You can continue and finish the link later.",
        ),
        column![
            row![
                icons::view(Icon::Link, 16.0, p.muted),
                text("Link challenge").font(SANS).size(11).color(p.muted)
            ]
            .spacing(7)
            .align_y(Alignment::Center),
            field(
                "Paste the link code — or type the http:// address",
                &state.link_challenge,
                Message::LinkChallengeChanged,
                false,
                p
            )
            .font(MONO),
            field(
                "Device label (optional, e.g. work laptop)",
                &state.device_label,
                Message::DeviceLabelChanged,
                false,
                p
            ),
            error_line(state, p),
            primary(
                if state.busy {
                    "Signing…"
                } else {
                    "Generate link code"
                },
                (!state.busy).then_some(Message::Submit),
                p
            ),
            link_button("I'll finish this later", Message::FinishLater, p),
        ]
        .spacing(10),
        p,
    )
}

fn link_response_view(state: &State, p: Palette) -> Element<'_, Message> {
    let reply = state.link_reply.as_ref();
    let sent = reply.is_some_and(|reply| reply.sent_automatically);
    let hint = if sent {
        "Reply sent — approve the link on your other device. This device joins the account once that lands."
    } else {
        "Paste this on your other device and approve the link there. This device joins the account once that lands."
    };
    let code = reply
        .map(|reply| reply.response.as_str())
        .unwrap_or_default();
    gate_card(
        if sent {
            "Reply sent"
        } else {
            "Finish on your other device"
        },
        Some(hint),
        column![
            container(text(code).font(MONO).size(10.5).color(p.ink))
                .padding(10)
                .width(Length::Fill)
                .style(move |_| input_container_style(p, false)),
            secondary(
                if state.copied {
                    "Copied"
                } else {
                    "Copy to clipboard"
                },
                Some(Message::CopyLinkResponse),
                p
            ),
            primary("Continue", Some(Message::FinishLater), p),
        ]
        .spacing(10),
        p,
    )
}

fn unlock_view<'a>(state: &'a State, p: Palette) -> Element<'a, Message> {
    let mut content: Column<'a, Message> = Column::new().spacing(10).align_x(Alignment::Center);
    if state.touch_id_available {
        let touch_label: &'a str = if state.busy {
            "Unlocking…"
        } else {
            "Unlock with Touch ID"
        };
        content = content.push(primary(
            touch_label,
            (!state.busy).then_some(Message::UseTouchId),
            p,
        ));
    }
    let unlock_label: &'a str = if state.busy { "Unlocking…" } else { "Unlock" };
    let skip_label: &'a str = "Skip for now";
    content = content
        .push(field(
            "Password",
            &state.password,
            Message::PasswordChanged,
            true,
            p,
        ))
        .push(error_line(state, p))
        .push(primary(
            unlock_label,
            (!state.busy).then_some(Message::Submit),
            p,
        ))
        .push(link_button(skip_label, Message::Skip, p))
        .push(
            text("Until you unlock, nodes you start stay unlinked to your account.")
                .font(SANS)
                .size(10.5)
                .color(p.muted_2),
        );
    gate_card(
        "Unlock your account",
        Some("Enter your password to unlock your account on this device for this session."),
        content,
        p,
    )
}

fn resume_view(state: &State, p: Palette) -> Element<'_, Message> {
    gate_card(
        "Confirm your recovery phrase",
        Some(
            "You created this account but never confirmed its recovery phrase. Enter your password to view it and finish.",
        ),
        column![
            field(
                "Password",
                &state.password,
                Message::PasswordChanged,
                true,
                p
            ),
            error_line(state, p),
            primary(
                if state.busy { "Loading…" } else { "Continue" },
                (!state.busy).then_some(Message::Submit),
                p
            ),
            link_button("Skip for now", Message::Skip, p),
        ]
        .spacing(10),
        p,
    )
}

fn secure_legacy_view(p: Palette) -> Element<'static, Message> {
    gate_card(
        "Secure your account",
        Some(
            "This account isn't password-protected yet. Set a password so a stolen device can't be used to sign as you. You can do this later from the Account view.",
        ),
        column![
            primary("Set a password", Some(Message::SecureLegacy), p),
            link_button("Not now", Message::Skip, p),
        ]
        .spacing(10),
        p,
    )
}

fn secure_legacy_password_view(state: &State, p: Palette) -> Element<'_, Message> {
    gate_card(
        "Set a password",
        Some("This encrypts your account key at rest on this device."),
        column![
            password_fields(state, "Password (min 8 characters)", "Confirm password", p),
            error_line(state, p),
            primary(
                if state.busy {
                    "Securing…"
                } else {
                    "Secure account"
                },
                (!state.busy).then_some(Message::Submit),
                p
            ),
        ]
        .spacing(10),
        p,
    )
}

fn reveal_legacy_view(state: &State, p: Palette) -> Element<'_, Message> {
    gate_card(
        "View your recovery phrase",
        Some(
            "You can write down your 24-word recovery phrase now, or do this later from the Account view.",
        ),
        column![
            error_line(state, p),
            primary(
                if state.busy {
                    "Loading…"
                } else {
                    "View recovery phrase"
                },
                (!state.busy).then_some(Message::Submit),
                p
            ),
            link_button("Skip — I'll do this later", Message::FinishLater, p),
        ]
        .spacing(10),
        p,
    )
}

fn password_fields<'a>(
    state: &'a State,
    password_placeholder: &'a str,
    confirm_placeholder: &'a str,
    p: Palette,
) -> Column<'a, Message> {
    column![
        field(
            password_placeholder,
            &state.password,
            Message::PasswordChanged,
            true,
            p
        ),
        field(
            confirm_placeholder,
            &state.confirm_password,
            Message::ConfirmPasswordChanged,
            true,
            p
        ),
    ]
    .spacing(10)
}

fn mode_tabs(state: &State, p: Palette) -> Element<'static, Message> {
    let mut tabs = vec![(EntryMode::Create, "Create")];
    if state.touch_id_available {
        tabs.push((EntryMode::TouchId, "Use Touch ID"));
    }
    tabs.extend([
        (EntryMode::Restore, "Restore"),
        (EntryMode::LinkDevice, "Link device"),
    ]);
    let mut items = row![].spacing(4).padding(4);
    for (mode, label) in tabs {
        items = items.push(tab(label, mode == state.mode, Message::SelectMode(mode), p));
    }
    container(items)
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(p.panel)),
            border: Border {
                radius: RADIUS_MD.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn step_rail(p: Palette) -> Element<'static, Message> {
    let mut items = row![].align_y(Alignment::Center).spacing(9);
    for (index, label) in ["Account", "Workspace", "Connect"].into_iter().enumerate() {
        if index > 0 {
            items = items.push(container(Space::new().width(22).height(1)).style(move |_| {
                container::Style {
                    background: Some(Background::Color(p.border_strong)),
                    ..Default::default()
                }
            }));
        }
        let active = index == 0;
        items = items.push(
            row![
                container(
                    text((index + 1).to_string())
                        .font(MONO)
                        .size(9)
                        .color(if active { p.on_filled } else { p.muted_2 })
                )
                .width(17)
                .height(17)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(move |_| container::Style {
                    background: active.then_some(Background::Color(p.filled)),
                    border: Border {
                        color: if active { p.filled } else { p.border_strong },
                        width: 1.0,
                        radius: 99.0.into()
                    },
                    ..Default::default()
                }),
                text(label)
                    .font(SANS)
                    .size(10.5)
                    .color(if active { p.ink } else { p.muted_2 }),
            ]
            .spacing(9)
            .align_y(Alignment::Center),
        );
    }
    items.into()
}

fn gate_card<'a>(
    title: &'a str,
    subtitle: Option<&'a str>,
    content: Column<'a, Message>,
    p: Palette,
) -> Element<'a, Message> {
    let mut header = column![text(title).font(SANS_SEMIBOLD).size(16).color(p.ink)].spacing(5);
    if let Some(subtitle) = subtitle {
        header = header.push(
            text(subtitle)
                .font(SANS_MEDIUM)
                .size(13)
                .line_height(1.4)
                .color(p.muted),
        );
    }
    let body = column![header, content].spacing(16);
    container(body)
        .width(CARD_WIDTH)
        .padding([27, 24])
        .style(move |_| container::Style {
            background: Some(Background::Color(p.sidebar)),
            border: Border {
                color: p.border,
                width: 1.0,
                radius: RADIUS_LG.into(),
            },
            shadow: Shadow {
                color: Color {
                    a: 0.20,
                    ..Color::from_rgb8(40, 38, 34)
                },
                offset: Vector::new(0.0, 18.0),
                blur_radius: 48.0,
            },
            ..Default::default()
        })
        .into()
}

fn field<'a>(
    placeholder: &str,
    value: &'a str,
    on_input: impl Fn(String) -> Message + 'a,
    secure: bool,
    p: Palette,
) -> TextInput<'a, Message> {
    text_input(placeholder, value)
        .on_input(on_input)
        .secure(secure)
        .padding([9, 11])
        .size(12.5)
        .font(SANS_MEDIUM)
        .style(move |_, status| iced::widget::text_input::Style {
            background: Background::Color(p.sunken),
            border: Border {
                color: if matches!(status, iced::widget::text_input::Status::Focused { .. }) {
                    theme::ACCENTS[0]
                } else {
                    p.border_strong
                },
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            icon: p.muted,
            placeholder: p.muted,
            value: p.ink,
            selection: theme::ACCENTS[0],
        })
}

fn primary<'a>(label: &'a str, message: Option<Message>, p: Palette) -> Element<'a, Message> {
    let enabled = message.is_some();
    let button = button(
        container(text(label).font(SANS_SEMIBOLD).size(12.5))
            .width(Length::Fill)
            .center_x(Length::Fill),
    )
    .width(Length::Fill)
    .padding([10, 0])
    .style(move |_, status| iced::widget::button::Style {
        background: Some(Background::Color(if !enabled {
            p.chip
        } else if matches!(status, iced::widget::button::Status::Hovered) {
            p.ink_soft
        } else {
            p.filled
        })),
        text_color: if !enabled { p.muted_3 } else { p.on_filled },
        border: Border {
            radius: RADIUS_MD.into(),
            ..Default::default()
        },
        ..Default::default()
    });
    let button = match message {
        Some(message) => button.on_press(message),
        None => button,
    };
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::Button, label, button)
        .disabled(!enabled)
        .into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    button.into()
}

fn secondary<'a>(label: &'a str, message: Option<Message>, p: Palette) -> Element<'a, Message> {
    let button = button(
        container(text(label).font(SANS_SEMIBOLD).size(12))
            .width(Length::Fill)
            .center_x(Length::Fill),
    )
    .width(Length::Fill)
    .padding([9, 0])
    .style(move |_, status| iced::widget::button::Style {
        background: Some(Background::Color(
            if matches!(status, iced::widget::button::Status::Hovered) {
                p.hover
            } else {
                p.paper
            },
        )),
        text_color: p.ink,
        border: Border {
            color: p.border,
            width: 1.0,
            radius: RADIUS_MD.into(),
        },
        ..Default::default()
    });
    let enabled = message.is_some();
    let button = match message {
        Some(message) => button.on_press(message),
        None => button,
    };
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::Button, label, button)
        .disabled(!enabled)
        .into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    button.into()
}

fn tab(
    label: &'static str,
    active: bool,
    message: Message,
    p: Palette,
) -> Element<'static, Message> {
    let tab = button(
        container(text(label).font(SANS_SEMIBOLD).size(12))
            .width(Length::Fill)
            .center_x(Length::Fill),
    )
    .width(Length::FillPortion(1))
    .padding([8, 0])
    .style(move |_, status| iced::widget::button::Style {
        background: (active || matches!(status, iced::widget::button::Status::Hovered))
            .then_some(Background::Color(if active { p.paper } else { p.hover })),
        text_color: if active { p.ink } else { p.muted },
        border: Border {
            radius: RADIUS_SM.into(),
            ..Default::default()
        },
        shadow: if active {
            Shadow {
                color: Color {
                    a: 0.05,
                    ..Color::from_rgb8(40, 38, 34)
                },
                offset: Vector::new(0.0, 1.0),
                blur_radius: 2.0,
            }
        } else {
            Shadow::default()
        },
        ..Default::default()
    })
    .on_press(message);
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Tab, label, tab);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    tab.into()
}

fn link_button<'a>(label: &'a str, message: Message, p: Palette) -> Element<'a, Message> {
    let link = button(
        container(text(label).font(SANS_SEMIBOLD).size(11).color(p.muted))
            .width(Length::Fill)
            .center_x(Length::Fill),
    )
    .width(Length::Fill)
    .padding(4)
    .style(|_, _| iced::widget::button::Style::default())
    .on_press(message);
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Link, label, link);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    link.into()
}

fn error_line<'a>(state: &'a State, p: Palette) -> Element<'a, Message> {
    match state.error.as_deref() {
        Some(error) => text(error).font(MONO).size(11.5).color(p.red).into(),
        None => Space::new().height(0).into(),
    }
}

fn word_cell<'a>(index: usize, word: &'a str, p: Palette) -> Element<'a, Message> {
    container(
        row![
            text(index.to_string())
                .font(MONO)
                .size(10)
                .color(p.muted_2)
                .width(16),
            text(word).font(MONO).size(12).color(p.ink),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .width(Length::FillPortion(1))
    .padding([6, 8])
    .style(move |_| input_container_style(p, false))
    .into()
}

fn input_container_style(p: Palette, focused: bool) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(p.sunken)),
        border: Border {
            color: if focused { theme::ACCENTS[0] } else { p.border },
            width: 1.0,
            radius: RADIUS_SM.into(),
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn absent_state() -> State {
        let mut state = State::default();
        update(
            &mut state,
            Message::Service(ServiceEvent::IdentityLoaded(Ok(IdentityReport {
                kind: IdentityKind::Absent,
                mnemonic_confirmed: true,
                touch_id_available: false,
            }))),
        );
        state
    }

    #[test]
    fn load_routes_saved_identity_to_unlock() {
        let mut state = State::default();
        update(
            &mut state,
            Message::Service(ServiceEvent::IdentityLoaded(Ok(IdentityReport {
                kind: IdentityKind::Locked,
                mnemonic_confirmed: true,
                touch_id_available: false,
            }))),
        );
        assert_eq!(state.stage, Stage::Unlock);
        assert!(!state.shows_first_run_steps());
    }

    #[test]
    fn create_validates_before_emitting_custody_command() {
        let mut state = absent_state();
        update(&mut state, Message::PasswordChanged("long enough".into()));
        update(
            &mut state,
            Message::ConfirmPasswordChanged("different".into()),
        );
        assert_eq!(update(&mut state, Message::Submit), None);
        assert_eq!(state.error.as_deref(), Some("passwords do not match"));

        update(
            &mut state,
            Message::ConfirmPasswordChanged("long enough".into()),
        );
        assert_eq!(
            update(&mut state, Message::Submit),
            Some(Command::CreateIdentity {
                password: "long enough".into(),
                display_name: None,
            })
        );
        assert!(state.password.is_empty());
        assert!(state.confirm_password.is_empty());
    }

    #[test]
    fn create_recovery_requires_the_requested_words() {
        let mut state = absent_state();
        let mnemonic = (1..=24)
            .map(|n| format!("word{n}"))
            .collect::<Vec<_>>()
            .join(" ");
        update(
            &mut state,
            Message::Service(ServiceEvent::IdentityCreated(Ok(CreatedIdentity {
                mnemonic,
            }))),
        );
        assert_eq!(state.stage, Stage::RecoveryPhrase);
        update(&mut state, Message::ContinueRecovery);
        assert_eq!(state.stage, Stage::ConfirmRecovery);
        assert_eq!(update(&mut state, Message::Submit), None);
        assert!(
            state
                .error
                .as_deref()
                .is_some_and(|error| error.contains("doesn't match"))
        );

        let words: Vec<_> = state
            .mnemonic
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        for (slot, index) in state.confirm_indices.into_iter().enumerate() {
            update(
                &mut state,
                Message::ConfirmWordChanged(slot, words[index].clone()),
            );
        }
        assert_eq!(
            update(&mut state, Message::Submit),
            Some(Command::ConfirmMnemonic)
        );
    }

    #[test]
    fn restore_normalizes_words_and_emits_one_command() {
        let mut state = absent_state();
        update(&mut state, Message::SelectMode(EntryMode::Restore));
        update(&mut state, Message::PasswordChanged("restored pass".into()));
        update(
            &mut state,
            Message::ConfirmPasswordChanged("restored pass".into()),
        );
        let words = (1..=24)
            .map(|n| format!("WORD{n}"))
            .collect::<Vec<_>>()
            .join("  ");
        update(&mut state, Message::RestoreWordsChanged(words));
        let command = update(&mut state, Message::Submit);
        assert!(
            matches!(command, Some(Command::RestoreIdentity { mnemonic, .. }) if mnemonic.starts_with("word1 word2"))
        );
        assert!(state.password.is_empty());
        assert!(state.confirm_password.is_empty());
        assert!(state.restore_words.is_empty());
    }

    #[test]
    fn service_failures_stay_inline_and_rearm_submit() {
        let mut state = absent_state();
        state.busy = true;
        update(
            &mut state,
            Message::Service(ServiceEvent::IdentityCreated(Err(
                "vault unavailable".into()
            ))),
        );
        assert!(!state.busy);
        assert_eq!(state.stage, Stage::Create);
        assert_eq!(state.error.as_deref(), Some("vault unavailable"));
    }

    #[test]
    fn account_actions_reuse_the_gate_without_first_run_chrome() {
        let mut state = State::default();
        state.stage = Stage::Ready;
        state.password = "secret".into();
        state.error = Some("old".into());

        begin_account_action(&mut state, AccountAction::Secure);
        assert_eq!(state.stage, Stage::SecureLegacyPassword);
        assert!(state.password.is_empty());
        assert!(state.error.is_none());
        assert!(!state.shows_first_run_steps());

        begin_account_action(&mut state, AccountAction::RevealRecovery);
        assert_eq!(state.stage, Stage::RevealLegacy);
        begin_account_action(&mut state, AccountAction::Unlock);
        assert_eq!(state.stage, Stage::Unlock);
    }

    #[test]
    fn secret_bearing_debug_output_is_redacted_and_transitions_clear_state() {
        let secret = "correct horse battery staple";
        let command = Command::RestoreIdentity {
            mnemonic: secret.into(),
            password: "password-secret".into(),
        };
        let message = Message::LinkChallengeChanged("capability-secret".into());
        let created = CreatedIdentity {
            mnemonic: secret.into(),
        };
        for debug in [
            format!("{command:?}"),
            format!("{message:?}"),
            format!("{created:?}"),
        ] {
            assert!(!debug.contains(secret));
            assert!(!debug.contains("password-secret"));
            assert!(!debug.contains("capability-secret"));
        }

        let mut state = State::default();
        state.password = "password-secret".into();
        state.confirm_password = "password-secret".into();
        state.restore_words = secret.into();
        state.link_challenge = "capability-secret".into();
        state.link_reply = Some(LinkReply {
            response: "link-response-secret".into(),
            account_name: None,
            device_key: None,
            sent_automatically: false,
        });
        let debug = format!("{state:?}");
        assert!(!debug.contains(secret));
        assert!(!debug.contains("password-secret"));
        assert!(!debug.contains("capability-secret"));
        assert!(!debug.contains("link-response-secret"));

        begin_account_action(&mut state, AccountAction::Unlock);
        assert!(state.password.is_empty());
        assert!(state.confirm_password.is_empty());
        assert!(state.restore_words.is_empty());
        assert!(state.link_challenge.is_empty());
        assert!(state.link_reply.is_none());
    }
}
