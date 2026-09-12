//! GPUI rendering of the existing WASM tree. Widget identities retain native
//! input state; interaction uses the same semantic events the guests consume.

use gpui_kit::component::radio::Radio;
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::{
    Disableable,
    button::{Button, ButtonCustomVariant, ButtonVariants},
    checkbox::Checkbox,
    input::{Input, InputEvent, InputState},
};
use gpui_kit::component::{
    IndexPath,
    searchable_list::SearchableListItem,
    select::{Select, SelectEvent, SelectState},
};
use gpui_kit::*;
use std::collections::HashMap;
use std::sync::Arc;
use ui_lang_wire as wire;

struct RangeControl {
    state: Entity<SliderState>,
    bounds: [f32; 3],
    value: f32,
    on_change: u32,
    on_release: Option<u32>,
    _subscription: Subscription,
}

struct SensorState {
    reset: Option<wire::SurfaceValue>,
    size: Option<Size<Pixels>>,
    on_hide: Option<u32>,
    on_show: Option<u32>,
    on_resize: Option<u32>,
    pending: Option<(Size<Pixels>, Task<()>)>,
}

struct EditorMount {
    view: Entity<crate::editor::wire::WireEditor>,
    _subscription: Subscription,
}

#[derive(Default)]
struct ViewerState {
    scale: f32,
    offset: Point<Pixels>,
    drag: Option<Point<Pixels>>,
}

#[derive(Clone)]
struct Choice {
    index: u32,
    label: String,
}

impl SearchableListItem for Choice {
    type Value = u32;
    fn title(&self) -> SharedString {
        self.label.clone().into()
    }
    fn value(&self) -> &u32 {
        &self.index
    }
}

struct Picker {
    state: Entity<SelectState<Vec<Choice>>>,
    options: Vec<String>,
    selected: Option<u32>,
    handler: u32,
    _subscription: Subscription,
}

struct Field {
    state: Entity<InputState>,
    on_input: u32,
    on_submit: Option<u32>,
    value: String,
    guest_value: String,
    placeholder: String,
    secure: bool,
    _subscription: Subscription,
}

pub struct ViewTree {
    root: wire::Node,
    fields: HashMap<String, Field>,
    scrolls: HashMap<String, ScrollHandle>,
    scroll_positions: HashMap<String, (Point<Pixels>, Point<Pixels>)>,
    pickers: HashMap<String, Picker>,
    drags: HashMap<String, Point<Pixels>>,
    containers: HashMap<String, [f64; 2]>,
    bounds: HashMap<String, Bounds<Pixels>>,
    sensors: HashMap<String, SensorState>,
    hovered: std::collections::HashSet<String>,
    ranges: HashMap<String, RangeControl>,
    images: HashMap<u64, Arc<RenderImage>>,
    viewers: HashMap<String, ViewerState>,
    vectors: HashMap<u64, Arc<[u8]>>,
    surfaces: HashMap<String, AnyView>,
    editor_store: Option<crate::editor::wire::EditorStore>,
    editors: HashMap<String, EditorMount>,
}

impl EventEmitter<wire::Event> for ViewTree {}

impl ViewTree {
    pub fn new(root: wire::Node) -> Self {
        Self {
            root,
            fields: HashMap::new(),
            scrolls: HashMap::new(),
            scroll_positions: HashMap::new(),
            pickers: HashMap::new(),
            drags: HashMap::new(),
            containers: HashMap::new(),
            bounds: HashMap::new(),
            sensors: HashMap::new(),
            hovered: Default::default(),
            ranges: HashMap::new(),
            images: HashMap::new(),
            viewers: HashMap::new(),
            vectors: HashMap::new(),
            surfaces: HashMap::new(),
            editor_store: None,
            editors: HashMap::new(),
        }
    }

    pub fn set_editor_store(
        &mut self,
        store: crate::editor::wire::EditorStore,
        cx: &mut Context<Self>,
    ) {
        self.editors.clear();
        self.editor_store = Some(store);
        cx.notify();
    }

    pub fn set_surface(&mut self, key: String, surface: AnyView, cx: &mut Context<Self>) {
        self.surfaces.insert(key, surface);
        cx.notify();
    }

    pub fn surface_requests(&self) -> Vec<(String, String, Vec<wire::SurfaceValue>, Option<u32>)> {
        let mut requests = Vec::new();
        self.root.clone().for_each_mut(&mut |node| {
            if let wire::Node::Surface {
                key,
                name,
                args,
                on_event,
            } = node
            {
                requests.push((key.clone(), name.clone(), args.clone(), *on_event));
            }
        });
        requests
    }

    pub fn replace(&mut self, mut root: wire::Node, cx: &mut Context<Self>) {
        let mut inputs = std::collections::HashSet::new();
        let mut scrolls = std::collections::HashSet::new();
        let mut pickers = std::collections::HashSet::new();
        let mut drags = std::collections::HashSet::new();
        let mut containers = std::collections::HashSet::new();
        let mut retained = std::collections::HashSet::new();
        let mut live_keys = std::collections::HashSet::new();
        root.for_each_mut(&mut |node| {
            if let Some(key) = node.key() {
                live_keys.insert(key.to_owned());
            }
            match node {
                wire::Node::Input { key, .. } => {
                    inputs.insert(key.clone());
                }
                wire::Node::Scroll { key, .. } => {
                    scrolls.insert(key.clone());
                }
                wire::Node::PickList { key, .. } | wire::Node::ComboBox { key, .. } => {
                    pickers.insert(key.clone());
                }
                wire::Node::ResizeHandle { key, .. } => {
                    drags.insert(key.clone());
                }
                wire::Node::Responsive { key, .. } => {
                    containers.insert(key.clone());
                }
                wire::Node::Sensor { key, .. }
                | wire::Node::Slider { key, .. }
                | wire::Node::Hover { key, .. }
                | wire::Node::Surface { key, .. }
                | wire::Node::Editor { key, .. } => {
                    retained.insert(key.clone());
                }
                wire::Node::Image {
                    hash,
                    data: Some(data),
                    ..
                } => {
                    self.remember_image(*hash, data);
                }
                wire::Node::ImageViewer {
                    key, hash, data, ..
                } => {
                    retained.insert(key.clone());
                    if let Some(data) = data {
                        self.remember_image(*hash, data);
                    }
                }
                wire::Node::Svg {
                    hash,
                    bytes: Some(bytes),
                    ..
                } => {
                    self.remember_vector(*hash, bytes);
                }
                _ => {}
            }
        });
        self.bounds.retain(|key, _| live_keys.contains(key));
        self.fields.retain(|key, _| inputs.contains(key));
        self.scrolls.retain(|key, _| scrolls.contains(key));
        self.scroll_positions.retain(|key, _| scrolls.contains(key));
        self.pickers.retain(|key, _| pickers.contains(key));
        self.drags.retain(|key, _| drags.contains(key));
        self.containers.retain(|key, _| containers.contains(key));
        self.ranges.retain(|key, _| retained.contains(key));
        self.editors.retain(|key, _| retained.contains(key));
        self.viewers.retain(|key, _| retained.contains(key));
        self.hovered.retain(|key| retained.contains(key));
        self.surfaces.retain(|key, _| retained.contains(key));
        self.sensors.retain(|key, sensor| {
            let mounted = retained.contains(key);
            if !mounted && sensor.size.is_some() {
                if let Some(message) = sensor.on_hide {
                    cx.emit(wire::Event::Message(message));
                }
            }
            mounted
        });
        self.root = root;
        cx.notify();
    }

    fn input(
        &mut self,
        node: &wire::Node,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let wire::Node::Input {
            key,
            value,
            placeholder,
            secure,
            on_input,
            on_submit,
            options,
            width,
            style,
            ..
        } = node
        else {
            unreachable!()
        };
        if !self.fields.contains_key(key) {
            let state = cx.new(|cx| {
                let mut state = InputState::new(window, cx)
                    .placeholder(placeholder.clone())
                    .masked(*secure);
                state.set_value(value.clone(), window, cx);
                state
            });
            let input_key = key.clone();
            let subscription = cx.subscribe_in(&state, window, move |this, input, event, _, cx| {
                let Some(field) = this.fields.get_mut(&input_key) else {
                    return;
                };
                match event {
                    InputEvent::Change => {
                        let text = input.read(cx).value().to_string();
                        if text == field.value {
                            return;
                        }
                        field.value = text.clone();
                        cx.emit(wire::Event::Input {
                            handler: field.on_input,
                            text,
                        });
                    }
                    InputEvent::PressEnter { .. } => {
                        if let Some(message) = field.on_submit {
                            cx.emit(wire::Event::Message(message));
                        }
                    }
                    InputEvent::Focus | InputEvent::Blur => {}
                }
            });
            self.fields.insert(
                key.clone(),
                Field {
                    state,
                    on_input: *on_input,
                    on_submit: *on_submit,
                    value: value.clone(),
                    guest_value: value.clone(),
                    placeholder: placeholder.clone(),
                    secure: *secure,
                    _subscription: subscription,
                },
            );
        }
        let field = self.fields.get_mut(key).expect("field inserted");
        field.on_input = *on_input;
        field.on_submit = *on_submit;
        if field.guest_value != *value {
            field.guest_value = value.clone();
            if field.value != *value {
                field.value = value.clone();
                field
                    .state
                    .update(cx, |state, cx| state.set_value(value.clone(), window, cx));
            }
        }
        if field.placeholder != *placeholder {
            field.placeholder = placeholder.clone();
            field.state.update(cx, |state, cx| {
                state.set_placeholder(placeholder.clone(), window, cx)
            });
        }
        if field.secure != *secure {
            field.secure = *secure;
            field
                .state
                .update(cx, |state, cx| state.set_masked(*secure, window, cx));
        }
        let input = Input::new(&field.state)
            .id(key.clone())
            .aria_label(options.label.clone())
            .disabled(options.disabled);
        let face = match options.disabled {
            true => style.disabled.unwrap_or(style.active),
            false => style.active,
        };
        let mut input = decoration(
            pad(dimensions(input, *width, None), options.padding),
            style.utility.background.or(face.background),
            style.utility.border.or(face.border),
        );
        if let Some(color) = style.utility.value.or(face.value) {
            input = input.text_color(rgba(color));
        }
        if let Some(size) = options.text_size {
            input = input.text_size(px(size));
        }
        if let Some(height) = options.line_height {
            input = input.line_height(relative(height));
        }
        if let Some(font) = &options.font {
            input = input.font_weight(font_weight(font.weight));
        }
        input.into_any_element()
    }

    fn node(
        &mut self,
        node: &wire::Node,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use wire::Node;
        match node {
            Node::Text {
                content,
                size,
                color,
                width,
                font,
                align_x,
                options,
                ..
            } => {
                let mut element = text_options(
                    dimensions(div(), *width, options.height),
                    *font,
                    *align_x,
                    options,
                )
                .child(content.clone());
                if let Some(size) = size {
                    element = element.text_size(px(*size));
                }
                if let Some(color) = color {
                    element = element.text_color(rgba(*color));
                }
                element.into_any_element()
            }
            Node::Space { width, height } => dimensions(div(), *width, *height).into_any_element(),
            Node::Linear {
                axis,
                spacing,
                padding,
                width,
                height,
                background,
                border,
                children,
                align,
                max_width,
                clip,
                wrap,
                ..
            } => {
                let mut element = dimensions(div().flex(), *width, *height);
                element = match axis {
                    wire::Axis::Column => element.flex_col(),
                    wire::Axis::Row => element.flex_row(),
                };
                if let Some(gap) = spacing {
                    element = element.gap(px(*gap));
                }
                if let Some(width) = max_width {
                    element = element.max_w(px(*width));
                }
                if *clip {
                    element = element.overflow_hidden();
                }
                if wrap.is_some() {
                    element = element.flex_wrap();
                }
                element = cross_align(element, *align);
                element = decoration(pad(element, *padding), *background, *border);
                for child in children {
                    element = element.child(self.node(child, window, cx));
                }
                element.into_any_element()
            }
            Node::KeyedColumn {
                spacing,
                padding,
                width,
                height,
                background,
                border,
                children,
                align,
                max_width,
                ..
            } => {
                let mut element = decoration(
                    pad(
                        dimensions(div().flex().flex_col(), *width, *height),
                        *padding,
                    ),
                    *background,
                    *border,
                );
                if let Some(gap) = spacing {
                    element = element.gap(px(*gap));
                }
                element = cross_align(element, *align);
                if let Some(width) = max_width {
                    element = element.max_w(px(*width));
                }
                for child in children {
                    element = element.child(self.node(child, window, cx));
                }
                element.into_any_element()
            }
            Node::Container {
                content,
                width,
                height,
                padding,
                border,
                background,
                max_width,
                max_height,
                clip,
                align_x,
                align_y,
                shadow,
                ..
            } => {
                let color = match background {
                    Some(wire::Background::Color(color)) => Some(*color),
                    _ => None,
                };
                let mut element = shadows(
                    decoration(
                        pad(dimensions(div().flex(), *width, *height), *padding),
                        color,
                        *border,
                    ),
                    *shadow,
                );
                element = horizontal_align(element, *align_x);
                element = vertical_align(element, *align_y);
                if let Some(width) = max_width {
                    element = element.max_w(px(*width));
                }
                if let Some(height) = max_height {
                    element = element.max_h(px(*height));
                }
                if *clip {
                    element = element.overflow_hidden();
                }
                element
                    .child(self.node(content, window, cx))
                    .into_any_element()
            }
            Node::Scroll {
                key,
                content,
                width,
                height,
                direction,
                anchor_x,
                anchor_y,
                auto_scroll,
                on_scroll,
                background,
                border,
                ..
            } => {
                let handle = self.scrolls.entry(key.clone()).or_default().clone();
                let element = decoration(
                    dimensions(div().relative(), *width, *height),
                    *background,
                    *border,
                )
                .id(key.clone())
                .track_scroll(&handle);
                let element = match direction {
                    wire::ScrollDirection::Vertical => element.overflow_y_scroll(),
                    wire::ScrollDirection::Horizontal => element.overflow_x_scroll(),
                    wire::ScrollDirection::Both => element.overflow_scroll(),
                };
                let route = key.clone();
                let anchors = (*anchor_x, *anchor_y);
                let follow = *auto_scroll;
                let handler = *on_scroll;
                let weak = cx.entity().downgrade();
                let observe = canvas(
                    |_, _, _| (),
                    move |_, _, _, cx| {
                        let maximum = handle.max_offset();
                        let offset = handle.offset();
                        let _ = weak.update(cx, |this, cx| {
                            let previous = this.scroll_positions.get(&route).copied();
                            let mut next = offset;
                            for (position, maximum, previous, anchor) in [
                                (
                                    &mut next.x,
                                    maximum.x,
                                    previous.map(|(offset, max)| (offset.x, max.x)),
                                    anchors.0,
                                ),
                                (
                                    &mut next.y,
                                    maximum.y,
                                    previous.map(|(offset, max)| (offset.y, max.y)),
                                    anchors.1,
                                ),
                            ] {
                                let at_end = previous.is_some_and(|(offset, max)| {
                                    f32::from(offset + max).abs() < 2.0
                                });
                                let initialize_end =
                                    previous.is_none() && anchor == wire::ScrollAnchor::End;
                                if initialize_end || (follow && at_end) {
                                    *position = -maximum;
                                }
                            }
                            if next != offset {
                                handle.set_offset(next);
                                cx.notify();
                            }
                            let changed = previous
                                .is_none_or(|(offset, max)| offset != next || max != maximum);
                            this.scroll_positions.insert(route.clone(), (next, maximum));
                            if changed {
                                if let Some(handler) = handler {
                                    let x = -f32::from(next.x);
                                    let y = -f32::from(next.y);
                                    let relative_x = x / f32::from(maximum.x).max(1.0);
                                    let relative_y = y / f32::from(maximum.y).max(1.0);
                                    cx.emit(wire::Event::ScrollOffset {
                                        handler,
                                        x,
                                        y,
                                        relative_x,
                                        relative_y,
                                    });
                                }
                            }
                        });
                    },
                )
                .absolute()
                .inset_0();
                element
                    .child(self.node(content, window, cx))
                    .child(observe)
                    .into_any_element()
            }
            Node::Button {
                key,
                content,
                label,
                on_press,
                width,
                height,
                padding,
                style,
                ..
            } => {
                let mut button =
                    button_style(Button::new(key.clone()), style, on_press.is_none(), cx)
                        .disabled(on_press.is_none());
                button = match content {
                    wire::ButtonContent::Label(text) => button.label(text.clone()),
                    wire::ButtonContent::Child(child) => button.child(self.node(child, window, cx)),
                };
                if let Some(label) = label {
                    button = button.accessibility_label(label.clone());
                }
                if let Some(message) = on_press {
                    let message = *message;
                    button = button.on_click(
                        cx.listener(move |_, _, _, cx| cx.emit(wire::Event::Message(message))),
                    );
                }
                dimensions(pad(button, *padding), *width, *height).into_any_element()
            }
            Node::Input { .. } => self.input(node, window, cx),
            Node::PickList {
                key,
                options,
                selected,
                on_select,
                placeholder,
                ..
            } => self.picker(
                key,
                options,
                *selected,
                *on_select,
                placeholder.as_deref().unwrap_or_default(),
                window,
                cx,
            ),
            Node::ComboBox {
                key,
                options,
                selected,
                on_select,
                placeholder,
                ..
            } => self.picker(key, options, *selected, *on_select, placeholder, window, cx),
            Node::Toggle {
                key,
                kind,
                label,
                checked,
                on_toggle,
                ..
            } => {
                if *kind == wire::ToggleKind::Switch {
                    let mut toggle = gpui_kit::component::switch::Switch::new(key.clone())
                        .label(label.clone())
                        .checked(*checked)
                        .disabled(on_toggle.is_none());
                    if let Some(handler) = on_toggle {
                        let handler = *handler;
                        toggle = toggle.on_click(cx.listener(move |_, on, _, cx| {
                            cx.emit(wire::Event::Toggle { handler, on: *on })
                        }));
                    }
                    return toggle.into_any_element();
                }
                let mut checkbox = Checkbox::new(key.clone())
                    .label(label.clone())
                    .checked(*checked)
                    .disabled(on_toggle.is_none());
                if let Some(handler) = on_toggle {
                    let handler = *handler;
                    checkbox = checkbox.on_click(cx.listener(move |_, on, _, cx| {
                        cx.emit(wire::Event::Toggle { handler, on: *on })
                    }));
                }
                checkbox.into_any_element()
            }
            Node::Radio {
                key,
                label,
                selected,
                on_select,
                ..
            } => {
                let message = *on_select;
                Radio::new(key.clone())
                    .label(label.clone())
                    .checked(*selected)
                    .on_click(
                        cx.listener(move |_, _, _, cx| cx.emit(wire::Event::Message(message))),
                    )
                    .into_any_element()
            }
            Node::Rule {
                axis,
                thickness,
                color,
                ..
            } => {
                let element = div().bg(color
                    .map(rgba)
                    .unwrap_or_else(|| gpui_kit::rgb(0xdad9d3).into()));
                match axis {
                    wire::Axis::Column => element.w(px(*thickness)).h_full().into_any_element(),
                    wire::Axis::Row => element.h(px(*thickness)).w_full().into_any_element(),
                }
            }
            Node::Lazy { content, .. } => self.node(content, window, cx),
            Node::ResizeHandle {
                key,
                on_press,
                on_release,
                on_drag,
                content,
                ..
            } => {
                let press_key = key.clone();
                let move_key = key.clone();
                let release_key = key.clone();
                let press = *on_press;
                let release = *on_release;
                let drag = *on_drag;
                div()
                    .id(key.clone())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            this.drags.insert(press_key.clone(), event.position);
                            if let Some(message) = press {
                                cx.emit(wire::Event::Message(message));
                            }
                        }),
                    )
                    .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                        let Some(previous) = this.drags.get_mut(&move_key) else {
                            return;
                        };
                        if event.pressed_button != Some(MouseButton::Left) {
                            this.drags.remove(&move_key);
                            return;
                        }
                        let delta = event.position - *previous;
                        *previous = event.position;
                        if let Some(handler) = drag {
                            cx.emit(wire::Event::Drag {
                                handler,
                                dx: f32::from(delta.x) as f64,
                                dy: f32::from(delta.y) as f64,
                            });
                        }
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            let was_dragging = this.drags.remove(&release_key).is_some();
                            if was_dragging {
                                if let Some(message) = release {
                                    cx.emit(wire::Event::Message(message));
                                }
                            }
                        }),
                    )
                    .child(self.node(content, window, cx))
                    .into_any_element()
            }
            Node::Responsive {
                key,
                content,
                width,
                height,
            } => {
                let weak = cx.entity().downgrade();
                let key = key.clone();
                let measure = canvas(
                    move |bounds, _, cx| {
                        let size = [
                            f32::from(bounds.size.width) as f64,
                            f32::from(bounds.size.height) as f64,
                        ];
                        let _ = weak.update(cx, |this, cx| {
                            let changed = this.containers.get(&key) != Some(&size);
                            if changed {
                                this.containers.insert(key, size);
                                cx.notify();
                            }
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0();
                dimensions(div().relative(), *width, *height)
                    .child(self.node(content, window, cx))
                    .child(measure)
                    .into_any_element()
            }
            Node::When {
                condition,
                children,
                ..
            } => {
                let mut element = div().flex().flex_col();
                if condition.matches(&self.containers) {
                    for child in children {
                        element = element.child(self.node(child, window, cx));
                    }
                }
                element.into_any_element()
            }
            Node::Sensor {
                key,
                reset,
                on_show,
                on_resize,
                on_hide,
                anticipate,
                delay,
                child,
                ..
            } => {
                let sensor = self.sensors.entry(key.clone()).or_insert(SensorState {
                    reset: reset.clone(),
                    size: None,
                    on_hide: *on_hide,
                    on_show: *on_show,
                    on_resize: *on_resize,
                    pending: None,
                });
                if sensor.reset != *reset {
                    sensor.reset = reset.clone();
                    sensor.size = None;
                    sensor.pending = None;
                }
                sensor.on_hide = *on_hide;
                sensor.on_show = *on_show;
                sensor.on_resize = *on_resize;
                let route = key.clone();
                let show = *on_show;
                let resize = *on_resize;
                let anticipate = px(anticipate.unwrap_or_default());
                let delay =
                    std::time::Duration::from_secs_f32(delay.unwrap_or_default().max(0.0) / 1000.0);
                let weak = cx.entity().downgrade();
                let measure = canvas(
                    move |bounds, window, cx| {
                        let viewport = window.content_mask().bounds;
                        let visible_bounds = Bounds::new(
                            bounds.origin - point(anticipate, anticipate),
                            bounds.size + size(anticipate * 2.0, anticipate * 2.0),
                        );
                        let visible = viewport.intersects(&visible_bounds);
                        let _ = weak.update(cx, |this, cx| {
                            let Some(sensor) = this.sensors.get_mut(&route) else {
                                return;
                            };
                            if !visible {
                                sensor.pending = None;
                                if sensor.size.take().is_some() {
                                    if let Some(message) = sensor.on_hide {
                                        cx.emit(wire::Event::Message(message));
                                    }
                                }
                                return;
                            }
                            let unchanged = sensor.size == Some(bounds.size);
                            if unchanged {
                                sensor.pending = None;
                                return;
                            }
                            if !delay.is_zero() {
                                let waiting = sensor
                                    .pending
                                    .as_ref()
                                    .is_some_and(|(size, _)| *size == bounds.size);
                                if waiting {
                                    return;
                                }
                                let route = route.clone();
                                let size = bounds.size;
                                let timer = cx.background_executor().timer(delay);
                                let pending = cx.spawn(async move |this, cx| {
                                    timer.await;
                                    let _ = this.update(cx, |this, cx| {
                                        let Some(sensor) = this.sensors.get_mut(&route) else {
                                            return;
                                        };
                                        let current = sensor
                                            .pending
                                            .as_ref()
                                            .is_some_and(|(pending, _)| *pending == size);
                                        if !current {
                                            return;
                                        }
                                        let handler = match sensor.size {
                                            None => sensor.on_show,
                                            Some(_) => sensor.on_resize,
                                        };
                                        sensor.size = Some(size);
                                        sensor.pending = None;
                                        if let Some(handler) = handler {
                                            cx.emit(wire::Event::Size {
                                                handler,
                                                width: f32::from(size.width),
                                                height: f32::from(size.height),
                                            });
                                        }
                                    });
                                });
                                sensor.pending = Some((size, pending));
                                return;
                            }
                            let handler = match sensor.size {
                                None => show,
                                Some(previous) if previous != bounds.size => resize,
                                Some(_) => None,
                            };
                            sensor.size = Some(bounds.size);
                            if let Some(handler) = handler {
                                cx.emit(wire::Event::Size {
                                    handler,
                                    width: f32::from(bounds.size.width),
                                    height: f32::from(bounds.size.height),
                                });
                            }
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0();
                div()
                    .relative()
                    .child(self.node(child, window, cx))
                    .child(measure)
                    .into_any_element()
            }
            Node::MouseArea { .. } => self.mouse_area(node, window, cx),
            Node::Slider { .. } => self.slider(node, window, cx),
            Node::RichText { .. } => self.rich_text(node, window, cx),
            Node::Flex { .. } => self.flex(node, window, cx),
            Node::Grid {
                key,
                children,
                width,
                height,
                padding,
                spacing,
                columns,
                fluid,
                aspect,
                background,
                border,
            } => {
                let available = self
                    .bounds
                    .get(key)
                    .map_or(f32::from(window.viewport_size().width), |bounds| {
                        f32::from(bounds.size.width)
                    });
                let columns = fluid
                    .filter(|value| *value > 0.0)
                    .map_or(columns.unwrap_or(1), |value| {
                        (available / value).ceil().max(1.0) as u32
                    })
                    .max(1);
                let mut grid = decoration(
                    pad(
                        dimensions(div().flex().flex_wrap(), *width, *height),
                        *padding,
                    ),
                    *background,
                    *border,
                );
                let gap = spacing.unwrap_or_default();
                grid = grid.gap(px(gap));
                let cell_width = ((available - gap * columns.saturating_sub(1) as f32)
                    / columns as f32)
                    .max(0.0);
                for child in children {
                    let cell = div()
                        .w(px(cell_width))
                        .h(px(cell_width / aspect.unwrap_or(1.0).max(0.001)));
                    grid = grid.child(cell.child(self.node(child, window, cx)));
                }
                grid.relative()
                    .child(self.measure(key, cx))
                    .into_any_element()
            }
            Node::Hover {
                key,
                children,
                open,
                width,
                height,
                padding,
                background,
                border,
                tint,
                radius,
            } => {
                let route = key.clone();
                let mut element = decoration(
                    pad(dimensions(div().relative(), *width, *height), *padding),
                    *background,
                    *border,
                )
                .id(key.clone());
                let reveal = *open || self.hovered.contains(key);
                if let Some(base) = children.first() {
                    element = element.child(self.node(base, window, cx));
                }
                if reveal {
                    if let Some(child) = children.get(1) {
                        let mut layer = div().absolute().inset_0().rounded(px(*radius));
                        if let Some(color) = tint {
                            layer = layer.bg(rgba(*color));
                        }
                        element = element.child(layer.child(self.node(child, window, cx)));
                    }
                }
                element
                    .on_hover(cx.listener(move |this, hovered, _, cx| {
                        match hovered {
                            true => {
                                this.hovered.insert(route.clone());
                            }
                            false => {
                                this.hovered.remove(&route);
                            }
                        }
                        cx.notify();
                    }))
                    .into_any_element()
            }
            Node::Tooltip {
                key,
                children,
                delay_ms,
                ..
            } => {
                let Some(content) = children.first() else {
                    return div().into_any_element();
                };
                let mut element = div()
                    .id(key.clone())
                    .tooltip_show_delay(std::time::Duration::from_millis(*delay_ms))
                    .child(self.node(content, window, cx));
                if let Some(tip) = children.get(1) {
                    let tip = tip.clone();
                    element =
                        element.tooltip(move |_, cx| cx.new(|_| ViewTree::new(tip.clone())).into());
                }
                element.into_any_element()
            }
            Node::Float {
                key,
                content,
                x,
                y,
                scale: _,
                shadow,
                radius,
            } => {
                let bounds = self.bounds.get(key).copied().unwrap_or_default();
                let viewport = window.viewport_size();
                let geometry = [
                    f32::from(bounds.origin.x) as f64,
                    f32::from(bounds.origin.y) as f64,
                    f32::from(bounds.size.width) as f64,
                    f32::from(bounds.size.height) as f64,
                    0.0,
                    0.0,
                    f32::from(viewport.width) as f64,
                    f32::from(viewport.height) as f64,
                ];
                let mut element = shadows(
                    div()
                        .relative()
                        .left(px(x.evaluate(geometry)))
                        .top(px(y.evaluate(geometry))),
                    *shadow,
                );
                if let Some(radius) = radius {
                    element = decoration(
                        element,
                        None,
                        Some(wire::Border {
                            color: None,
                            width: None,
                            radius: Some(*radius),
                        }),
                    );
                }
                // Authored floating rails use unit scale; their measurement is
                // outside the translated child to avoid positional feedback.
                div()
                    .relative()
                    .child(element.child(self.node(content, window, cx)))
                    .child(self.measure(key, cx))
                    .into_any_element()
            }
            Node::Image {
                hash,
                data,
                width,
                height,
                fit,
                opacity,
                ..
            } => self.picture(
                *hash,
                data.as_ref(),
                *width,
                *height,
                *fit,
                opacity.unwrap_or(1.0),
            ),
            Node::ImageViewer { .. } => self.image_viewer(node, window, cx),
            Node::Svg {
                hash,
                bytes,
                color,
                width,
                height,
                opacity,
                ..
            } => {
                if let Some(bytes) = bytes {
                    self.remember_vector(*hash, bytes);
                }
                let mut element =
                    dimensions(div(), *width, *height).opacity(opacity.unwrap_or(1.0));
                if let Some(bytes) = self.vectors.get(hash) {
                    let mut icon = svg().data(bytes).size_full();
                    if let Some(color) = color {
                        icon = icon.text_color(rgba(*color));
                    }
                    element = element.child(icon);
                }
                element.into_any_element()
            }
            Node::Canvas {
                key,
                width,
                height,
                commands,
                ..
            } => {
                let bounds = self.bounds.get(key).copied().unwrap_or_default();
                let known = |length: Option<wire::Length>, measured: Pixels| match length {
                    Some(wire::Length::Fixed(value)) => value,
                    _ => f32::from(measured).max(1.0),
                };
                dimensions(div().relative(), *width, *height)
                    .child(
                        img(Arc::new(Image::from_bytes(
                            ImageFormat::Svg,
                            canvas_svg(
                                commands,
                                known(*width, bounds.size.width),
                                known(*height, bounds.size.height),
                            ),
                        )))
                        .size_full()
                        .object_fit(ObjectFit::Fill),
                    )
                    .child(self.measure(key, cx))
                    .into_any_element()
            }
            Node::Qr { code, .. } => qr(code),
            Node::Surface { key, name, .. } => match self.surfaces.get(key) {
                Some(surface) => surface.clone().into_any_element(),
                None => div()
                    .child(format!("Unavailable host surface: {name}"))
                    .into_any_element(),
            },
            Node::Stack {
                children,
                width,
                height,
                padding,
                background,
                border,
                clip,
                under,
                ..
            } => {
                let mut element = decoration(
                    pad(dimensions(div().relative(), *width, *height), *padding),
                    *background,
                    *border,
                );
                if *clip {
                    element = element.overflow_hidden();
                }
                if *under == 0 {
                    element = element.grid().grid_cols(1).grid_rows(1);
                }
                for (index, child) in children.iter().enumerate() {
                    let content = self.node(child, window, cx);
                    element = match (*under, index) {
                        (0, _) => element.child(div().col_start(1).row_start(1).child(content)),
                        (base, index) if index == base as usize => element.child(content),
                        _ => element.child(div().absolute().inset_0().child(content)),
                    };
                }
                element.into_any_element()
            }
            Node::Overlay {
                key,
                children,
                backdrop,
                padding,
                align_x,
                align_y,
                on_dismiss,
            } => {
                let mut element = div().relative().size_full();
                if let Some(base) = children.first() {
                    element = element.child(self.node(base, window, cx));
                }
                if let Some(modal) = children.get(1) {
                    let shade = div()
                        .id(format!("{key}/backdrop"))
                        .absolute()
                        .inset_0()
                        .bg(rgba(*backdrop));
                    let mut layer = div()
                        .id(format!("{key}/layer"))
                        .absolute()
                        .inset_0()
                        .flex()
                        .p(px(*padding));
                    if let Some(message) = on_dismiss {
                        let message = *message;
                        layer = layer.on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |_, _, _, cx| cx.emit(wire::Event::Message(message))),
                        );
                    }
                    layer = match align_x {
                        wire::AlignX::Left => layer.justify_start(),
                        wire::AlignX::Center => layer.justify_center(),
                        wire::AlignX::Right => layer.justify_end(),
                    };
                    layer = match align_y {
                        wire::AlignY::Top => layer.items_start(),
                        wire::AlignY::Center => layer.items_center(),
                        wire::AlignY::Bottom => layer.items_end(),
                    };
                    element = element.child(shade).child(
                        layer.child(
                            div()
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .child(self.node(modal, window, cx)),
                        ),
                    );
                }
                element.into_any_element()
            }
            Node::Progress {
                value,
                min,
                max,
                axis,
                length,
                girth,
                background,
                bar,
                border,
                ..
            } => {
                let span = max - min;
                let valid = span.is_finite() && span > 0.0;
                let fraction = match valid {
                    true => ((value - min) / span).clamp(0.0, 1.0),
                    false => 0.0,
                };
                let fill = div().bg(bar
                    .map(rgba)
                    .unwrap_or_else(|| gpui_kit::rgb(0xa05a3c).into()));
                match axis {
                    wire::Axis::Row => {
                        decoration(dimensions(div(), *length, *girth), *background, *border)
                            .child(fill.w(relative(fraction)).h_full())
                            .into_any_element()
                    }
                    wire::Axis::Column => decoration(
                        dimensions(div().flex().flex_col().justify_end(), *girth, *length),
                        *background,
                        *border,
                    )
                    .child(fill.h(relative(fraction)).w_full())
                    .into_any_element(),
                }
            }
            Node::Pin {
                content,
                x,
                y,
                width,
                height,
                ..
            } => dimensions(div().absolute().left(px(*x)).top(px(*y)), *width, *height)
                .child(self.node(content, window, cx))
                .into_any_element(),
            Node::Editor { .. } => self.editor(node, window, cx),
        }
    }

    fn editor(
        &mut self,
        node: &wire::Node,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let wire::Node::Editor { key, .. } = node else {
            unreachable!()
        };
        let Some(store) = self.editor_store.clone() else {
            return div().child("Editor host is unavailable").into_any_element();
        };
        if !self.editors.contains_key(key) {
            let view = cx.new(|cx| {
                crate::editor::wire::WireEditor::new(key.clone(), store.clone(), window, cx)
            });
            let subscription = cx.subscribe(&view, move |_, _, _: &(), cx| {
                for event in store.drain() {
                    cx.emit(event);
                }
            });
            self.editors.insert(
                key.clone(),
                EditorMount {
                    view,
                    _subscription: subscription,
                },
            );
        }
        let editor = self.editors.get(key).expect("editor inserted");
        editor.view.update(cx, |editor, cx| editor.sync(window, cx));
        editor.view.clone().into_any_element()
    }

    fn measure(&self, key: &str, cx: &Context<Self>) -> impl IntoElement + use<> {
        let route = key.to_owned();
        let weak = cx.entity().downgrade();
        canvas(
            move |bounds, _, cx| {
                let _ = weak.update(cx, |this, cx| {
                    let changed = this.bounds.get(&route) != Some(&bounds);
                    if changed {
                        this.bounds.insert(route, bounds);
                        cx.notify();
                    }
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0()
    }

    fn mouse_area(
        &mut self,
        node: &wire::Node,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let wire::Node::MouseArea {
            key,
            content,
            on_press,
            on_release,
            on_double_click,
            on_right_press,
            on_right_release,
            on_middle_press,
            on_middle_release,
            on_enter,
            on_exit,
            on_move,
            on_press_at,
            on_scroll,
        } = node
        else {
            unreachable!()
        };
        let mut element = div().id(key.clone()).relative();
        for (button, down, up) in [
            (MouseButton::Left, *on_press, *on_release),
            (MouseButton::Right, *on_right_press, *on_right_release),
            (MouseButton::Middle, *on_middle_press, *on_middle_release),
        ] {
            if let Some(message) = down {
                element = element.on_mouse_down(
                    button,
                    cx.listener(move |_, _, _, cx| cx.emit(wire::Event::Message(message))),
                );
            }
            if let Some(message) = up {
                element = element.on_mouse_up(
                    button,
                    cx.listener(move |_, _, _, cx| cx.emit(wire::Event::Message(message))),
                );
            }
        }
        if let Some(message) = on_double_click {
            let message = *message;
            element = element.on_mouse_down(
                MouseButton::Left,
                cx.listener(move |_, event, _, cx| {
                    if event.click_count == 2 {
                        cx.emit(wire::Event::Message(message));
                    }
                }),
            );
        }
        let enter = *on_enter;
        let exit = *on_exit;
        element = element.on_hover(cx.listener(move |_, hovered, _, cx| {
            let message = match hovered {
                true => enter,
                false => exit,
            };
            if let Some(message) = message {
                cx.emit(wire::Event::Message(message));
            }
        }));
        if let Some(handler) = on_move {
            let handler = *handler;
            let route = key.clone();
            element = element.on_mouse_move(cx.listener(move |this, event, _, cx| {
                let origin = this
                    .bounds
                    .get(&route)
                    .map_or(Point::default(), |bounds| bounds.origin);
                let local = event.position - origin;
                cx.emit(wire::Event::Pointer {
                    handler,
                    x: f32::from(local.x),
                    y: f32::from(local.y),
                });
            }));
        }
        if let Some(handler) = on_press_at {
            let handler = *handler;
            let route = key.clone();
            element = element.capture_any_mouse_down(cx.listener(
                move |this, event: &MouseDownEvent, _, cx| {
                    if event.button != MouseButton::Left {
                        return;
                    }
                    let Some(bounds) = this.bounds.get(&route) else {
                        return;
                    };
                    if !bounds.contains(&event.position) {
                        return;
                    }
                    let local = event.position - bounds.origin;
                    cx.emit(wire::Event::Pointer {
                        handler,
                        x: f32::from(local.x),
                        y: f32::from(local.y),
                    });
                },
            ));
        }
        if let Some(handler) = on_scroll {
            let handler = *handler;
            element = element.on_scroll_wheel(cx.listener(move |_, event, _, cx| {
                let (delta, pixels) = match event.delta {
                    ScrollDelta::Pixels(delta) => {
                        (point(f32::from(delta.x), f32::from(delta.y)), true)
                    }
                    ScrollDelta::Lines(delta) => (delta, false),
                };
                cx.emit(wire::Event::Scroll {
                    handler,
                    dx: delta.x,
                    dy: delta.y,
                    pixels,
                });
            }));
        }
        element
            .child(self.node(content, window, cx))
            .child(self.measure(key, cx))
            .into_any_element()
    }

    fn slider(
        &mut self,
        node: &wire::Node,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let wire::Node::Slider {
            key,
            value,
            min,
            max,
            step,
            on_change,
            on_release,
            axis,
            width,
            height,
            ..
        } = node
        else {
            unreachable!()
        };
        let bounds = [*min, *max, *step];
        let rebuild = self
            .ranges
            .get(key)
            .is_none_or(|control| control.bounds != bounds);
        if rebuild {
            let state = cx.new(|_| {
                SliderState::new()
                    .min(*min)
                    .max(*max)
                    .step(*step)
                    .default_value(*value)
            });
            let route = key.clone();
            let subscription = cx.subscribe_in(&state, window, move |this, _, event, _, cx| {
                let Some(control) = this.ranges.get_mut(&route) else {
                    return;
                };
                match event {
                    SliderEvent::Change(value) => {
                        control.value = value.start();
                        cx.emit(wire::Event::Slide {
                            handler: control.on_change,
                            value: control.value,
                        });
                    }
                    SliderEvent::Release(_) => {
                        if let Some(message) = control.on_release {
                            cx.emit(wire::Event::Message(message));
                        }
                    }
                }
            });
            self.ranges.insert(
                key.clone(),
                RangeControl {
                    state,
                    bounds,
                    value: *value,
                    on_change: *on_change,
                    on_release: *on_release,
                    _subscription: subscription,
                },
            );
        }
        let control = self.ranges.get_mut(key).expect("range inserted");
        control.on_change = *on_change;
        control.on_release = *on_release;
        if control.value != *value {
            control.value = *value;
            control
                .state
                .update(cx, |state, cx| state.set_value(*value, window, cx));
        }
        let slider = match axis {
            wire::Axis::Row => Slider::new(&control.state).horizontal(),
            wire::Axis::Column => Slider::new(&control.state).vertical(),
        };
        dimensions(div(), *width, *height)
            .child(slider)
            .into_any_element()
    }

    fn rich_text(
        &mut self,
        node: &wire::Node,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let wire::Node::RichText {
            key,
            spans,
            size,
            color,
            font,
            width,
            align_x,
            options,
            on_link,
        } = node
        else {
            unreachable!()
        };
        let mut text = String::new();
        let mut highlights = Vec::new();
        let mut ranges = Vec::new();
        let mut links = Vec::new();
        for span in spans {
            let start = text.len();
            text.push_str(&span.content);
            let range = start..text.len();
            let mut style = HighlightStyle::default();
            style.color = span.color.map(rgba);
            style.background_color = span.background.map(rgba);
            if let Some(font) = &span.font {
                style.font_weight = Some(font_weight(font.weight));
            }
            if span.underline {
                style.underline = Some(UnderlineStyle {
                    thickness: px(1.0),
                    color: span.color.map(rgba),
                    wavy: false,
                });
            }
            if span.strikethrough {
                style.strikethrough = Some(StrikethroughStyle {
                    thickness: px(1.0),
                    color: span.color.map(rgba),
                });
            }
            highlights.push((range.clone(), style));
            if let Some(link) = &span.link {
                ranges.push(range);
                links.push(link.clone());
            }
        }
        let styled = StyledText::new(text).with_highlights(highlights);
        let mut interactive = InteractiveText::new(key.clone(), styled);
        if let Some(handler) = on_link {
            let handler = *handler;
            let weak = cx.entity().downgrade();
            interactive = interactive.on_click(ranges, move |index, _, cx| {
                let Some(link) = links.get(index) else {
                    return;
                };
                let _ = weak.update(cx, |_, cx| {
                    cx.emit(wire::Event::Input {
                        handler,
                        text: link.clone(),
                    })
                });
            });
        }
        let mut element = text_options(
            dimensions(div(), *width, options.height),
            *font,
            *align_x,
            options,
        );
        if let Some(size) = size {
            element = element.text_size(px(*size));
        }
        if let Some(color) = color {
            element = element.text_color(rgba(*color));
        }
        element.child(interactive).into_any_element()
    }

    fn flex(
        &mut self,
        node: &wire::Node,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let wire::Node::Flex {
            layout,
            children,
            items,
            background,
            border,
            ..
        } = node
        else {
            unreachable!()
        };
        let mut element = decoration(
            pad(
                dimensions(div().flex(), layout.width, layout.height),
                layout.padding,
            ),
            *background,
            *border,
        );
        element = match layout.direction {
            wire::FlexDirection::Row => element.flex_row(),
            wire::FlexDirection::RowReverse => element.flex_row_reverse(),
            wire::FlexDirection::Column => element.flex_col(),
            wire::FlexDirection::ColumnReverse => element.flex_col_reverse(),
        };
        element = match layout.wrap {
            wire::FlexWrap::NoWrap => element,
            wire::FlexWrap::Wrap => element.flex_wrap(),
            wire::FlexWrap::WrapReverse => element.flex_wrap_reverse(),
        };
        if let Some(width) = layout.max_width {
            element = element.max_w(px(width));
        }
        if let Some(height) = layout.max_height {
            element = element.max_h(px(height));
        }
        if let Some(gap) = layout.row_gap {
            element = element.gap_y(px(gap));
        }
        if let Some(gap) = layout.column_gap {
            element = element.gap_x(px(gap));
        }
        if layout.clip {
            element = element.overflow_hidden();
        }
        if let Some(alignment) = layout.justify {
            element = justify(element, alignment);
        }
        if let Some(alignment) = layout.items {
            element = align_items(element, alignment);
        }
        if let Some(alignment) = layout.content {
            element = match alignment {
                wire::FlexContentAlignment::Start | wire::FlexContentAlignment::FlexStart => {
                    element.content_start()
                }
                wire::FlexContentAlignment::End | wire::FlexContentAlignment::FlexEnd => {
                    element.content_end()
                }
                wire::FlexContentAlignment::Center => element.content_center(),
                wire::FlexContentAlignment::SpaceBetween => element.content_between(),
                wire::FlexContentAlignment::SpaceAround => element.content_around(),
                wire::FlexContentAlignment::SpaceEvenly => element.content_evenly(),
                wire::FlexContentAlignment::Stretch => element.content_stretch(),
            };
        }
        let mut order: Vec<_> = children.iter().enumerate().collect();
        order.sort_by_key(|(index, _)| items.get(*index).map_or(0, |item| item.order));
        for (index, child) in order {
            let mut item = div();
            if let Some(rules) = items.get(index) {
                item.style().flex_grow = rules.grow;
                item.style().flex_shrink = Some(rules.shrink);
                item.style().flex_basis = match rules.basis {
                    wire::FlexBasis::Auto | wire::FlexBasis::Content => Some(auto()),
                    wire::FlexBasis::Fixed(value) => Some(px(value).into()),
                    wire::FlexBasis::Percent(value) => Some(relative(value / 100.0).into()),
                };
                if let Some(alignment) = rules.align {
                    item = match alignment {
                        wire::FlexItemAlignment::Start => item.self_start(),
                        wire::FlexItemAlignment::FlexStart => item.self_flex_start(),
                        wire::FlexItemAlignment::End => item.self_end(),
                        wire::FlexItemAlignment::FlexEnd => item.self_flex_end(),
                        wire::FlexItemAlignment::Center => item.self_center(),
                        wire::FlexItemAlignment::Baseline => item.self_baseline(),
                        wire::FlexItemAlignment::Stretch => item.self_stretch(),
                    };
                }
                let margin = |value| match value {
                    wire::FlexMargin::Zero => px(0.0).into(),
                    wire::FlexMargin::Auto => auto(),
                    wire::FlexMargin::Fixed(value) => px(value).into(),
                    wire::FlexMargin::Percent(value) => relative(value / 100.0).into(),
                };
                item = item
                    .mt(margin(rules.margins.top))
                    .mr(margin(rules.margins.right))
                    .mb(margin(rules.margins.bottom))
                    .ml(margin(rules.margins.left));
            }
            element = element.child(item.child(self.node(child, window, cx)));
        }
        let mut outer = dimensions(div(), layout.surface_width, layout.surface_height);
        if let Some(width) = layout.surface_max_width {
            outer = outer.max_w(px(width));
        }
        outer.child(element).into_any_element()
    }

    fn remember_image(&mut self, hash: u64, data: &wire::ImageData) {
        if self.images.contains_key(&hash) || self.images.len() >= 4096 {
            return;
        }
        let bytes: usize = self
            .images
            .values()
            .filter_map(|image| image.as_bytes(0))
            .map(<[u8]>::len)
            .sum();
        let Some(image) = decode_image(data) else {
            return;
        };
        let next = image.as_bytes(0).map_or(0, <[u8]>::len);
        if bytes.saturating_add(next) > 64 << 20 {
            return;
        }
        self.images.insert(hash, Arc::new(image));
    }

    fn remember_vector(&mut self, hash: u64, bytes: &[u8]) {
        if self.vectors.contains_key(&hash) || self.vectors.len() >= 4096 {
            return;
        }
        let total: usize = self.vectors.values().map(|bytes| bytes.len()).sum();
        if total.saturating_add(bytes.len()) > 16 << 20 {
            return;
        }
        self.vectors.insert(hash, Arc::from(bytes));
    }

    fn image_viewer(
        &mut self,
        node: &wire::Node,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let wire::Node::ImageViewer {
            key,
            hash,
            data,
            width,
            height,
            options,
            ..
        } = node
        else {
            unreachable!()
        };
        if let Some(data) = data {
            self.remember_image(*hash, data);
        }
        let viewer = self.viewers.entry(key.clone()).or_default();
        if viewer.scale == 0.0 {
            viewer.scale = 1.0;
        }
        let mut element =
            dimensions(div().relative().overflow_hidden(), *width, *height).id(key.clone());
        if let Some(image) = self.images.get(hash) {
            let original = image.size(0);
            let viewport = self
                .bounds
                .get(key)
                .map_or(window.viewport_size(), |bounds| bounds.size);
            let inset = options.padding.unwrap_or_default() * 2.0;
            let ratio = ((f32::from(viewport.width) - inset) / u32::from(original.width) as f32)
                .min((f32::from(viewport.height) - inset) / u32::from(original.height) as f32)
                .max(0.0);
            let width = u32::from(original.width) as f32 * ratio * viewer.scale;
            let height = u32::from(original.height) as f32 * ratio * viewer.scale;
            let x = (f32::from(viewport.width) - width) / 2.0 + f32::from(viewer.offset.x);
            let y = (f32::from(viewport.height) - height) / 2.0 + f32::from(viewer.offset.y);
            element = element.child(
                img(image.clone())
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(width))
                    .h(px(height)),
            );
        }
        let (minimum, maximum) = options.scale_bounds.unwrap_or((0.25, 10.0));
        let step = options.scale_step.unwrap_or(0.1);
        let wheel_key = key.clone();
        element = element.on_scroll_wheel(cx.listener(move |this, event, _, cx| {
            let delta = match event.delta {
                ScrollDelta::Pixels(delta) => f32::from(delta.y),
                ScrollDelta::Lines(delta) => delta.y,
            };
            let Some(viewer) = this.viewers.get_mut(&wheel_key) else {
                return;
            };
            viewer.scale =
                (viewer.scale * (1.0 + step).powf(delta.signum())).clamp(minimum, maximum);
            cx.stop_propagation();
            cx.notify();
        }));
        let down_key = key.clone();
        element = element.on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event, _, cx| {
                if let Some(viewer) = this.viewers.get_mut(&down_key) {
                    viewer.drag = Some(event.position);
                }
                cx.stop_propagation();
            }),
        );
        let move_key = key.clone();
        element = element.on_mouse_move(cx.listener(move |this, event, _, cx| {
            let Some(viewer) = this.viewers.get_mut(&move_key) else {
                return;
            };
            let Some(previous) = viewer.drag else {
                return;
            };
            if event.pressed_button != Some(MouseButton::Left) {
                viewer.drag = None;
                return;
            }
            viewer.offset += event.position - previous;
            viewer.drag = Some(event.position);
            cx.notify();
        }));
        let up_key = key.clone();
        element = element.on_mouse_up(
            MouseButton::Left,
            cx.listener(move |this, _, _, _| {
                if let Some(viewer) = this.viewers.get_mut(&up_key) {
                    viewer.drag = None;
                }
            }),
        );
        element.child(self.measure(key, cx)).into_any_element()
    }

    fn picture(
        &mut self,
        hash: u64,
        data: Option<&wire::ImageData>,
        width: Option<wire::Length>,
        height: Option<wire::Length>,
        fit: Option<wire::ContentFit>,
        opacity: f32,
    ) -> AnyElement {
        if let Some(data) = data {
            self.remember_image(hash, data);
        }
        let mut element = dimensions(div(), width, height).opacity(opacity);
        if let Some(image) = self.images.get(&hash) {
            element = element.child(img(image.clone()).size_full().object_fit(object_fit(fit)));
        }
        element.into_any_element()
    }

    fn picker(
        &mut self,
        key: &str,
        options: &[String],
        selected: Option<u32>,
        handler: u32,
        placeholder: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let choices = || {
            options
                .iter()
                .enumerate()
                .map(|(index, label)| Choice {
                    index: index as u32,
                    label: label.clone(),
                })
                .collect::<Vec<_>>()
        };
        let index = selected.map(|index| IndexPath::new(index as usize));
        if !self.pickers.contains_key(key) {
            let state = cx.new(|cx| SelectState::new(choices(), index, window, cx));
            let route = key.to_owned();
            let subscription = cx.subscribe_in(&state, window, move |this, _, event, _, cx| {
                let SelectEvent::Confirm(Some(index)) = event else {
                    return;
                };
                let Some(picker) = this.pickers.get_mut(&route) else {
                    return;
                };
                picker.selected = Some(*index);
                cx.emit(wire::Event::Select {
                    handler: picker.handler,
                    index: *index,
                });
            });
            self.pickers.insert(
                key.into(),
                Picker {
                    state,
                    options: options.to_vec(),
                    selected,
                    handler,
                    _subscription: subscription,
                },
            );
        }
        let picker = self.pickers.get_mut(key).expect("picker inserted");
        picker.handler = handler;
        if picker.options != options {
            picker.options = options.to_vec();
            picker
                .state
                .update(cx, |state, cx| state.set_items(choices(), window, cx));
        }
        if picker.selected != selected {
            picker.selected = selected;
            picker
                .state
                .update(cx, |state, cx| state.set_selected_index(index, window, cx));
        }
        Select::new(&picker.state)
            .placeholder(placeholder.to_owned())
            .into_any_element()
    }
}

impl Render for ViewTree {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.node(&self.root.clone(), window, cx)
    }
}

fn rgba(color: wire::Rgba) -> Hsla {
    let [r, g, b, a] = color.0;
    gpui_kit::Rgba { r, g, b, a }.into()
}

fn dimensions<T: Styled>(
    mut element: T,
    width: Option<wire::Length>,
    height: Option<wire::Length>,
) -> T {
    element = match width {
        Some(wire::Length::Fixed(value)) => element.w(px(value)),
        Some(wire::Length::Fill) => element.w_full(),
        Some(wire::Length::FillPortion(_)) => element.flex_1(),
        Some(wire::Length::Shrink) | None => element,
    };
    match height {
        Some(wire::Length::Fixed(value)) => element.h(px(value)),
        Some(wire::Length::Fill) => element.h_full(),
        Some(wire::Length::FillPortion(_)) => element.flex_1(),
        Some(wire::Length::Shrink) | None => element,
    }
}

fn pad<T: Styled>(element: T, padding: Option<wire::Edges>) -> T {
    match padding {
        Some(edges) => element
            .pt(px(edges.top))
            .pr(px(edges.right))
            .pb(px(edges.bottom))
            .pl(px(edges.left)),
        None => element,
    }
}

fn decoration<T: Styled>(
    mut element: T,
    background: Option<wire::Rgba>,
    border: Option<wire::Border>,
) -> T {
    if let Some(color) = background {
        element = element.bg(rgba(color));
    }
    if let Some(border) = border {
        if let Some(color) = border.color {
            element = element.border_color(rgba(color));
        }
        if let Some(width) = border.width {
            element = element.border(px(width));
        }
        if let Some([tl, tr, br, bl]) = border.radius {
            element = element
                .rounded_tl(px(tl))
                .rounded_tr(px(tr))
                .rounded_br(px(br))
                .rounded_bl(px(bl));
        }
    }
    element
}

fn button_style(mut button: Button, style: &wire::ButtonStyle, disabled: bool, cx: &App) -> Button {
    button = match style.preset {
        wire::ButtonPreset::Primary => button.primary(),
        wire::ButtonPreset::Secondary => button.secondary(),
        wire::ButtonPreset::Success => button.success(),
        wire::ButtonPreset::Warning => button.warning(),
        wire::ButtonPreset::Danger => button.danger(),
        wire::ButtonPreset::Text => button.text(),
        wire::ButtonPreset::Background => button.secondary(),
        wire::ButtonPreset::Subtle => button.ghost(),
    };
    let base = style
        .recipe
        .as_ref()
        .map_or(wire::Face::default(), |recipe| recipe.base);
    let active = face_over(base, style.active);
    let face = match disabled {
        true => face_over(active, style.disabled.unwrap_or_default()),
        false => active,
    };
    let hover = face_over(face, style.hovered.unwrap_or_default());
    let pressed = face_over(hover, style.pressed.unwrap_or_default());
    let custom = style.recipe.is_some() || face.background.is_some() || face.text.is_some();
    if custom {
        let mut variant = ButtonCustomVariant::new(cx);
        if let Some(color) = face.background {
            variant = variant.color(rgba(color));
        }
        if let Some(color) = face.text {
            variant = variant.foreground(rgba(color));
        }
        if let Some(color) = hover.background {
            variant = variant.hover(rgba(color));
        }
        if let Some(color) = pressed.background {
            variant = variant.active(rgba(color));
        }
        if let Some(recipe) = &style.recipe {
            if let Some(color) = recipe.hover_background {
                variant = variant.hover(rgba(color));
            }
            if let Some(color) = recipe.pressed_background {
                variant = variant.active(rgba(color));
            }
            if let Some(size) = recipe.text_size {
                button = button.text_size(px(size));
            }
            if let Some(line_height) = recipe.line_height {
                button = button.line_height(relative(line_height));
            }
            if let Some(font) = &recipe.font {
                button = button.font_weight(font_weight(font.weight));
            }
            if disabled {
                if let Some(color) = recipe.disabled_background {
                    variant = variant.color(rgba(color));
                }
                if let Some(color) = recipe.disabled_text {
                    variant = variant.foreground(rgba(color));
                }
                if let Some(opacity) = recipe.disabled_opacity {
                    button = button.opacity(opacity);
                }
            }
        }
        button = button.custom(variant);
    }
    decoration(button, None, face.border)
}

fn face_over(base: wire::Face, next: wire::Face) -> wire::Face {
    wire::Face {
        background: next.background.or(base.background),
        text: next.text.or(base.text),
        border: next.border.or(base.border),
    }
}

fn font_weight(weight: wire::Weight) -> FontWeight {
    match weight {
        wire::Weight::Thin => FontWeight::THIN,
        wire::Weight::ExtraLight => FontWeight::EXTRA_LIGHT,
        wire::Weight::Light => FontWeight::LIGHT,
        wire::Weight::Normal => FontWeight::NORMAL,
        wire::Weight::Medium => FontWeight::MEDIUM,
        wire::Weight::Semibold => FontWeight::SEMIBOLD,
        wire::Weight::Bold => FontWeight::BOLD,
        wire::Weight::ExtraBold => FontWeight::EXTRA_BOLD,
        wire::Weight::Black => FontWeight::BLACK,
    }
}

fn text_options(
    mut element: Div,
    font: wire::Font,
    align: Option<wire::AlignX>,
    options: &wire::TextOptions,
) -> Div {
    element = element.font_weight(font_weight(font.weight));
    if font.monospace {
        element = element.font_family("monospace");
    }
    if let Some(font) = &options.font {
        let family = match &font.family {
            wire::FontFamily::Named(name) => name.clone(),
            wire::FontFamily::Serif => "serif".into(),
            wire::FontFamily::SansSerif => "sans-serif".into(),
            wire::FontFamily::Monospace => "monospace".into(),
            wire::FontFamily::Cursive => "cursive".into(),
            wire::FontFamily::Fantasy => "fantasy".into(),
        };
        element = element
            .font_family(family)
            .font_weight(font_weight(font.weight));
        if font.style != wire::FontStyle::Normal {
            element = element.italic();
        }
    }
    element = match align {
        Some(wire::AlignX::Center) => element.text_center(),
        Some(wire::AlignX::Right) => element.text_right(),
        _ => element,
    };
    if let Some(height) = options.line_height {
        element = match height {
            wire::LineHeight::Relative(value) => element.line_height(relative(value)),
            wire::LineHeight::Absolute(value) => element.line_height(px(value)),
        };
    }
    if options.wrapping == Some(wire::Wrapping::None) {
        element = element.whitespace_nowrap();
    }
    element
}

fn horizontal_align(element: Div, alignment: Option<wire::AlignX>) -> Div {
    match alignment {
        Some(wire::AlignX::Left) => element.justify_start(),
        Some(wire::AlignX::Center) => element.justify_center(),
        Some(wire::AlignX::Right) => element.justify_end(),
        None => element,
    }
}

fn vertical_align(element: Div, alignment: Option<wire::AlignY>) -> Div {
    match alignment {
        Some(wire::AlignY::Top) => element.items_start(),
        Some(wire::AlignY::Center) => element.items_center(),
        Some(wire::AlignY::Bottom) => element.items_end(),
        None => element,
    }
}

fn cross_align(element: Div, alignment: Option<wire::AlignX>) -> Div {
    match alignment {
        Some(wire::AlignX::Left) => element.items_start(),
        Some(wire::AlignX::Center) => element.items_center(),
        Some(wire::AlignX::Right) => element.items_end(),
        None => element,
    }
}

fn justify(element: Div, alignment: wire::FlexContentAlignment) -> Div {
    match alignment {
        wire::FlexContentAlignment::Start | wire::FlexContentAlignment::FlexStart => {
            element.justify_start()
        }
        wire::FlexContentAlignment::End | wire::FlexContentAlignment::FlexEnd => {
            element.justify_end()
        }
        wire::FlexContentAlignment::Center => element.justify_center(),
        wire::FlexContentAlignment::SpaceBetween => element.justify_between(),
        wire::FlexContentAlignment::SpaceAround => element.justify_around(),
        wire::FlexContentAlignment::SpaceEvenly => element.justify_evenly(),
        wire::FlexContentAlignment::Stretch => element,
    }
}

fn align_items(element: Div, alignment: wire::FlexItemAlignment) -> Div {
    match alignment {
        wire::FlexItemAlignment::Start | wire::FlexItemAlignment::FlexStart => {
            element.items_start()
        }
        wire::FlexItemAlignment::End | wire::FlexItemAlignment::FlexEnd => element.items_end(),
        wire::FlexItemAlignment::Center => element.items_center(),
        wire::FlexItemAlignment::Baseline => element.items_baseline(),
        wire::FlexItemAlignment::Stretch => element.items_stretch(),
    }
}

fn shadows(element: Div, shadow: wire::Shadow) -> Div {
    let Some(color) = shadow.color else {
        return element;
    };
    element.shadow(vec![BoxShadow {
        color: rgba(color),
        offset: point(
            px(shadow.x.unwrap_or_default()),
            px(shadow.y.unwrap_or_default()),
        ),
        blur_radius: px(shadow.blur.unwrap_or_default()),
        spread_radius: px(0.0),
        inset: false,
    }])
}

fn object_fit(fit: Option<wire::ContentFit>) -> ObjectFit {
    match fit {
        Some(wire::ContentFit::Cover) => ObjectFit::Cover,
        Some(wire::ContentFit::Fill) => ObjectFit::Fill,
        Some(wire::ContentFit::None) => ObjectFit::None,
        Some(wire::ContentFit::ScaleDown) => ObjectFit::ScaleDown,
        Some(wire::ContentFit::Contain) | None => ObjectFit::Contain,
    }
}

fn decode_image(data: &wire::ImageData) -> Option<RenderImage> {
    let mut pixels = match data {
        wire::ImageData::Rgba {
            width,
            height,
            pixels,
        } => image::RgbaImage::from_raw(*width, *height, pixels.clone())?,
        wire::ImageData::Encoded(bytes) => {
            let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
                .with_guessed_format()
                .ok()?;
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(8192);
            limits.max_image_height = Some(8192);
            limits.max_alloc = Some(64 << 20);
            reader.limits(limits);
            reader.decode().ok()?.into_rgba8()
        }
    };
    for pixel in pixels.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Some(RenderImage::new(vec![image::Frame::new(pixels)]))
}

fn qr(code: &wire::Qr) -> AnyElement {
    let Some(payload) = &code.payload else {
        return div().into_any_element();
    };
    let correction = match code.correction {
        Some(wire::QrCorrection::Low) => qrcode::EcLevel::L,
        Some(wire::QrCorrection::Quartile) => qrcode::EcLevel::Q,
        Some(wire::QrCorrection::High) => qrcode::EcLevel::H,
        Some(wire::QrCorrection::Medium) | None => qrcode::EcLevel::M,
    };
    let result = match code.version {
        Some(wire::QrVersion::Normal(value)) => qrcode::QrCode::with_version(
            payload,
            qrcode::Version::Normal(i16::from(value)),
            correction,
        ),
        Some(wire::QrVersion::Micro(value)) => qrcode::QrCode::with_version(
            payload,
            qrcode::Version::Micro(i16::from(value)),
            correction,
        ),
        None => qrcode::QrCode::with_error_correction_level(payload, correction),
    };
    let Ok(matrix) = result else {
        return div()
            .child("QR payload exceeds the selected code capacity")
            .into_any_element();
    };
    let modules = matrix.width() + 8;
    let total = match code.size {
        Some(wire::QrSize::Total(size)) => size,
        Some(wire::QrSize::Cell(size)) => size * modules as f32,
        None => 4.0 * modules as f32,
    };
    let cell = code.cell.map(rgba).unwrap_or_else(|| rgb(0).into());
    let background = code
        .background
        .map(rgba)
        .unwrap_or_else(|| rgb(0xffffff).into());
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            window.paint_quad(fill(bounds, background));
            let unit = f32::from(bounds.size.width.min(bounds.size.height)) / modules as f32;
            for y in 0..matrix.width() {
                for x in 0..matrix.width() {
                    if matrix[(x, y)] == qrcode::Color::Dark {
                        let origin = bounds.origin
                            + point(px((x + 4) as f32 * unit), px((y + 4) as f32 * unit));
                        window
                            .paint_quad(fill(Bounds::new(origin, size(px(unit), px(unit))), cell));
                    }
                }
            }
        },
    )
    .w(px(total))
    .h(px(total))
    .into_any_element()
}

fn svg_color(color: wire::Rgba) -> String {
    let [r, g, b, a] = color.0;
    format!(
        "rgba({},{},{},{a})",
        (r * 255.0) as u8,
        (g * 255.0) as u8,
        (b * 255.0) as u8
    )
}

fn canvas_svg(commands: &[wire::CanvasCommand], width: f32, height: f32) -> Vec<u8> {
    use std::fmt::Write;
    let mut svg =
        format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\">");
    let mut depth = 0;
    for (index, command) in commands.iter().enumerate() {
        match command {
            wire::CanvasCommand::Push {
                translate,
                rotate,
                scale,
                clip,
            } => {
                let _ = write!(
                    svg,
                    "<g transform=\"translate({} {}) rotate({}) scale({} {})\">",
                    translate[0],
                    translate[1],
                    rotate.to_degrees(),
                    scale[0],
                    scale[1]
                );
                if let Some([x, y, w, h]) = clip {
                    let _ = write!(
                        svg,
                        "<defs><clipPath id=\"c{index}\"><rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\"/></clipPath></defs><g clip-path=\"url(#c{index})\">"
                    );
                } else {
                    svg.push_str("<g>");
                }
                depth += 1;
            }
            wire::CanvasCommand::Pop => {
                if depth > 0 {
                    svg.push_str("</g></g>");
                    depth -= 1;
                }
            }
            wire::CanvasCommand::Draw {
                shape,
                fill,
                even_odd,
                stroke,
            } => {
                let path = canvas_path(shape);
                let color = fill.map(svg_color).unwrap_or_else(|| "none".into());
                let rule = match even_odd {
                    true => "evenodd",
                    false => "nonzero",
                };
                let _ = write!(
                    svg,
                    "<path d=\"{path}\" fill=\"{color}\" fill-rule=\"{rule}\""
                );
                if let Some(stroke) = stroke {
                    let cap = match stroke.cap {
                        wire::CanvasLineCap::Butt => "butt",
                        wire::CanvasLineCap::Square => "square",
                        wire::CanvasLineCap::Round => "round",
                    };
                    let join = match stroke.join {
                        wire::CanvasLineJoin::Miter => "miter",
                        wire::CanvasLineJoin::Round => "round",
                        wire::CanvasLineJoin::Bevel => "bevel",
                    };
                    let dash = stroke
                        .dash
                        .iter()
                        .map(f32::to_string)
                        .collect::<Vec<_>>()
                        .join(" ");
                    let _ = write!(
                        svg,
                        " stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"{cap}\" stroke-linejoin=\"{join}\" stroke-dasharray=\"{dash}\" stroke-dashoffset=\"{}\"",
                        svg_color(stroke.color),
                        stroke.width,
                        stroke.dash_offset
                    );
                }
                svg.push_str("/>");
            }
        }
    }
    for _ in 0..depth {
        svg.push_str("</g></g>");
    }
    svg.push_str("</svg>");
    svg.into_bytes()
}

fn canvas_path(shape: &wire::CanvasShape) -> String {
    let segments = match shape {
        wire::CanvasShape::Rectangle {
            position,
            size,
            radius,
        } => vec![wire::CanvasSegment::Rectangle {
            position: *position,
            size: *size,
            radius: *radius,
        }],
        wire::CanvasShape::Circle { center, radius } => vec![wire::CanvasSegment::Circle {
            center: *center,
            radius: *radius,
        }],
        wire::CanvasShape::Line { from, to } => vec![
            wire::CanvasSegment::Move(*from),
            wire::CanvasSegment::Line(*to),
        ],
        wire::CanvasShape::Path(segments) => segments.clone(),
    };
    let mut path = String::new();
    let mut cursor = [0.0, 0.0];
    let mut origin = cursor;
    use std::fmt::Write;
    for segment in segments {
        match segment {
            wire::CanvasSegment::Move([x, y]) => {
                let _ = write!(path, "M{x} {y} ");
                cursor = [x, y];
                origin = cursor;
            }
            wire::CanvasSegment::Line([x, y]) => {
                let _ = write!(path, "L{x} {y} ");
                cursor = [x, y];
            }
            wire::CanvasSegment::Close => {
                path.push_str("Z ");
                cursor = origin;
            }
            wire::CanvasSegment::Rectangle {
                position: [x, y],
                size: [w, h],
                radius: [tl, tr, br, bl],
            } => {
                let _ = write!(
                    path,
                    "M{} {y} H{} Q{} {y} {} {} V{} Q{} {} {} {} H{} Q{x} {} {x} {} V{} Q{x} {y} {} {y} Z ",
                    x + tl,
                    x + w - tr,
                    x + w,
                    x + w,
                    y + tr,
                    y + h - br,
                    x + w,
                    y + h,
                    x + w - br,
                    y + h,
                    x + bl,
                    y + h,
                    y + h - bl,
                    y + tl,
                    x + tl
                );
            }
            wire::CanvasSegment::Circle {
                center: [x, y],
                radius,
            } => {
                let _ = write!(
                    path,
                    "M{} {y} a{radius} {radius} 0 1 0 {} 0 a{radius} {radius} 0 1 0 {} 0 Z ",
                    x - radius,
                    radius * 2.0,
                    -radius * 2.0
                );
            }
            wire::CanvasSegment::Bezier { a, b, end } => {
                let _ = write!(
                    path,
                    "C{} {} {} {} {} {} ",
                    a[0], a[1], b[0], b[1], end[0], end[1]
                );
                cursor = end;
            }
            wire::CanvasSegment::Quadratic { control, end } => {
                let _ = write!(
                    path,
                    "Q{} {} {} {} ",
                    control[0], control[1], end[0], end[1]
                );
                cursor = end;
            }
            wire::CanvasSegment::Arc {
                center,
                radius,
                start,
                end,
            } => {
                append_arc(&mut path, center, [radius, radius], 0.0, start, end);
                cursor = [
                    center[0] + radius * end.cos(),
                    center[1] + radius * end.sin(),
                ];
            }
            wire::CanvasSegment::Ellipse {
                center,
                radius,
                rotation,
                start,
                end,
            } => {
                append_arc(&mut path, center, radius, rotation, start, end);
                cursor = [
                    center[0] + radius[0] * end.cos() * rotation.cos()
                        - radius[1] * end.sin() * rotation.sin(),
                    center[1]
                        + radius[0] * end.cos() * rotation.sin()
                        + radius[1] * end.sin() * rotation.cos(),
                ];
            }
            wire::CanvasSegment::ArcTo { a, b, radius } => {
                cursor = append_arc_to(&mut path, cursor, a, b, radius);
            }
        }
    }
    path
}

fn append_arc(
    path: &mut String,
    center: [f32; 2],
    radius: [f32; 2],
    rotation: f32,
    start: f32,
    end: f32,
) {
    use std::fmt::Write;
    let point = |angle: f32| {
        let x = radius[0] * angle.cos();
        let y = radius[1] * angle.sin();
        [
            center[0] + x * rotation.cos() - y * rotation.sin(),
            center[1] + x * rotation.sin() + y * rotation.cos(),
        ]
    };
    let a = point(start);
    let sweep = i32::from(end >= start);
    let _ = write!(path, "M{} {} ", a[0], a[1]);
    let steps = ((end - start).abs() / std::f32::consts::PI)
        .ceil()
        .clamp(1.0, 128.0) as usize;
    for index in 1..=steps {
        let b = point(start + (end - start) * index as f32 / steps as f32);
        let _ = write!(
            path,
            "A{} {} {} 0 {sweep} {} {} ",
            radius[0],
            radius[1],
            rotation.to_degrees(),
            b[0],
            b[1]
        );
    }
}

fn append_arc_to(
    path: &mut String,
    current: [f32; 2],
    a: [f32; 2],
    b: [f32; 2],
    radius: f32,
) -> [f32; 2] {
    use std::fmt::Write;
    let from = [current[0] - a[0], current[1] - a[1]];
    let to = [b[0] - a[0], b[1] - a[1]];
    let first = from[0].hypot(from[1]);
    let second = to[0].hypot(to[1]);
    let cross = from[0] * to[1] - from[1] * to[0];
    let degenerate = radius <= 0.0
        || first < f32::EPSILON
        || second < f32::EPSILON
        || cross.abs() < f32::EPSILON;
    if degenerate {
        let _ = write!(path, "L{} {} ", a[0], a[1]);
        return a;
    }
    let from = [from[0] / first, from[1] / first];
    let to = [to[0] / second, to[1] / second];
    let angle = (from[0] * to[0] + from[1] * to[1]).clamp(-1.0, 1.0).acos();
    let distance = radius / (angle / 2.0).tan();
    let enter = [a[0] + from[0] * distance, a[1] + from[1] * distance];
    let exit = [a[0] + to[0] * distance, a[1] + to[1] * distance];
    let sweep = i32::from(cross < 0.0);
    let _ = write!(
        path,
        "L{} {} A{radius} {radius} 0 0 {sweep} {} {} ",
        enter[0], enter[1], exit[0], exit[1]
    );
    exit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_and_pixels_keep_the_wire_meaning() {
        let mut path = String::new();
        let end = append_arc_to(&mut path, [0.0, 0.0], [10.0, 0.0], [10.0, 10.0], 2.0);
        assert!((end[0] - 10.0).abs() < 0.001 && (end[1] - 2.0).abs() < 0.001);
        assert!(path.contains("A2 2"));
        path.clear();
        append_arc(
            &mut path,
            [10.0, 10.0],
            [5.0, 5.0],
            0.0,
            0.0,
            std::f32::consts::TAU,
        );
        assert_eq!(path.matches('A').count(), 2);
        let pixels = wire::ImageData::Rgba {
            width: 1,
            height: 1,
            pixels: vec![255, 10, 20, 255],
        };
        let image = decode_image(&pixels).expect("one valid pixel");
        assert_eq!(image.as_bytes(0), Some([20, 10, 255, 255].as_slice()));
        assert!(decode_image(&wire::ImageData::Encoded(vec![1, 2, 3])).is_none());
    }
}
