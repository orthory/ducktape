use super::*;
impl super::ForgeView {
    fn tracker_list(
        &self,
        key: String,
        tab: &str,
        open: impl Fn(i64) -> Message + Clone + 'static,
    ) -> wire::Node {
        match self.repo_phase.as_str() {
            "loading" => self.loading_tracker(key),
            "failed" => self.tracker_unavailable(key),
            "ready" => {
                let items = crate::host::filter_forge_items(&self.items, tab);
                if items.is_empty() {
                    return match tab {
                        "issues" => self.empty_issues(key),
                        _ => self.empty_pulls(key),
                    };
                }
                native::column(
                    key.clone(),
                    items.into_iter().map(|item| {
                        self.tracker_item(format!("{key}/{}", item.number), open.clone(), item)
                    }),
                )
            }
            _ => native::column(key, []),
        }
    }
    fn code_notice(&self, key: String, title: &str, note: &str) -> wire::Node {
        let mut children = Vec::new();
        if !title.is_empty() {
            children.push(native::heading(format!("{key}/title"), title));
        }
        children.push(native::text(
            format!("{key}/context"),
            "Synced from the node · view only",
        ));
        if !note.is_empty() {
            children.push(native::text(format!("{key}/note"), note));
        }
        native::column(key, children)
    }
    pub(super) fn organization_header(&self, key: String) -> wire::Node {
        let mut children = vec![native::heading(format!("{key}/name"), &self.org)];
        if self.list_phase == "ready" {
            let count = crate::host::plural(self.repos.len() as i64, "repository", "repositories");
            children.push(native::text(format!("{key}/repositories"), count));
            if !self.tier.is_empty() {
                children.push(native::text(format!("{key}/tier"), &self.tier));
            }
        }
        if !self.about.is_empty() {
            children.push(native::text(format!("{key}/about"), &self.about));
        }
        native::column(key, children)
    }
    pub(super) fn repository(
        &self,
        key: String,
        open: impl Fn(String) -> Message + Clone + 'static,
        repo: crate::host::ForgeRepo,
    ) -> wire::Node {
        let content = native::column(
            format!("{key}/content"),
            [
                native::heading(format!("{key}/name"), &repo.name),
                native::text(format!("{key}/head"), &repo.head),
            ],
        );
        let mut button = native::button_child(
            key,
            content,
            Some(ducktape_view_guest::slots::message(open(repo.name.clone()))),
            Default::default(),
        );
        if let wire::Node::Button {
            label,
            description,
            width,
            ..
        } = &mut button
        {
            *label = Some("Open repo".into());
            *description = Some(repo.name);
            *width = Some(wire::Length::Fill);
        }
        button
    }
    pub(super) fn repository_breadcrumb(&self, key: String) -> wire::Node {
        native::text(key, format!("{}/", self.org))
    }
    pub(super) fn back_to_tracker(
        &self,
        key: String,
        back: impl Fn() -> Message + Clone + 'static,
    ) -> wire::Node {
        let label = match self.forge_item_kind.as_str() {
            "pr" => "Back to pull requests",
            "issue" => "Back to issues",
            _ => return native::column(key, []),
        };
        native::button(
            key,
            label,
            Some(ducktape_view_guest::slots::message(back())),
            Default::default(),
        )
    }
    pub(super) fn tree_root(&self, key: String) -> wire::Node {
        native::text(key, "▾ /")
    }
    pub(super) fn tree_directory(&self, key: String, name: String) -> wire::Node {
        native::text(key, format!("▸ {name}/"))
    }

    pub(super) fn tree_file(&self, key: String, name: String, selected: bool) -> wire::Node {
        let content = if selected {
            format!("• {name}")
        } else {
            name
        };
        native::text(key, content)
    }
    pub(super) fn code_header(&self, key: String) -> wire::Node {
        native::heading(
            key,
            crate::host::forge_file_header(
                &self.opened_dir,
                &self.opened_rev,
                &self.tree_path,
                &self.tree_rev,
                &self.file_path,
            ),
        )
    }
    pub(super) fn loading_tree(&self, key: String) -> wire::Node {
        self.code_notice(key, "", "Loading repository files…")
    }
    pub(super) fn loading_file(&self, key: String) -> wire::Node {
        self.code_notice(key, &self.file_path, "Loading file…")
    }
    pub(super) fn code_unavailable(&self, key: String) -> wire::Node {
        self.code_notice(key, "", "Could not load code. Pick Code to try again.")
    }
    pub(super) fn file_note(&self, key: String) -> wire::Node {
        self.code_notice(key, &self.file_path, &self.file_note)
    }
    pub(super) fn uncommitted_repository(&self, key: String) -> wire::Node {
        self.code_notice(
            key,
            "",
            "Nothing is committed on this repository yet, so there is no file to read.",
        )
    }
    pub(super) fn empty_commit(&self, key: String) -> wire::Node {
        self.code_notice(key, "", "This commit has no files to read.")
    }
    pub(super) fn omitted_directory(&self, key: String) -> wire::Node {
        self.code_notice(
            key,
            "",
            "This directory has entries outside the browser's display limits.",
        )
    }
    pub(super) fn choose_file(&self, key: String) -> wire::Node {
        self.code_notice(key, "", "Pick a file from the tree to read it.")
    }
    pub(super) fn binary_file(&self, key: String) -> wire::Node {
        self.code_notice(
            key,
            &self.file_path,
            &crate::host::binary_note(&self.file_text),
        )
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
            let children: Vec<wire::Node> = vec![
                {
                    let node_scope = format!("{}/tree-pane", node_scope);
                    native::sized(
                        native::container(
                            node_scope.clone(),
                            wire::Node::Scroll {
                                on_scroll: None,
                                virtual_rows: false,
                                key: format!("{}/@layout:241", use_scope),
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
                                    let children: Vec<wire::Node> = vec![
                                        native::padded(
                                            native::sized(
                                                native::container(
                                                    format!("{}/@container:251", use_scope),
                                                    native::text_options(
                                                        native::text(
                                                            format!("{}/@text:258", use_scope),
                                                            "FILES".to_owned().to_string(),
                                                        ),
                                                        wire::TextOptions {
                                                            wrapping: Some(wire::Wrapping::None),
                                                            ..Default::default()
                                                        },
                                                    ),
                                                ),
                                                Some(wire::Length::Fill),
                                                None,
                                            ),
                                            wire::Edges {
                                                top: 5.0f32,
                                                right: 16.0f32,
                                                bottom: 8.0f32,
                                                left: 16.0f32,
                                            },
                                        ),
                                        {
                                            let mut children: Vec<wire::Node> = Vec::new();
                                            if self.tree_phase == "loading" {
                                                children.push(native::padded(
                                                    native::sized(
                                                        native::container(
                                                            format!("{}/@container:831", use_scope),
                                                            native::sized(
                                                                native::text(
                                                                    format!(
                                                                        "{}/@text:837",
                                                                        use_scope
                                                                    ),
                                                                    "Loading repository files…"
                                                                        .to_owned()
                                                                        .to_string(),
                                                                ),
                                                                Some(wire::Length::Fill),
                                                                None,
                                                            ),
                                                        ),
                                                        Some(wire::Length::Fill),
                                                        None,
                                                    ),
                                                    wire::Edges {
                                                        top: 8.0f32,
                                                        right: 16.0f32,
                                                        bottom: 0.0f32,
                                                        left: 16.0f32,
                                                    },
                                                ));
                                            }
                                            if self.tree_phase == "failed" {
                                                children
                .push(native::padded(native::sized(native::container(format!("{}/@container:844",
                use_scope), native::sized(native::text(format!("{}/@text:850",
                use_scope), "Could not load code. Pick Code to try again.".to_owned()
                .to_string(),), Some(wire::Length::Fill), None,),),
                Some(wire::Length::Fill), None,), wire::Edges { top : 8.0f32, right :
                16.0f32, bottom : 0.0f32, left : 16.0f32, },),);
                                            }
                                            if self.tree_phase == "ready" {
                                                children.push({ let mut children : Vec < wire::Node > =
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
                .tree_root(format!("{}/ForgeTreeDirRow@3647",
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
                .tree_directory(format!("{}/ForgeTreeDirRow@3665",
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
                .tree_file(format!("{}/ForgeTreeFileRow@3681",
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
                use_scope), children,), Some(wire::Length::Fill), None,) });
                                            }
                                            native::sized(
                                                native::column(
                                                    format!("{}/@layout:829", use_scope),
                                                    children,
                                                ),
                                                Some(wire::Length::Fill),
                                                None,
                                            )
                                        },
                                    ];
                                    native::padded(
                                        native::sized(
                                            native::column(
                                                format!("{}/@layout:246", use_scope),
                                                children,
                                            ),
                                            Some(wire::Length::Fill),
                                            None,
                                        ),
                                        wire::Edges {
                                            top: 9.0f32,
                                            right: 0.0f32,
                                            bottom: 9.0f32,
                                            left: 0.0f32,
                                        },
                                    )
                                }),
                            },
                        ),
                        Some(wire::Length::Fixed(self.tree_width as f32)),
                        Some(wire::Length::Fill),
                    )
                },
                {
                    let node_scope = format!("{}/tree-resize", node_scope);
                    wire::Node::ResizeHandle {
                        key: node_scope.clone(),
                        on_press: None,
                        on_release: None,
                        on_drag: Some(
                            ::ducktape_view_guest::slots::handler::<(f64, f64), Message>(Box::new(
                                {
                                    let route = {
                                        let route_callback = cb_3.clone();
                                        move |delta: (f64, f64)| route_callback(delta.0, delta.1)
                                    };
                                    move |sent: (f64, f64)| Some(route(sent))
                                },
                            )),
                        ),
                        cursor: Some(wire::mouse::Cursor::ResizingHorizontally),
                        content: Box::new({
                            let node_scope = format!("{}/tree-divider", node_scope);
                            wire::Node::Container {
                                shadow: Default::default(),
                                max_width: None,
                                max_height: None,
                                clip: false,
                                key: node_scope.clone(),
                                width: Some(wire::Length::Fixed(10.0f32)),
                                height: Some(wire::Length::Fill),
                                padding: None,
                                align_x: Some(wire::AlignX::Left),
                                align_y: None,
                                background: None.map(wire::Background::Color),
                                border: None,
                                snap: None,
                                content: Box::new(native::sized(
                                    native::container(
                                        format!("{}/@container:271", use_scope),
                                        wire::Node::Space {
                                            width: Some(wire::Length::Fixed(1.0f32)),
                                            height: Some(wire::Length::Fixed(1.0f32)),
                                        },
                                    ),
                                    Some(wire::Length::Fixed(1.0f32)),
                                    Some(wire::Length::Fill),
                                )),
                            }
                        }),
                    }
                },
                {
                    let children: Vec<wire::Node> = vec![
                        self.code_header(format!("{}/ForgeCodeHeader@1442", use_scope)),
                        wire::Node::Scroll {
                            on_scroll: None,
                            virtual_rows: false,
                            key: format!("{}/@layout:280", use_scope),
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
                                if self.tree_phase == "loading" {
                                    children.push(self.loading_tree(format!(
                                        "{}/ForgeCodeEmpty@3701",
                                        use_scope
                                    )));
                                }
                                if self.file_phase == "loading"
                                    && !crate::host::forge_file_header(
                                        ::std::convert::AsRef::as_ref(&self.opened_dir),
                                        ::std::convert::AsRef::as_ref(&self.opened_rev),
                                        ::std::convert::AsRef::as_ref(&self.tree_path),
                                        ::std::convert::AsRef::as_ref(&self.tree_rev),
                                        ::std::convert::AsRef::as_ref(&self.file_path),
                                    )
                                    .is_empty()
                                {
                                    children.push(self.loading_file(format!(
                                        "{}/ForgeCodeEmpty@3703",
                                        use_scope
                                    )));
                                }
                                if self.tree_phase == "failed" {
                                    children.push(self.code_unavailable(format!(
                                        "{}/ForgeCodeEmpty@3705",
                                        use_scope
                                    )));
                                }
                                if self.file_phase == "failed"
                                    && !crate::host::forge_file_header(
                                        ::std::convert::AsRef::as_ref(&self.opened_dir),
                                        ::std::convert::AsRef::as_ref(&self.opened_rev),
                                        ::std::convert::AsRef::as_ref(&self.tree_path),
                                        ::std::convert::AsRef::as_ref(&self.tree_rev),
                                        ::std::convert::AsRef::as_ref(&self.file_path),
                                    )
                                    .is_empty()
                                {
                                    children.push(
                                        self.file_note(format!(
                                            "{}/ForgeCodeEmpty@3707",
                                            use_scope
                                        )),
                                    );
                                }
                                if self.tree_phase == "ready"
                                    && crate::host::forge_file_header(
                                        ::std::convert::AsRef::as_ref(&self.opened_dir),
                                        ::std::convert::AsRef::as_ref(&self.opened_rev),
                                        ::std::convert::AsRef::as_ref(&self.tree_path),
                                        ::std::convert::AsRef::as_ref(&self.tree_rev),
                                        ::std::convert::AsRef::as_ref(&self.file_path),
                                    )
                                    .is_empty()
                                    && self.tree_entries.is_empty()
                                    && !self.tree_born
                                {
                                    children.push(self.uncommitted_repository(format!(
                                        "{}/ForgeCodeEmpty@3709",
                                        use_scope
                                    )));
                                }
                                if self.tree_phase == "ready"
                                    && crate::host::forge_file_header(
                                        ::std::convert::AsRef::as_ref(&self.opened_dir),
                                        ::std::convert::AsRef::as_ref(&self.opened_rev),
                                        ::std::convert::AsRef::as_ref(&self.tree_path),
                                        ::std::convert::AsRef::as_ref(&self.tree_rev),
                                        ::std::convert::AsRef::as_ref(&self.file_path),
                                    )
                                    .is_empty()
                                    && self.tree_entries.is_empty()
                                    && self.tree_born
                                    && !self.tree_truncated
                                {
                                    children.push(self.empty_commit(format!(
                                        "{}/ForgeCodeEmpty@3714",
                                        use_scope
                                    )));
                                }
                                if self.tree_phase == "ready"
                                    && crate::host::forge_file_header(
                                        ::std::convert::AsRef::as_ref(&self.opened_dir),
                                        ::std::convert::AsRef::as_ref(&self.opened_rev),
                                        ::std::convert::AsRef::as_ref(&self.tree_path),
                                        ::std::convert::AsRef::as_ref(&self.tree_rev),
                                        ::std::convert::AsRef::as_ref(&self.file_path),
                                    )
                                    .is_empty()
                                    && self.tree_entries.is_empty()
                                    && self.tree_born
                                    && self.tree_truncated
                                {
                                    children.push(self.omitted_directory(format!(
                                        "{}/ForgeCodeEmpty@3716",
                                        use_scope
                                    )));
                                }
                                if self.tree_phase == "ready"
                                    && crate::host::forge_file_header(
                                        ::std::convert::AsRef::as_ref(&self.opened_dir),
                                        ::std::convert::AsRef::as_ref(&self.opened_rev),
                                        ::std::convert::AsRef::as_ref(&self.tree_path),
                                        ::std::convert::AsRef::as_ref(&self.tree_rev),
                                        ::std::convert::AsRef::as_ref(&self.file_path),
                                    )
                                    .is_empty()
                                    && !self.tree_entries.is_empty()
                                {
                                    children.push(
                                        self.choose_file(format!(
                                            "{}/ForgeCodeEmpty@3721",
                                            use_scope
                                        )),
                                    );
                                }
                                if self.file_phase == "ready"
                                    && !crate::host::forge_file_header(
                                        ::std::convert::AsRef::as_ref(&self.opened_dir),
                                        ::std::convert::AsRef::as_ref(&self.opened_rev),
                                        ::std::convert::AsRef::as_ref(&self.tree_path),
                                        ::std::convert::AsRef::as_ref(&self.tree_rev),
                                        ::std::convert::AsRef::as_ref(&self.file_path),
                                    )
                                    .is_empty()
                                    && self.file_binary
                                {
                                    children.push(
                                        self.binary_file(format!(
                                            "{}/ForgeCodeEmpty@3726",
                                            use_scope
                                        )),
                                    );
                                }
                                if self.file_phase == "ready"
                                    && !crate::host::forge_file_header(
                                        ::std::convert::AsRef::as_ref(&self.opened_dir),
                                        ::std::convert::AsRef::as_ref(&self.opened_rev),
                                        ::std::convert::AsRef::as_ref(&self.tree_path),
                                        ::std::convert::AsRef::as_ref(&self.tree_rev),
                                        ::std::convert::AsRef::as_ref(&self.file_path),
                                    )
                                    .is_empty()
                                    && !self.file_binary
                                    && self.file_picture
                                {
                                    children.push({
                                        let children: Vec<wire::Node> = vec![
                                            {
                                                let node_scope =
                                                    format!("{}/forge-picture", node_scope);
                                                wire::Node::Surface {
                                                    key: node_scope.clone(),
                                                    name: String::from("picture"),
                                                    args: ::std::vec![
                                                        {
                                                            let surface_arg = &("forge".to_owned());
                                                            wire::SurfaceValue::Str(
                                                                ::std::string::ToString::to_string(
                                                                    surface_arg,
                                                                ),
                                                            )
                                                        },
                                                        {
                                                            let surface_arg =
                                                                &(self.file_path.to_owned());
                                                            wire::SurfaceValue::Str(
                                                                ::std::string::ToString::to_string(
                                                                    surface_arg,
                                                                ),
                                                            )
                                                        }
                                                    ],
                                                    on_event: None,
                                                }
                                            },
                                            native::text_options(
                                                native::text(
                                                    format!("{}/@text:996", use_scope),
                                                    crate::host::picture_caption(
                                                        self.file_width,
                                                        self.file_height,
                                                    )
                                                    .to_string(),
                                                ),
                                                wire::TextOptions {
                                                    wrapping: Some(wire::Wrapping::None),
                                                    ..Default::default()
                                                },
                                            ),
                                        ];
                                        native::spaced(
                                            native::padded(
                                                native::sized(
                                                    native::column(
                                                        format!("{}/@layout:988", use_scope),
                                                        children,
                                                    ),
                                                    Some(wire::Length::Fill),
                                                    None,
                                                ),
                                                wire::Edges {
                                                    top: 13.0f32,
                                                    right: 16.0f32,
                                                    bottom: 13.0f32,
                                                    left: 16.0f32,
                                                },
                                            ),
                                            9.0f32,
                                        )
                                    });
                                }
                                if self.file_phase == "ready"
                                    && !crate::host::forge_file_header(
                                        ::std::convert::AsRef::as_ref(&self.opened_dir),
                                        ::std::convert::AsRef::as_ref(&self.opened_rev),
                                        ::std::convert::AsRef::as_ref(&self.tree_path),
                                        ::std::convert::AsRef::as_ref(&self.tree_rev),
                                        ::std::convert::AsRef::as_ref(&self.file_path),
                                    )
                                    .is_empty()
                                    && !self.file_binary
                                    && !self.file_picture
                                    && crate::host::markdown_path(::std::convert::AsRef::as_ref(
                                        &self.file_path,
                                    ))
                                {
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
                9.0f32,) });
                                }
                                if self.file_phase == "ready"
                                    && !crate::host::forge_file_header(
                                        ::std::convert::AsRef::as_ref(&self.opened_dir),
                                        ::std::convert::AsRef::as_ref(&self.opened_rev),
                                        ::std::convert::AsRef::as_ref(&self.tree_path),
                                        ::std::convert::AsRef::as_ref(&self.tree_rev),
                                        ::std::convert::AsRef::as_ref(&self.file_path),
                                    )
                                    .is_empty()
                                    && !self.file_binary
                                    && !self.file_picture
                                    && !crate::host::markdown_path(::std::convert::AsRef::as_ref(
                                        &self.file_path,
                                    ))
                                {
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
                9.0f32,) });
                                }
                                native::sized(
                                    native::column(format!("{}/@layout:956", use_scope), children),
                                    Some(wire::Length::Fill),
                                    None,
                                )
                            }),
                        },
                    ];
                    native::sized(
                        native::column(format!("{}/@layout:273", use_scope), children),
                        Some(wire::Length::Fill),
                        Some(wire::Length::Fill),
                    )
                },
            ];
            native::sized(
                native::row(node_scope.clone(), children),
                Some(wire::Length::Fill),
                Some(wire::Length::Fill),
            )
        }
    }

    pub(super) fn tracker_item(
        &self,
        key: String,
        open: impl Fn(i64) -> Message + Clone + 'static,
        item: crate::host::ForgeItem,
    ) -> wire::Node {
        let content = native::column(
            format!("{key}/content"),
            [
                native::heading(format!("{key}/title"), &item.title),
                native::text(
                    format!("{key}/details"),
                    format!(
                        "{} · {} · #{} · opened by {}",
                        item.kind, item.state, item.number, item.author_name
                    ),
                ),
            ],
        );
        let mut button = native::button_child(
            key,
            content,
            Some(ducktape_view_guest::slots::message(open(item.number))),
            Default::default(),
        );
        if let wire::Node::Button {
            label,
            description,
            width,
            ..
        } = &mut button
        {
            *label = Some("Open item".into());
            *description = Some(item.title);
            *width = Some(wire::Length::Fill);
        }
        button
    }
    pub(super) fn issues(
        &self,
        key: String,
        open: impl Fn(i64) -> Message + Clone + 'static,
    ) -> wire::Node {
        self.tracker_list(key, "issues", open)
    }
    pub(super) fn pull_requests(
        &self,
        key: String,
        open: impl Fn(i64) -> Message + Clone + 'static,
    ) -> wire::Node {
        self.tracker_list(key, "pulls", open)
    }
    pub(super) fn pull_request_status(&self, key: String) -> wire::Node {
        let label = match self.forge_item_state.as_str() {
            "open" => "Open",
            "merged" => "Merged",
            _ => "Closed",
        };
        native::text(key, label)
    }
    pub(super) fn render_diff_count_56(&self, use_scope: String) -> wire::Node {
        let node_scope = format!("{}/root", use_scope);
        {
            let mut children: Vec<wire::Node> = vec![
                {
                    let children: Vec<wire::Node> = vec![
                        native::text_options(
                            native::text(
                                format!("{}/@text:789", use_scope),
                                "+".to_owned().to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                        native::text_options(
                            native::text(
                                format!("{}/@text:795", use_scope),
                                self.forge_item_additions.to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                    ];
                    wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:788", use_scope),
                        wrap: None,
                        axis: wire::Axis::Row,
                        spacing: Some(0.0f32),
                        padding: None,
                        width: None,
                        height: None,
                        align: Some(wire::AlignX::Center),
                        background: None,
                        border: None,
                        children: children,
                    }
                },
                {
                    let children: Vec<wire::Node> = vec![
                        native::text_options(
                            native::text(
                                format!("{}/@text:802", use_scope),
                                "−".to_owned().to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                        native::text_options(
                            native::text(
                                format!("{}/@text:808", use_scope),
                                self.forge_item_deletions.to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                    ];
                    wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:801", use_scope),
                        wrap: None,
                        axis: wire::Axis::Row,
                        spacing: Some(0.0f32),
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
            if self.forge_item_files_changed > 0 {
                children.push({
                    let children: Vec<wire::Node> = vec![
                        native::text_options(
                            native::text(
                                format!("{}/@text:816", use_scope),
                                "·".to_owned().to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                        native::text_options(
                            native::text(
                                format!("{}/@text:822", use_scope),
                                self.forge_item_files_changed.to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                        native::text_options(
                            native::text(
                                format!("{}/@text:828", use_scope),
                                "files".to_owned().to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
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
                let children: Vec<wire::Node> = vec![
                    native::padded(
                        native::sized(
                            native::container(format!("{}/@container:858", use_scope), {
                                let children: Vec<wire::Node> = vec![
                                    native::text_options(
                                        native::text(
                                            format!("{}/@text:871", use_scope),
                                            "opened by".to_owned().to_string(),
                                        ),
                                        wire::TextOptions {
                                            wrapping: Some(wire::Wrapping::None),
                                            ..Default::default()
                                        },
                                    ),
                                    native::text_options(
                                        native::text(
                                            format!("{}/@text:876", use_scope),
                                            self.forge_item_author.to_owned().to_string(),
                                        ),
                                        wire::TextOptions {
                                            wrapping: Some(wire::Wrapping::None),
                                            ..Default::default()
                                        },
                                    ),
                                ];
                                wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:866", use_scope),
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
                            top: 8.0f32,
                            right: 13.0f32,
                            bottom: 8.0f32,
                            left: 13.0f32,
                        },
                    ),
                    native::sized(
                        native::container(
                            format!("{}/@container:882", use_scope),
                            wire::Node::Space {
                                width: Some(wire::Length::Fixed(1.0f32)),
                                height: Some(wire::Length::Fixed(1.0f32)),
                            },
                        ),
                        Some(wire::Length::Fill),
                        Some(wire::Length::Fixed(1.0f32)),
                    ),
                    native::padded(
                        native::sized(
                            native::container(
                                format!("{}/@container:888", use_scope),
                                self.item_body(
                                    format!("{}/RichBody@2067", use_scope),
                                    cb_15.clone(),
                                ),
                            ),
                            Some(wire::Length::Fill),
                            None,
                        ),
                        wire::Edges {
                            top: 13.0f32,
                            right: 15.0f32,
                            bottom: 13.0f32,
                            left: 15.0f32,
                        },
                    ),
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
                {
                    let children: Vec<wire::Node> = vec![
                        native::text_options(
                            native::text(
                                format!("{}/@text:789", use_scope),
                                "+".to_owned().to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                        native::text_options(
                            native::text(
                                format!("{}/@text:795", use_scope),
                                self.forge_item_additions.to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                    ];
                    wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:788", use_scope),
                        wrap: None,
                        axis: wire::Axis::Row,
                        spacing: Some(0.0f32),
                        padding: None,
                        width: None,
                        height: None,
                        align: Some(wire::AlignX::Center),
                        background: None,
                        border: None,
                        children: children,
                    }
                },
                {
                    let children: Vec<wire::Node> = vec![
                        native::text_options(
                            native::text(
                                format!("{}/@text:802", use_scope),
                                "−".to_owned().to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                        native::text_options(
                            native::text(
                                format!("{}/@text:808", use_scope),
                                self.forge_item_deletions.to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                    ];
                    wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:801", use_scope),
                        wrap: None,
                        axis: wire::Axis::Row,
                        spacing: Some(0.0f32),
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
            if false {
                children.push({
                    let children: Vec<wire::Node> = vec![
                        native::text_options(
                            native::text(
                                format!("{}/@text:816", use_scope),
                                "·".to_owned().to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                        native::text_options(
                            native::text(format!("{}/@text:822", use_scope), 0.to_string()),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
                        native::text_options(
                            native::text(
                                format!("{}/@text:828", use_scope),
                                "files".to_owned().to_string(),
                            ),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                ..Default::default()
                            },
                        ),
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
                children.push(native::padded(
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
                ));
            }
            if !(arg_0.kind == "file") && arg_0.kind == "hunk" {
                children.push(native::padded(
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
                ));
            }
            if !(arg_0.kind == "file" || arg_0.kind == "hunk") && arg_0.kind == "add" {
                children.push(native::sized(
                    native::container(format!("{}/@container:1154", use_scope), {
                        let mut children: Vec<wire::Node> = vec![wire::Node::Container {
                            shadow: Default::default(),
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:1160", use_scope),
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
                            content: Box::new(native::sized(
                                native::text_options(
                                    native::text(
                                        format!("{}/@text:1167", use_scope),
                                        arg_0.old_no.to_owned().to_string(),
                                    ),
                                    wire::TextOptions {
                                        wrapping: Some(wire::Wrapping::None),
                                        ..Default::default()
                                    },
                                ),
                                Some(wire::Length::Fill),
                                None,
                            )),
                        }];
                        if arg_0.path.is_empty() {
                            children.push(wire::Node::Container {
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
                                content: Box::new(native::sized(
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
                                )),
                            });
                        }
                        if !arg_0.path.is_empty() {
                            children.push(wire::Node::Button {
                                checked: None,
                                expanded: None,
                                description: None,
                                key: format!("{}/@button:1192", use_scope),
                                content: wire::ButtonContent::Child(Box::new(
                                    wire::Node::Container {
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
                                        content: Box::new(native::sized(
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
                                        )),
                                    },
                                )),
                                label: Some(String::from("Comment on this line".to_owned())),
                                on_press: Some(::ducktape_view_guest::slots::message(cb_0(
                                    arg_0.path.to_owned(),
                                    arg_0.new_no.to_owned(),
                                    "new".to_owned(),
                                ))),
                                width: Some(wire::Length::Fixed(34.0f32)),
                                height: Some(wire::Length::Fixed(20.0f32)),
                                padding: Some(wire::Edges::all(0.0f32)),
                                style: wire::ButtonStyle::default(),
                            });
                        }
                        children.push(wire::Node::Container {
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
                            content: Box::new(native::text_options(
                                native::text(
                                    format!("{}/@text:1221", use_scope),
                                    arg_0.sign.to_owned().to_string(),
                                ),
                                wire::TextOptions {
                                    wrapping: Some(wire::Wrapping::None),
                                    ..Default::default()
                                },
                            )),
                        });
                        children.push(wire::Node::Container {
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
                            content: Box::new(native::sized(
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
                            )),
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
                    }),
                    Some(wire::Length::Fill),
                    None,
                ));
            }
            if !(arg_0.kind == "file" || arg_0.kind == "hunk" || arg_0.kind == "add")
                && arg_0.kind == "del"
            {
                children.push(native::sized(
                    native::container(format!("{}/@container:1241", use_scope), {
                        let mut children: Vec<wire::Node> = Vec::new();
                        if arg_0.path.is_empty() {
                            children.push(wire::Node::Container {
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
                                content: Box::new(native::sized(
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
                                )),
                            });
                        }
                        if !arg_0.path.is_empty() {
                            children.push(wire::Node::Button {
                                checked: None,
                                expanded: None,
                                description: None,
                                key: format!("{}/@button:1264", use_scope),
                                content: wire::ButtonContent::Child(Box::new(
                                    wire::Node::Container {
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
                                        content: Box::new(native::sized(
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
                                        )),
                                    },
                                )),
                                label: Some(String::from(
                                    "Comment on this deleted line".to_owned(),
                                )),
                                on_press: Some(::ducktape_view_guest::slots::message(cb_0(
                                    arg_0.path.to_owned(),
                                    arg_0.old_no.to_owned(),
                                    "old".to_owned(),
                                ))),
                                width: Some(wire::Length::Fixed(34.0f32)),
                                height: Some(wire::Length::Fixed(20.0f32)),
                                padding: Some(wire::Edges::all(0.0f32)),
                                style: wire::ButtonStyle::default(),
                            });
                        }
                        children.push(wire::Node::Container {
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
                            content: Box::new(native::sized(
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
                            )),
                        });
                        children.push(wire::Node::Container {
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
                            content: Box::new(native::text_options(
                                native::text(
                                    format!("{}/@text:1308", use_scope),
                                    arg_0.sign.to_owned().to_string(),
                                ),
                                wire::TextOptions {
                                    wrapping: Some(wire::Wrapping::None),
                                    ..Default::default()
                                },
                            )),
                        });
                        children.push(wire::Node::Container {
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
                            content: Box::new(native::sized(
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
                            )),
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
                    }),
                    Some(wire::Length::Fill),
                    None,
                ));
            }
            if !(arg_0.kind == "file"
                || arg_0.kind == "hunk"
                || arg_0.kind == "add"
                || arg_0.kind == "del")
                && arg_0.kind == "ctx"
            {
                children.push(native::sized(
                    native::container(format!("{}/@container:1328", use_scope), {
                        let mut children: Vec<wire::Node> = vec![wire::Node::Container {
                            shadow: Default::default(),
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:1334", use_scope),
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
                            content: Box::new(native::sized(
                                native::text_options(
                                    native::text(
                                        format!("{}/@text:1341", use_scope),
                                        arg_0.old_no.to_owned().to_string(),
                                    ),
                                    wire::TextOptions {
                                        wrapping: Some(wire::Wrapping::None),
                                        ..Default::default()
                                    },
                                ),
                                Some(wire::Length::Fill),
                                None,
                            )),
                        }];
                        if arg_0.path.is_empty() {
                            children.push(wire::Node::Container {
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
                                content: Box::new(native::sized(
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
                                )),
                            });
                        }
                        if !arg_0.path.is_empty() {
                            children.push(wire::Node::Button {
                                checked: None,
                                expanded: None,
                                description: None,
                                key: format!("{}/@button:1366", use_scope),
                                content: wire::ButtonContent::Child(Box::new(
                                    wire::Node::Container {
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
                                        content: Box::new(native::sized(
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
                                        )),
                                    },
                                )),
                                label: Some(String::from("Comment on this line".to_owned())),
                                on_press: Some(::ducktape_view_guest::slots::message(cb_0(
                                    arg_0.path.to_owned(),
                                    arg_0.new_no.to_owned(),
                                    "new".to_owned(),
                                ))),
                                width: Some(wire::Length::Fixed(34.0f32)),
                                height: Some(wire::Length::Fixed(20.0f32)),
                                padding: Some(wire::Edges::all(0.0f32)),
                                style: wire::ButtonStyle::default(),
                            });
                        }
                        children.push(wire::Node::Container {
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
                            content: Box::new(native::text_options(
                                native::text(
                                    format!("{}/@text:1395", use_scope),
                                    arg_0.sign.to_owned().to_string(),
                                ),
                                wire::TextOptions {
                                    wrapping: Some(wire::Wrapping::None),
                                    ..Default::default()
                                },
                            )),
                        });
                        children.push(wire::Node::Container {
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
                            content: Box::new(native::sized(
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
                            )),
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
                    }),
                    Some(wire::Length::Fill),
                    None,
                ));
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
                let children: Vec<wire::Node> = vec![
                    native::padded(
                        native::sized(
                            native::container(format!("{}/@container:1063", use_scope), {
                                let children: Vec<wire::Node> = vec![
                                    self.icon(
                                        format!("{}/Icon@2244", use_scope),
                                        "branch",
                                        13f32,
                                        "@media:82",
                                    ),
                                    native::text_options(
                                        native::text(
                                            format!("{}/@text:1081", use_scope),
                                            self.forge_item_branches.to_owned().to_string(),
                                        ),
                                        wire::TextOptions {
                                            wrapping: Some(wire::Wrapping::None),
                                            ..Default::default()
                                        },
                                    ),
                                    self.render_diff_count_62(format!(
                                        "{}/DiffCount@2255",
                                        use_scope
                                    )),
                                    wire::Node::Space {
                                        width: Some(wire::Length::Fill),
                                        height: None,
                                    },
                                ];
                                wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:1071", use_scope),
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
                            }),
                            Some(wire::Length::Fill),
                            None,
                        ),
                        wire::Edges {
                            top: 10.0f32,
                            right: 14.0f32,
                            bottom: 10.0f32,
                            left: 14.0f32,
                        },
                    ),
                    native::sized(
                        native::container(
                            format!("{}/@container:1093", use_scope),
                            wire::Node::Space {
                                width: Some(wire::Length::Fixed(1.0f32)),
                                height: Some(wire::Length::Fixed(1.0f32)),
                            },
                        ),
                        Some(wire::Length::Fill),
                        Some(wire::Length::Fixed(1.0f32)),
                    ),
                    {
                        let mut children: Vec<_> = Vec::new();
                        for line in self.diff_rows.iter() {
                            let key = line.key;
                            let key_recon = format!("{}/key({})", use_scope, key);
                            let child: wire::Node = self.render_diff_row_63(
                                format!("{}/DiffRow@2268", key_recon),
                                cb_5.clone(),
                                line.clone(),
                            );
                            children.push((key, child));
                        }
                        let (keys, children) = children
                            .into_iter()
                            .map(|(key, child)| (wire::ListKey::from(key), child))
                            .unzip();
                        wire::Node::KeyedColumn {
                            key: format!("{}/@keyed:1099", use_scope),
                            keys: Some(keys),
                            children,
                            background: None,
                            border: None,
                            spacing: None,
                            padding: None,
                            width: Some(wire::Length::Fill),
                            height: None,
                            max_width: None,
                            align: None,
                            virtual_row: Some(20.0f32),
                        }
                    },
                ];
                native::sized(
                    native::column(format!("{}/@layout:1062", use_scope), children),
                    Some(wire::Length::Fill),
                    None,
                )
            }),
        }
    }
    pub(super) fn merged_notice(&self, key: String) -> wire::Node {
        native::text(
            key,
            crate::host::forge_merge_note(&self.forge_item_merge_oid, &self.forge_item_branches),
        )
    }
    pub(super) fn merge_advisory(&self, key: String) -> wire::Node {
        let note = match self.forge_item_change_requests {
            0 => return native::column(key, []),
            1 => "a reviewer requested changes — merge not recommended".into(),
            count => format!("{count} reviewers requested changes — merge not recommended"),
        };
        native::text(key, note)
    }
    pub(super) fn merge_button(
        &self,
        key: String,
        merge: impl Fn() -> Message + Clone + 'static,
    ) -> wire::Node {
        let disabled = self.merge_busy || !self.connected || self.forge_item_source_oid.is_empty();
        let label = if self.merge_busy {
            "Merging…"
        } else {
            "Merge pull request"
        };
        native::button(
            key,
            label,
            (!disabled).then(|| ducktape_view_guest::slots::message(merge())),
            Default::default(),
        )
    }

    pub(super) fn review(
        &self,
        key: String,
        open: impl Fn(String) -> Message + Clone + 'static,
        review: crate::host::ForgeReview,
    ) -> wire::Node {
        let mut children = vec![
            native::heading(format!("{key}/author"), &review.author_name),
            native::text(
                format!("{key}/verdict"),
                crate::host::verdict_label(&review.verdict),
            ),
            native::text(format!("{key}/commit"), &review.commit),
            self.finality(format!("{key}/finality"), review.created_at),
        ];
        if review.outdated {
            children.push(native::text(format!("{key}/outdated"), "outdated"));
        }
        if !review.body.is_empty() {
            children.push(self.rich_body(format!("{key}/body"), open.clone(), review.blocks));
        }
        for (index, comment) in review.comments.into_iter().enumerate() {
            let scope = format!("{key}/comment/{index}");
            children.push(native::column(
                scope.clone(),
                [
                    native::text(format!("{scope}/anchor"), comment.anchor),
                    native::text(format!("{scope}/kind"), "review comment"),
                    self.rich_body(format!("{scope}/body"), open.clone(), comment.blocks),
                ],
            ));
        }
        native::column(key, children)
    }
}
