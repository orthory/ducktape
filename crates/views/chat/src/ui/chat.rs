use super::*;
impl super::ChatView {
    pub(super) fn render_timeline_selection(
        &self,
        use_scope: String,
        cb_13: impl Fn() -> Message + Clone + 'static,
        cb_18: impl Fn() -> Message + Clone + 'static,
    ) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        native::padded(
            native::sized(
                native::container(node_scope.clone(), {
                    let children: Vec<wire::Node> = vec![
                        native::text_options(
                            native::text(
                                format!("{}/@text:200", use_scope),
                                crate::host::copy_range_label(crate::host::copy_range_count(
                                    ::std::convert::AsRef::as_ref(&self.messages),
                                    self.copy_anchor_seq,
                                    self.copy_head_seq,
                                ))
                                .to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                        wire::Node::Space {
                            width: Some(wire::Length::Fill),
                            height: None,
                        },
                        native::padded(
                            native::button(
                                format!("{}/@button:207", use_scope),
                                String::from("Clear"),
                                Some(::ducktape_view_guest::slots::message(cb_13())),
                                wire::ButtonPreset::Secondary,
                            ),
                            wire::Edges::all(5.0f32),
                        ),
                        {
                            let node_scope = format!("{}/copy-range", node_scope);
                            native::padded(
                                native::button(
                                    node_scope.clone(),
                                    String::from("Copy"),
                                    Some(::ducktape_view_guest::slots::message(cb_18())),
                                    wire::ButtonPreset::Secondary,
                                ),
                                wire::Edges::all(5.0f32),
                            )
                        },
                    ];
                    wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:195", use_scope),
                        wrap: None,
                        axis: wire::Axis::Row,
                        spacing: Some(9.0f32),
                        padding: None,
                        width: Some(wire::Length::Fill),
                        height: None,
                        align: Some(wire::AlignX::Center),
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
                Some(wire::Length::Fill),
                None,
            ),
            wire::Edges {
                top: 7.0f32,
                right: 12.0f32,
                bottom: 7.0f32,
                left: 12.0f32,
            },
        )
    }
    pub(super) fn render_thread_selection(
        &self,
        use_scope: String,
        cb_13: impl Fn() -> Message + Clone + 'static,
        cb_18: impl Fn() -> Message + Clone + 'static,
    ) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        native::padded(
            native::sized(
                native::container(node_scope.clone(), {
                    let children: Vec<wire::Node> = vec![
                        native::text_options(
                            native::text(
                                format!("{}/@text:200", use_scope),
                                crate::host::copy_range_label(crate::host::copy_range_count(
                                    ::std::convert::AsRef::as_ref(&self.thread_messages),
                                    self.copy_anchor_seq,
                                    self.copy_head_seq,
                                ))
                                .to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                        wire::Node::Space {
                            width: Some(wire::Length::Fill),
                            height: None,
                        },
                        native::padded(
                            native::button(
                                format!("{}/@button:207", use_scope),
                                String::from("Clear"),
                                Some(::ducktape_view_guest::slots::message(cb_13())),
                                wire::ButtonPreset::Secondary,
                            ),
                            wire::Edges::all(5.0f32),
                        ),
                        {
                            let node_scope = format!("{}/copy-range", node_scope);
                            native::padded(
                                native::button(
                                    node_scope.clone(),
                                    String::from("Copy"),
                                    Some(::ducktape_view_guest::slots::message(cb_18())),
                                    wire::ButtonPreset::Secondary,
                                ),
                                wire::Edges::all(5.0f32),
                            )
                        },
                    ];
                    wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:195", use_scope),
                        wrap: None,
                        axis: wire::Axis::Row,
                        spacing: Some(9.0f32),
                        padding: None,
                        width: Some(wire::Length::Fill),
                        height: None,
                        align: Some(wire::AlignX::Center),
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
                Some(wire::Length::Fill),
                None,
            ),
            wire::Edges {
                top: 7.0f32,
                right: 12.0f32,
                bottom: 7.0f32,
                left: 12.0f32,
            },
        )
    }
    pub(super) fn chat_screen(&self, use_scope: String) -> wire::Node {
        if !self.connected {
            return self.disconnected(format!("{use_scope}/disconnected"));
        }
        let _component_owner =
            ::ducktape_view_guest::slots::component("ChatScreen", &use_scope, false);
        {
            let children: Vec<wire::Node> = vec![
                {
                    let node_scope = format!("{}/channel-sidebar", use_scope);
                    wire::Node::Container {
                        shadow: Default::default(),
                        max_width: None,
                        max_height: None,
                        clip: true,
                        key: node_scope.clone(),
                        width: Some(wire::Length::Fixed(self.sidebar_width as f32)),
                        height: Some(wire::Length::Fill),
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: None,
                        border: None,
                        snap: None,
                        content: Box::new({
                            let children: Vec<wire::Node> = vec![
                                native::padded(
                                    native::sized(
                                        native::container(
                                            format!("{}/@container:290", use_scope),
                                            {
                                                let mut children: Vec<wire::Node> =
                                                    vec![native::text_options(
                                                        native::text(
                                                            format!("{}/@text:302", use_scope),
                                                            self.network_name
                                                                .to_owned()
                                                                .to_string(),
                                                        ),
                                                        wire::TextOptions {
                                                            wrapping: Some(wire::Wrapping::None),
                                                            ..Default::default()
                                                        },
                                                    )];
                                                if crate::host::connection_degraded(
                                                    ::std::convert::AsRef::as_ref(&self.status),
                                                ) {
                                                    children.push(native::sized(
                                                        native::container(
                                                            format!("{}/@container:309", use_scope),
                                                            wire::Node::Space {
                                                                width: Some(wire::Length::Fixed(
                                                                    1.0f32,
                                                                )),
                                                                height: Some(wire::Length::Fixed(
                                                                    1.0f32,
                                                                )),
                                                            },
                                                        ),
                                                        Some(wire::Length::Fixed(7.0f32)),
                                                        Some(wire::Length::Fixed(7.0f32)),
                                                    ));
                                                }
                                                if !crate::host::connection_degraded(
                                                    ::std::convert::AsRef::as_ref(&self.status),
                                                ) {
                                                    children.push(native::sized(
                                                        native::container(
                                                            format!("{}/@container:317", use_scope),
                                                            wire::Node::Space {
                                                                width: Some(wire::Length::Fixed(
                                                                    1.0f32,
                                                                )),
                                                                height: Some(wire::Length::Fixed(
                                                                    1.0f32,
                                                                )),
                                                            },
                                                        ),
                                                        Some(wire::Length::Fixed(7.0f32)),
                                                        Some(wire::Length::Fixed(7.0f32)),
                                                    ));
                                                }
                                                children.push(wire::Node::Space {
                                                    width: Some(wire::Length::Fill),
                                                    height: None,
                                                });
                                                children.push(native::text_options(
                                                    native::text(
                                                        format!("{}/@text:325", use_scope),
                                                        crate::host::height_label(
                                                            self.block_height,
                                                        )
                                                        .to_string(),
                                                    ),
                                                    wire::TextOptions {
                                                        wrapping: Some(wire::Wrapping::None),
                                                            ..Default::default()
                                                    },
                                                ));
                                                wire::Node::Linear {
                                                    max_width: None,
                                                    clip: false,
                                                    key: format!("{}/@layout:296", use_scope),
                                                    wrap: None,
                                                    axis: wire::Axis::Row,
                                                    spacing: Some(8.0f32),
                                                    padding: None,
                                                    width: Some(wire::Length::Fill),
                                                    height: Some(wire::Length::Fill),
                                                    align: Some(wire::AlignX::Center),
                                                    background: None,
                                                    border: None,
                                                    children: children,
                                                }
                                            },
                                        ),
                                        Some(wire::Length::Fill),
                                        Some(wire::Length::Fixed(50.0f32)),
                                    ),
                                    wire::Edges {
                                        top: 0.0f32,
                                        right: 16.0f32,
                                        bottom: 0.0f32,
                                        left: 16.0f32,
                                    },
                                ),
                                native::sized(
                                    native::container(
                                        format!("{}/@container:331", use_scope),
                                        wire::Node::Space {
                                            width: Some(wire::Length::Fixed(1.0f32)),
                                            height: Some(wire::Length::Fixed(1.0f32)),
                                        },
                                    ),
                                    Some(wire::Length::Fill),
                                    Some(wire::Length::Fixed(1.0f32)),
                                ),
                                native::padded(
                                    native::sized(
                                        native::container(
                                            format!("{}/@container:337", use_scope),
                                            {
                                                let mut children: Vec<wire::Node> =
                                                    vec![{
                                                        let node_scope =
                                                            format!("{}/chat-search", node_scope);
                                                        wire::Node::Input {
                options : wire::InputOptions { label : "Search messages".to_owned()
                .to_string(), description : None, disabled : ! self.connected, padding :
                Some(wire::Edges::all(6.2f32)), text_size : Some(13.0f32), line_height :
                Some(1.2f32), align : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Normal,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                }, key : node_scope.clone(), placeholder : String::from("Search…"
                .to_owned()), value : self.search_draft.to_string(), on_input :
                ::ducktape_view_guest::slots::handler:: < String, Message, > (Box::new({
                let route = Message::SearchDraftChanged as fn (String) -> Message; move |
                sent : String | Some(route(sent)) }),), on_submit :
                Some(::ducktape_view_guest::slots::message(Message::SearchChatSubmit,),),
                width : Some(wire::Length::Fill), secure : false, style :
                Default::default(), }
                                                    }];
                                                if self.search_phase != SearchPhase::Idle
                                                    || !self
                                                        .search_draft
                                                        .trim()
                                                        .to_owned()
                                                        .is_empty()
                                                {
                                                    children
                .push(wire::Node::Button { checked : None, expanded : None, description :
                None, key : format!("{}/@button:379", use_scope), content :
                wire::ButtonContent::Child(Box::new(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:386", use_scope), width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), padding :
                None, align_x : Some(wire::AlignX::Center), align_y :
                Some(wire::AlignY::Center), background : None
                .map(wire::Background::Color), border : None, snap : None, content :
                Box::new(native::text_options(native::text(format!("{}/@text:392",
                use_scope), "×".to_owned().to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },),), }),), label :
                Some(String::from("Clear message search".to_owned()),), on_press :
                Some(::ducktape_view_guest::slots::message(Message::ClearChatSearch,),),
                width : Some(wire::Length::Fixed(27.0f32)), height :
                Some(wire::Length::Fixed(27.0f32)), padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), });
                                                }
                                                wire::Node::Linear {
                                                    max_width: None,
                                                    clip: false,
                                                    key: format!("{}/@layout:347", use_scope),
                                                    wrap: None,
                                                    axis: wire::Axis::Row,
                                                    spacing: Some(6.0f32),
                                                    padding: None,
                                                    width: Some(wire::Length::Fill),
                                                    height: Some(wire::Length::Fixed(31.0f32)),
                                                    align: Some(wire::AlignX::Center),
                                                    background: None,
                                                    border: None,
                                                    children: children,
                                                }
                                            },
                                        ),
                                        Some(wire::Length::Fill),
                                        None,
                                    ),
                                    wire::Edges {
                                        top: 11.0f32,
                                        right: 16.0f32,
                                        bottom: 6.0f32,
                                        left: 16.0f32,
                                    },
                                ),
                                native::padded(
                                    native::sized(
                                        native::container(
                                            format!("{}/@container:396", use_scope),
                                            {
                                                let mut children: Vec<wire::Node> = vec![
                                                    native::text_options(
                                                        native::text(
                                                            format!("{}/@text:408", use_scope),
                                                            "CHANNELS".to_owned().to_string(),
                                                        ),
                                                        wire::TextOptions {
                                                            wrapping: Some(wire::Wrapping::None),
                                                            ..Default::default()
                                                        },
                                                    ),
                                                    wire::Node::Space {
                                                        width: Some(wire::Length::Fill),
                                                        height: None,
                                                    },
                                                    native::text_options(
                                                        native::text(
                                                            format!("{}/@text:415", use_scope),
                                                            (self.rooms.len() as i64).to_string(),
                                                        ),
                                                        wire::TextOptions {
                                                            wrapping: Some(wire::Wrapping::None),
                                                            ..Default::default()
                                                        },
                                                    ),
                                                ];
                                                if !self.channel_create_open {
                                                    children.push(wire::Node::Button { checked : None,
                expanded : Some(self.channel_create_open), description : None, key :
                format!("{}/@button:422", use_scope), content :
                wire::ButtonContent::Child(Box::new({ let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "plus")),); wire::Node::Svg
                { inherit_button_ink : true, key : format!("{}/@media:435", use_scope),
                hash : hash, bytes : bytes, label : None, color : None, hover : None, fit
                : None, rotation : None, opacity : None, width :
                Some(wire::Length::Fixed(16.0f32)), height :
                Some(wire::Length::Fixed(16.0f32)), } }),), label :
                Some(String::from("New channel".to_owned())), on_press : if self.loading
                || self.busy || ! self.connected { None } else {
                Some(::ducktape_view_guest::slots::message(Message::ToggleChannelCreate,),)
                }, width : None, height : None, padding : Some(wire::Edges::all(0.0f32)),
                style : wire::ButtonStyle::default(), });
                                                }
                                                if self.channel_create_open {
                                                    children.push(wire::Node::Button { checked : None, expanded : Some(self
                .channel_create_open), description : None, key :
                format!("{}/@button:444", use_scope), content :
                wire::ButtonContent::Child(Box::new(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:453", use_scope), width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), padding :
                None, align_x : Some(wire::AlignX::Center), align_y :
                Some(wire::AlignY::Center), background : None
                .map(wire::Background::Color), border : None, snap : None, content :
                Box::new(native::text_options(native::text(format!("{}/@text:459",
                use_scope), "×".to_owned().to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },),), }),), label :
                Some(String::from("Close new channel".to_owned())), on_press : if self
                .loading || self.busy { None } else {
                Some(::ducktape_view_guest::slots::message(Message::ToggleChannelCreate,),)
                }, width : Some(wire::Length::Fixed(24.0f32)), height :
                Some(wire::Length::Fixed(24.0f32)), padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), });
                                                }
                                                wire::Node::Linear {
                                                    max_width: None,
                                                    clip: false,
                                                    key: format!("{}/@layout:403", use_scope),
                                                    wrap: None,
                                                    axis: wire::Axis::Row,
                                                    spacing: Some(6.0f32),
                                                    padding: None,
                                                    width: Some(wire::Length::Fill),
                                                    height: None,
                                                    align: Some(wire::AlignX::Center),
                                                    background: None,
                                                    border: None,
                                                    children: children,
                                                }
                                            },
                                        ),
                                        Some(wire::Length::Fill),
                                        None,
                                    ),
                                    wire::Edges {
                                        top: 14.0f32,
                                        right: 16.0f32,
                                        bottom: 6.0f32,
                                        left: 16.0f32,
                                    },
                                ),
                                wire::Node::Scroll {
                                    on_scroll: None,
                                    virtual_rows: false,
                                    key: format!("{}/@layout:463", use_scope),
                                    direction: wire::ScrollDirection::Vertical,
                                    width: Some(wire::Length::Fill),
                                    height: Some(wire::Length::Fill),
                                    bar_hidden: true,
                                    bar_width: None,
                                    bar_margin: None,
                                    scroller_width: None,
                                    bar_spacing: None,
                                    anchor_x: wire::ScrollAnchor::Start,
                                    anchor_y: wire::ScrollAnchor::Start,
                                    auto_scroll: false,
                                    background: None,
                                    border: None,
                                    content: Box::new({
                                        let mut children: Vec<wire::Node> = Vec::new();
                                        for (index, room) in self.rooms.iter().enumerate() {
                                            let for_scope =
                                                format!("{}/@for:3076({})", use_scope, index);
                                            children.push(
                                                self.channel_button(
                                                    format!("{}/ChannelButton@3077", for_scope),
                                                    (move |event_0| {
                                                        Message::ChooseChannel(event_0)
                                                    })
                                                    .clone(),
                                                    room.channel.clone(),
                                                    room.channel.id == self.active_channel,
                                                    room.unread,
                                                ),
                                            );
                                        }
                                        if !self.dm_rows.is_empty() {
                                            children
                .push(native::padded(native::sized(native::container(format!("{}/@container:491",
                use_scope), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:503",
                use_scope), "DIRECT".to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },),
                wire::Node::Space { width : Some(wire::Length::Fill), height : None, },
                native::text_options(native::text(format!("{}/@text:510", use_scope),
                (self.dm_rows.len() as i64).to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },)]; wire::Node::Linear {
                max_width : None, clip : false, key : format!("{}/@layout:498",
                use_scope), wrap : None, axis : wire::Axis::Row, spacing : Some(6.0f32),
                padding : None, width : Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } },), Some(wire::Length::Fill), None,), wire::Edges { top :
                14.0f32, right : 16.0f32, bottom : 6.0f32, left : 16.0f32, },),);
                                        }
                                        for (index, dm) in self.dm_rows.iter().enumerate() {
                                            let for_scope =
                                                format!("{}/@for:3119({})", use_scope, index);
                                            children.push(self.direct_message(
                                                format!("{}/DmButton@3120", for_scope),
                                                (move |event_0| Message::ChooseDm(event_0)).clone(),
                                                dm.peer.clone(),
                                                dm.peer.key == self.active_dm_peer,
                                                dm.unread,
                                            ));
                                        }
                                        native::spaced(
                                            native::sized(
                                                native::column(
                                                    format!("{}/@layout:469", use_scope),
                                                    children,
                                                ),
                                                Some(wire::Length::Fill),
                                                None,
                                            ),
                                            2.0f32,
                                        )
                                    }),
                                },
                            ];
                            native::sized(
                                native::column(format!("{}/@layout:289", use_scope), children),
                                Some(wire::Length::Fill),
                                Some(wire::Length::Fill),
                            )
                        }),
                    }
                },
                {
                    let node_scope = format!("{}/sidebar-resize", use_scope);
                    wire::Node::ResizeHandle {
                        key: node_scope.clone(),
                        on_press: None,
                        on_release: None,
                        on_drag: Some(
                            ::ducktape_view_guest::slots::handler::<(f64, f64), Message>(Box::new(
                                {
                                    let route = {
                                        let _route_state_scope_0 = use_scope.clone();
                                        let route_callback = (move |event_0, event_1| {
                                            Message::SidebarResized(event_0, event_1)
                                        })
                                        .clone();
                                        move |delta: (f64, f64)| route_callback(delta.0, delta.1)
                                    };
                                    move |sent: (f64, f64)| Some(route(sent))
                                },
                            )),
                        ),
                        cursor: Some(wire::mouse::Cursor::ResizingHorizontally),
                        content: Box::new({
                            let node_scope = format!("{}/sidebar-divider", node_scope);
                            wire::Node::Container {
                                shadow: Default::default(),
                                max_width: None,
                                max_height: None,
                                clip: false,
                                key: node_scope.clone(),
                                width: Some(wire::Length::Fixed(10.0f32)),
                                height: Some(wire::Length::Fill),
                                padding: None,
                                align_x: Some(wire::AlignX::Left),
                                align_y: None,
                                background: None.map(wire::Background::Color),
                                border: None,
                                snap: None,
                                content: Box::new(native::sized(
                                    native::container(
                                        format!("{}/@container:534", use_scope),
                                        wire::Node::Space {
                                            width: Some(wire::Length::Fixed(1.0f32)),
                                            height: Some(wire::Length::Fixed(1.0f32)),
                                        },
                                    ),
                                    Some(wire::Length::Fixed(1.0f32)),
                                    Some(wire::Length::Fill),
                                )),
                            }
                        }),
                    }
                },
                wire::Node::Container {
                    shadow: Default::default(),
                    max_width: None,
                    max_height: None,
                    clip: true,
                    key: format!("{}/@container:536", use_scope),
                    width: Some(wire::Length::Fill),
                    height: Some(wire::Length::Fill),
                    padding: None,
                    align_x: None,
                    align_y: None,
                    background: None,
                    border: None,
                    snap: Some(true),
                    content: Box::new({
                        let mut children: Vec<wire::Node> = vec![{
                            let mut children: Vec<wire::Node> = Vec::new();
                            if !self.active_channel.is_empty() {
                                children
                .push({ let children : Vec < wire::Node > =
                vec![native::padded(native::sized(native::container(format!("{}/@container:547",
                use_scope), { let mut children : Vec < wire::Node > = Vec::new(); if !
                self.active_dm.name.is_empty() { children.push(wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                true, key : format!("{}/@container:581", use_scope), width :
                Some(wire::Length::Fill), height : None, padding : None, align_x : None,
                align_y : None, background : None.map(wire::Background::Color), border :
                None, snap : None, content : Box::new(self
                .direct_message_header(format!("{}/DmHeader@3185", use_scope)),), }); } if
                self.active_dm.name.is_empty() { children
                .push(native::text_options(native::text(format!("{}/@text:584",
                use_scope), "#".to_owned().to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },),); } if self.active_dm
                .name.is_empty() { children.push(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : true, key
                : format!("{}/@container:600", use_scope), width :
                Some(wire::Length::Fill), height : None, padding : None, align_x : None,
                align_y : None, background : None.map(wire::Background::Color), border :
                None, snap : None, content :
                Box::new(native::text_options(native::text(format!("{}/@text:601",
                use_scope), self.active_channel_name.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), }); } if self.active_channel_archived {
                children.push(self.archived_badge(format!("{}/Badge.Outline@3211",
                use_scope),),); } if self.active_channel_members_only { children
                .push(self.private_badge(format!("{}/Badge.Outline@3213", use_scope),),);
                } if self.huddle_joined && self.huddle_channel == self.active_channel {
                children.push(self.huddle_controls(format!("{}/HuddleLivePill@3220",
                use_scope), (move | | Message::LeaveHuddleHere).clone(), (move | |
                Message::ShowHuddle).clone(),),); } if ! self.huddle_joined && ! self
                .active_channel_archived { children.push(self
                .start_huddle(format!("{}/HuddleStart@3225", use_scope), (move | |
                Message::JoinHuddleSubmit).clone(),),); } if ! self.channel_members
                .is_empty() { children.push({ let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:636",
                use_scope), "·".to_owned().to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:641", use_scope),
                (self.channel_members.len() as i64).to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:647", use_scope),
                "added".to_owned().to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },)]; wire::Node::Linear {
                max_width : None, clip : false, key : format!("{}/@layout:635",
                use_scope), wrap : None, axis : wire::Axis::Row, spacing : Some(4.0f32),
                padding : None, width : None, height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }); } children.push(wire::Node::Button { checked : None,
                expanded : Some(self.channel_settings_open), description : None, key :
                format!("{}/@button:660", use_scope), content :
                wire::ButtonContent::Child(Box::new(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:668", use_scope), width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), padding :
                None, align_x : Some(wire::AlignX::Center), align_y :
                Some(wire::AlignY::Center), background : None
                .map(wire::Background::Color), border : None, snap : None, content :
                Box::new(native::text_options(native::text(format!("{}/@text:674",
                use_scope), "⋯".to_owned().to_string(),), wire::TextOptions { wrapping
                : Some(wire::Wrapping::None), ..Default::default() },),), }),), label :
                Some(String::from("Channel details".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(Message::ToggleChannelSettings,),),
                width : Some(wire::Length::Fixed(27.0f32)), height :
                Some(wire::Length::Fixed(25.0f32)), padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), });
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:553", use_scope), wrap : None, axis :
                wire::Axis::Row, spacing : Some(9.0f32), padding : None, width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } },), Some(wire::Length::Fill),
                Some(wire::Length::Fixed(50.0f32)),), wire::Edges { top : 0.0f32, right :
                18.0f32, bottom : 0.0f32, left : 18.0f32, },),
                native::sized(native::container(format!("{}/@container:678", use_scope),
                wire::Node::Space { width : Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(1.0f32)), },), Some(wire::Length::Fill),
                Some(wire::Length::Fixed(1.0f32)),)];
                native::sized(native::column(format!("{}/@layout:546", use_scope),
                children,), Some(wire::Length::Fill), None,) });
                            }
                            if self.copy_surface == CopySurface::Timeline
                                && crate::host::copy_range_count(
                                    ::std::convert::AsRef::as_ref(&self.messages),
                                    self.copy_anchor_seq,
                                    self.copy_head_seq,
                                ) > 0
                            {
                                children.push({
                                    let node_scope = format!("{}/timeline-selection", use_scope);
                                    self.render_timeline_selection(
                                        node_scope.clone(),
                                        (move || Message::ClearCopyRange).clone(),
                                        (move || Message::CopySelectedMessages).clone(),
                                    )
                                });
                            }
                            children.push({ let mut
                children : Vec < wire::Node > = vec![{ let mut children : Vec <
                wire::Node > = Vec::new(); if ! self.connected { children.push(self
                .disconnected(format!("{}/EmptyState@3303", use_scope),),); } if self
                .connected && ! self.loading && self.messages.is_empty() { children
                .push(self.empty_messages(format!("{}/EmptyState@3308", use_scope),),); }
                if self.connected && self.loading && self.messages.is_empty() { children
                .push({ let children : Vec < wire::Node > = vec![self
                .loading_messages(format!("{}/SkeletonRow@3320", use_scope),), self
                .loading_messages(format!("{}/SkeletonRow@3321", use_scope),), self
                .loading_messages(format!("{}/SkeletonRow@3322", use_scope),)];
                native::spaced(native::padded(native::sized(native::column(format!("{}/@layout:712",
                use_scope), children,), Some(wire::Length::Fill), None,), wire::Edges {
                top : 4.0f32, right : 0.0f32, bottom : 0.0f32, left : 0.0f32, },),
                14.0f32,) }); } if self.connected && ! self.messages.is_empty() {
                children.push({ let children : Vec < wire::Node > =
                vec![wire::Node::Sensor { key : format!("{}/@sensor:731", use_scope),
                reset : None, on_show : Some(::ducktape_view_guest::slots::handler:: <
                (f32, f32), Message, > (Box::new({ let route = { let route_scope =
                use_scope.clone(); move | size : (f64, f64) |
                Message::ChatScreenChatResized(route_scope.clone(), size.0, size.1,) };
                move | sent : (f32, f32) | Some(route((f64::from(sent.0), f64::from(sent
                .1))),) }),),), on_resize : Some(::ducktape_view_guest::slots::handler::
                < (f32, f32), Message, > (Box::new({ let route = { let route_scope =
                use_scope.clone(); move | size : (f64, f64) |
                Message::ChatScreenChatResized(route_scope.clone(), size.0, size.1,) };
                move | sent : (f32, f32) | Some(route((f64::from(sent.0), f64::from(sent
                .1))),) }),),), on_hide : None, anticipate : None, delay : None, child :
                Box::new(wire::Node::Space { width : Some(wire::Length::Fill), height :
                Some(wire::Length::Fill), }), }, wire::Node::MouseArea { key :
                format!("{}/@mouse:733", use_scope), on_press : None, on_release : None,
                on_double_click : None, on_right_press : None, on_right_release : None,
                on_middle_press : None, on_middle_release : None, on_enter : None,
                on_exit : None, on_move : None, on_press_at :
                Some(::ducktape_view_guest::slots::handler:: < (f32, f32), Message, >
                (Box::new({ let route = { let route_scope = use_scope.clone(); move |
                point : (f64, f64) | Message::ChatScreenChatPointerPressed(route_scope
                .clone(), point.0, point.1,) }; move | sent : (f32, f32) |
                Some(route((f64::from(sent.0), f64::from(sent.1))),) }),),), on_scroll :
                None, content : Box::new(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:743", use_scope), width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), padding :
                None, align_x : None, align_y : Some(wire::AlignY::Bottom), background :
                None.map(wire::Background::Color), border : None, snap : None, content :
                Box::new({ let node_scope = format!("{}/message-stream", use_scope);
                wire::Node::Scroll { on_scroll :
                Some(::ducktape_view_guest::slots::handler:: < (f32, f32, f32, f32),
                Message, > (Box::new({ let route = { let _route_state_scope_0 = use_scope
                .clone(); let route_callback = (move | event_0, event_1, event_2, event_3
                | Message::ChatScrolled(event_0, event_1, event_2, event_3)).clone();
                move | offset : (f64, f64, f64, f64) | route_callback(offset.0, offset.1,
                offset.2, offset.3,) }; move | sent : (f32, f32, f32, f32) |
                Some(route((f64::from(sent.0), f64::from(sent.1), f64::from(sent.2),
                f64::from(sent.3),)),) }),),), virtual_rows : true, key : node_scope
                .clone(), direction : wire::ScrollDirection::Vertical, width :
                Some(wire::Length::Fill), height : Some(wire::Length::Shrink), bar_hidden
                : false, bar_width : None, bar_margin : None, scroller_width : None,
                bar_spacing : None, anchor_x : wire::ScrollAnchor::Start, anchor_y :
                wire::ScrollAnchor::End, auto_scroll : ! self.history_view, background :
                None, border : None, content : Box::new({ let mut children : Vec <
                wire::Node > = Vec::new(); if self.has_older_history && self
                .history_loading { children.push(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:779", use_scope), width :
                Some(wire::Length::Fill), height : None, padding : Some(wire::Edges { top
                : 4.0f32, right : 0.0f32, bottom : 8.0f32, left : 0.0f32, }), align_x :
                Some(wire::AlignX::Center), align_y : None, background : None
                .map(wire::Background::Color), border : None, snap : None, content :
                Box::new(native::padded(native::button(format!("{}/@button:785",
                use_scope), String::from("Loading older messages…"), None,
                wire::ButtonPreset::Secondary,), wire::Edges::all(6.0f32),),), }); } if
                self.has_older_history && ! self.history_loading { children
                .push(wire::Node::Container { shadow : Default::default(), max_width :
                None, max_height : None, clip : false, key : format!("{}/@container:794",
                use_scope), width : Some(wire::Length::Fill), height : None, padding :
                Some(wire::Edges { top : 4.0f32, right : 0.0f32, bottom : 8.0f32, left :
                0.0f32, }), align_x : Some(wire::AlignX::Center), align_y : None,
                background : None.map(wire::Background::Color), border : None, snap :
                None, content :
                Box::new(native::padded(native::button(format!("{}/@button:800",
                use_scope), String::from("Load older messages"), if self.busy { None }
                else {
                Some(::ducktape_view_guest::slots::message(Message::LoadMoreHistory,),)
                }, wire::ButtonPreset::Secondary,), wire::Edges::all(6.0f32),),), }); }
                children.push({ let _lazy_context_543 = use_scope.to_owned(); let
                _lazy_event_543_0 = (move | event_0 | Message::CancelRun(event_0,))
                .clone(); let lazy_event_543_1 = (move | event_0, event_1 |
                Message::PressMessage(event_0, event_1,)).clone(); let _lazy_event_543_2
                = (move | | Message::ClearCopyRange).clone(); let _lazy_event_543_3 =
                (move | | { Message::CopySelectedMessages }).clone(); let
                _lazy_event_543_4 = (move | | Message::SearchChatSubmit).clone(); let
                _lazy_event_543_5 = (move | | Message::ClearChatSearch).clone(); let
                _lazy_event_543_6 = (move | event_0, event_1, event_2 |
                Message::OpenChatSearchHit(event_0, event_1, event_2,)).clone(); let
                _lazy_event_543_7 = (move | | { Message::ToggleChannelCreate }).clone();
                let _lazy_event_543_8 = (move | event_0 |
                Message::ChooseChannel(event_0,)).clone(); let _lazy_event_543_9 = (move
                | event_0 | Message::ChooseDm(event_0,)).clone(); let _lazy_event_543_10
                = (move | | { Message::ToggleChannelSettings }).clone(); let
                _lazy_event_543_11 = (move | | Message::ShowHuddle).clone(); let
                _lazy_event_543_12 = (move | | Message::LeaveHuddleHere).clone(); let
                _lazy_event_543_13 = (move | | Message::JoinHuddleSubmit).clone(); let
                _lazy_event_543_14 = (move | | Message::LoadMoreHistory).clone(); let
                _lazy_event_543_15 = (move | event_0, event_1, event_2, event_3 |
                Message::ChatScrolled(event_0, event_1, event_2, event_3)).clone(); let
                lazy_event_543_16 = (move | event_0 | Message::OpenMessageLink(event_0,))
                .clone(); let lazy_event_543_17 = (move | event_0 |
                Message::OpenRun(event_0,)).clone(); let _lazy_event_543_18 = (move |
                event_0, event_1 | Message::CopyToClipboard(event_0, event_1,)).clone();
                let _lazy_event_543_19 = (move | event_0 |
                Message::CopyMessageLink(event_0,)).clone(); let lazy_event_543_20 =
                (move | event_0, event_1 | Message::AddReactionAt(event_0, event_1,))
                .clone(); let lazy_event_543_21 = (move | event_0, event_1 |
                Message::RemoveReactionAt(event_0, event_1,)).clone(); let
                lazy_event_543_22 = (move | event_0 | Message::OpenThreadFor(event_0,))
                .clone(); let lazy_event_543_23 = (move | event_0, event_1, event_2 |
                Message::OpenMessageActions(event_0, event_1, event_2,)).clone(); let
                lazy_event_543_24 = (move | event_0, event_1, event_2 |
                Message::OpenMessageReactions(event_0, event_1, event_2,)).clone(); let
                _lazy_event_543_25 = (move | event_0, event_1, event_2 |
                Message::BeginMessageEdit(event_0, event_1, event_2,)).clone(); let
                _lazy_event_543_26 = (move | event_0, event_1, event_2 |
                Message::ArmMessageDelete(event_0, event_1, event_2,)).clone(); let
                _lazy_event_543_27 = (move | | { Message::ClearMessageSelection })
                .clone(); let _lazy_event_543_28 = (move | event_0 |
                Message::AddReactionSubmit(event_0,)).clone(); let _lazy_event_543_29 =
                (move | | { Message::DeleteMessageSubmit }).clone(); let
                _lazy_event_543_30 = (move | | { Message::RenameChannelSubmit }).clone();
                let _lazy_event_543_31 = (move | | { Message::ArchiveChannelSubmit })
                .clone(); let _lazy_event_543_32 = (move | | {
                Message::UnarchiveChannelSubmit }).clone(); let _lazy_event_543_33 =
                (move | | { Message::AddChannelMemberSubmit }).clone(); let
                _lazy_event_543_34 = (move | event_0 |
                Message::RemoveChannelMemberSubmit(event_0,)).clone(); let
                _lazy_event_543_35 = (move | | Message::CloseThread).clone(); let
                _lazy_event_543_36 = (move | event_0, event_1 |
                Message::SidebarResized(event_0, event_1,)).clone(); let
                _lazy_event_543_37 = (move | event_0, event_1 |
                Message::DetailsResized(event_0, event_1,)).clone(); let
                _lazy_event_543_38 = (move | event_0, event_1 |
                Message::ThreadResized(event_0, event_1,)).clone(); let
                _lazy_event_543_39 = (move | event_0, event_1, event_2 |
                Message::OpenThreadMessageActions(event_0, event_1, event_2,)).clone();
                let _lazy_event_543_40 = (move | event_0, event_1, event_2 |
                Message::OpenThreadMessageReactions(event_0, event_1, event_2,)).clone();
                let _lazy_event_543_41 = (move | event_0, event_1, event_2 |
                Message::BeginThreadMessageEdit(event_0, event_1, event_2,)).clone(); let
                _lazy_event_543_42 = (move | event_0, event_1, event_2 |
                Message::ArmThreadMessageDelete(event_0, event_1, event_2,)).clone(); let
                _lazy_event_543_43 = (move | | { Message::ClearThreadMessageSelection })
                .clone(); let _lazy_event_543_44 = (move | | {
                Message::DeleteThreadMessageSubmit }).clone(); let _lazy_event_543_45 =
                (move | | Message::LoadMoreThread).clone(); { let lazy_key =
                format!("{}/@lazy:861", use_scope);
                ::ducktape_view_guest::memo_lazy((self.active_channel.to_owned(), self
                .unread_boundary, self.unread_marker_seq, self.selected_message_seq, self
                .copy_anchor_seq, self.copy_head_seq, self.copy_surface.clone(), self
                .timeline_revision, node_scope.to_owned(), (),), move |
                dependency | { let _active_channel : String = dependency.0.clone(); let
                unread_boundary : i64 = dependency.1.clone(); let unread_marker_seq : i64
                = dependency.2.clone(); let selected_message_seq : i64 = dependency.3
                .clone(); let copy_anchor_seq : i64 = dependency.4.clone(); let
                copy_head_seq : i64 = dependency.5.clone(); let copy_surface :
                CopySurface = dependency.6.clone(); let lazy_scope = dependency.8
                .clone(); let cached_timeline : crate ::host::Timeline = self.timeline
                .clone(); { let message_timeline_scope_3465 =
                format!("{}/MessageTimeline@3465", lazy_scope); { let mut children : Vec
                < _ > = Vec::new(); for message in cached_timeline.messages.iter() { let
                key = message.view_key; let key_recon = format!("{}/key({})",
                message_timeline_scope_3465, key); let child : wire::Node = { let mut
                children : Vec < wire::Node > = Vec::new(); if unread_boundary > 0 &&
                message.seq == unread_marker_seq { children.push({ let children : Vec
                < wire::Node > =
                vec![native::sized(native::container(format!("{}/@container:50",
                key_recon), native::text(format!("{}/@text:55", key_recon), "".to_owned()
                .to_string(),),), Some(wire::Length::Fill),
                Some(wire::Length::Fixed(1.0f32)),),
                native::text_options(native::text(format!("{}/@text:56", key_recon),
                "NEW".to_owned().to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },),
                native::sized(native::container(format!("{}/@container:62", key_recon),
                native::text(format!("{}/@text:67", key_recon), "".to_owned()
                .to_string(),),), Some(wire::Length::Fill),
                Some(wire::Length::Fixed(1.0f32)),)]; wire::Node::Linear { max_width :
                None, clip : false, key : format!("{}/@layout:43", key_recon), wrap :
                None, axis : wire::Axis::Row, spacing : Some(8.0f32), padding :
                Some(wire::Edges { top : 8.0f32, right : 0.0f32, bottom : 2.0f32, left :
                0.0f32, }), width : Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }); } if message.seq == selected_message_seq { children
                .push({ let node_scope = format!("{}/message({})", format!("{}/key({})",
                message_timeline_scope_3465, key), message.id); { let children : Vec
                < wire::Node > = vec![{ let message_card_scope_2680 =
                format!("{}/MessageCard@2680", key_recon); { let mut children : Vec <
                wire::Node > = Vec::new(); if message.show_author { children
                .push(wire::Node::Space { width : Some(wire::Length::Fixed(1.0f32)),
                height : Some(wire::Length::Fixed(14.0f32)), }); } children.push({ let
                children : Vec < wire::Node > = vec![{ let mut children : Vec <
                wire::Node > = Vec::new(); match & crate ::host::message_plate(message
                .deleted, true, crate ::host::seq_in_copy_range(message.seq,
                copy_anchor_seq, copy_head_seq, copy_surface.clone(),
                CopySurface::Timeline,),) { RowPlate::Plain => { children
                .push(native::padded(native::sized(native::container(format!("{}/@container:571",
                message_card_scope_2680), { let message_contents_scope_1995 =
                format!("{}/MessageContents@1995", message_card_scope_2680); { let children : Vec < wire::Node > = vec![{ let mut children : Vec <
                wire::Node > = Vec::new(); if message.show_author { children.push({ let
                message_avatar_scope_1750 = format!("{}/MessageAvatar@1750",
                message_contents_scope_1995); { let node_scope = format!("{}/root",
                message_avatar_scope_1750); { let mut children : Vec < wire::Node > =
                Vec::new(); if message.avatar_kind == "human" { children.push({ let
                person_avatar_scope_798 = format!("{}/PersonAvatar@798",
                message_avatar_scope_1750); { let node_scope = format!("{}/root",
                person_avatar_scope_798); { let children : Vec < wire::Node > =
                vec![{ let principal_avatar_scope_863 = format!("{}/PrincipalAvatar@863",
                person_avatar_scope_798); { let node_scope = format!("{}/root",
                principal_avatar_scope_863); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let principal_plate_scope_909 =
                format!("{}/PrincipalPlate@909", principal_avatar_scope_863); { let
                node_scope = format!("{}/root", principal_plate_scope_909); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                human_plate_scope_925 = format!("{}/HumanPlate@925",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                human_plate_scope_925); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:328", human_plate_scope_925), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([((30.0 / 2.0) as f32).max(0.0).min(f32::MAX), ((30.0
                / 2.0) as f32).max(0.0).min(f32::MAX), ((30.0 / 2.0) as f32).max(0.0)
                .min(f32::MAX), ((30.0 / 2.0) as f32).max(0.0).min(f32::MAX),]), }), snap
                : None, content :
                Box::new(native::text_options(native::text(format!("{}/@text:336",
                human_plate_scope_925), message.initial.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), }); } native::column(node_scope.clone(),
                children) } } }); } native::column(node_scope.clone(), children) } } });
                } native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } if ! (message
                .avatar_kind == "human") && message.avatar_kind == "agent" { children
                .push({ let agent_avatar_scope_804 = format!("{}/AgentAvatar@804",
                message_avatar_scope_1750); { let node_scope = format!("{}/root",
                agent_avatar_scope_804); { let children : Vec < wire::Node > = vec![{
                let principal_avatar_scope_873 = format!("{}/PrincipalAvatar@873",
                agent_avatar_scope_804); { let node_scope = format!("{}/root",
                principal_avatar_scope_873); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let principal_plate_scope_909 =
                format!("{}/PrincipalPlate@909", principal_avatar_scope_873); { let
                node_scope = format!("{}/root", principal_plate_scope_909); { let children : Vec < wire::Node > = vec![{ let agent_plate_scope_919 =
                format!("{}/AgentPlate@919", principal_plate_scope_909); { let node_scope
                = format!("{}/root", agent_plate_scope_919); { let mut children : Vec <
                wire::Node > = Vec::new(); { children.push({ let agent_square_scope_1174
                = format!("{}/AgentSquare@1174", agent_plate_scope_919); { let node_scope
                = format!("{}/root", agent_square_scope_1174); wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : node_scope.clone(), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content :
                Box::new(native::text_options(native::text(format!("{}/@text:390",
                agent_square_scope_1174), message.initial.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), } } }); } native::column(node_scope.clone(),
                children) } } }]; native::column(node_scope.clone(), children) } } }); }
                native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } if ! (message
                .avatar_kind == "human" || message.avatar_kind == "agent") { children
                .push({ let agent_avatar_scope_810 = format!("{}/AgentAvatar@810",
                message_avatar_scope_1750); { let node_scope = format!("{}/root",
                agent_avatar_scope_810); { let children : Vec < wire::Node > = vec![{
                let principal_avatar_scope_873 = format!("{}/PrincipalAvatar@873",
                agent_avatar_scope_810); { let node_scope = format!("{}/root",
                principal_avatar_scope_873); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let principal_plate_scope_909 =
                format!("{}/PrincipalPlate@909", principal_avatar_scope_873); { let
                node_scope = format!("{}/root", principal_plate_scope_909); { let children : Vec < wire::Node > = vec![{ let agent_plate_scope_919 =
                format!("{}/AgentPlate@919", principal_plate_scope_909); { let node_scope
                = format!("{}/root", agent_plate_scope_919); { let mut children : Vec <
                wire::Node > = Vec::new(); { children.push({ let agent_square_scope_1174
                = format!("{}/AgentSquare@1174", agent_plate_scope_919); { let node_scope
                = format!("{}/root", agent_square_scope_1174); wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : node_scope.clone(), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content :
                Box::new(native::text_options(native::text(format!("{}/@text:390",
                agent_square_scope_1174), message.initial.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), } } }); } native::column(node_scope.clone(),
                children) } } }]; native::column(node_scope.clone(), children) } } }); }
                native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } wire::Node::Stack
                { key : node_scope.clone(), width : Some(wire::Length::Fixed(30.0f32)),
                height : Some(wire::Length::Fixed(30.0f32)), padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, } }
                } }); } if ! message.show_author { children.push(wire::Node::Space {
                width : Some(wire::Length::Fixed(30.0f32)), height : None, }); } children
                .push({ let mut children : Vec < wire::Node > = Vec::new(); if message
                .show_author { children.push({ let mut children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:347",
                message_contents_scope_1995), message.author.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; if message.avatar_kind == "agent" { children
                .push(native::padded(native::container(format!("{}/@container:354",
                message_contents_scope_1995),
                native::text_options(native::text(format!("{}/@text:360",
                message_contents_scope_1995), "AGENT".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), wire::Edges { top : 2.0f32, right : 5.0f32,
                bottom : 2.0f32, left : 5.0f32, },),); } if message.height > 0 { children
                .push(native::text_options(native::text(format!("{}/@text:377",
                message_contents_scope_1995), crate ::host::height_label_short(message
                .height).to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },),); } if message.edited
                { children.push(native::text_options(native::text(format!("{}/@text:384",
                message_contents_scope_1995), "· edited".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } children.push(wire::Node::Space { width :
                Some(wire::Length::Fill), height : None, }); wire::Node::Linear {
                max_width : None, clip : false, key : format!("{}/@layout:342",
                message_contents_scope_1995), wrap : None, axis : wire::Axis::Row,
                spacing : Some(7.0f32), padding : None, width : Some(wire::Length::Fill),
                height : None, align : Some(wire::AlignX::Center), background : None,
                border : None, children : children, } }); } children
                .push(wire::Node::MouseArea { key : format!("{}/@mouse:395",
                message_contents_scope_1995), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_543_1(message.seq,
                CopySurface::Timeline.clone(),),),), on_release : None, on_double_click :
                None, on_right_press : None, on_right_release : None, on_middle_press :
                None, on_middle_release : None, on_enter : None, on_exit : None, on_move
                : None, on_press_at : None, on_scroll : None, content : Box::new({ let
                message_body_scope_1809 = format!("{}/MessageBody@1809",
                message_contents_scope_1995); { let children : Vec < wire::Node > =
                vec![{ let rich_body_scope_688 = format!("{}/RichBody@688",
                message_body_scope_1809); { let mut children : Vec < wire::Node > =
                Vec::new(); for (index, block) in message.blocks.iter().enumerate() { let
                for_scope = format!("{}/@for:703({})", rich_body_scope_688, index); if
                block.kind == "divider" { children.push({ let
                component_separator_scope_705 = format!("{}/Separator@705", for_scope); {
                let node_scope = format!("{}/root", component_separator_scope_705);
                wire::Node::Rule { key : node_scope.clone(), axis : wire::Axis::Row,
                thickness : 1.0f32, color : None, weak : false, radius : None, snap :
                None, } } }); } if block.kind == "code" { children
                .push(native::padded(native::sized(native::container(format!("{}/@container:90",
                for_scope), { let mut children : Vec < wire::Node > = Vec::new(); if !
                block.lang.is_empty() { children
                .push(native::text_options(native::text(format!("{}/@text:100",
                for_scope), block.lang.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },),); }
                children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:106",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),);
                native::spaced(native::sized(native::column(format!("{}/@layout:98",
                for_scope), children,), Some(wire::Length::Fill), None,), 6.0f32,) },),
                Some(wire::Length::Fill), None,), wire::Edges { top : 11.0f32, right :
                11.0f32, bottom : 11.0f32, left : 11.0f32, },),); } if block.kind ==
                "quote" { children.push({ let children : Vec < wire::Node > = vec![{
                let mut children : Vec < wire::Node > = Vec::new(); if block.rich {
                children.push({ let rich_line_scope_755 = format!("{}/RichLine@755",
                for_scope); { let mut rich_spans : Vec < wire::RichSpan > = Vec::new();
                for span in block.spans.iter().cloned() { rich_spans.push(wire::RichSpan
                { content : span.mention.to_owned().to_string(), size : None, line_height
                : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Medium,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : Some(span.mention_link.to_owned()), background :
                None, border : Some(wire::Border { color : None, width : None, radius :
                Some([4.0f32, 4.0f32, 4.0f32, 4.0f32,]), }), padding : Some(wire::Edges {
                top : 0.0f32, right : 1.0f32, bottom : 0.0f32, left : 1.0f32, }),
                underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.link_text.to_owned().to_string(),
                size : None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Medium,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : Some(span.link.to_owned()), background : None,
                border : None, padding : None, underline : true, strikethrough : false,
                }); rich_spans.push(wire::RichSpan { content : span.bold_italic
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Bold, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Italic, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.bold
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Bold, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Normal, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.italic
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Normal, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Italic, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.plain
                .to_owned().to_string(), size : None, line_height : None, font : None,
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); }
                wire::Node::RichText { options : wire::TextOptions { wrapping :
                Some(wire::Wrapping::WordOrGlyph), ..Default::default() }, key :
                format!("{}/@text:32", rich_line_scope_755), size : Some(13.5f32), color
                : None, font : wire::Font { monospace : false, weight :
                wire::Weight::Normal, }, width : Some(wire::Length::Fill), align_x :
                None, spans : rich_spans, on_link :
                Some(::ducktape_view_guest::slots::handler:: < String, Message, >
                (Box::new({ let route = { let route_callback = lazy_event_543_16.clone();
                move | link : String | route_callback(link) }; move | sent : String |
                Some(route(sent)) }),),), } } }); } if ! block.rich { children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:137",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),); }
                native::padded(native::sized(native::column(format!("{}/@layout:121",
                for_scope), children,), Some(wire::Length::Fill), None,), wire::Edges {
                top : 2.0f32, right : 0.0f32, bottom : 2.0f32, left : 13.0f32, },) },
                native::sized(native::container(format!("{}/@container:144", for_scope),
                wire::Node::Space { width : Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(1.0f32)), },),
                Some(wire::Length::Fixed(3.0f32)), Some(wire::Length::Fill),)];
                wire::Node::Stack { key : format!("{}/@layout:120", for_scope), width :
                Some(wire::Length::Fill), height : None, padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, }
                }); } if block.kind == "paragraph" { if block.rich { children.push({ let
                rich_line_scope_780 = format!("{}/RichLine@780", for_scope); { let mut
                rich_spans : Vec < wire::RichSpan > = Vec::new(); for span in block.spans
                .iter().cloned() { rich_spans.push(wire::RichSpan { content : span
                .mention.to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Medium, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), color : None, link : Some(span
                .mention_link.to_owned()), background : None, border : Some(wire::Border
                { color : None, width : None, radius : Some([4.0f32, 4.0f32, 4.0f32,
                4.0f32,]), }), padding : Some(wire::Edges { top : 0.0f32, right : 1.0f32,
                bottom : 0.0f32, left : 1.0f32, }), underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.link_text
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Medium, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), color : None, link : Some(span.link
                .to_owned()), background : None, border : None, padding : None, underline
                : true, strikethrough : false, }); rich_spans.push(wire::RichSpan {
                content : span.bold_italic.to_owned().to_string(), size : None,
                line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Bold,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Italic, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.bold.to_owned().to_string(), size :
                None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Bold,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.italic.to_owned().to_string(), size
                : None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Normal,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Italic, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.plain.to_owned().to_string(), size
                : None, line_height : None, font : None, color : None, link : None,
                background : None, border : None, padding : None, underline : false,
                strikethrough : false, }); } wire::Node::RichText { options :
                wire::TextOptions { wrapping : Some(wire::Wrapping::WordOrGlyph),
                ..Default::default() }, key : format!("{}/@text:32",
                rich_line_scope_780), size : Some(13.5f32), color : None, font :
                wire::Font { monospace : false, weight : wire::Weight::Normal, }, width :
                Some(wire::Length::Fill), align_x : None, spans : rich_spans, on_link :
                Some(::ducktape_view_guest::slots::handler:: < String, Message, >
                (Box::new({ let route = { let route_callback = lazy_event_543_16.clone();
                move | link : String | route_callback(link) }; move | sent : String |
                Some(route(sent)) }),),), } } }); } if ! block.rich { children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:157",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),); } } }
                native::spaced(native::sized(native::column(format!("{}/@layout:75",
                rich_body_scope_688), children,), Some(wire::Length::Fill), None,),
                5.0f32,) } }]; wire::Node::Linear { max_width : Some(760.0f32), clip :
                false, key : format!("{}/@layout:60", message_body_scope_1809), wrap :
                None, axis : wire::Axis::Column, spacing : None, padding : None, width :
                Some(wire::Length::Fill), height : None, align : None, background : None,
                border : None, children : children, } } }), }); if message.edited && !
                message.show_author { children
                .push(native::text_options(native::text(format!("{}/@text:408",
                message_contents_scope_1995), "· edited".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } if ! crate
                ::host::run_of_message(::std::convert::AsRef::as_ref(& message.id),)
                .is_empty() { children.push({ let children : Vec < wire::Node > =
                vec![native::padded(native::button(format!("{}/@button:420",
                message_contents_scope_1995), String::from("View run"),
                Some(::ducktape_view_guest::slots::message(lazy_event_543_17(crate
                ::host::run_of_message(::std::convert::AsRef::as_ref(& message
                .id),),),),), wire::ButtonPreset::Secondary,),
                wire::Edges::all(3.0f32),)];
                native::padded(native::sized(native::row(format!("{}/@layout:419",
                message_contents_scope_1995), children,), Some(wire::Length::Fill),
                None,), wire::Edges { top : 4.0f32, right : 0.0f32, bottom : 0.0f32, left
                : 0.0f32, },) }); } if ! message.reactions.is_empty() { children.push({
                let mut items = Vec::new(); for (index, reaction) in message.reactions
                .iter().enumerate() { let for_scope = format!("{}/@for:1848({})",
                message_contents_scope_1995, index); let flex_child : wire::Node = { let
                reaction_chip_scope_1849 = format!("{}/ReactionChip@1849", for_scope); {
                let node_scope = format!("{}/root", reaction_chip_scope_1849); { let mut
                children : Vec < wire::Node > = Vec::new(); if reaction.reacted_by_me {
                children.push(wire::Node::Button { checked : Some(reaction
                .reacted_by_me), expanded : None, description :
                Some(String::from(reaction.emoji.to_owned())), key :
                format!("{}/@button:218", reaction_chip_scope_1849), content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:225",
                reaction_chip_scope_1849), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:232",
                reaction_chip_scope_1849), reaction.emoji.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:238",
                reaction_chip_scope_1849), reaction.count.to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:231", reaction_chip_scope_1849), wrap :
                None, axis : wire::Axis::Row, spacing : Some(4.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 1.0f32, right : 8.0f32, bottom : 1.0f32, left : 6.0f32, },),),),
                label : Some(String::from("Remove reaction".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_543_21(message.seq,
                reaction.emoji.to_owned()),),), width : None, height : None, padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), });
                } if ! reaction.reacted_by_me { children.push(wire::Node::Button {
                checked : Some(reaction.reacted_by_me), expanded : None, description :
                Some(String::from(reaction.emoji.to_owned())), key :
                format!("{}/@button:262", reaction_chip_scope_1849), content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:269",
                reaction_chip_scope_1849), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:276",
                reaction_chip_scope_1849), reaction.emoji.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:282",
                reaction_chip_scope_1849), reaction.count.to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:275", reaction_chip_scope_1849), wrap :
                None, axis : wire::Axis::Row, spacing : Some(4.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 1.0f32, right : 8.0f32, bottom : 1.0f32, left : 6.0f32, },),),),
                label : Some(String::from("Add reaction".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_543_20(message.seq,
                reaction.emoji.to_owned()),),), width : None, height : None, padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), });
                } native::column(node_scope.clone(), children) } } }; items
                .push((wire::FlexItem::default(), flex_child)); } let (items, children) =
                items.into_iter().unzip(); wire::Node::Flex { key :
                format!("{}/@layout:427", message_contents_scope_1995), items, children,
                background : None, border : None, layout : wire::FlexLayout { direction :
                wire::FlexDirection::Row, wrap : wire::FlexWrap::Wrap, justify : None,
                items : Some(wire::FlexItemAlignment::Start), content : None, row_gap :
                Some(5.0f32), column_gap : Some(5.0f32), padding : Some(wire::Edges { top
                : 6.0f32, right : 0.0f32, bottom : 0.0f32, left : 0.0f32, }), width :
                Some(wire::Length::Fill), height : None, max_width : None, max_height :
                None, clip : false, surface_width : None, surface_height : None,
                surface_max_width : None, }, } }); } if message.reply_count > 0 {
                children.push({ let children : Vec < wire::Node > =
                vec![wire::Node::Button { checked : None, expanded : None, description :
                None, key : format!("{}/@button:447", message_contents_scope_1995),
                content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:452",
                message_contents_scope_1995), { let children : Vec < wire::Node > =
                vec![{ let component_icon_scope_1872 = format!("{}/Icon@1872",
                message_contents_scope_1995); { let node_scope = format!("{}/root",
                component_icon_scope_1872); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "nav-chat"),),);
                wire::Node::Svg { inherit_button_ink : false, key :
                format!("{}/@media:52", component_icon_scope_1872), hash : hash, bytes :
                bytes, label : None, color : None, hover : None, fit : None, rotation :
                None, opacity : None, width : Some(wire::Length::Fixed(12.0f32)), height
                : Some(wire::Length::Fixed(12.0f32)), } }); } native::column(node_scope
                .clone(), children) } } },
                native::text_options(native::text(format!("{}/@text:464",
                message_contents_scope_1995), crate ::host::plural(message.reply_count,
                ::std::convert::AsRef::as_ref(& "reply"), ::std::convert::AsRef::as_ref(&
                "replies"),).to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },)]; wire::Node::Linear {
                max_width : None, clip : false, key : format!("{}/@layout:458",
                message_contents_scope_1995), wrap : None, axis : wire::Axis::Row,
                spacing : Some(6.0f32), padding : None, width : None, height : None,
                align : Some(wire::AlignX::Center), background : None, border : None,
                children : children, } },), wire::Edges { top : 3.0f32, right : 9.0f32,
                bottom : 3.0f32, left : 7.0f32, },),),), label :
                Some(String::from("Open thread".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_543_22(message
                .seq),),), width : None, height : None, padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), }];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:441", message_contents_scope_1995), wrap : None, axis
                : wire::Axis::Row, spacing : Some(6.0f32), padding : Some(wire::Edges {
                top : 6.0f32, right : 0.0f32, bottom : 0.0f32, left : 0.0f32, }), width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }); }
                native::spaced(native::sized(native::column(format!("{}/@layout:340",
                message_contents_scope_1995), children,), Some(wire::Length::Fill),
                None,), 2.0f32,) }); if message.pending { children
                .push(native::padded(native::container(format!("{}/@container:506",
                message_contents_scope_1995), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:508",
                message_contents_scope_1995), message.meta.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },), { let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "dot")),); wire::Node::Svg {
                inherit_button_ink : false, key : format!("{}/@media:514",
                message_contents_scope_1995), hash : hash, bytes : bytes, label : None,
                color : None, hover : None, fit : None, rotation : None, opacity :
                Some(1.0f32), width : Some(wire::Length::Fixed(6.0f32)), height :
                Some(wire::Length::Fixed(6.0f32)), } }]; wire::Node::Linear { max_width :
                None, clip : false, key : format!("{}/@layout:507",
                message_contents_scope_1995), wrap : None, axis : wire::Axis::Row,
                spacing : Some(5.0f32), padding : None, width : None, height : None,
                align : Some(wire::AlignX::Center), background : None, border : None,
                children : children, } },), wire::Edges { top : 0.0f32, right : 7.0f32,
                bottom : 0.0f32, left : 0.0f32, },),); } wire::Node::Linear { max_width :
                None, clip : false, key : format!("{}/@layout:331",
                message_contents_scope_1995), wrap : None, axis : wire::Axis::Row,
                spacing : Some(11.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Left), background : None, border : None, children :
                children, } }]; native::sized(native::column(format!("{}/@layout:330",
                message_contents_scope_1995), children,), Some(wire::Length::Fill),
                None,) } },), Some(wire::Length::Fill), None,), wire::Edges { top :
                4.0f32, right : 7.0f32, bottom : 4.0f32, left : 7.0f32, },),); }
                RowPlate::Selected => { children
                .push(native::padded(native::sized(native::container(format!("{}/@container:591",
                message_card_scope_2680), { let message_contents_scope_2015 =
                format!("{}/MessageContents@2015", message_card_scope_2680); { let children : Vec < wire::Node > = vec![{ let mut children : Vec <
                wire::Node > = Vec::new(); if message.show_author { children.push({ let
                message_avatar_scope_1750 = format!("{}/MessageAvatar@1750",
                message_contents_scope_2015); { let node_scope = format!("{}/root",
                message_avatar_scope_1750); { let mut children : Vec < wire::Node > =
                Vec::new(); if message.avatar_kind == "human" { children.push({ let
                person_avatar_scope_798 = format!("{}/PersonAvatar@798",
                message_avatar_scope_1750); { let node_scope = format!("{}/root",
                person_avatar_scope_798); { let children : Vec < wire::Node > =
                vec![{ let principal_avatar_scope_863 = format!("{}/PrincipalAvatar@863",
                person_avatar_scope_798); { let node_scope = format!("{}/root",
                principal_avatar_scope_863); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let principal_plate_scope_909 =
                format!("{}/PrincipalPlate@909", principal_avatar_scope_863); { let
                node_scope = format!("{}/root", principal_plate_scope_909); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                human_plate_scope_925 = format!("{}/HumanPlate@925",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                human_plate_scope_925); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:328", human_plate_scope_925), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([((30.0 / 2.0) as f32).max(0.0).min(f32::MAX), ((30.0
                / 2.0) as f32).max(0.0).min(f32::MAX), ((30.0 / 2.0) as f32).max(0.0)
                .min(f32::MAX), ((30.0 / 2.0) as f32).max(0.0).min(f32::MAX),]), }), snap
                : None, content :
                Box::new(native::text_options(native::text(format!("{}/@text:336",
                human_plate_scope_925), message.initial.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), }); } native::column(node_scope.clone(),
                children) } } }); } native::column(node_scope.clone(), children) } } });
                } native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } if ! (message
                .avatar_kind == "human") && message.avatar_kind == "agent" { children
                .push({ let agent_avatar_scope_804 = format!("{}/AgentAvatar@804",
                message_avatar_scope_1750); { let node_scope = format!("{}/root",
                agent_avatar_scope_804); { let children : Vec < wire::Node > = vec![{
                let principal_avatar_scope_873 = format!("{}/PrincipalAvatar@873",
                agent_avatar_scope_804); { let node_scope = format!("{}/root",
                principal_avatar_scope_873); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let principal_plate_scope_909 =
                format!("{}/PrincipalPlate@909", principal_avatar_scope_873); { let
                node_scope = format!("{}/root", principal_plate_scope_909); { let children : Vec < wire::Node > = vec![{ let agent_plate_scope_919 =
                format!("{}/AgentPlate@919", principal_plate_scope_909); { let node_scope
                = format!("{}/root", agent_plate_scope_919); { let mut children : Vec <
                wire::Node > = Vec::new(); { children.push({ let agent_square_scope_1174
                = format!("{}/AgentSquare@1174", agent_plate_scope_919); { let node_scope
                = format!("{}/root", agent_square_scope_1174); wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : node_scope.clone(), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content :
                Box::new(native::text_options(native::text(format!("{}/@text:390",
                agent_square_scope_1174), message.initial.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), } } }); } native::column(node_scope.clone(),
                children) } } }]; native::column(node_scope.clone(), children) } } }); }
                native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } if ! (message
                .avatar_kind == "human" || message.avatar_kind == "agent") { children
                .push({ let agent_avatar_scope_810 = format!("{}/AgentAvatar@810",
                message_avatar_scope_1750); { let node_scope = format!("{}/root",
                agent_avatar_scope_810); { let children : Vec < wire::Node > = vec![{
                let principal_avatar_scope_873 = format!("{}/PrincipalAvatar@873",
                agent_avatar_scope_810); { let node_scope = format!("{}/root",
                principal_avatar_scope_873); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let principal_plate_scope_909 =
                format!("{}/PrincipalPlate@909", principal_avatar_scope_873); { let
                node_scope = format!("{}/root", principal_plate_scope_909); { let children : Vec < wire::Node > = vec![{ let agent_plate_scope_919 =
                format!("{}/AgentPlate@919", principal_plate_scope_909); { let node_scope
                = format!("{}/root", agent_plate_scope_919); { let mut children : Vec <
                wire::Node > = Vec::new(); { children.push({ let agent_square_scope_1174
                = format!("{}/AgentSquare@1174", agent_plate_scope_919); { let node_scope
                = format!("{}/root", agent_square_scope_1174); wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : node_scope.clone(), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content :
                Box::new(native::text_options(native::text(format!("{}/@text:390",
                agent_square_scope_1174), message.initial.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), } } }); } native::column(node_scope.clone(),
                children) } } }]; native::column(node_scope.clone(), children) } } }); }
                native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } wire::Node::Stack
                { key : node_scope.clone(), width : Some(wire::Length::Fixed(30.0f32)),
                height : Some(wire::Length::Fixed(30.0f32)), padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, } }
                } }); } if ! message.show_author { children.push(wire::Node::Space {
                width : Some(wire::Length::Fixed(30.0f32)), height : None, }); } children
                .push({ let mut children : Vec < wire::Node > = Vec::new(); if message
                .show_author { children.push({ let mut children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:347",
                message_contents_scope_2015), message.author.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; if message.avatar_kind == "agent" { children
                .push(native::padded(native::container(format!("{}/@container:354",
                message_contents_scope_2015),
                native::text_options(native::text(format!("{}/@text:360",
                message_contents_scope_2015), "AGENT".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), wire::Edges { top : 2.0f32, right : 5.0f32,
                bottom : 2.0f32, left : 5.0f32, },),); } if message.height > 0 { children
                .push(native::text_options(native::text(format!("{}/@text:377",
                message_contents_scope_2015), crate ::host::height_label_short(message
                .height).to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },),); } if message.edited
                { children.push(native::text_options(native::text(format!("{}/@text:384",
                message_contents_scope_2015), "· edited".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } children.push(wire::Node::Space { width :
                Some(wire::Length::Fill), height : None, }); wire::Node::Linear {
                max_width : None, clip : false, key : format!("{}/@layout:342",
                message_contents_scope_2015), wrap : None, axis : wire::Axis::Row,
                spacing : Some(7.0f32), padding : None, width : Some(wire::Length::Fill),
                height : None, align : Some(wire::AlignX::Center), background : None,
                border : None, children : children, } }); } children
                .push(wire::Node::MouseArea { key : format!("{}/@mouse:395",
                message_contents_scope_2015), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_543_1(message.seq,
                CopySurface::Timeline.clone(),),),), on_release : None, on_double_click :
                None, on_right_press : None, on_right_release : None, on_middle_press :
                None, on_middle_release : None, on_enter : None, on_exit : None, on_move
                : None, on_press_at : None, on_scroll : None, content : Box::new({ let
                message_body_scope_1809 = format!("{}/MessageBody@1809",
                message_contents_scope_2015); { let children : Vec < wire::Node > =
                vec![{ let rich_body_scope_688 = format!("{}/RichBody@688",
                message_body_scope_1809); { let mut children : Vec < wire::Node > =
                Vec::new(); for (index, block) in message.blocks.iter().enumerate() { let
                for_scope = format!("{}/@for:703({})", rich_body_scope_688, index); if
                block.kind == "divider" { children.push({ let
                component_separator_scope_705 = format!("{}/Separator@705", for_scope); {
                let node_scope = format!("{}/root", component_separator_scope_705);
                wire::Node::Rule { key : node_scope.clone(), axis : wire::Axis::Row,
                thickness : 1.0f32, color : None, weak : false, radius : None, snap :
                None, } } }); } if block.kind == "code" { children
                .push(native::padded(native::sized(native::container(format!("{}/@container:90",
                for_scope), { let mut children : Vec < wire::Node > = Vec::new(); if !
                block.lang.is_empty() { children
                .push(native::text_options(native::text(format!("{}/@text:100",
                for_scope), block.lang.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },),); }
                children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:106",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),);
                native::spaced(native::sized(native::column(format!("{}/@layout:98",
                for_scope), children,), Some(wire::Length::Fill), None,), 6.0f32,) },),
                Some(wire::Length::Fill), None,), wire::Edges { top : 11.0f32, right :
                11.0f32, bottom : 11.0f32, left : 11.0f32, },),); } if block.kind ==
                "quote" { children.push({ let children : Vec < wire::Node > = vec![{
                let mut children : Vec < wire::Node > = Vec::new(); if block.rich {
                children.push({ let rich_line_scope_755 = format!("{}/RichLine@755",
                for_scope); { let mut rich_spans : Vec < wire::RichSpan > = Vec::new();
                for span in block.spans.iter().cloned() { rich_spans.push(wire::RichSpan
                { content : span.mention.to_owned().to_string(), size : None, line_height
                : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Medium,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : Some(span.mention_link.to_owned()), background :
                None, border : Some(wire::Border { color : None, width : None, radius :
                Some([4.0f32, 4.0f32, 4.0f32, 4.0f32,]), }), padding : Some(wire::Edges {
                top : 0.0f32, right : 1.0f32, bottom : 0.0f32, left : 1.0f32, }),
                underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.link_text.to_owned().to_string(),
                size : None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Medium,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : Some(span.link.to_owned()), background : None,
                border : None, padding : None, underline : true, strikethrough : false,
                }); rich_spans.push(wire::RichSpan { content : span.bold_italic
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Bold, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Italic, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.bold
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Bold, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Normal, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.italic
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Normal, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Italic, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.plain
                .to_owned().to_string(), size : None, line_height : None, font : None,
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); }
                wire::Node::RichText { options : wire::TextOptions { wrapping :
                Some(wire::Wrapping::WordOrGlyph), ..Default::default() }, key :
                format!("{}/@text:32", rich_line_scope_755), size : Some(13.5f32), color
                : None, font : wire::Font { monospace : false, weight :
                wire::Weight::Normal, }, width : Some(wire::Length::Fill), align_x :
                None, spans : rich_spans, on_link :
                Some(::ducktape_view_guest::slots::handler:: < String, Message, >
                (Box::new({ let route = { let route_callback = lazy_event_543_16.clone();
                move | link : String | route_callback(link) }; move | sent : String |
                Some(route(sent)) }),),), } } }); } if ! block.rich { children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:137",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),); }
                native::padded(native::sized(native::column(format!("{}/@layout:121",
                for_scope), children,), Some(wire::Length::Fill), None,), wire::Edges {
                top : 2.0f32, right : 0.0f32, bottom : 2.0f32, left : 13.0f32, },) },
                native::sized(native::container(format!("{}/@container:144", for_scope),
                wire::Node::Space { width : Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(1.0f32)), },),
                Some(wire::Length::Fixed(3.0f32)), Some(wire::Length::Fill),)];
                wire::Node::Stack { key : format!("{}/@layout:120", for_scope), width :
                Some(wire::Length::Fill), height : None, padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, }
                }); } if block.kind == "paragraph" { if block.rich { children.push({ let
                rich_line_scope_780 = format!("{}/RichLine@780", for_scope); { let mut
                rich_spans : Vec < wire::RichSpan > = Vec::new(); for span in block.spans
                .iter().cloned() { rich_spans.push(wire::RichSpan { content : span
                .mention.to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Medium, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), color : None, link : Some(span
                .mention_link.to_owned()), background : None, border : Some(wire::Border
                { color : None, width : None, radius : Some([4.0f32, 4.0f32, 4.0f32,
                4.0f32,]), }), padding : Some(wire::Edges { top : 0.0f32, right : 1.0f32,
                bottom : 0.0f32, left : 1.0f32, }), underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.link_text
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Medium, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), color : None, link : Some(span.link
                .to_owned()), background : None, border : None, padding : None, underline
                : true, strikethrough : false, }); rich_spans.push(wire::RichSpan {
                content : span.bold_italic.to_owned().to_string(), size : None,
                line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Bold,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Italic, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.bold.to_owned().to_string(), size :
                None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Bold,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.italic.to_owned().to_string(), size
                : None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Normal,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Italic, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.plain.to_owned().to_string(), size
                : None, line_height : None, font : None, color : None, link : None,
                background : None, border : None, padding : None, underline : false,
                strikethrough : false, }); } wire::Node::RichText { options :
                wire::TextOptions { wrapping : Some(wire::Wrapping::WordOrGlyph),
                ..Default::default() }, key : format!("{}/@text:32",
                rich_line_scope_780), size : Some(13.5f32), color : None, font :
                wire::Font { monospace : false, weight : wire::Weight::Normal, }, width :
                Some(wire::Length::Fill), align_x : None, spans : rich_spans, on_link :
                Some(::ducktape_view_guest::slots::handler:: < String, Message, >
                (Box::new({ let route = { let route_callback = lazy_event_543_16.clone();
                move | link : String | route_callback(link) }; move | sent : String |
                Some(route(sent)) }),),), } } }); } if ! block.rich { children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:157",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),); } } }
                native::spaced(native::sized(native::column(format!("{}/@layout:75",
                rich_body_scope_688), children,), Some(wire::Length::Fill), None,),
                5.0f32,) } }]; wire::Node::Linear { max_width : Some(760.0f32), clip :
                false, key : format!("{}/@layout:60", message_body_scope_1809), wrap :
                None, axis : wire::Axis::Column, spacing : None, padding : None, width :
                Some(wire::Length::Fill), height : None, align : None, background : None,
                border : None, children : children, } } }), }); if message.edited && !
                message.show_author { children
                .push(native::text_options(native::text(format!("{}/@text:408",
                message_contents_scope_2015), "· edited".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } if ! crate
                ::host::run_of_message(::std::convert::AsRef::as_ref(& message.id),)
                .is_empty() { children.push({ let children : Vec < wire::Node > =
                vec![native::padded(native::button(format!("{}/@button:420",
                message_contents_scope_2015), String::from("View run"),
                Some(::ducktape_view_guest::slots::message(lazy_event_543_17(crate
                ::host::run_of_message(::std::convert::AsRef::as_ref(& message
                .id),),),),), wire::ButtonPreset::Secondary,),
                wire::Edges::all(3.0f32),)];
                native::padded(native::sized(native::row(format!("{}/@layout:419",
                message_contents_scope_2015), children,), Some(wire::Length::Fill),
                None,), wire::Edges { top : 4.0f32, right : 0.0f32, bottom : 0.0f32, left
                : 0.0f32, },) }); } if ! message.reactions.is_empty() { children.push({
                let mut items = Vec::new(); for (index, reaction) in message.reactions
                .iter().enumerate() { let for_scope = format!("{}/@for:1848({})",
                message_contents_scope_2015, index); let flex_child : wire::Node = { let
                reaction_chip_scope_1849 = format!("{}/ReactionChip@1849", for_scope); {
                let node_scope = format!("{}/root", reaction_chip_scope_1849); { let mut
                children : Vec < wire::Node > = Vec::new(); if reaction.reacted_by_me {
                children.push(wire::Node::Button { checked : Some(reaction
                .reacted_by_me), expanded : None, description :
                Some(String::from(reaction.emoji.to_owned())), key :
                format!("{}/@button:218", reaction_chip_scope_1849), content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:225",
                reaction_chip_scope_1849), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:232",
                reaction_chip_scope_1849), reaction.emoji.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:238",
                reaction_chip_scope_1849), reaction.count.to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:231", reaction_chip_scope_1849), wrap :
                None, axis : wire::Axis::Row, spacing : Some(4.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 1.0f32, right : 8.0f32, bottom : 1.0f32, left : 6.0f32, },),),),
                label : Some(String::from("Remove reaction".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_543_21(message.seq,
                reaction.emoji.to_owned()),),), width : None, height : None, padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), });
                } if ! reaction.reacted_by_me { children.push(wire::Node::Button {
                checked : Some(reaction.reacted_by_me), expanded : None, description :
                Some(String::from(reaction.emoji.to_owned())), key :
                format!("{}/@button:262", reaction_chip_scope_1849), content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:269",
                reaction_chip_scope_1849), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:276",
                reaction_chip_scope_1849), reaction.emoji.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:282",
                reaction_chip_scope_1849), reaction.count.to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:275", reaction_chip_scope_1849), wrap :
                None, axis : wire::Axis::Row, spacing : Some(4.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 1.0f32, right : 8.0f32, bottom : 1.0f32, left : 6.0f32, },),),),
                label : Some(String::from("Add reaction".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_543_20(message.seq,
                reaction.emoji.to_owned()),),), width : None, height : None, padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), });
                } native::column(node_scope.clone(), children) } } }; items
                .push((wire::FlexItem::default(), flex_child)); } let (items, children) =
                items.into_iter().unzip(); wire::Node::Flex { key :
                format!("{}/@layout:427", message_contents_scope_2015), items, children,
                background : None, border : None, layout : wire::FlexLayout { direction :
                wire::FlexDirection::Row, wrap : wire::FlexWrap::Wrap, justify : None,
                items : Some(wire::FlexItemAlignment::Start), content : None, row_gap :
                Some(5.0f32), column_gap : Some(5.0f32), padding : Some(wire::Edges { top
                : 6.0f32, right : 0.0f32, bottom : 0.0f32, left : 0.0f32, }), width :
                Some(wire::Length::Fill), height : None, max_width : None, max_height :
                None, clip : false, surface_width : None, surface_height : None,
                surface_max_width : None, }, } }); } if message.reply_count > 0 {
                children.push({ let children : Vec < wire::Node > =
                vec![wire::Node::Button { checked : None, expanded : None, description :
                None, key : format!("{}/@button:447", message_contents_scope_2015),
                content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:452",
                message_contents_scope_2015), { let children : Vec < wire::Node > =
                vec![{ let component_icon_scope_1872 = format!("{}/Icon@1872",
                message_contents_scope_2015); { let node_scope = format!("{}/root",
                component_icon_scope_1872); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "nav-chat"),),);
                wire::Node::Svg { inherit_button_ink : false, key :
                format!("{}/@media:52", component_icon_scope_1872), hash : hash, bytes :
                bytes, label : None, color : None, hover : None, fit : None, rotation :
                None, opacity : None, width : Some(wire::Length::Fixed(12.0f32)), height
                : Some(wire::Length::Fixed(12.0f32)), } }); } native::column(node_scope
                .clone(), children) } } },
                native::text_options(native::text(format!("{}/@text:464",
                message_contents_scope_2015), crate ::host::plural(message.reply_count,
                ::std::convert::AsRef::as_ref(& "reply"), ::std::convert::AsRef::as_ref(&
                "replies"),).to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },)]; wire::Node::Linear {
                max_width : None, clip : false, key : format!("{}/@layout:458",
                message_contents_scope_2015), wrap : None, axis : wire::Axis::Row,
                spacing : Some(6.0f32), padding : None, width : None, height : None,
                align : Some(wire::AlignX::Center), background : None, border : None,
                children : children, } },), wire::Edges { top : 3.0f32, right : 9.0f32,
                bottom : 3.0f32, left : 7.0f32, },),),), label :
                Some(String::from("Open thread".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_543_22(message
                .seq),),), width : None, height : None, padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), }];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:441", message_contents_scope_2015), wrap : None, axis
                : wire::Axis::Row, spacing : Some(6.0f32), padding : Some(wire::Edges {
                top : 6.0f32, right : 0.0f32, bottom : 0.0f32, left : 0.0f32, }), width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }); }
                native::spaced(native::sized(native::column(format!("{}/@layout:340",
                message_contents_scope_2015), children,), Some(wire::Length::Fill),
                None,), 2.0f32,) }); if message.pending { children
                .push(native::padded(native::container(format!("{}/@container:506",
                message_contents_scope_2015), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:508",
                message_contents_scope_2015), message.meta.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },), { let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "dot")),); wire::Node::Svg {
                inherit_button_ink : false, key : format!("{}/@media:514",
                message_contents_scope_2015), hash : hash, bytes : bytes, label : None,
                color : None, hover : None, fit : None, rotation : None, opacity :
                Some(1.0f32), width : Some(wire::Length::Fixed(6.0f32)), height :
                Some(wire::Length::Fixed(6.0f32)), } }]; wire::Node::Linear { max_width :
                None, clip : false, key : format!("{}/@layout:507",
                message_contents_scope_2015), wrap : None, axis : wire::Axis::Row,
                spacing : Some(5.0f32), padding : None, width : None, height : None,
                align : Some(wire::AlignX::Center), background : None, border : None,
                children : children, } },), wire::Edges { top : 0.0f32, right : 7.0f32,
                bottom : 0.0f32, left : 0.0f32, },),); } wire::Node::Linear { max_width :
                None, clip : false, key : format!("{}/@layout:331",
                message_contents_scope_2015), wrap : None, axis : wire::Axis::Row,
                spacing : Some(11.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Left), background : None, border : None, children :
                children, } }]; native::sized(native::column(format!("{}/@layout:330",
                message_contents_scope_2015), children,), Some(wire::Length::Fill),
                None,) } },), Some(wire::Length::Fill), None,), wire::Edges { top :
                4.0f32, right : 7.0f32, bottom : 4.0f32, left : 7.0f32, },),); }
                RowPlate::Ranged => { children
                .push(native::padded(native::sized(native::container(format!("{}/@container:611",
                message_card_scope_2680), { let message_contents_scope_2035 =
                format!("{}/MessageContents@2035", message_card_scope_2680); { let children : Vec < wire::Node > = vec![{ let mut children : Vec <
                wire::Node > = Vec::new(); if message.show_author { children.push({ let
                message_avatar_scope_1750 = format!("{}/MessageAvatar@1750",
                message_contents_scope_2035); { let node_scope = format!("{}/root",
                message_avatar_scope_1750); { let mut children : Vec < wire::Node > =
                Vec::new(); if message.avatar_kind == "human" { children.push({ let
                person_avatar_scope_798 = format!("{}/PersonAvatar@798",
                message_avatar_scope_1750); { let node_scope = format!("{}/root",
                person_avatar_scope_798); { let children : Vec < wire::Node > =
                vec![{ let principal_avatar_scope_863 = format!("{}/PrincipalAvatar@863",
                person_avatar_scope_798); { let node_scope = format!("{}/root",
                principal_avatar_scope_863); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let principal_plate_scope_909 =
                format!("{}/PrincipalPlate@909", principal_avatar_scope_863); { let
                node_scope = format!("{}/root", principal_plate_scope_909); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                human_plate_scope_925 = format!("{}/HumanPlate@925",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                human_plate_scope_925); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:328", human_plate_scope_925), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([((30.0 / 2.0) as f32).max(0.0).min(f32::MAX), ((30.0
                / 2.0) as f32).max(0.0).min(f32::MAX), ((30.0 / 2.0) as f32).max(0.0)
                .min(f32::MAX), ((30.0 / 2.0) as f32).max(0.0).min(f32::MAX),]), }), snap
                : None, content :
                Box::new(native::text_options(native::text(format!("{}/@text:336",
                human_plate_scope_925), message.initial.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), }); } native::column(node_scope.clone(),
                children) } } }); } native::column(node_scope.clone(), children) } } });
                } native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } if ! (message
                .avatar_kind == "human") && message.avatar_kind == "agent" { children
                .push({ let agent_avatar_scope_804 = format!("{}/AgentAvatar@804",
                message_avatar_scope_1750); { let node_scope = format!("{}/root",
                agent_avatar_scope_804); { let children : Vec < wire::Node > = vec![{
                let principal_avatar_scope_873 = format!("{}/PrincipalAvatar@873",
                agent_avatar_scope_804); { let node_scope = format!("{}/root",
                principal_avatar_scope_873); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let principal_plate_scope_909 =
                format!("{}/PrincipalPlate@909", principal_avatar_scope_873); { let
                node_scope = format!("{}/root", principal_plate_scope_909); { let children : Vec < wire::Node > = vec![{ let agent_plate_scope_919 =
                format!("{}/AgentPlate@919", principal_plate_scope_909); { let node_scope
                = format!("{}/root", agent_plate_scope_919); { let mut children : Vec <
                wire::Node > = Vec::new(); { children.push({ let agent_square_scope_1174
                = format!("{}/AgentSquare@1174", agent_plate_scope_919); { let node_scope
                = format!("{}/root", agent_square_scope_1174); wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : node_scope.clone(), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content :
                Box::new(native::text_options(native::text(format!("{}/@text:390",
                agent_square_scope_1174), message.initial.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), } } }); } native::column(node_scope.clone(),
                children) } } }]; native::column(node_scope.clone(), children) } } }); }
                native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } if ! (message
                .avatar_kind == "human" || message.avatar_kind == "agent") { children
                .push({ let agent_avatar_scope_810 = format!("{}/AgentAvatar@810",
                message_avatar_scope_1750); { let node_scope = format!("{}/root",
                agent_avatar_scope_810); { let children : Vec < wire::Node > = vec![{
                let principal_avatar_scope_873 = format!("{}/PrincipalAvatar@873",
                agent_avatar_scope_810); { let node_scope = format!("{}/root",
                principal_avatar_scope_873); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let principal_plate_scope_909 =
                format!("{}/PrincipalPlate@909", principal_avatar_scope_873); { let
                node_scope = format!("{}/root", principal_plate_scope_909); { let children : Vec < wire::Node > = vec![{ let agent_plate_scope_919 =
                format!("{}/AgentPlate@919", principal_plate_scope_909); { let node_scope
                = format!("{}/root", agent_plate_scope_919); { let mut children : Vec <
                wire::Node > = Vec::new(); { children.push({ let agent_square_scope_1174
                = format!("{}/AgentSquare@1174", agent_plate_scope_919); { let node_scope
                = format!("{}/root", agent_square_scope_1174); wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : node_scope.clone(), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content :
                Box::new(native::text_options(native::text(format!("{}/@text:390",
                agent_square_scope_1174), message.initial.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), } } }); } native::column(node_scope.clone(),
                children) } } }]; native::column(node_scope.clone(), children) } } }); }
                native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } wire::Node::Stack
                { key : node_scope.clone(), width : Some(wire::Length::Fixed(30.0f32)),
                height : Some(wire::Length::Fixed(30.0f32)), padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, } }
                } }); } if ! message.show_author { children.push(wire::Node::Space {
                width : Some(wire::Length::Fixed(30.0f32)), height : None, }); } children
                .push({ let mut children : Vec < wire::Node > = Vec::new(); if message
                .show_author { children.push({ let mut children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:347",
                message_contents_scope_2035), message.author.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; if message.avatar_kind == "agent" { children
                .push(native::padded(native::container(format!("{}/@container:354",
                message_contents_scope_2035),
                native::text_options(native::text(format!("{}/@text:360",
                message_contents_scope_2035), "AGENT".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), wire::Edges { top : 2.0f32, right : 5.0f32,
                bottom : 2.0f32, left : 5.0f32, },),); } if message.height > 0 { children
                .push(native::text_options(native::text(format!("{}/@text:377",
                message_contents_scope_2035), crate ::host::height_label_short(message
                .height).to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },),); } if message.edited
                { children.push(native::text_options(native::text(format!("{}/@text:384",
                message_contents_scope_2035), "· edited".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } children.push(wire::Node::Space { width :
                Some(wire::Length::Fill), height : None, }); wire::Node::Linear {
                max_width : None, clip : false, key : format!("{}/@layout:342",
                message_contents_scope_2035), wrap : None, axis : wire::Axis::Row,
                spacing : Some(7.0f32), padding : None, width : Some(wire::Length::Fill),
                height : None, align : Some(wire::AlignX::Center), background : None,
                border : None, children : children, } }); } children
                .push(wire::Node::MouseArea { key : format!("{}/@mouse:395",
                message_contents_scope_2035), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_543_1(message.seq,
                CopySurface::Timeline.clone(),),),), on_release : None, on_double_click :
                None, on_right_press : None, on_right_release : None, on_middle_press :
                None, on_middle_release : None, on_enter : None, on_exit : None, on_move
                : None, on_press_at : None, on_scroll : None, content : Box::new({ let
                message_body_scope_1809 = format!("{}/MessageBody@1809",
                message_contents_scope_2035); { let children : Vec < wire::Node > =
                vec![{ let rich_body_scope_688 = format!("{}/RichBody@688",
                message_body_scope_1809); { let mut children : Vec < wire::Node > =
                Vec::new(); for (index, block) in message.blocks.iter().enumerate() { let
                for_scope = format!("{}/@for:703({})", rich_body_scope_688, index); if
                block.kind == "divider" { children.push({ let
                component_separator_scope_705 = format!("{}/Separator@705", for_scope); {
                let node_scope = format!("{}/root", component_separator_scope_705);
                wire::Node::Rule { key : node_scope.clone(), axis : wire::Axis::Row,
                thickness : 1.0f32, color : None, weak : false, radius : None, snap :
                None, } } }); } if block.kind == "code" { children
                .push(native::padded(native::sized(native::container(format!("{}/@container:90",
                for_scope), { let mut children : Vec < wire::Node > = Vec::new(); if !
                block.lang.is_empty() { children
                .push(native::text_options(native::text(format!("{}/@text:100",
                for_scope), block.lang.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },),); }
                children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:106",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),);
                native::spaced(native::sized(native::column(format!("{}/@layout:98",
                for_scope), children,), Some(wire::Length::Fill), None,), 6.0f32,) },),
                Some(wire::Length::Fill), None,), wire::Edges { top : 11.0f32, right :
                11.0f32, bottom : 11.0f32, left : 11.0f32, },),); } if block.kind ==
                "quote" { children.push({ let children : Vec < wire::Node > = vec![{
                let mut children : Vec < wire::Node > = Vec::new(); if block.rich {
                children.push({ let rich_line_scope_755 = format!("{}/RichLine@755",
                for_scope); { let mut rich_spans : Vec < wire::RichSpan > = Vec::new();
                for span in block.spans.iter().cloned() { rich_spans.push(wire::RichSpan
                { content : span.mention.to_owned().to_string(), size : None, line_height
                : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Medium,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : Some(span.mention_link.to_owned()), background :
                None, border : Some(wire::Border { color : None, width : None, radius :
                Some([4.0f32, 4.0f32, 4.0f32, 4.0f32,]), }), padding : Some(wire::Edges {
                top : 0.0f32, right : 1.0f32, bottom : 0.0f32, left : 1.0f32, }),
                underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.link_text.to_owned().to_string(),
                size : None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Medium,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : Some(span.link.to_owned()), background : None,
                border : None, padding : None, underline : true, strikethrough : false,
                }); rich_spans.push(wire::RichSpan { content : span.bold_italic
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Bold, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Italic, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.bold
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Bold, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Normal, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.italic
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Normal, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Italic, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.plain
                .to_owned().to_string(), size : None, line_height : None, font : None,
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); }
                wire::Node::RichText { options : wire::TextOptions { wrapping :
                Some(wire::Wrapping::WordOrGlyph), ..Default::default() }, key :
                format!("{}/@text:32", rich_line_scope_755), size : Some(13.5f32), color
                : None, font : wire::Font { monospace : false, weight :
                wire::Weight::Normal, }, width : Some(wire::Length::Fill), align_x :
                None, spans : rich_spans, on_link :
                Some(::ducktape_view_guest::slots::handler:: < String, Message, >
                (Box::new({ let route = { let route_callback = lazy_event_543_16.clone();
                move | link : String | route_callback(link) }; move | sent : String |
                Some(route(sent)) }),),), } } }); } if ! block.rich { children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:137",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),); }
                native::padded(native::sized(native::column(format!("{}/@layout:121",
                for_scope), children,), Some(wire::Length::Fill), None,), wire::Edges {
                top : 2.0f32, right : 0.0f32, bottom : 2.0f32, left : 13.0f32, },) },
                native::sized(native::container(format!("{}/@container:144", for_scope),
                wire::Node::Space { width : Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(1.0f32)), },),
                Some(wire::Length::Fixed(3.0f32)), Some(wire::Length::Fill),)];
                wire::Node::Stack { key : format!("{}/@layout:120", for_scope), width :
                Some(wire::Length::Fill), height : None, padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, }
                }); } if block.kind == "paragraph" { if block.rich { children.push({ let
                rich_line_scope_780 = format!("{}/RichLine@780", for_scope); { let mut
                rich_spans : Vec < wire::RichSpan > = Vec::new(); for span in block.spans
                .iter().cloned() { rich_spans.push(wire::RichSpan { content : span
                .mention.to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Medium, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), color : None, link : Some(span
                .mention_link.to_owned()), background : None, border : Some(wire::Border
                { color : None, width : None, radius : Some([4.0f32, 4.0f32, 4.0f32,
                4.0f32,]), }), padding : Some(wire::Edges { top : 0.0f32, right : 1.0f32,
                bottom : 0.0f32, left : 1.0f32, }), underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.link_text
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Medium, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), color : None, link : Some(span.link
                .to_owned()), background : None, border : None, padding : None, underline
                : true, strikethrough : false, }); rich_spans.push(wire::RichSpan {
                content : span.bold_italic.to_owned().to_string(), size : None,
                line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Bold,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Italic, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.bold.to_owned().to_string(), size :
                None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Bold,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.italic.to_owned().to_string(), size
                : None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Normal,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Italic, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.plain.to_owned().to_string(), size
                : None, line_height : None, font : None, color : None, link : None,
                background : None, border : None, padding : None, underline : false,
                strikethrough : false, }); } wire::Node::RichText { options :
                wire::TextOptions { wrapping : Some(wire::Wrapping::WordOrGlyph),
                ..Default::default() }, key : format!("{}/@text:32",
                rich_line_scope_780), size : Some(13.5f32), color : None, font :
                wire::Font { monospace : false, weight : wire::Weight::Normal, }, width :
                Some(wire::Length::Fill), align_x : None, spans : rich_spans, on_link :
                Some(::ducktape_view_guest::slots::handler:: < String, Message, >
                (Box::new({ let route = { let route_callback = lazy_event_543_16.clone();
                move | link : String | route_callback(link) }; move | sent : String |
                Some(route(sent)) }),),), } } }); } if ! block.rich { children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:157",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),); } } }
                native::spaced(native::sized(native::column(format!("{}/@layout:75",
                rich_body_scope_688), children,), Some(wire::Length::Fill), None,),
                5.0f32,) } }]; wire::Node::Linear { max_width : Some(760.0f32), clip :
                false, key : format!("{}/@layout:60", message_body_scope_1809), wrap :
                None, axis : wire::Axis::Column, spacing : None, padding : None, width :
                Some(wire::Length::Fill), height : None, align : None, background : None,
                border : None, children : children, } } }), }); if message.edited && !
                message.show_author { children
                .push(native::text_options(native::text(format!("{}/@text:408",
                message_contents_scope_2035), "· edited".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } if ! crate
                ::host::run_of_message(::std::convert::AsRef::as_ref(& message.id),)
                .is_empty() { children.push({ let children : Vec < wire::Node > =
                vec![native::padded(native::button(format!("{}/@button:420",
                message_contents_scope_2035), String::from("View run"),
                Some(::ducktape_view_guest::slots::message(lazy_event_543_17(crate
                ::host::run_of_message(::std::convert::AsRef::as_ref(& message
                .id),),),),), wire::ButtonPreset::Secondary,),
                wire::Edges::all(3.0f32),)];
                native::padded(native::sized(native::row(format!("{}/@layout:419",
                message_contents_scope_2035), children,), Some(wire::Length::Fill),
                None,), wire::Edges { top : 4.0f32, right : 0.0f32, bottom : 0.0f32, left
                : 0.0f32, },) }); } if ! message.reactions.is_empty() { children.push({
                let mut items = Vec::new(); for (index, reaction) in message.reactions
                .iter().enumerate() { let for_scope = format!("{}/@for:1848({})",
                message_contents_scope_2035, index); let flex_child : wire::Node = { let
                reaction_chip_scope_1849 = format!("{}/ReactionChip@1849", for_scope); {
                let node_scope = format!("{}/root", reaction_chip_scope_1849); { let mut
                children : Vec < wire::Node > = Vec::new(); if reaction.reacted_by_me {
                children.push(wire::Node::Button { checked : Some(reaction
                .reacted_by_me), expanded : None, description :
                Some(String::from(reaction.emoji.to_owned())), key :
                format!("{}/@button:218", reaction_chip_scope_1849), content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:225",
                reaction_chip_scope_1849), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:232",
                reaction_chip_scope_1849), reaction.emoji.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:238",
                reaction_chip_scope_1849), reaction.count.to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:231", reaction_chip_scope_1849), wrap :
                None, axis : wire::Axis::Row, spacing : Some(4.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 1.0f32, right : 8.0f32, bottom : 1.0f32, left : 6.0f32, },),),),
                label : Some(String::from("Remove reaction".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_543_21(message.seq,
                reaction.emoji.to_owned()),),), width : None, height : None, padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), });
                } if ! reaction.reacted_by_me { children.push(wire::Node::Button {
                checked : Some(reaction.reacted_by_me), expanded : None, description :
                Some(String::from(reaction.emoji.to_owned())), key :
                format!("{}/@button:262", reaction_chip_scope_1849), content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:269",
                reaction_chip_scope_1849), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:276",
                reaction_chip_scope_1849), reaction.emoji.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:282",
                reaction_chip_scope_1849), reaction.count.to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:275", reaction_chip_scope_1849), wrap :
                None, axis : wire::Axis::Row, spacing : Some(4.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 1.0f32, right : 8.0f32, bottom : 1.0f32, left : 6.0f32, },),),),
                label : Some(String::from("Add reaction".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_543_20(message.seq,
                reaction.emoji.to_owned()),),), width : None, height : None, padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), });
                } native::column(node_scope.clone(), children) } } }; items
                .push((wire::FlexItem::default(), flex_child)); } let (items, children) =
                items.into_iter().unzip(); wire::Node::Flex { key :
                format!("{}/@layout:427", message_contents_scope_2035), items, children,
                background : None, border : None, layout : wire::FlexLayout { direction :
                wire::FlexDirection::Row, wrap : wire::FlexWrap::Wrap, justify : None,
                items : Some(wire::FlexItemAlignment::Start), content : None, row_gap :
                Some(5.0f32), column_gap : Some(5.0f32), padding : Some(wire::Edges { top
                : 6.0f32, right : 0.0f32, bottom : 0.0f32, left : 0.0f32, }), width :
                Some(wire::Length::Fill), height : None, max_width : None, max_height :
                None, clip : false, surface_width : None, surface_height : None,
                surface_max_width : None, }, } }); } if message.reply_count > 0 {
                children.push({ let children : Vec < wire::Node > =
                vec![wire::Node::Button { checked : None, expanded : None, description :
                None, key : format!("{}/@button:447", message_contents_scope_2035),
                content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:452",
                message_contents_scope_2035), { let children : Vec < wire::Node > =
                vec![{ let component_icon_scope_1872 = format!("{}/Icon@1872",
                message_contents_scope_2035); { let node_scope = format!("{}/root",
                component_icon_scope_1872); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "nav-chat"),),);
                wire::Node::Svg { inherit_button_ink : false, key :
                format!("{}/@media:52", component_icon_scope_1872), hash : hash, bytes :
                bytes, label : None, color : None, hover : None, fit : None, rotation :
                None, opacity : None, width : Some(wire::Length::Fixed(12.0f32)), height
                : Some(wire::Length::Fixed(12.0f32)), } }); } native::column(node_scope
                .clone(), children) } } },
                native::text_options(native::text(format!("{}/@text:464",
                message_contents_scope_2035), crate ::host::plural(message.reply_count,
                ::std::convert::AsRef::as_ref(& "reply"), ::std::convert::AsRef::as_ref(&
                "replies"),).to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },)]; wire::Node::Linear {
                max_width : None, clip : false, key : format!("{}/@layout:458",
                message_contents_scope_2035), wrap : None, axis : wire::Axis::Row,
                spacing : Some(6.0f32), padding : None, width : None, height : None,
                align : Some(wire::AlignX::Center), background : None, border : None,
                children : children, } },), wire::Edges { top : 3.0f32, right : 9.0f32,
                bottom : 3.0f32, left : 7.0f32, },),),), label :
                Some(String::from("Open thread".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_543_22(message
                .seq),),), width : None, height : None, padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), }];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:441", message_contents_scope_2035), wrap : None, axis
                : wire::Axis::Row, spacing : Some(6.0f32), padding : Some(wire::Edges {
                top : 6.0f32, right : 0.0f32, bottom : 0.0f32, left : 0.0f32, }), width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }); }
                native::spaced(native::sized(native::column(format!("{}/@layout:340",
                message_contents_scope_2035), children,), Some(wire::Length::Fill),
                None,), 2.0f32,) }); if message.pending { children
                .push(native::padded(native::container(format!("{}/@container:506",
                message_contents_scope_2035), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:508",
                message_contents_scope_2035), message.meta.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },), { let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "dot")),); wire::Node::Svg {
                inherit_button_ink : false, key : format!("{}/@media:514",
                message_contents_scope_2035), hash : hash, bytes : bytes, label : None,
                color : None, hover : None, fit : None, rotation : None, opacity :
                Some(1.0f32), width : Some(wire::Length::Fixed(6.0f32)), height :
                Some(wire::Length::Fixed(6.0f32)), } }]; wire::Node::Linear { max_width :
                None, clip : false, key : format!("{}/@layout:507",
                message_contents_scope_2035), wrap : None, axis : wire::Axis::Row,
                spacing : Some(5.0f32), padding : None, width : None, height : None,
                align : Some(wire::AlignX::Center), background : None, border : None,
                children : children, } },), wire::Edges { top : 0.0f32, right : 7.0f32,
                bottom : 0.0f32, left : 0.0f32, },),); } wire::Node::Linear { max_width :
                None, clip : false, key : format!("{}/@layout:331",
                message_contents_scope_2035), wrap : None, axis : wire::Axis::Row,
                spacing : Some(11.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Left), background : None, border : None, children :
                children, } }]; native::sized(native::column(format!("{}/@layout:330",
                message_contents_scope_2035), children,), Some(wire::Length::Fill),
                None,) } },), Some(wire::Length::Fill), None,), wire::Edges { top :
                4.0f32, right : 7.0f32, bottom : 4.0f32, left : 7.0f32, },),); } }
                wire::Node::Stack { key : format!("{}/@layout:549",
                message_card_scope_2680), width : Some(wire::Length::Fill), height :
                None, padding : None, background : None, border : None, clip : false,
                under : 0u32, children : children, } }, { let mut children : Vec <
                wire::Node > = Vec::new(); if ! message.deleted && ! message.pending {
                children.push(wire::Node::Container { shadow : Default::default(),
                max_width : None, max_height : None, clip : false, key :
                format!("{}/@container:632", message_card_scope_2680), width :
                Some(wire::Length::Fill), height : None, padding : Some(wire::Edges { top
                : 0.0f32, right : 8.0f32, bottom : 0.0f32, left : 0.0f32, }), align_x :
                Some(wire::AlignX::Right), align_y : Some(wire::AlignY::Top), background
                : None.map(wire::Background::Color), border : None, snap : None, content
                : Box::new(native::padded(native::container(format!("{}/@container:640",
                message_card_scope_2680), { let children : Vec < wire::Node > =
                vec![wire::Node::Button { checked : None, expanded : None, description :
                None, key : format!("{}/@button:661", message_card_scope_2680), content :
                wire::ButtonContent::Child(Box::new(native::text(format!("{}/@text:668",
                message_card_scope_2680), "👍".to_owned().to_string(),),),), label :
                Some(String::from("React with 👍".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_543_20(message.seq,
                "👍".to_owned()),),), width : Some(wire::Length::Fixed(27.0f32)),
                height : Some(wire::Length::Fixed(25.0f32)), padding :
                Some(wire::Edges::all(4.0f32)), style : wire::ButtonStyle::default(), },
                wire::Node::Button { checked : None, expanded : None, description : None,
                key : format!("{}/@button:672", message_card_scope_2680), content :
                wire::ButtonContent::Child(Box::new(native::text(format!("{}/@text:679",
                message_card_scope_2680), "✅".to_owned().to_string(),),),), label :
                Some(String::from("React with ✅".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_543_20(message.seq,
                "✅".to_owned()),),), width : Some(wire::Length::Fixed(27.0f32)), height
                : Some(wire::Length::Fixed(25.0f32)), padding :
                Some(wire::Edges::all(4.0f32)), style : wire::ButtonStyle::default(), },
                wire::Node::Button { checked : None, expanded : None, description : None,
                key : format!("{}/@button:683", message_card_scope_2680), content :
                wire::ButtonContent::Child(Box::new(native::text(format!("{}/@text:690",
                message_card_scope_2680), "👀".to_owned().to_string(),),),), label :
                Some(String::from("React with 👀".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_543_20(message.seq,
                "👀".to_owned()),),), width : Some(wire::Length::Fixed(27.0f32)),
                height : Some(wire::Length::Fixed(25.0f32)), padding :
                Some(wire::Edges::all(4.0f32)), style : wire::ButtonStyle::default(), },
                native::sized(native::container(format!("{}/@container:694",
                message_card_scope_2680), wire::Node::Space { width :
                Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(1.0f32)), },),
                Some(wire::Length::Fixed(1.0f32)), Some(wire::Length::Fixed(16.0f32)),),
                wire::Node::Button { checked : None, expanded : None, description : None,
                key : format!("{}/@button:700", message_card_scope_2680), content :
                wire::ButtonContent::Child(Box::new(native::text(format!("{}/@text:707",
                message_card_scope_2680), "♡".to_owned().to_string(),),),), label :
                Some(String::from("Manage reactions".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_543_24(message.seq,
                message.body.to_owned(), message.rev,),),), width :
                Some(wire::Length::Fixed(27.0f32)), height :
                Some(wire::Length::Fixed(25.0f32)), padding :
                Some(wire::Edges::all(4.0f32)), style : wire::ButtonStyle::default(), },
                wire::Node::Button { checked : None, expanded : None, description : None,
                key : format!("{}/@button:711", message_card_scope_2680), content :
                wire::ButtonContent::Child(Box::new({ let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "nav-chat"),),);
                wire::Node::Svg { inherit_button_ink : true, key :
                format!("{}/@media:720", message_card_scope_2680), hash : hash, bytes :
                bytes, label : None, color : None, hover : None, fit : None, rotation :
                None, opacity : None, width : Some(wire::Length::Fixed(15.0f32)), height
                : Some(wire::Length::Fixed(15.0f32)), } }),), label :
                Some(String::from("Open thread".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_543_22(message
                .seq),),), width : None, height : None, padding :
                Some(wire::Edges::all(5.0f32)), style : wire::ButtonStyle::default(), },
                wire::Node::Button { checked : None, expanded : None, description : None,
                key : format!("{}/@button:728", message_card_scope_2680), content :
                wire::ButtonContent::Child(Box::new(native::text(format!("{}/@text:735",
                message_card_scope_2680), "⋯".to_owned().to_string(),),),), label :
                Some(String::from("More message actions".to_owned()),), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_543_23(message.seq,
                message.body.to_owned(), message.rev,),),), width :
                Some(wire::Length::Fixed(27.0f32)), height :
                Some(wire::Length::Fixed(25.0f32)), padding :
                Some(wire::Edges::all(4.0f32)), style : wire::ButtonStyle::default(), }];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:660", message_card_scope_2680), wrap : None, axis :
                wire::Axis::Row, spacing : Some(1.0f32), padding : None, width : None,
                height : None, align : Some(wire::AlignX::Center), background : None,
                border : None, children : children, } },), wire::Edges { top : 2.0f32,
                right : 2.0f32, bottom : 2.0f32, left : 2.0f32, },),), }); } if message
                .deleted || message.pending { children.push(wire::Node::Space { width :
                Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(1.0f32)), }); }
                native::sized(native::column(format!("{}/@layout:630",
                message_card_scope_2680), children,), Some(wire::Length::Fill), None,)
                }]; wire::Node::Hover { key : format!("{}/@layout:544",
                message_card_scope_2680), width : None, height : None, padding : None,
                background : None, border : None, tint : None, radius : 9.0f32, open :
                true, children : children, } });
                native::sized(native::column(format!("{}/@layout:530",
                message_card_scope_2680), children,), Some(wire::Length::Fill), None,) }
                }]; wire::Node::Stack { key : node_scope.clone(), width :
                Some(wire::Length::Fill), height : None, padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, } }
                }); } if message.seq != selected_message_seq { children.push({ let
                _lazy_context_410 = message_timeline_scope_3465.to_owned(); let
                lazy_event_410_0 = lazy_event_543_20.clone(); let lazy_event_410_1 =
                lazy_event_543_21.clone(); let lazy_event_410_2 = lazy_event_543_22
                .clone(); let lazy_event_410_3 = lazy_event_543_24.clone(); let
                lazy_event_410_4 = lazy_event_543_23.clone(); let lazy_event_410_5 =
                lazy_event_543_16.clone(); let lazy_event_410_6 = lazy_event_543_17
                .clone(); let lazy_event_410_7 = lazy_event_543_1.clone(); { let lazy_key
                = format!("{}/@lazy:96", key_recon);
                ::ducktape_view_guest::memo_lazy((message.clone(), copy_anchor_seq,
                copy_head_seq, copy_surface.clone(), format!("{}/key({})",
                message_timeline_scope_3465, key) .to_owned(), (),), move |
                dependency | { let cached_message : crate ::host::ChatMessage =
                dependency.0.clone(); let copy_anchor_seq : i64 = dependency.1.clone();
                let copy_head_seq : i64 = dependency.2.clone(); let copy_surface :
                CopySurface = dependency.3.clone(); let lazy_scope = dependency.4
                .clone(); { let node_scope = format!("{}/message({})", lazy_scope,
                cached_message.id); { let children : Vec < wire::Node > = vec![{ let
                message_card_scope_2701 = format!("{}/MessageCard@2701", node_scope); {
                let mut children : Vec < wire::Node > = Vec::new(); if cached_message
                .show_author { children.push(wire::Node::Space { width :
                Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(14.0f32)), }); } children.push({ let children : Vec < wire::Node > = vec![{ let mut children : Vec <
                wire::Node > = Vec::new(); match & crate
                ::host::message_plate(cached_message.deleted, false, crate
                ::host::seq_in_copy_range(cached_message.seq, copy_anchor_seq,
                copy_head_seq, copy_surface.clone(), CopySurface::Timeline,),) {
                RowPlate::Plain => { children
                .push(native::padded(native::sized(native::container(format!("{}/@container:571",
                message_card_scope_2701), { let message_contents_scope_1995 =
                format!("{}/MessageContents@1995", message_card_scope_2701); { let children : Vec < wire::Node > = vec![{ let mut children : Vec <
                wire::Node > = Vec::new(); if cached_message.show_author { children
                .push({ let message_avatar_scope_1750 = format!("{}/MessageAvatar@1750",
                message_contents_scope_1995); { let node_scope = format!("{}/root",
                message_avatar_scope_1750); { let mut children : Vec < wire::Node > =
                Vec::new(); if cached_message.avatar_kind == "human" { children.push({
                let person_avatar_scope_798 = format!("{}/PersonAvatar@798",
                message_avatar_scope_1750); { let node_scope = format!("{}/root",
                person_avatar_scope_798); { let children : Vec < wire::Node > =
                vec![{ let principal_avatar_scope_863 = format!("{}/PrincipalAvatar@863",
                person_avatar_scope_798); { let node_scope = format!("{}/root",
                principal_avatar_scope_863); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let principal_plate_scope_909 =
                format!("{}/PrincipalPlate@909", principal_avatar_scope_863); { let
                node_scope = format!("{}/root", principal_plate_scope_909); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                human_plate_scope_925 = format!("{}/HumanPlate@925",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                human_plate_scope_925); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:328", human_plate_scope_925), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([((30.0 / 2.0) as f32).max(0.0).min(f32::MAX), ((30.0
                / 2.0) as f32).max(0.0).min(f32::MAX), ((30.0 / 2.0) as f32).max(0.0)
                .min(f32::MAX), ((30.0 / 2.0) as f32).max(0.0).min(f32::MAX),]), }), snap
                : None, content :
                Box::new(native::text_options(native::text(format!("{}/@text:336",
                human_plate_scope_925), cached_message.initial.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), }); } native::column(node_scope.clone(),
                children) } } }); } native::column(node_scope.clone(), children) } } });
                } native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } if !
                (cached_message.avatar_kind == "human") && cached_message.avatar_kind ==
                "agent" { children.push({ let agent_avatar_scope_804 =
                format!("{}/AgentAvatar@804", message_avatar_scope_1750); { let
                node_scope = format!("{}/root", agent_avatar_scope_804); { let children : Vec < wire::Node > = vec![{ let principal_avatar_scope_873 =
                format!("{}/PrincipalAvatar@873", agent_avatar_scope_804); { let
                node_scope = format!("{}/root", principal_avatar_scope_873); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                principal_plate_scope_909 = format!("{}/PrincipalPlate@909",
                principal_avatar_scope_873); { let node_scope = format!("{}/root",
                principal_plate_scope_909); { let children : Vec < wire::Node > =
                vec![{ let agent_plate_scope_919 = format!("{}/AgentPlate@919",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                agent_plate_scope_919); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let agent_square_scope_1174 =
                format!("{}/AgentSquare@1174", agent_plate_scope_919); { let node_scope =
                format!("{}/root", agent_square_scope_1174); wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : node_scope.clone(), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content :
                Box::new(native::text_options(native::text(format!("{}/@text:390",
                agent_square_scope_1174), cached_message.initial.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), } } }); } native::column(node_scope.clone(),
                children) } } }]; native::column(node_scope.clone(), children) } } }); }
                native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } if !
                (cached_message.avatar_kind == "human" || cached_message.avatar_kind ==
                "agent") { children.push({ let agent_avatar_scope_810 =
                format!("{}/AgentAvatar@810", message_avatar_scope_1750); { let
                node_scope = format!("{}/root", agent_avatar_scope_810); { let children : Vec < wire::Node > = vec![{ let principal_avatar_scope_873 =
                format!("{}/PrincipalAvatar@873", agent_avatar_scope_810); { let
                node_scope = format!("{}/root", principal_avatar_scope_873); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                principal_plate_scope_909 = format!("{}/PrincipalPlate@909",
                principal_avatar_scope_873); { let node_scope = format!("{}/root",
                principal_plate_scope_909); { let children : Vec < wire::Node > =
                vec![{ let agent_plate_scope_919 = format!("{}/AgentPlate@919",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                agent_plate_scope_919); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let agent_square_scope_1174 =
                format!("{}/AgentSquare@1174", agent_plate_scope_919); { let node_scope =
                format!("{}/root", agent_square_scope_1174); wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : node_scope.clone(), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content :
                Box::new(native::text_options(native::text(format!("{}/@text:390",
                agent_square_scope_1174), cached_message.initial.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), } } }); } native::column(node_scope.clone(),
                children) } } }]; native::column(node_scope.clone(), children) } } }); }
                native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } wire::Node::Stack
                { key : node_scope.clone(), width : Some(wire::Length::Fixed(30.0f32)),
                height : Some(wire::Length::Fixed(30.0f32)), padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, } }
                } }); } if ! cached_message.show_author { children.push(wire::Node::Space
                { width : Some(wire::Length::Fixed(30.0f32)), height : None, }); }
                children.push({ let mut children : Vec < wire::Node > = Vec::new(); if
                cached_message.show_author { children.push({ let mut children : Vec <
                wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:347",
                message_contents_scope_1995), cached_message.author.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; if cached_message.avatar_kind == "agent" {
                children
                .push(native::padded(native::container(format!("{}/@container:354",
                message_contents_scope_1995),
                native::text_options(native::text(format!("{}/@text:360",
                message_contents_scope_1995), "AGENT".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), wire::Edges { top : 2.0f32, right : 5.0f32,
                bottom : 2.0f32, left : 5.0f32, },),); } if cached_message.height > 0 {
                children.push(native::text_options(native::text(format!("{}/@text:377",
                message_contents_scope_1995), crate
                ::host::height_label_short(cached_message.height).to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } if cached_message.edited { children
                .push(native::text_options(native::text(format!("{}/@text:384",
                message_contents_scope_1995), "· edited".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } children.push(wire::Node::Space { width :
                Some(wire::Length::Fill), height : None, }); wire::Node::Linear {
                max_width : None, clip : false, key : format!("{}/@layout:342",
                message_contents_scope_1995), wrap : None, axis : wire::Axis::Row,
                spacing : Some(7.0f32), padding : None, width : Some(wire::Length::Fill),
                height : None, align : Some(wire::AlignX::Center), background : None,
                border : None, children : children, } }); } children
                .push(wire::Node::MouseArea { key : format!("{}/@mouse:395",
                message_contents_scope_1995), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_410_7(cached_message
                .seq, CopySurface::Timeline.clone(),),),), on_release : None,
                on_double_click : None, on_right_press : None, on_right_release : None,
                on_middle_press : None, on_middle_release : None, on_enter : None,
                on_exit : None, on_move : None, on_press_at : None, on_scroll : None,
                content : Box::new({ let message_body_scope_1809 =
                format!("{}/MessageBody@1809", message_contents_scope_1995); { let children : Vec < wire::Node > = vec![{ let rich_body_scope_688 =
                format!("{}/RichBody@688", message_body_scope_1809); { let mut children :
                Vec < wire::Node > = Vec::new(); for (index, block) in cached_message
                .blocks.iter().enumerate() { let for_scope = format!("{}/@for:703({})",
                rich_body_scope_688, index); if block.kind == "divider" { children.push({
                let component_separator_scope_705 = format!("{}/Separator@705",
                for_scope); { let node_scope = format!("{}/root",
                component_separator_scope_705); wire::Node::Rule { key : node_scope
                .clone(), axis : wire::Axis::Row, thickness : 1.0f32, color : None, weak
                : false, radius : None, snap : None, } } }); } if block.kind == "code" {
                children
                .push(native::padded(native::sized(native::container(format!("{}/@container:90",
                for_scope), { let mut children : Vec < wire::Node > = Vec::new(); if !
                block.lang.is_empty() { children
                .push(native::text_options(native::text(format!("{}/@text:100",
                for_scope), block.lang.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },),); }
                children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:106",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),);
                native::spaced(native::sized(native::column(format!("{}/@layout:98",
                for_scope), children,), Some(wire::Length::Fill), None,), 6.0f32,) },),
                Some(wire::Length::Fill), None,), wire::Edges { top : 11.0f32, right :
                11.0f32, bottom : 11.0f32, left : 11.0f32, },),); } if block.kind ==
                "quote" { children.push({ let children : Vec < wire::Node > = vec![{
                let mut children : Vec < wire::Node > = Vec::new(); if block.rich {
                children.push({ let rich_line_scope_755 = format!("{}/RichLine@755",
                for_scope); { let mut rich_spans : Vec < wire::RichSpan > = Vec::new();
                for span in block.spans.iter().cloned() { rich_spans.push(wire::RichSpan
                { content : span.mention.to_owned().to_string(), size : None, line_height
                : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Medium,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : Some(span.mention_link.to_owned()), background :
                None, border : Some(wire::Border { color : None, width : None, radius :
                Some([4.0f32, 4.0f32, 4.0f32, 4.0f32,]), }), padding : Some(wire::Edges {
                top : 0.0f32, right : 1.0f32, bottom : 0.0f32, left : 1.0f32, }),
                underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.link_text.to_owned().to_string(),
                size : None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Medium,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : Some(span.link.to_owned()), background : None,
                border : None, padding : None, underline : true, strikethrough : false,
                }); rich_spans.push(wire::RichSpan { content : span.bold_italic
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Bold, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Italic, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.bold
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Bold, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Normal, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.italic
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Normal, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Italic, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.plain
                .to_owned().to_string(), size : None, line_height : None, font : None,
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); }
                wire::Node::RichText { options : wire::TextOptions { wrapping :
                Some(wire::Wrapping::WordOrGlyph), ..Default::default() }, key :
                format!("{}/@text:32", rich_line_scope_755), size : Some(13.5f32), color
                : None, font : wire::Font { monospace : false, weight :
                wire::Weight::Normal, }, width : Some(wire::Length::Fill), align_x :
                None, spans : rich_spans, on_link :
                Some(::ducktape_view_guest::slots::handler:: < String, Message, >
                (Box::new({ let route = { let route_callback = lazy_event_410_5.clone();
                move | link : String | route_callback(link) }; move | sent : String |
                Some(route(sent)) }),),), } } }); } if ! block.rich { children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:137",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),); }
                native::padded(native::sized(native::column(format!("{}/@layout:121",
                for_scope), children,), Some(wire::Length::Fill), None,), wire::Edges {
                top : 2.0f32, right : 0.0f32, bottom : 2.0f32, left : 13.0f32, },) },
                native::sized(native::container(format!("{}/@container:144", for_scope),
                wire::Node::Space { width : Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(1.0f32)), },),
                Some(wire::Length::Fixed(3.0f32)), Some(wire::Length::Fill),)];
                wire::Node::Stack { key : format!("{}/@layout:120", for_scope), width :
                Some(wire::Length::Fill), height : None, padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, }
                }); } if block.kind == "paragraph" { if block.rich { children.push({ let
                rich_line_scope_780 = format!("{}/RichLine@780", for_scope); { let mut
                rich_spans : Vec < wire::RichSpan > = Vec::new(); for span in block.spans
                .iter().cloned() { rich_spans.push(wire::RichSpan { content : span
                .mention.to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Medium, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), color : None, link : Some(span
                .mention_link.to_owned()), background : None, border : Some(wire::Border
                { color : None, width : None, radius : Some([4.0f32, 4.0f32, 4.0f32,
                4.0f32,]), }), padding : Some(wire::Edges { top : 0.0f32, right : 1.0f32,
                bottom : 0.0f32, left : 1.0f32, }), underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.link_text
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Medium, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), color : None, link : Some(span.link
                .to_owned()), background : None, border : None, padding : None, underline
                : true, strikethrough : false, }); rich_spans.push(wire::RichSpan {
                content : span.bold_italic.to_owned().to_string(), size : None,
                line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Bold,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Italic, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.bold.to_owned().to_string(), size :
                None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Bold,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.italic.to_owned().to_string(), size
                : None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Normal,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Italic, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.plain.to_owned().to_string(), size
                : None, line_height : None, font : None, color : None, link : None,
                background : None, border : None, padding : None, underline : false,
                strikethrough : false, }); } wire::Node::RichText { options :
                wire::TextOptions { wrapping : Some(wire::Wrapping::WordOrGlyph),
                ..Default::default() }, key : format!("{}/@text:32",
                rich_line_scope_780), size : Some(13.5f32), color : None, font :
                wire::Font { monospace : false, weight : wire::Weight::Normal, }, width :
                Some(wire::Length::Fill), align_x : None, spans : rich_spans, on_link :
                Some(::ducktape_view_guest::slots::handler:: < String, Message, >
                (Box::new({ let route = { let route_callback = lazy_event_410_5.clone();
                move | link : String | route_callback(link) }; move | sent : String |
                Some(route(sent)) }),),), } } }); } if ! block.rich { children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:157",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),); } } }
                native::spaced(native::sized(native::column(format!("{}/@layout:75",
                rich_body_scope_688), children,), Some(wire::Length::Fill), None,),
                5.0f32,) } }]; wire::Node::Linear { max_width : Some(760.0f32), clip :
                false, key : format!("{}/@layout:60", message_body_scope_1809), wrap :
                None, axis : wire::Axis::Column, spacing : None, padding : None, width :
                Some(wire::Length::Fill), height : None, align : None, background : None,
                border : None, children : children, } } }), }); if cached_message.edited
                && ! cached_message.show_author { children
                .push(native::text_options(native::text(format!("{}/@text:408",
                message_contents_scope_1995), "· edited".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } if ! crate
                ::host::run_of_message(::std::convert::AsRef::as_ref(& cached_message
                .id),).is_empty() { children.push({ let children : Vec < wire::Node >
                = vec![native::padded(native::button(format!("{}/@button:420",
                message_contents_scope_1995), String::from("View run"),
                Some(::ducktape_view_guest::slots::message(lazy_event_410_6(crate
                ::host::run_of_message(::std::convert::AsRef::as_ref(& cached_message
                .id),),),),), wire::ButtonPreset::Secondary,),
                wire::Edges::all(3.0f32),)];
                native::padded(native::sized(native::row(format!("{}/@layout:419",
                message_contents_scope_1995), children,), Some(wire::Length::Fill),
                None,), wire::Edges { top : 4.0f32, right : 0.0f32, bottom : 0.0f32, left
                : 0.0f32, },) }); } if ! cached_message.reactions.is_empty() { children
                .push({ let mut items = Vec::new(); for (index, reaction) in
                cached_message.reactions.iter().enumerate() { let for_scope =
                format!("{}/@for:1848({})", message_contents_scope_1995, index); let
                flex_child : wire::Node = { let reaction_chip_scope_1849 =
                format!("{}/ReactionChip@1849", for_scope); { let node_scope =
                format!("{}/root", reaction_chip_scope_1849); { let mut children : Vec <
                wire::Node > = Vec::new(); if reaction.reacted_by_me { children
                .push(wire::Node::Button { checked : Some(reaction.reacted_by_me),
                expanded : None, description : Some(String::from(reaction.emoji
                .to_owned())), key : format!("{}/@button:218", reaction_chip_scope_1849),
                content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:225",
                reaction_chip_scope_1849), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:232",
                reaction_chip_scope_1849), reaction.emoji.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:238",
                reaction_chip_scope_1849), reaction.count.to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:231", reaction_chip_scope_1849), wrap :
                None, axis : wire::Axis::Row, spacing : Some(4.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 1.0f32, right : 8.0f32, bottom : 1.0f32, left : 6.0f32, },),),),
                label : Some(String::from("Remove reaction".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_410_1(cached_message
                .seq, reaction.emoji.to_owned(),),),), width : None, height : None,
                padding : Some(wire::Edges::all(0.0f32)), style :
                wire::ButtonStyle::default(), }); } if ! reaction.reacted_by_me {
                children.push(wire::Node::Button { checked : Some(reaction
                .reacted_by_me), expanded : None, description :
                Some(String::from(reaction.emoji.to_owned())), key :
                format!("{}/@button:262", reaction_chip_scope_1849), content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:269",
                reaction_chip_scope_1849), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:276",
                reaction_chip_scope_1849), reaction.emoji.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:282",
                reaction_chip_scope_1849), reaction.count.to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:275", reaction_chip_scope_1849), wrap :
                None, axis : wire::Axis::Row, spacing : Some(4.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 1.0f32, right : 8.0f32, bottom : 1.0f32, left : 6.0f32, },),),),
                label : Some(String::from("Add reaction".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_410_0(cached_message
                .seq, reaction.emoji.to_owned(),),),), width : None, height : None,
                padding : Some(wire::Edges::all(0.0f32)), style :
                wire::ButtonStyle::default(), }); } native::column(node_scope.clone(),
                children) } } }; items.push((wire::FlexItem::default(), flex_child)); }
                let (items, children) = items.into_iter().unzip(); wire::Node::Flex { key
                : format!("{}/@layout:427", message_contents_scope_1995), items,
                children, background : None, border : None, layout : wire::FlexLayout {
                direction : wire::FlexDirection::Row, wrap : wire::FlexWrap::Wrap,
                justify : None, items : Some(wire::FlexItemAlignment::Start), content :
                None, row_gap : Some(5.0f32), column_gap : Some(5.0f32), padding :
                Some(wire::Edges { top : 6.0f32, right : 0.0f32, bottom : 0.0f32, left :
                0.0f32, }), width : Some(wire::Length::Fill), height : None, max_width :
                None, max_height : None, clip : false, surface_width : None,
                surface_height : None, surface_max_width : None, }, } }); } if
                cached_message.reply_count > 0 { children.push({ let children : Vec <
                wire::Node > = vec![wire::Node::Button { checked : None, expanded : None,
                description : None, key : format!("{}/@button:447",
                message_contents_scope_1995), content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:452",
                message_contents_scope_1995), { let children : Vec < wire::Node > =
                vec![{ let component_icon_scope_1872 = format!("{}/Icon@1872",
                message_contents_scope_1995); { let node_scope = format!("{}/root",
                component_icon_scope_1872); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "nav-chat"),),);
                wire::Node::Svg { inherit_button_ink : false, key :
                format!("{}/@media:52", component_icon_scope_1872), hash : hash, bytes :
                bytes, label : None, color : None, hover : None, fit : None, rotation :
                None, opacity : None, width : Some(wire::Length::Fixed(12.0f32)), height
                : Some(wire::Length::Fixed(12.0f32)), } }); } native::column(node_scope
                .clone(), children) } } },
                native::text_options(native::text(format!("{}/@text:464",
                message_contents_scope_1995), crate ::host::plural(cached_message
                .reply_count, ::std::convert::AsRef::as_ref(& "reply"),
                ::std::convert::AsRef::as_ref(& "replies"),).to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:458", message_contents_scope_1995), wrap
                : None, axis : wire::Axis::Row, spacing : Some(6.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 3.0f32, right : 9.0f32, bottom : 3.0f32, left : 7.0f32, },),),),
                label : Some(String::from("Open thread".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_410_2(cached_message
                .seq),),), width : None, height : None, padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), }];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:441", message_contents_scope_1995), wrap : None, axis
                : wire::Axis::Row, spacing : Some(6.0f32), padding : Some(wire::Edges {
                top : 6.0f32, right : 0.0f32, bottom : 0.0f32, left : 0.0f32, }), width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }); }
                native::spaced(native::sized(native::column(format!("{}/@layout:340",
                message_contents_scope_1995), children,), Some(wire::Length::Fill),
                None,), 2.0f32,) }); if cached_message.pending { children
                .push(native::padded(native::container(format!("{}/@container:506",
                message_contents_scope_1995), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:508",
                message_contents_scope_1995), cached_message.meta.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },), { let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "dot")),); wire::Node::Svg {
                inherit_button_ink : false, key : format!("{}/@media:514",
                message_contents_scope_1995), hash : hash, bytes : bytes, label : None,
                color : None, hover : None, fit : None, rotation : None, opacity :
                Some(1.0f32), width : Some(wire::Length::Fixed(6.0f32)), height :
                Some(wire::Length::Fixed(6.0f32)), } }]; wire::Node::Linear { max_width :
                None, clip : false, key : format!("{}/@layout:507",
                message_contents_scope_1995), wrap : None, axis : wire::Axis::Row,
                spacing : Some(5.0f32), padding : None, width : None, height : None,
                align : Some(wire::AlignX::Center), background : None, border : None,
                children : children, } },), wire::Edges { top : 0.0f32, right : 7.0f32,
                bottom : 0.0f32, left : 0.0f32, },),); } wire::Node::Linear { max_width :
                None, clip : false, key : format!("{}/@layout:331",
                message_contents_scope_1995), wrap : None, axis : wire::Axis::Row,
                spacing : Some(11.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Left), background : None, border : None, children :
                children, } }]; native::sized(native::column(format!("{}/@layout:330",
                message_contents_scope_1995), children,), Some(wire::Length::Fill),
                None,) } },), Some(wire::Length::Fill), None,), wire::Edges { top :
                4.0f32, right : 7.0f32, bottom : 4.0f32, left : 7.0f32, },),); }
                RowPlate::Selected => { children
                .push(native::padded(native::sized(native::container(format!("{}/@container:591",
                message_card_scope_2701), { let message_contents_scope_2015 =
                format!("{}/MessageContents@2015", message_card_scope_2701); { let children : Vec < wire::Node > = vec![{ let mut children : Vec <
                wire::Node > = Vec::new(); if cached_message.show_author { children
                .push({ let message_avatar_scope_1750 = format!("{}/MessageAvatar@1750",
                message_contents_scope_2015); { let node_scope = format!("{}/root",
                message_avatar_scope_1750); { let mut children : Vec < wire::Node > =
                Vec::new(); if cached_message.avatar_kind == "human" { children.push({
                let person_avatar_scope_798 = format!("{}/PersonAvatar@798",
                message_avatar_scope_1750); { let node_scope = format!("{}/root",
                person_avatar_scope_798); { let children : Vec < wire::Node > =
                vec![{ let principal_avatar_scope_863 = format!("{}/PrincipalAvatar@863",
                person_avatar_scope_798); { let node_scope = format!("{}/root",
                principal_avatar_scope_863); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let principal_plate_scope_909 =
                format!("{}/PrincipalPlate@909", principal_avatar_scope_863); { let
                node_scope = format!("{}/root", principal_plate_scope_909); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                human_plate_scope_925 = format!("{}/HumanPlate@925",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                human_plate_scope_925); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:328", human_plate_scope_925), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([((30.0 / 2.0) as f32).max(0.0).min(f32::MAX), ((30.0
                / 2.0) as f32).max(0.0).min(f32::MAX), ((30.0 / 2.0) as f32).max(0.0)
                .min(f32::MAX), ((30.0 / 2.0) as f32).max(0.0).min(f32::MAX),]), }), snap
                : None, content :
                Box::new(native::text_options(native::text(format!("{}/@text:336",
                human_plate_scope_925), cached_message.initial.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), }); } native::column(node_scope.clone(),
                children) } } }); } native::column(node_scope.clone(), children) } } });
                } native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } if !
                (cached_message.avatar_kind == "human") && cached_message.avatar_kind ==
                "agent" { children.push({ let agent_avatar_scope_804 =
                format!("{}/AgentAvatar@804", message_avatar_scope_1750); { let
                node_scope = format!("{}/root", agent_avatar_scope_804); { let children : Vec < wire::Node > = vec![{ let principal_avatar_scope_873 =
                format!("{}/PrincipalAvatar@873", agent_avatar_scope_804); { let
                node_scope = format!("{}/root", principal_avatar_scope_873); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                principal_plate_scope_909 = format!("{}/PrincipalPlate@909",
                principal_avatar_scope_873); { let node_scope = format!("{}/root",
                principal_plate_scope_909); { let children : Vec < wire::Node > =
                vec![{ let agent_plate_scope_919 = format!("{}/AgentPlate@919",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                agent_plate_scope_919); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let agent_square_scope_1174 =
                format!("{}/AgentSquare@1174", agent_plate_scope_919); { let node_scope =
                format!("{}/root", agent_square_scope_1174); wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : node_scope.clone(), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content :
                Box::new(native::text_options(native::text(format!("{}/@text:390",
                agent_square_scope_1174), cached_message.initial.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), } } }); } native::column(node_scope.clone(),
                children) } } }]; native::column(node_scope.clone(), children) } } }); }
                native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } if !
                (cached_message.avatar_kind == "human" || cached_message.avatar_kind ==
                "agent") { children.push({ let agent_avatar_scope_810 =
                format!("{}/AgentAvatar@810", message_avatar_scope_1750); { let
                node_scope = format!("{}/root", agent_avatar_scope_810); { let children : Vec < wire::Node > = vec![{ let principal_avatar_scope_873 =
                format!("{}/PrincipalAvatar@873", agent_avatar_scope_810); { let
                node_scope = format!("{}/root", principal_avatar_scope_873); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                principal_plate_scope_909 = format!("{}/PrincipalPlate@909",
                principal_avatar_scope_873); { let node_scope = format!("{}/root",
                principal_plate_scope_909); { let children : Vec < wire::Node > =
                vec![{ let agent_plate_scope_919 = format!("{}/AgentPlate@919",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                agent_plate_scope_919); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let agent_square_scope_1174 =
                format!("{}/AgentSquare@1174", agent_plate_scope_919); { let node_scope =
                format!("{}/root", agent_square_scope_1174); wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : node_scope.clone(), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content :
                Box::new(native::text_options(native::text(format!("{}/@text:390",
                agent_square_scope_1174), cached_message.initial.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), } } }); } native::column(node_scope.clone(),
                children) } } }]; native::column(node_scope.clone(), children) } } }); }
                native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } wire::Node::Stack
                { key : node_scope.clone(), width : Some(wire::Length::Fixed(30.0f32)),
                height : Some(wire::Length::Fixed(30.0f32)), padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, } }
                } }); } if ! cached_message.show_author { children.push(wire::Node::Space
                { width : Some(wire::Length::Fixed(30.0f32)), height : None, }); }
                children.push({ let mut children : Vec < wire::Node > = Vec::new(); if
                cached_message.show_author { children.push({ let mut children : Vec <
                wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:347",
                message_contents_scope_2015), cached_message.author.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; if cached_message.avatar_kind == "agent" {
                children
                .push(native::padded(native::container(format!("{}/@container:354",
                message_contents_scope_2015),
                native::text_options(native::text(format!("{}/@text:360",
                message_contents_scope_2015), "AGENT".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), wire::Edges { top : 2.0f32, right : 5.0f32,
                bottom : 2.0f32, left : 5.0f32, },),); } if cached_message.height > 0 {
                children.push(native::text_options(native::text(format!("{}/@text:377",
                message_contents_scope_2015), crate
                ::host::height_label_short(cached_message.height).to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } if cached_message.edited { children
                .push(native::text_options(native::text(format!("{}/@text:384",
                message_contents_scope_2015), "· edited".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } children.push(wire::Node::Space { width :
                Some(wire::Length::Fill), height : None, }); wire::Node::Linear {
                max_width : None, clip : false, key : format!("{}/@layout:342",
                message_contents_scope_2015), wrap : None, axis : wire::Axis::Row,
                spacing : Some(7.0f32), padding : None, width : Some(wire::Length::Fill),
                height : None, align : Some(wire::AlignX::Center), background : None,
                border : None, children : children, } }); } children
                .push(wire::Node::MouseArea { key : format!("{}/@mouse:395",
                message_contents_scope_2015), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_410_7(cached_message
                .seq, CopySurface::Timeline.clone(),),),), on_release : None,
                on_double_click : None, on_right_press : None, on_right_release : None,
                on_middle_press : None, on_middle_release : None, on_enter : None,
                on_exit : None, on_move : None, on_press_at : None, on_scroll : None,
                content : Box::new({ let message_body_scope_1809 =
                format!("{}/MessageBody@1809", message_contents_scope_2015); { let children : Vec < wire::Node > = vec![{ let rich_body_scope_688 =
                format!("{}/RichBody@688", message_body_scope_1809); { let mut children :
                Vec < wire::Node > = Vec::new(); for (index, block) in cached_message
                .blocks.iter().enumerate() { let for_scope = format!("{}/@for:703({})",
                rich_body_scope_688, index); if block.kind == "divider" { children.push({
                let component_separator_scope_705 = format!("{}/Separator@705",
                for_scope); { let node_scope = format!("{}/root",
                component_separator_scope_705); wire::Node::Rule { key : node_scope
                .clone(), axis : wire::Axis::Row, thickness : 1.0f32, color : None, weak
                : false, radius : None, snap : None, } } }); } if block.kind == "code" {
                children
                .push(native::padded(native::sized(native::container(format!("{}/@container:90",
                for_scope), { let mut children : Vec < wire::Node > = Vec::new(); if !
                block.lang.is_empty() { children
                .push(native::text_options(native::text(format!("{}/@text:100",
                for_scope), block.lang.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },),); }
                children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:106",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),);
                native::spaced(native::sized(native::column(format!("{}/@layout:98",
                for_scope), children,), Some(wire::Length::Fill), None,), 6.0f32,) },),
                Some(wire::Length::Fill), None,), wire::Edges { top : 11.0f32, right :
                11.0f32, bottom : 11.0f32, left : 11.0f32, },),); } if block.kind ==
                "quote" { children.push({ let children : Vec < wire::Node > = vec![{
                let mut children : Vec < wire::Node > = Vec::new(); if block.rich {
                children.push({ let rich_line_scope_755 = format!("{}/RichLine@755",
                for_scope); { let mut rich_spans : Vec < wire::RichSpan > = Vec::new();
                for span in block.spans.iter().cloned() { rich_spans.push(wire::RichSpan
                { content : span.mention.to_owned().to_string(), size : None, line_height
                : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Medium,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : Some(span.mention_link.to_owned()), background :
                None, border : Some(wire::Border { color : None, width : None, radius :
                Some([4.0f32, 4.0f32, 4.0f32, 4.0f32,]), }), padding : Some(wire::Edges {
                top : 0.0f32, right : 1.0f32, bottom : 0.0f32, left : 1.0f32, }),
                underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.link_text.to_owned().to_string(),
                size : None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Medium,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : Some(span.link.to_owned()), background : None,
                border : None, padding : None, underline : true, strikethrough : false,
                }); rich_spans.push(wire::RichSpan { content : span.bold_italic
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Bold, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Italic, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.bold
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Bold, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Normal, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.italic
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Normal, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Italic, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.plain
                .to_owned().to_string(), size : None, line_height : None, font : None,
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); }
                wire::Node::RichText { options : wire::TextOptions { wrapping :
                Some(wire::Wrapping::WordOrGlyph), ..Default::default() }, key :
                format!("{}/@text:32", rich_line_scope_755), size : Some(13.5f32), color
                : None, font : wire::Font { monospace : false, weight :
                wire::Weight::Normal, }, width : Some(wire::Length::Fill), align_x :
                None, spans : rich_spans, on_link :
                Some(::ducktape_view_guest::slots::handler:: < String, Message, >
                (Box::new({ let route = { let route_callback = lazy_event_410_5.clone();
                move | link : String | route_callback(link) }; move | sent : String |
                Some(route(sent)) }),),), } } }); } if ! block.rich { children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:137",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),); }
                native::padded(native::sized(native::column(format!("{}/@layout:121",
                for_scope), children,), Some(wire::Length::Fill), None,), wire::Edges {
                top : 2.0f32, right : 0.0f32, bottom : 2.0f32, left : 13.0f32, },) },
                native::sized(native::container(format!("{}/@container:144", for_scope),
                wire::Node::Space { width : Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(1.0f32)), },),
                Some(wire::Length::Fixed(3.0f32)), Some(wire::Length::Fill),)];
                wire::Node::Stack { key : format!("{}/@layout:120", for_scope), width :
                Some(wire::Length::Fill), height : None, padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, }
                }); } if block.kind == "paragraph" { if block.rich { children.push({ let
                rich_line_scope_780 = format!("{}/RichLine@780", for_scope); { let mut
                rich_spans : Vec < wire::RichSpan > = Vec::new(); for span in block.spans
                .iter().cloned() { rich_spans.push(wire::RichSpan { content : span
                .mention.to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Medium, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), color : None, link : Some(span
                .mention_link.to_owned()), background : None, border : Some(wire::Border
                { color : None, width : None, radius : Some([4.0f32, 4.0f32, 4.0f32,
                4.0f32,]), }), padding : Some(wire::Edges { top : 0.0f32, right : 1.0f32,
                bottom : 0.0f32, left : 1.0f32, }), underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.link_text
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Medium, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), color : None, link : Some(span.link
                .to_owned()), background : None, border : None, padding : None, underline
                : true, strikethrough : false, }); rich_spans.push(wire::RichSpan {
                content : span.bold_italic.to_owned().to_string(), size : None,
                line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Bold,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Italic, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.bold.to_owned().to_string(), size :
                None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Bold,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.italic.to_owned().to_string(), size
                : None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Normal,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Italic, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.plain.to_owned().to_string(), size
                : None, line_height : None, font : None, color : None, link : None,
                background : None, border : None, padding : None, underline : false,
                strikethrough : false, }); } wire::Node::RichText { options :
                wire::TextOptions { wrapping : Some(wire::Wrapping::WordOrGlyph),
                ..Default::default() }, key : format!("{}/@text:32",
                rich_line_scope_780), size : Some(13.5f32), color : None, font :
                wire::Font { monospace : false, weight : wire::Weight::Normal, }, width :
                Some(wire::Length::Fill), align_x : None, spans : rich_spans, on_link :
                Some(::ducktape_view_guest::slots::handler:: < String, Message, >
                (Box::new({ let route = { let route_callback = lazy_event_410_5.clone();
                move | link : String | route_callback(link) }; move | sent : String |
                Some(route(sent)) }),),), } } }); } if ! block.rich { children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:157",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),); } } }
                native::spaced(native::sized(native::column(format!("{}/@layout:75",
                rich_body_scope_688), children,), Some(wire::Length::Fill), None,),
                5.0f32,) } }]; wire::Node::Linear { max_width : Some(760.0f32), clip :
                false, key : format!("{}/@layout:60", message_body_scope_1809), wrap :
                None, axis : wire::Axis::Column, spacing : None, padding : None, width :
                Some(wire::Length::Fill), height : None, align : None, background : None,
                border : None, children : children, } } }), }); if cached_message.edited
                && ! cached_message.show_author { children
                .push(native::text_options(native::text(format!("{}/@text:408",
                message_contents_scope_2015), "· edited".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } if ! crate
                ::host::run_of_message(::std::convert::AsRef::as_ref(& cached_message
                .id),).is_empty() { children.push({ let children : Vec < wire::Node >
                = vec![native::padded(native::button(format!("{}/@button:420",
                message_contents_scope_2015), String::from("View run"),
                Some(::ducktape_view_guest::slots::message(lazy_event_410_6(crate
                ::host::run_of_message(::std::convert::AsRef::as_ref(& cached_message
                .id),),),),), wire::ButtonPreset::Secondary,),
                wire::Edges::all(3.0f32),)];
                native::padded(native::sized(native::row(format!("{}/@layout:419",
                message_contents_scope_2015), children,), Some(wire::Length::Fill),
                None,), wire::Edges { top : 4.0f32, right : 0.0f32, bottom : 0.0f32, left
                : 0.0f32, },) }); } if ! cached_message.reactions.is_empty() { children
                .push({ let mut items = Vec::new(); for (index, reaction) in
                cached_message.reactions.iter().enumerate() { let for_scope =
                format!("{}/@for:1848({})", message_contents_scope_2015, index); let
                flex_child : wire::Node = { let reaction_chip_scope_1849 =
                format!("{}/ReactionChip@1849", for_scope); { let node_scope =
                format!("{}/root", reaction_chip_scope_1849); { let mut children : Vec <
                wire::Node > = Vec::new(); if reaction.reacted_by_me { children
                .push(wire::Node::Button { checked : Some(reaction.reacted_by_me),
                expanded : None, description : Some(String::from(reaction.emoji
                .to_owned())), key : format!("{}/@button:218", reaction_chip_scope_1849),
                content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:225",
                reaction_chip_scope_1849), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:232",
                reaction_chip_scope_1849), reaction.emoji.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:238",
                reaction_chip_scope_1849), reaction.count.to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:231", reaction_chip_scope_1849), wrap :
                None, axis : wire::Axis::Row, spacing : Some(4.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 1.0f32, right : 8.0f32, bottom : 1.0f32, left : 6.0f32, },),),),
                label : Some(String::from("Remove reaction".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_410_1(cached_message
                .seq, reaction.emoji.to_owned(),),),), width : None, height : None,
                padding : Some(wire::Edges::all(0.0f32)), style :
                wire::ButtonStyle::default(), }); } if ! reaction.reacted_by_me {
                children.push(wire::Node::Button { checked : Some(reaction
                .reacted_by_me), expanded : None, description :
                Some(String::from(reaction.emoji.to_owned())), key :
                format!("{}/@button:262", reaction_chip_scope_1849), content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:269",
                reaction_chip_scope_1849), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:276",
                reaction_chip_scope_1849), reaction.emoji.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:282",
                reaction_chip_scope_1849), reaction.count.to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:275", reaction_chip_scope_1849), wrap :
                None, axis : wire::Axis::Row, spacing : Some(4.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 1.0f32, right : 8.0f32, bottom : 1.0f32, left : 6.0f32, },),),),
                label : Some(String::from("Add reaction".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_410_0(cached_message
                .seq, reaction.emoji.to_owned(),),),), width : None, height : None,
                padding : Some(wire::Edges::all(0.0f32)), style :
                wire::ButtonStyle::default(), }); } native::column(node_scope.clone(),
                children) } } }; items.push((wire::FlexItem::default(), flex_child)); }
                let (items, children) = items.into_iter().unzip(); wire::Node::Flex { key
                : format!("{}/@layout:427", message_contents_scope_2015), items,
                children, background : None, border : None, layout : wire::FlexLayout {
                direction : wire::FlexDirection::Row, wrap : wire::FlexWrap::Wrap,
                justify : None, items : Some(wire::FlexItemAlignment::Start), content :
                None, row_gap : Some(5.0f32), column_gap : Some(5.0f32), padding :
                Some(wire::Edges { top : 6.0f32, right : 0.0f32, bottom : 0.0f32, left :
                0.0f32, }), width : Some(wire::Length::Fill), height : None, max_width :
                None, max_height : None, clip : false, surface_width : None,
                surface_height : None, surface_max_width : None, }, } }); } if
                cached_message.reply_count > 0 { children.push({ let children : Vec <
                wire::Node > = vec![wire::Node::Button { checked : None, expanded : None,
                description : None, key : format!("{}/@button:447",
                message_contents_scope_2015), content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:452",
                message_contents_scope_2015), { let children : Vec < wire::Node > =
                vec![{ let component_icon_scope_1872 = format!("{}/Icon@1872",
                message_contents_scope_2015); { let node_scope = format!("{}/root",
                component_icon_scope_1872); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "nav-chat"),),);
                wire::Node::Svg { inherit_button_ink : false, key :
                format!("{}/@media:52", component_icon_scope_1872), hash : hash, bytes :
                bytes, label : None, color : None, hover : None, fit : None, rotation :
                None, opacity : None, width : Some(wire::Length::Fixed(12.0f32)), height
                : Some(wire::Length::Fixed(12.0f32)), } }); } native::column(node_scope
                .clone(), children) } } },
                native::text_options(native::text(format!("{}/@text:464",
                message_contents_scope_2015), crate ::host::plural(cached_message
                .reply_count, ::std::convert::AsRef::as_ref(& "reply"),
                ::std::convert::AsRef::as_ref(& "replies"),).to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:458", message_contents_scope_2015), wrap
                : None, axis : wire::Axis::Row, spacing : Some(6.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 3.0f32, right : 9.0f32, bottom : 3.0f32, left : 7.0f32, },),),),
                label : Some(String::from("Open thread".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_410_2(cached_message
                .seq),),), width : None, height : None, padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), }];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:441", message_contents_scope_2015), wrap : None, axis
                : wire::Axis::Row, spacing : Some(6.0f32), padding : Some(wire::Edges {
                top : 6.0f32, right : 0.0f32, bottom : 0.0f32, left : 0.0f32, }), width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }); }
                native::spaced(native::sized(native::column(format!("{}/@layout:340",
                message_contents_scope_2015), children,), Some(wire::Length::Fill),
                None,), 2.0f32,) }); if cached_message.pending { children
                .push(native::padded(native::container(format!("{}/@container:506",
                message_contents_scope_2015), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:508",
                message_contents_scope_2015), cached_message.meta.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },), { let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "dot")),); wire::Node::Svg {
                inherit_button_ink : false, key : format!("{}/@media:514",
                message_contents_scope_2015), hash : hash, bytes : bytes, label : None,
                color : None, hover : None, fit : None, rotation : None, opacity :
                Some(1.0f32), width : Some(wire::Length::Fixed(6.0f32)), height :
                Some(wire::Length::Fixed(6.0f32)), } }]; wire::Node::Linear { max_width :
                None, clip : false, key : format!("{}/@layout:507",
                message_contents_scope_2015), wrap : None, axis : wire::Axis::Row,
                spacing : Some(5.0f32), padding : None, width : None, height : None,
                align : Some(wire::AlignX::Center), background : None, border : None,
                children : children, } },), wire::Edges { top : 0.0f32, right : 7.0f32,
                bottom : 0.0f32, left : 0.0f32, },),); } wire::Node::Linear { max_width :
                None, clip : false, key : format!("{}/@layout:331",
                message_contents_scope_2015), wrap : None, axis : wire::Axis::Row,
                spacing : Some(11.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Left), background : None, border : None, children :
                children, } }]; native::sized(native::column(format!("{}/@layout:330",
                message_contents_scope_2015), children,), Some(wire::Length::Fill),
                None,) } },), Some(wire::Length::Fill), None,), wire::Edges { top :
                4.0f32, right : 7.0f32, bottom : 4.0f32, left : 7.0f32, },),); }
                RowPlate::Ranged => { children
                .push(native::padded(native::sized(native::container(format!("{}/@container:611",
                message_card_scope_2701), { let message_contents_scope_2035 =
                format!("{}/MessageContents@2035", message_card_scope_2701); { let children : Vec < wire::Node > = vec![{ let mut children : Vec <
                wire::Node > = Vec::new(); if cached_message.show_author { children
                .push({ let message_avatar_scope_1750 = format!("{}/MessageAvatar@1750",
                message_contents_scope_2035); { let node_scope = format!("{}/root",
                message_avatar_scope_1750); { let mut children : Vec < wire::Node > =
                Vec::new(); if cached_message.avatar_kind == "human" { children.push({
                let person_avatar_scope_798 = format!("{}/PersonAvatar@798",
                message_avatar_scope_1750); { let node_scope = format!("{}/root",
                person_avatar_scope_798); { let children : Vec < wire::Node > =
                vec![{ let principal_avatar_scope_863 = format!("{}/PrincipalAvatar@863",
                person_avatar_scope_798); { let node_scope = format!("{}/root",
                principal_avatar_scope_863); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let principal_plate_scope_909 =
                format!("{}/PrincipalPlate@909", principal_avatar_scope_863); { let
                node_scope = format!("{}/root", principal_plate_scope_909); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                human_plate_scope_925 = format!("{}/HumanPlate@925",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                human_plate_scope_925); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:328", human_plate_scope_925), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([((30.0 / 2.0) as f32).max(0.0).min(f32::MAX), ((30.0
                / 2.0) as f32).max(0.0).min(f32::MAX), ((30.0 / 2.0) as f32).max(0.0)
                .min(f32::MAX), ((30.0 / 2.0) as f32).max(0.0).min(f32::MAX),]), }), snap
                : None, content :
                Box::new(native::text_options(native::text(format!("{}/@text:336",
                human_plate_scope_925), cached_message.initial.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), }); } native::column(node_scope.clone(),
                children) } } }); } native::column(node_scope.clone(), children) } } });
                } native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } if !
                (cached_message.avatar_kind == "human") && cached_message.avatar_kind ==
                "agent" { children.push({ let agent_avatar_scope_804 =
                format!("{}/AgentAvatar@804", message_avatar_scope_1750); { let
                node_scope = format!("{}/root", agent_avatar_scope_804); { let children : Vec < wire::Node > = vec![{ let principal_avatar_scope_873 =
                format!("{}/PrincipalAvatar@873", agent_avatar_scope_804); { let
                node_scope = format!("{}/root", principal_avatar_scope_873); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                principal_plate_scope_909 = format!("{}/PrincipalPlate@909",
                principal_avatar_scope_873); { let node_scope = format!("{}/root",
                principal_plate_scope_909); { let children : Vec < wire::Node > =
                vec![{ let agent_plate_scope_919 = format!("{}/AgentPlate@919",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                agent_plate_scope_919); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let agent_square_scope_1174 =
                format!("{}/AgentSquare@1174", agent_plate_scope_919); { let node_scope =
                format!("{}/root", agent_square_scope_1174); wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : node_scope.clone(), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content :
                Box::new(native::text_options(native::text(format!("{}/@text:390",
                agent_square_scope_1174), cached_message.initial.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), } } }); } native::column(node_scope.clone(),
                children) } } }]; native::column(node_scope.clone(), children) } } }); }
                native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } if !
                (cached_message.avatar_kind == "human" || cached_message.avatar_kind ==
                "agent") { children.push({ let agent_avatar_scope_810 =
                format!("{}/AgentAvatar@810", message_avatar_scope_1750); { let
                node_scope = format!("{}/root", agent_avatar_scope_810); { let children : Vec < wire::Node > = vec![{ let principal_avatar_scope_873 =
                format!("{}/PrincipalAvatar@873", agent_avatar_scope_810); { let
                node_scope = format!("{}/root", principal_avatar_scope_873); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                principal_plate_scope_909 = format!("{}/PrincipalPlate@909",
                principal_avatar_scope_873); { let node_scope = format!("{}/root",
                principal_plate_scope_909); { let children : Vec < wire::Node > =
                vec![{ let agent_plate_scope_919 = format!("{}/AgentPlate@919",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                agent_plate_scope_919); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let agent_square_scope_1174 =
                format!("{}/AgentSquare@1174", agent_plate_scope_919); { let node_scope =
                format!("{}/root", agent_square_scope_1174); wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : node_scope.clone(), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content :
                Box::new(native::text_options(native::text(format!("{}/@text:390",
                agent_square_scope_1174), cached_message.initial.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), } } }); } native::column(node_scope.clone(),
                children) } } }]; native::column(node_scope.clone(), children) } } }); }
                native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } wire::Node::Stack
                { key : node_scope.clone(), width : Some(wire::Length::Fixed(30.0f32)),
                height : Some(wire::Length::Fixed(30.0f32)), padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, } }
                } }); } if ! cached_message.show_author { children.push(wire::Node::Space
                { width : Some(wire::Length::Fixed(30.0f32)), height : None, }); }
                children.push({ let mut children : Vec < wire::Node > = Vec::new(); if
                cached_message.show_author { children.push({ let mut children : Vec <
                wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:347",
                message_contents_scope_2035), cached_message.author.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; if cached_message.avatar_kind == "agent" {
                children
                .push(native::padded(native::container(format!("{}/@container:354",
                message_contents_scope_2035),
                native::text_options(native::text(format!("{}/@text:360",
                message_contents_scope_2035), "AGENT".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), wire::Edges { top : 2.0f32, right : 5.0f32,
                bottom : 2.0f32, left : 5.0f32, },),); } if cached_message.height > 0 {
                children.push(native::text_options(native::text(format!("{}/@text:377",
                message_contents_scope_2035), crate
                ::host::height_label_short(cached_message.height).to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } if cached_message.edited { children
                .push(native::text_options(native::text(format!("{}/@text:384",
                message_contents_scope_2035), "· edited".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } children.push(wire::Node::Space { width :
                Some(wire::Length::Fill), height : None, }); wire::Node::Linear {
                max_width : None, clip : false, key : format!("{}/@layout:342",
                message_contents_scope_2035), wrap : None, axis : wire::Axis::Row,
                spacing : Some(7.0f32), padding : None, width : Some(wire::Length::Fill),
                height : None, align : Some(wire::AlignX::Center), background : None,
                border : None, children : children, } }); } children
                .push(wire::Node::MouseArea { key : format!("{}/@mouse:395",
                message_contents_scope_2035), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_410_7(cached_message
                .seq, CopySurface::Timeline.clone(),),),), on_release : None,
                on_double_click : None, on_right_press : None, on_right_release : None,
                on_middle_press : None, on_middle_release : None, on_enter : None,
                on_exit : None, on_move : None, on_press_at : None, on_scroll : None,
                content : Box::new({ let message_body_scope_1809 =
                format!("{}/MessageBody@1809", message_contents_scope_2035); { let children : Vec < wire::Node > = vec![{ let rich_body_scope_688 =
                format!("{}/RichBody@688", message_body_scope_1809); { let mut children :
                Vec < wire::Node > = Vec::new(); for (index, block) in cached_message
                .blocks.iter().enumerate() { let for_scope = format!("{}/@for:703({})",
                rich_body_scope_688, index); if block.kind == "divider" { children.push({
                let component_separator_scope_705 = format!("{}/Separator@705",
                for_scope); { let node_scope = format!("{}/root",
                component_separator_scope_705); wire::Node::Rule { key : node_scope
                .clone(), axis : wire::Axis::Row, thickness : 1.0f32, color : None, weak
                : false, radius : None, snap : None, } } }); } if block.kind == "code" {
                children
                .push(native::padded(native::sized(native::container(format!("{}/@container:90",
                for_scope), { let mut children : Vec < wire::Node > = Vec::new(); if !
                block.lang.is_empty() { children
                .push(native::text_options(native::text(format!("{}/@text:100",
                for_scope), block.lang.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },),); }
                children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:106",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),);
                native::spaced(native::sized(native::column(format!("{}/@layout:98",
                for_scope), children,), Some(wire::Length::Fill), None,), 6.0f32,) },),
                Some(wire::Length::Fill), None,), wire::Edges { top : 11.0f32, right :
                11.0f32, bottom : 11.0f32, left : 11.0f32, },),); } if block.kind ==
                "quote" { children.push({ let children : Vec < wire::Node > = vec![{
                let mut children : Vec < wire::Node > = Vec::new(); if block.rich {
                children.push({ let rich_line_scope_755 = format!("{}/RichLine@755",
                for_scope); { let mut rich_spans : Vec < wire::RichSpan > = Vec::new();
                for span in block.spans.iter().cloned() { rich_spans.push(wire::RichSpan
                { content : span.mention.to_owned().to_string(), size : None, line_height
                : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Medium,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : Some(span.mention_link.to_owned()), background :
                None, border : Some(wire::Border { color : None, width : None, radius :
                Some([4.0f32, 4.0f32, 4.0f32, 4.0f32,]), }), padding : Some(wire::Edges {
                top : 0.0f32, right : 1.0f32, bottom : 0.0f32, left : 1.0f32, }),
                underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.link_text.to_owned().to_string(),
                size : None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Medium,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : Some(span.link.to_owned()), background : None,
                border : None, padding : None, underline : true, strikethrough : false,
                }); rich_spans.push(wire::RichSpan { content : span.bold_italic
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Bold, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Italic, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.bold
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Bold, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Normal, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.italic
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Normal, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Italic, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.plain
                .to_owned().to_string(), size : None, line_height : None, font : None,
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); }
                wire::Node::RichText { options : wire::TextOptions { wrapping :
                Some(wire::Wrapping::WordOrGlyph), ..Default::default() }, key :
                format!("{}/@text:32", rich_line_scope_755), size : Some(13.5f32), color
                : None, font : wire::Font { monospace : false, weight :
                wire::Weight::Normal, }, width : Some(wire::Length::Fill), align_x :
                None, spans : rich_spans, on_link :
                Some(::ducktape_view_guest::slots::handler:: < String, Message, >
                (Box::new({ let route = { let route_callback = lazy_event_410_5.clone();
                move | link : String | route_callback(link) }; move | sent : String |
                Some(route(sent)) }),),), } } }); } if ! block.rich { children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:137",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),); }
                native::padded(native::sized(native::column(format!("{}/@layout:121",
                for_scope), children,), Some(wire::Length::Fill), None,), wire::Edges {
                top : 2.0f32, right : 0.0f32, bottom : 2.0f32, left : 13.0f32, },) },
                native::sized(native::container(format!("{}/@container:144", for_scope),
                wire::Node::Space { width : Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(1.0f32)), },),
                Some(wire::Length::Fixed(3.0f32)), Some(wire::Length::Fill),)];
                wire::Node::Stack { key : format!("{}/@layout:120", for_scope), width :
                Some(wire::Length::Fill), height : None, padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, }
                }); } if block.kind == "paragraph" { if block.rich { children.push({ let
                rich_line_scope_780 = format!("{}/RichLine@780", for_scope); { let mut
                rich_spans : Vec < wire::RichSpan > = Vec::new(); for span in block.spans
                .iter().cloned() { rich_spans.push(wire::RichSpan { content : span
                .mention.to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Medium, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), color : None, link : Some(span
                .mention_link.to_owned()), background : None, border : Some(wire::Border
                { color : None, width : None, radius : Some([4.0f32, 4.0f32, 4.0f32,
                4.0f32,]), }), padding : Some(wire::Edges { top : 0.0f32, right : 1.0f32,
                bottom : 0.0f32, left : 1.0f32, }), underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.link_text
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Medium, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), color : None, link : Some(span.link
                .to_owned()), background : None, border : None, padding : None, underline
                : true, strikethrough : false, }); rich_spans.push(wire::RichSpan {
                content : span.bold_italic.to_owned().to_string(), size : None,
                line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Bold,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Italic, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.bold.to_owned().to_string(), size :
                None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Bold,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.italic.to_owned().to_string(), size
                : None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Normal,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Italic, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.plain.to_owned().to_string(), size
                : None, line_height : None, font : None, color : None, link : None,
                background : None, border : None, padding : None, underline : false,
                strikethrough : false, }); } wire::Node::RichText { options :
                wire::TextOptions { wrapping : Some(wire::Wrapping::WordOrGlyph),
                ..Default::default() }, key : format!("{}/@text:32",
                rich_line_scope_780), size : Some(13.5f32), color : None, font :
                wire::Font { monospace : false, weight : wire::Weight::Normal, }, width :
                Some(wire::Length::Fill), align_x : None, spans : rich_spans, on_link :
                Some(::ducktape_view_guest::slots::handler:: < String, Message, >
                (Box::new({ let route = { let route_callback = lazy_event_410_5.clone();
                move | link : String | route_callback(link) }; move | sent : String |
                Some(route(sent)) }),),), } } }); } if ! block.rich { children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:157",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),); } } }
                native::spaced(native::sized(native::column(format!("{}/@layout:75",
                rich_body_scope_688), children,), Some(wire::Length::Fill), None,),
                5.0f32,) } }]; wire::Node::Linear { max_width : Some(760.0f32), clip :
                false, key : format!("{}/@layout:60", message_body_scope_1809), wrap :
                None, axis : wire::Axis::Column, spacing : None, padding : None, width :
                Some(wire::Length::Fill), height : None, align : None, background : None,
                border : None, children : children, } } }), }); if cached_message.edited
                && ! cached_message.show_author { children
                .push(native::text_options(native::text(format!("{}/@text:408",
                message_contents_scope_2035), "· edited".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } if ! crate
                ::host::run_of_message(::std::convert::AsRef::as_ref(& cached_message
                .id),).is_empty() { children.push({ let children : Vec < wire::Node >
                = vec![native::padded(native::button(format!("{}/@button:420",
                message_contents_scope_2035), String::from("View run"),
                Some(::ducktape_view_guest::slots::message(lazy_event_410_6(crate
                ::host::run_of_message(::std::convert::AsRef::as_ref(& cached_message
                .id),),),),), wire::ButtonPreset::Secondary,),
                wire::Edges::all(3.0f32),)];
                native::padded(native::sized(native::row(format!("{}/@layout:419",
                message_contents_scope_2035), children,), Some(wire::Length::Fill),
                None,), wire::Edges { top : 4.0f32, right : 0.0f32, bottom : 0.0f32, left
                : 0.0f32, },) }); } if ! cached_message.reactions.is_empty() { children
                .push({ let mut items = Vec::new(); for (index, reaction) in
                cached_message.reactions.iter().enumerate() { let for_scope =
                format!("{}/@for:1848({})", message_contents_scope_2035, index); let
                flex_child : wire::Node = { let reaction_chip_scope_1849 =
                format!("{}/ReactionChip@1849", for_scope); { let node_scope =
                format!("{}/root", reaction_chip_scope_1849); { let mut children : Vec <
                wire::Node > = Vec::new(); if reaction.reacted_by_me { children
                .push(wire::Node::Button { checked : Some(reaction.reacted_by_me),
                expanded : None, description : Some(String::from(reaction.emoji
                .to_owned())), key : format!("{}/@button:218", reaction_chip_scope_1849),
                content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:225",
                reaction_chip_scope_1849), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:232",
                reaction_chip_scope_1849), reaction.emoji.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:238",
                reaction_chip_scope_1849), reaction.count.to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:231", reaction_chip_scope_1849), wrap :
                None, axis : wire::Axis::Row, spacing : Some(4.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 1.0f32, right : 8.0f32, bottom : 1.0f32, left : 6.0f32, },),),),
                label : Some(String::from("Remove reaction".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_410_1(cached_message
                .seq, reaction.emoji.to_owned(),),),), width : None, height : None,
                padding : Some(wire::Edges::all(0.0f32)), style :
                wire::ButtonStyle::default(), }); } if ! reaction.reacted_by_me {
                children.push(wire::Node::Button { checked : Some(reaction
                .reacted_by_me), expanded : None, description :
                Some(String::from(reaction.emoji.to_owned())), key :
                format!("{}/@button:262", reaction_chip_scope_1849), content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:269",
                reaction_chip_scope_1849), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:276",
                reaction_chip_scope_1849), reaction.emoji.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:282",
                reaction_chip_scope_1849), reaction.count.to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:275", reaction_chip_scope_1849), wrap :
                None, axis : wire::Axis::Row, spacing : Some(4.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 1.0f32, right : 8.0f32, bottom : 1.0f32, left : 6.0f32, },),),),
                label : Some(String::from("Add reaction".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_410_0(cached_message
                .seq, reaction.emoji.to_owned(),),),), width : None, height : None,
                padding : Some(wire::Edges::all(0.0f32)), style :
                wire::ButtonStyle::default(), }); } native::column(node_scope.clone(),
                children) } } }; items.push((wire::FlexItem::default(), flex_child)); }
                let (items, children) = items.into_iter().unzip(); wire::Node::Flex { key
                : format!("{}/@layout:427", message_contents_scope_2035), items,
                children, background : None, border : None, layout : wire::FlexLayout {
                direction : wire::FlexDirection::Row, wrap : wire::FlexWrap::Wrap,
                justify : None, items : Some(wire::FlexItemAlignment::Start), content :
                None, row_gap : Some(5.0f32), column_gap : Some(5.0f32), padding :
                Some(wire::Edges { top : 6.0f32, right : 0.0f32, bottom : 0.0f32, left :
                0.0f32, }), width : Some(wire::Length::Fill), height : None, max_width :
                None, max_height : None, clip : false, surface_width : None,
                surface_height : None, surface_max_width : None, }, } }); } if
                cached_message.reply_count > 0 { children.push({ let children : Vec <
                wire::Node > = vec![wire::Node::Button { checked : None, expanded : None,
                description : None, key : format!("{}/@button:447",
                message_contents_scope_2035), content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:452",
                message_contents_scope_2035), { let children : Vec < wire::Node > =
                vec![{ let component_icon_scope_1872 = format!("{}/Icon@1872",
                message_contents_scope_2035); { let node_scope = format!("{}/root",
                component_icon_scope_1872); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "nav-chat"),),);
                wire::Node::Svg { inherit_button_ink : false, key :
                format!("{}/@media:52", component_icon_scope_1872), hash : hash, bytes :
                bytes, label : None, color : None, hover : None, fit : None, rotation :
                None, opacity : None, width : Some(wire::Length::Fixed(12.0f32)), height
                : Some(wire::Length::Fixed(12.0f32)), } }); } native::column(node_scope
                .clone(), children) } } },
                native::text_options(native::text(format!("{}/@text:464",
                message_contents_scope_2035), crate ::host::plural(cached_message
                .reply_count, ::std::convert::AsRef::as_ref(& "reply"),
                ::std::convert::AsRef::as_ref(& "replies"),).to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:458", message_contents_scope_2035), wrap
                : None, axis : wire::Axis::Row, spacing : Some(6.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 3.0f32, right : 9.0f32, bottom : 3.0f32, left : 7.0f32, },),),),
                label : Some(String::from("Open thread".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_410_2(cached_message
                .seq),),), width : None, height : None, padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), }];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:441", message_contents_scope_2035), wrap : None, axis
                : wire::Axis::Row, spacing : Some(6.0f32), padding : Some(wire::Edges {
                top : 6.0f32, right : 0.0f32, bottom : 0.0f32, left : 0.0f32, }), width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }); }
                native::spaced(native::sized(native::column(format!("{}/@layout:340",
                message_contents_scope_2035), children,), Some(wire::Length::Fill),
                None,), 2.0f32,) }); if cached_message.pending { children
                .push(native::padded(native::container(format!("{}/@container:506",
                message_contents_scope_2035), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:508",
                message_contents_scope_2035), cached_message.meta.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },), { let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "dot")),); wire::Node::Svg {
                inherit_button_ink : false, key : format!("{}/@media:514",
                message_contents_scope_2035), hash : hash, bytes : bytes, label : None,
                color : None, hover : None, fit : None, rotation : None, opacity :
                Some(1.0f32), width : Some(wire::Length::Fixed(6.0f32)), height :
                Some(wire::Length::Fixed(6.0f32)), } }]; wire::Node::Linear { max_width :
                None, clip : false, key : format!("{}/@layout:507",
                message_contents_scope_2035), wrap : None, axis : wire::Axis::Row,
                spacing : Some(5.0f32), padding : None, width : None, height : None,
                align : Some(wire::AlignX::Center), background : None, border : None,
                children : children, } },), wire::Edges { top : 0.0f32, right : 7.0f32,
                bottom : 0.0f32, left : 0.0f32, },),); } wire::Node::Linear { max_width :
                None, clip : false, key : format!("{}/@layout:331",
                message_contents_scope_2035), wrap : None, axis : wire::Axis::Row,
                spacing : Some(11.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Left), background : None, border : None, children :
                children, } }]; native::sized(native::column(format!("{}/@layout:330",
                message_contents_scope_2035), children,), Some(wire::Length::Fill),
                None,) } },), Some(wire::Length::Fill), None,), wire::Edges { top :
                4.0f32, right : 7.0f32, bottom : 4.0f32, left : 7.0f32, },),); } }
                wire::Node::Stack { key : format!("{}/@layout:549",
                message_card_scope_2701), width : Some(wire::Length::Fill), height :
                None, padding : None, background : None, border : None, clip : false,
                under : 0u32, children : children, } }, { let mut children : Vec <
                wire::Node > = Vec::new(); if ! cached_message.deleted && !
                cached_message.pending { children.push(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:632", message_card_scope_2701), width :
                Some(wire::Length::Fill), height : None, padding : Some(wire::Edges { top
                : 0.0f32, right : 8.0f32, bottom : 0.0f32, left : 0.0f32, }), align_x :
                Some(wire::AlignX::Right), align_y : Some(wire::AlignY::Top), background
                : None.map(wire::Background::Color), border : None, snap : None, content
                : Box::new(native::padded(native::container(format!("{}/@container:640",
                message_card_scope_2701), { let children : Vec < wire::Node > =
                vec![wire::Node::Button { checked : None, expanded : None, description :
                None, key : format!("{}/@button:661", message_card_scope_2701), content :
                wire::ButtonContent::Child(Box::new(native::text(format!("{}/@text:668",
                message_card_scope_2701), "👍".to_owned().to_string(),),),), label :
                Some(String::from("React with 👍".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_410_0(cached_message
                .seq, "👍".to_owned()),),), width : Some(wire::Length::Fixed(27.0f32)),
                height : Some(wire::Length::Fixed(25.0f32)), padding :
                Some(wire::Edges::all(4.0f32)), style : wire::ButtonStyle::default(), },
                wire::Node::Button { checked : None, expanded : None, description : None,
                key : format!("{}/@button:672", message_card_scope_2701), content :
                wire::ButtonContent::Child(Box::new(native::text(format!("{}/@text:679",
                message_card_scope_2701), "✅".to_owned().to_string(),),),), label :
                Some(String::from("React with ✅".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_410_0(cached_message
                .seq, "✅".to_owned()),),), width : Some(wire::Length::Fixed(27.0f32)),
                height : Some(wire::Length::Fixed(25.0f32)), padding :
                Some(wire::Edges::all(4.0f32)), style : wire::ButtonStyle::default(), },
                wire::Node::Button { checked : None, expanded : None, description : None,
                key : format!("{}/@button:683", message_card_scope_2701), content :
                wire::ButtonContent::Child(Box::new(native::text(format!("{}/@text:690",
                message_card_scope_2701), "👀".to_owned().to_string(),),),), label :
                Some(String::from("React with 👀".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_410_0(cached_message
                .seq, "👀".to_owned()),),), width : Some(wire::Length::Fixed(27.0f32)),
                height : Some(wire::Length::Fixed(25.0f32)), padding :
                Some(wire::Edges::all(4.0f32)), style : wire::ButtonStyle::default(), },
                native::sized(native::container(format!("{}/@container:694",
                message_card_scope_2701), wire::Node::Space { width :
                Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(1.0f32)), },),
                Some(wire::Length::Fixed(1.0f32)), Some(wire::Length::Fixed(16.0f32)),),
                wire::Node::Button { checked : None, expanded : None, description : None,
                key : format!("{}/@button:700", message_card_scope_2701), content :
                wire::ButtonContent::Child(Box::new(native::text(format!("{}/@text:707",
                message_card_scope_2701), "♡".to_owned().to_string(),),),), label :
                Some(String::from("Manage reactions".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_410_3(cached_message
                .seq, cached_message.body.to_owned(), cached_message.rev,),),), width :
                Some(wire::Length::Fixed(27.0f32)), height :
                Some(wire::Length::Fixed(25.0f32)), padding :
                Some(wire::Edges::all(4.0f32)), style : wire::ButtonStyle::default(), },
                wire::Node::Button { checked : None, expanded : None, description : None,
                key : format!("{}/@button:711", message_card_scope_2701), content :
                wire::ButtonContent::Child(Box::new({ let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "nav-chat"),),);
                wire::Node::Svg { inherit_button_ink : true, key :
                format!("{}/@media:720", message_card_scope_2701), hash : hash, bytes :
                bytes, label : None, color : None, hover : None, fit : None, rotation :
                None, opacity : None, width : Some(wire::Length::Fixed(15.0f32)), height
                : Some(wire::Length::Fixed(15.0f32)), } }),), label :
                Some(String::from("Open thread".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_410_2(cached_message
                .seq),),), width : None, height : None, padding :
                Some(wire::Edges::all(5.0f32)), style : wire::ButtonStyle::default(), },
                wire::Node::Button { checked : None, expanded : None, description : None,
                key : format!("{}/@button:728", message_card_scope_2701), content :
                wire::ButtonContent::Child(Box::new(native::text(format!("{}/@text:735",
                message_card_scope_2701), "⋯".to_owned().to_string(),),),), label :
                Some(String::from("More message actions".to_owned()),), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_410_4(cached_message
                .seq, cached_message.body.to_owned(), cached_message.rev,),),), width :
                Some(wire::Length::Fixed(27.0f32)), height :
                Some(wire::Length::Fixed(25.0f32)), padding :
                Some(wire::Edges::all(4.0f32)), style : wire::ButtonStyle::default(), }];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:660", message_card_scope_2701), wrap : None, axis :
                wire::Axis::Row, spacing : Some(1.0f32), padding : None, width : None,
                height : None, align : Some(wire::AlignX::Center), background : None,
                border : None, children : children, } },), wire::Edges { top : 2.0f32,
                right : 2.0f32, bottom : 2.0f32, left : 2.0f32, },),), }); } if
                cached_message.deleted || cached_message.pending { children
                .push(wire::Node::Space { width : Some(wire::Length::Fixed(1.0f32)),
                height : Some(wire::Length::Fixed(1.0f32)), }); }
                native::sized(native::column(format!("{}/@layout:630",
                message_card_scope_2701), children,), Some(wire::Length::Fill), None,)
                }]; wire::Node::Hover { key : format!("{}/@layout:544",
                message_card_scope_2701), width : None, height : None, padding : None,
                background : None, border : None, tint : None, radius : 9.0f32, open :
                false, children : children, } });
                native::sized(native::column(format!("{}/@layout:530",
                message_card_scope_2701), children,), Some(wire::Length::Fill), None,) }
                }]; wire::Node::Stack { key : node_scope.clone(), width :
                Some(wire::Length::Fill), height : None, padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, } }
                } }, 410u64, & key_recon, lazy_key,) } }); } for (index, live) in
                cached_timeline.live_agents.iter().enumerate() { let for_scope =
                format!("{}/@for:2718({})", key_recon, index); if crate
                ::host::run_in_thread(::std::borrow::Borrow::borrow(& live), message
                .seq,) { children.push(wire::Node::Button { checked : None, expanded :
                None, description : None, key : format!("{}/@button:117", for_scope),
                content :
                wire::ButtonContent::Child(Box::new(native::text(format!("{}/@text:122",
                for_scope), crate
                ::host::live_thread_label(::std::convert::AsRef::as_ref(& live.agent),)
                .to_string(),),),), label : Some(String::from(crate
                ::host::live_thread_label(::std::convert::AsRef::as_ref(& live
                .agent),),),), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_543_22(message
                .seq),),), width : None, height : None, padding :
                Some(wire::Edges::all(4.0f32)), style : wire::ButtonStyle::default(), });
                } } native::spaced(native::sized(native::column(format!("{}/@layout:41",
                key_recon), children,), Some(wire::Length::Fill), None,), 0.0f32,) };
                children.push((key, child)); } let (keys, children) = children
                .into_iter().map(| (key, child) | (wire::ListKey::from(key), child))
                .unzip(); wire::Node::KeyedColumn { key : format!("{}/@keyed:36",
                message_timeline_scope_3465), keys : Some(keys), children, background :
                None, border : None, spacing : Some(3.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, max_width : None, align : None,
                virtual_row : Some(44.0f32), } } } }, 543u64, & use_scope, lazy_key,) }
                });
                native::spaced(native::padded(native::sized(native::column(format!("{}/@layout:769",
                use_scope), children,), Some(wire::Length::Fill), None,), wire::Edges {
                top : 0.0f32, right : 6.0f32, bottom : 0.0f32, left : 0.0f32, },),
                3.0f32,) }), } }), }), }, { let mut children =
                vec![::ducktape_view_guest::wire::Node::Space { width :
                Some(::ducktape_view_guest::wire::Length::Fill), height :
                Some(::ducktape_view_guest::wire::Length::Fill) }]; if self
                .selected_message_seq > 0 && self.message_action !=
                MessageAction::Toolbar { children.push({ let mut children : Vec <
                wire::Node > = vec![wire::Node::MouseArea { key :
                format!("{}/@mouse:901", use_scope), on_press :
                Some(::ducktape_view_guest::slots::message(Message::ClearMessageSelection,),),
                on_release : None, on_double_click : None, on_right_press : None,
                on_right_release : None, on_middle_press : None, on_middle_release :
                None, on_enter : None, on_exit : None, on_move : None, on_press_at :
                None, on_scroll : None, content : Box::new(wire::Node::Space { width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fixed(crate
                ::host::block_action_menu_y(self.chat_screen_states.get(& use_scope)
                .map_or_else(| | self.chat_screen_initial.chat_pointer_y.clone(), | state
                | state.chat_pointer_y.clone(),), self.chat_screen_states.get(&
                use_scope).map_or_else(| | self.chat_screen_initial.chat_height.clone(),
                | state | state.chat_height.clone(),),) as f32,),), }), }]; if self
                .message_action == MessageAction::More { children.push({ let children
                : Vec < wire::Node > = vec![{ let node_scope =
                format!("{}/message-action-focus", use_scope); wire::Node::Input {
                options : wire::InputOptions { label : "Message action focus".to_owned()
                .to_string(), description : None, disabled : false, padding :
                Some(wire::Edges::all(0.0f32)), text_size : Some(1.0f32), line_height :
                Some(1.0f32), align : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Normal,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                }, key : node_scope.clone(), placeholder : String::from(""), value : self
                .chat_screen_states.get(& use_scope).map_or_else(| | self
                .chat_screen_initial.message_action_focus.clone(), | state | state
                .message_action_focus.clone(),).to_string(), on_input :
                ::ducktape_view_guest::slots::handler:: < String, Message, > (Box::new({
                let route = { let scope = use_scope.clone(); move | value |
                Message::ChatScreenMessageActionFocusChanged(scope.clone(), value,) };
                move | sent : String | Some(route(sent)) }),), on_submit : None, width :
                Some(wire::Length::Fixed(1.0f32)), secure : false, style :
                Default::default(), } },
                native::padded(native::sized(native::container(format!("{}/@container:918",
                use_scope), { let children : Vec < wire::Node > =
                vec![wire::Node::Button { checked : None, expanded : None, description :
                None, key : format!("{}/@button:937", use_scope), content :
                wire::ButtonContent::Child(Box::new(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:944", use_scope), width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), padding :
                Some(wire::Edges { top : 0.0f32, right : 9.0f32, bottom : 0.0f32, left :
                9.0f32, }), align_x : None, align_y : Some(wire::AlignY::Center),
                background : None.map(wire::Background::Color), border : None, snap :
                None, content : Box::new({ let children : Vec < wire::Node > =
                vec![self.icon(format!("{}/Icon@3559", use_scope), "emoji", 14f32,
                "@media:82",), native::text_options(native::text(format!("{}/@text:961",
                use_scope), "Add reaction".to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },)];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:951", use_scope), wrap : None, axis :
                wire::Axis::Row, spacing : Some(9.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }), }),), label : Some(String::from("Manage reactions"
                .to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(Message::OpenMessageReactions(self
                .selected_message_seq, self.message_edit_draft.to_owned(), self
                .selected_message_rev,),),), width : Some(wire::Length::Fill), height :
                Some(wire::Length::Fixed(30.0f32)), padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), },
                wire::Node::Button { checked : None, expanded : None, description : None,
                key : format!("{}/@button:969", use_scope), content :
                wire::ButtonContent::Child(Box::new(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:976", use_scope), width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), padding :
                Some(wire::Edges { top : 0.0f32, right : 9.0f32, bottom : 0.0f32, left :
                9.0f32, }), align_x : None, align_y : Some(wire::AlignY::Center),
                background : None.map(wire::Background::Color), border : None, snap :
                None, content : Box::new({ let children : Vec < wire::Node > =
                vec![self.icon(format!("{}/Icon@3591", use_scope), "nav-chat", 14f32,
                "@media:82",), native::text_options(native::text(format!("{}/@text:993",
                use_scope), "Reply in thread".to_owned().to_string(),), wire::TextOptions
                { wrapping : Some(wire::Wrapping::None), ..Default::default() },)];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:983", use_scope), wrap : None, axis :
                wire::Axis::Row, spacing : Some(9.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }), }),), label : Some(String::from("Reply in thread"
                .to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(Message::OpenThreadFor(self
                .selected_message_seq),),), width : Some(wire::Length::Fill), height :
                Some(wire::Length::Fixed(30.0f32)), padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), },
                wire::Node::Button { checked : None, expanded : None, description : None,
                key : format!("{}/@button:1009", use_scope), content :
                wire::ButtonContent::Child(Box::new(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:1016", use_scope), width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), padding :
                Some(wire::Edges { top : 0.0f32, right : 9.0f32, bottom : 0.0f32, left :
                9.0f32, }), align_x : None, align_y : Some(wire::AlignY::Center),
                background : None.map(wire::Background::Color), border : None, snap :
                None, content : Box::new({ let children : Vec < wire::Node > =
                vec![self.icon(format!("{}/Icon@3631", use_scope), "link", 14f32,
                "@media:82",), native::text_options(native::text(format!("{}/@text:1033",
                use_scope), "Copy link".to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },)];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:1023", use_scope), wrap : None, axis :
                wire::Axis::Row, spacing : Some(9.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }), }),), label : Some(String::from("Copy message link"
                .to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(Message::CopyMessageLink(crate
                ::host::duck_channel_message_link(self.active_channel.to_owned(), self
                .selected_message_seq, self.network_chain_id.to_owned(),),),),), width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fixed(30.0f32)),
                padding : Some(wire::Edges::all(0.0f32)), style :
                wire::ButtonStyle::default(), }, wire::Node::Button { checked : None,
                expanded : None, description : None, key : format!("{}/@button:1041",
                use_scope), content :
                wire::ButtonContent::Child(Box::new(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:1048", use_scope), width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), padding :
                Some(wire::Edges { top : 0.0f32, right : 9.0f32, bottom : 0.0f32, left :
                9.0f32, }), align_x : None, align_y : Some(wire::AlignY::Center),
                background : None.map(wire::Background::Color), border : None, snap :
                None, content : Box::new({ let children : Vec < wire::Node > =
                vec![self.icon(format!("{}/Icon@3663", use_scope), "pencil", 14f32,
                "@media:82",), native::text_options(native::text(format!("{}/@text:1065",
                use_scope), "Edit message".to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },)];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:1055", use_scope), wrap : None, axis :
                wire::Axis::Row, spacing : Some(9.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }), }),), label : Some(String::from("Edit message"
                .to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(Message::BeginMessageEdit(self
                .selected_message_seq, self.message_edit_draft.to_owned(), self
                .selected_message_rev,),),), width : Some(wire::Length::Fill), height :
                Some(wire::Length::Fixed(30.0f32)), padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), },
                native::sized(native::container(format!("{}/@container:1073", use_scope),
                wire::Node::Space { width : Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(1.0f32)), },), Some(wire::Length::Fill),
                Some(wire::Length::Fixed(1.0f32)),), wire::Node::Button { checked : None,
                expanded : None, description : None, key : format!("{}/@button:1079",
                use_scope), content :
                wire::ButtonContent::Child(Box::new(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:1086", use_scope), width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), padding :
                Some(wire::Edges { top : 0.0f32, right : 9.0f32, bottom : 0.0f32, left :
                9.0f32, }), align_x : None, align_y : Some(wire::AlignY::Center),
                background : None.map(wire::Background::Color), border : None, snap :
                None, content : Box::new({ let children : Vec < wire::Node > =
                vec![self.icon(format!("{}/Icon@3701", use_scope), "trash", 14f32,
                "@media:70",), native::text_options(native::text(format!("{}/@text:1103",
                use_scope), "Delete message…".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:1093", use_scope), wrap : None, axis :
                wire::Axis::Row, spacing : Some(9.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }), }),), label : Some(String::from("Delete message"
                .to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(Message::ArmMessageDelete(self
                .selected_message_seq, self.message_edit_draft.to_owned(), self
                .selected_message_rev,),),), width : Some(wire::Length::Fill), height :
                Some(wire::Length::Fixed(30.0f32)), padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), }];
                native::spaced(native::sized(native::column(format!("{}/@layout:929",
                use_scope), children,), Some(wire::Length::Fill), None,), 1.0f32,) },),
                Some(wire::Length::Fixed(200.0f32)), None,), wire::Edges { top : 5.0f32,
                right : 5.0f32, bottom : 5.0f32, left : 5.0f32, },)]; wire::Node::Stack {
                key : format!("{}/@layout:904", use_scope), width : None, height : None,
                padding : None, background : None, border : None, clip : false, under :
                0u32, children : children, } }); } if self.message_action ==
                MessageAction::Reactions { children.push({ let children : Vec <
                wire::Node > = vec![{ let node_scope =
                format!("{}/message-reaction-focus", use_scope); wire::Node::Input {
                options : wire::InputOptions { label : "Message reaction focus"
                .to_owned().to_string(), description : None, disabled : false, padding :
                Some(wire::Edges::all(0.0f32)), text_size : Some(1.0f32), line_height :
                Some(1.0f32), align : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Normal,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                }, key : node_scope.clone(), placeholder : String::from(""), value : self
                .chat_screen_states.get(& use_scope).map_or_else(| | self
                .chat_screen_initial.message_action_focus.clone(), | state | state
                .message_action_focus.clone(),).to_string(), on_input :
                ::ducktape_view_guest::slots::handler:: < String, Message, > (Box::new({
                let route = { let scope = use_scope.clone(); move | value |
                Message::ChatScreenMessageActionFocusChanged(scope.clone(), value,) };
                move | sent : String | Some(route(sent)) }),), on_submit : None, width :
                Some(wire::Length::Fixed(1.0f32)), secure : false, style :
                Default::default(), } },
                native::padded(native::container(format!("{}/@container:1126",
                use_scope), { let mut items = Vec::new(); for (index, emoji) in crate
                ::host::reaction_palette().iter().enumerate() { let for_scope =
                format!("{}/@for:3746({})", use_scope, index); let flex_child :
                wire::Node = wire::Node::Button { checked : None, expanded : None,
                description : Some(String::from(emoji.to_owned())), key :
                format!("{}/@button:1144", for_scope), content :
                wire::ButtonContent::Child(Box::new(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:1153", for_scope), width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), padding :
                None, align_x : Some(wire::AlignX::Center), align_y :
                Some(wire::AlignY::Center), background : None
                .map(wire::Background::Color), border : None, snap : None, content :
                Box::new(native::text_options(native::text(format!("{}/@text:1159",
                for_scope), emoji.to_owned().to_string(),), wire::TextOptions { wrapping
                : Some(wire::Wrapping::None), ..Default::default() },),), }),), label :
                Some(String::from("Add reaction".to_owned())), on_press : if self
                .active_channel_archived { None } else {
                Some(::ducktape_view_guest::slots::message(Message::AddReactionSubmit(emoji
                .to_owned()),),) }, width : Some(wire::Length::Fixed(27.0f32)), height :
                Some(wire::Length::Fixed(27.0f32)), padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), };
                items.push((wire::FlexItem::default(), flex_child)); } let (items,
                children) = items.into_iter().unzip(); wire::Node::Flex { key :
                format!("{}/@layout:1136", use_scope), items, children, background :
                None, border : None, layout : wire::FlexLayout { direction :
                wire::FlexDirection::Row, wrap : wire::FlexWrap::Wrap, justify : None,
                items : Some(wire::FlexItemAlignment::Start), content : None, row_gap :
                Some(2.0f32), column_gap : Some(2.0f32), padding : None, width :
                Some(wire::Length::Fixed(234.0f32)), height : None, max_width : None,
                max_height : None, clip : false, surface_width : None, surface_height :
                None, surface_max_width : None, }, } },), wire::Edges { top : 8.0f32,
                right : 8.0f32, bottom : 8.0f32, left : 8.0f32, },)]; wire::Node::Stack {
                key : format!("{}/@layout:1112", use_scope), width : None, height : None,
                padding : None, background : None, border : None, clip : false, under :
                0u32, children : children, } }); } if self.message_action ==
                MessageAction::Editing { children
                .push(native::padded(native::sized(native::container(format!("{}/@container:1172",
                use_scope), { let children : Vec < wire::Node > = vec![{ let
                node_scope = format!("{}/message-edit", use_scope); wire::Node::Surface {
                key : node_scope.clone(), name : String::from("chat_composer"), args :
                ::std::vec![{ let surface_arg = & (crate
                ::host::edit_scope(::std::convert::AsRef::as_ref(& (self.endpoint)),
                ::std::convert::AsRef::as_ref(& (self.active_channel)), self
                .selected_message_seq));
                ::ducktape_view_guest::wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                }, { let surface_arg = & ("edit".to_owned());
                ::ducktape_view_guest::wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                }, { let surface_arg = & (true);
                ::ducktape_view_guest::wire::SurfaceValue::Bool(* (surface_arg)) }, { let
                surface_arg = & ("Edit message".to_owned());
                ::ducktape_view_guest::wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                }, { let surface_arg = & (self.busy);
                ::ducktape_view_guest::wire::SurfaceValue::Bool(* (surface_arg)) }, { let
                surface_arg = & (false);
                ::ducktape_view_guest::wire::SurfaceValue::Bool(* (surface_arg)) }, { let
                surface_arg = & ("Could not save changes".to_owned());
                ::ducktape_view_guest::wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                }], on_event : None, } }, wire::Node::Button { checked : None, expanded :
                None, description : None, key : format!("{}/@button:1189", use_scope),
                content : wire::ButtonContent::Child(Box::new(wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : format!("{}/@container:1197", use_scope), width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), padding :
                None, align_x : Some(wire::AlignX::Center), align_y :
                Some(wire::AlignY::Center), background : None
                .map(wire::Background::Color), border : None, snap : None, content :
                Box::new(native::text(format!("{}/@text:1203", use_scope), "×"
                .to_owned().to_string(),),), }),), label :
                Some(String::from("Cancel message edit".to_owned())), on_press : if self
                .busy { None } else {
                Some(::ducktape_view_guest::slots::message(Message::ClearMessageSelection,),)
                }, width : Some(wire::Length::Fixed(28.0f32)), height :
                Some(wire::Length::Fixed(28.0f32)), padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), }];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:1183", use_scope), wrap : None, axis :
                wire::Axis::Row, spacing : Some(4.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } },), Some(wire::Length::Fill), None,), wire::Edges { top :
                3.0f32, right : 3.0f32, bottom : 3.0f32, left : 3.0f32, },),); } if self
                .message_action == MessageAction::Delete { children.push({ let children : Vec < wire::Node > = vec![{ let node_scope =
                format!("{}/message-delete-focus", use_scope); wire::Node::Input {
                options : wire::InputOptions { label : "Message delete focus".to_owned()
                .to_string(), description : None, disabled : false, padding :
                Some(wire::Edges::all(0.0f32)), text_size : Some(1.0f32), line_height :
                Some(1.0f32), align : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Normal,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                }, key : node_scope.clone(), placeholder : String::from(""), value : self
                .chat_screen_states.get(& use_scope).map_or_else(| | self
                .chat_screen_initial.message_action_focus.clone(), | state | state
                .message_action_focus.clone(),).to_string(), on_input :
                ::ducktape_view_guest::slots::handler:: < String, Message, > (Box::new({
                let route = { let scope = use_scope.clone(); move | value |
                Message::ChatScreenMessageActionFocusChanged(scope.clone(), value,) };
                move | sent : String | Some(route(sent)) }),), on_submit : None, width :
                Some(wire::Length::Fixed(1.0f32)), secure : false, style :
                Default::default(), } },
                native::padded(native::container(format!("{}/@container:1218",
                use_scope), { let children : Vec < wire::Node > =
                vec![native::text(format!("{}/@text:1229", use_scope),
                "Delete this message?".to_owned().to_string(),),
                native::padded(native::button(format!("{}/@button:1230", use_scope),
                String::from("Delete"), if self.busy { None } else {
                Some(::ducktape_view_guest::slots::message(Message::DeleteMessageSubmit,),)
                }, wire::ButtonPreset::Secondary,), wire::Edges::all(5.0f32),),
                native::padded(native::button(format!("{}/@button:1235", use_scope),
                String::from("Cancel"), if self.busy { None } else {
                Some(::ducktape_view_guest::slots::message(Message::ClearMessageSelection,),)
                }, wire::ButtonPreset::Secondary,), wire::Edges::all(5.0f32),)];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:1228", use_scope), wrap : None, axis :
                wire::Axis::Row, spacing : Some(5.0f32), padding : None, width : None,
                height : None, align : Some(wire::AlignX::Center), background : None,
                border : None, children : children, } },), wire::Edges { top : 3.0f32,
                right : 3.0f32, bottom : 3.0f32, left : 3.0f32, },)]; wire::Node::Stack {
                key : format!("{}/@layout:1208", use_scope), width : None, height : None,
                padding : None, background : None, border : None, clip : false, under :
                0u32, children : children, } }); }
                native::column(format!("{}/@layout:900", use_scope), children,) }); }
                wire::Node::Overlay { key : format!("{}/@overlay:881", use_scope),
                padding : 8.0f32, backdrop : wire::Rgba([0.0 / 255.0, 0.0 / 255.0, 0.0 /
                255.0, 0.000000,]), align_x : wire::AlignX::Right, align_y :
                wire::AlignY::Top, on_dismiss :
                Some(::ducktape_view_guest::slots::message(Message::ClearMessageSelection,),),
                children : children, } }]; wire::Node::Stack { key :
                format!("{}/@layout:730", use_scope), width : Some(wire::Length::Fill),
                height : Some(wire::Length::Fill), padding : None, background : None,
                border : None, clip : false, under : 0u32, children : children, } }); }
                native::spaced(native::padded(native::sized(native::column(format!("{}/@layout:690",
                use_scope), children,), Some(wire::Length::Fill),
                Some(wire::Length::Fill),), wire::Edges { top : 16.0f32, right : 12.0f32,
                bottom : 8.0f32, left : 18.0f32, },), 9.0f32,) }]; if ! self.messages
                .is_empty() && (self.history_view || ! self.at_live_tail) { children
                .push(wire::Node::Container { shadow : Default::default(), max_width :
                None, max_height : None, clip : false, key :
                format!("{}/@container:1257", use_scope), width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), padding :
                Some(wire::Edges { top : 0.0f32, right : 18.0f32, bottom : 10.0f32, left
                : 18.0f32, }), align_x : Some(wire::AlignX::Center), align_y :
                Some(wire::AlignY::Bottom), background : None
                .map(wire::Background::Color), border : None, snap : None, content :
                Box::new(native::padded(native::button(format!("{}/@button:1266",
                use_scope), String::from("↓  Jump to latest"),
                Some(::ducktape_view_guest::slots::message(Message::ChooseChannel(self
                .active_channel.to_owned()),),), wire::ButtonPreset::Secondary,),
                wire::Edges { top : 5f32, right : 10f32, bottom : 5f32, left : 10f32,
                },),), }); } if self.search_phase == SearchPhase::Searching || ! self
                .search_hits.is_empty() || crate
                ::host::search_answer_stands(::std::convert::AsRef::as_ref(& self
                .search_query), ::std::convert::AsRef::as_ref(& self.search_draft), self
                .search_phase == SearchPhase::Searching,) { children
                .push(wire::Node::Container { shadow : Default::default(), max_width :
                None, max_height : None, clip : false, key :
                format!("{}/@container:1292", use_scope), width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), padding :
                Some(wire::Edges { top : 16.0f32, right : 18.0f32, bottom : 0.0f32, left
                : 18.0f32, }), align_x : None, align_y : Some(wire::AlignY::Top),
                background : None.map(wire::Background::Color), border : None, snap :
                None, content : Box::new(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : Some(260.0f32), clip :
                false, key : format!("{}/@container:1300", use_scope), width :
                Some(wire::Length::Fill), height : None, padding : Some(wire::Edges { top
                : 6.0f32, right : 6.0f32, bottom : 6.0f32, left : 6.0f32, }), align_x :
                None, align_y : None, background : None, border : None, snap : None,
                content : Box::new({ let mut children : Vec < wire::Node > = Vec::new();
                if self.search_phase == SearchPhase::Searching { children.push({ let children : Vec < wire::Node > = vec![self
                .loading_messages(format!("{}/SkeletonRow@3922", use_scope),)];
                native::spaced(native::padded(native::sized(native::column(format!("{}/@layout:1314",
                use_scope), children,), Some(wire::Length::Fill), None,), wire::Edges {
                top : 8.0f32, right : 8.0f32, bottom : 8.0f32, left : 8.0f32, },),
                14.0f32,) }); } if self.search_phase == SearchPhase::Done && self
                .search_hits.is_empty() { children.push(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:1321", use_scope), width :
                Some(wire::Length::Fill), height : None, padding : Some(wire::Edges { top
                : 14.0f32, right : 14.0f32, bottom : 14.0f32, left : 14.0f32, }), align_x
                : Some(wire::AlignX::Center), align_y : None, background : None
                .map(wire::Background::Color), border : None, snap : None, content :
                Box::new(native::text(format!("{}/@text:1326", use_scope),
                "No messages match".to_owned().to_string(),),), }); } if self
                .search_phase == SearchPhase::Done && ! self.search_hits.is_empty() {
                children.push(wire::Node::Scroll { on_scroll : None, virtual_rows :
                false, key : format!("{}/@layout:1328", use_scope), direction :
                wire::ScrollDirection::Vertical, width : Some(wire::Length::Fill), height
                : Some(wire::Length::Shrink), bar_hidden : false, bar_width : None,
                bar_margin : None, scroller_width : None, bar_spacing : None, anchor_x :
                wire::ScrollAnchor::Start, anchor_y : wire::ScrollAnchor::Start,
                auto_scroll : false, background : None, border : None, content :
                Box::new({ let mut children : Vec < wire::Node > = Vec::new(); for
                (index, hit) in self.search_hits.iter().enumerate() { let for_scope =
                format!("{}/@for:3937({})", use_scope, index); children.push(self
                .search_result(format!("{}/ChatSearchResult@3938",
                for_scope), (move | event_0, event_1, event_2 |
                Message::OpenChatSearchHit(event_0, event_1, event_2,)).clone(), hit
                .clone(),),); }
                native::spaced(native::sized(native::column(format!("{}/@layout:1333",
                use_scope), children,), Some(wire::Length::Fill), None,), 1.0f32,) }),
                }); } native::sized(native::column(format!("{}/@layout:1312", use_scope),
                children,), Some(wire::Length::Fill), None,) }), }), }); }
                wire::Node::Stack { key : format!("{}/@layout:689", use_scope), width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), padding :
                None, background : None, border : None, clip : false, under : 0u32,
                children : children, } });
                            children.push(native::sized(
                                native::container(
                                    format!("{}/@container:1340", use_scope),
                                    wire::Node::Space {
                                        width: Some(wire::Length::Fixed(1.0f32)),
                                        height: Some(wire::Length::Fixed(1.0f32)),
                                    },
                                ),
                                Some(wire::Length::Fill),
                                Some(wire::Length::Fixed(1.0f32)),
                            ));
                            if !self.post_refusal.is_empty() {
                                children.push(native::padded(
                                    native::sized(
                                        native::container(
                                            format!("{}/@container:1355", use_scope),
                                            self.composer_gate(format!(
                                                "{}/ComposerGate@3964",
                                                use_scope
                                            )),
                                        ),
                                        Some(wire::Length::Fill),
                                        None,
                                    ),
                                    wire::Edges {
                                        top: 12.0f32,
                                        right: 18.0f32,
                                        bottom: 0.0f32,
                                        left: 18.0f32,
                                    },
                                ));
                            }
                            children.push(native::padded(
                                native::sized(
                                    native::container(format!("{}/@container:1362", use_scope), {
                                        let node_scope = format!("{}/composer", use_scope);
                                        wire::Node::Surface {
                                            key: node_scope.clone(),
                                            name: String::from("chat_composer"),
                                            args: ::std::vec![
                                                {
                                                    let surface_arg =
                                                        &(crate::host::composer_scope(
                                                            ::std::convert::AsRef::as_ref(
                                                                &(self.endpoint),
                                                            ),
                                                            ::std::convert::AsRef::as_ref(
                                                                &(self.active_channel),
                                                            ),
                                                        ));
                                                    ::ducktape_view_guest::wire::SurfaceValue::Str(
                                                        ::std::string::ToString::to_string(
                                                            surface_arg,
                                                        ),
                                                    )
                                                },
                                                {
                                                    let surface_arg = &("message".to_owned());
                                                    ::ducktape_view_guest::wire::SurfaceValue::Str(
                                                        ::std::string::ToString::to_string(
                                                            surface_arg,
                                                        ),
                                                    )
                                                },
                                                {
                                                    let surface_arg = &(false);
                                                    ::ducktape_view_guest::wire::SurfaceValue::Bool(
                                                        *(surface_arg),
                                                    )
                                                },
                                                {
                                                    let surface_arg =
                                                        &("Message the channel…".to_owned());
                                                    ::ducktape_view_guest::wire::SurfaceValue::Str(
                                                        ::std::string::ToString::to_string(
                                                            surface_arg,
                                                        ),
                                                    )
                                                },
                                                {
                                                    let surface_arg = &(((self.loading
                                                        || (!self.connected))
                                                        || (self.active_channel).is_empty())
                                                        || (!(self.post_refusal).is_empty()));
                                                    ::ducktape_view_guest::wire::SurfaceValue::Bool(
                                                        *(surface_arg),
                                                    )
                                                },
                                                {
                                                    let surface_arg = &(self.busy);
                                                    ::ducktape_view_guest::wire::SurfaceValue::Bool(
                                                        *(surface_arg),
                                                    )
                                                },
                                                {
                                                    let surface_arg =
                                                        &("An earlier message wasn’t sent"
                                                            .to_owned());
                                                    ::ducktape_view_guest::wire::SurfaceValue::Str(
                                                        ::std::string::ToString::to_string(
                                                            surface_arg,
                                                        ),
                                                    )
                                                }
                                            ],
                                            on_event: None,
                                        }
                                    }),
                                    Some(wire::Length::Fill),
                                    None,
                                ),
                                wire::Edges {
                                    top: 12.0f32,
                                    right: 18.0f32,
                                    bottom: 14.0f32,
                                    left: 18.0f32,
                                },
                            ));
                            native::sized(
                                native::column(format!("{}/@layout:544", use_scope), children),
                                Some(wire::Length::Fill),
                                Some(wire::Length::Fill),
                            )
                        }];
                        if self.channel_settings_open && !self.active_channel.is_empty() {
                            children.push({
                                let node_scope = format!("{}/details-resize", use_scope);
                                wire::Node::ResizeHandle {
                                    key: node_scope.clone(),
                                    on_press: None,
                                    on_release: None,
                                    on_drag: Some(::ducktape_view_guest::slots::handler::<
                                        (f64, f64),
                                        Message,
                                    >(Box::new(
                                        {
                                            let route = {
                                                let _route_state_scope_0 = use_scope.clone();
                                                let route_callback = (move |event_0, event_1| {
                                                    Message::DetailsResized(event_0, event_1)
                                                })
                                                .clone();
                                                move |delta: (f64, f64)| {
                                                    route_callback(delta.0, delta.1)
                                                }
                                            };
                                            move |sent: (f64, f64)| Some(route(sent))
                                        },
                                    ))),
                                    cursor: Some(wire::mouse::Cursor::ResizingHorizontally),
                                    content: Box::new({
                                        let node_scope = format!("{}/details-divider", node_scope);
                                        wire::Node::Container {
                                            shadow: Default::default(),
                                            max_width: None,
                                            max_height: None,
                                            clip: false,
                                            key: node_scope.clone(),
                                            width: Some(wire::Length::Fixed(10.0f32)),
                                            height: Some(wire::Length::Fill),
                                            padding: None,
                                            align_x: Some(wire::AlignX::Center),
                                            align_y: None,
                                            background: None,
                                            border: None,
                                            snap: None,
                                            content: Box::new(native::sized(
                                                native::container(
                                                    format!("{}/@container:1385", use_scope),
                                                    wire::Node::Space {
                                                        width: Some(wire::Length::Fixed(2.0f32)),
                                                        height: Some(wire::Length::Fixed(1.0f32)),
                                                    },
                                                ),
                                                Some(wire::Length::Fixed(2.0f32)),
                                                Some(wire::Length::Fill),
                                            )),
                                        }
                                    }),
                                }
                            });
                            children.push({ let node_scope = format!("{}/details-pane",
                use_scope); native::sized(native::container(node_scope.clone(), { let children : Vec < wire::Node > =
                vec![native::padded(native::sized(native::container(format!("{}/@container:1393",
                use_scope), { let children : Vec < wire::Node > =
                vec![native::sized(native::text_options(native::text(format!("{}/@text:1405",
                use_scope), "Channel details".to_owned().to_string(),), wire::TextOptions
                { wrapping : Some(wire::Wrapping::None), ..Default::default() },),
                Some(wire::Length::Fill), None,), wire::Node::Button { checked : None,
                expanded : Some(self.channel_settings_open), description : None, key :
                format!("{}/@button:1412", use_scope), content :
                wire::ButtonContent::Child(Box::new(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:1420", use_scope), width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), padding :
                None, align_x : Some(wire::AlignX::Center), align_y :
                Some(wire::AlignY::Center), background : None
                .map(wire::Background::Color), border : None, snap : None, content :
                Box::new(native::text(format!("{}/@text:1426", use_scope), "×"
                .to_owned().to_string(),),), }),), label :
                Some(String::from("Close channel details".to_owned()),), on_press :
                Some(::ducktape_view_guest::slots::message(Message::ToggleChannelSettings,),),
                width : Some(wire::Length::Fixed(28.0f32)), height :
                Some(wire::Length::Fixed(28.0f32)), padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), }];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:1399", use_scope), wrap : None, axis :
                wire::Axis::Row, spacing : Some(6.0f32), padding : None, width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } },), Some(wire::Length::Fill),
                Some(wire::Length::Fixed(50.0f32)),), wire::Edges { top : 0.0f32, right :
                10.0f32, bottom : 0.0f32, left : 16.0f32, },),
                native::sized(native::container(format!("{}/@container:1430", use_scope),
                wire::Node::Space { width : Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(1.0f32)), },), Some(wire::Length::Fill),
                Some(wire::Length::Fixed(1.0f32)),), wire::Node::Scroll { on_scroll :
                None, virtual_rows : false, key : format!("{}/@layout:1436", use_scope),
                direction : wire::ScrollDirection::Vertical, width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), bar_hidden :
                false, bar_width : None, bar_margin : None, scroller_width : None,
                bar_spacing : None, anchor_x : wire::ScrollAnchor::Start, anchor_y :
                wire::ScrollAnchor::Start, auto_scroll : false, background : None, border
                : None, content : Box::new({ let children : Vec < wire::Node > =
                vec![{ let mut children : Vec < wire::Node > = vec![{ let mut children :
                Vec < wire::Node > = Vec::new(); if ! self.active_channel_members_only {
                children.push(native::text_options(native::text(format!("{}/@text:1456",
                use_scope), "#".to_owned().to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },),); } if self
                .active_channel_members_only { children
                .push(native::text_options(native::text(format!("{}/@text:1463",
                use_scope), "◆".to_owned().to_string(),), wire::TextOptions { wrapping
                : Some(wire::Wrapping::None), ..Default::default() },),); } children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:1468",
                use_scope), self.active_channel_name.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },), Some(wire::Length::Fill), None,),);
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:1450", use_scope), wrap : None, axis :
                wire::Axis::Row, spacing : Some(7.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }]; if self.active_channel_archived || self
                .active_channel_members_only { children.push({ let mut children : Vec <
                wire::Node > = Vec::new(); if self.active_channel_archived { children
                .push(self.archived_badge(format!("{}/Badge.Outline@4085",
                use_scope),),); } if self.active_channel_members_only { children
                .push(self.private_badge(format!("{}/Badge.Outline@4087", use_scope),),);
                } wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:1476", use_scope), wrap : None, axis :
                wire::Axis::Row, spacing : Some(5.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }); }
                native::spaced(native::sized(native::column(format!("{}/@layout:1449",
                use_scope), children,), Some(wire::Length::Fill), None,), 7.0f32,) }, {
                let children : Vec < wire::Node > = vec![self
                .name_label(format!("{}/Eyebrow@4089", use_scope)), { let children :
                Vec < wire::Node > = vec![{ let node_scope = format!("{}/channel-name",
                node_scope); wire::Node::Input { options : wire::InputOptions { label :
                "Channel name".to_owned().to_string(), description : None, disabled :
                self.busy, padding : Some(wire::Edges::all(6.6f32)), text_size :
                Some(13.0f32), line_height : Some(1.2f32), align : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Normal, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), }, key : node_scope.clone(), placeholder :
                String::from("Channel name".to_owned()), value : self.channel_name_draft
                .to_string(), on_input : ::ducktape_view_guest::slots::handler:: <
                String, Message, > (Box::new({ let route =
                Message::ChannelNameDraftChanged as fn (String) -> Message; move | sent :
                String | Some(route(sent)) }),), on_submit :
                Some(::ducktape_view_guest::slots::message(Message::RenameChannelSubmit,),),
                width : Some(wire::Length::Fill), secure : false, style :
                Default::default(), } },
                native::padded(native::button(format!("{}/@button:1506", use_scope),
                String::from("Rename"), if self.busy || self.channel_name_draft.trim()
                .to_owned().is_empty() { None } else {
                Some(::ducktape_view_guest::slots::message(Message::RenameChannelSubmit,),)
                }, wire::ButtonPreset::Secondary,), wire::Edges::all(6.0f32),)];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:1487", use_scope), wrap : None, axis :
                wire::Axis::Row, spacing : Some(6.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }];
                native::spaced(native::sized(native::column(format!("{}/@layout:1485",
                use_scope), children,), Some(wire::Length::Fill), None,), 6.0f32,) },
                wire::Node::Button { checked : None, expanded : None, description : None,
                key : format!("{}/@button:1513", use_scope), content :
                wire::ButtonContent::Label(String::from("Copy channel link"),), label :
                Some(String::from("Copy channel link".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(Message::CopyToClipboard(crate
                ::host::duck_channel_link(self.active_channel.to_owned(), self
                .network_chain_id.to_owned(),), "Channel link copied".to_owned(),),),),
                width : Some(wire::Length::Fill), height : None, padding :
                Some(wire::Edges::all(6.0f32)), style : wire::ButtonStyle::default(), },
                { let mut children : Vec < wire::Node > = vec![{ let children : Vec <
                wire::Node > = vec![self.members_label(format!("{}/Eyebrow@4128",
                use_scope)), wire::Node::Space { width : Some(wire::Length::Fill), height
                : None, }, native::text_options(native::text(format!("{}/@text:1530",
                use_scope), crate ::host::count_label(self.channel_members.len() as i64,)
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:1520", use_scope), wrap : None, axis :
                wire::Axis::Row, spacing : Some(6.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }, { let children : Vec < wire::Node > = vec![{ let
                node_scope = format!("{}/member-key", node_scope); wire::Node::Input {
                options : wire::InputOptions { label : "Member account or public key"
                .to_owned().to_string(), description : None, disabled : self.busy,
                padding : Some(wire::Edges::all(7.4f32)), text_size : Some(11.5f32),
                line_height : Some(1.2f32), align : None, font : Some(wire::NamedFont {
                family : wire::FontFamily::Named("Geist Mono".into()), weight :
                wire::Weight::Normal, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Normal, }), }, key : node_scope.clone(), placeholder :
                String::from("acct:123 or public key".to_owned(),), value : self
                .member_key_draft.to_string(), on_input :
                ::ducktape_view_guest::slots::handler:: < String, Message, > (Box::new({
                let route = Message::MemberKeyDraftChanged as fn (String) -> Message;
                move | sent : String | Some(route(sent)) }),), on_submit :
                Some(::ducktape_view_guest::slots::message(Message::AddChannelMemberSubmit,),),
                width : Some(wire::Length::Fill), secure : false, style :
                Default::default(), } },
                native::padded(native::button(format!("{}/@button:1556", use_scope),
                String::from("Add"), if self.busy || self.member_key_draft.trim()
                .to_owned().is_empty() { None } else {
                Some(::ducktape_view_guest::slots::message(Message::AddChannelMemberSubmit,),)
                }, wire::ButtonPreset::Secondary,), wire::Edges::all(6.0f32),)];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:1536", use_scope), wrap : None, axis :
                wire::Axis::Row, spacing : Some(6.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }]; if self.channel_members.is_empty() { children
                .push(native::sized(native::text(format!("{}/@text:1562", use_scope),
                "No members added. An Open channel needs none — membership only gates posting in a members-only channel."
                .to_owned().to_string(),), Some(wire::Length::Fill), None,),); } if !
                self.channel_members.is_empty() { children.push({ let mut children : Vec
                < wire::Node > = Vec::new(); for (index, member) in self.channel_members
                .iter().enumerate() { let for_scope = format!("{}/@for:4173({})",
                use_scope, index); children.push(self
                .member_row(format!("{}/ChatMemberRow@4174", for_scope),
                (move | event_0 | Message::RemoveChannelMemberSubmit(event_0)).clone(),
                member.clone(),),); }
                native::spaced(native::sized(native::column(format!("{}/@layout:1569",
                use_scope), children,), Some(wire::Length::Fill), None,), 1.0f32,) }); }
                native::spaced(native::sized(native::column(format!("{}/@layout:1519",
                use_scope), children,), Some(wire::Length::Fill), None,), 6.0f32,) }];
                native::spaced(native::padded(native::sized(native::column(format!("{}/@layout:1441",
                use_scope), children,), Some(wire::Length::Fill), None,), wire::Edges {
                top : 14.0f32, right : 16.0f32, bottom : 14.0f32, left : 16.0f32, },),
                16.0f32,) }), },
                native::sized(native::container(format!("{}/@container:1574", use_scope),
                wire::Node::Space { width : Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(1.0f32)), },), Some(wire::Length::Fill),
                Some(wire::Length::Fixed(1.0f32)),),
                native::padded(native::sized(native::container(format!("{}/@container:1580",
                use_scope), { let mut children : Vec < wire::Node > = Vec::new(); if !
                self.active_channel_archived { children
                .push(native::padded(native::sized(native::button(format!("{}/@button:1595",
                use_scope), String::from("Archive channel"), if self.busy { None } else {
                Some(::ducktape_view_guest::slots::message(Message::ArchiveChannelSubmit,),)
                }, wire::ButtonPreset::Secondary,), Some(wire::Length::Fill), None,),
                wire::Edges::all(6.0f32),),); } if self.active_channel_archived {
                children
                .push(native::padded(native::sized(native::button(format!("{}/@button:1605",
                use_scope), String::from("Unarchive channel"), if self.busy { None } else
                {
                Some(::ducktape_view_guest::slots::message(Message::UnarchiveChannelSubmit,),)
                }, wire::ButtonPreset::Secondary,), Some(wire::Length::Fill), None,),
                wire::Edges::all(6.0f32),),); }
                native::sized(native::column(format!("{}/@layout:1587", use_scope),
                children,), Some(wire::Length::Fill), None,) },),
                Some(wire::Length::Fill), None,), wire::Edges { top : 10.0f32, right :
                16.0f32, bottom : 12.0f32, left : 16.0f32, },)];
                native::sized(native::column(format!("{}/@layout:1392", use_scope),
                children,), Some(wire::Length::Fill), Some(wire::Length::Fill),) },),
                Some(wire::Length::Fixed(self.details_width as f32)),
                Some(wire::Length::Fill),) });
                        }
                        if self.active_thread_seq > 0 && !self.channel_settings_open {
                            children.push({
                                let node_scope = format!("{}/thread-resize", use_scope);
                                wire::Node::ResizeHandle {
                                    key: node_scope.clone(),
                                    on_press: None,
                                    on_release: None,
                                    on_drag: Some(::ducktape_view_guest::slots::handler::<
                                        (f64, f64),
                                        Message,
                                    >(Box::new(
                                        {
                                            let route = {
                                                let _route_state_scope_0 = use_scope.clone();
                                                let route_callback = (move |event_0, event_1| {
                                                    Message::ThreadResized(event_0, event_1)
                                                })
                                                .clone();
                                                move |delta: (f64, f64)| {
                                                    route_callback(delta.0, delta.1)
                                                }
                                            };
                                            move |sent: (f64, f64)| Some(route(sent))
                                        },
                                    ))),
                                    cursor: Some(wire::mouse::Cursor::ResizingHorizontally),
                                    content: Box::new({
                                        let node_scope = format!("{}/thread-divider", node_scope);
                                        wire::Node::Container {
                                            shadow: Default::default(),
                                            max_width: None,
                                            max_height: None,
                                            clip: false,
                                            key: node_scope.clone(),
                                            width: Some(wire::Length::Fixed(10.0f32)),
                                            height: Some(wire::Length::Fill),
                                            padding: None,
                                            align_x: Some(wire::AlignX::Center),
                                            align_y: None,
                                            background: None,
                                            border: None,
                                            snap: None,
                                            content: Box::new(native::sized(
                                                native::container(
                                                    format!("{}/@container:1619", use_scope),
                                                    wire::Node::Space {
                                                        width: Some(wire::Length::Fixed(2.0f32)),
                                                        height: Some(wire::Length::Fixed(1.0f32)),
                                                    },
                                                ),
                                                Some(wire::Length::Fixed(2.0f32)),
                                                Some(wire::Length::Fill),
                                            )),
                                        }
                                    }),
                                }
                            });
                            children.push({ let node_scope = format!("{}/thread-pane",
                use_scope); native::sized(native::container(node_scope.clone(), { let children : Vec < wire::Node > = vec![wire::Node::Sensor { key :
                format!("{}/@sensor:1631", use_scope), reset : None, on_show :
                Some(::ducktape_view_guest::slots::handler:: < (f32, f32), Message, >
                (Box::new({ let route = { let route_scope = use_scope.clone(); move |
                size : (f64, f64) | Message::ChatScreenThreadResized(route_scope.clone(),
                size.0, size.1,) }; move | sent : (f32, f32) | Some(route((f64::from(sent
                .0), f64::from(sent.1))),) }),),), on_resize :
                Some(::ducktape_view_guest::slots::handler:: < (f32, f32), Message, >
                (Box::new({ let route = { let route_scope = use_scope.clone(); move |
                size : (f64, f64) | Message::ChatScreenThreadResized(route_scope.clone(),
                size.0, size.1,) }; move | sent : (f32, f32) | Some(route((f64::from(sent
                .0), f64::from(sent.1))),) }),),), on_hide : None, anticipate : None,
                delay : None, child : Box::new(wire::Node::Space { width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), }), },
                wire::Node::MouseArea { key : format!("{}/@mouse:1633", use_scope),
                on_press : None, on_release : None, on_double_click : None,
                on_right_press : None, on_right_release : None, on_middle_press : None,
                on_middle_release : None, on_enter : None, on_exit : None, on_move :
                None, on_press_at : Some(::ducktape_view_guest::slots::handler:: < (f32,
                f32), Message, > (Box::new({ let route = { let route_scope = use_scope
                .clone(); move | point : (f64, f64) |
                Message::ChatScreenThreadPointerPressed(route_scope.clone(), point.0,
                point.1,) }; move | sent : (f32, f32) | Some(route((f64::from(sent.0),
                f64::from(sent.1))),) }),),), on_scroll : None, content : Box::new({ let
                mut children : Vec < wire::Node > =
                vec![native::padded(native::sized(native::container(format!("{}/@container:1640",
                use_scope), { let mut children : Vec < wire::Node > = Vec::new(); if self
                .thread_target_seq <= 0 { children
                .push(native::text_options(native::text(format!("{}/@text:1653",
                use_scope), "Thread".to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },),); } if
                self.thread_target_seq > 0 { children
                .push(native::text_options(native::text(format!("{}/@text:1660",
                use_scope), "Thread result".to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },),); }
                children.push({ let mut children : Vec < wire::Node > = Vec::new(); if
                self.active_dm.name.is_empty() { children
                .push(native::text_options(native::text(format!("{}/@text:1671",
                use_scope), "#".to_owned().to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },),); } children
                .push(native::text_options(native::text(format!("{}/@text:1676",
                use_scope), self.active_channel_name.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:1666", use_scope), wrap : None, axis :
                wire::Axis::Row, spacing : Some(2.0f32), padding : None, width : None,
                height : None, align : Some(wire::AlignX::Center), background : None,
                border : None, children : children, } }); children.push(wire::Node::Space
                { width : Some(wire::Length::Fill), height : None, }); children
                .push(wire::Node::Button { checked : None, expanded : None, description :
                None, key : format!("{}/@button:1682", use_scope), content :
                wire::ButtonContent::Child(Box::new(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:1690", use_scope), width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), padding :
                None, align_x : Some(wire::AlignX::Center), align_y :
                Some(wire::AlignY::Center), background : None
                .map(wire::Background::Color), border : None, snap : None, content :
                Box::new(native::text(format!("{}/@text:1696", use_scope), "×"
                .to_owned().to_string(),),), }),), label :
                Some(String::from("Close thread".to_owned())), on_press : if self.busy {
                None } else {
                Some(::ducktape_view_guest::slots::message(Message::CloseThread),) },
                width : Some(wire::Length::Fixed(24.0f32)), height :
                Some(wire::Length::Fixed(24.0f32)), padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), });
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:1646", use_scope), wrap : None, axis :
                wire::Axis::Row, spacing : Some(7.0f32), padding : None, width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } },), Some(wire::Length::Fill),
                Some(wire::Length::Fixed(50.0f32)),), wire::Edges { top : 0.0f32, right :
                16.0f32, bottom : 0.0f32, left : 16.0f32, },),
                native::sized(native::container(format!("{}/@container:1700", use_scope),
                wire::Node::Space { width : Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(1.0f32)), },), Some(wire::Length::Fill),
                Some(wire::Length::Fixed(1.0f32)),)]; if self.copy_surface ==
                CopySurface::Thread && crate
                ::host::copy_range_count(::std::convert::AsRef::as_ref(& self
                .thread_messages), self.copy_anchor_seq, self.copy_head_seq,) > 0 {
                children.push({ let node_scope = format!("{}/thread-selection",
                node_scope); self.render_thread_selection(node_scope.clone(), (move | |
                Message::ClearCopyRange).clone(), (move | |
                Message::CopySelectedMessages).clone(),) }); } children.push({ let
                node_scope = format!("{}/thread-stream", node_scope); wire::Node::Scroll
                { on_scroll : None, virtual_rows : true, key : node_scope.clone(),
                direction : wire::ScrollDirection::Vertical, width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), bar_hidden :
                false, bar_width : None, bar_margin : None, scroller_width : None,
                bar_spacing : None, anchor_x : wire::ScrollAnchor::Start, anchor_y :
                wire::ScrollAnchor::End, auto_scroll : self.thread_target_seq <= 0,
                background : None, border : None, content : Box::new({ let mut children :
                Vec < wire::Node > = vec![{ let _lazy_context_720 = use_scope.to_owned();
                let _lazy_event_720_0 = (move | event_0 | Message::CancelRun(event_0,))
                .clone(); let lazy_event_720_1 = (move | event_0, event_1 |
                Message::PressMessage(event_0, event_1,)).clone(); let _lazy_event_720_2
                = (move | | Message::ClearCopyRange).clone(); let _lazy_event_720_3 =
                (move | | { Message::CopySelectedMessages }).clone(); let
                _lazy_event_720_4 = (move | | Message::SearchChatSubmit).clone(); let
                _lazy_event_720_5 = (move | | Message::ClearChatSearch).clone(); let
                _lazy_event_720_6 = (move | event_0, event_1, event_2 |
                Message::OpenChatSearchHit(event_0, event_1, event_2,)).clone(); let
                _lazy_event_720_7 = (move | | { Message::ToggleChannelCreate }).clone();
                let _lazy_event_720_8 = (move | event_0 |
                Message::ChooseChannel(event_0,)).clone(); let _lazy_event_720_9 = (move
                | event_0 | Message::ChooseDm(event_0,)).clone(); let _lazy_event_720_10
                = (move | | { Message::ToggleChannelSettings }).clone(); let
                _lazy_event_720_11 = (move | | Message::ShowHuddle).clone(); let
                _lazy_event_720_12 = (move | | Message::LeaveHuddleHere).clone(); let
                _lazy_event_720_13 = (move | | Message::JoinHuddleSubmit).clone(); let
                _lazy_event_720_14 = (move | | Message::LoadMoreHistory).clone(); let
                _lazy_event_720_15 = (move | event_0, event_1, event_2, event_3 |
                Message::ChatScrolled(event_0, event_1, event_2, event_3)).clone(); let
                lazy_event_720_16 = (move | event_0 | Message::OpenMessageLink(event_0,))
                .clone(); let lazy_event_720_17 = (move | event_0 |
                Message::OpenRun(event_0,)).clone(); let _lazy_event_720_18 = (move |
                event_0, event_1 | Message::CopyToClipboard(event_0, event_1,)).clone();
                let _lazy_event_720_19 = (move | event_0 |
                Message::CopyMessageLink(event_0,)).clone(); let lazy_event_720_20 =
                (move | event_0, event_1 | Message::AddReactionAt(event_0, event_1,))
                .clone(); let lazy_event_720_21 = (move | event_0, event_1 |
                Message::RemoveReactionAt(event_0, event_1,)).clone(); let
                lazy_event_720_22 = (move | event_0 | Message::OpenThreadFor(event_0,))
                .clone(); let _lazy_event_720_23 = (move | event_0, event_1, event_2 |
                Message::OpenMessageActions(event_0, event_1, event_2,)).clone(); let
                _lazy_event_720_24 = (move | event_0, event_1, event_2 |
                Message::OpenMessageReactions(event_0, event_1, event_2,)).clone(); let
                _lazy_event_720_25 = (move | event_0, event_1, event_2 |
                Message::BeginMessageEdit(event_0, event_1, event_2,)).clone(); let
                _lazy_event_720_26 = (move | event_0, event_1, event_2 |
                Message::ArmMessageDelete(event_0, event_1, event_2,)).clone(); let
                _lazy_event_720_27 = (move | | { Message::ClearMessageSelection })
                .clone(); let _lazy_event_720_28 = (move | event_0 |
                Message::AddReactionSubmit(event_0,)).clone(); let _lazy_event_720_29 =
                (move | | { Message::DeleteMessageSubmit }).clone(); let
                _lazy_event_720_30 = (move | | { Message::RenameChannelSubmit }).clone();
                let _lazy_event_720_31 = (move | | { Message::ArchiveChannelSubmit })
                .clone(); let _lazy_event_720_32 = (move | | {
                Message::UnarchiveChannelSubmit }).clone(); let _lazy_event_720_33 =
                (move | | { Message::AddChannelMemberSubmit }).clone(); let
                _lazy_event_720_34 = (move | event_0 |
                Message::RemoveChannelMemberSubmit(event_0,)).clone(); let
                _lazy_event_720_35 = (move | | Message::CloseThread).clone(); let
                _lazy_event_720_36 = (move | event_0, event_1 |
                Message::SidebarResized(event_0, event_1,)).clone(); let
                _lazy_event_720_37 = (move | event_0, event_1 |
                Message::DetailsResized(event_0, event_1,)).clone(); let
                _lazy_event_720_38 = (move | event_0, event_1 |
                Message::ThreadResized(event_0, event_1,)).clone(); let lazy_event_720_39
                = (move | event_0, event_1, event_2 |
                Message::OpenThreadMessageActions(event_0, event_1, event_2,)).clone();
                let lazy_event_720_40 = (move | event_0, event_1, event_2 |
                Message::OpenThreadMessageReactions(event_0, event_1, event_2,)).clone();
                let _lazy_event_720_41 = (move | event_0, event_1, event_2 |
                Message::BeginThreadMessageEdit(event_0, event_1, event_2,)).clone(); let
                _lazy_event_720_42 = (move | event_0, event_1, event_2 |
                Message::ArmThreadMessageDelete(event_0, event_1, event_2,)).clone(); let
                _lazy_event_720_43 = (move | | { Message::ClearThreadMessageSelection })
                .clone(); let _lazy_event_720_44 = (move | | {
                Message::DeleteThreadMessageSubmit }).clone(); let _lazy_event_720_45 =
                (move | | Message::LoadMoreThread).clone(); { let lazy_key =
                format!("{}/@lazy:1750", use_scope);
                ::ducktape_view_guest::memo_lazy((self.active_channel.to_owned(), self
                .active_thread_seq, self.thread_target_seq, self.thread_selected_seq,
                self.copy_anchor_seq, self.copy_head_seq, self.copy_surface.clone(), self
                .thread_messages_revision, node_scope.to_owned(), (),), move | dependency | { let _active_channel : String =
                dependency.0.clone(); let active_thread_seq : i64 = dependency.1.clone();
                let thread_target_seq : i64 = dependency.2.clone(); let
                thread_selected_seq : i64 = dependency.3.clone(); let copy_anchor_seq :
                i64 = dependency.4.clone(); let copy_head_seq : i64 = dependency.5
                .clone(); let copy_surface : CopySurface = dependency.6.clone(); let
                lazy_scope = dependency.8.clone(); let cached_thread_messages : Vec <
                crate ::host::ChatMessage > = self.thread_messages.clone(); { let
                thread_timeline_scope_4354 = format!("{}/ThreadTimeline@4354",
                lazy_scope); { let mut children : Vec < _ > = Vec::new(); for
                thread_message in cached_thread_messages.iter() { let key =
                thread_message.view_key; let key_recon = format!("{}/key({})",
                thread_timeline_scope_4354, key); let child : wire::Node = { let mut
                children : Vec < wire::Node > = Vec::new(); if thread_message.seq ==
                active_thread_seq { children.push({ let thread_parent_block_scope_2747 =
                format!("{}/ThreadParentBlock@2747", key_recon); { let node_scope =
                format!("{}/root", thread_parent_block_scope_2747); { let mut children :
                Vec < wire::Node > = vec![{ let children : Vec < wire::Node > =
                vec![{ let principal_avatar_scope_2430 =
                format!("{}/PrincipalAvatar@2430", thread_parent_block_scope_2747); { let
                node_scope = format!("{}/root", principal_avatar_scope_2430); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                principal_plate_scope_909 = format!("{}/PrincipalPlate@909",
                principal_avatar_scope_2430); { let node_scope = format!("{}/root",
                principal_plate_scope_909); { let mut children : Vec < wire::Node > =
                Vec::new(); if thread_message.avatar_kind == "agent" { children.push({
                let agent_plate_scope_919 = format!("{}/AgentPlate@919",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                agent_plate_scope_919); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let agent_square_scope_1174 =
                format!("{}/AgentSquare@1174", agent_plate_scope_919); { let node_scope =
                format!("{}/root", agent_square_scope_1174); wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : node_scope.clone(), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content :
                Box::new(native::text_options(native::text(format!("{}/@text:390",
                agent_square_scope_1174), thread_message.initial.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), } } }); } native::column(node_scope.clone(),
                children) } } }); } if ! (thread_message.avatar_kind == "agent") {
                children.push({ let human_plate_scope_925 = format!("{}/HumanPlate@925",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                human_plate_scope_925); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:328", human_plate_scope_925), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([((30.0 / 2.0) as f32).max(0.0).min(f32::MAX), ((30.0
                / 2.0) as f32).max(0.0).min(f32::MAX), ((30.0 / 2.0) as f32).max(0.0)
                .min(f32::MAX), ((30.0 / 2.0) as f32).max(0.0).min(f32::MAX),]), }), snap
                : None, content :
                Box::new(native::text_options(native::text(format!("{}/@text:336",
                human_plate_scope_925), thread_message.initial.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), }); } native::column(node_scope.clone(),
                children) } } }); } native::column(node_scope.clone(), children) } } });
                } native::column(node_scope.clone(), children) } } }, { let children
                : Vec < wire::Node > = vec![{ let mut children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:1030",
                thread_parent_block_scope_2747), thread_message.author.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; if thread_message.height > 0 { children
                .push(native::text_options(native::text(format!("{}/@text:1038",
                thread_parent_block_scope_2747), crate
                ::host::height_label_short(thread_message.height).to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } if thread_message.edited { children
                .push(native::text_options(native::text(format!("{}/@text:1047",
                thread_parent_block_scope_2747), "· edited".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } if ! crate
                ::host::run_of_message(::std::convert::AsRef::as_ref(& thread_message
                .id),).is_empty() { children
                .push(native::padded(native::button(format!("{}/@button:1054",
                thread_parent_block_scope_2747), String::from("View run"),
                Some(::ducktape_view_guest::slots::message(lazy_event_720_17(crate
                ::host::run_of_message(::std::convert::AsRef::as_ref(& thread_message
                .id),),),),), wire::ButtonPreset::Secondary,),
                wire::Edges::all(3.0f32),),); } children.push(wire::Node::Space { width :
                Some(wire::Length::Fill), height : None, }); wire::Node::Linear {
                max_width : None, clip : false, key : format!("{}/@layout:1025",
                thread_parent_block_scope_2747), wrap : None, axis : wire::Axis::Row,
                spacing : Some(6.0f32), padding : None, width : Some(wire::Length::Fill),
                height : None, align : Some(wire::AlignX::Center), background : None,
                border : None, children : children, } }, { let message_body_scope_2472 =
                format!("{}/MessageBody@2472", thread_parent_block_scope_2747); { let children : Vec < wire::Node > = vec![{ let rich_body_scope_688 =
                format!("{}/RichBody@688", message_body_scope_2472); { let mut children :
                Vec < wire::Node > = Vec::new(); for (index, block) in thread_message
                .blocks.iter().enumerate() { let for_scope = format!("{}/@for:703({})",
                rich_body_scope_688, index); if block.kind == "divider" { children.push({
                let component_separator_scope_705 = format!("{}/Separator@705",
                for_scope); { let node_scope = format!("{}/root",
                component_separator_scope_705); wire::Node::Rule { key : node_scope
                .clone(), axis : wire::Axis::Row, thickness : 1.0f32, color : None, weak
                : false, radius : None, snap : None, } } }); } if block.kind == "code" {
                children
                .push(native::padded(native::sized(native::container(format!("{}/@container:90",
                for_scope), { let mut children : Vec < wire::Node > = Vec::new(); if !
                block.lang.is_empty() { children
                .push(native::text_options(native::text(format!("{}/@text:100",
                for_scope), block.lang.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },),); }
                children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:106",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),);
                native::spaced(native::sized(native::column(format!("{}/@layout:98",
                for_scope), children,), Some(wire::Length::Fill), None,), 6.0f32,) },),
                Some(wire::Length::Fill), None,), wire::Edges { top : 11.0f32, right :
                11.0f32, bottom : 11.0f32, left : 11.0f32, },),); } if block.kind ==
                "quote" { children.push({ let children : Vec < wire::Node > = vec![{
                let mut children : Vec < wire::Node > = Vec::new(); if block.rich {
                children.push({ let rich_line_scope_755 = format!("{}/RichLine@755",
                for_scope); { let mut rich_spans : Vec < wire::RichSpan > = Vec::new();
                for span in block.spans.iter().cloned() { rich_spans.push(wire::RichSpan
                { content : span.mention.to_owned().to_string(), size : None, line_height
                : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Medium,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : Some(span.mention_link.to_owned()), background :
                None, border : Some(wire::Border { color : None, width : None, radius :
                Some([4.0f32, 4.0f32, 4.0f32, 4.0f32,]), }), padding : Some(wire::Edges {
                top : 0.0f32, right : 1.0f32, bottom : 0.0f32, left : 1.0f32, }),
                underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.link_text.to_owned().to_string(),
                size : None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Medium,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : Some(span.link.to_owned()), background : None,
                border : None, padding : None, underline : true, strikethrough : false,
                }); rich_spans.push(wire::RichSpan { content : span.bold_italic
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Bold, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Italic, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.bold
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Bold, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Normal, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.italic
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Normal, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Italic, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.plain
                .to_owned().to_string(), size : None, line_height : None, font : None,
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); }
                wire::Node::RichText { options : wire::TextOptions { wrapping :
                Some(wire::Wrapping::WordOrGlyph), ..Default::default() }, key :
                format!("{}/@text:32", rich_line_scope_755), size : Some(13.5f32), color
                : None, font : wire::Font { monospace : false, weight :
                wire::Weight::Normal, }, width : Some(wire::Length::Fill), align_x :
                None, spans : rich_spans, on_link :
                Some(::ducktape_view_guest::slots::handler:: < String, Message, >
                (Box::new({ let route = { let route_callback = lazy_event_720_16.clone();
                move | link : String | route_callback(link) }; move | sent : String |
                Some(route(sent)) }),),), } } }); } if ! block.rich { children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:137",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),); }
                native::padded(native::sized(native::column(format!("{}/@layout:121",
                for_scope), children,), Some(wire::Length::Fill), None,), wire::Edges {
                top : 2.0f32, right : 0.0f32, bottom : 2.0f32, left : 13.0f32, },) },
                native::sized(native::container(format!("{}/@container:144", for_scope),
                wire::Node::Space { width : Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(1.0f32)), },),
                Some(wire::Length::Fixed(3.0f32)), Some(wire::Length::Fill),)];
                wire::Node::Stack { key : format!("{}/@layout:120", for_scope), width :
                Some(wire::Length::Fill), height : None, padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, }
                }); } if block.kind == "paragraph" { if block.rich { children.push({ let
                rich_line_scope_780 = format!("{}/RichLine@780", for_scope); { let mut
                rich_spans : Vec < wire::RichSpan > = Vec::new(); for span in block.spans
                .iter().cloned() { rich_spans.push(wire::RichSpan { content : span
                .mention.to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Medium, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), color : None, link : Some(span
                .mention_link.to_owned()), background : None, border : Some(wire::Border
                { color : None, width : None, radius : Some([4.0f32, 4.0f32, 4.0f32,
                4.0f32,]), }), padding : Some(wire::Edges { top : 0.0f32, right : 1.0f32,
                bottom : 0.0f32, left : 1.0f32, }), underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.link_text
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Medium, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), color : None, link : Some(span.link
                .to_owned()), background : None, border : None, padding : None, underline
                : true, strikethrough : false, }); rich_spans.push(wire::RichSpan {
                content : span.bold_italic.to_owned().to_string(), size : None,
                line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Bold,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Italic, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.bold.to_owned().to_string(), size :
                None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Bold,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.italic.to_owned().to_string(), size
                : None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Normal,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Italic, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.plain.to_owned().to_string(), size
                : None, line_height : None, font : None, color : None, link : None,
                background : None, border : None, padding : None, underline : false,
                strikethrough : false, }); } wire::Node::RichText { options :
                wire::TextOptions { wrapping : Some(wire::Wrapping::WordOrGlyph),
                ..Default::default() }, key : format!("{}/@text:32",
                rich_line_scope_780), size : Some(13.5f32), color : None, font :
                wire::Font { monospace : false, weight : wire::Weight::Normal, }, width :
                Some(wire::Length::Fill), align_x : None, spans : rich_spans, on_link :
                Some(::ducktape_view_guest::slots::handler:: < String, Message, >
                (Box::new({ let route = { let route_callback = lazy_event_720_16.clone();
                move | link : String | route_callback(link) }; move | sent : String |
                Some(route(sent)) }),),), } } }); } if ! block.rich { children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:157",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),); } } }
                native::spaced(native::sized(native::column(format!("{}/@layout:75",
                rich_body_scope_688), children,), Some(wire::Length::Fill), None,),
                5.0f32,) } }]; wire::Node::Linear { max_width : Some(760.0f32), clip :
                false, key : format!("{}/@layout:60", message_body_scope_2472), wrap :
                None, axis : wire::Axis::Column, spacing : None, padding : None, width :
                Some(wire::Length::Fill), height : None, align : None, background : None,
                border : None, children : children, } } }];
                native::spaced(native::sized(native::column(format!("{}/@layout:1024",
                thread_parent_block_scope_2747), children,), Some(wire::Length::Fill),
                None,), 2.0f32,) }]; wire::Node::Linear { max_width : None, clip : false,
                key : format!("{}/@layout:1006", thread_parent_block_scope_2747), wrap :
                None, axis : wire::Axis::Row, spacing : Some(11.0f32), padding :
                Some(wire::Edges { top : 0.0f32, right : 0.0f32, bottom : 14.0f32, left :
                0.0f32, }), width : Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Left), background : None, border : None, children :
                children, } },
                native::sized(native::container(format!("{}/@container:1062",
                thread_parent_block_scope_2747), wire::Node::Space { width :
                Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(1.0f32)), },), Some(wire::Length::Fill),
                Some(wire::Length::Fixed(1.0f32)),)]; if thread_message.reply_count > 0 {
                children.push({ let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:1076",
                thread_parent_block_scope_2747), crate ::host::plural(thread_message
                .reply_count, ::std::convert::AsRef::as_ref(& "reply"),
                ::std::convert::AsRef::as_ref(& "replies"),).to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:1069", thread_parent_block_scope_2747),
                wrap : None, axis : wire::Axis::Row, spacing : Some(4.0f32), padding :
                Some(wire::Edges { top : 13.0f32, right : 0.0f32, bottom : 4.0f32, left :
                0.0f32, }), width : Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }); } native::sized(native::column(node_scope.clone(),
                children), Some(wire::Length::Fill), None,) } } }); } if thread_message
                .seq != active_thread_seq && (thread_message.seq == thread_target_seq ||
                thread_message.seq == thread_selected_seq) { children.push({ let
                thread_message_card_scope_2752 = format!("{}/ThreadMessageCard@2752",
                key_recon); { let mut children : Vec < wire::Node > = Vec::new(); if
                thread_message.show_author { children.push(wire::Node::Space { width :
                Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(14.0f32)), }); } children.push({ let children : Vec < wire::Node > = vec![{ let mut children : Vec <
                wire::Node > = Vec::new(); match & crate
                ::host::message_plate(thread_message.deleted, thread_message.seq ==
                thread_target_seq, crate ::host::seq_in_copy_range(thread_message.seq,
                copy_anchor_seq, copy_head_seq, copy_surface.clone(),
                CopySurface::Thread,),) { RowPlate::Plain => { children
                .push(native::padded(native::sized(native::container(format!("{}/@container:783",
                thread_message_card_scope_2752), { let message_contents_scope_2207 =
                format!("{}/MessageContents@2207", thread_message_card_scope_2752); { let
                children : Vec < wire::Node > = vec![{ let mut children : Vec <
                wire::Node > = Vec::new(); if thread_message.show_author { children
                .push({ let message_avatar_scope_1750 = format!("{}/MessageAvatar@1750",
                message_contents_scope_2207); { let node_scope = format!("{}/root",
                message_avatar_scope_1750); { let mut children : Vec < wire::Node > =
                Vec::new(); if thread_message.avatar_kind == "human" { children.push({
                let person_avatar_scope_798 = format!("{}/PersonAvatar@798",
                message_avatar_scope_1750); { let node_scope = format!("{}/root",
                person_avatar_scope_798); { let children : Vec < wire::Node > =
                vec![{ let principal_avatar_scope_863 = format!("{}/PrincipalAvatar@863",
                person_avatar_scope_798); { let node_scope = format!("{}/root",
                principal_avatar_scope_863); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let principal_plate_scope_909 =
                format!("{}/PrincipalPlate@909", principal_avatar_scope_863); { let
                node_scope = format!("{}/root", principal_plate_scope_909); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                human_plate_scope_925 = format!("{}/HumanPlate@925",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                human_plate_scope_925); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:328", human_plate_scope_925), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([((30.0 / 2.0) as f32).max(0.0).min(f32::MAX), ((30.0
                / 2.0) as f32).max(0.0).min(f32::MAX), ((30.0 / 2.0) as f32).max(0.0)
                .min(f32::MAX), ((30.0 / 2.0) as f32).max(0.0).min(f32::MAX),]), }), snap
                : None, content :
                Box::new(native::text_options(native::text(format!("{}/@text:336",
                human_plate_scope_925), thread_message.initial.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), }); } native::column(node_scope.clone(),
                children) } } }); } native::column(node_scope.clone(), children) } } });
                } native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } if !
                (thread_message.avatar_kind == "human") && thread_message.avatar_kind ==
                "agent" { children.push({ let agent_avatar_scope_804 =
                format!("{}/AgentAvatar@804", message_avatar_scope_1750); { let
                node_scope = format!("{}/root", agent_avatar_scope_804); { let children : Vec < wire::Node > = vec![{ let principal_avatar_scope_873 =
                format!("{}/PrincipalAvatar@873", agent_avatar_scope_804); { let
                node_scope = format!("{}/root", principal_avatar_scope_873); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                principal_plate_scope_909 = format!("{}/PrincipalPlate@909",
                principal_avatar_scope_873); { let node_scope = format!("{}/root",
                principal_plate_scope_909); { let children : Vec < wire::Node > =
                vec![{ let agent_plate_scope_919 = format!("{}/AgentPlate@919",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                agent_plate_scope_919); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let agent_square_scope_1174 =
                format!("{}/AgentSquare@1174", agent_plate_scope_919); { let node_scope =
                format!("{}/root", agent_square_scope_1174); wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : node_scope.clone(), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content :
                Box::new(native::text_options(native::text(format!("{}/@text:390",
                agent_square_scope_1174), thread_message.initial.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), } } }); } native::column(node_scope.clone(),
                children) } } }]; native::column(node_scope.clone(), children) } } }); }
                native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } if !
                (thread_message.avatar_kind == "human" || thread_message.avatar_kind ==
                "agent") { children.push({ let agent_avatar_scope_810 =
                format!("{}/AgentAvatar@810", message_avatar_scope_1750); { let
                node_scope = format!("{}/root", agent_avatar_scope_810); { let children : Vec < wire::Node > = vec![{ let principal_avatar_scope_873 =
                format!("{}/PrincipalAvatar@873", agent_avatar_scope_810); { let
                node_scope = format!("{}/root", principal_avatar_scope_873); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                principal_plate_scope_909 = format!("{}/PrincipalPlate@909",
                principal_avatar_scope_873); { let node_scope = format!("{}/root",
                principal_plate_scope_909); { let children : Vec < wire::Node > =
                vec![{ let agent_plate_scope_919 = format!("{}/AgentPlate@919",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                agent_plate_scope_919); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let agent_square_scope_1174 =
                format!("{}/AgentSquare@1174", agent_plate_scope_919); { let node_scope =
                format!("{}/root", agent_square_scope_1174); wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : node_scope.clone(), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content :
                Box::new(native::text_options(native::text(format!("{}/@text:390",
                agent_square_scope_1174), thread_message.initial.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), } } }); } native::column(node_scope.clone(),
                children) } } }]; native::column(node_scope.clone(), children) } } }); }
                native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } wire::Node::Stack
                { key : node_scope.clone(), width : Some(wire::Length::Fixed(30.0f32)),
                height : Some(wire::Length::Fixed(30.0f32)), padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, } }
                } }); } if ! thread_message.show_author { children.push(wire::Node::Space
                { width : Some(wire::Length::Fixed(30.0f32)), height : None, }); }
                children.push({ let mut children : Vec < wire::Node > = Vec::new(); if
                thread_message.show_author { children.push({ let mut children : Vec <
                wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:347",
                message_contents_scope_2207), thread_message.author.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; if thread_message.avatar_kind == "agent" {
                children
                .push(native::padded(native::container(format!("{}/@container:354",
                message_contents_scope_2207),
                native::text_options(native::text(format!("{}/@text:360",
                message_contents_scope_2207), "AGENT".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), wire::Edges { top : 2.0f32, right : 5.0f32,
                bottom : 2.0f32, left : 5.0f32, },),); } if thread_message.height > 0 {
                children.push(native::text_options(native::text(format!("{}/@text:377",
                message_contents_scope_2207), crate
                ::host::height_label_short(thread_message.height).to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } if thread_message.edited { children
                .push(native::text_options(native::text(format!("{}/@text:384",
                message_contents_scope_2207), "· edited".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } children.push(wire::Node::Space { width :
                Some(wire::Length::Fill), height : None, }); wire::Node::Linear {
                max_width : None, clip : false, key : format!("{}/@layout:342",
                message_contents_scope_2207), wrap : None, axis : wire::Axis::Row,
                spacing : Some(7.0f32), padding : None, width : Some(wire::Length::Fill),
                height : None, align : Some(wire::AlignX::Center), background : None,
                border : None, children : children, } }); } children
                .push(wire::Node::MouseArea { key : format!("{}/@mouse:395",
                message_contents_scope_2207), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_720_1(thread_message
                .seq, CopySurface::Thread.clone(),),),), on_release : None,
                on_double_click : None, on_right_press : None, on_right_release : None,
                on_middle_press : None, on_middle_release : None, on_enter : None,
                on_exit : None, on_move : None, on_press_at : None, on_scroll : None,
                content : Box::new({ let message_body_scope_1809 =
                format!("{}/MessageBody@1809", message_contents_scope_2207); { let children : Vec < wire::Node > = vec![{ let rich_body_scope_688 =
                format!("{}/RichBody@688", message_body_scope_1809); { let mut children :
                Vec < wire::Node > = Vec::new(); for (index, block) in thread_message
                .blocks.iter().enumerate() { let for_scope = format!("{}/@for:703({})",
                rich_body_scope_688, index); if block.kind == "divider" { children.push({
                let component_separator_scope_705 = format!("{}/Separator@705",
                for_scope); { let node_scope = format!("{}/root",
                component_separator_scope_705); wire::Node::Rule { key : node_scope
                .clone(), axis : wire::Axis::Row, thickness : 1.0f32, color : None, weak
                : false, radius : None, snap : None, } } }); } if block.kind == "code" {
                children
                .push(native::padded(native::sized(native::container(format!("{}/@container:90",
                for_scope), { let mut children : Vec < wire::Node > = Vec::new(); if !
                block.lang.is_empty() { children
                .push(native::text_options(native::text(format!("{}/@text:100",
                for_scope), block.lang.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },),); }
                children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:106",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),);
                native::spaced(native::sized(native::column(format!("{}/@layout:98",
                for_scope), children,), Some(wire::Length::Fill), None,), 6.0f32,) },),
                Some(wire::Length::Fill), None,), wire::Edges { top : 11.0f32, right :
                11.0f32, bottom : 11.0f32, left : 11.0f32, },),); } if block.kind ==
                "quote" { children.push({ let children : Vec < wire::Node > = vec![{
                let mut children : Vec < wire::Node > = Vec::new(); if block.rich {
                children.push({ let rich_line_scope_755 = format!("{}/RichLine@755",
                for_scope); { let mut rich_spans : Vec < wire::RichSpan > = Vec::new();
                for span in block.spans.iter().cloned() { rich_spans.push(wire::RichSpan
                { content : span.mention.to_owned().to_string(), size : None, line_height
                : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Medium,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : Some(span.mention_link.to_owned()), background :
                None, border : Some(wire::Border { color : None, width : None, radius :
                Some([4.0f32, 4.0f32, 4.0f32, 4.0f32,]), }), padding : Some(wire::Edges {
                top : 0.0f32, right : 1.0f32, bottom : 0.0f32, left : 1.0f32, }),
                underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.link_text.to_owned().to_string(),
                size : None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Medium,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : Some(span.link.to_owned()), background : None,
                border : None, padding : None, underline : true, strikethrough : false,
                }); rich_spans.push(wire::RichSpan { content : span.bold_italic
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Bold, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Italic, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.bold
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Bold, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Normal, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.italic
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Normal, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Italic, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.plain
                .to_owned().to_string(), size : None, line_height : None, font : None,
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); }
                wire::Node::RichText { options : wire::TextOptions { wrapping :
                Some(wire::Wrapping::WordOrGlyph), ..Default::default() }, key :
                format!("{}/@text:32", rich_line_scope_755), size : Some(13.5f32), color
                : None, font : wire::Font { monospace : false, weight :
                wire::Weight::Normal, }, width : Some(wire::Length::Fill), align_x :
                None, spans : rich_spans, on_link :
                Some(::ducktape_view_guest::slots::handler:: < String, Message, >
                (Box::new({ let route = { let route_callback = lazy_event_720_16.clone();
                move | link : String | route_callback(link) }; move | sent : String |
                Some(route(sent)) }),),), } } }); } if ! block.rich { children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:137",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),); }
                native::padded(native::sized(native::column(format!("{}/@layout:121",
                for_scope), children,), Some(wire::Length::Fill), None,), wire::Edges {
                top : 2.0f32, right : 0.0f32, bottom : 2.0f32, left : 13.0f32, },) },
                native::sized(native::container(format!("{}/@container:144", for_scope),
                wire::Node::Space { width : Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(1.0f32)), },),
                Some(wire::Length::Fixed(3.0f32)), Some(wire::Length::Fill),)];
                wire::Node::Stack { key : format!("{}/@layout:120", for_scope), width :
                Some(wire::Length::Fill), height : None, padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, }
                }); } if block.kind == "paragraph" { if block.rich { children.push({ let
                rich_line_scope_780 = format!("{}/RichLine@780", for_scope); { let mut
                rich_spans : Vec < wire::RichSpan > = Vec::new(); for span in block.spans
                .iter().cloned() { rich_spans.push(wire::RichSpan { content : span
                .mention.to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Medium, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), color : None, link : Some(span
                .mention_link.to_owned()), background : None, border : Some(wire::Border
                { color : None, width : None, radius : Some([4.0f32, 4.0f32, 4.0f32,
                4.0f32,]), }), padding : Some(wire::Edges { top : 0.0f32, right : 1.0f32,
                bottom : 0.0f32, left : 1.0f32, }), underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.link_text
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Medium, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), color : None, link : Some(span.link
                .to_owned()), background : None, border : None, padding : None, underline
                : true, strikethrough : false, }); rich_spans.push(wire::RichSpan {
                content : span.bold_italic.to_owned().to_string(), size : None,
                line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Bold,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Italic, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.bold.to_owned().to_string(), size :
                None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Bold,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.italic.to_owned().to_string(), size
                : None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Normal,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Italic, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.plain.to_owned().to_string(), size
                : None, line_height : None, font : None, color : None, link : None,
                background : None, border : None, padding : None, underline : false,
                strikethrough : false, }); } wire::Node::RichText { options :
                wire::TextOptions { wrapping : Some(wire::Wrapping::WordOrGlyph),
                ..Default::default() }, key : format!("{}/@text:32",
                rich_line_scope_780), size : Some(13.5f32), color : None, font :
                wire::Font { monospace : false, weight : wire::Weight::Normal, }, width :
                Some(wire::Length::Fill), align_x : None, spans : rich_spans, on_link :
                Some(::ducktape_view_guest::slots::handler:: < String, Message, >
                (Box::new({ let route = { let route_callback = lazy_event_720_16.clone();
                move | link : String | route_callback(link) }; move | sent : String |
                Some(route(sent)) }),),), } } }); } if ! block.rich { children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:157",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),); } } }
                native::spaced(native::sized(native::column(format!("{}/@layout:75",
                rich_body_scope_688), children,), Some(wire::Length::Fill), None,),
                5.0f32,) } }]; wire::Node::Linear { max_width : Some(760.0f32), clip :
                false, key : format!("{}/@layout:60", message_body_scope_1809), wrap :
                None, axis : wire::Axis::Column, spacing : None, padding : None, width :
                Some(wire::Length::Fill), height : None, align : None, background : None,
                border : None, children : children, } } }), }); if thread_message.edited
                && ! thread_message.show_author { children
                .push(native::text_options(native::text(format!("{}/@text:408",
                message_contents_scope_2207), "· edited".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } if ! crate
                ::host::run_of_message(::std::convert::AsRef::as_ref(& thread_message
                .id),).is_empty() { children.push({ let children : Vec < wire::Node >
                = vec![native::padded(native::button(format!("{}/@button:420",
                message_contents_scope_2207), String::from("View run"),
                Some(::ducktape_view_guest::slots::message(lazy_event_720_17(crate
                ::host::run_of_message(::std::convert::AsRef::as_ref(& thread_message
                .id),),),),), wire::ButtonPreset::Secondary,),
                wire::Edges::all(3.0f32),)];
                native::padded(native::sized(native::row(format!("{}/@layout:419",
                message_contents_scope_2207), children,), Some(wire::Length::Fill),
                None,), wire::Edges { top : 4.0f32, right : 0.0f32, bottom : 0.0f32, left
                : 0.0f32, },) }); } if ! thread_message.reactions.is_empty() { children
                .push({ let mut items = Vec::new(); for (index, reaction) in
                thread_message.reactions.iter().enumerate() { let for_scope =
                format!("{}/@for:1848({})", message_contents_scope_2207, index); let
                flex_child : wire::Node = { let reaction_chip_scope_1849 =
                format!("{}/ReactionChip@1849", for_scope); { let node_scope =
                format!("{}/root", reaction_chip_scope_1849); { let mut children : Vec <
                wire::Node > = Vec::new(); if reaction.reacted_by_me { children
                .push(wire::Node::Button { checked : Some(reaction.reacted_by_me),
                expanded : None, description : Some(String::from(reaction.emoji
                .to_owned())), key : format!("{}/@button:218", reaction_chip_scope_1849),
                content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:225",
                reaction_chip_scope_1849), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:232",
                reaction_chip_scope_1849), reaction.emoji.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:238",
                reaction_chip_scope_1849), reaction.count.to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:231", reaction_chip_scope_1849), wrap :
                None, axis : wire::Axis::Row, spacing : Some(4.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 1.0f32, right : 8.0f32, bottom : 1.0f32, left : 6.0f32, },),),),
                label : Some(String::from("Remove reaction".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_720_21(thread_message
                .seq, reaction.emoji.to_owned(),),),), width : None, height : None,
                padding : Some(wire::Edges::all(0.0f32)), style :
                wire::ButtonStyle::default(), }); } if ! reaction.reacted_by_me {
                children.push(wire::Node::Button { checked : Some(reaction
                .reacted_by_me), expanded : None, description :
                Some(String::from(reaction.emoji.to_owned())), key :
                format!("{}/@button:262", reaction_chip_scope_1849), content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:269",
                reaction_chip_scope_1849), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:276",
                reaction_chip_scope_1849), reaction.emoji.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:282",
                reaction_chip_scope_1849), reaction.count.to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:275", reaction_chip_scope_1849), wrap :
                None, axis : wire::Axis::Row, spacing : Some(4.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 1.0f32, right : 8.0f32, bottom : 1.0f32, left : 6.0f32, },),),),
                label : Some(String::from("Add reaction".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_720_20(thread_message
                .seq, reaction.emoji.to_owned(),),),), width : None, height : None,
                padding : Some(wire::Edges::all(0.0f32)), style :
                wire::ButtonStyle::default(), }); } native::column(node_scope.clone(),
                children) } } }; items.push((wire::FlexItem::default(), flex_child)); }
                let (items, children) = items.into_iter().unzip(); wire::Node::Flex { key
                : format!("{}/@layout:427", message_contents_scope_2207), items,
                children, background : None, border : None, layout : wire::FlexLayout {
                direction : wire::FlexDirection::Row, wrap : wire::FlexWrap::Wrap,
                justify : None, items : Some(wire::FlexItemAlignment::Start), content :
                None, row_gap : Some(5.0f32), column_gap : Some(5.0f32), padding :
                Some(wire::Edges { top : 6.0f32, right : 0.0f32, bottom : 0.0f32, left :
                0.0f32, }), width : Some(wire::Length::Fill), height : None, max_width :
                None, max_height : None, clip : false, surface_width : None,
                surface_height : None, surface_max_width : None, }, } }); } if
                thread_message.reply_count > 0 { children.push({ let children : Vec <
                wire::Node > = vec![wire::Node::Button { checked : None, expanded : None,
                description : None, key : format!("{}/@button:447",
                message_contents_scope_2207), content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:452",
                message_contents_scope_2207), { let children : Vec < wire::Node > =
                vec![{ let component_icon_scope_1872 = format!("{}/Icon@1872",
                message_contents_scope_2207); { let node_scope = format!("{}/root",
                component_icon_scope_1872); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "nav-chat"),),);
                wire::Node::Svg { inherit_button_ink : false, key :
                format!("{}/@media:52", component_icon_scope_1872), hash : hash, bytes :
                bytes, label : None, color : None, hover : None, fit : None, rotation :
                None, opacity : None, width : Some(wire::Length::Fixed(12.0f32)), height
                : Some(wire::Length::Fixed(12.0f32)), } }); } native::column(node_scope
                .clone(), children) } } },
                native::text_options(native::text(format!("{}/@text:464",
                message_contents_scope_2207), crate ::host::plural(thread_message
                .reply_count, ::std::convert::AsRef::as_ref(& "reply"),
                ::std::convert::AsRef::as_ref(& "replies"),).to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:458", message_contents_scope_2207), wrap
                : None, axis : wire::Axis::Row, spacing : Some(6.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 3.0f32, right : 9.0f32, bottom : 3.0f32, left : 7.0f32, },),),),
                label : Some(String::from("Open thread".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_720_22(thread_message
                .seq),),), width : None, height : None, padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), }];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:441", message_contents_scope_2207), wrap : None, axis
                : wire::Axis::Row, spacing : Some(6.0f32), padding : Some(wire::Edges {
                top : 6.0f32, right : 0.0f32, bottom : 0.0f32, left : 0.0f32, }), width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }); }
                native::spaced(native::sized(native::column(format!("{}/@layout:340",
                message_contents_scope_2207), children,), Some(wire::Length::Fill),
                None,), 2.0f32,) }); if thread_message.pending { children
                .push(native::padded(native::container(format!("{}/@container:506",
                message_contents_scope_2207), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:508",
                message_contents_scope_2207), thread_message.meta.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },), { let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "dot")),); wire::Node::Svg {
                inherit_button_ink : false, key : format!("{}/@media:514",
                message_contents_scope_2207), hash : hash, bytes : bytes, label : None,
                color : None, hover : None, fit : None, rotation : None, opacity :
                Some(1.0f32), width : Some(wire::Length::Fixed(6.0f32)), height :
                Some(wire::Length::Fixed(6.0f32)), } }]; wire::Node::Linear { max_width :
                None, clip : false, key : format!("{}/@layout:507",
                message_contents_scope_2207), wrap : None, axis : wire::Axis::Row,
                spacing : Some(5.0f32), padding : None, width : None, height : None,
                align : Some(wire::AlignX::Center), background : None, border : None,
                children : children, } },), wire::Edges { top : 0.0f32, right : 7.0f32,
                bottom : 0.0f32, left : 0.0f32, },),); } wire::Node::Linear { max_width :
                None, clip : false, key : format!("{}/@layout:331",
                message_contents_scope_2207), wrap : None, axis : wire::Axis::Row,
                spacing : Some(11.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Left), background : None, border : None, children :
                children, } }]; native::sized(native::column(format!("{}/@layout:330",
                message_contents_scope_2207), children,), Some(wire::Length::Fill),
                None,) } },), Some(wire::Length::Fill), None,), wire::Edges { top :
                4.0f32, right : 7.0f32, bottom : 4.0f32, left : 7.0f32, },),); }
                RowPlate::Selected => { children
                .push(native::padded(native::sized(native::container(format!("{}/@container:803",
                thread_message_card_scope_2752), { let message_contents_scope_2227 =
                format!("{}/MessageContents@2227", thread_message_card_scope_2752); { let
                children : Vec < wire::Node > = vec![{ let mut children : Vec <
                wire::Node > = Vec::new(); if thread_message.show_author { children
                .push({ let message_avatar_scope_1750 = format!("{}/MessageAvatar@1750",
                message_contents_scope_2227); { let node_scope = format!("{}/root",
                message_avatar_scope_1750); { let mut children : Vec < wire::Node > =
                Vec::new(); if thread_message.avatar_kind == "human" { children.push({
                let person_avatar_scope_798 = format!("{}/PersonAvatar@798",
                message_avatar_scope_1750); { let node_scope = format!("{}/root",
                person_avatar_scope_798); { let children : Vec < wire::Node > =
                vec![{ let principal_avatar_scope_863 = format!("{}/PrincipalAvatar@863",
                person_avatar_scope_798); { let node_scope = format!("{}/root",
                principal_avatar_scope_863); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let principal_plate_scope_909 =
                format!("{}/PrincipalPlate@909", principal_avatar_scope_863); { let
                node_scope = format!("{}/root", principal_plate_scope_909); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                human_plate_scope_925 = format!("{}/HumanPlate@925",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                human_plate_scope_925); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:328", human_plate_scope_925), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([((30.0 / 2.0) as f32).max(0.0).min(f32::MAX), ((30.0
                / 2.0) as f32).max(0.0).min(f32::MAX), ((30.0 / 2.0) as f32).max(0.0)
                .min(f32::MAX), ((30.0 / 2.0) as f32).max(0.0).min(f32::MAX),]), }), snap
                : None, content :
                Box::new(native::text_options(native::text(format!("{}/@text:336",
                human_plate_scope_925), thread_message.initial.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), }); } native::column(node_scope.clone(),
                children) } } }); } native::column(node_scope.clone(), children) } } });
                } native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } if !
                (thread_message.avatar_kind == "human") && thread_message.avatar_kind ==
                "agent" { children.push({ let agent_avatar_scope_804 =
                format!("{}/AgentAvatar@804", message_avatar_scope_1750); { let
                node_scope = format!("{}/root", agent_avatar_scope_804); { let children : Vec < wire::Node > = vec![{ let principal_avatar_scope_873 =
                format!("{}/PrincipalAvatar@873", agent_avatar_scope_804); { let
                node_scope = format!("{}/root", principal_avatar_scope_873); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                principal_plate_scope_909 = format!("{}/PrincipalPlate@909",
                principal_avatar_scope_873); { let node_scope = format!("{}/root",
                principal_plate_scope_909); { let children : Vec < wire::Node > =
                vec![{ let agent_plate_scope_919 = format!("{}/AgentPlate@919",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                agent_plate_scope_919); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let agent_square_scope_1174 =
                format!("{}/AgentSquare@1174", agent_plate_scope_919); { let node_scope =
                format!("{}/root", agent_square_scope_1174); wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : node_scope.clone(), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content :
                Box::new(native::text_options(native::text(format!("{}/@text:390",
                agent_square_scope_1174), thread_message.initial.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), } } }); } native::column(node_scope.clone(),
                children) } } }]; native::column(node_scope.clone(), children) } } }); }
                native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } if !
                (thread_message.avatar_kind == "human" || thread_message.avatar_kind ==
                "agent") { children.push({ let agent_avatar_scope_810 =
                format!("{}/AgentAvatar@810", message_avatar_scope_1750); { let
                node_scope = format!("{}/root", agent_avatar_scope_810); { let children : Vec < wire::Node > = vec![{ let principal_avatar_scope_873 =
                format!("{}/PrincipalAvatar@873", agent_avatar_scope_810); { let
                node_scope = format!("{}/root", principal_avatar_scope_873); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                principal_plate_scope_909 = format!("{}/PrincipalPlate@909",
                principal_avatar_scope_873); { let node_scope = format!("{}/root",
                principal_plate_scope_909); { let children : Vec < wire::Node > =
                vec![{ let agent_plate_scope_919 = format!("{}/AgentPlate@919",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                agent_plate_scope_919); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let agent_square_scope_1174 =
                format!("{}/AgentSquare@1174", agent_plate_scope_919); { let node_scope =
                format!("{}/root", agent_square_scope_1174); wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : node_scope.clone(), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content :
                Box::new(native::text_options(native::text(format!("{}/@text:390",
                agent_square_scope_1174), thread_message.initial.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), } } }); } native::column(node_scope.clone(),
                children) } } }]; native::column(node_scope.clone(), children) } } }); }
                native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } wire::Node::Stack
                { key : node_scope.clone(), width : Some(wire::Length::Fixed(30.0f32)),
                height : Some(wire::Length::Fixed(30.0f32)), padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, } }
                } }); } if ! thread_message.show_author { children.push(wire::Node::Space
                { width : Some(wire::Length::Fixed(30.0f32)), height : None, }); }
                children.push({ let mut children : Vec < wire::Node > = Vec::new(); if
                thread_message.show_author { children.push({ let mut children : Vec <
                wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:347",
                message_contents_scope_2227), thread_message.author.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; if thread_message.avatar_kind == "agent" {
                children
                .push(native::padded(native::container(format!("{}/@container:354",
                message_contents_scope_2227),
                native::text_options(native::text(format!("{}/@text:360",
                message_contents_scope_2227), "AGENT".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), wire::Edges { top : 2.0f32, right : 5.0f32,
                bottom : 2.0f32, left : 5.0f32, },),); } if thread_message.height > 0 {
                children.push(native::text_options(native::text(format!("{}/@text:377",
                message_contents_scope_2227), crate
                ::host::height_label_short(thread_message.height).to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } if thread_message.edited { children
                .push(native::text_options(native::text(format!("{}/@text:384",
                message_contents_scope_2227), "· edited".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } children.push(wire::Node::Space { width :
                Some(wire::Length::Fill), height : None, }); wire::Node::Linear {
                max_width : None, clip : false, key : format!("{}/@layout:342",
                message_contents_scope_2227), wrap : None, axis : wire::Axis::Row,
                spacing : Some(7.0f32), padding : None, width : Some(wire::Length::Fill),
                height : None, align : Some(wire::AlignX::Center), background : None,
                border : None, children : children, } }); } children
                .push(wire::Node::MouseArea { key : format!("{}/@mouse:395",
                message_contents_scope_2227), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_720_1(thread_message
                .seq, CopySurface::Thread.clone(),),),), on_release : None,
                on_double_click : None, on_right_press : None, on_right_release : None,
                on_middle_press : None, on_middle_release : None, on_enter : None,
                on_exit : None, on_move : None, on_press_at : None, on_scroll : None,
                content : Box::new({ let message_body_scope_1809 =
                format!("{}/MessageBody@1809", message_contents_scope_2227); { let children : Vec < wire::Node > = vec![{ let rich_body_scope_688 =
                format!("{}/RichBody@688", message_body_scope_1809); { let mut children :
                Vec < wire::Node > = Vec::new(); for (index, block) in thread_message
                .blocks.iter().enumerate() { let for_scope = format!("{}/@for:703({})",
                rich_body_scope_688, index); if block.kind == "divider" { children.push({
                let component_separator_scope_705 = format!("{}/Separator@705",
                for_scope); { let node_scope = format!("{}/root",
                component_separator_scope_705); wire::Node::Rule { key : node_scope
                .clone(), axis : wire::Axis::Row, thickness : 1.0f32, color : None, weak
                : false, radius : None, snap : None, } } }); } if block.kind == "code" {
                children
                .push(native::padded(native::sized(native::container(format!("{}/@container:90",
                for_scope), { let mut children : Vec < wire::Node > = Vec::new(); if !
                block.lang.is_empty() { children
                .push(native::text_options(native::text(format!("{}/@text:100",
                for_scope), block.lang.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },),); }
                children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:106",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),);
                native::spaced(native::sized(native::column(format!("{}/@layout:98",
                for_scope), children,), Some(wire::Length::Fill), None,), 6.0f32,) },),
                Some(wire::Length::Fill), None,), wire::Edges { top : 11.0f32, right :
                11.0f32, bottom : 11.0f32, left : 11.0f32, },),); } if block.kind ==
                "quote" { children.push({ let children : Vec < wire::Node > = vec![{
                let mut children : Vec < wire::Node > = Vec::new(); if block.rich {
                children.push({ let rich_line_scope_755 = format!("{}/RichLine@755",
                for_scope); { let mut rich_spans : Vec < wire::RichSpan > = Vec::new();
                for span in block.spans.iter().cloned() { rich_spans.push(wire::RichSpan
                { content : span.mention.to_owned().to_string(), size : None, line_height
                : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Medium,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : Some(span.mention_link.to_owned()), background :
                None, border : Some(wire::Border { color : None, width : None, radius :
                Some([4.0f32, 4.0f32, 4.0f32, 4.0f32,]), }), padding : Some(wire::Edges {
                top : 0.0f32, right : 1.0f32, bottom : 0.0f32, left : 1.0f32, }),
                underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.link_text.to_owned().to_string(),
                size : None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Medium,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : Some(span.link.to_owned()), background : None,
                border : None, padding : None, underline : true, strikethrough : false,
                }); rich_spans.push(wire::RichSpan { content : span.bold_italic
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Bold, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Italic, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.bold
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Bold, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Normal, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.italic
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Normal, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Italic, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.plain
                .to_owned().to_string(), size : None, line_height : None, font : None,
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); }
                wire::Node::RichText { options : wire::TextOptions { wrapping :
                Some(wire::Wrapping::WordOrGlyph), ..Default::default() }, key :
                format!("{}/@text:32", rich_line_scope_755), size : Some(13.5f32), color
                : None, font : wire::Font { monospace : false, weight :
                wire::Weight::Normal, }, width : Some(wire::Length::Fill), align_x :
                None, spans : rich_spans, on_link :
                Some(::ducktape_view_guest::slots::handler:: < String, Message, >
                (Box::new({ let route = { let route_callback = lazy_event_720_16.clone();
                move | link : String | route_callback(link) }; move | sent : String |
                Some(route(sent)) }),),), } } }); } if ! block.rich { children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:137",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),); }
                native::padded(native::sized(native::column(format!("{}/@layout:121",
                for_scope), children,), Some(wire::Length::Fill), None,), wire::Edges {
                top : 2.0f32, right : 0.0f32, bottom : 2.0f32, left : 13.0f32, },) },
                native::sized(native::container(format!("{}/@container:144", for_scope),
                wire::Node::Space { width : Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(1.0f32)), },),
                Some(wire::Length::Fixed(3.0f32)), Some(wire::Length::Fill),)];
                wire::Node::Stack { key : format!("{}/@layout:120", for_scope), width :
                Some(wire::Length::Fill), height : None, padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, }
                }); } if block.kind == "paragraph" { if block.rich { children.push({ let
                rich_line_scope_780 = format!("{}/RichLine@780", for_scope); { let mut
                rich_spans : Vec < wire::RichSpan > = Vec::new(); for span in block.spans
                .iter().cloned() { rich_spans.push(wire::RichSpan { content : span
                .mention.to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Medium, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), color : None, link : Some(span
                .mention_link.to_owned()), background : None, border : Some(wire::Border
                { color : None, width : None, radius : Some([4.0f32, 4.0f32, 4.0f32,
                4.0f32,]), }), padding : Some(wire::Edges { top : 0.0f32, right : 1.0f32,
                bottom : 0.0f32, left : 1.0f32, }), underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.link_text
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Medium, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), color : None, link : Some(span.link
                .to_owned()), background : None, border : None, padding : None, underline
                : true, strikethrough : false, }); rich_spans.push(wire::RichSpan {
                content : span.bold_italic.to_owned().to_string(), size : None,
                line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Bold,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Italic, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.bold.to_owned().to_string(), size :
                None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Bold,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.italic.to_owned().to_string(), size
                : None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Normal,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Italic, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.plain.to_owned().to_string(), size
                : None, line_height : None, font : None, color : None, link : None,
                background : None, border : None, padding : None, underline : false,
                strikethrough : false, }); } wire::Node::RichText { options :
                wire::TextOptions { wrapping : Some(wire::Wrapping::WordOrGlyph),
                ..Default::default() }, key : format!("{}/@text:32",
                rich_line_scope_780), size : Some(13.5f32), color : None, font :
                wire::Font { monospace : false, weight : wire::Weight::Normal, }, width :
                Some(wire::Length::Fill), align_x : None, spans : rich_spans, on_link :
                Some(::ducktape_view_guest::slots::handler:: < String, Message, >
                (Box::new({ let route = { let route_callback = lazy_event_720_16.clone();
                move | link : String | route_callback(link) }; move | sent : String |
                Some(route(sent)) }),),), } } }); } if ! block.rich { children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:157",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),); } } }
                native::spaced(native::sized(native::column(format!("{}/@layout:75",
                rich_body_scope_688), children,), Some(wire::Length::Fill), None,),
                5.0f32,) } }]; wire::Node::Linear { max_width : Some(760.0f32), clip :
                false, key : format!("{}/@layout:60", message_body_scope_1809), wrap :
                None, axis : wire::Axis::Column, spacing : None, padding : None, width :
                Some(wire::Length::Fill), height : None, align : None, background : None,
                border : None, children : children, } } }), }); if thread_message.edited
                && ! thread_message.show_author { children
                .push(native::text_options(native::text(format!("{}/@text:408",
                message_contents_scope_2227), "· edited".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } if ! crate
                ::host::run_of_message(::std::convert::AsRef::as_ref(& thread_message
                .id),).is_empty() { children.push({ let children : Vec < wire::Node >
                = vec![native::padded(native::button(format!("{}/@button:420",
                message_contents_scope_2227), String::from("View run"),
                Some(::ducktape_view_guest::slots::message(lazy_event_720_17(crate
                ::host::run_of_message(::std::convert::AsRef::as_ref(& thread_message
                .id),),),),), wire::ButtonPreset::Secondary,),
                wire::Edges::all(3.0f32),)];
                native::padded(native::sized(native::row(format!("{}/@layout:419",
                message_contents_scope_2227), children,), Some(wire::Length::Fill),
                None,), wire::Edges { top : 4.0f32, right : 0.0f32, bottom : 0.0f32, left
                : 0.0f32, },) }); } if ! thread_message.reactions.is_empty() { children
                .push({ let mut items = Vec::new(); for (index, reaction) in
                thread_message.reactions.iter().enumerate() { let for_scope =
                format!("{}/@for:1848({})", message_contents_scope_2227, index); let
                flex_child : wire::Node = { let reaction_chip_scope_1849 =
                format!("{}/ReactionChip@1849", for_scope); { let node_scope =
                format!("{}/root", reaction_chip_scope_1849); { let mut children : Vec <
                wire::Node > = Vec::new(); if reaction.reacted_by_me { children
                .push(wire::Node::Button { checked : Some(reaction.reacted_by_me),
                expanded : None, description : Some(String::from(reaction.emoji
                .to_owned())), key : format!("{}/@button:218", reaction_chip_scope_1849),
                content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:225",
                reaction_chip_scope_1849), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:232",
                reaction_chip_scope_1849), reaction.emoji.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:238",
                reaction_chip_scope_1849), reaction.count.to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:231", reaction_chip_scope_1849), wrap :
                None, axis : wire::Axis::Row, spacing : Some(4.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 1.0f32, right : 8.0f32, bottom : 1.0f32, left : 6.0f32, },),),),
                label : Some(String::from("Remove reaction".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_720_21(thread_message
                .seq, reaction.emoji.to_owned(),),),), width : None, height : None,
                padding : Some(wire::Edges::all(0.0f32)), style :
                wire::ButtonStyle::default(), }); } if ! reaction.reacted_by_me {
                children.push(wire::Node::Button { checked : Some(reaction
                .reacted_by_me), expanded : None, description :
                Some(String::from(reaction.emoji.to_owned())), key :
                format!("{}/@button:262", reaction_chip_scope_1849), content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:269",
                reaction_chip_scope_1849), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:276",
                reaction_chip_scope_1849), reaction.emoji.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:282",
                reaction_chip_scope_1849), reaction.count.to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:275", reaction_chip_scope_1849), wrap :
                None, axis : wire::Axis::Row, spacing : Some(4.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 1.0f32, right : 8.0f32, bottom : 1.0f32, left : 6.0f32, },),),),
                label : Some(String::from("Add reaction".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_720_20(thread_message
                .seq, reaction.emoji.to_owned(),),),), width : None, height : None,
                padding : Some(wire::Edges::all(0.0f32)), style :
                wire::ButtonStyle::default(), }); } native::column(node_scope.clone(),
                children) } } }; items.push((wire::FlexItem::default(), flex_child)); }
                let (items, children) = items.into_iter().unzip(); wire::Node::Flex { key
                : format!("{}/@layout:427", message_contents_scope_2227), items,
                children, background : None, border : None, layout : wire::FlexLayout {
                direction : wire::FlexDirection::Row, wrap : wire::FlexWrap::Wrap,
                justify : None, items : Some(wire::FlexItemAlignment::Start), content :
                None, row_gap : Some(5.0f32), column_gap : Some(5.0f32), padding :
                Some(wire::Edges { top : 6.0f32, right : 0.0f32, bottom : 0.0f32, left :
                0.0f32, }), width : Some(wire::Length::Fill), height : None, max_width :
                None, max_height : None, clip : false, surface_width : None,
                surface_height : None, surface_max_width : None, }, } }); } if
                thread_message.reply_count > 0 { children.push({ let children : Vec <
                wire::Node > = vec![wire::Node::Button { checked : None, expanded : None,
                description : None, key : format!("{}/@button:447",
                message_contents_scope_2227), content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:452",
                message_contents_scope_2227), { let children : Vec < wire::Node > =
                vec![{ let component_icon_scope_1872 = format!("{}/Icon@1872",
                message_contents_scope_2227); { let node_scope = format!("{}/root",
                component_icon_scope_1872); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "nav-chat"),),);
                wire::Node::Svg { inherit_button_ink : false, key :
                format!("{}/@media:52", component_icon_scope_1872), hash : hash, bytes :
                bytes, label : None, color : None, hover : None, fit : None, rotation :
                None, opacity : None, width : Some(wire::Length::Fixed(12.0f32)), height
                : Some(wire::Length::Fixed(12.0f32)), } }); } native::column(node_scope
                .clone(), children) } } },
                native::text_options(native::text(format!("{}/@text:464",
                message_contents_scope_2227), crate ::host::plural(thread_message
                .reply_count, ::std::convert::AsRef::as_ref(& "reply"),
                ::std::convert::AsRef::as_ref(& "replies"),).to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:458", message_contents_scope_2227), wrap
                : None, axis : wire::Axis::Row, spacing : Some(6.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 3.0f32, right : 9.0f32, bottom : 3.0f32, left : 7.0f32, },),),),
                label : Some(String::from("Open thread".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_720_22(thread_message
                .seq),),), width : None, height : None, padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), }];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:441", message_contents_scope_2227), wrap : None, axis
                : wire::Axis::Row, spacing : Some(6.0f32), padding : Some(wire::Edges {
                top : 6.0f32, right : 0.0f32, bottom : 0.0f32, left : 0.0f32, }), width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }); }
                native::spaced(native::sized(native::column(format!("{}/@layout:340",
                message_contents_scope_2227), children,), Some(wire::Length::Fill),
                None,), 2.0f32,) }); if thread_message.pending { children
                .push(native::padded(native::container(format!("{}/@container:506",
                message_contents_scope_2227), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:508",
                message_contents_scope_2227), thread_message.meta.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },), { let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "dot")),); wire::Node::Svg {
                inherit_button_ink : false, key : format!("{}/@media:514",
                message_contents_scope_2227), hash : hash, bytes : bytes, label : None,
                color : None, hover : None, fit : None, rotation : None, opacity :
                Some(1.0f32), width : Some(wire::Length::Fixed(6.0f32)), height :
                Some(wire::Length::Fixed(6.0f32)), } }]; wire::Node::Linear { max_width :
                None, clip : false, key : format!("{}/@layout:507",
                message_contents_scope_2227), wrap : None, axis : wire::Axis::Row,
                spacing : Some(5.0f32), padding : None, width : None, height : None,
                align : Some(wire::AlignX::Center), background : None, border : None,
                children : children, } },), wire::Edges { top : 0.0f32, right : 7.0f32,
                bottom : 0.0f32, left : 0.0f32, },),); } wire::Node::Linear { max_width :
                None, clip : false, key : format!("{}/@layout:331",
                message_contents_scope_2227), wrap : None, axis : wire::Axis::Row,
                spacing : Some(11.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Left), background : None, border : None, children :
                children, } }]; native::sized(native::column(format!("{}/@layout:330",
                message_contents_scope_2227), children,), Some(wire::Length::Fill),
                None,) } },), Some(wire::Length::Fill), None,), wire::Edges { top :
                4.0f32, right : 7.0f32, bottom : 4.0f32, left : 7.0f32, },),); }
                RowPlate::Ranged => { children
                .push(native::padded(native::sized(native::container(format!("{}/@container:823",
                thread_message_card_scope_2752), { let message_contents_scope_2247 =
                format!("{}/MessageContents@2247", thread_message_card_scope_2752); { let
                children : Vec < wire::Node > = vec![{ let mut children : Vec <
                wire::Node > = Vec::new(); if thread_message.show_author { children
                .push({ let message_avatar_scope_1750 = format!("{}/MessageAvatar@1750",
                message_contents_scope_2247); { let node_scope = format!("{}/root",
                message_avatar_scope_1750); { let mut children : Vec < wire::Node > =
                Vec::new(); if thread_message.avatar_kind == "human" { children.push({
                let person_avatar_scope_798 = format!("{}/PersonAvatar@798",
                message_avatar_scope_1750); { let node_scope = format!("{}/root",
                person_avatar_scope_798); { let children : Vec < wire::Node > =
                vec![{ let principal_avatar_scope_863 = format!("{}/PrincipalAvatar@863",
                person_avatar_scope_798); { let node_scope = format!("{}/root",
                principal_avatar_scope_863); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let principal_plate_scope_909 =
                format!("{}/PrincipalPlate@909", principal_avatar_scope_863); { let
                node_scope = format!("{}/root", principal_plate_scope_909); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                human_plate_scope_925 = format!("{}/HumanPlate@925",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                human_plate_scope_925); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:328", human_plate_scope_925), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([((30.0 / 2.0) as f32).max(0.0).min(f32::MAX), ((30.0
                / 2.0) as f32).max(0.0).min(f32::MAX), ((30.0 / 2.0) as f32).max(0.0)
                .min(f32::MAX), ((30.0 / 2.0) as f32).max(0.0).min(f32::MAX),]), }), snap
                : None, content :
                Box::new(native::text_options(native::text(format!("{}/@text:336",
                human_plate_scope_925), thread_message.initial.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), }); } native::column(node_scope.clone(),
                children) } } }); } native::column(node_scope.clone(), children) } } });
                } native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } if !
                (thread_message.avatar_kind == "human") && thread_message.avatar_kind ==
                "agent" { children.push({ let agent_avatar_scope_804 =
                format!("{}/AgentAvatar@804", message_avatar_scope_1750); { let
                node_scope = format!("{}/root", agent_avatar_scope_804); { let children : Vec < wire::Node > = vec![{ let principal_avatar_scope_873 =
                format!("{}/PrincipalAvatar@873", agent_avatar_scope_804); { let
                node_scope = format!("{}/root", principal_avatar_scope_873); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                principal_plate_scope_909 = format!("{}/PrincipalPlate@909",
                principal_avatar_scope_873); { let node_scope = format!("{}/root",
                principal_plate_scope_909); { let children : Vec < wire::Node > =
                vec![{ let agent_plate_scope_919 = format!("{}/AgentPlate@919",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                agent_plate_scope_919); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let agent_square_scope_1174 =
                format!("{}/AgentSquare@1174", agent_plate_scope_919); { let node_scope =
                format!("{}/root", agent_square_scope_1174); wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : node_scope.clone(), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content :
                Box::new(native::text_options(native::text(format!("{}/@text:390",
                agent_square_scope_1174), thread_message.initial.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), } } }); } native::column(node_scope.clone(),
                children) } } }]; native::column(node_scope.clone(), children) } } }); }
                native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } if !
                (thread_message.avatar_kind == "human" || thread_message.avatar_kind ==
                "agent") { children.push({ let agent_avatar_scope_810 =
                format!("{}/AgentAvatar@810", message_avatar_scope_1750); { let
                node_scope = format!("{}/root", agent_avatar_scope_810); { let children : Vec < wire::Node > = vec![{ let principal_avatar_scope_873 =
                format!("{}/PrincipalAvatar@873", agent_avatar_scope_810); { let
                node_scope = format!("{}/root", principal_avatar_scope_873); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                principal_plate_scope_909 = format!("{}/PrincipalPlate@909",
                principal_avatar_scope_873); { let node_scope = format!("{}/root",
                principal_plate_scope_909); { let children : Vec < wire::Node > =
                vec![{ let agent_plate_scope_919 = format!("{}/AgentPlate@919",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                agent_plate_scope_919); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let agent_square_scope_1174 =
                format!("{}/AgentSquare@1174", agent_plate_scope_919); { let node_scope =
                format!("{}/root", agent_square_scope_1174); wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : node_scope.clone(), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content :
                Box::new(native::text_options(native::text(format!("{}/@text:390",
                agent_square_scope_1174), thread_message.initial.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), } } }); } native::column(node_scope.clone(),
                children) } } }]; native::column(node_scope.clone(), children) } } }); }
                native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } wire::Node::Stack
                { key : node_scope.clone(), width : Some(wire::Length::Fixed(30.0f32)),
                height : Some(wire::Length::Fixed(30.0f32)), padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, } }
                } }); } if ! thread_message.show_author { children.push(wire::Node::Space
                { width : Some(wire::Length::Fixed(30.0f32)), height : None, }); }
                children.push({ let mut children : Vec < wire::Node > = Vec::new(); if
                thread_message.show_author { children.push({ let mut children : Vec <
                wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:347",
                message_contents_scope_2247), thread_message.author.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; if thread_message.avatar_kind == "agent" {
                children
                .push(native::padded(native::container(format!("{}/@container:354",
                message_contents_scope_2247),
                native::text_options(native::text(format!("{}/@text:360",
                message_contents_scope_2247), "AGENT".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), wire::Edges { top : 2.0f32, right : 5.0f32,
                bottom : 2.0f32, left : 5.0f32, },),); } if thread_message.height > 0 {
                children.push(native::text_options(native::text(format!("{}/@text:377",
                message_contents_scope_2247), crate
                ::host::height_label_short(thread_message.height).to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } if thread_message.edited { children
                .push(native::text_options(native::text(format!("{}/@text:384",
                message_contents_scope_2247), "· edited".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } children.push(wire::Node::Space { width :
                Some(wire::Length::Fill), height : None, }); wire::Node::Linear {
                max_width : None, clip : false, key : format!("{}/@layout:342",
                message_contents_scope_2247), wrap : None, axis : wire::Axis::Row,
                spacing : Some(7.0f32), padding : None, width : Some(wire::Length::Fill),
                height : None, align : Some(wire::AlignX::Center), background : None,
                border : None, children : children, } }); } children
                .push(wire::Node::MouseArea { key : format!("{}/@mouse:395",
                message_contents_scope_2247), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_720_1(thread_message
                .seq, CopySurface::Thread.clone(),),),), on_release : None,
                on_double_click : None, on_right_press : None, on_right_release : None,
                on_middle_press : None, on_middle_release : None, on_enter : None,
                on_exit : None, on_move : None, on_press_at : None, on_scroll : None,
                content : Box::new({ let message_body_scope_1809 =
                format!("{}/MessageBody@1809", message_contents_scope_2247); { let children : Vec < wire::Node > = vec![{ let rich_body_scope_688 =
                format!("{}/RichBody@688", message_body_scope_1809); { let mut children :
                Vec < wire::Node > = Vec::new(); for (index, block) in thread_message
                .blocks.iter().enumerate() { let for_scope = format!("{}/@for:703({})",
                rich_body_scope_688, index); if block.kind == "divider" { children.push({
                let component_separator_scope_705 = format!("{}/Separator@705",
                for_scope); { let node_scope = format!("{}/root",
                component_separator_scope_705); wire::Node::Rule { key : node_scope
                .clone(), axis : wire::Axis::Row, thickness : 1.0f32, color : None, weak
                : false, radius : None, snap : None, } } }); } if block.kind == "code" {
                children
                .push(native::padded(native::sized(native::container(format!("{}/@container:90",
                for_scope), { let mut children : Vec < wire::Node > = Vec::new(); if !
                block.lang.is_empty() { children
                .push(native::text_options(native::text(format!("{}/@text:100",
                for_scope), block.lang.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },),); }
                children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:106",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),);
                native::spaced(native::sized(native::column(format!("{}/@layout:98",
                for_scope), children,), Some(wire::Length::Fill), None,), 6.0f32,) },),
                Some(wire::Length::Fill), None,), wire::Edges { top : 11.0f32, right :
                11.0f32, bottom : 11.0f32, left : 11.0f32, },),); } if block.kind ==
                "quote" { children.push({ let children : Vec < wire::Node > = vec![{
                let mut children : Vec < wire::Node > = Vec::new(); if block.rich {
                children.push({ let rich_line_scope_755 = format!("{}/RichLine@755",
                for_scope); { let mut rich_spans : Vec < wire::RichSpan > = Vec::new();
                for span in block.spans.iter().cloned() { rich_spans.push(wire::RichSpan
                { content : span.mention.to_owned().to_string(), size : None, line_height
                : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Medium,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : Some(span.mention_link.to_owned()), background :
                None, border : Some(wire::Border { color : None, width : None, radius :
                Some([4.0f32, 4.0f32, 4.0f32, 4.0f32,]), }), padding : Some(wire::Edges {
                top : 0.0f32, right : 1.0f32, bottom : 0.0f32, left : 1.0f32, }),
                underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.link_text.to_owned().to_string(),
                size : None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Medium,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : Some(span.link.to_owned()), background : None,
                border : None, padding : None, underline : true, strikethrough : false,
                }); rich_spans.push(wire::RichSpan { content : span.bold_italic
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Bold, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Italic, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.bold
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Bold, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Normal, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.italic
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Normal, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Italic, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.plain
                .to_owned().to_string(), size : None, line_height : None, font : None,
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); }
                wire::Node::RichText { options : wire::TextOptions { wrapping :
                Some(wire::Wrapping::WordOrGlyph), ..Default::default() }, key :
                format!("{}/@text:32", rich_line_scope_755), size : Some(13.5f32), color
                : None, font : wire::Font { monospace : false, weight :
                wire::Weight::Normal, }, width : Some(wire::Length::Fill), align_x :
                None, spans : rich_spans, on_link :
                Some(::ducktape_view_guest::slots::handler:: < String, Message, >
                (Box::new({ let route = { let route_callback = lazy_event_720_16.clone();
                move | link : String | route_callback(link) }; move | sent : String |
                Some(route(sent)) }),),), } } }); } if ! block.rich { children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:137",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),); }
                native::padded(native::sized(native::column(format!("{}/@layout:121",
                for_scope), children,), Some(wire::Length::Fill), None,), wire::Edges {
                top : 2.0f32, right : 0.0f32, bottom : 2.0f32, left : 13.0f32, },) },
                native::sized(native::container(format!("{}/@container:144", for_scope),
                wire::Node::Space { width : Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(1.0f32)), },),
                Some(wire::Length::Fixed(3.0f32)), Some(wire::Length::Fill),)];
                wire::Node::Stack { key : format!("{}/@layout:120", for_scope), width :
                Some(wire::Length::Fill), height : None, padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, }
                }); } if block.kind == "paragraph" { if block.rich { children.push({ let
                rich_line_scope_780 = format!("{}/RichLine@780", for_scope); { let mut
                rich_spans : Vec < wire::RichSpan > = Vec::new(); for span in block.spans
                .iter().cloned() { rich_spans.push(wire::RichSpan { content : span
                .mention.to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Medium, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), color : None, link : Some(span
                .mention_link.to_owned()), background : None, border : Some(wire::Border
                { color : None, width : None, radius : Some([4.0f32, 4.0f32, 4.0f32,
                4.0f32,]), }), padding : Some(wire::Edges { top : 0.0f32, right : 1.0f32,
                bottom : 0.0f32, left : 1.0f32, }), underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.link_text
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Medium, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), color : None, link : Some(span.link
                .to_owned()), background : None, border : None, padding : None, underline
                : true, strikethrough : false, }); rich_spans.push(wire::RichSpan {
                content : span.bold_italic.to_owned().to_string(), size : None,
                line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Bold,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Italic, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.bold.to_owned().to_string(), size :
                None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Bold,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.italic.to_owned().to_string(), size
                : None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Normal,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Italic, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.plain.to_owned().to_string(), size
                : None, line_height : None, font : None, color : None, link : None,
                background : None, border : None, padding : None, underline : false,
                strikethrough : false, }); } wire::Node::RichText { options :
                wire::TextOptions { wrapping : Some(wire::Wrapping::WordOrGlyph),
                ..Default::default() }, key : format!("{}/@text:32",
                rich_line_scope_780), size : Some(13.5f32), color : None, font :
                wire::Font { monospace : false, weight : wire::Weight::Normal, }, width :
                Some(wire::Length::Fill), align_x : None, spans : rich_spans, on_link :
                Some(::ducktape_view_guest::slots::handler:: < String, Message, >
                (Box::new({ let route = { let route_callback = lazy_event_720_16.clone();
                move | link : String | route_callback(link) }; move | sent : String |
                Some(route(sent)) }),),), } } }); } if ! block.rich { children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:157",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),); } } }
                native::spaced(native::sized(native::column(format!("{}/@layout:75",
                rich_body_scope_688), children,), Some(wire::Length::Fill), None,),
                5.0f32,) } }]; wire::Node::Linear { max_width : Some(760.0f32), clip :
                false, key : format!("{}/@layout:60", message_body_scope_1809), wrap :
                None, axis : wire::Axis::Column, spacing : None, padding : None, width :
                Some(wire::Length::Fill), height : None, align : None, background : None,
                border : None, children : children, } } }), }); if thread_message.edited
                && ! thread_message.show_author { children
                .push(native::text_options(native::text(format!("{}/@text:408",
                message_contents_scope_2247), "· edited".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } if ! crate
                ::host::run_of_message(::std::convert::AsRef::as_ref(& thread_message
                .id),).is_empty() { children.push({ let children : Vec < wire::Node >
                = vec![native::padded(native::button(format!("{}/@button:420",
                message_contents_scope_2247), String::from("View run"),
                Some(::ducktape_view_guest::slots::message(lazy_event_720_17(crate
                ::host::run_of_message(::std::convert::AsRef::as_ref(& thread_message
                .id),),),),), wire::ButtonPreset::Secondary,),
                wire::Edges::all(3.0f32),)];
                native::padded(native::sized(native::row(format!("{}/@layout:419",
                message_contents_scope_2247), children,), Some(wire::Length::Fill),
                None,), wire::Edges { top : 4.0f32, right : 0.0f32, bottom : 0.0f32, left
                : 0.0f32, },) }); } if ! thread_message.reactions.is_empty() { children
                .push({ let mut items = Vec::new(); for (index, reaction) in
                thread_message.reactions.iter().enumerate() { let for_scope =
                format!("{}/@for:1848({})", message_contents_scope_2247, index); let
                flex_child : wire::Node = { let reaction_chip_scope_1849 =
                format!("{}/ReactionChip@1849", for_scope); { let node_scope =
                format!("{}/root", reaction_chip_scope_1849); { let mut children : Vec <
                wire::Node > = Vec::new(); if reaction.reacted_by_me { children
                .push(wire::Node::Button { checked : Some(reaction.reacted_by_me),
                expanded : None, description : Some(String::from(reaction.emoji
                .to_owned())), key : format!("{}/@button:218", reaction_chip_scope_1849),
                content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:225",
                reaction_chip_scope_1849), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:232",
                reaction_chip_scope_1849), reaction.emoji.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:238",
                reaction_chip_scope_1849), reaction.count.to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:231", reaction_chip_scope_1849), wrap :
                None, axis : wire::Axis::Row, spacing : Some(4.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 1.0f32, right : 8.0f32, bottom : 1.0f32, left : 6.0f32, },),),),
                label : Some(String::from("Remove reaction".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_720_21(thread_message
                .seq, reaction.emoji.to_owned(),),),), width : None, height : None,
                padding : Some(wire::Edges::all(0.0f32)), style :
                wire::ButtonStyle::default(), }); } if ! reaction.reacted_by_me {
                children.push(wire::Node::Button { checked : Some(reaction
                .reacted_by_me), expanded : None, description :
                Some(String::from(reaction.emoji.to_owned())), key :
                format!("{}/@button:262", reaction_chip_scope_1849), content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:269",
                reaction_chip_scope_1849), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:276",
                reaction_chip_scope_1849), reaction.emoji.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:282",
                reaction_chip_scope_1849), reaction.count.to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:275", reaction_chip_scope_1849), wrap :
                None, axis : wire::Axis::Row, spacing : Some(4.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 1.0f32, right : 8.0f32, bottom : 1.0f32, left : 6.0f32, },),),),
                label : Some(String::from("Add reaction".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_720_20(thread_message
                .seq, reaction.emoji.to_owned(),),),), width : None, height : None,
                padding : Some(wire::Edges::all(0.0f32)), style :
                wire::ButtonStyle::default(), }); } native::column(node_scope.clone(),
                children) } } }; items.push((wire::FlexItem::default(), flex_child)); }
                let (items, children) = items.into_iter().unzip(); wire::Node::Flex { key
                : format!("{}/@layout:427", message_contents_scope_2247), items,
                children, background : None, border : None, layout : wire::FlexLayout {
                direction : wire::FlexDirection::Row, wrap : wire::FlexWrap::Wrap,
                justify : None, items : Some(wire::FlexItemAlignment::Start), content :
                None, row_gap : Some(5.0f32), column_gap : Some(5.0f32), padding :
                Some(wire::Edges { top : 6.0f32, right : 0.0f32, bottom : 0.0f32, left :
                0.0f32, }), width : Some(wire::Length::Fill), height : None, max_width :
                None, max_height : None, clip : false, surface_width : None,
                surface_height : None, surface_max_width : None, }, } }); } if
                thread_message.reply_count > 0 { children.push({ let children : Vec <
                wire::Node > = vec![wire::Node::Button { checked : None, expanded : None,
                description : None, key : format!("{}/@button:447",
                message_contents_scope_2247), content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:452",
                message_contents_scope_2247), { let children : Vec < wire::Node > =
                vec![{ let component_icon_scope_1872 = format!("{}/Icon@1872",
                message_contents_scope_2247); { let node_scope = format!("{}/root",
                component_icon_scope_1872); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "nav-chat"),),);
                wire::Node::Svg { inherit_button_ink : false, key :
                format!("{}/@media:52", component_icon_scope_1872), hash : hash, bytes :
                bytes, label : None, color : None, hover : None, fit : None, rotation :
                None, opacity : None, width : Some(wire::Length::Fixed(12.0f32)), height
                : Some(wire::Length::Fixed(12.0f32)), } }); } native::column(node_scope
                .clone(), children) } } },
                native::text_options(native::text(format!("{}/@text:464",
                message_contents_scope_2247), crate ::host::plural(thread_message
                .reply_count, ::std::convert::AsRef::as_ref(& "reply"),
                ::std::convert::AsRef::as_ref(& "replies"),).to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:458", message_contents_scope_2247), wrap
                : None, axis : wire::Axis::Row, spacing : Some(6.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 3.0f32, right : 9.0f32, bottom : 3.0f32, left : 7.0f32, },),),),
                label : Some(String::from("Open thread".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_720_22(thread_message
                .seq),),), width : None, height : None, padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), }];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:441", message_contents_scope_2247), wrap : None, axis
                : wire::Axis::Row, spacing : Some(6.0f32), padding : Some(wire::Edges {
                top : 6.0f32, right : 0.0f32, bottom : 0.0f32, left : 0.0f32, }), width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }); }
                native::spaced(native::sized(native::column(format!("{}/@layout:340",
                message_contents_scope_2247), children,), Some(wire::Length::Fill),
                None,), 2.0f32,) }); if thread_message.pending { children
                .push(native::padded(native::container(format!("{}/@container:506",
                message_contents_scope_2247), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:508",
                message_contents_scope_2247), thread_message.meta.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },), { let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "dot")),); wire::Node::Svg {
                inherit_button_ink : false, key : format!("{}/@media:514",
                message_contents_scope_2247), hash : hash, bytes : bytes, label : None,
                color : None, hover : None, fit : None, rotation : None, opacity :
                Some(1.0f32), width : Some(wire::Length::Fixed(6.0f32)), height :
                Some(wire::Length::Fixed(6.0f32)), } }]; wire::Node::Linear { max_width :
                None, clip : false, key : format!("{}/@layout:507",
                message_contents_scope_2247), wrap : None, axis : wire::Axis::Row,
                spacing : Some(5.0f32), padding : None, width : None, height : None,
                align : Some(wire::AlignX::Center), background : None, border : None,
                children : children, } },), wire::Edges { top : 0.0f32, right : 7.0f32,
                bottom : 0.0f32, left : 0.0f32, },),); } wire::Node::Linear { max_width :
                None, clip : false, key : format!("{}/@layout:331",
                message_contents_scope_2247), wrap : None, axis : wire::Axis::Row,
                spacing : Some(11.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Left), background : None, border : None, children :
                children, } }]; native::sized(native::column(format!("{}/@layout:330",
                message_contents_scope_2247), children,), Some(wire::Length::Fill),
                None,) } },), Some(wire::Length::Fill), None,), wire::Edges { top :
                4.0f32, right : 7.0f32, bottom : 4.0f32, left : 7.0f32, },),); } }
                wire::Node::Stack { key : format!("{}/@layout:778",
                thread_message_card_scope_2752), width : Some(wire::Length::Fill), height
                : None, padding : None, background : None, border : None, clip : false,
                under : 0u32, children : children, } }, { let mut children : Vec <
                wire::Node > = Vec::new(); if ! thread_message.deleted && !
                thread_message.pending { children.push(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:844", thread_message_card_scope_2752), width
                : Some(wire::Length::Fill), height : None, padding : Some(wire::Edges {
                top : 0.0f32, right : 8.0f32, bottom : 0.0f32, left : 0.0f32, }), align_x
                : Some(wire::AlignX::Right), align_y : Some(wire::AlignY::Top),
                background : None.map(wire::Background::Color), border : None, snap :
                None, content :
                Box::new(native::padded(native::container(format!("{}/@container:854",
                thread_message_card_scope_2752), { let children : Vec < wire::Node >
                = vec![wire::Node::Button { checked : None, expanded : None, description
                : None, key : format!("{}/@button:865", thread_message_card_scope_2752),
                content :
                wire::ButtonContent::Child(Box::new(native::text(format!("{}/@text:872",
                thread_message_card_scope_2752), "♡".to_owned().to_string(),),),),
                label : Some(String::from("Manage reactions".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_720_40(thread_message
                .seq, thread_message.body.to_owned(), thread_message.rev,),),), width :
                Some(wire::Length::Fixed(27.0f32)), height :
                Some(wire::Length::Fixed(25.0f32)), padding :
                Some(wire::Edges::all(4.0f32)), style : wire::ButtonStyle::default(), },
                wire::Node::Button { checked : None, expanded : None, description : None,
                key : format!("{}/@button:876", thread_message_card_scope_2752), content
                :
                wire::ButtonContent::Child(Box::new(native::text(format!("{}/@text:883",
                thread_message_card_scope_2752), "⋯".to_owned().to_string(),),),),
                label : Some(String::from("More message actions".to_owned()),), on_press
                :
                Some(::ducktape_view_guest::slots::message(lazy_event_720_39(thread_message
                .seq, thread_message.body.to_owned(), thread_message.rev,),),), width :
                Some(wire::Length::Fixed(27.0f32)), height :
                Some(wire::Length::Fixed(25.0f32)), padding :
                Some(wire::Edges::all(4.0f32)), style : wire::ButtonStyle::default(), }];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:864", thread_message_card_scope_2752), wrap : None,
                axis : wire::Axis::Row, spacing : Some(1.0f32), padding : None, width :
                None, height : None, align : Some(wire::AlignX::Center), background :
                None, border : None, children : children, } },), wire::Edges { top :
                2.0f32, right : 2.0f32, bottom : 2.0f32, left : 2.0f32, },),), }); } if
                thread_message.deleted || thread_message.pending { children
                .push(wire::Node::Space { width : Some(wire::Length::Fixed(1.0f32)),
                height : Some(wire::Length::Fixed(1.0f32)), }); }
                native::sized(native::column(format!("{}/@layout:842",
                thread_message_card_scope_2752), children,), Some(wire::Length::Fill),
                None,) }]; wire::Node::Hover { key : format!("{}/@layout:773",
                thread_message_card_scope_2752), width : None, height : None, padding :
                None, background : None, border : None, tint : None, radius : 9.0f32,
                open : thread_message.seq == thread_selected_seq, children : children, }
                }); native::sized(native::column(format!("{}/@layout:767",
                thread_message_card_scope_2752), children,), Some(wire::Length::Fill),
                None,) } }); } if thread_message.seq != active_thread_seq &&
                thread_message.seq != thread_target_seq && thread_message.seq !=
                thread_selected_seq { children.push({ let _lazy_context_424 =
                thread_timeline_scope_4354.to_owned(); let lazy_event_424_0 =
                lazy_event_720_20.clone(); let lazy_event_424_1 = lazy_event_720_21
                .clone(); let lazy_event_424_2 = lazy_event_720_22.clone(); let
                lazy_event_424_3 = lazy_event_720_39.clone(); let lazy_event_424_4 =
                lazy_event_720_40.clone(); let lazy_event_424_5 = lazy_event_720_16
                .clone(); let lazy_event_424_6 = lazy_event_720_17.clone(); let
                lazy_event_424_7 = lazy_event_720_1.clone(); { let lazy_key =
                format!("{}/@lazy:165", key_recon);
                ::ducktape_view_guest::memo_lazy((thread_message.clone(),
                copy_anchor_seq, copy_head_seq, copy_surface.clone(),
                format!("{}/key({})", thread_timeline_scope_4354, key) .to_owned(), match
                self.active_palette { AppTheme::App => "app", AppTheme::AppDark =>
                "app-dark", },), move | dependency | { let cached_reply : crate
                ::host::ChatMessage = dependency.0.clone(); let copy_anchor_seq : i64 =
                dependency.1.clone(); let copy_head_seq : i64 = dependency.2.clone(); let
                copy_surface : CopySurface = dependency.3.clone(); let lazy_scope =
                dependency.4.clone(); { let thread_message_card_scope_2769 =
                format!("{}/ThreadMessageCard@2769", lazy_scope); { let mut children :
                Vec < wire::Node > = Vec::new(); if cached_reply.show_author { children
                .push(wire::Node::Space { width : Some(wire::Length::Fixed(1.0f32)),
                height : Some(wire::Length::Fixed(14.0f32)), }); } children.push({ let
                children : Vec < wire::Node > = vec![{ let mut children : Vec <
                wire::Node > = Vec::new(); match & crate
                ::host::message_plate(cached_reply.deleted, false, crate
                ::host::seq_in_copy_range(cached_reply.seq, copy_anchor_seq,
                copy_head_seq, copy_surface.clone(), CopySurface::Thread,),) {
                RowPlate::Plain => { children
                .push(native::padded(native::sized(native::container(format!("{}/@container:783",
                thread_message_card_scope_2769), { let message_contents_scope_2207 =
                format!("{}/MessageContents@2207", thread_message_card_scope_2769); { let
                children : Vec < wire::Node > = vec![{ let mut children : Vec <
                wire::Node > = Vec::new(); if cached_reply.show_author { children.push({
                let message_avatar_scope_1750 = format!("{}/MessageAvatar@1750",
                message_contents_scope_2207); { let node_scope = format!("{}/root",
                message_avatar_scope_1750); { let mut children : Vec < wire::Node > =
                Vec::new(); if cached_reply.avatar_kind == "human" { children.push({ let
                person_avatar_scope_798 = format!("{}/PersonAvatar@798",
                message_avatar_scope_1750); { let node_scope = format!("{}/root",
                person_avatar_scope_798); { let children : Vec < wire::Node > =
                vec![{ let principal_avatar_scope_863 = format!("{}/PrincipalAvatar@863",
                person_avatar_scope_798); { let node_scope = format!("{}/root",
                principal_avatar_scope_863); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let principal_plate_scope_909 =
                format!("{}/PrincipalPlate@909", principal_avatar_scope_863); { let
                node_scope = format!("{}/root", principal_plate_scope_909); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                human_plate_scope_925 = format!("{}/HumanPlate@925",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                human_plate_scope_925); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:328", human_plate_scope_925), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([((30.0 / 2.0) as f32).max(0.0).min(f32::MAX), ((30.0
                / 2.0) as f32).max(0.0).min(f32::MAX), ((30.0 / 2.0) as f32).max(0.0)
                .min(f32::MAX), ((30.0 / 2.0) as f32).max(0.0).min(f32::MAX),]), }), snap
                : None, content :
                Box::new(native::text_options(native::text(format!("{}/@text:336",
                human_plate_scope_925), cached_reply.initial.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), }); } native::column(node_scope.clone(),
                children) } } }); } native::column(node_scope.clone(), children) } } });
                } native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } if ! (cached_reply
                .avatar_kind == "human") && cached_reply.avatar_kind == "agent" {
                children.push({ let agent_avatar_scope_804 =
                format!("{}/AgentAvatar@804", message_avatar_scope_1750); { let
                node_scope = format!("{}/root", agent_avatar_scope_804); { let children : Vec < wire::Node > = vec![{ let principal_avatar_scope_873 =
                format!("{}/PrincipalAvatar@873", agent_avatar_scope_804); { let
                node_scope = format!("{}/root", principal_avatar_scope_873); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                principal_plate_scope_909 = format!("{}/PrincipalPlate@909",
                principal_avatar_scope_873); { let node_scope = format!("{}/root",
                principal_plate_scope_909); { let children : Vec < wire::Node > =
                vec![{ let agent_plate_scope_919 = format!("{}/AgentPlate@919",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                agent_plate_scope_919); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let agent_square_scope_1174 =
                format!("{}/AgentSquare@1174", agent_plate_scope_919); { let node_scope =
                format!("{}/root", agent_square_scope_1174); wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : node_scope.clone(), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content :
                Box::new(native::text_options(native::text(format!("{}/@text:390",
                agent_square_scope_1174), cached_reply.initial.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), } } }); } native::column(node_scope.clone(),
                children) } } }]; native::column(node_scope.clone(), children) } } }); }
                native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } if ! (cached_reply
                .avatar_kind == "human" || cached_reply.avatar_kind == "agent") {
                children.push({ let agent_avatar_scope_810 =
                format!("{}/AgentAvatar@810", message_avatar_scope_1750); { let
                node_scope = format!("{}/root", agent_avatar_scope_810); { let children : Vec < wire::Node > = vec![{ let principal_avatar_scope_873 =
                format!("{}/PrincipalAvatar@873", agent_avatar_scope_810); { let
                node_scope = format!("{}/root", principal_avatar_scope_873); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                principal_plate_scope_909 = format!("{}/PrincipalPlate@909",
                principal_avatar_scope_873); { let node_scope = format!("{}/root",
                principal_plate_scope_909); { let children : Vec < wire::Node > =
                vec![{ let agent_plate_scope_919 = format!("{}/AgentPlate@919",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                agent_plate_scope_919); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let agent_square_scope_1174 =
                format!("{}/AgentSquare@1174", agent_plate_scope_919); { let node_scope =
                format!("{}/root", agent_square_scope_1174); wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : node_scope.clone(), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content :
                Box::new(native::text_options(native::text(format!("{}/@text:390",
                agent_square_scope_1174), cached_reply.initial.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), } } }); } native::column(node_scope.clone(),
                children) } } }]; native::column(node_scope.clone(), children) } } }); }
                native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } wire::Node::Stack
                { key : node_scope.clone(), width : Some(wire::Length::Fixed(30.0f32)),
                height : Some(wire::Length::Fixed(30.0f32)), padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, } }
                } }); } if ! cached_reply.show_author { children.push(wire::Node::Space {
                width : Some(wire::Length::Fixed(30.0f32)), height : None, }); } children
                .push({ let mut children : Vec < wire::Node > = Vec::new(); if
                cached_reply.show_author { children.push({ let mut children : Vec <
                wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:347",
                message_contents_scope_2207), cached_reply.author.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; if cached_reply.avatar_kind == "agent" {
                children
                .push(native::padded(native::container(format!("{}/@container:354",
                message_contents_scope_2207),
                native::text_options(native::text(format!("{}/@text:360",
                message_contents_scope_2207), "AGENT".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), wire::Edges { top : 2.0f32, right : 5.0f32,
                bottom : 2.0f32, left : 5.0f32, },),); } if cached_reply.height > 0 {
                children.push(native::text_options(native::text(format!("{}/@text:377",
                message_contents_scope_2207), crate
                ::host::height_label_short(cached_reply.height).to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } if cached_reply.edited { children
                .push(native::text_options(native::text(format!("{}/@text:384",
                message_contents_scope_2207), "· edited".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } children.push(wire::Node::Space { width :
                Some(wire::Length::Fill), height : None, }); wire::Node::Linear {
                max_width : None, clip : false, key : format!("{}/@layout:342",
                message_contents_scope_2207), wrap : None, axis : wire::Axis::Row,
                spacing : Some(7.0f32), padding : None, width : Some(wire::Length::Fill),
                height : None, align : Some(wire::AlignX::Center), background : None,
                border : None, children : children, } }); } children
                .push(wire::Node::MouseArea { key : format!("{}/@mouse:395",
                message_contents_scope_2207), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_424_7(cached_reply
                .seq, CopySurface::Thread.clone(),),),), on_release : None,
                on_double_click : None, on_right_press : None, on_right_release : None,
                on_middle_press : None, on_middle_release : None, on_enter : None,
                on_exit : None, on_move : None, on_press_at : None, on_scroll : None,
                content : Box::new({ let message_body_scope_1809 =
                format!("{}/MessageBody@1809", message_contents_scope_2207); { let children : Vec < wire::Node > = vec![{ let rich_body_scope_688 =
                format!("{}/RichBody@688", message_body_scope_1809); { let mut children :
                Vec < wire::Node > = Vec::new(); for (index, block) in cached_reply
                .blocks.iter().enumerate() { let for_scope = format!("{}/@for:703({})",
                rich_body_scope_688, index); if block.kind == "divider" { children.push({
                let component_separator_scope_705 = format!("{}/Separator@705",
                for_scope); { let node_scope = format!("{}/root",
                component_separator_scope_705); wire::Node::Rule { key : node_scope
                .clone(), axis : wire::Axis::Row, thickness : 1.0f32, color : None, weak
                : false, radius : None, snap : None, } } }); } if block.kind == "code" {
                children
                .push(native::padded(native::sized(native::container(format!("{}/@container:90",
                for_scope), { let mut children : Vec < wire::Node > = Vec::new(); if !
                block.lang.is_empty() { children
                .push(native::text_options(native::text(format!("{}/@text:100",
                for_scope), block.lang.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },),); }
                children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:106",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),);
                native::spaced(native::sized(native::column(format!("{}/@layout:98",
                for_scope), children,), Some(wire::Length::Fill), None,), 6.0f32,) },),
                Some(wire::Length::Fill), None,), wire::Edges { top : 11.0f32, right :
                11.0f32, bottom : 11.0f32, left : 11.0f32, },),); } if block.kind ==
                "quote" { children.push({ let children : Vec < wire::Node > = vec![{
                let mut children : Vec < wire::Node > = Vec::new(); if block.rich {
                children.push({ let rich_line_scope_755 = format!("{}/RichLine@755",
                for_scope); { let mut rich_spans : Vec < wire::RichSpan > = Vec::new();
                for span in block.spans.iter().cloned() { rich_spans.push(wire::RichSpan
                { content : span.mention.to_owned().to_string(), size : None, line_height
                : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Medium,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : Some(span.mention_link.to_owned()), background :
                None, border : Some(wire::Border { color : None, width : None, radius :
                Some([4.0f32, 4.0f32, 4.0f32, 4.0f32,]), }), padding : Some(wire::Edges {
                top : 0.0f32, right : 1.0f32, bottom : 0.0f32, left : 1.0f32, }),
                underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.link_text.to_owned().to_string(),
                size : None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Medium,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : Some(span.link.to_owned()), background : None,
                border : None, padding : None, underline : true, strikethrough : false,
                }); rich_spans.push(wire::RichSpan { content : span.bold_italic
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Bold, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Italic, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.bold
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Bold, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Normal, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.italic
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Normal, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Italic, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.plain
                .to_owned().to_string(), size : None, line_height : None, font : None,
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); }
                wire::Node::RichText { options : wire::TextOptions { wrapping :
                Some(wire::Wrapping::WordOrGlyph), ..Default::default() }, key :
                format!("{}/@text:32", rich_line_scope_755), size : Some(13.5f32), color
                : None, font : wire::Font { monospace : false, weight :
                wire::Weight::Normal, }, width : Some(wire::Length::Fill), align_x :
                None, spans : rich_spans, on_link :
                Some(::ducktape_view_guest::slots::handler:: < String, Message, >
                (Box::new({ let route = { let route_callback = lazy_event_424_5.clone();
                move | link : String | route_callback(link) }; move | sent : String |
                Some(route(sent)) }),),), } } }); } if ! block.rich { children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:137",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),); }
                native::padded(native::sized(native::column(format!("{}/@layout:121",
                for_scope), children,), Some(wire::Length::Fill), None,), wire::Edges {
                top : 2.0f32, right : 0.0f32, bottom : 2.0f32, left : 13.0f32, },) },
                native::sized(native::container(format!("{}/@container:144", for_scope),
                wire::Node::Space { width : Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(1.0f32)), },),
                Some(wire::Length::Fixed(3.0f32)), Some(wire::Length::Fill),)];
                wire::Node::Stack { key : format!("{}/@layout:120", for_scope), width :
                Some(wire::Length::Fill), height : None, padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, }
                }); } if block.kind == "paragraph" { if block.rich { children.push({ let
                rich_line_scope_780 = format!("{}/RichLine@780", for_scope); { let mut
                rich_spans : Vec < wire::RichSpan > = Vec::new(); for span in block.spans
                .iter().cloned() { rich_spans.push(wire::RichSpan { content : span
                .mention.to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Medium, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), color : None, link : Some(span
                .mention_link.to_owned()), background : None, border : Some(wire::Border
                { color : None, width : None, radius : Some([4.0f32, 4.0f32, 4.0f32,
                4.0f32,]), }), padding : Some(wire::Edges { top : 0.0f32, right : 1.0f32,
                bottom : 0.0f32, left : 1.0f32, }), underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.link_text
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Medium, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), color : None, link : Some(span.link
                .to_owned()), background : None, border : None, padding : None, underline
                : true, strikethrough : false, }); rich_spans.push(wire::RichSpan {
                content : span.bold_italic.to_owned().to_string(), size : None,
                line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Bold,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Italic, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.bold.to_owned().to_string(), size :
                None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Bold,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.italic.to_owned().to_string(), size
                : None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Normal,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Italic, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.plain.to_owned().to_string(), size
                : None, line_height : None, font : None, color : None, link : None,
                background : None, border : None, padding : None, underline : false,
                strikethrough : false, }); } wire::Node::RichText { options :
                wire::TextOptions { wrapping : Some(wire::Wrapping::WordOrGlyph),
                ..Default::default() }, key : format!("{}/@text:32",
                rich_line_scope_780), size : Some(13.5f32), color : None, font :
                wire::Font { monospace : false, weight : wire::Weight::Normal, }, width :
                Some(wire::Length::Fill), align_x : None, spans : rich_spans, on_link :
                Some(::ducktape_view_guest::slots::handler:: < String, Message, >
                (Box::new({ let route = { let route_callback = lazy_event_424_5.clone();
                move | link : String | route_callback(link) }; move | sent : String |
                Some(route(sent)) }),),), } } }); } if ! block.rich { children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:157",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),); } } }
                native::spaced(native::sized(native::column(format!("{}/@layout:75",
                rich_body_scope_688), children,), Some(wire::Length::Fill), None,),
                5.0f32,) } }]; wire::Node::Linear { max_width : Some(760.0f32), clip :
                false, key : format!("{}/@layout:60", message_body_scope_1809), wrap :
                None, axis : wire::Axis::Column, spacing : None, padding : None, width :
                Some(wire::Length::Fill), height : None, align : None, background : None,
                border : None, children : children, } } }), }); if cached_reply.edited &&
                ! cached_reply.show_author { children
                .push(native::text_options(native::text(format!("{}/@text:408",
                message_contents_scope_2207), "· edited".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } if ! crate
                ::host::run_of_message(::std::convert::AsRef::as_ref(& cached_reply.id),)
                .is_empty() { children.push({ let children : Vec < wire::Node > =
                vec![native::padded(native::button(format!("{}/@button:420",
                message_contents_scope_2207), String::from("View run"),
                Some(::ducktape_view_guest::slots::message(lazy_event_424_6(crate
                ::host::run_of_message(::std::convert::AsRef::as_ref(& cached_reply
                .id),),),),), wire::ButtonPreset::Secondary,),
                wire::Edges::all(3.0f32),)];
                native::padded(native::sized(native::row(format!("{}/@layout:419",
                message_contents_scope_2207), children,), Some(wire::Length::Fill),
                None,), wire::Edges { top : 4.0f32, right : 0.0f32, bottom : 0.0f32, left
                : 0.0f32, },) }); } if ! cached_reply.reactions.is_empty() { children
                .push({ let mut items = Vec::new(); for (index, reaction) in cached_reply
                .reactions.iter().enumerate() { let for_scope =
                format!("{}/@for:1848({})", message_contents_scope_2207, index); let
                flex_child : wire::Node = { let reaction_chip_scope_1849 =
                format!("{}/ReactionChip@1849", for_scope); { let node_scope =
                format!("{}/root", reaction_chip_scope_1849); { let mut children : Vec <
                wire::Node > = Vec::new(); if reaction.reacted_by_me { children
                .push(wire::Node::Button { checked : Some(reaction.reacted_by_me),
                expanded : None, description : Some(String::from(reaction.emoji
                .to_owned())), key : format!("{}/@button:218", reaction_chip_scope_1849),
                content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:225",
                reaction_chip_scope_1849), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:232",
                reaction_chip_scope_1849), reaction.emoji.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:238",
                reaction_chip_scope_1849), reaction.count.to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:231", reaction_chip_scope_1849), wrap :
                None, axis : wire::Axis::Row, spacing : Some(4.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 1.0f32, right : 8.0f32, bottom : 1.0f32, left : 6.0f32, },),),),
                label : Some(String::from("Remove reaction".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_424_1(cached_reply
                .seq, reaction.emoji.to_owned(),),),), width : None, height : None,
                padding : Some(wire::Edges::all(0.0f32)), style :
                wire::ButtonStyle::default(), }); } if ! reaction.reacted_by_me {
                children.push(wire::Node::Button { checked : Some(reaction
                .reacted_by_me), expanded : None, description :
                Some(String::from(reaction.emoji.to_owned())), key :
                format!("{}/@button:262", reaction_chip_scope_1849), content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:269",
                reaction_chip_scope_1849), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:276",
                reaction_chip_scope_1849), reaction.emoji.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:282",
                reaction_chip_scope_1849), reaction.count.to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:275", reaction_chip_scope_1849), wrap :
                None, axis : wire::Axis::Row, spacing : Some(4.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 1.0f32, right : 8.0f32, bottom : 1.0f32, left : 6.0f32, },),),),
                label : Some(String::from("Add reaction".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_424_0(cached_reply
                .seq, reaction.emoji.to_owned(),),),), width : None, height : None,
                padding : Some(wire::Edges::all(0.0f32)), style :
                wire::ButtonStyle::default(), }); } native::column(node_scope.clone(),
                children) } } }; items.push((wire::FlexItem::default(), flex_child)); }
                let (items, children) = items.into_iter().unzip(); wire::Node::Flex { key
                : format!("{}/@layout:427", message_contents_scope_2207), items,
                children, background : None, border : None, layout : wire::FlexLayout {
                direction : wire::FlexDirection::Row, wrap : wire::FlexWrap::Wrap,
                justify : None, items : Some(wire::FlexItemAlignment::Start), content :
                None, row_gap : Some(5.0f32), column_gap : Some(5.0f32), padding :
                Some(wire::Edges { top : 6.0f32, right : 0.0f32, bottom : 0.0f32, left :
                0.0f32, }), width : Some(wire::Length::Fill), height : None, max_width :
                None, max_height : None, clip : false, surface_width : None,
                surface_height : None, surface_max_width : None, }, } }); } if
                cached_reply.reply_count > 0 { children.push({ let children : Vec <
                wire::Node > = vec![wire::Node::Button { checked : None, expanded : None,
                description : None, key : format!("{}/@button:447",
                message_contents_scope_2207), content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:452",
                message_contents_scope_2207), { let children : Vec < wire::Node > =
                vec![{ let component_icon_scope_1872 = format!("{}/Icon@1872",
                message_contents_scope_2207); { let node_scope = format!("{}/root",
                component_icon_scope_1872); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "nav-chat"),),);
                wire::Node::Svg { inherit_button_ink : false, key :
                format!("{}/@media:52", component_icon_scope_1872), hash : hash, bytes :
                bytes, label : None, color : None, hover : None, fit : None, rotation :
                None, opacity : None, width : Some(wire::Length::Fixed(12.0f32)), height
                : Some(wire::Length::Fixed(12.0f32)), } }); } native::column(node_scope
                .clone(), children) } } },
                native::text_options(native::text(format!("{}/@text:464",
                message_contents_scope_2207), crate ::host::plural(cached_reply
                .reply_count, ::std::convert::AsRef::as_ref(& "reply"),
                ::std::convert::AsRef::as_ref(& "replies"),).to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:458", message_contents_scope_2207), wrap
                : None, axis : wire::Axis::Row, spacing : Some(6.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 3.0f32, right : 9.0f32, bottom : 3.0f32, left : 7.0f32, },),),),
                label : Some(String::from("Open thread".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_424_2(cached_reply
                .seq),),), width : None, height : None, padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), }];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:441", message_contents_scope_2207), wrap : None, axis
                : wire::Axis::Row, spacing : Some(6.0f32), padding : Some(wire::Edges {
                top : 6.0f32, right : 0.0f32, bottom : 0.0f32, left : 0.0f32, }), width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }); }
                native::spaced(native::sized(native::column(format!("{}/@layout:340",
                message_contents_scope_2207), children,), Some(wire::Length::Fill),
                None,), 2.0f32,) }); if cached_reply.pending { children
                .push(native::padded(native::container(format!("{}/@container:506",
                message_contents_scope_2207), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:508",
                message_contents_scope_2207), cached_reply.meta.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },), { let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "dot")),); wire::Node::Svg {
                inherit_button_ink : false, key : format!("{}/@media:514",
                message_contents_scope_2207), hash : hash, bytes : bytes, label : None,
                color : None, hover : None, fit : None, rotation : None, opacity :
                Some(1.0f32), width : Some(wire::Length::Fixed(6.0f32)), height :
                Some(wire::Length::Fixed(6.0f32)), } }]; wire::Node::Linear { max_width :
                None, clip : false, key : format!("{}/@layout:507",
                message_contents_scope_2207), wrap : None, axis : wire::Axis::Row,
                spacing : Some(5.0f32), padding : None, width : None, height : None,
                align : Some(wire::AlignX::Center), background : None, border : None,
                children : children, } },), wire::Edges { top : 0.0f32, right : 7.0f32,
                bottom : 0.0f32, left : 0.0f32, },),); } wire::Node::Linear { max_width :
                None, clip : false, key : format!("{}/@layout:331",
                message_contents_scope_2207), wrap : None, axis : wire::Axis::Row,
                spacing : Some(11.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Left), background : None, border : None, children :
                children, } }]; native::sized(native::column(format!("{}/@layout:330",
                message_contents_scope_2207), children,), Some(wire::Length::Fill),
                None,) } },), Some(wire::Length::Fill), None,), wire::Edges { top :
                4.0f32, right : 7.0f32, bottom : 4.0f32, left : 7.0f32, },),); }
                RowPlate::Selected => { children
                .push(native::padded(native::sized(native::container(format!("{}/@container:803",
                thread_message_card_scope_2769), { let message_contents_scope_2227 =
                format!("{}/MessageContents@2227", thread_message_card_scope_2769); { let
                children : Vec < wire::Node > = vec![{ let mut children : Vec <
                wire::Node > = Vec::new(); if cached_reply.show_author { children.push({
                let message_avatar_scope_1750 = format!("{}/MessageAvatar@1750",
                message_contents_scope_2227); { let node_scope = format!("{}/root",
                message_avatar_scope_1750); { let mut children : Vec < wire::Node > =
                Vec::new(); if cached_reply.avatar_kind == "human" { children.push({ let
                person_avatar_scope_798 = format!("{}/PersonAvatar@798",
                message_avatar_scope_1750); { let node_scope = format!("{}/root",
                person_avatar_scope_798); { let children : Vec < wire::Node > =
                vec![{ let principal_avatar_scope_863 = format!("{}/PrincipalAvatar@863",
                person_avatar_scope_798); { let node_scope = format!("{}/root",
                principal_avatar_scope_863); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let principal_plate_scope_909 =
                format!("{}/PrincipalPlate@909", principal_avatar_scope_863); { let
                node_scope = format!("{}/root", principal_plate_scope_909); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                human_plate_scope_925 = format!("{}/HumanPlate@925",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                human_plate_scope_925); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:328", human_plate_scope_925), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([((30.0 / 2.0) as f32).max(0.0).min(f32::MAX), ((30.0
                / 2.0) as f32).max(0.0).min(f32::MAX), ((30.0 / 2.0) as f32).max(0.0)
                .min(f32::MAX), ((30.0 / 2.0) as f32).max(0.0).min(f32::MAX),]), }), snap
                : None, content :
                Box::new(native::text_options(native::text(format!("{}/@text:336",
                human_plate_scope_925), cached_reply.initial.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), }); } native::column(node_scope.clone(),
                children) } } }); } native::column(node_scope.clone(), children) } } });
                } native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } if ! (cached_reply
                .avatar_kind == "human") && cached_reply.avatar_kind == "agent" {
                children.push({ let agent_avatar_scope_804 =
                format!("{}/AgentAvatar@804", message_avatar_scope_1750); { let
                node_scope = format!("{}/root", agent_avatar_scope_804); { let children : Vec < wire::Node > = vec![{ let principal_avatar_scope_873 =
                format!("{}/PrincipalAvatar@873", agent_avatar_scope_804); { let
                node_scope = format!("{}/root", principal_avatar_scope_873); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                principal_plate_scope_909 = format!("{}/PrincipalPlate@909",
                principal_avatar_scope_873); { let node_scope = format!("{}/root",
                principal_plate_scope_909); { let children : Vec < wire::Node > =
                vec![{ let agent_plate_scope_919 = format!("{}/AgentPlate@919",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                agent_plate_scope_919); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let agent_square_scope_1174 =
                format!("{}/AgentSquare@1174", agent_plate_scope_919); { let node_scope =
                format!("{}/root", agent_square_scope_1174); wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : node_scope.clone(), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content :
                Box::new(native::text_options(native::text(format!("{}/@text:390",
                agent_square_scope_1174), cached_reply.initial.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), } } }); } native::column(node_scope.clone(),
                children) } } }]; native::column(node_scope.clone(), children) } } }); }
                native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } if ! (cached_reply
                .avatar_kind == "human" || cached_reply.avatar_kind == "agent") {
                children.push({ let agent_avatar_scope_810 =
                format!("{}/AgentAvatar@810", message_avatar_scope_1750); { let
                node_scope = format!("{}/root", agent_avatar_scope_810); { let children : Vec < wire::Node > = vec![{ let principal_avatar_scope_873 =
                format!("{}/PrincipalAvatar@873", agent_avatar_scope_810); { let
                node_scope = format!("{}/root", principal_avatar_scope_873); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                principal_plate_scope_909 = format!("{}/PrincipalPlate@909",
                principal_avatar_scope_873); { let node_scope = format!("{}/root",
                principal_plate_scope_909); { let children : Vec < wire::Node > =
                vec![{ let agent_plate_scope_919 = format!("{}/AgentPlate@919",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                agent_plate_scope_919); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let agent_square_scope_1174 =
                format!("{}/AgentSquare@1174", agent_plate_scope_919); { let node_scope =
                format!("{}/root", agent_square_scope_1174); wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : node_scope.clone(), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content :
                Box::new(native::text_options(native::text(format!("{}/@text:390",
                agent_square_scope_1174), cached_reply.initial.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), } } }); } native::column(node_scope.clone(),
                children) } } }]; native::column(node_scope.clone(), children) } } }); }
                native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } wire::Node::Stack
                { key : node_scope.clone(), width : Some(wire::Length::Fixed(30.0f32)),
                height : Some(wire::Length::Fixed(30.0f32)), padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, } }
                } }); } if ! cached_reply.show_author { children.push(wire::Node::Space {
                width : Some(wire::Length::Fixed(30.0f32)), height : None, }); } children
                .push({ let mut children : Vec < wire::Node > = Vec::new(); if
                cached_reply.show_author { children.push({ let mut children : Vec <
                wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:347",
                message_contents_scope_2227), cached_reply.author.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; if cached_reply.avatar_kind == "agent" {
                children
                .push(native::padded(native::container(format!("{}/@container:354",
                message_contents_scope_2227),
                native::text_options(native::text(format!("{}/@text:360",
                message_contents_scope_2227), "AGENT".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), wire::Edges { top : 2.0f32, right : 5.0f32,
                bottom : 2.0f32, left : 5.0f32, },),); } if cached_reply.height > 0 {
                children.push(native::text_options(native::text(format!("{}/@text:377",
                message_contents_scope_2227), crate
                ::host::height_label_short(cached_reply.height).to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } if cached_reply.edited { children
                .push(native::text_options(native::text(format!("{}/@text:384",
                message_contents_scope_2227), "· edited".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } children.push(wire::Node::Space { width :
                Some(wire::Length::Fill), height : None, }); wire::Node::Linear {
                max_width : None, clip : false, key : format!("{}/@layout:342",
                message_contents_scope_2227), wrap : None, axis : wire::Axis::Row,
                spacing : Some(7.0f32), padding : None, width : Some(wire::Length::Fill),
                height : None, align : Some(wire::AlignX::Center), background : None,
                border : None, children : children, } }); } children
                .push(wire::Node::MouseArea { key : format!("{}/@mouse:395",
                message_contents_scope_2227), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_424_7(cached_reply
                .seq, CopySurface::Thread.clone(),),),), on_release : None,
                on_double_click : None, on_right_press : None, on_right_release : None,
                on_middle_press : None, on_middle_release : None, on_enter : None,
                on_exit : None, on_move : None, on_press_at : None, on_scroll : None,
                content : Box::new({ let message_body_scope_1809 =
                format!("{}/MessageBody@1809", message_contents_scope_2227); { let children : Vec < wire::Node > = vec![{ let rich_body_scope_688 =
                format!("{}/RichBody@688", message_body_scope_1809); { let mut children :
                Vec < wire::Node > = Vec::new(); for (index, block) in cached_reply
                .blocks.iter().enumerate() { let for_scope = format!("{}/@for:703({})",
                rich_body_scope_688, index); if block.kind == "divider" { children.push({
                let component_separator_scope_705 = format!("{}/Separator@705",
                for_scope); { let node_scope = format!("{}/root",
                component_separator_scope_705); wire::Node::Rule { key : node_scope
                .clone(), axis : wire::Axis::Row, thickness : 1.0f32, color : None, weak
                : false, radius : None, snap : None, } } }); } if block.kind == "code" {
                children
                .push(native::padded(native::sized(native::container(format!("{}/@container:90",
                for_scope), { let mut children : Vec < wire::Node > = Vec::new(); if !
                block.lang.is_empty() { children
                .push(native::text_options(native::text(format!("{}/@text:100",
                for_scope), block.lang.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },),); }
                children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:106",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),);
                native::spaced(native::sized(native::column(format!("{}/@layout:98",
                for_scope), children,), Some(wire::Length::Fill), None,), 6.0f32,) },),
                Some(wire::Length::Fill), None,), wire::Edges { top : 11.0f32, right :
                11.0f32, bottom : 11.0f32, left : 11.0f32, },),); } if block.kind ==
                "quote" { children.push({ let children : Vec < wire::Node > = vec![{
                let mut children : Vec < wire::Node > = Vec::new(); if block.rich {
                children.push({ let rich_line_scope_755 = format!("{}/RichLine@755",
                for_scope); { let mut rich_spans : Vec < wire::RichSpan > = Vec::new();
                for span in block.spans.iter().cloned() { rich_spans.push(wire::RichSpan
                { content : span.mention.to_owned().to_string(), size : None, line_height
                : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Medium,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : Some(span.mention_link.to_owned()), background :
                None, border : Some(wire::Border { color : None, width : None, radius :
                Some([4.0f32, 4.0f32, 4.0f32, 4.0f32,]), }), padding : Some(wire::Edges {
                top : 0.0f32, right : 1.0f32, bottom : 0.0f32, left : 1.0f32, }),
                underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.link_text.to_owned().to_string(),
                size : None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Medium,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : Some(span.link.to_owned()), background : None,
                border : None, padding : None, underline : true, strikethrough : false,
                }); rich_spans.push(wire::RichSpan { content : span.bold_italic
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Bold, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Italic, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.bold
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Bold, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Normal, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.italic
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Normal, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Italic, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.plain
                .to_owned().to_string(), size : None, line_height : None, font : None,
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); }
                wire::Node::RichText { options : wire::TextOptions { wrapping :
                Some(wire::Wrapping::WordOrGlyph), ..Default::default() }, key :
                format!("{}/@text:32", rich_line_scope_755), size : Some(13.5f32), color
                : None, font : wire::Font { monospace : false, weight :
                wire::Weight::Normal, }, width : Some(wire::Length::Fill), align_x :
                None, spans : rich_spans, on_link :
                Some(::ducktape_view_guest::slots::handler:: < String, Message, >
                (Box::new({ let route = { let route_callback = lazy_event_424_5.clone();
                move | link : String | route_callback(link) }; move | sent : String |
                Some(route(sent)) }),),), } } }); } if ! block.rich { children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:137",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),); }
                native::padded(native::sized(native::column(format!("{}/@layout:121",
                for_scope), children,), Some(wire::Length::Fill), None,), wire::Edges {
                top : 2.0f32, right : 0.0f32, bottom : 2.0f32, left : 13.0f32, },) },
                native::sized(native::container(format!("{}/@container:144", for_scope),
                wire::Node::Space { width : Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(1.0f32)), },),
                Some(wire::Length::Fixed(3.0f32)), Some(wire::Length::Fill),)];
                wire::Node::Stack { key : format!("{}/@layout:120", for_scope), width :
                Some(wire::Length::Fill), height : None, padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, }
                }); } if block.kind == "paragraph" { if block.rich { children.push({ let
                rich_line_scope_780 = format!("{}/RichLine@780", for_scope); { let mut
                rich_spans : Vec < wire::RichSpan > = Vec::new(); for span in block.spans
                .iter().cloned() { rich_spans.push(wire::RichSpan { content : span
                .mention.to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Medium, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), color : None, link : Some(span
                .mention_link.to_owned()), background : None, border : Some(wire::Border
                { color : None, width : None, radius : Some([4.0f32, 4.0f32, 4.0f32,
                4.0f32,]), }), padding : Some(wire::Edges { top : 0.0f32, right : 1.0f32,
                bottom : 0.0f32, left : 1.0f32, }), underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.link_text
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Medium, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), color : None, link : Some(span.link
                .to_owned()), background : None, border : None, padding : None, underline
                : true, strikethrough : false, }); rich_spans.push(wire::RichSpan {
                content : span.bold_italic.to_owned().to_string(), size : None,
                line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Bold,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Italic, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.bold.to_owned().to_string(), size :
                None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Bold,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.italic.to_owned().to_string(), size
                : None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Normal,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Italic, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.plain.to_owned().to_string(), size
                : None, line_height : None, font : None, color : None, link : None,
                background : None, border : None, padding : None, underline : false,
                strikethrough : false, }); } wire::Node::RichText { options :
                wire::TextOptions { wrapping : Some(wire::Wrapping::WordOrGlyph),
                ..Default::default() }, key : format!("{}/@text:32",
                rich_line_scope_780), size : Some(13.5f32), color : None, font :
                wire::Font { monospace : false, weight : wire::Weight::Normal, }, width :
                Some(wire::Length::Fill), align_x : None, spans : rich_spans, on_link :
                Some(::ducktape_view_guest::slots::handler:: < String, Message, >
                (Box::new({ let route = { let route_callback = lazy_event_424_5.clone();
                move | link : String | route_callback(link) }; move | sent : String |
                Some(route(sent)) }),),), } } }); } if ! block.rich { children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:157",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),); } } }
                native::spaced(native::sized(native::column(format!("{}/@layout:75",
                rich_body_scope_688), children,), Some(wire::Length::Fill), None,),
                5.0f32,) } }]; wire::Node::Linear { max_width : Some(760.0f32), clip :
                false, key : format!("{}/@layout:60", message_body_scope_1809), wrap :
                None, axis : wire::Axis::Column, spacing : None, padding : None, width :
                Some(wire::Length::Fill), height : None, align : None, background : None,
                border : None, children : children, } } }), }); if cached_reply.edited &&
                ! cached_reply.show_author { children
                .push(native::text_options(native::text(format!("{}/@text:408",
                message_contents_scope_2227), "· edited".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } if ! crate
                ::host::run_of_message(::std::convert::AsRef::as_ref(& cached_reply.id),)
                .is_empty() { children.push({ let children : Vec < wire::Node > =
                vec![native::padded(native::button(format!("{}/@button:420",
                message_contents_scope_2227), String::from("View run"),
                Some(::ducktape_view_guest::slots::message(lazy_event_424_6(crate
                ::host::run_of_message(::std::convert::AsRef::as_ref(& cached_reply
                .id),),),),), wire::ButtonPreset::Secondary,),
                wire::Edges::all(3.0f32),)];
                native::padded(native::sized(native::row(format!("{}/@layout:419",
                message_contents_scope_2227), children,), Some(wire::Length::Fill),
                None,), wire::Edges { top : 4.0f32, right : 0.0f32, bottom : 0.0f32, left
                : 0.0f32, },) }); } if ! cached_reply.reactions.is_empty() { children
                .push({ let mut items = Vec::new(); for (index, reaction) in cached_reply
                .reactions.iter().enumerate() { let for_scope =
                format!("{}/@for:1848({})", message_contents_scope_2227, index); let
                flex_child : wire::Node = { let reaction_chip_scope_1849 =
                format!("{}/ReactionChip@1849", for_scope); { let node_scope =
                format!("{}/root", reaction_chip_scope_1849); { let mut children : Vec <
                wire::Node > = Vec::new(); if reaction.reacted_by_me { children
                .push(wire::Node::Button { checked : Some(reaction.reacted_by_me),
                expanded : None, description : Some(String::from(reaction.emoji
                .to_owned())), key : format!("{}/@button:218", reaction_chip_scope_1849),
                content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:225",
                reaction_chip_scope_1849), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:232",
                reaction_chip_scope_1849), reaction.emoji.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:238",
                reaction_chip_scope_1849), reaction.count.to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:231", reaction_chip_scope_1849), wrap :
                None, axis : wire::Axis::Row, spacing : Some(4.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 1.0f32, right : 8.0f32, bottom : 1.0f32, left : 6.0f32, },),),),
                label : Some(String::from("Remove reaction".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_424_1(cached_reply
                .seq, reaction.emoji.to_owned(),),),), width : None, height : None,
                padding : Some(wire::Edges::all(0.0f32)), style :
                wire::ButtonStyle::default(), }); } if ! reaction.reacted_by_me {
                children.push(wire::Node::Button { checked : Some(reaction
                .reacted_by_me), expanded : None, description :
                Some(String::from(reaction.emoji.to_owned())), key :
                format!("{}/@button:262", reaction_chip_scope_1849), content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:269",
                reaction_chip_scope_1849), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:276",
                reaction_chip_scope_1849), reaction.emoji.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:282",
                reaction_chip_scope_1849), reaction.count.to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:275", reaction_chip_scope_1849), wrap :
                None, axis : wire::Axis::Row, spacing : Some(4.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 1.0f32, right : 8.0f32, bottom : 1.0f32, left : 6.0f32, },),),),
                label : Some(String::from("Add reaction".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_424_0(cached_reply
                .seq, reaction.emoji.to_owned(),),),), width : None, height : None,
                padding : Some(wire::Edges::all(0.0f32)), style :
                wire::ButtonStyle::default(), }); } native::column(node_scope.clone(),
                children) } } }; items.push((wire::FlexItem::default(), flex_child)); }
                let (items, children) = items.into_iter().unzip(); wire::Node::Flex { key
                : format!("{}/@layout:427", message_contents_scope_2227), items,
                children, background : None, border : None, layout : wire::FlexLayout {
                direction : wire::FlexDirection::Row, wrap : wire::FlexWrap::Wrap,
                justify : None, items : Some(wire::FlexItemAlignment::Start), content :
                None, row_gap : Some(5.0f32), column_gap : Some(5.0f32), padding :
                Some(wire::Edges { top : 6.0f32, right : 0.0f32, bottom : 0.0f32, left :
                0.0f32, }), width : Some(wire::Length::Fill), height : None, max_width :
                None, max_height : None, clip : false, surface_width : None,
                surface_height : None, surface_max_width : None, }, } }); } if
                cached_reply.reply_count > 0 { children.push({ let children : Vec <
                wire::Node > = vec![wire::Node::Button { checked : None, expanded : None,
                description : None, key : format!("{}/@button:447",
                message_contents_scope_2227), content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:452",
                message_contents_scope_2227), { let children : Vec < wire::Node > =
                vec![{ let component_icon_scope_1872 = format!("{}/Icon@1872",
                message_contents_scope_2227); { let node_scope = format!("{}/root",
                component_icon_scope_1872); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "nav-chat"),),);
                wire::Node::Svg { inherit_button_ink : false, key :
                format!("{}/@media:52", component_icon_scope_1872), hash : hash, bytes :
                bytes, label : None, color : None, hover : None, fit : None, rotation :
                None, opacity : None, width : Some(wire::Length::Fixed(12.0f32)), height
                : Some(wire::Length::Fixed(12.0f32)), } }); } native::column(node_scope
                .clone(), children) } } },
                native::text_options(native::text(format!("{}/@text:464",
                message_contents_scope_2227), crate ::host::plural(cached_reply
                .reply_count, ::std::convert::AsRef::as_ref(& "reply"),
                ::std::convert::AsRef::as_ref(& "replies"),).to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:458", message_contents_scope_2227), wrap
                : None, axis : wire::Axis::Row, spacing : Some(6.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 3.0f32, right : 9.0f32, bottom : 3.0f32, left : 7.0f32, },),),),
                label : Some(String::from("Open thread".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_424_2(cached_reply
                .seq),),), width : None, height : None, padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), }];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:441", message_contents_scope_2227), wrap : None, axis
                : wire::Axis::Row, spacing : Some(6.0f32), padding : Some(wire::Edges {
                top : 6.0f32, right : 0.0f32, bottom : 0.0f32, left : 0.0f32, }), width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }); }
                native::spaced(native::sized(native::column(format!("{}/@layout:340",
                message_contents_scope_2227), children,), Some(wire::Length::Fill),
                None,), 2.0f32,) }); if cached_reply.pending { children
                .push(native::padded(native::container(format!("{}/@container:506",
                message_contents_scope_2227), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:508",
                message_contents_scope_2227), cached_reply.meta.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },), { let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "dot")),); wire::Node::Svg {
                inherit_button_ink : false, key : format!("{}/@media:514",
                message_contents_scope_2227), hash : hash, bytes : bytes, label : None,
                color : None, hover : None, fit : None, rotation : None, opacity :
                Some(1.0f32), width : Some(wire::Length::Fixed(6.0f32)), height :
                Some(wire::Length::Fixed(6.0f32)), } }]; wire::Node::Linear { max_width :
                None, clip : false, key : format!("{}/@layout:507",
                message_contents_scope_2227), wrap : None, axis : wire::Axis::Row,
                spacing : Some(5.0f32), padding : None, width : None, height : None,
                align : Some(wire::AlignX::Center), background : None, border : None,
                children : children, } },), wire::Edges { top : 0.0f32, right : 7.0f32,
                bottom : 0.0f32, left : 0.0f32, },),); } wire::Node::Linear { max_width :
                None, clip : false, key : format!("{}/@layout:331",
                message_contents_scope_2227), wrap : None, axis : wire::Axis::Row,
                spacing : Some(11.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Left), background : None, border : None, children :
                children, } }]; native::sized(native::column(format!("{}/@layout:330",
                message_contents_scope_2227), children,), Some(wire::Length::Fill),
                None,) } },), Some(wire::Length::Fill), None,), wire::Edges { top :
                4.0f32, right : 7.0f32, bottom : 4.0f32, left : 7.0f32, },),); }
                RowPlate::Ranged => { children
                .push(native::padded(native::sized(native::container(format!("{}/@container:823",
                thread_message_card_scope_2769), { let message_contents_scope_2247 =
                format!("{}/MessageContents@2247", thread_message_card_scope_2769); { let
                children : Vec < wire::Node > = vec![{ let mut children : Vec <
                wire::Node > = Vec::new(); if cached_reply.show_author { children.push({
                let message_avatar_scope_1750 = format!("{}/MessageAvatar@1750",
                message_contents_scope_2247); { let node_scope = format!("{}/root",
                message_avatar_scope_1750); { let mut children : Vec < wire::Node > =
                Vec::new(); if cached_reply.avatar_kind == "human" { children.push({ let
                person_avatar_scope_798 = format!("{}/PersonAvatar@798",
                message_avatar_scope_1750); { let node_scope = format!("{}/root",
                person_avatar_scope_798); { let children : Vec < wire::Node > =
                vec![{ let principal_avatar_scope_863 = format!("{}/PrincipalAvatar@863",
                person_avatar_scope_798); { let node_scope = format!("{}/root",
                principal_avatar_scope_863); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let principal_plate_scope_909 =
                format!("{}/PrincipalPlate@909", principal_avatar_scope_863); { let
                node_scope = format!("{}/root", principal_plate_scope_909); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                human_plate_scope_925 = format!("{}/HumanPlate@925",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                human_plate_scope_925); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:328", human_plate_scope_925), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([((30.0 / 2.0) as f32).max(0.0).min(f32::MAX), ((30.0
                / 2.0) as f32).max(0.0).min(f32::MAX), ((30.0 / 2.0) as f32).max(0.0)
                .min(f32::MAX), ((30.0 / 2.0) as f32).max(0.0).min(f32::MAX),]), }), snap
                : None, content :
                Box::new(native::text_options(native::text(format!("{}/@text:336",
                human_plate_scope_925), cached_reply.initial.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), }); } native::column(node_scope.clone(),
                children) } } }); } native::column(node_scope.clone(), children) } } });
                } native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } if ! (cached_reply
                .avatar_kind == "human") && cached_reply.avatar_kind == "agent" {
                children.push({ let agent_avatar_scope_804 =
                format!("{}/AgentAvatar@804", message_avatar_scope_1750); { let
                node_scope = format!("{}/root", agent_avatar_scope_804); { let children : Vec < wire::Node > = vec![{ let principal_avatar_scope_873 =
                format!("{}/PrincipalAvatar@873", agent_avatar_scope_804); { let
                node_scope = format!("{}/root", principal_avatar_scope_873); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                principal_plate_scope_909 = format!("{}/PrincipalPlate@909",
                principal_avatar_scope_873); { let node_scope = format!("{}/root",
                principal_plate_scope_909); { let children : Vec < wire::Node > =
                vec![{ let agent_plate_scope_919 = format!("{}/AgentPlate@919",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                agent_plate_scope_919); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let agent_square_scope_1174 =
                format!("{}/AgentSquare@1174", agent_plate_scope_919); { let node_scope =
                format!("{}/root", agent_square_scope_1174); wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : node_scope.clone(), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content :
                Box::new(native::text_options(native::text(format!("{}/@text:390",
                agent_square_scope_1174), cached_reply.initial.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), } } }); } native::column(node_scope.clone(),
                children) } } }]; native::column(node_scope.clone(), children) } } }); }
                native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } if ! (cached_reply
                .avatar_kind == "human" || cached_reply.avatar_kind == "agent") {
                children.push({ let agent_avatar_scope_810 =
                format!("{}/AgentAvatar@810", message_avatar_scope_1750); { let
                node_scope = format!("{}/root", agent_avatar_scope_810); { let children : Vec < wire::Node > = vec![{ let principal_avatar_scope_873 =
                format!("{}/PrincipalAvatar@873", agent_avatar_scope_810); { let
                node_scope = format!("{}/root", principal_avatar_scope_873); { let mut
                children : Vec < wire::Node > = Vec::new(); { children.push({ let
                principal_plate_scope_909 = format!("{}/PrincipalPlate@909",
                principal_avatar_scope_873); { let node_scope = format!("{}/root",
                principal_plate_scope_909); { let children : Vec < wire::Node > =
                vec![{ let agent_plate_scope_919 = format!("{}/AgentPlate@919",
                principal_plate_scope_909); { let node_scope = format!("{}/root",
                agent_plate_scope_919); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let agent_square_scope_1174 =
                format!("{}/AgentSquare@1174", agent_plate_scope_919); { let node_scope =
                format!("{}/root", agent_square_scope_1174); wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : node_scope.clone(), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content :
                Box::new(native::text_options(native::text(format!("{}/@text:390",
                agent_square_scope_1174), cached_reply.initial.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), } } }); } native::column(node_scope.clone(),
                children) } } }]; native::column(node_scope.clone(), children) } } }); }
                native::column(node_scope.clone(), children) } } }];
                native::column(node_scope.clone(), children) } } }); } wire::Node::Stack
                { key : node_scope.clone(), width : Some(wire::Length::Fixed(30.0f32)),
                height : Some(wire::Length::Fixed(30.0f32)), padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, } }
                } }); } if ! cached_reply.show_author { children.push(wire::Node::Space {
                width : Some(wire::Length::Fixed(30.0f32)), height : None, }); } children
                .push({ let mut children : Vec < wire::Node > = Vec::new(); if
                cached_reply.show_author { children.push({ let mut children : Vec <
                wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:347",
                message_contents_scope_2247), cached_reply.author.to_owned()
                .to_string(),), wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; if cached_reply.avatar_kind == "agent" {
                children
                .push(native::padded(native::container(format!("{}/@container:354",
                message_contents_scope_2247),
                native::text_options(native::text(format!("{}/@text:360",
                message_contents_scope_2247), "AGENT".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),), wire::Edges { top : 2.0f32, right : 5.0f32,
                bottom : 2.0f32, left : 5.0f32, },),); } if cached_reply.height > 0 {
                children.push(native::text_options(native::text(format!("{}/@text:377",
                message_contents_scope_2247), crate
                ::host::height_label_short(cached_reply.height).to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } if cached_reply.edited { children
                .push(native::text_options(native::text(format!("{}/@text:384",
                message_contents_scope_2247), "· edited".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } children.push(wire::Node::Space { width :
                Some(wire::Length::Fill), height : None, }); wire::Node::Linear {
                max_width : None, clip : false, key : format!("{}/@layout:342",
                message_contents_scope_2247), wrap : None, axis : wire::Axis::Row,
                spacing : Some(7.0f32), padding : None, width : Some(wire::Length::Fill),
                height : None, align : Some(wire::AlignX::Center), background : None,
                border : None, children : children, } }); } children
                .push(wire::Node::MouseArea { key : format!("{}/@mouse:395",
                message_contents_scope_2247), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_424_7(cached_reply
                .seq, CopySurface::Thread.clone(),),),), on_release : None,
                on_double_click : None, on_right_press : None, on_right_release : None,
                on_middle_press : None, on_middle_release : None, on_enter : None,
                on_exit : None, on_move : None, on_press_at : None, on_scroll : None,
                content : Box::new({ let message_body_scope_1809 =
                format!("{}/MessageBody@1809", message_contents_scope_2247); { let children : Vec < wire::Node > = vec![{ let rich_body_scope_688 =
                format!("{}/RichBody@688", message_body_scope_1809); { let mut children :
                Vec < wire::Node > = Vec::new(); for (index, block) in cached_reply
                .blocks.iter().enumerate() { let for_scope = format!("{}/@for:703({})",
                rich_body_scope_688, index); if block.kind == "divider" { children.push({
                let component_separator_scope_705 = format!("{}/Separator@705",
                for_scope); { let node_scope = format!("{}/root",
                component_separator_scope_705); wire::Node::Rule { key : node_scope
                .clone(), axis : wire::Axis::Row, thickness : 1.0f32, color : None, weak
                : false, radius : None, snap : None, } } }); } if block.kind == "code" {
                children
                .push(native::padded(native::sized(native::container(format!("{}/@container:90",
                for_scope), { let mut children : Vec < wire::Node > = Vec::new(); if !
                block.lang.is_empty() { children
                .push(native::text_options(native::text(format!("{}/@text:100",
                for_scope), block.lang.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },),); }
                children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:106",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),);
                native::spaced(native::sized(native::column(format!("{}/@layout:98",
                for_scope), children,), Some(wire::Length::Fill), None,), 6.0f32,) },),
                Some(wire::Length::Fill), None,), wire::Edges { top : 11.0f32, right :
                11.0f32, bottom : 11.0f32, left : 11.0f32, },),); } if block.kind ==
                "quote" { children.push({ let children : Vec < wire::Node > = vec![{
                let mut children : Vec < wire::Node > = Vec::new(); if block.rich {
                children.push({ let rich_line_scope_755 = format!("{}/RichLine@755",
                for_scope); { let mut rich_spans : Vec < wire::RichSpan > = Vec::new();
                for span in block.spans.iter().cloned() { rich_spans.push(wire::RichSpan
                { content : span.mention.to_owned().to_string(), size : None, line_height
                : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Medium,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : Some(span.mention_link.to_owned()), background :
                None, border : Some(wire::Border { color : None, width : None, radius :
                Some([4.0f32, 4.0f32, 4.0f32, 4.0f32,]), }), padding : Some(wire::Edges {
                top : 0.0f32, right : 1.0f32, bottom : 0.0f32, left : 1.0f32, }),
                underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.link_text.to_owned().to_string(),
                size : None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Medium,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : Some(span.link.to_owned()), background : None,
                border : None, padding : None, underline : true, strikethrough : false,
                }); rich_spans.push(wire::RichSpan { content : span.bold_italic
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Bold, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Italic, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.bold
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Bold, stretch : wire::FontStretch::Normal, style :
                wire::FontStyle::Normal, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.italic
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Normal, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Italic, }), color : None, link : None, background :
                None, border : None, padding : None, underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.plain
                .to_owned().to_string(), size : None, line_height : None, font : None,
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); }
                wire::Node::RichText { options : wire::TextOptions { wrapping :
                Some(wire::Wrapping::WordOrGlyph), ..Default::default() }, key :
                format!("{}/@text:32", rich_line_scope_755), size : Some(13.5f32), color
                : None, font : wire::Font { monospace : false, weight :
                wire::Weight::Normal, }, width : Some(wire::Length::Fill), align_x :
                None, spans : rich_spans, on_link :
                Some(::ducktape_view_guest::slots::handler:: < String, Message, >
                (Box::new({ let route = { let route_callback = lazy_event_424_5.clone();
                move | link : String | route_callback(link) }; move | sent : String |
                Some(route(sent)) }),),), } } }); } if ! block.rich { children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:137",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),); }
                native::padded(native::sized(native::column(format!("{}/@layout:121",
                for_scope), children,), Some(wire::Length::Fill), None,), wire::Edges {
                top : 2.0f32, right : 0.0f32, bottom : 2.0f32, left : 13.0f32, },) },
                native::sized(native::container(format!("{}/@container:144", for_scope),
                wire::Node::Space { width : Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(1.0f32)), },),
                Some(wire::Length::Fixed(3.0f32)), Some(wire::Length::Fill),)];
                wire::Node::Stack { key : format!("{}/@layout:120", for_scope), width :
                Some(wire::Length::Fill), height : None, padding : None, background :
                None, border : None, clip : false, under : 0u32, children : children, }
                }); } if block.kind == "paragraph" { if block.rich { children.push({ let
                rich_line_scope_780 = format!("{}/RichLine@780", for_scope); { let mut
                rich_spans : Vec < wire::RichSpan > = Vec::new(); for span in block.spans
                .iter().cloned() { rich_spans.push(wire::RichSpan { content : span
                .mention.to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Medium, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), color : None, link : Some(span
                .mention_link.to_owned()), background : None, border : Some(wire::Border
                { color : None, width : None, radius : Some([4.0f32, 4.0f32, 4.0f32,
                4.0f32,]), }), padding : Some(wire::Edges { top : 0.0f32, right : 1.0f32,
                bottom : 0.0f32, left : 1.0f32, }), underline : false, strikethrough :
                false, }); rich_spans.push(wire::RichSpan { content : span.link_text
                .to_owned().to_string(), size : None, line_height : None, font :
                Some(wire::NamedFont { family : wire::FontFamily::Named("Geist".into()),
                weight : wire::Weight::Medium, stretch : wire::FontStretch::Normal, style
                : wire::FontStyle::Normal, }), color : None, link : Some(span.link
                .to_owned()), background : None, border : None, padding : None, underline
                : true, strikethrough : false, }); rich_spans.push(wire::RichSpan {
                content : span.bold_italic.to_owned().to_string(), size : None,
                line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Bold,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Italic, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.bold.to_owned().to_string(), size :
                None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Bold,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.italic.to_owned().to_string(), size
                : None, line_height : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Normal,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Italic, }),
                color : None, link : None, background : None, border : None, padding :
                None, underline : false, strikethrough : false, }); rich_spans
                .push(wire::RichSpan { content : span.plain.to_owned().to_string(), size
                : None, line_height : None, font : None, color : None, link : None,
                background : None, border : None, padding : None, underline : false,
                strikethrough : false, }); } wire::Node::RichText { options :
                wire::TextOptions { wrapping : Some(wire::Wrapping::WordOrGlyph),
                ..Default::default() }, key : format!("{}/@text:32",
                rich_line_scope_780), size : Some(13.5f32), color : None, font :
                wire::Font { monospace : false, weight : wire::Weight::Normal, }, width :
                Some(wire::Length::Fill), align_x : None, spans : rich_spans, on_link :
                Some(::ducktape_view_guest::slots::handler:: < String, Message, >
                (Box::new({ let route = { let route_callback = lazy_event_424_5.clone();
                move | link : String | route_callback(link) }; move | sent : String |
                Some(route(sent)) }),),), } } }); } if ! block.rich { children
                .push(native::sized(native::text_options(native::text(format!("{}/@text:157",
                for_scope), block.text.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::WordOrGlyph), ..Default::default() },),
                Some(wire::Length::Fill), None,),); } } }
                native::spaced(native::sized(native::column(format!("{}/@layout:75",
                rich_body_scope_688), children,), Some(wire::Length::Fill), None,),
                5.0f32,) } }]; wire::Node::Linear { max_width : Some(760.0f32), clip :
                false, key : format!("{}/@layout:60", message_body_scope_1809), wrap :
                None, axis : wire::Axis::Column, spacing : None, padding : None, width :
                Some(wire::Length::Fill), height : None, align : None, background : None,
                border : None, children : children, } } }), }); if cached_reply.edited &&
                ! cached_reply.show_author { children
                .push(native::text_options(native::text(format!("{}/@text:408",
                message_contents_scope_2247), "· edited".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),); } if ! crate
                ::host::run_of_message(::std::convert::AsRef::as_ref(& cached_reply.id),)
                .is_empty() { children.push({ let children : Vec < wire::Node > =
                vec![native::padded(native::button(format!("{}/@button:420",
                message_contents_scope_2247), String::from("View run"),
                Some(::ducktape_view_guest::slots::message(lazy_event_424_6(crate
                ::host::run_of_message(::std::convert::AsRef::as_ref(& cached_reply
                .id),),),),), wire::ButtonPreset::Secondary,),
                wire::Edges::all(3.0f32),)];
                native::padded(native::sized(native::row(format!("{}/@layout:419",
                message_contents_scope_2247), children,), Some(wire::Length::Fill),
                None,), wire::Edges { top : 4.0f32, right : 0.0f32, bottom : 0.0f32, left
                : 0.0f32, },) }); } if ! cached_reply.reactions.is_empty() { children
                .push({ let mut items = Vec::new(); for (index, reaction) in cached_reply
                .reactions.iter().enumerate() { let for_scope =
                format!("{}/@for:1848({})", message_contents_scope_2247, index); let
                flex_child : wire::Node = { let reaction_chip_scope_1849 =
                format!("{}/ReactionChip@1849", for_scope); { let node_scope =
                format!("{}/root", reaction_chip_scope_1849); { let mut children : Vec <
                wire::Node > = Vec::new(); if reaction.reacted_by_me { children
                .push(wire::Node::Button { checked : Some(reaction.reacted_by_me),
                expanded : None, description : Some(String::from(reaction.emoji
                .to_owned())), key : format!("{}/@button:218", reaction_chip_scope_1849),
                content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:225",
                reaction_chip_scope_1849), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:232",
                reaction_chip_scope_1849), reaction.emoji.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:238",
                reaction_chip_scope_1849), reaction.count.to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:231", reaction_chip_scope_1849), wrap :
                None, axis : wire::Axis::Row, spacing : Some(4.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 1.0f32, right : 8.0f32, bottom : 1.0f32, left : 6.0f32, },),),),
                label : Some(String::from("Remove reaction".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_424_1(cached_reply
                .seq, reaction.emoji.to_owned(),),),), width : None, height : None,
                padding : Some(wire::Edges::all(0.0f32)), style :
                wire::ButtonStyle::default(), }); } if ! reaction.reacted_by_me {
                children.push(wire::Node::Button { checked : Some(reaction
                .reacted_by_me), expanded : None, description :
                Some(String::from(reaction.emoji.to_owned())), key :
                format!("{}/@button:262", reaction_chip_scope_1849), content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:269",
                reaction_chip_scope_1849), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:276",
                reaction_chip_scope_1849), reaction.emoji.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:282",
                reaction_chip_scope_1849), reaction.count.to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:275", reaction_chip_scope_1849), wrap :
                None, axis : wire::Axis::Row, spacing : Some(4.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 1.0f32, right : 8.0f32, bottom : 1.0f32, left : 6.0f32, },),),),
                label : Some(String::from("Add reaction".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_424_0(cached_reply
                .seq, reaction.emoji.to_owned(),),),), width : None, height : None,
                padding : Some(wire::Edges::all(0.0f32)), style :
                wire::ButtonStyle::default(), }); } native::column(node_scope.clone(),
                children) } } }; items.push((wire::FlexItem::default(), flex_child)); }
                let (items, children) = items.into_iter().unzip(); wire::Node::Flex { key
                : format!("{}/@layout:427", message_contents_scope_2247), items,
                children, background : None, border : None, layout : wire::FlexLayout {
                direction : wire::FlexDirection::Row, wrap : wire::FlexWrap::Wrap,
                justify : None, items : Some(wire::FlexItemAlignment::Start), content :
                None, row_gap : Some(5.0f32), column_gap : Some(5.0f32), padding :
                Some(wire::Edges { top : 6.0f32, right : 0.0f32, bottom : 0.0f32, left :
                0.0f32, }), width : Some(wire::Length::Fill), height : None, max_width :
                None, max_height : None, clip : false, surface_width : None,
                surface_height : None, surface_max_width : None, }, } }); } if
                cached_reply.reply_count > 0 { children.push({ let children : Vec <
                wire::Node > = vec![wire::Node::Button { checked : None, expanded : None,
                description : None, key : format!("{}/@button:447",
                message_contents_scope_2247), content :
                wire::ButtonContent::Child(Box::new(native::padded(native::container(format!("{}/@container:452",
                message_contents_scope_2247), { let children : Vec < wire::Node > =
                vec![{ let component_icon_scope_1872 = format!("{}/Icon@1872",
                message_contents_scope_2247); { let node_scope = format!("{}/root",
                component_icon_scope_1872); { let mut children : Vec < wire::Node > =
                Vec::new(); { children.push({ let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "nav-chat"),),);
                wire::Node::Svg { inherit_button_ink : false, key :
                format!("{}/@media:52", component_icon_scope_1872), hash : hash, bytes :
                bytes, label : None, color : None, hover : None, fit : None, rotation :
                None, opacity : None, width : Some(wire::Length::Fixed(12.0f32)), height
                : Some(wire::Length::Fixed(12.0f32)), } }); } native::column(node_scope
                .clone(), children) } } },
                native::text_options(native::text(format!("{}/@text:464",
                message_contents_scope_2247), crate ::host::plural(cached_reply
                .reply_count, ::std::convert::AsRef::as_ref(& "reply"),
                ::std::convert::AsRef::as_ref(& "replies"),).to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:458", message_contents_scope_2247), wrap
                : None, axis : wire::Axis::Row, spacing : Some(6.0f32), padding : None,
                width : None, height : None, align : Some(wire::AlignX::Center),
                background : None, border : None, children : children, } },), wire::Edges
                { top : 3.0f32, right : 9.0f32, bottom : 3.0f32, left : 7.0f32, },),),),
                label : Some(String::from("Open thread".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_424_2(cached_reply
                .seq),),), width : None, height : None, padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), }];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:441", message_contents_scope_2247), wrap : None, axis
                : wire::Axis::Row, spacing : Some(6.0f32), padding : Some(wire::Edges {
                top : 6.0f32, right : 0.0f32, bottom : 0.0f32, left : 0.0f32, }), width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }); }
                native::spaced(native::sized(native::column(format!("{}/@layout:340",
                message_contents_scope_2247), children,), Some(wire::Length::Fill),
                None,), 2.0f32,) }); if cached_reply.pending { children
                .push(native::padded(native::container(format!("{}/@container:506",
                message_contents_scope_2247), { let children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:508",
                message_contents_scope_2247), cached_reply.meta.to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },), { let (hash, bytes) =
                ::ducktape_view_guest::slots::picture(crate
                ::host::icon(::std::convert::AsRef::as_ref(& "dot")),); wire::Node::Svg {
                inherit_button_ink : false, key : format!("{}/@media:514",
                message_contents_scope_2247), hash : hash, bytes : bytes, label : None,
                color : None, hover : None, fit : None, rotation : None, opacity :
                Some(1.0f32), width : Some(wire::Length::Fixed(6.0f32)), height :
                Some(wire::Length::Fixed(6.0f32)), } }]; wire::Node::Linear { max_width :
                None, clip : false, key : format!("{}/@layout:507",
                message_contents_scope_2247), wrap : None, axis : wire::Axis::Row,
                spacing : Some(5.0f32), padding : None, width : None, height : None,
                align : Some(wire::AlignX::Center), background : None, border : None,
                children : children, } },), wire::Edges { top : 0.0f32, right : 7.0f32,
                bottom : 0.0f32, left : 0.0f32, },),); } wire::Node::Linear { max_width :
                None, clip : false, key : format!("{}/@layout:331",
                message_contents_scope_2247), wrap : None, axis : wire::Axis::Row,
                spacing : Some(11.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Left), background : None, border : None, children :
                children, } }]; native::sized(native::column(format!("{}/@layout:330",
                message_contents_scope_2247), children,), Some(wire::Length::Fill),
                None,) } },), Some(wire::Length::Fill), None,), wire::Edges { top :
                4.0f32, right : 7.0f32, bottom : 4.0f32, left : 7.0f32, },),); } }
                wire::Node::Stack { key : format!("{}/@layout:778",
                thread_message_card_scope_2769), width : Some(wire::Length::Fill), height
                : None, padding : None, background : None, border : None, clip : false,
                under : 0u32, children : children, } }, { let mut children : Vec <
                wire::Node > = Vec::new(); if ! cached_reply.deleted && ! cached_reply
                .pending { children.push(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:844", thread_message_card_scope_2769), width
                : Some(wire::Length::Fill), height : None, padding : Some(wire::Edges {
                top : 0.0f32, right : 8.0f32, bottom : 0.0f32, left : 0.0f32, }), align_x
                : Some(wire::AlignX::Right), align_y : Some(wire::AlignY::Top),
                background : None.map(wire::Background::Color), border : None, snap :
                None, content :
                Box::new(native::padded(native::container(format!("{}/@container:854",
                thread_message_card_scope_2769), { let children : Vec < wire::Node >
                = vec![wire::Node::Button { checked : None, expanded : None, description
                : None, key : format!("{}/@button:865", thread_message_card_scope_2769),
                content :
                wire::ButtonContent::Child(Box::new(native::text(format!("{}/@text:872",
                thread_message_card_scope_2769), "♡".to_owned().to_string(),),),),
                label : Some(String::from("Manage reactions".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(lazy_event_424_4(cached_reply
                .seq, cached_reply.body.to_owned(), cached_reply.rev,),),), width :
                Some(wire::Length::Fixed(27.0f32)), height :
                Some(wire::Length::Fixed(25.0f32)), padding :
                Some(wire::Edges::all(4.0f32)), style : wire::ButtonStyle::default(), },
                wire::Node::Button { checked : None, expanded : None, description : None,
                key : format!("{}/@button:876", thread_message_card_scope_2769), content
                :
                wire::ButtonContent::Child(Box::new(native::text(format!("{}/@text:883",
                thread_message_card_scope_2769), "⋯".to_owned().to_string(),),),),
                label : Some(String::from("More message actions".to_owned()),), on_press
                :
                Some(::ducktape_view_guest::slots::message(lazy_event_424_3(cached_reply
                .seq, cached_reply.body.to_owned(), cached_reply.rev,),),), width :
                Some(wire::Length::Fixed(27.0f32)), height :
                Some(wire::Length::Fixed(25.0f32)), padding :
                Some(wire::Edges::all(4.0f32)), style : wire::ButtonStyle::default(), }];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:864", thread_message_card_scope_2769), wrap : None,
                axis : wire::Axis::Row, spacing : Some(1.0f32), padding : None, width :
                None, height : None, align : Some(wire::AlignX::Center), background :
                None, border : None, children : children, } },), wire::Edges { top :
                2.0f32, right : 2.0f32, bottom : 2.0f32, left : 2.0f32, },),), }); } if
                cached_reply.deleted || cached_reply.pending { children
                .push(wire::Node::Space { width : Some(wire::Length::Fixed(1.0f32)),
                height : Some(wire::Length::Fixed(1.0f32)), }); }
                native::sized(native::column(format!("{}/@layout:842",
                thread_message_card_scope_2769), children,), Some(wire::Length::Fill),
                None,) }]; wire::Node::Hover { key : format!("{}/@layout:773",
                thread_message_card_scope_2769), width : None, height : None, padding :
                None, background : None, border : None, tint : None, radius : 9.0f32,
                open : false, children : children, } });
                native::sized(native::column(format!("{}/@layout:767",
                thread_message_card_scope_2769), children,), Some(wire::Length::Fill),
                None,) } } }, 424u64, & key_recon, lazy_key,) } }); }
                native::spaced(native::sized(native::column(format!("{}/@layout:142",
                key_recon), children,), Some(wire::Length::Fill), None,), 0.0f32,) };
                children.push((key, child)); } let (keys, children) = children
                .into_iter().map(| (key, child) | (wire::ListKey::from(key), child))
                .unzip(); wire::Node::KeyedColumn { key : format!("{}/@keyed:137",
                thread_timeline_scope_4354), keys : Some(keys), children, background :
                None, border : None, spacing : Some(3.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, max_width : None, align : None,
                virtual_row : Some(44.0f32), } } } }, 720u64, & use_scope, lazy_key,) }
                }]; for (index, live) in self.live_agents.iter().enumerate() { let
                for_scope = format!("{}/@for:4377({})", use_scope, index); if crate
                ::host::run_in_thread(::std::borrow::Borrow::borrow(& live), self
                .active_thread_seq,) { children.push(self
                .live_run_card(format!("{}/LiveRunCard@4379", for_scope), (move
                | event_0 | Message::CancelRun(event_0)).clone(), (move | event_0 |
                Message::OpenRun(event_0)).clone(), live.clone(),),); } } if self
                .thread_has_more && self.thread_next_reply_seq > 0 && self.thread_loading
                { children
                .push(native::padded(native::sized(native::button(format!("{}/@button:1781",
                use_scope), String::from("Loading replies…"), None,
                wire::ButtonPreset::Secondary,), Some(wire::Length::Fill), None,),
                wire::Edges::all(5.0f32),),); } if self.thread_has_more && self
                .thread_next_reply_seq > 0 && ! self.thread_loading { children
                .push(native::padded(native::sized(native::button(format!("{}/@button:1791",
                use_scope), String::from("Load more replies"), if self.busy { None } else
                { Some(::ducktape_view_guest::slots::message(Message::LoadMoreThread,),)
                }, wire::ButtonPreset::Secondary,), Some(wire::Length::Fill), None,),
                wire::Edges::all(5.0f32),),); }
                native::spaced(native::padded(native::sized(native::column(format!("{}/@layout:1735",
                use_scope), children,), Some(wire::Length::Fill), None,), wire::Edges {
                top : 12.0f32, right : 16.0f32, bottom : 8.0f32, left : 16.0f32, },),
                3.0f32,) }), } }); children
                .push(native::padded(native::sized(native::container(format!("{}/@container:1823",
                use_scope), { let node_scope = format!("{}/reply_composer", node_scope);
                wire::Node::Surface { key : node_scope.clone(), name :
                String::from("chat_composer"), args : ::std::vec![{ let surface_arg = &
                (crate ::host::thread_scope(::std::convert::AsRef::as_ref(& (self
                .endpoint)), ::std::convert::AsRef::as_ref(& (self.active_channel)), self
                .active_thread_seq));
                ::ducktape_view_guest::wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                }, { let surface_arg = & ("reply".to_owned());
                ::ducktape_view_guest::wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                }, { let surface_arg = & (true);
                ::ducktape_view_guest::wire::SurfaceValue::Bool(* (surface_arg)) }, { let
                surface_arg = & ("Reply…".to_owned());
                ::ducktape_view_guest::wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                }, { let surface_arg = & (((self.thread_loading || (! self.connected)) ||
                (! (self.post_refusal).is_empty())));
                ::ducktape_view_guest::wire::SurfaceValue::Bool(* (surface_arg)) }, { let
                surface_arg = & (false);
                ::ducktape_view_guest::wire::SurfaceValue::Bool(* (surface_arg)) }, { let
                surface_arg = & ("Unsent reply".to_owned());
                ::ducktape_view_guest::wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                }], on_event : None, } },), Some(wire::Length::Fill), None,), wire::Edges
                { top : 10.0f32, right : 16.0f32, bottom : 14.0f32, left : 16.0f32,
                },),); native::sized(native::column(format!("{}/@layout:1634",
                use_scope), children,), Some(wire::Length::Fill),
                Some(wire::Length::Fill),) }), }, { let mut children =
                vec![::ducktape_view_guest::wire::Node::Space { width :
                Some(::ducktape_view_guest::wire::Length::Fill), height :
                Some(::ducktape_view_guest::wire::Length::Fill) }]; if self
                .thread_selected_seq > 0 && self.thread_message_action !=
                MessageAction::Toolbar { children.push({ let mut children : Vec <
                wire::Node > = vec![wire::Node::MouseArea { key :
                format!("{}/@mouse:1850", use_scope), on_press :
                Some(::ducktape_view_guest::slots::message(Message::ClearThreadMessageSelection,),),
                on_release : None, on_double_click : None, on_right_press : None,
                on_right_release : None, on_middle_press : None, on_middle_release :
                None, on_enter : None, on_exit : None, on_move : None, on_press_at :
                None, on_scroll : None, content : Box::new(wire::Node::Space { width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fixed(crate
                ::host::block_action_menu_y(self.chat_screen_states.get(& use_scope)
                .map_or_else(| | self.chat_screen_initial.thread_pointer_y.clone(), |
                state | state.thread_pointer_y.clone(),), self.chat_screen_states.get(&
                use_scope).map_or_else(| | self.chat_screen_initial.thread_height
                .clone(), | state | state.thread_height.clone(),),) as f32,),), }), }];
                if self.thread_message_action == MessageAction::More { children.push({
                let children : Vec < wire::Node > = vec![{ let node_scope =
                format!("{}/thread-action-focus", node_scope); wire::Node::Input {
                options : wire::InputOptions { label : "Thread action focus".to_owned()
                .to_string(), description : None, disabled : false, padding :
                Some(wire::Edges::all(0.0f32)), text_size : Some(1.0f32), line_height :
                Some(1.0f32), align : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Normal,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                }, key : node_scope.clone(), placeholder : String::from(""), value : self
                .chat_screen_states.get(& use_scope).map_or_else(| | self
                .chat_screen_initial.message_action_focus.clone(), | state | state
                .message_action_focus.clone(),).to_string(), on_input :
                ::ducktape_view_guest::slots::handler:: < String, Message, > (Box::new({
                let route = { let scope = use_scope.clone(); move | value |
                Message::ChatScreenMessageActionFocusChanged(scope.clone(), value,) };
                move | sent : String | Some(route(sent)) }),), on_submit : None, width :
                Some(wire::Length::Fixed(1.0f32)), secure : false, style :
                Default::default(), } },
                native::padded(native::sized(native::container(format!("{}/@container:1864",
                use_scope), { let children : Vec < wire::Node > =
                vec![wire::Node::Button { checked : None, expanded : None, description :
                None, key : format!("{}/@button:1877", use_scope), content :
                wire::ButtonContent::Child(Box::new(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:1884", use_scope), width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), padding :
                Some(wire::Edges { top : 0.0f32, right : 9.0f32, bottom : 0.0f32, left :
                9.0f32, }), align_x : None, align_y : Some(wire::AlignY::Center),
                background : None.map(wire::Background::Color), border : None, snap :
                None, content : Box::new({ let children : Vec < wire::Node > =
                vec![self.icon(format!("{}/Icon@4499", use_scope), "emoji", 14f32,
                "@media:82",), native::text_options(native::text(format!("{}/@text:1901",
                use_scope), "Add reaction".to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },)];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:1891", use_scope), wrap : None, axis :
                wire::Axis::Row, spacing : Some(9.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }), }),), label : Some(String::from("Manage reactions"
                .to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(Message::OpenThreadMessageReactions(self
                .thread_selected_seq, self.thread_edit_draft.to_owned(), self
                .thread_selected_rev,),),), width : Some(wire::Length::Fill), height :
                Some(wire::Length::Fixed(30.0f32)), padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), },
                wire::Node::Button { checked : None, expanded : None, description : None,
                key : format!("{}/@button:1913", use_scope), content :
                wire::ButtonContent::Child(Box::new(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:1920", use_scope), width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), padding :
                Some(wire::Edges { top : 0.0f32, right : 9.0f32, bottom : 0.0f32, left :
                9.0f32, }), align_x : None, align_y : Some(wire::AlignY::Center),
                background : None.map(wire::Background::Color), border : None, snap :
                None, content : Box::new({ let children : Vec < wire::Node > =
                vec![self.icon(format!("{}/Icon@4535", use_scope), "link", 14f32,
                "@media:82",), native::text_options(native::text(format!("{}/@text:1937",
                use_scope), "Copy link".to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },)];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:1927", use_scope), wrap : None, axis :
                wire::Axis::Row, spacing : Some(9.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }), }),), label : Some(String::from("Copy message link"
                .to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(Message::CopyMessageLink(crate
                ::host::duck_channel_message_link(self.active_channel.to_owned(), self
                .thread_selected_seq, self.network_chain_id.to_owned(),),),),), width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fixed(30.0f32)),
                padding : Some(wire::Edges::all(0.0f32)), style :
                wire::ButtonStyle::default(), }, wire::Node::Button { checked : None,
                expanded : None, description : None, key : format!("{}/@button:1945",
                use_scope), content :
                wire::ButtonContent::Child(Box::new(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:1952", use_scope), width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), padding :
                Some(wire::Edges { top : 0.0f32, right : 9.0f32, bottom : 0.0f32, left :
                9.0f32, }), align_x : None, align_y : Some(wire::AlignY::Center),
                background : None.map(wire::Background::Color), border : None, snap :
                None, content : Box::new({ let children : Vec < wire::Node > =
                vec![self.icon(format!("{}/Icon@4567", use_scope), "pencil", 14f32,
                "@media:82",), native::text_options(native::text(format!("{}/@text:1969",
                use_scope), "Edit message".to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },)];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:1959", use_scope), wrap : None, axis :
                wire::Axis::Row, spacing : Some(9.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }), }),), label : Some(String::from("Edit message"
                .to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(Message::BeginThreadMessageEdit(self
                .thread_selected_seq, self.thread_edit_draft.to_owned(), self
                .thread_selected_rev,),),), width : Some(wire::Length::Fill), height :
                Some(wire::Length::Fixed(30.0f32)), padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), },
                native::sized(native::container(format!("{}/@container:1977", use_scope),
                wire::Node::Space { width : Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(1.0f32)), },), Some(wire::Length::Fill),
                Some(wire::Length::Fixed(1.0f32)),), wire::Node::Button { checked : None,
                expanded : None, description : None, key : format!("{}/@button:1983",
                use_scope), content :
                wire::ButtonContent::Child(Box::new(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:1990", use_scope), width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), padding :
                Some(wire::Edges { top : 0.0f32, right : 9.0f32, bottom : 0.0f32, left :
                9.0f32, }), align_x : None, align_y : Some(wire::AlignY::Center),
                background : None.map(wire::Background::Color), border : None, snap :
                None, content : Box::new({ let children : Vec < wire::Node > =
                vec![self.icon(format!("{}/Icon@4605", use_scope), "trash", 14f32,
                "@media:70",), native::text_options(native::text(format!("{}/@text:2007",
                use_scope), "Delete message…".to_owned().to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; wire::Node::Linear { max_width : None, clip :
                false, key : format!("{}/@layout:1997", use_scope), wrap : None, axis :
                wire::Axis::Row, spacing : Some(9.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }), }),), label : Some(String::from("Delete message"
                .to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(Message::ArmThreadMessageDelete(self
                .thread_selected_seq, self.thread_edit_draft.to_owned(), self
                .thread_selected_rev,),),), width : Some(wire::Length::Fill), height :
                Some(wire::Length::Fixed(30.0f32)), padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), }];
                native::spaced(native::sized(native::column(format!("{}/@layout:1875",
                use_scope), children,), Some(wire::Length::Fill), None,), 1.0f32,) },),
                Some(wire::Length::Fixed(200.0f32)), None,), wire::Edges { top : 5.0f32,
                right : 5.0f32, bottom : 5.0f32, left : 5.0f32, },)]; wire::Node::Stack {
                key : format!("{}/@layout:1853", use_scope), width : None, height : None,
                padding : None, background : None, border : None, clip : false, under :
                0u32, children : children, } }); } if self.thread_message_action ==
                MessageAction::Reactions { children.push({ let children : Vec <
                wire::Node > = vec![{ let node_scope =
                format!("{}/thread-reaction-focus", node_scope); wire::Node::Input {
                options : wire::InputOptions { label : "Thread reaction focus".to_owned()
                .to_string(), description : None, disabled : false, padding :
                Some(wire::Edges::all(0.0f32)), text_size : Some(1.0f32), line_height :
                Some(1.0f32), align : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Normal,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                }, key : node_scope.clone(), placeholder : String::from(""), value : self
                .chat_screen_states.get(& use_scope).map_or_else(| | self
                .chat_screen_initial.message_action_focus.clone(), | state | state
                .message_action_focus.clone(),).to_string(), on_input :
                ::ducktape_view_guest::slots::handler:: < String, Message, > (Box::new({
                let route = { let scope = use_scope.clone(); move | value |
                Message::ChatScreenMessageActionFocusChanged(scope.clone(), value,) };
                move | sent : String | Some(route(sent)) }),), on_submit : None, width :
                Some(wire::Length::Fixed(1.0f32)), secure : false, style :
                Default::default(), } },
                native::padded(native::container(format!("{}/@container:2028",
                use_scope), { let mut items = Vec::new(); for (index, emoji) in crate
                ::host::reaction_palette().iter().enumerate() { let for_scope =
                format!("{}/@for:4648({})", use_scope, index); let flex_child :
                wire::Node = wire::Node::Button { checked : None, expanded : None,
                description : Some(String::from(emoji.to_owned())), key :
                format!("{}/@button:2046", for_scope), content :
                wire::ButtonContent::Child(Box::new(wire::Node::Container { shadow :
                Default::default(), max_width : None, max_height : None, clip : false,
                key : format!("{}/@container:2055", for_scope), width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), padding :
                None, align_x : Some(wire::AlignX::Center), align_y :
                Some(wire::AlignY::Center), background : None
                .map(wire::Background::Color), border : None, snap : None, content :
                Box::new(native::text_options(native::text(format!("{}/@text:2061",
                for_scope), emoji.to_owned().to_string(),), wire::TextOptions { wrapping
                : Some(wire::Wrapping::None), ..Default::default() },),), }),), label :
                Some(String::from("Add reaction".to_owned())), on_press : if self
                .active_channel_archived { None } else {
                Some(::ducktape_view_guest::slots::message(Message::AddReactionAt(self
                .thread_selected_seq, emoji.to_owned(),),),) }, width :
                Some(wire::Length::Fixed(27.0f32)), height :
                Some(wire::Length::Fixed(27.0f32)), padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), };
                items.push((wire::FlexItem::default(), flex_child)); } let (items,
                children) = items.into_iter().unzip(); wire::Node::Flex { key :
                format!("{}/@layout:2038", use_scope), items, children, background :
                None, border : None, layout : wire::FlexLayout { direction :
                wire::FlexDirection::Row, wrap : wire::FlexWrap::Wrap, justify : None,
                items : Some(wire::FlexItemAlignment::Start), content : None, row_gap :
                Some(2.0f32), column_gap : Some(2.0f32), padding : None, width :
                Some(wire::Length::Fixed(234.0f32)), height : None, max_width : None,
                max_height : None, clip : false, surface_width : None, surface_height :
                None, surface_max_width : None, }, } },), wire::Edges { top : 8.0f32,
                right : 8.0f32, bottom : 8.0f32, left : 8.0f32, },)]; wire::Node::Stack {
                key : format!("{}/@layout:2016", use_scope), width : None, height : None,
                padding : None, background : None, border : None, clip : false, under :
                0u32, children : children, } }); } if self.thread_message_action ==
                MessageAction::Editing { children
                .push(native::padded(native::sized(native::container(format!("{}/@container:2070",
                use_scope), { let children : Vec < wire::Node > = vec![{ let
                node_scope = format!("{}/thread-edit", node_scope); wire::Node::Surface {
                key : node_scope.clone(), name : String::from("chat_composer"), args :
                ::std::vec![{ let surface_arg = & (crate
                ::host::edit_scope(::std::convert::AsRef::as_ref(& (self.endpoint)),
                ::std::convert::AsRef::as_ref(& (self.active_channel)), self
                .thread_selected_seq));
                ::ducktape_view_guest::wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                }, { let surface_arg = & ("thread_edit".to_owned());
                ::ducktape_view_guest::wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                }, { let surface_arg = & (true);
                ::ducktape_view_guest::wire::SurfaceValue::Bool(* (surface_arg)) }, { let
                surface_arg = & ("Edit message".to_owned());
                ::ducktape_view_guest::wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                }, { let surface_arg = & (self.busy);
                ::ducktape_view_guest::wire::SurfaceValue::Bool(* (surface_arg)) }, { let
                surface_arg = & (false);
                ::ducktape_view_guest::wire::SurfaceValue::Bool(* (surface_arg)) }, { let
                surface_arg = & ("Could not save changes".to_owned());
                ::ducktape_view_guest::wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                }], on_event : None, } }, wire::Node::Button { checked : None, expanded :
                None, description : None, key : format!("{}/@button:2087", use_scope),
                content : wire::ButtonContent::Child(Box::new(wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : format!("{}/@container:2095", use_scope), width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), padding :
                None, align_x : Some(wire::AlignX::Center), align_y :
                Some(wire::AlignY::Center), background : None
                .map(wire::Background::Color), border : None, snap : None, content :
                Box::new(native::text(format!("{}/@text:2101", use_scope), "×"
                .to_owned().to_string(),),), }),), label :
                Some(String::from("Cancel message edit".to_owned())), on_press : if self
                .busy { None } else {
                Some(::ducktape_view_guest::slots::message(Message::ClearThreadMessageSelection,),)
                }, width : Some(wire::Length::Fixed(28.0f32)), height :
                Some(wire::Length::Fixed(28.0f32)), padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), }];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:2081", use_scope), wrap : None, axis :
                wire::Axis::Row, spacing : Some(4.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } },), Some(wire::Length::Fill), None,), wire::Edges { top :
                3.0f32, right : 3.0f32, bottom : 3.0f32, left : 3.0f32, },),); } if self
                .thread_message_action == MessageAction::Delete { children.push({ let children : Vec < wire::Node > = vec![{ let node_scope =
                format!("{}/thread-delete-focus", node_scope); wire::Node::Input {
                options : wire::InputOptions { label : "Thread delete focus".to_owned()
                .to_string(), description : None, disabled : false, padding :
                Some(wire::Edges::all(0.0f32)), text_size : Some(1.0f32), line_height :
                Some(1.0f32), align : None, font : Some(wire::NamedFont { family :
                wire::FontFamily::Named("Geist".into()), weight : wire::Weight::Normal,
                stretch : wire::FontStretch::Normal, style : wire::FontStyle::Normal, }),
                }, key : node_scope.clone(), placeholder : String::from(""), value : self
                .chat_screen_states.get(& use_scope).map_or_else(| | self
                .chat_screen_initial.message_action_focus.clone(), | state | state
                .message_action_focus.clone(),).to_string(), on_input :
                ::ducktape_view_guest::slots::handler:: < String, Message, > (Box::new({
                let route = { let scope = use_scope.clone(); move | value |
                Message::ChatScreenMessageActionFocusChanged(scope.clone(), value,) };
                move | sent : String | Some(route(sent)) }),), on_submit : None, width :
                Some(wire::Length::Fixed(1.0f32)), secure : false, style :
                Default::default(), } },
                native::padded(native::container(format!("{}/@container:2116",
                use_scope), { let children : Vec < wire::Node > =
                vec![native::text(format!("{}/@text:2127", use_scope),
                "Delete this message?".to_owned().to_string(),),
                native::padded(native::button(format!("{}/@button:2128", use_scope),
                String::from("Delete"), if self.busy { None } else {
                Some(::ducktape_view_guest::slots::message(Message::DeleteThreadMessageSubmit,),)
                }, wire::ButtonPreset::Secondary,), wire::Edges::all(5.0f32),),
                native::padded(native::button(format!("{}/@button:2133", use_scope),
                String::from("Cancel"), if self.busy { None } else {
                Some(::ducktape_view_guest::slots::message(Message::ClearThreadMessageSelection,),)
                }, wire::ButtonPreset::Secondary,), wire::Edges::all(5.0f32),)];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:2126", use_scope), wrap : None, axis :
                wire::Axis::Row, spacing : Some(5.0f32), padding : None, width : None,
                height : None, align : Some(wire::AlignX::Center), background : None,
                border : None, children : children, } },), wire::Edges { top : 3.0f32,
                right : 3.0f32, bottom : 3.0f32, left : 3.0f32, },)]; wire::Node::Stack {
                key : format!("{}/@layout:2106", use_scope), width : None, height : None,
                padding : None, background : None, border : None, clip : false, under :
                0u32, children : children, } }); }
                native::column(format!("{}/@layout:1849", use_scope), children,) }); }
                wire::Node::Overlay { key : format!("{}/@overlay:1836", use_scope),
                padding : 8.0f32, backdrop : wire::Rgba([0.0 / 255.0, 0.0 / 255.0, 0.0 /
                255.0, 0.000000,]), align_x : wire::AlignX::Right, align_y :
                wire::AlignY::Top, on_dismiss :
                Some(::ducktape_view_guest::slots::message(Message::ClearThreadMessageSelection,),),
                children : children, } }]; wire::Node::Stack { key :
                format!("{}/@layout:1630", use_scope), width : Some(wire::Length::Fill),
                height : Some(wire::Length::Fill), padding : None, background : None,
                border : None, clip : false, under : 0u32, children : children, } },),
                Some(wire::Length::Fixed(self.thread_width as f32)),
                Some(wire::Length::Fill),) });
                        }
                        native::sized(
                            native::row(format!("{}/@layout:543", use_scope), children),
                            Some(wire::Length::Fill),
                            Some(wire::Length::Fill),
                        )
                    }),
                },
            ];
            native::sized(
                native::row(format!("{}/@layout:282", use_scope), children),
                Some(wire::Length::Fill),
                Some(wire::Length::Fill),
            )
        }
    }
}
