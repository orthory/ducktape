use super::*;
impl super::ChatView {
    pub(super) fn render_channel_button_1(
        &self,
        palette: Palette,
        use_scope: String,
        cb_10: impl Fn(String) -> Message + Clone + 'static,
        arg_0: crate::host::ChatChannel,
        arg_1: bool,
        arg_2: bool,
    ) -> ::ducktape_view_guest::wire::Node {
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
            key: format!("{}/@container:6", use_scope),
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
                    children
                        .push(::ducktape_view_guest::wire::Node::Button {
                            checked: Some(arg_1),
                            expanded: None,
                            description: None,
                            key: format!("{}/@button:13", use_scope),
                            content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                Box::new({
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
                                            key: format!("{}/@container:30", use_scope),
                                            width: Some(
                                                ::ducktape_view_guest::wire::Length::Fixed((2.5) as f32),
                                            ),
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
                                            key: format!("{}/@container:37", use_scope),
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: None,
                                            padding: Some(::ducktape_view_guest::wire::Edges {
                                                top: (7.0) as f32,
                                                right: (8.0) as f32,
                                                bottom: (7.0) as f32,
                                                left: (5.5) as f32,
                                            }),
                                            align_x: None,
                                            align_y: None,
                                            background: (None)
                                                .map(::ducktape_view_guest::wire::Background::Color),
                                            border: None,
                                            snap: None,
                                            content: Box::new({
                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                if arg_0.members_only {
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
                                                            key: format!("{}/@text:50", use_scope),
                                                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                            color: Some(palette.colors[73]),
                                                            font: ::ducktape_view_guest::wire::Font {
                                                                monospace: false,
                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                            },
                                                            width: None,
                                                            align_x: None,
                                                            content: ("◆".to_owned()).to_string(),
                                                        });
                                                }
                                                if !arg_0.members_only  {
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
                                                            key: format!("{}/@text:56", use_scope),
                                                            size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                            color: Some(palette.colors[73]),
                                                            font: ::ducktape_view_guest::wire::Font {
                                                                monospace: false,
                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                            },
                                                            width: None,
                                                            align_x: None,
                                                            content: ("#".to_owned()).to_string(),
                                                        });
                                                }
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
                                                        key: format!("{}/@container:61", use_scope),
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
                                                                    weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                }),
                                                            },
                                                            key: format!("{}/@text:62", use_scope),
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
                                                if arg_0.archived {
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
                                                            key: format!("{}/@text:69", use_scope),
                                                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                            color: Some(palette.colors[71]),
                                                            font: ::ducktape_view_guest::wire::Font {
                                                                monospace: false,
                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                            },
                                                            width: None,
                                                            align_x: None,
                                                            content: ("archived".to_owned()).to_string(),
                                                        });
                                                }
                                                if arg_0.huddle_count > 0  {
                                                    children
                                                        .push(
                                                            self
                                                                .icon(
                                                                    format!("{}/Icon@1489", use_scope),
                                                                    "headphones",
                                                                    12f32,
                                                                    (palette).colors[25usize],
                                                                    "@media:58",
                                                                ),
                                                        );
                                                }
                                                ::ducktape_view_guest::wire::Node::Linear {
                                                    max_width: None,
                                                    clip: false,
                                                    key: format!("{}/@layout:44", use_scope),
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
                                            }),
                                        });
                                    ::ducktape_view_guest::wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:29", use_scope),
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
                                }),
                            ),
                            label: Some(String::from(arg_0.name.to_owned())),
                            on_press: if self.busy  {
                                None
                            } else {
                                Some(
                                    ::ducktape_view_guest::slots::message(
                                        (cb_10)(arg_0.id.to_owned()),
                                    ),
                                )
                            },
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
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
                    children
                        .push(::ducktape_view_guest::wire::Node::Button {
                            checked: Some(arg_1),
                            expanded: None,
                            description: None,
                            key: format!("{}/@button:85", use_scope),
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
                                    key: format!("{}/@container:93", use_scope),
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                        top: (7.0) as f32,
                                        right: (8.0) as f32,
                                        bottom: (7.0) as f32,
                                        left: (8.0) as f32,
                                    }),
                                    align_x: None,
                                    align_y: None,
                                    background: (None)
                                        .map(::ducktape_view_guest::wire::Background::Color),
                                    border: None,
                                    snap: None,
                                    content: Box::new({
                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                        if arg_0.members_only {
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
                                                    key: format!("{}/@text:106", use_scope),
                                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: Some(palette.colors[73]),
                                                    font: ::ducktape_view_guest::wire::Font {
                                                        monospace: false,
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    },
                                                    width: None,
                                                    align_x: None,
                                                    content: ("◆".to_owned()).to_string(),
                                                });
                                        }
                                        if !arg_0.members_only  {
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
                                                    key: format!("{}/@text:112", use_scope),
                                                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: Some(palette.colors[73]),
                                                    font: ::ducktape_view_guest::wire::Font {
                                                        monospace: false,
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    },
                                                    width: None,
                                                    align_x: None,
                                                    content: ("#".to_owned()).to_string(),
                                                });
                                        }
                                        if arg_2 {
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
                                                    key: format!("{}/@container:118", use_scope),
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
                                                        key: format!("{}/@text:119", use_scope),
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
                                        if !arg_2  {
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
                                                    key: format!("{}/@container:126", use_scope),
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
                                                                weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                            }),
                                                        },
                                                        key: format!("{}/@text:127", use_scope),
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
                                        if arg_0.archived {
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
                                                    key: format!("{}/@text:134", use_scope),
                                                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: Some(palette.colors[71]),
                                                    font: ::ducktape_view_guest::wire::Font {
                                                        monospace: false,
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    },
                                                    width: None,
                                                    align_x: None,
                                                    content: ("archived".to_owned()).to_string(),
                                                });
                                        }
                                        if arg_0.huddle_count > 0  {
                                            children
                                                .push(
                                                    self
                                                        .icon(
                                                            format!("{}/Icon@1554", use_scope),
                                                            "headphones",
                                                            12f32,
                                                            (palette).colors[25usize],
                                                            "@media:58",
                                                        ),
                                                );
                                        }
                                        if arg_2 {
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
                                                    key: format!("{}/@container:147", use_scope),
                                                    width: Some(
                                                        ::ducktape_view_guest::wire::Length::Fixed((7.0) as f32),
                                                    ),
                                                    height: Some(
                                                        ::ducktape_view_guest::wire::Length::Fixed((7.0) as f32),
                                                    ),
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
                                                        width: Some(
                                                            ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                                        ),
                                                        height: Some(
                                                            ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                                        ),
                                                    }),
                                                });
                                        }
                                        ::ducktape_view_guest::wire::Node::Linear {
                                            max_width: None,
                                            clip: false,
                                            key: format!("{}/@layout:100", use_scope),
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
                                    }),
                                }),
                            ),
                            label: Some(String::from(arg_0.name.to_owned())),
                            on_press: if self.busy  {
                                None
                            } else {
                                Some(
                                    ::ducktape_view_guest::slots::message(
                                        (cb_10)(arg_0.id.to_owned()),
                                    ),
                                )
                            },
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
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
                    key: format!("{}/@layout:11", use_scope),
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
    pub(super) fn render_skeleton_row_35(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
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
                key: format!("{}/@container:1117", use_scope),
                width: Some(::ducktape_view_guest::wire::Length::Fixed((30.0) as f32)),
                height: Some(::ducktape_view_guest::wire::Length::Fixed((30.0) as f32)),
                padding: None,
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[56]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: None,
                    width: None,
                    radius: Some([
                        ((15.0) as f32).max(0.0).min(f32::MAX),
                        ((15.0) as f32).max(0.0).min(f32::MAX),
                        ((15.0) as f32).max(0.0).min(f32::MAX),
                        ((15.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new(::ducktape_view_guest::wire::Node::Space {
                    width: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                    height: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                }),
            });
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
                    key: format!("{}/@container:1129", use_scope),
                    width: Some(::ducktape_view_guest::wire::Length::Fixed((96.0) as f32)),
                    height: Some(::ducktape_view_guest::wire::Length::Fixed((9.0) as f32)),
                    padding: None,
                    align_x: None,
                    align_y: None,
                    background: (Some(palette.colors[56]))
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
                    content: Box::new(::ducktape_view_guest::wire::Node::Space {
                        width: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                    }),
                });
                children.push(::ducktape_view_guest::wire::Node::Container {
                    shadow: ::ducktape_view_guest::wire::Shadow {
                        color: None,
                        x: None,
                        y: None,
                        blur: None,
                    },
                    max_width: Some(((420.0) as f32).max(0.0).min(f32::MAX)),
                    max_height: None,
                    clip: false,
                    key: format!("{}/@container:1136", use_scope),
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: Some(::ducktape_view_guest::wire::Length::Fixed((9.0) as f32)),
                    padding: None,
                    align_x: None,
                    align_y: None,
                    background: (Some(palette.colors[56]))
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
                    content: Box::new(::ducktape_view_guest::wire::Node::Space {
                        width: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                    }),
                });
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: format!("{}/@layout:1124", use_scope),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: Some((6.0) as f32),
                    padding: Some(::ducktape_view_guest::wire::Edges {
                        top: (4.0) as f32,
                        right: (0.0) as f32,
                        bottom: (0.0) as f32,
                        left: (0.0) as f32,
                    }),
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
                key: format!("{}/@layout:1112", use_scope),
                wrap: None,
                axis: ::ducktape_view_guest::wire::Axis::Row,
                spacing: Some((11.0) as f32),
                padding: None,
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: None,
                align: Some(::ducktape_view_guest::wire::AlignX::Left),
                background: None,
                border: None,
                children: children,
            }
        }
    }
    pub(super) fn render_chat_search_result_41(
        &self,
        palette: Palette,
        use_scope: String,
        cb_26: impl Fn(String, i64, i64) -> Message + Clone + 'static,
        arg_0: crate::host::ChatSearchHit,
    ) -> ::ducktape_view_guest::wire::Node {
        ::ducktape_view_guest::wire::Node::Button {
            checked: None,
            expanded: None,
            description: None,
            key: format!("{}/@button:893", use_scope),
            content: ::ducktape_view_guest::wire::ButtonContent::Child(Box::new({
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children.push({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children.push(::ducktape_view_guest::wire::Node::Text {
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
                                weight: ::ducktape_view_guest::wire::Weight::Medium,
                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:905", use_scope),
                        size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[4]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        align_x: None,
                        content: (arg_0.author.to_owned()).to_string(),
                    });
                    children.push(::ducktape_view_guest::wire::Node::Text {
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
                                weight: ::ducktape_view_guest::wire::Weight::Medium,
                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:911", use_scope),
                        size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[5]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: (arg_0.meta.to_owned()).to_string(),
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:900", use_scope),
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
                children.push(::ducktape_view_guest::wire::Node::Text {
                    options: ::ducktape_view_guest::wire::TextOptions {
                        height: None,
                        align_y: None,
                        line_height: None,
                        shaping: None,
                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::WordOrGlyph),
                        tracking: 0.0f32,
                        font: Some(::ducktape_view_guest::wire::NamedFont {
                            family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                        }),
                    },
                    key: format!("{}/@text:916", use_scope),
                    size: Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[4]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    align_x: None,
                    content: (arg_0.text.to_owned()).to_string(),
                });
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: format!("{}/@layout:899", use_scope),
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
            })),
            label: Some(String::from(arg_0.text.to_owned())),
            on_press: Some(::ducktape_view_guest::slots::message((cb_26)(
                arg_0.channel_id.to_owned(),
                arg_0.root_seq,
                arg_0.seq,
            ))),
            width: Some(::ducktape_view_guest::wire::Length::Fill),
            height: None,
            padding: Some(::ducktape_view_guest::wire::Edges::all((8.0) as f32)),
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
                        family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
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
                            ((9.0) as f32).max(0.0).min(f32::MAX),
                            ((9.0) as f32).max(0.0).min(f32::MAX),
                            ((9.0) as f32).max(0.0).min(f32::MAX),
                            ((9.0) as f32).max(0.0).min(f32::MAX),
                        ]),
                    }),
                },
                hovered: Some(::ducktape_view_guest::wire::Face {
                    background: Some({
                        let mut color = palette.colors[4];
                        color.0[3] = 0.060000;
                        color
                    }),
                    text: Some(palette.colors[4]),
                    border: Some(::ducktape_view_guest::wire::Border {
                        color: Some({
                            let mut color = palette.colors[4];
                            color.0[3] = 0.090000;
                            color
                        }),
                        width: None,
                        radius: None,
                    }),
                }),
                pressed: Some(::ducktape_view_guest::wire::Face {
                    background: Some({
                        let mut color = palette.colors[4];
                        color.0[3] = 0.100000;
                        color
                    }),
                    text: Some(palette.colors[4]),
                    border: Some(::ducktape_view_guest::wire::Border {
                        color: Some({
                            let mut color = palette.colors[4];
                            color.0[3] = 0.130000;
                            color
                        }),
                        width: None,
                        radius: None,
                    }),
                }),
                disabled: None,
            },
        }
    }
    pub(super) fn render_composer_gate_44(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if self.post_refusal == "channel_archived" {
                    children.push(
                        self.render_gate_note_42(palette, format!("{}/GateNote@2379", use_scope)),
                    );
                }
                if (!(self.post_refusal == "channel_archived"))
                    && (self.post_refusal == "members_only")
                {
                    children.push(
                        self.render_gate_note_43(palette, format!("{}/GateNote@2384", use_scope)),
                    );
                }
                if !((self.post_refusal == "channel_archived")
                    || (self.post_refusal == "members_only"))
                {
                    children.push(::ducktape_view_guest::wire::Node::Space {
                        width: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
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
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(super) fn render_chat_member_row_47(
        &self,
        palette: Palette,
        use_scope: String,
        cb_35: impl Fn(String) -> Message + Clone + 'static,
        arg_0: crate::host::ChatMember,
    ) -> ::ducktape_view_guest::wire::Node {
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
                        family: ::ducktape_view_guest::wire::FontFamily::Named("Geist Mono".into()),
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                    }),
                },
                key: format!("{}/@text:168", use_scope),
                size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                color: Some(palette.colors[15]),
                font: ::ducktape_view_guest::wire::Font {
                    monospace: false,
                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                },
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                align_x: None,
                content: (arg_0.label.to_owned()).to_string(),
            });
            children.push(::ducktape_view_guest::wire::Node::Button {
                checked: None,
                expanded: None,
                description: Some(String::from(arg_0.label.to_owned())),
                key: format!("{}/@button:175", use_scope),
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
                        key: format!("{}/@container:184", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: Some(::ducktape_view_guest::wire::Length::Fill),
                        padding: None,
                        align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                        align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
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
                            key: format!("{}/@text:195", use_scope),
                            size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: None,
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("×".to_owned()).to_string(),
                        }),
                    },
                )),
                label: Some(String::from("Remove member".to_owned())),
                on_press: if self.busy {
                    None
                } else {
                    Some(::ducktape_view_guest::slots::message((cb_35)(
                        arg_0.key.to_owned(),
                    )))
                },
                width: Some(::ducktape_view_guest::wire::Length::Fixed((24.0) as f32)),
                height: Some(::ducktape_view_guest::wire::Length::Fixed((24.0) as f32)),
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
                            family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
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
                                ((6.0) as f32).max(0.0).min(f32::MAX),
                                ((6.0) as f32).max(0.0).min(f32::MAX),
                                ((6.0) as f32).max(0.0).min(f32::MAX),
                                ((6.0) as f32).max(0.0).min(f32::MAX),
                            ]),
                        }),
                    },
                    hovered: Some(::ducktape_view_guest::wire::Face {
                        background: Some(palette.colors[22]),
                        text: Some(palette.colors[4]),
                        border: None,
                    }),
                    pressed: Some(::ducktape_view_guest::wire::Face {
                        background: Some(palette.colors[23]),
                        text: Some(palette.colors[4]),
                        border: None,
                    }),
                    disabled: None,
                },
            });
            ::ducktape_view_guest::wire::Node::Linear {
                max_width: None,
                clip: false,
                key: format!("{}/@layout:163", use_scope),
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
        }
    }
    pub(super) fn render_live_run_card_49(
        &self,
        palette: Palette,
        use_scope: String,
        cb_8: impl Fn(String) -> Message + Clone + 'static,
        cb_30: impl Fn(String) -> Message + Clone + 'static,
        arg_0: crate::host::LiveRunHint,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
            children.push({
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children.push(::ducktape_view_guest::wire::Node::Text {
                    options: ::ducktape_view_guest::wire::TextOptions {
                        height: None,
                        align_y: None,
                        line_height: None,
                        shaping: None,
                        wrapping: None,
                        tracking: 0.0f32,
                        font: Some(::ducktape_view_guest::wire::NamedFont {
                            family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                        }),
                    },
                    key: format!("{}/@text:1159", use_scope),
                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[4]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    align_x: None,
                    content: (arg_0.agent.to_owned()).to_string(),
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
                    key: format!("{}/@container:1165", use_scope),
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
                        key: format!("{}/@text:1171", use_scope),
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
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: format!("{}/@layout:1158", use_scope),
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
            children.push(::ducktape_view_guest::wire::Node::Text {
                options: ::ducktape_view_guest::wire::TextOptions {
                    height: None,
                    align_y: None,
                    line_height: None,
                    shaping: None,
                    wrapping: None,
                    tracking: 0.0f32,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                    }),
                },
                key: format!("{}/@text:1177", use_scope),
                size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                color: Some(palette.colors[5]),
                font: ::ducktape_view_guest::wire::Font {
                    monospace: false,
                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                },
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                align_x: None,
                content: (arg_0.status.to_owned()).to_string(),
            });
            children.push({
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children.push(::ducktape_view_guest::wire::Node::Button {
                    checked: None,
                    expanded: None,
                    description: None,
                    key: format!("{}/@button:1183", use_scope),
                    content: ::ducktape_view_guest::wire::ButtonContent::Label(String::from(
                        "View run",
                    )),
                    label: None,
                    on_press: Some(::ducktape_view_guest::slots::message((cb_30)(
                        arg_0.dispatch_id.to_owned(),
                    ))),
                    width: None,
                    height: None,
                    padding: Some(::ducktape_view_guest::wire::Edges::all((4.0) as f32)),
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
                children.push(::ducktape_view_guest::wire::Node::Button {
                    checked: None,
                    expanded: None,
                    description: None,
                    key: format!("{}/@button:1187", use_scope),
                    content: ::ducktape_view_guest::wire::ButtonContent::Label(String::from(
                        "Stop",
                    )),
                    label: None,
                    on_press: Some(::ducktape_view_guest::slots::message((cb_8)(
                        arg_0.run_id.to_owned(),
                    ))),
                    width: None,
                    height: None,
                    padding: Some(::ducktape_view_guest::wire::Edges::all((4.0) as f32)),
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
                    key: format!("{}/@layout:1182", use_scope),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Row,
                    spacing: Some((6.0) as f32),
                    padding: None,
                    width: None,
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
                key: format!("{}/@layout:1151", use_scope),
                wrap: None,
                axis: ::ducktape_view_guest::wire::Axis::Column,
                spacing: Some((5.0) as f32),
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (6.0) as f32,
                    right: (7.0) as f32,
                    bottom: (6.0) as f32,
                    left: (7.0) as f32,
                }),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: None,
                align: None,
                background: None,
                border: None,
                children: children,
            }
        }
    }
}
