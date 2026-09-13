use super::*;
impl super::ForgeView {
    pub(super) fn render_forge_org_header_3(&self, use_scope: String) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = vec![
                { let mut children : Vec < wire::Node > = vec![wire::Node::Container {
                shadow : Default::default(), max_width : None, max_height : None, clip :
                false, key : format!("{}/@container:46", use_scope), width :
                Some(wire::Length::Fixed(30.0f32)), height :
                Some(wire::Length::Fixed(30.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content : Box::new(self.icon(format!("{}/Icon@1222", use_scope),
                "branch", 16f32, "@media:76",),), },
                native::text_size(native::text_options(native::text(format!("{}/@text:59",
                use_scope), self.org.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },),
                16.0f32,), native::padded(native::container(format!("{}/@container:65",
                use_scope), native::text_options(native::text(format!("{}/@text:71",
                use_scope), "ORG".to_owned().to_string(),), wire::TextOptions { wrapping
                : Some(wire::Wrapping::None), ..Default::default() },),), wire::Edges {
                top : 2.0f32, right : 6.0f32, bottom : 2.0f32, left : 6.0f32, },),
                wire::Node::Space { width : Some(wire::Length::Fill), height : None, }];
                if self.list_phase == "ready" { children.push({ let mut children : Vec <
                wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:83", use_scope),
                crate ::host::plural(self.repos.len() as i64,
                ::std::convert::AsRef::as_ref(& "repository"),
                ::std::convert::AsRef::as_ref(& "repositories"),).to_string(),),
                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                ..Default::default() },)]; if ! self.tier.is_empty() { children
                .push(native::text_options(native::text(format!("{}/@text:93",
                use_scope), "·".to_owned().to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },),); children
                .push(native::text_options(native::text(format!("{}/@text:99",
                use_scope), self.tier.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },),); }
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:82", use_scope), wrap : None, axis : wire::Axis::Row,
                spacing : Some(5.0f32), padding : None, width : None, height : None,
                align : Some(wire::AlignX::Center), background : None, border : None,
                children : children, } }); } wire::Node::Linear { max_width : None, clip
                : false, key : format!("{}/@layout:41", use_scope), wrap : None, axis :
                wire::Axis::Row, spacing : Some(10.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }
            ];
            if !self.about.is_empty() {
                children
                    .push(wire::Node::Container {
                        shadow: Default::default(),
                        max_width: Some(680.0f32),
                        max_height: None,
                        clip: false,
                        key: format!("{}/@container:106", use_scope),
                        width: Some(wire::Length::Fill),
                        height: None,
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: None.map(wire::Background::Color),
                        border: None,
                        snap: None,
                        content: Box::new(
                            native::sized(
                                native::text(
                                    format!("{}/@text:107", use_scope),
                                    self.about.to_owned().to_string(),
                                ),
                                Some(wire::Length::Fill),
                                None,
                            ),
                        ),
                    });
            }
            native::spaced(
                native::sized(
                    native::column(node_scope.clone(), children),
                    Some(wire::Length::Fill),
                    None,
                ),
                7.0f32,
            )
        }
    }
    pub(super) fn render_repo_card_5(
        &self,
        use_scope: String,
        cb_11: impl Fn(String) -> Message + Clone + 'static,
        arg_0: crate::host::ForgeRepo,
    ) -> wire::Node {
        wire::Node::Button {
            checked: None,
            expanded: None,
            description: Some(String::from(arg_0.name.to_owned())),
            key: format!("{}/@button:120", use_scope),
            content: wire::ButtonContent::Child(
                Box::new(
                    native::padded(
                        native::sized(
                            native::container(
                                format!("{}/@container:127", use_scope),
                                {
                                    let mut children: Vec<wire::Node> = vec![
                                        self.icon(format!("{}/Icon@1307", use_scope), "branch",
                                        14f32, "@media:82",), wire::Node::Container { shadow :
                                        Default::default(), max_width : None, max_height : None,
                                        clip : true, key : format!("{}/@container:144", use_scope),
                                        width : Some(wire::Length::Fill), height : None, padding :
                                        None, align_x : None, align_y : None, background : None
                                        .map(wire::Background::Color), border : None, snap : None,
                                        content :
                                        Box::new(native::text_options(native::text(format!("{}/@text:145",
                                        use_scope), arg_0.name.to_owned().to_string(),),
                                        wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                                        ..Default::default() },),), },
                                        native::text_options(native::text(format!("{}/@text:151",
                                        use_scope), arg_0.head.to_owned().to_string(),),
                                        wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                                        ..Default::default() },)
                                    ];
                                    wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:134", use_scope),
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
                                },
                            ),
                            Some(wire::Length::Fill),
                            None,
                        ),
                        wire::Edges {
                            top: 11.0f32,
                            right: 14.0f32,
                            bottom: 11.0f32,
                            left: 14.0f32,
                        },
                    ),
                ),
            ),
            label: Some(String::from("Open repo".to_owned())),
            on_press: Some(
                ::ducktape_view_guest::slots::message(cb_11(arg_0.name.to_owned())),
            ),
            width: Some(wire::Length::Fill),
            height: None,
            padding: Some(wire::Edges::all(0.0f32)),
            style: wire::ButtonStyle::default(),
        }
    }
    pub(super) fn render_repo_crumb_7(&self, use_scope: String) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = vec![
                wire::Node::Container { shadow : Default::default(), max_width : None,
                max_height : None, clip : false, key : format!("{}/@container:173",
                use_scope), width : Some(wire::Length::Fixed(28.0f32)), height :
                Some(wire::Length::Fixed(28.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : Some(wire::Border { color : None, width :
                None, radius : Some([8.0f32, 8.0f32, 8.0f32, 8.0f32,]), }), snap : None,
                content : Box::new(self.icon(format!("{}/Icon@1349", use_scope),
                "branch", 15f32, "@media:76",),), },
                native::text_options(native::text(format!("{}/@text:186", use_scope),
                self.org.to_owned().to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:192", use_scope), "/"
                .to_owned().to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },)
            ];
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
    pub(super) fn render_back_to_list_9(
        &self,
        use_scope: String,
        cb_1: impl Fn() -> Message + Clone + 'static,
    ) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = Vec::new();
            if self.forge_item_kind == "pr" {
                children
                    .push(wire::Node::Button {
                        checked: None,
                        expanded: None,
                        description: None,
                        key: format!("{}/@button:662", use_scope),
                        content: wire::ButtonContent::Child(
                            Box::new(
                                native::padded(
                                    native::container(
                                        format!("{}/@container:667", use_scope),
                                        {
                                            let mut children: Vec<wire::Node> = vec![
                                                native::text_options(native::text(format!("{}/@text:674",
                                                use_scope), "‹".to_owned().to_string(),),
                                                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                                                ..Default::default() },),
                                                native::text_options(native::text(format!("{}/@text:679",
                                                use_scope), "Pull requests".to_owned().to_string(),),
                                                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                                                ..Default::default() },)
                                            ];
                                            wire::Node::Linear {
                                                max_width: None,
                                                clip: false,
                                                key: format!("{}/@layout:673", use_scope),
                                                wrap: None,
                                                axis: wire::Axis::Row,
                                                spacing: Some(5.0f32),
                                                padding: None,
                                                width: None,
                                                height: None,
                                                align: Some(wire::AlignX::Center),
                                                background: None,
                                                border: None,
                                                children: children,
                                            }
                                        },
                                    ),
                                    wire::Edges {
                                        top: 4.0f32,
                                        right: 9.0f32,
                                        bottom: 4.0f32,
                                        left: 7.0f32,
                                    },
                                ),
                            ),
                        ),
                        label: Some(String::from("Back to pull requests".to_owned())),
                        on_press: Some(::ducktape_view_guest::slots::message(cb_1())),
                        width: None,
                        height: None,
                        padding: Some(wire::Edges::all(0.0f32)),
                        style: wire::ButtonStyle::default(),
                    });
            }
            if !(self.forge_item_kind == "pr") && self.forge_item_kind == "issue" {
                children
                    .push(wire::Node::Button {
                        checked: None,
                        expanded: None,
                        description: None,
                        key: format!("{}/@button:688", use_scope),
                        content: wire::ButtonContent::Child(
                            Box::new(
                                native::padded(
                                    native::container(
                                        format!("{}/@container:693", use_scope),
                                        {
                                            let mut children: Vec<wire::Node> = vec![
                                                native::text_options(native::text(format!("{}/@text:700",
                                                use_scope), "‹".to_owned().to_string(),),
                                                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                                                ..Default::default() },),
                                                native::text_options(native::text(format!("{}/@text:705",
                                                use_scope), "Issues".to_owned().to_string(),),
                                                wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                                                ..Default::default() },)
                                            ];
                                            wire::Node::Linear {
                                                max_width: None,
                                                clip: false,
                                                key: format!("{}/@layout:699", use_scope),
                                                wrap: None,
                                                axis: wire::Axis::Row,
                                                spacing: Some(5.0f32),
                                                padding: None,
                                                width: None,
                                                height: None,
                                                align: Some(wire::AlignX::Center),
                                                background: None,
                                                border: None,
                                                children: children,
                                            }
                                        },
                                    ),
                                    wire::Edges {
                                        top: 4.0f32,
                                        right: 9.0f32,
                                        bottom: 4.0f32,
                                        left: 7.0f32,
                                    },
                                ),
                            ),
                        ),
                        label: Some(String::from("Back to issues".to_owned())),
                        on_press: Some(::ducktape_view_guest::slots::message(cb_1())),
                        width: None,
                        height: None,
                        padding: Some(wire::Edges::all(0.0f32)),
                        style: wire::ButtonStyle::default(),
                    });
            }
            native::column(node_scope.clone(), children)
        }
    }
    pub(super) fn render_forge_tree_dir_row_16(&self, use_scope: String) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = vec![
                self.icon(format!("{}/Icon@1475", use_scope), "chevron-down", 10f32,
                "@media:28",), self.icon(format!("{}/Icon@1486", use_scope), "folder",
                13f32, "@media:64",),
                native::sized(native::text_options(native::text(format!("{}/@text:323",
                use_scope), "/".to_owned().to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },),
                Some(wire::Length::Fill), None,)
            ];
            wire::Node::Linear {
                max_width: None,
                clip: false,
                key: node_scope.clone(),
                wrap: None,
                axis: wire::Axis::Row,
                spacing: Some(6.0f32),
                padding: Some(wire::Edges {
                    top: 5.0f32,
                    right: 14.0f32,
                    bottom: 5.0f32,
                    left: (10.0 + 0.0 * 15.0) as f32,
                }),
                width: Some(wire::Length::Fill),
                height: None,
                align: Some(wire::AlignX::Center),
                background: None,
                border: None,
                children: children,
            }
        }
    }
    pub(super) fn render_forge_tree_dir_row_17(
        &self,
        use_scope: String,
        arg_0: String,
    ) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = Vec::new();
            {
                children
                    .push(
                        self
                            .icon(
                                format!("{}/Icon@1481", use_scope),
                                "chevron-right",
                                10f32,
                                "@media:28",
                            ),
                    );
            }
            children
                .push(
                    self
                        .icon(
                            format!("{}/Icon@1486", use_scope),
                            "folder",
                            13f32,
                            "@media:64",
                        ),
                );
            children
                .push(
                    native::sized(
                        native::text_options(
                            native::text(
                                format!("{}/@text:323", use_scope),
                                arg_0.to_owned().to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                        Some(wire::Length::Fill),
                        None,
                    ),
                );
            wire::Node::Linear {
                max_width: None,
                clip: false,
                key: node_scope.clone(),
                wrap: None,
                axis: wire::Axis::Row,
                spacing: Some(6.0f32),
                padding: Some(wire::Edges {
                    top: 5.0f32,
                    right: 14.0f32,
                    bottom: 5.0f32,
                    left: (10.0 + 0.0 * 15.0) as f32,
                }),
                width: Some(wire::Length::Fill),
                height: None,
                align: Some(wire::AlignX::Center),
                background: None,
                border: None,
                children: children,
            }
        }
    }
    pub(super) fn render_forge_tree_file_face_19(
        &self,
        use_scope: String,
        arg_0: String,
    ) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = vec![
                self.icon(format!("{}/Icon@1529", use_scope), "file", 12f32,
                "@media:22",),
                native::sized(native::text_options(native::text(format!("{}/@text:367",
                use_scope), arg_0.to_owned().to_string(),), wire::TextOptions { wrapping
                : Some(wire::Wrapping::None), ..Default::default() },),
                Some(wire::Length::Fill), None,)
            ];
            wire::Node::Linear {
                max_width: None,
                clip: false,
                key: node_scope.clone(),
                wrap: None,
                axis: wire::Axis::Row,
                spacing: Some(6.0f32),
                padding: Some(wire::Edges {
                    top: 5.0f32,
                    right: 14.0f32,
                    bottom: 5.0f32,
                    left: (10.0 + 0.0 * 15.0) as f32,
                }),
                width: Some(wire::Length::Fill),
                height: None,
                align: Some(wire::AlignX::Center),
                background: None,
                border: None,
                children: children,
            }
        }
    }
    pub(super) fn render_forge_tree_file_face_20(
        &self,
        use_scope: String,
        arg_0: String,
    ) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = vec![
                self.icon(format!("{}/Icon@1529", use_scope), "file", 12f32,
                "@media:22",)
            ];
            {
                children
                    .push(
                        native::sized(
                            native::text_options(
                                native::text(
                                    format!("{}/@text:375", use_scope),
                                    arg_0.to_owned().to_string(),
                                ),
                                wire::TextOptions {
                                    wrapping: Some(wire::Wrapping::None),
                                    ..Default::default()
                                },
                            ),
                            Some(wire::Length::Fill),
                            None,
                        ),
                    );
            }
            wire::Node::Linear {
                max_width: None,
                clip: false,
                key: node_scope.clone(),
                wrap: None,
                axis: wire::Axis::Row,
                spacing: Some(6.0f32),
                padding: Some(wire::Edges {
                    top: 5.0f32,
                    right: 14.0f32,
                    bottom: 5.0f32,
                    left: (10.0 + 0.0 * 15.0) as f32,
                }),
                width: Some(wire::Length::Fill),
                height: None,
                align: Some(wire::AlignX::Center),
                background: None,
                border: None,
                children: children,
            }
        }
    }
    pub(super) fn render_forge_tree_file_row_21(
        &self,
        use_scope: String,
        arg_0: String,
        arg_2: bool,
    ) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = Vec::new();
            if arg_2 {
                children
                    .push(
                        native::sized(
                            native::container(
                                format!("{}/@container:336", use_scope),
                                self
                                    .render_forge_tree_file_face_19(
                                        format!("{}/ForgeTreeFileFace@1505", use_scope),
                                        arg_0.to_owned(),
                                    ),
                            ),
                            Some(wire::Length::Fill),
                            None,
                        ),
                    );
            }
            if !arg_2 {
                children
                    .push(
                        native::sized(
                            native::container(
                                format!("{}/@container:343", use_scope),
                                self
                                    .render_forge_tree_file_face_20(
                                        format!("{}/ForgeTreeFileFace@1512", use_scope),
                                        arg_0.to_owned(),
                                    ),
                            ),
                            Some(wire::Length::Fill),
                            None,
                        ),
                    );
            }
            native::sized(
                native::column(node_scope.clone(), children),
                Some(wire::Length::Fill),
                None,
            )
        }
    }
    pub(super) fn render_forge_code_header_22(&self, use_scope: String) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = vec![
                native::padded(native::sized(native::container(format!("{}/@container:393",
                use_scope), { let mut children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:407",
                use_scope), crate
                ::host::forge_file_header(::std::convert::AsRef::as_ref(& self
                .opened_dir), ::std::convert::AsRef::as_ref(& self.opened_rev),
                ::std::convert::AsRef::as_ref(& self.tree_path),
                ::std::convert::AsRef::as_ref(& self.tree_rev),
                ::std::convert::AsRef::as_ref(& self.file_path),).to_owned()
                .to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },),
                wire::Node::Container { shadow : Default::default(), max_width : None,
                max_height : None, clip : true, key : format!("{}/@container:416",
                use_scope), width : Some(wire::Length::Fill), height : None, padding :
                None, align_x : None, align_y : None, background : None
                .map(wire::Background::Color), border : None, snap : None, content :
                Box::new({ let mut children : Vec < wire::Node > = Vec::new(); if ! ""
                .is_empty() { children
                .push(native::text_options(native::text(format!("{}/@text:419",
                use_scope), "".to_owned().to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },),); }
                native::sized(native::column(format!("{}/@layout:417", use_scope),
                children,), Some(wire::Length::Fill), None,) }), }]; if ! "".is_empty() {
                children.push(native::text_options(native::text(format!("{}/@text:426",
                use_scope), "".to_owned().to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },),); } if ! ""
                .is_empty() && ! "".is_empty() { children
                .push(native::text_options(native::text(format!("{}/@text:433",
                use_scope), "·".to_owned().to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },),); } if ! ""
                .is_empty() { children
                .push(native::text_options(native::text(format!("{}/@text:440",
                use_scope), "".to_owned().to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },),); }
                wire::Node::Linear { max_width : None, clip : true, key :
                format!("{}/@layout:400", use_scope), wrap : None, axis :
                wire::Axis::Row, spacing : Some(10.0f32), padding : None, width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } },), Some(wire::Length::Fill),
                Some(wire::Length::Fixed(42.0f32)),), wire::Edges { top : 0.0f32, right :
                16.0f32, bottom : 0.0f32, left : 16.0f32, },),
                native::sized(native::container(format!("{}/@container:446", use_scope),
                wire::Node::Space { width : Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(1.0f32)), },), Some(wire::Length::Fill),
                Some(wire::Length::Fixed(1.0f32)),)
            ];
            native::sized(
                native::column(node_scope.clone(), children),
                Some(wire::Length::Fill),
                None,
            )
        }
    }
    pub(super) fn render_forge_code_empty_23(&self, use_scope: String) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        wire::Node::Container {
            shadow: Default::default(),
            max_width: None,
            max_height: None,
            clip: false,
            key: node_scope.clone(),
            width: Some(wire::Length::Fill),
            height: None,
            padding: Some(wire::Edges {
                top: 48.0f32,
                right: 48.0f32,
                bottom: 48.0f32,
                left: 48.0f32,
            }),
            align_x: Some(wire::AlignX::Center),
            align_y: None,
            background: None.map(wire::Background::Color),
            border: None,
            snap: None,
            content: Box::new({
                let mut children: Vec<wire::Node> = Vec::new();
                if !"".is_empty() {
                    children
                        .push(
                            native::text_options(
                                native::text(
                                    format!("{}/@text:475", use_scope),
                                    "".to_owned().to_string(),
                                ),
                                wire::TextOptions {
                                    wrapping: Some(wire::Wrapping::None),
                                    ..Default::default()
                                },
                            ),
                        );
                }
                children
                    .push(
                        native::text_options(
                            native::text(
                                format!("{}/@text:481", use_scope),
                                "Synced from the node · view only".to_owned().to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                    );
                if !"Loading repository files…".is_empty() {
                    children
                        .push(
                            native::text_options(
                                native::text(
                                    format!("{}/@text:487", use_scope),
                                    "Loading repository files…".to_owned().to_string(),
                                ),
                                wire::TextOptions {
                                    wrapping: Some(wire::Wrapping::None),
                                    ..Default::default()
                                },
                            ),
                        );
                }
                wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: format!("{}/@layout:473", use_scope),
                    wrap: None,
                    axis: wire::Axis::Column,
                    spacing: Some(9.0f32),
                    padding: None,
                    width: None,
                    height: None,
                    align: Some(wire::AlignX::Center),
                    background: None,
                    border: None,
                    children: children,
                }
            }),
        }
    }
    pub(super) fn render_forge_code_empty_24(&self, use_scope: String) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        wire::Node::Container {
            shadow: Default::default(),
            max_width: None,
            max_height: None,
            clip: false,
            key: node_scope.clone(),
            width: Some(wire::Length::Fill),
            height: None,
            padding: Some(wire::Edges {
                top: 48.0f32,
                right: 48.0f32,
                bottom: 48.0f32,
                left: 48.0f32,
            }),
            align_x: Some(wire::AlignX::Center),
            align_y: None,
            background: None.map(wire::Background::Color),
            border: None,
            snap: None,
            content: Box::new({
                let mut children: Vec<wire::Node> = Vec::new();
                if !self.file_path.is_empty() {
                    children
                        .push(
                            native::text_options(
                                native::text(
                                    format!("{}/@text:475", use_scope),
                                    self.file_path.to_owned().to_string(),
                                ),
                                wire::TextOptions {
                                    wrapping: Some(wire::Wrapping::None),
                                    ..Default::default()
                                },
                            ),
                        );
                }
                children
                    .push(
                        native::text_options(
                            native::text(
                                format!("{}/@text:481", use_scope),
                                "Synced from the node · view only".to_owned().to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                    );
                if !"Loading file…".is_empty() {
                    children
                        .push(
                            native::text_options(
                                native::text(
                                    format!("{}/@text:487", use_scope),
                                    "Loading file…".to_owned().to_string(),
                                ),
                                wire::TextOptions {
                                    wrapping: Some(wire::Wrapping::None),
                                    ..Default::default()
                                },
                            ),
                        );
                }
                wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: format!("{}/@layout:473", use_scope),
                    wrap: None,
                    axis: wire::Axis::Column,
                    spacing: Some(9.0f32),
                    padding: None,
                    width: None,
                    height: None,
                    align: Some(wire::AlignX::Center),
                    background: None,
                    border: None,
                    children: children,
                }
            }),
        }
    }
    pub(super) fn render_forge_code_empty_25(&self, use_scope: String) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        wire::Node::Container {
            shadow: Default::default(),
            max_width: None,
            max_height: None,
            clip: false,
            key: node_scope.clone(),
            width: Some(wire::Length::Fill),
            height: None,
            padding: Some(wire::Edges {
                top: 48.0f32,
                right: 48.0f32,
                bottom: 48.0f32,
                left: 48.0f32,
            }),
            align_x: Some(wire::AlignX::Center),
            align_y: None,
            background: None.map(wire::Background::Color),
            border: None,
            snap: None,
            content: Box::new({
                let mut children: Vec<wire::Node> = Vec::new();
                if !"".is_empty() {
                    children
                        .push(
                            native::text_options(
                                native::text(
                                    format!("{}/@text:475", use_scope),
                                    "".to_owned().to_string(),
                                ),
                                wire::TextOptions {
                                    wrapping: Some(wire::Wrapping::None),
                                    ..Default::default()
                                },
                            ),
                        );
                }
                children
                    .push(
                        native::text_options(
                            native::text(
                                format!("{}/@text:481", use_scope),
                                "Synced from the node · view only".to_owned().to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                    );
                if !"Could not load code. Pick Code to try again.".is_empty() {
                    children
                        .push(
                            native::text_options(
                                native::text(
                                    format!("{}/@text:487", use_scope),
                                    "Could not load code. Pick Code to try again."
                                        .to_owned()
                                        .to_string(),
                                ),
                                wire::TextOptions {
                                    wrapping: Some(wire::Wrapping::None),
                                    ..Default::default()
                                },
                            ),
                        );
                }
                wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: format!("{}/@layout:473", use_scope),
                    wrap: None,
                    axis: wire::Axis::Column,
                    spacing: Some(9.0f32),
                    padding: None,
                    width: None,
                    height: None,
                    align: Some(wire::AlignX::Center),
                    background: None,
                    border: None,
                    children: children,
                }
            }),
        }
    }
    pub(super) fn render_forge_code_empty_26(&self, use_scope: String) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        wire::Node::Container {
            shadow: Default::default(),
            max_width: None,
            max_height: None,
            clip: false,
            key: node_scope.clone(),
            width: Some(wire::Length::Fill),
            height: None,
            padding: Some(wire::Edges {
                top: 48.0f32,
                right: 48.0f32,
                bottom: 48.0f32,
                left: 48.0f32,
            }),
            align_x: Some(wire::AlignX::Center),
            align_y: None,
            background: None.map(wire::Background::Color),
            border: None,
            snap: None,
            content: Box::new({
                let mut children: Vec<wire::Node> = Vec::new();
                if !self.file_path.is_empty() {
                    children
                        .push(
                            native::text_options(
                                native::text(
                                    format!("{}/@text:475", use_scope),
                                    self.file_path.to_owned().to_string(),
                                ),
                                wire::TextOptions {
                                    wrapping: Some(wire::Wrapping::None),
                                    ..Default::default()
                                },
                            ),
                        );
                }
                children
                    .push(
                        native::text_options(
                            native::text(
                                format!("{}/@text:481", use_scope),
                                "Synced from the node · view only".to_owned().to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                    );
                if !self.file_note.is_empty() {
                    children
                        .push(
                            native::text_options(
                                native::text(
                                    format!("{}/@text:487", use_scope),
                                    self.file_note.to_owned().to_string(),
                                ),
                                wire::TextOptions {
                                    wrapping: Some(wire::Wrapping::None),
                                    ..Default::default()
                                },
                            ),
                        );
                }
                wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: format!("{}/@layout:473", use_scope),
                    wrap: None,
                    axis: wire::Axis::Column,
                    spacing: Some(9.0f32),
                    padding: None,
                    width: None,
                    height: None,
                    align: Some(wire::AlignX::Center),
                    background: None,
                    border: None,
                    children: children,
                }
            }),
        }
    }
    pub(super) fn render_forge_code_empty_27(&self, use_scope: String) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        wire::Node::Container {
            shadow: Default::default(),
            max_width: None,
            max_height: None,
            clip: false,
            key: node_scope.clone(),
            width: Some(wire::Length::Fill),
            height: None,
            padding: Some(wire::Edges {
                top: 48.0f32,
                right: 48.0f32,
                bottom: 48.0f32,
                left: 48.0f32,
            }),
            align_x: Some(wire::AlignX::Center),
            align_y: None,
            background: None.map(wire::Background::Color),
            border: None,
            snap: None,
            content: Box::new({
                let mut children: Vec<wire::Node> = Vec::new();
                if !"".is_empty() {
                    children
                        .push(
                            native::text_options(
                                native::text(
                                    format!("{}/@text:475", use_scope),
                                    "".to_owned().to_string(),
                                ),
                                wire::TextOptions {
                                    wrapping: Some(wire::Wrapping::None),
                                    ..Default::default()
                                },
                            ),
                        );
                }
                children
                    .push(
                        native::text_options(
                            native::text(
                                format!("{}/@text:481", use_scope),
                                "Synced from the node · view only".to_owned().to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                    );
                if !"Nothing is committed on this repository yet, so there is no file to read."
                    .is_empty()
                {
                    children
                        .push(
                            native::text_options(
                                native::text(
                                    format!("{}/@text:487", use_scope),
                                    "Nothing is committed on this repository yet, so there is no file to read."
                                        .to_owned()
                                        .to_string(),
                                ),
                                wire::TextOptions {
                                    wrapping: Some(wire::Wrapping::None),
                                    ..Default::default()
                                },
                            ),
                        );
                }
                wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: format!("{}/@layout:473", use_scope),
                    wrap: None,
                    axis: wire::Axis::Column,
                    spacing: Some(9.0f32),
                    padding: None,
                    width: None,
                    height: None,
                    align: Some(wire::AlignX::Center),
                    background: None,
                    border: None,
                    children: children,
                }
            }),
        }
    }
    pub(super) fn render_forge_code_empty_28(&self, use_scope: String) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        wire::Node::Container {
            shadow: Default::default(),
            max_width: None,
            max_height: None,
            clip: false,
            key: node_scope.clone(),
            width: Some(wire::Length::Fill),
            height: None,
            padding: Some(wire::Edges {
                top: 48.0f32,
                right: 48.0f32,
                bottom: 48.0f32,
                left: 48.0f32,
            }),
            align_x: Some(wire::AlignX::Center),
            align_y: None,
            background: None.map(wire::Background::Color),
            border: None,
            snap: None,
            content: Box::new({
                let mut children: Vec<wire::Node> = Vec::new();
                if !"".is_empty() {
                    children
                        .push(
                            native::text_options(
                                native::text(
                                    format!("{}/@text:475", use_scope),
                                    "".to_owned().to_string(),
                                ),
                                wire::TextOptions {
                                    wrapping: Some(wire::Wrapping::None),
                                    ..Default::default()
                                },
                            ),
                        );
                }
                children
                    .push(
                        native::text_options(
                            native::text(
                                format!("{}/@text:481", use_scope),
                                "Synced from the node · view only".to_owned().to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                    );
                if !"This commit has no files to read.".is_empty() {
                    children
                        .push(
                            native::text_options(
                                native::text(
                                    format!("{}/@text:487", use_scope),
                                    "This commit has no files to read.".to_owned().to_string(),
                                ),
                                wire::TextOptions {
                                    wrapping: Some(wire::Wrapping::None),
                                    ..Default::default()
                                },
                            ),
                        );
                }
                wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: format!("{}/@layout:473", use_scope),
                    wrap: None,
                    axis: wire::Axis::Column,
                    spacing: Some(9.0f32),
                    padding: None,
                    width: None,
                    height: None,
                    align: Some(wire::AlignX::Center),
                    background: None,
                    border: None,
                    children: children,
                }
            }),
        }
    }
    pub(super) fn render_forge_code_empty_29(&self, use_scope: String) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        wire::Node::Container {
            shadow: Default::default(),
            max_width: None,
            max_height: None,
            clip: false,
            key: node_scope.clone(),
            width: Some(wire::Length::Fill),
            height: None,
            padding: Some(wire::Edges {
                top: 48.0f32,
                right: 48.0f32,
                bottom: 48.0f32,
                left: 48.0f32,
            }),
            align_x: Some(wire::AlignX::Center),
            align_y: None,
            background: None.map(wire::Background::Color),
            border: None,
            snap: None,
            content: Box::new({
                let mut children: Vec<wire::Node> = Vec::new();
                if !"".is_empty() {
                    children
                        .push(
                            native::text_options(
                                native::text(
                                    format!("{}/@text:475", use_scope),
                                    "".to_owned().to_string(),
                                ),
                                wire::TextOptions {
                                    wrapping: Some(wire::Wrapping::None),
                                    ..Default::default()
                                },
                            ),
                        );
                }
                children
                    .push(
                        native::text_options(
                            native::text(
                                format!("{}/@text:481", use_scope),
                                "Synced from the node · view only".to_owned().to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                    );
                if !"This directory has entries outside the browser's display limits."
                    .is_empty()
                {
                    children
                        .push(
                            native::text_options(
                                native::text(
                                    format!("{}/@text:487", use_scope),
                                    "This directory has entries outside the browser's display limits."
                                        .to_owned()
                                        .to_string(),
                                ),
                                wire::TextOptions {
                                    wrapping: Some(wire::Wrapping::None),
                                    ..Default::default()
                                },
                            ),
                        );
                }
                wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: format!("{}/@layout:473", use_scope),
                    wrap: None,
                    axis: wire::Axis::Column,
                    spacing: Some(9.0f32),
                    padding: None,
                    width: None,
                    height: None,
                    align: Some(wire::AlignX::Center),
                    background: None,
                    border: None,
                    children: children,
                }
            }),
        }
    }
    pub(super) fn render_forge_code_empty_30(&self, use_scope: String) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        wire::Node::Container {
            shadow: Default::default(),
            max_width: None,
            max_height: None,
            clip: false,
            key: node_scope.clone(),
            width: Some(wire::Length::Fill),
            height: None,
            padding: Some(wire::Edges {
                top: 48.0f32,
                right: 48.0f32,
                bottom: 48.0f32,
                left: 48.0f32,
            }),
            align_x: Some(wire::AlignX::Center),
            align_y: None,
            background: None.map(wire::Background::Color),
            border: None,
            snap: None,
            content: Box::new({
                let mut children: Vec<wire::Node> = Vec::new();
                if !"".is_empty() {
                    children
                        .push(
                            native::text_options(
                                native::text(
                                    format!("{}/@text:475", use_scope),
                                    "".to_owned().to_string(),
                                ),
                                wire::TextOptions {
                                    wrapping: Some(wire::Wrapping::None),
                                    ..Default::default()
                                },
                            ),
                        );
                }
                children
                    .push(
                        native::text_options(
                            native::text(
                                format!("{}/@text:481", use_scope),
                                "Synced from the node · view only".to_owned().to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                    );
                if !"Pick a file from the tree to read it.".is_empty() {
                    children
                        .push(
                            native::text_options(
                                native::text(
                                    format!("{}/@text:487", use_scope),
                                    "Pick a file from the tree to read it."
                                        .to_owned()
                                        .to_string(),
                                ),
                                wire::TextOptions {
                                    wrapping: Some(wire::Wrapping::None),
                                    ..Default::default()
                                },
                            ),
                        );
                }
                wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: format!("{}/@layout:473", use_scope),
                    wrap: None,
                    axis: wire::Axis::Column,
                    spacing: Some(9.0f32),
                    padding: None,
                    width: None,
                    height: None,
                    align: Some(wire::AlignX::Center),
                    background: None,
                    border: None,
                    children: children,
                }
            }),
        }
    }
    pub(super) fn render_forge_code_empty_31(&self, use_scope: String) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        wire::Node::Container {
            shadow: Default::default(),
            max_width: None,
            max_height: None,
            clip: false,
            key: node_scope.clone(),
            width: Some(wire::Length::Fill),
            height: None,
            padding: Some(wire::Edges {
                top: 48.0f32,
                right: 48.0f32,
                bottom: 48.0f32,
                left: 48.0f32,
            }),
            align_x: Some(wire::AlignX::Center),
            align_y: None,
            background: None.map(wire::Background::Color),
            border: None,
            snap: None,
            content: Box::new({
                let mut children: Vec<wire::Node> = Vec::new();
                if !self.file_path.is_empty() {
                    children
                        .push(
                            native::text_options(
                                native::text(
                                    format!("{}/@text:475", use_scope),
                                    self.file_path.to_owned().to_string(),
                                ),
                                wire::TextOptions {
                                    wrapping: Some(wire::Wrapping::None),
                                    ..Default::default()
                                },
                            ),
                        );
                }
                children
                    .push(
                        native::text_options(
                            native::text(
                                format!("{}/@text:481", use_scope),
                                "Synced from the node · view only".to_owned().to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                    );
                if !crate::host::binary_note(
                        ::std::convert::AsRef::as_ref(&self.file_text),
                    )
                    .is_empty()
                {
                    children
                        .push(
                            native::text_options(
                                native::text(
                                    format!("{}/@text:487", use_scope),
                                    crate::host::binary_note(
                                            ::std::convert::AsRef::as_ref(&self.file_text),
                                        )
                                        .to_owned()
                                        .to_string(),
                                ),
                                wire::TextOptions {
                                    wrapping: Some(wire::Wrapping::None),
                                    ..Default::default()
                                },
                            ),
                        );
                }
                wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: format!("{}/@layout:473", use_scope),
                    wrap: None,
                    axis: wire::Axis::Column,
                    spacing: Some(9.0f32),
                    padding: None,
                    width: None,
                    height: None,
                    align: Some(wire::AlignX::Center),
                    background: None,
                    border: None,
                    children: children,
                }
            }),
        }
    }
    pub(super) fn render_forge_code_tab_32(
        &self,
        use_scope: String,
        ctx_0: String,
        cb_0: impl Fn(String) -> Message + Clone + 'static,
        cb_1: impl Fn(String) -> Message + Clone + 'static,
        cb_2: impl Fn(String) -> Message + Clone + 'static,
        cb_3: impl Fn(f64, f64) -> Message + Clone + 'static,
    ) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = vec![
                { let node_scope = format!("{}/tree-pane", node_scope);
                native::sized(native::container(node_scope.clone(), wire::Node::Scroll {
                on_scroll : None, virtual_rows : false, key : format!("{}/@layout:241",
                use_scope), direction : wire::ScrollDirection::Vertical, width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), bar_hidden :
                false, bar_width : None, bar_margin : None, scroller_width : None,
                bar_spacing : None, anchor_x : wire::ScrollAnchor::Start, anchor_y :
                wire::ScrollAnchor::Start, auto_scroll : false, background : None, border
                : None, content : Box::new({ let mut children : Vec < wire::Node > =
                vec![native::padded(native::sized(native::container(format!("{}/@container:251",
                use_scope), native::text_options(native::text(format!("{}/@text:258",
                use_scope), "FILES".to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },),),
                Some(wire::Length::Fill), None,), wire::Edges { top : 5.0f32, right :
                16.0f32, bottom : 8.0f32, left : 16.0f32, },), { let mut children : Vec <
                wire::Node > = Vec::new(); if self.tree_phase == "loading" { children
                .push(native::padded(native::sized(native::container(format!("{}/@container:831",
                use_scope), native::sized(native::text(format!("{}/@text:837",
                use_scope), "Loading repository files…".to_owned().to_string(),),
                Some(wire::Length::Fill), None,),), Some(wire::Length::Fill), None,),
                wire::Edges { top : 8.0f32, right : 16.0f32, bottom : 0.0f32, left :
                16.0f32, },),); } if self.tree_phase == "failed" { children
                .push(native::padded(native::sized(native::container(format!("{}/@container:844",
                use_scope), native::sized(native::text(format!("{}/@text:850",
                use_scope), "Could not load code. Pick Code to try again.".to_owned()
                .to_string(),), Some(wire::Length::Fill), None,),),
                Some(wire::Length::Fill), None,), wire::Edges { top : 8.0f32, right :
                16.0f32, bottom : 0.0f32, left : 16.0f32, },),); } if self.tree_phase ==
                "ready" { children.push({ let mut children : Vec < wire::Node > =
                Vec::new(); if self.tree_entries.is_empty() && ! self.tree_born {
                children
                .push(native::padded(native::sized(native::container(format!("{}/@container:859",
                use_scope), native::sized(native::text(format!("{}/@text:865",
                use_scope), "Nothing committed on this repository yet.".to_owned()
                .to_string(),), Some(wire::Length::Fill), None,),),
                Some(wire::Length::Fill), None,), wire::Edges { top : 8.0f32, right :
                16.0f32, bottom : 0.0f32, left : 16.0f32, },),); } if self.tree_entries
                .is_empty() && self.tree_born && ! self.tree_truncated { children
                .push(native::padded(native::sized(native::container(format!("{}/@container:872",
                use_scope), native::sized(native::text(format!("{}/@text:878",
                use_scope), "No files in this commit.".to_owned().to_string(),),
                Some(wire::Length::Fill), None,),), Some(wire::Length::Fill), None,),
                wire::Edges { top : 8.0f32, right : 16.0f32, bottom : 0.0f32, left :
                16.0f32, },),); } if self.tree_entries.is_empty() && self.tree_born &&
                self.tree_truncated { children
                .push(native::padded(native::sized(native::container(format!("{}/@container:885",
                use_scope), native::sized(native::text(format!("{}/@text:891",
                use_scope), "This directory has entries that cannot be shown.".to_owned()
                .to_string(),), Some(wire::Length::Fill), None,),),
                Some(wire::Length::Fill), None,), wire::Edges { top : 8.0f32, right :
                16.0f32, bottom : 0.0f32, left : 16.0f32, },),); } if ! self.tree_path
                .is_empty() { children.push(wire::Node::Button { checked : None, expanded
                : None, description : None, key : format!("{}/@button:898", use_scope),
                content : wire::ButtonContent::Child(Box::new(self
                .render_forge_tree_dir_row_16(format!("{}/ForgeTreeDirRow@3647",
                use_scope),),),), label : Some(String::from("Back to the repository root"
                .to_owned()),), on_press :
                Some(::ducktape_view_guest::slots::message(cb_0("".to_owned())),), width
                : Some(wire::Length::Fill), height : None, padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), });
                } for (index, entry) in self.tree_entries.iter().enumerate() { let
                for_scope = format!("{}/@for:3655({})", use_scope, index); children
                .push({ let mut children : Vec < wire::Node > = Vec::new(); if entry.kind
                == "dir" { children.push(wire::Node::Button { checked : None, expanded :
                None, description : Some(String::from(entry.path.to_owned())), key :
                format!("{}/@button:915", for_scope), content :
                wire::ButtonContent::Child(Box::new(self
                .render_forge_tree_dir_row_17(format!("{}/ForgeTreeDirRow@3665",
                for_scope), entry.name.to_owned(),),),), label :
                Some(String::from("Open directory".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(cb_0(entry.path
                .to_owned()),),), width : Some(wire::Length::Fill), height : None,
                padding : Some(wire::Edges::all(0.0f32)), style :
                wire::ButtonStyle::default(), }); } if entry.kind != "dir" { children
                .push(wire::Node::Button { checked : None, expanded : None, description :
                Some(String::from(entry.path.to_owned())), key :
                format!("{}/@button:931", for_scope), content :
                wire::ButtonContent::Child(Box::new(self
                .render_forge_tree_file_row_21(format!("{}/ForgeTreeFileRow@3681",
                for_scope), entry.name.to_owned(), entry.path == crate
                ::host::forge_file_header(::std::convert::AsRef::as_ref(& self
                .opened_dir), ::std::convert::AsRef::as_ref(& self.opened_rev),
                ::std::convert::AsRef::as_ref(& self.tree_path),
                ::std::convert::AsRef::as_ref(& self.tree_rev),
                ::std::convert::AsRef::as_ref(& self.file_path),),),),), label :
                Some(String::from("Open file".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(cb_1(entry.path
                .to_owned()),),), width : Some(wire::Length::Fill), height : None,
                padding : Some(wire::Edges::all(0.0f32)), style :
                wire::ButtonStyle::default(), }); }
                native::sized(native::column(format!("{}/@layout:913", for_scope),
                children,), Some(wire::Length::Fill), None,) }); } if self.tree_truncated
                { children
                .push(native::padded(native::sized(native::container(format!("{}/@container:947",
                use_scope), native::sized(native::text(format!("{}/@text:948",
                use_scope), "Some entries are not shown.".to_owned().to_string(),),
                Some(wire::Length::Fill), None,),), Some(wire::Length::Fill), None,),
                wire::Edges { top : 12.0f32, right : 12.0f32, bottom : 12.0f32, left :
                12.0f32, },),); } native::sized(native::column(format!("{}/@layout:857",
                use_scope), children,), Some(wire::Length::Fill), None,) }); }
                native::sized(native::column(format!("{}/@layout:829", use_scope),
                children,), Some(wire::Length::Fill), None,) }];
                native::padded(native::sized(native::column(format!("{}/@layout:246",
                use_scope), children,), Some(wire::Length::Fill), None,), wire::Edges {
                top : 9.0f32, right : 0.0f32, bottom : 9.0f32, left : 0.0f32, },) }),
                },), Some(wire::Length::Fixed(self.tree_width as f32)),
                Some(wire::Length::Fill),) }, { let node_scope =
                format!("{}/tree-resize", node_scope); wire::Node::ResizeHandle { key :
                node_scope.clone(), on_press : None, on_release : None, on_drag :
                Some(::ducktape_view_guest::slots::handler:: < (f64, f64), Message, >
                (Box::new({ let route = { let route_callback = cb_3.clone(); move | delta
                : (f64, f64) | route_callback(delta.0, delta.1) }; move | sent : (f64,
                f64) | Some(route(sent)) }),),), cursor :
                Some(wire::mouse::Cursor::ResizingHorizontally), content : Box::new({ let
                node_scope = format!("{}/tree-divider", node_scope);
                wire::Node::Container { shadow : Default::default(), max_width : None,
                max_height : None, clip : false, key : node_scope.clone(), width :
                Some(wire::Length::Fixed(10.0f32)), height : Some(wire::Length::Fill),
                padding : None, align_x : Some(wire::AlignX::Left), align_y : None,
                background : None.map(wire::Background::Color), border : None, snap :
                None, content :
                Box::new(native::sized(native::container(format!("{}/@container:271",
                use_scope), wire::Node::Space { width :
                Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(1.0f32)), },),
                Some(wire::Length::Fixed(1.0f32)), Some(wire::Length::Fill),),), } }), }
                }, { let mut children : Vec < wire::Node > = vec![self
                .render_forge_code_header_22(format!("{}/ForgeCodeHeader@1442",
                use_scope),), wire::Node::Scroll { on_scroll : None, virtual_rows :
                false, key : format!("{}/@layout:280", use_scope), direction :
                wire::ScrollDirection::Vertical, width : Some(wire::Length::Fill), height
                : Some(wire::Length::Fill), bar_hidden : false, bar_width : None,
                bar_margin : None, scroller_width : None, bar_spacing : None, anchor_x :
                wire::ScrollAnchor::Start, anchor_y : wire::ScrollAnchor::Start,
                auto_scroll : false, background : None, border : None, content :
                Box::new({ let mut children : Vec < wire::Node > = Vec::new(); if self
                .tree_phase == "loading" { children.push(self
                .render_forge_code_empty_23(format!("{}/ForgeCodeEmpty@3701",
                use_scope),),); } if self.file_phase == "loading" && ! crate
                ::host::forge_file_header(::std::convert::AsRef::as_ref(& self
                .opened_dir), ::std::convert::AsRef::as_ref(& self.opened_rev),
                ::std::convert::AsRef::as_ref(& self.tree_path),
                ::std::convert::AsRef::as_ref(& self.tree_rev),
                ::std::convert::AsRef::as_ref(& self.file_path),).is_empty() { children
                .push(self.render_forge_code_empty_24(format!("{}/ForgeCodeEmpty@3703",
                use_scope),),); } if self.tree_phase == "failed" { children.push(self
                .render_forge_code_empty_25(format!("{}/ForgeCodeEmpty@3705",
                use_scope),),); } if self.file_phase == "failed" && ! crate
                ::host::forge_file_header(::std::convert::AsRef::as_ref(& self
                .opened_dir), ::std::convert::AsRef::as_ref(& self.opened_rev),
                ::std::convert::AsRef::as_ref(& self.tree_path),
                ::std::convert::AsRef::as_ref(& self.tree_rev),
                ::std::convert::AsRef::as_ref(& self.file_path),).is_empty() { children
                .push(self.render_forge_code_empty_26(format!("{}/ForgeCodeEmpty@3707",
                use_scope),),); } if self.tree_phase == "ready" && crate
                ::host::forge_file_header(::std::convert::AsRef::as_ref(& self
                .opened_dir), ::std::convert::AsRef::as_ref(& self.opened_rev),
                ::std::convert::AsRef::as_ref(& self.tree_path),
                ::std::convert::AsRef::as_ref(& self.tree_rev),
                ::std::convert::AsRef::as_ref(& self.file_path),).is_empty() && self
                .tree_entries.is_empty() && ! self.tree_born { children.push(self
                .render_forge_code_empty_27(format!("{}/ForgeCodeEmpty@3709",
                use_scope),),); } if self.tree_phase == "ready" && crate
                ::host::forge_file_header(::std::convert::AsRef::as_ref(& self
                .opened_dir), ::std::convert::AsRef::as_ref(& self.opened_rev),
                ::std::convert::AsRef::as_ref(& self.tree_path),
                ::std::convert::AsRef::as_ref(& self.tree_rev),
                ::std::convert::AsRef::as_ref(& self.file_path),).is_empty() && self
                .tree_entries.is_empty() && self.tree_born && ! self.tree_truncated {
                children.push(self
                .render_forge_code_empty_28(format!("{}/ForgeCodeEmpty@3714",
                use_scope),),); } if self.tree_phase == "ready" && crate
                ::host::forge_file_header(::std::convert::AsRef::as_ref(& self
                .opened_dir), ::std::convert::AsRef::as_ref(& self.opened_rev),
                ::std::convert::AsRef::as_ref(& self.tree_path),
                ::std::convert::AsRef::as_ref(& self.tree_rev),
                ::std::convert::AsRef::as_ref(& self.file_path),).is_empty() && self
                .tree_entries.is_empty() && self.tree_born && self.tree_truncated {
                children.push(self
                .render_forge_code_empty_29(format!("{}/ForgeCodeEmpty@3716",
                use_scope),),); } if self.tree_phase == "ready" && crate
                ::host::forge_file_header(::std::convert::AsRef::as_ref(& self
                .opened_dir), ::std::convert::AsRef::as_ref(& self.opened_rev),
                ::std::convert::AsRef::as_ref(& self.tree_path),
                ::std::convert::AsRef::as_ref(& self.tree_rev),
                ::std::convert::AsRef::as_ref(& self.file_path),).is_empty() && ! self
                .tree_entries.is_empty() { children.push(self
                .render_forge_code_empty_30(format!("{}/ForgeCodeEmpty@3721",
                use_scope),),); } if self.file_phase == "ready" && ! crate
                ::host::forge_file_header(::std::convert::AsRef::as_ref(& self
                .opened_dir), ::std::convert::AsRef::as_ref(& self.opened_rev),
                ::std::convert::AsRef::as_ref(& self.tree_path),
                ::std::convert::AsRef::as_ref(& self.tree_rev),
                ::std::convert::AsRef::as_ref(& self.file_path),).is_empty() && self
                .file_binary { children.push(self
                .render_forge_code_empty_31(format!("{}/ForgeCodeEmpty@3726",
                use_scope),),); } if self.file_phase == "ready" && ! crate
                ::host::forge_file_header(::std::convert::AsRef::as_ref(& self
                .opened_dir), ::std::convert::AsRef::as_ref(& self.opened_rev),
                ::std::convert::AsRef::as_ref(& self.tree_path),
                ::std::convert::AsRef::as_ref(& self.tree_rev),
                ::std::convert::AsRef::as_ref(& self.file_path),).is_empty() && ! self
                .file_binary && self.file_picture { children.push({ let mut children :
                Vec < wire::Node > = vec![{ let node_scope = format!("{}/forge-picture",
                node_scope); wire::Node::Surface { key : node_scope.clone(), name :
                String::from("picture"), args : ::std::vec![{ let surface_arg = &
                ("forge".to_owned());
                wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                }, { let surface_arg = & (self.file_path.to_owned());
                wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                }], on_event : None, } },
                native::text_options(native::text(format!("{}/@text:996", use_scope),
                crate ::host::picture_caption(self.file_width, self.file_height,)
                .to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },)];
                native::spaced(native::padded(native::sized(native::column(format!("{}/@layout:988",
                use_scope), children,), Some(wire::Length::Fill), None,), wire::Edges {
                top : 13.0f32, right : 16.0f32, bottom : 13.0f32, left : 16.0f32, },),
                9.0f32,) }); } if self.file_phase == "ready" && ! crate
                ::host::forge_file_header(::std::convert::AsRef::as_ref(& self
                .opened_dir), ::std::convert::AsRef::as_ref(& self.opened_rev),
                ::std::convert::AsRef::as_ref(& self.tree_path),
                ::std::convert::AsRef::as_ref(& self.tree_rev),
                ::std::convert::AsRef::as_ref(& self.file_path),).is_empty() && ! self
                .file_binary && ! self.file_picture && crate
                ::host::markdown_path(::std::convert::AsRef::as_ref(& self.file_path),) {
                children.push({ let mut children : Vec < wire::Node > = vec![{ let
                _lazy_context_739 = ctx_0.to_owned(); let lazy_event_739_0 = cb_2
                .clone(); let _lazy_event_739_1 = cb_0.clone(); let _lazy_event_739_2 =
                cb_1.clone(); let _lazy_event_739_3 = cb_3.clone(); { let lazy_key =
                format!("{}/@lazy:1023", use_scope);
                ::ducktape_view_guest::memo_lazy((self.file_text.to_owned(), self
                .file_path.to_owned(), self.dark, self.file_text_revision, node_scope
                .to_owned(), match self.active_palette { AppTheme::App => "app",
                AppTheme::AppDark => "app-dark", },), move | dependency | { let
                _file_text : String = dependency.0.clone(); let file_path : String =
                dependency.1.clone(); let dark : bool = dependency.2.clone(); let
                lazy_scope = dependency.4.clone(); let cached_doc : String = self
                .file_text.to_owned(); { let node_scope = format!("{}/forge-markdown",
                lazy_scope); wire::Node::Surface { key : node_scope.clone(), name :
                String::from("forge_markdown"), args : ::std::vec![{ let surface_arg = &
                (cached_doc.to_owned());
                wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                }, { let surface_arg = & (file_path.to_owned());
                wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                }, { let surface_arg = & (dark); wire::SurfaceValue::Bool(*
                (surface_arg)) }], on_event :
                Some(::ducktape_view_guest::slots::handler:: < wire::SurfaceValue,
                Message, > (Box::new({ let route = { let route_callback =
                lazy_event_739_0.clone(); move | value | route_callback(value) }; move |
                sent | { match sent { wire::SurfaceValue::Str(item) => Some(item), _ =>
                None, } .map(& route) } }),),), } } }, 739u64, & use_scope, lazy_key,) }
                }]; if self.file_truncated { children
                .push(native::text_options(native::text(format!("{}/@text:1026",
                use_scope), "This file is larger than the 64 KiB preview limit."
                .to_owned().to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },),); }
                native::spaced(native::padded(native::sized(native::column(format!("{}/@layout:1013",
                use_scope), children,), Some(wire::Length::Fill), None,), wire::Edges {
                top : 13.0f32, right : 16.0f32, bottom : 13.0f32, left : 16.0f32, },),
                9.0f32,) }); } if self.file_phase == "ready" && ! crate
                ::host::forge_file_header(::std::convert::AsRef::as_ref(& self
                .opened_dir), ::std::convert::AsRef::as_ref(& self.opened_rev),
                ::std::convert::AsRef::as_ref(& self.tree_path),
                ::std::convert::AsRef::as_ref(& self.tree_rev),
                ::std::convert::AsRef::as_ref(& self.file_path),).is_empty() && ! self
                .file_binary && ! self.file_picture && ! crate
                ::host::markdown_path(::std::convert::AsRef::as_ref(& self.file_path),) {
                children.push({ let mut children : Vec < wire::Node > = vec![{ let
                _lazy_context_745 = ctx_0.to_owned(); let _lazy_event_745_0 = cb_2
                .clone(); let _lazy_event_745_1 = cb_0.clone(); let _lazy_event_745_2 =
                cb_1.clone(); let _lazy_event_745_3 = cb_3.clone(); { let lazy_key =
                format!("{}/@lazy:1056", use_scope);
                ::ducktape_view_guest::memo_lazy((self.file_text.to_owned(), self
                .file_path.to_owned(), self.dark, self.file_text_revision, node_scope
                .to_owned(), match self.active_palette { AppTheme::App => "app",
                AppTheme::AppDark => "app-dark", },), move | dependency | { let
                _file_text : String = dependency.0.clone(); let file_path : String =
                dependency.1.clone(); let dark : bool = dependency.2.clone(); let
                lazy_scope = dependency.4.clone(); let cached_source : String = self
                .file_text.to_owned(); { let node_scope = format!("{}/forge-code",
                lazy_scope); wire::Node::Surface { key : node_scope.clone(), name :
                String::from("forge_code"), args : ::std::vec![{ let surface_arg = &
                (cached_source.to_owned());
                wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                }, { let surface_arg = & (file_path.to_owned());
                wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                }, { let surface_arg = & (dark); wire::SurfaceValue::Bool(*
                (surface_arg)) }], on_event : None, } } }, 745u64, & use_scope,
                lazy_key,) } }]; if self.file_truncated { children
                .push(native::text_options(native::text(format!("{}/@text:1059",
                use_scope), "This file is larger than the 64 KiB preview limit."
                .to_owned().to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },),); }
                native::spaced(native::padded(native::sized(native::column(format!("{}/@layout:1032",
                use_scope), children,), Some(wire::Length::Fill), None,), wire::Edges {
                top : 13.0f32, right : 0.0f32, bottom : 13.0f32, left : 0.0f32, },),
                9.0f32,) }); } native::sized(native::column(format!("{}/@layout:956",
                use_scope), children,), Some(wire::Length::Fill), None,) }), }];
                native::sized(native::column(format!("{}/@layout:273", use_scope),
                children,), Some(wire::Length::Fill), Some(wire::Length::Fill),) }
            ];
            native::sized(
                native::row(node_scope.clone(), children),
                Some(wire::Length::Fill),
                Some(wire::Length::Fill),
            )
        }
    }
    pub(super) fn render_pr_state_plate_39(
        &self,
        use_scope: String,
        arg_0: String,
    ) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = Vec::new();
            if arg_0 == "open" {
                children
                    .push(wire::Node::Container {
                        shadow: Default::default(),
                        max_width: None,
                        max_height: None,
                        clip: false,
                        key: format!("{}/@container:587", use_scope),
                        width: Some(wire::Length::Fixed(24.0f32)),
                        height: Some(wire::Length::Fixed(24.0f32)),
                        padding: None,
                        align_x: Some(wire::AlignX::Center),
                        align_y: Some(wire::AlignY::Center),
                        background: None,
                        border: None,
                        snap: None,
                        content: Box::new(
                            self
                                .icon(
                                    format!("{}/Icon@1765", use_scope),
                                    "pull-request",
                                    13f32,
                                    "@media:58",
                                ),
                        ),
                    });
            }
            if !(arg_0 == "open") && arg_0 == "merged" {
                children
                    .push(wire::Node::Container {
                        shadow: Default::default(),
                        max_width: None,
                        max_height: None,
                        clip: false,
                        key: format!("{}/@container:603", use_scope),
                        width: Some(wire::Length::Fixed(24.0f32)),
                        height: Some(wire::Length::Fixed(24.0f32)),
                        padding: None,
                        align_x: Some(wire::AlignX::Center),
                        align_y: Some(wire::AlignY::Center),
                        background: None,
                        border: None,
                        snap: None,
                        content: Box::new(
                            self
                                .icon(
                                    format!("{}/Icon@1781", use_scope),
                                    "pull-request",
                                    13f32,
                                    "@media:82",
                                ),
                        ),
                    });
            }
            if !(arg_0 == "open" || arg_0 == "merged") {
                children
                    .push(wire::Node::Container {
                        shadow: Default::default(),
                        max_width: None,
                        max_height: None,
                        clip: false,
                        key: format!("{}/@container:619", use_scope),
                        width: Some(wire::Length::Fixed(24.0f32)),
                        height: Some(wire::Length::Fixed(24.0f32)),
                        padding: None,
                        align_x: Some(wire::AlignX::Center),
                        align_y: Some(wire::AlignY::Center),
                        background: None,
                        border: None,
                        snap: None,
                        content: Box::new(
                            self
                                .icon(
                                    format!("{}/Icon@1797", use_scope),
                                    "pull-request",
                                    13f32,
                                    "@media:82",
                                ),
                        ),
                    });
            }
            native::column(node_scope.clone(), children)
        }
    }
    pub(super) fn render_issue_state_glyph_42(
        &self,
        use_scope: String,
        arg_0: String,
    ) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = Vec::new();
            if arg_0 == "open" {
                children
                    .push(
                        self
                            .icon(
                                format!("{}/Icon@1809", use_scope),
                                "issue-open",
                                17f32,
                                "@media:58",
                            ),
                    );
            }
            if !(arg_0 == "open") {
                children
                    .push(
                        self
                            .icon(
                                format!("{}/Icon@1815", use_scope),
                                "issue-closed",
                                17f32,
                                "@media:82",
                            ),
                    );
            }
            native::column(node_scope.clone(), children)
        }
    }
    pub(super) fn render_tracker_row_43(
        &self,
        use_scope: String,
        cb_0: impl Fn(i64) -> Message + Clone + 'static,
        arg_0: crate::host::ForgeItem,
    ) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = vec![
                wire::Node::Button { checked : None, expanded : None, description :
                Some(String::from(arg_0.title.to_owned())), key :
                format!("{}/@button:514", use_scope), content :
                wire::ButtonContent::Child(Box::new(native::padded(native::sized(native::container(format!("{}/@container:521",
                use_scope), { let mut children : Vec < wire::Node > = Vec::new(); if
                arg_0.kind == "pr" { children.push(self
                .render_pr_state_plate_39(format!("{}/PrStatePlate@1703", use_scope),
                arg_0.state.to_owned(),),); } if ! (arg_0.kind == "pr") && arg_0.kind ==
                "issue" { children.push(self
                .render_issue_state_glyph_42(format!("{}/IssueStateGlyph@1705",
                use_scope), arg_0.state.to_owned(),),); } children.push({ let mut
                children : Vec < wire::Node > =
                vec![native::sized(native::text_options(native::text(format!("{}/@text:539",
                use_scope), arg_0.title.to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },),
                Some(wire::Length::Fill), None,), { let mut children : Vec < wire::Node >
                = vec![{ let mut children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:548",
                use_scope), "#".to_owned().to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:554", use_scope),
                arg_0.number.to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },)]; wire::Node::Linear
                { max_width : None, clip : false, key : format!("{}/@layout:547",
                use_scope), wrap : None, axis : wire::Axis::Row, spacing : Some(0.0f32),
                padding : None, width : None, height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }, native::text_options(native::text(format!("{}/@text:560",
                use_scope), "· opened by".to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:566", use_scope),
                arg_0.author_name.to_owned().to_string(),), wire::TextOptions { wrapping
                : Some(wire::Wrapping::None), ..Default::default() },)];
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:546", use_scope), wrap : None, axis :
                wire::Axis::Row, spacing : Some(5.0f32), padding : None, width : None,
                height : None, align : Some(wire::AlignX::Center), background : None,
                border : None, children : children, } }];
                native::spaced(native::sized(native::column(format!("{}/@layout:538",
                use_scope), children,), Some(wire::Length::Fill), None,), 4.0f32,) });
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:528", use_scope), wrap : None, axis :
                wire::Axis::Row, spacing : Some(13.0f32), padding : None, width :
                Some(wire::Length::Fill), height : None, align :
                Some(wire::AlignX::Left), background : None, border : None, children :
                children, } },), Some(wire::Length::Fill), None,), wire::Edges { top :
                13.0f32, right : 24.0f32, bottom : 13.0f32, left : 24.0f32, },),),),
                label : Some(String::from("Open item".to_owned())), on_press :
                Some(::ducktape_view_guest::slots::message(cb_0(arg_0.number)),), width :
                Some(wire::Length::Fill), height : None, padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), },
                native::sized(native::container(format!("{}/@container:575", use_scope),
                wire::Node::Space { width : Some(wire::Length::Fixed(1.0f32)), height :
                Some(wire::Length::Fixed(1.0f32)), },), Some(wire::Length::Fill),
                Some(wire::Length::Fixed(1.0f32)),)
            ];
            native::sized(
                native::column(node_scope.clone(), children),
                Some(wire::Length::Fill),
                None,
            )
        }
    }
    pub(super) fn render_forge_tracker_list_44(
        &self,
        use_scope: String,
        cb_10: impl Fn(i64) -> Message + Clone + 'static,
    ) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = Vec::new();
            if self.repo_phase == "idle" {
                children
                    .push(wire::Node::Space {
                        width: Some(wire::Length::Fixed(1.0f32)),
                        height: Some(wire::Length::Fixed(1.0f32)),
                    });
            }
            if !(self.repo_phase == "idle") && self.repo_phase == "loading" {
                children
                    .push(
                        native::padded(
                            native::sized(
                                native::container(
                                    format!("{}/@container:1546", use_scope),
                                    self
                                        .loading_tracker(
                                            format!("{}/EmptyPlate@2715", use_scope),
                                        ),
                                ),
                                Some(wire::Length::Fill),
                                None,
                            ),
                            wire::Edges {
                                top: 22.0f32,
                                right: 22.0f32,
                                bottom: 22.0f32,
                                left: 22.0f32,
                            },
                        ),
                    );
            }
            if !(self.repo_phase == "idle" || self.repo_phase == "loading")
                && self.repo_phase == "failed"
            {
                children
                    .push(
                        native::padded(
                            native::sized(
                                native::container(
                                    format!("{}/@container:1549", use_scope),
                                    self
                                        .tracker_unavailable(
                                            format!("{}/EmptyPlate@2718", use_scope),
                                        ),
                                ),
                                Some(wire::Length::Fill),
                                None,
                            ),
                            wire::Edges {
                                top: 22.0f32,
                                right: 22.0f32,
                                bottom: 22.0f32,
                                left: 22.0f32,
                            },
                        ),
                    );
            }
            if !(self.repo_phase == "idle" || self.repo_phase == "loading"
                || self.repo_phase == "failed") && self.repo_phase == "ready"
            {
                children
                    .push({
                        let mut children: Vec<wire::Node> = Vec::new();
                        if crate::host::filter_forge_items(
                                ::std::convert::AsRef::as_ref(&self.items),
                                ::std::convert::AsRef::as_ref(&"issues"),
                            )
                            .is_empty()
                        {
                            children
                                .push(
                                    native::padded(
                                        native::sized(
                                            native::container(
                                                format!("{}/@container:1556", use_scope),
                                                self
                                                    .empty_issues(
                                                        format!("{}/EmptyPlate@2725", use_scope),
                                                    ),
                                            ),
                                            Some(wire::Length::Fill),
                                            None,
                                        ),
                                        wire::Edges {
                                            top: 22.0f32,
                                            right: 22.0f32,
                                            bottom: 22.0f32,
                                            left: 22.0f32,
                                        },
                                    ),
                                );
                        }
                        if !crate::host::filter_forge_items(
                                ::std::convert::AsRef::as_ref(&self.items),
                                ::std::convert::AsRef::as_ref(&"issues"),
                            )
                            .is_empty()
                        {
                            children
                                .push(wire::Node::Scroll {
                                    on_scroll: None,
                                    virtual_rows: false,
                                    key: format!("{}/@layout:1559", use_scope),
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
                                    auto_scroll: false,
                                    background: None,
                                    border: None,
                                    content: Box::new({
                                        let mut children: Vec<wire::Node> = Vec::new();
                                        for (index, item) in crate::host::filter_forge_items(
                                                ::std::convert::AsRef::as_ref(&self.items),
                                                ::std::convert::AsRef::as_ref(&"issues"),
                                            )
                                            .iter()
                                            .enumerate()
                                        {
                                            let for_scope = format!(
                                                "{}/@for:2740({})", use_scope, index
                                            );
                                            children
                                                .push(
                                                    self
                                                        .render_tracker_row_43(
                                                            format!("{}/TrackerRow@2741", for_scope),
                                                            cb_10.clone(),
                                                            item.clone(),
                                                        ),
                                                );
                                        }
                                        native::spaced(
                                            native::padded(
                                                native::sized(
                                                    native::column(
                                                        format!("{}/@layout:1564", use_scope),
                                                        children,
                                                    ),
                                                    Some(wire::Length::Fill),
                                                    None,
                                                ),
                                                wire::Edges {
                                                    top: 6.0f32,
                                                    right: 12.0f32,
                                                    bottom: 18.0f32,
                                                    left: 12.0f32,
                                                },
                                            ),
                                            1.0f32,
                                        )
                                    }),
                                });
                        }
                        native::sized(
                            native::column(
                                format!("{}/@layout:1554", use_scope),
                                children,
                            ),
                            Some(wire::Length::Fill),
                            Some(wire::Length::Fill),
                        )
                    });
            }
            native::sized(
                native::column(node_scope.clone(), children),
                Some(wire::Length::Fill),
                Some(wire::Length::Fill),
            )
        }
    }
    pub(super) fn render_forge_tracker_list_46(
        &self,
        use_scope: String,
        cb_10: impl Fn(i64) -> Message + Clone + 'static,
    ) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = Vec::new();
            if self.repo_phase == "idle" {
                children
                    .push(wire::Node::Space {
                        width: Some(wire::Length::Fixed(1.0f32)),
                        height: Some(wire::Length::Fixed(1.0f32)),
                    });
            }
            if !(self.repo_phase == "idle") && self.repo_phase == "loading" {
                children
                    .push(
                        native::padded(
                            native::sized(
                                native::container(
                                    format!("{}/@container:1546", use_scope),
                                    self
                                        .loading_tracker(
                                            format!("{}/EmptyPlate@2715", use_scope),
                                        ),
                                ),
                                Some(wire::Length::Fill),
                                None,
                            ),
                            wire::Edges {
                                top: 22.0f32,
                                right: 22.0f32,
                                bottom: 22.0f32,
                                left: 22.0f32,
                            },
                        ),
                    );
            }
            if !(self.repo_phase == "idle" || self.repo_phase == "loading")
                && self.repo_phase == "failed"
            {
                children
                    .push(
                        native::padded(
                            native::sized(
                                native::container(
                                    format!("{}/@container:1549", use_scope),
                                    self
                                        .tracker_unavailable(
                                            format!("{}/EmptyPlate@2718", use_scope),
                                        ),
                                ),
                                Some(wire::Length::Fill),
                                None,
                            ),
                            wire::Edges {
                                top: 22.0f32,
                                right: 22.0f32,
                                bottom: 22.0f32,
                                left: 22.0f32,
                            },
                        ),
                    );
            }
            if !(self.repo_phase == "idle" || self.repo_phase == "loading"
                || self.repo_phase == "failed") && self.repo_phase == "ready"
            {
                children
                    .push({
                        let mut children: Vec<wire::Node> = Vec::new();
                        if crate::host::filter_forge_items(
                                ::std::convert::AsRef::as_ref(&self.items),
                                ::std::convert::AsRef::as_ref(&"pulls"),
                            )
                            .is_empty()
                        {
                            children
                                .push(
                                    native::padded(
                                        native::sized(
                                            native::container(
                                                format!("{}/@container:1556", use_scope),
                                                self
                                                    .empty_pulls(
                                                        format!("{}/EmptyPlate@2725", use_scope),
                                                    ),
                                            ),
                                            Some(wire::Length::Fill),
                                            None,
                                        ),
                                        wire::Edges {
                                            top: 22.0f32,
                                            right: 22.0f32,
                                            bottom: 22.0f32,
                                            left: 22.0f32,
                                        },
                                    ),
                                );
                        }
                        if !crate::host::filter_forge_items(
                                ::std::convert::AsRef::as_ref(&self.items),
                                ::std::convert::AsRef::as_ref(&"pulls"),
                            )
                            .is_empty()
                        {
                            children
                                .push(wire::Node::Scroll {
                                    on_scroll: None,
                                    virtual_rows: false,
                                    key: format!("{}/@layout:1559", use_scope),
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
                                    auto_scroll: false,
                                    background: None,
                                    border: None,
                                    content: Box::new({
                                        let mut children: Vec<wire::Node> = Vec::new();
                                        for (index, item) in crate::host::filter_forge_items(
                                                ::std::convert::AsRef::as_ref(&self.items),
                                                ::std::convert::AsRef::as_ref(&"pulls"),
                                            )
                                            .iter()
                                            .enumerate()
                                        {
                                            let for_scope = format!(
                                                "{}/@for:2740({})", use_scope, index
                                            );
                                            children
                                                .push(
                                                    self
                                                        .render_tracker_row_43(
                                                            format!("{}/TrackerRow@2741", for_scope),
                                                            cb_10.clone(),
                                                            item.clone(),
                                                        ),
                                                );
                                        }
                                        native::spaced(
                                            native::padded(
                                                native::sized(
                                                    native::column(
                                                        format!("{}/@layout:1564", use_scope),
                                                        children,
                                                    ),
                                                    Some(wire::Length::Fill),
                                                    None,
                                                ),
                                                wire::Edges {
                                                    top: 6.0f32,
                                                    right: 12.0f32,
                                                    bottom: 18.0f32,
                                                    left: 12.0f32,
                                                },
                                            ),
                                            1.0f32,
                                        )
                                    }),
                                });
                        }
                        native::sized(
                            native::column(
                                format!("{}/@layout:1554", use_scope),
                                children,
                            ),
                            Some(wire::Length::Fill),
                            Some(wire::Length::Fill),
                        )
                    });
            }
            native::sized(
                native::column(node_scope.clone(), children),
                Some(wire::Length::Fill),
                Some(wire::Length::Fill),
            )
        }
    }
    pub(super) fn render_pr_state_pill_49(&self, use_scope: String) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = Vec::new();
            if self.forge_item_state == "open" {
                children
                    .push(
                        native::padded(
                            native::container(
                                format!("{}/@container:720", use_scope),
                                {
                                    let mut children: Vec<wire::Node> = vec![
                                        self.icon(format!("{}/Icon@1897", use_scope),
                                        "pull-request", 13f32, "@media:58",),
                                        native::text_options(native::text(format!("{}/@text:734",
                                        use_scope), "Open".to_owned().to_string(),),
                                        wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                                        ..Default::default() },)
                                    ];
                                    wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:728", use_scope),
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
                            ),
                            wire::Edges {
                                top: 5.0f32,
                                right: 11.0f32,
                                bottom: 5.0f32,
                                left: 11.0f32,
                            },
                        ),
                    );
            }
            if !(self.forge_item_state == "open") && self.forge_item_state == "merged" {
                children
                    .push(
                        native::padded(
                            native::container(
                                format!("{}/@container:741", use_scope),
                                {
                                    let mut children: Vec<wire::Node> = vec![
                                        self.icon(format!("{}/Icon@1918", use_scope),
                                        "pull-request", 13f32, "@media:82",),
                                        native::text_options(native::text(format!("{}/@text:755",
                                        use_scope), "Merged".to_owned().to_string(),),
                                        wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                                        ..Default::default() },)
                                    ];
                                    wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:749", use_scope),
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
                            ),
                            wire::Edges {
                                top: 5.0f32,
                                right: 11.0f32,
                                bottom: 5.0f32,
                                left: 11.0f32,
                            },
                        ),
                    );
            }
            if !(self.forge_item_state == "open" || self.forge_item_state == "merged") {
                children
                    .push(
                        native::padded(
                            native::container(
                                format!("{}/@container:762", use_scope),
                                {
                                    let mut children: Vec<wire::Node> = vec![
                                        self.icon(format!("{}/Icon@1939", use_scope),
                                        "pull-request", 13f32, "@media:82",),
                                        native::text_options(native::text(format!("{}/@text:776",
                                        use_scope), "Closed".to_owned().to_string(),),
                                        wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                                        ..Default::default() },)
                                    ];
                                    wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:770", use_scope),
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
                            ),
                            wire::Edges {
                                top: 5.0f32,
                                right: 11.0f32,
                                bottom: 5.0f32,
                                left: 11.0f32,
                            },
                        ),
                    );
            }
            native::column(node_scope.clone(), children)
        }
    }
    pub(super) fn render_diff_count_56(&self, use_scope: String) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = vec![
                { let mut children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:789",
                use_scope), "+".to_owned().to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:795", use_scope),
                self.forge_item_additions.to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },)]; wire::Node::Linear
                { max_width : None, clip : false, key : format!("{}/@layout:788",
                use_scope), wrap : None, axis : wire::Axis::Row, spacing : Some(0.0f32),
                padding : None, width : None, height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }, { let mut children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:802",
                use_scope), "−".to_owned().to_string(),), wire::TextOptions { wrapping
                : Some(wire::Wrapping::None), ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:808", use_scope),
                self.forge_item_deletions.to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },)]; wire::Node::Linear
                { max_width : None, clip : false, key : format!("{}/@layout:801",
                use_scope), wrap : None, axis : wire::Axis::Row, spacing : Some(0.0f32),
                padding : None, width : None, height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }
            ];
            if self.forge_item_files_changed > 0 {
                children
                    .push({
                        let mut children: Vec<wire::Node> = vec![
                            native::text_options(native::text(format!("{}/@text:816",
                            use_scope), "·".to_owned().to_string(),), wire::TextOptions
                            { wrapping : Some(wire::Wrapping::None), ..Default::default()
                            },),
                            native::text_options(native::text(format!("{}/@text:822",
                            use_scope), self.forge_item_files_changed.to_string(),),
                            wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                            ..Default::default() },),
                            native::text_options(native::text(format!("{}/@text:828",
                            use_scope), "files".to_owned().to_string(),),
                            wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                            ..Default::default() },)
                        ];
                        wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:815", use_scope),
                            wrap: None,
                            axis: wire::Axis::Row,
                            spacing: Some(4.0f32),
                            padding: None,
                            width: None,
                            height: None,
                            align: Some(wire::AlignX::Center),
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
            }
            wire::Node::Linear {
                max_width: None,
                clip: false,
                key: node_scope.clone(),
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
        }
    }
    pub(super) fn render_issue_body_card_60(
        &self,
        use_scope: String,
        cb_15: impl Fn(String) -> Message + Clone + 'static,
    ) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        wire::Node::Container {
            shadow: Default::default(),
            max_width: None,
            max_height: None,
            clip: true,
            key: node_scope.clone(),
            width: Some(wire::Length::Fill),
            height: None,
            padding: None,
            align_x: None,
            align_y: None,
            background: None,
            border: None,
            snap: None,
            content: Box::new({
                let mut children: Vec<wire::Node> = vec![
                    native::padded(native::sized(native::container(format!("{}/@container:858",
                    use_scope), { let mut children : Vec < wire::Node > =
                    vec![native::text_options(native::text(format!("{}/@text:871",
                    use_scope), "opened by".to_owned().to_string(),), wire::TextOptions {
                    wrapping : Some(wire::Wrapping::None), ..Default::default() },),
                    native::text_options(native::text(format!("{}/@text:876", use_scope),
                    self.forge_item_author.to_owned().to_string(),), wire::TextOptions {
                    wrapping : Some(wire::Wrapping::None), ..Default::default() },)];
                    wire::Node::Linear { max_width : None, clip : false, key :
                    format!("{}/@layout:866", use_scope), wrap : None, axis :
                    wire::Axis::Row, spacing : Some(7.0f32), padding : None, width :
                    Some(wire::Length::Fill), height : None, align :
                    Some(wire::AlignX::Center), background : None, border : None,
                    children : children, } },), Some(wire::Length::Fill), None,),
                    wire::Edges { top : 8.0f32, right : 13.0f32, bottom : 8.0f32, left :
                    13.0f32, },),
                    native::sized(native::container(format!("{}/@container:882",
                    use_scope), wire::Node::Space { width :
                    Some(wire::Length::Fixed(1.0f32)), height :
                    Some(wire::Length::Fixed(1.0f32)), },), Some(wire::Length::Fill),
                    Some(wire::Length::Fixed(1.0f32)),),
                    native::padded(native::sized(native::container(format!("{}/@container:888",
                    use_scope), self.item_body(format!("{}/RichBody@2067",
                    use_scope), cb_15.clone(),),), Some(wire::Length::Fill), None,),
                    wire::Edges { top : 13.0f32, right : 15.0f32, bottom : 13.0f32, left
                    : 15.0f32, },)
                ];
                native::sized(
                    native::column(format!("{}/@layout:857", use_scope), children),
                    Some(wire::Length::Fill),
                    None,
                )
            }),
        }
    }
    pub(super) fn render_diff_count_62(&self, use_scope: String) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = vec![
                { let mut children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:789",
                use_scope), "+".to_owned().to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:795", use_scope),
                self.forge_item_additions.to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },)]; wire::Node::Linear
                { max_width : None, clip : false, key : format!("{}/@layout:788",
                use_scope), wrap : None, axis : wire::Axis::Row, spacing : Some(0.0f32),
                padding : None, width : None, height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }, { let mut children : Vec < wire::Node > =
                vec![native::text_options(native::text(format!("{}/@text:802",
                use_scope), "−".to_owned().to_string(),), wire::TextOptions { wrapping
                : Some(wire::Wrapping::None), ..Default::default() },),
                native::text_options(native::text(format!("{}/@text:808", use_scope),
                self.forge_item_deletions.to_string(),), wire::TextOptions { wrapping :
                Some(wire::Wrapping::None), ..Default::default() },)]; wire::Node::Linear
                { max_width : None, clip : false, key : format!("{}/@layout:801",
                use_scope), wrap : None, axis : wire::Axis::Row, spacing : Some(0.0f32),
                padding : None, width : None, height : None, align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } }
            ];
            if false {
                children
                    .push({
                        let mut children: Vec<wire::Node> = vec![
                            native::text_options(native::text(format!("{}/@text:816",
                            use_scope), "·".to_owned().to_string(),), wire::TextOptions
                            { wrapping : Some(wire::Wrapping::None), ..Default::default()
                            },),
                            native::text_options(native::text(format!("{}/@text:822",
                            use_scope), 0.to_string(),), wire::TextOptions { wrapping :
                            Some(wire::Wrapping::None), ..Default::default() },),
                            native::text_options(native::text(format!("{}/@text:828",
                            use_scope), "files".to_owned().to_string(),),
                            wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                            ..Default::default() },)
                        ];
                        wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:815", use_scope),
                            wrap: None,
                            axis: wire::Axis::Row,
                            spacing: Some(4.0f32),
                            padding: None,
                            width: None,
                            height: None,
                            align: Some(wire::AlignX::Center),
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
            }
            wire::Node::Linear {
                max_width: None,
                clip: false,
                key: node_scope.clone(),
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
        }
    }
    pub(super) fn render_diff_row_63(
        &self,
        use_scope: String,
        cb_0: impl Fn(String, String, String) -> Message + Clone + 'static,
        arg_0: crate::host::DiffLine,
    ) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = Vec::new();
            if arg_0.kind == "file" {
                children
                    .push(
                        native::padded(
                            native::sized(
                                native::container(
                                    format!("{}/@container:1122", use_scope),
                                    native::sized(
                                        native::text_options(
                                            native::text(
                                                format!("{}/@text:1130", use_scope),
                                                arg_0.text.to_owned().to_string(),
                                            ),
                                            wire::TextOptions {
                                                wrapping: Some(wire::Wrapping::None),
                                                ..Default::default()
                                            },
                                        ),
                                        Some(wire::Length::Fill),
                                        None,
                                    ),
                                ),
                                Some(wire::Length::Fill),
                                None,
                            ),
                            wire::Edges {
                                top: 5.0f32,
                                right: 14.0f32,
                                bottom: 5.0f32,
                                left: 14.0f32,
                            },
                        ),
                    );
            }
            if !(arg_0.kind == "file") && arg_0.kind == "hunk" {
                children
                    .push(
                        native::padded(
                            native::sized(
                                native::container(
                                    format!("{}/@container:1138", use_scope),
                                    native::sized(
                                        native::text_options(
                                            native::text(
                                                format!("{}/@text:1146", use_scope),
                                                arg_0.text.to_owned().to_string(),
                                            ),
                                            wire::TextOptions {
                                                wrapping: Some(wire::Wrapping::None),
                                                ..Default::default()
                                            },
                                        ),
                                        Some(wire::Length::Fill),
                                        None,
                                    ),
                                ),
                                Some(wire::Length::Fill),
                                None,
                            ),
                            wire::Edges {
                                top: 5.0f32,
                                right: 14.0f32,
                                bottom: 5.0f32,
                                left: 14.0f32,
                            },
                        ),
                    );
            }
            if !(arg_0.kind == "file" || arg_0.kind == "hunk") && arg_0.kind == "add" {
                children
                    .push(
                        native::sized(
                            native::container(
                                format!("{}/@container:1154", use_scope),
                                {
                                    let mut children: Vec<wire::Node> = vec![
                                        wire::Node::Container { shadow : Default::default(),
                                        max_width : None, max_height : None, clip : false, key :
                                        format!("{}/@container:1160", use_scope), width :
                                        Some(wire::Length::Fixed(34.0f32)), height :
                                        Some(wire::Length::Fixed(20.0f32)), padding :
                                        Some(wire::Edges { top : 0.0f32, right : 8.0f32, bottom :
                                        0.0f32, left : 0.0f32, }), align_x : None, align_y :
                                        Some(wire::AlignY::Center), background : None, border :
                                        None, snap : None, content :
                                        Box::new(native::sized(native::text_options(native::text(format!("{}/@text:1167",
                                        use_scope), arg_0.old_no.to_owned().to_string(),),
                                        wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                                        ..Default::default() },), Some(wire::Length::Fill),
                                        None,),), }
                                    ];
                                    if arg_0.path.is_empty() {
                                        children
                                            .push(wire::Node::Container {
                                                shadow: Default::default(),
                                                max_width: None,
                                                max_height: None,
                                                clip: false,
                                                key: format!("{}/@container:1176", use_scope),
                                                width: Some(wire::Length::Fixed(34.0f32)),
                                                height: Some(wire::Length::Fixed(20.0f32)),
                                                padding: Some(wire::Edges {
                                                    top: 0.0f32,
                                                    right: 8.0f32,
                                                    bottom: 0.0f32,
                                                    left: 0.0f32,
                                                }),
                                                align_x: None,
                                                align_y: Some(wire::AlignY::Center),
                                                background: None,
                                                border: None,
                                                snap: None,
                                                content: Box::new(
                                                    native::sized(
                                                        native::text_options(
                                                            native::text(
                                                                format!("{}/@text:1183", use_scope),
                                                                arg_0.new_no.to_owned().to_string(),
                                                            ),
                                                            wire::TextOptions {
                                                                wrapping: Some(wire::Wrapping::None),
                                                                ..Default::default()
                                                            },
                                                        ),
                                                        Some(wire::Length::Fill),
                                                        None,
                                                    ),
                                                ),
                                            });
                                    }
                                    if !arg_0.path.is_empty() {
                                        children
                                            .push(wire::Node::Button {
                                                checked: None,
                                                expanded: None,
                                                description: None,
                                                key: format!("{}/@button:1192", use_scope),
                                                content: wire::ButtonContent::Child(
                                                    Box::new(wire::Node::Container {
                                                        shadow: Default::default(),
                                                        max_width: None,
                                                        max_height: None,
                                                        clip: false,
                                                        key: format!("{}/@container:1199", use_scope),
                                                        width: Some(wire::Length::Fill),
                                                        height: Some(wire::Length::Fixed(20.0f32)),
                                                        padding: Some(wire::Edges {
                                                            top: 0.0f32,
                                                            right: 8.0f32,
                                                            bottom: 0.0f32,
                                                            left: 0.0f32,
                                                        }),
                                                        align_x: None,
                                                        align_y: Some(wire::AlignY::Center),
                                                        background: None.map(wire::Background::Color),
                                                        border: None,
                                                        snap: None,
                                                        content: Box::new(
                                                            native::sized(
                                                                native::text_options(
                                                                    native::text(
                                                                        format!("{}/@text:1205", use_scope),
                                                                        arg_0.new_no.to_owned().to_string(),
                                                                    ),
                                                                    wire::TextOptions {
                                                                        wrapping: Some(wire::Wrapping::None),
                                                                        ..Default::default()
                                                                    },
                                                                ),
                                                                Some(wire::Length::Fill),
                                                                None,
                                                            ),
                                                        ),
                                                    }),
                                                ),
                                                label: Some(
                                                    String::from("Comment on this line".to_owned()),
                                                ),
                                                on_press: Some(
                                                    ::ducktape_view_guest::slots::message(
                                                        cb_0(
                                                            arg_0.path.to_owned(),
                                                            arg_0.new_no.to_owned(),
                                                            "new".to_owned(),
                                                        ),
                                                    ),
                                                ),
                                                width: Some(wire::Length::Fixed(34.0f32)),
                                                height: Some(wire::Length::Fixed(20.0f32)),
                                                padding: Some(wire::Edges::all(0.0f32)),
                                                style: wire::ButtonStyle::default(),
                                            });
                                    }
                                    children
                                        .push(wire::Node::Container {
                                            shadow: Default::default(),
                                            max_width: None,
                                            max_height: None,
                                            clip: false,
                                            key: format!("{}/@container:1215", use_scope),
                                            width: Some(wire::Length::Fixed(14.0f32)),
                                            height: Some(wire::Length::Fixed(20.0f32)),
                                            padding: None,
                                            align_x: Some(wire::AlignX::Center),
                                            align_y: Some(wire::AlignY::Center),
                                            background: None.map(wire::Background::Color),
                                            border: None,
                                            snap: None,
                                            content: Box::new(
                                                native::text_options(
                                                    native::text(
                                                        format!("{}/@text:1221", use_scope),
                                                        arg_0.sign.to_owned().to_string(),
                                                    ),
                                                    wire::TextOptions {
                                                        wrapping: Some(wire::Wrapping::None),
                                                        ..Default::default()
                                                    },
                                                ),
                                            ),
                                        });
                                    children
                                        .push(wire::Node::Container {
                                            shadow: Default::default(),
                                            max_width: None,
                                            max_height: None,
                                            clip: false,
                                            key: format!("{}/@container:1227", use_scope),
                                            width: Some(wire::Length::Fill),
                                            height: Some(wire::Length::Fixed(20.0f32)),
                                            padding: Some(wire::Edges {
                                                top: 0.0f32,
                                                right: 12.0f32,
                                                bottom: 0.0f32,
                                                left: 0.0f32,
                                            }),
                                            align_x: None,
                                            align_y: Some(wire::AlignY::Center),
                                            background: None.map(wire::Background::Color),
                                            border: None,
                                            snap: None,
                                            content: Box::new(
                                                native::sized(
                                                    native::text_options(
                                                        native::text(
                                                            format!("{}/@text:1233", use_scope),
                                                            arg_0.text.to_owned().to_string(),
                                                        ),
                                                        wire::TextOptions {
                                                            wrapping: Some(wire::Wrapping::None),
                                                            ..Default::default()
                                                        },
                                                    ),
                                                    Some(wire::Length::Fill),
                                                    None,
                                                ),
                                            ),
                                        });
                                    wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:1155", use_scope),
                                        wrap: None,
                                        axis: wire::Axis::Row,
                                        spacing: Some(0.0f32),
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
                    );
            }
            if !(arg_0.kind == "file" || arg_0.kind == "hunk" || arg_0.kind == "add")
                && arg_0.kind == "del"
            {
                children
                    .push(
                        native::sized(
                            native::container(
                                format!("{}/@container:1241", use_scope),
                                {
                                    let mut children: Vec<wire::Node> = Vec::new();
                                    if arg_0.path.is_empty() {
                                        children
                                            .push(wire::Node::Container {
                                                shadow: Default::default(),
                                                max_width: None,
                                                max_height: None,
                                                clip: false,
                                                key: format!("{}/@container:1248", use_scope),
                                                width: Some(wire::Length::Fixed(34.0f32)),
                                                height: Some(wire::Length::Fixed(20.0f32)),
                                                padding: Some(wire::Edges {
                                                    top: 0.0f32,
                                                    right: 8.0f32,
                                                    bottom: 0.0f32,
                                                    left: 0.0f32,
                                                }),
                                                align_x: None,
                                                align_y: Some(wire::AlignY::Center),
                                                background: None,
                                                border: None,
                                                snap: None,
                                                content: Box::new(
                                                    native::sized(
                                                        native::text_options(
                                                            native::text(
                                                                format!("{}/@text:1255", use_scope),
                                                                arg_0.old_no.to_owned().to_string(),
                                                            ),
                                                            wire::TextOptions {
                                                                wrapping: Some(wire::Wrapping::None),
                                                                ..Default::default()
                                                            },
                                                        ),
                                                        Some(wire::Length::Fill),
                                                        None,
                                                    ),
                                                ),
                                            });
                                    }
                                    if !arg_0.path.is_empty() {
                                        children
                                            .push(wire::Node::Button {
                                                checked: None,
                                                expanded: None,
                                                description: None,
                                                key: format!("{}/@button:1264", use_scope),
                                                content: wire::ButtonContent::Child(
                                                    Box::new(wire::Node::Container {
                                                        shadow: Default::default(),
                                                        max_width: None,
                                                        max_height: None,
                                                        clip: false,
                                                        key: format!("{}/@container:1271", use_scope),
                                                        width: Some(wire::Length::Fill),
                                                        height: Some(wire::Length::Fixed(20.0f32)),
                                                        padding: Some(wire::Edges {
                                                            top: 0.0f32,
                                                            right: 8.0f32,
                                                            bottom: 0.0f32,
                                                            left: 0.0f32,
                                                        }),
                                                        align_x: None,
                                                        align_y: Some(wire::AlignY::Center),
                                                        background: None.map(wire::Background::Color),
                                                        border: None,
                                                        snap: None,
                                                        content: Box::new(
                                                            native::sized(
                                                                native::text_options(
                                                                    native::text(
                                                                        format!("{}/@text:1277", use_scope),
                                                                        arg_0.old_no.to_owned().to_string(),
                                                                    ),
                                                                    wire::TextOptions {
                                                                        wrapping: Some(wire::Wrapping::None),
                                                                        ..Default::default()
                                                                    },
                                                                ),
                                                                Some(wire::Length::Fill),
                                                                None,
                                                            ),
                                                        ),
                                                    }),
                                                ),
                                                label: Some(
                                                    String::from("Comment on this deleted line".to_owned()),
                                                ),
                                                on_press: Some(
                                                    ::ducktape_view_guest::slots::message(
                                                        cb_0(
                                                            arg_0.path.to_owned(),
                                                            arg_0.old_no.to_owned(),
                                                            "old".to_owned(),
                                                        ),
                                                    ),
                                                ),
                                                width: Some(wire::Length::Fixed(34.0f32)),
                                                height: Some(wire::Length::Fixed(20.0f32)),
                                                padding: Some(wire::Edges::all(0.0f32)),
                                                style: wire::ButtonStyle::default(),
                                            });
                                    }
                                    children
                                        .push(wire::Node::Container {
                                            shadow: Default::default(),
                                            max_width: None,
                                            max_height: None,
                                            clip: false,
                                            key: format!("{}/@container:1287", use_scope),
                                            width: Some(wire::Length::Fixed(34.0f32)),
                                            height: Some(wire::Length::Fixed(20.0f32)),
                                            padding: Some(wire::Edges {
                                                top: 0.0f32,
                                                right: 8.0f32,
                                                bottom: 0.0f32,
                                                left: 0.0f32,
                                            }),
                                            align_x: None,
                                            align_y: Some(wire::AlignY::Center),
                                            background: None,
                                            border: None,
                                            snap: None,
                                            content: Box::new(
                                                native::sized(
                                                    native::text_options(
                                                        native::text(
                                                            format!("{}/@text:1294", use_scope),
                                                            arg_0.new_no.to_owned().to_string(),
                                                        ),
                                                        wire::TextOptions {
                                                            wrapping: Some(wire::Wrapping::None),
                                                            ..Default::default()
                                                        },
                                                    ),
                                                    Some(wire::Length::Fill),
                                                    None,
                                                ),
                                            ),
                                        });
                                    children
                                        .push(wire::Node::Container {
                                            shadow: Default::default(),
                                            max_width: None,
                                            max_height: None,
                                            clip: false,
                                            key: format!("{}/@container:1302", use_scope),
                                            width: Some(wire::Length::Fixed(14.0f32)),
                                            height: Some(wire::Length::Fixed(20.0f32)),
                                            padding: None,
                                            align_x: Some(wire::AlignX::Center),
                                            align_y: Some(wire::AlignY::Center),
                                            background: None.map(wire::Background::Color),
                                            border: None,
                                            snap: None,
                                            content: Box::new(
                                                native::text_options(
                                                    native::text(
                                                        format!("{}/@text:1308", use_scope),
                                                        arg_0.sign.to_owned().to_string(),
                                                    ),
                                                    wire::TextOptions {
                                                        wrapping: Some(wire::Wrapping::None),
                                                        ..Default::default()
                                                    },
                                                ),
                                            ),
                                        });
                                    children
                                        .push(wire::Node::Container {
                                            shadow: Default::default(),
                                            max_width: None,
                                            max_height: None,
                                            clip: false,
                                            key: format!("{}/@container:1314", use_scope),
                                            width: Some(wire::Length::Fill),
                                            height: Some(wire::Length::Fixed(20.0f32)),
                                            padding: Some(wire::Edges {
                                                top: 0.0f32,
                                                right: 12.0f32,
                                                bottom: 0.0f32,
                                                left: 0.0f32,
                                            }),
                                            align_x: None,
                                            align_y: Some(wire::AlignY::Center),
                                            background: None.map(wire::Background::Color),
                                            border: None,
                                            snap: None,
                                            content: Box::new(
                                                native::sized(
                                                    native::text_options(
                                                        native::text(
                                                            format!("{}/@text:1320", use_scope),
                                                            arg_0.text.to_owned().to_string(),
                                                        ),
                                                        wire::TextOptions {
                                                            wrapping: Some(wire::Wrapping::None),
                                                            ..Default::default()
                                                        },
                                                    ),
                                                    Some(wire::Length::Fill),
                                                    None,
                                                ),
                                            ),
                                        });
                                    wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:1242", use_scope),
                                        wrap: None,
                                        axis: wire::Axis::Row,
                                        spacing: Some(0.0f32),
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
                    );
            }
            if !(arg_0.kind == "file" || arg_0.kind == "hunk" || arg_0.kind == "add"
                || arg_0.kind == "del") && arg_0.kind == "ctx"
            {
                children
                    .push(
                        native::sized(
                            native::container(
                                format!("{}/@container:1328", use_scope),
                                {
                                    let mut children: Vec<wire::Node> = vec![
                                        wire::Node::Container { shadow : Default::default(),
                                        max_width : None, max_height : None, clip : false, key :
                                        format!("{}/@container:1334", use_scope), width :
                                        Some(wire::Length::Fixed(34.0f32)), height :
                                        Some(wire::Length::Fixed(20.0f32)), padding :
                                        Some(wire::Edges { top : 0.0f32, right : 8.0f32, bottom :
                                        0.0f32, left : 0.0f32, }), align_x : None, align_y :
                                        Some(wire::AlignY::Center), background : None, border :
                                        None, snap : None, content :
                                        Box::new(native::sized(native::text_options(native::text(format!("{}/@text:1341",
                                        use_scope), arg_0.old_no.to_owned().to_string(),),
                                        wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                                        ..Default::default() },), Some(wire::Length::Fill),
                                        None,),), }
                                    ];
                                    if arg_0.path.is_empty() {
                                        children
                                            .push(wire::Node::Container {
                                                shadow: Default::default(),
                                                max_width: None,
                                                max_height: None,
                                                clip: false,
                                                key: format!("{}/@container:1350", use_scope),
                                                width: Some(wire::Length::Fixed(34.0f32)),
                                                height: Some(wire::Length::Fixed(20.0f32)),
                                                padding: Some(wire::Edges {
                                                    top: 0.0f32,
                                                    right: 8.0f32,
                                                    bottom: 0.0f32,
                                                    left: 0.0f32,
                                                }),
                                                align_x: None,
                                                align_y: Some(wire::AlignY::Center),
                                                background: None,
                                                border: None,
                                                snap: None,
                                                content: Box::new(
                                                    native::sized(
                                                        native::text_options(
                                                            native::text(
                                                                format!("{}/@text:1357", use_scope),
                                                                arg_0.new_no.to_owned().to_string(),
                                                            ),
                                                            wire::TextOptions {
                                                                wrapping: Some(wire::Wrapping::None),
                                                                ..Default::default()
                                                            },
                                                        ),
                                                        Some(wire::Length::Fill),
                                                        None,
                                                    ),
                                                ),
                                            });
                                    }
                                    if !arg_0.path.is_empty() {
                                        children
                                            .push(wire::Node::Button {
                                                checked: None,
                                                expanded: None,
                                                description: None,
                                                key: format!("{}/@button:1366", use_scope),
                                                content: wire::ButtonContent::Child(
                                                    Box::new(wire::Node::Container {
                                                        shadow: Default::default(),
                                                        max_width: None,
                                                        max_height: None,
                                                        clip: false,
                                                        key: format!("{}/@container:1373", use_scope),
                                                        width: Some(wire::Length::Fill),
                                                        height: Some(wire::Length::Fixed(20.0f32)),
                                                        padding: Some(wire::Edges {
                                                            top: 0.0f32,
                                                            right: 8.0f32,
                                                            bottom: 0.0f32,
                                                            left: 0.0f32,
                                                        }),
                                                        align_x: None,
                                                        align_y: Some(wire::AlignY::Center),
                                                        background: None.map(wire::Background::Color),
                                                        border: None,
                                                        snap: None,
                                                        content: Box::new(
                                                            native::sized(
                                                                native::text_options(
                                                                    native::text(
                                                                        format!("{}/@text:1379", use_scope),
                                                                        arg_0.new_no.to_owned().to_string(),
                                                                    ),
                                                                    wire::TextOptions {
                                                                        wrapping: Some(wire::Wrapping::None),
                                                                        ..Default::default()
                                                                    },
                                                                ),
                                                                Some(wire::Length::Fill),
                                                                None,
                                                            ),
                                                        ),
                                                    }),
                                                ),
                                                label: Some(
                                                    String::from("Comment on this line".to_owned()),
                                                ),
                                                on_press: Some(
                                                    ::ducktape_view_guest::slots::message(
                                                        cb_0(
                                                            arg_0.path.to_owned(),
                                                            arg_0.new_no.to_owned(),
                                                            "new".to_owned(),
                                                        ),
                                                    ),
                                                ),
                                                width: Some(wire::Length::Fixed(34.0f32)),
                                                height: Some(wire::Length::Fixed(20.0f32)),
                                                padding: Some(wire::Edges::all(0.0f32)),
                                                style: wire::ButtonStyle::default(),
                                            });
                                    }
                                    children
                                        .push(wire::Node::Container {
                                            shadow: Default::default(),
                                            max_width: None,
                                            max_height: None,
                                            clip: false,
                                            key: format!("{}/@container:1389", use_scope),
                                            width: Some(wire::Length::Fixed(14.0f32)),
                                            height: Some(wire::Length::Fixed(20.0f32)),
                                            padding: None,
                                            align_x: Some(wire::AlignX::Center),
                                            align_y: Some(wire::AlignY::Center),
                                            background: None.map(wire::Background::Color),
                                            border: None,
                                            snap: None,
                                            content: Box::new(
                                                native::text_options(
                                                    native::text(
                                                        format!("{}/@text:1395", use_scope),
                                                        arg_0.sign.to_owned().to_string(),
                                                    ),
                                                    wire::TextOptions {
                                                        wrapping: Some(wire::Wrapping::None),
                                                        ..Default::default()
                                                    },
                                                ),
                                            ),
                                        });
                                    children
                                        .push(wire::Node::Container {
                                            shadow: Default::default(),
                                            max_width: None,
                                            max_height: None,
                                            clip: false,
                                            key: format!("{}/@container:1401", use_scope),
                                            width: Some(wire::Length::Fill),
                                            height: Some(wire::Length::Fixed(20.0f32)),
                                            padding: Some(wire::Edges {
                                                top: 0.0f32,
                                                right: 12.0f32,
                                                bottom: 0.0f32,
                                                left: 0.0f32,
                                            }),
                                            align_x: None,
                                            align_y: Some(wire::AlignY::Center),
                                            background: None.map(wire::Background::Color),
                                            border: None,
                                            snap: None,
                                            content: Box::new(
                                                native::sized(
                                                    native::text_options(
                                                        native::text(
                                                            format!("{}/@text:1407", use_scope),
                                                            arg_0.text.to_owned().to_string(),
                                                        ),
                                                        wire::TextOptions {
                                                            wrapping: Some(wire::Wrapping::None),
                                                            ..Default::default()
                                                        },
                                                    ),
                                                    Some(wire::Length::Fill),
                                                    None,
                                                ),
                                            ),
                                        });
                                    wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:1329", use_scope),
                                        wrap: None,
                                        axis: wire::Axis::Row,
                                        spacing: Some(0.0f32),
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
                    );
            }
            native::sized(
                native::column(node_scope.clone(), children),
                Some(wire::Length::Fill),
                None,
            )
        }
    }
    pub(super) fn render_diff_pane_64(
        &self,
        use_scope: String,
        cb_5: impl Fn(String, String, String) -> Message + Clone + 'static,
    ) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        wire::Node::Container {
            shadow: Default::default(),
            max_width: None,
            max_height: None,
            clip: true,
            key: node_scope.clone(),
            width: Some(wire::Length::Fill),
            height: None,
            padding: None,
            align_x: None,
            align_y: None,
            background: None,
            border: None,
            snap: None,
            content: Box::new({
                let mut children: Vec<wire::Node> = vec![
                    native::padded(native::sized(native::container(format!("{}/@container:1063",
                    use_scope), { let mut children : Vec < wire::Node > = vec![self
                    .icon(format!("{}/Icon@2244", use_scope), "branch", 13f32,
                    "@media:82",),
                    native::text_options(native::text(format!("{}/@text:1081",
                    use_scope), self.forge_item_branches.to_owned().to_string(),),
                    wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                    ..Default::default() },), self
                    .render_diff_count_62(format!("{}/DiffCount@2255", use_scope),),
                    wire::Node::Space { width : Some(wire::Length::Fill), height : None,
                    }]; wire::Node::Linear { max_width : None, clip : false, key :
                    format!("{}/@layout:1071", use_scope), wrap : None, axis :
                    wire::Axis::Row, spacing : Some(9.0f32), padding : None, width :
                    Some(wire::Length::Fill), height : None, align :
                    Some(wire::AlignX::Center), background : None, border : None,
                    children : children, } },), Some(wire::Length::Fill), None,),
                    wire::Edges { top : 10.0f32, right : 14.0f32, bottom : 10.0f32, left
                    : 14.0f32, },),
                    native::sized(native::container(format!("{}/@container:1093",
                    use_scope), wire::Node::Space { width :
                    Some(wire::Length::Fixed(1.0f32)), height :
                    Some(wire::Length::Fixed(1.0f32)), },), Some(wire::Length::Fill),
                    Some(wire::Length::Fixed(1.0f32)),), { let mut children : Vec < _ > =
                    Vec::new(); for line in self.diff_rows.iter() { let key = line.key;
                    let key_recon = format!("{}/key({})", use_scope, key); let child :
                    wire::Node = self.render_diff_row_63(format!("{}/DiffRow@2268",
                    key_recon), cb_5.clone(), line.clone(),); children.push((key,
                    child)); } let (keys, children) = children.into_iter().map(| (key,
                    child) | { (wire::ListKey::from(key), child) }).unzip();
                    wire::Node::KeyedColumn { key : format!("{}/@keyed:1099", use_scope),
                    keys : Some(keys), children, background : None, border : None,
                    spacing : None, padding : None, width : Some(wire::Length::Fill),
                    height : None, max_width : None, align : None, virtual_row :
                    Some(20.0f32), } }
                ];
                native::sized(
                    native::column(format!("{}/@layout:1062", use_scope), children),
                    Some(wire::Length::Fill),
                    None,
                )
            }),
        }
    }
    pub(super) fn render_merged_banner_66(&self, use_scope: String) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = vec![
                wire::Node::Container { shadow : Default::default(), max_width : None,
                max_height : None, clip : false, key : format!("{}/@container:913",
                use_scope), width : Some(wire::Length::Fixed(24.0f32)), height :
                Some(wire::Length::Fixed(24.0f32)), padding : None, align_x :
                Some(wire::AlignX::Center), align_y : Some(wire::AlignY::Center),
                background : None, border : None, snap : None, content :
                Box::new(native::text_options(native::text(format!("{}/@text:923",
                use_scope), "✓".to_owned().to_string(),), wire::TextOptions { wrapping
                : Some(wire::Wrapping::None), ..Default::default() },),), },
                native::sized(native::text_options(native::text(format!("{}/@text:929",
                use_scope), crate
                ::host::forge_merge_note(::std::convert::AsRef::as_ref(& self
                .forge_item_merge_oid), ::std::convert::AsRef::as_ref(& self
                .forge_item_branches),).to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },),
                Some(wire::Length::Fill), None,)
            ];
            wire::Node::Linear {
                max_width: None,
                clip: false,
                key: node_scope.clone(),
                wrap: None,
                axis: wire::Axis::Row,
                spacing: Some(9.0f32),
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
    pub(super) fn render_merge_advisory_67(&self, use_scope: String) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = Vec::new();
            if self.forge_item_change_requests == 1 {
                children
                    .push({
                        let mut children: Vec<wire::Node> = vec![
                            native::sized(native::container(format!("{}/@container:950",
                            use_scope), wire::Node::Space { width :
                            Some(wire::Length::Fixed(1.0f32)), height :
                            Some(wire::Length::Fixed(1.0f32)), },),
                            Some(wire::Length::Fixed(6.0f32)),
                            Some(wire::Length::Fixed(6.0f32)),),
                            native::sized(native::text(format!("{}/@text:957",
                            use_scope),
                            "a reviewer requested changes — merge not recommended"
                            .to_owned().to_string(),), Some(wire::Length::Fill), None,)
                        ];
                        wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:945", use_scope),
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
                    });
            }
            if self.forge_item_change_requests > 1 {
                children
                    .push({
                        let mut children: Vec<wire::Node> = vec![
                            native::sized(native::container(format!("{}/@container:968",
                            use_scope), wire::Node::Space { width :
                            Some(wire::Length::Fixed(1.0f32)), height :
                            Some(wire::Length::Fixed(1.0f32)), },),
                            Some(wire::Length::Fixed(6.0f32)),
                            Some(wire::Length::Fixed(6.0f32)),),
                            native::text_options(native::text(format!("{}/@text:975",
                            use_scope), self.forge_item_change_requests.to_string(),),
                            wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                            ..Default::default() },),
                            native::sized(native::text(format!("{}/@text:981",
                            use_scope),
                            "reviewers requested changes — merge not recommended"
                            .to_owned().to_string(),), Some(wire::Length::Fill), None,)
                        ];
                        wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:963", use_scope),
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
                    });
            }
            native::sized(
                native::column(node_scope.clone(), children),
                Some(wire::Length::Fill),
                None,
            )
        }
    }
    pub(super) fn render_merge_button_69(
        &self,
        use_scope: String,
        cb_7: impl Fn() -> Message + Clone + 'static,
    ) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = Vec::new();
            if self.merge_busy {
                children
                    .push(wire::Node::Button {
                        checked: None,
                        expanded: None,
                        description: None,
                        key: format!("{}/@button:993", use_scope),
                        content: wire::ButtonContent::Child(
                            Box::new({
                                let mut children: Vec<wire::Node> = vec![
                                    native::text_options(native::text(format!("{}/@text:1002",
                                    use_scope), "Merging…".to_owned().to_string(),),
                                    wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                                    ..Default::default() },)
                                ];
                                wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:1001", use_scope),
                                    wrap: None,
                                    axis: wire::Axis::Row,
                                    spacing: Some(7.0f32),
                                    padding: None,
                                    width: None,
                                    height: None,
                                    align: Some(wire::AlignX::Center),
                                    background: None,
                                    border: None,
                                    children: children,
                                }
                            }),
                        ),
                        label: Some(String::from("Merging".to_owned())),
                        on_press: None,
                        width: None,
                        height: None,
                        padding: Some(wire::Edges {
                            top: 9f32,
                            right: 18f32,
                            bottom: 9f32,
                            left: 18f32,
                        }),
                        style: wire::ButtonStyle::default(),
                    });
            }
            if !self.merge_busy {
                children
                    .push(wire::Node::Button {
                        checked: None,
                        expanded: None,
                        description: None,
                        key: format!("{}/@button:1009", use_scope),
                        content: wire::ButtonContent::Child(
                            Box::new({
                                let mut children: Vec<wire::Node> = vec![
                                    self.icon(format!("{}/Icon@2186", use_scope),
                                    "pull-request", 13f32, "@media:76",),
                                    native::text_options(native::text(format!("{}/@text:1023",
                                    use_scope), "Merge pull request".to_owned().to_string(),),
                                    wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                                    ..Default::default() },)
                                ];
                                wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:1017", use_scope),
                                    wrap: None,
                                    axis: wire::Axis::Row,
                                    spacing: Some(7.0f32),
                                    padding: None,
                                    width: None,
                                    height: None,
                                    align: Some(wire::AlignX::Center),
                                    background: None,
                                    border: None,
                                    children: children,
                                }
                            }),
                        ),
                        label: Some(String::from("Merge pull request".to_owned())),
                        on_press: if !self.connected
                            || self.forge_item_source_oid.is_empty()
                        {
                            None
                        } else {
                            Some(::ducktape_view_guest::slots::message(cb_7()))
                        },
                        width: None,
                        height: None,
                        padding: Some(wire::Edges {
                            top: 9f32,
                            right: 18f32,
                            bottom: 9f32,
                            left: 18f32,
                        }),
                        style: wire::ButtonStyle::default(),
                    });
            }
            native::column(node_scope.clone(), children)
        }
    }
    pub(super) fn render_review_verdict_71(
        &self,
        use_scope: String,
        arg_0: String,
    ) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = Vec::new();
            if arg_0 == "approve" {
                children
                    .push(
                        native::text_options(
                            native::text(
                                format!("{}/@text:1514", use_scope),
                                crate::host::verdict_label(
                                        ::std::convert::AsRef::as_ref(&arg_0),
                                    )
                                    .to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                    );
            }
            if !(arg_0 == "approve") && arg_0 == "request_changes" {
                children
                    .push(
                        native::text_options(
                            native::text(
                                format!("{}/@text:1521", use_scope),
                                crate::host::verdict_label(
                                        ::std::convert::AsRef::as_ref(&arg_0),
                                    )
                                    .to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                    );
            }
            if !(arg_0 == "approve" || arg_0 == "request_changes") {
                children
                    .push(
                        native::text_options(
                            native::text(
                                format!("{}/@text:1528", use_scope),
                                crate::host::verdict_label(
                                        ::std::convert::AsRef::as_ref(&arg_0),
                                    )
                                    .to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                    );
            }
            native::column(node_scope.clone(), children)
        }
    }
    pub(super) fn render_review_card_76(
        &self,
        use_scope: String,
        cb_15: impl Fn(String) -> Message + Clone + 'static,
        arg_0: crate::host::ForgeReview,
    ) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        native::padded(
            native::sized(
                native::container(
                    node_scope.clone(),
                    {
                        let mut children: Vec<wire::Node> = vec![
                            { let mut children : Vec < wire::Node > =
                            vec![native::text_options(native::text(format!("{}/@text:1441",
                            use_scope), arg_0.author_name.to_owned().to_string(),),
                            wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                            ..Default::default() },), self
                            .render_review_verdict_71(format!("{}/ReviewVerdict@2615",
                            use_scope), arg_0.verdict.to_owned(),),
                            native::text_options(native::text(format!("{}/@text:1448",
                            use_scope), arg_0.commit.to_owned().to_string(),),
                            wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                            ..Default::default() },)]; if arg_0.outdated { children
                            .push(native::padded(native::container(format!("{}/@container:1455",
                            use_scope),
                            native::text_options(native::text(format!("{}/@text:1461",
                            use_scope), "outdated".to_owned().to_string(),),
                            wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                            ..Default::default() },),), wire::Edges { top : 2.0f32, right
                            : 6.0f32, bottom : 2.0f32, left : 6.0f32, },),); } children
                            .push(wire::Node::Space { width : Some(wire::Length::Fill),
                            height : None, }); children.push(self
                            .finality(format!("{}/FinalityChip@2636",
                            use_scope), arg_0.created_at,),); wire::Node::Linear {
                            max_width : None, clip : false, key :
                            format!("{}/@layout:1436", use_scope), wrap : None, axis :
                            wire::Axis::Row, spacing : Some(7.0f32), padding : None,
                            width : Some(wire::Length::Fill), height : None, align :
                            Some(wire::AlignX::Center), background : None, border : None,
                            children : children, } }
                        ];
                        if !arg_0.body.is_empty() {
                            children
                                .push(
                                    self
                                        .rich_body(
                                            format!("{}/RichBody@2638", use_scope),
                                            cb_15.clone(),
                                            arg_0.blocks.clone(),
                                        ),
                                );
                        }
                        for (index, comment) in arg_0.comments.iter().enumerate() {
                            let for_scope = format!(
                                "{}/@for:2641({})", use_scope, index
                            );
                            children
                                .push(
                                    native::padded(
                                        native::sized(
                                            native::container(
                                                format!("{}/@container:1474", for_scope),
                                                {
                                                    let mut children: Vec<wire::Node> = vec![
                                                        { let mut children : Vec < wire::Node > =
                                                        vec![native::sized(native::text_options(native::text(format!("{}/@text:1491",
                                                        for_scope), comment.anchor.to_owned().to_string(),),
                                                        wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                                                        ..Default::default() },), Some(wire::Length::Fill), None,),
                                                        native::text_options(native::text(format!("{}/@text:1498",
                                                        for_scope), "review comment".to_owned().to_string(),),
                                                        wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                                                        ..Default::default() },)]; wire::Node::Linear { max_width :
                                                        None, clip : false, key : format!("{}/@layout:1486",
                                                        for_scope), wrap : None, axis : wire::Axis::Row, spacing :
                                                        Some(7.0f32), padding : None, width :
                                                        Some(wire::Length::Fill), height : None, align :
                                                        Some(wire::AlignX::Center), background : None, border :
                                                        None, children : children, } }, self
                                                        .rich_body(format!("{}/RichBody@2672", for_scope),
                                                        cb_15.clone(), comment.blocks.clone(),)
                                                    ];
                                                    native::spaced(
                                                        native::sized(
                                                            native::column(
                                                                format!("{}/@layout:1485", for_scope),
                                                                children,
                                                            ),
                                                            Some(wire::Length::Fill),
                                                            None,
                                                        ),
                                                        4.0f32,
                                                    )
                                                },
                                            ),
                                            Some(wire::Length::Fill),
                                            None,
                                        ),
                                        wire::Edges {
                                            top: 9.0f32,
                                            right: 11.0f32,
                                            bottom: 9.0f32,
                                            left: 11.0f32,
                                        },
                                    ),
                                );
                        }
                        native::spaced(
                            native::sized(
                                native::column(
                                    format!("{}/@layout:1435", use_scope),
                                    children,
                                ),
                                Some(wire::Length::Fill),
                                None,
                            ),
                            6.0f32,
                        )
                    },
                ),
                Some(wire::Length::Fill),
                None,
            ),
            wire::Edges {
                top: 11.0f32,
                right: 13.0f32,
                bottom: 11.0f32,
                left: 13.0f32,
            },
        )
    }
}
