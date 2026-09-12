use super::*;
impl super::ForgeView {
    pub(super) fn render_forge_org_header_3(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
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
                        key: format!("{}/@container:46", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fixed((30.0) as f32)),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((30.0) as f32)),
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
                        content: Box::new(self.icon(
                            format!("{}/Icon@1222", use_scope),
                            "branch",
                            16f32,
                            (palette).colors[38usize],
                            "@media:76",
                        )),
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
                                    "Geist".into(),
                                ),
                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:59", use_scope),
                        size: Some(((16.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[7]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: (self.org.to_owned()).to_string(),
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
                        key: format!("{}/@container:65", use_scope),
                        width: None,
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (2.0) as f32,
                            right: (6.0) as f32,
                            bottom: (2.0) as f32,
                            left: (6.0) as f32,
                        }),
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[16]))
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
                            key: format!("{}/@text:71", use_scope),
                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[17]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("ORG".to_owned()).to_string(),
                        }),
                    });
                    children.push(::ducktape_view_guest::wire::Node::Space {
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                    });
                    if self.list_phase == "ready" {
                        children.push({
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
                                        weight: ::ducktape_view_guest::wire::Weight::Medium,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:83", use_scope),
                                size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[71]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: (crate::host::plural(
                                    (self.repos).len() as i64,
                                    ::std::convert::AsRef::as_ref(&("repository")),
                                    ::std::convert::AsRef::as_ref(&("repositories")),
                                ))
                                .to_string(),
                            });
                            if !(self.tier).is_empty() {
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
                                    key: format!("{}/@text:93", use_scope),
                                    size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[71]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("·".to_owned()).to_string(),
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
                                    key: format!("{}/@text:99", use_scope),
                                    size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[71]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: (self.tier.to_owned()).to_string(),
                                });
                            }
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:82", use_scope),
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
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:41", use_scope),
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
                if !(self.about).is_empty() {
                    children.push(::ducktape_view_guest::wire::Node::Container {
                        shadow: ::ducktape_view_guest::wire::Shadow {
                            color: None,
                            x: None,
                            y: None,
                            blur: None,
                        },
                        max_width: Some(((680.0) as f32).max(0.0).min(f32::MAX)),
                        max_height: None,
                        clip: false,
                        key: format!("{}/@container:106", use_scope),
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
                                line_height: Some(
                                    ::ducktape_view_guest::wire::LineHeight::Relative(
                                        ((1.5) as f32).max(f32::EPSILON).min(f32::MAX),
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
                            key: format!("{}/@text:107", use_scope),
                            size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[70]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            align_x: None,
                            content: (self.about.to_owned()).to_string(),
                        }),
                    });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: Some((7.0) as f32),
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
    pub(super) fn render_repo_card_5(
        &self,
        palette: Palette,
        use_scope: String,
        cb_11: impl Fn(String) -> Message + Clone + 'static,
        arg_0: crate::host::ForgeRepo,
    ) -> ::ducktape_view_guest::wire::Node {
        ::ducktape_view_guest::wire::Node::Button {
            checked: None,
            expanded: None,
            description: Some(String::from(arg_0.name.to_owned())),
            key: format!("{}/@button:120", use_scope),
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
                    key: format!("{}/@container:127", use_scope),
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    padding: Some(::ducktape_view_guest::wire::Edges {
                        top: (11.0) as f32,
                        right: (14.0) as f32,
                        bottom: (11.0) as f32,
                        left: (14.0) as f32,
                    }),
                    align_x: None,
                    align_y: None,
                    background: (None).map(::ducktape_view_guest::wire::Background::Color),
                    border: None,
                    snap: None,
                    content: Box::new({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        children.push(self.icon(
                            format!("{}/Icon@1307", use_scope),
                            "branch",
                            14f32,
                            (palette).colors[5usize],
                            "@media:82",
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
                            key: format!("{}/@container:144", use_scope),
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
                                key: format!("{}/@text:145", use_scope),
                                size: Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[7]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: (arg_0.name.to_owned()).to_string(),
                            }),
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
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:151", use_scope),
                            size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[41]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: (arg_0.head.to_owned()).to_string(),
                        });
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:134", use_scope),
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
                    }),
                },
            )),
            label: Some(String::from("Open repo".to_owned())),
            on_press: Some(::ducktape_view_guest::slots::message((cb_11)(
                arg_0.name.to_owned(),
            ))),
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
                        family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
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
                            ((10.0) as f32).max(0.0).min(f32::MAX),
                            ((10.0) as f32).max(0.0).min(f32::MAX),
                            ((10.0) as f32).max(0.0).min(f32::MAX),
                            ((10.0) as f32).max(0.0).min(f32::MAX),
                        ]),
                    }),
                },
                hovered: Some(::ducktape_view_guest::wire::Face {
                    background: Some(palette.colors[88]),
                    text: Some(palette.colors[4]),
                    border: Some(::ducktape_view_guest::wire::Border {
                        color: Some(palette.colors[93]),
                        width: None,
                        radius: None,
                    }),
                }),
                pressed: Some(::ducktape_view_guest::wire::Face {
                    background: Some(palette.colors[55]),
                    text: Some(palette.colors[4]),
                    border: None,
                }),
                disabled: None,
            },
        }
    }
    pub(super) fn render_repo_crumb_7(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
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
                    key: format!("{}/@container:173", use_scope),
                    width: Some(::ducktape_view_guest::wire::Length::Fixed((28.0) as f32)),
                    height: Some(::ducktape_view_guest::wire::Length::Fixed((28.0) as f32)),
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
                    content: Box::new(self.icon(
                        format!("{}/Icon@1349", use_scope),
                        "branch",
                        15f32,
                        (palette).colors[38usize],
                        "@media:76",
                    )),
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
                            family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                        }),
                    },
                    key: format!("{}/@text:186", use_scope),
                    size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[70]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: (self.org.to_owned()).to_string(),
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
                            family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                        }),
                    },
                    key: format!("{}/@text:192", use_scope),
                    size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[96]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: ("/".to_owned()).to_string(),
                });
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
    pub(super) fn render_back_to_list_9(
        &self,
        palette: Palette,
        use_scope: String,
        cb_1: impl Fn() -> Message + Clone + 'static,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if self.forge_item_kind == "pr" {
                    children
                        .push(::ducktape_view_guest::wire::Node::Button {
                            checked: None,
                            expanded: None,
                            description: None,
                            key: format!("{}/@button:662", use_scope),
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
                                    key: format!("{}/@container:667", use_scope),
                                    width: None,
                                    height: None,
                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                        top: (4.0) as f32,
                                        right: (9.0) as f32,
                                        bottom: (4.0) as f32,
                                        left: (7.0) as f32,
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
                                                key: format!("{}/@text:674", use_scope),
                                                size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                color: Some(palette.colors[5]),
                                                font: ::ducktape_view_guest::wire::Font {
                                                    monospace: false,
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                },
                                                width: None,
                                                align_x: None,
                                                content: ("‹".to_owned()).to_string(),
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
                                                key: format!("{}/@text:679", use_scope),
                                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                color: Some(palette.colors[5]),
                                                font: ::ducktape_view_guest::wire::Font {
                                                    monospace: false,
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                },
                                                width: None,
                                                align_x: None,
                                                content: ("Pull requests".to_owned()).to_string(),
                                            });
                                        ::ducktape_view_guest::wire::Node::Linear {
                                            max_width: None,
                                            clip: false,
                                            key: format!("{}/@layout:673", use_scope),
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
                                    }),
                                }),
                            ),
                            label: Some(
                                String::from("Back to pull requests".to_owned()),
                            ),
                            on_press: Some(
                                ::ducktape_view_guest::slots::message((cb_1)()),
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
                }
                if (!(self.forge_item_kind == "pr")) && (self.forge_item_kind == "issue") {
                    children
                        .push(::ducktape_view_guest::wire::Node::Button {
                            checked: None,
                            expanded: None,
                            description: None,
                            key: format!("{}/@button:688", use_scope),
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
                                    key: format!("{}/@container:693", use_scope),
                                    width: None,
                                    height: None,
                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                        top: (4.0) as f32,
                                        right: (9.0) as f32,
                                        bottom: (4.0) as f32,
                                        left: (7.0) as f32,
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
                                                key: format!("{}/@text:700", use_scope),
                                                size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                color: Some(palette.colors[5]),
                                                font: ::ducktape_view_guest::wire::Font {
                                                    monospace: false,
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                },
                                                width: None,
                                                align_x: None,
                                                content: ("‹".to_owned()).to_string(),
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
                                                key: format!("{}/@text:705", use_scope),
                                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                color: Some(palette.colors[5]),
                                                font: ::ducktape_view_guest::wire::Font {
                                                    monospace: false,
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                },
                                                width: None,
                                                align_x: None,
                                                content: ("Issues".to_owned()).to_string(),
                                            });
                                        ::ducktape_view_guest::wire::Node::Linear {
                                            max_width: None,
                                            clip: false,
                                            key: format!("{}/@layout:699", use_scope),
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
                                    }),
                                }),
                            ),
                            label: Some(String::from("Back to issues".to_owned())),
                            on_press: Some(
                                ::ducktape_view_guest::slots::message((cb_1)()),
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
    pub(super) fn render_forge_tree_dir_row_16(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children.push(self.icon(
                    format!("{}/Icon@1475", use_scope),
                    "chevron-down",
                    10f32,
                    (palette).colors[71usize],
                    "@media:28",
                ));
                children.push(self.icon(
                    format!("{}/Icon@1486", use_scope),
                    "folder",
                    13f32,
                    (palette).colors[30usize],
                    "@media:64",
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
                    key: format!("{}/@text:323", use_scope),
                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[15]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    align_x: None,
                    content: ("/".to_owned()).to_string(),
                });
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Row,
                    spacing: Some((6.0) as f32),
                    padding: Some(::ducktape_view_guest::wire::Edges {
                        top: (5.0) as f32,
                        right: (14.0) as f32,
                        bottom: (5.0) as f32,
                        left: (10.0 + (0.0 * 15.0)) as f32,
                    }),
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
    pub(super) fn render_forge_tree_dir_row_17(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                {
                    children.push(self.icon(
                        format!("{}/Icon@1481", use_scope),
                        "chevron-right",
                        10f32,
                        (palette).colors[71usize],
                        "@media:28",
                    ));
                }
                children.push(self.icon(
                    format!("{}/Icon@1486", use_scope),
                    "folder",
                    13f32,
                    (palette).colors[30usize],
                    "@media:64",
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
                    key: format!("{}/@text:323", use_scope),
                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[15]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    align_x: None,
                    content: (arg_0.to_owned()).to_string(),
                });
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Row,
                    spacing: Some((6.0) as f32),
                    padding: Some(::ducktape_view_guest::wire::Edges {
                        top: (5.0) as f32,
                        right: (14.0) as f32,
                        bottom: (5.0) as f32,
                        left: (10.0 + (0.0 * 15.0)) as f32,
                    }),
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
    pub(super) fn render_forge_tree_file_face_19(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children.push(self.icon(
                    format!("{}/Icon@1529", use_scope),
                    "file",
                    12f32,
                    (palette).colors[73usize],
                    "@media:22",
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
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                        }),
                    },
                    key: format!("{}/@text:367", use_scope),
                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[7]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    align_x: None,
                    content: (arg_0.to_owned()).to_string(),
                });
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Row,
                    spacing: Some((6.0) as f32),
                    padding: Some(::ducktape_view_guest::wire::Edges {
                        top: (5.0) as f32,
                        right: (14.0) as f32,
                        bottom: (5.0) as f32,
                        left: (10.0 + (0.0 * 15.0)) as f32,
                    }),
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
    pub(super) fn render_forge_tree_file_face_20(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children.push(self.icon(
                    format!("{}/Icon@1529", use_scope),
                    "file",
                    12f32,
                    (palette).colors[73usize],
                    "@media:22",
                ));
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
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:375", use_scope),
                        size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[13]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        align_x: None,
                        content: (arg_0.to_owned()).to_string(),
                    });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Row,
                    spacing: Some((6.0) as f32),
                    padding: Some(::ducktape_view_guest::wire::Edges {
                        top: (5.0) as f32,
                        right: (14.0) as f32,
                        bottom: (5.0) as f32,
                        left: (10.0 + (0.0 * 15.0)) as f32,
                    }),
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
    pub(super) fn render_forge_tree_file_row_21(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
        arg_2: bool,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if arg_2 {
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
                        key: format!("{}/@container:336", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[91]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
                        snap: None,
                        content: Box::new(self.render_forge_tree_file_face_19(
                            palette,
                            format!("{}/ForgeTreeFileFace@1505", use_scope),
                            arg_0.to_owned(),
                        )),
                    });
                }
                if !arg_2 {
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
                        key: format!("{}/@container:343", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: (None).map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
                        snap: None,
                        content: Box::new(self.render_forge_tree_file_face_20(
                            palette,
                            format!("{}/ForgeTreeFileFace@1512", use_scope),
                            arg_0.to_owned(),
                        )),
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
    pub(super) fn render_forge_code_header_22(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
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
                        key: format!("{}/@container:393", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: Some(
                            ::ducktape_view_guest::wire::Length::Fixed((42.0) as f32),
                        ),
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (0.0) as f32,
                            right: (16.0) as f32,
                            bottom: (0.0) as f32,
                            left: (16.0) as f32,
                        }),
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[3]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
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
                                    key: format!("{}/@text:407", use_scope),
                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[15]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: (crate::host::forge_file_header(
                                            ::std::convert::AsRef::as_ref(&(self.opened_dir)),
                                            ::std::convert::AsRef::as_ref(&(self.opened_rev)),
                                            ::std::convert::AsRef::as_ref(&(self.tree_path)),
                                            ::std::convert::AsRef::as_ref(&(self.tree_rev)),
                                            ::std::convert::AsRef::as_ref(&(self.file_path)),
                                        )
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
                                    clip: true,
                                    key: format!("{}/@container:416", use_scope),
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                    padding: None,
                                    align_x: None,
                                    align_y: None,
                                    background: (None)
                                        .map(::ducktape_view_guest::wire::Background::Color),
                                    border: None,
                                    snap: None,
                                    content: Box::new({
                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                        if !("").is_empty()  {
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
                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                        }),
                                                    },
                                                    key: format!("{}/@text:419", use_scope),
                                                    size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: Some(palette.colors[71]),
                                                    font: ::ducktape_view_guest::wire::Font {
                                                        monospace: false,
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    },
                                                    width: None,
                                                    align_x: None,
                                                    content: ("".to_owned()).to_string(),
                                                });
                                        }
                                        ::ducktape_view_guest::wire::Node::Linear {
                                            max_width: None,
                                            clip: false,
                                            key: format!("{}/@layout:417", use_scope),
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
                                });
                            if !("").is_empty()  {
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
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:426", use_scope),
                                        size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[73]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: ("".to_owned()).to_string(),
                                    });
                            }
                            if (!("").is_empty()) && (!("").is_empty())  {
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
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:433", use_scope),
                                        size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[73]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: ("·".to_owned()).to_string(),
                                    });
                            }
                            if !("").is_empty()  {
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
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:440", use_scope),
                                        size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[73]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: ("".to_owned()).to_string(),
                                    });
                            }
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: true,
                                key: format!("{}/@layout:400", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                spacing: Some((10.0) as f32),
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: Some(::ducktape_view_guest::wire::Length::Fill),
                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                background: None,
                                border: None,
                                children: children,
                            }
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
                    key: format!("{}/@container:446", use_scope),
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                    padding: None,
                    align_x: None,
                    align_y: None,
                    background: (Some(palette.colors[60]))
                        .map(::ducktape_view_guest::wire::Background::Color),
                    border: None,
                    snap: None,
                    content: Box::new(::ducktape_view_guest::wire::Node::Space {
                        width: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                    }),
                });
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
    pub(super) fn render_forge_code_empty_23(
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
                    top: (48.0) as f32,
                    right: (48.0) as f32,
                    bottom: (48.0) as f32,
                    left: (48.0) as f32,
                }),
                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                align_y: None,
                background: (None).map(::ducktape_view_guest::wire::Background::Color),
                border: None,
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    if !("").is_empty() {
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
                            key: format!("{}/@text:475", use_scope),
                            size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[70]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("".to_owned()).to_string(),
                        });
                    }
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
                        key: format!("{}/@text:481", use_scope),
                        size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[73]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("Synced from the node · view only".to_owned()).to_string(),
                    });
                    if !("Loading repository files…").is_empty() {
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
                            key: format!("{}/@text:487", use_scope),
                            size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[73]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("Loading repository files…".to_owned()).to_string(),
                        });
                    }
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:473", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: Some((9.0) as f32),
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
    pub(super) fn render_forge_code_empty_24(
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
                    top: (48.0) as f32,
                    right: (48.0) as f32,
                    bottom: (48.0) as f32,
                    left: (48.0) as f32,
                }),
                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                align_y: None,
                background: (None).map(::ducktape_view_guest::wire::Background::Color),
                border: None,
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    if !(self.file_path).is_empty() {
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
                            key: format!("{}/@text:475", use_scope),
                            size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[70]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: (self.file_path.to_owned()).to_string(),
                        });
                    }
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
                        key: format!("{}/@text:481", use_scope),
                        size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[73]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("Synced from the node · view only".to_owned()).to_string(),
                    });
                    if !("Loading file…").is_empty() {
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
                            key: format!("{}/@text:487", use_scope),
                            size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[73]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("Loading file…".to_owned()).to_string(),
                        });
                    }
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:473", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: Some((9.0) as f32),
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
    pub(super) fn render_forge_code_empty_25(
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
                    top: (48.0) as f32,
                    right: (48.0) as f32,
                    bottom: (48.0) as f32,
                    left: (48.0) as f32,
                }),
                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                align_y: None,
                background: (None).map(::ducktape_view_guest::wire::Background::Color),
                border: None,
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    if !("").is_empty() {
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
                            key: format!("{}/@text:475", use_scope),
                            size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[70]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("".to_owned()).to_string(),
                        });
                    }
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
                        key: format!("{}/@text:481", use_scope),
                        size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[73]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("Synced from the node · view only".to_owned()).to_string(),
                    });
                    if !("Could not load code. Pick Code to try again.").is_empty() {
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
                            key: format!("{}/@text:487", use_scope),
                            size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[73]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("Could not load code. Pick Code to try again.".to_owned())
                                .to_string(),
                        });
                    }
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:473", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: Some((9.0) as f32),
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
    pub(super) fn render_forge_code_empty_26(
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
                    top: (48.0) as f32,
                    right: (48.0) as f32,
                    bottom: (48.0) as f32,
                    left: (48.0) as f32,
                }),
                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                align_y: None,
                background: (None).map(::ducktape_view_guest::wire::Background::Color),
                border: None,
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    if !(self.file_path).is_empty() {
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
                            key: format!("{}/@text:475", use_scope),
                            size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[70]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: (self.file_path.to_owned()).to_string(),
                        });
                    }
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
                        key: format!("{}/@text:481", use_scope),
                        size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[73]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("Synced from the node · view only".to_owned()).to_string(),
                    });
                    if !(self.file_note).is_empty() {
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
                            key: format!("{}/@text:487", use_scope),
                            size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[73]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: (self.file_note.to_owned()).to_string(),
                        });
                    }
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:473", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: Some((9.0) as f32),
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
    pub(super) fn render_forge_code_empty_27(
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
                    top: (48.0) as f32,
                    right: (48.0) as f32,
                    bottom: (48.0) as f32,
                    left: (48.0) as f32,
                }),
                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                align_y: None,
                background: (None).map(::ducktape_view_guest::wire::Background::Color),
                border: None,
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    if !("").is_empty() {
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
                            key: format!("{}/@text:475", use_scope),
                            size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[70]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("".to_owned()).to_string(),
                        });
                    }
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
                        key: format!("{}/@text:481", use_scope),
                        size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[73]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("Synced from the node · view only".to_owned()).to_string(),
                    });
                    if !("Nothing is committed on this repository yet, so there is no file to read.")
                        .is_empty() 
                    {
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
                                key: format!("{}/@text:487", use_scope),
                                size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[73]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("Nothing is committed on this repository yet, so there is no file to read."
                                    .to_owned())
                                    .to_string(),
                            });
                    }
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:473", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: Some((9.0) as f32),
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
    pub(super) fn render_forge_code_empty_28(
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
                    top: (48.0) as f32,
                    right: (48.0) as f32,
                    bottom: (48.0) as f32,
                    left: (48.0) as f32,
                }),
                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                align_y: None,
                background: (None).map(::ducktape_view_guest::wire::Background::Color),
                border: None,
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    if !("").is_empty() {
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
                            key: format!("{}/@text:475", use_scope),
                            size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[70]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("".to_owned()).to_string(),
                        });
                    }
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
                        key: format!("{}/@text:481", use_scope),
                        size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[73]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("Synced from the node · view only".to_owned()).to_string(),
                    });
                    if !("This commit has no files to read.").is_empty() {
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
                            key: format!("{}/@text:487", use_scope),
                            size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[73]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("This commit has no files to read.".to_owned()).to_string(),
                        });
                    }
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:473", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: Some((9.0) as f32),
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
    pub(super) fn render_forge_code_empty_29(
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
                    top: (48.0) as f32,
                    right: (48.0) as f32,
                    bottom: (48.0) as f32,
                    left: (48.0) as f32,
                }),
                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                align_y: None,
                background: (None).map(::ducktape_view_guest::wire::Background::Color),
                border: None,
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    if !("").is_empty() {
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
                            key: format!("{}/@text:475", use_scope),
                            size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[70]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("".to_owned()).to_string(),
                        });
                    }
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
                        key: format!("{}/@text:481", use_scope),
                        size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[73]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("Synced from the node · view only".to_owned()).to_string(),
                    });
                    if !("This directory has entries outside the browser's display limits.")
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
                                        "Geist".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:487", use_scope),
                            size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[73]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content:
                                ("This directory has entries outside the browser's display limits."
                                    .to_owned())
                                .to_string(),
                        });
                    }
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:473", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: Some((9.0) as f32),
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
    pub(super) fn render_forge_code_empty_30(
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
                    top: (48.0) as f32,
                    right: (48.0) as f32,
                    bottom: (48.0) as f32,
                    left: (48.0) as f32,
                }),
                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                align_y: None,
                background: (None).map(::ducktape_view_guest::wire::Background::Color),
                border: None,
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    if !("").is_empty() {
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
                            key: format!("{}/@text:475", use_scope),
                            size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[70]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("".to_owned()).to_string(),
                        });
                    }
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
                        key: format!("{}/@text:481", use_scope),
                        size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[73]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("Synced from the node · view only".to_owned()).to_string(),
                    });
                    if !("Pick a file from the tree to read it.").is_empty() {
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
                            key: format!("{}/@text:487", use_scope),
                            size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[73]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("Pick a file from the tree to read it.".to_owned())
                                .to_string(),
                        });
                    }
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:473", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: Some((9.0) as f32),
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
    pub(super) fn render_forge_code_empty_31(
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
                    top: (48.0) as f32,
                    right: (48.0) as f32,
                    bottom: (48.0) as f32,
                    left: (48.0) as f32,
                }),
                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                align_y: None,
                background: (None).map(::ducktape_view_guest::wire::Background::Color),
                border: None,
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    if !(self.file_path).is_empty() {
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
                            key: format!("{}/@text:475", use_scope),
                            size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[70]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: (self.file_path.to_owned()).to_string(),
                        });
                    }
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
                        key: format!("{}/@text:481", use_scope),
                        size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[73]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("Synced from the node · view only".to_owned()).to_string(),
                    });
                    if !(crate::host::binary_note(::std::convert::AsRef::as_ref(&(self.file_text))))
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
                                        "Geist".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:487", use_scope),
                            size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[73]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: (crate::host::binary_note(::std::convert::AsRef::as_ref(
                                &(self.file_text),
                            ))
                            .to_owned())
                            .to_string(),
                        });
                    }
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:473", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: Some((9.0) as f32),
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
    pub(super) fn render_forge_code_tab_32(
        &self,
        palette: Palette,
        use_scope: String,
        ctx_0: String,
        cb_0: impl Fn(String) -> Message + Clone + 'static,
        cb_1: impl Fn(String) -> Message + Clone + 'static,
        cb_2: impl Fn(String) -> Message + Clone + 'static,
        cb_3: impl Fn(f64, f64) -> Message + Clone + 'static,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push({
                        let node_scope = format!("{}/tree-pane", node_scope);
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
                                ::ducktape_view_guest::wire::Length::Fixed(
                                    (self.tree_width) as f32,
                                ),
                            ),
                            height: Some(::ducktape_view_guest::wire::Length::Fill),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[54]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Scroll {
                                on_scroll: None,
                                virtual_rows: false,
                                key: format!("{}/@layout:241", use_scope),
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
                                            key: format!("{}/@container:251", use_scope),
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: None,
                                            padding: Some(::ducktape_view_guest::wire::Edges {
                                                top: (5.0) as f32,
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
                                                key: format!("{}/@text:258", use_scope),
                                                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                color: Some(palette.colors[73]),
                                                font: ::ducktape_view_guest::wire::Font {
                                                    monospace: false,
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                },
                                                width: None,
                                                align_x: None,
                                                content: ("FILES".to_owned()).to_string(),
                                            }),
                                        });
                                    children
                                        .push({
                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                            if self.tree_phase == "loading"  {
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
                                                        key: format!("{}/@container:831", use_scope),
                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                        height: None,
                                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                                            top: (8.0) as f32,
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
                                                        content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                                height: None,
                                                                align_y: None,
                                                                line_height: Some(
                                                                    ::ducktape_view_guest::wire::LineHeight::Relative(
                                                                        ((1.5) as f32).max(f32::EPSILON).min(f32::MAX),
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
                                                            key: format!("{}/@text:837", use_scope),
                                                            size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                            color: Some(palette.colors[73]),
                                                            font: ::ducktape_view_guest::wire::Font {
                                                                monospace: false,
                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                            },
                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                            align_x: None,
                                                            content: ("Loading repository files…".to_owned())
                                                                .to_string(),
                                                        }),
                                                    });
                                            }
                                            if self.tree_phase == "failed"  {
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
                                                        key: format!("{}/@container:844", use_scope),
                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                        height: None,
                                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                                            top: (8.0) as f32,
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
                                                        content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                                height: None,
                                                                align_y: None,
                                                                line_height: Some(
                                                                    ::ducktape_view_guest::wire::LineHeight::Relative(
                                                                        ((1.5) as f32).max(f32::EPSILON).min(f32::MAX),
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
                                                            key: format!("{}/@text:850", use_scope),
                                                            size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                            color: Some(palette.colors[73]),
                                                            font: ::ducktape_view_guest::wire::Font {
                                                                monospace: false,
                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                            },
                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                            align_x: None,
                                                            content: ("Could not load code. Pick Code to try again."
                                                                .to_owned())
                                                                .to_string(),
                                                        }),
                                                    });
                                            }
                                            if self.tree_phase == "ready"  {
                                                children
                                                    .push({
                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                        if (self.tree_entries).is_empty() && (!self.tree_born)  {
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
                                                                    key: format!("{}/@container:859", use_scope),
                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                    height: None,
                                                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                                                        top: (8.0) as f32,
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
                                                                    content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                                                        options: ::ducktape_view_guest::wire::TextOptions {
                                                                            height: None,
                                                                            align_y: None,
                                                                            line_height: Some(
                                                                                ::ducktape_view_guest::wire::LineHeight::Relative(
                                                                                    ((1.5) as f32).max(f32::EPSILON).min(f32::MAX),
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
                                                                        key: format!("{}/@text:865", use_scope),
                                                                        size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                        color: Some(palette.colors[73]),
                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                            monospace: false,
                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                        },
                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                        align_x: None,
                                                                        content: ("Nothing committed on this repository yet."
                                                                            .to_owned())
                                                                            .to_string(),
                                                                    }),
                                                                });
                                                        }
                                                        if ((self.tree_entries).is_empty() && self.tree_born)
                                                            && (!self.tree_truncated) 
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
                                                                    key: format!("{}/@container:872", use_scope),
                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                    height: None,
                                                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                                                        top: (8.0) as f32,
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
                                                                    content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                                                        options: ::ducktape_view_guest::wire::TextOptions {
                                                                            height: None,
                                                                            align_y: None,
                                                                            line_height: Some(
                                                                                ::ducktape_view_guest::wire::LineHeight::Relative(
                                                                                    ((1.5) as f32).max(f32::EPSILON).min(f32::MAX),
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
                                                                        key: format!("{}/@text:878", use_scope),
                                                                        size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                        color: Some(palette.colors[73]),
                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                            monospace: false,
                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                        },
                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                        align_x: None,
                                                                        content: ("No files in this commit.".to_owned()).to_string(),
                                                                    }),
                                                                });
                                                        }
                                                        if ((self.tree_entries).is_empty() && self.tree_born)
                                                            && self.tree_truncated 
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
                                                                    key: format!("{}/@container:885", use_scope),
                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                    height: None,
                                                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                                                        top: (8.0) as f32,
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
                                                                    content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                                                        options: ::ducktape_view_guest::wire::TextOptions {
                                                                            height: None,
                                                                            align_y: None,
                                                                            line_height: Some(
                                                                                ::ducktape_view_guest::wire::LineHeight::Relative(
                                                                                    ((1.5) as f32).max(f32::EPSILON).min(f32::MAX),
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
                                                                        key: format!("{}/@text:891", use_scope),
                                                                        size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                        color: Some(palette.colors[73]),
                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                            monospace: false,
                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                        },
                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                        align_x: None,
                                                                        content: ("This directory has entries that cannot be shown."
                                                                            .to_owned())
                                                                            .to_string(),
                                                                    }),
                                                                });
                                                        }
                                                        if !(self.tree_path).is_empty()  {
                                                            children
                                                                .push(::ducktape_view_guest::wire::Node::Button {
                                                                    checked: None,
                                                                    expanded: None,
                                                                    description: None,
                                                                    key: format!("{}/@button:898", use_scope),
                                                                    content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                                        Box::new(
                                                                            self
                                                                                .render_forge_tree_dir_row_16(
                                                                                    palette,
                                                                                    format!("{}/ForgeTreeDirRow@3647", use_scope),
                                                                                ),
                                                                        ),
                                                                    ),
                                                                    label: Some(
                                                                        String::from("Back to the repository root".to_owned()),
                                                                    ),
                                                                    on_press: Some(
                                                                        ::ducktape_view_guest::slots::message((cb_0)("".to_owned())),
                                                                    ),
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
                                                                            background: Some(palette.colors[58]),
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
                                                        }
                                                        for (index, entry) in self.tree_entries.iter().enumerate() {
                                                            let for_scope = format!(
                                                                "{}/@for:3655({})", use_scope, index
                                                            );
                                                            children
                                                                .push({
                                                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                    if entry.kind == "dir"  {
                                                                        children
                                                                            .push(::ducktape_view_guest::wire::Node::Button {
                                                                                checked: None,
                                                                                expanded: None,
                                                                                description: Some(String::from(entry.path.to_owned())),
                                                                                key: format!("{}/@button:915", for_scope),
                                                                                content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                                                    Box::new(
                                                                                        self
                                                                                            .render_forge_tree_dir_row_17(
                                                                                                palette,
                                                                                                format!("{}/ForgeTreeDirRow@3665", for_scope),
                                                                                                entry.name.to_owned(),
                                                                                            ),
                                                                                    ),
                                                                                ),
                                                                                label: Some(String::from("Open directory".to_owned())),
                                                                                on_press: Some(
                                                                                    ::ducktape_view_guest::slots::message(
                                                                                        (cb_0)(entry.path.to_owned()),
                                                                                    ),
                                                                                ),
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
                                                                                        background: Some(palette.colors[58]),
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
                                                                    }
                                                                    if entry.kind != "dir"  {
                                                                        children
                                                                            .push(::ducktape_view_guest::wire::Node::Button {
                                                                                checked: None,
                                                                                expanded: None,
                                                                                description: Some(String::from(entry.path.to_owned())),
                                                                                key: format!("{}/@button:931", for_scope),
                                                                                content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                                                    Box::new(
                                                                                        self
                                                                                            .render_forge_tree_file_row_21(
                                                                                                palette,
                                                                                                format!("{}/ForgeTreeFileRow@3681", for_scope),
                                                                                                entry.name.to_owned(),
                                                                                                entry.path
                                                                                                    == crate::host::forge_file_header(
                                                                                                        ::std::convert::AsRef::as_ref(&(self.opened_dir)),
                                                                                                        ::std::convert::AsRef::as_ref(&(self.opened_rev)),
                                                                                                        ::std::convert::AsRef::as_ref(&(self.tree_path)),
                                                                                                        ::std::convert::AsRef::as_ref(&(self.tree_rev)),
                                                                                                        ::std::convert::AsRef::as_ref(&(self.file_path)),
                                                                                                    ) ,
                                                                                            ),
                                                                                    ),
                                                                                ),
                                                                                label: Some(String::from("Open file".to_owned())),
                                                                                on_press: Some(
                                                                                    ::ducktape_view_guest::slots::message(
                                                                                        (cb_1)(entry.path.to_owned()),
                                                                                    ),
                                                                                ),
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
                                                                                        background: Some(palette.colors[58]),
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
                                                                    }
                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                        max_width: None,
                                                                        clip: false,
                                                                        key: format!("{}/@layout:913", for_scope),
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
                                                                });
                                                        }
                                                        if self.tree_truncated {
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
                                                                    key: format!("{}/@container:947", use_scope),
                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                    height: None,
                                                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                                                        top: (12.0) as f32,
                                                                        right: (12.0) as f32,
                                                                        bottom: (12.0) as f32,
                                                                        left: (12.0) as f32,
                                                                    }),
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
                                                                            line_height: Some(
                                                                                ::ducktape_view_guest::wire::LineHeight::Relative(
                                                                                    ((1.5) as f32).max(f32::EPSILON).min(f32::MAX),
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
                                                                        key: format!("{}/@text:948", use_scope),
                                                                        size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                        color: Some(palette.colors[73]),
                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                            monospace: false,
                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                        },
                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                        align_x: None,
                                                                        content: ("Some entries are not shown.".to_owned())
                                                                            .to_string(),
                                                                    }),
                                                                });
                                                        }
                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                            max_width: None,
                                                            clip: false,
                                                            key: format!("{}/@layout:857", use_scope),
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
                                                    });
                                            }
                                            ::ducktape_view_guest::wire::Node::Linear {
                                                max_width: None,
                                                clip: false,
                                                key: format!("{}/@layout:829", use_scope),
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
                                        });
                                    ::ducktape_view_guest::wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:246", use_scope),
                                        wrap: None,
                                        axis: ::ducktape_view_guest::wire::Axis::Column,
                                        spacing: None,
                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                            top: (9.0) as f32,
                                            right: (0.0) as f32,
                                            bottom: (9.0) as f32,
                                            left: (0.0) as f32,
                                        }),
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: None,
                                        align: None,
                                        background: None,
                                        border: None,
                                        children: children,
                                    }
                                }),
                            }),
                        }
                    });
                children.push({
                    let node_scope = format!("{}/tree-resize", node_scope);
                    ::ducktape_view_guest::wire::Node::ResizeHandle {
                        key: node_scope.clone(),
                        on_press: None,
                        on_release: None,
                        on_drag: Some(
                            ::ducktape_view_guest::slots::handler::<(f64, f64), Message>(Box::new(
                                {
                                    let route = {
                                        let route_callback = (cb_3).clone();
                                        move |delta: (f64, f64)| (route_callback)(delta.0, delta.1)
                                    };
                                    move |sent: (f64, f64)| Some(route(sent))
                                },
                            )),
                        ),
                        cursor: Some(
                            ::ducktape_view_guest::wire::mouse::Cursor::ResizingHorizontally,
                        ),
                        content: Box::new({
                            let node_scope = format!("{}/tree-divider", node_scope);
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
                                width: Some(::ducktape_view_guest::wire::Length::Fixed(
                                    (10.0) as f32,
                                )),
                                height: Some(::ducktape_view_guest::wire::Length::Fill),
                                padding: None,
                                align_x: Some(::ducktape_view_guest::wire::AlignX::Left),
                                align_y: None,
                                background: (None)
                                    .map(::ducktape_view_guest::wire::Background::Color),
                                border: None,
                                snap: None,
                                content: Box::new(::ducktape_view_guest::wire::Node::Container {
                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                        color: None,
                                        x: None,
                                        y: None,
                                        blur: None,
                                    },
                                    max_width: None,
                                    max_height: None,
                                    clip: false,
                                    key: format!("{}/@container:271", use_scope),
                                    width: Some(::ducktape_view_guest::wire::Length::Fixed(
                                        (1.0) as f32,
                                    )),
                                    height: Some(::ducktape_view_guest::wire::Length::Fill),
                                    padding: None,
                                    align_x: None,
                                    align_y: None,
                                    background: (Some(palette.colors[60]))
                                        .map(::ducktape_view_guest::wire::Background::Color),
                                    border: None,
                                    snap: None,
                                    content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                        width: Some(::ducktape_view_guest::wire::Length::Fixed(
                                            (1.0) as f32,
                                        )),
                                        height: Some(::ducktape_view_guest::wire::Length::Fixed(
                                            (1.0) as f32,
                                        )),
                                    }),
                                }),
                            }
                        }),
                    }
                });
                children
                    .push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        children
                            .push(
                                self
                                    .render_forge_code_header_22(
                                        palette,
                                        format!("{}/ForgeCodeHeader@1442", use_scope),
                                    ),
                            );
                        children
                            .push(::ducktape_view_guest::wire::Node::Scroll {
                                on_scroll: None,
                                virtual_rows: false,
                                key: format!("{}/@layout:280", use_scope),
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
                                    if self.tree_phase == "loading"  {
                                        children
                                            .push(
                                                self
                                                    .render_forge_code_empty_23(
                                                        palette,
                                                        format!("{}/ForgeCodeEmpty@3701", use_scope),
                                                    ),
                                            );
                                    }
                                    if (self.file_phase == "loading")
                                        && (!(crate::host::forge_file_header(
                                            ::std::convert::AsRef::as_ref(&(self.opened_dir)),
                                            ::std::convert::AsRef::as_ref(&(self.opened_rev)),
                                            ::std::convert::AsRef::as_ref(&(self.tree_path)),
                                            ::std::convert::AsRef::as_ref(&(self.tree_rev)),
                                            ::std::convert::AsRef::as_ref(&(self.file_path)),
                                        ))
                                            .is_empty()) 
                                    {
                                        children
                                            .push(
                                                self
                                                    .render_forge_code_empty_24(
                                                        palette,
                                                        format!("{}/ForgeCodeEmpty@3703", use_scope),
                                                    ),
                                            );
                                    }
                                    if self.tree_phase == "failed"  {
                                        children
                                            .push(
                                                self
                                                    .render_forge_code_empty_25(
                                                        palette,
                                                        format!("{}/ForgeCodeEmpty@3705", use_scope),
                                                    ),
                                            );
                                    }
                                    if (self.file_phase == "failed")
                                        && (!(crate::host::forge_file_header(
                                            ::std::convert::AsRef::as_ref(&(self.opened_dir)),
                                            ::std::convert::AsRef::as_ref(&(self.opened_rev)),
                                            ::std::convert::AsRef::as_ref(&(self.tree_path)),
                                            ::std::convert::AsRef::as_ref(&(self.tree_rev)),
                                            ::std::convert::AsRef::as_ref(&(self.file_path)),
                                        ))
                                            .is_empty()) 
                                    {
                                        children
                                            .push(
                                                self
                                                    .render_forge_code_empty_26(
                                                        palette,
                                                        format!("{}/ForgeCodeEmpty@3707", use_scope),
                                                    ),
                                            );
                                    }
                                    if (((self.tree_phase == "ready")
                                        && (crate::host::forge_file_header(
                                            ::std::convert::AsRef::as_ref(&(self.opened_dir)),
                                            ::std::convert::AsRef::as_ref(&(self.opened_rev)),
                                            ::std::convert::AsRef::as_ref(&(self.tree_path)),
                                            ::std::convert::AsRef::as_ref(&(self.tree_rev)),
                                            ::std::convert::AsRef::as_ref(&(self.file_path)),
                                        ))
                                            .is_empty()) && (self.tree_entries).is_empty())
                                        && (!self.tree_born) 
                                    {
                                        children
                                            .push(
                                                self
                                                    .render_forge_code_empty_27(
                                                        palette,
                                                        format!("{}/ForgeCodeEmpty@3709", use_scope),
                                                    ),
                                            );
                                    }
                                    if ((((self.tree_phase == "ready")
                                        && (crate::host::forge_file_header(
                                            ::std::convert::AsRef::as_ref(&(self.opened_dir)),
                                            ::std::convert::AsRef::as_ref(&(self.opened_rev)),
                                            ::std::convert::AsRef::as_ref(&(self.tree_path)),
                                            ::std::convert::AsRef::as_ref(&(self.tree_rev)),
                                            ::std::convert::AsRef::as_ref(&(self.file_path)),
                                        ))
                                            .is_empty()) && (self.tree_entries).is_empty())
                                        && self.tree_born) && (!self.tree_truncated) 
                                    {
                                        children
                                            .push(
                                                self
                                                    .render_forge_code_empty_28(
                                                        palette,
                                                        format!("{}/ForgeCodeEmpty@3714", use_scope),
                                                    ),
                                            );
                                    }
                                    if ((((self.tree_phase == "ready")
                                        && (crate::host::forge_file_header(
                                            ::std::convert::AsRef::as_ref(&(self.opened_dir)),
                                            ::std::convert::AsRef::as_ref(&(self.opened_rev)),
                                            ::std::convert::AsRef::as_ref(&(self.tree_path)),
                                            ::std::convert::AsRef::as_ref(&(self.tree_rev)),
                                            ::std::convert::AsRef::as_ref(&(self.file_path)),
                                        ))
                                            .is_empty()) && (self.tree_entries).is_empty())
                                        && self.tree_born) && self.tree_truncated 
                                    {
                                        children
                                            .push(
                                                self
                                                    .render_forge_code_empty_29(
                                                        palette,
                                                        format!("{}/ForgeCodeEmpty@3716", use_scope),
                                                    ),
                                            );
                                    }
                                    if ((self.tree_phase == "ready")
                                        && (crate::host::forge_file_header(
                                            ::std::convert::AsRef::as_ref(&(self.opened_dir)),
                                            ::std::convert::AsRef::as_ref(&(self.opened_rev)),
                                            ::std::convert::AsRef::as_ref(&(self.tree_path)),
                                            ::std::convert::AsRef::as_ref(&(self.tree_rev)),
                                            ::std::convert::AsRef::as_ref(&(self.file_path)),
                                        ))
                                            .is_empty()) && (!(self.tree_entries).is_empty()) 
                                    {
                                        children
                                            .push(
                                                self
                                                    .render_forge_code_empty_30(
                                                        palette,
                                                        format!("{}/ForgeCodeEmpty@3721", use_scope),
                                                    ),
                                            );
                                    }
                                    if ((self.file_phase == "ready")
                                        && (!(crate::host::forge_file_header(
                                            ::std::convert::AsRef::as_ref(&(self.opened_dir)),
                                            ::std::convert::AsRef::as_ref(&(self.opened_rev)),
                                            ::std::convert::AsRef::as_ref(&(self.tree_path)),
                                            ::std::convert::AsRef::as_ref(&(self.tree_rev)),
                                            ::std::convert::AsRef::as_ref(&(self.file_path)),
                                        ))
                                            .is_empty())) && self.file_binary 
                                    {
                                        children
                                            .push(
                                                self
                                                    .render_forge_code_empty_31(
                                                        palette,
                                                        format!("{}/ForgeCodeEmpty@3726", use_scope),
                                                    ),
                                            );
                                    }
                                    if (((self.file_phase == "ready")
                                        && (!(crate::host::forge_file_header(
                                            ::std::convert::AsRef::as_ref(&(self.opened_dir)),
                                            ::std::convert::AsRef::as_ref(&(self.opened_rev)),
                                            ::std::convert::AsRef::as_ref(&(self.tree_path)),
                                            ::std::convert::AsRef::as_ref(&(self.tree_rev)),
                                            ::std::convert::AsRef::as_ref(&(self.file_path)),
                                        ))
                                            .is_empty())) && (!self.file_binary)) && self.file_picture 
                                    {
                                        children
                                            .push({
                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                children
                                                    .push({
                                                        let node_scope = format!("{}/forge-picture", node_scope);
                                                        ::ducktape_view_guest::wire::Node::Surface {
                                                            key: node_scope.clone(),
                                                            name: String::from("picture"),
                                                            args: ::std::vec![
                                                                { let surface_arg = & ("forge".to_owned());
                                                                ::ducktape_view_guest::wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                                                                }, { let surface_arg = & (self.file_path.to_owned());
                                                                ::ducktape_view_guest::wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                                                                }
                                                            ],
                                                            on_event: None,
                                                        }
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
                                                        key: format!("{}/@text:996", use_scope),
                                                        size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                        color: Some(palette.colors[73]),
                                                        font: ::ducktape_view_guest::wire::Font {
                                                            monospace: false,
                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                        },
                                                        width: None,
                                                        align_x: None,
                                                        content: (crate::host::picture_caption(
                                                            self.file_width,
                                                            self.file_height,
                                                        ))
                                                            .to_string(),
                                                    });
                                                ::ducktape_view_guest::wire::Node::Linear {
                                                    max_width: None,
                                                    clip: false,
                                                    key: format!("{}/@layout:988", use_scope),
                                                    wrap: None,
                                                    axis: ::ducktape_view_guest::wire::Axis::Column,
                                                    spacing: Some((9.0) as f32),
                                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                                        top: (13.0) as f32,
                                                        right: (16.0) as f32,
                                                        bottom: (13.0) as f32,
                                                        left: (16.0) as f32,
                                                    }),
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    height: None,
                                                    align: None,
                                                    background: None,
                                                    border: None,
                                                    children: children,
                                                }
                                            });
                                    }
                                    if ((((self.file_phase == "ready")
                                        && (!(crate::host::forge_file_header(
                                            ::std::convert::AsRef::as_ref(&(self.opened_dir)),
                                            ::std::convert::AsRef::as_ref(&(self.opened_rev)),
                                            ::std::convert::AsRef::as_ref(&(self.tree_path)),
                                            ::std::convert::AsRef::as_ref(&(self.tree_rev)),
                                            ::std::convert::AsRef::as_ref(&(self.file_path)),
                                        ))
                                            .is_empty())) && (!self.file_binary))
                                        && (!self.file_picture))
                                        && crate::host::markdown_path(
                                            ::std::convert::AsRef::as_ref(&(self.file_path)),
                                        ) 
                                    {
                                        children
                                            .push({
                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                children
                                                    .push({
                                                        let _lazy_context_739 = (ctx_0).to_owned();
                                                        let lazy_event_739_0 = (cb_2).clone();
                                                        let _lazy_event_739_1 = (cb_0).clone();
                                                        let _lazy_event_739_2 = (cb_1).clone();
                                                        let _lazy_event_739_3 = (cb_3).clone();
                                                        {
                                                            let lazy_key = format!("{}/@lazy:1023", use_scope);
                                                            ::ducktape_view_guest::memo_lazy(
                                                                (
                                                                    self.file_text.to_owned(),
                                                                    self.file_path.to_owned(),
                                                                    self.dark,
                                                                    self.file_text_revision,
                                                                    (node_scope).to_owned(),
                                                                    palette.name,
                                                                ),
                                                                move |dependency| {
                                                                    let _file_text: String = dependency.0.clone();
                                                                    let file_path: String = dependency.1.clone();
                                                                    let dark: bool = dependency.2.clone();
                                                                    let lazy_scope = dependency.4.clone();
                                                                    let cached_doc: String = self.file_text.to_owned();
                                                                    {
                                                                        let node_scope = format!("{}/forge-markdown", lazy_scope);
                                                                        ::ducktape_view_guest::wire::Node::Surface {
                                                                            key: node_scope.clone(),
                                                                            name: String::from("forge_markdown"),
                                                                            args: ::std::vec![
                                                                                { let surface_arg = & (cached_doc.to_owned());
                                                                                ::ducktape_view_guest::wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                                                                                }, { let surface_arg = & (file_path.to_owned());
                                                                                ::ducktape_view_guest::wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                                                                                }, { let surface_arg = & (dark);
                                                                                ::ducktape_view_guest::wire::SurfaceValue::Bool(*
                                                                                (surface_arg)) }
                                                                            ],
                                                                            on_event: Some(
                                                                                ::ducktape_view_guest::slots::handler::<
                                                                                    ::ducktape_view_guest::wire::SurfaceValue,
                                                                                    Message,
                                                                                >(
                                                                                    Box::new({
                                                                                        let route = {
                                                                                            let route_callback = (lazy_event_739_0).clone();
                                                                                            move |value| (route_callback)(value)
                                                                                        };
                                                                                        move |sent| {
                                                                                            (match sent {
                                                                                                ::ducktape_view_guest::wire::SurfaceValue::Str(item) => {
                                                                                                    Some(item)
                                                                                                }
                                                                                                _ => None,
                                                                                            })
                                                                                                .map(&route)
                                                                                        }
                                                                                    }),
                                                                                ),
                                                                            ),
                                                                        }
                                                                    }
                                                                },
                                                                739u64,
                                                                &(use_scope),
                                                                lazy_key,
                                                            )
                                                        }
                                                    });
                                                if self.file_truncated {
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
                                                            key: format!("{}/@text:1026", use_scope),
                                                            size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                            color: Some(palette.colors[73]),
                                                            font: ::ducktape_view_guest::wire::Font {
                                                                monospace: false,
                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                            },
                                                            width: None,
                                                            align_x: None,
                                                            content: ("This file is larger than the 64 KiB preview limit."
                                                                .to_owned())
                                                                .to_string(),
                                                        });
                                                }
                                                ::ducktape_view_guest::wire::Node::Linear {
                                                    max_width: None,
                                                    clip: false,
                                                    key: format!("{}/@layout:1013", use_scope),
                                                    wrap: None,
                                                    axis: ::ducktape_view_guest::wire::Axis::Column,
                                                    spacing: Some((9.0) as f32),
                                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                                        top: (13.0) as f32,
                                                        right: (16.0) as f32,
                                                        bottom: (13.0) as f32,
                                                        left: (16.0) as f32,
                                                    }),
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    height: None,
                                                    align: None,
                                                    background: None,
                                                    border: None,
                                                    children: children,
                                                }
                                            });
                                    }
                                    if ((((self.file_phase == "ready")
                                        && (!(crate::host::forge_file_header(
                                            ::std::convert::AsRef::as_ref(&(self.opened_dir)),
                                            ::std::convert::AsRef::as_ref(&(self.opened_rev)),
                                            ::std::convert::AsRef::as_ref(&(self.tree_path)),
                                            ::std::convert::AsRef::as_ref(&(self.tree_rev)),
                                            ::std::convert::AsRef::as_ref(&(self.file_path)),
                                        ))
                                            .is_empty())) && (!self.file_binary))
                                        && (!self.file_picture))
                                        && (!crate::host::markdown_path(
                                            ::std::convert::AsRef::as_ref(&(self.file_path)),
                                        )) 
                                    {
                                        children
                                            .push({
                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                children
                                                    .push({
                                                        let _lazy_context_745 = (ctx_0).to_owned();
                                                        let _lazy_event_745_0 = (cb_2).clone();
                                                        let _lazy_event_745_1 = (cb_0).clone();
                                                        let _lazy_event_745_2 = (cb_1).clone();
                                                        let _lazy_event_745_3 = (cb_3).clone();
                                                        {
                                                            let lazy_key = format!("{}/@lazy:1056", use_scope);
                                                            ::ducktape_view_guest::memo_lazy(
                                                                (
                                                                    self.file_text.to_owned(),
                                                                    self.file_path.to_owned(),
                                                                    self.dark,
                                                                    self.file_text_revision,
                                                                    (node_scope).to_owned(),
                                                                    palette.name,
                                                                ),
                                                                move |dependency| {
                                                                    let _file_text: String = dependency.0.clone();
                                                                    let file_path: String = dependency.1.clone();
                                                                    let dark: bool = dependency.2.clone();
                                                                    let lazy_scope = dependency.4.clone();
                                                                    let cached_source: String = self.file_text.to_owned();
                                                                    {
                                                                        let node_scope = format!("{}/forge-code", lazy_scope);
                                                                        ::ducktape_view_guest::wire::Node::Surface {
                                                                            key: node_scope.clone(),
                                                                            name: String::from("forge_code"),
                                                                            args: ::std::vec![
                                                                                { let surface_arg = & (cached_source.to_owned());
                                                                                ::ducktape_view_guest::wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                                                                                }, { let surface_arg = & (file_path.to_owned());
                                                                                ::ducktape_view_guest::wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                                                                                }, { let surface_arg = & (dark);
                                                                                ::ducktape_view_guest::wire::SurfaceValue::Bool(*
                                                                                (surface_arg)) }
                                                                            ],
                                                                            on_event: None,
                                                                        }
                                                                    }
                                                                },
                                                                745u64,
                                                                &(use_scope),
                                                                lazy_key,
                                                            )
                                                        }
                                                    });
                                                if self.file_truncated {
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
                                                            key: format!("{}/@text:1059", use_scope),
                                                            size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                            color: Some(palette.colors[73]),
                                                            font: ::ducktape_view_guest::wire::Font {
                                                                monospace: false,
                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                            },
                                                            width: None,
                                                            align_x: None,
                                                            content: ("This file is larger than the 64 KiB preview limit."
                                                                .to_owned())
                                                                .to_string(),
                                                        });
                                                }
                                                ::ducktape_view_guest::wire::Node::Linear {
                                                    max_width: None,
                                                    clip: false,
                                                    key: format!("{}/@layout:1032", use_scope),
                                                    wrap: None,
                                                    axis: ::ducktape_view_guest::wire::Axis::Column,
                                                    spacing: Some((9.0) as f32),
                                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                                        top: (13.0) as f32,
                                                        right: (0.0) as f32,
                                                        bottom: (13.0) as f32,
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
                                    }
                                    ::ducktape_view_guest::wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:956", use_scope),
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
                            });
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:273", use_scope),
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
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Row,
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
    pub(super) fn render_pr_state_plate_39(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if arg_0 == "open" {
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
                        key: format!("{}/@container:587", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fixed((24.0) as f32)),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((24.0) as f32)),
                        padding: None,
                        align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                        align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                        background: (Some(palette.colors[27]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: Some(::ducktape_view_guest::wire::Border {
                            color: Some(palette.colors[28]),
                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                            radius: Some([
                                ((7.0) as f32).max(0.0).min(f32::MAX),
                                ((7.0) as f32).max(0.0).min(f32::MAX),
                                ((7.0) as f32).max(0.0).min(f32::MAX),
                                ((7.0) as f32).max(0.0).min(f32::MAX),
                            ]),
                        }),
                        snap: None,
                        content: Box::new(self.icon(
                            format!("{}/Icon@1765", use_scope),
                            "pull-request",
                            13f32,
                            (palette).colors[25usize],
                            "@media:58",
                        )),
                    });
                }
                if (!(arg_0 == "open")) && (arg_0 == "merged") {
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
                        key: format!("{}/@container:603", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fixed((24.0) as f32)),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((24.0) as f32)),
                        padding: None,
                        align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                        align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                        background: (Some(palette.colors[104]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: Some(::ducktape_view_guest::wire::Border {
                            color: Some(palette.colors[105]),
                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                            radius: Some([
                                ((7.0) as f32).max(0.0).min(f32::MAX),
                                ((7.0) as f32).max(0.0).min(f32::MAX),
                                ((7.0) as f32).max(0.0).min(f32::MAX),
                                ((7.0) as f32).max(0.0).min(f32::MAX),
                            ]),
                        }),
                        snap: None,
                        content: Box::new(self.icon(
                            format!("{}/Icon@1781", use_scope),
                            "pull-request",
                            13f32,
                            (palette).colors[5usize],
                            "@media:82",
                        )),
                    });
                }
                if !((arg_0 == "open") || (arg_0 == "merged")) {
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
                        key: format!("{}/@container:619", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fixed((24.0) as f32)),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((24.0) as f32)),
                        padding: None,
                        align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                        align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                        background: (Some(palette.colors[55]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: Some(::ducktape_view_guest::wire::Border {
                            color: Some(palette.colors[39]),
                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                            radius: Some([
                                ((7.0) as f32).max(0.0).min(f32::MAX),
                                ((7.0) as f32).max(0.0).min(f32::MAX),
                                ((7.0) as f32).max(0.0).min(f32::MAX),
                                ((7.0) as f32).max(0.0).min(f32::MAX),
                            ]),
                        }),
                        snap: None,
                        content: Box::new(self.icon(
                            format!("{}/Icon@1797", use_scope),
                            "pull-request",
                            13f32,
                            (palette).colors[5usize],
                            "@media:82",
                        )),
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
    pub(super) fn render_issue_state_glyph_42(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if arg_0 == "open" {
                    children.push(self.icon(
                        format!("{}/Icon@1809", use_scope),
                        "issue-open",
                        17f32,
                        (palette).colors[25usize],
                        "@media:58",
                    ));
                }
                if !(arg_0 == "open") {
                    children.push(self.icon(
                        format!("{}/Icon@1815", use_scope),
                        "issue-closed",
                        17f32,
                        (palette).colors[5usize],
                        "@media:82",
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
    pub(super) fn render_tracker_row_43(
        &self,
        palette: Palette,
        use_scope: String,
        cb_0: impl Fn(i64) -> Message + Clone + 'static,
        arg_0: crate::host::ForgeItem,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push(::ducktape_view_guest::wire::Node::Button {
                        checked: None,
                        expanded: None,
                        description: Some(String::from(arg_0.title.to_owned())),
                        key: format!("{}/@button:514", use_scope),
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
                                key: format!("{}/@container:521", use_scope),
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                padding: Some(::ducktape_view_guest::wire::Edges {
                                    top: (13.0) as f32,
                                    right: (24.0) as f32,
                                    bottom: (13.0) as f32,
                                    left: (24.0) as f32,
                                }),
                                align_x: None,
                                align_y: None,
                                background: (None)
                                    .map(::ducktape_view_guest::wire::Background::Color),
                                border: None,
                                snap: None,
                                content: Box::new({
                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                    if arg_0.kind == "pr"  {
                                        children
                                            .push(
                                                self
                                                    .render_pr_state_plate_39(
                                                        palette,
                                                        format!("{}/PrStatePlate@1703", use_scope),
                                                        arg_0.state.to_owned(),
                                                    ),
                                            );
                                    }
                                    if (!(arg_0.kind == "pr")) && (arg_0.kind == "issue")  {
                                        children
                                            .push(
                                                self
                                                    .render_issue_state_glyph_42(
                                                        palette,
                                                        format!("{}/IssueStateGlyph@1705", use_scope),
                                                        arg_0.state.to_owned(),
                                                    ),
                                            );
                                    }
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
                                                    key: format!("{}/@text:539", use_scope),
                                                    size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: Some(palette.colors[7]),
                                                    font: ::ducktape_view_guest::wire::Font {
                                                        monospace: false,
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    },
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    align_x: None,
                                                    content: (arg_0.title.to_owned()).to_string(),
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
                                                                                "Geist Mono".into(),
                                                                            ),
                                                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                        }),
                                                                    },
                                                                    key: format!("{}/@text:548", use_scope),
                                                                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                    color: Some(palette.colors[71]),
                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                        monospace: false,
                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                    },
                                                                    width: None,
                                                                    align_x: None,
                                                                    content: ("#".to_owned()).to_string(),
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
                                                                    key: format!("{}/@text:554", use_scope),
                                                                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                    color: Some(palette.colors[71]),
                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                        monospace: false,
                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                    },
                                                                    width: None,
                                                                    align_x: None,
                                                                    content: (arg_0.number).to_string(),
                                                                });
                                                            ::ducktape_view_guest::wire::Node::Linear {
                                                                max_width: None,
                                                                clip: false,
                                                                key: format!("{}/@layout:547", use_scope),
                                                                wrap: None,
                                                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                spacing: Some((0.0) as f32),
                                                                padding: None,
                                                                width: None,
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
                                                            key: format!("{}/@text:560", use_scope),
                                                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                            color: Some(palette.colors[71]),
                                                            font: ::ducktape_view_guest::wire::Font {
                                                                monospace: false,
                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                            },
                                                            width: None,
                                                            align_x: None,
                                                            content: ("· opened by".to_owned()).to_string(),
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
                                                            key: format!("{}/@text:566", use_scope),
                                                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                            color: Some(palette.colors[71]),
                                                            font: ::ducktape_view_guest::wire::Font {
                                                                monospace: false,
                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                            },
                                                            width: None,
                                                            align_x: None,
                                                            content: (arg_0.author_name.to_owned()).to_string(),
                                                        });
                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                        max_width: None,
                                                        clip: false,
                                                        key: format!("{}/@layout:546", use_scope),
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
                                            ::ducktape_view_guest::wire::Node::Linear {
                                                max_width: None,
                                                clip: false,
                                                key: format!("{}/@layout:538", use_scope),
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
                                    ::ducktape_view_guest::wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:528", use_scope),
                                        wrap: None,
                                        axis: ::ducktape_view_guest::wire::Axis::Row,
                                        spacing: Some((13.0) as f32),
                                        padding: None,
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: None,
                                        align: Some(::ducktape_view_guest::wire::AlignX::Left),
                                        background: None,
                                        border: None,
                                        children: children,
                                    }
                                }),
                            }),
                        ),
                        label: Some(String::from("Open item".to_owned())),
                        on_press: Some(
                            ::ducktape_view_guest::slots::message((cb_0)(arg_0.number)),
                        ),
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
                    key: format!("{}/@container:575", use_scope),
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                    padding: None,
                    align_x: None,
                    align_y: None,
                    background: (Some(palette.colors[55]))
                        .map(::ducktape_view_guest::wire::Background::Color),
                    border: None,
                    snap: None,
                    content: Box::new(::ducktape_view_guest::wire::Node::Space {
                        width: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                    }),
                });
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
    pub(super) fn render_forge_tracker_list_44(
        &self,
        palette: Palette,
        use_scope: String,
        cb_10: impl Fn(i64) -> Message + Clone + 'static,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if self.repo_phase == "idle" {
                    children.push(::ducktape_view_guest::wire::Node::Space {
                        width: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                    });
                }
                if (!(self.repo_phase == "idle")) && (self.repo_phase == "loading") {
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
                        key: format!("{}/@container:1546", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
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
                        content: Box::new(self.render_empty_plate_34(
                            palette,
                            format!("{}/EmptyPlate@2715", use_scope),
                        )),
                    });
                }
                if (!((self.repo_phase == "idle") || (self.repo_phase == "loading")))
                    && (self.repo_phase == "failed")
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
                        key: format!("{}/@container:1549", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
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
                        content: Box::new(self.render_empty_plate_35(
                            palette,
                            format!("{}/EmptyPlate@2718", use_scope),
                        )),
                    });
                }
                if (!(((self.repo_phase == "idle") || (self.repo_phase == "loading"))
                    || (self.repo_phase == "failed")))
                    && (self.repo_phase == "ready")
                {
                    children.push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        if (crate::host::filter_forge_items(
                            ::std::convert::AsRef::as_ref(&(self.items)),
                            ::std::convert::AsRef::as_ref(&("issues")),
                        ))
                        .is_empty()
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
                                key: format!("{}/@container:1556", use_scope),
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                padding: Some(::ducktape_view_guest::wire::Edges {
                                    top: (22.0) as f32,
                                    right: (22.0) as f32,
                                    bottom: (22.0) as f32,
                                    left: (22.0) as f32,
                                }),
                                align_x: None,
                                align_y: None,
                                background: (None)
                                    .map(::ducktape_view_guest::wire::Background::Color),
                                border: None,
                                snap: None,
                                content: Box::new(self.render_empty_plate_36(
                                    palette,
                                    format!("{}/EmptyPlate@2725", use_scope),
                                )),
                            });
                        }
                        if !(crate::host::filter_forge_items(
                            ::std::convert::AsRef::as_ref(&(self.items)),
                            ::std::convert::AsRef::as_ref(&("issues")),
                        ))
                        .is_empty()
                        {
                            children.push(::ducktape_view_guest::wire::Node::Scroll {
                                on_scroll: None,
                                virtual_rows: false,
                                key: format!("{}/@layout:1559", use_scope),
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
                                    let mut children: Vec<::ducktape_view_guest::wire::Node> =
                                        Vec::new();
                                    for (index, item) in crate::host::filter_forge_items(
                                        ::std::convert::AsRef::as_ref(&(self.items)),
                                        ::std::convert::AsRef::as_ref(&("issues")),
                                    )
                                    .iter()
                                    .enumerate()
                                    {
                                        let for_scope =
                                            format!("{}/@for:2740({})", use_scope, index);
                                        children.push(self.render_tracker_row_43(
                                            palette,
                                            format!("{}/TrackerRow@2741", for_scope),
                                            (cb_10).clone(),
                                            item.clone(),
                                        ));
                                    }
                                    ::ducktape_view_guest::wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:1564", use_scope),
                                        wrap: None,
                                        axis: ::ducktape_view_guest::wire::Axis::Column,
                                        spacing: Some((1.0) as f32),
                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                            top: (6.0) as f32,
                                            right: (12.0) as f32,
                                            bottom: (18.0) as f32,
                                            left: (12.0) as f32,
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
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:1554", use_scope),
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
                    key: node_scope.clone(),
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
    pub(super) fn render_forge_tracker_list_46(
        &self,
        palette: Palette,
        use_scope: String,
        cb_10: impl Fn(i64) -> Message + Clone + 'static,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if self.repo_phase == "idle" {
                    children.push(::ducktape_view_guest::wire::Node::Space {
                        width: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                    });
                }
                if (!(self.repo_phase == "idle")) && (self.repo_phase == "loading") {
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
                        key: format!("{}/@container:1546", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
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
                        content: Box::new(self.render_empty_plate_34(
                            palette,
                            format!("{}/EmptyPlate@2715", use_scope),
                        )),
                    });
                }
                if (!((self.repo_phase == "idle") || (self.repo_phase == "loading")))
                    && (self.repo_phase == "failed")
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
                        key: format!("{}/@container:1549", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
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
                        content: Box::new(self.render_empty_plate_35(
                            palette,
                            format!("{}/EmptyPlate@2718", use_scope),
                        )),
                    });
                }
                if (!(((self.repo_phase == "idle") || (self.repo_phase == "loading"))
                    || (self.repo_phase == "failed")))
                    && (self.repo_phase == "ready")
                {
                    children.push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        if (crate::host::filter_forge_items(
                            ::std::convert::AsRef::as_ref(&(self.items)),
                            ::std::convert::AsRef::as_ref(&("pulls")),
                        ))
                        .is_empty()
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
                                key: format!("{}/@container:1556", use_scope),
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                padding: Some(::ducktape_view_guest::wire::Edges {
                                    top: (22.0) as f32,
                                    right: (22.0) as f32,
                                    bottom: (22.0) as f32,
                                    left: (22.0) as f32,
                                }),
                                align_x: None,
                                align_y: None,
                                background: (None)
                                    .map(::ducktape_view_guest::wire::Background::Color),
                                border: None,
                                snap: None,
                                content: Box::new(self.render_empty_plate_45(
                                    palette,
                                    format!("{}/EmptyPlate@2725", use_scope),
                                )),
                            });
                        }
                        if !(crate::host::filter_forge_items(
                            ::std::convert::AsRef::as_ref(&(self.items)),
                            ::std::convert::AsRef::as_ref(&("pulls")),
                        ))
                        .is_empty()
                        {
                            children.push(::ducktape_view_guest::wire::Node::Scroll {
                                on_scroll: None,
                                virtual_rows: false,
                                key: format!("{}/@layout:1559", use_scope),
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
                                    let mut children: Vec<::ducktape_view_guest::wire::Node> =
                                        Vec::new();
                                    for (index, item) in crate::host::filter_forge_items(
                                        ::std::convert::AsRef::as_ref(&(self.items)),
                                        ::std::convert::AsRef::as_ref(&("pulls")),
                                    )
                                    .iter()
                                    .enumerate()
                                    {
                                        let for_scope =
                                            format!("{}/@for:2740({})", use_scope, index);
                                        children.push(self.render_tracker_row_43(
                                            palette,
                                            format!("{}/TrackerRow@2741", for_scope),
                                            (cb_10).clone(),
                                            item.clone(),
                                        ));
                                    }
                                    ::ducktape_view_guest::wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:1564", use_scope),
                                        wrap: None,
                                        axis: ::ducktape_view_guest::wire::Axis::Column,
                                        spacing: Some((1.0) as f32),
                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                            top: (6.0) as f32,
                                            right: (12.0) as f32,
                                            bottom: (18.0) as f32,
                                            left: (12.0) as f32,
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
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:1554", use_scope),
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
                    key: node_scope.clone(),
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
    pub(super) fn render_pr_state_pill_49(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if self.forge_item_state == "open" {
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
                        key: format!("{}/@container:720", use_scope),
                        width: None,
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (5.0) as f32,
                            right: (11.0) as f32,
                            bottom: (5.0) as f32,
                            left: (11.0) as f32,
                        }),
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[27]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: Some(::ducktape_view_guest::wire::Border {
                            color: Some(palette.colors[28]),
                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
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
                            children.push(self.icon(
                                format!("{}/Icon@1897", use_scope),
                                "pull-request",
                                13f32,
                                (palette).colors[25usize],
                                "@media:58",
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
                                key: format!("{}/@text:734", use_scope),
                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[25]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("Open".to_owned()).to_string(),
                            });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:728", use_scope),
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
                        }),
                    });
                }
                if (!(self.forge_item_state == "open")) && (self.forge_item_state == "merged") {
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
                        key: format!("{}/@container:741", use_scope),
                        width: None,
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (5.0) as f32,
                            right: (11.0) as f32,
                            bottom: (5.0) as f32,
                            left: (11.0) as f32,
                        }),
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[104]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: Some(::ducktape_view_guest::wire::Border {
                            color: Some(palette.colors[105]),
                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
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
                            children.push(self.icon(
                                format!("{}/Icon@1918", use_scope),
                                "pull-request",
                                13f32,
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
                                key: format!("{}/@text:755", use_scope),
                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[103]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("Merged".to_owned()).to_string(),
                            });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:749", use_scope),
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
                        }),
                    });
                }
                if !((self.forge_item_state == "open") || (self.forge_item_state == "merged")) {
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
                        key: format!("{}/@container:762", use_scope),
                        width: None,
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (5.0) as f32,
                            right: (11.0) as f32,
                            bottom: (5.0) as f32,
                            left: (11.0) as f32,
                        }),
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[55]))
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
                        content: Box::new({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children.push(self.icon(
                                format!("{}/Icon@1939", use_scope),
                                "pull-request",
                                13f32,
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
                                key: format!("{}/@text:776", use_scope),
                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[5]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("Closed".to_owned()).to_string(),
                            });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:770", use_scope),
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
    pub(super) fn render_diff_count_56(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
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
                        key: format!("{}/@text:789", use_scope),
                        size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[25]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("+".to_owned()).to_string(),
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
                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:795", use_scope),
                        size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[25]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: (self.forge_item_additions).to_string(),
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:788", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((0.0) as f32),
                        padding: None,
                        width: None,
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
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
                        key: format!("{}/@text:802", use_scope),
                        size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[80]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("−".to_owned()).to_string(),
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
                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:808", use_scope),
                        size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[80]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: (self.forge_item_deletions).to_string(),
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:801", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((0.0) as f32),
                        padding: None,
                        width: None,
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                        background: None,
                        border: None,
                        children: children,
                    }
                });
                if self.forge_item_files_changed > 0 {
                    children.push({
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
                                    weight: ::ducktape_view_guest::wire::Weight::Medium,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:816", use_scope),
                            size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[70]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("·".to_owned()).to_string(),
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
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:822", use_scope),
                            size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[70]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: (self.forge_item_files_changed).to_string(),
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
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:828", use_scope),
                            size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[70]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("files".to_owned()).to_string(),
                        });
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:815", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: Some((4.0) as f32),
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
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
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
            }
        }
    }
    pub(super) fn render_issue_body_card_60(
        &self,
        palette: Palette,
        use_scope: String,
        cb_15: impl Fn(String) -> Message + Clone + 'static,
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
                clip: true,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: None,
                padding: None,
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[3]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[63]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
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
                        key: format!("{}/@container:858", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (8.0) as f32,
                            right: (13.0) as f32,
                            bottom: (8.0) as f32,
                            left: (13.0) as f32,
                        }),
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[87]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
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
                                key: format!("{}/@text:871", use_scope),
                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[70]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("opened by".to_owned()).to_string(),
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
                                            "Geist".into(),
                                        ),
                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:876", use_scope),
                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[7]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: (self.forge_item_author.to_owned()).to_string(),
                            });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:866", use_scope),
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
                        key: format!("{}/@container:882", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[60]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
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
                        max_width: None,
                        max_height: None,
                        clip: false,
                        key: format!("{}/@container:888", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (13.0) as f32,
                            right: (15.0) as f32,
                            bottom: (13.0) as f32,
                            left: (15.0) as f32,
                        }),
                        align_x: None,
                        align_y: None,
                        background: (None).map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
                        snap: None,
                        content: Box::new(self.render_rich_body_59(
                            palette,
                            format!("{}/RichBody@2067", use_scope),
                            (cb_15).clone(),
                        )),
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:857", use_scope),
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
    pub(super) fn render_diff_count_62(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
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
                        key: format!("{}/@text:789", use_scope),
                        size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[25]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("+".to_owned()).to_string(),
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
                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:795", use_scope),
                        size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[25]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: (self.forge_item_additions).to_string(),
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:788", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((0.0) as f32),
                        padding: None,
                        width: None,
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
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
                        key: format!("{}/@text:802", use_scope),
                        size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[80]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("−".to_owned()).to_string(),
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
                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:808", use_scope),
                        size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[80]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: (self.forge_item_deletions).to_string(),
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:801", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((0.0) as f32),
                        padding: None,
                        width: None,
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                        background: None,
                        border: None,
                        children: children,
                    }
                });
                if false {
                    children.push({
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
                                    weight: ::ducktape_view_guest::wire::Weight::Medium,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:816", use_scope),
                            size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[70]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("·".to_owned()).to_string(),
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
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:822", use_scope),
                            size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[70]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: (0).to_string(),
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
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:828", use_scope),
                            size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[70]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("files".to_owned()).to_string(),
                        });
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:815", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: Some((4.0) as f32),
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
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
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
            }
        }
    }
    pub(super) fn render_diff_row_63(
        &self,
        palette: Palette,
        use_scope: String,
        cb_0: impl Fn(String, String, String) -> Message + Clone + 'static,
        arg_0: crate::host::DiffLine,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if arg_0.kind == "file" {
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
                        key: format!("{}/@container:1122", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (5.0) as f32,
                            right: (14.0) as f32,
                            bottom: (5.0) as f32,
                            left: (14.0) as f32,
                        }),
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[87]))
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
                            key: format!("{}/@text:1130", use_scope),
                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[70]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            align_x: None,
                            content: (arg_0.text.to_owned()).to_string(),
                        }),
                    });
                }
                if (!(arg_0.kind == "file")) && (arg_0.kind == "hunk") {
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
                        key: format!("{}/@container:1138", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (5.0) as f32,
                            right: (14.0) as f32,
                            bottom: (5.0) as f32,
                            left: (14.0) as f32,
                        }),
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[114]))
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
                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:1146", use_scope),
                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[103]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            align_x: None,
                            content: (arg_0.text.to_owned()).to_string(),
                        }),
                    });
                }
                if (!((arg_0.kind == "file") || (arg_0.kind == "hunk"))) && (arg_0.kind == "add") {
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
                            key: format!("{}/@container:1154", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[108]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new({
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
                                        key: format!("{}/@container:1160", use_scope),
                                        width: Some(
                                            ::ducktape_view_guest::wire::Length::Fixed((34.0) as f32),
                                        ),
                                        height: Some(
                                            ::ducktape_view_guest::wire::Length::Fixed((20.0) as f32),
                                        ),
                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                            top: (0.0) as f32,
                                            right: (8.0) as f32,
                                            bottom: (0.0) as f32,
                                            left: (0.0) as f32,
                                        }),
                                        align_x: None,
                                        align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                        background: (Some(palette.colors[109]))
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
                                            key: format!("{}/@text:1167", use_scope),
                                            size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                            color: Some(palette.colors[102]),
                                            font: ::ducktape_view_guest::wire::Font {
                                                monospace: false,
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            },
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            align_x: Some(::ducktape_view_guest::wire::AlignX::Right),
                                            content: (arg_0.old_no.to_owned()).to_string(),
                                        }),
                                    });
                                if (arg_0.path).is_empty() {
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
                                            key: format!("{}/@container:1176", use_scope),
                                            width: Some(
                                                ::ducktape_view_guest::wire::Length::Fixed((34.0) as f32),
                                            ),
                                            height: Some(
                                                ::ducktape_view_guest::wire::Length::Fixed((20.0) as f32),
                                            ),
                                            padding: Some(::ducktape_view_guest::wire::Edges {
                                                top: (0.0) as f32,
                                                right: (8.0) as f32,
                                                bottom: (0.0) as f32,
                                                left: (0.0) as f32,
                                            }),
                                            align_x: None,
                                            align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                            background: (Some(palette.colors[109]))
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
                                                key: format!("{}/@text:1183", use_scope),
                                                size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                color: Some(palette.colors[102]),
                                                font: ::ducktape_view_guest::wire::Font {
                                                    monospace: false,
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                },
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                align_x: Some(::ducktape_view_guest::wire::AlignX::Right),
                                                content: (arg_0.new_no.to_owned()).to_string(),
                                            }),
                                        });
                                }
                                if !(arg_0.path).is_empty()  {
                                    children
                                        .push(::ducktape_view_guest::wire::Node::Button {
                                            checked: None,
                                            expanded: None,
                                            description: None,
                                            key: format!("{}/@button:1192", use_scope),
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
                                                    key: format!("{}/@container:1199", use_scope),
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    height: Some(
                                                        ::ducktape_view_guest::wire::Length::Fixed((20.0) as f32),
                                                    ),
                                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                                        top: (0.0) as f32,
                                                        right: (8.0) as f32,
                                                        bottom: (0.0) as f32,
                                                        left: (0.0) as f32,
                                                    }),
                                                    align_x: None,
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
                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                            }),
                                                        },
                                                        key: format!("{}/@text:1205", use_scope),
                                                        size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                        color: None,
                                                        font: ::ducktape_view_guest::wire::Font {
                                                            monospace: false,
                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                        },
                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                        align_x: Some(::ducktape_view_guest::wire::AlignX::Right),
                                                        content: (arg_0.new_no.to_owned()).to_string(),
                                                    }),
                                                }),
                                            ),
                                            label: Some(
                                                String::from("Comment on this line".to_owned()),
                                            ),
                                            on_press: Some(
                                                ::ducktape_view_guest::slots::message(
                                                    (cb_0)(
                                                        arg_0.path.to_owned(),
                                                        arg_0.new_no.to_owned(),
                                                        "new".to_owned(),
                                                    ),
                                                ),
                                            ),
                                            width: Some(
                                                ::ducktape_view_guest::wire::Length::Fixed((34.0) as f32),
                                            ),
                                            height: Some(
                                                ::ducktape_view_guest::wire::Length::Fixed((20.0) as f32),
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
                                                    background: Some(palette.colors[109]),
                                                    text: Some(palette.colors[102]),
                                                    border: None,
                                                },
                                                hovered: Some(::ducktape_view_guest::wire::Face {
                                                    background: Some(palette.colors[18]),
                                                    text: Some(palette.colors[16]),
                                                    border: None,
                                                }),
                                                pressed: Some(::ducktape_view_guest::wire::Face {
                                                    background: Some(palette.colors[90]),
                                                    text: Some(palette.colors[16]),
                                                    border: None,
                                                }),
                                                disabled: None,
                                            },
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
                                        clip: false,
                                        key: format!("{}/@container:1215", use_scope),
                                        width: Some(
                                            ::ducktape_view_guest::wire::Length::Fixed((14.0) as f32),
                                        ),
                                        height: Some(
                                            ::ducktape_view_guest::wire::Length::Fixed((20.0) as f32),
                                        ),
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
                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                }),
                                            },
                                            key: format!("{}/@text:1221", use_scope),
                                            size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                            color: Some(palette.colors[110]),
                                            font: ::ducktape_view_guest::wire::Font {
                                                monospace: false,
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            },
                                            width: None,
                                            align_x: None,
                                            content: (arg_0.sign.to_owned()).to_string(),
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
                                        key: format!("{}/@container:1227", use_scope),
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: Some(
                                            ::ducktape_view_guest::wire::Length::Fixed((20.0) as f32),
                                        ),
                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                            top: (0.0) as f32,
                                            right: (12.0) as f32,
                                            bottom: (0.0) as f32,
                                            left: (0.0) as f32,
                                        }),
                                        align_x: None,
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
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                }),
                                            },
                                            key: format!("{}/@text:1233", use_scope),
                                            size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                            color: Some(palette.colors[69]),
                                            font: ::ducktape_view_guest::wire::Font {
                                                monospace: false,
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            },
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            align_x: None,
                                            content: (arg_0.text.to_owned()).to_string(),
                                        }),
                                    });
                                ::ducktape_view_guest::wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:1155", use_scope),
                                    wrap: None,
                                    axis: ::ducktape_view_guest::wire::Axis::Row,
                                    spacing: Some((0.0) as f32),
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
                }
                if (!(((arg_0.kind == "file") || (arg_0.kind == "hunk")) || (arg_0.kind == "add")))
                    && (arg_0.kind == "del")
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
                            key: format!("{}/@container:1241", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[111]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new({
                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                if (arg_0.path).is_empty() {
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
                                            key: format!("{}/@container:1248", use_scope),
                                            width: Some(
                                                ::ducktape_view_guest::wire::Length::Fixed((34.0) as f32),
                                            ),
                                            height: Some(
                                                ::ducktape_view_guest::wire::Length::Fixed((20.0) as f32),
                                            ),
                                            padding: Some(::ducktape_view_guest::wire::Edges {
                                                top: (0.0) as f32,
                                                right: (8.0) as f32,
                                                bottom: (0.0) as f32,
                                                left: (0.0) as f32,
                                            }),
                                            align_x: None,
                                            align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                            background: (Some(palette.colors[112]))
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
                                                key: format!("{}/@text:1255", use_scope),
                                                size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                color: Some(palette.colors[102]),
                                                font: ::ducktape_view_guest::wire::Font {
                                                    monospace: false,
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                },
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                align_x: Some(::ducktape_view_guest::wire::AlignX::Right),
                                                content: (arg_0.old_no.to_owned()).to_string(),
                                            }),
                                        });
                                }
                                if !(arg_0.path).is_empty()  {
                                    children
                                        .push(::ducktape_view_guest::wire::Node::Button {
                                            checked: None,
                                            expanded: None,
                                            description: None,
                                            key: format!("{}/@button:1264", use_scope),
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
                                                    key: format!("{}/@container:1271", use_scope),
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    height: Some(
                                                        ::ducktape_view_guest::wire::Length::Fixed((20.0) as f32),
                                                    ),
                                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                                        top: (0.0) as f32,
                                                        right: (8.0) as f32,
                                                        bottom: (0.0) as f32,
                                                        left: (0.0) as f32,
                                                    }),
                                                    align_x: None,
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
                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                            }),
                                                        },
                                                        key: format!("{}/@text:1277", use_scope),
                                                        size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                        color: None,
                                                        font: ::ducktape_view_guest::wire::Font {
                                                            monospace: false,
                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                        },
                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                        align_x: Some(::ducktape_view_guest::wire::AlignX::Right),
                                                        content: (arg_0.old_no.to_owned()).to_string(),
                                                    }),
                                                }),
                                            ),
                                            label: Some(
                                                String::from("Comment on this deleted line".to_owned()),
                                            ),
                                            on_press: Some(
                                                ::ducktape_view_guest::slots::message(
                                                    (cb_0)(
                                                        arg_0.path.to_owned(),
                                                        arg_0.old_no.to_owned(),
                                                        "old".to_owned(),
                                                    ),
                                                ),
                                            ),
                                            width: Some(
                                                ::ducktape_view_guest::wire::Length::Fixed((34.0) as f32),
                                            ),
                                            height: Some(
                                                ::ducktape_view_guest::wire::Length::Fixed((20.0) as f32),
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
                                                    background: Some(palette.colors[112]),
                                                    text: Some(palette.colors[102]),
                                                    border: None,
                                                },
                                                hovered: Some(::ducktape_view_guest::wire::Face {
                                                    background: Some(palette.colors[18]),
                                                    text: Some(palette.colors[16]),
                                                    border: None,
                                                }),
                                                pressed: Some(::ducktape_view_guest::wire::Face {
                                                    background: Some(palette.colors[90]),
                                                    text: Some(palette.colors[16]),
                                                    border: None,
                                                }),
                                                disabled: None,
                                            },
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
                                        clip: false,
                                        key: format!("{}/@container:1287", use_scope),
                                        width: Some(
                                            ::ducktape_view_guest::wire::Length::Fixed((34.0) as f32),
                                        ),
                                        height: Some(
                                            ::ducktape_view_guest::wire::Length::Fixed((20.0) as f32),
                                        ),
                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                            top: (0.0) as f32,
                                            right: (8.0) as f32,
                                            bottom: (0.0) as f32,
                                            left: (0.0) as f32,
                                        }),
                                        align_x: None,
                                        align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                        background: (Some(palette.colors[112]))
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
                                            key: format!("{}/@text:1294", use_scope),
                                            size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                            color: Some(palette.colors[102]),
                                            font: ::ducktape_view_guest::wire::Font {
                                                monospace: false,
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            },
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            align_x: Some(::ducktape_view_guest::wire::AlignX::Right),
                                            content: (arg_0.new_no.to_owned()).to_string(),
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
                                        key: format!("{}/@container:1302", use_scope),
                                        width: Some(
                                            ::ducktape_view_guest::wire::Length::Fixed((14.0) as f32),
                                        ),
                                        height: Some(
                                            ::ducktape_view_guest::wire::Length::Fixed((20.0) as f32),
                                        ),
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
                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                }),
                                            },
                                            key: format!("{}/@text:1308", use_scope),
                                            size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                            color: Some(palette.colors[113]),
                                            font: ::ducktape_view_guest::wire::Font {
                                                monospace: false,
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            },
                                            width: None,
                                            align_x: None,
                                            content: (arg_0.sign.to_owned()).to_string(),
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
                                        key: format!("{}/@container:1314", use_scope),
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: Some(
                                            ::ducktape_view_guest::wire::Length::Fixed((20.0) as f32),
                                        ),
                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                            top: (0.0) as f32,
                                            right: (12.0) as f32,
                                            bottom: (0.0) as f32,
                                            left: (0.0) as f32,
                                        }),
                                        align_x: None,
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
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                }),
                                            },
                                            key: format!("{}/@text:1320", use_scope),
                                            size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                            color: Some(palette.colors[69]),
                                            font: ::ducktape_view_guest::wire::Font {
                                                monospace: false,
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            },
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            align_x: None,
                                            content: (arg_0.text.to_owned()).to_string(),
                                        }),
                                    });
                                ::ducktape_view_guest::wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:1242", use_scope),
                                    wrap: None,
                                    axis: ::ducktape_view_guest::wire::Axis::Row,
                                    spacing: Some((0.0) as f32),
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
                }
                if (!((((arg_0.kind == "file") || (arg_0.kind == "hunk"))
                    || (arg_0.kind == "add"))
                    || (arg_0.kind == "del")))
                    && (arg_0.kind == "ctx")
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
                            key: format!("{}/@container:1328", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[3]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new({
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
                                        key: format!("{}/@container:1334", use_scope),
                                        width: Some(
                                            ::ducktape_view_guest::wire::Length::Fixed((34.0) as f32),
                                        ),
                                        height: Some(
                                            ::ducktape_view_guest::wire::Length::Fixed((20.0) as f32),
                                        ),
                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                            top: (0.0) as f32,
                                            right: (8.0) as f32,
                                            bottom: (0.0) as f32,
                                            left: (0.0) as f32,
                                        }),
                                        align_x: None,
                                        align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                        background: (Some(palette.colors[87]))
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
                                            key: format!("{}/@text:1341", use_scope),
                                            size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                            color: Some(palette.colors[102]),
                                            font: ::ducktape_view_guest::wire::Font {
                                                monospace: false,
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            },
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            align_x: Some(::ducktape_view_guest::wire::AlignX::Right),
                                            content: (arg_0.old_no.to_owned()).to_string(),
                                        }),
                                    });
                                if (arg_0.path).is_empty() {
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
                                            key: format!("{}/@container:1350", use_scope),
                                            width: Some(
                                                ::ducktape_view_guest::wire::Length::Fixed((34.0) as f32),
                                            ),
                                            height: Some(
                                                ::ducktape_view_guest::wire::Length::Fixed((20.0) as f32),
                                            ),
                                            padding: Some(::ducktape_view_guest::wire::Edges {
                                                top: (0.0) as f32,
                                                right: (8.0) as f32,
                                                bottom: (0.0) as f32,
                                                left: (0.0) as f32,
                                            }),
                                            align_x: None,
                                            align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                            background: (Some(palette.colors[87]))
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
                                                key: format!("{}/@text:1357", use_scope),
                                                size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                color: Some(palette.colors[102]),
                                                font: ::ducktape_view_guest::wire::Font {
                                                    monospace: false,
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                },
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                align_x: Some(::ducktape_view_guest::wire::AlignX::Right),
                                                content: (arg_0.new_no.to_owned()).to_string(),
                                            }),
                                        });
                                }
                                if !(arg_0.path).is_empty()  {
                                    children
                                        .push(::ducktape_view_guest::wire::Node::Button {
                                            checked: None,
                                            expanded: None,
                                            description: None,
                                            key: format!("{}/@button:1366", use_scope),
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
                                                    key: format!("{}/@container:1373", use_scope),
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    height: Some(
                                                        ::ducktape_view_guest::wire::Length::Fixed((20.0) as f32),
                                                    ),
                                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                                        top: (0.0) as f32,
                                                        right: (8.0) as f32,
                                                        bottom: (0.0) as f32,
                                                        left: (0.0) as f32,
                                                    }),
                                                    align_x: None,
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
                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                            }),
                                                        },
                                                        key: format!("{}/@text:1379", use_scope),
                                                        size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                        color: None,
                                                        font: ::ducktape_view_guest::wire::Font {
                                                            monospace: false,
                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                        },
                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                        align_x: Some(::ducktape_view_guest::wire::AlignX::Right),
                                                        content: (arg_0.new_no.to_owned()).to_string(),
                                                    }),
                                                }),
                                            ),
                                            label: Some(
                                                String::from("Comment on this line".to_owned()),
                                            ),
                                            on_press: Some(
                                                ::ducktape_view_guest::slots::message(
                                                    (cb_0)(
                                                        arg_0.path.to_owned(),
                                                        arg_0.new_no.to_owned(),
                                                        "new".to_owned(),
                                                    ),
                                                ),
                                            ),
                                            width: Some(
                                                ::ducktape_view_guest::wire::Length::Fixed((34.0) as f32),
                                            ),
                                            height: Some(
                                                ::ducktape_view_guest::wire::Length::Fixed((20.0) as f32),
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
                                                    background: Some(palette.colors[87]),
                                                    text: Some(palette.colors[102]),
                                                    border: None,
                                                },
                                                hovered: Some(::ducktape_view_guest::wire::Face {
                                                    background: Some(palette.colors[18]),
                                                    text: Some(palette.colors[16]),
                                                    border: None,
                                                }),
                                                pressed: Some(::ducktape_view_guest::wire::Face {
                                                    background: Some(palette.colors[90]),
                                                    text: Some(palette.colors[16]),
                                                    border: None,
                                                }),
                                                disabled: None,
                                            },
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
                                        clip: false,
                                        key: format!("{}/@container:1389", use_scope),
                                        width: Some(
                                            ::ducktape_view_guest::wire::Length::Fixed((14.0) as f32),
                                        ),
                                        height: Some(
                                            ::ducktape_view_guest::wire::Length::Fixed((20.0) as f32),
                                        ),
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
                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                }),
                                            },
                                            key: format!("{}/@text:1395", use_scope),
                                            size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                            color: Some(palette.colors[102]),
                                            font: ::ducktape_view_guest::wire::Font {
                                                monospace: false,
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            },
                                            width: None,
                                            align_x: None,
                                            content: (arg_0.sign.to_owned()).to_string(),
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
                                        key: format!("{}/@container:1401", use_scope),
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: Some(
                                            ::ducktape_view_guest::wire::Length::Fixed((20.0) as f32),
                                        ),
                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                            top: (0.0) as f32,
                                            right: (12.0) as f32,
                                            bottom: (0.0) as f32,
                                            left: (0.0) as f32,
                                        }),
                                        align_x: None,
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
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                }),
                                            },
                                            key: format!("{}/@text:1407", use_scope),
                                            size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                            color: Some(palette.colors[69]),
                                            font: ::ducktape_view_guest::wire::Font {
                                                monospace: false,
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            },
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            align_x: None,
                                            content: (arg_0.text.to_owned()).to_string(),
                                        }),
                                    });
                                ::ducktape_view_guest::wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:1329", use_scope),
                                    wrap: None,
                                    axis: ::ducktape_view_guest::wire::Axis::Row,
                                    spacing: Some((0.0) as f32),
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
    pub(super) fn render_diff_pane_64(
        &self,
        palette: Palette,
        use_scope: String,
        cb_5: impl Fn(String, String, String) -> Message + Clone + 'static,
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
                clip: true,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: None,
                padding: None,
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[3]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[63]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
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
                        key: format!("{}/@container:1063", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (10.0) as f32,
                            right: (14.0) as f32,
                            bottom: (10.0) as f32,
                            left: (14.0) as f32,
                        }),
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[87]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
                        snap: None,
                        content: Box::new({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children.push(self.icon(
                                format!("{}/Icon@2244", use_scope),
                                "branch",
                                13f32,
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
                                            "Geist Mono".into(),
                                        ),
                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:1081", use_scope),
                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[15]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: (self.forge_item_branches.to_owned()).to_string(),
                            });
                            children.push(self.render_diff_count_62(
                                palette,
                                format!("{}/DiffCount@2255", use_scope),
                            ));
                            children.push(::ducktape_view_guest::wire::Node::Space {
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                            });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:1071", use_scope),
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
                        key: format!("{}/@container:1093", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[60]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
                        snap: None,
                        content: Box::new(::ducktape_view_guest::wire::Node::Space {
                            width: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                            height: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                        }),
                    });
                    children.push({
                        let mut children: Vec<_> = Vec::new();
                        for line in self.diff_rows.iter() {
                            let key = line.key;
                            let key_recon = format!("{}/key({})", use_scope, key);
                            let child: ::ducktape_view_guest::wire::Node = self.render_diff_row_63(
                                palette,
                                format!("{}/DiffRow@2268", key_recon),
                                (cb_5).clone(),
                                line.clone(),
                            );
                            children.push((key, child));
                        }
                        let (keys, children) = children
                            .into_iter()
                            .map(|(key, child)| {
                                (::ducktape_view_guest::wire::ListKey::from(key), child)
                            })
                            .unzip();
                        ::ducktape_view_guest::wire::Node::KeyedColumn {
                            key: format!("{}/@keyed:1099", use_scope),
                            keys: Some(keys),
                            children,
                            background: None,
                            border: None,
                            spacing: None,
                            padding: None,
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            max_width: None,
                            align: None,
                            virtual_row: Some((20.0) as f32),
                        }
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:1062", use_scope),
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
    pub(super) fn render_merged_banner_66(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
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
                    key: format!("{}/@container:913", use_scope),
                    width: Some(::ducktape_view_guest::wire::Length::Fixed((24.0) as f32)),
                    height: Some(::ducktape_view_guest::wire::Length::Fixed((24.0) as f32)),
                    padding: None,
                    align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                    align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                    background: (Some(palette.colors[104]))
                        .map(::ducktape_view_guest::wire::Background::Color),
                    border: Some(::ducktape_view_guest::wire::Border {
                        color: Some(palette.colors[105]),
                        width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
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
                        key: format!("{}/@text:923", use_scope),
                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[103]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("✓".to_owned()).to_string(),
                    }),
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
                            family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                        }),
                    },
                    key: format!("{}/@text:929", use_scope),
                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[103]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    align_x: None,
                    content: (crate::host::forge_merge_note(
                        ::std::convert::AsRef::as_ref(&(self.forge_item_merge_oid)),
                        ::std::convert::AsRef::as_ref(&(self.forge_item_branches)),
                    )
                    .to_owned())
                    .to_string(),
                });
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
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
            }
        }
    }
    pub(super) fn render_merge_advisory_67(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if self.forge_item_change_requests == 1 {
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
                            key: format!("{}/@container:950", use_scope),
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
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:957", use_scope),
                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[30]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            align_x: None,
                            content: ("a reviewer requested changes — merge not recommended"
                                .to_owned())
                            .to_string(),
                        });
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:945", use_scope),
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
                if self.forge_item_change_requests > 1 {
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
                            key: format!("{}/@container:968", use_scope),
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
                            key: format!("{}/@text:975", use_scope),
                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[30]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: (self.forge_item_change_requests).to_string(),
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
                                        "Geist".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:981", use_scope),
                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[30]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            align_x: None,
                            content: ("reviewers requested changes — merge not recommended"
                                .to_owned())
                            .to_string(),
                        });
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:963", use_scope),
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
    pub(super) fn render_merge_button_69(
        &self,
        palette: Palette,
        use_scope: String,
        cb_7: impl Fn() -> Message + Clone + 'static,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if self.merge_busy {
                    children.push(::ducktape_view_guest::wire::Node::Button {
                        checked: None,
                        expanded: None,
                        description: None,
                        key: format!("{}/@button:993", use_scope),
                        content: ::ducktape_view_guest::wire::ButtonContent::Child(Box::new({
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
                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:1002", use_scope),
                                size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[9]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("Merging…".to_owned()).to_string(),
                            });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:1001", use_scope),
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
                        label: Some(String::from("Merging".to_owned())),
                        on_press: None,
                        width: None,
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: 9f32,
                            right: 18f32,
                            bottom: 9f32,
                            left: 18f32,
                        }),
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
                }
                if !self.merge_busy {
                    children.push(::ducktape_view_guest::wire::Node::Button {
                        checked: None,
                        expanded: None,
                        description: None,
                        key: format!("{}/@button:1009", use_scope),
                        content: ::ducktape_view_guest::wire::ButtonContent::Child(Box::new({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children.push(self.icon(
                                format!("{}/Icon@2186", use_scope),
                                "pull-request",
                                13f32,
                                (palette).colors[38usize],
                                "@media:76",
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
                                key: format!("{}/@text:1023", use_scope),
                                size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[9]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("Merge pull request".to_owned()).to_string(),
                            });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:1017", use_scope),
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
                        label: Some(String::from("Merge pull request".to_owned())),
                        on_press: if (!self.connected) || (self.forge_item_source_oid).is_empty() {
                            None
                        } else {
                            Some(::ducktape_view_guest::slots::message((cb_7)()))
                        },
                        width: None,
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: 9f32,
                            right: 18f32,
                            bottom: 9f32,
                            left: 18f32,
                        }),
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
    pub(super) fn render_review_verdict_71(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if arg_0 == "approve" {
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
                        key: format!("{}/@text:1514", use_scope),
                        size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[25]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: (crate::host::verdict_label(::std::convert::AsRef::as_ref(
                            &(arg_0),
                        )))
                        .to_string(),
                    });
                }
                if (!(arg_0 == "approve")) && (arg_0 == "request_changes") {
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
                        key: format!("{}/@text:1521", use_scope),
                        size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[80]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: (crate::host::verdict_label(::std::convert::AsRef::as_ref(
                            &(arg_0),
                        )))
                        .to_string(),
                    });
                }
                if !((arg_0 == "approve") || (arg_0 == "request_changes")) {
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
                        key: format!("{}/@text:1528", use_scope),
                        size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[71]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: (crate::host::verdict_label(::std::convert::AsRef::as_ref(
                            &(arg_0),
                        )))
                        .to_string(),
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
    pub(super) fn render_review_card_76(
        &self,
        palette: Palette,
        use_scope: String,
        cb_15: impl Fn(String) -> Message + Clone + 'static,
        arg_0: crate::host::ForgeReview,
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
                background: (Some(palette.colors[3]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[63]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((10.0) as f32).max(0.0).min(f32::MAX),
                        ((10.0) as f32).max(0.0).min(f32::MAX),
                        ((10.0) as f32).max(0.0).min(f32::MAX),
                        ((10.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children.push({
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
                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:1441", use_scope),
                            size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[7]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: (arg_0.author_name.to_owned()).to_string(),
                        });
                        children.push(self.render_review_verdict_71(
                            palette,
                            format!("{}/ReviewVerdict@2615", use_scope),
                            arg_0.verdict.to_owned(),
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
                            key: format!("{}/@text:1448", use_scope),
                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[72]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: (arg_0.commit.to_owned()).to_string(),
                        });
                        if arg_0.outdated {
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
                                key: format!("{}/@container:1455", use_scope),
                                width: None,
                                height: None,
                                padding: Some(::ducktape_view_guest::wire::Edges {
                                    top: (2.0) as f32,
                                    right: (6.0) as f32,
                                    bottom: (2.0) as f32,
                                    left: (6.0) as f32,
                                }),
                                align_x: None,
                                align_y: None,
                                background: (Some(palette.colors[55]))
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
                                            stretch:
                                                ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:1461", use_scope),
                                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[71]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("outdated".to_owned()).to_string(),
                                }),
                            });
                        }
                        children.push(::ducktape_view_guest::wire::Node::Space {
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                        });
                        children.push(self.render_finality_chip_72(
                            palette,
                            format!("{}/FinalityChip@2636", use_scope),
                            arg_0.created_at,
                        ));
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:1436", use_scope),
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
                    if !(arg_0.body).is_empty() {
                        children.push(self.render_rich_body_73(
                            palette,
                            format!("{}/RichBody@2638", use_scope),
                            (cb_15).clone(),
                            arg_0.blocks.clone(),
                        ));
                    }
                    for (index, comment) in arg_0.comments.iter().enumerate() {
                        let for_scope = format!("{}/@for:2641({})", use_scope, index);
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
                                key: format!("{}/@container:1474", for_scope),
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
                                                    key: format!("{}/@text:1491", for_scope),
                                                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: Some(palette.colors[16]),
                                                    font: ::ducktape_view_guest::wire::Font {
                                                        monospace: false,
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    },
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    align_x: None,
                                                    content: (comment.anchor.to_owned()).to_string(),
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
                                                    key: format!("{}/@text:1498", for_scope),
                                                    size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: Some(palette.colors[73]),
                                                    font: ::ducktape_view_guest::wire::Font {
                                                        monospace: false,
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    },
                                                    width: None,
                                                    align_x: None,
                                                    content: ("review comment".to_owned()).to_string(),
                                                });
                                            ::ducktape_view_guest::wire::Node::Linear {
                                                max_width: None,
                                                clip: false,
                                                key: format!("{}/@layout:1486", for_scope),
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
                                        .push(
                                            self
                                                .render_rich_body_75(
                                                    palette,
                                                    format!("{}/RichBody@2672", for_scope),
                                                    (cb_15).clone(),
                                                    comment.blocks.clone(),
                                                ),
                                        );
                                    ::ducktape_view_guest::wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:1485", for_scope),
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
                    }
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:1435", use_scope),
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
}
