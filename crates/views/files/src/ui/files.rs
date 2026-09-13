use super::*;
impl super::FilesView {
    pub(super) fn files_screen(&self, use_scope: String) -> wire::Node {
        let _component_owner = ::ducktape_view_guest::slots::component(
            "FilesScreen",
            &use_scope,
            false,
        );
        {
            let mut children: Vec<wire::Node> = vec![
                { let node_scope = format!("{}/crumb", use_scope); self
                .breadcrumb(node_scope.clone(), (move | event_0 |
                Message::OpenDirAt(event_0)).clone()) },
                native::padded(native::sized(native::container(format!("{}/@container:47",
                use_scope), { let mut children : Vec < wire::Node > =
                vec![wire::Node::Button { checked : None, expanded : None, description :
                None, key : format!("{}/@button:60", use_scope), content :
                wire::ButtonContent::Child(Box::new(native::text(format!("{}/@text:68",
                use_scope), "↑".to_owned().to_string(),),),), label :
                Some(String::from("Parent directory".to_owned())), on_press : if * self
                .derived_loading() || self.path == "/" { None } else {
                Some(::ducktape_view_guest::slots::message((move | event_0 | {
                Message::OpenDirAt(event_0) }) (crate
                ::host::fs_parent(::std::convert::AsRef::as_ref(& self.path),)
                .to_owned(),),),) }, width : Some(wire::Length::Fixed(26.0f32)), height :
                Some(wire::Length::Fixed(26.0f32)), padding :
                Some(wire::Edges::all(0.0f32)), style : wire::ButtonStyle::default(), },
                { let node_scope = format!("{}/fs-new", use_scope); wire::Node::Input {
                options : wire::InputOptions { label : "New entry name".to_owned()
                .to_string(), description : None, disabled : * self.derived_loading(),
                padding : None, text_size : None, line_height : None, align : None, font
                : None, }, key : node_scope.clone(), placeholder :
                String::from("new name…".to_owned()), value : self.new_name
                .to_string(), on_input : ::ducktape_view_guest::slots::handler:: <
                String, Message, > (Box::new({ let route = Message::NewNameChanged as fn
                (String) -> Message; move | sent : String | Some(route(sent)) }),),
                on_submit : None, width : Some(wire::Length::Fixed(160.0f32)), secure :
                false, style : Default::default(), } },
                native::padded(native::button(format!("{}/@button:85", use_scope),
                String::from("+ Folder"), if * self.derived_loading() || self.new_name
                .trim().to_owned().is_empty() || ! (* self.derived_refusal()).is_empty()
                { None } else {
                Some(::ducktape_view_guest::slots::message(Message::MkdirSubmit),) },
                wire::ButtonPreset::Secondary,), wire::Edges::all(5.0f32),),
                native::padded(native::button(format!("{}/@button:90", use_scope),
                String::from("+ File"), if * self.derived_loading() || self.new_name
                .trim().to_owned().is_empty() || ! (* self.derived_refusal()).is_empty()
                { None } else {
                Some(::ducktape_view_guest::slots::message(Message::NewFileSubmit,),) },
                wire::ButtonPreset::Secondary,), wire::Edges::all(5.0f32),),
                wire::Node::Space { width : Some(wire::Length::Fill), height : None, }];
                if * self.derived_loading() { children
                .push(native::text_options(native::text(format!("{}/@text:97",
                use_scope), "Loading…".to_owned().to_string(),), wire::TextOptions {
                wrapping : Some(wire::Wrapping::None), ..Default::default() },)); } if !
                self.preview_path.is_empty() { children
                .push(native::padded(native::button(format!("{}/@button:105", use_scope),
                String::from("Delete object"), if * self.derived_loading() || ! self
                .delete_target.is_empty() { None } else {
                Some(::ducktape_view_guest::slots::message((move | event_0 | {
                Message::ArmDeleteAt(event_0) }) (self.preview_path.to_owned()),),) },
                wire::ButtonPreset::Secondary,), wire::Edges::all(5.0f32),)); } children
                .push({ let mut children = vec![wire::Node::Space { width :
                Some(wire::Length::Fill), height : Some(wire::Length::Fill), }]; if !
                self.delete_target.is_empty() { children.push(self
                .confirm_delete(format!("{}/ConfirmDelete@1236", use_scope))); }
                wire::Node::Overlay { key : format!("{}/@overlay:113", use_scope),
                padding : 30.0f32, backdrop : wire::Rgba([0., 0., 0., 0.45]), align_x :
                wire::AlignX::Center, align_y : wire::AlignY::Center, on_dismiss :
                Some(::ducktape_view_guest::slots::message(Message::DisarmDeleteNow,),),
                children : children, } }); children.push(wire::Node::Button { checked :
                None, expanded : Some(self.files_screen_states.get(& use_scope)
                .map_or_else(| | self.files_screen_initial.history_open.clone(), | state
                | state.history_open.clone()),), description : None, key :
                format!("{}/@button:134", use_scope), content :
                wire::ButtonContent::Label(String::from("History"),), label : None,
                on_press :
                Some(::ducktape_view_guest::slots::message(Message::FilesScreenFsToggleHistory(use_scope
                .clone()),),), width : None, height : None, padding :
                Some(wire::Edges::all(5.0f32)), style : wire::ButtonStyle::default(), });
                wire::Node::Linear { max_width : None, clip : false, key :
                format!("{}/@layout:54", use_scope), wrap : None, axis : wire::Axis::Row,
                spacing : Some(8.0f32), padding : None, width : Some(wire::Length::Fill),
                height : Some(wire::Length::Fixed(28.0f32)), align :
                Some(wire::AlignX::Center), background : None, border : None, children :
                children, } },), Some(wire::Length::Fill), None,), wire::Edges { top :
                10.0f32, right : 20.0f32, bottom : 10.0f32, left : 20.0f32, },)
            ];
            if !(*self.derived_refusal()).is_empty() {
                children
                    .push(
                        native::padded(
                            native::sized(
                                native::container(
                                    format!("{}/@container:147", use_scope),
                                    native::sized(
                                        native::text_options(
                                            native::text(
                                                format!("{}/@text:153", use_scope),
                                                (*self.derived_refusal()).to_owned().to_string(),
                                            ),
                                            wire::TextOptions {
                                                wrapping: Some(wire::Wrapping::Word),
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
                                top: 0.0f32,
                                right: 20.0f32,
                                bottom: 10.0f32,
                                left: 20.0f32,
                            },
                        ),
                    );
            }
            children
                .push(
                    native::sized(
                        native::container(
                            format!("{}/@container:159", use_scope),
                            wire::Node::Space {
                                width: Some(wire::Length::Fixed(1.0f32)),
                                height: Some(wire::Length::Fixed(1.0f32)),
                            },
                        ),
                        Some(wire::Length::Fill),
                        Some(wire::Length::Fixed(1.0f32)),
                    ),
                );
            children
                .push({
                    let mut children: Vec<wire::Node> = vec![
                        { let node_scope = format!("{}/tree-pane", use_scope);
                        wire::Node::Container { shadow : Default::default(), max_width :
                        None, max_height : None, clip : true, key : node_scope.clone(),
                        width : Some(wire::Length::Fixed(self.tree_width as f32)), height
                        : Some(wire::Length::Fill), padding : None, align_x : None,
                        align_y : None, background : None, border : None, snap : None,
                        content : Box::new({ let children : Vec < wire::Node > =
                        vec![wire::Node::Container { shadow : Default::default(),
                        max_width : None, max_height : None, clip : false, key :
                        format!("{}/@container:185", use_scope), width :
                        Some(wire::Length::Fill), height :
                        Some(wire::Length::Fixed(50.0f32)), padding : Some(wire::Edges {
                        top : 0.0f32, right : 14.0f32, bottom : 0.0f32, left : 14.0f32,
                        }), align_x : None, align_y : Some(wire::AlignY::Center),
                        background : None.map(wire::Background::Color), border : None,
                        snap : None, content : Box::new({ let children : Vec <
                        wire::Node > =
                        vec![native::text_options(native::text(format!("{}/@text:193",
                        use_scope), "duckfs".to_owned().to_string(),), wire::TextOptions
                        { wrapping : Some(wire::Wrapping::None), ..Default::default()
                        },), native::text_options(native::text(format!("{}/@text:199",
                        use_scope), "content-addressed · replicated".to_owned()
                        .to_string(),), wire::TextOptions { wrapping :
                        Some(wire::Wrapping::None), ..Default::default() },)];
                        native::spaced(native::sized(native::column(format!("{}/@layout:192",
                        use_scope), children,), Some(wire::Length::Fill), None,),
                        2.0f32,) }), },
                        native::sized(native::container(format!("{}/@container:205",
                        use_scope), wire::Node::Space { width :
                        Some(wire::Length::Fixed(1.0f32)), height :
                        Some(wire::Length::Fixed(1.0f32)), },), Some(wire::Length::Fill),
                        Some(wire::Length::Fixed(1.0f32)),), wire::Node::Scroll {
                        on_scroll : None, virtual_rows : true, key :
                        format!("{}/@layout:211", use_scope), direction :
                        wire::ScrollDirection::Vertical, width :
                        Some(wire::Length::Fill), height : Some(wire::Length::Fill),
                        bar_hidden : true, bar_width : None, bar_margin : None,
                        scroller_width : None, bar_spacing : None, anchor_x :
                        wire::ScrollAnchor::Start, anchor_y : wire::ScrollAnchor::Start,
                        auto_scroll : false, background : None, border : None, content :
                        Box::new({ let mut children : Vec < wire::Node > = Vec::new(); if
                        self.connected && self.listed && self.directories.is_empty() &&
                        self.omitted == 0 { children
                        .push(native::padded(native::sized(native::container(format!("{}/@container:230",
                        use_scope), native::text(format!("{}/@text:237", use_scope),
                        "No folders here.".to_owned().to_string(),),),
                        Some(wire::Length::Fill), None,), wire::Edges { top : 6.0f32,
                        right : 12.0f32, bottom : 6.0f32, left : 12.0f32, },)); } if self
                        .connected && self.listed { children.push({ let mut children :
                        Vec < _ > = Vec::new(); for entry in self.directories.iter() {
                        let key = entry.key; let key_recon = format!("{}/key({})",
                        use_scope, key); let child : wire::Node = self
                        .folder_row(format!("{}/FsTreeRow@1356", key_recon), (move |
                        event_0 | Message::OpenDirAt(event_0)).clone(), entry.clone());
                        children.push((key, child)); } let (keys, children) = children
                        .into_iter().map(| (key, child) | (wire::ListKey::from(key),
                        child)).unzip(); wire::Node::KeyedColumn { key :
                        format!("{}/@keyed:243", use_scope), keys : Some(keys), children,
                        background : None, border : None, spacing : None, padding : None,
                        width : Some(wire::Length::Fill), height : None, max_width :
                        None, align : None, virtual_row : Some(27.0f32), } }); }
                        native::spaced(native::padded(native::sized(native::column(format!("{}/@layout:217",
                        use_scope), children,), Some(wire::Length::Fill), None,),
                        wire::Edges { top : 8.0f32, right : 6.0f32, bottom : 8.0f32, left
                        : 6.0f32, },), 1.0f32,) }), }];
                        native::sized(native::column(format!("{}/@layout:175",
                        use_scope), children,), Some(wire::Length::Fill),
                        Some(wire::Length::Fill),) }), } }, { let node_scope =
                        format!("{}/tree-resize", use_scope); wire::Node::ResizeHandle {
                        key : node_scope.clone(), on_press : None, on_release : None,
                        on_drag : Some(::ducktape_view_guest::slots::handler:: < (f64,
                        f64), Message, > (Box::new({ let route = { let
                        _route_state_scope_0 = use_scope.clone(); let route_callback =
                        (move | event_0, event_1 | Message::TreeResized(event_0,
                        event_1,)).clone(); move | delta : (f64, f64) |
                        route_callback(delta.0, delta.1) }; move | sent : (f64, f64) |
                        Some(route(sent)) }),),), cursor :
                        Some(wire::mouse::Cursor::ResizingHorizontally), content :
                        Box::new({ let node_scope = format!("{}/tree-divider",
                        node_scope); wire::Node::Container { shadow : Default::default(),
                        max_width : None, max_height : None, clip : false, key :
                        node_scope.clone(), width : Some(wire::Length::Fixed(10.0f32)),
                        height : Some(wire::Length::Fill), padding : None, align_x :
                        Some(wire::AlignX::Left), align_y : None, background : None
                        .map(wire::Background::Color), border : None, snap : None,
                        content :
                        Box::new(native::sized(native::container(format!("{}/@container:257",
                        use_scope), wire::Node::Space { width :
                        Some(wire::Length::Fixed(1.0f32)), height :
                        Some(wire::Length::Fixed(1.0f32)), },),
                        Some(wire::Length::Fixed(1.0f32)), Some(wire::Length::Fill),),),
                        } }), } }, { let mut children : Vec < wire::Node > = Vec::new();
                        if ! self.connected { children
                        .push(native::padded(native::sized(native::container(format!("{}/@container:268",
                        use_scope), self.disconnected(format!("{}/EmptyState@1385",
                        use_scope)),), Some(wire::Length::Fill),
                        Some(wire::Length::Fill),), wire::Edges { top : 22.0f32, right :
                        22.0f32, bottom : 22.0f32, left : 22.0f32, },)); } if self
                        .connected && self.files_screen_states.get(& use_scope)
                        .map_or_else(| | self.files_screen_initial.history_open.clone(),
                        | state | state.history_open.clone()) { children
                        .push(wire::Node::Scroll { on_scroll : None, virtual_rows :
                        false, key : format!("{}/@layout:278", use_scope), direction :
                        wire::ScrollDirection::Vertical, width :
                        Some(wire::Length::Fill), height : Some(wire::Length::Fill),
                        bar_hidden : false, bar_width : None, bar_margin : None,
                        scroller_width : None, bar_spacing : None, anchor_x :
                        wire::ScrollAnchor::Start, anchor_y : wire::ScrollAnchor::Start,
                        auto_scroll : false, background : None, border : None, content :
                        Box::new({ let mut children : Vec < wire::Node > = Vec::new(); if
                        ! self.diff_from.is_empty() { children.push({ let mut children :
                        Vec < wire::Node > = vec![{ let children : Vec < wire::Node >
                        = vec![self.changes_heading(format!("{}/GroupLabel@1407",
                        use_scope)), wire::Node::Space { width :
                        Some(wire::Length::Fill), height : None, },
                        native::padded(native::button(format!("{}/@button:297",
                        use_scope), String::from("Back"),
                        Some(::ducktape_view_guest::slots::message(Message::CloseDiffNow),),
                        wire::ButtonPreset::Secondary,), wire::Edges::all(4.0f32),)];
                        wire::Node::Linear { max_width : None, clip : false, key :
                        format!("{}/@layout:290", use_scope), wrap : None, axis :
                        wire::Axis::Row, spacing : Some(8.0f32), padding : None, width :
                        Some(wire::Length::Fill), height : None, align :
                        Some(wire::AlignX::Center), background : None, border : None,
                        children : children, } }]; if self.diff.is_empty() && self
                        .diff_omitted == 0 { children
                        .push(native::text(format!("{}/@text:305", use_scope),
                        "No differences.".to_owned().to_string(),)); } for (index, entry)
                        in self.diff.iter().enumerate() { let for_scope =
                        format!("{}/@for:1418({})", use_scope, index); children.push({
                        let children : Vec < wire::Node > =
                        vec![native::sized(native::text_options(native::text(format!("{}/@text:312",
                        for_scope), entry.kind.to_owned().to_string(),),
                        wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                        ..Default::default() },), Some(wire::Length::Fixed(64.0f32)),
                        None,),
                        native::sized(native::text_options(native::text(format!("{}/@text:319",
                        for_scope), entry.path.to_owned().to_string(),),
                        wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                        ..Default::default() },), Some(wire::Length::Fill), None,)];
                        wire::Node::Linear { max_width : None, clip : false, key :
                        format!("{}/@layout:307", for_scope), wrap : None, axis :
                        wire::Axis::Row, spacing : Some(8.0f32), padding : None, width :
                        Some(wire::Length::Fill), height : None, align :
                        Some(wire::AlignX::Center), background : None, border : None,
                        children : children, } }); }
                        native::spaced(native::sized(native::column(format!("{}/@layout:289",
                        use_scope), children,), Some(wire::Length::Fill), None,),
                        6.0f32,) }); } if self.diff_from.is_empty() { children.push({ let
                        mut children : Vec < wire::Node > = Vec::new(); if ! self.history
                        .is_empty() { children.push(self
                        .snapshots_heading(format!("{}/GroupLabel@1444", use_scope))); }
                        if self.history.is_empty() && self.omitted == 0 { children
                        .push(native::text(format!("{}/@text:334", use_scope),
                        "No snapshots yet.".to_owned().to_string(),)); } for (index,
                        snapshot) in self.history.iter().enumerate() { let for_scope =
                        format!("{}/@for:1447({})", use_scope, index); children
                        .push(native::padded(native::sized(native::container(format!("{}/@container:336",
                        for_scope), { let mut children : Vec < wire::Node > = vec![{ let
                        children : Vec < wire::Node > =
                        vec![native::text_options(native::text(format!("{}/@text:350",
                        for_scope), snapshot.short_id.to_owned().to_string(),),
                        wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                        ..Default::default() },),
                        native::text_options(native::text(format!("{}/@text:356",
                        for_scope), crate ::host::height_label(snapshot.height)
                        .to_string(),), wire::TextOptions { wrapping :
                        Some(wire::Wrapping::None), ..Default::default() },),
                        wire::Node::Space { width : Some(wire::Length::Fill), height :
                        None, },
                        native::text_options(native::text(format!("{}/@text:363",
                        for_scope), snapshot.author.to_owned().to_string(),),
                        wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                        ..Default::default() },),
                        native::padded(native::button(format!("{}/@button:369",
                        for_scope), String::from("Diff"),
                        Some(::ducktape_view_guest::slots::message((move | event_0 |
                        Message::ShowDiffOf(event_0,)) (snapshot.id.to_owned()),),),
                        wire::ButtonPreset::Secondary,), wire::Edges::all(3.0f32),)];
                        wire::Node::Linear { max_width : None, clip : false, key :
                        format!("{}/@layout:345", for_scope), wrap : None, axis :
                        wire::Axis::Row, spacing : Some(8.0f32), padding : None, width :
                        Some(wire::Length::Fill), height : None, align :
                        Some(wire::AlignX::Center), background : None, border : None,
                        children : children, } }]; if ! snapshot.message.is_empty() {
                        children.push(native::text(format!("{}/@text:377", for_scope),
                        snapshot.message.to_owned().to_string(),)); }
                        native::spaced(native::sized(native::column(format!("{}/@layout:344",
                        for_scope), children,), Some(wire::Length::Fill), None,),
                        3.0f32,) },), Some(wire::Length::Fill), None,), wire::Edges { top
                        : 11.0f32, right : 11.0f32, bottom : 11.0f32, left : 11.0f32,
                        },)); }
                        native::spaced(native::sized(native::column(format!("{}/@layout:327",
                        use_scope), children,), Some(wire::Length::Fill), None,),
                        8.0f32,) }); }
                        native::spaced(native::padded(native::sized(native::column(format!("{}/@layout:283",
                        use_scope), children,), Some(wire::Length::Fill), None,),
                        wire::Edges { top : 18.0f32, right : 18.0f32, bottom : 18.0f32,
                        left : 18.0f32, },), 8.0f32,) }), }); } if self.connected && !
                        self.files_screen_states.get(& use_scope).map_or_else(| | self
                        .files_screen_initial.history_open.clone(), | state | state
                        .history_open.clone()) { children.push({ let mut children : Vec <
                        wire::Node > = vec![self
                        .object_table_header(format!("{}/ObjectTableHeader@1492",
                        use_scope))]; if self.listed && self.entries.is_empty() && self
                        .omitted == 0 { children
                        .push(native::padded(native::sized(native::container(format!("{}/@container:388",
                        use_scope), self.empty_directory(format!("{}/EmptyPlate@1501",
                        use_scope)),), Some(wire::Length::Fill), None,), wire::Edges {
                        top : 22.0f32, right : 22.0f32, bottom : 22.0f32, left : 22.0f32,
                        },)); } if self.listed && ! self.entries.is_empty() { children
                        .push(wire::Node::Scroll { on_scroll : None, virtual_rows : true,
                        key : format!("{}/@layout:391", use_scope), direction :
                        wire::ScrollDirection::Vertical, width :
                        Some(wire::Length::Fill), height : Some(wire::Length::Fill),
                        bar_hidden : false, bar_width : None, bar_margin : None,
                        scroller_width : None, bar_spacing : None, anchor_x :
                        wire::ScrollAnchor::Start, anchor_y : wire::ScrollAnchor::Start,
                        auto_scroll : false, background : None, border : None, content :
                        Box::new({ let mut children : Vec < _ > = Vec::new(); for entry
                        in self.entries.iter() { let key = entry.key; let key_recon =
                        format!("{}/key({})", use_scope, key); let child : wire::Node =
                        self.object_row(format!("{}/ObjectRow@1509", key_recon), (move |
                        event_0 | Message::OpenDirAt(event_0)).clone(), (move | event_0 |
                        Message::OpenFileAt(event_0)).clone(), entry.clone(), entry.path
                        == self.preview_path); children.push((key, child)); } let (keys,
                        children) = children.into_iter().map(| (key, child) |
                        (wire::ListKey::from(key), child)).unzip();
                        wire::Node::KeyedColumn { key : format!("{}/@keyed:396",
                        use_scope), keys : Some(keys), children, background : None,
                        border : None, spacing : None, padding : None, width :
                        Some(wire::Length::Fill), height : None, max_width : None, align
                        : None, virtual_row : Some(39.0f32), } }), }); } if ! self
                        .preview_path.is_empty() { children.push({ let node_scope =
                        format!("{}/preview-resize", use_scope); wire::Node::ResizeHandle
                        { key : node_scope.clone(), on_press : None, on_release : None,
                        on_drag : Some(::ducktape_view_guest::slots::handler:: < (f64,
                        f64), Message, > (Box::new({ let route = { let
                        _route_state_scope_0 = use_scope.clone(); let route_callback =
                        (move | event_0, event_1 | Message::PreviewResized(event_0,
                        event_1,)).clone(); move | delta : (f64, f64) |
                        route_callback(delta.0, delta.1) }; move | sent : (f64, f64) |
                        Some(route(sent)) }),),), cursor :
                        Some(wire::mouse::Cursor::ResizingVertically), content :
                        Box::new({ let node_scope = format!("{}/preview-divider",
                        node_scope); wire::Node::Container { shadow : Default::default(),
                        max_width : None, max_height : None, clip : false, key :
                        node_scope.clone(), width : Some(wire::Length::Fill), height :
                        Some(wire::Length::Fixed(10.0f32)), padding : None, align_x :
                        None, align_y : Some(wire::AlignY::Top), background : None
                        .map(wire::Background::Color), border : None, snap : None,
                        content :
                        Box::new(native::sized(native::container(format!("{}/@container:408",
                        use_scope), wire::Node::Space { width :
                        Some(wire::Length::Fixed(1.0f32)), height :
                        Some(wire::Length::Fixed(1.0f32)), },), Some(wire::Length::Fill),
                        Some(wire::Length::Fixed(1.0f32)),),), } }), } }); children
                        .push({ let node_scope = format!("{}/preview-pane", use_scope); {
                        let children : Vec < wire::Node > =
                        vec![native::padded(native::sized(native::container(format!("{}/@container:411",
                        use_scope), { let children : Vec < wire::Node > = vec![{ let
                        mut children : Vec < wire::Node > =
                        vec![native::sized(native::text_options(native::text(format!("{}/@text:426",
                        use_scope), self.preview_path.to_owned().to_string(),),
                        wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                        ..Default::default() },), Some(wire::Length::Fill), None,)]; if
                        self.preview_truncated { children
                        .push(native::text_options(native::text(format!("{}/@text:434",
                        use_scope), "first 64 KiB".to_owned().to_string(),),
                        wire::TextOptions { wrapping : Some(wire::Wrapping::None),
                        ..Default::default() },)); } if self.preview_clipped && ! * self
                        .derived_draft_here() { children
                        .push(native::text_options(native::text(format!("{}/@text:440",
                        use_scope), "Preview shortened for display.".to_owned()
                        .to_string(),), wire::TextOptions { wrapping :
                        Some(wire::Wrapping::None), ..Default::default() },)); } if !
                        self.preview_binary && ! self.preview_picture && ! * self
                        .derived_draft_here() && ! self.preview_truncated { children
                        .push(native::padded(native::button(format!("{}/@button:446",
                        use_scope), String::from("Edit"), if * self.derived_loading() ||
                        (* self.derived_draft_parked() || self.preview_base.is_empty() ||
                        self.chain.is_empty()) { None } else {
                        Some(::ducktape_view_guest::slots::message((move | event_0 |
                        Message::BeginEdit(event_0,)) ((* self.derived_edit_context())
                        .to_owned()),),) }, wire::ButtonPreset::Secondary,),
                        wire::Edges::all(4.0f32),)); } if * self.derived_draft_here() {
                        children
                        .push(native::padded(native::button(format!("{}/@button:455",
                        use_scope), String::from("Cancel"),
                        Some(::ducktape_view_guest::slots::message((move | event_0 |
                        Message::CancelEdit(event_0,)) ((* self.derived_edit_context())
                        .to_owned()),),), wire::ButtonPreset::Secondary,),
                        wire::Edges::all(4.0f32),)); } if * self.derived_draft_here() {
                        children
                        .push(native::padded(native::button(format!("{}/@button:463",
                        use_scope), String::from("Save"), if * self.derived_loading() {
                        None } else { Some(::ducktape_view_guest::slots::message((move |
                        event_0 | Message::SaveEdit(event_0,)) ((* self
                        .derived_edit_context()).to_owned()),),) },
                        wire::ButtonPreset::Secondary,), wire::Edges::all(4.0f32),)); }
                        wire::Node::Linear { max_width : None, clip : false, key :
                        format!("{}/@layout:421", use_scope), wrap : None, axis :
                        wire::Axis::Row, spacing : Some(8.0f32), padding : None, width :
                        Some(wire::Length::Fill), height : None, align :
                        Some(wire::AlignX::Center), background : None, border : None,
                        children : children, } }, { let mut children : Vec < wire::Node >
                        = Vec::new(); if * self.derived_draft_here() { children.push({
                        let node_scope = format!("{}/fs-editor", node_scope); { let
                        editor = & self.draft; let (document, on_document) = editor
                        .document("app:draft".to_owned(), Message::EditDraft as fn
                        (::ducktape_view_guest::EditorDocumentUpdate,) -> Message);
                        wire::Node::Editor { options : Box::new(wire::EditorOptions {
                        binding : Some(Box::new(::ducktape_view_guest::EditorBinding:: <
                        (), > ::plain(Message::DraftTransaction),),), presentation :
                        None, size : None, padding : None, line_height : None, wrapping :
                        Some(wire::Wrapping::Word), font : None, style :
                        wire::InputStyle::default(), }), key : node_scope.clone(),
                        placeholder : "File contents…".to_owned(), document : document,
                        on_document : on_document, editable : ! * self.derived_loading(),
                        width : None, height : None, min_height : Some(200.0f32),
                        max_height : None, } } }); } if ! * self.derived_draft_here() {
                        children.push(wire::Node::Scroll { on_scroll : None, virtual_rows
                        : false, key : format!("{}/@layout:483", use_scope), direction :
                        wire::ScrollDirection::Vertical, width :
                        Some(wire::Length::Fill), height : Some(wire::Length::Fill),
                        bar_hidden : false, bar_width : None, bar_margin : None,
                        scroller_width : None, bar_spacing : None, anchor_x :
                        wire::ScrollAnchor::Start, anchor_y : wire::ScrollAnchor::Start,
                        auto_scroll : false, background : None, border : None, content :
                        Box::new({ let mut children : Vec < wire::Node > = Vec::new(); if
                        self.preview_binary { children
                        .push(native::text(format!("{}/@text:490", use_scope), self
                        .preview_display_text.to_owned().to_string(),)); } if self
                        .preview_picture { children.push({ let node_scope =
                        format!("{}/fs-picture", node_scope); wire::Node::Surface { key :
                        node_scope.clone(), name : String::from("picture"), args :
                        ::std::vec![{ let surface_arg = & ("files".to_owned());
                        wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                        }, { let surface_arg = & (self.preview_path.to_owned());
                        wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                        }], on_event : None, } }); children
                        .push(native::text_options(native::text(format!("{}/@text:505",
                        use_scope), crate ::host::picture_caption(self.preview_width,
                        self.preview_height,).to_string(),), wire::TextOptions { wrapping
                        : Some(wire::Wrapping::None), ..Default::default() },)); } if !
                        self.preview_binary && ! self.preview_picture && crate
                        ::host::markdown_path(::std::convert::AsRef::as_ref(& self
                        .preview_path),) { children.push({ let _lazy_context_246 =
                        use_scope.to_owned(); let lazy_event_246_0 = (move | event_0 |
                        Message::OpenLinkAt(event_0,)).clone(); let _lazy_event_246_1 =
                        (move | event_0 | Message::OpenDirAt(event_0,)).clone(); let
                        _lazy_event_246_2 = (move | event_0 |
                        Message::OpenFileAt(event_0,)).clone(); let _lazy_event_246_3 =
                        (move | | Message::MkdirSubmit).clone(); let _lazy_event_246_4 =
                        (move | | Message::NewFileSubmit).clone(); let _lazy_event_246_5
                        = (move | event_0 | Message::ArmDeleteAt(event_0,)).clone(); let
                        _lazy_event_246_6 = (move | | Message::DisarmDeleteNow).clone();
                        let _lazy_event_246_7 = (move | | Message::DeleteSubmit).clone();
                        let _lazy_event_246_8 = (move | | Message::CloseDiffNow).clone();
                        let _lazy_event_246_9 = (move | event_0 |
                        Message::ShowDiffOf(event_0,)).clone(); let _lazy_event_246_10 =
                        (move | event_0 | Message::BeginEdit(event_0,)).clone(); let
                        _lazy_event_246_11 = (move | event_0 |
                        Message::CancelEdit(event_0,)).clone(); let _lazy_event_246_12 =
                        (move | event_0 | Message::SaveEdit(event_0,)).clone(); let
                        _lazy_event_246_13 = (move | event_0, event_1 |
                        Message::TreeResized(event_0, event_1,)).clone(); let
                        _lazy_event_246_14 = (move | event_0, event_1 |
                        Message::PreviewResized(event_0, event_1,)).clone(); let
                        _lazy_event_246_15 = (move | event_0, event_1 |
                        Message::ObjectResized(event_0, event_1,)).clone(); { let
                        lazy_key = format!("{}/@lazy:511", use_scope);
                        ::ducktape_view_guest::memo_lazy((self.preview_display_text
                        .to_owned(), self.preview_path.to_owned(), self.dark, self
                        .preview_text_revision, node_scope.to_owned(), match self
                        .active_palette { AppTheme::App => "app", AppTheme::AppDark =>
                        "app-dark", },), move | dependency | { let _preview_text : String
                        = dependency.0.clone(); let _preview_path : String = dependency.1
                        .clone(); let dark : bool = dependency.2.clone(); let lazy_scope
                        = dependency.4.clone(); let cached_doc : String = self
                        .preview_display_text.to_owned(); { let node_scope =
                        format!("{}/fs-markdown", lazy_scope); wire::Node::Surface { key
                        : node_scope.clone(), name : String::from("agent_markdown"), args
                        : ::std::vec![{ let surface_arg = & (cached_doc.to_owned());
                        wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                        }, { let surface_arg = & (dark); wire::SurfaceValue::Bool(*
                        (surface_arg)) }], on_event :
                        Some(::ducktape_view_guest::slots::handler:: <
                        wire::SurfaceValue, Message, > (Box::new({ let route = { let
                        route_callback = lazy_event_246_0.clone(); move | value |
                        route_callback(value) }; move | sent | { match sent {
                        wire::SurfaceValue::Str(item) => Some(item), _ => None, } .map(&
                        route) } }),),), } } }, 246u64, & use_scope, lazy_key,) } }); }
                        if ! self.preview_binary && ! self.preview_picture && ! crate
                        ::host::markdown_path(::std::convert::AsRef::as_ref(& self
                        .preview_path),) { children.push({ let _lazy_context_249 =
                        use_scope.to_owned(); let _lazy_event_249_0 = (move | event_0 |
                        Message::OpenLinkAt(event_0,)).clone(); let _lazy_event_249_1 =
                        (move | event_0 | Message::OpenDirAt(event_0,)).clone(); let
                        _lazy_event_249_2 = (move | event_0 |
                        Message::OpenFileAt(event_0,)).clone(); let _lazy_event_249_3 =
                        (move | | Message::MkdirSubmit).clone(); let _lazy_event_249_4 =
                        (move | | Message::NewFileSubmit).clone(); let _lazy_event_249_5
                        = (move | event_0 | Message::ArmDeleteAt(event_0,)).clone(); let
                        _lazy_event_249_6 = (move | | Message::DisarmDeleteNow).clone();
                        let _lazy_event_249_7 = (move | | Message::DeleteSubmit).clone();
                        let _lazy_event_249_8 = (move | | Message::CloseDiffNow).clone();
                        let _lazy_event_249_9 = (move | event_0 |
                        Message::ShowDiffOf(event_0,)).clone(); let _lazy_event_249_10 =
                        (move | event_0 | Message::BeginEdit(event_0,)).clone(); let
                        _lazy_event_249_11 = (move | event_0 |
                        Message::CancelEdit(event_0,)).clone(); let _lazy_event_249_12 =
                        (move | event_0 | Message::SaveEdit(event_0,)).clone(); let
                        _lazy_event_249_13 = (move | event_0, event_1 |
                        Message::TreeResized(event_0, event_1,)).clone(); let
                        _lazy_event_249_14 = (move | event_0, event_1 |
                        Message::PreviewResized(event_0, event_1,)).clone(); let
                        _lazy_event_249_15 = (move | event_0, event_1 |
                        Message::ObjectResized(event_0, event_1,)).clone(); { let
                        lazy_key = format!("{}/@lazy:514", use_scope);
                        ::ducktape_view_guest::memo_lazy((self.preview_display_text
                        .to_owned(), self.preview_path.to_owned(), self.dark, self
                        .preview_text_revision, node_scope.to_owned(), match self
                        .active_palette { AppTheme::App => "app", AppTheme::AppDark =>
                        "app-dark", },), move | dependency | { let _preview_text : String
                        = dependency.0.clone(); let preview_path : String = dependency.1
                        .clone(); let dark : bool = dependency.2.clone(); let lazy_scope
                        = dependency.4.clone(); let cached_source : String = self
                        .preview_display_text.to_owned(); { let node_scope =
                        format!("{}/fs-code", lazy_scope); wire::Node::Surface { key :
                        node_scope.clone(), name : String::from("forge_code"), args :
                        ::std::vec![{ let surface_arg = & (cached_source.to_owned());
                        wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                        }, { let surface_arg = & (preview_path.to_owned());
                        wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                        }, { let surface_arg = & (dark); wire::SurfaceValue::Bool(*
                        (surface_arg)) }], on_event : None, } } }, 249u64, & use_scope,
                        lazy_key,) } }); }
                        native::spaced(native::sized(native::column(format!("{}/@layout:488",
                        use_scope), children,), Some(wire::Length::Fill), None,),
                        6.0f32,) }), }); } wire::Node::Stack { key :
                        format!("{}/@layout:468", use_scope), width :
                        Some(wire::Length::Fill), height : Some(wire::Length::Fill),
                        padding : None, background : None, border : None, clip : false,
                        under : 0u32, children : children, } }];
                        native::spaced(native::sized(native::column(format!("{}/@layout:416",
                        use_scope), children,), Some(wire::Length::Fill),
                        Some(wire::Length::Fill),), 8.0f32,) },),
                        Some(wire::Length::Fill), Some(wire::Length::Fill),), wire::Edges
                        { top : 16.0f32, right : 16.0f32, bottom : 16.0f32, left :
                        16.0f32, },)]; native::sized(native::column(node_scope.clone(),
                        children), Some(wire::Length::Fill),
                        Some(wire::Length::Fixed(self.preview_pane_height as f32)),) }
                        }); } native::sized(native::column(format!("{}/@layout:379",
                        use_scope), children,), Some(wire::Length::Fill),
                        Some(wire::Length::Fill),) }); }
                        native::sized(native::column(format!("{}/@layout:259",
                        use_scope), children,), Some(wire::Length::Fill),
                        Some(wire::Length::Fill),) }
                    ];
                    if self.connected {
                        if !self.preview_entry.path.is_empty() {
                            children
                                .push({
                                    let node_scope = format!("{}/object-resize", use_scope);
                                    wire::Node::ResizeHandle {
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
                                                        let _route_state_scope_0 = use_scope.clone();
                                                        let route_callback = (move |event_0, event_1| Message::ObjectResized(
                                                            event_0,
                                                            event_1,
                                                        ))
                                                            .clone();
                                                        move |delta: (f64, f64)| route_callback(delta.0, delta.1)
                                                    };
                                                    move |sent: (f64, f64)| Some(route(sent))
                                                }),
                                            ),
                                        ),
                                        cursor: Some(wire::mouse::Cursor::ResizingHorizontally),
                                        content: Box::new({
                                            let node_scope = format!("{}/object-divider", node_scope);
                                            wire::Node::Container {
                                                shadow: Default::default(),
                                                max_width: None,
                                                max_height: None,
                                                clip: false,
                                                key: node_scope.clone(),
                                                width: Some(wire::Length::Fixed(10.0f32)),
                                                height: Some(wire::Length::Fill),
                                                padding: None,
                                                align_x: Some(wire::AlignX::Center),
                                                align_y: None,
                                                background: None,
                                                border: None,
                                                snap: None,
                                                content: Box::new(
                                                    native::sized(
                                                        native::container(
                                                            format!("{}/@container:525", use_scope),
                                                            wire::Node::Space {
                                                                width: Some(wire::Length::Fixed(2.0f32)),
                                                                height: Some(wire::Length::Fixed(1.0f32)),
                                                            },
                                                        ),
                                                        Some(wire::Length::Fixed(2.0f32)),
                                                        Some(wire::Length::Fill),
                                                    ),
                                                ),
                                            }
                                        }),
                                    }
                                });
                            children
                                .push({
                                    let node_scope = format!("{}/object-panel", use_scope);
                                    self.object_panel(node_scope.clone())
                                });
                        }
                    }
                    native::sized(
                        native::row(format!("{}/@layout:165", use_scope), children),
                        Some(wire::Length::Fill),
                        Some(wire::Length::Fill),
                    )
                });
            native::sized(
                native::column(format!("{}/@layout:29", use_scope), children),
                Some(wire::Length::Fill),
                Some(wire::Length::Fill),
            )
        }
    }
}
