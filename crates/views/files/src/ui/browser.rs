use super::*;
impl super::FilesView {
    pub(super) fn render_breadcrumb(
        &self,
        palette: Palette,
        use_scope: String,
        cb_8: impl Fn(String) -> Message + Clone + 'static,
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
                    key: format!("{}/@container:130", use_scope),
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: Some(::ducktape_view_guest::wire::Length::Fixed((50.0) as f32)),
                    padding: Some(::ducktape_view_guest::wire::Edges {
                        top: (0.0) as f32,
                        right: (20.0) as f32,
                        bottom: (0.0) as f32,
                        left: (20.0) as f32,
                    }),
                    align_x: None,
                    align_y: None,
                    background: (None).map(::ducktape_view_guest::wire::Background::Color),
                    border: None,
                    snap: None,
                    content: Box::new({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        children.push(::ducktape_view_guest::wire::Node::Button {
                            checked: None,
                            expanded: None,
                            description: None,
                            key: format!("{}/@button:141", use_scope),
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
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch:
                                                ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:146", use_scope),
                                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[7]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("duckfs".to_owned()).to_string(),
                                },
                            )),
                            label: Some(String::from("Go to the duckfs root".to_owned())),
                            on_press: Some(::ducktape_view_guest::slots::message((cb_8)(
                                "/".to_owned(),
                            ))),
                            width: None,
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
                                    text: Some(palette.colors[7]),
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
                                    background: Some(palette.colors[55]),
                                    text: None,
                                    border: None,
                                }),
                                pressed: Some(::ducktape_view_guest::wire::Face {
                                    background: Some(palette.colors[56]),
                                    text: None,
                                    border: None,
                                }),
                                disabled: None,
                            },
                        });
                        if !(self.path).is_empty() {
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
                                key: format!("{}/@text:156", use_scope),
                                size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[7]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: (self.path.to_owned()).to_string(),
                            });
                        }
                        if !(crate::host::fs_counts_summary(
                            self.connected,
                            self.listed,
                            ::std::convert::AsRef::as_ref(&(self.entries)),
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
                                key: format!("{}/@container:163", use_scope),
                                width: None,
                                height: None,
                                padding: Some(::ducktape_view_guest::wire::Edges {
                                    top: (0.0) as f32,
                                    right: (0.0) as f32,
                                    bottom: (0.0) as f32,
                                    left: (4.0) as f32,
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
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            stretch:
                                                ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:164", use_scope),
                                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[72]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: (crate::host::fs_counts_summary(
                                        self.connected,
                                        self.listed,
                                        ::std::convert::AsRef::as_ref(&(self.entries)),
                                    )
                                    .to_owned())
                                    .to_string(),
                                }),
                            });
                        }
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:135", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: Some((6.0) as f32),
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
                    key: format!("{}/@container:170", use_scope),
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
    pub(super) fn render_fs_tree_face_4(
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
                    format!("{}/Icon@709", use_scope),
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
                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                "Geist Mono".into(),
                            ),
                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                        }),
                    },
                    key: format!("{}/@text:90", use_scope),
                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[5]),
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
                    spacing: Some((7.0) as f32),
                    padding: Some(::ducktape_view_guest::wire::Edges {
                        top: (6.0) as f32,
                        right: (12.0) as f32,
                        bottom: (6.0) as f32,
                        left: (11.0 + (0.0 * 14.0)) as f32,
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
    pub(super) fn render_fs_tree_row_5(
        &self,
        palette: Palette,
        use_scope: String,
        cb_8: impl Fn(String) -> Message + Clone + 'static,
        arg_0: crate::host::FsEntry,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                {
                    children.push(::ducktape_view_guest::wire::Node::Button {
                        checked: Some(false),
                        expanded: None,
                        description: None,
                        key: format!("{}/@button:54", use_scope),
                        content: ::ducktape_view_guest::wire::ButtonContent::Child(Box::new(
                            self.render_fs_tree_face_4(
                                palette,
                                format!("{}/FsTreeFace@686", use_scope),
                                arg_0.name.to_owned(),
                            ),
                        )),
                        label: Some(String::from("Open directory".to_owned())),
                        on_press: Some(::ducktape_view_guest::slots::message((cb_8)(
                            arg_0.path.to_owned(),
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
                                text: None,
                                border: None,
                            }),
                            pressed: Some(::ducktape_view_guest::wire::Face {
                                background: Some(palette.colors[56]),
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
    pub(super) fn render_object_table_header_9(
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
                    key: format!("{}/@container:182", use_scope),
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: Some(::ducktape_view_guest::wire::Length::Fixed((50.0) as f32)),
                    padding: Some(::ducktape_view_guest::wire::Edges {
                        top: (0.0) as f32,
                        right: (20.0) as f32,
                        bottom: (0.0) as f32,
                        left: (20.0) as f32,
                    }),
                    align_x: None,
                    align_y: None,
                    background: (None).map(::ducktape_view_guest::wire::Background::Color),
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
                                        "Geist Mono".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:193", use_scope),
                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[97]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            align_x: None,
                            content: ("NAME".to_owned()).to_string(),
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
                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:200", use_scope),
                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[97]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: Some(::ducktape_view_guest::wire::Length::Fixed((72.0) as f32)),
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Right),
                            content: ("SIZE".to_owned()).to_string(),
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
                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:208", use_scope),
                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[97]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: Some(::ducktape_view_guest::wire::Length::Fixed((92.0) as f32)),
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Right),
                            content: ("OBJECT".to_owned()).to_string(),
                        });
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:187", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: Some((12.0) as f32),
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
                    key: format!("{}/@container:218", use_scope),
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
    pub(super) fn render_object_row_face_13(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: crate::host::FsEntry,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children.push({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    if arg_0.kind == "dir" {
                        children.push(self.icon(
                            format!("{}/Icon@925", use_scope),
                            "folder",
                            16f32,
                            (palette).colors[30usize],
                            "@media:64",
                        ));
                    }
                    if arg_0.kind != "dir" {
                        children.push(self.icon(
                            format!("{}/Icon@931", use_scope),
                            "file",
                            16f32,
                            (palette).colors[73usize],
                            "@media:22",
                        ));
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
                                    "Geist Mono".into(),
                                ),
                                weight: ::ducktape_view_guest::wire::Weight::Medium,
                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:311", use_scope),
                        size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[15]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        align_x: None,
                        content: (arg_0.name.to_owned()).to_string(),
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: true,
                        key: format!("{}/@layout:293", use_scope),
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
                    key: format!("{}/@container:318", use_scope),
                    width: Some(::ducktape_view_guest::wire::Length::Fixed((72.0) as f32)),
                    height: None,
                    padding: None,
                    align_x: None,
                    align_y: None,
                    background: (None).map(::ducktape_view_guest::wire::Background::Color),
                    border: None,
                    snap: None,
                    content: Box::new({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        if arg_0.kind == "dir" {
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
                                key: format!("{}/@text:321", use_scope),
                                size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[41]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                align_x: Some(::ducktape_view_guest::wire::AlignX::Right),
                                content: ("—".to_owned()).to_string(),
                            });
                        }
                        if arg_0.kind != "dir" {
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
                                key: format!("{}/@text:330", use_scope),
                                size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[41]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                align_x: Some(::ducktape_view_guest::wire::AlignX::Right),
                                content: (crate::host::size_label(arg_0.size)).to_string(),
                            });
                        }
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:319", use_scope),
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
                    key: format!("{}/@container:338", use_scope),
                    width: Some(::ducktape_view_guest::wire::Length::Fixed((92.0) as f32)),
                    height: None,
                    padding: None,
                    align_x: None,
                    align_y: None,
                    background: (None).map(::ducktape_view_guest::wire::Background::Color),
                    border: None,
                    snap: None,
                    content: Box::new({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        if (arg_0.object).is_empty() {
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
                                key: format!("{}/@text:341", use_scope),
                                size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[72]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                align_x: Some(::ducktape_view_guest::wire::AlignX::Right),
                                content: ("—".to_owned()).to_string(),
                            });
                        }
                        if !(arg_0.object).is_empty() {
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
                                key: format!("{}/@text:350", use_scope),
                                size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[72]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                align_x: Some(::ducktape_view_guest::wire::AlignX::Right),
                                content: (arg_0.object.to_owned()).to_string(),
                            });
                        }
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:339", use_scope),
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
                    clip: true,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Row,
                    spacing: Some((12.0) as f32),
                    padding: Some(::ducktape_view_guest::wire::Edges {
                        top: (11.0) as f32,
                        right: (20.0) as f32,
                        bottom: (11.0) as f32,
                        left: (20.0) as f32,
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
    pub(super) fn render_object_row_14(
        &self,
        palette: Palette,
        use_scope: String,
        cb_8: impl Fn(String) -> Message + Clone + 'static,
        cb_9: impl Fn(String) -> Message + Clone + 'static,
        arg_0: crate::host::FsEntry,
        arg_1: bool,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if arg_0.kind == "dir" {
                    children.push(::ducktape_view_guest::wire::Node::Button {
                        checked: None,
                        expanded: None,
                        description: None,
                        key: format!("{}/@button:233", use_scope),
                        content: ::ducktape_view_guest::wire::ButtonContent::Child(Box::new(
                            self.render_object_row_face_13(
                                palette,
                                format!("{}/ObjectRowFace@864", use_scope),
                                arg_0.clone(),
                            ),
                        )),
                        label: Some(String::from("Open directory".to_owned())),
                        on_press: Some(::ducktape_view_guest::slots::message((cb_8)(
                            arg_0.path.to_owned(),
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
                                        ((0.0) as f32).max(0.0).min(f32::MAX),
                                        ((0.0) as f32).max(0.0).min(f32::MAX),
                                        ((0.0) as f32).max(0.0).min(f32::MAX),
                                        ((0.0) as f32).max(0.0).min(f32::MAX),
                                    ]),
                                }),
                            },
                            hovered: Some(::ducktape_view_guest::wire::Face {
                                background: Some(palette.colors[57]),
                                text: None,
                                border: None,
                            }),
                            pressed: Some(::ducktape_view_guest::wire::Face {
                                background: Some(palette.colors[56]),
                                text: None,
                                border: None,
                            }),
                            disabled: None,
                        },
                    });
                }
                if (arg_0.kind != "dir") && arg_1 {
                    children.push(::ducktape_view_guest::wire::Node::Button {
                        checked: Some(arg_1),
                        expanded: None,
                        description: None,
                        key: format!("{}/@button:244", use_scope),
                        content: ::ducktape_view_guest::wire::ButtonContent::Child(Box::new(
                            self.render_object_row_face_13(
                                palette,
                                format!("{}/ObjectRowFace@876", use_scope),
                                arg_0.clone(),
                            ),
                        )),
                        label: Some(String::from("Show object".to_owned())),
                        on_press: Some(::ducktape_view_guest::slots::message((cb_9)(
                            arg_0.path.to_owned(),
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
                                        ((0.0) as f32).max(0.0).min(f32::MAX),
                                        ((0.0) as f32).max(0.0).min(f32::MAX),
                                        ((0.0) as f32).max(0.0).min(f32::MAX),
                                        ((0.0) as f32).max(0.0).min(f32::MAX),
                                    ]),
                                }),
                            },
                            hovered: Some(::ducktape_view_guest::wire::Face {
                                background: Some(palette.colors[91]),
                                text: None,
                                border: None,
                            }),
                            pressed: Some(::ducktape_view_guest::wire::Face {
                                background: Some(palette.colors[56]),
                                text: None,
                                border: None,
                            }),
                            disabled: None,
                        },
                    });
                }
                if (arg_0.kind != "dir") && (!arg_1) {
                    children.push(::ducktape_view_guest::wire::Node::Button {
                        checked: Some(arg_1),
                        expanded: None,
                        description: None,
                        key: format!("{}/@button:256", use_scope),
                        content: ::ducktape_view_guest::wire::ButtonContent::Child(Box::new(
                            self.render_object_row_face_13(
                                palette,
                                format!("{}/ObjectRowFace@888", use_scope),
                                arg_0.clone(),
                            ),
                        )),
                        label: Some(String::from("Show object".to_owned())),
                        on_press: Some(::ducktape_view_guest::slots::message((cb_9)(
                            arg_0.path.to_owned(),
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
                                        ((0.0) as f32).max(0.0).min(f32::MAX),
                                        ((0.0) as f32).max(0.0).min(f32::MAX),
                                        ((0.0) as f32).max(0.0).min(f32::MAX),
                                        ((0.0) as f32).max(0.0).min(f32::MAX),
                                    ]),
                                }),
                            },
                            hovered: Some(::ducktape_view_guest::wire::Face {
                                background: Some(palette.colors[57]),
                                text: None,
                                border: None,
                            }),
                            pressed: Some(::ducktape_view_guest::wire::Face {
                                background: Some(palette.colors[56]),
                                text: None,
                                border: None,
                            }),
                            disabled: None,
                        },
                    });
                }
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
                    key: format!("{}/@container:267", use_scope),
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: Some(::ducktape_view_guest::wire::Length::Fixed((1.0) as f32)),
                    padding: None,
                    align_x: None,
                    align_y: None,
                    background: (Some(palette.colors[6]))
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
    pub(super) fn render_object_fact_15(
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
                clip: true,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (9.0) as f32,
                    right: (12.0) as f32,
                    bottom: (9.0) as f32,
                    left: (12.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (None).map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[63]),
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
                        key: format!("{}/@text:475", use_scope),
                        size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[71]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("object id".to_owned()).to_string(),
                    });
                    children.push(::ducktape_view_guest::wire::Node::Space {
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
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
                        key: format!("{}/@text:482", use_scope),
                        size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[13]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("—".to_owned()).to_string(),
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:470", use_scope),
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
                }),
            }
        }
    }
    pub(super) fn render_object_fact_16(
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
                clip: true,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (9.0) as f32,
                    right: (12.0) as f32,
                    bottom: (9.0) as f32,
                    left: (12.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (None).map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[63]),
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
                        key: format!("{}/@text:475", use_scope),
                        size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[71]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("object id".to_owned()).to_string(),
                    });
                    children.push(::ducktape_view_guest::wire::Node::Space {
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
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
                        key: format!("{}/@text:482", use_scope),
                        size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[13]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: (self.preview_entry.object.to_owned()).to_string(),
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:470", use_scope),
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
                }),
            }
        }
    }
    pub(super) fn render_object_fact_17(
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
                clip: true,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (9.0) as f32,
                    right: (12.0) as f32,
                    bottom: (9.0) as f32,
                    left: (12.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (None).map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[63]),
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
                        key: format!("{}/@text:475", use_scope),
                        size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[71]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("size".to_owned()).to_string(),
                    });
                    children.push(::ducktape_view_guest::wire::Node::Space {
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
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
                        key: format!("{}/@text:482", use_scope),
                        size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[13]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("—".to_owned()).to_string(),
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:470", use_scope),
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
                }),
            }
        }
    }
    pub(super) fn render_object_fact_18(
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
                clip: true,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (9.0) as f32,
                    right: (12.0) as f32,
                    bottom: (9.0) as f32,
                    left: (12.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (None).map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[63]),
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
                        key: format!("{}/@text:475", use_scope),
                        size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[71]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("size".to_owned()).to_string(),
                    });
                    children.push(::ducktape_view_guest::wire::Node::Space {
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
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
                        key: format!("{}/@text:482", use_scope),
                        size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[13]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: (crate::host::size_label(self.preview_entry.size).to_owned())
                            .to_string(),
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:470", use_scope),
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
                }),
            }
        }
    }
    pub(super) fn render_object_panel(
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
                        key: format!("{}/@container:365", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: Some(::ducktape_view_guest::wire::Length::Fill),
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[54]))
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
                                    key: format!("{}/@container:371", use_scope),
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: Some(
                                        ::ducktape_view_guest::wire::Length::Fixed((50.0) as f32),
                                    ),
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
                                                key: format!("{}/@text:382", use_scope),
                                                size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                color: Some(palette.colors[4]),
                                                font: ::ducktape_view_guest::wire::Font {
                                                    monospace: false,
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                },
                                                width: None,
                                                align_x: None,
                                                content: ("Object".to_owned()).to_string(),
                                            });
                                        children
                                            .push(::ducktape_view_guest::wire::Node::Space {
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                height: None,
                                            });
                                        if self.preview_entry.kind == "dir"  {
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
                                                    key: format!("{}/@container:390", use_scope),
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
                                                    background: (Some(palette.colors[55]))
                                                        .map(::ducktape_view_guest::wire::Background::Color),
                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                        color: None,
                                                        width: None,
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
                                                        key: format!("{}/@text:396", use_scope),
                                                        size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                        color: Some(palette.colors[41]),
                                                        font: ::ducktape_view_guest::wire::Font {
                                                            monospace: false,
                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                        },
                                                        width: None,
                                                        align_x: None,
                                                        content: ("DIR".to_owned()).to_string(),
                                                    }),
                                                });
                                        }
                                        if self.preview_entry.kind != "dir"  {
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
                                                    key: format!("{}/@container:403", use_scope),
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
                                                    background: (Some(palette.colors[55]))
                                                        .map(::ducktape_view_guest::wire::Background::Color),
                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                        color: None,
                                                        width: None,
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
                                                        key: format!("{}/@text:409", use_scope),
                                                        size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                        color: Some(palette.colors[41]),
                                                        font: ::ducktape_view_guest::wire::Font {
                                                            monospace: false,
                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                        },
                                                        width: None,
                                                        align_x: None,
                                                        content: ("FILE".to_owned()).to_string(),
                                                    }),
                                                });
                                        }
                                        ::ducktape_view_guest::wire::Node::Linear {
                                            max_width: None,
                                            clip: false,
                                            key: format!("{}/@layout:376", use_scope),
                                            wrap: None,
                                            axis: ::ducktape_view_guest::wire::Axis::Row,
                                            spacing: Some((8.0) as f32),
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
                                    key: format!("{}/@container:415", use_scope),
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
                                .push(::ducktape_view_guest::wire::Node::Scroll {
                                    on_scroll: None,
                                    virtual_rows: false,
                                    key: format!("{}/@layout:421", use_scope),
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
                                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                    }),
                                                },
                                                key: format!("{}/@text:427", use_scope),
                                                size: Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                color: Some(palette.colors[7]),
                                                font: ::ducktape_view_guest::wire::Font {
                                                    monospace: false,
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                },
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                align_x: None,
                                                content: (self.preview_entry.name.to_owned()).to_string(),
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
                                                key: format!("{}/@container:433", use_scope),
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                height: None,
                                                padding: Some(::ducktape_view_guest::wire::Edges {
                                                    top: (4.0) as f32,
                                                    right: (0.0) as f32,
                                                    bottom: (0.0) as f32,
                                                    left: (0.0) as f32,
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
                                                    key: format!("{}/@text:434", use_scope),
                                                    size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: Some(palette.colors[72]),
                                                    font: ::ducktape_view_guest::wire::Font {
                                                        monospace: false,
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    },
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    align_x: None,
                                                    content: (self.preview_entry.path.to_owned()).to_string(),
                                                }),
                                            });
                                        children
                                            .push({
                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                if (self.preview_entry.object).is_empty() {
                                                    children
                                                        .push(
                                                            self
                                                                .render_object_fact_15(
                                                                    palette,
                                                                    format!("{}/ObjectFact@1073", use_scope),
                                                                ),
                                                        );
                                                }
                                                if !(self.preview_entry.object).is_empty()  {
                                                    children
                                                        .push(
                                                            self
                                                                .render_object_fact_16(
                                                                    palette,
                                                                    format!("{}/ObjectFact@1075", use_scope),
                                                                ),
                                                        );
                                                }
                                                if self.preview_entry.kind == "dir"  {
                                                    children
                                                        .push(
                                                            self
                                                                .render_object_fact_17(
                                                                    palette,
                                                                    format!("{}/ObjectFact@1077", use_scope),
                                                                ),
                                                        );
                                                }
                                                if self.preview_entry.kind != "dir"  {
                                                    children
                                                        .push(
                                                            self
                                                                .render_object_fact_18(
                                                                    palette,
                                                                    format!("{}/ObjectFact@1079", use_scope),
                                                                ),
                                                        );
                                                }
                                                ::ducktape_view_guest::wire::Node::Linear {
                                                    max_width: None,
                                                    clip: false,
                                                    key: format!("{}/@layout:440", use_scope),
                                                    wrap: None,
                                                    axis: ::ducktape_view_guest::wire::Axis::Column,
                                                    spacing: Some((7.0) as f32),
                                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                                        top: (14.0) as f32,
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
                                            key: format!("{}/@layout:426", use_scope),
                                            wrap: None,
                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                            spacing: None,
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
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:370", use_scope),
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
                        }),
                    });
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Row,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fixed(
                        (self.object_width) as f32,
                    )),
                    height: Some(::ducktape_view_guest::wire::Length::Fill),
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
}
