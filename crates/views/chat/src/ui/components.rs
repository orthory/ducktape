use super::*;
impl super::ChatView {
    pub(super) fn render_channel_button_1(
        &self,
        use_scope: String,
        cb_10: impl Fn(String) -> Message + Clone + 'static,
        arg_0: crate::host::ChatChannel,
        arg_1: bool,
        arg_2: bool,
    ) -> wire::Node {
        native::padded(
            native::sized(
                native::container(format!("{}/@container:6", use_scope), {
                    let mut children: Vec<wire::Node> = Vec::new();
                    if arg_1 {
                        children.push(wire::Node::Button {
                            checked: Some(arg_1),
                            expanded: None,
                            description: None,
                            key: format!("{}/@button:13", use_scope),
                            content: wire::ButtonContent::Child(Box::new({
                                let mut children: Vec<wire::Node> = vec![
                                    native::sized(
                                        native::container(
                                            format!("{}/@container:30", use_scope),
                                            wire::Node::Space {
                                                width: Some(wire::Length::Fixed(1.0f32)),
                                                height: Some(wire::Length::Fixed(1.0f32)),
                                            },
                                        ),
                                        Some(wire::Length::Fixed(2.5f32)),
                                        Some(wire::Length::Fill),
                                    ),
                                    native::padded(
                                        native::sized(
                                            native::container(
                                                format!("{}/@container:37", use_scope),
                                                {
                                                    let mut children: Vec<wire::Node> = Vec::new();
                                                    if arg_0.members_only {
                                                        children.push(native::text_options(
                                                            native::text(
                                                                format!("{}/@text:50", use_scope),
                                                                "◆".to_owned().to_string(),
                                                            ),
                                                            wire::TextOptions {
                                                                wrapping: Some(wire::Wrapping::None)
                                                                    ..Default::default(),
                                                            },
                                                        ));
                                                    }
                                                    if !arg_0.members_only {
                                                        children.push(native::text_options(
                                                            native::text(
                                                                format!("{}/@text:56", use_scope),
                                                                "#".to_owned().to_string(),
                                                            ),
                                                            wire::TextOptions {
                                                                wrapping: Some(
                                                                    wire::Wrapping::None,
                                                                ),
                                                                ..Default::default()
                                                            },
                                                        ));
                                                    }
                                                    children.push(wire::Node::Container {
                                                        shadow: Default::default(),
                                                        max_width: None,
                                                        max_height: None,
                                                        clip: true,
                                                        key: format!("{}/@container:61", use_scope),
                                                        width: Some(wire::Length::Fill),
                                                        height: None,
                                                        padding: None,
                                                        align_x: None,
                                                        align_y: None,
                                                        background: None
                                                            .map(wire::Background::Color),
                                                        border: None,
                                                        snap: None,
                                                        content: Box::new(native::text_options(
                                                            native::text(
                                                                format!("{}/@text:62", use_scope),
                                                                arg_0.name.to_owned().to_string(),
                                                            ),
                                                            wire::TextOptions {
                                                                wrapping: Some(wire::Wrapping::None)
                                                                    ..Default::default(),
                                                            },
                                                        )),
                                                    });
                                                    if arg_0.archived {
                                                        children.push(native::text_options(
                                                            native::text(
                                                                format!("{}/@text:69", use_scope),
                                                                "archived".to_owned().to_string(),
                                                            ),
                                                            wire::TextOptions {
                                                                wrapping: Some(wire::Wrapping::None)
                                                                    ..Default::default(),
                                                            },
                                                        ));
                                                    }
                                                    if arg_0.huddle_count > 0 {
                                                        children.push(self.icon(
                                                            format!("{}/Icon@1489", use_scope),
                                                            "headphones",
                                                            12f32,
                                                            "@media:58",
                                                        ));
                                                    }
                                                    wire::Node::Linear {
                                                        max_width: None,
                                                        clip: false,
                                                        key: format!("{}/@layout:44", use_scope),
                                                        wrap: None,
                                                        axis: wire::Axis::Row,
                                                        spacing: Some(7.0f32),
                                                        padding: None,
                                                        width: Some(wire::Length::Fill),
                                                        height: None,
                                                        align: Some(wire::AlignX::Center),
                                                        background: None,
                                                        border: None,
                                                        children: children,
                                                    }
                                                },
                                            ),
                                            Some(wire::Length::Fill),
                                            None,
                                        ),
                                        wire::Edges {
                                            top: 7.0f32,
                                            right: 8.0f32,
                                            bottom: 7.0f32,
                                            left: 5.5f32,
                                        },
                                    ),
                                ];
                                native::spaced(
                                    native::row(format!("{}/@layout:29", use_scope), children),
                                    0.0f32,
                                )
                            })),
                            label: Some(String::from(arg_0.name.to_owned())),
                            on_press: if self.busy {
                                None
                            } else {
                                Some(::ducktape_view_guest::slots::message(cb_10(
                                    arg_0.id.to_owned(),
                                )))
                            },
                            width: Some(wire::Length::Fill),
                            height: None,
                            padding: Some(wire::Edges::all(0.0f32)),
                            style: wire::ButtonStyle::default(),
                        });
                    }
                    if !arg_1 {
                        children.push(wire::Node::Button {
                            checked: Some(arg_1),
                            expanded: None,
                            description: None,
                            key: format!("{}/@button:85", use_scope),
                            content: wire::ButtonContent::Child(Box::new(native::padded(
                                native::sized(
                                    native::container(format!("{}/@container:93", use_scope), {
                                        let mut children: Vec<wire::Node> = Vec::new();
                                        if arg_0.members_only {
                                            children.push(native::text_options(
                                                native::text(
                                                    format!("{}/@text:106", use_scope),
                                                    "◆".to_owned().to_string(),
                                                ),
                                                wire::TextOptions {
                                                    wrapping: Some(wire::Wrapping::None),
                                                    ..Default::default()
                                                },
                                            ));
                                        }
                                        if !arg_0.members_only {
                                            children.push(native::text_options(
                                                native::text(
                                                    format!("{}/@text:112", use_scope),
                                                    "#".to_owned().to_string(),
                                                ),
                                                wire::TextOptions {
                                                    wrapping: Some(wire::Wrapping::None),
                                                    ..Default::default()
                                                },
                                            ));
                                        }
                                        if arg_2 {
                                            children.push(wire::Node::Container {
                                                shadow: Default::default(),
                                                max_width: None,
                                                max_height: None,
                                                clip: true,
                                                key: format!("{}/@container:118", use_scope),
                                                width: Some(wire::Length::Fill),
                                                height: None,
                                                padding: None,
                                                align_x: None,
                                                align_y: None,
                                                background: None.map(wire::Background::Color),
                                                border: None,
                                                snap: None,
                                                content: Box::new(native::text_options(
                                                    native::text(
                                                        format!("{}/@text:119", use_scope),
                                                        arg_0.name.to_owned().to_string(),
                                                    ),
                                                    wire::TextOptions {
                                                        wrapping: Some(wire::Wrapping::None),
                                                        ..Default::default()
                                                    },
                                                )),
                                            });
                                        }
                                        if !arg_2 {
                                            children.push(wire::Node::Container {
                                                shadow: Default::default(),
                                                max_width: None,
                                                max_height: None,
                                                clip: true,
                                                key: format!("{}/@container:126", use_scope),
                                                width: Some(wire::Length::Fill),
                                                height: None,
                                                padding: None,
                                                align_x: None,
                                                align_y: None,
                                                background: None.map(wire::Background::Color),
                                                border: None,
                                                snap: None,
                                                content: Box::new(native::text_options(
                                                    native::text(
                                                        format!("{}/@text:127", use_scope),
                                                        arg_0.name.to_owned().to_string(),
                                                    ),
                                                    wire::TextOptions {
                                                        wrapping: Some(wire::Wrapping::None),
                                                        ..Default::default()
                                                    },
                                                )),
                                            });
                                        }
                                        if arg_0.archived {
                                            children.push(native::text_options(
                                                native::text(
                                                    format!("{}/@text:134", use_scope),
                                                    "archived".to_owned().to_string(),
                                                ),
                                                wire::TextOptions {
                                                    wrapping: Some(wire::Wrapping::None),
                                                    ..Default::default()
                                                },
                                            ));
                                        }
                                        if arg_0.huddle_count > 0 {
                                            children.push(self.icon(
                                                format!("{}/Icon@1554", use_scope),
                                                "headphones",
                                                12f32,
                                                "@media:58",
                                            ));
                                        }
                                        if arg_2 {
                                            children.push(native::sized(
                                                native::container(
                                                    format!("{}/@container:147", use_scope),
                                                    wire::Node::Space {
                                                        width: Some(wire::Length::Fixed(1.0f32)),
                                                        height: Some(wire::Length::Fixed(1.0f32)),
                                                    },
                                                ),
                                                Some(wire::Length::Fixed(7.0f32)),
                                                Some(wire::Length::Fixed(7.0f32)),
                                            ));
                                        }
                                        wire::Node::Linear {
                                            max_width: None,
                                            clip: false,
                                            key: format!("{}/@layout:100", use_scope),
                                            wrap: None,
                                            axis: wire::Axis::Row,
                                            spacing: Some(7.0f32),
                                            padding: None,
                                            width: Some(wire::Length::Fill),
                                            height: None,
                                            align: Some(wire::AlignX::Center),
                                            background: None,
                                            border: None,
                                            children: children,
                                        }
                                    }),
                                    Some(wire::Length::Fill),
                                    None,
                                ),
                                wire::Edges {
                                    top: 7.0f32,
                                    right: 8.0f32,
                                    bottom: 7.0f32,
                                    left: 8.0f32,
                                },
                            ))),
                            label: Some(String::from(arg_0.name.to_owned())),
                            on_press: if self.busy {
                                None
                            } else {
                                Some(::ducktape_view_guest::slots::message(cb_10(
                                    arg_0.id.to_owned(),
                                )))
                            },
                            width: Some(wire::Length::Fill),
                            height: None,
                            padding: Some(wire::Edges::all(0.0f32)),
                            style: wire::ButtonStyle::default(),
                        });
                    }
                    native::sized(
                        native::column(format!("{}/@layout:11", use_scope), children),
                        Some(wire::Length::Fill),
                        None,
                    )
                }),
                Some(wire::Length::Fill),
                None,
            ),
            wire::Edges {
                top: 0.0f32,
                right: 8.0f32,
                bottom: 0.0f32,
                left: 8.0f32,
            },
        )
    }
    pub(super) fn render_skeleton_row_35(&self, use_scope: String) -> wire::Node {
        let mut children: Vec<wire::Node> = vec![
            native::sized(
                native::container(
                    format!("{}/@container:1117", use_scope),
                    wire::Node::Space {
                        width: Some(wire::Length::Fixed(1.0f32)),
                        height: Some(wire::Length::Fixed(1.0f32)),
                    },
                ),
                Some(wire::Length::Fixed(30.0f32)),
                Some(wire::Length::Fixed(30.0f32)),
            ),
            {
                let mut children: Vec<wire::Node> = vec![
                    native::sized(
                        native::container(
                            format!("{}/@container:1129", use_scope),
                            wire::Node::Space {
                                width: Some(wire::Length::Fixed(1.0f32)),
                                height: Some(wire::Length::Fixed(1.0f32)),
                            },
                        ),
                        Some(wire::Length::Fixed(96.0f32)),
                        Some(wire::Length::Fixed(9.0f32)),
                    ),
                    wire::Node::Container {
                        shadow: Default::default(),
                        max_width: Some(420.0f32),
                        max_height: None,
                        clip: false,
                        key: format!("{}/@container:1136", use_scope),
                        width: Some(wire::Length::Fill),
                        height: Some(wire::Length::Fixed(9.0f32)),
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: None,
                        border: Some(wire::Border {
                            color: None,
                            width: None,
                            radius: Some([4.0f32, 4.0f32, 4.0f32, 4.0f32]),
                        }),
                        snap: None,
                        content: Box::new(wire::Node::Space {
                            width: Some(wire::Length::Fixed(1.0f32)),
                            height: Some(wire::Length::Fixed(1.0f32)),
                        }),
                    },
                ];
                native::spaced(
                    native::padded(
                        native::sized(
                            native::column(format!("{}/@layout:1124", use_scope), children),
                            Some(wire::Length::Fill),
                            None,
                        ),
                        wire::Edges {
                            top: 4.0f32,
                            right: 0.0f32,
                            bottom: 0.0f32,
                            left: 0.0f32,
                        },
                    ),
                    6.0f32,
                )
            },
        ];
        wire::Node::Linear {
            max_width: None,
            clip: false,
            key: format!("{}/@layout:1112", use_scope),
            wrap: None,
            axis: wire::Axis::Row,
            spacing: Some(11.0f32),
            padding: None,
            width: Some(wire::Length::Fill),
            height: None,
            align: Some(wire::AlignX::Left),
            background: None,
            border: None,
            children: children,
        }
    }
    pub(super) fn render_chat_search_result_41(
        &self,
        use_scope: String,
        cb_26: impl Fn(String, i64, i64) -> Message + Clone + 'static,
        arg_0: crate::host::ChatSearchHit,
    ) -> wire::Node {
        wire::Node::Button {
            checked: None,
            expanded: None,
            description: None,
            key: format!("{}/@button:893", use_scope),
            content: wire::ButtonContent::Child(Box::new({
                let mut children: Vec<wire::Node> = vec![
                    {
                        let mut children: Vec<wire::Node> = vec![
                            native::sized(
                                native::text(
                                    format!("{}/@text:905", use_scope),
                                    arg_0.author.to_owned().to_string(),
                                ),
                                Some(wire::Length::Fill),
                                None,
                            ),
                            native::text(
                                format!("{}/@text:911", use_scope),
                                arg_0.meta.to_owned().to_string(),
                            ),
                        ];
                        wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:900", use_scope),
                            wrap: None,
                            axis: wire::Axis::Row,
                            spacing: Some(7.0f32),
                            padding: None,
                            width: Some(wire::Length::Fill),
                            height: None,
                            align: Some(wire::AlignX::Center),
                            background: None,
                            border: None,
                            children: children,
                        }
                    },
                    native::sized(
                        native::text_options(
                            native::text(
                                format!("{}/@text:916", use_scope),
                                arg_0.text.to_owned().to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::WordOrGlyph)..Default::default(),
                            },
                        ),
                        Some(wire::Length::Fill),
                        None,
                    ),
                ];
                native::spaced(
                    native::sized(
                        native::column(format!("{}/@layout:899", use_scope), children),
                        Some(wire::Length::Fill),
                        None,
                    ),
                    3.0f32,
                )
            })),
            label: Some(String::from(arg_0.text.to_owned())),
            on_press: Some(::ducktape_view_guest::slots::message(cb_26(
                arg_0.channel_id.to_owned(),
                arg_0.root_seq,
                arg_0.seq,
            ))),
            width: Some(wire::Length::Fill),
            height: None,
            padding: Some(wire::Edges::all(8.0f32)),
            style: wire::ButtonStyle::default(),
        }
    }
    pub(super) fn render_composer_gate_44(&self, use_scope: String) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = Vec::new();
            if self.post_refusal == "channel_archived" {
                children.push(self.archived_notice(format!("{}/GateNote@2379", use_scope)));
            }
            if !(self.post_refusal == "channel_archived") && self.post_refusal == "members_only" {
                children.push(self.private_notice(format!("{}/GateNote@2384", use_scope)));
            }
            if !(self.post_refusal == "channel_archived" || self.post_refusal == "members_only") {
                children.push(wire::Node::Space {
                    width: Some(wire::Length::Fixed(1.0f32)),
                    height: Some(wire::Length::Fixed(1.0f32)),
                });
            }
            native::sized(
                native::column(node_scope.clone(), children),
                Some(wire::Length::Fill),
                None,
            )
        }
    }
    pub(super) fn render_chat_member_row_47(
        &self,
        use_scope: String,
        cb_35: impl Fn(String) -> Message + Clone + 'static,
        arg_0: crate::host::ChatMember,
    ) -> wire::Node {
        let mut children: Vec<wire::Node> = vec![
            native::sized(
                native::text_options(
                    native::text(
                        format!("{}/@text:168", use_scope),
                        arg_0.label.to_owned().to_string(),
                    ),
                    wire::TextOptions {
                        wrapping: Some(wire::Wrapping::None),
                        ..Default::default()
                    },
                ),
                Some(wire::Length::Fill),
                None,
            ),
            wire::Node::Button {
                checked: None,
                expanded: None,
                description: Some(String::from(arg_0.label.to_owned())),
                key: format!("{}/@button:175", use_scope),
                content: wire::ButtonContent::Child(Box::new(wire::Node::Container {
                    shadow: Default::default(),
                    max_width: None,
                    max_height: None,
                    clip: false,
                    key: format!("{}/@container:184", use_scope),
                    width: Some(wire::Length::Fill),
                    height: Some(wire::Length::Fill),
                    padding: None,
                    align_x: Some(wire::AlignX::Center),
                    align_y: Some(wire::AlignY::Center),
                    background: None.map(wire::Background::Color),
                    border: None,
                    snap: None,
                    content: Box::new(native::text_options(
                        native::text(
                            format!("{}/@text:195", use_scope),
                            "×".to_owned().to_string(),
                        ),
                        wire::TextOptions {
                            wrapping: Some(wire::Wrapping::None),
                            ..Default::default()
                        },
                    )),
                })),
                label: Some(String::from("Remove member".to_owned())),
                on_press: if self.busy {
                    None
                } else {
                    Some(::ducktape_view_guest::slots::message(cb_35(
                        arg_0.key.to_owned(),
                    )))
                },
                width: Some(wire::Length::Fixed(24.0f32)),
                height: Some(wire::Length::Fixed(24.0f32)),
                padding: Some(wire::Edges::all(0.0f32)),
                style: wire::ButtonStyle::default(),
            },
        ];
        wire::Node::Linear {
            max_width: None,
            clip: false,
            key: format!("{}/@layout:163", use_scope),
            wrap: None,
            axis: wire::Axis::Row,
            spacing: Some(6.0f32),
            padding: None,
            width: Some(wire::Length::Fill),
            height: None,
            align: Some(wire::AlignX::Center),
            background: None,
            border: None,
            children: children,
        }
    }
    pub(super) fn render_live_run_card_49(
        &self,
        use_scope: String,
        cb_8: impl Fn(String) -> Message + Clone + 'static,
        cb_30: impl Fn(String) -> Message + Clone + 'static,
        arg_0: crate::host::LiveRunHint,
    ) -> wire::Node {
        let mut children: Vec<wire::Node> = vec![
            {
                let mut children: Vec<wire::Node> = vec![
                    native::sized(
                        native::text(
                            format!("{}/@text:1159", use_scope),
                            arg_0.agent.to_owned().to_string(),
                        ),
                        Some(wire::Length::Fill),
                        None,
                    ),
                    native::padded(
                        native::container(
                            format!("{}/@container:1165", use_scope),
                            native::text_options(
                                native::text(
                                    format!("{}/@text:1171", use_scope),
                                    "AGENT".to_owned().to_string(),
                                ),
                                wire::TextOptions {
                                    wrapping: Some(wire::Wrapping::None),
                                    ..Default::default()
                                },
                            ),
                        ),
                        wire::Edges {
                            top: 2.0f32,
                            right: 5.0f32,
                            bottom: 2.0f32,
                            left: 5.0f32,
                        },
                    ),
                ];
                wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: format!("{}/@layout:1158", use_scope),
                    wrap: None,
                    axis: wire::Axis::Row,
                    spacing: Some(6.0f32),
                    padding: None,
                    width: Some(wire::Length::Fill),
                    height: None,
                    align: Some(wire::AlignX::Center),
                    background: None,
                    border: None,
                    children: children,
                }
            },
            native::sized(
                native::text(
                    format!("{}/@text:1177", use_scope),
                    arg_0.status.to_owned().to_string(),
                ),
                Some(wire::Length::Fill),
                None,
            ),
            {
                let mut children: Vec<wire::Node> = vec![
                    native::padded(
                        native::button(
                            format!("{}/@button:1183", use_scope),
                            String::from("View run"),
                            Some(::ducktape_view_guest::slots::message(cb_30(
                                arg_0.dispatch_id.to_owned(),
                            ))),
                            wire::ButtonPreset::Secondary,
                        ),
                        wire::Edges::all(4.0f32),
                    ),
                    native::padded(
                        native::button(
                            format!("{}/@button:1187", use_scope),
                            String::from("Stop"),
                            Some(::ducktape_view_guest::slots::message(cb_8(
                                arg_0.run_id.to_owned(),
                            ))),
                            wire::ButtonPreset::Secondary,
                        ),
                        wire::Edges::all(4.0f32),
                    ),
                ];
                wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: format!("{}/@layout:1182", use_scope),
                    wrap: None,
                    axis: wire::Axis::Row,
                    spacing: Some(6.0f32),
                    padding: None,
                    width: None,
                    height: None,
                    align: Some(wire::AlignX::Center),
                    background: None,
                    border: None,
                    children: children,
                }
            },
        ];
        native::spaced(
            native::padded(
                native::sized(
                    native::column(format!("{}/@layout:1151", use_scope), children),
                    Some(wire::Length::Fill),
                    None,
                ),
                wire::Edges {
                    top: 6.0f32,
                    right: 7.0f32,
                    bottom: 6.0f32,
                    left: 7.0f32,
                },
            ),
            5.0f32,
        )
    }
}
