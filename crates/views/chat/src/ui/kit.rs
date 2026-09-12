use super::*;
impl super::ChatView {
    pub(super) fn render_agent_square_2(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
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
                width: Some(::ducktape_view_guest::wire::Length::Fixed((18.0) as f32)),
                height: Some(::ducktape_view_guest::wire::Length::Fixed((18.0) as f32)),
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
                    key: format!("{}/@text:390", use_scope),
                    size: Some(((8.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[38]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: (arg_0.to_owned()).to_string(),
                }),
            }
        }
    }
    pub(super) fn render_agent_square_3(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
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
                width: Some(::ducktape_view_guest::wire::Length::Fixed((18.0) as f32)),
                height: Some(::ducktape_view_guest::wire::Length::Fixed((18.0) as f32)),
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
                    key: format!("{}/@text:390", use_scope),
                    size: Some(((8.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[38]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: (arg_0.to_owned()).to_string(),
                }),
            }
        }
    }
    pub(super) fn render_agent_square_4(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
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
                width: Some(::ducktape_view_guest::wire::Length::Fixed((18.0) as f32)),
                height: Some(::ducktape_view_guest::wire::Length::Fixed((18.0) as f32)),
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
                    key: format!("{}/@text:390", use_scope),
                    size: Some(((8.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[38]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: (arg_0.to_owned()).to_string(),
                }),
            }
        }
    }
    pub(super) fn render_agent_square_5(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
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
                width: Some(::ducktape_view_guest::wire::Length::Fixed((18.0) as f32)),
                height: Some(::ducktape_view_guest::wire::Length::Fixed((18.0) as f32)),
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
                    key: format!("{}/@text:390", use_scope),
                    size: Some(((8.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[38]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: (arg_0.to_owned()).to_string(),
                }),
            }
        }
    }
    pub(super) fn render_agent_square_6(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
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
                width: Some(::ducktape_view_guest::wire::Length::Fixed((18.0) as f32)),
                height: Some(::ducktape_view_guest::wire::Length::Fixed((18.0) as f32)),
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
                    key: format!("{}/@text:390", use_scope),
                    size: Some(((8.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[38]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: (arg_0.to_owned()).to_string(),
                }),
            }
        }
    }
    pub(super) fn render_agent_plate_7(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if false {
                    children.push(self.render_agent_square_2(
                        palette,
                        format!("{}/AgentSquare@1160", use_scope),
                        arg_0.to_owned(),
                    ));
                }
                if (false) && (true) {
                    children.push(self.render_agent_square_3(
                        palette,
                        format!("{}/AgentSquare@1167", use_scope),
                        arg_0.to_owned(),
                    ));
                }
                if (false) && (true) {
                    children.push(self.render_agent_square_4(
                        palette,
                        format!("{}/AgentSquare@1174", use_scope),
                        arg_0.to_owned(),
                    ));
                }
                if (false) && (true) {
                    children.push(self.render_agent_square_5(
                        palette,
                        format!("{}/AgentSquare@1181", use_scope),
                        arg_0.to_owned(),
                    ));
                }
                if true {
                    children.push(self.render_agent_square_6(
                        palette,
                        format!("{}/AgentSquare@1188", use_scope),
                        arg_0.to_owned(),
                    ));
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
    }
    pub(super) fn render_human_plate_8(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if true {
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
                        key: format!("{}/@container:313", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fixed((18.0) as f32)),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((18.0) as f32)),
                        padding: None,
                        align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                        align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                        background: (Some(palette.colors[99]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: Some(::ducktape_view_guest::wire::Border {
                            color: None,
                            width: None,
                            radius: Some([
                                ((18.0 / 2.0) as f32).max(0.0).min(f32::MAX),
                                ((18.0 / 2.0) as f32).max(0.0).min(f32::MAX),
                                ((18.0 / 2.0) as f32).max(0.0).min(f32::MAX),
                                ((18.0 / 2.0) as f32).max(0.0).min(f32::MAX),
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
                            key: format!("{}/@text:321", use_scope),
                            size: Some(((8.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[100]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: (arg_0.to_owned()).to_string(),
                        }),
                    });
                }
                if false {
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
                        key: format!("{}/@container:328", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fixed((18.0) as f32)),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((18.0) as f32)),
                        padding: None,
                        align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                        align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                        background: (Some(palette.colors[35]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: Some(::ducktape_view_guest::wire::Border {
                            color: None,
                            width: None,
                            radius: Some([
                                ((18.0 / 2.0) as f32).max(0.0).min(f32::MAX),
                                ((18.0 / 2.0) as f32).max(0.0).min(f32::MAX),
                                ((18.0 / 2.0) as f32).max(0.0).min(f32::MAX),
                                ((18.0 / 2.0) as f32).max(0.0).min(f32::MAX),
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
                            key: format!("{}/@text:336", use_scope),
                            size: Some(((8.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[5]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: (arg_0.to_owned()).to_string(),
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
    }
    pub(super) fn render_principal_plate_9(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
        arg_1: bool,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if arg_1 {
                    children.push(self.render_agent_plate_7(
                        palette,
                        format!("{}/AgentPlate@919", use_scope),
                        arg_0.to_owned(),
                    ));
                }
                if !arg_1 {
                    children.push(self.render_human_plate_8(
                        palette,
                        format!("{}/HumanPlate@925", use_scope),
                        arg_0.to_owned(),
                    ));
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
    }
    pub(super) fn render_principal_avatar_10(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
        arg_1: bool,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                {
                    children.push(self.render_principal_plate_9(
                        palette,
                        format!("{}/PrincipalPlate@909", use_scope),
                        arg_0.to_owned(),
                        arg_1,
                    ));
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
    }
    pub(super) fn render_agent_square_14(
        &self,
        palette: Palette,
        use_scope: String,
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
                width: Some(::ducktape_view_guest::wire::Length::Fixed((24.0) as f32)),
                height: Some(::ducktape_view_guest::wire::Length::Fixed((24.0) as f32)),
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
                    key: format!("{}/@text:390", use_scope),
                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[38]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: (self.active_dm.initials.to_owned()).to_string(),
                }),
            }
        }
    }
    pub(super) fn render_agent_square_15(
        &self,
        palette: Palette,
        use_scope: String,
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
                width: Some(::ducktape_view_guest::wire::Length::Fixed((24.0) as f32)),
                height: Some(::ducktape_view_guest::wire::Length::Fixed((24.0) as f32)),
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
                    key: format!("{}/@text:390", use_scope),
                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[38]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: (self.active_dm.initials.to_owned()).to_string(),
                }),
            }
        }
    }
    pub(super) fn render_agent_square_16(
        &self,
        palette: Palette,
        use_scope: String,
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
                width: Some(::ducktape_view_guest::wire::Length::Fixed((24.0) as f32)),
                height: Some(::ducktape_view_guest::wire::Length::Fixed((24.0) as f32)),
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
                    key: format!("{}/@text:390", use_scope),
                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[38]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: (self.active_dm.initials.to_owned()).to_string(),
                }),
            }
        }
    }
    pub(super) fn render_agent_square_17(
        &self,
        palette: Palette,
        use_scope: String,
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
                width: Some(::ducktape_view_guest::wire::Length::Fixed((24.0) as f32)),
                height: Some(::ducktape_view_guest::wire::Length::Fixed((24.0) as f32)),
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
                    key: format!("{}/@text:390", use_scope),
                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[38]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: (self.active_dm.initials.to_owned()).to_string(),
                }),
            }
        }
    }
    pub(super) fn render_agent_square_18(
        &self,
        palette: Palette,
        use_scope: String,
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
                width: Some(::ducktape_view_guest::wire::Length::Fixed((24.0) as f32)),
                height: Some(::ducktape_view_guest::wire::Length::Fixed((24.0) as f32)),
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
                    key: format!("{}/@text:390", use_scope),
                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[38]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: (self.active_dm.initials.to_owned()).to_string(),
                }),
            }
        }
    }
    pub(super) fn render_agent_plate_19(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if false {
                    children.push(self.render_agent_square_14(
                        palette,
                        format!("{}/AgentSquare@1160", use_scope),
                    ));
                }
                if (false) && (true) {
                    children.push(self.render_agent_square_15(
                        palette,
                        format!("{}/AgentSquare@1167", use_scope),
                    ));
                }
                if (false) && (true) {
                    children.push(self.render_agent_square_16(
                        palette,
                        format!("{}/AgentSquare@1174", use_scope),
                    ));
                }
                if true {
                    children.push(self.render_agent_square_17(
                        palette,
                        format!("{}/AgentSquare@1181", use_scope),
                    ));
                }
                if false {
                    children.push(self.render_agent_square_18(
                        palette,
                        format!("{}/AgentSquare@1188", use_scope),
                    ));
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
    }
    pub(super) fn render_human_plate_20(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if false {
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
                        key: format!("{}/@container:313", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fixed((24.0) as f32)),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((24.0) as f32)),
                        padding: None,
                        align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                        align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                        background: (Some(palette.colors[99]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: Some(::ducktape_view_guest::wire::Border {
                            color: None,
                            width: None,
                            radius: Some([
                                ((24.0 / 2.0) as f32).max(0.0).min(f32::MAX),
                                ((24.0 / 2.0) as f32).max(0.0).min(f32::MAX),
                                ((24.0 / 2.0) as f32).max(0.0).min(f32::MAX),
                                ((24.0 / 2.0) as f32).max(0.0).min(f32::MAX),
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
                            key: format!("{}/@text:321", use_scope),
                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[100]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: (self.active_dm.initials.to_owned()).to_string(),
                        }),
                    });
                }
                if true {
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
                        key: format!("{}/@container:328", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fixed((24.0) as f32)),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((24.0) as f32)),
                        padding: None,
                        align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                        align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                        background: (Some(palette.colors[35]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: Some(::ducktape_view_guest::wire::Border {
                            color: None,
                            width: None,
                            radius: Some([
                                ((24.0 / 2.0) as f32).max(0.0).min(f32::MAX),
                                ((24.0 / 2.0) as f32).max(0.0).min(f32::MAX),
                                ((24.0 / 2.0) as f32).max(0.0).min(f32::MAX),
                                ((24.0 / 2.0) as f32).max(0.0).min(f32::MAX),
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
                            key: format!("{}/@text:336", use_scope),
                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[5]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: (self.active_dm.initials.to_owned()).to_string(),
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
    }
    pub(super) fn render_principal_plate_21(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if self.active_dm.is_agent {
                    children.push(
                        self.render_agent_plate_19(
                            palette,
                            format!("{}/AgentPlate@919", use_scope),
                        ),
                    );
                }
                if !self.active_dm.is_agent {
                    children.push(
                        self.render_human_plate_20(
                            palette,
                            format!("{}/HumanPlate@925", use_scope),
                        ),
                    );
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
    }
    pub(super) fn render_principal_avatar_22(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                {
                    children.push(self.render_principal_plate_21(
                        palette,
                        format!("{}/PrincipalPlate@909", use_scope),
                    ));
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
    }
    pub(super) fn render_badge_outline_24(
        &self,
        palette: Palette,
        use_scope: String,
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
                width: None,
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (3.0) as f32,
                    right: (7.0) as f32,
                    bottom: (3.0) as f32,
                    left: (7.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[3]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[40]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                    options: ::ducktape_view_guest::wire::TextOptions {
                        height: None,
                        align_y: None,
                        line_height: Some(::ducktape_view_guest::wire::LineHeight::Relative(
                            1.35f32,
                        )),
                        shaping: None,
                        wrapping: None,
                        tracking: 0.0f32,
                        font: Some(::ducktape_view_guest::wire::NamedFont {
                            family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                        }),
                    },
                    key: format!("{}/@text:15", use_scope),
                    size: Some(9.0f32),
                    color: Some(palette.colors[13]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                    },
                    width: None,
                    align_x: None,
                    content: ("Archived".to_owned()).to_string(),
                }),
            }
        }
    }
    pub(super) fn render_badge_outline_25(
        &self,
        palette: Palette,
        use_scope: String,
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
                width: None,
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (3.0) as f32,
                    right: (7.0) as f32,
                    bottom: (3.0) as f32,
                    left: (7.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[3]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[40]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                    options: ::ducktape_view_guest::wire::TextOptions {
                        height: None,
                        align_y: None,
                        line_height: Some(::ducktape_view_guest::wire::LineHeight::Relative(
                            1.35f32,
                        )),
                        shaping: None,
                        wrapping: None,
                        tracking: 0.0f32,
                        font: Some(::ducktape_view_guest::wire::NamedFont {
                            family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                        }),
                    },
                    key: format!("{}/@text:15", use_scope),
                    size: Some(9.0f32),
                    color: Some(palette.colors[13]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                    },
                    width: None,
                    align_x: None,
                    content: ("Members only".to_owned()).to_string(),
                }),
            }
        }
    }
    pub(super) fn render_pulse_dot_26(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                {
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
                        key: format!("{}/@container:183", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fixed((6.0) as f32)),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((6.0) as f32)),
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[29]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: Some(::ducktape_view_guest::wire::Border {
                            color: None,
                            width: None,
                            radius: Some([
                                ((6.0 / 2.0) as f32).max(0.0).min(f32::MAX),
                                ((6.0 / 2.0) as f32).max(0.0).min(f32::MAX),
                                ((6.0 / 2.0) as f32).max(0.0).min(f32::MAX),
                                ((6.0 / 2.0) as f32).max(0.0).min(f32::MAX),
                            ]),
                        }),
                        snap: None,
                        content: Box::new(::ducktape_view_guest::wire::Node::Space {
                            width: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                            height: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
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
    }
    pub(super) fn render_huddle_live_pill_29(
        &self,
        palette: Palette,
        use_scope: String,
        cb_23: impl Fn() -> Message + Clone + 'static,
        cb_42: impl Fn() -> Message + Clone + 'static,
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
                width: None,
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (5.0) as f32,
                    right: (10.0) as f32,
                    bottom: (5.0) as f32,
                    left: (9.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[37]))
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
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children.push(::ducktape_view_guest::wire::Node::Button {
                        checked: None,
                        expanded: None,
                        description: None,
                        key: format!("{}/@button:220", use_scope),
                        content: ::ducktape_view_guest::wire::ButtonContent::Child(Box::new({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children.push(self.render_pulse_dot_26(
                                palette,
                                format!("{}/PulseDot@1041", use_scope),
                            ));
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
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:228", use_scope),
                                size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[38]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("LIVE".to_owned()).to_string(),
                            });
                            if !(crate::host::mmss(self.huddle_now - self.huddle_joined_at))
                                .is_empty()
                            {
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
                                    key: format!("{}/@text:235", use_scope),
                                    size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[38]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: (crate::host::mmss(
                                        self.huddle_now - self.huddle_joined_at,
                                    )
                                    .to_owned())
                                    .to_string(),
                                });
                            }
                            if self.call_muted {
                                children.push(self.icon(
                                    format!("{}/Icon@1056", use_scope),
                                    "mic-off",
                                    11f32,
                                    (palette).colors[70usize],
                                    "@media:34",
                                ));
                            }
                            children.push(self.icon(
                                format!("{}/Icon@1061", use_scope),
                                "popout",
                                11f32,
                                (palette).colors[70usize],
                                "@media:34",
                            ));
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:226", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                spacing: Some((8.0) as f32),
                                padding: None,
                                width: None,
                                height: None,
                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                background: None,
                                border: None,
                                children: children,
                            }
                        })),
                        label: Some(String::from("Show the huddle window".to_owned())),
                        on_press: Some(::ducktape_view_guest::slots::message((cb_42)())),
                        width: None,
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: 0f32,
                            right: 0f32,
                            bottom: 0f32,
                            left: 0f32,
                        }),
                        style: ::ducktape_view_guest::wire::ButtonStyle {
                            preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                            recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                base: ::ducktape_view_guest::wire::Face {
                                    background: Some(::ducktape_view_guest::wire::Rgba([
                                        0.0 / 255.0,
                                        0.0 / 255.0,
                                        0.0 / 255.0,
                                        0.000000,
                                    ])),
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
                                background: Some(::ducktape_view_guest::wire::Rgba([
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.000000,
                                ])),
                                text: Some(palette.colors[38]),
                                border: Some(::ducktape_view_guest::wire::Border {
                                    color: Some(::ducktape_view_guest::wire::Rgba([
                                        0.0 / 255.0,
                                        0.0 / 255.0,
                                        0.0 / 255.0,
                                        0.000000,
                                    ])),
                                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                    radius: Some([
                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                    ]),
                                }),
                            },
                            hovered: Some(::ducktape_view_guest::wire::Face {
                                background: Some(palette.colors[68]),
                                text: Some(palette.colors[38]),
                                border: None,
                            }),
                            pressed: Some(::ducktape_view_guest::wire::Face {
                                background: Some(palette.colors[68]),
                                text: Some(palette.colors[38]),
                                border: None,
                            }),
                            disabled: None,
                        },
                    });
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
                        key: format!("{}/@container:255", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((14.0) as f32)),
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[115]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
                        snap: None,
                        content: Box::new(::ducktape_view_guest::wire::Node::Space {
                            width: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                            height: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                        }),
                    });
                    children.push(::ducktape_view_guest::wire::Node::Button {
                        checked: None,
                        expanded: None,
                        description: None,
                        key: format!("{}/@button:261", use_scope),
                        content: ::ducktape_view_guest::wire::ButtonContent::Child(Box::new(
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
                                key: format!("{}/@container:269", use_scope),
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: Some(::ducktape_view_guest::wire::Length::Fill),
                                padding: None,
                                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
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
                                            stretch:
                                                ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:275", use_scope),
                                    size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[116]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("✕".to_owned()).to_string(),
                                }),
                            },
                        )),
                        label: Some(String::from("Leave the huddle".to_owned())),
                        on_press: Some(::ducktape_view_guest::slots::message((cb_23)())),
                        width: Some(::ducktape_view_guest::wire::Length::Fixed((24.0) as f32)),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((24.0) as f32)),
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: 0f32,
                            right: 0f32,
                            bottom: 0f32,
                            left: 0f32,
                        }),
                        style: ::ducktape_view_guest::wire::ButtonStyle {
                            preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                            recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                base: ::ducktape_view_guest::wire::Face {
                                    background: Some(::ducktape_view_guest::wire::Rgba([
                                        0.0 / 255.0,
                                        0.0 / 255.0,
                                        0.0 / 255.0,
                                        0.000000,
                                    ])),
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
                                background: Some(::ducktape_view_guest::wire::Rgba([
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.000000,
                                ])),
                                text: Some(palette.colors[116]),
                                border: Some(::ducktape_view_guest::wire::Border {
                                    color: Some(::ducktape_view_guest::wire::Rgba([
                                        0.0 / 255.0,
                                        0.0 / 255.0,
                                        0.0 / 255.0,
                                        0.000000,
                                    ])),
                                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                    radius: Some([
                                        ((5.0) as f32).max(0.0).min(f32::MAX),
                                        ((5.0) as f32).max(0.0).min(f32::MAX),
                                        ((5.0) as f32).max(0.0).min(f32::MAX),
                                        ((5.0) as f32).max(0.0).min(f32::MAX),
                                    ]),
                                }),
                            },
                            hovered: Some(::ducktape_view_guest::wire::Face {
                                background: Some(palette.colors[69]),
                                text: Some(palette.colors[116]),
                                border: None,
                            }),
                            pressed: Some(::ducktape_view_guest::wire::Face {
                                background: Some(palette.colors[69]),
                                text: Some(palette.colors[116]),
                                border: None,
                            }),
                            disabled: None,
                        },
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:219", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((8.0) as f32),
                        padding: None,
                        width: None,
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(super) fn render_huddle_start_31(
        &self,
        palette: Palette,
        use_scope: String,
        cb_22: impl Fn() -> Message + Clone + 'static,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Button {
                checked: None,
                expanded: None,
                description: None,
                key: node_scope.clone(),
                content: ::ducktape_view_guest::wire::ButtonContent::Child(Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children.push(self.icon(
                        format!("{}/Icon@1109", use_scope),
                        "headphones",
                        14f32,
                        (palette).colors[5usize],
                        "@media:82",
                    ));
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
                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:300", use_scope),
                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[15]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("Huddle".to_owned()).to_string(),
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:294", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((7.0) as f32),
                        padding: None,
                        width: None,
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                        background: None,
                        border: None,
                        children: children,
                    }
                })),
                label: Some(String::from("Start a huddle".to_owned())),
                on_press: Some(::ducktape_view_guest::slots::message((cb_22)())),
                width: None,
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: 5f32,
                    right: 9f32,
                    bottom: 5f32,
                    left: 9f32,
                }),
                style: ::ducktape_view_guest::wire::ButtonStyle {
                    preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                    recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                        base: ::ducktape_view_guest::wire::Face {
                            background: Some(::ducktape_view_guest::wire::Rgba([
                                0.0 / 255.0,
                                0.0 / 255.0,
                                0.0 / 255.0,
                                0.000000,
                            ])),
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
                            family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                        }),
                    }),
                    active: ::ducktape_view_guest::wire::Face {
                        background: Some(palette.colors[3]),
                        text: Some(palette.colors[15]),
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
                    },
                    hovered: Some(::ducktape_view_guest::wire::Face {
                        background: Some(palette.colors[6]),
                        text: Some(palette.colors[15]),
                        border: Some(::ducktape_view_guest::wire::Border {
                            color: Some(palette.colors[94]),
                            width: None,
                            radius: None,
                        }),
                    }),
                    pressed: Some(::ducktape_view_guest::wire::Face {
                        background: Some(palette.colors[56]),
                        text: Some(palette.colors[15]),
                        border: None,
                    }),
                    disabled: None,
                },
            }
        }
    }
    pub(super) fn render_empty_state_33(
        &self,
        palette: Palette,
        use_scope: String,
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
                height: Some(::ducktape_view_guest::wire::Length::Fill),
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (22.0) as f32,
                    right: (22.0) as f32,
                    bottom: (22.0) as f32,
                    left: (22.0) as f32,
                }),
                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                background: (None).map(::ducktape_view_guest::wire::Background::Color),
                border: None,
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
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
                        key: format!("{}/@container:33", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fixed((42.0) as f32)),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((42.0) as f32)),
                        padding: None,
                        align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                        align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                        background: (Some(palette.colors[3]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: Some(::ducktape_view_guest::wire::Border {
                            color: Some(palette.colors[39]),
                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                            radius: Some([
                                ((21.0) as f32).max(0.0).min(f32::MAX),
                                ((21.0) as f32).max(0.0).min(f32::MAX),
                                ((21.0) as f32).max(0.0).min(f32::MAX),
                                ((21.0) as f32).max(0.0).min(f32::MAX),
                            ]),
                        }),
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
                            key: format!("{}/@text:43", use_scope),
                            size: Some(((20.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[7]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("◇".to_owned()).to_string(),
                        }),
                    });
                    children.push(::ducktape_view_guest::wire::Node::Text {
                        options: ::ducktape_view_guest::wire::TextOptions {
                            height: None,
                            align_y: None,
                            line_height: Some(::ducktape_view_guest::wire::LineHeight::Relative(
                                1.35f32,
                            )),
                            shaping: None,
                            wrapping: None,
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
                        key: format!("{}/@text:44", use_scope),
                        size: Some(16.0f32),
                        color: Some(palette.colors[4]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                        },
                        width: None,
                        align_x: None,
                        content: ("Not connected".to_owned()).to_string(),
                    });
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: Some(
                                    ::ducktape_view_guest::wire::LineHeight::Relative(1.5f32),
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
                            key: format!("{}/@text:45", use_scope),
                            size: Some(12.5f32),
                            color: Some(palette.colors[5]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("Click the network name in the titlebar to pick or reconnect a network."
                                .to_owned())
                                .to_string(),
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:28", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: Some((7.0) as f32),
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(super) fn render_empty_state_34(
        &self,
        palette: Palette,
        use_scope: String,
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
                height: Some(::ducktape_view_guest::wire::Length::Fill),
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (22.0) as f32,
                    right: (22.0) as f32,
                    bottom: (22.0) as f32,
                    left: (22.0) as f32,
                }),
                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                background: (None).map(::ducktape_view_guest::wire::Background::Color),
                border: None,
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
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
                        key: format!("{}/@container:33", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fixed((42.0) as f32)),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((42.0) as f32)),
                        padding: None,
                        align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                        align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                        background: (Some(palette.colors[3]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: Some(::ducktape_view_guest::wire::Border {
                            color: Some(palette.colors[39]),
                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                            radius: Some([
                                ((21.0) as f32).max(0.0).min(f32::MAX),
                                ((21.0) as f32).max(0.0).min(f32::MAX),
                                ((21.0) as f32).max(0.0).min(f32::MAX),
                                ((21.0) as f32).max(0.0).min(f32::MAX),
                            ]),
                        }),
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
                            key: format!("{}/@text:43", use_scope),
                            size: Some(((20.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[7]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("◇".to_owned()).to_string(),
                        }),
                    });
                    children.push(::ducktape_view_guest::wire::Node::Text {
                        options: ::ducktape_view_guest::wire::TextOptions {
                            height: None,
                            align_y: None,
                            line_height: Some(::ducktape_view_guest::wire::LineHeight::Relative(
                                1.35f32,
                            )),
                            shaping: None,
                            wrapping: None,
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
                        key: format!("{}/@text:44", use_scope),
                        size: Some(16.0f32),
                        color: Some(palette.colors[4]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                        },
                        width: None,
                        align_x: None,
                        content: ("No messages yet".to_owned()).to_string(),
                    });
                    children.push(::ducktape_view_guest::wire::Node::Text {
                        options: ::ducktape_view_guest::wire::TextOptions {
                            height: None,
                            align_y: None,
                            line_height: Some(::ducktape_view_guest::wire::LineHeight::Relative(
                                1.5f32,
                            )),
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
                        key: format!("{}/@text:45", use_scope),
                        size: Some(12.5f32),
                        color: Some(palette.colors[5]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("Nobody has posted here. Send the first message below."
                            .to_owned())
                        .to_string(),
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:28", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: Some((7.0) as f32),
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(super) fn render_gate_note_42(
        &self,
        palette: Palette,
        use_scope: String,
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
                    top: (11.0) as f32,
                    right: (13.0) as f32,
                    bottom: (11.0) as f32,
                    left: (13.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[84]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[33]),
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
                    children.push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
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
                            key: format!("{}/@container:133", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fixed((6.0) as f32)),
                            height: Some(::ducktape_view_guest::wire::Length::Fixed((6.0) as f32)),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[34]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: None,
                                width: None,
                                radius: Some([
                                    ((3.0) as f32).max(0.0).min(f32::MAX),
                                    ((3.0) as f32).max(0.0).min(f32::MAX),
                                    ((3.0) as f32).max(0.0).min(f32::MAX),
                                    ((3.0) as f32).max(0.0).min(f32::MAX),
                                ]),
                            }),
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(::ducktape_view_guest::wire::Length::Fixed(
                                    (1.0) as f32,
                                )),
                                height: Some(::ducktape_view_guest::wire::Length::Fixed(
                                    (1.0) as f32,
                                )),
                            }),
                        });
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:132", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Column,
                            spacing: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (4.0) as f32,
                                right: (0.0) as f32,
                                bottom: (0.0) as f32,
                                left: (0.0) as f32,
                            }),
                            width: None,
                            height: None,
                            align: None,
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
                    children.push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        children.push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: Some(
                                    ::ducktape_view_guest::wire::LineHeight::Relative(
                                        ((1.45) as f32).max(f32::EPSILON).min(f32::MAX),
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
                            key: format!("{}/@text:141", use_scope),
                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[30]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            align_x: None,
                            content: ("This channel is archived — new messages are refused."
                                .to_owned())
                            .to_string(),
                        });
                        {
                            children.push(::ducktape_view_guest::wire::Node::Text {
                                options: ::ducktape_view_guest::wire::TextOptions {
                                    height: None,
                                    align_y: None,
                                    line_height: Some(
                                        ::ducktape_view_guest::wire::LineHeight::Relative(
                                            ((1.45) as f32).max(f32::EPSILON).min(f32::MAX),
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
                                key: format!("{}/@text:148", use_scope),
                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[70]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                align_x: None,
                                content: ("Unarchive it from Channel details to post here again."
                                    .to_owned())
                                .to_string(),
                            });
                        }
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:140", use_scope),
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
                        key: format!("{}/@layout:127", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((8.0) as f32),
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Left),
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(super) fn render_gate_note_43(
        &self,
        palette: Palette,
        use_scope: String,
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
                    top: (11.0) as f32,
                    right: (13.0) as f32,
                    bottom: (11.0) as f32,
                    left: (13.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[84]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[33]),
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
                    children.push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
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
                            key: format!("{}/@container:133", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fixed((6.0) as f32)),
                            height: Some(::ducktape_view_guest::wire::Length::Fixed((6.0) as f32)),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[34]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: None,
                                width: None,
                                radius: Some([
                                    ((3.0) as f32).max(0.0).min(f32::MAX),
                                    ((3.0) as f32).max(0.0).min(f32::MAX),
                                    ((3.0) as f32).max(0.0).min(f32::MAX),
                                    ((3.0) as f32).max(0.0).min(f32::MAX),
                                ]),
                            }),
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(::ducktape_view_guest::wire::Length::Fixed(
                                    (1.0) as f32,
                                )),
                                height: Some(::ducktape_view_guest::wire::Length::Fixed(
                                    (1.0) as f32,
                                )),
                            }),
                        });
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:132", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Column,
                            spacing: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (4.0) as f32,
                                right: (0.0) as f32,
                                bottom: (0.0) as f32,
                                left: (0.0) as f32,
                            }),
                            width: None,
                            height: None,
                            align: None,
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
                    children.push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        children.push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: Some(
                                    ::ducktape_view_guest::wire::LineHeight::Relative(
                                        ((1.45) as f32).max(f32::EPSILON).min(f32::MAX),
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
                            key: format!("{}/@text:141", use_scope),
                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[30]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            align_x: None,
                            content:
                                ("This channel is members-only and your key is not on its roster."
                                    .to_owned())
                                .to_string(),
                        });
                        {
                            children.push(::ducktape_view_guest::wire::Node::Text {
                                options: ::ducktape_view_guest::wire::TextOptions {
                                    height: None,
                                    align_y: None,
                                    line_height: Some(
                                        ::ducktape_view_guest::wire::LineHeight::Relative(
                                            ((1.45) as f32).max(f32::EPSILON).min(f32::MAX),
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
                                key: format!("{}/@text:148", use_scope),
                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[70]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                align_x: None,
                                content: ("Ask a member to add your key from Channel details."
                                    .to_owned())
                                .to_string(),
                            });
                        }
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:140", use_scope),
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
                        key: format!("{}/@layout:127", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((8.0) as f32),
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Left),
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(super) fn render_eyebrow_45(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
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
                                "Geist Mono".into(),
                            ),
                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                        }),
                    },
                    key: format!("{}/@text:193", use_scope),
                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[73]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: ("NAME".to_owned()).to_string(),
                });
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Row,
                    spacing: Some((8.0) as f32),
                    padding: None,
                    width: None,
                    height: None,
                    align: Some(::ducktape_view_guest::wire::AlignX::Center),
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(super) fn render_eyebrow_46(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
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
                                "Geist Mono".into(),
                            ),
                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                        }),
                    },
                    key: format!("{}/@text:193", use_scope),
                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[73]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: ("MEMBERS".to_owned()).to_string(),
                });
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Row,
                    spacing: Some((8.0) as f32),
                    padding: None,
                    width: None,
                    height: None,
                    align: Some(::ducktape_view_guest::wire::AlignX::Center),
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
}
