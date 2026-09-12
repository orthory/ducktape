//! GPUI rendering of the existing WASM tree. Widget identities retain native
//! input state; interaction uses the same semantic events the guests consume.

use std::collections::HashMap;
use gpui_kit::*;
use gpui_kit::component::{Disableable, button::Button, checkbox::Checkbox, input::{Input, InputEvent, InputState}};
use ui_lang_wire as wire;

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
}

impl EventEmitter<wire::Event> for ViewTree {}

impl ViewTree {
    pub fn new(root: wire::Node) -> Self {
        Self { root, fields: HashMap::new(), scrolls: HashMap::new() }
    }

    pub fn replace(&mut self, mut root: wire::Node, cx: &mut Context<Self>) {
        let mut inputs = std::collections::HashSet::new();
        let mut scrolls = std::collections::HashSet::new();
        root.for_each_mut(&mut |node| match node {
            wire::Node::Input { key, .. } => { inputs.insert(key.clone()); }
            wire::Node::Scroll { key, .. } => { scrolls.insert(key.clone()); }
            _ => {}
        });
        self.fields.retain(|key, _| inputs.contains(key));
        self.scrolls.retain(|key, _| scrolls.contains(key));
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
                Checkbox::new(key.clone()).label(label.clone()).checked(*selected)
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
            Node::Pin { content, x, y, width, height, .. } => dimensions(div().absolute().left(px(*x)).top(px(*y)), *width, *height)
                .child(self.node(content, window, cx)).into_any_element(),
            // Unported controls remain explicit during migration. This arm is
            // removed as each concrete wire variant gets its native element.
            other => div().text_color(gpui_kit::rgb(0xb42318)).child(format!("GPUI control pending: {}", node_kind(other))).into_any_element(),
        }
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
