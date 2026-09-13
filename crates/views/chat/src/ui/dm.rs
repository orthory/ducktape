use super::*;
impl super::ChatView {
    pub(super) fn render_dm_row_11(
        &self,
        use_scope: String,
        arg_0: crate::host::DmPeer,
        arg_2: bool,
    ) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = vec![
                self.principal_avatar(
                    format!("{}/PrincipalAvatar@1309", use_scope),
                    arg_0.initials.to_owned(),
                    arg_0.is_agent,
                ),
                wire::Node::Container {
                    shadow: Default::default(),
                    max_width: None,
                    max_height: None,
                    clip: true,
                    key: format!("{}/@container:108", use_scope),
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
                            format!("{}/@text:109", use_scope),
                            arg_0.name.to_owned().to_string(),
                        ),
                        wire::TextOptions {
                            wrapping: Some(wire::Wrapping::None),
                            ..Default::default()
                        },
                    )),
                },
            ];
            if arg_0.is_agent {
                children.push(native::padded(
                    native::container(
                        format!("{}/@container:132", use_scope),
                        native::text_options(
                            native::text(
                                format!("{}/@text:138", use_scope),
                                "AI".to_owned().to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                    ),
                    wire::Edges {
                        top: 1.0f32,
                        right: 4.0f32,
                        bottom: 1.0f32,
                        left: 4.0f32,
                    },
                ));
            }
            wire::Node::Linear {
                max_width: None,
                clip: false,
                key: node_scope.clone(),
                wrap: None,
                axis: wire::Axis::Row,
                spacing: Some(8.0f32),
                padding: None,
                width: Some(wire::Length::Fill),
                height: None,
                align: Some(wire::AlignX::Center),
                background: None,
                border: None,
                children: children,
            }
        }
    }
    pub(super) fn render_dm_row_12(
        &self,
        use_scope: String,
        arg_0: crate::host::DmPeer,
        arg_2: bool,
    ) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = vec![self.principal_avatar(
                format!("{}/PrincipalAvatar@1309", use_scope),
                arg_0.initials.to_owned(),
                arg_0.is_agent,
            )];
            if !false && arg_2 {
                children.push(wire::Node::Container {
                    shadow: Default::default(),
                    max_width: None,
                    max_height: None,
                    clip: true,
                    key: format!("{}/@container:115", use_scope),
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
                            format!("{}/@text:116", use_scope),
                            arg_0.name.to_owned().to_string(),
                        ),
                        wire::TextOptions {
                            wrapping: Some(wire::Wrapping::None),
                            ..Default::default()
                        },
                    )),
                });
            }
            if !false && !arg_2 {
                children.push(wire::Node::Container {
                    shadow: Default::default(),
                    max_width: None,
                    max_height: None,
                    clip: true,
                    key: format!("{}/@container:123", use_scope),
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
                            format!("{}/@text:124", use_scope),
                            arg_0.name.to_owned().to_string(),
                        ),
                        wire::TextOptions {
                            wrapping: Some(wire::Wrapping::None),
                            ..Default::default()
                        },
                    )),
                });
            }
            if arg_0.is_agent {
                children.push(native::padded(
                    native::container(
                        format!("{}/@container:132", use_scope),
                        native::text_options(
                            native::text(
                                format!("{}/@text:138", use_scope),
                                "AI".to_owned().to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                    ),
                    wire::Edges {
                        top: 1.0f32,
                        right: 4.0f32,
                        bottom: 1.0f32,
                        left: 4.0f32,
                    },
                ));
            }
            if arg_2 && !false {
                children.push(native::sized(
                    native::container(
                        format!("{}/@container:148", use_scope),
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
                key: node_scope.clone(),
                wrap: None,
                axis: wire::Axis::Row,
                spacing: Some(8.0f32),
                padding: None,
                width: Some(wire::Length::Fill),
                height: None,
                align: Some(wire::AlignX::Center),
                background: None,
                border: None,
                children: children,
            }
        }
    }
    pub(super) fn render_dm_button_13(
        &self,
        use_scope: String,
        cb_11: impl Fn(String) -> Message + Clone + 'static,
        arg_0: crate::host::DmPeer,
        arg_1: bool,
        arg_2: bool,
    ) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        native::padded(
            native::sized(
                native::container(node_scope.clone(), {
                    let mut children: Vec<wire::Node> = Vec::new();
                    if arg_1 {
                        children.push(wire::Node::Button {
                            checked: Some(arg_1),
                            expanded: None,
                            description: None,
                            key: format!("{}/@button:26", use_scope),
                            content: wire::ButtonContent::Child(Box::new({
                                let mut children: Vec<wire::Node> = vec![
                                    native::sized(
                                        native::container(
                                            format!("{}/@container:42", use_scope),
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
                                                format!("{}/@container:49", use_scope),
                                                self.render_dm_row_11(
                                                    format!("{}/DmRow@1265", use_scope),
                                                    arg_0.clone(),
                                                    arg_2,
                                                ),
                                            ),
                                            Some(wire::Length::Fill),
                                            None,
                                        ),
                                        wire::Edges {
                                            top: 6.0f32,
                                            right: 8.0f32,
                                            bottom: 6.0f32,
                                            left: 5.5f32,
                                        },
                                    ),
                                ];
                                native::spaced(
                                    native::row(format!("{}/@layout:41", use_scope), children),
                                    0.0f32,
                                )
                            })),
                            label: Some(String::from(arg_0.name.to_owned())),
                            on_press: if self.busy {
                                None
                            } else {
                                Some(::ducktape_view_guest::slots::message(cb_11(
                                    arg_0.key.to_owned(),
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
                            key: format!("{}/@button:65", use_scope),
                            content: wire::ButtonContent::Child(Box::new(native::padded(
                                native::sized(
                                    native::container(
                                        format!("{}/@container:73", use_scope),
                                        self.render_dm_row_12(
                                            format!("{}/DmRow@1289", use_scope),
                                            arg_0.clone(),
                                            arg_2,
                                        ),
                                    ),
                                    Some(wire::Length::Fill),
                                    None,
                                ),
                                wire::Edges {
                                    top: 6.0f32,
                                    right: 8.0f32,
                                    bottom: 6.0f32,
                                    left: 8.0f32,
                                },
                            ))),
                            label: Some(String::from(arg_0.name.to_owned())),
                            on_press: if self.busy {
                                None
                            } else {
                                Some(::ducktape_view_guest::slots::message(cb_11(
                                    arg_0.key.to_owned(),
                                )))
                            },
                            width: Some(wire::Length::Fill),
                            height: None,
                            padding: Some(wire::Edges::all(0.0f32)),
                            style: wire::ButtonStyle::default(),
                        });
                    }
                    native::sized(
                        native::column(format!("{}/@layout:24", use_scope), children),
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
    pub(super) fn render_dm_header_23(&self, use_scope: String) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = vec![
                self.active_dm_avatar(format!("{}/PrincipalAvatar@1388", use_scope)),
                native::text_options(
                    native::text(
                        format!("{}/@text:186", use_scope),
                        self.active_dm.name.to_owned().to_string(),
                    ),
                    wire::TextOptions {
                        wrapping: Some(wire::Wrapping::None),
                        ..Default::default()
                    },
                ),
            ];
            if self.active_dm.is_agent {
                children.push(native::padded(
                    native::container(
                        format!("{}/@container:193", use_scope),
                        native::text_options(
                            native::text(
                                format!("{}/@text:199", use_scope),
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
                ));
            }
            wire::Node::Linear {
                max_width: None,
                clip: false,
                key: node_scope.clone(),
                wrap: None,
                axis: wire::Axis::Row,
                spacing: Some(9.0f32),
                padding: None,
                width: None,
                height: None,
                align: Some(wire::AlignX::Center),
                background: None,
                border: None,
                children: children,
            }
        }
    }
}
