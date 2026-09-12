impl PagesView {
    pub(crate) fn page_button(
        &self,
        palette: Palette,
        use_scope: String,
        page: crate::host::PageItem,
        selected: bool,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
            if selected {
                children.push(::ducktape_view_guest::wire::Node::Button {
                    checked: Some(selected),
                    expanded: None,
                    description: None,
                    key: format!("{}/@button:14", use_scope),
                    content: ::ducktape_view_guest::wire::ButtonContent::Child(Box::new({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        if (!(page.prefix).is_empty()) {
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
                                key: format!("{}/@text:29", use_scope),
                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[73]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: (page.prefix.to_owned()).to_string(),
                            });
                        }
                        children.push(self.icon(
                            palette,
                            format!("{}/Icon@1644", use_scope),
                            "doc",
                            14.,
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
                            key: format!("{}/@container:40", use_scope),
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
                                key: format!("{}/@text:41", use_scope),
                                size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[4]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: (page.title.to_owned()).to_string(),
                            }),
                        });
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:23", use_scope),
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
                    })),
                    label: Some(String::from(page.title.to_owned())),
                    on_press: if (!(self.host_error).is_empty()) {
                        None
                    } else {
                        Some(::ducktape_view_guest::slots::message((|event_0| {
                            Message::ChoosePage(event_0)
                        })(
                            page.id.to_owned()
                        )))
                    },
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    padding: Some(::ducktape_view_guest::wire::Edges {
                        top: 7f32,
                        right: 12f32,
                        bottom: 7f32,
                        left: 12f32,
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
                            background: Some(palette.colors[58]),
                            text: Some(palette.colors[4]),
                            border: None,
                        }),
                        pressed: Some(::ducktape_view_guest::wire::Face {
                            background: Some(palette.colors[91]),
                            text: Some(palette.colors[4]),
                            border: None,
                        }),
                        disabled: None,
                    },
                });
            }
            if (!selected) {
                children.push(::ducktape_view_guest::wire::Node::Button {
                    checked: Some(selected),
                    expanded: None,
                    description: None,
                    key: format!("{}/@button:50", use_scope),
                    content: ::ducktape_view_guest::wire::ButtonContent::Child(Box::new({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        if (!(page.prefix).is_empty()) {
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
                                key: format!("{}/@text:65", use_scope),
                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[73]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: (page.prefix.to_owned()).to_string(),
                            });
                        }
                        children.push(self.icon(
                            palette,
                            format!("{}/Icon@1680", use_scope),
                            "doc",
                            14.,
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
                            key: format!("{}/@container:76", use_scope),
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
                                key: format!("{}/@text:77", use_scope),
                                size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[5]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: (page.title.to_owned()).to_string(),
                            }),
                        });
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:59", use_scope),
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
                    })),
                    label: Some(String::from(page.title.to_owned())),
                    on_press: if (!(self.host_error).is_empty()) {
                        None
                    } else {
                        Some(::ducktape_view_guest::slots::message((|event_0| {
                            Message::ChoosePage(event_0)
                        })(
                            page.id.to_owned()
                        )))
                    },
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    padding: Some(::ducktape_view_guest::wire::Edges {
                        top: 7f32,
                        right: 12f32,
                        bottom: 7f32,
                        left: 12f32,
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
                key: format!("{}/@layout:12", use_scope),
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
    pub(crate) fn search_result(
        &self,
        palette: Palette,
        use_scope: String,
        hit: crate::host::PageSearchHit,
    ) -> ::ducktape_view_guest::wire::Node {
        ::ducktape_view_guest::wire::Node::Button {
            checked: None,
            expanded: None,
            description: None,
            key: format!("{}/@button:89", use_scope),
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
                                    "Geist Mono".into(),
                                ),
                                weight: ::ducktape_view_guest::wire::Weight::Medium,
                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:106", use_scope),
                        size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[5]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        align_x: None,
                        content: (hit.page_title.to_owned()).to_string(),
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
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:112", use_scope),
                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[5]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: (hit.kind.to_owned()).to_string(),
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:97", use_scope),
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
                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::Word),
                        tracking: 0.0f32,
                        font: Some(::ducktape_view_guest::wire::NamedFont {
                            family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                        }),
                    },
                    key: format!("{}/@text:118", use_scope),
                    size: Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[4]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    align_x: None,
                    content: (hit.text.to_owned()).to_string(),
                });
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: format!("{}/@layout:96", use_scope),
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
            })),
            label: Some(String::from(hit.text.to_owned())),
            on_press: if (!(self.host_error).is_empty()) {
                None
            } else {
                Some(::ducktape_view_guest::slots::message(
                    (|event_0, event_1| Message::OpenPageSearchHit(event_0, event_1))(
                        hit.page_id.to_owned(),
                        hit.block_id.to_owned(),
                    ),
                ))
            },
            width: Some(::ducktape_view_guest::wire::Length::Fill),
            height: None,
            padding: Some(::ducktape_view_guest::wire::Edges::all((7.0) as f32)),
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
                            ((8.0) as f32).max(0.0).min(f32::MAX),
                            ((8.0) as f32).max(0.0).min(f32::MAX),
                            ((8.0) as f32).max(0.0).min(f32::MAX),
                            ((8.0) as f32).max(0.0).min(f32::MAX),
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
                            color.0[3] = 0.080000;
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
                            color.0[3] = 0.120000;
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
    pub(crate) fn comment_thread(
        &self,
        palette: Palette,
        use_scope: String,
        thread: crate::host::PageCommentThread,
        replying: bool,
        expanded: bool,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
            children.push({
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children.push(self.comment_avatar(
                    palette,
                    format!("{}/PersonAvatar@1769", use_scope),
                    crate::host::initials_of(::std::convert::AsRef::as_ref(&(thread.author))),
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
                    key: format!("{}/@text:165", use_scope),
                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[7]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: (thread.author.to_owned()).to_string(),
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
                    key: format!("{}/@text:171", use_scope),
                    size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[73]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: (thread.meta.to_owned()).to_string(),
                });
                children.push(::ducktape_view_guest::wire::Node::Space {
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                });
                if (!thread.resolved) {
                    children.push(::ducktape_view_guest::wire::Node::Button {
                        checked: None,
                        expanded: None,
                        description: None,
                        key: format!("{}/@button:180", use_scope),
                        content: ::ducktape_view_guest::wire::ButtonContent::Label(String::from(
                            "Resolve",
                        )),
                        label: Some(String::from("Resolve thread".to_owned())),
                        on_press: if ((self.busy || (!(self.host_error).is_empty()))
                            || (!(self.host_error).is_empty()))
                        {
                            None
                        } else {
                            Some(::ducktape_view_guest::slots::message(
                                (|event_0, event_1| Message::ResolveThreadSubmit(event_0, event_1))(
                                    thread.id.to_owned(),
                                    true,
                                ),
                            ))
                        },
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
                                        radius: Some([9.0; 4]),
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
                            active: ::ducktape_view_guest::wire::Face {
                                background: Some(::ducktape_view_guest::wire::Rgba([
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.000000,
                                ])),
                                text: Some(palette.colors[5]),
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
                            },
                            hovered: Some(::ducktape_view_guest::wire::Face {
                                background: Some({
                                    let mut color = palette.colors[4];
                                    color.0[3] = 0.090000;
                                    color
                                }),
                                text: Some(palette.colors[4]),
                                border: None,
                            }),
                            pressed: Some(::ducktape_view_guest::wire::Face {
                                background: Some({
                                    let mut color = palette.colors[4];
                                    color.0[3] = 0.140000;
                                    color
                                }),
                                text: None,
                                border: None,
                            }),
                            disabled: None,
                        },
                    });
                }
                if thread.resolved {
                    children.push(::ducktape_view_guest::wire::Node::Button {
                        checked: None,
                        expanded: None,
                        description: None,
                        key: format!("{}/@button:190", use_scope),
                        content: ::ducktape_view_guest::wire::ButtonContent::Label(String::from(
                            "Reopen",
                        )),
                        label: Some(String::from("Reopen thread".to_owned())),
                        on_press: if ((self.busy || (!(self.host_error).is_empty()))
                            || (!(self.host_error).is_empty()))
                        {
                            None
                        } else {
                            Some(::ducktape_view_guest::slots::message(
                                (|event_0, event_1| Message::ResolveThreadSubmit(event_0, event_1))(
                                    thread.id.to_owned(),
                                    false,
                                ),
                            ))
                        },
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
                                        radius: Some([9.0; 4]),
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
                            active: ::ducktape_view_guest::wire::Face {
                                background: Some(::ducktape_view_guest::wire::Rgba([
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.000000,
                                ])),
                                text: Some(palette.colors[5]),
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
                            },
                            hovered: Some(::ducktape_view_guest::wire::Face {
                                background: Some({
                                    let mut color = palette.colors[4];
                                    color.0[3] = 0.090000;
                                    color
                                }),
                                text: Some(palette.colors[4]),
                                border: None,
                            }),
                            pressed: Some(::ducktape_view_guest::wire::Face {
                                background: Some({
                                    let mut color = palette.colors[4];
                                    color.0[3] = 0.140000;
                                    color
                                }),
                                text: None,
                                border: None,
                            }),
                            disabled: None,
                        },
                    });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: format!("{}/@layout:155", use_scope),
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
            });
            children.push(::ducktape_view_guest::wire::Node::Text {
                options: ::ducktape_view_guest::wire::TextOptions {
                    height: None,
                    align_y: None,
                    line_height: Some(::ducktape_view_guest::wire::LineHeight::Relative(
                        ((1.55) as f32).max(f32::EPSILON).min(f32::MAX),
                    )),
                    shaping: None,
                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::Word),
                    tracking: 0.0f32,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                    }),
                },
                key: format!("{}/@text:199", use_scope),
                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                color: Some(palette.colors[15]),
                font: ::ducktape_view_guest::wire::Font {
                    monospace: false,
                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                },
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                align_x: None,
                content: (crate::host::opener_text(::std::borrow::Borrow::borrow(&(thread))))
                    .to_string(),
            });
            if (!(crate::host::thread_replies(::std::borrow::Borrow::borrow(&(thread)), expanded))
                .is_empty())
            {
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
                                key: format!("{}/@container:214", use_scope),
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(::ducktape_view_guest::wire::Length::Fill),
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
                            .push({
                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                for (index, reply) in crate::host::thread_replies(
                                        ::std::borrow::Borrow::borrow(&(thread)),
                                        expanded,
                                    )
                                    .iter()
                                    .enumerate()
                                {
                                    let for_scope = format!(
                                        "{}/@for:1830({})", use_scope, index
                                    );
                                    children
                                        .push({
                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                            children
                                                .push({
                                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                    children
                                                        .push(
                                                            self
                                                                .comment_avatar(
                                                                    palette,
                                                                    format!("{}/PersonAvatar@1837", for_scope),
                                                                    crate::host::initials_of(
                                                                        ::std::convert::AsRef::as_ref(&(reply.author)),
                                                                    ),
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
                                                                        "Geist".into(),
                                                                    ),
                                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                }),
                                                            },
                                                            key: format!("{}/@text:233", for_scope),
                                                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                            color: Some(palette.colors[7]),
                                                            font: ::ducktape_view_guest::wire::Font {
                                                                monospace: false,
                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                            },
                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                            align_x: None,
                                                            content: (reply.author.to_owned()).to_string(),
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
                                                            key: format!("{}/@text:240", for_scope),
                                                            size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                            color: Some(palette.colors[73]),
                                                            font: ::ducktape_view_guest::wire::Font {
                                                                monospace: false,
                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                            },
                                                            width: None,
                                                            align_x: None,
                                                            content: (reply.meta.to_owned()).to_string(),
                                                        });
                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                        max_width: None,
                                                        clip: false,
                                                        key: format!("{}/@layout:223", for_scope),
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
                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::Word),
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
                                                    key: format!("{}/@text:246", for_scope),
                                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: Some(palette.colors[15]),
                                                    font: ::ducktape_view_guest::wire::Font {
                                                        monospace: false,
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    },
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    align_x: None,
                                                    content: (reply.text.to_owned()).to_string(),
                                                });
                                            ::ducktape_view_guest::wire::Node::Linear {
                                                max_width: None,
                                                clip: false,
                                                key: format!("{}/@layout:222", for_scope),
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
                                        });
                                }
                                ::ducktape_view_guest::wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:220", use_scope),
                                    wrap: None,
                                    axis: ::ducktape_view_guest::wire::Axis::Column,
                                    spacing: Some((8.0) as f32),
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
                            key: format!("{}/@layout:209", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: Some((12.0) as f32),
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (0.0) as f32,
                                right: (0.0) as f32,
                                bottom: (0.0) as f32,
                                left: (17.0) as f32,
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
            if ((!replying)
                && ((!thread.resolved)
                    || (!(crate::host::reply_toggle_label(
                        ::std::borrow::Borrow::borrow(&(thread)),
                        expanded,
                    ))
                    .is_empty())))
            {
                children.push({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    if (!(crate::host::reply_toggle_label(
                        ::std::borrow::Borrow::borrow(&(thread)),
                        expanded,
                    ))
                    .is_empty())
                    {
                        children.push(::ducktape_view_guest::wire::Node::Button {
                            checked: None,
                            expanded: Some(expanded),
                            description: None,
                            key: format!("{}/@button:265", use_scope),
                            content: ::ducktape_view_guest::wire::ButtonContent::Child(Box::new(
                                ::ducktape_view_guest::wire::Node::Text {
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
                                            stretch:
                                                ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:272", use_scope),
                                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[5]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: (crate::host::reply_toggle_label(
                                        ::std::borrow::Borrow::borrow(&(thread)),
                                        expanded,
                                    ))
                                    .to_string(),
                                },
                            )),
                            label: Some(String::from("Show every reply".to_owned())),
                            on_press: if (!(self.host_error).is_empty()) {
                                None
                            } else {
                                Some(::ducktape_view_guest::slots::message((|event_0| {
                                    Message::ToggleThreadReplies(event_0)
                                })(
                                    thread.id.to_owned(),
                                )))
                            },
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
                                active: ::ducktape_view_guest::wire::Face {
                                    background: Some(::ducktape_view_guest::wire::Rgba([
                                        0.0 / 255.0,
                                        0.0 / 255.0,
                                        0.0 / 255.0,
                                        0.000000,
                                    ])),
                                    text: Some(palette.colors[5]),
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
                                },
                                hovered: Some(::ducktape_view_guest::wire::Face {
                                    background: Some({
                                        let mut color = palette.colors[4];
                                        color.0[3] = 0.090000;
                                        color
                                    }),
                                    text: Some(palette.colors[4]),
                                    border: None,
                                }),
                                pressed: Some(::ducktape_view_guest::wire::Face {
                                    background: Some({
                                        let mut color = palette.colors[4];
                                        color.0[3] = 0.140000;
                                        color
                                    }),
                                    text: None,
                                    border: None,
                                }),
                                disabled: None,
                            },
                        });
                    }
                    if (!thread.resolved) {
                        children.push({
                            let node_scope = format!("{}/reply-on({})", use_scope, thread.id);
                            ::ducktape_view_guest::wire::Node::Button {
                                checked: None,
                                expanded: None,
                                description: None,
                                key: node_scope.clone(),
                                content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                    String::from("Reply"),
                                ),
                                label: Some(String::from("Reply to this thread".to_owned())),
                                on_press: if ((self.busy || (!(self.host_error).is_empty()))
                                    || (!(self.host_error).is_empty()))
                                {
                                    None
                                } else {
                                    Some(::ducktape_view_guest::slots::message((|event_0| {
                                        Message::SelectReplyThread(event_0)
                                    })(
                                        thread.id.to_owned(),
                                    )))
                                },
                                width: None,
                                height: None,
                                padding: Some(::ducktape_view_guest::wire::Edges::all(
                                    (4.0) as f32,
                                )),
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
                                        text_size: Some(11.0f32),
                                        line_height: Some(1.35f32),
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                            stretch:
                                                ::ducktape_view_guest::wire::FontStretch::Normal,
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
                                            color: None,
                                            width: None,
                                            radius: Some([
                                                ((6.0) as f32).max(0.0).min(f32::MAX),
                                                ((6.0) as f32).max(0.0).min(f32::MAX),
                                                ((6.0) as f32).max(0.0).min(f32::MAX),
                                                ((6.0) as f32).max(0.0).min(f32::MAX),
                                            ]),
                                        }),
                                    },
                                    hovered: Some(::ducktape_view_guest::wire::Face {
                                        background: Some({
                                            let mut color = palette.colors[4];
                                            color.0[3] = 0.090000;
                                            color
                                        }),
                                        text: Some(palette.colors[4]),
                                        border: None,
                                    }),
                                    pressed: Some(::ducktape_view_guest::wire::Face {
                                        background: Some({
                                            let mut color = palette.colors[4];
                                            color.0[3] = 0.140000;
                                            color
                                        }),
                                        text: None,
                                        border: None,
                                    }),
                                    disabled: None,
                                },
                            }
                        });
                    }
                    children.push(::ducktape_view_guest::wire::Node::Space {
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:259", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((2.0) as f32),
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
            if ((!thread.resolved) && replying) {
                children.push({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children.push({
                        let node_scope = format!("{}/thread-reply({})", use_scope, thread.id);
                        ::ducktape_view_guest::wire::Node::Input {
                            options: ::ducktape_view_guest::wire::InputOptions {
                                label: ("Reply".to_owned()).to_string(),
                                description: None,
                                disabled: ((self.busy || (!(self.host_error).is_empty()))
                                    || (!(self.host_error).is_empty())),
                                padding: Some(::ducktape_view_guest::wire::Edges::all(
                                    (6.2) as f32,
                                )),
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
                            placeholder: String::from("Reply…".to_owned()),
                            value: (self.reply_draft).to_string(),
                            on_input: ::ducktape_view_guest::slots::handler::<String, Message>(
                                Box::new({
                                    let route = Message::ReplyDraftChanged as fn(String) -> Message;
                                    move |sent: String| Some(route(sent))
                                }),
                            ),
                            on_submit: Some(::ducktape_view_guest::slots::message((|event_0| {
                                Message::PostThreadReply(event_0)
                            })(
                                thread.id.to_owned(),
                            ))),
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
                                    background: Some(::ducktape_view_guest::wire::Rgba([
                                        0.0 / 255.0,
                                        0.0 / 255.0,
                                        0.0 / 255.0,
                                        0.000000,
                                    ])),
                                    border: Some(::ducktape_view_guest::wire::Border {
                                        color: Some({
                                            let mut color = palette.colors[4];
                                            color.0[3] = 0.080000;
                                            color
                                        }),
                                        width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                        radius: Some([
                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                        ]),
                                    }),
                                    value: Some(palette.colors[4]),
                                    placeholder: Some(palette.colors[5]),
                                    selection: Some({
                                        let mut color = palette.colors[4];
                                        color.0[3] = 0.180000;
                                        color
                                    }),
                                },
                                hovered: Some(::ducktape_view_guest::wire::InputFace {
                                    icon: None,
                                    background: Some({
                                        let mut color = palette.colors[4];
                                        color.0[3] = 0.040000;
                                        color
                                    }),
                                    border: Some(::ducktape_view_guest::wire::Border {
                                        color: Some({
                                            let mut color = palette.colors[4];
                                            color.0[3] = 0.110000;
                                            color
                                        }),
                                        width: None,
                                        radius: None,
                                    }),
                                    value: None,
                                    placeholder: None,
                                    selection: None,
                                }),
                                focused: Some(::ducktape_view_guest::wire::InputFace {
                                    icon: None,
                                    background: Some({
                                        let mut color = palette.colors[4];
                                        color.0[3] = 0.040000;
                                        color
                                    }),
                                    border: Some(::ducktape_view_guest::wire::Border {
                                        color: Some(palette.colors[42]),
                                        width: None,
                                        radius: None,
                                    }),
                                    value: None,
                                    placeholder: None,
                                    selection: None,
                                }),
                                disabled: Some(::ducktape_view_guest::wire::InputFace {
                                    icon: None,
                                    background: None,
                                    border: None,
                                    value: Some(palette.colors[5]),
                                    placeholder: None,
                                    selection: None,
                                }),
                            }),
                        }
                    });
                    children.push(::ducktape_view_guest::wire::Node::Button {
                        checked: None,
                        expanded: None,
                        description: None,
                        key: format!("{}/@button:313", use_scope),
                        content: ::ducktape_view_guest::wire::ButtonContent::Label(String::from(
                            "Post",
                        )),
                        label: Some(String::from("Post reply".to_owned())),
                        on_press: if (((self.busy || (!(self.host_error).is_empty()))
                            || (!(self.host_error).is_empty()))
                            || ((self.reply_draft).trim().to_owned()).is_empty())
                        {
                            None
                        } else {
                            Some(::ducktape_view_guest::slots::message((|event_0| {
                                Message::PostThreadReply(event_0)
                            })(
                                thread.id.to_owned(),
                            )))
                        },
                        width: None,
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges::all((5.0) as f32)),
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
                        key: format!("{}/@layout:293", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((5.0) as f32),
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
                key: format!("{}/@layout:154", use_scope),
                wrap: None,
                axis: ::ducktape_view_guest::wire::Axis::Column,
                spacing: Some((8.0) as f32),
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
    pub(crate) fn resolved_thread(
        &self,
        palette: Palette,
        use_scope: String,
        thread: crate::host::PageCommentThread,
        expanded: bool,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
            children.push({
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children.push(self.comment_avatar(
                    palette,
                    format!("{}/PersonAvatar@1769", use_scope),
                    crate::host::initials_of(::std::convert::AsRef::as_ref(&(thread.author))),
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
                    key: format!("{}/@text:165", use_scope),
                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[7]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: (thread.author.to_owned()).to_string(),
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
                    key: format!("{}/@text:171", use_scope),
                    size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[73]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: (thread.meta.to_owned()).to_string(),
                });
                children.push(::ducktape_view_guest::wire::Node::Space {
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                });
                if (!thread.resolved) {
                    children.push(::ducktape_view_guest::wire::Node::Button {
                        checked: None,
                        expanded: None,
                        description: None,
                        key: format!("{}/@button:180", use_scope),
                        content: ::ducktape_view_guest::wire::ButtonContent::Label(String::from(
                            "Resolve",
                        )),
                        label: Some(String::from("Resolve thread".to_owned())),
                        on_press: if ((self.busy || (!(self.host_error).is_empty()))
                            || (!(self.host_error).is_empty()))
                        {
                            None
                        } else {
                            Some(::ducktape_view_guest::slots::message(
                                (|event_0, event_1| Message::ResolveThreadSubmit(event_0, event_1))(
                                    thread.id.to_owned(),
                                    true,
                                ),
                            ))
                        },
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
                                        radius: Some([9.0; 4]),
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
                            active: ::ducktape_view_guest::wire::Face {
                                background: Some(::ducktape_view_guest::wire::Rgba([
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.000000,
                                ])),
                                text: Some(palette.colors[5]),
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
                            },
                            hovered: Some(::ducktape_view_guest::wire::Face {
                                background: Some({
                                    let mut color = palette.colors[4];
                                    color.0[3] = 0.090000;
                                    color
                                }),
                                text: Some(palette.colors[4]),
                                border: None,
                            }),
                            pressed: Some(::ducktape_view_guest::wire::Face {
                                background: Some({
                                    let mut color = palette.colors[4];
                                    color.0[3] = 0.140000;
                                    color
                                }),
                                text: None,
                                border: None,
                            }),
                            disabled: None,
                        },
                    });
                }
                if thread.resolved {
                    children.push(::ducktape_view_guest::wire::Node::Button {
                        checked: None,
                        expanded: None,
                        description: None,
                        key: format!("{}/@button:190", use_scope),
                        content: ::ducktape_view_guest::wire::ButtonContent::Label(String::from(
                            "Reopen",
                        )),
                        label: Some(String::from("Reopen thread".to_owned())),
                        on_press: if ((self.busy || (!(self.host_error).is_empty()))
                            || (!(self.host_error).is_empty()))
                        {
                            None
                        } else {
                            Some(::ducktape_view_guest::slots::message(
                                (|event_0, event_1| Message::ResolveThreadSubmit(event_0, event_1))(
                                    thread.id.to_owned(),
                                    false,
                                ),
                            ))
                        },
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
                                        radius: Some([9.0; 4]),
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
                            active: ::ducktape_view_guest::wire::Face {
                                background: Some(::ducktape_view_guest::wire::Rgba([
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.000000,
                                ])),
                                text: Some(palette.colors[5]),
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
                            },
                            hovered: Some(::ducktape_view_guest::wire::Face {
                                background: Some({
                                    let mut color = palette.colors[4];
                                    color.0[3] = 0.090000;
                                    color
                                }),
                                text: Some(palette.colors[4]),
                                border: None,
                            }),
                            pressed: Some(::ducktape_view_guest::wire::Face {
                                background: Some({
                                    let mut color = palette.colors[4];
                                    color.0[3] = 0.140000;
                                    color
                                }),
                                text: None,
                                border: None,
                            }),
                            disabled: None,
                        },
                    });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: format!("{}/@layout:155", use_scope),
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
            });
            children.push(::ducktape_view_guest::wire::Node::Text {
                options: ::ducktape_view_guest::wire::TextOptions {
                    height: None,
                    align_y: None,
                    line_height: Some(::ducktape_view_guest::wire::LineHeight::Relative(
                        ((1.55) as f32).max(f32::EPSILON).min(f32::MAX),
                    )),
                    shaping: None,
                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::Word),
                    tracking: 0.0f32,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                    }),
                },
                key: format!("{}/@text:199", use_scope),
                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                color: Some(palette.colors[15]),
                font: ::ducktape_view_guest::wire::Font {
                    monospace: false,
                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                },
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                align_x: None,
                content: (crate::host::opener_text(::std::borrow::Borrow::borrow(&(thread))))
                    .to_string(),
            });
            if (!(crate::host::thread_replies(::std::borrow::Borrow::borrow(&(thread)), expanded))
                .is_empty())
            {
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
                                key: format!("{}/@container:214", use_scope),
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(::ducktape_view_guest::wire::Length::Fill),
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
                            .push({
                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                for (index, reply) in crate::host::thread_replies(
                                        ::std::borrow::Borrow::borrow(&(thread)),
                                        expanded,
                                    )
                                    .iter()
                                    .enumerate()
                                {
                                    let for_scope = format!(
                                        "{}/@for:1830({})", use_scope, index
                                    );
                                    children
                                        .push({
                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                            children
                                                .push({
                                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                    children
                                                        .push(
                                                            self
                                                                .comment_avatar(
                                                                    palette,
                                                                    format!("{}/PersonAvatar@1837", for_scope),
                                                                    crate::host::initials_of(
                                                                        ::std::convert::AsRef::as_ref(&(reply.author)),
                                                                    ),
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
                                                                        "Geist".into(),
                                                                    ),
                                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                }),
                                                            },
                                                            key: format!("{}/@text:233", for_scope),
                                                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                            color: Some(palette.colors[7]),
                                                            font: ::ducktape_view_guest::wire::Font {
                                                                monospace: false,
                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                            },
                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                            align_x: None,
                                                            content: (reply.author.to_owned()).to_string(),
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
                                                            key: format!("{}/@text:240", for_scope),
                                                            size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                            color: Some(palette.colors[73]),
                                                            font: ::ducktape_view_guest::wire::Font {
                                                                monospace: false,
                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                            },
                                                            width: None,
                                                            align_x: None,
                                                            content: (reply.meta.to_owned()).to_string(),
                                                        });
                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                        max_width: None,
                                                        clip: false,
                                                        key: format!("{}/@layout:223", for_scope),
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
                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::Word),
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
                                                    key: format!("{}/@text:246", for_scope),
                                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: Some(palette.colors[15]),
                                                    font: ::ducktape_view_guest::wire::Font {
                                                        monospace: false,
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    },
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    align_x: None,
                                                    content: (reply.text.to_owned()).to_string(),
                                                });
                                            ::ducktape_view_guest::wire::Node::Linear {
                                                max_width: None,
                                                clip: false,
                                                key: format!("{}/@layout:222", for_scope),
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
                                        });
                                }
                                ::ducktape_view_guest::wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:220", use_scope),
                                    wrap: None,
                                    axis: ::ducktape_view_guest::wire::Axis::Column,
                                    spacing: Some((8.0) as f32),
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
                            key: format!("{}/@layout:209", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: Some((12.0) as f32),
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (0.0) as f32,
                                right: (0.0) as f32,
                                bottom: (0.0) as f32,
                                left: (17.0) as f32,
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
            if ((!false)
                && ((!thread.resolved)
                    || (!(crate::host::reply_toggle_label(
                        ::std::borrow::Borrow::borrow(&(thread)),
                        expanded,
                    ))
                    .is_empty())))
            {
                children.push({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    if (!(crate::host::reply_toggle_label(
                        ::std::borrow::Borrow::borrow(&(thread)),
                        expanded,
                    ))
                    .is_empty())
                    {
                        children.push(::ducktape_view_guest::wire::Node::Button {
                            checked: None,
                            expanded: Some(expanded),
                            description: None,
                            key: format!("{}/@button:265", use_scope),
                            content: ::ducktape_view_guest::wire::ButtonContent::Child(Box::new(
                                ::ducktape_view_guest::wire::Node::Text {
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
                                            stretch:
                                                ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:272", use_scope),
                                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[5]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: (crate::host::reply_toggle_label(
                                        ::std::borrow::Borrow::borrow(&(thread)),
                                        expanded,
                                    ))
                                    .to_string(),
                                },
                            )),
                            label: Some(String::from("Show every reply".to_owned())),
                            on_press: if (!(self.host_error).is_empty()) {
                                None
                            } else {
                                Some(::ducktape_view_guest::slots::message((|event_0| {
                                    Message::ToggleThreadReplies(event_0)
                                })(
                                    thread.id.to_owned(),
                                )))
                            },
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
                                active: ::ducktape_view_guest::wire::Face {
                                    background: Some(::ducktape_view_guest::wire::Rgba([
                                        0.0 / 255.0,
                                        0.0 / 255.0,
                                        0.0 / 255.0,
                                        0.000000,
                                    ])),
                                    text: Some(palette.colors[5]),
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
                                },
                                hovered: Some(::ducktape_view_guest::wire::Face {
                                    background: Some({
                                        let mut color = palette.colors[4];
                                        color.0[3] = 0.090000;
                                        color
                                    }),
                                    text: Some(palette.colors[4]),
                                    border: None,
                                }),
                                pressed: Some(::ducktape_view_guest::wire::Face {
                                    background: Some({
                                        let mut color = palette.colors[4];
                                        color.0[3] = 0.140000;
                                        color
                                    }),
                                    text: None,
                                    border: None,
                                }),
                                disabled: None,
                            },
                        });
                    }
                    if (!thread.resolved) {
                        children.push({
                            let node_scope = format!("{}/reply-on({})", use_scope, thread.id);
                            ::ducktape_view_guest::wire::Node::Button {
                                checked: None,
                                expanded: None,
                                description: None,
                                key: node_scope.clone(),
                                content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                    String::from("Reply"),
                                ),
                                label: Some(String::from("Reply to this thread".to_owned())),
                                on_press: if ((self.busy || (!(self.host_error).is_empty()))
                                    || (!(self.host_error).is_empty()))
                                {
                                    None
                                } else {
                                    Some(::ducktape_view_guest::slots::message((|event_0| {
                                        Message::SelectReplyThread(event_0)
                                    })(
                                        thread.id.to_owned(),
                                    )))
                                },
                                width: None,
                                height: None,
                                padding: Some(::ducktape_view_guest::wire::Edges::all(
                                    (4.0) as f32,
                                )),
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
                                        text_size: Some(11.0f32),
                                        line_height: Some(1.35f32),
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                            stretch:
                                                ::ducktape_view_guest::wire::FontStretch::Normal,
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
                                            color: None,
                                            width: None,
                                            radius: Some([
                                                ((6.0) as f32).max(0.0).min(f32::MAX),
                                                ((6.0) as f32).max(0.0).min(f32::MAX),
                                                ((6.0) as f32).max(0.0).min(f32::MAX),
                                                ((6.0) as f32).max(0.0).min(f32::MAX),
                                            ]),
                                        }),
                                    },
                                    hovered: Some(::ducktape_view_guest::wire::Face {
                                        background: Some({
                                            let mut color = palette.colors[4];
                                            color.0[3] = 0.090000;
                                            color
                                        }),
                                        text: Some(palette.colors[4]),
                                        border: None,
                                    }),
                                    pressed: Some(::ducktape_view_guest::wire::Face {
                                        background: Some({
                                            let mut color = palette.colors[4];
                                            color.0[3] = 0.140000;
                                            color
                                        }),
                                        text: None,
                                        border: None,
                                    }),
                                    disabled: None,
                                },
                            }
                        });
                    }
                    children.push(::ducktape_view_guest::wire::Node::Space {
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:259", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((2.0) as f32),
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
            if ((!thread.resolved) && false) {
                children.push({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children.push({
                        let node_scope = format!("{}/thread-reply({})", use_scope, thread.id);
                        ::ducktape_view_guest::wire::Node::Input {
                            options: ::ducktape_view_guest::wire::InputOptions {
                                label: ("Reply".to_owned()).to_string(),
                                description: None,
                                disabled: ((self.busy || (!(self.host_error).is_empty()))
                                    || (!(self.host_error).is_empty())),
                                padding: Some(::ducktape_view_guest::wire::Edges::all(
                                    (6.2) as f32,
                                )),
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
                            placeholder: String::from("Reply…".to_owned()),
                            value: (self.reply_draft).to_string(),
                            on_input: ::ducktape_view_guest::slots::handler::<String, Message>(
                                Box::new({
                                    let route = Message::ReplyDraftChanged as fn(String) -> Message;
                                    move |sent: String| Some(route(sent))
                                }),
                            ),
                            on_submit: Some(::ducktape_view_guest::slots::message((|event_0| {
                                Message::PostThreadReply(event_0)
                            })(
                                thread.id.to_owned(),
                            ))),
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
                                    background: Some(::ducktape_view_guest::wire::Rgba([
                                        0.0 / 255.0,
                                        0.0 / 255.0,
                                        0.0 / 255.0,
                                        0.000000,
                                    ])),
                                    border: Some(::ducktape_view_guest::wire::Border {
                                        color: Some({
                                            let mut color = palette.colors[4];
                                            color.0[3] = 0.080000;
                                            color
                                        }),
                                        width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                        radius: Some([
                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                        ]),
                                    }),
                                    value: Some(palette.colors[4]),
                                    placeholder: Some(palette.colors[5]),
                                    selection: Some({
                                        let mut color = palette.colors[4];
                                        color.0[3] = 0.180000;
                                        color
                                    }),
                                },
                                hovered: Some(::ducktape_view_guest::wire::InputFace {
                                    icon: None,
                                    background: Some({
                                        let mut color = palette.colors[4];
                                        color.0[3] = 0.040000;
                                        color
                                    }),
                                    border: Some(::ducktape_view_guest::wire::Border {
                                        color: Some({
                                            let mut color = palette.colors[4];
                                            color.0[3] = 0.110000;
                                            color
                                        }),
                                        width: None,
                                        radius: None,
                                    }),
                                    value: None,
                                    placeholder: None,
                                    selection: None,
                                }),
                                focused: Some(::ducktape_view_guest::wire::InputFace {
                                    icon: None,
                                    background: Some({
                                        let mut color = palette.colors[4];
                                        color.0[3] = 0.040000;
                                        color
                                    }),
                                    border: Some(::ducktape_view_guest::wire::Border {
                                        color: Some(palette.colors[42]),
                                        width: None,
                                        radius: None,
                                    }),
                                    value: None,
                                    placeholder: None,
                                    selection: None,
                                }),
                                disabled: Some(::ducktape_view_guest::wire::InputFace {
                                    icon: None,
                                    background: None,
                                    border: None,
                                    value: Some(palette.colors[5]),
                                    placeholder: None,
                                    selection: None,
                                }),
                            }),
                        }
                    });
                    children.push(::ducktape_view_guest::wire::Node::Button {
                        checked: None,
                        expanded: None,
                        description: None,
                        key: format!("{}/@button:313", use_scope),
                        content: ::ducktape_view_guest::wire::ButtonContent::Label(String::from(
                            "Post",
                        )),
                        label: Some(String::from("Post reply".to_owned())),
                        on_press: if (((self.busy || (!(self.host_error).is_empty()))
                            || (!(self.host_error).is_empty()))
                            || ((self.reply_draft).trim().to_owned()).is_empty())
                        {
                            None
                        } else {
                            Some(::ducktape_view_guest::slots::message((|event_0| {
                                Message::PostThreadReply(event_0)
                            })(
                                thread.id.to_owned(),
                            )))
                        },
                        width: None,
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges::all((5.0) as f32)),
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
                        key: format!("{}/@layout:293", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((5.0) as f32),
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
                key: format!("{}/@layout:154", use_scope),
                wrap: None,
                axis: ::ducktape_view_guest::wire::Axis::Column,
                spacing: Some((8.0) as f32),
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
