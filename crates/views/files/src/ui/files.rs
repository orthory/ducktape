use super::*;
impl super::FilesView {
    pub(super) fn render_files_screen_20(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        let _component_owner =
            ::ducktape_view_guest::slots::component("FilesScreen", &use_scope, false);
        {
            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
            children.push({
                let node_scope = format!("{}/crumb", use_scope);
                self.render_breadcrumb(
                    palette,
                    node_scope.clone(),
                    (move |event_0| Message::OpenDirAt(event_0)).clone(),
                )
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
                key: format!("{}/@container:47", use_scope),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (10.0) as f32,
                    right: (20.0) as f32,
                    bottom: (10.0) as f32,
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
                        key: format!("{}/@button:60", use_scope),
                        content: ::ducktape_view_guest::wire::ButtonContent::Child(Box::new(
                            ::ducktape_view_guest::wire::Node::Text {
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
                                key: format!("{}/@text:68", use_scope),
                                size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: None,
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("↑".to_owned()).to_string(),
                            },
                        )),
                        label: Some(String::from("Parent directory".to_owned())),
                        on_press: if (*self.derived_loading()) || (self.path == "/") {
                            None
                        } else {
                            Some(::ducktape_view_guest::slots::message((move |event_0| {
                                Message::OpenDirAt(event_0)
                            })(
                                crate::host::fs_parent(::std::convert::AsRef::as_ref(&(self.path)))
                                    .to_owned(),
                            )))
                        },
                        width: Some(::ducktape_view_guest::wire::Length::Fixed((26.0) as f32)),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((26.0) as f32)),
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
                                background: Some(palette.colors[3]),
                                text: Some(palette.colors[5]),
                                border: Some(::ducktape_view_guest::wire::Border {
                                    color: Some(palette.colors[63]),
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
                                text: None,
                                border: None,
                            }),
                            disabled: None,
                        },
                    });
                    children.push({
                        let node_scope = format!("{}/fs-new", use_scope);
                        ::ducktape_view_guest::wire::Node::Input {
                            options: ::ducktape_view_guest::wire::InputOptions {
                                label: ("New entry name".to_owned()).to_string(),
                                description: None,
                                disabled: (*self.derived_loading()),
                                padding: Some(::ducktape_view_guest::wire::Edges::all(
                                    (5.0) as f32,
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
                            placeholder: String::from("new name…".to_owned()),
                            value: (self.new_name).to_string(),
                            on_input: ::ducktape_view_guest::slots::handler::<String, Message>(
                                Box::new({
                                    let route = Message::NewNameChanged as fn(String) -> Message;
                                    move |sent: String| Some(route(sent))
                                }),
                            ),
                            on_submit: None,
                            width: Some(::ducktape_view_guest::wire::Length::Fixed((160.0) as f32)),
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
                                    background: Some(palette.colors[3]),
                                    border: Some(::ducktape_view_guest::wire::Border {
                                        color: None,
                                        width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                        radius: Some([
                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                        ]),
                                    }),
                                    value: Some(palette.colors[4]),
                                    placeholder: Some(palette.colors[72]),
                                    selection: Some({
                                        let mut color = palette.colors[4];
                                        color.0[3] = 0.180000;
                                        color
                                    }),
                                },
                                hovered: Some(::ducktape_view_guest::wire::InputFace {
                                    icon: None,
                                    background: Some(palette.colors[6]),
                                    border: Some(::ducktape_view_guest::wire::Border {
                                        color: Some(palette.colors[40]),
                                        width: None,
                                        radius: None,
                                    }),
                                    value: None,
                                    placeholder: None,
                                    selection: None,
                                }),
                                focused: None,
                                disabled: Some(::ducktape_view_guest::wire::InputFace {
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
                    children.push(::ducktape_view_guest::wire::Node::Button {
                        checked: None,
                        expanded: None,
                        description: None,
                        key: format!("{}/@button:85", use_scope),
                        content: ::ducktape_view_guest::wire::ButtonContent::Label(String::from(
                            "+ Folder",
                        )),
                        label: None,
                        on_press: if ((*self.derived_loading())
                            || ((self.new_name).trim().to_owned()).is_empty())
                            || (!(*self.derived_refusal()).is_empty())
                        {
                            None
                        } else {
                            Some(::ducktape_view_guest::slots::message(Message::MkdirSubmit))
                        },
                        width: None,
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges::all((5.0) as f32)),
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
                    children.push(::ducktape_view_guest::wire::Node::Button {
                        checked: None,
                        expanded: None,
                        description: None,
                        key: format!("{}/@button:90", use_scope),
                        content: ::ducktape_view_guest::wire::ButtonContent::Label(String::from(
                            "+ File",
                        )),
                        label: None,
                        on_press: if ((*self.derived_loading())
                            || ((self.new_name).trim().to_owned()).is_empty())
                            || (!(*self.derived_refusal()).is_empty())
                        {
                            None
                        } else {
                            Some(::ducktape_view_guest::slots::message(
                                Message::NewFileSubmit,
                            ))
                        },
                        width: None,
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges::all((5.0) as f32)),
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
                    children.push(::ducktape_view_guest::wire::Node::Space {
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                    });
                    if *self.derived_loading() {
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
                            key: format!("{}/@text:97", use_scope),
                            size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[70]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("Loading…".to_owned()).to_string(),
                        });
                    }
                    if !(self.preview_path).is_empty() {
                        children.push(::ducktape_view_guest::wire::Node::Button {
                            checked: None,
                            expanded: None,
                            description: None,
                            key: format!("{}/@button:105", use_scope),
                            content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                String::from("Delete object"),
                            ),
                            label: None,
                            on_press: if (*self.derived_loading())
                                || (!(self.delete_target).is_empty())
                            {
                                None
                            } else {
                                Some(::ducktape_view_guest::slots::message((move |event_0| {
                                    Message::ArmDeleteAt(event_0)
                                })(
                                    self.preview_path.to_owned(),
                                )))
                            },
                            width: None,
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges::all((5.0) as f32)),
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
                                        color: Some(palette.colors[63]),
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
                                    background: Some(palette.colors[65]),
                                    text: Some(palette.colors[4]),
                                    border: Some(::ducktape_view_guest::wire::Border {
                                        color: Some(palette.colors[64]),
                                        width: None,
                                        radius: None,
                                    }),
                                }),
                                pressed: Some(::ducktape_view_guest::wire::Face {
                                    background: Some(palette.colors[65]),
                                    text: None,
                                    border: None,
                                }),
                                disabled: None,
                            },
                        });
                    }
                    children.push({
                        let mut children = vec![::ducktape_view_guest::wire::Node::Space {
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(::ducktape_view_guest::wire::Length::Fill),
                        }];
                        if !(self.delete_target).is_empty() {
                            children.push(
                                self.render_confirm_delete_2(
                                    palette,
                                    format!("{}/ConfirmDelete@1236", use_scope),
                                    use_scope.clone(),
                                    (move |event_0| Message::ArmDeleteAt(event_0)).clone(),
                                    (move |event_0| Message::BeginEdit(event_0)).clone(),
                                    (move |event_0| Message::CancelEdit(event_0)).clone(),
                                    (move || Message::CloseDiffNow).clone(),
                                    (move || Message::DeleteSubmit).clone(),
                                    (move || Message::DisarmDeleteNow).clone(),
                                    (move || Message::MkdirSubmit).clone(),
                                    (move || Message::NewFileSubmit).clone(),
                                    (move |event_0| Message::OpenDirAt(event_0)).clone(),
                                    (move |event_0| Message::OpenFileAt(event_0)).clone(),
                                    (move |event_0| Message::SaveEdit(event_0)).clone(),
                                    (move |event_0| Message::ShowDiffOf(event_0)).clone(),
                                    (move |event_0| Message::OpenLinkAt(event_0)).clone(),
                                    (move |event_0, event_1| {
                                        Message::ObjectResized(event_0, event_1)
                                    })
                                    .clone(),
                                    (move |event_0, event_1| {
                                        Message::PreviewResized(event_0, event_1)
                                    })
                                    .clone(),
                                    (move |event_0, event_1| {
                                        Message::TreeResized(event_0, event_1)
                                    })
                                    .clone(),
                                ),
                            );
                        }
                        ::ducktape_view_guest::wire::Node::Overlay {
                            key: format!("{}/@overlay:113", use_scope),
                            padding: (30.0) as f32,
                            backdrop: palette.colors[85],
                            align_x: ::ducktape_view_guest::wire::AlignX::Center,
                            align_y: ::ducktape_view_guest::wire::AlignY::Center,
                            on_dismiss: Some(::ducktape_view_guest::slots::message(
                                Message::DisarmDeleteNow,
                            )),
                            children: children,
                        }
                    });
                    children.push(::ducktape_view_guest::wire::Node::Button {
                        checked: None,
                        expanded: Some(self.files_screen_states.get(&use_scope).map_or_else(
                            || self.files_screen_initial.history_open.clone(),
                            |state| state.history_open.clone(),
                        )),
                        description: None,
                        key: format!("{}/@button:134", use_scope),
                        content: ::ducktape_view_guest::wire::ButtonContent::Label(String::from(
                            "History",
                        )),
                        label: None,
                        on_press: Some(::ducktape_view_guest::slots::message(
                            Message::FilesScreenFsToggleHistory((use_scope).clone()),
                        )),
                        width: None,
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges::all((5.0) as f32)),
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
                                background: Some(palette.colors[3]),
                                text: Some(palette.colors[5]),
                                border: Some(::ducktape_view_guest::wire::Border {
                                    color: Some(palette.colors[63]),
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
                                text: None,
                                border: None,
                            }),
                            disabled: None,
                        },
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:54", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((8.0) as f32),
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: Some(::ducktape_view_guest::wire::Length::Fixed((28.0) as f32)),
                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            });
            if !(*self.derived_refusal()).is_empty() {
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
                    key: format!("{}/@container:147", use_scope),
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    padding: Some(::ducktape_view_guest::wire::Edges {
                        top: (0.0) as f32,
                        right: (20.0) as f32,
                        bottom: (10.0) as f32,
                        left: (20.0) as f32,
                    }),
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
                        key: format!("{}/@text:153", use_scope),
                        size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[70]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        align_x: None,
                        content: ((*self.derived_refusal()).to_owned()).to_string(),
                    }),
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
                key: format!("{}/@container:159", use_scope),
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
            children
                .push({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push({
                            let node_scope = format!("{}/tree-pane", use_scope);
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
                                            key: format!("{}/@container:185", use_scope),
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: Some(
                                                ::ducktape_view_guest::wire::Length::Fixed((50.0) as f32),
                                            ),
                                            padding: Some(::ducktape_view_guest::wire::Edges {
                                                top: (0.0) as f32,
                                                right: (14.0) as f32,
                                                bottom: (0.0) as f32,
                                                left: (14.0) as f32,
                                            }),
                                            align_x: None,
                                            align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
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
                                                        key: format!("{}/@text:193", use_scope),
                                                        size: Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                        color: Some(palette.colors[4]),
                                                        font: ::ducktape_view_guest::wire::Font {
                                                            monospace: false,
                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                        },
                                                        width: None,
                                                        align_x: None,
                                                        content: ("duckfs".to_owned()).to_string(),
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
                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                            }),
                                                        },
                                                        key: format!("{}/@text:199", use_scope),
                                                        size: Some(((9.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                        color: Some(palette.colors[72]),
                                                        font: ::ducktape_view_guest::wire::Font {
                                                            monospace: false,
                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                        },
                                                        width: None,
                                                        align_x: None,
                                                        content: ("content-addressed · replicated".to_owned())
                                                            .to_string(),
                                                    });
                                                ::ducktape_view_guest::wire::Node::Linear {
                                                    max_width: None,
                                                    clip: false,
                                                    key: format!("{}/@layout:192", use_scope),
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
                                            key: format!("{}/@container:205", use_scope),
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
                                            virtual_rows: true,
                                            key: format!("{}/@layout:211", use_scope),
                                            direction: ::ducktape_view_guest::wire::ScrollDirection::Vertical,
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: Some(::ducktape_view_guest::wire::Length::Fill),
                                            bar_hidden: true,
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
                                                if ((self.connected && self.listed)
                                                    && (self.directories).is_empty()) && (self.omitted == 0) 
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
                                                            key: format!("{}/@container:230", use_scope),
                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                            height: None,
                                                            padding: Some(::ducktape_view_guest::wire::Edges {
                                                                top: (6.0) as f32,
                                                                right: (12.0) as f32,
                                                                bottom: (6.0) as f32,
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
                                                                key: format!("{}/@text:237", use_scope),
                                                                size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                color: Some(palette.colors[72]),
                                                                font: ::ducktape_view_guest::wire::Font {
                                                                    monospace: false,
                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                },
                                                                width: None,
                                                                align_x: None,
                                                                content: ("No folders here.".to_owned()).to_string(),
                                                            }),
                                                        });
                                                }
                                                if self.connected && self.listed  {
                                                    children
                                                        .push({
                                                            let mut children: Vec<_> = Vec::new();
                                                            for entry in self.directories.iter() {
                                                                let key = entry.key;
                                                                let key_recon = format!("{}/key({})", use_scope, key);
                                                                let child: ::ducktape_view_guest::wire::Node = self
                                                                    .render_fs_tree_row_5(
                                                                        palette,
                                                                        format!("{}/FsTreeRow@1356", key_recon),
                                                                        (move |event_0| Message::OpenDirAt(event_0)).clone(),
                                                                        entry.clone(),
                                                                    );
                                                                children.push((key, child));
                                                            }
                                                            let (keys, children) = children
                                                                .into_iter()
                                                                .map(|(key, child)| (
                                                                    ::ducktape_view_guest::wire::ListKey::from(key),
                                                                    child,
                                                                ))
                                                                .unzip();
                                                            ::ducktape_view_guest::wire::Node::KeyedColumn {
                                                                key: format!("{}/@keyed:243", use_scope),
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
                                                                virtual_row: Some((27.0) as f32),
                                                            }
                                                        });
                                                }
                                                ::ducktape_view_guest::wire::Node::Linear {
                                                    max_width: None,
                                                    clip: false,
                                                    key: format!("{}/@layout:217", use_scope),
                                                    wrap: None,
                                                    axis: ::ducktape_view_guest::wire::Axis::Column,
                                                    spacing: Some((1.0) as f32),
                                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                                        top: (8.0) as f32,
                                                        right: (6.0) as f32,
                                                        bottom: (8.0) as f32,
                                                        left: (6.0) as f32,
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
                                        key: format!("{}/@layout:175", use_scope),
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
                            }
                        });
                    children
                        .push({
                            let node_scope = format!("{}/tree-resize", use_scope);
                            ::ducktape_view_guest::wire::Node::ResizeHandle {
                                key: node_scope.clone(),
                                on_press: None,
                                on_release: None,
                                on_drag: Some(
                                    ::ducktape_view_guest::slots::handler::<
                                        (f64, f64),
                                        Message,
                                    >(
                                        Box::new({
                                            let route = {
                                                let _route_state_scope_0 = (use_scope).clone();
                                                let route_callback = (move |event_0, event_1| Message::TreeResized(
                                                    event_0,
                                                    event_1,
                                                ))
                                                    .clone();
                                                move |delta: (f64, f64)| (route_callback)(delta.0, delta.1)
                                            };
                                            move |sent: (f64, f64)| Some(route(sent))
                                        }),
                                    ),
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
                                        width: Some(
                                            ::ducktape_view_guest::wire::Length::Fixed((10.0) as f32),
                                        ),
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
                                            key: format!("{}/@container:257", use_scope),
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
                                        }),
                                    }
                                }),
                            }
                        });
                    children
                        .push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            if !self.connected  {
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
                                        key: format!("{}/@container:268", use_scope),
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: Some(::ducktape_view_guest::wire::Length::Fill),
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
                                        content: Box::new(
                                            self
                                                .render_empty_state_6(
                                                    palette,
                                                    format!("{}/EmptyState@1385", use_scope),
                                                ),
                                        ),
                                    });
                            }
                            if self.connected
                                && self
                                    .files_screen_states
                                    .get(&use_scope)
                                    .map_or_else(
                                        || self.files_screen_initial.history_open.clone(),
                                        |state| state.history_open.clone(),
                                    ) 
                            {
                                children
                                    .push(::ducktape_view_guest::wire::Node::Scroll {
                                        on_scroll: None,
                                        virtual_rows: false,
                                        key: format!("{}/@layout:278", use_scope),
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
                                            if !(self.diff_from).is_empty()  {
                                                children
                                                    .push({
                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                        children
                                                            .push({
                                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                children
                                                                    .push(
                                                                        self
                                                                            .render_group_label_7(
                                                                                palette,
                                                                                format!("{}/GroupLabel@1407", use_scope),
                                                                            ),
                                                                    );
                                                                children
                                                                    .push(::ducktape_view_guest::wire::Node::Space {
                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                        height: None,
                                                                    });
                                                                children
                                                                    .push(::ducktape_view_guest::wire::Node::Button {
                                                                        checked: None,
                                                                        expanded: None,
                                                                        description: None,
                                                                        key: format!("{}/@button:297", use_scope),
                                                                        content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                                            String::from("Back"),
                                                                        ),
                                                                        label: None,
                                                                        on_press: Some(
                                                                            ::ducktape_view_guest::slots::message(Message::CloseDiffNow),
                                                                        ),
                                                                        width: None,
                                                                        height: None,
                                                                        padding: Some(
                                                                            ::ducktape_view_guest::wire::Edges::all((4.0) as f32),
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
                                                                                background: Some(palette.colors[3]),
                                                                                text: Some(palette.colors[5]),
                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                    color: Some(palette.colors[63]),
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
                                                                                text: Some(palette.colors[4]),
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
                                                                ::ducktape_view_guest::wire::Node::Linear {
                                                                    max_width: None,
                                                                    clip: false,
                                                                    key: format!("{}/@layout:290", use_scope),
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
                                                        if (self.diff).is_empty() && (self.diff_omitted == 0)  {
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
                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                        }),
                                                                    },
                                                                    key: format!("{}/@text:305", use_scope),
                                                                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                    color: Some(palette.colors[70]),
                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                        monospace: false,
                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                    },
                                                                    width: None,
                                                                    align_x: None,
                                                                    content: ("No differences.".to_owned()).to_string(),
                                                                });
                                                        }
                                                        for (index, entry) in self.diff.iter().enumerate() {
                                                            let for_scope = format!(
                                                                "{}/@for:1418({})", use_scope, index
                                                            );
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
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                }),
                                                                            },
                                                                            key: format!("{}/@text:312", for_scope),
                                                                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                            color: Some(palette.colors[71]),
                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                monospace: false,
                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                            },
                                                                            width: Some(
                                                                                ::ducktape_view_guest::wire::Length::Fixed((64.0) as f32),
                                                                            ),
                                                                            align_x: None,
                                                                            content: (entry.kind.to_owned()).to_string(),
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
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                }),
                                                                            },
                                                                            key: format!("{}/@text:319", for_scope),
                                                                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                            color: Some(palette.colors[4]),
                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                monospace: false,
                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                            },
                                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                            align_x: None,
                                                                            content: (entry.path.to_owned()).to_string(),
                                                                        });
                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                        max_width: None,
                                                                        clip: false,
                                                                        key: format!("{}/@layout:307", for_scope),
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
                                                        }
                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                            max_width: None,
                                                            clip: false,
                                                            key: format!("{}/@layout:289", use_scope),
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
                                                    });
                                            }
                                            if (self.diff_from).is_empty() {
                                                children
                                                    .push({
                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                        if !(self.history).is_empty()  {
                                                            children
                                                                .push(
                                                                    self
                                                                        .render_group_label_8(
                                                                            palette,
                                                                            format!("{}/GroupLabel@1444", use_scope),
                                                                        ),
                                                                );
                                                        }
                                                        if (self.history).is_empty() && (self.omitted == 0)  {
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
                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                        }),
                                                                    },
                                                                    key: format!("{}/@text:334", use_scope),
                                                                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                    color: Some(palette.colors[70]),
                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                        monospace: false,
                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                    },
                                                                    width: None,
                                                                    align_x: None,
                                                                    content: ("No snapshots yet.".to_owned()).to_string(),
                                                                });
                                                        }
                                                        for (index, snapshot) in self.history.iter().enumerate() {
                                                            let for_scope = format!(
                                                                "{}/@for:1447({})", use_scope, index
                                                            );
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
                                                                    key: format!("{}/@container:336", for_scope),
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
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                            }),
                                                                                        },
                                                                                        key: format!("{}/@text:350", for_scope),
                                                                                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                        color: Some(palette.colors[4]),
                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                            monospace: false,
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                        },
                                                                                        width: None,
                                                                                        align_x: None,
                                                                                        content: (snapshot.short_id.to_owned()).to_string(),
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
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                            }),
                                                                                        },
                                                                                        key: format!("{}/@text:356", for_scope),
                                                                                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                        color: Some(palette.colors[71]),
                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                            monospace: false,
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                        },
                                                                                        width: None,
                                                                                        align_x: None,
                                                                                        content: (crate::host::height_label(snapshot.height))
                                                                                            .to_string(),
                                                                                    });
                                                                                children
                                                                                    .push(::ducktape_view_guest::wire::Node::Space {
                                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                        height: None,
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
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                            }),
                                                                                        },
                                                                                        key: format!("{}/@text:363", for_scope),
                                                                                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                        color: Some(palette.colors[71]),
                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                            monospace: false,
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                        },
                                                                                        width: None,
                                                                                        align_x: None,
                                                                                        content: (snapshot.author.to_owned()).to_string(),
                                                                                    });
                                                                                children
                                                                                    .push(::ducktape_view_guest::wire::Node::Button {
                                                                                        checked: None,
                                                                                        expanded: None,
                                                                                        description: None,
                                                                                        key: format!("{}/@button:369", for_scope),
                                                                                        content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                                                            String::from("Diff"),
                                                                                        ),
                                                                                        label: None,
                                                                                        on_press: Some(
                                                                                            ::ducktape_view_guest::slots::message(
                                                                                                (move |event_0| Message::ShowDiffOf(
                                                                                                    event_0,
                                                                                                ))(snapshot.id.to_owned()),
                                                                                            ),
                                                                                        ),
                                                                                        width: None,
                                                                                        height: None,
                                                                                        padding: Some(
                                                                                            ::ducktape_view_guest::wire::Edges::all((3.0) as f32),
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
                                                                                                background: Some(palette.colors[3]),
                                                                                                text: Some(palette.colors[5]),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some(palette.colors[63]),
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
                                                                                                text: Some(palette.colors[4]),
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
                                                                                ::ducktape_view_guest::wire::Node::Linear {
                                                                                    max_width: None,
                                                                                    clip: false,
                                                                                    key: format!("{}/@layout:345", for_scope),
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
                                                                        if !(snapshot.message).is_empty()  {
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
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                        }),
                                                                                    },
                                                                                    key: format!("{}/@text:377", for_scope),
                                                                                    size: Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                    color: Some(palette.colors[4]),
                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                        monospace: false,
                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                    },
                                                                                    width: None,
                                                                                    align_x: None,
                                                                                    content: (snapshot.message.to_owned()).to_string(),
                                                                                });
                                                                        }
                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                            max_width: None,
                                                                            clip: false,
                                                                            key: format!("{}/@layout:344", for_scope),
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
                                                                    }),
                                                                });
                                                        }
                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                            max_width: None,
                                                            clip: false,
                                                            key: format!("{}/@layout:327", use_scope),
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
                                            }
                                            ::ducktape_view_guest::wire::Node::Linear {
                                                max_width: None,
                                                clip: false,
                                                key: format!("{}/@layout:283", use_scope),
                                                wrap: None,
                                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                                spacing: Some((8.0) as f32),
                                                padding: Some(::ducktape_view_guest::wire::Edges {
                                                    top: (18.0) as f32,
                                                    right: (18.0) as f32,
                                                    bottom: (18.0) as f32,
                                                    left: (18.0) as f32,
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
                            if self.connected
                                && (!self
                                    .files_screen_states
                                    .get(&use_scope)
                                    .map_or_else(
                                        || self.files_screen_initial.history_open.clone(),
                                        |state| state.history_open.clone(),
                                    )) 
                            {
                                children
                                    .push({
                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                        children
                                            .push(
                                                self
                                                    .render_object_table_header_9(
                                                        palette,
                                                        format!("{}/ObjectTableHeader@1492", use_scope),
                                                    ),
                                            );
                                        if (self.listed && (self.entries).is_empty())
                                            && (self.omitted == 0) 
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
                                                    key: format!("{}/@container:388", use_scope),
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
                                                    content: Box::new(
                                                        self
                                                            .render_empty_plate_10(
                                                                palette,
                                                                format!("{}/EmptyPlate@1501", use_scope),
                                                            ),
                                                    ),
                                                });
                                        }
                                        if self.listed && (!(self.entries).is_empty())  {
                                            children
                                                .push(::ducktape_view_guest::wire::Node::Scroll {
                                                    on_scroll: None,
                                                    virtual_rows: true,
                                                    key: format!("{}/@layout:391", use_scope),
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
                                                        let mut children: Vec<_> = Vec::new();
                                                        for entry in self.entries.iter() {
                                                            let key = entry.key;
                                                            let key_recon = format!("{}/key({})", use_scope, key);
                                                            let child: ::ducktape_view_guest::wire::Node = self
                                                                .render_object_row_14(
                                                                    palette,
                                                                    format!("{}/ObjectRow@1509", key_recon),
                                                                    (move |event_0| Message::OpenDirAt(event_0)).clone(),
                                                                    (move |event_0| Message::OpenFileAt(event_0)).clone(),
                                                                    entry.clone(),
                                                                    entry.path == self.preview_path ,
                                                                );
                                                            children.push((key, child));
                                                        }
                                                        let (keys, children) = children
                                                            .into_iter()
                                                            .map(|(key, child)| (
                                                                ::ducktape_view_guest::wire::ListKey::from(key),
                                                                child,
                                                            ))
                                                            .unzip();
                                                        ::ducktape_view_guest::wire::Node::KeyedColumn {
                                                            key: format!("{}/@keyed:396", use_scope),
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
                                                            virtual_row: Some((39.0) as f32),
                                                        }
                                                    }),
                                                });
                                        }
                                        if !(self.preview_path).is_empty()  {
                                            children
                                                .push({
                                                    let node_scope = format!("{}/preview-resize", use_scope);
                                                    ::ducktape_view_guest::wire::Node::ResizeHandle {
                                                        key: node_scope.clone(),
                                                        on_press: None,
                                                        on_release: None,
                                                        on_drag: Some(
                                                            ::ducktape_view_guest::slots::handler::<
                                                                (f64, f64),
                                                                Message,
                                                            >(
                                                                Box::new({
                                                                    let route = {
                                                                        let _route_state_scope_0 = (use_scope).clone();
                                                                        let route_callback = (move |event_0, event_1| Message::PreviewResized(
                                                                            event_0,
                                                                            event_1,
                                                                        ))
                                                                            .clone();
                                                                        move |delta: (f64, f64)| (route_callback)(delta.0, delta.1)
                                                                    };
                                                                    move |sent: (f64, f64)| Some(route(sent))
                                                                }),
                                                            ),
                                                        ),
                                                        cursor: Some(
                                                            ::ducktape_view_guest::wire::mouse::Cursor::ResizingVertically,
                                                        ),
                                                        content: Box::new({
                                                            let node_scope = format!("{}/preview-divider", node_scope);
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
                                                                height: Some(
                                                                    ::ducktape_view_guest::wire::Length::Fixed((10.0) as f32),
                                                                ),
                                                                padding: None,
                                                                align_x: None,
                                                                align_y: Some(::ducktape_view_guest::wire::AlignY::Top),
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
                                                                    key: format!("{}/@container:408", use_scope),
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
                                                                }),
                                                            }
                                                        }),
                                                    }
                                                });
                                            children
                                                .push({
                                                    let node_scope = format!("{}/preview-pane", use_scope);
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
                                                                key: format!("{}/@container:411", use_scope),
                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                height: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                padding: Some(::ducktape_view_guest::wire::Edges {
                                                                    top: (16.0) as f32,
                                                                    right: (16.0) as f32,
                                                                    bottom: (16.0) as f32,
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
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                        }),
                                                                                    },
                                                                                    key: format!("{}/@text:426", use_scope),
                                                                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                    color: Some(palette.colors[71]),
                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                        monospace: false,
                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                    },
                                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                    align_x: None,
                                                                                    content: (self.preview_path.to_owned()).to_string(),
                                                                                });
                                                                            if self.preview_truncated {
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
                                                                                        key: format!("{}/@text:434", use_scope),
                                                                                        size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                        color: Some(palette.colors[70]),
                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                            monospace: false,
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                        },
                                                                                        width: None,
                                                                                        align_x: None,
                                                                                        content: ("first 64 KiB".to_owned()).to_string(),
                                                                                    });
                                                                            }
                                                                            if self.preview_clipped && (!(*self.derived_draft_here())) 
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
                                                                                        key: format!("{}/@text:440", use_scope),
                                                                                        size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                        color: Some(palette.colors[70]),
                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                            monospace: false,
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                        },
                                                                                        width: None,
                                                                                        align_x: None,
                                                                                        content: ("Preview shortened for display.".to_owned())
                                                                                            .to_string(),
                                                                                    });
                                                                            }
                                                                            if (((!self.preview_binary) && (!self.preview_picture))
                                                                                && (!(*self.derived_draft_here())))
                                                                                && (!self.preview_truncated) 
                                                                            {
                                                                                children
                                                                                    .push(::ducktape_view_guest::wire::Node::Button {
                                                                                        checked: None,
                                                                                        expanded: None,
                                                                                        description: None,
                                                                                        key: format!("{}/@button:446", use_scope),
                                                                                        content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                                                            String::from("Edit"),
                                                                                        ),
                                                                                        label: None,
                                                                                        on_press: if ((*self.derived_loading())
                                                                                            || (((*self.derived_draft_parked())
                                                                                                || (self.preview_base).is_empty())
                                                                                                || (self.chain).is_empty())) 
                                                                                        {
                                                                                            None
                                                                                        } else {
                                                                                            Some(
                                                                                                ::ducktape_view_guest::slots::message(
                                                                                                    (move |event_0| Message::BeginEdit(
                                                                                                        event_0,
                                                                                                    ))((*self.derived_edit_context()).to_owned()),
                                                                                                ),
                                                                                            )
                                                                                        },
                                                                                        width: None,
                                                                                        height: None,
                                                                                        padding: Some(
                                                                                            ::ducktape_view_guest::wire::Edges::all((4.0) as f32),
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
                                                                                                background: Some(palette.colors[3]),
                                                                                                text: Some(palette.colors[5]),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some(palette.colors[63]),
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
                                                                                                text: Some(palette.colors[4]),
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
                                                                            if *self.derived_draft_here()  {
                                                                                children
                                                                                    .push(::ducktape_view_guest::wire::Node::Button {
                                                                                        checked: None,
                                                                                        expanded: None,
                                                                                        description: None,
                                                                                        key: format!("{}/@button:455", use_scope),
                                                                                        content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                                                            String::from("Cancel"),
                                                                                        ),
                                                                                        label: None,
                                                                                        on_press: Some(
                                                                                            ::ducktape_view_guest::slots::message(
                                                                                                (move |event_0| Message::CancelEdit(
                                                                                                    event_0,
                                                                                                ))((*self.derived_edit_context()).to_owned()),
                                                                                            ),
                                                                                        ),
                                                                                        width: None,
                                                                                        height: None,
                                                                                        padding: Some(
                                                                                            ::ducktape_view_guest::wire::Edges::all((4.0) as f32),
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
                                                                                                background: Some(palette.colors[3]),
                                                                                                text: Some(palette.colors[5]),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some(palette.colors[63]),
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
                                                                                                text: Some(palette.colors[4]),
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
                                                                            if *self.derived_draft_here()  {
                                                                                children
                                                                                    .push(::ducktape_view_guest::wire::Node::Button {
                                                                                        checked: None,
                                                                                        expanded: None,
                                                                                        description: None,
                                                                                        key: format!("{}/@button:463", use_scope),
                                                                                        content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                                                            String::from("Save"),
                                                                                        ),
                                                                                        label: None,
                                                                                        on_press: if (*self.derived_loading())  {
                                                                                            None
                                                                                        } else {
                                                                                            Some(
                                                                                                ::ducktape_view_guest::slots::message(
                                                                                                    (move |event_0| Message::SaveEdit(
                                                                                                        event_0,
                                                                                                    ))((*self.derived_edit_context()).to_owned()),
                                                                                                ),
                                                                                            )
                                                                                        },
                                                                                        width: None,
                                                                                        height: None,
                                                                                        padding: Some(
                                                                                            ::ducktape_view_guest::wire::Edges::all((4.0) as f32),
                                                                                        ),
                                                                                        style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                                            preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                                            recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                                                base: ::ducktape_view_guest::wire::Face {
                                                                                                    background: Some(palette.colors[7]),
                                                                                                    text: Some(palette.colors[9]),
                                                                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                                                                        color: None,
                                                                                                        width: None,
                                                                                                        radius: Some([5.0; 4]),
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
                                                                            }
                                                                            ::ducktape_view_guest::wire::Node::Linear {
                                                                                max_width: None,
                                                                                clip: false,
                                                                                key: format!("{}/@layout:421", use_scope),
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
                                                                        .push({
                                                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                            if *self.derived_draft_here()  {
                                                                                children
                                                                                    .push({
                                                                                        let node_scope = format!("{}/fs-editor", node_scope);
                                                                                        {
                                                                                            let editor = &(self.draft);
                                                                                            let (document, on_document) = editor
                                                                                                .document(
                                                                                                    ("app:draft").to_owned(),
                                                                                                    Message::EditDraft
                                                                                                        as fn(
                                                                                                            ::ducktape_view_guest::EditorDocumentUpdate,
                                                                                                        ) -> Message,
                                                                                                );
                                                                                            ::ducktape_view_guest::wire::Node::Editor {
                                                                                                options: Box::new(::ducktape_view_guest::wire::EditorOptions {
                                                                                                    binding: Some(
                                                                                                        Box::new(
                                                                                                            ::ducktape_view_guest::EditorBinding::<
                                                                                                                (),
                                                                                                            >::plain(Message::DraftTransaction),
                                                                                                        ),
                                                                                                    ),
                                                                                                    presentation: None,
                                                                                                    size: Some((12.0) as f32),
                                                                                                    padding: Some((6.6) as f32),
                                                                                                    line_height: Some(
                                                                                                        ::ducktape_view_guest::wire::LineHeight::Relative(
                                                                                                            ((1.3) as f32).max(f32::EPSILON).min(f32::MAX),
                                                                                                        ),
                                                                                                    ),
                                                                                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::Word),
                                                                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                            "Geist".into(),
                                                                                                        ),
                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                    }),
                                                                                                    style: ::ducktape_view_guest::wire::InputStyle {
                                                                                                        active: ::ducktape_view_guest::wire::InputFace {
                                                                                                            icon: None,
                                                                                                            background: Some(palette.colors[3]),
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
                                                                                                            value: Some(palette.colors[4]),
                                                                                                            placeholder: Some(palette.colors[72]),
                                                                                                            selection: Some({
                                                                                                                let mut color = palette.colors[4];
                                                                                                                color.0[3] = 0.180000;
                                                                                                                color
                                                                                                            }),
                                                                                                        },
                                                                                                        hovered: Some(::ducktape_view_guest::wire::InputFace {
                                                                                                            icon: None,
                                                                                                            background: Some(palette.colors[6]),
                                                                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                                                                color: Some(palette.colors[40]),
                                                                                                                width: None,
                                                                                                                radius: None,
                                                                                                            }),
                                                                                                            value: None,
                                                                                                            placeholder: None,
                                                                                                            selection: None,
                                                                                                        }),
                                                                                                        focused: Some(::ducktape_view_guest::wire::InputFace {
                                                                                                            icon: None,
                                                                                                            background: Some(palette.colors[6]),
                                                                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                                                                color: Some(palette.colors[42]),
                                                                                                                width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                                                radius: None,
                                                                                                            }),
                                                                                                            value: None,
                                                                                                            placeholder: None,
                                                                                                            selection: None,
                                                                                                        }),
                                                                                                        focused_hovered: None,
                                                                                                        disabled: None,
                                                                                                        ..::std::default::Default::default()
                                                                                                    },
                                                                                                }),
                                                                                                key: node_scope.clone(),
                                                                                                placeholder: "File contents…".to_owned(),
                                                                                                document: document,
                                                                                                on_document: on_document,
                                                                                                editable: !((*self.derived_loading())),
                                                                                                width: None,
                                                                                                height: None,
                                                                                                min_height: Some((200.0) as f32),
                                                                                                max_height: None,
                                                                                            }
                                                                                        }
                                                                                    });
                                                                            }
                                                                            if !(*self.derived_draft_here())  {
                                                                                children
                                                                                    .push(::ducktape_view_guest::wire::Node::Scroll {
                                                                                        on_scroll: None,
                                                                                        virtual_rows: false,
                                                                                        key: format!("{}/@layout:483", use_scope),
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
                                                                                            if self.preview_binary {
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
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                            }),
                                                                                                        },
                                                                                                        key: format!("{}/@text:490", use_scope),
                                                                                                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                        color: Some(palette.colors[71]),
                                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                                            monospace: false,
                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                        },
                                                                                                        width: None,
                                                                                                        align_x: None,
                                                                                                        content: (self.preview_display_text.to_owned()).to_string(),
                                                                                                    });
                                                                                            }
                                                                                            if self.preview_picture {
                                                                                                children
                                                                                                    .push({
                                                                                                        let node_scope = format!("{}/fs-picture", node_scope);
                                                                                                        ::ducktape_view_guest::wire::Node::Surface {
                                                                                                            key: node_scope.clone(),
                                                                                                            name: String::from("picture"),
                                                                                                            args: ::std::vec![
                                                                                                                { let surface_arg = & ("files".to_owned());
                                                                                                                ::ducktape_view_guest::wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                                                                                                                }, { let surface_arg = & (self.preview_path.to_owned());
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
                                                                                                        key: format!("{}/@text:505", use_scope),
                                                                                                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                        color: Some(palette.colors[71]),
                                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                                            monospace: false,
                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                        },
                                                                                                        width: None,
                                                                                                        align_x: None,
                                                                                                        content: (crate::host::picture_caption(
                                                                                                            self.preview_width,
                                                                                                            self.preview_height,
                                                                                                        ))
                                                                                                            .to_string(),
                                                                                                    });
                                                                                            }
                                                                                            if ((!self.preview_binary) && (!self.preview_picture))
                                                                                                && crate::host::markdown_path(
                                                                                                    ::std::convert::AsRef::as_ref(&(self.preview_path)),
                                                                                                ) 
                                                                                            {
                                                                                                children
                                                                                                    .push({
                                                                                                        let _lazy_context_246 = (use_scope).to_owned();
                                                                                                        let lazy_event_246_0 = (move |event_0| Message::OpenLinkAt(
                                                                                                            event_0,
                                                                                                        ))
                                                                                                            .clone();
                                                                                                        let _lazy_event_246_1 = (move |event_0| Message::OpenDirAt(
                                                                                                            event_0,
                                                                                                        ))
                                                                                                            .clone();
                                                                                                        let _lazy_event_246_2 = (move |event_0| Message::OpenFileAt(
                                                                                                            event_0,
                                                                                                        ))
                                                                                                            .clone();
                                                                                                        let _lazy_event_246_3 = (move || Message::MkdirSubmit)
                                                                                                            .clone();
                                                                                                        let _lazy_event_246_4 = (move || Message::NewFileSubmit)
                                                                                                            .clone();
                                                                                                        let _lazy_event_246_5 = (move |event_0| Message::ArmDeleteAt(
                                                                                                            event_0,
                                                                                                        ))
                                                                                                            .clone();
                                                                                                        let _lazy_event_246_6 = (move || Message::DisarmDeleteNow)
                                                                                                            .clone();
                                                                                                        let _lazy_event_246_7 = (move || Message::DeleteSubmit)
                                                                                                            .clone();
                                                                                                        let _lazy_event_246_8 = (move || Message::CloseDiffNow)
                                                                                                            .clone();
                                                                                                        let _lazy_event_246_9 = (move |event_0| Message::ShowDiffOf(
                                                                                                            event_0,
                                                                                                        ))
                                                                                                            .clone();
                                                                                                        let _lazy_event_246_10 = (move |event_0| Message::BeginEdit(
                                                                                                            event_0,
                                                                                                        ))
                                                                                                            .clone();
                                                                                                        let _lazy_event_246_11 = (move |event_0| Message::CancelEdit(
                                                                                                            event_0,
                                                                                                        ))
                                                                                                            .clone();
                                                                                                        let _lazy_event_246_12 = (move |event_0| Message::SaveEdit(
                                                                                                            event_0,
                                                                                                        ))
                                                                                                            .clone();
                                                                                                        let _lazy_event_246_13 = (move |event_0, event_1| Message::TreeResized(
                                                                                                            event_0,
                                                                                                            event_1,
                                                                                                        ))
                                                                                                            .clone();
                                                                                                        let _lazy_event_246_14 = (move |event_0, event_1| Message::PreviewResized(
                                                                                                            event_0,
                                                                                                            event_1,
                                                                                                        ))
                                                                                                            .clone();
                                                                                                        let _lazy_event_246_15 = (move |event_0, event_1| Message::ObjectResized(
                                                                                                            event_0,
                                                                                                            event_1,
                                                                                                        ))
                                                                                                            .clone();
                                                                                                        {
                                                                                                            let lazy_key = format!("{}/@lazy:511", use_scope);
                                                                                                            ::ducktape_view_guest::memo_lazy(
                                                                                                                (
                                                                                                                    self.preview_display_text.to_owned(),
                                                                                                                    self.preview_path.to_owned(),
                                                                                                                    self.dark,
                                                                                                                    self.preview_text_revision,
                                                                                                                    (node_scope).to_owned(),
                                                                                                                    palette.name,
                                                                                                                ),
                                                                                                                move |dependency| {
                                                                                                                    let _preview_text: String = dependency.0.clone();
                                                                                                                    let _preview_path: String = dependency.1.clone();
                                                                                                                    let dark: bool = dependency.2.clone();
                                                                                                                    let lazy_scope = dependency.4.clone();
                                                                                                                    let cached_doc: String = self
                                                                                                                        .preview_display_text
                                                                                                                        .to_owned();
                                                                                                                    {
                                                                                                                        let node_scope = format!("{}/fs-markdown", lazy_scope);
                                                                                                                        ::ducktape_view_guest::wire::Node::Surface {
                                                                                                                            key: node_scope.clone(),
                                                                                                                            name: String::from("agent_markdown"),
                                                                                                                            args: ::std::vec![
                                                                                                                                { let surface_arg = & (cached_doc.to_owned());
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
                                                                                                                                            let route_callback = (lazy_event_246_0).clone();
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
                                                                                                                246u64,
                                                                                                                &(use_scope),
                                                                                                                lazy_key,
                                                                                                            )
                                                                                                        }
                                                                                                    });
                                                                                            }
                                                                                            if ((!self.preview_binary) && (!self.preview_picture))
                                                                                                && (!crate::host::markdown_path(
                                                                                                    ::std::convert::AsRef::as_ref(&(self.preview_path)),
                                                                                                )) 
                                                                                            {
                                                                                                children
                                                                                                    .push({
                                                                                                        let _lazy_context_249 = (use_scope).to_owned();
                                                                                                        let _lazy_event_249_0 = (move |event_0| Message::OpenLinkAt(
                                                                                                            event_0,
                                                                                                        ))
                                                                                                            .clone();
                                                                                                        let _lazy_event_249_1 = (move |event_0| Message::OpenDirAt(
                                                                                                            event_0,
                                                                                                        ))
                                                                                                            .clone();
                                                                                                        let _lazy_event_249_2 = (move |event_0| Message::OpenFileAt(
                                                                                                            event_0,
                                                                                                        ))
                                                                                                            .clone();
                                                                                                        let _lazy_event_249_3 = (move || Message::MkdirSubmit)
                                                                                                            .clone();
                                                                                                        let _lazy_event_249_4 = (move || Message::NewFileSubmit)
                                                                                                            .clone();
                                                                                                        let _lazy_event_249_5 = (move |event_0| Message::ArmDeleteAt(
                                                                                                            event_0,
                                                                                                        ))
                                                                                                            .clone();
                                                                                                        let _lazy_event_249_6 = (move || Message::DisarmDeleteNow)
                                                                                                            .clone();
                                                                                                        let _lazy_event_249_7 = (move || Message::DeleteSubmit)
                                                                                                            .clone();
                                                                                                        let _lazy_event_249_8 = (move || Message::CloseDiffNow)
                                                                                                            .clone();
                                                                                                        let _lazy_event_249_9 = (move |event_0| Message::ShowDiffOf(
                                                                                                            event_0,
                                                                                                        ))
                                                                                                            .clone();
                                                                                                        let _lazy_event_249_10 = (move |event_0| Message::BeginEdit(
                                                                                                            event_0,
                                                                                                        ))
                                                                                                            .clone();
                                                                                                        let _lazy_event_249_11 = (move |event_0| Message::CancelEdit(
                                                                                                            event_0,
                                                                                                        ))
                                                                                                            .clone();
                                                                                                        let _lazy_event_249_12 = (move |event_0| Message::SaveEdit(
                                                                                                            event_0,
                                                                                                        ))
                                                                                                            .clone();
                                                                                                        let _lazy_event_249_13 = (move |event_0, event_1| Message::TreeResized(
                                                                                                            event_0,
                                                                                                            event_1,
                                                                                                        ))
                                                                                                            .clone();
                                                                                                        let _lazy_event_249_14 = (move |event_0, event_1| Message::PreviewResized(
                                                                                                            event_0,
                                                                                                            event_1,
                                                                                                        ))
                                                                                                            .clone();
                                                                                                        let _lazy_event_249_15 = (move |event_0, event_1| Message::ObjectResized(
                                                                                                            event_0,
                                                                                                            event_1,
                                                                                                        ))
                                                                                                            .clone();
                                                                                                        {
                                                                                                            let lazy_key = format!("{}/@lazy:514", use_scope);
                                                                                                            ::ducktape_view_guest::memo_lazy(
                                                                                                                (
                                                                                                                    self.preview_display_text.to_owned(),
                                                                                                                    self.preview_path.to_owned(),
                                                                                                                    self.dark,
                                                                                                                    self.preview_text_revision,
                                                                                                                    (node_scope).to_owned(),
                                                                                                                    palette.name,
                                                                                                                ),
                                                                                                                move |dependency| {
                                                                                                                    let _preview_text: String = dependency.0.clone();
                                                                                                                    let preview_path: String = dependency.1.clone();
                                                                                                                    let dark: bool = dependency.2.clone();
                                                                                                                    let lazy_scope = dependency.4.clone();
                                                                                                                    let cached_source: String = self
                                                                                                                        .preview_display_text
                                                                                                                        .to_owned();
                                                                                                                    {
                                                                                                                        let node_scope = format!("{}/fs-code", lazy_scope);
                                                                                                                        ::ducktape_view_guest::wire::Node::Surface {
                                                                                                                            key: node_scope.clone(),
                                                                                                                            name: String::from("forge_code"),
                                                                                                                            args: ::std::vec![
                                                                                                                                { let surface_arg = & (cached_source.to_owned());
                                                                                                                                ::ducktape_view_guest::wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                                                                                                                                }, { let surface_arg = & (preview_path.to_owned());
                                                                                                                                ::ducktape_view_guest::wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                                                                                                                                }, { let surface_arg = & (dark);
                                                                                                                                ::ducktape_view_guest::wire::SurfaceValue::Bool(*
                                                                                                                                (surface_arg)) }
                                                                                                                            ],
                                                                                                                            on_event: None,
                                                                                                                        }
                                                                                                                    }
                                                                                                                },
                                                                                                                249u64,
                                                                                                                &(use_scope),
                                                                                                                lazy_key,
                                                                                                            )
                                                                                                        }
                                                                                                    });
                                                                                            }
                                                                                            ::ducktape_view_guest::wire::Node::Linear {
                                                                                                max_width: None,
                                                                                                clip: false,
                                                                                                key: format!("{}/@layout:488", use_scope),
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
                                                                            ::ducktape_view_guest::wire::Node::Stack {
                                                                                key: format!("{}/@layout:468", use_scope),
                                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                height: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                padding: None,
                                                                                background: None,
                                                                                border: None,
                                                                                clip: false,
                                                                                under: 0u32,
                                                                                children: children,
                                                                            }
                                                                        });
                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                        max_width: None,
                                                                        clip: false,
                                                                        key: format!("{}/@layout:416", use_scope),
                                                                        wrap: None,
                                                                        axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                        spacing: Some((8.0) as f32),
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
                                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                                            spacing: None,
                                                            padding: None,
                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                            height: Some(
                                                                ::ducktape_view_guest::wire::Length::Fixed(
                                                                    (self.preview_pane_height) as f32,
                                                                ),
                                                            ),
                                                            align: None,
                                                            background: None,
                                                            border: None,
                                                            children: children,
                                                        }
                                                    }
                                                });
                                        }
                                        ::ducktape_view_guest::wire::Node::Linear {
                                            max_width: None,
                                            clip: false,
                                            key: format!("{}/@layout:379", use_scope),
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
                                key: format!("{}/@layout:259", use_scope),
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
                    if self.connected {
                        if !(self.preview_entry.path).is_empty()  {
                            children
                                .push({
                                    let node_scope = format!("{}/object-resize", use_scope);
                                    ::ducktape_view_guest::wire::Node::ResizeHandle {
                                        key: node_scope.clone(),
                                        on_press: None,
                                        on_release: None,
                                        on_drag: Some(
                                            ::ducktape_view_guest::slots::handler::<
                                                (f64, f64),
                                                Message,
                                            >(
                                                Box::new({
                                                    let route = {
                                                        let _route_state_scope_0 = (use_scope).clone();
                                                        let route_callback = (move |event_0, event_1| Message::ObjectResized(
                                                            event_0,
                                                            event_1,
                                                        ))
                                                            .clone();
                                                        move |delta: (f64, f64)| (route_callback)(delta.0, delta.1)
                                                    };
                                                    move |sent: (f64, f64)| Some(route(sent))
                                                }),
                                            ),
                                        ),
                                        cursor: Some(
                                            ::ducktape_view_guest::wire::mouse::Cursor::ResizingHorizontally,
                                        ),
                                        content: Box::new({
                                            let node_scope = format!("{}/object-divider", node_scope);
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
                                                    ::ducktape_view_guest::wire::Length::Fixed((10.0) as f32),
                                                ),
                                                height: Some(::ducktape_view_guest::wire::Length::Fill),
                                                padding: None,
                                                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                align_y: None,
                                                background: (Some(palette.colors[54]))
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
                                                    key: format!("{}/@container:525", use_scope),
                                                    width: Some(
                                                        ::ducktape_view_guest::wire::Length::Fixed((2.0) as f32),
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
                                                            ::ducktape_view_guest::wire::Length::Fixed((2.0) as f32),
                                                        ),
                                                        height: Some(
                                                            ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                                        ),
                                                    }),
                                                }),
                                            }
                                        }),
                                    }
                                });
                            children
                                .push({
                                    let node_scope = format!("{}/object-panel", use_scope);
                                    self.render_object_panel(palette, node_scope.clone())
                                });
                        }
                    }
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:165", use_scope),
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
                });
            ::ducktape_view_guest::wire::Node::Linear {
                max_width: None,
                clip: false,
                key: format!("{}/@layout:29", use_scope),
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
