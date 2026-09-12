use super::*;
impl super::FilesView {
    pub(crate) fn view(&self) -> ::ducktape_view_guest::wire::Node {
        let palette = self.palette();
        {
            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
            if *self.derived_draft_parked() {
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
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:384", "FilesView"),
                        size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: None,
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("Unsaved changes to:".to_owned()).to_string(),
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
                        key: format!("{}/@text:385", "FilesView"),
                        size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: None,
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: (self.draft_path.to_owned()).to_string(),
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
                        key: format!("{}/@text:386", "FilesView"),
                        size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: None,
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("Return to this file to continue editing.".to_owned())
                            .to_string(),
                    });
                    children.push(::ducktape_view_guest::wire::Node::Button {
                        checked: None,
                        expanded: None,
                        description: None,
                        key: format!("{}/@button:387", "FilesView"),
                        content: ::ducktape_view_guest::wire::ButtonContent::Label(String::from(
                            "Discard unsaved changes",
                        )),
                        label: None,
                        on_press: Some(::ducktape_view_guest::slots::message(
                            Message::DiscardDraft(self.draft_id),
                        )),
                        width: None,
                        height: None,
                        padding: None,
                        style: ::ducktape_view_guest::wire::ButtonStyle {
                            preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                            recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                base: ::ducktape_view_guest::wire::Face {
                                    background: None,
                                    text: None,
                                    border: None,
                                },
                                hover_background: None,
                                pressed_background: None,
                                disabled_background: None,
                                disabled_text: None,
                                disabled_opacity: None,
                                focus_ring: None,
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
                            active: ::ducktape_view_guest::wire::Face::default(),
                            hovered: None,
                            pressed: None,
                            disabled: None,
                        },
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:383", "FilesView"),
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
            }
            if !(self.notice).is_empty() {
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
                    key: format!("{}/@text:389", "FilesView"),
                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: None,
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: (self.notice.to_owned()).to_string(),
                });
            }
            if self.omitted > 0 {
                children.push({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children.push({
                        let node_scope = format!("{}/display-omitted", "FilesView");
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
                            key: node_scope.clone(),
                            size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: None,
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: (self.omitted).to_string(),
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
                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                    "Geist".into(),
                                ),
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:393", "FilesView"),
                        size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: None,
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("rows are not shown.".to_owned()).to_string(),
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:391", "FilesView"),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((4.0) as f32),
                        padding: None,
                        width: None,
                        height: None,
                        align: None,
                        background: None,
                        border: None,
                        children: children,
                    }
                });
            }
            children.push(::ducktape_view_guest::wire::Node::Sensor {
                key: format!("{}/@sensor:394", "FilesView"),
                reset: None,
                on_show: Some(
                    ::ducktape_view_guest::slots::handler::<(f32, f32), Message>(Box::new({
                        let route =
                            move |size: (f64, f64)| Message::ViewportChanged(size.0, size.1);
                        move |sent: (f32, f32)| Some(route((f64::from(sent.0), f64::from(sent.1))))
                    })),
                ),
                on_resize: Some(
                    ::ducktape_view_guest::slots::handler::<(f32, f32), Message>(Box::new({
                        let route =
                            move |size: (f64, f64)| Message::ViewportChanged(size.0, size.1);
                        move |sent: (f32, f32)| Some(route((f64::from(sent.0), f64::from(sent.1))))
                    })),
                ),
                on_hide: None,
                anticipate: None,
                delay: None,
                child: Box::new({
                    let node_scope = format!("{}/root", "FilesView");
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
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[2]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
                        snap: None,
                        content: Box::new(self.render_files_screen_20(
                            palette,
                            format!("{}/FilesScreen@2164", node_scope),
                        )),
                    }
                }),
            });
            ::ducktape_view_guest::wire::Node::Linear {
                max_width: None,
                clip: false,
                key: format!("{}/@layout:381", "FilesView"),
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
