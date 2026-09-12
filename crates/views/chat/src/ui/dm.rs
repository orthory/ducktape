use super::*;
impl super::ChatView {
    pub(super) fn render_dm_row_11(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: crate::host::DmPeer,
        arg_2: bool,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children.push(self.render_principal_avatar_10(
                    palette,
                    format!("{}/PrincipalAvatar@1309", use_scope),
                    arg_0.initials.to_owned(),
                    arg_0.is_agent,
                ));
                children.push(::ducktape_view_guest::wire::Node::Container {
                    shadow: ::ducktape_view_guest::wire::Shadow {
                        color: None,
                        x: None,
                        y: None,
                        blur: None,
                    },
                    max_width: None,
                    max_height: None,
                    clip: true,
                    key: format!("{}/@container:108", use_scope),
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    padding: None,
                    align_x: None,
                    align_y: None,
                    background: (None).map(::ducktape_view_guest::wire::Background::Color),
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
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:109", use_scope),
                        size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[4]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: (arg_0.name.to_owned()).to_string(),
                    }),
                });
                if arg_0.is_agent {
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
                        key: format!("{}/@container:132", use_scope),
                        width: None,
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (1.0) as f32,
                            right: (4.0) as f32,
                            bottom: (1.0) as f32,
                            left: (4.0) as f32,
                        }),
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[73]))
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
                            key: format!("{}/@text:138", use_scope),
                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[9]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("AI".to_owned()).to_string(),
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
                        key: format!("{}/@container:148", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fixed((7.0) as f32)),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((7.0) as f32)),
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[16]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: Some(::ducktape_view_guest::wire::Border {
                            color: None,
                            width: None,
                            radius: Some([
                                ((3.5) as f32).max(0.0).min(f32::MAX),
                                ((3.5) as f32).max(0.0).min(f32::MAX),
                                ((3.5) as f32).max(0.0).min(f32::MAX),
                                ((3.5) as f32).max(0.0).min(f32::MAX),
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
                    axis: ::ducktape_view_guest::wire::Axis::Row,
                    spacing: Some((8.0) as f32),
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: Some(::ducktape_view_guest::wire::AlignX::Center),
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(super) fn render_dm_row_12(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: crate::host::DmPeer,
        arg_2: bool,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children.push(self.render_principal_avatar_10(
                    palette,
                    format!("{}/PrincipalAvatar@1309", use_scope),
                    arg_0.initials.to_owned(),
                    arg_0.is_agent,
                ));
                if (!false) && arg_2 {
                    children.push(::ducktape_view_guest::wire::Node::Container {
                        shadow: ::ducktape_view_guest::wire::Shadow {
                            color: None,
                            x: None,
                            y: None,
                            blur: None,
                        },
                        max_width: None,
                        max_height: None,
                        clip: true,
                        key: format!("{}/@container:115", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: (None).map(::ducktape_view_guest::wire::Background::Color),
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
                            key: format!("{}/@text:116", use_scope),
                            size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[4]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: (arg_0.name.to_owned()).to_string(),
                        }),
                    });
                }
                if (!false) && (!arg_2) {
                    children.push(::ducktape_view_guest::wire::Node::Container {
                        shadow: ::ducktape_view_guest::wire::Shadow {
                            color: None,
                            x: None,
                            y: None,
                            blur: None,
                        },
                        max_width: None,
                        max_height: None,
                        clip: true,
                        key: format!("{}/@container:123", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: (None).map(::ducktape_view_guest::wire::Background::Color),
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
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:124", use_scope),
                            size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[5]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: (arg_0.name.to_owned()).to_string(),
                        }),
                    });
                }
                if arg_0.is_agent {
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
                        key: format!("{}/@container:132", use_scope),
                        width: None,
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (1.0) as f32,
                            right: (4.0) as f32,
                            bottom: (1.0) as f32,
                            left: (4.0) as f32,
                        }),
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[73]))
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
                            key: format!("{}/@text:138", use_scope),
                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[9]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("AI".to_owned()).to_string(),
                        }),
                    });
                }
                if arg_2 && (!false) {
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
                        key: format!("{}/@container:148", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fixed((7.0) as f32)),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((7.0) as f32)),
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[16]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: Some(::ducktape_view_guest::wire::Border {
                            color: None,
                            width: None,
                            radius: Some([
                                ((3.5) as f32).max(0.0).min(f32::MAX),
                                ((3.5) as f32).max(0.0).min(f32::MAX),
                                ((3.5) as f32).max(0.0).min(f32::MAX),
                                ((3.5) as f32).max(0.0).min(f32::MAX),
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
                    axis: ::ducktape_view_guest::wire::Axis::Row,
                    spacing: Some((8.0) as f32),
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: Some(::ducktape_view_guest::wire::AlignX::Center),
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(super) fn render_dm_button_13(
        &self,
        palette: Palette,
        use_scope: String,
        cb_11: impl Fn(String) -> Message + Clone + 'static,
        arg_0: crate::host::DmPeer,
        arg_1: bool,
        arg_2: bool,
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
                    top: (0.0) as f32,
                    right: (8.0) as f32,
                    bottom: (0.0) as f32,
                    left: (8.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (None).map(::ducktape_view_guest::wire::Background::Color),
                border: None,
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    if arg_1 {
                        children.push(::ducktape_view_guest::wire::Node::Button {
                            checked: Some(arg_1),
                            expanded: None,
                            description: None,
                            key: format!("{}/@button:26", use_scope),
                            content: ::ducktape_view_guest::wire::ButtonContent::Child(Box::new({
                                let mut children: Vec<::ducktape_view_guest::wire::Node> =
                                    Vec::new();
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
                                    key: format!("{}/@container:42", use_scope),
                                    width: Some(::ducktape_view_guest::wire::Length::Fixed(
                                        (2.5) as f32,
                                    )),
                                    height: Some(::ducktape_view_guest::wire::Length::Fill),
                                    padding: None,
                                    align_x: None,
                                    align_y: None,
                                    background: (Some(palette.colors[16]))
                                        .map(::ducktape_view_guest::wire::Background::Color),
                                    border: Some(::ducktape_view_guest::wire::Border {
                                        color: None,
                                        width: None,
                                        radius: Some([
                                            ((1.25) as f32).max(0.0).min(f32::MAX),
                                            ((1.25) as f32).max(0.0).min(f32::MAX),
                                            ((1.25) as f32).max(0.0).min(f32::MAX),
                                            ((1.25) as f32).max(0.0).min(f32::MAX),
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
                                    key: format!("{}/@container:49", use_scope),
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                        top: (6.0) as f32,
                                        right: (8.0) as f32,
                                        bottom: (6.0) as f32,
                                        left: (5.5) as f32,
                                    }),
                                    align_x: None,
                                    align_y: None,
                                    background: (None)
                                        .map(::ducktape_view_guest::wire::Background::Color),
                                    border: None,
                                    snap: None,
                                    content: Box::new(self.render_dm_row_11(
                                        palette,
                                        format!("{}/DmRow@1265", use_scope),
                                        arg_0.clone(),
                                        arg_2,
                                    )),
                                });
                                ::ducktape_view_guest::wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:41", use_scope),
                                    wrap: None,
                                    axis: ::ducktape_view_guest::wire::Axis::Row,
                                    spacing: Some((0.0) as f32),
                                    padding: None,
                                    width: None,
                                    height: None,
                                    align: None,
                                    background: None,
                                    border: None,
                                    children: children,
                                }
                            })),
                            label: Some(String::from(arg_0.name.to_owned())),
                            on_press: if self.busy {
                                None
                            } else {
                                Some(::ducktape_view_guest::slots::message((cb_11)(
                                    arg_0.key.to_owned(),
                                )))
                            },
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges::all((0.0) as f32)),
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
                                    background: Some(palette.colors[91]),
                                    text: Some(palette.colors[4]),
                                    border: Some(::ducktape_view_guest::wire::Border {
                                        color: Some(::ducktape_view_guest::wire::Rgba([
                                            0.0 / 255.0,
                                            0.0 / 255.0,
                                            0.0 / 255.0,
                                            0.000000,
                                        ])),
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
                                    background: Some(palette.colors[91]),
                                    text: Some(palette.colors[4]),
                                    border: None,
                                }),
                                pressed: Some(::ducktape_view_guest::wire::Face {
                                    background: Some(palette.colors[58]),
                                    text: Some(palette.colors[4]),
                                    border: None,
                                }),
                                disabled: None,
                            },
                        });
                    }
                    if !arg_1 {
                        children.push(::ducktape_view_guest::wire::Node::Button {
                            checked: Some(arg_1),
                            expanded: None,
                            description: None,
                            key: format!("{}/@button:65", use_scope),
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
                                    key: format!("{}/@container:73", use_scope),
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                        top: (6.0) as f32,
                                        right: (8.0) as f32,
                                        bottom: (6.0) as f32,
                                        left: (8.0) as f32,
                                    }),
                                    align_x: None,
                                    align_y: None,
                                    background: (None)
                                        .map(::ducktape_view_guest::wire::Background::Color),
                                    border: None,
                                    snap: None,
                                    content: Box::new(self.render_dm_row_12(
                                        palette,
                                        format!("{}/DmRow@1289", use_scope),
                                        arg_0.clone(),
                                        arg_2,
                                    )),
                                },
                            )),
                            label: Some(String::from(arg_0.name.to_owned())),
                            on_press: if self.busy {
                                None
                            } else {
                                Some(::ducktape_view_guest::slots::message((cb_11)(
                                    arg_0.key.to_owned(),
                                )))
                            },
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges::all((0.0) as f32)),
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
                                    text: Some(palette.colors[5]),
                                    border: Some(::ducktape_view_guest::wire::Border {
                                        color: Some(::ducktape_view_guest::wire::Rgba([
                                            0.0 / 255.0,
                                            0.0 / 255.0,
                                            0.0 / 255.0,
                                            0.000000,
                                        ])),
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
                                    background: Some(palette.colors[58]),
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
                    }
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:24", use_scope),
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
                }),
            }
        }
    }
    pub(super) fn render_dm_header_23(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children.push(self.render_principal_avatar_22(
                    palette,
                    format!("{}/PrincipalAvatar@1388", use_scope),
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
                            family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                        }),
                    },
                    key: format!("{}/@text:186", use_scope),
                    size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[4]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: (self.active_dm.name.to_owned()).to_string(),
                });
                if self.active_dm.is_agent {
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
                        key: format!("{}/@container:193", use_scope),
                        width: None,
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (2.0) as f32,
                            right: (5.0) as f32,
                            bottom: (2.0) as f32,
                            left: (5.0) as f32,
                        }),
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[7]))
                            .map(::ducktape_view_guest::wire::Background::Color),
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
                            key: format!("{}/@text:199", use_scope),
                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[9]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("AGENT".to_owned()).to_string(),
                        }),
                    });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Row,
                    spacing: Some((9.0) as f32),
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
