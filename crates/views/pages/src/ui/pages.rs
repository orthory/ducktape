impl PagesView {
    pub(crate) fn pages(&self, palette: Palette, use_scope: String) -> wire::Node {
        {
            let mut children: Vec<wire::Node> = Vec::new();
            children.push({
                let node_scope = format!("{}/page-list", use_scope);
                wire::Node::Container {
                    shadow: wire::Shadow {
                        color: None,
                        x: None,
                        y: None,
                        blur: None,
                    },
                    max_width: None,
                    max_height: None,
                    clip: true,
                    key: node_scope.clone(),
                    width: Some(wire::Length::Fixed((self.sidebar_width) as f32)),
                    height: Some(wire::Length::Fill),
                    padding: None,
                    align_x: None,
                    align_y: None,
                    background: (Some(palette.colors[54])).map(wire::Background::Color),
                    border: None,
                    snap: None,
                    content: Box::new({
                        let mut children: Vec<wire::Node> = Vec::new();
                        children.push(
                            self.sidebar_header(
                                palette,
                                format!("{}/SidebarHeader@670", use_scope),
                            ),
                        );
                        if self.page_create_open {
                            children.push({
                                let mut children: Vec<wire::Node> = Vec::new();
                                children.push({
                                    let node_scope = format!("{}/new-page", node_scope);
                                    wire::Node::Input {
                                        options: wire::InputOptions {
                                            label: "New page title".to_owned(),
                                            description: None,
                                            disabled: ((self.loading
                                                || (self.busy || (!(self.host_error).is_empty())))
                                                || (!self.connected)),
                                            padding: Some(wire::Edges::all((6.2) as f32)),
                                            text_size: Some((13.0) as f32),
                                            line_height: Some((1.2) as f32),
                                            align: None,
                                            font: Some(wire::NamedFont {
                                                family: wire::FontFamily::Named("Geist".into()),
                                                weight: wire::Weight::Normal,
                                                stretch: wire::FontStretch::Normal,
                                                style: wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: node_scope.clone(),
                                        placeholder: String::from("New page".to_owned()),
                                        value: (self.page_draft).to_string(),
                                        on_input: ::ducktape_view_guest::slots::handler::<
                                            String,
                                            Message,
                                        >(
                                            Box::new({
                                                let route = Message::PageDraftChanged
                                                    as fn(String) -> Message;
                                                move |sent: String| Some(route(sent))
                                            }),
                                        ),
                                        on_submit: Some(::ducktape_view_guest::slots::message(
                                            Message::CreatePageSubmit,
                                        )),
                                        width: Some(wire::Length::Fill),
                                        secure: (false),
                                        style: Box::new(wire::InputStyle {
                                            utility: wire::InputFace {
                                                background: Some(palette.colors[3]),
                                                border: Some(wire::Border {
                                                    color: Some(palette.colors[39]),
                                                    width: Some(1f32),
                                                    radius: Some([10f32; 4]),
                                                }),
                                                ..Default::default()
                                            },
                                            focus_border: Some(palette.colors[42]),
                                            focused_hovered: None,
                                            active: wire::InputFace {
                                                icon: None,
                                                background: Some(palette.colors[6]),
                                                border: Some(wire::Border {
                                                    color: Some({
                                                        let mut color = palette.colors[4];
                                                        color.0[3] = 0.160000;
                                                        color
                                                    }),
                                                    width: Some(
                                                        ((1.0) as f32).max(0.0).min(f32::MAX),
                                                    ),
                                                    radius: Some([
                                                        ((8.0) as f32).max(0.0).min(f32::MAX),
                                                        ((8.0) as f32).max(0.0).min(f32::MAX),
                                                        ((8.0) as f32).max(0.0).min(f32::MAX),
                                                        ((8.0) as f32).max(0.0).min(f32::MAX),
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
                                            hovered: Some(wire::InputFace {
                                                icon: None,
                                                background: Some(palette.colors[55]),
                                                border: Some(wire::Border {
                                                    color: Some({
                                                        let mut color = palette.colors[4];
                                                        color.0[3] = 0.210000;
                                                        color
                                                    }),
                                                    width: None,
                                                    radius: None,
                                                }),
                                                value: None,
                                                placeholder: None,
                                                selection: None,
                                            }),
                                            focused: None,
                                            disabled: Some(wire::InputFace {
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
                                children.push(wire::Node::Button {
                                    checked: None,
                                    expanded: None,
                                    description: None,
                                    key: format!("{}/@button:112", use_scope),
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
                                            key: format!("{}/@container:120", use_scope),
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
                                                    wrapping: None,
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
                                                key: format!("{}/@text:126", use_scope),
                                                size: Some(
                                                    ((14.0) as f32).max(f32::EPSILON).min(f32::MAX),
                                                ),
                                                color: None,
                                                font: wire::Font {
                                                    monospace: false,
                                                    weight: wire::Weight::Normal,
                                                },
                                                width: None,
                                                align_x: None,
                                                content: "+".to_owned(),
                                            }),
                                        },
                                    )),
                                    label: Some(String::from("Create page".to_owned())),
                                    on_press: if (((self.loading
                                        || (self.busy || (!(self.host_error).is_empty())))
                                        || (!self.connected))
                                        || ((self.page_draft).trim().to_owned()).is_empty())
                                    {
                                        None
                                    } else {
                                        Some(::ducktape_view_guest::slots::message(
                                            Message::CreatePageSubmit,
                                        ))
                                    },
                                    width: Some(wire::Length::Fixed((28.0) as f32)),
                                    height: Some(wire::Length::Fixed((28.0) as f32)),
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
                                        active: wire::Face::default(),
                                        hovered: None,
                                        pressed: None,
                                        disabled: None,
                                    },
                                });
                                wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:92", use_scope),
                                    wrap: None,
                                    axis: wire::Axis::Row,
                                    spacing: Some((5.0) as f32),
                                    padding: None,
                                    width: Some(wire::Length::Fill),
                                    height: Some(wire::Length::Fixed((28.0) as f32)),
                                    align: Some(wire::AlignX::Center),
                                    background: None,
                                    border: None,
                                    children,
                                }
                            });
                        }
                        children.push(wire::Node::Scroll {
                            on_scroll: None,
                            virtual_rows: false,
                            key: format!("{}/@layout:127", use_scope),
                            direction: wire::ScrollDirection::Vertical,
                            width: Some(wire::Length::Fill),
                            height: Some(wire::Length::Fill),
                            bar_hidden: false,
                            bar_width: None,
                            bar_margin: None,
                            scroller_width: None,
                            bar_spacing: None,
                            anchor_x: wire::ScrollAnchor::Start,
                            anchor_y: wire::ScrollAnchor::Start,
                            auto_scroll: (false),
                            background: None,
                            border: None,
                            content: Box::new({
                                let mut children: Vec<wire::Node> = Vec::new();
                                for (index, page) in self.pages.iter().enumerate() {
                                    let for_scope = format!("{}/@for:754({})", use_scope, index);
                                    children.push(self.page_button(
                                        palette,
                                        format!("{}/PageButton@755", for_scope),
                                        page.clone(),
                                        (page.id == self.active_page),
                                    ));
                                }
                                wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:132", use_scope),
                                    wrap: None,
                                    axis: wire::Axis::Column,
                                    spacing: Some((2.0) as f32),
                                    padding: None,
                                    width: Some(wire::Length::Fill),
                                    height: None,
                                    align: None,
                                    background: None,
                                    border: None,
                                    children,
                                }
                            }),
                        });
                        wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:40", use_scope),
                            wrap: None,
                            axis: wire::Axis::Column,
                            spacing: Some((0.0) as f32),
                            padding: None,
                            width: Some(wire::Length::Fill),
                            height: Some(wire::Length::Fill),
                            align: None,
                            background: None,
                            border: None,
                            children,
                        }
                    }),
                }
            });
            children.push({
                let node_scope = format!("{}/sidebar-resize", use_scope);
                wire::Node::ResizeHandle {
                    key: node_scope.clone(),
                    on_press: None,
                    on_release: None,
                    on_drag: Some(
                        ::ducktape_view_guest::slots::handler::<(f64, f64), Message>(Box::new({
                            let route = {
                                let route_callback = (move |event_0, event_1| {
                                    Message::SidebarResized(event_0, event_1)
                                })
                                .clone();
                                move |delta: (f64, f64)| (route_callback)(delta.0, delta.1)
                            };
                            move |sent: (f64, f64)| Some(route(sent))
                        })),
                    ),
                    cursor: Some(wire::mouse::Cursor::ResizingHorizontally),
                    content: Box::new({
                        let node_scope = format!("{}/sidebar-divider", node_scope);
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
                            width: Some(wire::Length::Fixed((10.0) as f32)),
                            height: Some(wire::Length::Fill),
                            padding: None,
                            align_x: Some(wire::AlignX::Left),
                            align_y: None,
                            background: (None).map(wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(wire::Node::Container {
                                shadow: wire::Shadow {
                                    color: None,
                                    x: None,
                                    y: None,
                                    blur: None,
                                },
                                max_width: None,
                                max_height: None,
                                clip: false,
                                key: format!("{}/@container:147", use_scope),
                                width: Some(wire::Length::Fixed((1.0) as f32)),
                                height: Some(wire::Length::Fill),
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
                            }),
                        }
                    }),
                }
            });
            children
                .push({
                    let mut children: Vec<wire::Node> = Vec::new();
                    children
                        .push({
                            let mut children: Vec<wire::Node> = Vec::new();
                            if (!(self.host_error).is_empty()) {
                                children
                                    .push(wire::Node::Container {
                                        shadow: wire::Shadow {
                                            color: None,
                                            x: None,
                                            y: None,
                                            blur: None,
                                        },
                                        max_width: None,
                                        max_height: None,
                                        clip: false,
                                        key: format!("{}/@container:156", use_scope),
                                        width: Some(wire::Length::Fill),
                                        height: None,
                                        padding: Some(wire::Edges {
                                            top: (12.0) as f32,
                                            right: (12.0) as f32,
                                            bottom: (12.0) as f32,
                                            left: (12.0) as f32,
                                        }),
                                        align_x: None,
                                        align_y: None,
                                        background: (Some(palette.colors[22]))
                                            .map(wire::Background::Color),
                                        border: None,
                                        snap: None,
                                        content: Box::new({
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
                                                            weight: wire::Weight::Normal,
                                                            stretch: wire::FontStretch::Normal,
                                                            style: wire::FontStyle::Normal,
                                                        }),
                                                    },
                                                    key: format!("{}/@text:158", use_scope),
                                                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: Some(palette.colors[20]),
                                                    font: wire::Font {
                                                        monospace: false,
                                                        weight: wire::Weight::Normal,
                                                    },
                                                    width: None,
                                                    align_x: None,
                                                    content: "Pages could not load".to_owned(),
                                                });
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
                                                            weight: wire::Weight::Normal,
                                                            stretch: wire::FontStretch::Normal,
                                                            style: wire::FontStyle::Normal,
                                                        }),
                                                    },
                                                    key: format!("{}/@text:159", use_scope),
                                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: Some(palette.colors[20]),
                                                    font: wire::Font {
                                                        monospace: false,
                                                        weight: wire::Weight::Normal,
                                                    },
                                                    width: None,
                                                    align_x: None,
                                                    content: self.host_error.to_owned(),
                                                });
                                            wire::Node::Linear {
                                                max_width: None,
                                                clip: false,
                                                key: format!("{}/@layout:157", use_scope),
                                                wrap: None,
                                                axis: wire::Axis::Column,
                                                spacing: Some((4.0) as f32),
                                                padding: None,
                                                width: Some(wire::Length::Fill),
                                                height: None,
                                                align: None,
                                                background: None,
                                                border: None,
                                                children,
                                            }
                                        }),
                                    });
                            }
                            if (self.connected && (!(self.active_page).is_empty())) {
                                children
                                    .push({
                                        let mut children: Vec<wire::Node> = Vec::new();
                                        children
                                            .push(wire::Node::Container {
                                                shadow: wire::Shadow {
                                                    color: None,
                                                    x: None,
                                                    y: None,
                                                    blur: None,
                                                },
                                                max_width: None,
                                                max_height: None,
                                                clip: false,
                                                key: format!("{}/@container:164", use_scope),
                                                width: Some(wire::Length::Fill),
                                                height: Some(wire::Length::Fixed((50.0) as f32)),
                                                padding: Some(wire::Edges {
                                                    top: (0.0) as f32,
                                                    right: (22.0) as f32,
                                                    bottom: (0.0) as f32,
                                                    left: (22.0) as f32,
                                                }),
                                                align_x: None,
                                                align_y: None,
                                                background: (None).map(wire::Background::Color),
                                                border: None,
                                                snap: None,
                                                content: Box::new({
                                                    let mut children: Vec<wire::Node> = Vec::new();
                                                    children
                                                        .push(wire::Node::Container {
                                                            shadow: wire::Shadow {
                                                                color: None,
                                                                x: None,
                                                                y: None,
                                                                blur: None,
                                                            },
                                                            max_width: None,
                                                            max_height: None,
                                                            clip: true,
                                                            key: format!("{}/@container:185", use_scope),
                                                            width: Some(wire::Length::Fill),
                                                            height: None,
                                                            padding: None,
                                                            align_x: None,
                                                            align_y: None,
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
                                                                        weight: wire::Weight::Semibold,
                                                                        stretch: wire::FontStretch::Normal,
                                                                        style: wire::FontStyle::Normal,
                                                                    }),
                                                                },
                                                                key: format!("{}/@text:186", use_scope),
                                                                size: Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                color: Some(palette.colors[4]),
                                                                font: wire::Font {
                                                                    monospace: false,
                                                                    weight: wire::Weight::Normal,
                                                                },
                                                                width: None,
                                                                align_x: None,
                                                                content: self.active_page_title.to_owned(),
                                                            }),
                                                        });
                                                    if (!(self.active_page_parent).is_empty()) {
                                                        children
                                                            .push(wire::Node::Text {
                                                                options: wire::TextOptions {
                                                                    height: None,
                                                                    align_y: None,
                                                                    line_height: None,
                                                                    shaping: None,
                                                                    wrapping: Some(wire::Wrapping::None),
                                                                    tracking: 0.0f32,
                                                                    font: Some(wire::NamedFont {
                                                                        family: wire::FontFamily::Named("Geist Mono".into()),
                                                                        weight: wire::Weight::Normal,
                                                                        stretch: wire::FontStretch::Normal,
                                                                        style: wire::FontStyle::Normal,
                                                                    }),
                                                                },
                                                                key: format!("{}/@text:196", use_scope),
                                                                size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                color: Some(palette.colors[72]),
                                                                font: wire::Font {
                                                                    monospace: false,
                                                                    weight: wire::Weight::Normal,
                                                                },
                                                                width: None,
                                                                align_x: None,
                                                                content: self.active_page_parent.to_owned(),
                                                            });
                                                    }
                                                    children
                                                        .push({
                                                            let node_scope = format!("{}/page-search", use_scope);
                                                            wire::Node::Input {
                                                                options: wire::InputOptions {
                                                                    label: "Search pages".to_owned(),
                                                                    description: None,
                                                                    disabled: (((!(self.host_error).is_empty())
                                                                        || (!self.connected)) || self.page_searching),
                                                                    padding: Some(wire::Edges::all((6.2) as f32)),
                                                                    text_size: Some((13.0) as f32),
                                                                    line_height: Some((1.2) as f32),
                                                                    align: None,
                                                                    font: Some(wire::NamedFont {
                                                                        family: wire::FontFamily::Named("Geist".into()),
                                                                        weight: wire::Weight::Normal,
                                                                        stretch: wire::FontStretch::Normal,
                                                                        style: wire::FontStyle::Normal,
                                                                    }),
                                                                },
                                                                key: node_scope.clone(),
                                                                placeholder: String::from("Search pages…".to_owned()),
                                                                value: (self.page_search_draft).to_string(),
                                                                on_input: ::ducktape_view_guest::slots::handler::<
                                                                    String,
                                                                    Message,
                                                                >(
                                                                    Box::new({
                                                                        let route = Message::SearchDraftChanged
                                                                            as fn(String) -> Message;
                                                                        move |sent: String| Some(route(sent))
                                                                    }),
                                                                ),
                                                                on_submit: Some(
                                                                    ::ducktape_view_guest::slots::message(
                                                                        Message::SearchPagesSubmit,
                                                                    ),
                                                                ),
                                                                width: Some(wire::Length::Fixed((190.0) as f32)),
                                                                secure: (false),
                                                                style: Box::new(wire::InputStyle {
                                                                    utility: wire::InputFace {
                                                                        background: Some(palette.colors[3]),
                                                                        border: Some(wire::Border {
                                                                            color: Some(palette.colors[39]),
                                                                            width: Some(1f32),
                                                                            radius: Some([10f32; 4]),
                                                                        }),
                                                                        ..Default::default()
                                                                    },
                                                                    focus_border: Some(palette.colors[42]),
                                                                    focused_hovered: None,
                                                                    active: wire::InputFace {
                                                                        icon: None,
                                                                        background: Some(
                                                                            wire::Rgba([
                                                                                0.0 / 255.0,
                                                                                0.0 / 255.0,
                                                                                0.0 / 255.0,
                                                                                0.000000,
                                                                            ]),
                                                                        ),
                                                                        border: Some(wire::Border {
                                                                            color: Some(
                                                                                wire::Rgba([
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
                                                                        value: Some(palette.colors[4]),
                                                                        placeholder: Some(palette.colors[5]),
                                                                        selection: Some({
                                                                            let mut color = palette.colors[4];
                                                                            color.0[3] = 0.180000;
                                                                            color
                                                                        }),
                                                                    },
                                                                    hovered: Some(wire::InputFace {
                                                                        icon: None,
                                                                        background: Some({
                                                                            let mut color = palette.colors[4];
                                                                            color.0[3] = 0.050000;
                                                                            color
                                                                        }),
                                                                        border: Some(wire::Border {
                                                                            color: Some({
                                                                                let mut color = palette.colors[4];
                                                                                color.0[3] = 0.080000;
                                                                                color
                                                                            }),
                                                                            width: None,
                                                                            radius: None,
                                                                        }),
                                                                        value: None,
                                                                        placeholder: None,
                                                                        selection: None,
                                                                    }),
                                                                    focused: Some(wire::InputFace {
                                                                        icon: None,
                                                                        background: Some({
                                                                            let mut color = palette.colors[4];
                                                                            color.0[3] = 0.050000;
                                                                            color
                                                                        }),
                                                                        border: Some(wire::Border {
                                                                            color: Some(palette.colors[42]),
                                                                            width: None,
                                                                            radius: None,
                                                                        }),
                                                                        value: None,
                                                                        placeholder: None,
                                                                        selection: None,
                                                                    }),
                                                                    disabled: Some(wire::InputFace {
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
                                                    if ((!((self.page_search_draft).trim().to_owned())
                                                        .is_empty()) || (!(self.page_search_hits).is_empty()))
                                                    {
                                                        children
                                                            .push(wire::Node::Button {
                                                                checked: None,
                                                                expanded: None,
                                                                description: None,
                                                                key: format!("{}/@button:224", use_scope),
                                                                content: wire::ButtonContent::Child(
                                                                    Box::new(wire::Node::Container {
                                                                        shadow: wire::Shadow {
                                                                            color: None,
                                                                            x: None,
                                                                            y: None,
                                                                            blur: None,
                                                                        },
                                                                        max_width: None,
                                                                        max_height: None,
                                                                        clip: false,
                                                                        key: format!("{}/@container:232", use_scope),
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
                                                                                wrapping: None,
                                                                                tracking: 0.0f32,
                                                                                font: Some(wire::NamedFont {
                                                                                    family: wire::FontFamily::Named("Geist".into()),
                                                                                    weight: wire::Weight::Normal,
                                                                                    stretch: wire::FontStretch::Normal,
                                                                                    style: wire::FontStyle::Normal,
                                                                                }),
                                                                            },
                                                                            key: format!("{}/@text:238", use_scope),
                                                                            size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                            color: None,
                                                                            font: wire::Font {
                                                                                monospace: false,
                                                                                weight: wire::Weight::Normal,
                                                                            },
                                                                            width: None,
                                                                            align_x: None,
                                                                            content: "×".to_owned(),
                                                                        }),
                                                                    }),
                                                                ),
                                                                label: Some(String::from("Clear page search".to_owned())),
                                                                on_press: if ((!(self.host_error).is_empty())) {
                                                                    None
                                                                } else {
                                                                    Some(
                                                                        ::ducktape_view_guest::slots::message(
                                                                            Message::ClearPageSearch,
                                                                        ),
                                                                    )
                                                                },
                                                                width: Some(wire::Length::Fixed((28.0) as f32)),
                                                                height: Some(wire::Length::Fixed((28.0) as f32)),
                                                                padding: Some(wire::Edges::all((0.0) as f32)),
                                                                style: wire::ButtonStyle {
                                                                    preset: wire::ButtonPreset::Primary,
                                                                    recipe: Some(wire::ButtonRecipe {
                                                                        base: wire::Face {
                                                                            background: Some(
                                                                                wire::Rgba([
                                                                                    0.0 / 255.0,
                                                                                    0.0 / 255.0,
                                                                                    0.0 / 255.0,
                                                                                    0.000000,
                                                                                ]),
                                                                            ),
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
                                                                        background: Some(
                                                                            wire::Rgba([
                                                                                0.0 / 255.0,
                                                                                0.0 / 255.0,
                                                                                0.0 / 255.0,
                                                                                0.000000,
                                                                            ]),
                                                                        ),
                                                                        text: Some(palette.colors[5]),
                                                                        border: Some(wire::Border {
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
                                                                    hovered: Some(wire::Face {
                                                                        background: Some({
                                                                            let mut color = palette.colors[4];
                                                                            color.0[3] = 0.100000;
                                                                            color
                                                                        }),
                                                                        text: Some(palette.colors[4]),
                                                                        border: None,
                                                                    }),
                                                                    pressed: Some(wire::Face {
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
                                                    }
                                                    children
                                                        .push(wire::Node::Button {
                                                            checked: None,
                                                            expanded: Some(self.block_comments_open),
                                                            description: None,
                                                            key: format!("{}/@button:249", use_scope),
                                                            content: wire::ButtonContent::Child(
                                                                Box::new({
                                                                    let mut children: Vec<wire::Node> = Vec::new();
                                                                    children
                                                                        .push(
                                                                            self
                                                                                .icon(
                                                                                    palette,
                                                                                    format!("{}/Icon@883", use_scope),
                                                                                    "nav-chat",
                                                                                    14.,
                                                                                ),
                                                                        );
                                                                    children
                                                                        .push(wire::Node::Text {
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
                                                                            key: format!("{}/@text:267", use_scope),
                                                                            size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                            color: Some(palette.colors[5]),
                                                                            font: wire::Font {
                                                                                monospace: false,
                                                                                weight: wire::Weight::Normal,
                                                                            },
                                                                            width: None,
                                                                            align_x: None,
                                                                            content: "Comments".to_owned(),
                                                                        });
                                                                    if (self.thread_total > 0) {
                                                                        children
                                                                            .push(wire::Node::Text {
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
                                                                                key: format!("{}/@text:273", use_scope),
                                                                                size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                color: Some(palette.colors[73]),
                                                                                font: wire::Font {
                                                                                    monospace: false,
                                                                                    weight: wire::Weight::Normal,
                                                                                },
                                                                                width: None,
                                                                                align_x: None,
                                                                                content: (crate::host::count_label(self.thread_total))
                                                                                    .to_string(),
                                                                            });
                                                                    }
                                                                    wire::Node::Linear {
                                                                        max_width: None,
                                                                        clip: false,
                                                                        key: format!("{}/@layout:257", use_scope),
                                                                        wrap: None,
                                                                        axis: wire::Axis::Row,
                                                                        spacing: Some((5.0) as f32),
                                                                        padding: None,
                                                                        width: None,
                                                                        height: Some(wire::Length::Fill),
                                                                        align: Some(wire::AlignX::Center),
                                                                        background: None,
                                                                        border: None,
                                                                        children,
                                                                    }
                                                                }),
                                                            ),
                                                            label: Some(String::from("Comments".to_owned())),
                                                            on_press: if ((self.busy
                                                                || (!(self.host_error).is_empty())))
                                                            {
                                                                None
                                                            } else {
                                                                Some(
                                                                    ::ducktape_view_guest::slots::message(
                                                                        Message::ToggleBlockComments,
                                                                    ),
                                                                )
                                                            },
                                                            width: None,
                                                            height: Some(wire::Length::Fixed((26.0) as f32)),
                                                            padding: Some(wire::Edges::all((5.0) as f32)),
                                                            style: wire::ButtonStyle {
                                                                preset: wire::ButtonPreset::Primary,
                                                                recipe: Some(wire::ButtonRecipe {
                                                                    base: wire::Face {
                                                                        background: Some(
                                                                            wire::Rgba([
                                                                                0.0 / 255.0,
                                                                                0.0 / 255.0,
                                                                                0.0 / 255.0,
                                                                                0.000000,
                                                                            ]),
                                                                        ),
                                                                        text: Some(palette.colors[4]),
                                                                        border: Some(wire::Border {
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
                                                                    font: Some(wire::NamedFont {
                                                                        family: wire::FontFamily::Named("Geist".into()),
                                                                        weight: wire::Weight::Semibold,
                                                                        stretch: wire::FontStretch::Normal,
                                                                        style: wire::FontStyle::Normal,
                                                                    }),
                                                                }),
                                                                active: wire::Face {
                                                                    background: Some(
                                                                        wire::Rgba([
                                                                            0.0 / 255.0,
                                                                            0.0 / 255.0,
                                                                            0.0 / 255.0,
                                                                            0.000000,
                                                                        ]),
                                                                    ),
                                                                    text: Some(palette.colors[5]),
                                                                    border: Some(wire::Border {
                                                                        color: Some(
                                                                            wire::Rgba([
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
                                                                hovered: Some(wire::Face {
                                                                    background: Some({
                                                                        let mut color = palette.colors[4];
                                                                        color.0[3] = 0.080000;
                                                                        color
                                                                    }),
                                                                    text: Some(palette.colors[4]),
                                                                    border: Some(wire::Border {
                                                                        color: Some({
                                                                            let mut color = palette.colors[4];
                                                                            color.0[3] = 0.100000;
                                                                            color
                                                                        }),
                                                                        width: None,
                                                                        radius: None,
                                                                    }),
                                                                }),
                                                                pressed: Some(wire::Face {
                                                                    background: Some({
                                                                        let mut color = palette.colors[4];
                                                                        color.0[3] = 0.120000;
                                                                        color
                                                                    }),
                                                                    text: Some(palette.colors[4]),
                                                                    border: None,
                                                                }),
                                                                disabled: None,
                                                            },
                                                        });
                                                    children
                                                        .push(wire::Node::Button {
                                                            checked: None,
                                                            expanded: None,
                                                            description: None,
                                                            key: format!("{}/@button:285", use_scope),
                                                            content: wire::ButtonContent::Child(
                                                                Box::new(wire::Node::Container {
                                                                    shadow: wire::Shadow {
                                                                        color: None,
                                                                        x: None,
                                                                        y: None,
                                                                        blur: None,
                                                                    },
                                                                    max_width: None,
                                                                    max_height: None,
                                                                    clip: false,
                                                                    key: format!("{}/@container:293", use_scope),
                                                                    width: Some(wire::Length::Fill),
                                                                    height: Some(wire::Length::Fill),
                                                                    padding: None,
                                                                    align_x: Some(wire::AlignX::Center),
                                                                    align_y: Some(wire::AlignY::Center),
                                                                    background: (None).map(wire::Background::Color),
                                                                    border: None,
                                                                    snap: None,
                                                                    content: Box::new(
                                                                        self
                                                                            .icon(
                                                                                palette,
                                                                                format!("{}/Icon@920", use_scope),
                                                                                "link",
                                                                                14.,
                                                                            ),
                                                                    ),
                                                                }),
                                                            ),
                                                            label: Some(String::from("Copy page link".to_owned())),
                                                            on_press: if (((!(self.host_error).is_empty())
                                                                || (self.active_page).is_empty()))
                                                            {
                                                                None
                                                            } else {
                                                                Some(
                                                                    ::ducktape_view_guest::slots::message(
                                                                        (move |event_0, event_1| Message::CopyToClipboard(
                                                                            event_0,
                                                                            event_1,
                                                                        ))(self.page_link.to_owned(), "Page link copied".to_owned()),
                                                                    ),
                                                                )
                                                            },
                                                            width: Some(wire::Length::Fixed((28.0) as f32)),
                                                            height: Some(wire::Length::Fixed((28.0) as f32)),
                                                            padding: Some(wire::Edges::all((0.0) as f32)),
                                                            style: wire::ButtonStyle {
                                                                preset: wire::ButtonPreset::Primary,
                                                                recipe: Some(wire::ButtonRecipe {
                                                                    base: wire::Face {
                                                                        background: Some(
                                                                            wire::Rgba([
                                                                                0.0 / 255.0,
                                                                                0.0 / 255.0,
                                                                                0.0 / 255.0,
                                                                                0.000000,
                                                                            ]),
                                                                        ),
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
                                                                    background: Some(
                                                                        wire::Rgba([
                                                                            0.0 / 255.0,
                                                                            0.0 / 255.0,
                                                                            0.0 / 255.0,
                                                                            0.000000,
                                                                        ]),
                                                                    ),
                                                                    text: Some(palette.colors[5]),
                                                                    border: Some(wire::Border {
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
                                                                hovered: Some(wire::Face {
                                                                    background: Some({
                                                                        let mut color = palette.colors[4];
                                                                        color.0[3] = 0.100000;
                                                                        color
                                                                    }),
                                                                    text: Some(palette.colors[4]),
                                                                    border: None,
                                                                }),
                                                                pressed: Some(wire::Face {
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
                                                    children
                                                        .push({
                                                            let node_scope = format!("{}/page-menu-trigger", use_scope);
                                                            wire::Node::Button {
                                                                checked: None,
                                                                expanded: Some(self.page_menu_open),
                                                                description: None,
                                                                key: node_scope.clone(),
                                                                content: wire::ButtonContent::Child(
                                                                    Box::new(wire::Node::Container {
                                                                        shadow: wire::Shadow {
                                                                            color: None,
                                                                            x: None,
                                                                            y: None,
                                                                            blur: None,
                                                                        },
                                                                        max_width: None,
                                                                        max_height: None,
                                                                        clip: false,
                                                                        key: format!("{}/@container:321", use_scope),
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
                                                                                wrapping: None,
                                                                                tracking: 0.0f32,
                                                                                font: Some(wire::NamedFont {
                                                                                    family: wire::FontFamily::Named("Geist".into()),
                                                                                    weight: wire::Weight::Normal,
                                                                                    stretch: wire::FontStretch::Normal,
                                                                                    style: wire::FontStyle::Normal,
                                                                                }),
                                                                            },
                                                                            key: format!("{}/@text:327", use_scope),
                                                                            size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                            color: None,
                                                                            font: wire::Font {
                                                                                monospace: false,
                                                                                weight: wire::Weight::Normal,
                                                                            },
                                                                            width: None,
                                                                            align_x: None,
                                                                            content: "•••".to_owned(),
                                                                        }),
                                                                    }),
                                                                ),
                                                                label: Some(String::from("Page actions".to_owned())),
                                                                on_press: if (((self.busy
                                                                    || (!(self.host_error).is_empty()))
                                                                    || self.page_delete_armed))
                                                                {
                                                                    None
                                                                } else {
                                                                    Some(
                                                                        ::ducktape_view_guest::slots::message(
                                                                            Message::TogglePageMenu,
                                                                        ),
                                                                    )
                                                                },
                                                                width: Some(wire::Length::Fixed((28.0) as f32)),
                                                                height: Some(wire::Length::Fixed((28.0) as f32)),
                                                                padding: Some(wire::Edges::all((0.0) as f32)),
                                                                style: wire::ButtonStyle {
                                                                    preset: wire::ButtonPreset::Primary,
                                                                    recipe: Some(wire::ButtonRecipe {
                                                                        base: wire::Face {
                                                                            background: Some(
                                                                                wire::Rgba([
                                                                                    0.0 / 255.0,
                                                                                    0.0 / 255.0,
                                                                                    0.0 / 255.0,
                                                                                    0.000000,
                                                                                ]),
                                                                            ),
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
                                                                        background: Some(
                                                                            wire::Rgba([
                                                                                0.0 / 255.0,
                                                                                0.0 / 255.0,
                                                                                0.0 / 255.0,
                                                                                0.000000,
                                                                            ]),
                                                                        ),
                                                                        text: Some(palette.colors[5]),
                                                                        border: Some(wire::Border {
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
                                                                    hovered: Some(wire::Face {
                                                                        background: Some({
                                                                            let mut color = palette.colors[4];
                                                                            color.0[3] = 0.100000;
                                                                            color
                                                                        }),
                                                                        text: Some(palette.colors[4]),
                                                                        border: None,
                                                                    }),
                                                                    pressed: Some(wire::Face {
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
                                                            }
                                                        });
                                                    if (self.autosave == "saving") {
                                                        children
                                                            .push(wire::Node::Container {
                                                                shadow: wire::Shadow {
                                                                    color: None,
                                                                    x: None,
                                                                    y: None,
                                                                    blur: None,
                                                                },
                                                                max_width: None,
                                                                max_height: None,
                                                                clip: false,
                                                                key: format!("{}/@container:337", use_scope),
                                                                width: None,
                                                                height: None,
                                                                padding: Some(wire::Edges {
                                                                    top: (4.0) as f32,
                                                                    right: (9.0) as f32,
                                                                    bottom: (4.0) as f32,
                                                                    left: (9.0) as f32,
                                                                }),
                                                                align_x: None,
                                                                align_y: None,
                                                                background: (Some(palette.colors[32]))
                                                                    .map(wire::Background::Color),
                                                                border: Some(wire::Border {
                                                                    color: Some(palette.colors[33]),
                                                                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                    radius: Some([
                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
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
                                                                            family: wire::FontFamily::Named("Geist Mono".into()),
                                                                            weight: wire::Weight::Medium,
                                                                            stretch: wire::FontStretch::Normal,
                                                                            style: wire::FontStyle::Normal,
                                                                        }),
                                                                    },
                                                                    key: format!("{}/@text:345", use_scope),
                                                                    size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                    color: Some(palette.colors[30]),
                                                                    font: wire::Font {
                                                                        monospace: false,
                                                                        weight: wire::Weight::Normal,
                                                                    },
                                                                    width: None,
                                                                    align_x: None,
                                                                    content: "saving…".to_owned(),
                                                                }),
                                                            });
                                                    }
                                                    if (self.autosave == "error") {
                                                        children
                                                            .push(wire::Node::Container {
                                                                shadow: wire::Shadow {
                                                                    color: None,
                                                                    x: None,
                                                                    y: None,
                                                                    blur: None,
                                                                },
                                                                max_width: None,
                                                                max_height: None,
                                                                clip: false,
                                                                key: format!("{}/@container:352", use_scope),
                                                                width: None,
                                                                height: None,
                                                                padding: Some(wire::Edges {
                                                                    top: (4.0) as f32,
                                                                    right: (9.0) as f32,
                                                                    bottom: (4.0) as f32,
                                                                    left: (9.0) as f32,
                                                                }),
                                                                align_x: None,
                                                                align_y: None,
                                                                background: (Some(palette.colors[22]))
                                                                    .map(wire::Background::Color),
                                                                border: Some(wire::Border {
                                                                    color: Some(palette.colors[23]),
                                                                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                    radius: Some([
                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
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
                                                                            family: wire::FontFamily::Named("Geist Mono".into()),
                                                                            weight: wire::Weight::Medium,
                                                                            stretch: wire::FontStretch::Normal,
                                                                            style: wire::FontStyle::Normal,
                                                                        }),
                                                                    },
                                                                    key: format!("{}/@text:360", use_scope),
                                                                    size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                    color: Some(palette.colors[20]),
                                                                    font: wire::Font {
                                                                        monospace: false,
                                                                        weight: wire::Weight::Normal,
                                                                    },
                                                                    width: None,
                                                                    align_x: None,
                                                                    content: "not saved".to_owned(),
                                                                }),
                                                            });
                                                    }
                                                    if (self.autosave == "saved") {
                                                        children
                                                            .push(wire::Node::Container {
                                                                shadow: wire::Shadow {
                                                                    color: None,
                                                                    x: None,
                                                                    y: None,
                                                                    blur: None,
                                                                },
                                                                max_width: None,
                                                                max_height: None,
                                                                clip: false,
                                                                key: format!("{}/@container:367", use_scope),
                                                                width: None,
                                                                height: None,
                                                                padding: Some(wire::Edges {
                                                                    top: (4.0) as f32,
                                                                    right: (9.0) as f32,
                                                                    bottom: (4.0) as f32,
                                                                    left: (9.0) as f32,
                                                                }),
                                                                align_x: None,
                                                                align_y: None,
                                                                background: (Some(palette.colors[106]))
                                                                    .map(wire::Background::Color),
                                                                border: Some(wire::Border {
                                                                    color: Some(palette.colors[107]),
                                                                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                    radius: Some([
                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
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
                                                                            family: wire::FontFamily::Named("Geist Mono".into()),
                                                                            weight: wire::Weight::Medium,
                                                                            stretch: wire::FontStretch::Normal,
                                                                            style: wire::FontStyle::Normal,
                                                                        }),
                                                                    },
                                                                    key: format!("{}/@text:375", use_scope),
                                                                    size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                    color: Some(palette.colors[75]),
                                                                    font: wire::Font {
                                                                        monospace: false,
                                                                        weight: wire::Weight::Normal,
                                                                    },
                                                                    width: None,
                                                                    align_x: None,
                                                                    content: "✓ synced".to_owned(),
                                                                }),
                                                            });
                                                    }
                                                    wire::Node::Linear {
                                                        max_width: None,
                                                        clip: false,
                                                        key: format!("{}/@layout:170", use_scope),
                                                        wrap: None,
                                                        axis: wire::Axis::Row,
                                                        spacing: Some((9.0) as f32),
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
                                        children
                                            .push(wire::Node::Container {
                                                shadow: wire::Shadow {
                                                    color: None,
                                                    x: None,
                                                    y: None,
                                                    blur: None,
                                                },
                                                max_width: None,
                                                max_height: None,
                                                clip: false,
                                                key: format!("{}/@container:381", use_scope),
                                                width: Some(wire::Length::Fill),
                                                height: Some(wire::Length::Fixed((1.0) as f32)),
                                                padding: None,
                                                align_x: None,
                                                align_y: None,
                                                background: (Some(palette.colors[60]))
                                                    .map(wire::Background::Color),
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
                                            key: format!("{}/@layout:163", use_scope),
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
                                    });
                            }
                            children
                                .push({
                                    let mut children: Vec<wire::Node> = Vec::new();
                                    children
                                        .push({
                                            let node_scope = format!("{}/pane-measure", use_scope);
                                            wire::Node::Sensor {
                                                key: node_scope.clone(),
                                                reset: None,
                                                on_show: Some(
                                                    ::ducktape_view_guest::slots::handler::<
                                                        (f32, f32),
                                                        Message,
                                                    >(
                                                        Box::new({
                                                            let route = {
                                                                let route_callback = (move |event_0, event_1| Message::PagesPaneResized(
                                                                    event_0,
                                                                    event_1,
                                                                ))
                                                                    .clone();
                                                                move |size: (f64, f64)| (route_callback)(size.0, size.1)
                                                            };
                                                            move |sent: (f32, f32)| Some(
                                                                route((f64::from(sent.0), f64::from(sent.1))),
                                                            )
                                                        }),
                                                    ),
                                                ),
                                                on_resize: Some(
                                                    ::ducktape_view_guest::slots::handler::<
                                                        (f32, f32),
                                                        Message,
                                                    >(
                                                        Box::new({
                                                            let route = {
                                                                let route_callback = (move |event_0, event_1| Message::PagesPaneResized(
                                                                    event_0,
                                                                    event_1,
                                                                ))
                                                                    .clone();
                                                                move |size: (f64, f64)| (route_callback)(size.0, size.1)
                                                            };
                                                            move |sent: (f32, f32)| Some(
                                                                route((f64::from(sent.0), f64::from(sent.1))),
                                                            )
                                                        }),
                                                    ),
                                                ),
                                                on_hide: None,
                                                anticipate: None,
                                                delay: None,
                                                child: Box::new(wire::Node::Space {
                                                    width: Some(wire::Length::Fill),
                                                    height: Some(wire::Length::Fill),
                                                }),
                                            }
                                        });
                                    if (!self.connected) {
                                        if (self.host_error).is_empty() {
                                            children
                                                .push(
                                                    self
                                                        .connection_empty(
                                                            palette,
                                                            format!("{}/EmptyState@1021", use_scope),
                                                        ),
                                                );
                                        }
                                    }
                                    if ((((self.host_error).is_empty() && self.connected)
                                        && self.loading) && (self.active_page).is_empty())
                                    {
                                        children
                                            .push(
                                                self
                                                    .loading_empty(
                                                        palette,
                                                        format!("{}/EmptyState@1026", use_scope),
                                                    ),
                                            );
                                    }
                                    if ((((self.host_error).is_empty() && self.connected)
                                        && (!self.loading)) && (self.active_page).is_empty())
                                    {
                                        children
                                            .push(
                                                self
                                                    .selection_empty(
                                                        palette,
                                                        format!("{}/EmptyState@1028", use_scope),
                                                    ),
                                            );
                                    }
                                    if (self.connected && (!(self.active_page).is_empty())) {
                                        children
                                            .push(wire::Node::Container {
                                                shadow: wire::Shadow {
                                                    color: None,
                                                    x: None,
                                                    y: None,
                                                    blur: None,
                                                },
                                                max_width: Some(
                                                    ((crate::host::document_width(
                                                        self.pages_pane_width,
                                                        self.block_comments_open,
                                                    )) as f32)
                                                        .max(0.0)
                                                        .min(f32::MAX),
                                                ),
                                                max_height: None,
                                                clip: false,
                                                key: format!("{}/@container:436", use_scope),
                                                width: Some(wire::Length::Fill),
                                                height: Some(wire::Length::Fill),
                                                padding: Some(wire::Edges {
                                                    top: (26.0) as f32,
                                                    right: (40.0) as f32,
                                                    bottom: (18.0) as f32,
                                                    left: (22.0) as f32,
                                                }),
                                                align_x: None,
                                                align_y: None,
                                                background: (None).map(wire::Background::Color),
                                                border: None,
                                                snap: None,
                                                content: Box::new({
                                                    let mut children: Vec<wire::Node> = Vec::new();
                                                    if (!(self.page_refusal).is_empty()) {
                                                        children
                                                            .push(wire::Node::Container {
                                                                shadow: wire::Shadow {
                                                                    color: None,
                                                                    x: None,
                                                                    y: None,
                                                                    blur: None,
                                                                },
                                                                max_width: None,
                                                                max_height: None,
                                                                clip: false,
                                                                key: format!("{}/@container:455", use_scope),
                                                                width: Some(wire::Length::Fill),
                                                                height: None,
                                                                padding: Some(wire::Edges {
                                                                    top: (9.0) as f32,
                                                                    right: (12.0) as f32,
                                                                    bottom: (9.0) as f32,
                                                                    left: (12.0) as f32,
                                                                }),
                                                                align_x: None,
                                                                align_y: None,
                                                                background: (Some(palette.colors[81]))
                                                                    .map(wire::Background::Color),
                                                                border: Some(wire::Border {
                                                                    color: Some(palette.colors[82]),
                                                                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                    radius: Some([
                                                                        ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                        ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                        ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                        ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                    ]),
                                                                }),
                                                                snap: None,
                                                                content: Box::new(wire::Node::Text {
                                                                    options: wire::TextOptions {
                                                                        height: None,
                                                                        align_y: None,
                                                                        line_height: None,
                                                                        shaping: None,
                                                                        wrapping: Some(wire::Wrapping::Word),
                                                                        tracking: 0.0f32,
                                                                        font: Some(wire::NamedFont {
                                                                            family: wire::FontFamily::Named("Geist".into()),
                                                                            weight: wire::Weight::Normal,
                                                                            stretch: wire::FontStretch::Normal,
                                                                            style: wire::FontStyle::Normal,
                                                                        }),
                                                                    },
                                                                    key: format!("{}/@text:464", use_scope),
                                                                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                    color: Some(palette.colors[80]),
                                                                    font: wire::Font {
                                                                        monospace: false,
                                                                        weight: wire::Weight::Normal,
                                                                    },
                                                                    width: Some(wire::Length::Fill),
                                                                    align_x: None,
                                                                    content: self.page_refusal.to_owned(),
                                                                }),
                                                            });
                                                    }
                                                    if (!(self.orphaned_comment_drafts).is_empty()) {
                                                        children
                                                            .push(wire::Node::Container {
                                                                shadow: wire::Shadow {
                                                                    color: None,
                                                                    x: None,
                                                                    y: None,
                                                                    blur: None,
                                                                },
                                                                max_width: None,
                                                                max_height: None,
                                                                clip: false,
                                                                key: format!("{}/@container:471", use_scope),
                                                                width: Some(wire::Length::Fill),
                                                                height: None,
                                                                padding: Some(wire::Edges {
                                                                    top: (7.0) as f32,
                                                                    right: (7.0) as f32,
                                                                    bottom: (7.0) as f32,
                                                                    left: (7.0) as f32,
                                                                }),
                                                                align_x: None,
                                                                align_y: None,
                                                                background: (Some(palette.colors[55]))
                                                                    .map(wire::Background::Color),
                                                                border: Some(wire::Border {
                                                                    color: Some({
                                                                        let mut color = palette.colors[4];
                                                                        color.0[3] = 0.090000;
                                                                        color
                                                                    }),
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
                                                                            key: format!("{}/@text:480", use_scope),
                                                                            size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                            color: Some(palette.colors[4]),
                                                                            font: wire::Font {
                                                                                monospace: false,
                                                                                weight: wire::Weight::Normal,
                                                                            },
                                                                            width: None,
                                                                            align_x: None,
                                                                            content: "Recovered drafts".to_owned(),
                                                                        });
                                                                    for (index, recovered_comment) in self
                                                                        .orphaned_comment_drafts
                                                                        .iter()
                                                                        .enumerate()
                                                                    {
                                                                        let for_scope = format!(
                                                                            "{}/@for:1106({})", use_scope, index
                                                                        );
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
                                                                                                weight: wire::Weight::Normal,
                                                                                                stretch: wire::FontStretch::Normal,
                                                                                                style: wire::FontStyle::Normal,
                                                                                            }),
                                                                                        },
                                                                                        key: format!("{}/@text:491", for_scope),
                                                                                        size: Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                        color: Some(palette.colors[5]),
                                                                                        font: wire::Font {
                                                                                            monospace: false,
                                                                                            weight: wire::Weight::Normal,
                                                                                        },
                                                                                        width: Some(wire::Length::Fill),
                                                                                        align_x: None,
                                                                                        content: recovered_comment.to_owned(),
                                                                                    });
                                                                                children
                                                                                    .push(wire::Node::Button {
                                                                                        checked: None,
                                                                                        expanded: None,
                                                                                        description: None,
                                                                                        key: format!("{}/@button:496", for_scope),
                                                                                        content: wire::ButtonContent::Label(String::from("Use")),
                                                                                        label: Some(String::from("Use as comment".to_owned())),
                                                                                        on_press: if ((self.loading
                                                                                            || (self.busy || (!(self.host_error).is_empty()))))
                                                                                        {
                                                                                            None
                                                                                        } else {
                                                                                            Some(
                                                                                                ::ducktape_view_guest::slots::message(
                                                                                                    (move |event_0| Message::UseOrphanedCommentDraft(
                                                                                                        event_0,
                                                                                                    ))(recovered_comment.to_owned()),
                                                                                                ),
                                                                                            )
                                                                                        },
                                                                                        width: None,
                                                                                        height: None,
                                                                                        padding: Some(wire::Edges::all((5.0) as f32)),
                                                                                        style: wire::ButtonStyle {
                                                                                            preset: wire::ButtonPreset::Primary,
                                                                                            recipe: Some(wire::ButtonRecipe {
                                                                                                base: wire::Face {
                                                                                                    background: Some(
                                                                                                        wire::Rgba([
                                                                                                            0.0 / 255.0,
                                                                                                            0.0 / 255.0,
                                                                                                            0.0 / 255.0,
                                                                                                            0.000000,
                                                                                                        ]),
                                                                                                    ),
                                                                                                    text: Some(palette.colors[4]),
                                                                                                    border: Some(wire::Border {
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
                                                                                                font: Some(wire::NamedFont {
                                                                                                    family: wire::FontFamily::Named("Geist".into()),
                                                                                                    weight: wire::Weight::Semibold,
                                                                                                    stretch: wire::FontStretch::Normal,
                                                                                                    style: wire::FontStyle::Normal,
                                                                                                }),
                                                                                            }),
                                                                                            active: wire::Face {
                                                                                                background: Some({
                                                                                                    let mut color = palette.colors[4];
                                                                                                    color.0[3] = 0.090000;
                                                                                                    color
                                                                                                }),
                                                                                                text: Some(palette.colors[4]),
                                                                                                border: Some(wire::Border {
                                                                                                    color: Some({
                                                                                                        let mut color = palette.colors[4];
                                                                                                        color.0[3] = 0.120000;
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
                                                                                            },
                                                                                            hovered: Some(wire::Face {
                                                                                                background: Some({
                                                                                                    let mut color = palette.colors[4];
                                                                                                    color.0[3] = 0.140000;
                                                                                                    color
                                                                                                }),
                                                                                                text: None,
                                                                                                border: None,
                                                                                            }),
                                                                                            pressed: Some(wire::Face {
                                                                                                background: Some({
                                                                                                    let mut color = palette.colors[4];
                                                                                                    color.0[3] = 0.180000;
                                                                                                    color
                                                                                                }),
                                                                                                text: None,
                                                                                                border: None,
                                                                                            }),
                                                                                            disabled: None,
                                                                                        },
                                                                                    });
                                                                                children
                                                                                    .push(wire::Node::Button {
                                                                                        checked: None,
                                                                                        expanded: None,
                                                                                        description: None,
                                                                                        key: format!("{}/@button:505", for_scope),
                                                                                        content: wire::ButtonContent::Label(
                                                                                            String::from("Discard"),
                                                                                        ),
                                                                                        label: None,
                                                                                        on_press: if ((self.loading
                                                                                            || (self.busy || (!(self.host_error).is_empty()))))
                                                                                        {
                                                                                            None
                                                                                        } else {
                                                                                            Some(
                                                                                                ::ducktape_view_guest::slots::message(
                                                                                                    (move |event_0| Message::DiscardOrphanedCommentDraft(
                                                                                                        event_0,
                                                                                                    ))(recovered_comment.to_owned()),
                                                                                                ),
                                                                                            )
                                                                                        },
                                                                                        width: None,
                                                                                        height: None,
                                                                                        padding: Some(wire::Edges::all((5.0) as f32)),
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
                                                                                    key: format!("{}/@layout:486", for_scope),
                                                                                    wrap: None,
                                                                                    axis: wire::Axis::Row,
                                                                                    spacing: Some((5.0) as f32),
                                                                                    padding: None,
                                                                                    width: Some(wire::Length::Fill),
                                                                                    height: None,
                                                                                    align: Some(wire::AlignX::Center),
                                                                                    background: None,
                                                                                    border: None,
                                                                                    children,
                                                                                }
                                                                            });
                                                                    }
                                                                    wire::Node::Linear {
                                                                        max_width: None,
                                                                        clip: false,
                                                                        key: format!("{}/@layout:479", use_scope),
                                                                        wrap: None,
                                                                        axis: wire::Axis::Column,
                                                                        spacing: Some((5.0) as f32),
                                                                        padding: None,
                                                                        width: Some(wire::Length::Fill),
                                                                        height: None,
                                                                        align: None,
                                                                        background: None,
                                                                        border: None,
                                                                        children,
                                                                    }
                                                                }),
                                                            });
                                                    }
                                                    children
                                                        .push({
                                                            let mut children: Vec<wire::Node> = Vec::new();
                                                            if (!(crate::editor_view::presentation_notice(
                                                                &(self.document),
                                                                &(self.document_paint),
                                                            ))
                                                                .is_empty())
                                                            {
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
                                                                                weight: wire::Weight::Normal,
                                                                                stretch: wire::FontStretch::Normal,
                                                                                style: wire::FontStyle::Normal,
                                                                            }),
                                                                        },
                                                                        key: format!("{}/@text:801", use_scope),
                                                                        size: Some(13.5f32),
                                                                        color: Some(palette.colors[5]),
                                                                        font: wire::Font {
                                                                            monospace: false,
                                                                            weight: wire::Weight::Normal,
                                                                        },
                                                                        width: None,
                                                                        align_x: None,
                                                                        content: (crate::editor_view::presentation_notice(
                                                                            &(self.document),
                                                                            &(self.document_paint),
                                                                        ))
                                                                            .to_string(),
                                                                    });
                                                            }
                                                            if (!(self.document_error).is_empty()) {
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
                                                                                weight: wire::Weight::Normal,
                                                                                stretch: wire::FontStretch::Normal,
                                                                                style: wire::FontStyle::Normal,
                                                                            }),
                                                                        },
                                                                        key: format!("{}/@text:803", use_scope),
                                                                        size: Some(13.5f32),
                                                                        color: Some(palette.colors[20]),
                                                                        font: wire::Font {
                                                                            monospace: false,
                                                                            weight: wire::Weight::Normal,
                                                                        },
                                                                        width: None,
                                                                        align_x: None,
                                                                        content: self.document_error.to_owned(),
                                                                    });
                                                            }
                                                            children
                                                                .push({
                                                                    let node_scope = format!("{}/document", use_scope);
                                                                    {
                                                                        let editor = &(self.document);
                                                                        let (document, on_document) = editor
                                                                            .document(
                                                                                ("app:document").to_owned(),
                                                                                Message::DocumentUpdated
                                                                                    as fn(
                                                                                        ::ducktape_view_guest::EditorDocumentUpdate,
                                                                                    ) -> Message,
                                                                            );
                                                                        wire::Node::Editor {
                                                                            options: Box::new(wire::EditorOptions {
                                                                                binding: Some(
                                                                                    Box::new(
                                                                                        crate::editor_binding::keys(
                                                                                                self.document_history.clone(),
                                                                                                self.document_menu.clone(),
                                                                                            )
                                                                                            .register(
                                                                                                move |value| Message::DocumentCommitted(value),
                                                                                                Message::DocumentTransaction,
                                                                                            ),
                                                                                    ),
                                                                                ),
                                                                                presentation: {
                                                                                    let presentation = crate::editor_view::paint(
                                                                                        editor.state_view(),
                                                                                        self.document_paint.clone(),
                                                                                    );
                                                                                    presentation
                                                                                        .validate(editor.state_view().text)
                                                                                        .expect("invalid editor presentation");
                                                                                    Some(Box::new(presentation))
                                                                                },
                                                                                size: Some((14.0) as f32),
                                                                                padding: None,
                                                                                line_height: Some(
                                                                                    wire::LineHeight::Relative(
                                                                                        ((1.65) as f32).max(f32::EPSILON).min(f32::MAX),
                                                                                    ),
                                                                                ),
                                                                                wrapping: Some(wire::Wrapping::Word),
                                                                                font: Some(wire::NamedFont {
                                                                                    family: wire::FontFamily::Named("Geist".into()),
                                                                                    weight: wire::Weight::Normal,
                                                                                    stretch: wire::FontStretch::Normal,
                                                                                    style: wire::FontStyle::Normal,
                                                                                }),
                                                                                style: wire::InputStyle {
                                                                                    active: wire::InputFace {
                                                                                        icon: None,
                                                                                        background: Some({
                                                                                            let mut color = palette.colors[4];
                                                                                            color.0[3] = 0.000000;
                                                                                            color
                                                                                        }),
                                                                                        border: Some(wire::Border {
                                                                                            color: None,
                                                                                            width: Some(((0.0) as f32).max(0.0).min(f32::MAX)),
                                                                                            radius: None,
                                                                                        }),
                                                                                        value: Some(palette.colors[0]),
                                                                                        placeholder: Some(palette.colors[72]),
                                                                                        selection: Some(palette.colors[1]),
                                                                                    },
                                                                                    hovered: None,
                                                                                    focused: None,
                                                                                    focused_hovered: None,
                                                                                    disabled: Some(wire::InputFace {
                                                                                        icon: None,
                                                                                        background: Some({
                                                                                            let mut color = palette.colors[4];
                                                                                            color.0[3] = 0.000000;
                                                                                            color
                                                                                        }),
                                                                                        border: Some(wire::Border {
                                                                                            color: None,
                                                                                            width: Some(((0.0) as f32).max(0.0).min(f32::MAX)),
                                                                                            radius: None,
                                                                                        }),
                                                                                        value: Some(palette.colors[0]),
                                                                                        placeholder: Some(palette.colors[72]),
                                                                                        selection: Some(palette.colors[1]),
                                                                                    }),
                                                                                    ..::std::default::Default::default()
                                                                                },
                                                                            }),
                                                                            key: node_scope.clone(),
                                                                            placeholder: "Write something… `#` for a heading, `-` for a list"
                                                                                .to_owned(),
                                                                            document,
                                                                            on_document,
                                                                            editable: !((((((!(self.host_error).is_empty())
                                                                                || self.loading) || (!self.connected))
                                                                                || (self.active_page).is_empty())
                                                                                || (self.active_page != self.buffer_page))),
                                                                            width: None,
                                                                            height: None,
                                                                            min_height: None,
                                                                            max_height: None,
                                                                        }
                                                                    }
                                                                });
                                                            wire::Node::Linear {
                                                                max_width: None,
                                                                clip: false,
                                                                key: format!("{}/@layout:799", use_scope),
                                                                wrap: None,
                                                                axis: wire::Axis::Column,
                                                                spacing: None,
                                                                padding: None,
                                                                width: Some(wire::Length::Fill),
                                                                height: Some(wire::Length::Fill),
                                                                align: None,
                                                                background: None,
                                                                border: None,
                                                                children,
                                                            }
                                                        });
                                                    if (!(self.subpages).is_empty()) {
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
                                                                            wrapping: Some(wire::Wrapping::None),
                                                                            tracking: 0.0f32,
                                                                            font: Some(wire::NamedFont {
                                                                                family: wire::FontFamily::Named("Geist Mono".into()),
                                                                                weight: wire::Weight::Medium,
                                                                                stretch: wire::FontStretch::Normal,
                                                                                style: wire::FontStyle::Normal,
                                                                            }),
                                                                        },
                                                                        key: format!("{}/@text:525", use_scope),
                                                                        size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                        color: Some(palette.colors[72]),
                                                                        font: wire::Font {
                                                                            monospace: false,
                                                                            weight: wire::Weight::Normal,
                                                                        },
                                                                        width: None,
                                                                        align_x: None,
                                                                        content: "Subpages".to_owned(),
                                                                    });
                                                                for (index, child) in self.subpages.iter().enumerate() {
                                                                    let for_scope = format!(
                                                                        "{}/@for:1152({})", use_scope, index
                                                                    );
                                                                    children
                                                                        .push(wire::Node::Button {
                                                                            checked: None,
                                                                            expanded: None,
                                                                            description: Some(String::from(child.title.to_owned())),
                                                                            key: format!("{}/@button:532", for_scope),
                                                                            content: wire::ButtonContent::Child(
                                                                                Box::new({
                                                                                    let mut children: Vec<wire::Node> = Vec::new();
                                                                                    children
                                                                                        .push(
                                                                                            self
                                                                                                .icon(
                                                                                                    palette,
                                                                                                    format!("{}/Icon@1166", for_scope),
                                                                                                    "doc",
                                                                                                    14.,
                                                                                                ),
                                                                                        );
                                                                                    if (child.title).is_empty() {
                                                                                        children
                                                                                            .push(wire::Node::Text {
                                                                                                options: wire::TextOptions {
                                                                                                    height: None,
                                                                                                    align_y: None,
                                                                                                    line_height: None,
                                                                                                    shaping: None,
                                                                                                    wrapping: Some(wire::Wrapping::None),
                                                                                                    tracking: 0.0f32,
                                                                                                    font: Some(wire::NamedFont {
                                                                                                        family: wire::FontFamily::Named("Geist".into()),
                                                                                                        weight: wire::Weight::Medium,
                                                                                                        stretch: wire::FontStretch::Normal,
                                                                                                        style: wire::FontStyle::Normal,
                                                                                                    }),
                                                                                                },
                                                                                                key: format!("{}/@text:551", for_scope),
                                                                                                size: Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                color: Some(palette.colors[5]),
                                                                                                font: wire::Font {
                                                                                                    monospace: false,
                                                                                                    weight: wire::Weight::Normal,
                                                                                                },
                                                                                                width: Some(wire::Length::Fill),
                                                                                                align_x: None,
                                                                                                content: "Untitled".to_owned(),
                                                                                            });
                                                                                    }
                                                                                    if (!(child.title).is_empty()) {
                                                                                        children
                                                                                            .push(wire::Node::Text {
                                                                                                options: wire::TextOptions {
                                                                                                    height: None,
                                                                                                    align_y: None,
                                                                                                    line_height: None,
                                                                                                    shaping: None,
                                                                                                    wrapping: Some(wire::Wrapping::None),
                                                                                                    tracking: 0.0f32,
                                                                                                    font: Some(wire::NamedFont {
                                                                                                        family: wire::FontFamily::Named("Geist".into()),
                                                                                                        weight: wire::Weight::Medium,
                                                                                                        stretch: wire::FontStretch::Normal,
                                                                                                        style: wire::FontStyle::Normal,
                                                                                                    }),
                                                                                                },
                                                                                                key: format!("{}/@text:559", for_scope),
                                                                                                size: Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                color: Some(palette.colors[4]),
                                                                                                font: wire::Font {
                                                                                                    monospace: false,
                                                                                                    weight: wire::Weight::Normal,
                                                                                                },
                                                                                                width: Some(wire::Length::Fill),
                                                                                                align_x: None,
                                                                                                content: child.title.to_owned(),
                                                                                            });
                                                                                    }
                                                                                    children
                                                                                        .push(wire::Node::Text {
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
                                                                                            key: format!("{}/@text:566", for_scope),
                                                                                            size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                            color: Some(palette.colors[73]),
                                                                                            font: wire::Font {
                                                                                                monospace: false,
                                                                                                weight: wire::Weight::Normal,
                                                                                            },
                                                                                            width: None,
                                                                                            align_x: None,
                                                                                            content: "›".to_owned(),
                                                                                        });
                                                                                    wire::Node::Linear {
                                                                                        max_width: None,
                                                                                        clip: false,
                                                                                        key: format!("{}/@layout:540", for_scope),
                                                                                        wrap: None,
                                                                                        axis: wire::Axis::Row,
                                                                                        spacing: Some((8.0) as f32),
                                                                                        padding: None,
                                                                                        width: Some(wire::Length::Fill),
                                                                                        height: None,
                                                                                        align: Some(wire::AlignX::Center),
                                                                                        background: None,
                                                                                        border: None,
                                                                                        children,
                                                                                    }
                                                                                }),
                                                                            ),
                                                                            label: Some(String::from("Open subpage".to_owned())),
                                                                            on_press: if ((!(self.host_error).is_empty())) {
                                                                                None
                                                                            } else {
                                                                                Some(
                                                                                    ::ducktape_view_guest::slots::message(
                                                                                        (move |event_0| Message::ChoosePage(
                                                                                            event_0,
                                                                                        ))(child.id.to_owned()),
                                                                                    ),
                                                                                )
                                                                            },
                                                                            width: Some(wire::Length::Fill),
                                                                            height: None,
                                                                            padding: Some(wire::Edges::all((6.0) as f32)),
                                                                            style: wire::ButtonStyle {
                                                                                preset: wire::ButtonPreset::Primary,
                                                                                recipe: Some(wire::ButtonRecipe {
                                                                                    base: wire::Face {
                                                                                        background: Some(
                                                                                            wire::Rgba([
                                                                                                0.0 / 255.0,
                                                                                                0.0 / 255.0,
                                                                                                0.0 / 255.0,
                                                                                                0.000000,
                                                                                            ]),
                                                                                        ),
                                                                                        text: Some(palette.colors[4]),
                                                                                        border: Some(wire::Border {
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
                                                                                    font: Some(wire::NamedFont {
                                                                                        family: wire::FontFamily::Named("Geist".into()),
                                                                                        weight: wire::Weight::Semibold,
                                                                                        stretch: wire::FontStretch::Normal,
                                                                                        style: wire::FontStyle::Normal,
                                                                                    }),
                                                                                }),
                                                                                active: wire::Face {
                                                                                    background: Some(
                                                                                        wire::Rgba([
                                                                                            0.0 / 255.0,
                                                                                            0.0 / 255.0,
                                                                                            0.0 / 255.0,
                                                                                            0.000000,
                                                                                        ]),
                                                                                    ),
                                                                                    text: Some(palette.colors[4]),
                                                                                    border: Some(wire::Border {
                                                                                        color: Some(
                                                                                            wire::Rgba([
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
                                                                                hovered: Some(wire::Face {
                                                                                    background: Some({
                                                                                        let mut color = palette.colors[4];
                                                                                        color.0[3] = 0.040000;
                                                                                        color
                                                                                    }),
                                                                                    text: Some(palette.colors[4]),
                                                                                    border: Some(wire::Border {
                                                                                        color: Some({
                                                                                            let mut color = palette.colors[4];
                                                                                            color.0[3] = 0.070000;
                                                                                            color
                                                                                        }),
                                                                                        width: None,
                                                                                        radius: None,
                                                                                    }),
                                                                                }),
                                                                                pressed: Some(wire::Face {
                                                                                    background: Some({
                                                                                        let mut color = palette.colors[4];
                                                                                        color.0[3] = 0.080000;
                                                                                        color
                                                                                    }),
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
                                                                    key: format!("{}/@layout:519", use_scope),
                                                                    wrap: None,
                                                                    axis: wire::Axis::Column,
                                                                    spacing: Some((2.0) as f32),
                                                                    padding: Some(wire::Edges {
                                                                        top: (10.0) as f32,
                                                                        right: (0.0) as f32,
                                                                        bottom: (0.0) as f32,
                                                                        left: (46.0) as f32,
                                                                    }),
                                                                    width: Some(wire::Length::Fill),
                                                                    height: None,
                                                                    align: None,
                                                                    background: None,
                                                                    border: None,
                                                                    children,
                                                                }
                                                            });
                                                    }
                                                    wire::Node::Linear {
                                                        max_width: None,
                                                        clip: false,
                                                        key: format!("{}/@layout:445", use_scope),
                                                        wrap: None,
                                                        axis: wire::Axis::Column,
                                                        spacing: Some((8.0) as f32),
                                                        padding: None,
                                                        width: Some(wire::Length::Fill),
                                                        height: Some(wire::Length::Fill),
                                                        align: None,
                                                        background: None,
                                                        border: None,
                                                        children,
                                                    }
                                                }),
                                            });
                                    }
                                    if (self.connected && (!(self.page_search_hits).is_empty()))
                                    {
                                        children
                                            .push(wire::Node::Container {
                                                shadow: wire::Shadow {
                                                    color: None,
                                                    x: None,
                                                    y: None,
                                                    blur: None,
                                                },
                                                max_width: None,
                                                max_height: None,
                                                clip: false,
                                                key: format!("{}/@container:589", use_scope),
                                                width: Some(wire::Length::Fill),
                                                height: Some(wire::Length::Fill),
                                                padding: Some(wire::Edges {
                                                    top: (26.0) as f32,
                                                    right: (40.0) as f32,
                                                    bottom: (0.0) as f32,
                                                    left: (22.0) as f32,
                                                }),
                                                align_x: None,
                                                align_y: Some(wire::AlignY::Top),
                                                background: (None).map(wire::Background::Color),
                                                border: None,
                                                snap: None,
                                                content: Box::new(wire::Node::Container {
                                                    shadow: wire::Shadow {
                                                        color: None,
                                                        x: None,
                                                        y: None,
                                                        blur: None,
                                                    },
                                                    max_width: Some(((766.0) as f32).max(0.0).min(f32::MAX)),
                                                    max_height: None,
                                                    clip: false,
                                                    key: format!("{}/@container:597", use_scope),
                                                    width: Some(wire::Length::Fill),
                                                    height: Some(wire::Length::Fixed((148.0) as f32)),
                                                    padding: Some(wire::Edges {
                                                        top: (5.0) as f32,
                                                        right: (5.0) as f32,
                                                        bottom: (5.0) as f32,
                                                        left: (5.0) as f32,
                                                    }),
                                                    align_x: None,
                                                    align_y: None,
                                                    background: (Some(palette.colors[55]))
                                                        .map(wire::Background::Color),
                                                    border: Some(wire::Border {
                                                        color: Some({
                                                            let mut color = palette.colors[4];
                                                            color.0[3] = 0.080000;
                                                            color
                                                        }),
                                                        width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                        radius: Some([
                                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                                        ]),
                                                    }),
                                                    snap: None,
                                                    content: Box::new(wire::Node::Scroll {
                                                        on_scroll: None,
                                                        virtual_rows: false,
                                                        key: format!("{}/@layout:607", use_scope),
                                                        direction: wire::ScrollDirection::Vertical,
                                                        width: Some(wire::Length::Fill),
                                                        height: Some(wire::Length::Fill),
                                                        bar_hidden: false,
                                                        bar_width: None,
                                                        bar_margin: None,
                                                        scroller_width: None,
                                                        bar_spacing: None,
                                                        anchor_x: wire::ScrollAnchor::Start,
                                                        anchor_y: wire::ScrollAnchor::Start,
                                                        auto_scroll: (false),
                                                        background: None,
                                                        border: None,
                                                        content: Box::new({
                                                            let mut children: Vec<wire::Node> = Vec::new();
                                                            for (index, hit) in self.page_search_hits.iter().enumerate()
                                                            {
                                                                let for_scope = format!(
                                                                    "{}/@for:1234({})", use_scope, index
                                                                );
                                                                children
                                                                    .push(
                                                                        self
                                                                            .search_result(
                                                                                palette,
                                                                                format!("{}/PageSearchResult@1235", for_scope),
                                                                                hit.clone(),
                                                                            ),
                                                                    );
                                                            }
                                                            wire::Node::Linear {
                                                                max_width: None,
                                                                clip: false,
                                                                key: format!("{}/@layout:612", use_scope),
                                                                wrap: None,
                                                                axis: wire::Axis::Column,
                                                                spacing: Some((1.0) as f32),
                                                                padding: None,
                                                                width: Some(wire::Length::Fill),
                                                                height: None,
                                                                align: None,
                                                                background: None,
                                                                border: None,
                                                                children,
                                                            }
                                                        }),
                                                    }),
                                                }),
                                            });
                                    }
                                    if ((self.connected && (self.page_search_hits).is_empty())
                                        && crate::host::search_answer_stands(
                                            &(self.page_search_query),
                                            &(self.page_search_draft),
                                            self.page_searching,
                                        ))
                                    {
                                        children
                                            .push(wire::Node::Container {
                                                shadow: wire::Shadow {
                                                    color: None,
                                                    x: None,
                                                    y: None,
                                                    blur: None,
                                                },
                                                max_width: None,
                                                max_height: None,
                                                clip: false,
                                                key: format!("{}/@container:641", use_scope),
                                                width: Some(wire::Length::Fill),
                                                height: Some(wire::Length::Fill),
                                                padding: Some(wire::Edges {
                                                    top: (26.0) as f32,
                                                    right: (40.0) as f32,
                                                    bottom: (0.0) as f32,
                                                    left: (22.0) as f32,
                                                }),
                                                align_x: None,
                                                align_y: Some(wire::AlignY::Top),
                                                background: (None).map(wire::Background::Color),
                                                border: None,
                                                snap: None,
                                                content: Box::new(wire::Node::Container {
                                                    shadow: wire::Shadow {
                                                        color: Some(palette.colors[46]),
                                                        x: None,
                                                        y: Some((8.0) as f32),
                                                        blur: Some((24.0) as f32),
                                                    },
                                                    max_width: Some(((766.0) as f32).max(0.0).min(f32::MAX)),
                                                    max_height: None,
                                                    clip: false,
                                                    key: format!("{}/@container:652", use_scope),
                                                    width: Some(wire::Length::Fill),
                                                    height: None,
                                                    padding: None,
                                                    align_x: None,
                                                    align_y: None,
                                                    background: (Some(palette.colors[55]))
                                                        .map(wire::Background::Color),
                                                    border: Some(wire::Border {
                                                        color: None,
                                                        width: None,
                                                        radius: Some([
                                                            ((12.0) as f32).max(0.0).min(f32::MAX),
                                                            ((12.0) as f32).max(0.0).min(f32::MAX),
                                                            ((12.0) as f32).max(0.0).min(f32::MAX),
                                                            ((12.0) as f32).max(0.0).min(f32::MAX),
                                                        ]),
                                                    }),
                                                    snap: None,
                                                    content: Box::new(
                                                        self
                                                            .comments_empty(
                                                                palette,
                                                                format!("{}/EmptyPlate@1282", use_scope),
                                                            ),
                                                    ),
                                                }),
                                            });
                                    }
                                    if ((self.connected && (!(self.active_page).is_empty()))
                                        && self.block_comments_open)
                                    {
                                        children
                                            .push(wire::Node::Sensor {
                                                key: format!("{}/@sensor:679", use_scope),
                                                reset: None,
                                                on_show: Some(
                                                    ::ducktape_view_guest::slots::handler::<
                                                        (f32, f32),
                                                        Message,
                                                    >(
                                                        Box::new({
                                                            let route = {
                                                                let route_callback = (move |event_0, event_1| Message::CommentsCardMeasured(
                                                                    event_0,
                                                                    event_1,
                                                                ))
                                                                    .clone();
                                                                move |size: (f64, f64)| (route_callback)(size.0, size.1)
                                                            };
                                                            move |sent: (f32, f32)| Some(
                                                                route((f64::from(sent.0), f64::from(sent.1))),
                                                            )
                                                        }),
                                                    ),
                                                ),
                                                on_resize: Some(
                                                    ::ducktape_view_guest::slots::handler::<
                                                        (f32, f32),
                                                        Message,
                                                    >(
                                                        Box::new({
                                                            let route = {
                                                                let route_callback = (move |event_0, event_1| Message::CommentsCardMeasured(
                                                                    event_0,
                                                                    event_1,
                                                                ))
                                                                    .clone();
                                                                move |size: (f64, f64)| (route_callback)(size.0, size.1)
                                                            };
                                                            move |sent: (f32, f32)| Some(
                                                                route((f64::from(sent.0), f64::from(sent.1))),
                                                            )
                                                        }),
                                                    ),
                                                ),
                                                on_hide: None,
                                                anticipate: None,
                                                delay: None,
                                                child: Box::new(wire::Node::Float {
                                                    key: format!("{}/@float:680", use_scope),
                                                    x: wire::FloatExpression {
                                                        ops: vec![
                                                            ::ducktape_view_guest::wire::FloatOp::Geometry(4),
                                                            ::ducktape_view_guest::wire::FloatOp::Geometry(6),
                                                            ::ducktape_view_guest::wire::FloatOp::Add,
                                                            ::ducktape_view_guest::wire::FloatOp::Geometry(0),
                                                            ::ducktape_view_guest::wire::FloatOp::Subtract,
                                                            ::ducktape_view_guest::wire::FloatOp::Geometry(2),
                                                            ::ducktape_view_guest::wire::FloatOp::Subtract,
                                                            ::ducktape_view_guest::wire::FloatOp::Number((crate
                                                            ::host::comments_right_anchor(self.pages_pane_width)) as
                                                            f64), ::ducktape_view_guest::wire::FloatOp::Multiply,
                                                            ::ducktape_view_guest::wire::FloatOp::Number((crate
                                                            ::host::comments_left_inset(self.pages_pane_width)) as f64),
                                                            ::ducktape_view_guest::wire::FloatOp::Add
                                                        ],
                                                    },
                                                    y: wire::FloatExpression {
                                                        ops: vec![
                                                            ::ducktape_view_guest::wire::FloatOp::Number((crate
                                                            ::host::comment_card_offset(self.pages_pane_width, self
                                                            .comment_anchor_y, self.pages_viewport_height)) as f64)
                                                        ],
                                                    },
                                                    scale: ((1.0) as f32).max(f32::EPSILON).min(f32::MAX),
                                                    shadow: wire::Shadow {
                                                        color: None,
                                                        x: None,
                                                        y: None,
                                                        blur: None,
                                                    },
                                                    radius: None,
                                                    content: Box::new({
                                                        let node_scope = format!("{}/comments-card", use_scope);
                                                        wire::Node::Container {
                                                            shadow: wire::Shadow {
                                                                color: Some(palette.colors[46]),
                                                                x: None,
                                                                y: Some((8.0) as f32),
                                                                blur: Some((24.0) as f32),
                                                            },
                                                            max_width: None,
                                                            max_height: Some(
                                                                ((crate::host::comment_card_height(
                                                                    self.comment_anchor_y,
                                                                    self.pages_viewport_height,
                                                                )) as f32)
                                                                    .max(0.0)
                                                                    .min(f32::MAX),
                                                            ),
                                                            clip: true,
                                                            key: node_scope.clone(),
                                                            width: Some(
                                                                wire::Length::Fixed(
                                                                    (crate::host::comments_card_width(self.pages_pane_width))
                                                                        as f32,
                                                                ),
                                                            ),
                                                            height: Some(wire::Length::Shrink),
                                                            padding: None,
                                                            align_x: None,
                                                            align_y: None,
                                                            background: (Some(palette.colors[55]))
                                                                .map(wire::Background::Color),
                                                            border: Some(wire::Border {
                                                                color: Some(palette.colors[60]),
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
                                                                let mut children: Vec<wire::Node> = Vec::new();
                                                                children
                                                                    .push(wire::Node::Container {
                                                                        shadow: wire::Shadow {
                                                                            color: None,
                                                                            x: None,
                                                                            y: None,
                                                                            blur: None,
                                                                        },
                                                                        max_width: None,
                                                                        max_height: None,
                                                                        clip: false,
                                                                        key: format!("{}/@container:706", use_scope),
                                                                        width: Some(wire::Length::Fill),
                                                                        height: Some(wire::Length::Fixed((44.0) as f32)),
                                                                        padding: Some(wire::Edges {
                                                                            top: (0.0) as f32,
                                                                            right: (16.0) as f32,
                                                                            bottom: (0.0) as f32,
                                                                            left: (16.0) as f32,
                                                                        }),
                                                                        align_x: None,
                                                                        align_y: None,
                                                                        background: (None).map(wire::Background::Color),
                                                                        border: None,
                                                                        snap: None,
                                                                        content: Box::new({
                                                                            let mut children: Vec<wire::Node> = Vec::new();
                                                                            children
                                                                                .push(wire::Node::Container {
                                                                                    shadow: wire::Shadow {
                                                                                        color: None,
                                                                                        x: None,
                                                                                        y: None,
                                                                                        blur: None,
                                                                                    },
                                                                                    max_width: None,
                                                                                    max_height: None,
                                                                                    clip: true,
                                                                                    key: format!("{}/@container:718", use_scope),
                                                                                    width: Some(wire::Length::Fill),
                                                                                    height: None,
                                                                                    padding: None,
                                                                                    align_x: None,
                                                                                    align_y: None,
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
                                                                                        key: format!("{}/@text:719", use_scope),
                                                                                        size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                        color: Some(palette.colors[4]),
                                                                                        font: wire::Font {
                                                                                            monospace: false,
                                                                                            weight: wire::Weight::Normal,
                                                                                        },
                                                                                        width: None,
                                                                                        align_x: None,
                                                                                        content: crate::host::comment_scope_label(
                                                                                                &(self.blocks),
                                                                                                &(self.scope_target),
                                                                                                &(self.active_page),
                                                                                                self.thread_total,
                                                                                            )
                                                                                            .to_owned(),
                                                                                    }),
                                                                                });
                                                                            if ((!self.scope_pinned)
                                                                                && (!(self.scope_target).is_empty()))
                                                                            {
                                                                                children
                                                                                    .push(wire::Node::Button {
                                                                                        checked: None,
                                                                                        expanded: None,
                                                                                        description: None,
                                                                                        key: format!("{}/@button:725", use_scope),
                                                                                        content: wire::ButtonContent::Label(
                                                                                            String::from("← This page"),
                                                                                        ),
                                                                                        label: Some(
                                                                                            String::from("All comments on this page".to_owned()),
                                                                                        ),
                                                                                        on_press: if ((self.busy
                                                                                            || (!(self.host_error).is_empty())))
                                                                                        {
                                                                                            None
                                                                                        } else {
                                                                                            Some(
                                                                                                ::ducktape_view_guest::slots::message(
                                                                                                    Message::WidenCommentScope,
                                                                                                ),
                                                                                            )
                                                                                        },
                                                                                        width: None,
                                                                                        height: None,
                                                                                        padding: Some(wire::Edges::all((4.0) as f32)),
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
                                                                                                text_size: Some(11.0f32),
                                                                                                line_height: Some(1.35f32),
                                                                                                font: Some(wire::NamedFont {
                                                                                                    family: wire::FontFamily::Named("Geist".into()),
                                                                                                    weight: wire::Weight::Medium,
                                                                                                    stretch: wire::FontStretch::Normal,
                                                                                                    style: wire::FontStyle::Normal,
                                                                                                }),
                                                                                            }),
                                                                                            active: wire::Face {
                                                                                                background: Some(
                                                                                                    wire::Rgba([
                                                                                                        0.0 / 255.0,
                                                                                                        0.0 / 255.0,
                                                                                                        0.0 / 255.0,
                                                                                                        0.000000,
                                                                                                    ]),
                                                                                                ),
                                                                                                text: Some(palette.colors[5]),
                                                                                                border: Some(wire::Border {
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
                                                                                            hovered: Some(wire::Face {
                                                                                                background: Some({
                                                                                                    let mut color = palette.colors[4];
                                                                                                    color.0[3] = 0.090000;
                                                                                                    color
                                                                                                }),
                                                                                                text: Some(palette.colors[4]),
                                                                                                border: None,
                                                                                            }),
                                                                                            pressed: Some(wire::Face {
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
                                                                            children
                                                                                .push(wire::Node::Button {
                                                                                    checked: None,
                                                                                    expanded: None,
                                                                                    description: None,
                                                                                    key: format!("{}/@button:734", use_scope),
                                                                                    content: wire::ButtonContent::Child(
                                                                                        Box::new(wire::Node::Container {
                                                                                            shadow: wire::Shadow {
                                                                                                color: None,
                                                                                                x: None,
                                                                                                y: None,
                                                                                                blur: None,
                                                                                            },
                                                                                            max_width: None,
                                                                                            max_height: None,
                                                                                            clip: false,
                                                                                            key: format!("{}/@container:742", use_scope),
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
                                                                                                key: format!("{}/@text:748", use_scope),
                                                                                                size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                color: Some(palette.colors[5]),
                                                                                                font: wire::Font {
                                                                                                    monospace: false,
                                                                                                    weight: wire::Weight::Normal,
                                                                                                },
                                                                                                width: None,
                                                                                                align_x: None,
                                                                                                content: "×".to_owned(),
                                                                                            }),
                                                                                        }),
                                                                                    ),
                                                                                    label: Some(String::from("Close comments".to_owned())),
                                                                                    on_press: if ((self.busy
                                                                                        || (!(self.host_error).is_empty())))
                                                                                    {
                                                                                        None
                                                                                    } else {
                                                                                        Some(
                                                                                            ::ducktape_view_guest::slots::message(
                                                                                                Message::CloseBlockComments,
                                                                                            ),
                                                                                        )
                                                                                    },
                                                                                    width: Some(wire::Length::Fixed((24.0) as f32)),
                                                                                    height: Some(wire::Length::Fixed((24.0) as f32)),
                                                                                    padding: Some(wire::Edges::all((4.0) as f32)),
                                                                                    style: wire::ButtonStyle {
                                                                                        preset: wire::ButtonPreset::Primary,
                                                                                        recipe: Some(wire::ButtonRecipe {
                                                                                            base: wire::Face {
                                                                                                background: Some(
                                                                                                    wire::Rgba([
                                                                                                        0.0 / 255.0,
                                                                                                        0.0 / 255.0,
                                                                                                        0.0 / 255.0,
                                                                                                        0.000000,
                                                                                                    ]),
                                                                                                ),
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
                                                                                            background: Some(
                                                                                                wire::Rgba([
                                                                                                    0.0 / 255.0,
                                                                                                    0.0 / 255.0,
                                                                                                    0.0 / 255.0,
                                                                                                    0.000000,
                                                                                                ]),
                                                                                            ),
                                                                                            text: Some(palette.colors[5]),
                                                                                            border: Some(wire::Border {
                                                                                                color: Some(
                                                                                                    wire::Rgba([
                                                                                                        0.0 / 255.0,
                                                                                                        0.0 / 255.0,
                                                                                                        0.0 / 255.0,
                                                                                                        0.000000,
                                                                                                    ]),
                                                                                                ),
                                                                                                width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                                radius: Some([
                                                                                                    ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                ]),
                                                                                            }),
                                                                                        },
                                                                                        hovered: Some(wire::Face {
                                                                                            background: Some(palette.colors[56]),
                                                                                            text: Some(palette.colors[4]),
                                                                                            border: None,
                                                                                        }),
                                                                                        pressed: Some(wire::Face {
                                                                                            background: Some(palette.colors[60]),
                                                                                            text: Some(palette.colors[4]),
                                                                                            border: None,
                                                                                        }),
                                                                                        disabled: None,
                                                                                    },
                                                                                });
                                                                            wire::Node::Linear {
                                                                                max_width: None,
                                                                                clip: false,
                                                                                key: format!("{}/@layout:712", use_scope),
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
                                                                children
                                                                    .push(wire::Node::Container {
                                                                        shadow: wire::Shadow {
                                                                            color: None,
                                                                            x: None,
                                                                            y: None,
                                                                            blur: None,
                                                                        },
                                                                        max_width: None,
                                                                        max_height: None,
                                                                        clip: false,
                                                                        key: format!("{}/@container:756", use_scope),
                                                                        width: Some(wire::Length::Fill),
                                                                        height: Some(wire::Length::Fixed((1.0) as f32)),
                                                                        padding: None,
                                                                        align_x: None,
                                                                        align_y: None,
                                                                        background: (Some(palette.colors[60]))
                                                                            .map(wire::Background::Color),
                                                                        border: None,
                                                                        snap: None,
                                                                        content: Box::new(wire::Node::Space {
                                                                            width: Some(wire::Length::Fixed((1.0) as f32)),
                                                                            height: Some(wire::Length::Fixed((1.0) as f32)),
                                                                        }),
                                                                    });
                                                                children
                                                                    .push({
                                                                        let mut children: Vec<wire::Node> = Vec::new();
                                                                        if self.threads_loading {
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
                                                                                            weight: wire::Weight::Normal,
                                                                                            stretch: wire::FontStretch::Normal,
                                                                                            style: wire::FontStyle::Normal,
                                                                                        }),
                                                                                    },
                                                                                    key: format!("{}/@text:769", use_scope),
                                                                                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                    color: Some(palette.colors[5]),
                                                                                    font: wire::Font {
                                                                                        monospace: false,
                                                                                        weight: wire::Weight::Normal,
                                                                                    },
                                                                                    width: None,
                                                                                    align_x: None,
                                                                                    content: "Loading comments…".to_owned(),
                                                                                });
                                                                        }
                                                                        children
                                                                            .push(wire::Node::Container {
                                                                                shadow: wire::Shadow {
                                                                                    color: None,
                                                                                    x: None,
                                                                                    y: None,
                                                                                    blur: None,
                                                                                },
                                                                                max_width: None,
                                                                                max_height: Some(
                                                                                    (((crate::host::comment_card_height(
                                                                                        self.comment_anchor_y,
                                                                                        self.pages_viewport_height,
                                                                                    ) - 150.0)) as f32)
                                                                                        .max(0.0)
                                                                                        .min(f32::MAX),
                                                                                ),
                                                                                clip: false,
                                                                                key: format!("{}/@container:774", use_scope),
                                                                                width: Some(wire::Length::Fill),
                                                                                height: Some(wire::Length::Shrink),
                                                                                padding: None,
                                                                                align_x: None,
                                                                                align_y: None,
                                                                                background: (None).map(wire::Background::Color),
                                                                                border: None,
                                                                                snap: None,
                                                                                content: Box::new(wire::Node::Scroll {
                                                                                    on_scroll: None,
                                                                                    virtual_rows: false,
                                                                                    key: format!("{}/@layout:775", use_scope),
                                                                                    direction: wire::ScrollDirection::Vertical,
                                                                                    width: Some(wire::Length::Fill),
                                                                                    height: Some(wire::Length::Shrink),
                                                                                    bar_hidden: false,
                                                                                    bar_width: None,
                                                                                    bar_margin: None,
                                                                                    scroller_width: None,
                                                                                    bar_spacing: None,
                                                                                    anchor_x: wire::ScrollAnchor::Start,
                                                                                    anchor_y: wire::ScrollAnchor::Start,
                                                                                    auto_scroll: (false),
                                                                                    background: None,
                                                                                    border: None,
                                                                                    content: Box::new({
                                                                                        let mut children: Vec<wire::Node> = Vec::new();
                                                                                        if (((crate::host::scope_groups(
                                                                                            self.comment_rows.clone(),
                                                                                            &(self.scope_target),
                                                                                            &(self.active_page),
                                                                                        ))
                                                                                            .is_empty()
                                                                                            && (crate::host::scope_resolved(
                                                                                                self.comment_rows.clone(),
                                                                                                &(self.scope_target),
                                                                                            ))
                                                                                                .is_empty()) && (!self.threads_loading))
                                                                                        {
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
                                                                                                            weight: wire::Weight::Normal,
                                                                                                            stretch: wire::FontStretch::Normal,
                                                                                                            style: wire::FontStyle::Normal,
                                                                                                        }),
                                                                                                    },
                                                                                                    key: format!("{}/@text:786", use_scope),
                                                                                                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                    color: Some(palette.colors[5]),
                                                                                                    font: wire::Font {
                                                                                                        monospace: false,
                                                                                                        weight: wire::Weight::Normal,
                                                                                                    },
                                                                                                    width: Some(wire::Length::Fill),
                                                                                                    align_x: Some(wire::AlignX::Center),
                                                                                                    content: (crate::host::empty_scope_label(
                                                                                                        &(self.scope_target),
                                                                                                    ))
                                                                                                        .to_string(),
                                                                                                });
                                                                                        }
                                                                                        for (index, comment_group) in crate::host::scope_groups(
                                                                                                self.comment_rows.clone(),
                                                                                                &(self.scope_target),
                                                                                                &(self.active_page),
                                                                                            )
                                                                                            .iter()
                                                                                            .enumerate()
                                                                                        {
                                                                                            let for_scope = format!(
                                                                                                "{}/@for:1413({})", use_scope, index
                                                                                            );
                                                                                            children
                                                                                                .push({
                                                                                                    let mut children: Vec<wire::Node> = Vec::new();
                                                                                                    if ((self.scope_target).is_empty()
                                                                                                        && (comment_group.target != self.active_page))
                                                                                                    {
                                                                                                        children
                                                                                                            .push(wire::Node::Button {
                                                                                                                checked: None,
                                                                                                                expanded: None,
                                                                                                                description: Some(
                                                                                                                    String::from(comment_group.anchor.to_owned()),
                                                                                                                ),
                                                                                                                key: format!("{}/@button:806", for_scope),
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
                                                                                                                                family: wire::FontFamily::Named("Geist Mono".into()),
                                                                                                                                weight: wire::Weight::Medium,
                                                                                                                                stretch: wire::FontStretch::Normal,
                                                                                                                                style: wire::FontStyle::Normal,
                                                                                                                            }),
                                                                                                                        },
                                                                                                                        key: format!("{}/@text:814", for_scope),
                                                                                                                        size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                        color: Some(palette.colors[72]),
                                                                                                                        font: wire::Font {
                                                                                                                            monospace: false,
                                                                                                                            weight: wire::Weight::Normal,
                                                                                                                        },
                                                                                                                        width: Some(wire::Length::Fill),
                                                                                                                        align_x: None,
                                                                                                                        content: comment_group.anchor.to_owned(),
                                                                                                                    }),
                                                                                                                ),
                                                                                                                label: Some(
                                                                                                                    String::from("Comments on this block".to_owned()),
                                                                                                                ),
                                                                                                                on_press: if ((self.busy
                                                                                                                    || (!(self.host_error).is_empty())))
                                                                                                                {
                                                                                                                    None
                                                                                                                } else {
                                                                                                                    Some(
                                                                                                                        ::ducktape_view_guest::slots::message(
                                                                                                                            (move |event_0| Message::NarrowCommentScope(
                                                                                                                                event_0,
                                                                                                                            ))(comment_group.target.to_owned()),
                                                                                                                        ),
                                                                                                                    )
                                                                                                                },
                                                                                                                width: Some(wire::Length::Fill),
                                                                                                                height: None,
                                                                                                                padding: Some(wire::Edges::all((4.0) as f32)),
                                                                                                                style: wire::ButtonStyle {
                                                                                                                    preset: wire::ButtonPreset::Primary,
                                                                                                                    recipe: Some(wire::ButtonRecipe {
                                                                                                                        base: wire::Face {
                                                                                                                            background: Some(
                                                                                                                                wire::Rgba([
                                                                                                                                    0.0 / 255.0,
                                                                                                                                    0.0 / 255.0,
                                                                                                                                    0.0 / 255.0,
                                                                                                                                    0.000000,
                                                                                                                                ]),
                                                                                                                            ),
                                                                                                                            text: Some(palette.colors[4]),
                                                                                                                            border: Some(wire::Border {
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
                                                                                                                        font: Some(wire::NamedFont {
                                                                                                                            family: wire::FontFamily::Named("Geist".into()),
                                                                                                                            weight: wire::Weight::Semibold,
                                                                                                                            stretch: wire::FontStretch::Normal,
                                                                                                                            style: wire::FontStyle::Normal,
                                                                                                                        }),
                                                                                                                    }),
                                                                                                                    active: wire::Face {
                                                                                                                        background: Some(
                                                                                                                            wire::Rgba([
                                                                                                                                0.0 / 255.0,
                                                                                                                                0.0 / 255.0,
                                                                                                                                0.0 / 255.0,
                                                                                                                                0.000000,
                                                                                                                            ]),
                                                                                                                        ),
                                                                                                                        text: Some(palette.colors[72]),
                                                                                                                        border: Some(wire::Border {
                                                                                                                            color: Some(
                                                                                                                                wire::Rgba([
                                                                                                                                    0.0 / 255.0,
                                                                                                                                    0.0 / 255.0,
                                                                                                                                    0.0 / 255.0,
                                                                                                                                    0.000000,
                                                                                                                                ]),
                                                                                                                            ),
                                                                                                                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                                                            radius: Some([
                                                                                                                                ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                                ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                            ]),
                                                                                                                        }),
                                                                                                                    },
                                                                                                                    hovered: Some(wire::Face {
                                                                                                                        background: Some({
                                                                                                                            let mut color = palette.colors[4];
                                                                                                                            color.0[3] = 0.060000;
                                                                                                                            color
                                                                                                                        }),
                                                                                                                        text: Some(palette.colors[4]),
                                                                                                                        border: None,
                                                                                                                    }),
                                                                                                                    pressed: Some(wire::Face {
                                                                                                                        background: Some({
                                                                                                                            let mut color = palette.colors[4];
                                                                                                                            color.0[3] = 0.100000;
                                                                                                                            color
                                                                                                                        }),
                                                                                                                        text: Some(palette.colors[4]),
                                                                                                                        border: None,
                                                                                                                    }),
                                                                                                                    disabled: None,
                                                                                                                },
                                                                                                            });
                                                                                                    }
                                                                                                    children
                                                                                                        .push({
                                                                                                            let mut children: Vec<wire::Node> = Vec::new();
                                                                                                            for (index, group_thread) in comment_group
                                                                                                                .threads
                                                                                                                .iter()
                                                                                                                .enumerate()
                                                                                                            {
                                                                                                                let for_scope = format!(
                                                                                                                    "{}/@for:1446({})", for_scope, index
                                                                                                                );
                                                                                                                children
                                                                                                                    .push(
                                                                                                                        self
                                                                                                                            .comment_thread(
                                                                                                                                palette,
                                                                                                                                format!("{}/PageCommentThreadCard@1447", for_scope),
                                                                                                                                group_thread.clone(),
                                                                                                                                (self.reply_thread == group_thread.id),
                                                                                                                                crate::host::expanded(
                                                                                                                                    &(self.expanded_threads),
                                                                                                                                    &(group_thread.id),
                                                                                                                                ),
                                                                                                                            ),
                                                                                                                    );
                                                                                                            }
                                                                                                            wire::Node::Linear {
                                                                                                                max_width: None,
                                                                                                                clip: false,
                                                                                                                key: format!("{}/@layout:824", for_scope),
                                                                                                                wrap: None,
                                                                                                                axis: wire::Axis::Column,
                                                                                                                spacing: Some((16.0) as f32),
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
                                                                                                        key: format!("{}/@layout:793", for_scope),
                                                                                                        wrap: None,
                                                                                                        axis: wire::Axis::Column,
                                                                                                        spacing: Some((8.0) as f32),
                                                                                                        padding: None,
                                                                                                        width: Some(wire::Length::Fill),
                                                                                                        height: None,
                                                                                                        align: None,
                                                                                                        background: None,
                                                                                                        border: None,
                                                                                                        children,
                                                                                                    }
                                                                                                });
                                                                                        }
                                                                                        if (!(crate::host::scope_resolved(
                                                                                            self.comment_rows.clone(),
                                                                                            &(self.scope_target),
                                                                                        ))
                                                                                            .is_empty())
                                                                                        {
                                                                                            children
                                                                                                .push(wire::Node::Button {
                                                                                                    checked: None,
                                                                                                    expanded: Some(self.resolved_open),
                                                                                                    description: None,
                                                                                                    key: format!("{}/@button:833", use_scope),
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
                                                                                                                    weight: wire::Weight::Medium,
                                                                                                                    stretch: wire::FontStretch::Normal,
                                                                                                                    style: wire::FontStyle::Normal,
                                                                                                                }),
                                                                                                            },
                                                                                                            key: format!("{}/@text:841", use_scope),
                                                                                                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[5]),
                                                                                                            font: wire::Font {
                                                                                                                monospace: false,
                                                                                                                weight: wire::Weight::Normal,
                                                                                                            },
                                                                                                            width: Some(wire::Length::Fill),
                                                                                                            align_x: None,
                                                                                                            content: (crate::host::resolved_label(
                                                                                                                &(crate::host::scope_resolved(
                                                                                                                    self.comment_rows.clone(),
                                                                                                                    &(self.scope_target),
                                                                                                                )),
                                                                                                            ))
                                                                                                                .to_string(),
                                                                                                        }),
                                                                                                    ),
                                                                                                    label: Some(String::from("Resolved threads".to_owned())),
                                                                                                    on_press: if ((self.busy
                                                                                                        || (!(self.host_error).is_empty())))
                                                                                                    {
                                                                                                        None
                                                                                                    } else {
                                                                                                        Some(
                                                                                                            ::ducktape_view_guest::slots::message(
                                                                                                                Message::ToggleResolvedComments,
                                                                                                            ),
                                                                                                        )
                                                                                                    },
                                                                                                    width: Some(wire::Length::Fill),
                                                                                                    height: None,
                                                                                                    padding: Some(wire::Edges::all((4.0) as f32)),
                                                                                                    style: wire::ButtonStyle {
                                                                                                        preset: wire::ButtonPreset::Primary,
                                                                                                        recipe: Some(wire::ButtonRecipe {
                                                                                                            base: wire::Face {
                                                                                                                background: Some(
                                                                                                                    wire::Rgba([
                                                                                                                        0.0 / 255.0,
                                                                                                                        0.0 / 255.0,
                                                                                                                        0.0 / 255.0,
                                                                                                                        0.000000,
                                                                                                                    ]),
                                                                                                                ),
                                                                                                                text: Some(palette.colors[4]),
                                                                                                                border: Some(wire::Border {
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
                                                                                                            font: Some(wire::NamedFont {
                                                                                                                family: wire::FontFamily::Named("Geist".into()),
                                                                                                                weight: wire::Weight::Semibold,
                                                                                                                stretch: wire::FontStretch::Normal,
                                                                                                                style: wire::FontStyle::Normal,
                                                                                                            }),
                                                                                                        }),
                                                                                                        active: wire::Face {
                                                                                                            background: Some(
                                                                                                                wire::Rgba([
                                                                                                                    0.0 / 255.0,
                                                                                                                    0.0 / 255.0,
                                                                                                                    0.0 / 255.0,
                                                                                                                    0.000000,
                                                                                                                ]),
                                                                                                            ),
                                                                                                            text: Some(palette.colors[5]),
                                                                                                            border: Some(wire::Border {
                                                                                                                color: Some(
                                                                                                                    wire::Rgba([
                                                                                                                        0.0 / 255.0,
                                                                                                                        0.0 / 255.0,
                                                                                                                        0.0 / 255.0,
                                                                                                                        0.000000,
                                                                                                                    ]),
                                                                                                                ),
                                                                                                                width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                                                radius: Some([
                                                                                                                    ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                    ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                    ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                    ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                ]),
                                                                                                            }),
                                                                                                        },
                                                                                                        hovered: Some(wire::Face {
                                                                                                            background: Some({
                                                                                                                let mut color = palette.colors[4];
                                                                                                                color.0[3] = 0.060000;
                                                                                                                color
                                                                                                            }),
                                                                                                            text: Some(palette.colors[4]),
                                                                                                            border: None,
                                                                                                        }),
                                                                                                        pressed: Some(wire::Face {
                                                                                                            background: Some({
                                                                                                                let mut color = palette.colors[4];
                                                                                                                color.0[3] = 0.100000;
                                                                                                                color
                                                                                                            }),
                                                                                                            text: Some(palette.colors[4]),
                                                                                                            border: None,
                                                                                                        }),
                                                                                                        disabled: None,
                                                                                                    },
                                                                                                });
                                                                                        }
                                                                                        if self.resolved_open {
                                                                                            children
                                                                                                .push({
                                                                                                    let mut children: Vec<wire::Node> = Vec::new();
                                                                                                    for (index, resolved_row) in crate::host::scope_resolved(
                                                                                                            self.comment_rows.clone(),
                                                                                                            &(self.scope_target),
                                                                                                        )
                                                                                                        .iter()
                                                                                                        .enumerate()
                                                                                                    {
                                                                                                        let for_scope = format!(
                                                                                                            "{}/@for:1474({})", use_scope, index
                                                                                                        );
                                                                                                        children
                                                                                                            .push(
                                                                                                                self
                                                                                                                    .resolved_thread(
                                                                                                                        palette,
                                                                                                                        format!("{}/PageCommentThreadCard@1475", for_scope),
                                                                                                                        resolved_row.thread.clone(),
                                                                                                                        crate::host::expanded(
                                                                                                                            &(self.expanded_threads),
                                                                                                                            &(resolved_row.thread.id),
                                                                                                                        ),
                                                                                                                    ),
                                                                                                            );
                                                                                                    }
                                                                                                    wire::Node::Linear {
                                                                                                        max_width: None,
                                                                                                        clip: false,
                                                                                                        key: format!("{}/@layout:852", use_scope),
                                                                                                        wrap: None,
                                                                                                        axis: wire::Axis::Column,
                                                                                                        spacing: Some((16.0) as f32),
                                                                                                        padding: None,
                                                                                                        width: Some(wire::Length::Fill),
                                                                                                        height: None,
                                                                                                        align: None,
                                                                                                        background: None,
                                                                                                        border: None,
                                                                                                        children,
                                                                                                    }
                                                                                                });
                                                                                        }
                                                                                        wire::Node::Linear {
                                                                                            max_width: None,
                                                                                            clip: false,
                                                                                            key: format!("{}/@layout:784", use_scope),
                                                                                            wrap: None,
                                                                                            axis: wire::Axis::Column,
                                                                                            spacing: Some((18.0) as f32),
                                                                                            padding: None,
                                                                                            width: Some(wire::Length::Fill),
                                                                                            height: None,
                                                                                            align: None,
                                                                                            background: None,
                                                                                            border: None,
                                                                                            children,
                                                                                        }
                                                                                    }),
                                                                                }),
                                                                            });
                                                                        wire::Node::Linear {
                                                                            max_width: None,
                                                                            clip: false,
                                                                            key: format!("{}/@layout:762", use_scope),
                                                                            wrap: None,
                                                                            axis: wire::Axis::Column,
                                                                            spacing: Some((10.0) as f32),
                                                                            padding: Some(wire::Edges {
                                                                                top: (12.0) as f32,
                                                                                right: (12.0) as f32,
                                                                                bottom: (12.0) as f32,
                                                                                left: (12.0) as f32,
                                                                            }),
                                                                            width: Some(wire::Length::Fill),
                                                                            height: Some(wire::Length::Shrink),
                                                                            align: None,
                                                                            background: None,
                                                                            border: None,
                                                                            children,
                                                                        }
                                                                    });
                                                                children
                                                                    .push(wire::Node::Container {
                                                                        shadow: wire::Shadow {
                                                                            color: None,
                                                                            x: None,
                                                                            y: None,
                                                                            blur: None,
                                                                        },
                                                                        max_width: None,
                                                                        max_height: None,
                                                                        clip: false,
                                                                        key: format!("{}/@container:866", use_scope),
                                                                        width: Some(wire::Length::Fill),
                                                                        height: Some(wire::Length::Fixed((1.0) as f32)),
                                                                        padding: None,
                                                                        align_x: None,
                                                                        align_y: None,
                                                                        background: (Some(palette.colors[60]))
                                                                            .map(wire::Background::Color),
                                                                        border: None,
                                                                        snap: None,
                                                                        content: Box::new(wire::Node::Space {
                                                                            width: Some(wire::Length::Fixed((1.0) as f32)),
                                                                            height: Some(wire::Length::Fixed((1.0) as f32)),
                                                                        }),
                                                                    });
                                                                children
                                                                    .push({
                                                                        let mut children: Vec<wire::Node> = Vec::new();
                                                                        children
                                                                            .push(wire::Node::Container {
                                                                                shadow: wire::Shadow {
                                                                                    color: None,
                                                                                    x: None,
                                                                                    y: None,
                                                                                    blur: None,
                                                                                },
                                                                                max_width: None,
                                                                                max_height: None,
                                                                                clip: true,
                                                                                key: format!("{}/@container:887", use_scope),
                                                                                width: Some(wire::Length::Fill),
                                                                                height: None,
                                                                                padding: None,
                                                                                align_x: None,
                                                                                align_y: None,
                                                                                background: (None).map(wire::Background::Color),
                                                                                border: None,
                                                                                snap: None,
                                                                                content: Box::new(wire::Node::Text {
                                                                                    options: wire::TextOptions {
                                                                                        height: None,
                                                                                        align_y: None,
                                                                                        line_height: None,
                                                                                        shaping: None,
                                                                                        wrapping: Some(wire::Wrapping::Word),
                                                                                        tracking: 0.0f32,
                                                                                        font: Some(wire::NamedFont {
                                                                                            family: wire::FontFamily::Named("Geist".into()),
                                                                                            weight: wire::Weight::Medium,
                                                                                            stretch: wire::FontStretch::Normal,
                                                                                            style: wire::FontStyle::Normal,
                                                                                        }),
                                                                                    },
                                                                                    key: format!("{}/@text:888", use_scope),
                                                                                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                    color: Some(palette.colors[5]),
                                                                                    font: wire::Font {
                                                                                        monospace: false,
                                                                                        weight: wire::Weight::Normal,
                                                                                    },
                                                                                    width: Some(wire::Length::Fill),
                                                                                    align_x: None,
                                                                                    content: crate::host::compose_hint_of(
                                                                                            &(self.blocks),
                                                                                            &(self.scope_target),
                                                                                            &(self.active_page),
                                                                                        )
                                                                                        .to_owned(),
                                                                                }),
                                                                            });
                                                                        children
                                                                            .push({
                                                                                let mut children: Vec<wire::Node> = Vec::new();
                                                                                children
                                                                                    .push({
                                                                                        let node_scope = format!(
                                                                                            "{}/page-comment({})", node_scope, self.active_page
                                                                                        );
                                                                                        wire::Node::Input {
                                                                                            options: wire::InputOptions {
                                                                                                label: "New page comment".to_owned(),
                                                                                                description: None,
                                                                                                disabled: ((self.busy || (!(self.host_error).is_empty()))
                                                                                                    || self.threads_loading),
                                                                                                padding: Some(wire::Edges::all((6.2) as f32)),
                                                                                                text_size: Some((13.0) as f32),
                                                                                                line_height: Some((1.2) as f32),
                                                                                                align: None,
                                                                                                font: Some(wire::NamedFont {
                                                                                                    family: wire::FontFamily::Named("Geist".into()),
                                                                                                    weight: wire::Weight::Normal,
                                                                                                    stretch: wire::FontStretch::Normal,
                                                                                                    style: wire::FontStyle::Normal,
                                                                                                }),
                                                                                            },
                                                                                            key: node_scope.clone(),
                                                                                            placeholder: String::from("Start a thread…".to_owned()),
                                                                                            value: (self.block_comment_draft).to_string(),
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
                                                                                                    Message::PostBlockCommentSubmit,
                                                                                                ),
                                                                                            ),
                                                                                            width: Some(wire::Length::Fill),
                                                                                            secure: (false),
                                                                                            style: Box::new(wire::InputStyle {
                                                                                                utility: wire::InputFace {
                                                                                                    background: Some(palette.colors[3]),
                                                                                                    border: Some(wire::Border {
                                                                                                        color: Some(palette.colors[39]),
                                                                                                        width: Some(1f32),
                                                                                                        radius: Some([10f32; 4]),
                                                                                                    }),
                                                                                                    ..Default::default()
                                                                                                },
                                                                                                focus_border: Some(palette.colors[42]),
                                                                                                focused_hovered: None,
                                                                                                active: wire::InputFace {
                                                                                                    icon: None,
                                                                                                    background: Some(
                                                                                                        wire::Rgba([
                                                                                                            0.0 / 255.0,
                                                                                                            0.0 / 255.0,
                                                                                                            0.0 / 255.0,
                                                                                                            0.000000,
                                                                                                        ]),
                                                                                                    ),
                                                                                                    border: Some(wire::Border {
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
                                                                                                hovered: Some(wire::InputFace {
                                                                                                    icon: None,
                                                                                                    background: Some({
                                                                                                        let mut color = palette.colors[4];
                                                                                                        color.0[3] = 0.040000;
                                                                                                        color
                                                                                                    }),
                                                                                                    border: Some(wire::Border {
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
                                                                                                focused: Some(wire::InputFace {
                                                                                                    icon: None,
                                                                                                    background: Some({
                                                                                                        let mut color = palette.colors[4];
                                                                                                        color.0[3] = 0.040000;
                                                                                                        color
                                                                                                    }),
                                                                                                    border: Some(wire::Border {
                                                                                                        color: Some(palette.colors[42]),
                                                                                                        width: None,
                                                                                                        radius: None,
                                                                                                    }),
                                                                                                    value: None,
                                                                                                    placeholder: None,
                                                                                                    selection: None,
                                                                                                }),
                                                                                                disabled: Some(wire::InputFace {
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
                                                                                children
                                                                                    .push({
                                                                                        let node_scope = format!("{}/post", node_scope);
                                                                                        wire::Node::Button {
                                                                                            checked: None,
                                                                                            expanded: None,
                                                                                            description: None,
                                                                                            key: node_scope.clone(),
                                                                                            content: wire::ButtonContent::Label(String::from("Post")),
                                                                                            label: None,
                                                                                            on_press: if ((((self.busy
                                                                                                || (!(self.host_error).is_empty()))
                                                                                                || ((self.block_comment_draft).trim().to_owned())
                                                                                                    .is_empty()) || self.threads_loading))
                                                                                            {
                                                                                                None
                                                                                            } else {
                                                                                                Some(
                                                                                                    ::ducktape_view_guest::slots::message(
                                                                                                        Message::PostBlockCommentSubmit,
                                                                                                    ),
                                                                                                )
                                                                                            },
                                                                                            width: None,
                                                                                            height: None,
                                                                                            padding: Some(wire::Edges::all((5.0) as f32)),
                                                                                            style: wire::ButtonStyle {
                                                                                                preset: wire::ButtonPreset::Primary,
                                                                                                recipe: Some(wire::ButtonRecipe {
                                                                                                    base: wire::Face {
                                                                                                        background: Some(palette.colors[7]),
                                                                                                        text: Some(palette.colors[9]),
                                                                                                        border: Some(wire::Border {
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
                                                                                        }
                                                                                    });
                                                                                wire::Node::Linear {
                                                                                    max_width: None,
                                                                                    clip: false,
                                                                                    key: format!("{}/@layout:895", use_scope),
                                                                                    wrap: None,
                                                                                    axis: wire::Axis::Row,
                                                                                    spacing: Some((5.0) as f32),
                                                                                    padding: None,
                                                                                    width: Some(wire::Length::Fill),
                                                                                    height: None,
                                                                                    align: Some(wire::AlignX::Center),
                                                                                    background: None,
                                                                                    border: None,
                                                                                    children,
                                                                                }
                                                                            });
                                                                        wire::Node::Linear {
                                                                            max_width: None,
                                                                            clip: false,
                                                                            key: format!("{}/@layout:872", use_scope),
                                                                            wrap: None,
                                                                            axis: wire::Axis::Column,
                                                                            spacing: Some((6.0) as f32),
                                                                            padding: Some(wire::Edges {
                                                                                top: (12.0) as f32,
                                                                                right: (12.0) as f32,
                                                                                bottom: (12.0) as f32,
                                                                                left: (12.0) as f32,
                                                                            }),
                                                                            width: Some(wire::Length::Fill),
                                                                            height: Some(wire::Length::Shrink),
                                                                            align: None,
                                                                            background: None,
                                                                            border: None,
                                                                            children,
                                                                        }
                                                                    });
                                                                wire::Node::Linear {
                                                                    max_width: None,
                                                                    clip: false,
                                                                    key: format!("{}/@layout:694", use_scope),
                                                                    wrap: None,
                                                                    axis: wire::Axis::Column,
                                                                    spacing: None,
                                                                    padding: None,
                                                                    width: Some(wire::Length::Fill),
                                                                    height: Some(wire::Length::Shrink),
                                                                    align: None,
                                                                    background: None,
                                                                    border: None,
                                                                    children,
                                                                }
                                                            }),
                                                        }
                                                    }),
                                                }),
                                            });
                                    }
                                    children
                                        .push({
                                            let mut children = vec![
                                                ::ducktape_view_guest::wire::Node::Space { width :
                                                Some(::ducktape_view_guest::wire::Length::Fill), height :
                                                Some(::ducktape_view_guest::wire::Length::Fill) }
                                            ];
                                            if self.page_menu_open {
                                                children
                                                    .push({
                                                        let node_scope = format!("{}/page-menu", use_scope);
                                                        wire::Node::Container {
                                                            shadow: wire::Shadow {
                                                                color: Some(palette.colors[46]),
                                                                x: None,
                                                                y: Some((8.0) as f32),
                                                                blur: Some((24.0) as f32),
                                                            },
                                                            max_width: None,
                                                            max_height: None,
                                                            clip: false,
                                                            key: node_scope.clone(),
                                                            width: Some(wire::Length::Fixed((186.0) as f32)),
                                                            height: None,
                                                            padding: Some(wire::Edges {
                                                                top: (4.0) as f32,
                                                                right: (4.0) as f32,
                                                                bottom: (4.0) as f32,
                                                                left: (4.0) as f32,
                                                            }),
                                                            align_x: None,
                                                            align_y: None,
                                                            background: (Some(palette.colors[55]))
                                                                .map(wire::Background::Color),
                                                            border: Some(wire::Border {
                                                                color: Some({
                                                                    let mut color = palette.colors[4];
                                                                    color.0[3] = 0.100000;
                                                                    color
                                                                }),
                                                                width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                radius: Some([
                                                                    ((10.0) as f32).max(0.0).min(f32::MAX),
                                                                    ((10.0) as f32).max(0.0).min(f32::MAX),
                                                                    ((10.0) as f32).max(0.0).min(f32::MAX),
                                                                    ((10.0) as f32).max(0.0).min(f32::MAX),
                                                                ]),
                                                            }),
                                                            snap: None,
                                                            content: Box::new(wire::Node::Button {
                                                                checked: None,
                                                                expanded: None,
                                                                description: None,
                                                                key: format!("{}/@button:951", use_scope),
                                                                content: wire::ButtonContent::Label(
                                                                    String::from("Delete page…"),
                                                                ),
                                                                label: Some(String::from("Delete page".to_owned())),
                                                                on_press: if ((self.busy
                                                                    || (!(self.host_error).is_empty())))
                                                                {
                                                                    None
                                                                } else {
                                                                    Some(
                                                                        ::ducktape_view_guest::slots::message(
                                                                            Message::ArmPageDelete,
                                                                        ),
                                                                    )
                                                                },
                                                                width: Some(wire::Length::Fill),
                                                                height: None,
                                                                padding: Some(wire::Edges::all((6.0) as f32)),
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
                                                                    active: wire::Face {
                                                                        background: Some(
                                                                            wire::Rgba([
                                                                                0.0 / 255.0,
                                                                                0.0 / 255.0,
                                                                                0.0 / 255.0,
                                                                                0.000000,
                                                                            ]),
                                                                        ),
                                                                        text: Some(palette.colors[20]),
                                                                        border: Some(wire::Border {
                                                                            color: Some(
                                                                                wire::Rgba([
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
                                                                    hovered: Some(wire::Face {
                                                                        background: Some(palette.colors[22]),
                                                                        text: Some(palette.colors[20]),
                                                                        border: Some(wire::Border {
                                                                            color: Some(palette.colors[23]),
                                                                            width: None,
                                                                            radius: None,
                                                                        }),
                                                                    }),
                                                                    pressed: Some(wire::Face {
                                                                        background: Some(palette.colors[23]),
                                                                        text: Some(palette.colors[4]),
                                                                        border: None,
                                                                    }),
                                                                    disabled: None,
                                                                },
                                                            }),
                                                        }
                                                    });
                                            }
                                            wire::Node::Overlay {
                                                key: format!("{}/@overlay:926", use_scope),
                                                padding: (8.0) as f32,
                                                backdrop: wire::Rgba([
                                                    0.0 / 255.0,
                                                    0.0 / 255.0,
                                                    0.0 / 255.0,
                                                    0.000000,
                                                ]),
                                                align_x: wire::AlignX::Right,
                                                align_y: wire::AlignY::Top,
                                                on_dismiss: Some(
                                                    ::ducktape_view_guest::slots::message(
                                                        Message::ClosePageMenu,
                                                    ),
                                                ),
                                                children,
                                            }
                                        });
                                    children
                                        .push({
                                            let mut children = vec![
                                                ::ducktape_view_guest::wire::Node::Space { width :
                                                Some(::ducktape_view_guest::wire::Length::Fill), height :
                                                Some(::ducktape_view_guest::wire::Length::Fill) }
                                            ];
                                            if self.page_delete_armed {
                                                children
                                                    .push(
                                                        self
                                                            .delete_dialog(
                                                                palette,
                                                                format!("{}/ConfirmDelete@1600", use_scope),
                                                            ),
                                                    );
                                            }
                                            wire::Node::Overlay {
                                                key: format!("{}/@overlay:961", use_scope),
                                                padding: (30.0) as f32,
                                                backdrop: palette.colors[85],
                                                align_x: wire::AlignX::Center,
                                                align_y: wire::AlignY::Center,
                                                on_dismiss: Some(
                                                    ::ducktape_view_guest::slots::message(
                                                        Message::DisarmPageDelete,
                                                    ),
                                                ),
                                                children,
                                            }
                                        });
                                    wire::Node::Stack {
                                        key: format!("{}/@layout:387", use_scope),
                                        width: Some(wire::Length::Fill),
                                        height: Some(wire::Length::Fill),
                                        padding: None,
                                        background: None,
                                        border: None,
                                        clip: true,
                                        under: 0u32,
                                        children,
                                    }
                                });
                            wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:154", use_scope),
                                wrap: None,
                                axis: wire::Axis::Column,
                                spacing: None,
                                padding: None,
                                width: Some(wire::Length::Fill),
                                height: Some(wire::Length::Fill),
                                align: None,
                                background: None,
                                border: None,
                                children,
                            }
                        });
                    wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:153", use_scope),
                        wrap: None,
                        axis: wire::Axis::Row,
                        spacing: None,
                        padding: None,
                        width: Some(wire::Length::Fill),
                        height: Some(wire::Length::Fill),
                        align: None,
                        background: None,
                        border: None,
                        children,
                    }
                });
            wire::Node::Linear {
                max_width: None,
                clip: false,
                key: format!("{}/@layout:33", use_scope),
                wrap: None,
                axis: wire::Axis::Row,
                spacing: None,
                padding: None,
                width: Some(wire::Length::Fill),
                height: Some(wire::Length::Fill),
                align: None,
                background: None,
                border: None,
                children,
            }
        }
    }
}
