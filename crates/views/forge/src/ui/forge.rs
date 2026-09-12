use super::*;
impl super::ForgeView {
    pub(super) fn render_code(
        &self,
        palette: Palette,
        use_scope: String,
        cb_8: impl Fn(String) -> Message + Clone + 'static,
        cb_9: impl Fn(String) -> Message + Clone + 'static,
        cb_15: impl Fn(String) -> Message + Clone + 'static,
        cb_16: impl Fn(f64, f64) -> Message + Clone + 'static,
    ) -> ::ducktape_view_guest::wire::Node {
        self.render_forge_code_tab_32(
            palette,
            format!("{}/ForgeCodeTab@3561", use_scope),
            use_scope.clone(),
            (cb_8).clone(),
            (cb_9).clone(),
            (cb_15).clone(),
            (cb_16).clone(),
        )
    }
    pub(super) fn render_linked_note_95(
        &self,
        palette: Palette,
        use_scope: String,
        cb_15: impl Fn(String) -> Message + Clone + 'static,
        arg_0: crate::host::ChatMessage,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (8.0) as f32,
                    right: (8.0) as f32,
                    bottom: (8.0) as f32,
                    left: (8.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[91]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: None,
                    width: None,
                    radius: Some([
                        ((8.0) as f32).max(0.0).min(f32::MAX),
                        ((8.0) as f32).max(0.0).min(f32::MAX),
                        ((8.0) as f32).max(0.0).min(f32::MAX),
                        ((8.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children.push(::ducktape_view_guest::wire::Node::Text {
                        options: ::ducktape_view_guest::wire::TextOptions {
                            height: None,
                            align_y: None,
                            line_height: None,
                            shaping: None,
                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                            tracking: 0.0f32,
                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                    "Geist".into(),
                                ),
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:778", use_scope),
                        size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[70]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("Linked note".to_owned()).to_string(),
                    });
                    children.push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        children.push(self.render_message_avatar_91(
                            palette,
                            format!("{}/MessageAvatar@3531", use_scope),
                            arg_0.initial.to_owned(),
                            arg_0.avatar_kind.to_owned(),
                        ));
                        children.push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children.push({
                                let mut children: Vec<::ducktape_view_guest::wire::Node> =
                                    Vec::new();
                                children.push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch:
                                                ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:795", use_scope),
                                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[4]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: (arg_0.author.to_owned()).to_string(),
                                });
                                children.push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                            stretch:
                                                ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:801", use_scope),
                                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[71]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: (arg_0.meta.to_owned()).to_string(),
                                });
                                children.push(::ducktape_view_guest::wire::Node::Space {
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                });
                                ::ducktape_view_guest::wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:790", use_scope),
                                    wrap: None,
                                    axis: ::ducktape_view_guest::wire::Axis::Row,
                                    spacing: Some((7.0) as f32),
                                    padding: None,
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                    align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                    background: None,
                                    border: None,
                                    children: children,
                                }
                            });
                            children.push(self.render_message_body_94(
                                palette,
                                format!("{}/MessageBody@3551", use_scope),
                                (cb_15).clone(),
                                arg_0.clone(),
                            ));
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:789", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                spacing: Some((2.0) as f32),
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: None,
                                background: None,
                                border: None,
                                children: children,
                            }
                        });
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:783", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: Some((9.0) as f32),
                            padding: None,
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            align: Some(::ducktape_view_guest::wire::AlignX::Left),
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:777", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: Some((6.0) as f32),
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        align: None,
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(super) fn render_forge(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
            if !self.connected {
                children.push(::ducktape_view_guest::wire::Node::Container {
                    shadow: ::ducktape_view_guest::wire::Shadow {
                        color: None,
                        x: None,
                        y: None,
                        blur: None,
                    },
                    max_width: None,
                    max_height: None,
                    clip: false,
                    key: format!("{}/@container:43", use_scope),
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: Some(::ducktape_view_guest::wire::Length::Fill),
                    padding: Some(::ducktape_view_guest::wire::Edges {
                        top: (22.0) as f32,
                        right: (22.0) as f32,
                        bottom: (22.0) as f32,
                        left: (22.0) as f32,
                    }),
                    align_x: None,
                    align_y: None,
                    background: (None).map(::ducktape_view_guest::wire::Background::Color),
                    border: None,
                    snap: None,
                    content: Box::new(
                        self.render_empty_state_1(
                            palette,
                            format!("{}/EmptyState@2791", use_scope),
                        ),
                    ),
                });
            }
            if self.connected && (self.open_repo).is_empty() {
                children
                    .push(::ducktape_view_guest::wire::Node::Scroll {
                        on_scroll: None,
                        virtual_rows: false,
                        key: format!("{}/@layout:56", use_scope),
                        direction: ::ducktape_view_guest::wire::ScrollDirection::Vertical,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: Some(::ducktape_view_guest::wire::Length::Fill),
                        bar_hidden: false,
                        bar_width: None,
                        bar_margin: None,
                        scroller_width: None,
                        bar_spacing: None,
                        anchor_x: ::ducktape_view_guest::wire::ScrollAnchor::Start,
                        anchor_y: ::ducktape_view_guest::wire::ScrollAnchor::Start,
                        auto_scroll: (false),
                        background: None,
                        border: None,
                        content: Box::new({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(
                                    self
                                        .render_forge_org_header_3(
                                            palette,
                                            format!("{}/ForgeOrgHeader@2809", use_scope),
                                        ),
                                );
                            if (self.repos).is_empty()
                                && (self.list_phase == "loading") 
                            {
                                children
                                    .push(::ducktape_view_guest::wire::Node::Container {
                                        shadow: ::ducktape_view_guest::wire::Shadow {
                                            color: None,
                                            x: None,
                                            y: None,
                                            blur: None,
                                        },
                                        max_width: None,
                                        max_height: None,
                                        clip: false,
                                        key: format!("{}/@container:74", use_scope),
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: None,
                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                            top: (30.0) as f32,
                                            right: (30.0) as f32,
                                            bottom: (30.0) as f32,
                                            left: (30.0) as f32,
                                        }),
                                        align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                        align_y: None,
                                        background: (None)
                                            .map(::ducktape_view_guest::wire::Background::Color),
                                        border: None,
                                        snap: None,
                                        content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                height: None,
                                                align_y: None,
                                                line_height: None,
                                                shaping: None,
                                                wrapping: None,
                                                tracking: 0.0f32,
                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                        "Geist".into(),
                                                    ),
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                }),
                                            },
                                            key: format!("{}/@text:79", use_scope),
                                            size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                            color: Some(palette.colors[71]),
                                            font: ::ducktape_view_guest::wire::Font {
                                                monospace: false,
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            },
                                            width: None,
                                            align_x: None,
                                            content: ("Loading repositories…".to_owned()).to_string(),
                                        }),
                                    });
                            }
                            if (self.repos).is_empty() && (self.list_phase == "failed") 
                            {
                                children
                                    .push(::ducktape_view_guest::wire::Node::Container {
                                        shadow: ::ducktape_view_guest::wire::Shadow {
                                            color: None,
                                            x: None,
                                            y: None,
                                            blur: None,
                                        },
                                        max_width: None,
                                        max_height: None,
                                        clip: false,
                                        key: format!("{}/@container:81", use_scope),
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: None,
                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                            top: (30.0) as f32,
                                            right: (30.0) as f32,
                                            bottom: (30.0) as f32,
                                            left: (30.0) as f32,
                                        }),
                                        align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                        align_y: None,
                                        background: (None)
                                            .map(::ducktape_view_guest::wire::Background::Color),
                                        border: None,
                                        snap: None,
                                        content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                height: None,
                                                align_y: None,
                                                line_height: None,
                                                shaping: None,
                                                wrapping: None,
                                                tracking: 0.0f32,
                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                        "Geist".into(),
                                                    ),
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                }),
                                            },
                                            key: format!("{}/@text:86", use_scope),
                                            size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                            color: Some(palette.colors[71]),
                                            font: ::ducktape_view_guest::wire::Font {
                                                monospace: false,
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            },
                                            width: None,
                                            align_x: None,
                                            content: ("Could not load repositories. Reopen Forge to try again."
                                                .to_owned())
                                                .to_string(),
                                        }),
                                    });
                            }
                            if (self.repos).is_empty() && (self.list_phase == "ready") 
                            {
                                children
                                    .push(::ducktape_view_guest::wire::Node::Container {
                                        shadow: ::ducktape_view_guest::wire::Shadow {
                                            color: None,
                                            x: None,
                                            y: None,
                                            blur: None,
                                        },
                                        max_width: None,
                                        max_height: None,
                                        clip: false,
                                        key: format!("{}/@container:94", use_scope),
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: None,
                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                            top: (30.0) as f32,
                                            right: (30.0) as f32,
                                            bottom: (30.0) as f32,
                                            left: (30.0) as f32,
                                        }),
                                        align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                        align_y: None,
                                        background: (Some(
                                            ::ducktape_view_guest::wire::Rgba([
                                                0.0 / 255.0,
                                                0.0 / 255.0,
                                                0.0 / 255.0,
                                                0.000000,
                                            ]),
                                        ))
                                            .map(::ducktape_view_guest::wire::Background::Color),
                                        border: Some(::ducktape_view_guest::wire::Border {
                                            color: Some(palette.colors[39]),
                                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                            radius: Some([
                                                ((12.0) as f32).max(0.0).min(f32::MAX),
                                                ((12.0) as f32).max(0.0).min(f32::MAX),
                                                ((12.0) as f32).max(0.0).min(f32::MAX),
                                                ((12.0) as f32).max(0.0).min(f32::MAX),
                                            ]),
                                        }),
                                        snap: None,
                                        content: Box::new({
                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                            children
                                                .push(::ducktape_view_guest::wire::Node::Text {
                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                        height: None,
                                                        align_y: None,
                                                        line_height: None,
                                                        shaping: None,
                                                        wrapping: None,
                                                        tracking: 0.0f32,
                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                "Geist".into(),
                                                            ),
                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                        }),
                                                    },
                                                    key: format!("{}/@text:104", use_scope),
                                                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: Some(palette.colors[71]),
                                                    font: ::ducktape_view_guest::wire::Font {
                                                        monospace: false,
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    },
                                                    width: None,
                                                    align_x: None,
                                                    content: ("No repos yet. Forge is a git remote — a repo appears when a push lands on it."
                                                        .to_owned())
                                                        .to_string(),
                                                });
                                            children
                                                .push(::ducktape_view_guest::wire::Node::Container {
                                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                                        color: None,
                                                        x: None,
                                                        y: None,
                                                        blur: None,
                                                    },
                                                    max_width: None,
                                                    max_height: None,
                                                    clip: false,
                                                    key: format!("{}/@container:108", use_scope),
                                                    width: None,
                                                    height: None,
                                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                                        top: (8.0) as f32,
                                                        right: (12.0) as f32,
                                                        bottom: (8.0) as f32,
                                                        left: (12.0) as f32,
                                                    }),
                                                    align_x: None,
                                                    align_y: None,
                                                    background: (Some(palette.colors[6]))
                                                        .map(::ducktape_view_guest::wire::Background::Color),
                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                        color: Some(palette.colors[39]),
                                                        width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                        radius: Some([
                                                            ((8.0) as f32).max(0.0).min(f32::MAX),
                                                            ((8.0) as f32).max(0.0).min(f32::MAX),
                                                            ((8.0) as f32).max(0.0).min(f32::MAX),
                                                            ((8.0) as f32).max(0.0).min(f32::MAX),
                                                        ]),
                                                    }),
                                                    snap: None,
                                                    content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                                        options: ::ducktape_view_guest::wire::TextOptions {
                                                            height: None,
                                                            align_y: None,
                                                            line_height: None,
                                                            shaping: None,
                                                            wrapping: Some(
                                                                ::ducktape_view_guest::wire::Wrapping::WordOrGlyph,
                                                            ),
                                                            tracking: 0.0f32,
                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                    "Geist Mono".into(),
                                                                ),
                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                            }),
                                                        },
                                                        key: format!("{}/@text:116", use_scope),
                                                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                        color: Some(palette.colors[15]),
                                                        font: ::ducktape_view_guest::wire::Font {
                                                            monospace: false,
                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                        },
                                                        width: None,
                                                        align_x: None,
                                                        content: (crate::host::forge_push_command(
                                                            ::std::convert::AsRef::as_ref(&(self.connected_rpc)),
                                                        ))
                                                            .to_string(),
                                                    }),
                                                });
                                            ::ducktape_view_guest::wire::Node::Linear {
                                                max_width: None,
                                                clip: false,
                                                key: format!("{}/@layout:103", use_scope),
                                                wrap: None,
                                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                                spacing: Some((10.0) as f32),
                                                padding: None,
                                                width: None,
                                                height: None,
                                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                background: None,
                                                border: None,
                                                children: children,
                                            }
                                        }),
                                    });
                            }
                            if !(self.repos).is_empty()  {
                                children
                                    .push({
                                        let mut items = Vec::new();
                                        let min_cell = ((320.0) as f32)
                                            .max(f32::EPSILON)
                                            .min(f32::MAX);
                                        for (index, repo) in self.repos.iter().enumerate() {
                                            let for_scope = format!(
                                                "{}/@for:2867({})", use_scope, index
                                            );
                                            let flex_child: ::ducktape_view_guest::wire::Node = self
                                                .render_repo_card_5(
                                                    palette,
                                                    format!("{}/RepoCard@2868", for_scope),
                                                    (move |event_0| Message::ForgeOpenRepo(event_0)).clone(),
                                                    repo.clone(),
                                                );
                                            items
                                                .push((
                                                    ::ducktape_view_guest::wire::FlexItem {
                                                        grow: Some(1.0),
                                                        shrink: 0.0,
                                                        basis: ::ducktape_view_guest::wire::FlexBasis::Fixed(
                                                            min_cell,
                                                        ),
                                                        ..Default::default()
                                                    },
                                                    flex_child,
                                                ));
                                        }
                                        let (items, children) = items.into_iter().unzip();
                                        ::ducktape_view_guest::wire::Node::Flex {
                                            key: format!("{}/@layout:123", use_scope),
                                            items,
                                            children,
                                            background: None,
                                            border: None,
                                            layout: ::ducktape_view_guest::wire::FlexLayout {
                                                direction: ::ducktape_view_guest::wire::FlexDirection::Row,
                                                wrap: ::ducktape_view_guest::wire::FlexWrap::Wrap,
                                                justify: None,
                                                items: None,
                                                content: None,
                                                row_gap: Some((10.0) as f32),
                                                column_gap: Some((10.0) as f32),
                                                padding: None,
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                height: None,
                                                max_width: None,
                                                max_height: None,
                                                clip: (false),
                                                surface_width: None,
                                                surface_height: None,
                                                surface_max_width: None,
                                            },
                                        }
                                    });
                            }
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:61", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                spacing: Some((14.0) as f32),
                                padding: Some(::ducktape_view_guest::wire::Edges {
                                    top: (16.0) as f32,
                                    right: (16.0) as f32,
                                    bottom: (16.0) as f32,
                                    left: (16.0) as f32,
                                }),
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: None,
                                background: None,
                                border: None,
                                children: children,
                            }
                        }),
                    });
            }
            if self.connected && (!(self.open_repo).is_empty()) {
                children
                    .push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        children
                            .push(::ducktape_view_guest::wire::Node::Container {
                                shadow: ::ducktape_view_guest::wire::Shadow {
                                    color: None,
                                    x: None,
                                    y: None,
                                    blur: None,
                                },
                                max_width: None,
                                max_height: None,
                                clip: false,
                                key: format!("{}/@container:130", use_scope),
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                padding: Some(::ducktape_view_guest::wire::Edges {
                                    top: (8.0) as f32,
                                    right: (16.0) as f32,
                                    bottom: (8.0) as f32,
                                    left: (16.0) as f32,
                                }),
                                align_x: None,
                                align_y: None,
                                background: (None)
                                    .map(::ducktape_view_guest::wire::Background::Color),
                                border: None,
                                snap: None,
                                content: Box::new({
                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                    children
                                        .push(::ducktape_view_guest::wire::Node::Button {
                                            checked: None,
                                            expanded: None,
                                            description: None,
                                            key: format!("{}/@button:145", use_scope),
                                            content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                Box::new(
                                                    self
                                                        .render_repo_crumb_7(
                                                            palette,
                                                            format!("{}/RepoCrumb@2893", use_scope),
                                                        ),
                                                ),
                                            ),
                                            label: Some(String::from("All repos".to_owned())),
                                            on_press: Some(
                                                ::ducktape_view_guest::slots::message(
                                                    Message::ForgeCloseRepo,
                                                ),
                                            ),
                                            width: None,
                                            height: None,
                                            padding: Some(
                                                ::ducktape_view_guest::wire::Edges::all((0.0) as f32),
                                            ),
                                            style: ::ducktape_view_guest::wire::ButtonStyle {
                                                preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                    base: ::ducktape_view_guest::wire::Face {
                                                        background: Some(
                                                            ::ducktape_view_guest::wire::Rgba([
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.000000,
                                                            ]),
                                                        ),
                                                        text: Some(palette.colors[4]),
                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                            color: None,
                                                            width: None,
                                                            radius: Some([8.0; 4]),
                                                        }),
                                                    },
                                                    hover_background: Some(palette.colors[14]),
                                                    pressed_background: Some(palette.colors[39]),
                                                    disabled_background: None,
                                                    disabled_text: None,
                                                    disabled_opacity: Some(0.5f32),
                                                    focus_ring: Some(palette.colors[42]),
                                                    text_size: Some(12.5f32),
                                                    line_height: None,
                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                            "Geist".into(),
                                                        ),
                                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                    }),
                                                }),
                                                active: ::ducktape_view_guest::wire::Face {
                                                    background: Some(
                                                        ::ducktape_view_guest::wire::Rgba([
                                                            0.0 / 255.0,
                                                            0.0 / 255.0,
                                                            0.0 / 255.0,
                                                            0.000000,
                                                        ]),
                                                    ),
                                                    text: Some(palette.colors[4]),
                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                        color: Some(
                                                            ::ducktape_view_guest::wire::Rgba([
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.000000,
                                                            ]),
                                                        ),
                                                        width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                        radius: Some([
                                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                                        ]),
                                                    }),
                                                },
                                                hovered: Some(::ducktape_view_guest::wire::Face {
                                                    background: Some(palette.colors[57]),
                                                    text: Some(palette.colors[4]),
                                                    border: None,
                                                }),
                                                pressed: Some(::ducktape_view_guest::wire::Face {
                                                    background: Some(palette.colors[55]),
                                                    text: Some(palette.colors[4]),
                                                    border: None,
                                                }),
                                                disabled: None,
                                            },
                                        });
                                    children
                                        .push({
                                            let node_scope = format!("{}/repo-pick", use_scope);
                                            {
                                                let options = crate::host::repo_names(
                                                    ::std::convert::AsRef::as_ref(&(self.repos)),
                                                );
                                                let selected = Some(self.open_repo.to_owned());
                                                ::ducktape_view_guest::wire::Node::PickList {
                                                    key: node_scope.clone(),
                                                    options: options
                                                        .iter()
                                                        .map(|option| option.to_string())
                                                        .collect(),
                                                    selected: selected
                                                        .as_ref()
                                                        .and_then(|chosen| {
                                                            options.iter().position(|option| option == chosen)
                                                        })
                                                        .map(|index| index as u32),
                                                    placeholder: None,
                                                    on_select: ::ducktape_view_guest::slots::handler::<
                                                        u32,
                                                        Message,
                                                    >(
                                                        Box::new({
                                                            let route = {
                                                                let route_callback = (move |event_0| Message::ForgeOpenRepo(
                                                                    event_0,
                                                                ))
                                                                    .clone();
                                                                move |value| (route_callback)(value)
                                                            };
                                                            let table: Vec<Message> = options
                                                                .iter()
                                                                .cloned()
                                                                .map(route)
                                                                .collect();
                                                            move |sent: u32| table.get(sent as usize).cloned()
                                                        }),
                                                    ),
                                                    width: None,
                                                    style: ::ducktape_view_guest::wire::PickListStyle {
                                                        active: Some(::ducktape_view_guest::wire::PickFace {
                                                            background: Some(
                                                                ::ducktape_view_guest::wire::Rgba([
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.000000,
                                                                ]),
                                                            ),
                                                            text: Some(palette.colors[4]),
                                                            placeholder: None,
                                                            handle: Some(palette.colors[5]),
                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                color: Some(
                                                                    ::ducktape_view_guest::wire::Rgba([
                                                                        0.0 / 255.0,
                                                                        0.0 / 255.0,
                                                                        0.0 / 255.0,
                                                                        0.000000,
                                                                    ]),
                                                                ),
                                                                width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                radius: Some([
                                                                    ((5.0) as f32).max(0.0).min(f32::MAX),
                                                                    ((5.0) as f32).max(0.0).min(f32::MAX),
                                                                    ((5.0) as f32).max(0.0).min(f32::MAX),
                                                                    ((5.0) as f32).max(0.0).min(f32::MAX),
                                                                ]),
                                                            }),
                                                        }),
                                                        hovered: Some(::ducktape_view_guest::wire::PickFace {
                                                            background: Some(palette.colors[57]),
                                                            text: Some(palette.colors[4]),
                                                            placeholder: None,
                                                            handle: Some(palette.colors[4]),
                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                color: Some(
                                                                    ::ducktape_view_guest::wire::Rgba([
                                                                        0.0 / 255.0,
                                                                        0.0 / 255.0,
                                                                        0.0 / 255.0,
                                                                        0.000000,
                                                                    ]),
                                                                ),
                                                                width: None,
                                                                radius: None,
                                                            }),
                                                        }),
                                                        opened: Some(::ducktape_view_guest::wire::PickFace {
                                                            background: Some(palette.colors[57]),
                                                            text: Some(palette.colors[16]),
                                                            placeholder: None,
                                                            handle: Some(palette.colors[16]),
                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                color: Some(
                                                                    ::ducktape_view_guest::wire::Rgba([
                                                                        0.0 / 255.0,
                                                                        0.0 / 255.0,
                                                                        0.0 / 255.0,
                                                                        0.000000,
                                                                    ]),
                                                                ),
                                                                width: None,
                                                                radius: None,
                                                            }),
                                                        }),
                                                        opened_hovered: Some(::ducktape_view_guest::wire::PickFace {
                                                            background: Some(palette.colors[57]),
                                                            text: Some(palette.colors[16]),
                                                            placeholder: None,
                                                            handle: Some(palette.colors[16]),
                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                color: Some(
                                                                    ::ducktape_view_guest::wire::Rgba([
                                                                        0.0 / 255.0,
                                                                        0.0 / 255.0,
                                                                        0.0 / 255.0,
                                                                        0.000000,
                                                                    ]),
                                                                ),
                                                                width: None,
                                                                radius: None,
                                                            }),
                                                        }),
                                                        menu: Some(::ducktape_view_guest::wire::MenuFace {
                                                            shadow: ::ducktape_view_guest::wire::Shadow {
                                                                color: Some(palette.colors[46]),
                                                                x: None,
                                                                y: Some((3.0) as f32),
                                                                blur: Some((12.0) as f32),
                                                            },
                                                            background: Some(palette.colors[3]),
                                                            text: Some(palette.colors[4]),
                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                color: Some(palette.colors[39]),
                                                                width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                radius: Some([
                                                                    ((11.0) as f32).max(0.0).min(f32::MAX),
                                                                    ((11.0) as f32).max(0.0).min(f32::MAX),
                                                                    ((11.0) as f32).max(0.0).min(f32::MAX),
                                                                    ((11.0) as f32).max(0.0).min(f32::MAX),
                                                                ]),
                                                            }),
                                                            selected_text: Some(palette.colors[4]),
                                                            selected_background: Some(palette.colors[91]),
                                                        }),
                                                    },
                                                    settings: Box::new(::ducktape_view_guest::wire::PickOptions {
                                                        menu_height: None,
                                                        padding: Some(((6.0) as f32).max(0.0).min(f32::MAX)),
                                                        text_size: Some(
                                                            ((13.0) as f32).max(f32::EPSILON).min(f32::MAX),
                                                        ),
                                                        line_height: None,
                                                        shaping: None,
                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                "Geist".into(),
                                                            ),
                                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                        }),
                                                        handle: Some(::ducktape_view_guest::wire::PickHandle::Arrow {
                                                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                        }),
                                                        on_open: None,
                                                        on_close: None,
                                                    }),
                                                }
                                            }
                                        });
                                    if !(self.branches).is_empty()  {
                                        children
                                            .push({
                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                children
                                                    .push(
                                                        self
                                                            .icon(
                                                                format!("{}/Icon@2922", use_scope),
                                                                "branch",
                                                                10f32,
                                                                (palette).colors[7usize],
                                                                "@media:16",
                                                            ),
                                                    );
                                                children
                                                    .push({
                                                        let node_scope = format!("{}/branch-pick", use_scope);
                                                        {
                                                            let options = crate::host::branch_names(
                                                                ::std::convert::AsRef::as_ref(&(self.branches)),
                                                            );
                                                            let selected = crate::host::pinned_branch(
                                                                ::std::convert::AsRef::as_ref(
                                                                    &(crate::host::forge_tree_branch(
                                                                        ::std::convert::AsRef::as_ref(&(self.branches)),
                                                                        ::std::convert::AsRef::as_ref(&(self.tree_pick)),
                                                                        ::std::convert::AsRef::as_ref(&(self.tree_rev)),
                                                                    )),
                                                                ),
                                                            );
                                                            ::ducktape_view_guest::wire::Node::PickList {
                                                                key: node_scope.clone(),
                                                                options: options
                                                                    .iter()
                                                                    .map(|option| option.to_string())
                                                                    .collect(),
                                                                selected: selected
                                                                    .as_ref()
                                                                    .and_then(|chosen| {
                                                                        options.iter().position(|option| option == chosen)
                                                                    })
                                                                    .map(|index| index as u32),
                                                                placeholder: Some(
                                                                    (crate::host::commit_label(
                                                                        ::std::convert::AsRef::as_ref(&(self.tree_rev)),
                                                                    ))
                                                                        .to_string(),
                                                                ),
                                                                on_select: ::ducktape_view_guest::slots::handler::<
                                                                    u32,
                                                                    Message,
                                                                >(
                                                                    Box::new({
                                                                        let route = {
                                                                            let route_callback = (move |event_0| Message::ForgePickBranch(
                                                                                event_0,
                                                                            ))
                                                                                .clone();
                                                                            move |value| (route_callback)(value)
                                                                        };
                                                                        let table: Vec<Message> = options
                                                                            .iter()
                                                                            .cloned()
                                                                            .map(route)
                                                                            .collect();
                                                                        move |sent: u32| table.get(sent as usize).cloned()
                                                                    }),
                                                                ),
                                                                width: None,
                                                                style: ::ducktape_view_guest::wire::PickListStyle {
                                                                    active: Some(::ducktape_view_guest::wire::PickFace {
                                                                        background: Some(
                                                                            ::ducktape_view_guest::wire::Rgba([
                                                                                0.0 / 255.0,
                                                                                0.0 / 255.0,
                                                                                0.0 / 255.0,
                                                                                0.000000,
                                                                            ]),
                                                                        ),
                                                                        text: Some(palette.colors[4]),
                                                                        placeholder: Some(palette.colors[5]),
                                                                        handle: Some(palette.colors[5]),
                                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                                            color: Some(
                                                                                ::ducktape_view_guest::wire::Rgba([
                                                                                    0.0 / 255.0,
                                                                                    0.0 / 255.0,
                                                                                    0.0 / 255.0,
                                                                                    0.000000,
                                                                                ]),
                                                                            ),
                                                                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                            radius: Some([
                                                                                ((5.0) as f32).max(0.0).min(f32::MAX),
                                                                                ((5.0) as f32).max(0.0).min(f32::MAX),
                                                                                ((5.0) as f32).max(0.0).min(f32::MAX),
                                                                                ((5.0) as f32).max(0.0).min(f32::MAX),
                                                                            ]),
                                                                        }),
                                                                    }),
                                                                    hovered: Some(::ducktape_view_guest::wire::PickFace {
                                                                        background: Some(palette.colors[57]),
                                                                        text: Some(palette.colors[4]),
                                                                        placeholder: Some(palette.colors[4]),
                                                                        handle: Some(palette.colors[4]),
                                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                                            color: Some(
                                                                                ::ducktape_view_guest::wire::Rgba([
                                                                                    0.0 / 255.0,
                                                                                    0.0 / 255.0,
                                                                                    0.0 / 255.0,
                                                                                    0.000000,
                                                                                ]),
                                                                            ),
                                                                            width: None,
                                                                            radius: None,
                                                                        }),
                                                                    }),
                                                                    opened: Some(::ducktape_view_guest::wire::PickFace {
                                                                        background: Some(palette.colors[57]),
                                                                        text: Some(palette.colors[16]),
                                                                        placeholder: Some(palette.colors[16]),
                                                                        handle: Some(palette.colors[16]),
                                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                                            color: Some(
                                                                                ::ducktape_view_guest::wire::Rgba([
                                                                                    0.0 / 255.0,
                                                                                    0.0 / 255.0,
                                                                                    0.0 / 255.0,
                                                                                    0.000000,
                                                                                ]),
                                                                            ),
                                                                            width: None,
                                                                            radius: None,
                                                                        }),
                                                                    }),
                                                                    opened_hovered: Some(::ducktape_view_guest::wire::PickFace {
                                                                        background: Some(palette.colors[57]),
                                                                        text: Some(palette.colors[16]),
                                                                        placeholder: Some(palette.colors[16]),
                                                                        handle: Some(palette.colors[16]),
                                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                                            color: Some(
                                                                                ::ducktape_view_guest::wire::Rgba([
                                                                                    0.0 / 255.0,
                                                                                    0.0 / 255.0,
                                                                                    0.0 / 255.0,
                                                                                    0.000000,
                                                                                ]),
                                                                            ),
                                                                            width: None,
                                                                            radius: None,
                                                                        }),
                                                                    }),
                                                                    menu: Some(::ducktape_view_guest::wire::MenuFace {
                                                                        shadow: ::ducktape_view_guest::wire::Shadow {
                                                                            color: Some(palette.colors[46]),
                                                                            x: None,
                                                                            y: Some((3.0) as f32),
                                                                            blur: Some((12.0) as f32),
                                                                        },
                                                                        background: Some(palette.colors[3]),
                                                                        text: Some(palette.colors[4]),
                                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                                            color: Some(palette.colors[39]),
                                                                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                            radius: Some([
                                                                                ((11.0) as f32).max(0.0).min(f32::MAX),
                                                                                ((11.0) as f32).max(0.0).min(f32::MAX),
                                                                                ((11.0) as f32).max(0.0).min(f32::MAX),
                                                                                ((11.0) as f32).max(0.0).min(f32::MAX),
                                                                            ]),
                                                                        }),
                                                                        selected_text: Some(palette.colors[4]),
                                                                        selected_background: Some(palette.colors[91]),
                                                                    }),
                                                                },
                                                                settings: Box::new(::ducktape_view_guest::wire::PickOptions {
                                                                    menu_height: None,
                                                                    padding: Some(((6.0) as f32).max(0.0).min(f32::MAX)),
                                                                    text_size: Some(
                                                                        ((12.0) as f32).max(f32::EPSILON).min(f32::MAX),
                                                                    ),
                                                                    line_height: None,
                                                                    shaping: None,
                                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                            "Geist Mono".into(),
                                                                        ),
                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                    }),
                                                                    handle: Some(::ducktape_view_guest::wire::PickHandle::Arrow {
                                                                        size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                    }),
                                                                    on_open: None,
                                                                    on_close: None,
                                                                }),
                                                            }
                                                        }
                                                    });
                                                ::ducktape_view_guest::wire::Node::Linear {
                                                    max_width: None,
                                                    clip: false,
                                                    key: format!("{}/@layout:178", use_scope),
                                                    wrap: None,
                                                    axis: ::ducktape_view_guest::wire::Axis::Row,
                                                    spacing: Some((5.0) as f32),
                                                    padding: None,
                                                    width: None,
                                                    height: None,
                                                    align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                    background: None,
                                                    border: None,
                                                    children: children,
                                                }
                                            });
                                    }
                                    children
                                        .push(::ducktape_view_guest::wire::Node::Space {
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: None,
                                        });
                                    if (self.forge_item_number > 0)
                                        && (self.item_phase == "ready") 
                                    {
                                        children
                                            .push(
                                                self
                                                    .render_back_to_list_9(
                                                        palette,
                                                        format!("{}/BackToList@2944", use_scope),
                                                        (move || Message::ForgeCloseItem).clone(),
                                                    ),
                                            );
                                    }
                                    if (self.forge_item_number > 0)
                                        && (self.item_phase != "ready") 
                                    {
                                        children
                                            .push(::ducktape_view_guest::wire::Node::Button {
                                                checked: None,
                                                expanded: None,
                                                description: None,
                                                key: format!("{}/@button:205", use_scope),
                                                content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                    String::from("Back to tracker"),
                                                ),
                                                label: None,
                                                on_press: Some(
                                                    ::ducktape_view_guest::slots::message(
                                                        Message::ForgeCloseItem,
                                                    ),
                                                ),
                                                width: None,
                                                height: None,
                                                padding: Some(
                                                    ::ducktape_view_guest::wire::Edges::all((6.0) as f32),
                                                ),
                                                style: ::ducktape_view_guest::wire::ButtonStyle {
                                                    preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                    recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                        base: ::ducktape_view_guest::wire::Face {
                                                            background: Some(palette.colors[12]),
                                                            text: Some(palette.colors[13]),
                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                color: Some(palette.colors[40]),
                                                                width: Some(1.0),
                                                                radius: Some([9.0; 4]),
                                                            }),
                                                        },
                                                        hover_background: Some(palette.colors[14]),
                                                        pressed_background: Some(palette.colors[6]),
                                                        disabled_background: None,
                                                        disabled_text: None,
                                                        disabled_opacity: Some(0.5f32),
                                                        focus_ring: Some(palette.colors[42]),
                                                        text_size: Some(12.5f32),
                                                        line_height: None,
                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                "Geist".into(),
                                                            ),
                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                        }),
                                                    }),
                                                    active: ::ducktape_view_guest::wire::Face::default(),
                                                    hovered: None,
                                                    pressed: None,
                                                    disabled: None,
                                                },
                                            });
                                    }
                                    ::ducktape_view_guest::wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:137", use_scope),
                                        wrap: None,
                                        axis: ::ducktape_view_guest::wire::Axis::Row,
                                        spacing: Some((9.0) as f32),
                                        padding: None,
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: None,
                                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                        background: None,
                                        border: None,
                                        children: children,
                                    }
                                }),
                            });
                        children
                            .push(::ducktape_view_guest::wire::Node::Container {
                                shadow: ::ducktape_view_guest::wire::Shadow {
                                    color: None,
                                    x: None,
                                    y: None,
                                    blur: None,
                                },
                                max_width: None,
                                max_height: None,
                                clip: false,
                                key: format!("{}/@container:209", use_scope),
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                padding: None,
                                align_x: None,
                                align_y: None,
                                background: (Some(palette.colors[60]))
                                    .map(::ducktape_view_guest::wire::Background::Color),
                                border: None,
                                snap: None,
                                content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                    width: Some(
                                        ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                    ),
                                    height: Some(
                                        ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                    ),
                                }),
                            });
                        children
                            .push(::ducktape_view_guest::wire::Node::Container {
                                shadow: ::ducktape_view_guest::wire::Shadow {
                                    color: None,
                                    x: None,
                                    y: None,
                                    blur: None,
                                },
                                max_width: None,
                                max_height: None,
                                clip: false,
                                key: format!("{}/@container:227", use_scope),
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                padding: Some(::ducktape_view_guest::wire::Edges {
                                    top: (0.0) as f32,
                                    right: (16.0) as f32,
                                    bottom: (0.0) as f32,
                                    left: (16.0) as f32,
                                }),
                                align_x: None,
                                align_y: None,
                                background: (None)
                                    .map(::ducktape_view_guest::wire::Background::Color),
                                border: None,
                                snap: None,
                                content: Box::new({
                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                    children
                                        .push(::ducktape_view_guest::wire::Node::Button {
                                            checked: Some(self.tab == "code" ),
                                            expanded: None,
                                            description: None,
                                            key: format!("{}/@button:237", use_scope),
                                            content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                Box::new(
                                                    self
                                                        .render_tab_label_10(
                                                            palette,
                                                            format!("{}/TabLabel@2986", use_scope),
                                                        ),
                                                ),
                                            ),
                                            label: Some(String::from("Browse the code".to_owned())),
                                            on_press: Some(
                                                ::ducktape_view_guest::slots::message(
                                                    (move |event_0| Message::SelectForgeTab(
                                                        event_0,
                                                    ))("code".to_owned()),
                                                ),
                                            ),
                                            width: None,
                                            height: None,
                                            padding: Some(
                                                ::ducktape_view_guest::wire::Edges::all((0.0) as f32),
                                            ),
                                            style: ::ducktape_view_guest::wire::ButtonStyle {
                                                preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                    base: ::ducktape_view_guest::wire::Face {
                                                        background: Some(
                                                            ::ducktape_view_guest::wire::Rgba([
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.000000,
                                                            ]),
                                                        ),
                                                        text: Some(palette.colors[4]),
                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                            color: None,
                                                            width: None,
                                                            radius: Some([8.0; 4]),
                                                        }),
                                                    },
                                                    hover_background: Some(palette.colors[14]),
                                                    pressed_background: Some(palette.colors[39]),
                                                    disabled_background: None,
                                                    disabled_text: None,
                                                    disabled_opacity: Some(0.5f32),
                                                    focus_ring: Some(palette.colors[42]),
                                                    text_size: Some(12.5f32),
                                                    line_height: None,
                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                            "Geist".into(),
                                                        ),
                                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                    }),
                                                }),
                                                active: ::ducktape_view_guest::wire::Face {
                                                    background: Some(
                                                        ::ducktape_view_guest::wire::Rgba([
                                                            0.0 / 255.0,
                                                            0.0 / 255.0,
                                                            0.0 / 255.0,
                                                            0.000000,
                                                        ]),
                                                    ),
                                                    text: Some(palette.colors[4]),
                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                        color: Some(
                                                            ::ducktape_view_guest::wire::Rgba([
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.000000,
                                                            ]),
                                                        ),
                                                        width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                        radius: Some([
                                                            ((0.0) as f32).max(0.0).min(f32::MAX),
                                                            ((0.0) as f32).max(0.0).min(f32::MAX),
                                                            ((0.0) as f32).max(0.0).min(f32::MAX),
                                                            ((0.0) as f32).max(0.0).min(f32::MAX),
                                                        ]),
                                                    }),
                                                },
                                                hovered: Some(::ducktape_view_guest::wire::Face {
                                                    background: Some(
                                                        ::ducktape_view_guest::wire::Rgba([
                                                            0.0 / 255.0,
                                                            0.0 / 255.0,
                                                            0.0 / 255.0,
                                                            0.000000,
                                                        ]),
                                                    ),
                                                    text: Some(palette.colors[4]),
                                                    border: None,
                                                }),
                                                pressed: Some(::ducktape_view_guest::wire::Face {
                                                    background: Some(
                                                        ::ducktape_view_guest::wire::Rgba([
                                                            0.0 / 255.0,
                                                            0.0 / 255.0,
                                                            0.0 / 255.0,
                                                            0.000000,
                                                        ]),
                                                    ),
                                                    text: Some(palette.colors[4]),
                                                    border: None,
                                                }),
                                                disabled: None,
                                            },
                                        });
                                    children
                                        .push(::ducktape_view_guest::wire::Node::Button {
                                            checked: Some(self.tab == "pulls" ),
                                            expanded: None,
                                            description: None,
                                            key: format!("{}/@button:251", use_scope),
                                            content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                Box::new(
                                                    self
                                                        .render_tab_label_11(
                                                            palette,
                                                            format!("{}/TabLabel@3000", use_scope),
                                                        ),
                                                ),
                                            ),
                                            label: Some(String::from("Show pull requests".to_owned())),
                                            on_press: Some(
                                                ::ducktape_view_guest::slots::message(
                                                    (move |event_0| Message::SelectForgeTab(
                                                        event_0,
                                                    ))("pulls".to_owned()),
                                                ),
                                            ),
                                            width: None,
                                            height: None,
                                            padding: Some(
                                                ::ducktape_view_guest::wire::Edges::all((0.0) as f32),
                                            ),
                                            style: ::ducktape_view_guest::wire::ButtonStyle {
                                                preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                    base: ::ducktape_view_guest::wire::Face {
                                                        background: Some(
                                                            ::ducktape_view_guest::wire::Rgba([
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.000000,
                                                            ]),
                                                        ),
                                                        text: Some(palette.colors[4]),
                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                            color: None,
                                                            width: None,
                                                            radius: Some([8.0; 4]),
                                                        }),
                                                    },
                                                    hover_background: Some(palette.colors[14]),
                                                    pressed_background: Some(palette.colors[39]),
                                                    disabled_background: None,
                                                    disabled_text: None,
                                                    disabled_opacity: Some(0.5f32),
                                                    focus_ring: Some(palette.colors[42]),
                                                    text_size: Some(12.5f32),
                                                    line_height: None,
                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                            "Geist".into(),
                                                        ),
                                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                    }),
                                                }),
                                                active: ::ducktape_view_guest::wire::Face {
                                                    background: Some(
                                                        ::ducktape_view_guest::wire::Rgba([
                                                            0.0 / 255.0,
                                                            0.0 / 255.0,
                                                            0.0 / 255.0,
                                                            0.000000,
                                                        ]),
                                                    ),
                                                    text: Some(palette.colors[4]),
                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                        color: Some(
                                                            ::ducktape_view_guest::wire::Rgba([
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.000000,
                                                            ]),
                                                        ),
                                                        width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                        radius: Some([
                                                            ((0.0) as f32).max(0.0).min(f32::MAX),
                                                            ((0.0) as f32).max(0.0).min(f32::MAX),
                                                            ((0.0) as f32).max(0.0).min(f32::MAX),
                                                            ((0.0) as f32).max(0.0).min(f32::MAX),
                                                        ]),
                                                    }),
                                                },
                                                hovered: Some(::ducktape_view_guest::wire::Face {
                                                    background: Some(
                                                        ::ducktape_view_guest::wire::Rgba([
                                                            0.0 / 255.0,
                                                            0.0 / 255.0,
                                                            0.0 / 255.0,
                                                            0.000000,
                                                        ]),
                                                    ),
                                                    text: Some(palette.colors[4]),
                                                    border: None,
                                                }),
                                                pressed: Some(::ducktape_view_guest::wire::Face {
                                                    background: Some(
                                                        ::ducktape_view_guest::wire::Rgba([
                                                            0.0 / 255.0,
                                                            0.0 / 255.0,
                                                            0.0 / 255.0,
                                                            0.000000,
                                                        ]),
                                                    ),
                                                    text: Some(palette.colors[4]),
                                                    border: None,
                                                }),
                                                disabled: None,
                                            },
                                        });
                                    children
                                        .push(::ducktape_view_guest::wire::Node::Button {
                                            checked: Some(self.tab == "issues" ),
                                            expanded: None,
                                            description: None,
                                            key: format!("{}/@button:265", use_scope),
                                            content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                Box::new(
                                                    self
                                                        .render_tab_label_12(
                                                            palette,
                                                            format!("{}/TabLabel@3014", use_scope),
                                                        ),
                                                ),
                                            ),
                                            label: Some(String::from("Show issues".to_owned())),
                                            on_press: Some(
                                                ::ducktape_view_guest::slots::message(
                                                    (move |event_0| Message::SelectForgeTab(
                                                        event_0,
                                                    ))("issues".to_owned()),
                                                ),
                                            ),
                                            width: None,
                                            height: None,
                                            padding: Some(
                                                ::ducktape_view_guest::wire::Edges::all((0.0) as f32),
                                            ),
                                            style: ::ducktape_view_guest::wire::ButtonStyle {
                                                preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                    base: ::ducktape_view_guest::wire::Face {
                                                        background: Some(
                                                            ::ducktape_view_guest::wire::Rgba([
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.000000,
                                                            ]),
                                                        ),
                                                        text: Some(palette.colors[4]),
                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                            color: None,
                                                            width: None,
                                                            radius: Some([8.0; 4]),
                                                        }),
                                                    },
                                                    hover_background: Some(palette.colors[14]),
                                                    pressed_background: Some(palette.colors[39]),
                                                    disabled_background: None,
                                                    disabled_text: None,
                                                    disabled_opacity: Some(0.5f32),
                                                    focus_ring: Some(palette.colors[42]),
                                                    text_size: Some(12.5f32),
                                                    line_height: None,
                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                            "Geist".into(),
                                                        ),
                                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                    }),
                                                }),
                                                active: ::ducktape_view_guest::wire::Face {
                                                    background: Some(
                                                        ::ducktape_view_guest::wire::Rgba([
                                                            0.0 / 255.0,
                                                            0.0 / 255.0,
                                                            0.0 / 255.0,
                                                            0.000000,
                                                        ]),
                                                    ),
                                                    text: Some(palette.colors[4]),
                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                        color: Some(
                                                            ::ducktape_view_guest::wire::Rgba([
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.000000,
                                                            ]),
                                                        ),
                                                        width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                        radius: Some([
                                                            ((0.0) as f32).max(0.0).min(f32::MAX),
                                                            ((0.0) as f32).max(0.0).min(f32::MAX),
                                                            ((0.0) as f32).max(0.0).min(f32::MAX),
                                                            ((0.0) as f32).max(0.0).min(f32::MAX),
                                                        ]),
                                                    }),
                                                },
                                                hovered: Some(::ducktape_view_guest::wire::Face {
                                                    background: Some(
                                                        ::ducktape_view_guest::wire::Rgba([
                                                            0.0 / 255.0,
                                                            0.0 / 255.0,
                                                            0.0 / 255.0,
                                                            0.000000,
                                                        ]),
                                                    ),
                                                    text: Some(palette.colors[4]),
                                                    border: None,
                                                }),
                                                pressed: Some(::ducktape_view_guest::wire::Face {
                                                    background: Some(
                                                        ::ducktape_view_guest::wire::Rgba([
                                                            0.0 / 255.0,
                                                            0.0 / 255.0,
                                                            0.0 / 255.0,
                                                            0.000000,
                                                        ]),
                                                    ),
                                                    text: Some(palette.colors[4]),
                                                    border: None,
                                                }),
                                                disabled: None,
                                            },
                                        });
                                    children
                                        .push(::ducktape_view_guest::wire::Node::Space {
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: None,
                                        });
                                    ::ducktape_view_guest::wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:232", use_scope),
                                        wrap: None,
                                        axis: ::ducktape_view_guest::wire::Axis::Row,
                                        spacing: Some((18.0) as f32),
                                        padding: None,
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: None,
                                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                        background: None,
                                        border: None,
                                        children: children,
                                    }
                                }),
                            });
                        children
                            .push(::ducktape_view_guest::wire::Node::Container {
                                shadow: ::ducktape_view_guest::wire::Shadow {
                                    color: None,
                                    x: None,
                                    y: None,
                                    blur: None,
                                },
                                max_width: None,
                                max_height: None,
                                clip: false,
                                key: format!("{}/@container:280", use_scope),
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                padding: None,
                                align_x: None,
                                align_y: None,
                                background: (Some(palette.colors[60]))
                                    .map(::ducktape_view_guest::wire::Background::Color),
                                border: None,
                                snap: None,
                                content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                    width: Some(
                                        ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                    ),
                                    height: Some(
                                        ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                    ),
                                }),
                            });
                        if self.forge_item_number <= 0  {
                            children
                                .push({
                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                    if self.tab == "code"  {
                                        children
                                            .push({
                                                let node_scope = format!("{}/code", use_scope);
                                                self.render_code(
                                                    palette,
                                                    node_scope.clone(),
                                                    (move |event_0| Message::ForgeOpenDir(event_0)).clone(),
                                                    (move |event_0| Message::ForgeOpenFile(event_0)).clone(),
                                                    (move |event_0| Message::OpenMessageLink(event_0)).clone(),
                                                    (move |event_0, event_1| Message::TreeResized(
                                                        event_0,
                                                        event_1,
                                                    ))
                                                        .clone(),
                                                )
                                            });
                                    }
                                    if (!(self.tab == "code")) && (self.tab == "issues")  {
                                        children
                                            .push(
                                                self
                                                    .render_forge_tracker_list_44(
                                                        palette,
                                                        format!("{}/ForgeTrackerList@3062", use_scope),
                                                        (move |event_0| Message::ForgeOpenItem(event_0)).clone(),
                                                    ),
                                            );
                                    }
                                    if (!((self.tab == "code") || (self.tab == "issues")))
                                        && (self.tab == "pulls") 
                                    {
                                        children
                                            .push(
                                                self
                                                    .render_forge_tracker_list_46(
                                                        palette,
                                                        format!("{}/ForgeTrackerList@3070", use_scope),
                                                        (move |event_0| Message::ForgeOpenItem(event_0)).clone(),
                                                    ),
                                            );
                                    }
                                    ::ducktape_view_guest::wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:287", use_scope),
                                        wrap: None,
                                        axis: ::ducktape_view_guest::wire::Axis::Column,
                                        spacing: None,
                                        padding: None,
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: Some(::ducktape_view_guest::wire::Length::Fill),
                                        align: None,
                                        background: None,
                                        border: None,
                                        children: children,
                                    }
                                });
                        }
                        if (self.forge_item_number > 0)
                            && (self.item_phase == "loading") 
                        {
                            children
                                .push(::ducktape_view_guest::wire::Node::Container {
                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                        color: None,
                                        x: None,
                                        y: None,
                                        blur: None,
                                    },
                                    max_width: None,
                                    max_height: None,
                                    clip: false,
                                    key: format!("{}/@container:341", use_scope),
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: Some(::ducktape_view_guest::wire::Length::Fill),
                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                        top: (16.0) as f32,
                                        right: (16.0) as f32,
                                        bottom: (16.0) as f32,
                                        left: (16.0) as f32,
                                    }),
                                    align_x: None,
                                    align_y: None,
                                    background: (None)
                                        .map(::ducktape_view_guest::wire::Background::Color),
                                    border: None,
                                    snap: None,
                                    content: Box::new(
                                        self
                                            .render_empty_plate_47(
                                                palette,
                                                format!("{}/EmptyPlate@3089", use_scope),
                                            ),
                                    ),
                                });
                        }
                        if (self.forge_item_number > 0)
                            && (self.item_phase == "failed") 
                        {
                            children
                                .push(::ducktape_view_guest::wire::Node::Container {
                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                        color: None,
                                        x: None,
                                        y: None,
                                        blur: None,
                                    },
                                    max_width: None,
                                    max_height: None,
                                    clip: false,
                                    key: format!("{}/@container:348", use_scope),
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: Some(::ducktape_view_guest::wire::Length::Fill),
                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                        top: (16.0) as f32,
                                        right: (16.0) as f32,
                                        bottom: (16.0) as f32,
                                        left: (16.0) as f32,
                                    }),
                                    align_x: None,
                                    align_y: None,
                                    background: (None)
                                        .map(::ducktape_view_guest::wire::Background::Color),
                                    border: None,
                                    snap: None,
                                    content: Box::new(
                                        self
                                            .render_empty_plate_48(
                                                palette,
                                                format!("{}/EmptyPlate@3096", use_scope),
                                            ),
                                    ),
                                });
                        }
                        if (self.forge_item_number > 0) && (self.item_phase == "ready") 
                        {
                            children
                                .push({
                                    let node_scope = format!("{}/item-detail", use_scope);
                                    ::ducktape_view_guest::wire::Node::Scroll {
                                        on_scroll: None,
                                        virtual_rows: true,
                                        key: node_scope.clone(),
                                        direction: ::ducktape_view_guest::wire::ScrollDirection::Vertical,
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: Some(::ducktape_view_guest::wire::Length::Fill),
                                        bar_hidden: false,
                                        bar_width: None,
                                        bar_margin: None,
                                        scroller_width: None,
                                        bar_spacing: None,
                                        anchor_x: ::ducktape_view_guest::wire::ScrollAnchor::Start,
                                        anchor_y: ::ducktape_view_guest::wire::ScrollAnchor::Start,
                                        auto_scroll: (false),
                                        background: None,
                                        border: None,
                                        content: Box::new({
                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                            children
                                                .push({
                                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                    children
                                                        .push(::ducktape_view_guest::wire::Node::Container {
                                                            shadow: ::ducktape_view_guest::wire::Shadow {
                                                                color: None,
                                                                x: None,
                                                                y: None,
                                                                blur: None,
                                                            },
                                                            max_width: None,
                                                            max_height: None,
                                                            clip: true,
                                                            key: format!("{}/@container:373", use_scope),
                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                            height: None,
                                                            padding: None,
                                                            align_x: None,
                                                            align_y: None,
                                                            background: (None)
                                                                .map(::ducktape_view_guest::wire::Background::Color),
                                                            border: None,
                                                            snap: None,
                                                            content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                                                options: ::ducktape_view_guest::wire::TextOptions {
                                                                    height: None,
                                                                    align_y: None,
                                                                    line_height: None,
                                                                    shaping: None,
                                                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                    tracking: 0.0f32,
                                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                            "Geist".into(),
                                                                        ),
                                                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                    }),
                                                                },
                                                                key: format!("{}/@text:374", use_scope),
                                                                size: Some(((16.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                color: Some(palette.colors[7]),
                                                                font: ::ducktape_view_guest::wire::Font {
                                                                    monospace: false,
                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                },
                                                                width: None,
                                                                align_x: None,
                                                                content: (self.forge_item_title.to_owned()).to_string(),
                                                            }),
                                                        });
                                                    if self.forge_item_kind == "pr"  {
                                                        children
                                                            .push(
                                                                self
                                                                    .render_pr_state_pill_49(
                                                                        palette,
                                                                        format!("{}/PrStatePill@3124", use_scope),
                                                                    ),
                                                            );
                                                    }
                                                    if self.forge_item_kind != "pr"  {
                                                        children
                                                            .push(
                                                                self
                                                                    .render_status_badge_54(
                                                                        palette,
                                                                        format!("{}/StatusBadge@3126", use_scope),
                                                                    ),
                                                            );
                                                    }
                                                    children
                                                        .push(::ducktape_view_guest::wire::Node::Button {
                                                            checked: None,
                                                            expanded: None,
                                                            description: None,
                                                            key: format!("{}/@button:387", use_scope),
                                                            content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                                Box::new(::ducktape_view_guest::wire::Node::Container {
                                                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                                                        color: None,
                                                                        x: None,
                                                                        y: None,
                                                                        blur: None,
                                                                    },
                                                                    max_width: None,
                                                                    max_height: None,
                                                                    clip: false,
                                                                    key: format!("{}/@container:394", use_scope),
                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                    height: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                    padding: None,
                                                                    align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                    align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                                                    background: (None)
                                                                        .map(::ducktape_view_guest::wire::Background::Color),
                                                                    border: None,
                                                                    snap: None,
                                                                    content: Box::new(
                                                                        self
                                                                            .icon(
                                                                                format!("{}/Icon@3143", use_scope),
                                                                                "link",
                                                                                13f32,
                                                                                (palette).colors[73usize],
                                                                                "@media:22",
                                                                            ),
                                                                    ),
                                                                }),
                                                            ),
                                                            label: Some(String::from("Copy item link".to_owned())),
                                                            on_press: Some(
                                                                ::ducktape_view_guest::slots::message(
                                                                    (move |event_0, event_1| Message::CopyToClipboard(
                                                                        event_0,
                                                                        event_1,
                                                                    ))(
                                                                        crate::host::duck_forge_item_link(
                                                                            ::std::convert::AsRef::as_ref(&(self.open_repo)),
                                                                            self.forge_item_number,
                                                                            ::std::convert::AsRef::as_ref(&(self.network_chain_id)),
                                                                        ),
                                                                        "Item link copied".to_owned(),
                                                                    ),
                                                                ),
                                                            ),
                                                            width: Some(
                                                                ::ducktape_view_guest::wire::Length::Fixed((26.0) as f32),
                                                            ),
                                                            height: Some(
                                                                ::ducktape_view_guest::wire::Length::Fixed((26.0) as f32),
                                                            ),
                                                            padding: Some(
                                                                ::ducktape_view_guest::wire::Edges::all((0.0) as f32),
                                                            ),
                                                            style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                    base: ::ducktape_view_guest::wire::Face {
                                                                        background: Some(
                                                                            ::ducktape_view_guest::wire::Rgba([
                                                                                0.0 / 255.0,
                                                                                0.0 / 255.0,
                                                                                0.0 / 255.0,
                                                                                0.000000,
                                                                            ]),
                                                                        ),
                                                                        text: Some(palette.colors[4]),
                                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                                            color: None,
                                                                            width: None,
                                                                            radius: Some([7.0; 4]),
                                                                        }),
                                                                    },
                                                                    hover_background: Some(palette.colors[14]),
                                                                    pressed_background: Some(palette.colors[39]),
                                                                    disabled_background: None,
                                                                    disabled_text: None,
                                                                    disabled_opacity: Some(0.5f32),
                                                                    focus_ring: Some(palette.colors[42]),
                                                                    text_size: Some(13.5f32),
                                                                    line_height: None,
                                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                            "Geist".into(),
                                                                        ),
                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                    }),
                                                                }),
                                                                active: ::ducktape_view_guest::wire::Face {
                                                                    background: Some(
                                                                        ::ducktape_view_guest::wire::Rgba([
                                                                            0.0 / 255.0,
                                                                            0.0 / 255.0,
                                                                            0.0 / 255.0,
                                                                            0.000000,
                                                                        ]),
                                                                    ),
                                                                    text: Some(palette.colors[5]),
                                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                                        color: None,
                                                                        width: None,
                                                                        radius: Some([
                                                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                        ]),
                                                                    }),
                                                                },
                                                                hovered: Some(::ducktape_view_guest::wire::Face {
                                                                    background: Some({
                                                                        let mut color = palette.colors[4];
                                                                        color.0[3] = 0.100000;
                                                                        color
                                                                    }),
                                                                    text: Some(palette.colors[4]),
                                                                    border: None,
                                                                }),
                                                                pressed: Some(::ducktape_view_guest::wire::Face {
                                                                    background: Some({
                                                                        let mut color = palette.colors[4];
                                                                        color.0[3] = 0.150000;
                                                                        color
                                                                    }),
                                                                    text: None,
                                                                    border: None,
                                                                }),
                                                                disabled: None,
                                                            },
                                                        });
                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                        max_width: None,
                                                        clip: false,
                                                        key: format!("{}/@layout:368", use_scope),
                                                        wrap: None,
                                                        axis: ::ducktape_view_guest::wire::Axis::Row,
                                                        spacing: Some((9.0) as f32),
                                                        padding: None,
                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                        height: None,
                                                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                        background: None,
                                                        border: None,
                                                        children: children,
                                                    }
                                                });
                                            children
                                                .push({
                                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                    if !(self.forge_item_author).is_empty()  {
                                                        children
                                                            .push(::ducktape_view_guest::wire::Node::Container {
                                                                shadow: ::ducktape_view_guest::wire::Shadow {
                                                                    color: None,
                                                                    x: None,
                                                                    y: None,
                                                                    blur: None,
                                                                },
                                                                max_width: Some(((160.0) as f32).max(0.0).min(f32::MAX)),
                                                                max_height: None,
                                                                clip: true,
                                                                key: format!("{}/@container:414", use_scope),
                                                                width: None,
                                                                height: None,
                                                                padding: None,
                                                                align_x: None,
                                                                align_y: None,
                                                                background: (None)
                                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                                border: None,
                                                                snap: None,
                                                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                        height: None,
                                                                        align_y: None,
                                                                        line_height: None,
                                                                        shaping: None,
                                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                        tracking: 0.0f32,
                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                "Geist Mono".into(),
                                                                            ),
                                                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                        }),
                                                                    },
                                                                    key: format!("{}/@text:415", use_scope),
                                                                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                    color: Some(palette.colors[71]),
                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                        monospace: false,
                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                    },
                                                                    width: None,
                                                                    align_x: None,
                                                                    content: (self.forge_item_author.to_owned()).to_string(),
                                                                }),
                                                            });
                                                    }
                                                    if !(self.forge_item_branches).is_empty()  {
                                                        children
                                                            .push(::ducktape_view_guest::wire::Node::Container {
                                                                shadow: ::ducktape_view_guest::wire::Shadow {
                                                                    color: None,
                                                                    x: None,
                                                                    y: None,
                                                                    blur: None,
                                                                },
                                                                max_width: None,
                                                                max_height: None,
                                                                clip: true,
                                                                key: format!("{}/@container:422", use_scope),
                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                height: None,
                                                                padding: None,
                                                                align_x: None,
                                                                align_y: None,
                                                                background: (None)
                                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                                border: None,
                                                                snap: None,
                                                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                        height: None,
                                                                        align_y: None,
                                                                        line_height: None,
                                                                        shaping: None,
                                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                        tracking: 0.0f32,
                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                "Geist Mono".into(),
                                                                            ),
                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                        }),
                                                                    },
                                                                    key: format!("{}/@text:423", use_scope),
                                                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                    color: Some(palette.colors[71]),
                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                        monospace: false,
                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                    },
                                                                    width: None,
                                                                    align_x: None,
                                                                    content: (self.forge_item_branches.to_owned()).to_string(),
                                                                }),
                                                            });
                                                    }
                                                    if (self.forge_item_branches).is_empty() {
                                                        children
                                                            .push(::ducktape_view_guest::wire::Node::Space {
                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                height: None,
                                                            });
                                                    }
                                                    if self.forge_item_files_changed > 0  {
                                                        children
                                                            .push(
                                                                self
                                                                    .render_diff_count_56(
                                                                        palette,
                                                                        format!("{}/DiffCount@3175", use_scope),
                                                                    ),
                                                            );
                                                    }
                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                        max_width: None,
                                                        clip: false,
                                                        key: format!("{}/@layout:408", use_scope),
                                                        wrap: None,
                                                        axis: ::ducktape_view_guest::wire::Axis::Row,
                                                        spacing: Some((10.0) as f32),
                                                        padding: None,
                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                        height: None,
                                                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                        background: None,
                                                        border: None,
                                                        children: children,
                                                    }
                                                });
                                            if !(self.forge_item_body).is_empty()  {
                                                children
                                                    .push(
                                                        self
                                                            .render_issue_body_card_60(
                                                                palette,
                                                                format!("{}/IssueBodyCard@3181", use_scope),
                                                                (move |event_0| Message::OpenMessageLink(event_0)).clone(),
                                                            ),
                                                    );
                                            }
                                            if !(self.diff_rows).is_empty()  {
                                                children
                                                    .push({
                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                        if self.forge_item_diff_truncated {
                                                            children
                                                                .push(::ducktape_view_guest::wire::Node::Text {
                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                        height: None,
                                                                        align_y: None,
                                                                        line_height: None,
                                                                        shaping: None,
                                                                        wrapping: None,
                                                                        tracking: 0.0f32,
                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                "Geist".into(),
                                                                            ),
                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                        }),
                                                                    },
                                                                    key: format!("{}/@text:444", use_scope),
                                                                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                    color: Some(palette.colors[70]),
                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                        monospace: false,
                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                    },
                                                                    width: None,
                                                                    align_x: None,
                                                                    content: ("Patch truncated — the counts above cover the whole diff."
                                                                        .to_owned())
                                                                        .to_string(),
                                                                });
                                                        }
                                                        children
                                                            .push(
                                                                self
                                                                    .render_diff_pane_64(
                                                                        palette,
                                                                        format!("{}/DiffPane@3194", use_scope),
                                                                        (move |event_0, event_1, event_2| Message::ForgeCommentOpen(
                                                                            event_0,
                                                                            event_1,
                                                                            event_2,
                                                                        ))
                                                                            .clone(),
                                                                    ),
                                                            );
                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                            max_width: None,
                                                            clip: false,
                                                            key: format!("{}/@layout:442", use_scope),
                                                            wrap: None,
                                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                                            spacing: Some((6.0) as f32),
                                                            padding: None,
                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                            height: None,
                                                            align: None,
                                                            background: None,
                                                            border: None,
                                                            children: children,
                                                        }
                                                    });
                                            }
                                            if self.forge_item_kind == "pr"  {
                                                children
                                                    .push({
                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                        children
                                                            .push(
                                                                self
                                                                    .render_group_label_65(
                                                                        palette,
                                                                        format!("{}/GroupLabel@3204", use_scope),
                                                                    ),
                                                            );
                                                        if self.forge_item_state == "merged"  {
                                                            children
                                                                .push(
                                                                    self
                                                                        .render_merged_banner_66(
                                                                            palette,
                                                                            format!("{}/MergedBanner@3206", use_scope),
                                                                        ),
                                                                );
                                                        }
                                                        if self.forge_item_state == "closed"  {
                                                            children
                                                                .push(::ducktape_view_guest::wire::Node::Text {
                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                        height: None,
                                                                        align_y: None,
                                                                        line_height: None,
                                                                        shaping: None,
                                                                        wrapping: None,
                                                                        tracking: 0.0f32,
                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                "Geist".into(),
                                                                            ),
                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                        }),
                                                                    },
                                                                    key: format!("{}/@text:465", use_scope),
                                                                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                    color: Some(palette.colors[70]),
                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                        monospace: false,
                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                    },
                                                                    width: None,
                                                                    align_x: None,
                                                                    content: ("Closed without merging.".to_owned()).to_string(),
                                                                });
                                                        }
                                                        if self.forge_item_state == "open"  {
                                                            children
                                                                .push({
                                                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                    if !(self.merge_conflicts).is_empty()  {
                                                                        children
                                                                            .push({
                                                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                children
                                                                                    .push(::ducktape_view_guest::wire::Node::Text {
                                                                                        options: ::ducktape_view_guest::wire::TextOptions {
                                                                                            height: None,
                                                                                            align_y: None,
                                                                                            line_height: None,
                                                                                            shaping: None,
                                                                                            wrapping: None,
                                                                                            tracking: 0.0f32,
                                                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                    "Geist".into(),
                                                                                                ),
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                            }),
                                                                                        },
                                                                                        key: format!("{}/@text:470", use_scope),
                                                                                        size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                        color: Some(palette.colors[70]),
                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                            monospace: false,
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                        },
                                                                                        width: None,
                                                                                        align_x: None,
                                                                                        content: ("Merge conflicts — resolve on the branch and push again:"
                                                                                            .to_owned())
                                                                                            .to_string(),
                                                                                    });
                                                                                for (index, conflict_path) in self
                                                                                    .merge_conflicts
                                                                                    .iter()
                                                                                    .enumerate()
                                                                                {
                                                                                    let for_scope = format!(
                                                                                        "{}/@for:3217({})", use_scope, index
                                                                                    );
                                                                                    children
                                                                                        .push(::ducktape_view_guest::wire::Node::Text {
                                                                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                                                                height: None,
                                                                                                align_y: None,
                                                                                                line_height: None,
                                                                                                shaping: None,
                                                                                                wrapping: None,
                                                                                                tracking: 0.0f32,
                                                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                        "Geist Mono".into(),
                                                                                                    ),
                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                }),
                                                                                            },
                                                                                            key: format!("{}/@text:475", for_scope),
                                                                                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                            color: Some(palette.colors[4]),
                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                monospace: false,
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                            },
                                                                                            width: None,
                                                                                            align_x: None,
                                                                                            content: (conflict_path.to_owned()).to_string(),
                                                                                        });
                                                                                }
                                                                                ::ducktape_view_guest::wire::Node::Linear {
                                                                                    max_width: None,
                                                                                    clip: false,
                                                                                    key: format!("{}/@layout:469", use_scope),
                                                                                    wrap: None,
                                                                                    axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                                    spacing: Some((3.0) as f32),
                                                                                    padding: None,
                                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                    height: None,
                                                                                    align: None,
                                                                                    background: None,
                                                                                    border: None,
                                                                                    children: children,
                                                                                }
                                                                            });
                                                                    }
                                                                    children
                                                                        .push(
                                                                            self
                                                                                .render_merge_advisory_67(
                                                                                    palette,
                                                                                    format!("{}/MergeAdvisory@3223", use_scope),
                                                                                ),
                                                                        );
                                                                    children
                                                                        .push({
                                                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                            children
                                                                                .push(
                                                                                    self
                                                                                        .render_merge_button_69(
                                                                                            palette,
                                                                                            format!("{}/MergeButton@3229", use_scope),
                                                                                            (move || Message::ForgeMergeSubmit).clone(),
                                                                                        ),
                                                                                );
                                                                            children
                                                                                .push(::ducktape_view_guest::wire::Node::Text {
                                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                                        height: None,
                                                                                        align_y: None,
                                                                                        line_height: None,
                                                                                        shaping: None,
                                                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                        tracking: 0.0f32,
                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                "Geist Mono".into(),
                                                                                            ),
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                        }),
                                                                                    },
                                                                                    key: format!("{}/@text:496", use_scope),
                                                                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                    color: Some(palette.colors[71]),
                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                        monospace: false,
                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                    },
                                                                                    width: None,
                                                                                    align_x: None,
                                                                                    content: (self.forge_item_approvals).to_string(),
                                                                                });
                                                                            children
                                                                                .push(::ducktape_view_guest::wire::Node::Text {
                                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                                        height: None,
                                                                                        align_y: None,
                                                                                        line_height: None,
                                                                                        shaping: None,
                                                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                        tracking: 0.0f32,
                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                "Geist".into(),
                                                                                            ),
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                        }),
                                                                                    },
                                                                                    key: format!("{}/@text:502", use_scope),
                                                                                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                    color: Some(palette.colors[70]),
                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                        monospace: false,
                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                    },
                                                                                    width: None,
                                                                                    align_x: None,
                                                                                    content: ("approvals".to_owned()).to_string(),
                                                                                });
                                                                            if self.forge_item_change_requests <= 0  {
                                                                                children
                                                                                    .push(::ducktape_view_guest::wire::Node::Text {
                                                                                        options: ::ducktape_view_guest::wire::TextOptions {
                                                                                            height: None,
                                                                                            align_y: None,
                                                                                            line_height: None,
                                                                                            shaping: None,
                                                                                            wrapping: None,
                                                                                            tracking: 0.0f32,
                                                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                    "Geist".into(),
                                                                                                ),
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                            }),
                                                                                        },
                                                                                        key: format!("{}/@text:514", use_scope),
                                                                                        size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                        color: Some(palette.colors[70]),
                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                            monospace: false,
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                        },
                                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                        align_x: None,
                                                                                        content: ("Approvals are advisory — merging is never gated."
                                                                                            .to_owned())
                                                                                            .to_string(),
                                                                                    });
                                                                            }
                                                                            ::ducktape_view_guest::wire::Node::Linear {
                                                                                max_width: None,
                                                                                clip: false,
                                                                                key: format!("{}/@layout:481", use_scope),
                                                                                wrap: None,
                                                                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                                spacing: Some((10.0) as f32),
                                                                                padding: None,
                                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                height: None,
                                                                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                background: None,
                                                                                border: None,
                                                                                children: children,
                                                                            }
                                                                        });
                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                        max_width: None,
                                                                        clip: false,
                                                                        key: format!("{}/@layout:467", use_scope),
                                                                        wrap: None,
                                                                        axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                        spacing: Some((9.0) as f32),
                                                                        padding: None,
                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                        height: None,
                                                                        align: None,
                                                                        background: None,
                                                                        border: None,
                                                                        children: children,
                                                                    }
                                                                });
                                                        }
                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                            max_width: None,
                                                            clip: false,
                                                            key: format!("{}/@layout:460", use_scope),
                                                            wrap: None,
                                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                                            spacing: Some((9.0) as f32),
                                                            padding: None,
                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                            height: None,
                                                            align: None,
                                                            background: None,
                                                            border: None,
                                                            children: children,
                                                        }
                                                    });
                                            }
                                            if self.forge_item_kind == "pr"  {
                                                children
                                                    .push({
                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                        children
                                                            .push(
                                                                self
                                                                    .render_group_label_70(
                                                                        palette,
                                                                        format!("{}/GroupLabel@3264", use_scope),
                                                                    ),
                                                            );
                                                        if (self.forge_item_reviews).is_empty() {
                                                            children
                                                                .push(::ducktape_view_guest::wire::Node::Text {
                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                        height: None,
                                                                        align_y: None,
                                                                        line_height: None,
                                                                        shaping: None,
                                                                        wrapping: None,
                                                                        tracking: 0.0f32,
                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                "Geist".into(),
                                                                            ),
                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                        }),
                                                                    },
                                                                    key: format!("{}/@text:523", use_scope),
                                                                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                    color: Some(palette.colors[70]),
                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                        monospace: false,
                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                    },
                                                                    width: None,
                                                                    align_x: None,
                                                                    content: ("No reviews yet.".to_owned()).to_string(),
                                                                });
                                                        }
                                                        for (index, review) in self
                                                            .forge_item_reviews
                                                            .iter()
                                                            .enumerate()
                                                        {
                                                            let for_scope = format!(
                                                                "{}/@for:3267({})", use_scope, index
                                                            );
                                                            children
                                                                .push(
                                                                    self
                                                                        .render_review_card_76(
                                                                            palette,
                                                                            format!("{}/ReviewCard@3268", for_scope),
                                                                            (move |event_0| Message::OpenMessageLink(event_0)).clone(),
                                                                            review.clone(),
                                                                        ),
                                                                );
                                                        }
                                                        children
                                                            .push({
                                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                children
                                                                    .push(::ducktape_view_guest::wire::Node::Button {
                                                                        checked: Some(self.review_verdict == "comment" ),
                                                                        expanded: None,
                                                                        description: None,
                                                                        key: format!("{}/@button:533", use_scope),
                                                                        content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                                            Box::new(::ducktape_view_guest::wire::Node::Text {
                                                                                options: ::ducktape_view_guest::wire::TextOptions {
                                                                                    height: None,
                                                                                    align_y: None,
                                                                                    line_height: None,
                                                                                    shaping: None,
                                                                                    wrapping: None,
                                                                                    tracking: 0.0f32,
                                                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                            "Geist".into(),
                                                                                        ),
                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                    }),
                                                                                },
                                                                                key: format!("{}/@text:539", use_scope),
                                                                                size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                color: None,
                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                    monospace: false,
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                },
                                                                                width: None,
                                                                                align_x: None,
                                                                                content: (crate::host::verdict_pick_label(
                                                                                    ::std::convert::AsRef::as_ref(&(self.review_verdict)),
                                                                                    ::std::convert::AsRef::as_ref(&("comment")),
                                                                                    ::std::convert::AsRef::as_ref(&("Comment")),
                                                                                ))
                                                                                    .to_string(),
                                                                            }),
                                                                        ),
                                                                        label: Some(
                                                                            String::from("Pick comment verdict".to_owned()),
                                                                        ),
                                                                        on_press: Some(
                                                                            ::ducktape_view_guest::slots::message(
                                                                                (move |event_0| Message::ForgeReviewPick(
                                                                                    event_0,
                                                                                ))("comment".to_owned()),
                                                                            ),
                                                                        ),
                                                                        width: None,
                                                                        height: None,
                                                                        padding: Some(
                                                                            ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
                                                                        ),
                                                                        style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                            preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                            recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                                base: ::ducktape_view_guest::wire::Face {
                                                                                    background: Some(
                                                                                        ::ducktape_view_guest::wire::Rgba([
                                                                                            0.0 / 255.0,
                                                                                            0.0 / 255.0,
                                                                                            0.0 / 255.0,
                                                                                            0.000000,
                                                                                        ]),
                                                                                    ),
                                                                                    text: Some(palette.colors[4]),
                                                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                                                        color: None,
                                                                                        width: None,
                                                                                        radius: Some([8.0; 4]),
                                                                                    }),
                                                                                },
                                                                                hover_background: Some(palette.colors[14]),
                                                                                pressed_background: Some(palette.colors[39]),
                                                                                disabled_background: None,
                                                                                disabled_text: None,
                                                                                disabled_opacity: Some(0.5f32),
                                                                                focus_ring: Some(palette.colors[42]),
                                                                                text_size: Some(12.5f32),
                                                                                line_height: None,
                                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                        "Geist".into(),
                                                                                    ),
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                }),
                                                                            }),
                                                                            active: ::ducktape_view_guest::wire::Face {
                                                                                background: Some(palette.colors[3]),
                                                                                text: Some(palette.colors[4]),
                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                    color: Some(palette.colors[63]),
                                                                                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                    radius: Some([
                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                    ]),
                                                                                }),
                                                                            },
                                                                            hovered: Some(::ducktape_view_guest::wire::Face {
                                                                                background: Some(palette.colors[55]),
                                                                                text: Some(palette.colors[4]),
                                                                                border: None,
                                                                            }),
                                                                            pressed: Some(::ducktape_view_guest::wire::Face {
                                                                                background: Some(palette.colors[56]),
                                                                                text: Some(palette.colors[4]),
                                                                                border: None,
                                                                            }),
                                                                            disabled: None,
                                                                        },
                                                                    });
                                                                children
                                                                    .push(::ducktape_view_guest::wire::Node::Button {
                                                                        checked: Some(self.review_verdict == "approve" ),
                                                                        expanded: None,
                                                                        description: None,
                                                                        key: format!("{}/@button:543", use_scope),
                                                                        content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                                            Box::new(::ducktape_view_guest::wire::Node::Text {
                                                                                options: ::ducktape_view_guest::wire::TextOptions {
                                                                                    height: None,
                                                                                    align_y: None,
                                                                                    line_height: None,
                                                                                    shaping: None,
                                                                                    wrapping: None,
                                                                                    tracking: 0.0f32,
                                                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                            "Geist".into(),
                                                                                        ),
                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                    }),
                                                                                },
                                                                                key: format!("{}/@text:549", use_scope),
                                                                                size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                color: None,
                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                    monospace: false,
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                },
                                                                                width: None,
                                                                                align_x: None,
                                                                                content: (crate::host::verdict_pick_label(
                                                                                    ::std::convert::AsRef::as_ref(&(self.review_verdict)),
                                                                                    ::std::convert::AsRef::as_ref(&("approve")),
                                                                                    ::std::convert::AsRef::as_ref(&("Approve")),
                                                                                ))
                                                                                    .to_string(),
                                                                            }),
                                                                        ),
                                                                        label: Some(
                                                                            String::from("Pick approve verdict".to_owned()),
                                                                        ),
                                                                        on_press: Some(
                                                                            ::ducktape_view_guest::slots::message(
                                                                                (move |event_0| Message::ForgeReviewPick(
                                                                                    event_0,
                                                                                ))("approve".to_owned()),
                                                                            ),
                                                                        ),
                                                                        width: None,
                                                                        height: None,
                                                                        padding: Some(
                                                                            ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
                                                                        ),
                                                                        style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                            preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                            recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                                base: ::ducktape_view_guest::wire::Face {
                                                                                    background: Some(
                                                                                        ::ducktape_view_guest::wire::Rgba([
                                                                                            0.0 / 255.0,
                                                                                            0.0 / 255.0,
                                                                                            0.0 / 255.0,
                                                                                            0.000000,
                                                                                        ]),
                                                                                    ),
                                                                                    text: Some(palette.colors[4]),
                                                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                                                        color: None,
                                                                                        width: None,
                                                                                        radius: Some([8.0; 4]),
                                                                                    }),
                                                                                },
                                                                                hover_background: Some(palette.colors[14]),
                                                                                pressed_background: Some(palette.colors[39]),
                                                                                disabled_background: None,
                                                                                disabled_text: None,
                                                                                disabled_opacity: Some(0.5f32),
                                                                                focus_ring: Some(palette.colors[42]),
                                                                                text_size: Some(12.5f32),
                                                                                line_height: None,
                                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                        "Geist".into(),
                                                                                    ),
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                }),
                                                                            }),
                                                                            active: ::ducktape_view_guest::wire::Face {
                                                                                background: Some(palette.colors[106]),
                                                                                text: Some(palette.colors[4]),
                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                    color: Some(palette.colors[107]),
                                                                                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                    radius: Some([
                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                    ]),
                                                                                }),
                                                                            },
                                                                            hovered: Some(::ducktape_view_guest::wire::Face {
                                                                                background: Some(palette.colors[27]),
                                                                                text: Some(palette.colors[4]),
                                                                                border: None,
                                                                            }),
                                                                            pressed: Some(::ducktape_view_guest::wire::Face {
                                                                                background: Some(palette.colors[27]),
                                                                                text: Some(palette.colors[4]),
                                                                                border: None,
                                                                            }),
                                                                            disabled: None,
                                                                        },
                                                                    });
                                                                children
                                                                    .push(::ducktape_view_guest::wire::Node::Button {
                                                                        checked: Some(self.review_verdict == "request_changes" ),
                                                                        expanded: None,
                                                                        description: None,
                                                                        key: format!("{}/@button:553", use_scope),
                                                                        content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                                            Box::new(::ducktape_view_guest::wire::Node::Text {
                                                                                options: ::ducktape_view_guest::wire::TextOptions {
                                                                                    height: None,
                                                                                    align_y: None,
                                                                                    line_height: None,
                                                                                    shaping: None,
                                                                                    wrapping: None,
                                                                                    tracking: 0.0f32,
                                                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                            "Geist".into(),
                                                                                        ),
                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                    }),
                                                                                },
                                                                                key: format!("{}/@text:559", use_scope),
                                                                                size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                color: None,
                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                    monospace: false,
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                },
                                                                                width: None,
                                                                                align_x: None,
                                                                                content: (crate::host::verdict_pick_label(
                                                                                    ::std::convert::AsRef::as_ref(&(self.review_verdict)),
                                                                                    ::std::convert::AsRef::as_ref(&("request_changes")),
                                                                                    ::std::convert::AsRef::as_ref(&("Request changes")),
                                                                                ))
                                                                                    .to_string(),
                                                                            }),
                                                                        ),
                                                                        label: Some(
                                                                            String::from("Pick request-changes verdict".to_owned()),
                                                                        ),
                                                                        on_press: Some(
                                                                            ::ducktape_view_guest::slots::message(
                                                                                (move |event_0| Message::ForgeReviewPick(
                                                                                    event_0,
                                                                                ))("request_changes".to_owned()),
                                                                            ),
                                                                        ),
                                                                        width: None,
                                                                        height: None,
                                                                        padding: Some(
                                                                            ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
                                                                        ),
                                                                        style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                            preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                            recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                                base: ::ducktape_view_guest::wire::Face {
                                                                                    background: Some(
                                                                                        ::ducktape_view_guest::wire::Rgba([
                                                                                            0.0 / 255.0,
                                                                                            0.0 / 255.0,
                                                                                            0.0 / 255.0,
                                                                                            0.000000,
                                                                                        ]),
                                                                                    ),
                                                                                    text: Some(palette.colors[4]),
                                                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                                                        color: None,
                                                                                        width: None,
                                                                                        radius: Some([8.0; 4]),
                                                                                    }),
                                                                                },
                                                                                hover_background: Some(palette.colors[14]),
                                                                                pressed_background: Some(palette.colors[39]),
                                                                                disabled_background: None,
                                                                                disabled_text: None,
                                                                                disabled_opacity: Some(0.5f32),
                                                                                focus_ring: Some(palette.colors[42]),
                                                                                text_size: Some(12.5f32),
                                                                                line_height: None,
                                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                        "Geist".into(),
                                                                                    ),
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                }),
                                                                            }),
                                                                            active: ::ducktape_view_guest::wire::Face {
                                                                                background: Some(palette.colors[81]),
                                                                                text: Some(palette.colors[4]),
                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                    color: Some(palette.colors[82]),
                                                                                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                    radius: Some([
                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                    ]),
                                                                                }),
                                                                            },
                                                                            hovered: Some(::ducktape_view_guest::wire::Face {
                                                                                background: Some(palette.colors[65]),
                                                                                text: Some(palette.colors[4]),
                                                                                border: None,
                                                                            }),
                                                                            pressed: Some(::ducktape_view_guest::wire::Face {
                                                                                background: Some(palette.colors[65]),
                                                                                text: Some(palette.colors[4]),
                                                                                border: None,
                                                                            }),
                                                                            disabled: None,
                                                                        },
                                                                    });
                                                                children
                                                                    .push(::ducktape_view_guest::wire::Node::Space {
                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                        height: None,
                                                                    });
                                                                ::ducktape_view_guest::wire::Node::Linear {
                                                                    max_width: None,
                                                                    clip: false,
                                                                    key: format!("{}/@layout:528", use_scope),
                                                                    wrap: None,
                                                                    axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                    spacing: Some((6.0) as f32),
                                                                    padding: None,
                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                    height: None,
                                                                    align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                    background: None,
                                                                    border: None,
                                                                    children: children,
                                                                }
                                                            });
                                                        if !(crate::host::forge_comment_target(
                                                            ::std::convert::AsRef::as_ref(&(self.comment_path)),
                                                            ::std::convert::AsRef::as_ref(&(self.comment_line)),
                                                            ::std::convert::AsRef::as_ref(&(self.comment_side)),
                                                        ))
                                                            .is_empty() 
                                                        {
                                                            children
                                                                .push({
                                                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                    children
                                                                        .push({
                                                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                            children
                                                                                .push(::ducktape_view_guest::wire::Node::Text {
                                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                                        height: None,
                                                                                        align_y: None,
                                                                                        line_height: None,
                                                                                        shaping: None,
                                                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                        tracking: 0.0f32,
                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                "Geist Mono".into(),
                                                                                            ),
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                        }),
                                                                                    },
                                                                                    key: format!("{}/@text:579", use_scope),
                                                                                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                    color: Some(palette.colors[16]),
                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                        monospace: false,
                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                    },
                                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                    align_x: None,
                                                                                    content: (crate::host::forge_comment_target(
                                                                                            ::std::convert::AsRef::as_ref(&(self.comment_path)),
                                                                                            ::std::convert::AsRef::as_ref(&(self.comment_line)),
                                                                                            ::std::convert::AsRef::as_ref(&(self.comment_side)),
                                                                                        )
                                                                                        .to_owned())
                                                                                        .to_string(),
                                                                                });
                                                                            children
                                                                                .push(::ducktape_view_guest::wire::Node::Button {
                                                                                    checked: None,
                                                                                    expanded: None,
                                                                                    description: None,
                                                                                    key: format!("{}/@button:586", use_scope),
                                                                                    content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                                                        String::from("Cancel"),
                                                                                    ),
                                                                                    label: None,
                                                                                    on_press: Some(
                                                                                        ::ducktape_view_guest::slots::message(
                                                                                            Message::ForgeCommentCancel,
                                                                                        ),
                                                                                    ),
                                                                                    width: None,
                                                                                    height: None,
                                                                                    padding: Some(
                                                                                        ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
                                                                                    ),
                                                                                    style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                                        preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                                        recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                                            base: ::ducktape_view_guest::wire::Face {
                                                                                                background: Some(palette.colors[12]),
                                                                                                text: Some(palette.colors[13]),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some(palette.colors[40]),
                                                                                                    width: Some(1.0),
                                                                                                    radius: Some([5.0; 4]),
                                                                                                }),
                                                                                            },
                                                                                            hover_background: Some(palette.colors[14]),
                                                                                            pressed_background: Some(palette.colors[6]),
                                                                                            disabled_background: None,
                                                                                            disabled_text: None,
                                                                                            disabled_opacity: Some(0.5f32),
                                                                                            focus_ring: Some(palette.colors[42]),
                                                                                            text_size: Some(11.0f32),
                                                                                            line_height: Some(1.35f32),
                                                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                    "Geist".into(),
                                                                                                ),
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                            }),
                                                                                        }),
                                                                                        active: ::ducktape_view_guest::wire::Face::default(),
                                                                                        hovered: None,
                                                                                        pressed: None,
                                                                                        disabled: None,
                                                                                    },
                                                                                });
                                                                            ::ducktape_view_guest::wire::Node::Linear {
                                                                                max_width: None,
                                                                                clip: false,
                                                                                key: format!("{}/@layout:574", use_scope),
                                                                                wrap: None,
                                                                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                                spacing: Some((7.0) as f32),
                                                                                padding: None,
                                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                height: None,
                                                                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                background: None,
                                                                                border: None,
                                                                                children: children,
                                                                            }
                                                                        });
                                                                    children
                                                                        .push({
                                                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                            children
                                                                                .push({
                                                                                    let node_scope = format!(
                                                                                        "{}/forge-comment-body", node_scope
                                                                                    );
                                                                                    ::ducktape_view_guest::wire::Node::Input {
                                                                                        options: ::ducktape_view_guest::wire::InputOptions {
                                                                                            label: ("Line comment".to_owned()).to_string(),
                                                                                            description: None,
                                                                                            disabled: (self.review_busy || (!self.connected)),
                                                                                            padding: Some(
                                                                                                ::ducktape_view_guest::wire::Edges::all((6.2) as f32),
                                                                                            ),
                                                                                            text_size: Some((13.0) as f32),
                                                                                            line_height: Some((1.2) as f32),
                                                                                            align: None,
                                                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                    "Geist".into(),
                                                                                                ),
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                            }),
                                                                                        },
                                                                                        key: node_scope.clone(),
                                                                                        placeholder: String::from(
                                                                                            "Comment on this line…".to_owned(),
                                                                                        ),
                                                                                        value: (self.comment_draft).to_string(),
                                                                                        on_input: ::ducktape_view_guest::slots::handler::<
                                                                                            String,
                                                                                            Message,
                                                                                        >(
                                                                                            Box::new({
                                                                                                let route = Message::CommentDraftChanged
                                                                                                    as fn(String) -> Message;
                                                                                                move |sent: String| Some(route(sent))
                                                                                            }),
                                                                                        ),
                                                                                        on_submit: Some(
                                                                                            ::ducktape_view_guest::slots::message(
                                                                                                (move |event_0| Message::ForgeCommentStage(
                                                                                                    event_0,
                                                                                                ))(self.comment_draft.to_owned()),
                                                                                            ),
                                                                                        ),
                                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                        secure: (false),
                                                                                        style: Box::new(::ducktape_view_guest::wire::InputStyle {
                                                                                            utility: ::ducktape_view_guest::wire::InputFace {
                                                                                                background: Some(palette.colors[3]),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some(palette.colors[39]),
                                                                                                    width: Some(1f32),
                                                                                                    radius: Some([10f32; 4]),
                                                                                                }),
                                                                                                ..Default::default()
                                                                                            },
                                                                                            focus_border: Some(palette.colors[42]),
                                                                                            focused_hovered: None,
                                                                                            active: ::ducktape_view_guest::wire::InputFace {
                                                                                                icon: None,
                                                                                                background: Some(palette.colors[3]),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: None,
                                                                                                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                                    radius: Some([
                                                                                                        ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ]),
                                                                                                }),
                                                                                                value: Some(palette.colors[4]),
                                                                                                placeholder: Some(palette.colors[72]),
                                                                                                selection: Some({
                                                                                                    let mut color = palette.colors[4];
                                                                                                    color.0[3] = 0.180000;
                                                                                                    color
                                                                                                }),
                                                                                            },
                                                                                            hovered: Some(::ducktape_view_guest::wire::InputFace {
                                                                                                icon: None,
                                                                                                background: Some(palette.colors[6]),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some(palette.colors[40]),
                                                                                                    width: None,
                                                                                                    radius: None,
                                                                                                }),
                                                                                                value: None,
                                                                                                placeholder: None,
                                                                                                selection: None,
                                                                                            }),
                                                                                            focused: None,
                                                                                            disabled: Some(::ducktape_view_guest::wire::InputFace {
                                                                                                icon: None,
                                                                                                background: Some({
                                                                                                    let mut color = palette.colors[6];
                                                                                                    color.0[3] = 0.540000;
                                                                                                    color
                                                                                                }),
                                                                                                border: None,
                                                                                                value: Some(palette.colors[5]),
                                                                                                placeholder: None,
                                                                                                selection: None,
                                                                                            }),
                                                                                        }),
                                                                                    }
                                                                                });
                                                                            children
                                                                                .push(::ducktape_view_guest::wire::Node::Button {
                                                                                    checked: None,
                                                                                    expanded: None,
                                                                                    description: None,
                                                                                    key: format!("{}/@button:609", use_scope),
                                                                                    content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                                                        String::from("Add comment"),
                                                                                    ),
                                                                                    label: None,
                                                                                    on_press: if (((self.review_busy || (!self.connected))
                                                                                        || (self.comment_draft).is_empty())
                                                                                        || crate::host::forge_comment_cap_reached(
                                                                                            ::std::convert::AsRef::as_ref(&(self.staged_comments)),
                                                                                        )) 
                                                                                    {
                                                                                        None
                                                                                    } else {
                                                                                        Some(
                                                                                            ::ducktape_view_guest::slots::message(
                                                                                                (move |event_0| Message::ForgeCommentStage(
                                                                                                    event_0,
                                                                                                ))(self.comment_draft.to_owned()),
                                                                                            ),
                                                                                        )
                                                                                    },
                                                                                    width: None,
                                                                                    height: None,
                                                                                    padding: Some(
                                                                                        ::ducktape_view_guest::wire::Edges::all((6.0) as f32),
                                                                                    ),
                                                                                    style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                                        preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                                        recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                                            base: ::ducktape_view_guest::wire::Face {
                                                                                                background: Some(palette.colors[12]),
                                                                                                text: Some(palette.colors[13]),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some(palette.colors[40]),
                                                                                                    width: Some(1.0),
                                                                                                    radius: Some([9.0; 4]),
                                                                                                }),
                                                                                            },
                                                                                            hover_background: Some(palette.colors[14]),
                                                                                            pressed_background: Some(palette.colors[6]),
                                                                                            disabled_background: None,
                                                                                            disabled_text: None,
                                                                                            disabled_opacity: Some(0.5f32),
                                                                                            focus_ring: Some(palette.colors[42]),
                                                                                            text_size: Some(12.5f32),
                                                                                            line_height: None,
                                                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                    "Geist".into(),
                                                                                                ),
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                            }),
                                                                                        }),
                                                                                        active: ::ducktape_view_guest::wire::Face::default(),
                                                                                        hovered: None,
                                                                                        pressed: None,
                                                                                        disabled: None,
                                                                                    },
                                                                                });
                                                                            ::ducktape_view_guest::wire::Node::Linear {
                                                                                max_width: None,
                                                                                clip: false,
                                                                                key: format!("{}/@layout:590", use_scope),
                                                                                wrap: None,
                                                                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                                spacing: Some((6.0) as f32),
                                                                                padding: None,
                                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                height: None,
                                                                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                background: None,
                                                                                border: None,
                                                                                children: children,
                                                                            }
                                                                        });
                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                        max_width: None,
                                                                        clip: false,
                                                                        key: format!("{}/@layout:573", use_scope),
                                                                        wrap: None,
                                                                        axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                        spacing: Some((6.0) as f32),
                                                                        padding: None,
                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                        height: None,
                                                                        align: None,
                                                                        background: None,
                                                                        border: None,
                                                                        children: children,
                                                                    }
                                                                });
                                                        }
                                                        if crate::host::forge_comment_cap_reached(
                                                            ::std::convert::AsRef::as_ref(&(self.staged_comments)),
                                                        ) {
                                                            children
                                                                .push(::ducktape_view_guest::wire::Node::Text {
                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                        height: None,
                                                                        align_y: None,
                                                                        line_height: None,
                                                                        shaping: None,
                                                                        wrapping: None,
                                                                        tracking: 0.0f32,
                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                "Geist".into(),
                                                                            ),
                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                        }),
                                                                    },
                                                                    key: format!("{}/@text:615", use_scope),
                                                                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                    color: Some(palette.colors[70]),
                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                        monospace: false,
                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                    },
                                                                    width: None,
                                                                    align_x: None,
                                                                    content: ("Comment limit reached for one review — submit this review, then start another."
                                                                        .to_owned())
                                                                        .to_string(),
                                                                });
                                                        }
                                                        if !(self.staged_comments).is_empty()  {
                                                            children
                                                                .push({
                                                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                    for (index, staged) in self
                                                                        .staged_comments
                                                                        .iter()
                                                                        .enumerate()
                                                                    {
                                                                        let for_scope = format!(
                                                                            "{}/@for:3367({})", use_scope, index
                                                                        );
                                                                        children
                                                                            .push({
                                                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                children
                                                                                    .push(::ducktape_view_guest::wire::Node::Container {
                                                                                        shadow: ::ducktape_view_guest::wire::Shadow {
                                                                                            color: None,
                                                                                            x: None,
                                                                                            y: None,
                                                                                            blur: None,
                                                                                        },
                                                                                        max_width: None,
                                                                                        max_height: None,
                                                                                        clip: false,
                                                                                        key: format!("{}/@container:630", for_scope),
                                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                        height: None,
                                                                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                                                                            top: (9.0) as f32,
                                                                                            right: (11.0) as f32,
                                                                                            bottom: (9.0) as f32,
                                                                                            left: (11.0) as f32,
                                                                                        }),
                                                                                        align_x: None,
                                                                                        align_y: None,
                                                                                        background: (Some(palette.colors[90]))
                                                                                            .map(::ducktape_view_guest::wire::Background::Color),
                                                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                                                            color: Some(palette.colors[19]),
                                                                                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                            radius: Some([
                                                                                                ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                            ]),
                                                                                        }),
                                                                                        snap: None,
                                                                                        content: Box::new({
                                                                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                            children
                                                                                                .push({
                                                                                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                                    children
                                                                                                        .push(::ducktape_view_guest::wire::Node::Text {
                                                                                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                                                                                height: None,
                                                                                                                align_y: None,
                                                                                                                line_height: None,
                                                                                                                shaping: None,
                                                                                                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                                                tracking: 0.0f32,
                                                                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                        "Geist Mono".into(),
                                                                                                                    ),
                                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                }),
                                                                                                            },
                                                                                                            key: format!("{}/@text:647", for_scope),
                                                                                                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[16]),
                                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                                monospace: false,
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                            },
                                                                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                                            align_x: None,
                                                                                                            content: (staged.anchor.to_owned()).to_string(),
                                                                                                        });
                                                                                                    children
                                                                                                        .push(::ducktape_view_guest::wire::Node::Text {
                                                                                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                                                                                height: None,
                                                                                                                align_y: None,
                                                                                                                line_height: None,
                                                                                                                shaping: None,
                                                                                                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                                                tracking: 0.0f32,
                                                                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                        "Geist Mono".into(),
                                                                                                                    ),
                                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                }),
                                                                                                            },
                                                                                                            key: format!("{}/@text:654", for_scope),
                                                                                                            size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[73]),
                                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                                monospace: false,
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                            },
                                                                                                            width: None,
                                                                                                            align_x: None,
                                                                                                            content: ("not sent yet".to_owned()).to_string(),
                                                                                                        });
                                                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                                                        max_width: None,
                                                                                                        clip: false,
                                                                                                        key: format!("{}/@layout:642", for_scope),
                                                                                                        wrap: None,
                                                                                                        axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                                                        spacing: Some((7.0) as f32),
                                                                                                        padding: None,
                                                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                                        height: None,
                                                                                                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                                        background: None,
                                                                                                        border: None,
                                                                                                        children: children,
                                                                                                    }
                                                                                                });
                                                                                            children
                                                                                                .push(::ducktape_view_guest::wire::Node::Text {
                                                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                                                        height: None,
                                                                                                        align_y: None,
                                                                                                        line_height: Some(
                                                                                                            ::ducktape_view_guest::wire::LineHeight::Relative(
                                                                                                                ((1.55) as f32).max(f32::EPSILON).min(f32::MAX),
                                                                                                            ),
                                                                                                        ),
                                                                                                        shaping: None,
                                                                                                        wrapping: None,
                                                                                                        tracking: 0.0f32,
                                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                "Geist".into(),
                                                                                                            ),
                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                        }),
                                                                                                    },
                                                                                                    key: format!("{}/@text:660", for_scope),
                                                                                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                    color: Some(palette.colors[15]),
                                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                                        monospace: false,
                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                    },
                                                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                                    align_x: None,
                                                                                                    content: (staged.body.to_owned()).to_string(),
                                                                                                });
                                                                                            ::ducktape_view_guest::wire::Node::Linear {
                                                                                                max_width: None,
                                                                                                clip: false,
                                                                                                key: format!("{}/@layout:641", for_scope),
                                                                                                wrap: None,
                                                                                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                                                spacing: Some((4.0) as f32),
                                                                                                padding: None,
                                                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                                height: None,
                                                                                                align: None,
                                                                                                background: None,
                                                                                                border: None,
                                                                                                children: children,
                                                                                            }
                                                                                        }),
                                                                                    });
                                                                                children
                                                                                    .push(::ducktape_view_guest::wire::Node::Button {
                                                                                        checked: None,
                                                                                        expanded: None,
                                                                                        description: None,
                                                                                        key: format!("{}/@button:666", for_scope),
                                                                                        content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                                                            String::from("Remove"),
                                                                                        ),
                                                                                        label: Some(
                                                                                            String::from("Remove staged comment".to_owned()),
                                                                                        ),
                                                                                        on_press: Some(
                                                                                            ::ducktape_view_guest::slots::message(
                                                                                                (move |event_0| Message::ForgeCommentDrop(
                                                                                                    event_0,
                                                                                                ))(staged.anchor.to_owned()),
                                                                                            ),
                                                                                        ),
                                                                                        width: None,
                                                                                        height: None,
                                                                                        padding: Some(
                                                                                            ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
                                                                                        ),
                                                                                        style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                                            preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                                            recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                                                base: ::ducktape_view_guest::wire::Face {
                                                                                                    background: Some(palette.colors[12]),
                                                                                                    text: Some(palette.colors[13]),
                                                                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                                                                        color: Some(palette.colors[40]),
                                                                                                        width: Some(1.0),
                                                                                                        radius: Some([5.0; 4]),
                                                                                                    }),
                                                                                                },
                                                                                                hover_background: Some(palette.colors[14]),
                                                                                                pressed_background: Some(palette.colors[6]),
                                                                                                disabled_background: None,
                                                                                                disabled_text: None,
                                                                                                disabled_opacity: Some(0.5f32),
                                                                                                focus_ring: Some(palette.colors[42]),
                                                                                                text_size: Some(11.0f32),
                                                                                                line_height: Some(1.35f32),
                                                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                        "Geist".into(),
                                                                                                    ),
                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                }),
                                                                                            }),
                                                                                            active: ::ducktape_view_guest::wire::Face::default(),
                                                                                            hovered: None,
                                                                                            pressed: None,
                                                                                            disabled: None,
                                                                                        },
                                                                                    });
                                                                                ::ducktape_view_guest::wire::Node::Linear {
                                                                                    max_width: None,
                                                                                    clip: false,
                                                                                    key: format!("{}/@layout:625", for_scope),
                                                                                    wrap: None,
                                                                                    axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                                    spacing: Some((7.0) as f32),
                                                                                    padding: None,
                                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                    height: None,
                                                                                    align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                    background: None,
                                                                                    border: None,
                                                                                    children: children,
                                                                                }
                                                                            });
                                                                    }
                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                        max_width: None,
                                                                        clip: false,
                                                                        key: format!("{}/@layout:623", use_scope),
                                                                        wrap: None,
                                                                        axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                        spacing: Some((4.0) as f32),
                                                                        padding: None,
                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                        height: None,
                                                                        align: None,
                                                                        background: None,
                                                                        border: None,
                                                                        children: children,
                                                                    }
                                                                });
                                                        }
                                                        children
                                                            .push({
                                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                children
                                                                    .push({
                                                                        let node_scope = format!(
                                                                            "{}/forge-review-body", node_scope
                                                                        );
                                                                        ::ducktape_view_guest::wire::Node::Input {
                                                                            options: ::ducktape_view_guest::wire::InputOptions {
                                                                                label: ("Review body".to_owned()).to_string(),
                                                                                description: None,
                                                                                disabled: (self.review_busy || (!self.connected)),
                                                                                padding: Some(
                                                                                    ::ducktape_view_guest::wire::Edges::all((6.2) as f32),
                                                                                ),
                                                                                text_size: Some((13.0) as f32),
                                                                                line_height: Some((1.2) as f32),
                                                                                align: None,
                                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                        "Geist".into(),
                                                                                    ),
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                }),
                                                                            },
                                                                            key: node_scope.clone(),
                                                                            placeholder: String::from("Leave a review…".to_owned()),
                                                                            value: (self.review_draft).to_string(),
                                                                            on_input: ::ducktape_view_guest::slots::handler::<
                                                                                String,
                                                                                Message,
                                                                            >(
                                                                                Box::new({
                                                                                    let route = Message::ReviewDraftChanged
                                                                                        as fn(String) -> Message;
                                                                                    move |sent: String| Some(route(sent))
                                                                                }),
                                                                            ),
                                                                            on_submit: Some(
                                                                                ::ducktape_view_guest::slots::message(
                                                                                    (move |event_0| Message::ForgeReviewSubmit(
                                                                                        event_0,
                                                                                    ))(self.review_draft.to_owned()),
                                                                                ),
                                                                            ),
                                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                            secure: (false),
                                                                            style: Box::new(::ducktape_view_guest::wire::InputStyle {
                                                                                utility: ::ducktape_view_guest::wire::InputFace {
                                                                                    background: Some(palette.colors[3]),
                                                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                                                        color: Some(palette.colors[39]),
                                                                                        width: Some(1f32),
                                                                                        radius: Some([10f32; 4]),
                                                                                    }),
                                                                                    ..Default::default()
                                                                                },
                                                                                focus_border: Some(palette.colors[42]),
                                                                                focused_hovered: None,
                                                                                active: ::ducktape_view_guest::wire::InputFace {
                                                                                    icon: None,
                                                                                    background: Some(palette.colors[3]),
                                                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                                                        color: None,
                                                                                        width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                        radius: Some([
                                                                                            ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                                            ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                                            ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                                            ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                                        ]),
                                                                                    }),
                                                                                    value: Some(palette.colors[4]),
                                                                                    placeholder: Some(palette.colors[72]),
                                                                                    selection: Some({
                                                                                        let mut color = palette.colors[4];
                                                                                        color.0[3] = 0.180000;
                                                                                        color
                                                                                    }),
                                                                                },
                                                                                hovered: Some(::ducktape_view_guest::wire::InputFace {
                                                                                    icon: None,
                                                                                    background: Some(palette.colors[6]),
                                                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                                                        color: Some(palette.colors[40]),
                                                                                        width: None,
                                                                                        radius: None,
                                                                                    }),
                                                                                    value: None,
                                                                                    placeholder: None,
                                                                                    selection: None,
                                                                                }),
                                                                                focused: None,
                                                                                disabled: Some(::ducktape_view_guest::wire::InputFace {
                                                                                    icon: None,
                                                                                    background: Some({
                                                                                        let mut color = palette.colors[6];
                                                                                        color.0[3] = 0.540000;
                                                                                        color
                                                                                    }),
                                                                                    border: None,
                                                                                    value: Some(palette.colors[5]),
                                                                                    placeholder: None,
                                                                                    selection: None,
                                                                                }),
                                                                            }),
                                                                        }
                                                                    });
                                                                children
                                                                    .push(::ducktape_view_guest::wire::Node::Button {
                                                                        checked: None,
                                                                        expanded: None,
                                                                        description: None,
                                                                        key: format!("{}/@button:693", use_scope),
                                                                        content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                                            String::from("Submit review"),
                                                                        ),
                                                                        label: None,
                                                                        on_press: if (((self.review_busy || (!self.connected))
                                                                            || (self.forge_item_source_oid).is_empty())
                                                                            || ((self.review_draft).is_empty()
                                                                                && (!(!(self.staged_comments).is_empty())))) 
                                                                        {
                                                                            None
                                                                        } else {
                                                                            Some(
                                                                                ::ducktape_view_guest::slots::message(
                                                                                    (move |event_0| Message::ForgeReviewSubmit(
                                                                                        event_0,
                                                                                    ))(self.review_draft.to_owned()),
                                                                                ),
                                                                            )
                                                                        },
                                                                        width: None,
                                                                        height: None,
                                                                        padding: Some(
                                                                            ::ducktape_view_guest::wire::Edges::all((6.0) as f32),
                                                                        ),
                                                                        style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                            preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                            recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                                base: ::ducktape_view_guest::wire::Face {
                                                                                    background: Some(palette.colors[7]),
                                                                                    text: Some(palette.colors[9]),
                                                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                                                        color: None,
                                                                                        width: None,
                                                                                        radius: Some([9.0; 4]),
                                                                                    }),
                                                                                },
                                                                                hover_background: Some(palette.colors[8]),
                                                                                pressed_background: Some({
                                                                                    let mut color = palette.colors[7];
                                                                                    color.0[3] = 0.800000;
                                                                                    color
                                                                                }),
                                                                                disabled_background: Some(palette.colors[10]),
                                                                                disabled_text: Some(palette.colors[11]),
                                                                                disabled_opacity: None,
                                                                                focus_ring: Some(palette.colors[42]),
                                                                                text_size: Some(12.5f32),
                                                                                line_height: None,
                                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                        "Geist".into(),
                                                                                    ),
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                }),
                                                                            }),
                                                                            active: ::ducktape_view_guest::wire::Face::default(),
                                                                            hovered: None,
                                                                            pressed: None,
                                                                            disabled: None,
                                                                        },
                                                                    });
                                                                ::ducktape_view_guest::wire::Node::Linear {
                                                                    max_width: None,
                                                                    clip: false,
                                                                    key: format!("{}/@layout:671", use_scope),
                                                                    wrap: None,
                                                                    axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                    spacing: Some((6.0) as f32),
                                                                    padding: None,
                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                    height: None,
                                                                    align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                    background: None,
                                                                    border: None,
                                                                    children: children,
                                                                }
                                                            });
                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                            max_width: None,
                                                            clip: false,
                                                            key: format!("{}/@layout:520", use_scope),
                                                            wrap: None,
                                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                                            spacing: Some((9.0) as f32),
                                                            padding: None,
                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                            height: None,
                                                            align: None,
                                                            background: None,
                                                            border: None,
                                                            children: children,
                                                        }
                                                    });
                                            }
                                            children
                                                .push({
                                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                    children
                                                        .push(
                                                            self
                                                                .render_group_label_77(
                                                                    palette,
                                                                    format!("{}/GroupLabel@3442", use_scope),
                                                                ),
                                                        );
                                                    for (index, note) in self.linked_note.iter().enumerate() {
                                                        let for_scope = format!(
                                                            "{}/@for:3448({})", use_scope, index
                                                        );
                                                        children
                                                            .push(
                                                                self
                                                                    .render_linked_note_95(
                                                                        palette,
                                                                        format!("{}/LinkedNote@3449", for_scope),
                                                                        (move |event_0| Message::OpenMessageLink(event_0)).clone(),
                                                                        note.clone(),
                                                                    ),
                                                            );
                                                    }
                                                    if (self.discussion).is_empty()
                                                        && (!self.discussion_clipped) 
                                                    {
                                                        children
                                                            .push(::ducktape_view_guest::wire::Node::Text {
                                                                options: ::ducktape_view_guest::wire::TextOptions {
                                                                    height: None,
                                                                    align_y: None,
                                                                    line_height: None,
                                                                    shaping: None,
                                                                    wrapping: None,
                                                                    tracking: 0.0f32,
                                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                            "Geist".into(),
                                                                        ),
                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                    }),
                                                                },
                                                                key: format!("{}/@text:710", use_scope),
                                                                size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                color: Some(palette.colors[70]),
                                                                font: ::ducktape_view_guest::wire::Font {
                                                                    monospace: false,
                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                },
                                                                width: None,
                                                                align_x: None,
                                                                content: ("No discussion yet.".to_owned()).to_string(),
                                                            });
                                                    }
                                                    if self.discussion_clipped {
                                                        children
                                                            .push(::ducktape_view_guest::wire::Node::Text {
                                                                options: ::ducktape_view_guest::wire::TextOptions {
                                                                    height: None,
                                                                    align_y: None,
                                                                    line_height: None,
                                                                    shaping: None,
                                                                    wrapping: None,
                                                                    tracking: 0.0f32,
                                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                            "Geist".into(),
                                                                        ),
                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                    }),
                                                                },
                                                                key: format!("{}/@text:712", use_scope),
                                                                size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                color: Some(palette.colors[70]),
                                                                font: ::ducktape_view_guest::wire::Font {
                                                                    monospace: false,
                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                },
                                                                width: None,
                                                                align_x: None,
                                                                content: ("Older comments are not shown.".to_owned())
                                                                    .to_string(),
                                                            });
                                                    }
                                                    children
                                                        .push({
                                                            let mut children: Vec<_> = Vec::new();
                                                            for message in self.discussion.iter() {
                                                                let key = message.seq;
                                                                let key_recon = format!("{}/key({})", use_scope, key);
                                                                let child: ::ducktape_view_guest::wire::Node = {
                                                                    let _lazy_context_660 = (use_scope).to_owned();
                                                                    let _lazy_event_660_0 = (move |event_0| Message::ForgeOpenRepo(
                                                                        event_0,
                                                                    ))
                                                                        .clone();
                                                                    let _lazy_event_660_1 = (move || Message::ForgeCloseRepo)
                                                                        .clone();
                                                                    let _lazy_event_660_2 = (move |event_0| Message::ForgePickBranch(
                                                                        event_0,
                                                                    ))
                                                                        .clone();
                                                                    let _lazy_event_660_3 = (move |event_0| Message::SelectForgeTab(
                                                                        event_0,
                                                                    ))
                                                                        .clone();
                                                                    let _lazy_event_660_4 = (move |event_0| Message::ForgeOpenItem(
                                                                        event_0,
                                                                    ))
                                                                        .clone();
                                                                    let _lazy_event_660_5 = (move || Message::ForgeCloseItem)
                                                                        .clone();
                                                                    let _lazy_event_660_6 = (move || Message::ForgeMergeSubmit)
                                                                        .clone();
                                                                    let _lazy_event_660_7 = (move |event_0| Message::ForgeReviewPick(
                                                                        event_0,
                                                                    ))
                                                                        .clone();
                                                                    let _lazy_event_660_8 = (move |event_0| Message::ForgeReviewSubmit(
                                                                        event_0,
                                                                    ))
                                                                        .clone();
                                                                    let _lazy_event_660_9 = (move |event_0, event_1, event_2| Message::ForgeCommentOpen(
                                                                        event_0,
                                                                        event_1,
                                                                        event_2,
                                                                    ))
                                                                        .clone();
                                                                    let _lazy_event_660_10 = (move |event_0| Message::ForgeCommentStage(
                                                                        event_0,
                                                                    ))
                                                                        .clone();
                                                                    let _lazy_event_660_11 = (move || {
                                                                        Message::ForgeCommentCancel
                                                                    })
                                                                        .clone();
                                                                    let _lazy_event_660_12 = (move |event_0| Message::ForgeCommentDrop(
                                                                        event_0,
                                                                    ))
                                                                        .clone();
                                                                    let _lazy_event_660_13 = (move |event_0| Message::ForgeOpenDir(
                                                                        event_0,
                                                                    ))
                                                                        .clone();
                                                                    let _lazy_event_660_14 = (move |event_0| Message::ForgeOpenFile(
                                                                        event_0,
                                                                    ))
                                                                        .clone();
                                                                    let lazy_event_660_15 = (move |event_0| Message::OpenMessageLink(
                                                                        event_0,
                                                                    ))
                                                                        .clone();
                                                                    let _lazy_event_660_16 = (move |event_0, event_1| Message::CopyToClipboard(
                                                                        event_0,
                                                                        event_1,
                                                                    ))
                                                                        .clone();
                                                                    let _lazy_event_660_17 = (move |event_0, event_1| Message::TreeResized(
                                                                        event_0,
                                                                        event_1,
                                                                    ))
                                                                        .clone();
                                                                    {
                                                                        let lazy_key = format!("{}/@lazy:724", key_recon);
                                                                        ::ducktape_view_guest::memo_lazy(
                                                                            (
                                                                                message.seq,
                                                                                message.render_rev,
                                                                                (format!("{}/key({})", node_scope, key)).to_owned(),
                                                                                palette.name,
                                                                            ),
                                                                            move |dependency| {
                                                                                let lazy_scope = dependency.2.clone();
                                                                                let cached_note: crate::host::ChatMessage = message.clone();
                                                                                {
                                                                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                    children
                                                                                        .push({
                                                                                            let message_avatar_scope_3473 = format!(
                                                                                                "{}/MessageAvatar@3473", lazy_scope
                                                                                            );
                                                                                            {
                                                                                                let node_scope = format!(
                                                                                                    "{}/root", message_avatar_scope_3473
                                                                                                );
                                                                                                {
                                                                                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                                    if cached_note.avatar_kind == "human"  {
                                                                                                        children
                                                                                                            .push({
                                                                                                                let person_avatar_scope_1124 = format!(
                                                                                                                    "{}/PersonAvatar@1124", message_avatar_scope_3473
                                                                                                                );
                                                                                                                {
                                                                                                                    let node_scope = format!(
                                                                                                                        "{}/root", person_avatar_scope_1124
                                                                                                                    );
                                                                                                                    {
                                                                                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                                                        children
                                                                                                                            .push({
                                                                                                                                let principal_avatar_scope_777 = format!(
                                                                                                                                    "{}/PrincipalAvatar@777", person_avatar_scope_1124
                                                                                                                                );
                                                                                                                                {
                                                                                                                                    let node_scope = format!(
                                                                                                                                        "{}/root", principal_avatar_scope_777
                                                                                                                                    );
                                                                                                                                    {
                                                                                                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                                                                        {
                                                                                                                                            children
                                                                                                                                                .push({
                                                                                                                                                    let principal_plate_scope_823 = format!(
                                                                                                                                                        "{}/PrincipalPlate@823", principal_avatar_scope_777
                                                                                                                                                    );
                                                                                                                                                    {
                                                                                                                                                        let node_scope = format!(
                                                                                                                                                            "{}/root", principal_plate_scope_823
                                                                                                                                                        );
                                                                                                                                                        {
                                                                                                                                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                                                                                            {
                                                                                                                                                                children
                                                                                                                                                                    .push({
                                                                                                                                                                        let human_plate_scope_839 = format!(
                                                                                                                                                                            "{}/HumanPlate@839", principal_plate_scope_823
                                                                                                                                                                        );
                                                                                                                                                                        {
                                                                                                                                                                            let node_scope = format!("{}/root", human_plate_scope_839);
                                                                                                                                                                            {
                                                                                                                                                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                                                                                                                if false  {
                                                                                                                                                                                    children
                                                                                                                                                                                        .push(::ducktape_view_guest::wire::Node::Container {
                                                                                                                                                                                            shadow: ::ducktape_view_guest::wire::Shadow {
                                                                                                                                                                                                color: None,
                                                                                                                                                                                                x: None,
                                                                                                                                                                                                y: None,
                                                                                                                                                                                                blur: None,
                                                                                                                                                                                            },
                                                                                                                                                                                            max_width: None,
                                                                                                                                                                                            max_height: None,
                                                                                                                                                                                            clip: false,
                                                                                                                                                                                            key: format!("{}/@container:222", human_plate_scope_839),
                                                                                                                                                                                            width: Some(
                                                                                                                                                                                                ::ducktape_view_guest::wire::Length::Fixed((30.0) as f32),
                                                                                                                                                                                            ),
                                                                                                                                                                                            height: Some(
                                                                                                                                                                                                ::ducktape_view_guest::wire::Length::Fixed((30.0) as f32),
                                                                                                                                                                                            ),
                                                                                                                                                                                            padding: None,
                                                                                                                                                                                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                                                                                                                            align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                                                                                                                                                                            background: (Some(palette.colors[99]))
                                                                                                                                                                                                .map(::ducktape_view_guest::wire::Background::Color),
                                                                                                                                                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                                                                                                                                                color: None,
                                                                                                                                                                                                width: None,
                                                                                                                                                                                                radius: Some([
                                                                                                                                                                                                    (((30.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                    (((30.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                    (((30.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                    (((30.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                ]),
                                                                                                                                                                                            }),
                                                                                                                                                                                            snap: None,
                                                                                                                                                                                            content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                                                                                                                                                                                options: ::ducktape_view_guest::wire::TextOptions {
                                                                                                                                                                                                    height: None,
                                                                                                                                                                                                    align_y: None,
                                                                                                                                                                                                    line_height: None,
                                                                                                                                                                                                    shaping: None,
                                                                                                                                                                                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                                                                                                                                    tracking: 0.0f32,
                                                                                                                                                                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                                                                            "Geist".into(),
                                                                                                                                                                                                        ),
                                                                                                                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                                                                                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                                                                                                    }),
                                                                                                                                                                                                },
                                                                                                                                                                                                key: format!("{}/@text:230", human_plate_scope_839),
                                                                                                                                                                                                size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                                                                                                color: Some(palette.colors[100]),
                                                                                                                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                                                                                                                    monospace: false,
                                                                                                                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                                                                                                },
                                                                                                                                                                                                width: None,
                                                                                                                                                                                                align_x: None,
                                                                                                                                                                                                content: (cached_note.initial.to_owned()).to_string(),
                                                                                                                                                                                            }),
                                                                                                                                                                                        });
                                                                                                                                                                                }
                                                                                                                                                                                if true  {
                                                                                                                                                                                    children
                                                                                                                                                                                        .push(::ducktape_view_guest::wire::Node::Container {
                                                                                                                                                                                            shadow: ::ducktape_view_guest::wire::Shadow {
                                                                                                                                                                                                color: None,
                                                                                                                                                                                                x: None,
                                                                                                                                                                                                y: None,
                                                                                                                                                                                                blur: None,
                                                                                                                                                                                            },
                                                                                                                                                                                            max_width: None,
                                                                                                                                                                                            max_height: None,
                                                                                                                                                                                            clip: false,
                                                                                                                                                                                            key: format!("{}/@container:237", human_plate_scope_839),
                                                                                                                                                                                            width: Some(
                                                                                                                                                                                                ::ducktape_view_guest::wire::Length::Fixed((30.0) as f32),
                                                                                                                                                                                            ),
                                                                                                                                                                                            height: Some(
                                                                                                                                                                                                ::ducktape_view_guest::wire::Length::Fixed((30.0) as f32),
                                                                                                                                                                                            ),
                                                                                                                                                                                            padding: None,
                                                                                                                                                                                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                                                                                                                            align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                                                                                                                                                                            background: (Some(palette.colors[35]))
                                                                                                                                                                                                .map(::ducktape_view_guest::wire::Background::Color),
                                                                                                                                                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                                                                                                                                                color: None,
                                                                                                                                                                                                width: None,
                                                                                                                                                                                                radius: Some([
                                                                                                                                                                                                    (((30.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                    (((30.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                    (((30.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                    (((30.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                ]),
                                                                                                                                                                                            }),
                                                                                                                                                                                            snap: None,
                                                                                                                                                                                            content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                                                                                                                                                                                options: ::ducktape_view_guest::wire::TextOptions {
                                                                                                                                                                                                    height: None,
                                                                                                                                                                                                    align_y: None,
                                                                                                                                                                                                    line_height: None,
                                                                                                                                                                                                    shaping: None,
                                                                                                                                                                                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                                                                                                                                    tracking: 0.0f32,
                                                                                                                                                                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                                                                            "Geist".into(),
                                                                                                                                                                                                        ),
                                                                                                                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                                                                                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                                                                                                    }),
                                                                                                                                                                                                },
                                                                                                                                                                                                key: format!("{}/@text:245", human_plate_scope_839),
                                                                                                                                                                                                size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                                                                                                color: Some(palette.colors[5]),
                                                                                                                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                                                                                                                    monospace: false,
                                                                                                                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                                                                                                },
                                                                                                                                                                                                width: None,
                                                                                                                                                                                                align_x: None,
                                                                                                                                                                                                content: (cached_note.initial.to_owned()).to_string(),
                                                                                                                                                                                            }),
                                                                                                                                                                                        });
                                                                                                                                                                                }
                                                                                                                                                                                ::ducktape_view_guest::wire::Node::Linear {
                                                                                                                                                                                    max_width: None,
                                                                                                                                                                                    clip: false,
                                                                                                                                                                                    key: node_scope.clone(),
                                                                                                                                                                                    wrap: None,
                                                                                                                                                                                    axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                                                                                                                                    spacing: None,
                                                                                                                                                                                    padding: None,
                                                                                                                                                                                    width: None,
                                                                                                                                                                                    height: None,
                                                                                                                                                                                    align: None,
                                                                                                                                                                                    background: None,
                                                                                                                                                                                    border: None,
                                                                                                                                                                                    children: children,
                                                                                                                                                                                }
                                                                                                                                                                            }
                                                                                                                                                                        }
                                                                                                                                                                    });
                                                                                                                                                            }
                                                                                                                                                            ::ducktape_view_guest::wire::Node::Linear {
                                                                                                                                                                max_width: None,
                                                                                                                                                                clip: false,
                                                                                                                                                                key: node_scope.clone(),
                                                                                                                                                                wrap: None,
                                                                                                                                                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                                                                                                                spacing: None,
                                                                                                                                                                padding: None,
                                                                                                                                                                width: None,
                                                                                                                                                                height: None,
                                                                                                                                                                align: None,
                                                                                                                                                                background: None,
                                                                                                                                                                border: None,
                                                                                                                                                                children: children,
                                                                                                                                                            }
                                                                                                                                                        }
                                                                                                                                                    }
                                                                                                                                                });
                                                                                                                                        }
                                                                                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                                                                                            max_width: None,
                                                                                                                                            clip: false,
                                                                                                                                            key: node_scope.clone(),
                                                                                                                                            wrap: None,
                                                                                                                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                                                                                            spacing: None,
                                                                                                                                            padding: None,
                                                                                                                                            width: None,
                                                                                                                                            height: None,
                                                                                                                                            align: None,
                                                                                                                                            background: None,
                                                                                                                                            border: None,
                                                                                                                                            children: children,
                                                                                                                                        }
                                                                                                                                    }
                                                                                                                                }
                                                                                                                            });
                                                                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                                                                            max_width: None,
                                                                                                                            clip: false,
                                                                                                                            key: node_scope.clone(),
                                                                                                                            wrap: None,
                                                                                                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                                                                            spacing: None,
                                                                                                                            padding: None,
                                                                                                                            width: None,
                                                                                                                            height: None,
                                                                                                                            align: None,
                                                                                                                            background: None,
                                                                                                                            border: None,
                                                                                                                            children: children,
                                                                                                                        }
                                                                                                                    }
                                                                                                                }
                                                                                                            });
                                                                                                    }
                                                                                                    if (!(cached_note.avatar_kind == "human"))
                                                                                                        && (cached_note.avatar_kind == "agent") 
                                                                                                    {
                                                                                                        children
                                                                                                            .push({
                                                                                                                let agent_avatar_scope_1130 = format!(
                                                                                                                    "{}/AgentAvatar@1130", message_avatar_scope_3473
                                                                                                                );
                                                                                                                {
                                                                                                                    let node_scope = format!(
                                                                                                                        "{}/root", agent_avatar_scope_1130
                                                                                                                    );
                                                                                                                    {
                                                                                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                                                        children
                                                                                                                            .push({
                                                                                                                                let principal_avatar_scope_787 = format!(
                                                                                                                                    "{}/PrincipalAvatar@787", agent_avatar_scope_1130
                                                                                                                                );
                                                                                                                                {
                                                                                                                                    let node_scope = format!(
                                                                                                                                        "{}/root", principal_avatar_scope_787
                                                                                                                                    );
                                                                                                                                    {
                                                                                                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                                                                        {
                                                                                                                                            children
                                                                                                                                                .push({
                                                                                                                                                    let principal_plate_scope_823 = format!(
                                                                                                                                                        "{}/PrincipalPlate@823", principal_avatar_scope_787
                                                                                                                                                    );
                                                                                                                                                    {
                                                                                                                                                        let node_scope = format!(
                                                                                                                                                            "{}/root", principal_plate_scope_823
                                                                                                                                                        );
                                                                                                                                                        {
                                                                                                                                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                                                                                            children
                                                                                                                                                                .push({
                                                                                                                                                                    let agent_plate_scope_833 = format!(
                                                                                                                                                                        "{}/AgentPlate@833", principal_plate_scope_823
                                                                                                                                                                    );
                                                                                                                                                                    {
                                                                                                                                                                        let node_scope = format!("{}/root", agent_plate_scope_833);
                                                                                                                                                                        {
                                                                                                                                                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                                                                                                            if false  {
                                                                                                                                                                                children
                                                                                                                                                                                    .push({
                                                                                                                                                                                        let agent_square_scope_881 = format!(
                                                                                                                                                                                            "{}/AgentSquare@881", agent_plate_scope_833
                                                                                                                                                                                        );
                                                                                                                                                                                        {
                                                                                                                                                                                            let node_scope = format!("{}/root", agent_square_scope_881);
                                                                                                                                                                                            ::ducktape_view_guest::wire::Node::Container {
                                                                                                                                                                                                shadow: ::ducktape_view_guest::wire::Shadow {
                                                                                                                                                                                                    color: None,
                                                                                                                                                                                                    x: None,
                                                                                                                                                                                                    y: None,
                                                                                                                                                                                                    blur: None,
                                                                                                                                                                                                },
                                                                                                                                                                                                max_width: None,
                                                                                                                                                                                                max_height: None,
                                                                                                                                                                                                clip: false,
                                                                                                                                                                                                key: node_scope.clone(),
                                                                                                                                                                                                width: Some(
                                                                                                                                                                                                    ::ducktape_view_guest::wire::Length::Fixed((30.0) as f32),
                                                                                                                                                                                                ),
                                                                                                                                                                                                height: Some(
                                                                                                                                                                                                    ::ducktape_view_guest::wire::Length::Fixed((30.0) as f32),
                                                                                                                                                                                                ),
                                                                                                                                                                                                padding: None,
                                                                                                                                                                                                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                                                                                                                                align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                                                                                                                                                                                background: (Some(palette.colors[7]))
                                                                                                                                                                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                                                                                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                                                                                                                    color: None,
                                                                                                                                                                                                    width: None,
                                                                                                                                                                                                    radius: Some([
                                                                                                                                                                                                        ((10.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((10.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((10.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((10.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                    ]),
                                                                                                                                                                                                }),
                                                                                                                                                                                                snap: None,
                                                                                                                                                                                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                                                                                                                                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                                                                                                                                                        height: None,
                                                                                                                                                                                                        align_y: None,
                                                                                                                                                                                                        line_height: None,
                                                                                                                                                                                                        shaping: None,
                                                                                                                                                                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                                                                                                                                        tracking: 0.0f32,
                                                                                                                                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                                                                                "Geist Mono".into(),
                                                                                                                                                                                                            ),
                                                                                                                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                                                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                                                                                                        }),
                                                                                                                                                                                                    },
                                                                                                                                                                                                    key: format!("{}/@text:299", agent_square_scope_881),
                                                                                                                                                                                                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                                                                                                    color: Some(palette.colors[38]),
                                                                                                                                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                                                                                                                                        monospace: false,
                                                                                                                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                                                                                                    },
                                                                                                                                                                                                    width: None,
                                                                                                                                                                                                    align_x: None,
                                                                                                                                                                                                    content: (cached_note.initial.to_owned()).to_string(),
                                                                                                                                                                                                }),
                                                                                                                                                                                            }
                                                                                                                                                                                        }
                                                                                                                                                                                    });
                                                                                                                                                                            }
                                                                                                                                                                            if (false) && (true)  {
                                                                                                                                                                                children
                                                                                                                                                                                    .push({
                                                                                                                                                                                        let agent_square_scope_888 = format!(
                                                                                                                                                                                            "{}/AgentSquare@888", agent_plate_scope_833
                                                                                                                                                                                        );
                                                                                                                                                                                        {
                                                                                                                                                                                            let node_scope = format!("{}/root", agent_square_scope_888);
                                                                                                                                                                                            ::ducktape_view_guest::wire::Node::Container {
                                                                                                                                                                                                shadow: ::ducktape_view_guest::wire::Shadow {
                                                                                                                                                                                                    color: None,
                                                                                                                                                                                                    x: None,
                                                                                                                                                                                                    y: None,
                                                                                                                                                                                                    blur: None,
                                                                                                                                                                                                },
                                                                                                                                                                                                max_width: None,
                                                                                                                                                                                                max_height: None,
                                                                                                                                                                                                clip: false,
                                                                                                                                                                                                key: node_scope.clone(),
                                                                                                                                                                                                width: Some(
                                                                                                                                                                                                    ::ducktape_view_guest::wire::Length::Fixed((30.0) as f32),
                                                                                                                                                                                                ),
                                                                                                                                                                                                height: Some(
                                                                                                                                                                                                    ::ducktape_view_guest::wire::Length::Fixed((30.0) as f32),
                                                                                                                                                                                                ),
                                                                                                                                                                                                padding: None,
                                                                                                                                                                                                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                                                                                                                                align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                                                                                                                                                                                background: (Some(palette.colors[7]))
                                                                                                                                                                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                                                                                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                                                                                                                    color: None,
                                                                                                                                                                                                    width: None,
                                                                                                                                                                                                    radius: Some([
                                                                                                                                                                                                        ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                    ]),
                                                                                                                                                                                                }),
                                                                                                                                                                                                snap: None,
                                                                                                                                                                                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                                                                                                                                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                                                                                                                                                        height: None,
                                                                                                                                                                                                        align_y: None,
                                                                                                                                                                                                        line_height: None,
                                                                                                                                                                                                        shaping: None,
                                                                                                                                                                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                                                                                                                                        tracking: 0.0f32,
                                                                                                                                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                                                                                "Geist Mono".into(),
                                                                                                                                                                                                            ),
                                                                                                                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                                                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                                                                                                        }),
                                                                                                                                                                                                    },
                                                                                                                                                                                                    key: format!("{}/@text:299", agent_square_scope_888),
                                                                                                                                                                                                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                                                                                                    color: Some(palette.colors[38]),
                                                                                                                                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                                                                                                                                        monospace: false,
                                                                                                                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                                                                                                    },
                                                                                                                                                                                                    width: None,
                                                                                                                                                                                                    align_x: None,
                                                                                                                                                                                                    content: (cached_note.initial.to_owned()).to_string(),
                                                                                                                                                                                                }),
                                                                                                                                                                                            }
                                                                                                                                                                                        }
                                                                                                                                                                                    });
                                                                                                                                                                            }
                                                                                                                                                                            if true  {
                                                                                                                                                                                children
                                                                                                                                                                                    .push({
                                                                                                                                                                                        let agent_square_scope_895 = format!(
                                                                                                                                                                                            "{}/AgentSquare@895", agent_plate_scope_833
                                                                                                                                                                                        );
                                                                                                                                                                                        {
                                                                                                                                                                                            let node_scope = format!("{}/root", agent_square_scope_895);
                                                                                                                                                                                            ::ducktape_view_guest::wire::Node::Container {
                                                                                                                                                                                                shadow: ::ducktape_view_guest::wire::Shadow {
                                                                                                                                                                                                    color: None,
                                                                                                                                                                                                    x: None,
                                                                                                                                                                                                    y: None,
                                                                                                                                                                                                    blur: None,
                                                                                                                                                                                                },
                                                                                                                                                                                                max_width: None,
                                                                                                                                                                                                max_height: None,
                                                                                                                                                                                                clip: false,
                                                                                                                                                                                                key: node_scope.clone(),
                                                                                                                                                                                                width: Some(
                                                                                                                                                                                                    ::ducktape_view_guest::wire::Length::Fixed((30.0) as f32),
                                                                                                                                                                                                ),
                                                                                                                                                                                                height: Some(
                                                                                                                                                                                                    ::ducktape_view_guest::wire::Length::Fixed((30.0) as f32),
                                                                                                                                                                                                ),
                                                                                                                                                                                                padding: None,
                                                                                                                                                                                                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                                                                                                                                align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                                                                                                                                                                                background: (Some(palette.colors[7]))
                                                                                                                                                                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                                                                                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                                                                                                                    color: None,
                                                                                                                                                                                                    width: None,
                                                                                                                                                                                                    radius: Some([
                                                                                                                                                                                                        ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                    ]),
                                                                                                                                                                                                }),
                                                                                                                                                                                                snap: None,
                                                                                                                                                                                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                                                                                                                                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                                                                                                                                                        height: None,
                                                                                                                                                                                                        align_y: None,
                                                                                                                                                                                                        line_height: None,
                                                                                                                                                                                                        shaping: None,
                                                                                                                                                                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                                                                                                                                        tracking: 0.0f32,
                                                                                                                                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                                                                                "Geist Mono".into(),
                                                                                                                                                                                                            ),
                                                                                                                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                                                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                                                                                                        }),
                                                                                                                                                                                                    },
                                                                                                                                                                                                    key: format!("{}/@text:299", agent_square_scope_895),
                                                                                                                                                                                                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                                                                                                    color: Some(palette.colors[38]),
                                                                                                                                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                                                                                                                                        monospace: false,
                                                                                                                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                                                                                                    },
                                                                                                                                                                                                    width: None,
                                                                                                                                                                                                    align_x: None,
                                                                                                                                                                                                    content: (cached_note.initial.to_owned()).to_string(),
                                                                                                                                                                                                }),
                                                                                                                                                                                            }
                                                                                                                                                                                        }
                                                                                                                                                                                    });
                                                                                                                                                                            }
                                                                                                                                                                            if (true) && (false)  {
                                                                                                                                                                                children
                                                                                                                                                                                    .push({
                                                                                                                                                                                        let agent_square_scope_902 = format!(
                                                                                                                                                                                            "{}/AgentSquare@902", agent_plate_scope_833
                                                                                                                                                                                        );
                                                                                                                                                                                        {
                                                                                                                                                                                            let node_scope = format!("{}/root", agent_square_scope_902);
                                                                                                                                                                                            ::ducktape_view_guest::wire::Node::Container {
                                                                                                                                                                                                shadow: ::ducktape_view_guest::wire::Shadow {
                                                                                                                                                                                                    color: None,
                                                                                                                                                                                                    x: None,
                                                                                                                                                                                                    y: None,
                                                                                                                                                                                                    blur: None,
                                                                                                                                                                                                },
                                                                                                                                                                                                max_width: None,
                                                                                                                                                                                                max_height: None,
                                                                                                                                                                                                clip: false,
                                                                                                                                                                                                key: node_scope.clone(),
                                                                                                                                                                                                width: Some(
                                                                                                                                                                                                    ::ducktape_view_guest::wire::Length::Fixed((30.0) as f32),
                                                                                                                                                                                                ),
                                                                                                                                                                                                height: Some(
                                                                                                                                                                                                    ::ducktape_view_guest::wire::Length::Fixed((30.0) as f32),
                                                                                                                                                                                                ),
                                                                                                                                                                                                padding: None,
                                                                                                                                                                                                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                                                                                                                                align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                                                                                                                                                                                background: (Some(palette.colors[7]))
                                                                                                                                                                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                                                                                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                                                                                                                    color: None,
                                                                                                                                                                                                    width: None,
                                                                                                                                                                                                    radius: Some([
                                                                                                                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                    ]),
                                                                                                                                                                                                }),
                                                                                                                                                                                                snap: None,
                                                                                                                                                                                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                                                                                                                                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                                                                                                                                                        height: None,
                                                                                                                                                                                                        align_y: None,
                                                                                                                                                                                                        line_height: None,
                                                                                                                                                                                                        shaping: None,
                                                                                                                                                                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                                                                                                                                        tracking: 0.0f32,
                                                                                                                                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                                                                                "Geist Mono".into(),
                                                                                                                                                                                                            ),
                                                                                                                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                                                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                                                                                                        }),
                                                                                                                                                                                                    },
                                                                                                                                                                                                    key: format!("{}/@text:299", agent_square_scope_902),
                                                                                                                                                                                                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                                                                                                    color: Some(palette.colors[38]),
                                                                                                                                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                                                                                                                                        monospace: false,
                                                                                                                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                                                                                                    },
                                                                                                                                                                                                    width: None,
                                                                                                                                                                                                    align_x: None,
                                                                                                                                                                                                    content: (cached_note.initial.to_owned()).to_string(),
                                                                                                                                                                                                }),
                                                                                                                                                                                            }
                                                                                                                                                                                        }
                                                                                                                                                                                    });
                                                                                                                                                                            }
                                                                                                                                                                            if false  {
                                                                                                                                                                                children
                                                                                                                                                                                    .push({
                                                                                                                                                                                        let agent_square_scope_909 = format!(
                                                                                                                                                                                            "{}/AgentSquare@909", agent_plate_scope_833
                                                                                                                                                                                        );
                                                                                                                                                                                        {
                                                                                                                                                                                            let node_scope = format!("{}/root", agent_square_scope_909);
                                                                                                                                                                                            ::ducktape_view_guest::wire::Node::Container {
                                                                                                                                                                                                shadow: ::ducktape_view_guest::wire::Shadow {
                                                                                                                                                                                                    color: None,
                                                                                                                                                                                                    x: None,
                                                                                                                                                                                                    y: None,
                                                                                                                                                                                                    blur: None,
                                                                                                                                                                                                },
                                                                                                                                                                                                max_width: None,
                                                                                                                                                                                                max_height: None,
                                                                                                                                                                                                clip: false,
                                                                                                                                                                                                key: node_scope.clone(),
                                                                                                                                                                                                width: Some(
                                                                                                                                                                                                    ::ducktape_view_guest::wire::Length::Fixed((30.0) as f32),
                                                                                                                                                                                                ),
                                                                                                                                                                                                height: Some(
                                                                                                                                                                                                    ::ducktape_view_guest::wire::Length::Fixed((30.0) as f32),
                                                                                                                                                                                                ),
                                                                                                                                                                                                padding: None,
                                                                                                                                                                                                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                                                                                                                                align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                                                                                                                                                                                background: (Some(palette.colors[7]))
                                                                                                                                                                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                                                                                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                                                                                                                    color: None,
                                                                                                                                                                                                    width: None,
                                                                                                                                                                                                    radius: Some([
                                                                                                                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                    ]),
                                                                                                                                                                                                }),
                                                                                                                                                                                                snap: None,
                                                                                                                                                                                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                                                                                                                                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                                                                                                                                                        height: None,
                                                                                                                                                                                                        align_y: None,
                                                                                                                                                                                                        line_height: None,
                                                                                                                                                                                                        shaping: None,
                                                                                                                                                                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                                                                                                                                        tracking: 0.0f32,
                                                                                                                                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                                                                                "Geist Mono".into(),
                                                                                                                                                                                                            ),
                                                                                                                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                                                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                                                                                                        }),
                                                                                                                                                                                                    },
                                                                                                                                                                                                    key: format!("{}/@text:299", agent_square_scope_909),
                                                                                                                                                                                                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                                                                                                    color: Some(palette.colors[38]),
                                                                                                                                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                                                                                                                                        monospace: false,
                                                                                                                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                                                                                                    },
                                                                                                                                                                                                    width: None,
                                                                                                                                                                                                    align_x: None,
                                                                                                                                                                                                    content: (cached_note.initial.to_owned()).to_string(),
                                                                                                                                                                                                }),
                                                                                                                                                                                            }
                                                                                                                                                                                        }
                                                                                                                                                                                    });
                                                                                                                                                                            }
                                                                                                                                                                            ::ducktape_view_guest::wire::Node::Linear {
                                                                                                                                                                                max_width: None,
                                                                                                                                                                                clip: false,
                                                                                                                                                                                key: node_scope.clone(),
                                                                                                                                                                                wrap: None,
                                                                                                                                                                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                                                                                                                                spacing: None,
                                                                                                                                                                                padding: None,
                                                                                                                                                                                width: None,
                                                                                                                                                                                height: None,
                                                                                                                                                                                align: None,
                                                                                                                                                                                background: None,
                                                                                                                                                                                border: None,
                                                                                                                                                                                children: children,
                                                                                                                                                                            }
                                                                                                                                                                        }
                                                                                                                                                                    }
                                                                                                                                                                });
                                                                                                                                                            ::ducktape_view_guest::wire::Node::Linear {
                                                                                                                                                                max_width: None,
                                                                                                                                                                clip: false,
                                                                                                                                                                key: node_scope.clone(),
                                                                                                                                                                wrap: None,
                                                                                                                                                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                                                                                                                spacing: None,
                                                                                                                                                                padding: None,
                                                                                                                                                                width: None,
                                                                                                                                                                height: None,
                                                                                                                                                                align: None,
                                                                                                                                                                background: None,
                                                                                                                                                                border: None,
                                                                                                                                                                children: children,
                                                                                                                                                            }
                                                                                                                                                        }
                                                                                                                                                    }
                                                                                                                                                });
                                                                                                                                        }
                                                                                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                                                                                            max_width: None,
                                                                                                                                            clip: false,
                                                                                                                                            key: node_scope.clone(),
                                                                                                                                            wrap: None,
                                                                                                                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                                                                                            spacing: None,
                                                                                                                                            padding: None,
                                                                                                                                            width: None,
                                                                                                                                            height: None,
                                                                                                                                            align: None,
                                                                                                                                            background: None,
                                                                                                                                            border: None,
                                                                                                                                            children: children,
                                                                                                                                        }
                                                                                                                                    }
                                                                                                                                }
                                                                                                                            });
                                                                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                                                                            max_width: None,
                                                                                                                            clip: false,
                                                                                                                            key: node_scope.clone(),
                                                                                                                            wrap: None,
                                                                                                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                                                                            spacing: None,
                                                                                                                            padding: None,
                                                                                                                            width: None,
                                                                                                                            height: None,
                                                                                                                            align: None,
                                                                                                                            background: None,
                                                                                                                            border: None,
                                                                                                                            children: children,
                                                                                                                        }
                                                                                                                    }
                                                                                                                }
                                                                                                            });
                                                                                                    }
                                                                                                    if !((cached_note.avatar_kind == "human")
                                                                                                        || (cached_note.avatar_kind == "agent")) 
                                                                                                    {
                                                                                                        children
                                                                                                            .push({
                                                                                                                let agent_avatar_scope_1136 = format!(
                                                                                                                    "{}/AgentAvatar@1136", message_avatar_scope_3473
                                                                                                                );
                                                                                                                {
                                                                                                                    let node_scope = format!(
                                                                                                                        "{}/root", agent_avatar_scope_1136
                                                                                                                    );
                                                                                                                    {
                                                                                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                                                        children
                                                                                                                            .push({
                                                                                                                                let principal_avatar_scope_787 = format!(
                                                                                                                                    "{}/PrincipalAvatar@787", agent_avatar_scope_1136
                                                                                                                                );
                                                                                                                                {
                                                                                                                                    let node_scope = format!(
                                                                                                                                        "{}/root", principal_avatar_scope_787
                                                                                                                                    );
                                                                                                                                    {
                                                                                                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                                                                        {
                                                                                                                                            children
                                                                                                                                                .push({
                                                                                                                                                    let principal_plate_scope_823 = format!(
                                                                                                                                                        "{}/PrincipalPlate@823", principal_avatar_scope_787
                                                                                                                                                    );
                                                                                                                                                    {
                                                                                                                                                        let node_scope = format!(
                                                                                                                                                            "{}/root", principal_plate_scope_823
                                                                                                                                                        );
                                                                                                                                                        {
                                                                                                                                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                                                                                            children
                                                                                                                                                                .push({
                                                                                                                                                                    let agent_plate_scope_833 = format!(
                                                                                                                                                                        "{}/AgentPlate@833", principal_plate_scope_823
                                                                                                                                                                    );
                                                                                                                                                                    {
                                                                                                                                                                        let node_scope = format!("{}/root", agent_plate_scope_833);
                                                                                                                                                                        {
                                                                                                                                                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                                                                                                            if false  {
                                                                                                                                                                                children
                                                                                                                                                                                    .push({
                                                                                                                                                                                        let agent_square_scope_881 = format!(
                                                                                                                                                                                            "{}/AgentSquare@881", agent_plate_scope_833
                                                                                                                                                                                        );
                                                                                                                                                                                        {
                                                                                                                                                                                            let node_scope = format!("{}/root", agent_square_scope_881);
                                                                                                                                                                                            ::ducktape_view_guest::wire::Node::Container {
                                                                                                                                                                                                shadow: ::ducktape_view_guest::wire::Shadow {
                                                                                                                                                                                                    color: None,
                                                                                                                                                                                                    x: None,
                                                                                                                                                                                                    y: None,
                                                                                                                                                                                                    blur: None,
                                                                                                                                                                                                },
                                                                                                                                                                                                max_width: None,
                                                                                                                                                                                                max_height: None,
                                                                                                                                                                                                clip: false,
                                                                                                                                                                                                key: node_scope.clone(),
                                                                                                                                                                                                width: Some(
                                                                                                                                                                                                    ::ducktape_view_guest::wire::Length::Fixed((30.0) as f32),
                                                                                                                                                                                                ),
                                                                                                                                                                                                height: Some(
                                                                                                                                                                                                    ::ducktape_view_guest::wire::Length::Fixed((30.0) as f32),
                                                                                                                                                                                                ),
                                                                                                                                                                                                padding: None,
                                                                                                                                                                                                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                                                                                                                                align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                                                                                                                                                                                background: (Some(palette.colors[7]))
                                                                                                                                                                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                                                                                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                                                                                                                    color: None,
                                                                                                                                                                                                    width: None,
                                                                                                                                                                                                    radius: Some([
                                                                                                                                                                                                        ((10.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((10.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((10.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((10.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                    ]),
                                                                                                                                                                                                }),
                                                                                                                                                                                                snap: None,
                                                                                                                                                                                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                                                                                                                                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                                                                                                                                                        height: None,
                                                                                                                                                                                                        align_y: None,
                                                                                                                                                                                                        line_height: None,
                                                                                                                                                                                                        shaping: None,
                                                                                                                                                                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                                                                                                                                        tracking: 0.0f32,
                                                                                                                                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                                                                                "Geist Mono".into(),
                                                                                                                                                                                                            ),
                                                                                                                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                                                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                                                                                                        }),
                                                                                                                                                                                                    },
                                                                                                                                                                                                    key: format!("{}/@text:299", agent_square_scope_881),
                                                                                                                                                                                                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                                                                                                    color: Some(palette.colors[38]),
                                                                                                                                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                                                                                                                                        monospace: false,
                                                                                                                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                                                                                                    },
                                                                                                                                                                                                    width: None,
                                                                                                                                                                                                    align_x: None,
                                                                                                                                                                                                    content: (cached_note.initial.to_owned()).to_string(),
                                                                                                                                                                                                }),
                                                                                                                                                                                            }
                                                                                                                                                                                        }
                                                                                                                                                                                    });
                                                                                                                                                                            }
                                                                                                                                                                            if (false) && (true)  {
                                                                                                                                                                                children
                                                                                                                                                                                    .push({
                                                                                                                                                                                        let agent_square_scope_888 = format!(
                                                                                                                                                                                            "{}/AgentSquare@888", agent_plate_scope_833
                                                                                                                                                                                        );
                                                                                                                                                                                        {
                                                                                                                                                                                            let node_scope = format!("{}/root", agent_square_scope_888);
                                                                                                                                                                                            ::ducktape_view_guest::wire::Node::Container {
                                                                                                                                                                                                shadow: ::ducktape_view_guest::wire::Shadow {
                                                                                                                                                                                                    color: None,
                                                                                                                                                                                                    x: None,
                                                                                                                                                                                                    y: None,
                                                                                                                                                                                                    blur: None,
                                                                                                                                                                                                },
                                                                                                                                                                                                max_width: None,
                                                                                                                                                                                                max_height: None,
                                                                                                                                                                                                clip: false,
                                                                                                                                                                                                key: node_scope.clone(),
                                                                                                                                                                                                width: Some(
                                                                                                                                                                                                    ::ducktape_view_guest::wire::Length::Fixed((30.0) as f32),
                                                                                                                                                                                                ),
                                                                                                                                                                                                height: Some(
                                                                                                                                                                                                    ::ducktape_view_guest::wire::Length::Fixed((30.0) as f32),
                                                                                                                                                                                                ),
                                                                                                                                                                                                padding: None,
                                                                                                                                                                                                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                                                                                                                                align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                                                                                                                                                                                background: (Some(palette.colors[7]))
                                                                                                                                                                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                                                                                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                                                                                                                    color: None,
                                                                                                                                                                                                    width: None,
                                                                                                                                                                                                    radius: Some([
                                                                                                                                                                                                        ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                    ]),
                                                                                                                                                                                                }),
                                                                                                                                                                                                snap: None,
                                                                                                                                                                                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                                                                                                                                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                                                                                                                                                        height: None,
                                                                                                                                                                                                        align_y: None,
                                                                                                                                                                                                        line_height: None,
                                                                                                                                                                                                        shaping: None,
                                                                                                                                                                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                                                                                                                                        tracking: 0.0f32,
                                                                                                                                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                                                                                "Geist Mono".into(),
                                                                                                                                                                                                            ),
                                                                                                                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                                                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                                                                                                        }),
                                                                                                                                                                                                    },
                                                                                                                                                                                                    key: format!("{}/@text:299", agent_square_scope_888),
                                                                                                                                                                                                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                                                                                                    color: Some(palette.colors[38]),
                                                                                                                                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                                                                                                                                        monospace: false,
                                                                                                                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                                                                                                    },
                                                                                                                                                                                                    width: None,
                                                                                                                                                                                                    align_x: None,
                                                                                                                                                                                                    content: (cached_note.initial.to_owned()).to_string(),
                                                                                                                                                                                                }),
                                                                                                                                                                                            }
                                                                                                                                                                                        }
                                                                                                                                                                                    });
                                                                                                                                                                            }
                                                                                                                                                                            if true  {
                                                                                                                                                                                children
                                                                                                                                                                                    .push({
                                                                                                                                                                                        let agent_square_scope_895 = format!(
                                                                                                                                                                                            "{}/AgentSquare@895", agent_plate_scope_833
                                                                                                                                                                                        );
                                                                                                                                                                                        {
                                                                                                                                                                                            let node_scope = format!("{}/root", agent_square_scope_895);
                                                                                                                                                                                            ::ducktape_view_guest::wire::Node::Container {
                                                                                                                                                                                                shadow: ::ducktape_view_guest::wire::Shadow {
                                                                                                                                                                                                    color: None,
                                                                                                                                                                                                    x: None,
                                                                                                                                                                                                    y: None,
                                                                                                                                                                                                    blur: None,
                                                                                                                                                                                                },
                                                                                                                                                                                                max_width: None,
                                                                                                                                                                                                max_height: None,
                                                                                                                                                                                                clip: false,
                                                                                                                                                                                                key: node_scope.clone(),
                                                                                                                                                                                                width: Some(
                                                                                                                                                                                                    ::ducktape_view_guest::wire::Length::Fixed((30.0) as f32),
                                                                                                                                                                                                ),
                                                                                                                                                                                                height: Some(
                                                                                                                                                                                                    ::ducktape_view_guest::wire::Length::Fixed((30.0) as f32),
                                                                                                                                                                                                ),
                                                                                                                                                                                                padding: None,
                                                                                                                                                                                                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                                                                                                                                align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                                                                                                                                                                                background: (Some(palette.colors[7]))
                                                                                                                                                                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                                                                                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                                                                                                                    color: None,
                                                                                                                                                                                                    width: None,
                                                                                                                                                                                                    radius: Some([
                                                                                                                                                                                                        ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                    ]),
                                                                                                                                                                                                }),
                                                                                                                                                                                                snap: None,
                                                                                                                                                                                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                                                                                                                                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                                                                                                                                                        height: None,
                                                                                                                                                                                                        align_y: None,
                                                                                                                                                                                                        line_height: None,
                                                                                                                                                                                                        shaping: None,
                                                                                                                                                                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                                                                                                                                        tracking: 0.0f32,
                                                                                                                                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                                                                                "Geist Mono".into(),
                                                                                                                                                                                                            ),
                                                                                                                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                                                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                                                                                                        }),
                                                                                                                                                                                                    },
                                                                                                                                                                                                    key: format!("{}/@text:299", agent_square_scope_895),
                                                                                                                                                                                                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                                                                                                    color: Some(palette.colors[38]),
                                                                                                                                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                                                                                                                                        monospace: false,
                                                                                                                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                                                                                                    },
                                                                                                                                                                                                    width: None,
                                                                                                                                                                                                    align_x: None,
                                                                                                                                                                                                    content: (cached_note.initial.to_owned()).to_string(),
                                                                                                                                                                                                }),
                                                                                                                                                                                            }
                                                                                                                                                                                        }
                                                                                                                                                                                    });
                                                                                                                                                                            }
                                                                                                                                                                            if (true) && (false)  {
                                                                                                                                                                                children
                                                                                                                                                                                    .push({
                                                                                                                                                                                        let agent_square_scope_902 = format!(
                                                                                                                                                                                            "{}/AgentSquare@902", agent_plate_scope_833
                                                                                                                                                                                        );
                                                                                                                                                                                        {
                                                                                                                                                                                            let node_scope = format!("{}/root", agent_square_scope_902);
                                                                                                                                                                                            ::ducktape_view_guest::wire::Node::Container {
                                                                                                                                                                                                shadow: ::ducktape_view_guest::wire::Shadow {
                                                                                                                                                                                                    color: None,
                                                                                                                                                                                                    x: None,
                                                                                                                                                                                                    y: None,
                                                                                                                                                                                                    blur: None,
                                                                                                                                                                                                },
                                                                                                                                                                                                max_width: None,
                                                                                                                                                                                                max_height: None,
                                                                                                                                                                                                clip: false,
                                                                                                                                                                                                key: node_scope.clone(),
                                                                                                                                                                                                width: Some(
                                                                                                                                                                                                    ::ducktape_view_guest::wire::Length::Fixed((30.0) as f32),
                                                                                                                                                                                                ),
                                                                                                                                                                                                height: Some(
                                                                                                                                                                                                    ::ducktape_view_guest::wire::Length::Fixed((30.0) as f32),
                                                                                                                                                                                                ),
                                                                                                                                                                                                padding: None,
                                                                                                                                                                                                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                                                                                                                                align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                                                                                                                                                                                background: (Some(palette.colors[7]))
                                                                                                                                                                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                                                                                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                                                                                                                    color: None,
                                                                                                                                                                                                    width: None,
                                                                                                                                                                                                    radius: Some([
                                                                                                                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                    ]),
                                                                                                                                                                                                }),
                                                                                                                                                                                                snap: None,
                                                                                                                                                                                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                                                                                                                                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                                                                                                                                                        height: None,
                                                                                                                                                                                                        align_y: None,
                                                                                                                                                                                                        line_height: None,
                                                                                                                                                                                                        shaping: None,
                                                                                                                                                                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                                                                                                                                        tracking: 0.0f32,
                                                                                                                                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                                                                                "Geist Mono".into(),
                                                                                                                                                                                                            ),
                                                                                                                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                                                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                                                                                                        }),
                                                                                                                                                                                                    },
                                                                                                                                                                                                    key: format!("{}/@text:299", agent_square_scope_902),
                                                                                                                                                                                                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                                                                                                    color: Some(palette.colors[38]),
                                                                                                                                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                                                                                                                                        monospace: false,
                                                                                                                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                                                                                                    },
                                                                                                                                                                                                    width: None,
                                                                                                                                                                                                    align_x: None,
                                                                                                                                                                                                    content: (cached_note.initial.to_owned()).to_string(),
                                                                                                                                                                                                }),
                                                                                                                                                                                            }
                                                                                                                                                                                        }
                                                                                                                                                                                    });
                                                                                                                                                                            }
                                                                                                                                                                            if false  {
                                                                                                                                                                                children
                                                                                                                                                                                    .push({
                                                                                                                                                                                        let agent_square_scope_909 = format!(
                                                                                                                                                                                            "{}/AgentSquare@909", agent_plate_scope_833
                                                                                                                                                                                        );
                                                                                                                                                                                        {
                                                                                                                                                                                            let node_scope = format!("{}/root", agent_square_scope_909);
                                                                                                                                                                                            ::ducktape_view_guest::wire::Node::Container {
                                                                                                                                                                                                shadow: ::ducktape_view_guest::wire::Shadow {
                                                                                                                                                                                                    color: None,
                                                                                                                                                                                                    x: None,
                                                                                                                                                                                                    y: None,
                                                                                                                                                                                                    blur: None,
                                                                                                                                                                                                },
                                                                                                                                                                                                max_width: None,
                                                                                                                                                                                                max_height: None,
                                                                                                                                                                                                clip: false,
                                                                                                                                                                                                key: node_scope.clone(),
                                                                                                                                                                                                width: Some(
                                                                                                                                                                                                    ::ducktape_view_guest::wire::Length::Fixed((30.0) as f32),
                                                                                                                                                                                                ),
                                                                                                                                                                                                height: Some(
                                                                                                                                                                                                    ::ducktape_view_guest::wire::Length::Fixed((30.0) as f32),
                                                                                                                                                                                                ),
                                                                                                                                                                                                padding: None,
                                                                                                                                                                                                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                                                                                                                                align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                                                                                                                                                                                background: (Some(palette.colors[7]))
                                                                                                                                                                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                                                                                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                                                                                                                    color: None,
                                                                                                                                                                                                    width: None,
                                                                                                                                                                                                    radius: Some([
                                                                                                                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                                    ]),
                                                                                                                                                                                                }),
                                                                                                                                                                                                snap: None,
                                                                                                                                                                                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                                                                                                                                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                                                                                                                                                        height: None,
                                                                                                                                                                                                        align_y: None,
                                                                                                                                                                                                        line_height: None,
                                                                                                                                                                                                        shaping: None,
                                                                                                                                                                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                                                                                                                                        tracking: 0.0f32,
                                                                                                                                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                                                                                "Geist Mono".into(),
                                                                                                                                                                                                            ),
                                                                                                                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                                                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                                                                                                        }),
                                                                                                                                                                                                    },
                                                                                                                                                                                                    key: format!("{}/@text:299", agent_square_scope_909),
                                                                                                                                                                                                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                                                                                                    color: Some(palette.colors[38]),
                                                                                                                                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                                                                                                                                        monospace: false,
                                                                                                                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                                                                                                    },
                                                                                                                                                                                                    width: None,
                                                                                                                                                                                                    align_x: None,
                                                                                                                                                                                                    content: (cached_note.initial.to_owned()).to_string(),
                                                                                                                                                                                                }),
                                                                                                                                                                                            }
                                                                                                                                                                                        }
                                                                                                                                                                                    });
                                                                                                                                                                            }
                                                                                                                                                                            ::ducktape_view_guest::wire::Node::Linear {
                                                                                                                                                                                max_width: None,
                                                                                                                                                                                clip: false,
                                                                                                                                                                                key: node_scope.clone(),
                                                                                                                                                                                wrap: None,
                                                                                                                                                                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                                                                                                                                spacing: None,
                                                                                                                                                                                padding: None,
                                                                                                                                                                                width: None,
                                                                                                                                                                                height: None,
                                                                                                                                                                                align: None,
                                                                                                                                                                                background: None,
                                                                                                                                                                                border: None,
                                                                                                                                                                                children: children,
                                                                                                                                                                            }
                                                                                                                                                                        }
                                                                                                                                                                    }
                                                                                                                                                                });
                                                                                                                                                            ::ducktape_view_guest::wire::Node::Linear {
                                                                                                                                                                max_width: None,
                                                                                                                                                                clip: false,
                                                                                                                                                                key: node_scope.clone(),
                                                                                                                                                                wrap: None,
                                                                                                                                                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                                                                                                                spacing: None,
                                                                                                                                                                padding: None,
                                                                                                                                                                width: None,
                                                                                                                                                                height: None,
                                                                                                                                                                align: None,
                                                                                                                                                                background: None,
                                                                                                                                                                border: None,
                                                                                                                                                                children: children,
                                                                                                                                                            }
                                                                                                                                                        }
                                                                                                                                                    }
                                                                                                                                                });
                                                                                                                                        }
                                                                                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                                                                                            max_width: None,
                                                                                                                                            clip: false,
                                                                                                                                            key: node_scope.clone(),
                                                                                                                                            wrap: None,
                                                                                                                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                                                                                            spacing: None,
                                                                                                                                            padding: None,
                                                                                                                                            width: None,
                                                                                                                                            height: None,
                                                                                                                                            align: None,
                                                                                                                                            background: None,
                                                                                                                                            border: None,
                                                                                                                                            children: children,
                                                                                                                                        }
                                                                                                                                    }
                                                                                                                                }
                                                                                                                            });
                                                                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                                                                            max_width: None,
                                                                                                                            clip: false,
                                                                                                                            key: node_scope.clone(),
                                                                                                                            wrap: None,
                                                                                                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                                                                            spacing: None,
                                                                                                                            padding: None,
                                                                                                                            width: None,
                                                                                                                            height: None,
                                                                                                                            align: None,
                                                                                                                            background: None,
                                                                                                                            border: None,
                                                                                                                            children: children,
                                                                                                                        }
                                                                                                                    }
                                                                                                                }
                                                                                                            });
                                                                                                    }
                                                                                                    ::ducktape_view_guest::wire::Node::Stack {
                                                                                                        key: node_scope.clone(),
                                                                                                        width: Some(
                                                                                                            ::ducktape_view_guest::wire::Length::Fixed((30.0) as f32),
                                                                                                        ),
                                                                                                        height: Some(
                                                                                                            ::ducktape_view_guest::wire::Length::Fixed((30.0) as f32),
                                                                                                        ),
                                                                                                        padding: None,
                                                                                                        background: None,
                                                                                                        border: None,
                                                                                                        clip: false,
                                                                                                        under: 0u32,
                                                                                                        children: children,
                                                                                                    }
                                                                                                }
                                                                                            }
                                                                                        });
                                                                                    children
                                                                                        .push({
                                                                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                            children
                                                                                                .push({
                                                                                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                                    children
                                                                                                        .push(::ducktape_view_guest::wire::Node::Text {
                                                                                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                                                                                height: None,
                                                                                                                align_y: None,
                                                                                                                line_height: None,
                                                                                                                shaping: None,
                                                                                                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                                                tracking: 0.0f32,
                                                                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                        "Geist".into(),
                                                                                                                    ),
                                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                }),
                                                                                                            },
                                                                                                            key: format!("{}/@text:737", lazy_scope),
                                                                                                            size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[4]),
                                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                                monospace: false,
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                            },
                                                                                                            width: None,
                                                                                                            align_x: None,
                                                                                                            content: (cached_note.author.to_owned()).to_string(),
                                                                                                        });
                                                                                                    children
                                                                                                        .push(::ducktape_view_guest::wire::Node::Text {
                                                                                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                                                                                height: None,
                                                                                                                align_y: None,
                                                                                                                line_height: None,
                                                                                                                shaping: None,
                                                                                                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                                                tracking: 0.0f32,
                                                                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                        "Geist Mono".into(),
                                                                                                                    ),
                                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                }),
                                                                                                            },
                                                                                                            key: format!("{}/@text:743", lazy_scope),
                                                                                                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[71]),
                                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                                monospace: false,
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                            },
                                                                                                            width: None,
                                                                                                            align_x: None,
                                                                                                            content: (cached_note.meta.to_owned()).to_string(),
                                                                                                        });
                                                                                                    children
                                                                                                        .push(::ducktape_view_guest::wire::Node::Space {
                                                                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                                            height: None,
                                                                                                        });
                                                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                                                        max_width: None,
                                                                                                        clip: false,
                                                                                                        key: format!("{}/@layout:732", lazy_scope),
                                                                                                        wrap: None,
                                                                                                        axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                                                        spacing: Some((7.0) as f32),
                                                                                                        padding: None,
                                                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                                        height: None,
                                                                                                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                                        background: None,
                                                                                                        border: None,
                                                                                                        children: children,
                                                                                                    }
                                                                                                });
                                                                                            children
                                                                                                .push({
                                                                                                    let message_body_scope_3493 = format!(
                                                                                                        "{}/MessageBody@3493", lazy_scope
                                                                                                    );
                                                                                                    {
                                                                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                                        children
                                                                                                            .push({
                                                                                                                let rich_body_scope_1146 = format!(
                                                                                                                    "{}/RichBody@1146", message_body_scope_3493
                                                                                                                );
                                                                                                                {
                                                                                                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                                                    for (index, block) in cached_note.blocks.iter().enumerate()
                                                                                                                    {
                                                                                                                        let for_scope = format!(
                                                                                                                            "{}/@for:1031({})", rich_body_scope_1146, index
                                                                                                                        );
                                                                                                                        if block.kind == "divider"  {
                                                                                                                            children
                                                                                                                                .push({
                                                                                                                                    let component_separator_scope_1033 = format!(
                                                                                                                                        "{}/Separator@1033", for_scope
                                                                                                                                    );
                                                                                                                                    {
                                                                                                                                        let node_scope = format!(
                                                                                                                                            "{}/root", component_separator_scope_1033
                                                                                                                                        );
                                                                                                                                        ::ducktape_view_guest::wire::Node::Rule {
                                                                                                                                            key: node_scope.clone(),
                                                                                                                                            axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                                                                                            thickness: (1.0) as f32,
                                                                                                                                            color: Some(palette.colors[39]),
                                                                                                                                            weak: false,
                                                                                                                                            radius: None,
                                                                                                                                            snap: None,
                                                                                                                                        }
                                                                                                                                    }
                                                                                                                                });
                                                                                                                        }
                                                                                                                        if block.kind == "code"  {
                                                                                                                            children
                                                                                                                                .push(::ducktape_view_guest::wire::Node::Container {
                                                                                                                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                                                                                                                        color: None,
                                                                                                                                        x: None,
                                                                                                                                        y: None,
                                                                                                                                        blur: None,
                                                                                                                                    },
                                                                                                                                    max_width: None,
                                                                                                                                    max_height: None,
                                                                                                                                    clip: false,
                                                                                                                                    key: format!("{}/@container:419", for_scope),
                                                                                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                                                                    height: None,
                                                                                                                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                                                                                                                        top: (11.0) as f32,
                                                                                                                                        right: (11.0) as f32,
                                                                                                                                        bottom: (11.0) as f32,
                                                                                                                                        left: (11.0) as f32,
                                                                                                                                    }),
                                                                                                                                    align_x: None,
                                                                                                                                    align_y: None,
                                                                                                                                    background: (Some(palette.colors[6]))
                                                                                                                                        .map(::ducktape_view_guest::wire::Background::Color),
                                                                                                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                                                                                                        color: Some(palette.colors[40]),
                                                                                                                                        width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                                                                        radius: Some([
                                                                                                                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                        ]),
                                                                                                                                    }),
                                                                                                                                    snap: None,
                                                                                                                                    content: Box::new({
                                                                                                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                                                                        if !(block.lang).is_empty()  {
                                                                                                                                            children
                                                                                                                                                .push(::ducktape_view_guest::wire::Node::Text {
                                                                                                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                                                                                                        height: None,
                                                                                                                                                        align_y: None,
                                                                                                                                                        line_height: None,
                                                                                                                                                        shaping: None,
                                                                                                                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                                                                                        tracking: 0.0f32,
                                                                                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                                "Geist Mono".into(),
                                                                                                                                                            ),
                                                                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                                                        }),
                                                                                                                                                    },
                                                                                                                                                    key: format!("{}/@text:429", for_scope),
                                                                                                                                                    size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                                                    color: Some(palette.colors[73]),
                                                                                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                                                                                        monospace: false,
                                                                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                                                    },
                                                                                                                                                    width: None,
                                                                                                                                                    align_x: None,
                                                                                                                                                    content: (block.lang.to_owned()).to_string(),
                                                                                                                                                });
                                                                                                                                        }
                                                                                                                                        children
                                                                                                                                            .push(::ducktape_view_guest::wire::Node::Text {
                                                                                                                                                options: ::ducktape_view_guest::wire::TextOptions {
                                                                                                                                                    height: None,
                                                                                                                                                    align_y: None,
                                                                                                                                                    line_height: Some(
                                                                                                                                                        ::ducktape_view_guest::wire::LineHeight::Relative(
                                                                                                                                                            ((1.5) as f32).max(f32::EPSILON).min(f32::MAX),
                                                                                                                                                        ),
                                                                                                                                                    ),
                                                                                                                                                    shaping: None,
                                                                                                                                                    wrapping: Some(
                                                                                                                                                        ::ducktape_view_guest::wire::Wrapping::WordOrGlyph,
                                                                                                                                                    ),
                                                                                                                                                    tracking: 0.0f32,
                                                                                                                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                            "Geist Mono".into(),
                                                                                                                                                        ),
                                                                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                                                    }),
                                                                                                                                                },
                                                                                                                                                key: format!("{}/@text:435", for_scope),
                                                                                                                                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                                                color: Some(palette.colors[4]),
                                                                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                                                                    monospace: false,
                                                                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                                                },
                                                                                                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                                                                                align_x: None,
                                                                                                                                                content: (block.text.to_owned()).to_string(),
                                                                                                                                            });
                                                                                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                                                                                            max_width: None,
                                                                                                                                            clip: false,
                                                                                                                                            key: format!("{}/@layout:427", for_scope),
                                                                                                                                            wrap: None,
                                                                                                                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                                                                                            spacing: Some((6.0) as f32),
                                                                                                                                            padding: None,
                                                                                                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                                                                            height: None,
                                                                                                                                            align: None,
                                                                                                                                            background: None,
                                                                                                                                            border: None,
                                                                                                                                            children: children,
                                                                                                                                        }
                                                                                                                                    }),
                                                                                                                                });
                                                                                                                        }
                                                                                                                        if block.kind == "quote"  {
                                                                                                                            children
                                                                                                                                .push({
                                                                                                                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                                                                    children
                                                                                                                                        .push({
                                                                                                                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                                                                            if block.rich {
                                                                                                                                                children
                                                                                                                                                    .push({
                                                                                                                                                        let rich_line_scope_1083 = format!(
                                                                                                                                                            "{}/RichLine@1083", for_scope
                                                                                                                                                        );
                                                                                                                                                        {
                                                                                                                                                            let mut rich_spans: Vec<
                                                                                                                                                                ::ducktape_view_guest::wire::RichSpan,
                                                                                                                                                            > = Vec::new();
                                                                                                                                                            for span in block.spans.iter().cloned() {
                                                                                                                                                                rich_spans
                                                                                                                                                                    .push(::ducktape_view_guest::wire::RichSpan {
                                                                                                                                                                        content: (span.mention.to_owned()).to_string(),
                                                                                                                                                                        size: None,
                                                                                                                                                                        line_height: None,
                                                                                                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                                                "Geist".into(),
                                                                                                                                                                            ),
                                                                                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                                                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                                                                        }),
                                                                                                                                                                        color: Some(palette.colors[16]),
                                                                                                                                                                        link: Some(span.mention_link.to_owned()),
                                                                                                                                                                        background: Some(palette.colors[18]),
                                                                                                                                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                                                                                                                                            color: None,
                                                                                                                                                                            width: None,
                                                                                                                                                                            radius: Some([
                                                                                                                                                                                ((4.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                ((4.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                ((4.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                                ((4.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                            ]),
                                                                                                                                                                        }),
                                                                                                                                                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                                                                                                                                                            top: (0.0) as f32,
                                                                                                                                                                            right: (1.0) as f32,
                                                                                                                                                                            bottom: (0.0) as f32,
                                                                                                                                                                            left: (1.0) as f32,
                                                                                                                                                                        }),
                                                                                                                                                                        underline: false,
                                                                                                                                                                        strikethrough: false,
                                                                                                                                                                    });
                                                                                                                                                                rich_spans
                                                                                                                                                                    .push(::ducktape_view_guest::wire::RichSpan {
                                                                                                                                                                        content: (span.link_text.to_owned()).to_string(),
                                                                                                                                                                        size: None,
                                                                                                                                                                        line_height: None,
                                                                                                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                                                "Geist".into(),
                                                                                                                                                                            ),
                                                                                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                                                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                                                                        }),
                                                                                                                                                                        color: Some(palette.colors[16]),
                                                                                                                                                                        link: Some(span.link.to_owned()),
                                                                                                                                                                        background: None,
                                                                                                                                                                        border: None,
                                                                                                                                                                        padding: None,
                                                                                                                                                                        underline: true,
                                                                                                                                                                        strikethrough: false,
                                                                                                                                                                    });
                                                                                                                                                                rich_spans
                                                                                                                                                                    .push(::ducktape_view_guest::wire::RichSpan {
                                                                                                                                                                        content: (span.bold_italic.to_owned()).to_string(),
                                                                                                                                                                        size: None,
                                                                                                                                                                        line_height: None,
                                                                                                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                                                "Geist".into(),
                                                                                                                                                                            ),
                                                                                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Bold,
                                                                                                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Italic,
                                                                                                                                                                        }),
                                                                                                                                                                        color: None,
                                                                                                                                                                        link: None,
                                                                                                                                                                        background: None,
                                                                                                                                                                        border: None,
                                                                                                                                                                        padding: None,
                                                                                                                                                                        underline: false,
                                                                                                                                                                        strikethrough: false,
                                                                                                                                                                    });
                                                                                                                                                                rich_spans
                                                                                                                                                                    .push(::ducktape_view_guest::wire::RichSpan {
                                                                                                                                                                        content: (span.bold.to_owned()).to_string(),
                                                                                                                                                                        size: None,
                                                                                                                                                                        line_height: None,
                                                                                                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                                                "Geist".into(),
                                                                                                                                                                            ),
                                                                                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Bold,
                                                                                                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                                                                        }),
                                                                                                                                                                        color: None,
                                                                                                                                                                        link: None,
                                                                                                                                                                        background: None,
                                                                                                                                                                        border: None,
                                                                                                                                                                        padding: None,
                                                                                                                                                                        underline: false,
                                                                                                                                                                        strikethrough: false,
                                                                                                                                                                    });
                                                                                                                                                                rich_spans
                                                                                                                                                                    .push(::ducktape_view_guest::wire::RichSpan {
                                                                                                                                                                        content: (span.italic.to_owned()).to_string(),
                                                                                                                                                                        size: None,
                                                                                                                                                                        line_height: None,
                                                                                                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                                                "Geist".into(),
                                                                                                                                                                            ),
                                                                                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Italic,
                                                                                                                                                                        }),
                                                                                                                                                                        color: None,
                                                                                                                                                                        link: None,
                                                                                                                                                                        background: None,
                                                                                                                                                                        border: None,
                                                                                                                                                                        padding: None,
                                                                                                                                                                        underline: false,
                                                                                                                                                                        strikethrough: false,
                                                                                                                                                                    });
                                                                                                                                                                rich_spans
                                                                                                                                                                    .push(::ducktape_view_guest::wire::RichSpan {
                                                                                                                                                                        content: (span.plain.to_owned()).to_string(),
                                                                                                                                                                        size: None,
                                                                                                                                                                        line_height: None,
                                                                                                                                                                        font: None,
                                                                                                                                                                        color: None,
                                                                                                                                                                        link: None,
                                                                                                                                                                        background: None,
                                                                                                                                                                        border: None,
                                                                                                                                                                        padding: None,
                                                                                                                                                                        underline: false,
                                                                                                                                                                        strikethrough: false,
                                                                                                                                                                    });
                                                                                                                                                            }
                                                                                                                                                            ::ducktape_view_guest::wire::Node::RichText {
                                                                                                                                                                options: ::ducktape_view_guest::wire::TextOptions {
                                                                                                                                                                    height: None,
                                                                                                                                                                    align_y: None,
                                                                                                                                                                    line_height: Some(
                                                                                                                                                                        ::ducktape_view_guest::wire::LineHeight::Relative(
                                                                                                                                                                            ((1.55) as f32).max(f32::EPSILON).min(f32::MAX),
                                                                                                                                                                        ),
                                                                                                                                                                    ),
                                                                                                                                                                    shaping: None,
                                                                                                                                                                    wrapping: Some(
                                                                                                                                                                        ::ducktape_view_guest::wire::Wrapping::WordOrGlyph,
                                                                                                                                                                    ),
                                                                                                                                                                    tracking: 0.0f32,
                                                                                                                                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                                            "Geist".into(),
                                                                                                                                                                        ),
                                                                                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                                                                    }),
                                                                                                                                                                },
                                                                                                                                                                key: format!("{}/@text:527", rich_line_scope_1083),
                                                                                                                                                                size: Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                                                                color: Some(palette.colors[15]),
                                                                                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                                                                                    monospace: false,
                                                                                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                                                                },
                                                                                                                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                                                                                                align_x: None,
                                                                                                                                                                spans: rich_spans,
                                                                                                                                                                on_link: Some(
                                                                                                                                                                    ::ducktape_view_guest::slots::handler::<
                                                                                                                                                                        String,
                                                                                                                                                                        Message,
                                                                                                                                                                    >(
                                                                                                                                                                        Box::new({
                                                                                                                                                                            let route = {
                                                                                                                                                                                let route_callback = (lazy_event_660_15).clone();
                                                                                                                                                                                move |link: String| (route_callback)(link)
                                                                                                                                                                            };
                                                                                                                                                                            move |sent: String| Some(route(sent))
                                                                                                                                                                        }),
                                                                                                                                                                    ),
                                                                                                                                                                ),
                                                                                                                                                            }
                                                                                                                                                        }
                                                                                                                                                    });
                                                                                                                                            }
                                                                                                                                            if !block.rich  {
                                                                                                                                                children
                                                                                                                                                    .push(::ducktape_view_guest::wire::Node::Text {
                                                                                                                                                        options: ::ducktape_view_guest::wire::TextOptions {
                                                                                                                                                            height: None,
                                                                                                                                                            align_y: None,
                                                                                                                                                            line_height: Some(
                                                                                                                                                                ::ducktape_view_guest::wire::LineHeight::Relative(
                                                                                                                                                                    ((1.55) as f32).max(f32::EPSILON).min(f32::MAX),
                                                                                                                                                                ),
                                                                                                                                                            ),
                                                                                                                                                            shaping: None,
                                                                                                                                                            wrapping: Some(
                                                                                                                                                                ::ducktape_view_guest::wire::Wrapping::WordOrGlyph,
                                                                                                                                                            ),
                                                                                                                                                            tracking: 0.0f32,
                                                                                                                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                                    "Geist".into(),
                                                                                                                                                                ),
                                                                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                                                            }),
                                                                                                                                                        },
                                                                                                                                                        key: format!("{}/@text:466", for_scope),
                                                                                                                                                        size: Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                                                        color: Some(palette.colors[5]),
                                                                                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                                                                                            monospace: false,
                                                                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                                                        },
                                                                                                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                                                                                        align_x: None,
                                                                                                                                                        content: (block.text.to_owned()).to_string(),
                                                                                                                                                    });
                                                                                                                                            }
                                                                                                                                            ::ducktape_view_guest::wire::Node::Linear {
                                                                                                                                                max_width: None,
                                                                                                                                                clip: false,
                                                                                                                                                key: format!("{}/@layout:450", for_scope),
                                                                                                                                                wrap: None,
                                                                                                                                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                                                                                                spacing: None,
                                                                                                                                                padding: Some(::ducktape_view_guest::wire::Edges {
                                                                                                                                                    top: (2.0) as f32,
                                                                                                                                                    right: (0.0) as f32,
                                                                                                                                                    bottom: (2.0) as f32,
                                                                                                                                                    left: (13.0) as f32,
                                                                                                                                                }),
                                                                                                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                                                                                height: None,
                                                                                                                                                align: None,
                                                                                                                                                background: None,
                                                                                                                                                border: None,
                                                                                                                                                children: children,
                                                                                                                                            }
                                                                                                                                        });
                                                                                                                                    children
                                                                                                                                        .push(::ducktape_view_guest::wire::Node::Container {
                                                                                                                                            shadow: ::ducktape_view_guest::wire::Shadow {
                                                                                                                                                color: None,
                                                                                                                                                x: None,
                                                                                                                                                y: None,
                                                                                                                                                blur: None,
                                                                                                                                            },
                                                                                                                                            max_width: None,
                                                                                                                                            max_height: None,
                                                                                                                                            clip: false,
                                                                                                                                            key: format!("{}/@container:473", for_scope),
                                                                                                                                            width: Some(
                                                                                                                                                ::ducktape_view_guest::wire::Length::Fixed((3.0) as f32),
                                                                                                                                            ),
                                                                                                                                            height: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                                                                            padding: None,
                                                                                                                                            align_x: None,
                                                                                                                                            align_y: None,
                                                                                                                                            background: (Some(palette.colors[19]))
                                                                                                                                                .map(::ducktape_view_guest::wire::Background::Color),
                                                                                                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                                                                                                color: None,
                                                                                                                                                width: None,
                                                                                                                                                radius: Some([
                                                                                                                                                    ((1.5) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                    ((1.5) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                    ((1.5) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                    ((1.5) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                ]),
                                                                                                                                            }),
                                                                                                                                            snap: None,
                                                                                                                                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                                                                                                                                width: Some(
                                                                                                                                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                                                                                                                                ),
                                                                                                                                                height: Some(
                                                                                                                                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                                                                                                                                ),
                                                                                                                                            }),
                                                                                                                                        });
                                                                                                                                    ::ducktape_view_guest::wire::Node::Stack {
                                                                                                                                        key: format!("{}/@layout:449", for_scope),
                                                                                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                                                                        height: None,
                                                                                                                                        padding: None,
                                                                                                                                        background: None,
                                                                                                                                        border: None,
                                                                                                                                        clip: false,
                                                                                                                                        under: 0u32,
                                                                                                                                        children: children,
                                                                                                                                    }
                                                                                                                                });
                                                                                                                        }
                                                                                                                        if block.kind == "paragraph"  {
                                                                                                                            if block.rich {
                                                                                                                                children
                                                                                                                                    .push({
                                                                                                                                        let rich_line_scope_1108 = format!(
                                                                                                                                            "{}/RichLine@1108", for_scope
                                                                                                                                        );
                                                                                                                                        {
                                                                                                                                            let mut rich_spans: Vec<
                                                                                                                                                ::ducktape_view_guest::wire::RichSpan,
                                                                                                                                            > = Vec::new();
                                                                                                                                            for span in block.spans.iter().cloned() {
                                                                                                                                                rich_spans
                                                                                                                                                    .push(::ducktape_view_guest::wire::RichSpan {
                                                                                                                                                        content: (span.mention.to_owned()).to_string(),
                                                                                                                                                        size: None,
                                                                                                                                                        line_height: None,
                                                                                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                                "Geist".into(),
                                                                                                                                                            ),
                                                                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                                                        }),
                                                                                                                                                        color: Some(palette.colors[16]),
                                                                                                                                                        link: Some(span.mention_link.to_owned()),
                                                                                                                                                        background: Some(palette.colors[18]),
                                                                                                                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                                                                                                                            color: None,
                                                                                                                                                            width: None,
                                                                                                                                                            radius: Some([
                                                                                                                                                                ((4.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                ((4.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                ((4.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                                ((4.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                                            ]),
                                                                                                                                                        }),
                                                                                                                                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                                                                                                                                            top: (0.0) as f32,
                                                                                                                                                            right: (1.0) as f32,
                                                                                                                                                            bottom: (0.0) as f32,
                                                                                                                                                            left: (1.0) as f32,
                                                                                                                                                        }),
                                                                                                                                                        underline: false,
                                                                                                                                                        strikethrough: false,
                                                                                                                                                    });
                                                                                                                                                rich_spans
                                                                                                                                                    .push(::ducktape_view_guest::wire::RichSpan {
                                                                                                                                                        content: (span.link_text.to_owned()).to_string(),
                                                                                                                                                        size: None,
                                                                                                                                                        line_height: None,
                                                                                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                                "Geist".into(),
                                                                                                                                                            ),
                                                                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                                                        }),
                                                                                                                                                        color: Some(palette.colors[16]),
                                                                                                                                                        link: Some(span.link.to_owned()),
                                                                                                                                                        background: None,
                                                                                                                                                        border: None,
                                                                                                                                                        padding: None,
                                                                                                                                                        underline: true,
                                                                                                                                                        strikethrough: false,
                                                                                                                                                    });
                                                                                                                                                rich_spans
                                                                                                                                                    .push(::ducktape_view_guest::wire::RichSpan {
                                                                                                                                                        content: (span.bold_italic.to_owned()).to_string(),
                                                                                                                                                        size: None,
                                                                                                                                                        line_height: None,
                                                                                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                                "Geist".into(),
                                                                                                                                                            ),
                                                                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Bold,
                                                                                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Italic,
                                                                                                                                                        }),
                                                                                                                                                        color: None,
                                                                                                                                                        link: None,
                                                                                                                                                        background: None,
                                                                                                                                                        border: None,
                                                                                                                                                        padding: None,
                                                                                                                                                        underline: false,
                                                                                                                                                        strikethrough: false,
                                                                                                                                                    });
                                                                                                                                                rich_spans
                                                                                                                                                    .push(::ducktape_view_guest::wire::RichSpan {
                                                                                                                                                        content: (span.bold.to_owned()).to_string(),
                                                                                                                                                        size: None,
                                                                                                                                                        line_height: None,
                                                                                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                                "Geist".into(),
                                                                                                                                                            ),
                                                                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Bold,
                                                                                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                                                        }),
                                                                                                                                                        color: None,
                                                                                                                                                        link: None,
                                                                                                                                                        background: None,
                                                                                                                                                        border: None,
                                                                                                                                                        padding: None,
                                                                                                                                                        underline: false,
                                                                                                                                                        strikethrough: false,
                                                                                                                                                    });
                                                                                                                                                rich_spans
                                                                                                                                                    .push(::ducktape_view_guest::wire::RichSpan {
                                                                                                                                                        content: (span.italic.to_owned()).to_string(),
                                                                                                                                                        size: None,
                                                                                                                                                        line_height: None,
                                                                                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                                "Geist".into(),
                                                                                                                                                            ),
                                                                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Italic,
                                                                                                                                                        }),
                                                                                                                                                        color: None,
                                                                                                                                                        link: None,
                                                                                                                                                        background: None,
                                                                                                                                                        border: None,
                                                                                                                                                        padding: None,
                                                                                                                                                        underline: false,
                                                                                                                                                        strikethrough: false,
                                                                                                                                                    });
                                                                                                                                                rich_spans
                                                                                                                                                    .push(::ducktape_view_guest::wire::RichSpan {
                                                                                                                                                        content: (span.plain.to_owned()).to_string(),
                                                                                                                                                        size: None,
                                                                                                                                                        line_height: None,
                                                                                                                                                        font: None,
                                                                                                                                                        color: None,
                                                                                                                                                        link: None,
                                                                                                                                                        background: None,
                                                                                                                                                        border: None,
                                                                                                                                                        padding: None,
                                                                                                                                                        underline: false,
                                                                                                                                                        strikethrough: false,
                                                                                                                                                    });
                                                                                                                                            }
                                                                                                                                            ::ducktape_view_guest::wire::Node::RichText {
                                                                                                                                                options: ::ducktape_view_guest::wire::TextOptions {
                                                                                                                                                    height: None,
                                                                                                                                                    align_y: None,
                                                                                                                                                    line_height: Some(
                                                                                                                                                        ::ducktape_view_guest::wire::LineHeight::Relative(
                                                                                                                                                            ((1.55) as f32).max(f32::EPSILON).min(f32::MAX),
                                                                                                                                                        ),
                                                                                                                                                    ),
                                                                                                                                                    shaping: None,
                                                                                                                                                    wrapping: Some(
                                                                                                                                                        ::ducktape_view_guest::wire::Wrapping::WordOrGlyph,
                                                                                                                                                    ),
                                                                                                                                                    tracking: 0.0f32,
                                                                                                                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                            "Geist".into(),
                                                                                                                                                        ),
                                                                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                                                    }),
                                                                                                                                                },
                                                                                                                                                key: format!("{}/@text:527", rich_line_scope_1108),
                                                                                                                                                size: Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                                                color: Some(palette.colors[15]),
                                                                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                                                                    monospace: false,
                                                                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                                                },
                                                                                                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                                                                                align_x: None,
                                                                                                                                                spans: rich_spans,
                                                                                                                                                on_link: Some(
                                                                                                                                                    ::ducktape_view_guest::slots::handler::<
                                                                                                                                                        String,
                                                                                                                                                        Message,
                                                                                                                                                    >(
                                                                                                                                                        Box::new({
                                                                                                                                                            let route = {
                                                                                                                                                                let route_callback = (lazy_event_660_15).clone();
                                                                                                                                                                move |link: String| (route_callback)(link)
                                                                                                                                                            };
                                                                                                                                                            move |sent: String| Some(route(sent))
                                                                                                                                                        }),
                                                                                                                                                    ),
                                                                                                                                                ),
                                                                                                                                            }
                                                                                                                                        }
                                                                                                                                    });
                                                                                                                            }
                                                                                                                            if !block.rich  {
                                                                                                                                children
                                                                                                                                    .push(::ducktape_view_guest::wire::Node::Text {
                                                                                                                                        options: ::ducktape_view_guest::wire::TextOptions {
                                                                                                                                            height: None,
                                                                                                                                            align_y: None,
                                                                                                                                            line_height: Some(
                                                                                                                                                ::ducktape_view_guest::wire::LineHeight::Relative(
                                                                                                                                                    ((1.55) as f32).max(f32::EPSILON).min(f32::MAX),
                                                                                                                                                ),
                                                                                                                                            ),
                                                                                                                                            shaping: None,
                                                                                                                                            wrapping: Some(
                                                                                                                                                ::ducktape_view_guest::wire::Wrapping::WordOrGlyph,
                                                                                                                                            ),
                                                                                                                                            tracking: 0.0f32,
                                                                                                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                                                                    "Geist".into(),
                                                                                                                                                ),
                                                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                                                            }),
                                                                                                                                        },
                                                                                                                                        key: format!("{}/@text:486", for_scope),
                                                                                                                                        size: Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                                        color: Some(palette.colors[15]),
                                                                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                                                                            monospace: false,
                                                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                                        },
                                                                                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                                                                        align_x: None,
                                                                                                                                        content: (block.text.to_owned()).to_string(),
                                                                                                                                    });
                                                                                                                            }
                                                                                                                        }
                                                                                                                    }
                                                                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                                                                        max_width: None,
                                                                                                                        clip: false,
                                                                                                                        key: format!("{}/@layout:404", rich_body_scope_1146),
                                                                                                                        wrap: None,
                                                                                                                        axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                                                                        spacing: Some((5.0) as f32),
                                                                                                                        padding: None,
                                                                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                                                        height: None,
                                                                                                                        align: None,
                                                                                                                        background: None,
                                                                                                                        border: None,
                                                                                                                        children: children,
                                                                                                                    }
                                                                                                                }
                                                                                                            });
                                                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                                                            max_width: Some(((760.0) as f32).max(0.0).min(f32::MAX)),
                                                                                                            clip: false,
                                                                                                            key: format!("{}/@layout:519", message_body_scope_3493),
                                                                                                            wrap: None,
                                                                                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                                                            spacing: None,
                                                                                                            padding: None,
                                                                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                                            height: None,
                                                                                                            align: None,
                                                                                                            background: None,
                                                                                                            border: None,
                                                                                                            children: children,
                                                                                                        }
                                                                                                    }
                                                                                                });
                                                                                            ::ducktape_view_guest::wire::Node::Linear {
                                                                                                max_width: None,
                                                                                                clip: false,
                                                                                                key: format!("{}/@layout:731", lazy_scope),
                                                                                                wrap: None,
                                                                                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                                                spacing: Some((2.0) as f32),
                                                                                                padding: None,
                                                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                                height: None,
                                                                                                align: None,
                                                                                                background: None,
                                                                                                border: None,
                                                                                                children: children,
                                                                                            }
                                                                                        });
                                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                                        max_width: None,
                                                                                        clip: false,
                                                                                        key: format!("{}/@layout:725", lazy_scope),
                                                                                        wrap: None,
                                                                                        axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                                        spacing: Some((9.0) as f32),
                                                                                        padding: None,
                                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                        height: None,
                                                                                        align: Some(::ducktape_view_guest::wire::AlignX::Left),
                                                                                        background: None,
                                                                                        border: None,
                                                                                        children: children,
                                                                                    }
                                                                                }
                                                                            },
                                                                            660u64,
                                                                            &(key_recon),
                                                                            lazy_key,
                                                                        )
                                                                    }
                                                                };
                                                                children.push((key, child));
                                                            }
                                                            let (keys, children) = children
                                                                .into_iter()
                                                                .map(|(key, child)| (
                                                                    ::ducktape_view_guest::wire::ListKey::from(key),
                                                                    child,
                                                                ))
                                                                .unzip();
                                                            ::ducktape_view_guest::wire::Node::KeyedColumn {
                                                                key: format!("{}/@keyed:713", use_scope),
                                                                keys: Some(keys),
                                                                children,
                                                                background: None,
                                                                border: None,
                                                                spacing: Some((9.0) as f32),
                                                                padding: None,
                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                height: None,
                                                                max_width: None,
                                                                align: None,
                                                                virtual_row: Some((44.0) as f32),
                                                            }
                                                        });
                                                    children
                                                        .push({
                                                            let node_scope = format!("{}/note", node_scope);
                                                            ::ducktape_view_guest::wire::Node::Surface {
                                                                key: node_scope.clone(),
                                                                name: String::from("forge_composer"),
                                                                args: ::std::vec![
                                                                    { let surface_arg = & (crate
                                                                    ::host::composer_scope(::std::convert::AsRef::as_ref(& (self
                                                                    .connected_rpc)), ::std::convert::AsRef::as_ref(& (self
                                                                    .forge_item_channel))).to_owned());
                                                                    ::ducktape_view_guest::wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                                                                    }, { let surface_arg = & ("note".to_owned());
                                                                    ::ducktape_view_guest::wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                                                                    }, { let surface_arg = & (true);
                                                                    ::ducktape_view_guest::wire::SurfaceValue::Bool(*
                                                                    (surface_arg)) }, { let surface_arg = & ("Write a note…"
                                                                    .to_owned());
                                                                    ::ducktape_view_guest::wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                                                                    }, { let surface_arg = & ((((! self.connected) || (self
                                                                    .forge_item_channel).is_empty()) || (self.item_phase !=
                                                                    "ready")));
                                                                    ::ducktape_view_guest::wire::SurfaceValue::Bool(*
                                                                    (surface_arg)) }, { let surface_arg = & (false);
                                                                    ::ducktape_view_guest::wire::SurfaceValue::Bool(*
                                                                    (surface_arg)) }, { let surface_arg = &
                                                                    ("The note wasn’t sent".to_owned());
                                                                    ::ducktape_view_guest::wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                                                                    }
                                                                ],
                                                                on_event: None,
                                                            }
                                                        });
                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                        max_width: None,
                                                        clip: false,
                                                        key: format!("{}/@layout:698", use_scope),
                                                        wrap: None,
                                                        axis: ::ducktape_view_guest::wire::Axis::Column,
                                                        spacing: Some((9.0) as f32),
                                                        padding: None,
                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                        height: None,
                                                        align: None,
                                                        background: None,
                                                        border: None,
                                                        children: children,
                                                    }
                                                });
                                            ::ducktape_view_guest::wire::Node::Linear {
                                                max_width: None,
                                                clip: false,
                                                key: format!("{}/@layout:360", use_scope),
                                                wrap: None,
                                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                                spacing: Some((12.0) as f32),
                                                padding: Some(::ducktape_view_guest::wire::Edges {
                                                    top: (14.0) as f32,
                                                    right: (18.0) as f32,
                                                    bottom: (18.0) as f32,
                                                    left: (18.0) as f32,
                                                }),
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                height: None,
                                                align: None,
                                                background: None,
                                                border: None,
                                                children: children,
                                            }
                                        }),
                                    }
                                });
                        }
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:129", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Column,
                            spacing: None,
                            padding: None,
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(::ducktape_view_guest::wire::Length::Fill),
                            align: None,
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
            }
            ::ducktape_view_guest::wire::Node::Linear {
                max_width: None,
                clip: false,
                key: format!("{}/@layout:36", use_scope),
                wrap: None,
                axis: ::ducktape_view_guest::wire::Axis::Column,
                spacing: None,
                padding: None,
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: Some(::ducktape_view_guest::wire::Length::Fill),
                align: None,
                background: None,
                border: None,
                children: children,
            }
        }
    }
}
