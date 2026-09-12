impl PagesView {
    pub(crate) fn sidebar_header(&self, palette: Palette, use_scope: String) -> wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<wire::Node> = Vec::new();
                children.push(wire::Node::Container {
                    shadow: wire::Shadow {
                        color: None,
                        x: None,
                        y: None,
                        blur: None,
                    },
                    max_width: None,
                    max_height: None,
                    clip: false,
                    key: format!("{}/@container:7", use_scope),
                    width: Some(wire::Length::Fill),
                    height: Some(wire::Length::Fixed((50.0) as f32)),
                    padding: Some(wire::Edges {
                        top: (0.0) as f32,
                        right: (14.0) as f32,
                        bottom: (0.0) as f32,
                        left: (14.0) as f32,
                    }),
                    align_x: None,
                    align_y: None,
                    background: (None).map(wire::Background::Color),
                    border: None,
                    snap: None,
                    content: Box::new({
                        let mut children: Vec<wire::Node> = Vec::new();
                        children.push(wire::Node::Text {
                            options: wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: Some(wire::Wrapping::None),
                                tracking: 0.0f32,
                                font: Some(wire::NamedFont {
                                    family: wire::FontFamily::Named("Geist".into()),
                                    weight: wire::Weight::Semibold,
                                    stretch: wire::FontStretch::Normal,
                                    style: wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:19", use_scope),
                            size: Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[4]),
                            font: wire::Font {
                                monospace: false,
                                weight: wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: "Pages".to_owned(),
                        });
                        children.push(wire::Node::Text {
                            options: wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: Some(wire::Wrapping::None),
                                tracking: 0.0f32,
                                font: Some(wire::NamedFont {
                                    family: wire::FontFamily::Named("Geist Mono".into()),
                                    weight: wire::Weight::Medium,
                                    stretch: wire::FontStretch::Normal,
                                    style: wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:25", use_scope),
                            size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[72]),
                            font: wire::Font {
                                monospace: false,
                                weight: wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ((self.pages).len() as i64).to_string(),
                        });
                        children.push(wire::Node::Space {
                            width: Some(wire::Length::Fill),
                            height: None,
                        });
                        children.push({
                            let mut children: Vec<wire::Node> = Vec::new();
                            if (!self.page_create_open) {
                                children.push(wire::Node::Button {
                                    checked: None,
                                    expanded: Some(self.page_create_open),
                                    description: None,
                                    key: format!("{}/@button:52", use_scope),
                                    content: wire::ButtonContent::Child(Box::new(self.icon(
                                        palette,
                                        format!("{}/Icon@680", use_scope),
                                        "plus",
                                        16.,
                                    ))),
                                    label: Some(String::from("New page".to_owned())),
                                    on_press: if ((self.loading
                                        || (self.busy || (!(self.host_error).is_empty())))
                                        || (!self.connected))
                                    {
                                        None
                                    } else {
                                        Some(::ducktape_view_guest::slots::message(
                                            Message::TogglePageCreate,
                                        ))
                                    },
                                    width: None,
                                    height: None,
                                    padding: Some(wire::Edges::all((0.0) as f32)),
                                    style: wire::ButtonStyle {
                                        preset: wire::ButtonPreset::Primary,
                                        recipe: Some(wire::ButtonRecipe {
                                            base: wire::Face {
                                                background: Some(wire::Rgba([
                                                    0.0 / 255.0,
                                                    0.0 / 255.0,
                                                    0.0 / 255.0,
                                                    0.000000,
                                                ])),
                                                text: Some(palette.colors[4]),
                                                border: Some(wire::Border {
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
                                            font: Some(wire::NamedFont {
                                                family: wire::FontFamily::Named("Geist".into()),
                                                weight: wire::Weight::Normal,
                                                stretch: wire::FontStretch::Normal,
                                                style: wire::FontStyle::Normal,
                                            }),
                                        }),
                                        active: wire::Face {
                                            background: Some(wire::Rgba([
                                                0.0 / 255.0,
                                                0.0 / 255.0,
                                                0.0 / 255.0,
                                                0.000000,
                                            ])),
                                            text: Some(palette.colors[5]),
                                            border: Some(wire::Border {
                                                color: Some(wire::Rgba([
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
                                        hovered: Some(wire::Face {
                                            background: Some(palette.colors[60]),
                                            text: Some(palette.colors[4]),
                                            border: None,
                                        }),
                                        pressed: Some(wire::Face {
                                            background: Some(palette.colors[56]),
                                            text: Some(palette.colors[4]),
                                            border: None,
                                        }),
                                        disabled: None,
                                    },
                                });
                            }
                            if self.page_create_open {
                                children.push(wire::Node::Button {
                                    checked: None,
                                    expanded: Some(self.page_create_open),
                                    description: None,
                                    key: format!("{}/@button:68", use_scope),
                                    content: wire::ButtonContent::Child(Box::new(
                                        wire::Node::Container {
                                            shadow: wire::Shadow {
                                                color: None,
                                                x: None,
                                                y: None,
                                                blur: None,
                                            },
                                            max_width: None,
                                            max_height: None,
                                            clip: false,
                                            key: format!("{}/@container:77", use_scope),
                                            width: Some(wire::Length::Fill),
                                            height: Some(wire::Length::Fill),
                                            padding: None,
                                            align_x: Some(wire::AlignX::Center),
                                            align_y: Some(wire::AlignY::Center),
                                            background: (None).map(wire::Background::Color),
                                            border: None,
                                            snap: None,
                                            content: Box::new(wire::Node::Text {
                                                options: wire::TextOptions {
                                                    height: None,
                                                    align_y: None,
                                                    line_height: None,
                                                    shaping: None,
                                                    wrapping: Some(wire::Wrapping::None),
                                                    tracking: 0.0f32,
                                                    font: Some(wire::NamedFont {
                                                        family: wire::FontFamily::Named(
                                                            "Geist".into(),
                                                        ),
                                                        weight: wire::Weight::Normal,
                                                        stretch: wire::FontStretch::Normal,
                                                        style: wire::FontStyle::Normal,
                                                    }),
                                                },
                                                key: format!("{}/@text:83", use_scope),
                                                size: Some(
                                                    ((13.0) as f32).max(f32::EPSILON).min(f32::MAX),
                                                ),
                                                color: Some(palette.colors[5]),
                                                font: wire::Font {
                                                    monospace: false,
                                                    weight: wire::Weight::Normal,
                                                },
                                                width: None,
                                                align_x: None,
                                                content: "×".to_owned(),
                                            }),
                                        },
                                    )),
                                    label: Some(String::from("Close new page".to_owned())),
                                    on_press: if (self.loading
                                        || (self.busy || (!(self.host_error).is_empty())))
                                    {
                                        None
                                    } else {
                                        Some(::ducktape_view_guest::slots::message(
                                            Message::TogglePageCreate,
                                        ))
                                    },
                                    width: Some(wire::Length::Fixed((24.0) as f32)),
                                    height: Some(wire::Length::Fixed((24.0) as f32)),
                                    padding: Some(wire::Edges::all((0.0) as f32)),
                                    style: wire::ButtonStyle {
                                        preset: wire::ButtonPreset::Primary,
                                        recipe: Some(wire::ButtonRecipe {
                                            base: wire::Face {
                                                background: Some(wire::Rgba([
                                                    0.0 / 255.0,
                                                    0.0 / 255.0,
                                                    0.0 / 255.0,
                                                    0.000000,
                                                ])),
                                                text: Some(palette.colors[4]),
                                                border: Some(wire::Border {
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
                                            font: Some(wire::NamedFont {
                                                family: wire::FontFamily::Named("Geist".into()),
                                                weight: wire::Weight::Normal,
                                                stretch: wire::FontStretch::Normal,
                                                style: wire::FontStyle::Normal,
                                            }),
                                        }),
                                        active: wire::Face {
                                            background: Some(palette.colors[60]),
                                            text: Some(palette.colors[5]),
                                            border: Some(wire::Border {
                                                color: Some(wire::Rgba([
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
                                        hovered: Some(wire::Face {
                                            background: Some(palette.colors[56]),
                                            text: Some(palette.colors[4]),
                                            border: None,
                                        }),
                                        pressed: Some(wire::Face {
                                            background: Some(palette.colors[56]),
                                            text: Some(palette.colors[4]),
                                            border: None,
                                        }),
                                        disabled: None,
                                    },
                                });
                            }
                            wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:50", use_scope),
                                wrap: None,
                                axis: wire::Axis::Column,
                                spacing: None,
                                padding: None,
                                width: None,
                                height: None,
                                align: None,
                                background: None,
                                border: None,
                                children,
                            }
                        });
                        wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:13", use_scope),
                            wrap: None,
                            axis: wire::Axis::Row,
                            spacing: Some((8.0) as f32),
                            padding: None,
                            width: Some(wire::Length::Fill),
                            height: Some(wire::Length::Fill),
                            align: Some(wire::AlignX::Center),
                            background: None,
                            border: None,
                            children,
                        }
                    }),
                });
                children.push(wire::Node::Container {
                    shadow: wire::Shadow {
                        color: None,
                        x: None,
                        y: None,
                        blur: None,
                    },
                    max_width: None,
                    max_height: None,
                    clip: false,
                    key: format!("{}/@container:33", use_scope),
                    width: Some(wire::Length::Fill),
                    height: Some(wire::Length::Fixed((1.0) as f32)),
                    padding: None,
                    align_x: None,
                    align_y: None,
                    background: (Some(palette.colors[60])).map(wire::Background::Color),
                    border: None,
                    snap: None,
                    content: Box::new(wire::Node::Space {
                        width: Some(wire::Length::Fixed((1.0) as f32)),
                        height: Some(wire::Length::Fixed((1.0) as f32)),
                    }),
                });
                wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children,
                }
            }
        }
    }
    pub(crate) fn connection_empty(&self, palette: Palette, use_scope: String) -> wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            wire::Node::Container {
                shadow: wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: Some(wire::Length::Fill),
                height: Some(wire::Length::Fill),
                padding: Some(wire::Edges {
                    top: (22.0) as f32,
                    right: (22.0) as f32,
                    bottom: (22.0) as f32,
                    left: (22.0) as f32,
                }),
                align_x: Some(wire::AlignX::Center),
                align_y: Some(wire::AlignY::Center),
                background: (None).map(wire::Background::Color),
                border: None,
                snap: None,
                content: Box::new({
                    let mut children: Vec<wire::Node> = Vec::new();
                    children.push(wire::Node::Container {
                        shadow: wire::Shadow {
                            color: None,
                            x: None,
                            y: None,
                            blur: None,
                        },
                        max_width: None,
                        max_height: None,
                        clip: false,
                        key: format!("{}/@container:53", use_scope),
                        width: Some(wire::Length::Fixed((42.0) as f32)),
                        height: Some(wire::Length::Fixed((42.0) as f32)),
                        padding: None,
                        align_x: Some(wire::AlignX::Center),
                        align_y: Some(wire::AlignY::Center),
                        background: (Some(palette.colors[3])).map(wire::Background::Color),
                        border: Some(wire::Border {
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
                        content: Box::new(wire::Node::Text {
                            options: wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: None,
                                tracking: 0.0f32,
                                font: Some(wire::NamedFont {
                                    family: wire::FontFamily::Named("Geist".into()),
                                    weight: wire::Weight::Normal,
                                    stretch: wire::FontStretch::Normal,
                                    style: wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:63", use_scope),
                            size: Some(((20.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[7]),
                            font: wire::Font {
                                monospace: false,
                                weight: wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: "◇".to_owned(),
                        }),
                    });
                    children.push(wire::Node::Text {
                        options: wire::TextOptions {
                            height: None,
                            align_y: None,
                            line_height: Some(wire::LineHeight::Relative(1.35f32)),
                            shaping: None,
                            wrapping: None,
                            tracking: 0.0f32,
                            font: Some(wire::NamedFont {
                                family: wire::FontFamily::Named("Geist".into()),
                                weight: wire::Weight::Semibold,
                                stretch: wire::FontStretch::Normal,
                                style: wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:64", use_scope),
                        size: Some(16.0f32),
                        color: Some(palette.colors[4]),
                        font: wire::Font {
                            monospace: false,
                            weight: wire::Weight::Semibold,
                        },
                        width: None,
                        align_x: None,
                        content: "Not connected".to_owned(),
                    });
                    children.push(wire::Node::Text {
                        options: wire::TextOptions {
                            height: None,
                            align_y: None,
                            line_height: Some(wire::LineHeight::Relative(1.5f32)),
                            shaping: None,
                            wrapping: None,
                            tracking: 0.0f32,
                            font: Some(wire::NamedFont {
                                family: wire::FontFamily::Named("Geist".into()),
                                weight: wire::Weight::Normal,
                                stretch: wire::FontStretch::Normal,
                                style: wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:65", use_scope),
                        size: Some(12.5f32),
                        color: Some(palette.colors[5]),
                        font: wire::Font {
                            monospace: false,
                            weight: wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content:
                            "Click the network name in the titlebar to pick or reconnect a network."
                                .to_owned(),
                    });
                    wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:48", use_scope),
                        wrap: None,
                        axis: wire::Axis::Column,
                        spacing: Some((7.0) as f32),
                        padding: None,
                        width: Some(wire::Length::Fill),
                        height: None,
                        align: Some(wire::AlignX::Center),
                        background: None,
                        border: None,
                        children,
                    }
                }),
            }
        }
    }
    pub(crate) fn loading_empty(&self, palette: Palette, use_scope: String) -> wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            wire::Node::Container {
                shadow: wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: Some(wire::Length::Fill),
                height: Some(wire::Length::Fill),
                padding: Some(wire::Edges {
                    top: (22.0) as f32,
                    right: (22.0) as f32,
                    bottom: (22.0) as f32,
                    left: (22.0) as f32,
                }),
                align_x: Some(wire::AlignX::Center),
                align_y: Some(wire::AlignY::Center),
                background: (None).map(wire::Background::Color),
                border: None,
                snap: None,
                content: Box::new({
                    let mut children: Vec<wire::Node> = Vec::new();
                    children.push(wire::Node::Container {
                        shadow: wire::Shadow {
                            color: None,
                            x: None,
                            y: None,
                            blur: None,
                        },
                        max_width: None,
                        max_height: None,
                        clip: false,
                        key: format!("{}/@container:53", use_scope),
                        width: Some(wire::Length::Fixed((42.0) as f32)),
                        height: Some(wire::Length::Fixed((42.0) as f32)),
                        padding: None,
                        align_x: Some(wire::AlignX::Center),
                        align_y: Some(wire::AlignY::Center),
                        background: (Some(palette.colors[3])).map(wire::Background::Color),
                        border: Some(wire::Border {
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
                        content: Box::new(wire::Node::Text {
                            options: wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: None,
                                tracking: 0.0f32,
                                font: Some(wire::NamedFont {
                                    family: wire::FontFamily::Named("Geist".into()),
                                    weight: wire::Weight::Normal,
                                    stretch: wire::FontStretch::Normal,
                                    style: wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:63", use_scope),
                            size: Some(((20.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[7]),
                            font: wire::Font {
                                monospace: false,
                                weight: wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: "◇".to_owned(),
                        }),
                    });
                    children.push(wire::Node::Text {
                        options: wire::TextOptions {
                            height: None,
                            align_y: None,
                            line_height: Some(wire::LineHeight::Relative(1.35f32)),
                            shaping: None,
                            wrapping: None,
                            tracking: 0.0f32,
                            font: Some(wire::NamedFont {
                                family: wire::FontFamily::Named("Geist".into()),
                                weight: wire::Weight::Semibold,
                                stretch: wire::FontStretch::Normal,
                                style: wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:64", use_scope),
                        size: Some(16.0f32),
                        color: Some(palette.colors[4]),
                        font: wire::Font {
                            monospace: false,
                            weight: wire::Weight::Semibold,
                        },
                        width: None,
                        align_x: None,
                        content: "Loading pages…".to_owned(),
                    });
                    children.push(wire::Node::Text {
                        options: wire::TextOptions {
                            height: None,
                            align_y: None,
                            line_height: Some(wire::LineHeight::Relative(1.5f32)),
                            shaping: None,
                            wrapping: None,
                            tracking: 0.0f32,
                            font: Some(wire::NamedFont {
                                family: wire::FontFamily::Named("Geist".into()),
                                weight: wire::Weight::Normal,
                                stretch: wire::FontStretch::Normal,
                                style: wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:65", use_scope),
                        size: Some(12.5f32),
                        color: Some(palette.colors[5]),
                        font: wire::Font {
                            monospace: false,
                            weight: wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: "Waiting for the page list.".to_owned(),
                    });
                    wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:48", use_scope),
                        wrap: None,
                        axis: wire::Axis::Column,
                        spacing: Some((7.0) as f32),
                        padding: None,
                        width: Some(wire::Length::Fill),
                        height: None,
                        align: Some(wire::AlignX::Center),
                        background: None,
                        border: None,
                        children,
                    }
                }),
            }
        }
    }
    pub(crate) fn selection_empty(&self, palette: Palette, use_scope: String) -> wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            wire::Node::Container {
                shadow: wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: Some(wire::Length::Fill),
                height: Some(wire::Length::Fill),
                padding: Some(wire::Edges {
                    top: (22.0) as f32,
                    right: (22.0) as f32,
                    bottom: (22.0) as f32,
                    left: (22.0) as f32,
                }),
                align_x: Some(wire::AlignX::Center),
                align_y: Some(wire::AlignY::Center),
                background: (None).map(wire::Background::Color),
                border: None,
                snap: None,
                content: Box::new({
                    let mut children: Vec<wire::Node> = Vec::new();
                    children.push(wire::Node::Container {
                        shadow: wire::Shadow {
                            color: None,
                            x: None,
                            y: None,
                            blur: None,
                        },
                        max_width: None,
                        max_height: None,
                        clip: false,
                        key: format!("{}/@container:53", use_scope),
                        width: Some(wire::Length::Fixed((42.0) as f32)),
                        height: Some(wire::Length::Fixed((42.0) as f32)),
                        padding: None,
                        align_x: Some(wire::AlignX::Center),
                        align_y: Some(wire::AlignY::Center),
                        background: (Some(palette.colors[3])).map(wire::Background::Color),
                        border: Some(wire::Border {
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
                        content: Box::new(wire::Node::Text {
                            options: wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: None,
                                tracking: 0.0f32,
                                font: Some(wire::NamedFont {
                                    family: wire::FontFamily::Named("Geist".into()),
                                    weight: wire::Weight::Normal,
                                    stretch: wire::FontStretch::Normal,
                                    style: wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:63", use_scope),
                            size: Some(((20.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[7]),
                            font: wire::Font {
                                monospace: false,
                                weight: wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: "◇".to_owned(),
                        }),
                    });
                    children.push(wire::Node::Text {
                        options: wire::TextOptions {
                            height: None,
                            align_y: None,
                            line_height: Some(wire::LineHeight::Relative(1.35f32)),
                            shaping: None,
                            wrapping: None,
                            tracking: 0.0f32,
                            font: Some(wire::NamedFont {
                                family: wire::FontFamily::Named("Geist".into()),
                                weight: wire::Weight::Semibold,
                                stretch: wire::FontStretch::Normal,
                                style: wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:64", use_scope),
                        size: Some(16.0f32),
                        color: Some(palette.colors[4]),
                        font: wire::Font {
                            monospace: false,
                            weight: wire::Weight::Semibold,
                        },
                        width: None,
                        align_x: None,
                        content: "No page selected".to_owned(),
                    });
                    children.push(wire::Node::Text {
                        options: wire::TextOptions {
                            height: None,
                            align_y: None,
                            line_height: Some(wire::LineHeight::Relative(1.5f32)),
                            shaping: None,
                            wrapping: None,
                            tracking: 0.0f32,
                            font: Some(wire::NamedFont {
                                family: wire::FontFamily::Named("Geist".into()),
                                weight: wire::Weight::Normal,
                                stretch: wire::FontStretch::Normal,
                                style: wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:65", use_scope),
                        size: Some(12.5f32),
                        color: Some(palette.colors[5]),
                        font: wire::Font {
                            monospace: false,
                            weight: wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: "Create a page from the sidebar.".to_owned(),
                    });
                    wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:48", use_scope),
                        wrap: None,
                        axis: wire::Axis::Column,
                        spacing: Some((7.0) as f32),
                        padding: None,
                        width: Some(wire::Length::Fill),
                        height: None,
                        align: Some(wire::AlignX::Center),
                        background: None,
                        border: None,
                        children,
                    }
                }),
            }
        }
    }
    pub(crate) fn comments_empty(&self, palette: Palette, use_scope: String) -> wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            wire::Node::Container {
                shadow: wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: Some(wire::Length::Fill),
                height: None,
                padding: Some(wire::Edges {
                    top: (30.0) as f32,
                    right: (30.0) as f32,
                    bottom: (30.0) as f32,
                    left: (30.0) as f32,
                }),
                align_x: Some(wire::AlignX::Center),
                align_y: None,
                background: (Some(wire::Rgba([
                    0.0 / 255.0,
                    0.0 / 255.0,
                    0.0 / 255.0,
                    0.000000,
                ])))
                .map(wire::Background::Color),
                border: Some(wire::Border {
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
                content: Box::new(wire::Node::Text {
                    options: wire::TextOptions {
                        height: None,
                        align_y: None,
                        line_height: None,
                        shaping: None,
                        wrapping: None,
                        tracking: 0.0f32,
                        font: Some(wire::NamedFont {
                            family: wire::FontFamily::Named("Geist".into()),
                            weight: wire::Weight::Normal,
                            stretch: wire::FontStretch::Normal,
                            style: wire::FontStyle::Normal,
                        }),
                    },
                    key: format!("{}/@text:77", use_scope),
                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[71]),
                    font: wire::Font {
                        monospace: false,
                        weight: wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: "No pages matched that search.".to_owned(),
                }),
            }
        }
    }
    pub(crate) fn comment_avatar(
        &self,
        palette: Palette,
        use_scope: String,
        initials: String,
    ) -> wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            wire::Node::Container {
                shadow: wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: Some(wire::Length::Fixed((22.0) as f32)),
                height: Some(wire::Length::Fixed((22.0) as f32)),
                padding: None,
                align_x: Some(wire::AlignX::Center),
                align_y: Some(wire::AlignY::Center),
                background: (Some(palette.colors[35])).map(wire::Background::Color),
                border: Some(wire::Border {
                    color: None,
                    width: None,
                    radius: Some([
                        ((22.0 / 2.0) as f32).max(0.0).min(f32::MAX),
                        ((22.0 / 2.0) as f32).max(0.0).min(f32::MAX),
                        ((22.0 / 2.0) as f32).max(0.0).min(f32::MAX),
                        ((22.0 / 2.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new(wire::Node::Text {
                    options: wire::TextOptions {
                        height: None,
                        align_y: None,
                        line_height: None,
                        shaping: None,
                        wrapping: Some(wire::Wrapping::None),
                        tracking: 0.0f32,
                        font: Some(wire::NamedFont {
                            family: wire::FontFamily::Named("Geist".into()),
                            weight: wire::Weight::Semibold,
                            stretch: wire::FontStretch::Normal,
                            style: wire::FontStyle::Normal,
                        }),
                    },
                    key: format!("{}/@text:232", use_scope),
                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[5]),
                    font: wire::Font {
                        monospace: false,
                        weight: wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: initials.to_owned(),
                }),
            }
        }
    }
    pub(crate) fn delete_dialog(&self, palette: Palette, use_scope: String) -> wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            wire::Node::Container {
                shadow: wire::Shadow {
                    color: Some(palette.colors[48]),
                    x: None,
                    y: Some((24.0) as f32),
                    blur: Some((60.0) as f32),
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: Some(wire::Length::Fixed((418.0) as f32)),
                height: None,
                padding: None,
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[3])).map(wire::Background::Color),
                border: Some(wire::Border {
                    color: Some(palette.colors[39]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((14.0) as f32).max(0.0).min(f32::MAX),
                        ((14.0) as f32).max(0.0).min(f32::MAX),
                        ((14.0) as f32).max(0.0).min(f32::MAX),
                        ((14.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<wire::Node> = Vec::new();
                    children.push({
                        let mut children: Vec<wire::Node> = Vec::new();
                        children.push(wire::Node::Text {
                            options: wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: Some(wire::Wrapping::None),
                                tracking: 0.0f32,
                                font: Some(wire::NamedFont {
                                    family: wire::FontFamily::Named("Geist".into()),
                                    weight: wire::Weight::Semibold,
                                    stretch: wire::FontStretch::Normal,
                                    style: wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:213", use_scope),
                            size: Some(((16.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[7]),
                            font: wire::Font {
                                monospace: false,
                                weight: wire::Weight::Normal,
                            },
                            width: Some(wire::Length::Fill),
                            align_x: None,
                            content: "Delete this page".to_owned(),
                        });
                        children.push(wire::Node::Button {
                            checked: None,
                            expanded: None,
                            description: None,
                            key: format!("{}/@button:135", use_scope),
                            content: wire::ButtonContent::Child(Box::new(wire::Node::Container {
                                shadow: wire::Shadow {
                                    color: None,
                                    x: None,
                                    y: None,
                                    blur: None,
                                },
                                max_width: None,
                                max_height: None,
                                clip: false,
                                key: format!("{}/@container:143", use_scope),
                                width: Some(wire::Length::Fill),
                                height: Some(wire::Length::Fill),
                                padding: None,
                                align_x: Some(wire::AlignX::Center),
                                align_y: Some(wire::AlignY::Center),
                                background: (None).map(wire::Background::Color),
                                border: None,
                                snap: None,
                                content: Box::new(wire::Node::Text {
                                    options: wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(wire::NamedFont {
                                            family: wire::FontFamily::Named("Geist".into()),
                                            weight: wire::Weight::Normal,
                                            stretch: wire::FontStretch::Normal,
                                            style: wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:149", use_scope),
                                    size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[5]),
                                    font: wire::Font {
                                        monospace: false,
                                        weight: wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: "×".to_owned(),
                                }),
                            })),
                            label: Some(String::from("Cancel".to_owned())),
                            on_press: if (self.busy || (!(self.host_error).is_empty())) {
                                None
                            } else {
                                Some(::ducktape_view_guest::slots::message(
                                    Message::DisarmPageDelete,
                                ))
                            },
                            width: Some(wire::Length::Fixed((26.0) as f32)),
                            height: Some(wire::Length::Fixed((26.0) as f32)),
                            padding: Some(wire::Edges::all((0.0) as f32)),
                            style: wire::ButtonStyle {
                                preset: wire::ButtonPreset::Primary,
                                recipe: Some(wire::ButtonRecipe {
                                    base: wire::Face {
                                        background: Some(wire::Rgba([
                                            0.0 / 255.0,
                                            0.0 / 255.0,
                                            0.0 / 255.0,
                                            0.000000,
                                        ])),
                                        text: Some(palette.colors[4]),
                                        border: Some(wire::Border {
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
                                    font: Some(wire::NamedFont {
                                        family: wire::FontFamily::Named("Geist".into()),
                                        weight: wire::Weight::Normal,
                                        stretch: wire::FontStretch::Normal,
                                        style: wire::FontStyle::Normal,
                                    }),
                                }),
                                active: wire::Face {
                                    background: Some(wire::Rgba([
                                        0.0 / 255.0,
                                        0.0 / 255.0,
                                        0.0 / 255.0,
                                        0.000000,
                                    ])),
                                    text: Some(palette.colors[5]),
                                    border: Some(wire::Border {
                                        color: Some(wire::Rgba([
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
                                hovered: Some(wire::Face {
                                    background: Some(palette.colors[55]),
                                    text: Some(palette.colors[4]),
                                    border: None,
                                }),
                                pressed: Some(wire::Face {
                                    background: Some(palette.colors[56]),
                                    text: Some(palette.colors[4]),
                                    border: None,
                                }),
                                disabled: None,
                            },
                        });
                        wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:208", use_scope),
                            wrap: None,
                            axis: wire::Axis::Row,
                            spacing: Some((10.0) as f32),
                            padding: None,
                            width: Some(wire::Length::Fill),
                            height: None,
                            align: Some(wire::AlignX::Center),
                            background: None,
                            border: None,
                            children,
                        }
                    });
                    children
                        .push({
                            let mut children: Vec<wire::Node> = Vec::new();
                            children
                                .push(wire::Node::Text {
                                    options: wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: None,
                                        tracking: 0.0f32,
                                        font: Some(wire::NamedFont {
                                            family: wire::FontFamily::Named("Geist".into()),
                                            weight: wire::Weight::Medium,
                                            stretch: wire::FontStretch::Normal,
                                            style: wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:159", use_scope),
                                    size: Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[4]),
                                    font: wire::Font {
                                        monospace: false,
                                        weight: wire::Weight::Normal,
                                    },
                                    width: Some(wire::Length::Fill),
                                    align_x: None,
                                    content: crate::host::keep_str(
                                            (!(self.active_page_title).is_empty()),
                                            &(self.active_page_title),
                                            &("Untitled"),
                                        )
                                        .to_owned(),
                                });
                            children
                                .push(wire::Node::Text {
                                    options: wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: Some(
                                            wire::LineHeight::Relative(
                                                ((1.5) as f32).max(f32::EPSILON).min(f32::MAX),
                                            ),
                                        ),
                                        shaping: None,
                                        wrapping: None,
                                        tracking: 0.0f32,
                                        font: Some(wire::NamedFont {
                                            family: wire::FontFamily::Named("Geist".into()),
                                            weight: wire::Weight::Normal,
                                            stretch: wire::FontStretch::Normal,
                                            style: wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:165", use_scope),
                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[70]),
                                    font: wire::Font {
                                        monospace: false,
                                        weight: wire::Weight::Normal,
                                    },
                                    width: Some(wire::Length::Fill),
                                    align_x: None,
                                    content: "Everything nested under it goes too — its blocks, any subpages beneath them, and every comment thread on any of it — for every member. This cannot be undone from the app."
                                        .to_owned(),
                                });
                            children
                                .push({
                                    let mut children: Vec<wire::Node> = Vec::new();
                                    children
                                        .push(wire::Node::Button {
                                            checked: None,
                                            expanded: None,
                                            description: None,
                                            key: format!("{}/@button:176", use_scope),
                                            content: wire::ButtonContent::Label(String::from("Cancel")),
                                            label: None,
                                            on_press: if ((self.busy
                                                || (!(self.host_error).is_empty())))
                                            {
                                                None
                                            } else {
                                                Some(
                                                    ::ducktape_view_guest::slots::message(
                                                        Message::DisarmPageDelete,
                                                    ),
                                                )
                                            },
                                            width: None,
                                            height: None,
                                            padding: Some(wire::Edges::all((7.0) as f32)),
                                            style: wire::ButtonStyle {
                                                preset: wire::ButtonPreset::Primary,
                                                recipe: Some(wire::ButtonRecipe {
                                                    base: wire::Face {
                                                        background: Some(palette.colors[12]),
                                                        text: Some(palette.colors[13]),
                                                        border: Some(wire::Border {
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
                                                    font: Some(wire::NamedFont {
                                                        family: wire::FontFamily::Named("Geist".into()),
                                                        weight: wire::Weight::Semibold,
                                                        stretch: wire::FontStretch::Normal,
                                                        style: wire::FontStyle::Normal,
                                                    }),
                                                }),
                                                active: wire::Face::default(),
                                                hovered: None,
                                                pressed: None,
                                                disabled: None,
                                            },
                                        });
                                    children
                                        .push(wire::Node::Button {
                                            checked: None,
                                            expanded: None,
                                            description: None,
                                            key: format!("{}/@button:181", use_scope),
                                            content: wire::ButtonContent::Child(
                                                Box::new(wire::Node::Text {
                                                    options: wire::TextOptions {
                                                        height: None,
                                                        align_y: None,
                                                        line_height: None,
                                                        shaping: None,
                                                        wrapping: Some(wire::Wrapping::None),
                                                        tracking: 0.0f32,
                                                        font: Some(wire::NamedFont {
                                                            family: wire::FontFamily::Named("Geist".into()),
                                                            weight: wire::Weight::Normal,
                                                            stretch: wire::FontStretch::Normal,
                                                            style: wire::FontStyle::Normal,
                                                        }),
                                                    },
                                                    key: format!("{}/@text:187", use_scope),
                                                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: None,
                                                    font: wire::Font {
                                                        monospace: false,
                                                        weight: wire::Weight::Normal,
                                                    },
                                                    width: None,
                                                    align_x: None,
                                                    content: "Delete page".to_owned(),
                                                }),
                                            ),
                                            label: Some(String::from("Delete page".to_owned())),
                                            on_press: if ((self.busy
                                                || (!(self.host_error).is_empty())))
                                            {
                                                None
                                            } else {
                                                Some(
                                                    ::ducktape_view_guest::slots::message(
                                                        Message::DeletePageSubmit,
                                                    ),
                                                )
                                            },
                                            width: None,
                                            height: None,
                                            padding: Some(wire::Edges::all((7.0) as f32)),
                                            style: wire::ButtonStyle {
                                                preset: wire::ButtonPreset::Primary,
                                                recipe: Some(wire::ButtonRecipe {
                                                    base: wire::Face {
                                                        background: Some(palette.colors[20]),
                                                        text: Some(palette.colors[21]),
                                                        border: Some(wire::Border {
                                                            color: None,
                                                            width: None,
                                                            radius: Some([9.0; 4]),
                                                        }),
                                                    },
                                                    hover_background: Some({
                                                        let mut color = palette.colors[20];
                                                        color.0[3] = 0.900000;
                                                        color
                                                    }),
                                                    pressed_background: Some({
                                                        let mut color = palette.colors[20];
                                                        color.0[3] = 0.800000;
                                                        color
                                                    }),
                                                    disabled_background: None,
                                                    disabled_text: None,
                                                    disabled_opacity: Some(0.5f32),
                                                    focus_ring: Some(palette.colors[42]),
                                                    text_size: Some(12.5f32),
                                                    line_height: None,
                                                    font: Some(wire::NamedFont {
                                                        family: wire::FontFamily::Named("Geist".into()),
                                                        weight: wire::Weight::Semibold,
                                                        stretch: wire::FontStretch::Normal,
                                                        style: wire::FontStyle::Normal,
                                                    }),
                                                }),
                                                active: wire::Face::default(),
                                                hovered: None,
                                                pressed: None,
                                                disabled: None,
                                            },
                                        });
                                    wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:171", use_scope),
                                        wrap: None,
                                        axis: wire::Axis::Row,
                                        spacing: Some((8.0) as f32),
                                        padding: None,
                                        width: Some(wire::Length::Fill),
                                        height: None,
                                        align: Some(wire::AlignX::Right),
                                        background: None,
                                        border: None,
                                        children,
                                    }
                                });
                            wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:158", use_scope),
                                wrap: None,
                                axis: wire::Axis::Column,
                                spacing: Some((13.0) as f32),
                                padding: None,
                                width: Some(wire::Length::Fill),
                                height: None,
                                align: None,
                                background: None,
                                border: None,
                                children,
                            }
                        });
                    wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:200", use_scope),
                        wrap: None,
                        axis: wire::Axis::Column,
                        spacing: Some((13.0) as f32),
                        padding: Some(wire::Edges {
                            top: (20.0) as f32,
                            right: (22.0) as f32,
                            bottom: (22.0) as f32,
                            left: (22.0) as f32,
                        }),
                        width: Some(wire::Length::Fill),
                        height: None,
                        align: None,
                        background: None,
                        border: None,
                        children,
                    }
                }),
            }
        }
    }
}
