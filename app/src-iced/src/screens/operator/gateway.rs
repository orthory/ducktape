use super::*;
use iced::widget::{column, row};
fn health_color(health: RouteHealth, p: Palette) -> Color {
    match health {
        RouteHealth::Serving(_) => p.green,
        RouteHealth::Failing(_) | RouteHealth::Unavailable => p.red,
        RouteHealth::Checking | RouteHealth::Reachable(_) => p.amber,
        RouteHealth::Idle | RouteHealth::Disabled => p.muted_3,
    }
}

fn validate_route_label(label: &str) -> Result<(), &'static str> {
    if label.is_empty() {
        return Ok(());
    }
    if label.len() > 63
        || label.starts_with('-')
        || label.ends_with('-')
        || !label
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err("Use lowercase letters, numbers, and hyphens.");
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteTarget {
    DuckFs,
    LoopbackHttp,
}

impl RouteTarget {
    const fn label(self) -> &'static str {
        match self {
            Self::DuckFs => "DuckFS content",
            Self::LoopbackHttp => "Local HTTP service",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteAudience {
    Network,
    Owner,
    Accounts,
}

impl RouteAudience {
    const fn label(self) -> &'static str {
        match self {
            Self::Network => "All identified network members",
            Self::Owner => "Owning account only",
            Self::Accounts => "Specific accounts",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RouteMethod {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
}

impl RouteMethod {
    const ALL: [Self; 6] = [
        Self::Get,
        Self::Head,
        Self::Post,
        Self::Put,
        Self::Patch,
        Self::Delete,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Head => "HEAD",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteHealth {
    Idle,
    Checking,
    Serving(u16),
    Reachable(u16),
    Failing(u16),
    Disabled,
    Unavailable,
}

impl RouteHealth {
    fn label(self) -> String {
        match self {
            Self::Idle => "Not checked".into(),
            Self::Checking => "Checking…".into(),
            Self::Serving(status) => format!("Serving · HTTP {status}"),
            Self::Reachable(status) => format!("Reachable · HTTP {status}"),
            Self::Failing(status) => format!("Failing · HTTP {status}"),
            Self::Disabled => "Health check needs HEAD".into(),
            Self::Unavailable => "Gateway unavailable".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayRoute {
    pub key: String,
    pub label: String,
    pub address: String,
    pub target: RouteTarget,
    pub revision: u64,
    pub this_node: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayDraft {
    pub label: String,
    pub address: String,
    pub target: RouteTarget,
    pub audience: RouteAudience,
    /// Raw editor text for the Accounts audience: hex account ids separated by
    /// commas or whitespace. Tokenized into ids at the save boundary.
    pub audience_accounts: String,
    pub default_path: String,
    pub port: String,
    pub methods: Vec<RouteMethod>,
    pub request_kib: String,
    pub response_kib: String,
    pub allow_authorization: bool,
    pub allow_upgrade: bool,
    pub revision: Option<u64>,
}

impl Default for GatewayDraft {
    fn default() -> Self {
        Self {
            label: String::new(),
            address: "Account ID route".into(),
            target: RouteTarget::DuckFs,
            audience: RouteAudience::Network,
            audience_accounts: String::new(),
            default_path: "index.html".into(),
            port: "3000".into(),
            methods: vec![RouteMethod::Get, RouteMethod::Head],
            request_kib: "256".into(),
            response_kib: "4096".into(),
            allow_authorization: false,
            allow_upgrade: false,
            revision: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayData {
    pub routes: Vec<GatewayRoute>,
    pub handle: Option<String>,
    pub account_bound: bool,
    pub desktop_signer: bool,
    pub managed_workspace: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GatewayState {
    pub data: Resource<GatewayData>,
    pub draft: GatewayDraft,
    pub selected: Option<String>,
    pub health: RouteHealth,
    pub busy: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayMessage {
    SelectRoute(String),
    NewRoute,
    LabelChanged(String),
    SetTarget(RouteTarget),
    SetAudience(RouteAudience),
    AccountsChanged(String),
    DefaultPathChanged(String),
    PortChanged(String),
    ToggleMethod(RouteMethod),
    RequestKibChanged(String),
    ResponseKibChanged(String),
    ToggleAuthorization,
    ToggleUpgrade,
    CheckHealth,
    CreateStarter,
    Save,
    Remove,
}

pub(super) fn update(state: &mut GatewayState, message: GatewayMessage) -> Option<Command> {
    match message {
        GatewayMessage::SelectRoute(key) => {
            state.selected = Some(key.clone());
            state.health = RouteHealth::Idle;
            state.note = None;
            return Some(Command::LoadGatewayRoute(key));
        }
        GatewayMessage::NewRoute => {
            state.selected = None;
            state.draft = GatewayDraft::default();
            state.health = RouteHealth::Idle;
            state.note = None;
        }
        GatewayMessage::LabelChanged(value) => {
            state.draft.label = value.to_ascii_lowercase();
        }
        GatewayMessage::SetTarget(target) => {
            state.draft.target = target;
            if target == RouteTarget::DuckFs {
                state.draft.methods = vec![RouteMethod::Get, RouteMethod::Head];
                state.draft.allow_authorization = false;
                state.draft.allow_upgrade = false;
            }
        }
        GatewayMessage::SetAudience(audience) => state.draft.audience = audience,
        GatewayMessage::AccountsChanged(value) => state.draft.audience_accounts = value,
        GatewayMessage::DefaultPathChanged(value) => state.draft.default_path = value,
        GatewayMessage::PortChanged(value) => state.draft.port = value,
        GatewayMessage::ToggleMethod(method) => {
            if let Some(index) = state.draft.methods.iter().position(|item| *item == method) {
                state.draft.methods.remove(index);
            } else {
                state.draft.methods.push(method);
                state.draft.methods.sort();
            }
        }
        GatewayMessage::RequestKibChanged(value) => state.draft.request_kib = value,
        GatewayMessage::ResponseKibChanged(value) => state.draft.response_kib = value,
        GatewayMessage::ToggleAuthorization => {
            state.draft.allow_authorization = !state.draft.allow_authorization
        }
        GatewayMessage::ToggleUpgrade => state.draft.allow_upgrade = !state.draft.allow_upgrade,
        GatewayMessage::CheckHealth => {
            let key = state.selected.clone()?;
            state.health = RouteHealth::Checking;
            return Some(Command::CheckGatewayHealth(key));
        }
        GatewayMessage::CreateStarter => {
            if let Err(error) = validate_gateway_draft(&state.draft) {
                state.note = Some(error);
                return None;
            }
            return Some(Command::CreateGatewayStarter(state.draft.clone()));
        }
        GatewayMessage::Save => {
            if let Err(error) = validate_gateway_draft(&state.draft) {
                state.note = Some(error);
                return None;
            }
            state.busy = true;
            state.note = None;
            return Some(Command::SaveGatewayRoute(state.draft.clone()));
        }
        GatewayMessage::Remove => {
            let key = state.selected.clone()?;
            state.busy = true;
            state.note = None;
            return Some(Command::RemoveGatewayRoute(key));
        }
    }
    None
}

fn validate_gateway_draft(draft: &GatewayDraft) -> Result<(), String> {
    if !draft.label.is_empty()
        && (draft.label.len() > 63
            || draft.label.starts_with('-')
            || draft.label.ends_with('-')
            || !draft
                .label
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'))
    {
        return Err("Use lowercase letters, numbers, and hyphens for the route label.".into());
    }
    let response = draft
        .response_kib
        .parse::<u64>()
        .map_err(|_| "Response cap must be a whole number of KiB.".to_string())?;
    if response > 4096 {
        return Err("Response cap must be 0..4096 KiB.".into());
    }
    if draft.target == RouteTarget::LoopbackHttp {
        let port = draft
            .port
            .parse::<u16>()
            .map_err(|_| "Loopback port must be 1..65535.".to_string())?;
        if port == 0 {
            return Err("Loopback port must be 1..65535.".into());
        }
        let request = draft
            .request_kib
            .parse::<u64>()
            .map_err(|_| "Request cap must be a whole number of KiB.".to_string())?;
        if request > 1024 {
            return Err("Request cap must be 0..1024 KiB.".into());
        }
        if draft.methods.is_empty() {
            return Err("Choose at least one allowed method.".into());
        }
    }
    if draft.audience == RouteAudience::Accounts && account_id_tokens(&draft.audience_accounts).next().is_none() {
        return Err("Choose at least one account for this audience.".into());
    }
    Ok(())
}

/// Split the Accounts-audience editor text into candidate hex account ids,
/// separated by commas or whitespace. Shared by validation, the save encoder,
/// and (indirectly) any UI that echoes the parsed list.
pub(crate) fn account_id_tokens(raw: &str) -> impl Iterator<Item = &str> {
    raw.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|token| !token.is_empty())
}

pub(super) fn view(state: &GatewayState, p: Palette) -> Element<'_, Message> {
    let Resource::Ready(data) = &state.data else {
        return resource_screen(
            &state.data,
            "Gateway",
            "No gateway routes are available.",
            Screen::Gateway,
            Icon::Browser,
            p,
        );
    };

    let mut route_rows = column![section_label("PUBLISHED ROUTES", p)].spacing(6);
    if data.routes.is_empty() {
        route_rows = route_rows.push(notice("No routes published.", p));
    }
    for route in &data.routes {
        let selected = state.selected.as_deref() == Some(route.key.as_str());
        route_rows = route_rows.push(
            button(row![
                column![
                    text(&route.address).font(MONO).size(10.5).color(p.ink),
                    text(if selected {
                        state.health.label()
                    } else {
                        "Published".into()
                    })
                    .font(SANS)
                    .size(9.5)
                    .color(if selected {
                        health_color(state.health, p)
                    } else {
                        p.muted
                    }),
                ]
                .spacing(3),
                Space::new().width(Length::Fill),
                column![
                    text(format!(
                        "{} · {}",
                        route.target.label(),
                        if route.this_node {
                            "this node"
                        } else {
                            "remote"
                        }
                    ))
                    .font(SANS)
                    .size(9.5)
                    .color(p.muted_3),
                    text(format!("r{}", route.revision))
                        .font(MONO)
                        .size(9)
                        .color(p.muted),
                ]
                .spacing(3)
                .align_x(Alignment::End),
            ])
            .width(Length::Fill)
            .padding([8, 9])
            .style(move |_, _| iced::widget::button::Style {
                background: selected.then_some(Background::Color(p.paper)),
                text_color: p.ink,
                border: Border {
                    color: if selected { p.border_strong } else { p.border },
                    width: 1.0,
                    radius: RADIUS_SM.into(),
                },
                ..Default::default()
            })
            .on_press(Message::Gateway(GatewayMessage::SelectRoute(
                route.key.clone(),
            ))),
        );
    }

    let can_mutate =
        data.desktop_signer && data.account_bound && data.managed_workspace && !state.busy;
    let editor = gateway_editor(state, can_mutate, p);
    let header = screen_header("Gateway", Some(data.routes.len()), p);
    let intro = text("Connect one account address to exact DuckFS content or a local HTTP service. The address, reverse proxy, and signed access policy are saved together.")
        .font(SANS).size(11).color(p.muted_3);
    let mut body = column![intro, route_rows, divider(p), editor].spacing(14);
    if data.handle.is_none() && data.account_bound {
        body = body.push(notice("Routes can exist by Account ID. Register a Duck name in Account to make them browsable as .duck addresses.", p));
    }
    if !data.desktop_signer {
        body = body.push(notice(
            "Saving routes requires the desktop user-key signer.",
            p,
        ));
    } else if !data.account_bound {
        body = body.push(notice(
            "Bind this node to your Identity account before saving routes.",
            p,
        ));
    }
    if let Some(note) = &state.note {
        body = body.push(notice(note, p));
    }
    container(column![
        header,
        scrollable(container(body).padding(Padding {
            top: 22.0,
            right: 20.0,
            bottom: 40.0,
            left: 20.0,
        }))
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| surface(p.sidebar))
    .into()
}

fn gateway_editor(state: &GatewayState, can_mutate: bool, p: Palette) -> Element<'_, Message> {
    let draft = &state.draft;
    let title = if draft.revision.is_some() {
        "Edit route"
    } else {
        "New route"
    };
    let label_error = validate_route_label(&draft.label).err();
    let mut methods = row![].spacing(5);
    for method in RouteMethod::ALL {
        methods = methods.push(toggle_button(
            method.label(),
            draft.methods.contains(&method),
            Message::Gateway(GatewayMessage::ToggleMethod(method)),
            draft.target == RouteTarget::LoopbackHttp,
            p,
        ));
    }
    let targets = row![
        toggle_button(
            "DuckFS content",
            draft.target == RouteTarget::DuckFs,
            Message::Gateway(GatewayMessage::SetTarget(RouteTarget::DuckFs)),
            true,
            p
        ),
        toggle_button(
            "Local HTTP service",
            draft.target == RouteTarget::LoopbackHttp,
            Message::Gateway(GatewayMessage::SetTarget(RouteTarget::LoopbackHttp)),
            true,
            p
        ),
    ]
    .spacing(6);
    let audiences = row![
        toggle_button(
            "Network",
            draft.audience == RouteAudience::Network,
            Message::Gateway(GatewayMessage::SetAudience(RouteAudience::Network)),
            true,
            p
        ),
        toggle_button(
            "Owner",
            draft.audience == RouteAudience::Owner,
            Message::Gateway(GatewayMessage::SetAudience(RouteAudience::Owner)),
            true,
            p
        ),
        toggle_button(
            "Accounts",
            draft.audience == RouteAudience::Accounts,
            Message::Gateway(GatewayMessage::SetAudience(RouteAudience::Accounts)),
            true,
            p
        ),
    ]
    .spacing(6);

    let mut fields = column![
        row![
            text(title).font(SANS).size(13).color(p.ink),
            Space::new().width(Length::Fill),
            outline_button(
                "New route",
                Message::Gateway(GatewayMessage::NewRoute),
                true,
                p
            ),
        ]
        .align_y(Alignment::Center),
        text(format!(
            "revision {}",
            draft.revision.map_or_else(|| "—".into(), |v| v.to_string())
        ))
        .font(MONO)
        .size(9)
        .color(p.muted),
        text(&draft.address).font(MONO).size(10.5).color(p.ink_soft),
        labeled_input(
            "Route label · blank = account apex",
            "api",
            &draft.label,
            |value| Message::Gateway(GatewayMessage::LabelChanged(value)),
            p
        ),
    ]
    .spacing(9);
    if let Some(error) = label_error {
        fields = fields.push(text(error).font(SANS).size(9.5).color(p.danger));
    }
    fields = fields
        .push(text("SOURCE").font(MONO).size(9).color(p.muted_2))
        .push(targets)
        .push(text("AUDIENCE").font(MONO).size(9).color(p.muted_2))
        .push(audiences)
        .push(
            text(draft.audience.label())
                .font(SANS)
                .size(10.5)
                .color(p.muted_3),
        );

    if draft.audience == RouteAudience::Accounts {
        // Without this input an Accounts-audience route can never satisfy the
        // "choose at least one account" gate, making the policy uncreatable.
        fields = fields.push(labeled_input(
            "Account IDs · comma or space separated",
            "acct1 acct2",
            &draft.audience_accounts,
            |value| Message::Gateway(GatewayMessage::AccountsChanged(value)),
            p,
        ));
    }

    if draft.target == RouteTarget::DuckFs {
        fields = fields
            .push(labeled_input(
                "Default path",
                "index.html",
                &draft.default_path,
                |value| Message::Gateway(GatewayMessage::DefaultPathChanged(value)),
                p,
            ))
            .push(outline_button(
                "Create starter file",
                Message::Gateway(GatewayMessage::CreateStarter),
                can_mutate && label_error.is_none(),
                p,
            ));
    } else {
        fields = fields
            .push(labeled_input(
                "Loopback port",
                "3000",
                &draft.port,
                |value| Message::Gateway(GatewayMessage::PortChanged(value)),
                p,
            ))
            .push(text("ALLOWED METHODS").font(MONO).size(9).color(p.muted_2))
            .push(methods)
            .push(toggle_button(
                "Allow explicit Authorization forwarding",
                draft.allow_authorization,
                Message::Gateway(GatewayMessage::ToggleAuthorization),
                true,
                p,
            ))
            .push(toggle_button(
                "Allow WebSocket upgrade",
                draft.allow_upgrade,
                Message::Gateway(GatewayMessage::ToggleUpgrade),
                true,
                p,
            ))
            .push(labeled_input(
                "Request KiB",
                "256",
                &draft.request_kib,
                |value| Message::Gateway(GatewayMessage::RequestKibChanged(value)),
                p,
            ));
    }
    fields = fields.push(labeled_input(
        "Response KiB",
        "4096",
        &draft.response_kib,
        |value| Message::Gateway(GatewayMessage::ResponseKibChanged(value)),
        p,
    ));
    if state.selected.is_some() {
        fields = fields.push(
            row![
                text(state.health.label())
                    .font(SANS)
                    .size(10)
                    .color(health_color(state.health, p)),
                Space::new().width(Length::Fill),
                outline_button(
                    "Check",
                    Message::Gateway(GatewayMessage::CheckHealth),
                    state.health != RouteHealth::Checking,
                    p
                ),
            ]
            .align_y(Alignment::Center),
        );
    }
    fields = fields.push(filled_button(
        "Save route",
        Message::Gateway(GatewayMessage::Save),
        can_mutate && label_error.is_none(),
        p,
    ));
    if state.selected.is_some() {
        fields = fields.push(danger_button(
            "Remove route",
            Message::Gateway(GatewayMessage::Remove),
            can_mutate,
            p,
        ));
    }
    fields.into()
}
