//! GPUI rendering of the existing WASM tree. Widget identities retain native
//! input state; interaction uses the same semantic events the guests consume.

use std::collections::HashMap;
use gpui_kit::*;
use gpui_kit::component::{Disableable, button::Button, checkbox::Checkbox, input::{Input, InputEvent, InputState}};
use ui_lang_wire as wire;
use gpui_kit::component::{IndexPath, select::{Select, SelectEvent, SelectState}, searchable_list::SearchableListItem};
use gpui_kit::component::radio::Radio;

#[derive(Clone)]
struct Choice { index: u32, label: String }

impl SearchableListItem for Choice {
    type Value = u32;
    fn title(&self) -> SharedString { self.label.clone().into() }
    fn value(&self) -> &u32 { &self.index }
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
    _subscription: Subscription,
}

pub struct ViewTree {
    root: wire::Node,
    fields: HashMap<String, Field>,
    scrolls: HashMap<String, ScrollHandle>,
    pickers: HashMap<String, Picker>,
    drags: HashMap<String, Point<Pixels>>,
    containers: HashMap<String, [f64; 2]>,
}

impl EventEmitter<wire::Event> for ViewTree {}

impl ViewTree {
    pub fn new(root: wire::Node) -> Self {
        Self { root, fields: HashMap::new(), scrolls: HashMap::new(), pickers: HashMap::new(), drags: HashMap::new(), containers: HashMap::new() }
    }

    pub fn replace(&mut self, mut root: wire::Node, cx: &mut Context<Self>) {
        let mut inputs = std::collections::HashSet::new();
        let mut scrolls = std::collections::HashSet::new();
        let mut pickers = std::collections::HashSet::new();
        let mut drags = std::collections::HashSet::new();
        let mut containers = std::collections::HashSet::new();
        root.for_each_mut(&mut |node| match node {
            wire::Node::Input { key, .. } => { inputs.insert(key.clone()); }
            wire::Node::Scroll { key, .. } => { scrolls.insert(key.clone()); }
            wire::Node::PickList { key, .. } | wire::Node::ComboBox { key, .. } => { pickers.insert(key.clone()); }
            wire::Node::ResizeHandle { key, .. } => { drags.insert(key.clone()); }
            wire::Node::Responsive { key, .. } => { containers.insert(key.clone()); }
            _ => {}
        });
        self.fields.retain(|key, _| inputs.contains(key));
        self.scrolls.retain(|key, _| scrolls.contains(key));
        self.pickers.retain(|key, _| pickers.contains(key));
        self.drags.retain(|key, _| drags.contains(key));
        self.containers.retain(|key, _| containers.contains(key));
        self.root = root;
        cx.notify();
    }

    fn input(&mut self, node: &wire::Node, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let wire::Node::Input { key, value, placeholder, secure, on_input, on_submit, options, .. } = node else { unreachable!() };
        if !self.fields.contains_key(key) {
            let state = cx.new(|cx| {
                let mut state = InputState::new(window, cx).placeholder(placeholder.clone()).masked(*secure);
                state.set_value(value.clone(), window, cx);
                state
            });
            let input_key = key.clone();
            let subscription = cx.subscribe_in(&state, window, move |this, input, event, _, cx| {
                let Some(field) = this.fields.get_mut(&input_key) else { return; };
                match event {
                    InputEvent::Change => {
                        let text = input.read(cx).value().to_string();
                        if text == field.value { return; }
                        field.value = text.clone();
                        cx.emit(wire::Event::Input { handler: field.on_input, text });
                    }
                    InputEvent::PressEnter { .. } => {
                        if let Some(message) = field.on_submit { cx.emit(wire::Event::Message(message)); }
                    }
                    InputEvent::Focus | InputEvent::Blur => {}
                }
            });
            self.fields.insert(key.clone(), Field { state, on_input: *on_input, on_submit: *on_submit,
                value: value.clone(), _subscription: subscription });
        }
        let field = self.fields.get_mut(key).expect("field inserted");
        field.on_input = *on_input;
        field.on_submit = *on_submit;
        if field.value != *value {
            field.value = value.clone();
            field.state.update(cx, |state, cx| state.set_value(value.clone(), window, cx));
        }
        Input::new(&field.state).id(key.clone()).aria_label(options.label.clone())
            .disabled(options.disabled).into_any_element()
    }

    fn node(&mut self, node: &wire::Node, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        use wire::Node;
        match node {
            Node::Text { content, size, color, width, .. } => {
                let mut element = dimensions(div(), *width, None).child(content.clone());
                if let Some(size) = size { element = element.text_size(px(*size)); }
                if let Some(color) = color { element = element.text_color(rgba(*color)); }
                element.into_any_element()
            }
            Node::Space { width, height } => dimensions(div(), *width, *height).into_any_element(),
            Node::Linear { axis, spacing, padding, width, height, background, border, children, .. } => {
                let mut element = dimensions(div().flex(), *width, *height);
                element = match axis { wire::Axis::Column => element.flex_col(), wire::Axis::Row => element.flex_row() };
                if let Some(gap) = spacing { element = element.gap(px(*gap)); }
                element = decoration(pad(element, *padding), *background, *border);
                for child in children { element = element.child(self.node(child, window, cx)); }
                element.into_any_element()
            }
            Node::KeyedColumn { spacing, padding, width, height, background, border, children, .. } => {
                let mut element = decoration(pad(dimensions(div().flex().flex_col(), *width, *height), *padding), *background, *border);
                if let Some(gap) = spacing { element = element.gap(px(*gap)); }
                for child in children { element = element.child(self.node(child, window, cx)); }
                element.into_any_element()
            }
            Node::Container { content, width, height, padding, border, background, max_width, max_height, clip, .. } => {
                let color = match background { Some(wire::Background::Color(color)) => Some(*color), _ => None };
                let mut element = decoration(pad(dimensions(div(), *width, *height), *padding), color, *border);
                if let Some(width) = max_width { element = element.max_w(px(*width)); }
                if let Some(height) = max_height { element = element.max_h(px(*height)); }
                if *clip { element = element.overflow_hidden(); }
                element.child(self.node(content, window, cx)).into_any_element()
            }
            Node::Scroll { key, content, width, height, direction, .. } => {
                let handle = self.scrolls.entry(key.clone()).or_default().clone();
                let element = dimensions(div(), *width, *height).id(key.clone()).track_scroll(&handle);
                let element = match direction {
                    wire::ScrollDirection::Vertical => element.overflow_y_scroll(),
                    wire::ScrollDirection::Horizontal => element.overflow_x_scroll(),
                    wire::ScrollDirection::Both => element.overflow_scroll(),
                };
                element.child(self.node(content, window, cx)).into_any_element()
            }
            Node::Button { key, content, label, on_press, .. } => {
                let mut button = Button::new(key.clone()).disabled(on_press.is_none());
                button = match content {
                    wire::ButtonContent::Label(text) => button.label(text.clone()),
                    wire::ButtonContent::Child(child) => button.child(self.node(child, window, cx)),
                };
                if let Some(label) = label { button = button.aria_label(label.clone()); }
                if let Some(message) = on_press {
                    let message = *message;
                    button = button.on_click(cx.listener(move |_, _, _, cx| cx.emit(wire::Event::Message(message))));
                }
                button.into_any_element()
            }
            Node::Input { .. } => self.input(node, window, cx),
            Node::PickList { key, options, selected, on_select, placeholder, .. } =>
                self.picker(key, options, *selected, *on_select, placeholder.as_deref().unwrap_or_default(), window, cx),
            Node::ComboBox { key, options, selected, on_select, placeholder, .. } =>
                self.picker(key, options, *selected, *on_select, placeholder, window, cx),
            Node::Toggle { key, label, checked, on_toggle, .. } => {
                let mut checkbox = Checkbox::new(key.clone()).label(label.clone()).checked(*checked).disabled(on_toggle.is_none());
                if let Some(handler) = on_toggle {
                    let handler = *handler;
                    checkbox = checkbox.on_click(cx.listener(move |_, on, _, cx| cx.emit(wire::Event::Toggle { handler, on: *on })));
                }
                checkbox.into_any_element()
            }
            Node::Radio { key, label, selected, on_select, .. } => {
                let message = *on_select;
                Radio::new(key.clone()).label(label.clone()).checked(*selected)
                    .on_click(cx.listener(move |_, _, _, cx| cx.emit(wire::Event::Message(message)))).into_any_element()
            }
            Node::Rule { axis, thickness, color, .. } => {
                let element = div().bg(color.map(rgba).unwrap_or_else(|| gpui_kit::rgb(0xdad9d3).into()));
                match axis {
                    wire::Axis::Column => element.w(px(*thickness)).h_full().into_any_element(),
                    wire::Axis::Row => element.h(px(*thickness)).w_full().into_any_element(),
                }
            }
            Node::Lazy { content, .. } => self.node(content, window, cx),
            Node::ResizeHandle { key, on_press, on_release, on_drag, content, .. } => {
                let press_key = key.clone();
                let move_key = key.clone();
                let release_key = key.clone();
                let press = *on_press;
                let release = *on_release;
                let drag = *on_drag;
                div().id(key.clone())
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        this.drags.insert(press_key.clone(), event.position);
                        if let Some(message) = press { cx.emit(wire::Event::Message(message)); }
                    }))
                    .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                        let Some(previous) = this.drags.get_mut(&move_key) else { return; };
                        if event.pressed_button != Some(MouseButton::Left) {
                            this.drags.remove(&move_key);
                            return;
                        }
                        let delta = event.position - *previous;
                        *previous = event.position;
                        if let Some(handler) = drag { cx.emit(wire::Event::Drag { handler, dx: f32::from(delta.x) as f64, dy: f32::from(delta.y) as f64 }); }
                    }))
                    .on_mouse_up(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                        let was_dragging = this.drags.remove(&release_key).is_some();
                        if was_dragging {
                            if let Some(message) = release { cx.emit(wire::Event::Message(message)); }
                        }
                    }))
                    .child(self.node(content, window, cx)).into_any_element()
            }
            Node::Responsive { key, content, width, height } => {
                let weak = cx.entity().downgrade();
                let key = key.clone();
                let measure = canvas(move |bounds, _, cx| {
                    let size = [f32::from(bounds.size.width) as f64, f32::from(bounds.size.height) as f64];
                    let _ = weak.update(cx, |this, cx| {
                        let changed = this.containers.get(&key) != Some(&size);
                        if changed { this.containers.insert(key, size); cx.notify(); }
                    });
                }, |_, _, _, _| {}).absolute().inset_0();
                dimensions(div().relative(), *width, *height)
                    .child(self.node(content, window, cx)).child(measure).into_any_element()
            }
            Node::When { condition, children, .. } => {
                let mut element = div().flex().flex_col();
                if condition.matches(&self.containers) {
                    for child in children { element = element.child(self.node(child, window, cx)); }
                }
                element.into_any_element()
            }
            Node::Stack { children, width, height, padding, background, border, clip, .. } => {
                let mut element = decoration(pad(dimensions(div().relative(), *width, *height), *padding), *background, *border);
                if *clip { element = element.overflow_hidden(); }
                for (index, child) in children.iter().enumerate() {
                    let content = self.node(child, window, cx);
                    element = match index {
                        0 => element.child(content),
                        _ => element.child(div().absolute().inset_0().child(content)),
                    };
                }
                element.into_any_element()
            }
            Node::Overlay { key, children, backdrop, padding, align_x, align_y, on_dismiss } => {
                let mut element = div().relative().size_full();
                if let Some(base) = children.first() { element = element.child(self.node(base, window, cx)); }
                if let Some(modal) = children.get(1) {
                    let mut shade = div().id(format!("{key}/backdrop")).absolute().inset_0().bg(rgba(*backdrop));
                    if let Some(message) = on_dismiss {
                        let message = *message;
                        shade = shade.on_mouse_down(MouseButton::Left, cx.listener(move |_, _, _, cx| cx.emit(wire::Event::Message(message))));
                    }
                    let mut layer = div().absolute().inset_0().flex().p(px(*padding));
                    layer = match align_x { wire::AlignX::Left => layer.justify_start(), wire::AlignX::Center => layer.justify_center(), wire::AlignX::Right => layer.justify_end() };
                    layer = match align_y { wire::AlignY::Top => layer.items_start(), wire::AlignY::Center => layer.items_center(), wire::AlignY::Bottom => layer.items_end() };
                    element = element.child(shade).child(layer.child(self.node(modal, window, cx)));
                }
                element.into_any_element()
            }
            Node::Progress { value, min, max, axis, length, girth, background, bar, border, .. } => {
                let span = max - min;
                let valid = span.is_finite() && span > 0.0;
                let fraction = match valid { true => ((value - min) / span).clamp(0.0, 1.0), false => 0.0 };
                let fill = div().bg(bar.map(rgba).unwrap_or_else(|| gpui_kit::rgb(0xa05a3c).into()));
                match axis {
                    wire::Axis::Row => decoration(dimensions(div(), *length, *girth), *background, *border)
                        .child(fill.w(relative(fraction)).h_full()).into_any_element(),
                    wire::Axis::Column => decoration(dimensions(div().flex().flex_col().justify_end(), *girth, *length), *background, *border)
                        .child(fill.h(relative(fraction)).w_full()).into_any_element(),
                }
            }
            Node::Pin { content, x, y, width, height, .. } => dimensions(div().absolute().left(px(*x)).top(px(*y)), *width, *height)
                .child(self.node(content, window, cx)).into_any_element(),
            // Unported controls remain explicit during migration. This arm is
            // removed as each concrete wire variant gets its native element.
            other => div().text_color(gpui_kit::rgb(0xb42318)).child(format!("GPUI control pending: {}", node_kind(other))).into_any_element(),
        }
    }

    fn picker(&mut self, key: &str, options: &[String], selected: Option<u32>, handler: u32, placeholder: &str, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let choices = || options.iter().enumerate().map(|(index, label)| Choice { index: index as u32, label: label.clone() }).collect::<Vec<_>>();
        let index = selected.map(|index| IndexPath::new(index as usize));
        if !self.pickers.contains_key(key) {
            let state = cx.new(|cx| SelectState::new(choices(), index, window, cx));
            let route = key.to_owned();
            let subscription = cx.subscribe_in(&state, window, move |this, _, event, _, cx| {
                let SelectEvent::Confirm(Some(index)) = event else { return; };
                let Some(picker) = this.pickers.get_mut(&route) else { return; };
                picker.selected = Some(*index);
                cx.emit(wire::Event::Select { handler: picker.handler, index: *index });
            });
            self.pickers.insert(key.into(), Picker { state, options: options.to_vec(), selected, handler, _subscription: subscription });
        }
        let picker = self.pickers.get_mut(key).expect("picker inserted");
        picker.handler = handler;
        if picker.options != options {
            picker.options = options.to_vec();
            picker.state.update(cx, |state, cx| state.set_items(choices(), window, cx));
        }
        if picker.selected != selected {
            picker.selected = selected;
            picker.state.update(cx, |state, cx| state.set_selected_index(index, window, cx));
        }
        Select::new(&picker.state).placeholder(placeholder.to_owned()).into_any_element()
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

fn dimensions(mut element: Div, width: Option<wire::Length>, height: Option<wire::Length>) -> Div {
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

fn pad(element: Div, padding: Option<wire::Edges>) -> Div {
    match padding {
        Some(edges) => element.pt(px(edges.top)).pr(px(edges.right)).pb(px(edges.bottom)).pl(px(edges.left)),
        None => element,
    }
}

fn decoration(mut element: Div, background: Option<wire::Rgba>, border: Option<wire::Border>) -> Div {
    if let Some(color) = background { element = element.bg(rgba(color)); }
    if let Some(border) = border {
        if let Some(color) = border.color { element = element.border_color(rgba(color)); }
        if let Some(width) = border.width { element = element.border(px(width)); }
        if let Some([tl, tr, br, bl]) = border.radius {
            element = element.rounded_tl(px(tl)).rounded_tr(px(tr)).rounded_br(px(br)).rounded_bl(px(bl));
        }
    }
    element
}

fn node_kind(node: &wire::Node) -> &'static str {
    use wire::Node;
    match node {
        Node::Qr { .. } => "qr", Node::RichText { .. } => "rich_text",
        Node::Flex { .. } => "flex", Node::Float { .. } => "float",
        Node::ResizeHandle { .. } => "resize_handle", Node::MouseArea { .. } => "mouse_area",
        Node::Tooltip { .. } => "tooltip", Node::Grid { .. } => "grid",
        Node::Responsive { .. } => "responsive", Node::When { .. } => "when",
        Node::Sensor { .. } => "sensor", Node::Image { .. } => "image",
        Node::ImageViewer { .. } => "image_viewer", Node::Svg { .. } => "svg",
        Node::Editor { .. } => "editor", Node::Slider { .. } => "slider",
        Node::ComboBox { .. } => "combo_box", Node::PickList { .. } => "pick_list",
        Node::Progress { .. } => "progress", _ => "surface",
    }
}
