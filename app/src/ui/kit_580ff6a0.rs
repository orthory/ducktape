#[allow(warnings, clippy::all)]
mod __ice_group_kit_580ff6a0 {
use super::*;
impl super::Ducktape {
pub(super) fn __ice_component_use_89(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 239, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 248, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:248", __ice_use_scope); let __text_value = ("Nothing matched that search.").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)).color(__ice_palette.colors[71]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).padding(::ui_lang_runtime::bounded_padding(30.0, 30.0, 30.0, 30.0)).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((12.0) as f32).max(0.0).min(f32::MAX), top_right: ((12.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((12.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((12.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_90(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 239, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 248, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:248", __ice_use_scope); let __text_value = ("Search failed.").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)).color(__ice_palette.colors[71]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).padding(::ui_lang_runtime::bounded_padding(30.0, 30.0, 30.0, 30.0)).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((12.0) as f32).max(0.0).min(f32::MAX), top_right: ((12.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((12.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((12.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_94(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_0: crate::backend::BellItem, __ice_arg_1: crate::backend::BellPresentation) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 276, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if __ice_arg_0.read { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 286, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_92(__ice_palette, format!("{}/StillDot@2087", __ice_use_scope), crate::backend::bell_severity(::std::convert::AsRef::as_ref(&(__ice_arg_0.reason)))));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!__ice_arg_0.read) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 288, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_93(__ice_palette, format!("{}/PulseDot@2089", __ice_use_scope), crate::backend::bell_severity(::std::convert::AsRef::as_ref(&(__ice_arg_0.reason)))));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 289, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 290, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 295, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:295", __ice_use_scope); let __text_value = (__ice_arg_1.title).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)).width(::iced::Fill).wrapping(::iced::widget::text::Wrapping::None).color(__ice_palette.colors[7]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(7.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:290", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 301, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:301", __ice_use_scope); let __text_value = (__ice_arg_1.detail).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)).width(::iced::Fill).line_height(::iced::widget::text::LineHeight::Relative(((1.45) as f32).max(f32::EPSILON).min(f32::MAX))).color(__ice_palette.colors[41]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(3.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:289", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(9.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Top); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = __ice_node_scope.as_str(); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_95(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_0: crate::backend::BellItem, __ice_arg_1: crate::backend::BellPresentation) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 251, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if __ice_arg_0.read { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 253, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:253", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 262, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_94(__ice_palette, format!("{}/BellBody@2063", __ice_use_scope), __ice_arg_0.clone(), __ice_arg_1.clone()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(9.0, 9.0, 10.0, 9.0)).width(::iced::Fill).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!__ice_arg_0.read) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 264, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:264", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 273, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_94(__ice_palette, format!("{}/BellBody@2074", __ice_use_scope), __ice_arg_0.clone(), __ice_arg_1.clone()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(9.0, 9.0, 10.0, 9.0)).width(::iced::Fill).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[89])); __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = __ice_node_scope.as_str(); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_119(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 45, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 46, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:46", __ice_use_scope); let __text_value = ("Channel name").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(12.5).line_height(::iced::widget::text::LineHeight::Relative(1.5)).font(::iced::Font { weight: ::iced::font::Weight::Semibold, ..Self::default_font() }).color(__ice_palette.colors[4]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 47, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 115, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/draft", __ice_node_scope); { let __a11y_key = __ice_node_scope.clone(); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __secure = false; let __role = if __secure { ::ui_lang_runtime::Role::PasswordInput } else { ::ui_lang_runtime::Role::TextInput }; let __input = ::ui_lang_runtime::accessible(::iced::widget::text_input("general", &self.channel_draft).id(::iced::widget::Id::from(__a11y_key.clone())).padding(::iced::Padding { top: 11.0, right: 13.0, bottom: 11.0, left: 13.0 }).width(::iced::Fill).secure(__secure).on_input_maybe(if __disabled { None } else { Some(__DucktapeMessage::__BindChannelDraft as fn(::std::string::String) -> __DucktapeMessage) }).style(move |__theme, __status| { let mut __style = ::iced::widget::text_input::default(__theme, __status); __style.background = __ice_palette.colors[3].into(); __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.0; __style.border.radius = 10.0.into(); if matches!(__status, ::iced::widget::text_input::Status::Focused { .. }) { __style.border.color = __ice_palette.colors[42]; } __style }), __a11y_id, __role).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Channel name".to_owned()).value_maybe((!__secure).then(|| (self.channel_draft).to_owned())).disabled(__disabled); __input.into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 48, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:48", __ice_use_scope); let __text_value = ("Used when creating a channel.").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(12.5).line_height(::iced::widget::text::LineHeight::Relative(1.5)).color(__ice_palette.colors[5]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(8); let __content = ::iced::widget::container(__layout).width(::iced::Fill); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = __ice_node_scope.as_str(); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_120(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 14, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 22, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 23, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:23", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 31, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:31", __ice_use_scope); let __text_value = ("!").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)).font(::iced::Font { weight: ::iced::font::Weight::Semibold, ..Self::default_font() }).color(__ice_palette.colors[21]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(24.0 as f32).height(24.0 as f32).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[24])); __style.border.radius = ::iced::border::Radius { top_left: ((7.0) as f32).max(0.0).min(f32::MAX), top_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((7.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 36, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 37, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:37", __ice_use_scope); let __text_value = ("Connection failed").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)).font(::iced::Font { weight: ::iced::font::Weight::Semibold, ..Self::default_font() }).color(__ice_palette.colors[4]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 42, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:42", __ice_use_scope); let __text_value = (self.error).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)).color(__ice_palette.colors[5]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(3.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:36", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(9.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:22", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).padding(::ui_lang_runtime::bounded_padding(13.0, 13.0, 13.0, 13.0)).width(::iced::Fill).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[22])); __style.border.color = __ice_palette.colors[23]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((11.0) as f32).max(0.0).min(f32::MAX), top_right: ((11.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((11.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((11.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_121(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 90, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 98, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 99, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:99", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 105, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(6.0 as f32).height(6.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[29])); __style.border.radius = ::iced::border::Radius { top_left: ((3.0) as f32).max(0.0).min(f32::MAX), top_right: ((3.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((3.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((3.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 106, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:106", __ice_use_scope); let __text_value = ("Ready").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(9).line_height(::iced::widget::text::LineHeight::Relative(1.35)).font(::iced::Font { weight: ::iced::font::Weight::Semibold, ..Self::default_font() }).color(__ice_palette.colors[4]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(5.0, __child_count)).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:98", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).padding(::ui_lang_runtime::bounded_padding(3.0, 7.0, 3.0, 7.0)).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[27])); __style.border.color = __ice_palette.colors[28]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((5.0) as f32).max(0.0).min(f32::MAX), top_right: ((5.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((5.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((5.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_122(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 128, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 139, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:139", __ice_use_scope); let __text_value = ("Esc").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(11).line_height(::iced::widget::text::LineHeight::Relative(1.5)).font(::iced::Font { weight: ::iced::font::Weight::Medium, ..Self::default_font() }).color(__ice_palette.colors[4]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).padding(::ui_lang_runtime::bounded_padding(3.0, 7.0, 3.0, 7.0)).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[14])); __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((5.0) as f32).max(0.0).min(f32::MAX), top_right: ((5.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((5.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((5.0) as f32).max(0.0).min(f32::MAX) }; __style.shadow.color = { let mut __color = ::iced::Color::from_rgba8(0, 0, 0, 1.000000); __color.a = 0.100000; __color }; __style.shadow.offset.y = 1.0 as f32; __style.shadow.blur_radius = 2.0 as f32; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_123(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 6, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 7, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 8, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 9, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:9", __ice_use_scope); let __text_value = ("Shared components").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(16).line_height(::iced::widget::text::LineHeight::Relative(1.35)).font(::iced::Font { weight: ::iced::font::Weight::Semibold, ..Self::default_font() }).color(__ice_palette.colors[7]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 10, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:10", __ice_use_scope); let __text_value = ("The app uses the default component library.").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(12.5).line_height(::iced::widget::text::LineHeight::Relative(1.5)).color(__ice_palette.colors[5]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(8); let __content = ::iced::widget::container(__layout).width(::iced::Fill); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:8", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 11, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 141, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 142, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/alert", __ice_node_scope); { let __ice_component_0416c6572742e4465737472756374697665_scope_10756 = __ice_node_scope.clone(); ::ui_lang_runtime::rev_memo(1182u64, (self.__ice_rev[20], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_120(__ice_palette, __ice_component_0416c6572742e4465737472756374697665_scope_10756))).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 143, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 148, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/badge", __ice_node_scope); { let __ice_component_042616467652e53756363657373_scope_10762 = __ice_node_scope.clone(); ::ui_lang_runtime::rev_memo(1184u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_121(__ice_palette, __ice_component_042616467652e53756363657373_scope_10762))).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 149, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/kbd", __ice_node_scope); { let __ice_component_kbd_scope_10763 = __ice_node_scope.clone(); ::ui_lang_runtime::rev_memo(1185u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_122(__ice_palette, __ice_component_kbd_scope_10763))).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(8.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:143", __ice_node_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 150, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/dismiss", __ice_node_scope); { let __a11y_key = __ice_node_scope.clone(); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = __DucktapeMessage::DismissError; let __button_content: __IceElement<'_, __DucktapeMessage> = ::iced::widget::text("Dismiss").size(12.5).font(::iced::Font { weight: ::iced::font::Weight::Semibold, ..Self::default_font() }).into(); let __button = ::iced::widget::button(__button_content).padding(::iced::Padding { top: 11.0, right: 16.0, bottom: 11.0, left: 16.0 }).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[8]), ::iced::widget::button::Status::Pressed => Some({ let mut __color = __ice_palette.colors[7]; __color.a = 0.800000; __color }), ::iced::widget::button::Status::Disabled => Some(__ice_palette.colors[7]), _ => Some(__ice_palette.colors[7]) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[9]; __style.border.radius = 9.0.into(); if matches!(__status, ::iced::widget::button::Status::Disabled) { __style.background = Some(__ice_palette.colors[10].into()); __style.text_color = __ice_palette.colors[11]; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Dismiss").disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 9.0).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(12.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:141", __ice_node_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(12); let __content = ::iced::widget::container(__layout).width(::iced::Fill); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:7", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).padding(::iced::Padding { top: 20.0, right: 20.0, bottom: 20.0, left: 20.0 }).width(::iced::Fill).clip(true).style(move |__theme| { let mut __style = ::iced::widget::container::Style { background: Some(__ice_palette.colors[3].into()), text_color: None, border: ::iced::Border { color: __ice_palette.colors[39], width: 1.0, radius: 11.0.into() }, ..::iced::widget::container::Style::default() }; __style.border.radius = ::iced::border::Radius { top_left: ((11.0) as f32).max(0.0).min(f32::MAX), top_right: ((11.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((11.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((11.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_186(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 239, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 248, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:248", __ice_use_scope); let __text_value = ("Nothing matched that search.").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)).color(__ice_palette.colors[71]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).padding(::ui_lang_runtime::bounded_padding(30.0, 30.0, 30.0, 30.0)).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((12.0) as f32).max(0.0).min(f32::MAX), top_right: ((12.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((12.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((12.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_187(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 239, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/kit.ice", 248, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:248", __ice_use_scope); let __text_value = ("Search failed.").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)).color(__ice_palette.colors[71]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).padding(::ui_lang_runtime::bounded_padding(30.0, 30.0, 30.0, 30.0)).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((12.0) as f32).max(0.0).min(f32::MAX), top_right: ((12.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((12.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((12.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
}
}
