impl PagesEditorFixture {
    pub(crate) fn view(&self) -> ::ducktape_view_guest::wire::Node {
        let palette = self.palette();
        {
            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
            children.push({
                let node_scope = format!("{}/document", "PagesEditorFixture");
                {
                    let editor = &(self.document);
                    let (document, on_document) = editor.document(
                        ("app:document").to_owned(),
                        Message::DocumentUpdated
                            as fn(::ducktape_view_guest::EditorDocumentUpdate) -> Message,
                    );
                    ::ducktape_view_guest::wire::Node::Editor {
                        options: Box::new(::ducktape_view_guest::wire::EditorOptions {
                            binding: Some(Box::new(
                                crate::editor_binding::keys(
                                    self.history.clone(),
                                    self.menu.clone(),
                                )
                                .register(
                                    move |value| Message::Committed(value),
                                    Message::DocumentTransaction,
                                ),
                            )),
                            presentation: {
                                let presentation = crate::presentation::paint(
                                    editor.state_view(),
                                    self.menu.clone(),
                                    self.paint_dark,
                                    self.commented.clone(),
                                    true,
                                );
                                presentation
                                    .validate(editor.state_view().text)
                                    .expect("invalid editor presentation");
                                Some(Box::new(presentation))
                            },
                            size: None,
                            padding: None,
                            line_height: None,
                            wrapping: None,
                            font: None,
                            style: ::ducktape_view_guest::wire::InputStyle {
                                active: ::std::default::Default::default(),
                                hovered: None,
                                focused: None,
                                focused_hovered: None,
                                disabled: None,
                                ..::std::default::Default::default()
                            },
                        }),
                        key: node_scope.clone(),
                        placeholder: String::new(),
                        document: document,
                        on_document: on_document,
                        editable: true,
                        width: Some((640.0) as f32),
                        height: None,
                        min_height: Some((240.0) as f32),
                        max_height: Some((240.0) as f32),
                    }
                }
            });
            if (64000 > (((self.document).text()).len() as i64)) {
                children.push({
                    let node_scope = format!("{}/echo", "PagesEditorFixture");
                    ::ducktape_view_guest::wire::Node::Text {
                        options: ::ducktape_view_guest::wire::TextOptions {
                            height: None,
                            align_y: None,
                            line_height: None,
                            shaping: None,
                            wrapping: None,
                            tracking: 0.0f32,
                            font: None,
                        },
                        key: node_scope.clone(),
                        size: None,
                        color: None,
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ((self.document).text()).to_string(),
                    }
                });
            }
            children.push({
                let node_scope = format!("{}/formatting-notice", "PagesEditorFixture");
                ::ducktape_view_guest::wire::Node::Text {
                    options: ::ducktape_view_guest::wire::TextOptions {
                        height: None,
                        align_y: None,
                        line_height: None,
                        shaping: None,
                        wrapping: None,
                        tracking: 0.0f32,
                        font: None,
                    },
                    key: node_scope.clone(),
                    size: None,
                    color: None,
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: (self.formatting_notice.to_owned()).to_string(),
                }
            });
            children.push({
                let node_scope = format!("{}/load-error", "PagesEditorFixture");
                ::ducktape_view_guest::wire::Node::Text {
                    options: ::ducktape_view_guest::wire::TextOptions {
                        height: None,
                        align_y: None,
                        line_height: None,
                        shaping: None,
                        wrapping: None,
                        tracking: 0.0f32,
                        font: None,
                    },
                    key: node_scope.clone(),
                    size: None,
                    color: None,
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: (self.load_error.to_owned()).to_string(),
                }
            });
            children.push(::ducktape_view_guest::wire::Node::Button {
                checked: None,
                expanded: None,
                description: None,
                key: format!("{}/@button:81", "PagesEditorFixture"),
                content: ::ducktape_view_guest::wire::ButtonContent::Label(String::from(
                    "Load document",
                )),
                label: None,
                on_press: Some(::ducktape_view_guest::slots::message(Message::Load)),
                width: None,
                height: None,
                padding: None,
                style: ::ducktape_view_guest::wire::ButtonStyle {
                    preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                    recipe: None,
                    active: ::ducktape_view_guest::wire::Face::default(),
                    hovered: None,
                    pressed: None,
                    disabled: None,
                },
            });
            ::ducktape_view_guest::wire::Node::Linear {
                max_width: None,
                clip: false,
                key: format!("{}/@layout:69", "PagesEditorFixture"),
                wrap: None,
                axis: ::ducktape_view_guest::wire::Axis::Column,
                spacing: Some((8.0) as f32),
                padding: None,
                width: Some(::ducktape_view_guest::wire::Length::Fixed((640.0) as f32)),
                height: None,
                align: None,
                background: None,
                border: None,
                children: children,
            }
        }
    }
}
