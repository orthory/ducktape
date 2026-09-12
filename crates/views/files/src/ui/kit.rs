use super::*;
impl super::FilesView {
    pub(super) fn render_modal_shell_1(
        &self,
        palette: Palette,
        use_scope: String,
        cb_0: impl Fn() -> Message + Clone + 'static,
        cb_1: impl Fn() -> Message + Clone + 'static,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: Some(palette.colors[48]),
                    x: None,
                    y: Some((24.0) as f32),
                    blur: Some((60.0) as f32),
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fixed((418.0) as f32)),
                height: None,
                padding: None,
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[3]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
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
                                    key: format!("{}/@text:93", use_scope),
                                    size: Some(((16.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[7]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    align_x: None,
                                    content: ("Delete this object".to_owned()).to_string(),
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Button {
                                    checked: None,
                                    expanded: None,
                                    description: None,
                                    key: format!("{}/@button:13", use_scope),
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
                                            key: format!("{}/@container:21", use_scope),
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
                                                            "Geist".into(),
                                                        ),
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                    }),
                                                },
                                                key: format!("{}/@text:27", use_scope),
                                                size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                color: Some(palette.colors[5]),
                                                font: ::ducktape_view_guest::wire::Font {
                                                    monospace: false,
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                },
                                                width: None,
                                                align_x: None,
                                                content: ("×".to_owned()).to_string(),
                                            }),
                                        }),
                                    ),
                                    label: Some(String::from("Cancel".to_owned())),
                                    on_press: if (*self.derived_loading())  {
                                        None
                                    } else {
                                        Some(::ducktape_view_guest::slots::message((cb_0)()))
                                    },
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
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:88", use_scope),
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
                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:37", use_scope),
                                    size: Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[4]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    align_x: None,
                                    content: (self.delete_target.to_owned()).to_string(),
                                });
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
                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[70]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    align_x: None,
                                    content: ("The committed object is removed from duckfs for every member. Earlier snapshots keep their copies."
                                        .to_owned())
                                        .to_string(),
                                });
                            children
                                .push({
                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                    children
                                        .push(::ducktape_view_guest::wire::Node::Button {
                                            checked: None,
                                            expanded: None,
                                            description: None,
                                            key: format!("{}/@button:54", use_scope),
                                            content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                String::from("Cancel"),
                                            ),
                                            label: None,
                                            on_press: if (*self.derived_loading())  {
                                                None
                                            } else {
                                                Some(::ducktape_view_guest::slots::message((cb_0)()))
                                            },
                                            width: None,
                                            height: None,
                                            padding: Some(
                                                ::ducktape_view_guest::wire::Edges::all((7.0) as f32),
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
                                    children
                                        .push(::ducktape_view_guest::wire::Node::Button {
                                            checked: None,
                                            expanded: None,
                                            description: None,
                                            key: format!("{}/@button:59", use_scope),
                                            content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                Box::new(::ducktape_view_guest::wire::Node::Text {
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
                                                    key: format!("{}/@text:65", use_scope),
                                                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: None,
                                                    font: ::ducktape_view_guest::wire::Font {
                                                        monospace: false,
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    },
                                                    width: None,
                                                    align_x: None,
                                                    content: ("Delete object".to_owned()).to_string(),
                                                }),
                                            ),
                                            label: Some(String::from("Delete object".to_owned())),
                                            on_press: if (*self.derived_loading())  {
                                                None
                                            } else {
                                                Some(::ducktape_view_guest::slots::message((cb_1)()))
                                            },
                                            width: None,
                                            height: None,
                                            padding: Some(
                                                ::ducktape_view_guest::wire::Edges::all((7.0) as f32),
                                            ),
                                            style: ::ducktape_view_guest::wire::ButtonStyle {
                                                preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                    base: ::ducktape_view_guest::wire::Face {
                                                        background: Some(palette.colors[20]),
                                                        text: Some(palette.colors[21]),
                                                        border: Some(::ducktape_view_guest::wire::Border {
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
                                        key: format!("{}/@layout:49", use_scope),
                                        wrap: None,
                                        axis: ::ducktape_view_guest::wire::Axis::Row,
                                        spacing: Some((8.0) as f32),
                                        padding: None,
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: None,
                                        align: Some(::ducktape_view_guest::wire::AlignX::Right),
                                        background: None,
                                        border: None,
                                        children: children,
                                    }
                                });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:36", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                spacing: Some((13.0) as f32),
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
                        key: format!("{}/@layout:80", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: Some((13.0) as f32),
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (20.0) as f32,
                            right: (22.0) as f32,
                            bottom: (22.0) as f32,
                            left: (22.0) as f32,
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
        }
    }
    pub(super) fn render_confirm_delete_2(
        &self,
        palette: Palette,
        use_scope: String,
        ctx_0: String,
        _cb_0: impl Fn(String) -> Message + Clone + 'static,
        _cb_1: impl Fn(String) -> Message + Clone + 'static,
        _cb_2: impl Fn(String) -> Message + Clone + 'static,
        _cb_3: impl Fn() -> Message + Clone + 'static,
        cb_4: impl Fn() -> Message + Clone + 'static,
        cb_5: impl Fn() -> Message + Clone + 'static,
        _cb_6: impl Fn() -> Message + Clone + 'static,
        _cb_7: impl Fn() -> Message + Clone + 'static,
        _cb_8: impl Fn(String) -> Message + Clone + 'static,
        _cb_9: impl Fn(String) -> Message + Clone + 'static,
        _cb_10: impl Fn(String) -> Message + Clone + 'static,
        _cb_11: impl Fn(String) -> Message + Clone + 'static,
        _cb_12: impl Fn(String) -> Message + Clone + 'static,
        _cb_13: impl Fn(f64, f64) -> Message + Clone + 'static,
        _cb_14: impl Fn(f64, f64) -> Message + Clone + 'static,
        _cb_15: impl Fn(f64, f64) -> Message + Clone + 'static,
    ) -> ::ducktape_view_guest::wire::Node {
        self.render_modal_shell_1(
            palette,
            format!("{}/ModalShell@1650", use_scope),
            ({
                let _route_state_scope_0 = (ctx_0).clone();
                let route_callback = (cb_5).clone();
                move || (route_callback)()
            })
            .clone(),
            ({
                let _route_state_scope_0 = (ctx_0).clone();
                let route_callback = (cb_4).clone();
                move || (route_callback)()
            })
            .clone(),
        )
    }
    pub(super) fn render_empty_state_6(
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
                        key: format!("{}/@container:116", use_scope),
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
                            key: format!("{}/@text:126", use_scope),
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
                        key: format!("{}/@text:127", use_scope),
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
                            key: format!("{}/@text:128", use_scope),
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
                        key: format!("{}/@layout:111", use_scope),
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
    pub(super) fn render_group_label_7(
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
                content: ("CHANGES VS HEAD".to_owned()).to_string(),
            }
        }
    }
    pub(super) fn render_group_label_8(
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
                content: ("SNAPSHOTS".to_owned()).to_string(),
            }
        }
    }
    pub(super) fn render_empty_plate_10(
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
                    key: format!("{}/@text:140", use_scope),
                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[71]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: ("Empty directory — nothing is committed under this path.".to_owned())
                        .to_string(),
                }),
            }
        }
    }
}
