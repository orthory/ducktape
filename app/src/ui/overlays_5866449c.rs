#[allow(warnings, clippy::all)]
mod __ice_group_overlays_5866449c {
use super::*;
impl super::Ducktape {
pub(super) fn __ice_component_use_91(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 18, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 29, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __overlay_base: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 38, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(::iced::Fill).height(::iced::Fill).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __overlay_open = self.channel_create_open; let __overlay_base: __IceElement<'_, __DucktapeMessage> = if __overlay_open { ::ui_lang_runtime::focus_barrier(__overlay_base).into() } else { __overlay_base }; let __overlay_stack = ::iced::widget::Stack::new().width(::iced::Fill).height(::iced::Fill).push(__overlay_base); if __overlay_open { let __overlay_layer: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 40, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/channel-modal", __ice_use_scope); { let __ice_component_04d6f64616c5368656c6c_scope_6600 = __ice_node_scope.clone(); ::ui_lang_runtime::rev_memo(1021u64, (self.__ice_rev[19], self.__ice_rev[43], self.__ice_rev[45], self.__ice_rev[8], self.__ice_rev[9], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_87(__ice_palette, __ice_component_04d6f64616c5368656c6c_scope_6600, (move || __DucktapeMessage::ClosePalette).clone(), (move || __DucktapeMessage::CreateChannelSubmit).clone(), (move || __DucktapeMessage::DismissToast).clone(), (move |__event_0, __event_1| __DucktapeMessage::OpenChatSearchHit(__event_0, __event_1)).clone(), (move |__event_0, __event_1| __DucktapeMessage::OpenPageSearchHit(__event_0, __event_1)).clone(), (move |__event_0| __DucktapeMessage::PaletteChanged(__event_0)).clone(), (move || __DucktapeMessage::ToggleChannelCreate).clone(), (move || __DucktapeMessage::ToggleChannelCreateMembersOnly).clone()))).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __overlay_backdrop = ::iced::widget::container(::iced::widget::space()).width(::iced::Fill).height(::iced::Fill).style(move |_| ::iced::widget::container::Style { background: ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[85])), ..::iced::widget::container::Style::default() }); let __overlay_backdrop: __IceElement<'_, __DucktapeMessage> = ::iced::widget::mouse_area(__overlay_backdrop).on_press((move || __DucktapeMessage::ToggleChannelCreate)()).on_release(__DucktapeMessage::__ExternNoop).on_right_press(__DucktapeMessage::__ExternNoop).on_right_release(__DucktapeMessage::__ExternNoop).on_middle_press(__DucktapeMessage::__ExternNoop).on_middle_release(__DucktapeMessage::__ExternNoop).on_scroll(|_| __DucktapeMessage::__ExternNoop).into(); let __overlay_panel = ::iced::widget::mouse_area(__overlay_layer).on_press(__DucktapeMessage::__ExternNoop).on_release(__DucktapeMessage::__ExternNoop).on_right_press(__DucktapeMessage::__ExternNoop).on_right_release(__DucktapeMessage::__ExternNoop).on_middle_press(__DucktapeMessage::__ExternNoop).on_middle_release(__DucktapeMessage::__ExternNoop).on_scroll(|_| __DucktapeMessage::__ExternNoop); let __overlay_panel: __IceElement<'_, __DucktapeMessage> = ::iced::widget::container(__overlay_panel).width(::iced::Fill).height(::iced::Fill).padding(30.0 as f32).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center).into(); let __overlay_surface: __IceElement<'_, __DucktapeMessage> = ::iced::widget::Stack::new().width(::iced::Fill).height(::iced::Fill).push(__overlay_backdrop).push(__overlay_panel).into(); __overlay_stack.push(::iced::widget::float(__overlay_surface).translate(|_, _| ::iced::Vector::new(::core::f32::EPSILON, 0.0))).into() } else { __overlay_stack.into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!(self.toast).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 251, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:251", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 258, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:258", __ice_use_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (move || __DucktapeMessage::DismissToast)(); let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 263, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_toast_scope_6823 = format!("{}/Toast@6823", __ice_use_scope); ::ui_lang_runtime::rev_memo(1064u64, (self.__ice_rev[117], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_88(__ice_palette, __ice_component_toast_scope_6823))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).padding(0.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.text_color = __ice_palette.colors[4]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((10.0) as f32).max(0.0).min(f32::MAX), top_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((10.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.text_color = __ice_palette.colors[4]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Dismiss".to_owned()).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(0.0, 0.0, 26.0, 0.0)).width(::iced::Fill).height(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Bottom) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 274, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __overlay_base: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 283, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(::iced::Fill).height(::iced::Fill).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __overlay_open = self.palette_open; let __overlay_base: __IceElement<'_, __DucktapeMessage> = if __overlay_open { ::ui_lang_runtime::focus_barrier(__overlay_base).into() } else { __overlay_base }; let __overlay_stack = ::iced::widget::Stack::new().width(::iced::Fill).height(::iced::Fill).push(__overlay_base); if __overlay_open { let __overlay_layer: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 285, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:285", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 296, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 297, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/palette-input", __ice_use_scope); { let __a11y_key = __ice_node_scope.clone(); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __secure = false; let __role = if __secure { ::ui_lang_runtime::Role::PasswordInput } else { ::ui_lang_runtime::Role::TextInput }; let __input = ::ui_lang_runtime::accessible(::iced::widget::text_input("Search messages and pages… (Esc closes)", &self.palette_draft).id(::iced::widget::Id::from(__a11y_key.clone())).padding(::iced::Padding { top: 11.0, right: 13.0, bottom: 11.0, left: 13.0 }).width(::iced::Fill).secure(__secure).width(::iced::Fill).padding(8.0 as f32).size(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)).line_height(::iced::widget::text::LineHeight::Relative(((1.2) as f32).max(f32::EPSILON).min(f32::MAX))).on_input_maybe(if __disabled { None } else { Some({ let __route_callback = (move |__event_0| __DucktapeMessage::PaletteChanged(__event_0)).clone(); move |__value| (__route_callback)(__value) }) }).on_submit_maybe(if __disabled { None } else { Some((move || __DucktapeMessage::ClosePalette)()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::text_input::default(__theme, __status); __style.background = __ice_palette.colors[3].into(); __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.0; __style.border.radius = 10.0.into(); __style.background = ::iced::Background::Color(__ice_palette.colors[6]); __style.border.color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.160000; __color }; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; __style.placeholder = __ice_palette.colors[5]; __style.value = __ice_palette.colors[4]; __style.selection = { let mut __color = __ice_palette.colors[4]; __color.a = 0.180000; __color }; if matches!(__status, ::iced::widget::text_input::Status::Focused { .. }) { __style.border.color = __ice_palette.colors[42]; } match __status { ::iced::widget::text_input::Status::Hovered => { __style.background = ::iced::Background::Color(__ice_palette.colors[55]); __style.border.color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.210000; __color }; } _ => {} } __style }), __a11y_id, __role).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Search everything".to_owned()).value_maybe((!__secure).then(|| (self.palette_draft).to_owned())).disabled(__disabled); __input.into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (self.palette_search_phase == SearchPhase::Searching) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 311, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:311", __ice_use_scope); let __text_value = ("Searching…").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)).color(__ice_palette.colors[5]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (((self.palette_search_phase == SearchPhase::Done) && (self.palette_chat_hits).is_empty()) && (self.palette_page_hits).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 319, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_0456d707479506c617465_scope_6879 = format!("{}/EmptyPlate@6879", __ice_use_scope); ::ui_lang_runtime::rev_memo(1073u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_89(__ice_palette, __ice_component_0456d707479506c617465_scope_6879))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((self.palette_search_phase == SearchPhase::Idle) && (!((self.palette_draft).trim().to_owned()).is_empty())) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 333, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_0456d707479506c617465_scope_6893 = format!("{}/EmptyPlate@6893", __ice_use_scope); ::ui_lang_runtime::rev_memo(1075u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_90(__ice_palette, __ice_component_0456d707479506c617465_scope_6893))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!(self.palette_chat_hits).is_empty()) || (!(self.palette_page_hits).is_empty())) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 340, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:340", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 341, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@layout:341", __ice_use_scope); let __scroll_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 346, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if (!(self.palette_chat_hits).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 348, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:348", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 349, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:349", __ice_use_scope); let __text_value = ("MESSAGES").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[5]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(0.0, 0.0, 0.0, 4.0)).width(::iced::Fill) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 354, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); for (__ice_index, hit) in self.palette_chat_hits.iter().enumerate() { let __for_scope = format!("{}/@for:6915({})", __ice_use_scope, __ice_index); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 356, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:356", __for_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (move |__event_0, __event_1| __DucktapeMessage::OpenChatSearchHit(__event_0, __event_1))(hit.channel_id.to_owned(), hit.seq); let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 362, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 369, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:369", __for_scope); let __text_value = (hit.text).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::WordOrGlyph).color(__ice_palette.colors[4]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 374, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:374", __for_scope); let __text_value = (hit.meta).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[5]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(1.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:362", __for_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).padding(::iced::Padding { top: 7.0, right: 12.0, bottom: 7.0, left: 12.0 }).width(::iced::Fill).padding(6.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 8.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.text_color = __ice_palette.colors[4]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((8.0) as f32).max(0.0).min(f32::MAX), top_right: ((8.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((8.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((8.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[57])); __style.text_color = __ice_palette.colors[4]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[14])); } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Open message".to_owned()).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 8.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(1.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:354", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.palette_page_hits).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 384, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:384", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 385, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:385", __ice_use_scope); let __text_value = ("PAGES").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[5]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(0.0, 0.0, 0.0, 4.0)).width(::iced::Fill) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 390, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); for (__ice_index, hit) in self.palette_page_hits.iter().enumerate() { let __for_scope = format!("{}/@for:6951({})", __ice_use_scope, __ice_index); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 392, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:392", __for_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (move |__event_0, __event_1| __DucktapeMessage::OpenPageSearchHit(__event_0, __event_1))(hit.page_id.to_owned(), hit.block_id.to_owned()); let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 398, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 401, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:401", __for_scope); let __text_value = (hit.text).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::WordOrGlyph).color(__ice_palette.colors[4]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 412, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 417, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:417", __for_scope); let __text_value = (hit.page_title).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)).width(::iced::Fill).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[5]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 423, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:423", __for_scope); let __text_value = (hit.kind).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Normal, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[5]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(7.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:412", __for_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(1.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:398", __for_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).padding(::iced::Padding { top: 7.0, right: 12.0, bottom: 7.0, left: 12.0 }).width(::iced::Fill).padding(6.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 8.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.text_color = __ice_palette.colors[4]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((8.0) as f32).max(0.0).min(f32::MAX), top_right: ((8.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((8.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((8.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[57])); __style.text_color = __ice_palette.colors[4]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[14])); } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Open page".to_owned()).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 8.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(1.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:390", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(4.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:346", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __layout = ::iced::widget::scrollable(__scroll_content).direction(::iced::widget::scrollable::Direction::Vertical(::iced::widget::scrollable::Scrollbar::new())).anchor_x(::iced::widget::scrollable::Anchor::Start).anchor_y(::iced::widget::scrollable::Anchor::Start).width(::iced::Fill).height(::iced::Shrink); ::ui_lang_runtime::accessible(__layout, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).max_height(380.0 as f32) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(8.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:296", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(10.0, 10.0, 10.0, 10.0)).width(540.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[3])); __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((14.0) as f32).max(0.0).min(f32::MAX), top_right: ((14.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((14.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((14.0) as f32).max(0.0).min(f32::MAX) }; __style.shadow.color = __ice_palette.colors[48]; __style.shadow.offset.y = 24.0 as f32; __style.shadow.blur_radius = 60.0 as f32; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __overlay_backdrop = ::iced::widget::container(::iced::widget::space()).width(::iced::Fill).height(::iced::Fill).style(move |_| ::iced::widget::container::Style { background: ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[85])), ..::iced::widget::container::Style::default() }); let __overlay_backdrop: __IceElement<'_, __DucktapeMessage> = ::iced::widget::mouse_area(__overlay_backdrop).on_press((move || __DucktapeMessage::ClosePalette)()).on_release(__DucktapeMessage::__ExternNoop).on_right_press(__DucktapeMessage::__ExternNoop).on_right_release(__DucktapeMessage::__ExternNoop).on_middle_press(__DucktapeMessage::__ExternNoop).on_middle_release(__DucktapeMessage::__ExternNoop).on_scroll(|_| __DucktapeMessage::__ExternNoop).into(); let __overlay_panel = ::iced::widget::mouse_area(__overlay_layer).on_press(__DucktapeMessage::__ExternNoop).on_release(__DucktapeMessage::__ExternNoop).on_right_press(__DucktapeMessage::__ExternNoop).on_right_release(__DucktapeMessage::__ExternNoop).on_middle_press(__DucktapeMessage::__ExternNoop).on_middle_release(__DucktapeMessage::__ExternNoop).on_scroll(|_| __DucktapeMessage::__ExternNoop); let __overlay_panel: __IceElement<'_, __DucktapeMessage> = ::iced::widget::container(__overlay_panel).width(::iced::Fill).height(::iced::Fill).padding(72.0 as f32).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Top).into(); let __overlay_surface: __IceElement<'_, __DucktapeMessage> = ::iced::widget::Stack::new().width(::iced::Fill).height(::iced::Fill).push(__overlay_backdrop).push(__overlay_panel).into(); __overlay_stack.push(::iced::widget::float(__overlay_surface).translate(|_, _| ::iced::Vector::new(::core::f32::EPSILON, 0.0))).into() } else { __overlay_stack.into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __layout = ::ui_lang_runtime::zstack(__children).width(::iced::Fill).height(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:18", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_188(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 18, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 29, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __overlay_base: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 38, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(::iced::Fill).height(::iced::Fill).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __overlay_open = false; let __overlay_base: __IceElement<'_, __DucktapeMessage> = if __overlay_open { ::ui_lang_runtime::focus_barrier(__overlay_base).into() } else { __overlay_base }; let __overlay_stack = ::iced::widget::Stack::new().width(::iced::Fill).height(::iced::Fill).push(__overlay_base); if __overlay_open { let __overlay_layer: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 40, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/channel-modal", __ice_use_scope); { let __ice_component_04d6f64616c5368656c6c_scope_6600 = __ice_node_scope.clone(); ::ui_lang_runtime::rev_memo(1021u64, (self.__ice_rev[43], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_184(__ice_palette, __ice_component_04d6f64616c5368656c6c_scope_6600, (move || __DucktapeMessage::ClosePalette).clone(), (move || __DucktapeMessage::CreateChannelSubmit).clone(), (move || __DucktapeMessage::DismissToast).clone(), (move |__event_0, __event_1| __DucktapeMessage::OpenChatSearchHit(__event_0, __event_1)).clone(), (move |__event_0, __event_1| __DucktapeMessage::OpenPageSearchHit(__event_0, __event_1)).clone(), (move |__event_0| __DucktapeMessage::PaletteChanged(__event_0)).clone(), (move || __DucktapeMessage::ToggleChannelCreate).clone(), (move || __DucktapeMessage::ToggleChannelCreateMembersOnly).clone()))).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __overlay_backdrop = ::iced::widget::container(::iced::widget::space()).width(::iced::Fill).height(::iced::Fill).style(move |_| ::iced::widget::container::Style { background: ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[85])), ..::iced::widget::container::Style::default() }); let __overlay_backdrop: __IceElement<'_, __DucktapeMessage> = ::iced::widget::mouse_area(__overlay_backdrop).on_press((move || __DucktapeMessage::ToggleChannelCreate)()).on_release(__DucktapeMessage::__ExternNoop).on_right_press(__DucktapeMessage::__ExternNoop).on_right_release(__DucktapeMessage::__ExternNoop).on_middle_press(__DucktapeMessage::__ExternNoop).on_middle_release(__DucktapeMessage::__ExternNoop).on_scroll(|_| __DucktapeMessage::__ExternNoop).into(); let __overlay_panel = ::iced::widget::mouse_area(__overlay_layer).on_press(__DucktapeMessage::__ExternNoop).on_release(__DucktapeMessage::__ExternNoop).on_right_press(__DucktapeMessage::__ExternNoop).on_right_release(__DucktapeMessage::__ExternNoop).on_middle_press(__DucktapeMessage::__ExternNoop).on_middle_release(__DucktapeMessage::__ExternNoop).on_scroll(|_| __DucktapeMessage::__ExternNoop); let __overlay_panel: __IceElement<'_, __DucktapeMessage> = ::iced::widget::container(__overlay_panel).width(::iced::Fill).height(::iced::Fill).padding(30.0 as f32).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center).into(); let __overlay_surface: __IceElement<'_, __DucktapeMessage> = ::iced::widget::Stack::new().width(::iced::Fill).height(::iced::Fill).push(__overlay_backdrop).push(__overlay_panel).into(); __overlay_stack.push(::iced::widget::float(__overlay_surface).translate(|_, _| ::iced::Vector::new(::core::f32::EPSILON, 0.0))).into() } else { __overlay_stack.into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!("").is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 251, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:251", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 258, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:258", __ice_use_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (move || __DucktapeMessage::DismissToast)(); let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 263, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_toast_scope_6823 = format!("{}/Toast@6823", __ice_use_scope); ::ui_lang_runtime::rev_memo(1064u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_185(__ice_palette, __ice_component_toast_scope_6823))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).padding(0.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.text_color = __ice_palette.colors[4]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((10.0) as f32).max(0.0).min(f32::MAX), top_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((10.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.text_color = __ice_palette.colors[4]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Dismiss".to_owned()).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(0.0, 0.0, 26.0, 0.0)).width(::iced::Fill).height(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Bottom) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 274, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __overlay_base: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 283, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(::iced::Fill).height(::iced::Fill).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __overlay_open = self.palette_open; let __overlay_base: __IceElement<'_, __DucktapeMessage> = if __overlay_open { ::ui_lang_runtime::focus_barrier(__overlay_base).into() } else { __overlay_base }; let __overlay_stack = ::iced::widget::Stack::new().width(::iced::Fill).height(::iced::Fill).push(__overlay_base); if __overlay_open { let __overlay_layer: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 285, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:285", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 296, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 297, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/palette-input", __ice_use_scope); { let __a11y_key = __ice_node_scope.clone(); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __secure = false; let __role = if __secure { ::ui_lang_runtime::Role::PasswordInput } else { ::ui_lang_runtime::Role::TextInput }; let __input = ::ui_lang_runtime::accessible(::iced::widget::text_input("Search messages and pages… (Esc closes)", &self.palette_draft).id(::iced::widget::Id::from(__a11y_key.clone())).padding(::iced::Padding { top: 11.0, right: 13.0, bottom: 11.0, left: 13.0 }).width(::iced::Fill).secure(__secure).width(::iced::Fill).padding(8.0 as f32).size(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)).line_height(::iced::widget::text::LineHeight::Relative(((1.2) as f32).max(f32::EPSILON).min(f32::MAX))).on_input_maybe(if __disabled { None } else { Some({ let __route_callback = (move |__event_0| __DucktapeMessage::PaletteChanged(__event_0)).clone(); move |__value| (__route_callback)(__value) }) }).on_submit_maybe(if __disabled { None } else { Some((move || __DucktapeMessage::ClosePalette)()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::text_input::default(__theme, __status); __style.background = __ice_palette.colors[3].into(); __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.0; __style.border.radius = 10.0.into(); __style.background = ::iced::Background::Color(__ice_palette.colors[6]); __style.border.color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.160000; __color }; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; __style.placeholder = __ice_palette.colors[5]; __style.value = __ice_palette.colors[4]; __style.selection = { let mut __color = __ice_palette.colors[4]; __color.a = 0.180000; __color }; if matches!(__status, ::iced::widget::text_input::Status::Focused { .. }) { __style.border.color = __ice_palette.colors[42]; } match __status { ::iced::widget::text_input::Status::Hovered => { __style.background = ::iced::Background::Color(__ice_palette.colors[55]); __style.border.color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.210000; __color }; } _ => {} } __style }), __a11y_id, __role).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Search everything".to_owned()).value_maybe((!__secure).then(|| (self.palette_draft).to_owned())).disabled(__disabled); __input.into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (self.palette_search_phase == SearchPhase::Searching) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 311, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:311", __ice_use_scope); let __text_value = ("Searching…").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)).color(__ice_palette.colors[5]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (((self.palette_search_phase == SearchPhase::Done) && (self.palette_chat_hits).is_empty()) && (self.palette_page_hits).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 319, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_0456d707479506c617465_scope_6879 = format!("{}/EmptyPlate@6879", __ice_use_scope); ::ui_lang_runtime::rev_memo(1073u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_186(__ice_palette, __ice_component_0456d707479506c617465_scope_6879))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((self.palette_search_phase == SearchPhase::Idle) && (!((self.palette_draft).trim().to_owned()).is_empty())) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 333, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_0456d707479506c617465_scope_6893 = format!("{}/EmptyPlate@6893", __ice_use_scope); ::ui_lang_runtime::rev_memo(1075u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_187(__ice_palette, __ice_component_0456d707479506c617465_scope_6893))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!(self.palette_chat_hits).is_empty()) || (!(self.palette_page_hits).is_empty())) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 340, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:340", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 341, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@layout:341", __ice_use_scope); let __scroll_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 346, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if (!(self.palette_chat_hits).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 348, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:348", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 349, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:349", __ice_use_scope); let __text_value = ("MESSAGES").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[5]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(0.0, 0.0, 0.0, 4.0)).width(::iced::Fill) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 354, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); for (__ice_index, hit) in self.palette_chat_hits.iter().enumerate() { let __for_scope = format!("{}/@for:6915({})", __ice_use_scope, __ice_index); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 356, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:356", __for_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (move |__event_0, __event_1| __DucktapeMessage::OpenChatSearchHit(__event_0, __event_1))(hit.channel_id.to_owned(), hit.seq); let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 362, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 369, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:369", __for_scope); let __text_value = (hit.text).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::WordOrGlyph).color(__ice_palette.colors[4]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 374, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:374", __for_scope); let __text_value = (hit.meta).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[5]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(1.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:362", __for_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).padding(::iced::Padding { top: 7.0, right: 12.0, bottom: 7.0, left: 12.0 }).width(::iced::Fill).padding(6.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 8.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.text_color = __ice_palette.colors[4]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((8.0) as f32).max(0.0).min(f32::MAX), top_right: ((8.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((8.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((8.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[57])); __style.text_color = __ice_palette.colors[4]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[14])); } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Open message".to_owned()).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 8.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(1.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:354", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.palette_page_hits).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 384, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:384", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 385, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:385", __ice_use_scope); let __text_value = ("PAGES").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[5]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(0.0, 0.0, 0.0, 4.0)).width(::iced::Fill) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 390, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); for (__ice_index, hit) in self.palette_page_hits.iter().enumerate() { let __for_scope = format!("{}/@for:6951({})", __ice_use_scope, __ice_index); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 392, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:392", __for_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (move |__event_0, __event_1| __DucktapeMessage::OpenPageSearchHit(__event_0, __event_1))(hit.page_id.to_owned(), hit.block_id.to_owned()); let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 398, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 401, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:401", __for_scope); let __text_value = (hit.text).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::WordOrGlyph).color(__ice_palette.colors[4]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 412, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 417, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:417", __for_scope); let __text_value = (hit.page_title).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)).width(::iced::Fill).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[5]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 423, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:423", __for_scope); let __text_value = (hit.kind).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Normal, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[5]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(7.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:412", __for_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(1.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:398", __for_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).padding(::iced::Padding { top: 7.0, right: 12.0, bottom: 7.0, left: 12.0 }).width(::iced::Fill).padding(6.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 8.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.text_color = __ice_palette.colors[4]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((8.0) as f32).max(0.0).min(f32::MAX), top_right: ((8.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((8.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((8.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[57])); __style.text_color = __ice_palette.colors[4]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[14])); } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Open page".to_owned()).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 8.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(1.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:390", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(4.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:346", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __layout = ::iced::widget::scrollable(__scroll_content).direction(::iced::widget::scrollable::Direction::Vertical(::iced::widget::scrollable::Scrollbar::new())).anchor_x(::iced::widget::scrollable::Anchor::Start).anchor_y(::iced::widget::scrollable::Anchor::Start).width(::iced::Fill).height(::iced::Shrink); ::ui_lang_runtime::accessible(__layout, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).max_height(380.0 as f32) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(8.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:296", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(10.0, 10.0, 10.0, 10.0)).width(540.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[3])); __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((14.0) as f32).max(0.0).min(f32::MAX), top_right: ((14.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((14.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((14.0) as f32).max(0.0).min(f32::MAX) }; __style.shadow.color = __ice_palette.colors[48]; __style.shadow.offset.y = 24.0 as f32; __style.shadow.blur_radius = 60.0 as f32; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __overlay_backdrop = ::iced::widget::container(::iced::widget::space()).width(::iced::Fill).height(::iced::Fill).style(move |_| ::iced::widget::container::Style { background: ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[85])), ..::iced::widget::container::Style::default() }); let __overlay_backdrop: __IceElement<'_, __DucktapeMessage> = ::iced::widget::mouse_area(__overlay_backdrop).on_press((move || __DucktapeMessage::ClosePalette)()).on_release(__DucktapeMessage::__ExternNoop).on_right_press(__DucktapeMessage::__ExternNoop).on_right_release(__DucktapeMessage::__ExternNoop).on_middle_press(__DucktapeMessage::__ExternNoop).on_middle_release(__DucktapeMessage::__ExternNoop).on_scroll(|_| __DucktapeMessage::__ExternNoop).into(); let __overlay_panel = ::iced::widget::mouse_area(__overlay_layer).on_press(__DucktapeMessage::__ExternNoop).on_release(__DucktapeMessage::__ExternNoop).on_right_press(__DucktapeMessage::__ExternNoop).on_right_release(__DucktapeMessage::__ExternNoop).on_middle_press(__DucktapeMessage::__ExternNoop).on_middle_release(__DucktapeMessage::__ExternNoop).on_scroll(|_| __DucktapeMessage::__ExternNoop); let __overlay_panel: __IceElement<'_, __DucktapeMessage> = ::iced::widget::container(__overlay_panel).width(::iced::Fill).height(::iced::Fill).padding(72.0 as f32).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Top).into(); let __overlay_surface: __IceElement<'_, __DucktapeMessage> = ::iced::widget::Stack::new().width(::iced::Fill).height(::iced::Fill).push(__overlay_backdrop).push(__overlay_panel).into(); __overlay_stack.push(::iced::widget::float(__overlay_surface).translate(|_, _| ::iced::Vector::new(::core::f32::EPSILON, 0.0))).into() } else { __overlay_stack.into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __layout = ::ui_lang_runtime::zstack(__children).width(::iced::Fill).height(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:18", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
}
}
