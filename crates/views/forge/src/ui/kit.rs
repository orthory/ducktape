use super::*;
impl super::ForgeView {
    pub(super) fn render_empty_state_0(
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
                        key: format!("{}/@container:23", use_scope),
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
                            key: format!("{}/@text:33", use_scope),
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
                        key: format!("{}/@text:34", use_scope),
                        size: Some(16.0f32),
                        color: Some(palette.colors[4]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                        },
                        width: None,
                        align_x: None,
                        content: ("Unable to read Forge".to_owned()).to_string(),
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
                        key: format!("{}/@text:35", use_scope),
                        size: Some(12.5f32),
                        color: Some(palette.colors[5]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: (self.host_error.to_owned()).to_string(),
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:18", use_scope),
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
    pub(super) fn render_empty_state_1(
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
                        key: format!("{}/@container:23", use_scope),
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
                            key: format!("{}/@text:33", use_scope),
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
                        key: format!("{}/@text:34", use_scope),
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
                            key: format!("{}/@text:35", use_scope),
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
                        key: format!("{}/@layout:18", use_scope),
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
    pub(super) fn render_tab_label_10(
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
                    if self.tab == "code" {
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
                            key: format!("{}/@text:358", use_scope),
                            size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[7]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("Code".to_owned()).to_string(),
                        });
                    }
                    if !(self.tab == "code") {
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
                            key: format!("{}/@text:365", use_scope),
                            size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[71]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("Code".to_owned()).to_string(),
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
                            key: format!("{}/@container:372", use_scope),
                            width: None,
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (1.0) as f32,
                                right: (7.0) as f32,
                                bottom: (1.0) as f32,
                                left: (7.0) as f32,
                            }),
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[55]))
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
                                key: format!("{}/@text:378", use_scope),
                                size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[71]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: (0).to_string(),
                            }),
                        });
                    }
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:351", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((7.0) as f32),
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (10.0) as f32,
                            right: (0.0) as f32,
                            bottom: (10.0) as f32,
                            left: (0.0) as f32,
                        }),
                        width: None,
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                        background: None,
                        border: None,
                        children: children,
                    }
                });
                if self.tab == "code" {
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
                        key: format!("{}/@container:385", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((2.0) as f32)),
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[7]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
                        snap: None,
                        content: Box::new(::ducktape_view_guest::wire::Node::Space {
                            width: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                            height: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                        }),
                    });
                }
                if !(self.tab == "code") {
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
                        key: format!("{}/@container:392", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((2.0) as f32)),
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: (Some(::ducktape_view_guest::wire::Rgba([
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.000000,
                        ])))
                        .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
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
                    width: Some(::ducktape_view_guest::wire::Length::Shrink),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(super) fn render_tab_label_11(
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
                    if self.tab == "pulls" {
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
                            key: format!("{}/@text:358", use_scope),
                            size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[7]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("Pull requests".to_owned()).to_string(),
                        });
                    }
                    if !(self.tab == "pulls") {
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
                            key: format!("{}/@text:365", use_scope),
                            size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[71]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("Pull requests".to_owned()).to_string(),
                        });
                    }
                    if crate::host::forge_open_count(
                        ::std::convert::AsRef::as_ref(&(self.items)),
                        ::std::convert::AsRef::as_ref(&("pr")),
                    ) > 0
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
                            key: format!("{}/@container:372", use_scope),
                            width: None,
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (1.0) as f32,
                                right: (7.0) as f32,
                                bottom: (1.0) as f32,
                                left: (7.0) as f32,
                            }),
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[55]))
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
                                key: format!("{}/@text:378", use_scope),
                                size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[71]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: (crate::host::forge_open_count(
                                    ::std::convert::AsRef::as_ref(&(self.items)),
                                    ::std::convert::AsRef::as_ref(&("pr")),
                                ))
                                .to_string(),
                            }),
                        });
                    }
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:351", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((7.0) as f32),
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (10.0) as f32,
                            right: (0.0) as f32,
                            bottom: (10.0) as f32,
                            left: (0.0) as f32,
                        }),
                        width: None,
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                        background: None,
                        border: None,
                        children: children,
                    }
                });
                if self.tab == "pulls" {
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
                        key: format!("{}/@container:385", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((2.0) as f32)),
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[7]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
                        snap: None,
                        content: Box::new(::ducktape_view_guest::wire::Node::Space {
                            width: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                            height: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                        }),
                    });
                }
                if !(self.tab == "pulls") {
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
                        key: format!("{}/@container:392", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((2.0) as f32)),
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: (Some(::ducktape_view_guest::wire::Rgba([
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.000000,
                        ])))
                        .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
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
                    width: Some(::ducktape_view_guest::wire::Length::Shrink),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(super) fn render_tab_label_12(
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
                    if self.tab == "issues" {
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
                            key: format!("{}/@text:358", use_scope),
                            size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[7]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("Issues".to_owned()).to_string(),
                        });
                    }
                    if !(self.tab == "issues") {
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
                            key: format!("{}/@text:365", use_scope),
                            size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[71]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("Issues".to_owned()).to_string(),
                        });
                    }
                    if crate::host::forge_open_count(
                        ::std::convert::AsRef::as_ref(&(self.items)),
                        ::std::convert::AsRef::as_ref(&("issue")),
                    ) > 0
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
                            key: format!("{}/@container:372", use_scope),
                            width: None,
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (1.0) as f32,
                                right: (7.0) as f32,
                                bottom: (1.0) as f32,
                                left: (7.0) as f32,
                            }),
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[55]))
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
                                key: format!("{}/@text:378", use_scope),
                                size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[71]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: (crate::host::forge_open_count(
                                    ::std::convert::AsRef::as_ref(&(self.items)),
                                    ::std::convert::AsRef::as_ref(&("issue")),
                                ))
                                .to_string(),
                            }),
                        });
                    }
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:351", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((7.0) as f32),
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (10.0) as f32,
                            right: (0.0) as f32,
                            bottom: (10.0) as f32,
                            left: (0.0) as f32,
                        }),
                        width: None,
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                        background: None,
                        border: None,
                        children: children,
                    }
                });
                if self.tab == "issues" {
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
                        key: format!("{}/@container:385", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((2.0) as f32)),
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[7]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
                        snap: None,
                        content: Box::new(::ducktape_view_guest::wire::Node::Space {
                            width: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                            height: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                        }),
                    });
                }
                if !(self.tab == "issues") {
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
                        key: format!("{}/@container:392", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((2.0) as f32)),
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: (Some(::ducktape_view_guest::wire::Rgba([
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.000000,
                        ])))
                        .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
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
                    width: Some(::ducktape_view_guest::wire::Length::Shrink),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(super) fn render_empty_plate_34(
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
                    top: (30.0) as f32,
                    right: (30.0) as f32,
                    bottom: (30.0) as f32,
                    left: (30.0) as f32,
                }),
                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                align_y: None,
                background: (Some(::ducktape_view_guest::wire::Rgba([
                    0.0 / 255.0,
                    0.0 / 255.0,
                    0.0 / 255.0,
                    0.000000,
                ])))
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
                content: Box::new(::ducktape_view_guest::wire::Node::Text {
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
                    key: format!("{}/@text:47", use_scope),
                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[71]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: ("Loading repository tracker…".to_owned()).to_string(),
                }),
            }
        }
    }
    pub(super) fn render_empty_plate_35(
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
                    key: format!("{}/@text:47", use_scope),
                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[71]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: ("Could not load this repository. Return to all repos and open it again to retry."
                        .to_owned())
                        .to_string(),
                }),
            }
        }
    }
    pub(super) fn render_empty_plate_36(
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
                    top: (30.0) as f32,
                    right: (30.0) as f32,
                    bottom: (30.0) as f32,
                    left: (30.0) as f32,
                }),
                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                align_y: None,
                background: (Some(::ducktape_view_guest::wire::Rgba([
                    0.0 / 255.0,
                    0.0 / 255.0,
                    0.0 / 255.0,
                    0.000000,
                ])))
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
                content: Box::new(::ducktape_view_guest::wire::Node::Text {
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
                    key: format!("{}/@text:47", use_scope),
                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[71]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: ("No issues — this app reads the tracker but cannot open one yet."
                        .to_owned())
                    .to_string(),
                }),
            }
        }
    }
    pub(super) fn render_empty_plate_45(
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
                    top: (30.0) as f32,
                    right: (30.0) as f32,
                    bottom: (30.0) as f32,
                    left: (30.0) as f32,
                }),
                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                align_y: None,
                background: (Some(::ducktape_view_guest::wire::Rgba([
                    0.0 / 255.0,
                    0.0 / 255.0,
                    0.0 / 255.0,
                    0.000000,
                ])))
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
                content: Box::new(::ducktape_view_guest::wire::Node::Text {
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
                    key: format!("{}/@text:47", use_scope),
                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[71]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content:
                        ("No pull requests — an agent run opens one when it delivers its work."
                            .to_owned())
                        .to_string(),
                }),
            }
        }
    }
    pub(super) fn render_empty_plate_47(
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
                    top: (30.0) as f32,
                    right: (30.0) as f32,
                    bottom: (30.0) as f32,
                    left: (30.0) as f32,
                }),
                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                align_y: None,
                background: (Some(::ducktape_view_guest::wire::Rgba([
                    0.0 / 255.0,
                    0.0 / 255.0,
                    0.0 / 255.0,
                    0.000000,
                ])))
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
                content: Box::new(::ducktape_view_guest::wire::Node::Text {
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
                    key: format!("{}/@text:47", use_scope),
                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[71]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: ("Loading tracker item…".to_owned()).to_string(),
                }),
            }
        }
    }
    pub(super) fn render_empty_plate_48(
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
                    top: (30.0) as f32,
                    right: (30.0) as f32,
                    bottom: (30.0) as f32,
                    left: (30.0) as f32,
                }),
                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                align_y: None,
                background: (Some(::ducktape_view_guest::wire::Rgba([
                    0.0 / 255.0,
                    0.0 / 255.0,
                    0.0 / 255.0,
                    0.000000,
                ])))
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
                content: Box::new(::ducktape_view_guest::wire::Node::Text {
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
                    key: format!("{}/@text:47", use_scope),
                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[71]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: ("Could not load this item. Go back and open it again to retry."
                        .to_owned())
                    .to_string(),
                }),
            }
        }
    }
    pub(super) fn render_badge_success(
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
                background: (Some(palette.colors[27]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[28]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
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
                        key: format!("{}/@container:91", use_scope),
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
                                ((3.0) as f32).max(0.0).min(f32::MAX),
                                ((3.0) as f32).max(0.0).min(f32::MAX),
                                ((3.0) as f32).max(0.0).min(f32::MAX),
                                ((3.0) as f32).max(0.0).min(f32::MAX),
                            ]),
                        }),
                        snap: None,
                        content: Box::new(::ducktape_view_guest::wire::Node::Space {
                            width: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                            height: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
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
                        key: format!("{}/@text:98", use_scope),
                        size: Some(9.0f32),
                        color: Some(palette.colors[4]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                        },
                        width: None,
                        align_x: None,
                        content: (self.forge_item_state.to_owned()).to_string(),
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:90", use_scope),
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
            }
        }
    }
    pub(super) fn render_badge_warning(
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
                background: (Some(palette.colors[32]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[33]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
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
                        key: format!("{}/@container:110", use_scope),
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
                            width: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                            height: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
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
                        key: format!("{}/@text:117", use_scope),
                        size: Some(9.0f32),
                        color: Some(palette.colors[4]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                        },
                        width: None,
                        align_x: None,
                        content: (self.forge_item_state.to_owned()).to_string(),
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:109", use_scope),
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
            }
        }
    }
    pub(super) fn render_badge_destructive(
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
                background: (Some(palette.colors[22]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[23]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
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
                        key: format!("{}/@container:129", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fixed((6.0) as f32)),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((6.0) as f32)),
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[24]))
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
                            width: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                            height: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
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
                        key: format!("{}/@text:136", use_scope),
                        size: Some(9.0f32),
                        color: Some(palette.colors[4]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                        },
                        width: None,
                        align_x: None,
                        content: (self.forge_item_state.to_owned()).to_string(),
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:128", use_scope),
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
            }
        }
    }
    pub(super) fn render_badge_outline(
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
                    key: format!("{}/@text:147", use_scope),
                    size: Some(9.0f32),
                    color: Some(palette.colors[13]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                    },
                    width: None,
                    align_x: None,
                    content: (self.forge_item_state.to_owned()).to_string(),
                }),
            }
        }
    }
    pub(super) fn render_status_badge_54(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
            if self.forge_item_state == "active" {
                children.push(
                    self.render_badge_success(palette, format!("{}/Badge.Success@687", use_scope)),
                );
            }
            if (!(self.forge_item_state == "active")) && (self.forge_item_state == "paused") {
                children.push(
                    self.render_badge_warning(palette, format!("{}/Badge.Warning@689", use_scope)),
                );
            }
            if (!((self.forge_item_state == "active") || (self.forge_item_state == "paused")))
                && (self.forge_item_state == "open")
            {
                children.push(
                    self.render_badge_success(palette, format!("{}/Badge.Success@691", use_scope)),
                );
            }
            if (!(((self.forge_item_state == "active") || (self.forge_item_state == "paused"))
                || (self.forge_item_state == "open")))
                && (self.forge_item_state == "closed")
            {
                children.push(self.render_badge_destructive(
                    palette,
                    format!("{}/Badge.Destructive@693", use_scope),
                ));
            }
            if (!((((self.forge_item_state == "active") || (self.forge_item_state == "paused"))
                || (self.forge_item_state == "open"))
                || (self.forge_item_state == "closed")))
                && (self.forge_item_state == "merged")
            {
                children.push(
                    self.render_badge_success(palette, format!("{}/Badge.Success@695", use_scope)),
                );
            }
            if (!(((((self.forge_item_state == "active") || (self.forge_item_state == "paused"))
                || (self.forge_item_state == "open"))
                || (self.forge_item_state == "closed"))
                || (self.forge_item_state == "merged")))
                && (self.forge_item_state == "passed")
            {
                children.push(
                    self.render_badge_success(palette, format!("{}/Badge.Success@697", use_scope)),
                );
            }
            if (!((((((self.forge_item_state == "active")
                || (self.forge_item_state == "paused"))
                || (self.forge_item_state == "open"))
                || (self.forge_item_state == "closed"))
                || (self.forge_item_state == "merged"))
                || (self.forge_item_state == "passed")))
                && (self.forge_item_state == "rejected")
            {
                children.push(self.render_badge_destructive(
                    palette,
                    format!("{}/Badge.Destructive@699", use_scope),
                ));
            }
            if (!(((((((self.forge_item_state == "active")
                || (self.forge_item_state == "paused"))
                || (self.forge_item_state == "open"))
                || (self.forge_item_state == "closed"))
                || (self.forge_item_state == "merged"))
                || (self.forge_item_state == "passed"))
                || (self.forge_item_state == "rejected")))
                && (self.forge_item_state == "applied")
            {
                children.push(
                    self.render_badge_success(palette, format!("{}/Badge.Success@701", use_scope)),
                );
            }
            if (!((((((((self.forge_item_state == "active")
                || (self.forge_item_state == "paused"))
                || (self.forge_item_state == "open"))
                || (self.forge_item_state == "closed"))
                || (self.forge_item_state == "merged"))
                || (self.forge_item_state == "passed"))
                || (self.forge_item_state == "rejected"))
                || (self.forge_item_state == "applied")))
                && (self.forge_item_state == "discarded")
            {
                children.push(
                    self.render_badge_warning(palette, format!("{}/Badge.Warning@703", use_scope)),
                );
            }
            if !(((((((((self.forge_item_state == "active")
                || (self.forge_item_state == "paused"))
                || (self.forge_item_state == "open"))
                || (self.forge_item_state == "closed"))
                || (self.forge_item_state == "merged"))
                || (self.forge_item_state == "passed"))
                || (self.forge_item_state == "rejected"))
                || (self.forge_item_state == "applied"))
                || (self.forge_item_state == "discarded"))
            {
                children.push(
                    self.render_badge_outline(palette, format!("{}/Badge.Outline@705", use_scope)),
                );
            }
            ::ducktape_view_guest::wire::Node::Linear {
                max_width: None,
                clip: false,
                key: format!("{}/@layout:58", use_scope),
                wrap: None,
                axis: ::ducktape_view_guest::wire::Axis::Row,
                spacing: None,
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
    pub(super) fn render_separator_57(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
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
    }
    pub(super) fn render_rich_line_58(
        &self,
        palette: Palette,
        use_scope: String,
        cb_0: impl Fn(String) -> Message + Clone + 'static,
        arg_0: crate::host::ChatBlock,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let mut rich_spans: Vec<::ducktape_view_guest::wire::RichSpan> = Vec::new();
            for span in arg_0.spans.iter().cloned() {
                rich_spans.push(::ducktape_view_guest::wire::RichSpan {
                    content: (span.mention.to_owned()).to_string(),
                    size: None,
                    line_height: None,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
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
                rich_spans.push(::ducktape_view_guest::wire::RichSpan {
                    content: (span.link_text.to_owned()).to_string(),
                    size: None,
                    line_height: None,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
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
                rich_spans.push(::ducktape_view_guest::wire::RichSpan {
                    content: (span.bold_italic.to_owned()).to_string(),
                    size: None,
                    line_height: None,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
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
                rich_spans.push(::ducktape_view_guest::wire::RichSpan {
                    content: (span.bold.to_owned()).to_string(),
                    size: None,
                    line_height: None,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
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
                rich_spans.push(::ducktape_view_guest::wire::RichSpan {
                    content: (span.italic.to_owned()).to_string(),
                    size: None,
                    line_height: None,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
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
                rich_spans.push(::ducktape_view_guest::wire::RichSpan {
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
                    line_height: Some(::ducktape_view_guest::wire::LineHeight::Relative(
                        ((1.55) as f32).max(f32::EPSILON).min(f32::MAX),
                    )),
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
                key: format!("{}/@text:527", use_scope),
                size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                color: Some(palette.colors[15]),
                font: ::ducktape_view_guest::wire::Font {
                    monospace: false,
                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                },
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                align_x: None,
                spans: rich_spans,
                on_link: Some(::ducktape_view_guest::slots::handler::<String, Message>(
                    Box::new({
                        let route = {
                            let route_callback = (cb_0).clone();
                            move |link: String| (route_callback)(link)
                        };
                        move |sent: String| Some(route(sent))
                    }),
                )),
            }
        }
    }
    pub(super) fn render_rich_body_59(
        &self,
        palette: Palette,
        use_scope: String,
        cb_0: impl Fn(String) -> Message + Clone + 'static,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
            for (index, block) in self.forge_item_blocks.iter().enumerate() {
                let for_scope = format!("{}/@for:1031({})", use_scope, index);
                if block.kind == "divider" {
                    children.push(
                        self.render_separator_57(palette, format!("{}/Separator@1033", for_scope)),
                    );
                }
                if block.kind == "code" {
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
                            if !(block.lang).is_empty() {
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
                                            stretch:
                                                ::ducktape_view_guest::wire::FontStretch::Normal,
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
                            children.push(::ducktape_view_guest::wire::Node::Text {
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
                if block.kind == "quote" {
                    children.push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        children.push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            if block.rich {
                                children.push(self.render_rich_line_58(
                                    palette,
                                    format!("{}/RichLine@1083", for_scope),
                                    (cb_0).clone(),
                                    block.clone(),
                                ));
                            }
                            if !block.rich {
                                children.push(::ducktape_view_guest::wire::Node::Text {
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
                                            stretch:
                                                ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:466", for_scope),
                                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
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
                            key: format!("{}/@container:473", for_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fixed((3.0) as f32)),
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
                                width: Some(::ducktape_view_guest::wire::Length::Fixed(
                                    (1.0) as f32,
                                )),
                                height: Some(::ducktape_view_guest::wire::Length::Fixed(
                                    (1.0) as f32,
                                )),
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
                if block.kind == "paragraph" {
                    if block.rich {
                        children.push(self.render_rich_line_58(
                            palette,
                            format!("{}/RichLine@1108", for_scope),
                            (cb_0).clone(),
                            block.clone(),
                        ));
                    }
                    if !block.rich {
                        children.push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: Some(
                                    ::ducktape_view_guest::wire::LineHeight::Relative(
                                        ((1.55) as f32).max(f32::EPSILON).min(f32::MAX),
                                    ),
                                ),
                                shaping: None,
                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::WordOrGlyph),
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
                            size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
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
                key: format!("{}/@layout:404", use_scope),
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
    }
    pub(super) fn render_group_label_65(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Text {
                options: ::ducktape_view_guest::wire::TextOptions {
                    height: None,
                    align_y: None,
                    line_height: None,
                    shaping: None,
                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                    tracking: 0.0f32,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named("Geist Mono".into()),
                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                    }),
                },
                key: node_scope.clone(),
                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                color: Some(palette.colors[73]),
                font: ::ducktape_view_guest::wire::Font {
                    monospace: false,
                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                },
                width: None,
                align_x: None,
                content: ("MERGE".to_owned()).to_string(),
            }
        }
    }
    pub(super) fn render_group_label_70(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Text {
                options: ::ducktape_view_guest::wire::TextOptions {
                    height: None,
                    align_y: None,
                    line_height: None,
                    shaping: None,
                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                    tracking: 0.0f32,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named("Geist Mono".into()),
                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                    }),
                },
                key: node_scope.clone(),
                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                color: Some(palette.colors[73]),
                font: ::ducktape_view_guest::wire::Font {
                    monospace: false,
                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                },
                width: None,
                align_x: None,
                content: ("REVIEWS".to_owned()).to_string(),
            }
        }
    }
    pub(super) fn render_finality_chip_72(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: i64,
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
                    key: format!("{}/@container:308", use_scope),
                    width: None,
                    height: None,
                    padding: Some(::ducktape_view_guest::wire::Edges {
                        top: (2.0) as f32,
                        right: (7.0) as f32,
                        bottom: (2.0) as f32,
                        left: (7.0) as f32,
                    }),
                    align_x: None,
                    align_y: None,
                    background: (Some(palette.colors[106]))
                        .map(::ducktape_view_guest::wire::Background::Color),
                    border: Some(::ducktape_view_guest::wire::Border {
                        color: Some(palette.colors[107]),
                        width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                        radius: Some([
                            ((5.0) as f32).max(0.0).min(f32::MAX),
                            ((5.0) as f32).max(0.0).min(f32::MAX),
                            ((5.0) as f32).max(0.0).min(f32::MAX),
                            ((5.0) as f32).max(0.0).min(f32::MAX),
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
                                        "Geist Mono".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:317", use_scope),
                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[75]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("✓ finalized".to_owned()).to_string(),
                        });
                        if arg_0 > 0 {
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
                                key: format!("{}/@text:324", use_scope),
                                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[75]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("·".to_owned()).to_string(),
                            });
                        }
                        if arg_0 > 0 {
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
                                key: format!("{}/@text:331", use_scope),
                                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[75]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("h".to_owned()).to_string(),
                            });
                        }
                        if arg_0 > 0 {
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
                                key: format!("{}/@text:338", use_scope),
                                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[75]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: (arg_0).to_string(),
                            });
                        }
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:316", use_scope),
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
    pub(super) fn render_rich_body_73(
        &self,
        palette: Palette,
        use_scope: String,
        cb_0: impl Fn(String) -> Message + Clone + 'static,
        arg_0: Vec<crate::host::ChatBlock>,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
            for (index, block) in arg_0.iter().enumerate() {
                let for_scope = format!("{}/@for:1031({})", use_scope, index);
                if block.kind == "divider" {
                    children.push(
                        self.render_separator_57(palette, format!("{}/Separator@1033", for_scope)),
                    );
                }
                if block.kind == "code" {
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
                            if !(block.lang).is_empty() {
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
                                            stretch:
                                                ::ducktape_view_guest::wire::FontStretch::Normal,
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
                            children.push(::ducktape_view_guest::wire::Node::Text {
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
                if block.kind == "quote" {
                    children.push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        children.push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            if block.rich {
                                children.push(self.render_rich_line_58(
                                    palette,
                                    format!("{}/RichLine@1083", for_scope),
                                    (cb_0).clone(),
                                    block.clone(),
                                ));
                            }
                            if !block.rich {
                                children.push(::ducktape_view_guest::wire::Node::Text {
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
                                            stretch:
                                                ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:466", for_scope),
                                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
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
                            key: format!("{}/@container:473", for_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fixed((3.0) as f32)),
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
                                width: Some(::ducktape_view_guest::wire::Length::Fixed(
                                    (1.0) as f32,
                                )),
                                height: Some(::ducktape_view_guest::wire::Length::Fixed(
                                    (1.0) as f32,
                                )),
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
                if block.kind == "paragraph" {
                    if block.rich {
                        children.push(self.render_rich_line_58(
                            palette,
                            format!("{}/RichLine@1108", for_scope),
                            (cb_0).clone(),
                            block.clone(),
                        ));
                    }
                    if !block.rich {
                        children.push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: Some(
                                    ::ducktape_view_guest::wire::LineHeight::Relative(
                                        ((1.55) as f32).max(f32::EPSILON).min(f32::MAX),
                                    ),
                                ),
                                shaping: None,
                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::WordOrGlyph),
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
                            size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
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
                key: format!("{}/@layout:404", use_scope),
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
    }
    pub(super) fn render_rich_line_74(
        &self,
        palette: Palette,
        use_scope: String,
        cb_0: impl Fn(String) -> Message + Clone + 'static,
        arg_0: crate::host::ChatBlock,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let mut rich_spans: Vec<::ducktape_view_guest::wire::RichSpan> = Vec::new();
            for span in arg_0.spans.iter().cloned() {
                rich_spans.push(::ducktape_view_guest::wire::RichSpan {
                    content: (span.mention.to_owned()).to_string(),
                    size: None,
                    line_height: None,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
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
                rich_spans.push(::ducktape_view_guest::wire::RichSpan {
                    content: (span.link_text.to_owned()).to_string(),
                    size: None,
                    line_height: None,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
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
                rich_spans.push(::ducktape_view_guest::wire::RichSpan {
                    content: (span.bold_italic.to_owned()).to_string(),
                    size: None,
                    line_height: None,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
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
                rich_spans.push(::ducktape_view_guest::wire::RichSpan {
                    content: (span.bold.to_owned()).to_string(),
                    size: None,
                    line_height: None,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
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
                rich_spans.push(::ducktape_view_guest::wire::RichSpan {
                    content: (span.italic.to_owned()).to_string(),
                    size: None,
                    line_height: None,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
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
                rich_spans.push(::ducktape_view_guest::wire::RichSpan {
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
                    line_height: Some(::ducktape_view_guest::wire::LineHeight::Relative(
                        ((1.55) as f32).max(f32::EPSILON).min(f32::MAX),
                    )),
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
                key: format!("{}/@text:527", use_scope),
                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                color: Some(palette.colors[15]),
                font: ::ducktape_view_guest::wire::Font {
                    monospace: false,
                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                },
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                align_x: None,
                spans: rich_spans,
                on_link: Some(::ducktape_view_guest::slots::handler::<String, Message>(
                    Box::new({
                        let route = {
                            let route_callback = (cb_0).clone();
                            move |link: String| (route_callback)(link)
                        };
                        move |sent: String| Some(route(sent))
                    }),
                )),
            }
        }
    }
    pub(super) fn render_rich_body_75(
        &self,
        palette: Palette,
        use_scope: String,
        cb_0: impl Fn(String) -> Message + Clone + 'static,
        arg_0: Vec<crate::host::ChatBlock>,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
            for (index, block) in arg_0.iter().enumerate() {
                let for_scope = format!("{}/@for:1031({})", use_scope, index);
                if block.kind == "divider" {
                    children.push(
                        self.render_separator_57(palette, format!("{}/Separator@1033", for_scope)),
                    );
                }
                if block.kind == "code" {
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
                            if !(block.lang).is_empty() {
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
                                            stretch:
                                                ::ducktape_view_guest::wire::FontStretch::Normal,
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
                            children.push(::ducktape_view_guest::wire::Node::Text {
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
                if block.kind == "quote" {
                    children.push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        children.push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            if block.rich {
                                children.push(self.render_rich_line_74(
                                    palette,
                                    format!("{}/RichLine@1083", for_scope),
                                    (cb_0).clone(),
                                    block.clone(),
                                ));
                            }
                            if !block.rich {
                                children.push(::ducktape_view_guest::wire::Node::Text {
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
                                            stretch:
                                                ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:466", for_scope),
                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
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
                            key: format!("{}/@container:473", for_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fixed((3.0) as f32)),
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
                                width: Some(::ducktape_view_guest::wire::Length::Fixed(
                                    (1.0) as f32,
                                )),
                                height: Some(::ducktape_view_guest::wire::Length::Fixed(
                                    (1.0) as f32,
                                )),
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
                if block.kind == "paragraph" {
                    if block.rich {
                        children.push(self.render_rich_line_74(
                            palette,
                            format!("{}/RichLine@1108", for_scope),
                            (cb_0).clone(),
                            block.clone(),
                        ));
                    }
                    if !block.rich {
                        children.push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: Some(
                                    ::ducktape_view_guest::wire::LineHeight::Relative(
                                        ((1.55) as f32).max(f32::EPSILON).min(f32::MAX),
                                    ),
                                ),
                                shaping: None,
                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::WordOrGlyph),
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
                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
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
                key: format!("{}/@layout:404", use_scope),
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
    }
    pub(super) fn render_group_label_77(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Text {
                options: ::ducktape_view_guest::wire::TextOptions {
                    height: None,
                    align_y: None,
                    line_height: None,
                    shaping: None,
                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                    tracking: 0.0f32,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named("Geist Mono".into()),
                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                    }),
                },
                key: node_scope.clone(),
                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                color: Some(palette.colors[73]),
                font: ::ducktape_view_guest::wire::Font {
                    monospace: false,
                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                },
                width: None,
                align_x: None,
                content: ("DISCUSSION".to_owned()).to_string(),
            }
        }
    }
    pub(super) fn render_human_plate_78(
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
                        key: format!("{}/@container:222", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fixed((30.0) as f32)),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((30.0) as f32)),
                        padding: None,
                        align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                        align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                        background: (Some(palette.colors[99]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: Some(::ducktape_view_guest::wire::Border {
                            color: None,
                            width: None,
                            radius: Some([
                                ((30.0 / 2.0) as f32).max(0.0).min(f32::MAX),
                                ((30.0 / 2.0) as f32).max(0.0).min(f32::MAX),
                                ((30.0 / 2.0) as f32).max(0.0).min(f32::MAX),
                                ((30.0 / 2.0) as f32).max(0.0).min(f32::MAX),
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
                            key: format!("{}/@text:230", use_scope),
                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
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
                        key: format!("{}/@container:237", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fixed((30.0) as f32)),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((30.0) as f32)),
                        padding: None,
                        align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                        align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                        background: (Some(palette.colors[35]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: Some(::ducktape_view_guest::wire::Border {
                            color: None,
                            width: None,
                            radius: Some([
                                ((30.0 / 2.0) as f32).max(0.0).min(f32::MAX),
                                ((30.0 / 2.0) as f32).max(0.0).min(f32::MAX),
                                ((30.0 / 2.0) as f32).max(0.0).min(f32::MAX),
                                ((30.0 / 2.0) as f32).max(0.0).min(f32::MAX),
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
                            key: format!("{}/@text:245", use_scope),
                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
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
    pub(super) fn render_principal_plate_79(
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
                    children.push(self.render_human_plate_78(
                        palette,
                        format!("{}/HumanPlate@839", use_scope),
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
    pub(super) fn render_principal_avatar_80(
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
                    children.push(self.render_principal_plate_79(
                        palette,
                        format!("{}/PrincipalPlate@823", use_scope),
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
    pub(super) fn render_person_avatar_81(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children.push(self.render_principal_avatar_80(
                    palette,
                    format!("{}/PrincipalAvatar@777", use_scope),
                    arg_0.to_owned(),
                ));
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
    pub(super) fn render_agent_square_82(
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
                    key: format!("{}/@text:299", use_scope),
                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
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
    pub(super) fn render_agent_square_83(
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
                    key: format!("{}/@text:299", use_scope),
                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
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
    pub(super) fn render_agent_square_84(
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
                    key: format!("{}/@text:299", use_scope),
                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
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
    pub(super) fn render_agent_square_85(
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
                    key: format!("{}/@text:299", use_scope),
                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
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
    pub(super) fn render_agent_square_86(
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
                    key: format!("{}/@text:299", use_scope),
                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
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
    pub(super) fn render_agent_plate_87(
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
                    children.push(self.render_agent_square_82(
                        palette,
                        format!("{}/AgentSquare@881", use_scope),
                        arg_0.to_owned(),
                    ));
                }
                if (false) && (true) {
                    children.push(self.render_agent_square_83(
                        palette,
                        format!("{}/AgentSquare@888", use_scope),
                        arg_0.to_owned(),
                    ));
                }
                if true {
                    children.push(self.render_agent_square_84(
                        palette,
                        format!("{}/AgentSquare@895", use_scope),
                        arg_0.to_owned(),
                    ));
                }
                if (true) && (false) {
                    children.push(self.render_agent_square_85(
                        palette,
                        format!("{}/AgentSquare@902", use_scope),
                        arg_0.to_owned(),
                    ));
                }
                if false {
                    children.push(self.render_agent_square_86(
                        palette,
                        format!("{}/AgentSquare@909", use_scope),
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
    pub(super) fn render_principal_plate_88(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children.push(self.render_agent_plate_87(
                    palette,
                    format!("{}/AgentPlate@833", use_scope),
                    arg_0.to_owned(),
                ));
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
    pub(super) fn render_principal_avatar_89(
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
                    children.push(self.render_principal_plate_88(
                        palette,
                        format!("{}/PrincipalPlate@823", use_scope),
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
    pub(super) fn render_agent_avatar_90(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children.push(self.render_principal_avatar_89(
                    palette,
                    format!("{}/PrincipalAvatar@787", use_scope),
                    arg_0.to_owned(),
                ));
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
    pub(super) fn render_message_avatar_91(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
        arg_1: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if arg_1 == "human" {
                    children.push(self.render_person_avatar_81(
                        palette,
                        format!("{}/PersonAvatar@1124", use_scope),
                        arg_0.to_owned(),
                    ));
                }
                if (!(arg_1 == "human")) && (arg_1 == "agent") {
                    children.push(self.render_agent_avatar_90(
                        palette,
                        format!("{}/AgentAvatar@1130", use_scope),
                        arg_0.to_owned(),
                    ));
                }
                if !((arg_1 == "human") || (arg_1 == "agent")) {
                    children.push(self.render_agent_avatar_90(
                        palette,
                        format!("{}/AgentAvatar@1136", use_scope),
                        arg_0.to_owned(),
                    ));
                }
                ::ducktape_view_guest::wire::Node::Stack {
                    key: node_scope.clone(),
                    width: Some(::ducktape_view_guest::wire::Length::Fixed((30.0) as f32)),
                    height: Some(::ducktape_view_guest::wire::Length::Fixed((30.0) as f32)),
                    padding: None,
                    background: None,
                    border: None,
                    clip: false,
                    under: 0u32,
                    children: children,
                }
            }
        }
    }
    pub(super) fn render_rich_line_92(
        &self,
        palette: Palette,
        use_scope: String,
        cb_0: impl Fn(String) -> Message + Clone + 'static,
        arg_0: crate::host::ChatBlock,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let mut rich_spans: Vec<::ducktape_view_guest::wire::RichSpan> = Vec::new();
            for span in arg_0.spans.iter().cloned() {
                rich_spans.push(::ducktape_view_guest::wire::RichSpan {
                    content: (span.mention.to_owned()).to_string(),
                    size: None,
                    line_height: None,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
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
                rich_spans.push(::ducktape_view_guest::wire::RichSpan {
                    content: (span.link_text.to_owned()).to_string(),
                    size: None,
                    line_height: None,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
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
                rich_spans.push(::ducktape_view_guest::wire::RichSpan {
                    content: (span.bold_italic.to_owned()).to_string(),
                    size: None,
                    line_height: None,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
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
                rich_spans.push(::ducktape_view_guest::wire::RichSpan {
                    content: (span.bold.to_owned()).to_string(),
                    size: None,
                    line_height: None,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
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
                rich_spans.push(::ducktape_view_guest::wire::RichSpan {
                    content: (span.italic.to_owned()).to_string(),
                    size: None,
                    line_height: None,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named("Geist".into()),
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
                rich_spans.push(::ducktape_view_guest::wire::RichSpan {
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
                    line_height: Some(::ducktape_view_guest::wire::LineHeight::Relative(
                        ((1.55) as f32).max(f32::EPSILON).min(f32::MAX),
                    )),
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
                key: format!("{}/@text:527", use_scope),
                size: Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                color: Some(palette.colors[15]),
                font: ::ducktape_view_guest::wire::Font {
                    monospace: false,
                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                },
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                align_x: None,
                spans: rich_spans,
                on_link: Some(::ducktape_view_guest::slots::handler::<String, Message>(
                    Box::new({
                        let route = {
                            let route_callback = (cb_0).clone();
                            move |link: String| (route_callback)(link)
                        };
                        move |sent: String| Some(route(sent))
                    }),
                )),
            }
        }
    }
    pub(super) fn render_rich_body_93(
        &self,
        palette: Palette,
        use_scope: String,
        cb_0: impl Fn(String) -> Message + Clone + 'static,
        arg_0: Vec<crate::host::ChatBlock>,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
            for (index, block) in arg_0.iter().enumerate() {
                let for_scope = format!("{}/@for:1031({})", use_scope, index);
                if block.kind == "divider" {
                    children.push(
                        self.render_separator_57(palette, format!("{}/Separator@1033", for_scope)),
                    );
                }
                if block.kind == "code" {
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
                            if !(block.lang).is_empty() {
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
                                            stretch:
                                                ::ducktape_view_guest::wire::FontStretch::Normal,
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
                            children.push(::ducktape_view_guest::wire::Node::Text {
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
                if block.kind == "quote" {
                    children.push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        children.push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            if block.rich {
                                children.push(self.render_rich_line_92(
                                    palette,
                                    format!("{}/RichLine@1083", for_scope),
                                    (cb_0).clone(),
                                    block.clone(),
                                ));
                            }
                            if !block.rich {
                                children.push(::ducktape_view_guest::wire::Node::Text {
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
                                            stretch:
                                                ::ducktape_view_guest::wire::FontStretch::Normal,
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
                            key: format!("{}/@container:473", for_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fixed((3.0) as f32)),
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
                                width: Some(::ducktape_view_guest::wire::Length::Fixed(
                                    (1.0) as f32,
                                )),
                                height: Some(::ducktape_view_guest::wire::Length::Fixed(
                                    (1.0) as f32,
                                )),
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
                if block.kind == "paragraph" {
                    if block.rich {
                        children.push(self.render_rich_line_92(
                            palette,
                            format!("{}/RichLine@1108", for_scope),
                            (cb_0).clone(),
                            block.clone(),
                        ));
                    }
                    if !block.rich {
                        children.push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: Some(
                                    ::ducktape_view_guest::wire::LineHeight::Relative(
                                        ((1.55) as f32).max(f32::EPSILON).min(f32::MAX),
                                    ),
                                ),
                                shaping: None,
                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::WordOrGlyph),
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
                key: format!("{}/@layout:404", use_scope),
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
    }
    pub(super) fn render_message_body_94(
        &self,
        palette: Palette,
        use_scope: String,
        cb_0: impl Fn(String) -> Message + Clone + 'static,
        arg_0: crate::host::ChatMessage,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
            children.push(self.render_rich_body_93(
                palette,
                format!("{}/RichBody@1146", use_scope),
                (cb_0).clone(),
                arg_0.blocks.clone(),
            ));
            ::ducktape_view_guest::wire::Node::Linear {
                max_width: Some(((760.0) as f32).max(0.0).min(f32::MAX)),
                clip: false,
                key: format!("{}/@layout:519", use_scope),
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
