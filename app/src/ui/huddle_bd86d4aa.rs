#[allow(warnings, clippy::all)]
mod __ice_group_huddle_bd86d4aa {
use super::*;
impl super::Ducktape {
pub(super) fn __ice_component_use_53(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_0: crate::backend::HuddleParticipant, __ice_arg_1: bool) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 157, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 170, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 175, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_51(__ice_palette, format!("{}/PrincipalAvatar@6120", __ice_use_scope), __ice_arg_0.initials.to_owned(), __ice_arg_0.is_agent));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 182, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:182", __ice_use_scope); let __text_value = (__ice_arg_0.label).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[4]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (__ice_arg_0.is_you || __ice_arg_1) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 195, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if __ice_arg_1 { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 197, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_52(__ice_palette, format!("{}/Icon@6142", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if __ice_arg_0.is_you { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 203, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:203", __ice_use_scope); let __text_value = ("you").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[70]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(4.0, __child_count)).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:195", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(7.0, __child_count)).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:170", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).padding(::ui_lang_runtime::bounded_padding(12.0, 8.0, 12.0, 8.0)).width(::iced::Fill).height(::iced::Fill).align_y(::iced::alignment::Vertical::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[55])); __style.border.color = __ice_palette.colors[63]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((11.0) as f32).max(0.0).min(f32::MAX), top_right: ((11.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((11.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((11.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_61(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_cb_0: impl Fn() -> __DucktapeMessage + Clone + 'static, __ice_cb_1: impl Fn() -> __DucktapeMessage + Clone + 'static, __ice_cb_2: impl Fn() -> __DucktapeMessage + Clone + 'static, __ice_cb_3: impl Fn() -> __DucktapeMessage + Clone + 'static, __ice_cb_4: impl Fn() -> __DucktapeMessage + Clone + 'static) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 221, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 222, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:222", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 227, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).height(1.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[60])); __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 234, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:234", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 242, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if (!self.call_muted) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 248, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:248", __ice_use_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_3)(); let __button_content: __IceElement<'_, __DucktapeMessage> = { let __button_inner: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 257, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:257", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 263, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_54(__ice_palette, format!("{}/Icon@6208", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).height(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::container(__button_inner).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).height(::iced::Fill).align_y(::iced::alignment::Vertical::Center).into() }; let __button = ::iced::widget::button(__button_content).width(32.0 as f32).height(32.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[55])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[40]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[94]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Mute the microphone".to_owned()).checked(self.call_muted).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if self.call_muted { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 272, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:272", __ice_use_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_3)(); let __button_content: __IceElement<'_, __DucktapeMessage> = { let __button_inner: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 281, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:281", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 287, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_55(__ice_palette, format!("{}/Icon@6232", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).height(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::container(__button_inner).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).height(::iced::Fill).align_y(::iced::alignment::Vertical::Center).into() }; let __button = ::iced::widget::button(__button_content).width(32.0 as f32).height(32.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[22])); __style.text_color = __ice_palette.colors[20]; __style.border.color = __ice_palette.colors[23]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[22])); __style.text_color = __ice_palette.colors[20]; __style.border.color = __ice_palette.colors[20]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[22])); __style.text_color = __ice_palette.colors[20]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Unmute the microphone".to_owned()).checked(self.call_muted).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!self.call_camera) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 296, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:296", __ice_use_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_2)(); let __button_content: __IceElement<'_, __DucktapeMessage> = { let __button_inner: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 305, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:305", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 311, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_56(__ice_palette, format!("{}/Icon@6256", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).height(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::container(__button_inner).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).height(::iced::Fill).align_y(::iced::alignment::Vertical::Center).into() }; let __button = ::iced::widget::button(__button_content).width(32.0 as f32).height(32.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[55])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[40]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[94]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Turn the camera on".to_owned()).checked(self.call_camera).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if self.call_camera { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 320, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:320", __ice_use_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_2)(); let __button_content: __IceElement<'_, __DucktapeMessage> = { let __button_inner: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 329, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:329", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 335, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_57(__ice_palette, format!("{}/Icon@6280", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).height(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::container(__button_inner).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).height(::iced::Fill).align_y(::iced::alignment::Vertical::Center).into() }; let __button = ::iced::widget::button(__button_content).width(32.0 as f32).height(32.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[94]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[94]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Turn the camera off".to_owned()).checked(self.call_camera).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!self.call_sharing) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 352, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/share", __ice_node_scope); { let __a11y_key = __ice_node_scope.clone(); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_4)(); let __button_content: __IceElement<'_, __DucktapeMessage> = { let __button_inner: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 361, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:361", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 367, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_58(__ice_palette, format!("{}/Icon@6312", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).height(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::container(__button_inner).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).height(::iced::Fill).align_y(::iced::alignment::Vertical::Center).into() }; let __button = ::iced::widget::button(__button_content).width(32.0 as f32).height(32.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[55])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[40]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[94]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Share this screen".to_owned()).checked(self.call_sharing).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if self.call_sharing { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 376, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/share-stop", __ice_node_scope); { let __a11y_key = __ice_node_scope.clone(); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_4)(); let __button_content: __IceElement<'_, __DucktapeMessage> = { let __button_inner: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 385, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:385", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 391, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_59(__ice_palette, format!("{}/Icon@6336", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).height(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::container(__button_inner).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).height(::iced::Fill).align_y(::iced::alignment::Vertical::Center).into() }; let __button = ::iced::widget::button(__button_content).width(32.0 as f32).height(32.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[94]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[94]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Stop sharing this screen".to_owned()).checked(self.call_sharing).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 403, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:403", __ice_use_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_0)(); let __button_content: __IceElement<'_, __DucktapeMessage> = { let __button_inner: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 411, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:411", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 417, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_60(__ice_palette, format!("{}/Icon@6362", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).height(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::container(__button_inner).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).height(::iced::Fill).align_y(::iced::alignment::Vertical::Center).into() }; let __button = ::iced::widget::button(__button_content).width(32.0 as f32).height(32.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[55])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[40]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[94]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Open the huddle channel".to_owned()).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 425, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(::iced::Fill).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 426, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/leave", __ice_node_scope); { let __a11y_key = __ice_node_scope.clone(); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_1)(); let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 432, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:432", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 433, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:433", __ice_use_scope); let __text_value = ("Leave").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[9]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).height(32.0 as f32).align_y(::iced::alignment::Vertical::Center) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).padding(::iced::Padding { top: 0.0, right: 13.0, bottom: 0.0, left: 13.0 }).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[66])); __style.text_color = __ice_palette.colors[9]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[67])); __style.text_color = __ice_palette.colors[9]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[67])); __style.text_color = __ice_palette.colors[9]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Leave the huddle".to_owned()).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(7.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:242", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(10.0, 12.0, 10.0, 12.0)).width(::iced::Fill).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[54])); __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = __ice_node_scope.as_str(); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_62(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 486, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 492, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:492", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 500, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 505, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_050756c7365446f74_scope_6450 = format!("{}/PulseDot@6450", __ice_use_scope); ::ui_lang_runtime::rev_memo(994u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_42(__ice_palette, __ice_component_050756c7365446f74_scope_6450))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!(crate::backend::mmss((self.huddle_now - self.huddle_joined_at))).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 507, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:507", __ice_use_scope); let __text_value = (crate::backend::mmss((self.huddle_now - self.huddle_joined_at))).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[4]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (crate::backend::mmss((self.huddle_now - self.huddle_joined_at))).is_empty() { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 514, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:514", __ice_use_scope); let __text_value = ("LIVE").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[4]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 526, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:526", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 527, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 528, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:528", __ice_use_scope); let __text_value = ("#").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[72]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 534, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:534", __ice_use_scope); let __text_value = (self.huddle_channel_name).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[5]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(3.0, __child_count)).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:527", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).clip(true) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(8.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:500", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(9.0, 9.0, 9.0, 13.0)).width(::iced::Fill).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[54])); __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 540, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:540", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 545, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).height(1.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[60])); __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if ((!(self.call_status).is_empty()) && (self.call_status != "live")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 551, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:551", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 559, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:559", __ice_use_scope); let __text_value = (self.call_status).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).width(::iced::Fill).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[70]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(7.0, 13.0, 7.0, 13.0)).width(::iced::Fill).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 571, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@layout:571", __ice_use_scope); let __scroll_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 576, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if (!(self.huddle_stage).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 590, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = crate::video::call_video_stage(::std::convert::AsRef::as_ref(&(self.huddle_stage))).map(move |__value| __DucktapeMessage::__ExternNoop).into();
#[cfg(test)]
let __ice_rendered = ::ui_lang_runtime::testing::sourced(__ice_rendered, __ice_render_source_location);
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if self.call_video_live { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 596, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = crate::video::call_video_tiles(::std::convert::AsRef::as_ref(&(self.huddle_stage))).map(move |__value| __DucktapeMessage::__ExternNoop).into();
#[cfg(test)]
let __ice_rendered = ::ui_lang_runtime::testing::sourced(__ice_rendered, __ice_render_source_location);
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 602, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); for (__ice_index, tile) in self.huddle_rows.iter().enumerate() { let __for_scope = format!("{}/@for:6548({})", __ice_use_scope, __ice_index); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 604, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_53(__ice_palette, format!("{}/HuddleTile@6549", __for_scope), tile.person.clone(), tile.muted));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __layout = ::iced::widget::grid(__children).spacing(::ui_lang_runtime::bounded_spacing(8.0, __child_count)).fluid(((168.0) as f32).max(f32::EPSILON).min(f32::MAX)); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:602", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(10.0, __child_count)).padding(::ui_lang_runtime::bounded_padding(12.0, 12.0, 12.0, 12.0)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:576", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __layout = ::iced::widget::scrollable(__scroll_content).direction(::iced::widget::scrollable::Direction::Vertical(::iced::widget::scrollable::Scrollbar::new())).anchor_x(::iced::widget::scrollable::Anchor::Start).anchor_y(::iced::widget::scrollable::Anchor::Start).width(::iced::Fill).height(::iced::Fill); ::ui_lang_runtime::accessible(__layout, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 605, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/controls", __ice_node_scope); ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_61(__ice_palette, __ice_node_scope.clone(), (move || __DucktapeMessage::HuddleGoChannel).clone(), (move || __DucktapeMessage::LeaveHuddleHere).clone(), (move || __DucktapeMessage::ToggleCallCamera).clone(), (move || __DucktapeMessage::ToggleCallMute).clone(), (move || __DucktapeMessage::ToggleCallScreen).clone())) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).width(::iced::Fill).height(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = __ice_node_scope.as_str(); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_199(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_0: crate::backend::HuddleParticipant, __ice_arg_1: bool) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 157, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 170, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 175, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_197(__ice_palette, format!("{}/PrincipalAvatar@6120", __ice_use_scope), __ice_arg_0.initials.to_owned(), __ice_arg_0.is_agent));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 182, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:182", __ice_use_scope); let __text_value = (__ice_arg_0.label).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[4]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (__ice_arg_0.is_you || __ice_arg_1) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 195, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if __ice_arg_1 { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 197, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_198(__ice_palette, format!("{}/Icon@6142", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if __ice_arg_0.is_you { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 203, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:203", __ice_use_scope); let __text_value = ("you").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[70]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(4.0, __child_count)).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:195", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(7.0, __child_count)).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:170", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).padding(::ui_lang_runtime::bounded_padding(12.0, 8.0, 12.0, 8.0)).width(::iced::Fill).height(::iced::Fill).align_y(::iced::alignment::Vertical::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[55])); __style.border.color = __ice_palette.colors[63]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((11.0) as f32).max(0.0).min(f32::MAX), top_right: ((11.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((11.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((11.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_207(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_cb_0: impl Fn() -> __DucktapeMessage + Clone + 'static, __ice_cb_1: impl Fn() -> __DucktapeMessage + Clone + 'static, __ice_cb_2: impl Fn() -> __DucktapeMessage + Clone + 'static, __ice_cb_3: impl Fn() -> __DucktapeMessage + Clone + 'static, __ice_cb_4: impl Fn() -> __DucktapeMessage + Clone + 'static) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 221, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 222, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:222", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 227, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).height(1.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[60])); __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 234, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:234", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 242, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if (!self.call_muted) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 248, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:248", __ice_use_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_3)(); let __button_content: __IceElement<'_, __DucktapeMessage> = { let __button_inner: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 257, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:257", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 263, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_200(__ice_palette, format!("{}/Icon@6208", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).height(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::container(__button_inner).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).height(::iced::Fill).align_y(::iced::alignment::Vertical::Center).into() }; let __button = ::iced::widget::button(__button_content).width(32.0 as f32).height(32.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[55])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[40]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[94]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Mute the microphone".to_owned()).checked(self.call_muted).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if self.call_muted { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 272, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:272", __ice_use_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_3)(); let __button_content: __IceElement<'_, __DucktapeMessage> = { let __button_inner: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 281, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:281", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 287, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_201(__ice_palette, format!("{}/Icon@6232", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).height(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::container(__button_inner).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).height(::iced::Fill).align_y(::iced::alignment::Vertical::Center).into() }; let __button = ::iced::widget::button(__button_content).width(32.0 as f32).height(32.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[22])); __style.text_color = __ice_palette.colors[20]; __style.border.color = __ice_palette.colors[23]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[22])); __style.text_color = __ice_palette.colors[20]; __style.border.color = __ice_palette.colors[20]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[22])); __style.text_color = __ice_palette.colors[20]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Unmute the microphone".to_owned()).checked(self.call_muted).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!self.call_camera) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 296, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:296", __ice_use_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_2)(); let __button_content: __IceElement<'_, __DucktapeMessage> = { let __button_inner: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 305, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:305", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 311, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_202(__ice_palette, format!("{}/Icon@6256", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).height(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::container(__button_inner).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).height(::iced::Fill).align_y(::iced::alignment::Vertical::Center).into() }; let __button = ::iced::widget::button(__button_content).width(32.0 as f32).height(32.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[55])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[40]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[94]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Turn the camera on".to_owned()).checked(self.call_camera).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if self.call_camera { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 320, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:320", __ice_use_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_2)(); let __button_content: __IceElement<'_, __DucktapeMessage> = { let __button_inner: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 329, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:329", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 335, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_203(__ice_palette, format!("{}/Icon@6280", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).height(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::container(__button_inner).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).height(::iced::Fill).align_y(::iced::alignment::Vertical::Center).into() }; let __button = ::iced::widget::button(__button_content).width(32.0 as f32).height(32.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[94]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[94]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Turn the camera off".to_owned()).checked(self.call_camera).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!self.call_sharing) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 352, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/share", __ice_node_scope); { let __a11y_key = __ice_node_scope.clone(); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_4)(); let __button_content: __IceElement<'_, __DucktapeMessage> = { let __button_inner: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 361, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:361", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 367, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_204(__ice_palette, format!("{}/Icon@6312", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).height(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::container(__button_inner).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).height(::iced::Fill).align_y(::iced::alignment::Vertical::Center).into() }; let __button = ::iced::widget::button(__button_content).width(32.0 as f32).height(32.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[55])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[40]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[94]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Share this screen".to_owned()).checked(self.call_sharing).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if self.call_sharing { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 376, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/share-stop", __ice_node_scope); { let __a11y_key = __ice_node_scope.clone(); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_4)(); let __button_content: __IceElement<'_, __DucktapeMessage> = { let __button_inner: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 385, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:385", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 391, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_205(__ice_palette, format!("{}/Icon@6336", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).height(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::container(__button_inner).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).height(::iced::Fill).align_y(::iced::alignment::Vertical::Center).into() }; let __button = ::iced::widget::button(__button_content).width(32.0 as f32).height(32.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[94]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[94]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Stop sharing this screen".to_owned()).checked(self.call_sharing).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 403, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:403", __ice_use_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_0)(); let __button_content: __IceElement<'_, __DucktapeMessage> = { let __button_inner: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 411, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:411", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 417, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_206(__ice_palette, format!("{}/Icon@6362", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).height(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::container(__button_inner).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).height(::iced::Fill).align_y(::iced::alignment::Vertical::Center).into() }; let __button = ::iced::widget::button(__button_content).width(32.0 as f32).height(32.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[55])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[40]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[94]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Open the huddle channel".to_owned()).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 425, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(::iced::Fill).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 426, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/leave", __ice_node_scope); { let __a11y_key = __ice_node_scope.clone(); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_1)(); let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 432, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:432", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 433, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:433", __ice_use_scope); let __text_value = ("Leave").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[9]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).height(32.0 as f32).align_y(::iced::alignment::Vertical::Center) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).padding(::iced::Padding { top: 0.0, right: 13.0, bottom: 0.0, left: 13.0 }).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[66])); __style.text_color = __ice_palette.colors[9]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[67])); __style.text_color = __ice_palette.colors[9]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[67])); __style.text_color = __ice_palette.colors[9]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Leave the huddle".to_owned()).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(7.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:242", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(10.0, 12.0, 10.0, 12.0)).width(::iced::Fill).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[54])); __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = __ice_node_scope.as_str(); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_208(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 486, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 492, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:492", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 500, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 505, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_050756c7365446f74_scope_6450 = format!("{}/PulseDot@6450", __ice_use_scope); ::ui_lang_runtime::rev_memo(994u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_110(__ice_palette, __ice_component_050756c7365446f74_scope_6450))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!("01:20").is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 507, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:507", __ice_use_scope); let __text_value = ("01:20").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[4]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ("01:20").is_empty() { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 514, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:514", __ice_use_scope); let __text_value = ("LIVE").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[4]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 526, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:526", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 527, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 528, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:528", __ice_use_scope); let __text_value = ("#").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[72]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 534, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:534", __ice_use_scope); let __text_value = (self.huddle_channel_name).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[5]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(3.0, __child_count)).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:527", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).clip(true) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(8.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:500", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(9.0, 9.0, 9.0, 13.0)).width(::iced::Fill).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[54])); __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 540, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:540", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 545, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).height(1.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[60])); __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if ((!(self.call_status).is_empty()) && (self.call_status != "live")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 551, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:551", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 559, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:559", __ice_use_scope); let __text_value = (self.call_status).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).width(::iced::Fill).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[70]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(7.0, 13.0, 7.0, 13.0)).width(::iced::Fill).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 571, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@layout:571", __ice_use_scope); let __scroll_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 576, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if (!(self.huddle_stage).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 590, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = crate::video::call_video_stage(::std::convert::AsRef::as_ref(&(self.huddle_stage))).map(move |__value| __DucktapeMessage::__ExternNoop).into();
#[cfg(test)]
let __ice_rendered = ::ui_lang_runtime::testing::sourced(__ice_rendered, __ice_render_source_location);
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if self.call_video_live { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 596, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = crate::video::call_video_tiles(::std::convert::AsRef::as_ref(&(self.huddle_stage))).map(move |__value| __DucktapeMessage::__ExternNoop).into();
#[cfg(test)]
let __ice_rendered = ::ui_lang_runtime::testing::sourced(__ice_rendered, __ice_render_source_location);
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 602, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); for (__ice_index, tile) in self.huddle_rows.iter().enumerate() { let __for_scope = format!("{}/@for:6548({})", __ice_use_scope, __ice_index); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 604, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_199(__ice_palette, format!("{}/HuddleTile@6549", __for_scope), tile.person.clone(), tile.muted));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __layout = ::iced::widget::grid(__children).spacing(::ui_lang_runtime::bounded_spacing(8.0, __child_count)).fluid(((168.0) as f32).max(f32::EPSILON).min(f32::MAX)); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:602", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(10.0, __child_count)).padding(::ui_lang_runtime::bounded_padding(12.0, 12.0, 12.0, 12.0)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:576", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __layout = ::iced::widget::scrollable(__scroll_content).direction(::iced::widget::scrollable::Direction::Vertical(::iced::widget::scrollable::Scrollbar::new())).anchor_x(::iced::widget::scrollable::Anchor::Start).anchor_y(::iced::widget::scrollable::Anchor::Start).width(::iced::Fill).height(::iced::Fill); ::ui_lang_runtime::accessible(__layout, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/huddle.ice", 605, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/controls", __ice_node_scope); ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_207(__ice_palette, __ice_node_scope.clone(), (move || __DucktapeMessage::HuddleGoChannel).clone(), (move || __DucktapeMessage::LeaveHuddleHere).clone(), (move || __DucktapeMessage::ToggleCallCamera).clone(), (move || __DucktapeMessage::ToggleCallMute).clone(), (move || __DucktapeMessage::ToggleCallScreen).clone())) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).width(::iced::Fill).height(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = __ice_node_scope.as_str(); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
}
}
